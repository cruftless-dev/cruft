
use super::PgError;
use sql_core::SqlValue;

const USECS_PER_SEC: u64 = 1_000_000;

fn invalid(text: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: super::type_name(super::oid::TIME),
        input: text.to_string(),
    }
}

fn parse_canonical(text: &str) -> Result<String, PgError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(invalid(text));
    }

    let parts: Vec<&str> = trimmed.split(':').collect();
    if parts.len() != 2 && parts.len() != 3 {
        return Err(invalid(text));
    }

    let hour = parse_field(parts[0], text)?;
    let minute = parse_field(parts[1], text)?;

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

    Ok(format_canonical(total))
}

fn parse_field(field: &str, text: &str) -> Result<u64, PgError> {
    if field.is_empty() || !field.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid(text));
    }
    field.parse::<u64>().map_err(|_| invalid(text))
}

fn parse_seconds(field: &str, text: &str) -> Result<(u64, u64), PgError> {
    let (int_part, frac_part) = match field.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (field, None),
    };

    let second = parse_field(int_part, text)?;

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

fn format_canonical(total: u64) -> String {
    let usec = total % USECS_PER_SEC;
    let totsec = total / USECS_PER_SEC;
    let second = totsec % 60;
    let totmin = totsec / 60;
    let minute = totmin % 60;
    let hour = totmin / 60;

    let mut out = format!("{hour:02}:{minute:02}:{second:02}");
    if usec > 0 {

        let frac = format!("{usec:06}");
        let trimmed = frac.trim_end_matches('0');
        out.push('.');
        out.push_str(trimmed);
    }
    out
}

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    debug_assert_eq!(oid, super::oid::TIME);
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
    use super::super::oid::TIME;
    use super::*;

    fn parse(s: &str) -> Result<String, PgError> {
        match input(TIME, s) {
            Ok(SqlValue::Text(t)) => Ok(t),
            Ok(_) => panic!("time input must produce Text"),
            Err(e) => Err(e),
        }
    }

    #[test]
    fn full_hms() {
        assert_eq!(parse("05:06:07").unwrap(), "05:06:07");
        assert_eq!(parse("15:36:39").unwrap(), "15:36:39");
        assert_eq!(parse("00:00:00").unwrap(), "00:00:00");
    }

    #[test]
    fn hh_mm_shorthand_defaults_seconds() {
        assert_eq!(parse("00:00").unwrap(), "00:00:00");
        assert_eq!(parse("01:00").unwrap(), "01:00:00");
        assert_eq!(parse("23:59").unwrap(), "23:59:00");
        assert_eq!(parse("12:01").unwrap(), "12:01:00");
    }

    #[test]
    fn fractional_seconds_preserved() {
        assert_eq!(parse("23:59:59.999999").unwrap(), "23:59:59.999999");
        assert_eq!(parse("13:30:25.575401").unwrap(), "13:30:25.575401");
    }

    #[test]
    fn fractional_zero_canonicalization() {

        assert_eq!(parse("00:00:00.000000").unwrap(), "00:00:00");
        assert_eq!(parse("23:59:59.990000").unwrap(), "23:59:59.99");
        assert_eq!(parse("12:00:00.500000").unwrap(), "12:00:00.5");
        assert_eq!(parse("12:00:00.010000").unwrap(), "12:00:00.01");
    }

    #[test]
    fn leading_trailing_whitespace_trimmed() {
        assert_eq!(parse("  12:00:00  ").unwrap(), "12:00:00");
    }

    #[test]
    fn hour_24_midnight_allowed() {
        assert_eq!(parse("24:00:00").unwrap(), "24:00:00");
    }

    #[test]
    fn leap_second_rounds_up() {

        assert_eq!(parse("23:59:60").unwrap(), "24:00:00");
    }

    #[test]
    fn seventh_fractional_digit_rounds_and_carries() {

        assert_eq!(parse("23:59:59.9999999").unwrap(), "24:00:00");

        assert_eq!(parse("00:00:00.0000005").unwrap(), "00:00:00.000001");
    }

    #[test]
    fn hour_24_with_nonzero_rejected() {
        assert!(parse("24:00:00.01").is_err());
        assert!(parse("24:01:00").is_err());
        assert!(parse("25:00:00").is_err());
    }

    #[test]
    fn leap_second_with_fraction_rejected() {
        assert!(parse("23:59:60.01").is_err());
    }

    #[test]
    fn field_range_rejected() {
        assert!(parse("12:60:00").is_err());
        assert!(parse("12:00:61").is_err());
        assert!(parse("25:61:61").is_err());
    }

    #[test]
    fn syntax_rejections() {
        assert!(parse("").is_err());
        assert!(parse("   ").is_err());
        assert!(parse("abc").is_err());
        assert!(parse("12").is_err());
        assert!(parse("12:00:00:00").is_err());
        assert!(parse("12:0a:00").is_err());
        assert!(parse("12:00:00.").is_err());
        assert!(parse("12:00:00.5x").is_err());
    }

    #[test]
    fn deferred_forms_rejected_this_rung() {

        assert!(parse("02:03 PST").is_err());
        assert!(parse("11:59:59.99 PM").is_err());
        assert!(parse("15:36:39 America/New_York").is_err());
    }

    #[test]
    fn error_is_invalid_input_syntax() {
        match input(TIME, "25:00:00") {
            Err(PgError::InvalidInputSyntax { typ, input }) => {
                assert_eq!(typ, "time without time zone");
                assert_eq!(input, "25:00:00");
            }
            other => panic!("expected InvalidInputSyntax, got {other:?}"),
        }
    }

    #[test]
    fn round_trips() {
        for s in [
            "00:00:00",
            "12:34:56",
            "23:59:59.999999",
            "24:00:00",
            "05:06:07.5",
        ] {
            let v = input(TIME, s).unwrap();
            let rendered = output(TIME, &v);
            assert_eq!(rendered, s);

            let v2 = input(TIME, &rendered).unwrap();
            assert_eq!(output(TIME, &v2), s);
        }
    }

    #[test]
    fn output_non_text_is_empty() {
        assert_eq!(output(TIME, &SqlValue::Null), "");
        assert_eq!(output(TIME, &SqlValue::Int(5)), "");
    }
}
