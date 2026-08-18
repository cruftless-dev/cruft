
use super::{type_name, PgError};
use sql_core::SqlValue;

fn is_pg_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r')
}

pub fn input(_oid: u32, text: &str) -> Result<SqlValue, PgError> {
    let typ = type_name(super::oid::MONEY);
    let invalid = || PgError::InvalidInputSyntax {
        typ,
        input: text.to_string(),
    };
    let out_of_range = || PgError::OutOfRange {
        typ,
        input: text.to_string(),
    };

    let chars: Vec<char> = text.chars().collect();

    let mut i = 0;
    let mut end = chars.len();
    while i < end && is_pg_space(chars[i]) {
        i += 1;
    }
    while end > i && is_pg_space(chars[end - 1]) {
        end -= 1;
    }
    if i == end {
        return Err(invalid());
    }

    let mut neg = false;
    let mut paren = false;
    match chars[i] {
        '(' => {
            paren = true;
            neg = true;
            i += 1;
        }
        '-' => {
            neg = true;
            i += 1;
        }
        '+' => {
            i += 1;
        }
        _ => {}
    }

    while i < end && is_pg_space(chars[i]) {
        i += 1;
    }
    if i < end && chars[i] == '$' {
        i += 1;
    }
    while i < end && is_pg_space(chars[i]) {
        i += 1;
    }

    let mut value: i128 = 0;
    let mut dec: u32 = 0;
    let mut seen_dot = false;
    let mut round_up = false;
    let mut digits = 0u32;
    while i < end {
        let c = chars[i];
        if c.is_ascii_digit() {
            digits += 1;
            let d = (c as u8 - b'0') as i128;
            if dec < 2 {
                value = value
                    .checked_mul(10)
                    .and_then(|v| v.checked_add(d))
                    .ok_or_else(out_of_range)?;
                if seen_dot {
                    dec += 1;
                }
            } else {

                if !round_up && c >= '5' {
                    round_up = true;
                }
            }
        } else if c == '.' && !seen_dot {
            seen_dot = true;
        } else if c == ',' {

        } else {
            break;
        }
        i += 1;
    }

    if digits == 0 {
        return Err(invalid());
    }

    while i < end && is_pg_space(chars[i]) {
        i += 1;
    }
    if paren {
        if i < end && chars[i] == ')' {
            i += 1;
        } else {
            return Err(invalid());
        }
        while i < end && is_pg_space(chars[i]) {
            i += 1;
        }
    }
    if i != end {
        return Err(invalid());
    }

    let scale = 10i128.pow(2 - dec);
    let mut mag = value.checked_mul(scale).ok_or_else(out_of_range)?;
    if round_up {
        mag = mag.checked_add(1).ok_or_else(out_of_range)?;
    }

    let signed = if neg { -mag } else { mag };
    if signed < i64::MIN as i128 || signed > i64::MAX as i128 {
        return Err(out_of_range());
    }
    Ok(SqlValue::Int(signed as i64))
}

pub fn output(_oid: u32, v: &SqlValue) -> String {
    let cents = match v {
        SqlValue::Int(c) => *c,
        _ => return String::new(),
    };
    let neg = cents < 0;
    let mag = (cents as i128).unsigned_abs();
    let dollars = mag / 100;
    let frac = (mag % 100) as u32;

    let digits = dollars.to_string();
    let mut grouped = String::new();
    let n = digits.len();
    for (idx, ch) in digits.chars().enumerate() {
        if idx > 0 && (n - idx) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }

    let body = format!("${grouped}.{frac:02}");
    if neg {
        format!("-{body}")
    } else {
        body
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONEY: u32 = super::super::oid::MONEY;

    fn cents(v: SqlValue) -> i64 {
        match v {
            SqlValue::Int(c) => c,
            other => panic!("expected Int, got {other:?}"),
        }
    }

    #[test]
    fn bare_integer_is_dollars() {

        assert_eq!(cents(input(MONEY, "123").unwrap()), 12300);
    }

    #[test]
    fn dollar_prefixed_and_fraction() {
        assert_eq!(cents(input(MONEY, "$123.45").unwrap()), 12345);
        assert_eq!(cents(input(MONEY, "123.45").unwrap()), 12345);

        assert_eq!(cents(input(MONEY, "$0.4").unwrap()), 40);
    }

    #[test]
    fn thousands_separators_are_ignored() {
        assert_eq!(cents(input(MONEY, "$1,234.56").unwrap()), 123456);
        assert_eq!(
            cents(input(MONEY, "$1,234,567,890.00").unwrap()),
            123456789000
        );
    }

    #[test]
    fn leading_sign_negative() {
        assert_eq!(cents(input(MONEY, "-$1.23").unwrap()), -123);
        assert_eq!(cents(input(MONEY, "-12345").unwrap()), -1234500);
    }

    #[test]
    fn parentheses_are_negative() {

        assert_eq!(cents(input(MONEY, "(1)").unwrap()), -100);
        assert_eq!(cents(input(MONEY, "($123,456.78)").unwrap()), -12345678);
    }

    #[test]
    fn fraction_rounding_matches_golden() {

        assert_eq!(cents(input(MONEY, "$123.451").unwrap()), 12345);
        assert_eq!(cents(input(MONEY, "$123.454").unwrap()), 12345);
        assert_eq!(cents(input(MONEY, "$123.455").unwrap()), 12346);
        assert_eq!(cents(input(MONEY, "$123.456").unwrap()), 12346);
        assert_eq!(cents(input(MONEY, "$123.459").unwrap()), 12346);
    }

    #[test]
    fn documented_min_max() {
        assert_eq!(
            cents(input(MONEY, "92233720368547758.07").unwrap()),
            i64::MAX
        );
        assert_eq!(
            cents(input(MONEY, "-92233720368547758.08").unwrap()),
            i64::MIN
        );
    }

    #[test]
    fn just_past_range_is_out_of_range() {
        for s in [
            "92233720368547758.08",
            "-92233720368547758.09",

            "92233720368547758.075",
            "-92233720368547758.085",
        ] {
            let e = input(MONEY, s).unwrap_err();
            assert_eq!(
                e,
                PgError::OutOfRange {
                    typ: "money",
                    input: s.into()
                },
                "for {s}"
            );
        }
    }

    #[test]
    fn large_integers_overflow_cents() {

        for s in [
            "123456789012345678",
            "9223372036854775807",
            "-123456789012345678",
            "-9223372036854775808",
            "192233720368547758.07",
        ] {
            let e = input(MONEY, s).unwrap_err();
            assert_eq!(
                e,
                PgError::OutOfRange {
                    typ: "money",
                    input: s.into()
                },
                "for {s}"
            );
        }
        assert_eq!(
            input(MONEY, "123456789012345678").unwrap_err().message(),
            "value \"123456789012345678\" is out of range for type money"
        );
    }

    #[test]
    fn within_range_large_values_parse() {

        assert_eq!(
            cents(input(MONEY, "12345678901234567").unwrap()),
            1234567890123456700
        );
    }

    #[test]
    fn invalid_rejects() {
        for s in [
            "\\x0001", "", "   ", "-", "$", "abc", "12.3.4", "(1", "1)", "$1.2x",
        ] {
            let e = input(MONEY, s).unwrap_err();
            assert_eq!(
                e,
                PgError::InvalidInputSyntax {
                    typ: "money",
                    input: s.into()
                },
                "expected invalid for {s:?}"
            );
        }
        assert_eq!(
            input(MONEY, "\\x0001").unwrap_err().message(),
            "invalid input syntax for type money: \"\\x0001\""
        );
    }

    #[test]
    fn output_formats_with_commas_and_two_decimals() {
        assert_eq!(output(MONEY, &SqlValue::Int(12300)), "$123.00");
        assert_eq!(output(MONEY, &SqlValue::Int(12345)), "$123.45");
        assert_eq!(output(MONEY, &SqlValue::Int(0)), "$0.00");
        assert_eq!(
            output(MONEY, &SqlValue::Int(123456789000)),
            "$1,234,567,890.00"
        );
        assert_eq!(
            output(MONEY, &SqlValue::Int(i64::MAX)),
            "$92,233,720,368,547,758.07"
        );
    }

    #[test]
    fn output_negatives_use_dash_dollar() {
        assert_eq!(output(MONEY, &SqlValue::Int(-100)), "-$1.00");
        assert_eq!(output(MONEY, &SqlValue::Int(-1234500)), "-$12,345.00");
        assert_eq!(output(MONEY, &SqlValue::Int(-12345678)), "-$123,456.78");
        assert_eq!(
            output(MONEY, &SqlValue::Int(i64::MIN)),
            "-$92,233,720,368,547,758.08"
        );
    }

    #[test]
    fn output_non_int_is_empty() {
        assert_eq!(output(MONEY, &SqlValue::Null), "");
    }

    #[test]
    fn round_trips() {
        for (s, canon) in [
            ("123", "$123.00"),
            ("$123.45", "$123.45"),
            ("-12345", "-$12,345.00"),
            ("($123,456.78)", "-$123,456.78"),
            ("92233720368547758.07", "$92,233,720,368,547,758.07"),
            ("-92233720368547758.08", "-$92,233,720,368,547,758.08"),
        ] {
            let v = input(MONEY, s).unwrap();
            let text = output(MONEY, &v);
            assert_eq!(text, canon, "output for {s}");

            assert_eq!(input(MONEY, &text).unwrap(), v, "reparse for {s}");
        }
    }
}
