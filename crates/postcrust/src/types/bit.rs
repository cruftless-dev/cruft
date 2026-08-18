
use super::PgError;
use sql_core::SqlValue;

fn err(oid: u32, input: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: super::type_name(oid),
        input: input.to_string(),
    }
}

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    for c in text.chars() {
        if c != '0' && c != '1' {
            return Err(err(oid, text));
        }
    }
    Ok(SqlValue::Text(text.to_string()))
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

    fn parse(oid: u32, s: &str) -> SqlValue {
        input(oid, s).expect("expected a valid bit string")
    }

    #[test]
    fn valid_bit_strings_stored_verbatim() {

        assert_eq!(
            parse(oid::BIT, "00000000000"),
            SqlValue::Text("00000000000".to_string())
        );
        assert_eq!(
            parse(oid::BIT, "11011000000"),
            SqlValue::Text("11011000000".to_string())
        );
        assert_eq!(parse(oid::BIT, "101"), SqlValue::Text("101".to_string()));
    }

    #[test]
    fn empty_string_is_valid() {

        assert_eq!(parse(oid::VARBIT, ""), SqlValue::Text(String::new()));
        assert_eq!(parse(oid::BIT, ""), SqlValue::Text(String::new()));
    }

    #[test]
    fn non_binary_char_rejected() {

        let e = input(oid::BIT, "1 0").unwrap_err();
        assert_eq!(
            e,
            PgError::InvalidInputSyntax {
                typ: "bit",
                input: "1 0".to_string()
            }
        );
        assert_eq!(e.message(), "invalid input syntax for type bit: \"1 0\"");

        assert!(matches!(
            input(oid::BIT, "102"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
        assert!(matches!(
            input(oid::BIT, "a5"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn varbit_reject_uses_bit_varying_type_name() {

        let e = input(oid::VARBIT, "2").unwrap_err();
        assert_eq!(
            e,
            PgError::InvalidInputSyntax {
                typ: "bit varying",
                input: "2".to_string()
            }
        );
        assert_eq!(
            e.message(),
            "invalid input syntax for type bit varying: \"2\""
        );
    }

    #[test]
    fn varbit_and_bit_share_base_behavior() {

        for s in ["", "0", "1", "010101", "01010101010"] {
            assert_eq!(parse(oid::BIT, s), parse(oid::VARBIT, s));
        }
    }

    #[test]
    fn output_round_trips() {
        assert_eq!(
            output(oid::BIT, &parse(oid::BIT, "11011000000")),
            "11011000000"
        );
        assert_eq!(output(oid::VARBIT, &parse(oid::VARBIT, "010101")), "010101");
        assert_eq!(output(oid::VARBIT, &parse(oid::VARBIT, "")), "");
    }

    #[test]
    fn output_non_text_is_empty() {
        assert_eq!(output(oid::BIT, &SqlValue::Null), "");
        assert_eq!(output(oid::VARBIT, &SqlValue::Int(42)), "");
    }

    #[test]
    fn typmod_length_rules_deferred() {

        assert_eq!(parse(oid::BIT, "10"), SqlValue::Text("10".to_string()));
        assert_eq!(
            parse(oid::BIT, "101011111010"),
            SqlValue::Text("101011111010".to_string())
        );
        assert_eq!(
            parse(oid::VARBIT, "101011111010"),
            SqlValue::Text("101011111010".to_string())
        );
    }
}
