
use super::PgError;
use sql_core::SqlValue;

const TYP: &str = "timestamp without time zone";

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
            let trimmed = micros.trim_end_matches('0');
            trimmed.to_string()
        }
    };
    Some((hour, minute, second, frac_canon))
}

fn canonicalize(y: i64, mo: u32, d: u32, h: u32, mi: u32, s: u32, frac: &str) -> String {
    let base = format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}");
    if frac.is_empty() {
        base
    } else {
        format!("{base}.{frac}")
    }
}

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    let _ = oid;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(invalid(text));
    }

    let (date_part, time_part): (&str, Option<&str>) = if let Some((d, t)) = trimmed.split_once('T')
    {
        (d, Some(t))
    } else if let Some((d, t)) = trimmed.split_once(' ') {
        (d, Some(t.trim_start()))
    } else {
        (trimmed, None)
    };

    let (year, month, day) = match parse_date(date_part) {
        Some(v) => v,
        None => return Err(invalid(text)),
    };

    let (h, mi, s, frac) = match time_part {
        None => (0, 0, 0, String::new()),
        Some(tp) => {

            if tp.is_empty()
                || !tp
                    .bytes()
                    .all(|b| b.is_ascii_digit() || b == b':' || b == b'.')
            {
                return Err(invalid(text));
            }
            match parse_time(tp) {
                Some(v) => v,
                None => return Err(invalid(text)),
            }
        }
    };

    Ok(SqlValue::Text(canonicalize(
        year, month, day, h, mi, s, &frac,
    )))
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
        input(oid::TIMESTAMP, s).map(|v| output(oid::TIMESTAMP, &v))
    }

    #[test]
    fn iso_space_no_fraction() {
        assert_eq!(parse("1997-01-02 03:04:05").unwrap(), "1997-01-02 03:04:05");
    }

    #[test]
    fn iso_with_fraction_trailing_zeros_stripped() {
        assert_eq!(
            parse("2004-02-29 15:44:17.71393").unwrap(),
            "2004-02-29 15:44:17.71393"
        );
        assert_eq!(
            parse("1997-02-10 17:32:01.400000").unwrap(),
            "1997-02-10 17:32:01.4"
        );

        assert_eq!(
            parse("1997-02-10 17:32:01.000000").unwrap(),
            "1997-02-10 17:32:01"
        );

        assert_eq!(
            parse("2018-11-02 12:34:56.78901234").unwrap(),
            "2018-11-02 12:34:56.789012"
        );
    }

    #[test]
    fn t_separator() {
        assert_eq!(parse("2001-09-22T18:19:20").unwrap(), "2001-09-22 18:19:20");
    }

    #[test]
    fn date_only_is_midnight() {
        assert_eq!(parse("1997-01-02").unwrap(), "1997-01-02 00:00:00");
    }

    #[test]
    fn hh_mm_defaults_seconds() {
        assert_eq!(parse("2020-01-01 00:00").unwrap(), "2020-01-01 00:00:00");
    }

    #[test]
    fn leap_day_accepted_and_rejected() {
        assert!(parse("1996-02-29 17:32:01").is_ok());
        assert!(parse("1997-02-29 17:32:01").is_err());
    }

    #[test]
    fn calendar_range_rejects() {
        assert!(parse("1997-13-01 00:00:00").is_err());
        assert!(parse("1997-00-01 00:00:00").is_err());
        assert!(parse("1997-04-31 00:00:00").is_err());
        assert!(parse("1997-01-32 00:00:00").is_err());
        assert!(parse("0000-01-01 00:00:00").is_err());
    }

    #[test]
    fn time_range_rejects() {
        assert!(parse("1997-01-02 24:00:00").is_err());
        assert!(parse("1997-01-02 12:60:00").is_err());
        assert!(parse("1997-01-02 12:00:60").is_err());
    }

    #[test]
    fn plain_garbage_rejected() {
        for s in [
            "garbage",
            "",
            "   ",
            "1997-01",
            "not-a-date",
            "1997/02/10 17:32:01",
        ] {
            assert!(parse(s).is_err(), "expected reject: {s:?}");
        }
    }

    #[test]
    fn error_shape_is_invalid_input_syntax() {
        let e = input(oid::TIMESTAMP, "garbage").unwrap_err();
        assert_eq!(
            e,
            PgError::InvalidInputSyntax {
                typ: "timestamp without time zone",
                input: "garbage".into()
            }
        );
    }

    #[test]
    fn round_trips() {
        for s in [
            "1997-01-02 00:00:00",
            "2004-02-29 15:44:17.71393",
            "2001-09-22 18:19:20",
            "1996-02-29 23:59:59.999999",
        ] {
            let once = parse(s).unwrap();
            let twice = parse(&once).unwrap();
            assert_eq!(once, twice, "not idempotent for {s:?}");
        }
    }

    #[test]
    fn canonical_sorts_chronologically() {
        let mut v = vec![
            parse("2020-01-01 00:00:00.5").unwrap(),
            parse("2020-01-01 00:00:00").unwrap(),
            parse("2019-12-31 23:59:59").unwrap(),
            parse("2020-01-01 00:00:01").unwrap(),
        ];
        v.sort();
        assert_eq!(
            v,
            vec![
                "2019-12-31 23:59:59",
                "2020-01-01 00:00:00",
                "2020-01-01 00:00:00.5",
                "2020-01-01 00:00:01",
            ]
        );
    }

    #[test]
    fn non_text_output_is_empty() {
        assert_eq!(output(oid::TIMESTAMP, &SqlValue::Null), "");
        assert_eq!(output(oid::TIMESTAMP, &SqlValue::Int(5)), "");
    }

    #[test]
    fn deferred_forms_are_rejected() {
        let deferred = [

            "Mon Feb 10 17:32:01 1997",
            "Feb 10 17:32:01 1997",
            "Feb 10 5:32PM 1997",

            "1997/02/10 17:32:01",
            "19970210 173201",
            "1997.041 17:32:01",
            "02-10-1997 17:32:01",

            "1997-02-10 17:32:01-08",
            "1997-02-10 17:32:01 -08:00",
            "2000-03-15 08:14:01 GMT+8",
            "19970210 173201 America/New_York",

            "4714-11-24 00:00:00 BC",

            "epoch",
            "now",
            "today",
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
