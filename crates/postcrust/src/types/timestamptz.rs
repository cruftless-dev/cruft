
use super::PgError;
use sql_core::SqlValue;

const TYP: &str = "timestamp with time zone";

fn invalid(text: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: TYP,
        input: text.to_string(),
    }
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
            if leap {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = m as i64;
    let d = d as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

fn parse_date(s: &str) -> Option<(i64, u32, u32)> {
    let mut it = s.splitn(3, '-');
    let y = it.next()?;
    let m = it.next()?;
    let d = it.next()?;
    if y.is_empty() || !y.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if m.len() != 2 || !m.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if d.len() != 2 || !d.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let year: i64 = y.parse().ok()?;
    let month: u32 = m.parse().ok()?;
    let day: u32 = d.parse().ok()?;
    if !(1..=9999).contains(&year) {
        return None;
    }
    if !(1..=12).contains(&month) {
        return None;
    }
    if day < 1 || day > days_in_month(year, month) {
        return None;
    }
    Some((year, month, day))
}

fn parse_time(s: &str) -> Option<(u32, u32, u32, String)> {
    let (hms, frac) = match s.split_once('.') {
        Some((h, f)) => (h, Some(f)),
        None => (s, None),
    };
    let mut it = hms.split(':');
    let h = it.next()?;
    let m = it.next()?;
    let sec = it.next();
    if it.next().is_some() {
        return None;
    }
    if h.len() != 2 || !h.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if m.len() != 2 || !m.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let hour: u32 = h.parse().ok()?;
    let minute: u32 = m.parse().ok()?;
    let second: u32 = match sec {
        Some(ss) => {
            if ss.len() != 2 || !ss.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            ss.parse().ok()?
        }
        None => 0,
    };
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let frac_canon = match frac {
        None => String::new(),
        Some(f) => {
            if sec.is_none() || f.is_empty() || !f.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let micros: String = f.chars().take(6).collect();
            micros.trim_end_matches('0').to_string()
        }
    };
    Some((hour, minute, second, frac_canon))
}

fn parse_offset(s: &str) -> Option<i64> {
    if s.eq_ignore_ascii_case("z") {
        return Some(0);
    }
    let (sign, rest) = match s.as_bytes().first()? {
        b'+' => (1i64, &s[1..]),
        b'-' => (-1i64, &s[1..]),
        _ => return None,
    };

    let (hh, mm, ss): (&str, &str, &str) = if rest.contains(':') {
        let mut it = rest.split(':');
        let h = it.next()?;
        let m = it.next().unwrap_or("00");
        let s2 = it.next().unwrap_or("00");
        if it.next().is_some() {
            return None;
        }
        (h, m, s2)
    } else {
        match rest.len() {
            1 | 2 => (rest, "00", "00"),
            4 => (&rest[..2], &rest[2..4], "00"),
            6 => (&rest[..2], &rest[2..4], &rest[4..6]),
            _ => return None,
        }
    };
    if hh.is_empty() || hh.len() > 2 || !hh.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if mm.len() != 2 || !mm.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if ss.len() != 2 || !ss.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let hour: i64 = hh.parse().ok()?;
    let minute: i64 = mm.parse().ok()?;
    let second: i64 = ss.parse().ok()?;

    if hour > 15 || minute > 59 || second > 59 {
        return None;
    }
    Some(sign * (hour * 3600 + minute * 60 + second))
}

fn split_date_time(trimmed: &str) -> (&str, Option<&str>) {
    if let Some((d, t)) = trimmed.split_once('T') {
        (d, Some(t))
    } else if let Some((d, t)) = trimmed.split_once(' ') {
        (d, Some(t.trim_start()))
    } else {
        (trimmed, None)
    }
}

fn split_time_offset(tp: &str) -> (&str, Option<&str>) {
    if let Some(idx) = tp.find(|c| c == '+' || c == '-') {
        (tp[..idx].trim_end(), Some(tp[idx..].trim()))
    } else if tp.ends_with('Z') || tp.ends_with('z') {
        (tp[..tp.len() - 1].trim_end(), Some(&tp[tp.len() - 1..]))
    } else {
        (tp, None)
    }
}

fn canonicalize(y: i64, mo: u32, d: u32, h: u32, mi: u32, s: u32, frac: &str) -> String {
    let base = format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}");
    if frac.is_empty() {
        format!("{base}+00")
    } else {
        format!("{base}.{frac}+00")
    }
}

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    let _ = oid;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(invalid(text));
    }

    let (date_part, time_part) = split_date_time(trimmed);

    let tp = match time_part {
        Some(tp) if !tp.is_empty() => tp,
        _ => return Err(invalid(text)),
    };

    let (year, month, day) = match parse_date(date_part) {
        Some(v) => v,
        None => return Err(invalid(text)),
    };

    let (time_str, offset_tok) = split_time_offset(tp);

    let offset_tok = match offset_tok {
        Some(o) => o,
        None => return Err(invalid(text)),
    };
    let offset_secs = match parse_offset(offset_tok) {
        Some(v) => v,
        None => return Err(invalid(text)),
    };

    let time_str = time_str.trim();
    if time_str.is_empty()
        || !time_str
            .bytes()
            .all(|b| b.is_ascii_digit() || b == b':' || b == b'.')
    {
        return Err(invalid(text));
    }
    let (h, mi, s, frac) = match parse_time(time_str) {
        Some(v) => v,
        None => return Err(invalid(text)),
    };

    let days = days_from_civil(year, month, day);
    let total = days * 86_400 + (h as i64) * 3600 + (mi as i64) * 60 + (s as i64) - offset_secs;
    let day_utc = total.div_euclid(86_400);
    let sod = total.rem_euclid(86_400);
    let (ny, nm, nd) = civil_from_days(day_utc);

    if !(1..=9999).contains(&ny) {
        return Err(invalid(text));
    }
    let nh = (sod / 3600) as u32;
    let nmi = ((sod % 3600) / 60) as u32;
    let ns = (sod % 60) as u32;

    Ok(SqlValue::Text(canonicalize(ny, nm, nd, nh, nmi, ns, &frac)))
}

pub fn output(oid: u32, v: &SqlValue) -> String {
    let _ = oid;
    match v {
        SqlValue::Text(s) => s.clone(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::oid;

    fn parse(s: &str) -> Result<String, PgError> {
        input(oid::TIMESTAMPTZ, s).map(|v| output(oid::TIMESTAMPTZ, &v))
    }

    #[test]
    fn utc_offset_zero_is_identity_instant() {
        assert_eq!(
            parse("2001-09-22 18:19:20+00").unwrap(),
            "2001-09-22 18:19:20+00"
        );
    }

    #[test]
    fn positive_offset_normalizes_to_utc() {

        assert_eq!(
            parse("2001-09-22 18:19:20+05:30").unwrap(),
            "2001-09-22 12:49:20+00"
        );
    }

    #[test]
    fn negative_offset_normalizes_to_utc() {

        assert_eq!(
            parse("2001-09-22 18:19:20-08").unwrap(),
            "2001-09-23 02:19:20+00"
        );
    }

    #[test]
    fn z_offset_is_utc() {
        assert_eq!(
            parse("2001-09-22 18:19:20Z").unwrap(),
            "2001-09-22 18:19:20+00"
        );
        assert_eq!(
            parse("2001-09-22T18:19:20z").unwrap(),
            "2001-09-22 18:19:20+00"
        );
    }

    #[test]
    fn t_separator_and_abutting_offset() {
        assert_eq!(
            parse("2001-09-22T18:19:20+00").unwrap(),
            "2001-09-22 18:19:20+00"
        );
    }

    #[test]
    fn space_separated_offset() {
        assert_eq!(
            parse("2001-09-22 18:19:20 +05:30").unwrap(),
            "2001-09-22 12:49:20+00"
        );
    }

    #[test]
    fn compact_hhmm_offset() {
        assert_eq!(
            parse("2001-09-22 18:19:20-0800").unwrap(),
            "2001-09-23 02:19:20+00"
        );
    }

    #[test]
    fn seconds_precision_offset() {

        assert_eq!(
            parse("2001-09-22 18:19:20+00:00:30").unwrap(),
            "2001-09-22 18:18:50+00"
        );
    }

    #[test]
    fn fractional_seconds_preserved_and_canonicalized() {
        assert_eq!(
            parse("2004-02-29 15:44:17.71393+00").unwrap(),
            "2004-02-29 15:44:17.71393+00"
        );

        assert_eq!(
            parse("1997-02-10 17:32:01.400000+00").unwrap(),
            "1997-02-10 17:32:01.4+00"
        );

        assert_eq!(
            parse("1997-02-10 17:32:01.000000+00").unwrap(),
            "1997-02-10 17:32:01+00"
        );

        assert_eq!(
            parse("2018-11-02 12:34:56.78901234-01").unwrap(),
            "2018-11-02 13:34:56.789012+00"
        );
    }

    #[test]
    fn offset_carries_across_date_boundary_backward() {

        assert_eq!(
            parse("2001-01-01 02:00:00+05").unwrap(),
            "2000-12-31 21:00:00+00"
        );
    }

    #[test]
    fn offset_carries_across_date_boundary_forward() {

        assert_eq!(
            parse("2001-12-31 22:00:00-05").unwrap(),
            "2002-01-01 03:00:00+00"
        );
    }

    #[test]
    fn offset_carry_respects_leap_day() {

        assert_eq!(
            parse("2000-03-01 00:30:00+02").unwrap(),
            "2000-02-29 22:30:00+00"
        );
    }

    #[test]
    fn calendar_range_rejects() {
        assert!(parse("1997-13-01 00:00:00+00").is_err());
        assert!(parse("1997-00-01 00:00:00+00").is_err());
        assert!(parse("1997-04-31 00:00:00+00").is_err());
        assert!(parse("1997-01-32 00:00:00+00").is_err());
        assert!(parse("0000-01-01 00:00:00+00").is_err());
        assert!(parse("1997-02-29 00:00:00+00").is_err());
    }

    #[test]
    fn time_range_rejects() {
        assert!(parse("1997-01-02 24:00:00+00").is_err());
        assert!(parse("1997-01-02 12:60:00+00").is_err());
        assert!(parse("1997-01-02 12:00:60+00").is_err());
    }

    #[test]
    fn offset_range_rejects() {
        assert!(parse("2001-01-01 00:00:00+25").is_err());
        assert!(parse("2001-01-01 00:00:00+16").is_err());
        assert!(parse("2001-01-01 00:00:00+05:60").is_err());
        assert!(parse("2001-01-01 00:00:00+5:3").is_err());
    }

    #[test]
    fn garbage_rejected() {
        for s in [
            "garbage",
            "",
            "   ",
            "1997-01",
            "not-a-date",
            "1997/02/10 17:32:01+00",
            "2001-09-22 18:19:20+",
        ] {
            assert!(parse(s).is_err(), "expected reject: {s:?}");
        }
    }

    #[test]
    fn error_shape_is_invalid_input_syntax() {
        let e = input(oid::TIMESTAMPTZ, "garbage").unwrap_err();
        assert_eq!(
            e,
            PgError::InvalidInputSyntax {
                typ: "timestamp with time zone",
                input: "garbage".into()
            }
        );
    }

    #[test]
    fn round_trips_of_canonical_values() {

        for s in [
            "2001-09-22 18:19:20+00",
            "2004-02-29 15:44:17.71393+00",
            "2000-12-31 21:00:00+00",
            "1996-02-29 23:59:59.999999+00",
        ] {
            let once = parse(s).unwrap();
            assert_eq!(once, s, "canonical value changed on first parse: {s:?}");
            let twice = parse(&once).unwrap();
            assert_eq!(once, twice, "not idempotent for {s:?}");
        }
    }

    #[test]
    fn canonical_sorts_chronologically() {
        let mut v = vec![
            parse("2020-01-01 00:00:00.5+00").unwrap(),
            parse("2020-01-01 00:00:00+00").unwrap(),
            parse("2019-12-31 23:59:59+00").unwrap(),
            parse("2020-01-01 00:00:01+00").unwrap(),
        ];
        v.sort();
        assert_eq!(
            v,
            vec![
                "2019-12-31 23:59:59+00",
                "2020-01-01 00:00:00+00",
                "2020-01-01 00:00:00.5+00",
                "2020-01-01 00:00:01+00",
            ]
        );
    }

    #[test]
    fn non_text_output_is_empty() {
        assert_eq!(output(oid::TIMESTAMPTZ, &SqlValue::Null), "");
        assert_eq!(output(oid::TIMESTAMPTZ, &SqlValue::Int(5)), "");
    }

    #[test]
    fn deferred_forms_are_rejected() {
        let deferred = [

            "2001-09-22 18:19:20",
            "2001-09-22T18:19:20",
            "2001-09-22",
            "2001-09-22 18:19:20.5",

            "2001-09-22 18:19:20 PST",
            "2001-09-22 18:19:20 America/New_York",
            "2001-09-22 18:19:20 GMT+8",
            "Sat Mar 12 23:58:48 2005 PST",

            "1997/02/10 17:32:01+00",
            "Feb 10 17:32:01 1997",
            "epoch",
            "now",
            "infinity",
            "-infinity",
        ];
        for s in deferred {
            assert!(
                parse(s).is_err(),
                "deferred form should reject for now: {s:?}"
            );
        }
    }
}
