
use super::PgError;
use sql_core::SqlValue;

fn err(input: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: super::type_name(super::oid::PG_LSN),
        input: input.to_string(),
    }
}

fn parse_half(half: &str) -> Option<u32> {
    if half.is_empty() || !half.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }

    u32::from_str_radix(half, 16).ok()
}

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    let _ = oid;

    let (hi_str, lo_str) = text.split_once('/').ok_or_else(|| err(text))?;

    let hi = parse_half(hi_str).ok_or_else(|| err(text))?;
    let lo = parse_half(lo_str).ok_or_else(|| err(text))?;

    Ok(SqlValue::Text(format!("{hi:X}/{lo:X}")))
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

    fn parse(s: &str) -> SqlValue {
        input(oid::PG_LSN, s).expect("expected a valid pg_lsn literal")
    }

    #[test]
    fn smallest_input() {

        assert_eq!(parse("0/0"), SqlValue::Text("0/0".to_string()));
    }

    #[test]
    fn mixed_case_normalized_to_uppercase() {

        assert_eq!(
            parse("16/b374d848"),
            SqlValue::Text("16/B374D848".to_string())
        );
    }

    #[test]
    fn largest_input() {

        assert_eq!(
            parse("FFFFFFFF/FFFFFFFF"),
            SqlValue::Text("FFFFFFFF/FFFFFFFF".to_string())
        );
    }

    #[test]
    fn canonical_value_round_trips() {

        assert_eq!(
            parse("16/B374D848"),
            SqlValue::Text("16/B374D848".to_string())
        );
    }

    #[test]
    fn leading_zeros_dropped() {

        assert_eq!(
            parse("00000016/0B374D848"),
            SqlValue::Text("16/B374D848".to_string())
        );
    }

    #[test]
    fn reject_missing_slash() {

        let bad = "16AE7F7";
        let e = input(oid::PG_LSN, bad).unwrap_err();
        assert_eq!(
            e,
            PgError::InvalidInputSyntax {
                typ: "pg_lsn",
                input: bad.to_string()
            }
        );

        assert_eq!(
            e.message(),
            "invalid input syntax for type pg_lsn: \"16AE7F7\""
        );
    }

    #[test]
    fn reject_empty_half() {

        assert!(matches!(
            input(oid::PG_LSN, "ABCD/"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
        assert!(matches!(
            input(oid::PG_LSN, "/ABCD"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_non_hex() {

        assert!(matches!(
            input(oid::PG_LSN, "G/0"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
        assert!(matches!(
            input(oid::PG_LSN, "-1/0"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
        assert!(matches!(
            input(oid::PG_LSN, " 0/12345678"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_overflow() {

        assert!(matches!(
            input(oid::PG_LSN, "100000000/0"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
        assert!(matches!(
            input(oid::PG_LSN, "0/1FFFFFFFF"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn output_round_trips_stored_canonical() {
        let v = parse("16/b374d848");
        assert_eq!(output(oid::PG_LSN, &v), "16/B374D848");
        assert_eq!(output(oid::PG_LSN, &parse("0/0")), "0/0");
    }

    #[test]
    fn output_non_text_is_empty() {
        assert_eq!(output(oid::PG_LSN, &SqlValue::Null), "");
        assert_eq!(output(oid::PG_LSN, &SqlValue::Int(42)), "");
    }
}
