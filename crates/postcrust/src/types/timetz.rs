
use super::PgError;
use sql_core::SqlValue;

const USECS_PER_SEC: u64 = 1_000_000;

fn invalid(text: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: super::type_name(super::oid::TIMETZ),
        input: text.to_string(),
    }
}

fn parse_canonical(text: &str) -> Result<String, PgError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(invalid(text));
    }

    let (time_str, offset_secs) = split_offset(trimmed, text)?;
    let time_canon = parse_time(time_str, text)?;
    Ok(format!("{}{}", time_canon, format_offset(offset_secs)))
}

fn split_offset<'a>(s: &'a str, text: &str) -> Result<(&'a str, i64), PgError> {
    if let Some(rest) = s.strip_suffix('Z').or_else(|| s.strip_suffix('z')) {
        return Ok((rest.trim_end(), 0));
    }
    let idx = s
        .find(|c| c == '+' || c == '-')
        .ok_or_else(|| invalid(text))?;
    let time_part = s[..idx].trim_end();
    let secs = parse_offset(&s[idx..], text)?;
    Ok((time_part, secs))
}

fn parse_offset(off: &str, text: &str) -> Result<i64, PgError> {
    let sign: i64 = match off.as_bytes().first() {
        Some(b'+') => 1,
        Some(b'-') => -1,
        _ => return Err(invalid(text)),
    };
    let rest = &off[1..];
    if rest.is_empty() {
        return Err(invalid(text));
    }

    let (h, m, sec) = if rest.contains(':') {
        let parts: Vec<&str> = rest.split(':').collect();
        if parts.len() < 2 || parts.len() > 3 {
            return Err(invalid(text));
        }
        let h = parse_digits(parts[0], text)?;
        let m = parse_digits(parts[1], text)?;
        let sec = if parts.len() == 3 {
            parse_digits(parts[2], text)?
        } else {
            0
        };
        (h, m, sec)
    } else {
        if !rest.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid(text));
        }
        match rest.len() {
            1 | 2 => (rest.parse().unwrap(), 0, 0),
            4 => (rest[..2].parse().unwrap(), rest[2..].parse().unwrap(), 0),
            6 => (
                rest[..2].parse().unwrap(),
                rest[2..4].parse().unwrap(),
                rest[4..].parse().unwrap(),
            ),
            _ => return Err(invalid(text)),
        }
    };

    if h > 15 || m > 59 || sec > 59 {
        return Err(invalid(text));
    }
    Ok(sign * (h * 3600 + m * 60 + sec) as i64)
}

fn parse_digits(field: &str, text: &str) -> Result<u64, PgError> {
    if field.is_empty() || !field.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid(text));
    }
    field.parse::<u64>().map_err(|_| invalid(text))
}

fn parse_time(text_body: &str, text: &str) -> Result<String, PgError> {
    let body = text_body.trim();
    if body.is_empty() {
        return Err(invalid(text));
    }

    let parts: Vec<&str> = body.split(':').collect();
    if parts.len() != 2 && parts.len() != 3 {
        return Err(invalid(text));
    }

    let hour = parse_digits(parts[0], text)?;
    let minute = parse_digits(parts[1], text)?;
    let (second, micros) = if parts.len() == 3 {
        parse_seconds(parts[2], text)?
    } else {
        (0u64, 0u64)
    };

    if minute > 59 || second > 60 || hour > 24 {
        return Err(invalid(text));
    }
    if second == 60 && micros > 0 {
        return Err(invalid(text));
    }
    if hour == 24 && (minute > 0 || second > 0 || micros > 0) {
        return Err(invalid(text));
    }

    let total = ((hour * 60 + minute) * 60 + second) * USECS_PER_SEC + micros;
    Ok(format_time(total))
}

fn parse_seconds(field: &str, text: &str) -> Result<(u64, u64), PgError> {
    let (int_part, frac_part) = match field.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (field, None),
    };

    let second = parse_digits(int_part, text)?;

    let micros = match frac_part {
        None => 0u64,
        Some(frac) => {
            if frac.is_empty() || !frac.bytes().all(|b| b.is_ascii_digit()) {
                return Err(invalid(text));
            }
            let bytes = frac.as_bytes();
            let mut m: u64 = 0;
            for i in 0..6 {
                let d = if i < bytes.len() {
                    (bytes[i] - b'0') as u64
                } else {
                    0
                };
                m = m * 10 + d;
            }
            if bytes.len() > 6 && bytes[6] >= b'5' {
                m += 1;
            }
            m
        }
    };

    Ok((second, micros))
}

fn format_time(total: u64) -> String {
    let usec = total % USECS_PER_SEC;
    let totsec = total / USECS_PER_SEC;
    let second = totsec % 60;
    let totmin = totsec / 60;
    let minute = totmin % 60;
    let hour = totmin / 60;

    let mut out = format!("{hour:02}:{minute:02}:{second:02}");
    if usec > 0 {
        let frac = format!("{usec:06}");
        out.push('.');
        out.push_str(frac.trim_end_matches('0'));
    }
    out
}

fn format_offset(total_secs: i64) -> String {
    let sign = if total_secs < 0 { '-' } else { '+' };
    let a = total_secs.unsigned_abs();
    let h = a / 3600;
    let m = (a % 3600) / 60;
    let s = a % 60;

    let mut out = format!("{sign}{h:02}");
    if s != 0 {
        out.push_str(&format!(":{m:02}:{s:02}"));
    } else if m != 0 {
        out.push_str(&format!(":{m:02}"));
    }
    out
}

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    debug_assert_eq!(oid, super::oid::TIMETZ);
    let _ = oid;
    parse_canonical(text).map(SqlValue::Text)
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
    use super::super::oid::TIMETZ;
    use super::*;

    fn parse(s: &str) -> Result<String, PgError> {
        match input(TIMETZ, s) {
            Ok(SqlValue::Text(t)) => Ok(t),
            Ok(_) => panic!("timetz input must produce Text"),
            Err(e) => Err(e),
        }
    }

    #[test]
    fn numeric_offset_hh() {

        assert_eq!(parse("00:01:00-07").unwrap(), "00:01:00-07");
        assert_eq!(parse("08:08:00-04").unwrap(), "08:08:00-04");
        assert_eq!(parse("07:07:00-08").unwrap(), "07:07:00-08");
        assert_eq!(parse("05:06:07+05").unwrap(), "05:06:07+05");
    }

    #[test]
    fn numeric_offset_single_digit_hour() {

        assert_eq!(parse("12:00:00+5").unwrap(), "12:00:00+05");
        assert_eq!(parse("12:00:00-8").unwrap(), "12:00:00-08");
    }

    #[test]
    fn numeric_offset_hh_mm_shows_minutes() {

        assert_eq!(parse("07:00:00+05:30").unwrap(), "07:00:00+05:30");
        assert_eq!(parse("13:30:25-04:30").unwrap(), "13:30:25-04:30");
    }

    #[test]
    fn numeric_offset_hhmm_compact_form() {

        assert_eq!(parse("07:00:00+0530").unwrap(), "07:00:00+05:30");
        assert_eq!(parse("07:00:00-0430").unwrap(), "07:00:00-04:30");

        assert_eq!(parse("07:00:00+0500").unwrap(), "07:00:00+05");
    }

    #[test]
    fn numeric_offset_with_seconds() {

        assert_eq!(parse("07:00:00+05:30:15").unwrap(), "07:00:00+05:30:15");
    }

    #[test]
    fn zulu_offset_is_plus_zero() {
        assert_eq!(parse("12:00:00Z").unwrap(), "12:00:00+00");
        assert_eq!(parse("12:00:00z").unwrap(), "12:00:00+00");

        assert_eq!(parse("12:00:00 Z").unwrap(), "12:00:00+00");
    }

    #[test]
    fn whitespace_separated_offset() {
        assert_eq!(parse("  12:00:00 -05  ").unwrap(), "12:00:00-05");
    }

    #[test]
    fn hh_mm_time_body_defaults_seconds() {

        assert_eq!(parse("00:01-07").unwrap(), "00:01:00-07");
        assert_eq!(parse("23:59-07").unwrap(), "23:59:00-07");
    }

    #[test]
    fn fractional_seconds_preserved_and_trimmed() {

        assert_eq!(parse("23:59:59.99-07").unwrap(), "23:59:59.99-07");
        assert_eq!(parse("13:30:25.575401-04").unwrap(), "13:30:25.575401-04");
        assert_eq!(parse("23:59:59.999999-07").unwrap(), "23:59:59.999999-07");
        assert_eq!(parse("12:00:00.500000-07").unwrap(), "12:00:00.5-07");
    }

    #[test]
    fn hour_24_midnight_allowed_with_offset() {
        assert_eq!(parse("24:00:00-07").unwrap(), "24:00:00-07");
    }

    #[test]
    fn leap_and_fraction_carry_into_time_body() {

        assert_eq!(parse("23:59:60-07").unwrap(), "24:00:00-07");

        assert_eq!(parse("23:59:59.9999999-07").unwrap(), "24:00:00-07");
    }

    #[test]
    fn time_body_range_rejects() {
        assert!(parse("24:00:00.01-07").is_err());
        assert!(parse("23:59:60.01-07").is_err());
        assert!(parse("24:01:00-07").is_err());
        assert!(parse("25:00:00-07").is_err());
        assert!(parse("12:60:00-07").is_err());
        assert!(parse("12:00:61-07").is_err());
    }

    #[test]
    fn offset_range_rejects() {
        assert!(parse("12:00:00+16").is_err());
        assert!(parse("12:00:00+16:00").is_err());
        assert!(parse("12:00:00+05:60").is_err());
        assert!(parse("12:00:00+05:30:60").is_err());
    }

    #[test]
    fn named_and_abbreviated_zones_deferred() {

        assert!(parse("07:07 PST").is_err());
        assert!(parse("00:01 PDT").is_err());
        assert!(parse("08:08 EDT").is_err());
        assert!(parse("15:36:39 America/New_York").is_err());
        assert!(parse("15:36:39 m2").is_err());
        assert!(parse("15:36:39 MSK m2").is_err());
        assert!(parse("11:59:59.99 PM PDT").is_err());
    }

    #[test]
    fn missing_offset_deferred() {

        assert!(parse("12:00:00").is_err());
        assert!(parse("00:01").is_err());
    }

    #[test]
    fn syntax_rejections() {
        assert!(parse("").is_err());
        assert!(parse("   ").is_err());
        assert!(parse("-07").is_err());
        assert!(parse("12:0a:00-07").is_err());
        assert!(parse("12-07").is_err());
        assert!(parse("12:00:00+").is_err());
        assert!(parse("12:00:00+aa").is_err());
        assert!(parse("12:00:00+05:").is_err());
    }

    #[test]
    fn error_is_invalid_input_syntax() {
        match input(TIMETZ, "25:00:00-07") {
            Err(PgError::InvalidInputSyntax { typ, input }) => {
                assert_eq!(typ, "time with time zone");
                assert_eq!(input, "25:00:00-07");
            }
            other => panic!("expected InvalidInputSyntax, got {other:?}"),
        }
    }

    #[test]
    fn round_trips() {
        for s in [
            "00:01:00-07",
            "07:07:00-08",
            "13:30:25.575401-04",
            "07:00:00+05:30",
            "24:00:00-07",
            "12:00:00+00",
            "23:59:59.99-07",
        ] {
            let v = input(TIMETZ, s).unwrap();
            let rendered = output(TIMETZ, &v);
            assert_eq!(rendered, s);
            let v2 = input(TIMETZ, &rendered).unwrap();
            assert_eq!(output(TIMETZ, &v2), s);
        }
    }

    #[test]
    fn output_non_text_is_empty() {
        assert_eq!(output(TIMETZ, &SqlValue::Null), "");
        assert_eq!(output(TIMETZ, &SqlValue::Int(5)), "");
    }
}
