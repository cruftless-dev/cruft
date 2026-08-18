
use crate::types::PgError;
use sql_core::SqlValue;

fn does_not_exist(name: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("function {name}(...) does not exist"),
    }
}

struct Parts {
    year: i64,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,

    micros: u32,
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let m = m as i64;
    let d = d as i64;
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn parse_source(s: &str) -> Option<Parts> {
    let s = s.trim();
    let (date, time): (&str, Option<&str>) = if let Some((d, t)) = s.split_once('T') {
        (d, Some(t))
    } else if let Some((d, t)) = s.split_once(' ') {
        (d, Some(t.trim_start()))
    } else {
        (s, None)
    };

    let mut dit = date.splitn(3, '-');
    let (y, mo, d) = (dit.next()?, dit.next()?, dit.next()?);
    if dit.next().is_some() {
        return None;
    }
    let year: i64 = digits_i64(y)?;
    let month: u32 = digits_u32(mo)?;
    let day: u32 = digits_u32(d)?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    let (hour, minute, second, micros) = match time {
        None => (0, 0, 0, 0),
        Some(t) => {
            let (hms, frac) = match t.split_once('.') {
                Some((h, f)) => (h, Some(f)),
                None => (t, None),
            };
            let mut tit = hms.split(':');
            let h = tit.next()?;
            let mi = tit.next()?;
            let se = tit.next();
            if tit.next().is_some() {
                return None;
            }
            let hour = digits_u32(h)?;
            let minute = digits_u32(mi)?;
            let second = match se {
                Some(ss) => digits_u32(ss)?,
                None => 0,
            };
            let micros = match frac {
                None => 0,
                Some(f) => {
                    if f.is_empty() || !f.bytes().all(|b| b.is_ascii_digit()) {
                        return None;
                    }
                    let bytes = f.as_bytes();
                    let mut m: u32 = 0;
                    for i in 0..6 {
                        let digit = if i < bytes.len() {
                            (bytes[i] - b'0') as u32
                        } else {
                            0
                        };
                        m = m * 10 + digit;
                    }
                    m
                }
            };
            if hour > 23 || minute > 59 || second > 59 {
                return None;
            }
            (hour, minute, second, micros)
        }
    };

    Some(Parts {
        year,
        month,
        day,
        hour,
        minute,
        second,
        micros,
    })
}

fn digits_i64(s: &str) -> Option<i64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn digits_u32(s: &str) -> Option<u32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn dow(p: &Parts) -> i64 {
    (days_from_civil(p.year, p.month, p.day).rem_euclid(7) + 4) % 7
}

fn doy(p: &Parts) -> i64 {
    days_from_civil(p.year, p.month, p.day) - days_from_civil(p.year, 1, 1) + 1
}

fn isodow(p: &Parts) -> i64 {
    match dow(p) {
        0 => 7,
        n => n,
    }
}

fn iso_weeks_in_year(y: i64) -> i64 {
    fn jan1_dow(y: i64) -> i64 {
        (y + y.div_euclid(4) - y.div_euclid(100) + y.div_euclid(400)).rem_euclid(7)
    }
    if jan1_dow(y) == 4 || jan1_dow(y - 1) == 3 {
        53
    } else {
        52
    }
}

fn iso_week_year(p: &Parts) -> (i64, i64) {
    let week = (doy(p) - isodow(p) + 10) / 7;
    if week < 1 {
        (p.year - 1, iso_weeks_in_year(p.year - 1))
    } else if week > iso_weeks_in_year(p.year) {
        (p.year + 1, 1)
    } else {
        (p.year, week)
    }
}

fn epoch(p: &Parts) -> f64 {
    let days = days_from_civil(p.year, p.month, p.day);
    let whole = days * 86_400 + (p.hour as i64) * 3600 + (p.minute as i64) * 60 + p.second as i64;
    whole as f64 + p.micros as f64 / 1_000_000.0
}

fn canonical(y: i64, mo: u32, d: u32, h: u32, mi: u32, s: u32, micros: u32) -> String {
    let base = format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}");
    if micros == 0 {
        base
    } else {
        let frac = format!("{micros:06}");
        format!("{base}.{}", frac.trim_end_matches('0'))
    }
}

fn date_part(field: &str, p: &Parts) -> Result<SqlValue, PgError> {
    let v: f64 = match field {
        "year" => p.year as f64,
        "month" => p.month as f64,
        "day" => p.day as f64,
        "hour" => p.hour as f64,
        "minute" => p.minute as f64,
        "second" => p.second as f64 + p.micros as f64 / 1_000_000.0,
        "dow" => dow(p) as f64,
        "doy" => doy(p) as f64,
        "quarter" => ((p.month - 1) / 3 + 1) as f64,
        "epoch" => epoch(p),

        "isodow" => isodow(p) as f64,
        "isoyear" => iso_week_year(p).0 as f64,
        "week" => iso_week_year(p).1 as f64,

        "decade" => p.year.div_euclid(10) as f64,
        "century" => (p.year + 99).div_euclid(100) as f64,
        "millennium" => (p.year + 999).div_euclid(1000) as f64,

        "microseconds" => (p.second as f64) * 1_000_000.0 + p.micros as f64,
        "milliseconds" => (p.second as f64) * 1000.0 + p.micros as f64 / 1000.0,

        other => {
            return Err(PgError::InvalidInputSyntax {
                typ: "date_part",
                input: other.to_string(),
            })
        }
    };
    Ok(SqlValue::Real(v))
}

fn date_trunc(field: &str, p: &Parts) -> Result<SqlValue, PgError> {

    let (y, mo, d, h, mi, s) = (p.year, p.month, p.day, p.hour, p.minute, p.second);
    let text = match field {

        "microseconds" => canonical(y, mo, d, h, mi, s, p.micros),
        "milliseconds" => canonical(y, mo, d, h, mi, s, p.micros / 1000 * 1000),
        "second" => canonical(y, mo, d, h, mi, s, 0),
        "minute" => canonical(y, mo, d, h, mi, 0, 0),
        "hour" => canonical(y, mo, d, h, 0, 0, 0),
        "day" => canonical(y, mo, d, 0, 0, 0, 0),
        "week" => {

            let isodow = match dow(p) {
                0 => 7,
                n => n,
            };
            let monday = days_from_civil(y, mo, d) - (isodow - 1);
            let (wy, wmo, wd) = civil_from_days(monday);
            canonical(wy, wmo, wd, 0, 0, 0, 0)
        }
        "month" => canonical(y, mo, 1, 0, 0, 0, 0),
        "quarter" => canonical(y, (mo - 1) / 3 * 3 + 1, 1, 0, 0, 0, 0),
        "year" => canonical(y, 1, 1, 0, 0, 0, 0),

        "decade" => canonical(y - y.rem_euclid(10), 1, 1, 0, 0, 0, 0),
        "century" => canonical((y - 1).div_euclid(100) * 100 + 1, 1, 1, 0, 0, 0, 0),
        "millennium" => canonical((y - 1).div_euclid(1000) * 1000 + 1, 1, 1, 0, 0, 0, 0),
        other => {
            return Err(PgError::InvalidInputSyntax {
                typ: "date_trunc",
                input: other.to_string(),
            })
        }
    };
    Ok(SqlValue::Text(text))
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    if name != "date_part" && name != "date_trunc" {
        return None;
    }

    if args.len() != 2 {
        return Some(Err(does_not_exist(name)));
    }

    if matches!(args[0], SqlValue::Null) || matches!(args[1], SqlValue::Null) {
        return Some(Ok(SqlValue::Null));
    }

    let field = match &args[0] {
        SqlValue::Text(f) => f.as_str(),
        _ => return Some(Err(does_not_exist(name))),
    };
    let source = match &args[1] {
        SqlValue::Text(s) => s.as_str(),
        _ => return Some(Err(does_not_exist(name))),
    };
    let parts = match parse_source(source) {
        Some(p) => p,
        None => return Some(Err(does_not_exist(name))),
    };

    let field_lc = field.to_ascii_lowercase();
    Some(if name == "date_part" {
        date_part(&field_lc, &parts)
    } else {
        date_trunc(&field_lc, &parts)
    })
}

#[cfg(test)]
mod tests {
    use super::call;
    use crate::types::PgError;
    use sql_core::SqlValue;

    fn t(s: &str) -> SqlValue {
        SqlValue::Text(s.to_string())
    }

    fn part(field: &str, src: &str) -> f64 {
        match call("date_part", &[t(field), t(src)]) {
            Some(Ok(SqlValue::Real(r))) => r,
            other => panic!("expected Real, got {other:?}"),
        }
    }

    fn trunc(field: &str, src: &str) -> String {
        match call("date_trunc", &[t(field), t(src)]) {
            Some(Ok(SqlValue::Text(s))) => s,
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn part_calendar_fields() {
        let ts = "1997-02-10 17:32:01";
        assert_eq!(part("year", ts), 1997.0);
        assert_eq!(part("month", ts), 2.0);
        assert_eq!(part("day", ts), 10.0);
        assert_eq!(part("hour", ts), 17.0);
        assert_eq!(part("minute", ts), 32.0);
        assert_eq!(part("second", ts), 1.0);
    }

    #[test]
    fn part_fractional_second_is_float() {

        assert_eq!(part("second", "1997-02-10 17:32:01.4"), 1.4);
    }

    #[test]
    fn part_dow_doy_quarter() {

        assert_eq!(part("dow", "1997-02-10 17:32:01"), 1.0);
        assert_eq!(part("doy", "1997-02-10 17:32:01"), 41.0);
        assert_eq!(part("dow", "1970-01-01 00:00:00"), 4.0);
        assert_eq!(part("doy", "1970-01-01 00:00:00"), 1.0);

        assert_eq!(part("dow", "1997-02-16 17:32:01"), 0.0);

        assert_eq!(part("quarter", "1997-02-10 17:32:01"), 1.0);
        assert_eq!(part("quarter", "2001-09-22 18:19:20"), 3.0);
    }

    #[test]
    fn part_epoch() {

        assert_eq!(part("epoch", "1970-01-01 00:00:00"), 0.0);
        assert_eq!(part("epoch", "1997-02-10 17:32:01"), 855_595_921.0);
        assert_eq!(part("epoch", "2001-09-22 18:19:20"), 1_001_182_760.0);
        assert_eq!(part("epoch", "1997-02-10 17:32:01.4"), 855_595_921.4);
    }

    #[test]
    fn part_date_only_source_is_midnight() {
        assert_eq!(part("hour", "1997-02-10"), 0.0);
        assert_eq!(part("day", "1997-02-10"), 10.0);
    }

    #[test]
    fn part_field_is_case_insensitive() {
        assert_eq!(part("YEAR", "1997-02-10 17:32:01"), 1997.0);
    }

    #[test]
    fn trunc_levels() {
        let ts = "2004-02-29 15:44:17.71393";
        assert_eq!(trunc("second", ts), "2004-02-29 15:44:17");
        assert_eq!(trunc("minute", ts), "2004-02-29 15:44:00");
        assert_eq!(trunc("hour", ts), "2004-02-29 15:00:00");
        assert_eq!(trunc("day", ts), "2004-02-29 00:00:00");
        assert_eq!(trunc("month", ts), "2004-02-01 00:00:00");
        assert_eq!(trunc("year", ts), "2004-01-01 00:00:00");
        assert_eq!(trunc("quarter", ts), "2004-01-01 00:00:00");
    }

    #[test]
    fn trunc_week_to_monday() {

        assert_eq!(
            trunc("week", "2004-02-29 15:44:17.71393"),
            "2004-02-23 00:00:00"
        );
    }

    #[test]
    fn unknown_field_errors() {
        match call("date_part", &[t("nonesuch"), t("1997-02-10 17:32:01")]) {
            Some(Err(PgError::InvalidInputSyntax { typ, input })) => {
                assert_eq!(typ, "date_part");
                assert_eq!(input, "nonesuch");
            }
            other => panic!("expected InvalidInputSyntax, got {other:?}"),
        }
    }

    #[test]
    fn null_propagates() {
        assert!(matches!(
            call("date_part", &[SqlValue::Null, t("1997-02-10 17:32:01")]),
            Some(Ok(SqlValue::Null))
        ));
        assert!(matches!(
            call("date_part", &[t("year"), SqlValue::Null]),
            Some(Ok(SqlValue::Null))
        ));
        assert!(matches!(
            call("date_trunc", &[t("day"), SqlValue::Null]),
            Some(Ok(SqlValue::Null))
        ));
    }

    #[test]
    fn wrong_arity_is_does_not_exist() {
        match call("date_part", &[t("year")]) {
            Some(Err(PgError::InvalidInputSyntax { typ, input })) => {
                assert_eq!(typ, "expression");
                assert_eq!(input, "function date_part(...) does not exist");
            }
            other => panic!("expected does-not-exist, got {other:?}"),
        }
    }

    #[test]
    fn wrong_type_source_is_does_not_exist() {
        match call("date_part", &[t("year"), SqlValue::Int(5)]) {
            Some(Err(PgError::InvalidInputSyntax { typ, .. })) => assert_eq!(typ, "expression"),
            other => panic!("expected does-not-exist, got {other:?}"),
        }
    }

    #[test]
    fn unclaimed_name_returns_none() {
        assert!(call("age", &[t("x"), t("y")]).is_none());
        assert!(call("extract", &[t("year"), t("1997-02-10")]).is_none());
    }
}
