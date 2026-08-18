
use super::PgError;
use sql_core::SqlValue;

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    let err = || PgError::InvalidInputSyntax {
        typ: super::type_name(oid),
        input: text.to_string(),
    };

    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(err());
    }
    let lower = trimmed.to_ascii_lowercase();
    let first = lower.as_bytes()[0];

    let matched = match first {
        b't' if "true".starts_with(&lower) => Some(true),
        b'f' if "false".starts_with(&lower) => Some(false),
        b'y' if "yes".starts_with(&lower) => Some(true),
        b'n' if "no".starts_with(&lower) => Some(false),

        b'o' if lower.len() >= 2 && "on".starts_with(&lower) => Some(true),
        b'o' if lower.len() >= 2 && "off".starts_with(&lower) => Some(false),
        b'1' if lower.len() == 1 => Some(true),
        b'0' if lower.len() == 1 => Some(false),
        _ => None,
    };

    match matched {
        Some(true) => Ok(SqlValue::Int(1)),
        Some(false) => Ok(SqlValue::Int(0)),
        None => Err(err()),
    }
}

pub fn output(_oid: u32, v: &SqlValue) -> String {
    match v {
        SqlValue::Int(1) => "t".to_string(),
        SqlValue::Int(0) => "f".to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::oid::BOOL;
    use super::*;

    fn t(s: &str) -> SqlValue {
        input(BOOL, s).unwrap_or_else(|e| panic!("expected {s:?} accepted: {}", e.message()))
    }

    fn is_true(s: &str) {
        assert_eq!(t(s), SqlValue::Int(1), "{s:?} should be true");
    }

    fn is_false(s: &str) {
        assert_eq!(t(s), SqlValue::Int(0), "{s:?} should be false");
    }

    fn is_err(s: &str) {
        assert!(input(BOOL, s).is_err(), "{s:?} should be an error");
    }

    #[test]
    fn true_tokens_and_prefixes() {
        for s in ["true", "t", "tr", "tru", "yes", "y", "ye", "on", "1"] {
            is_true(s);
        }
    }

    #[test]
    fn false_tokens_and_prefixes() {
        for s in [
            "false", "f", "fa", "fal", "fals", "no", "n", "off", "of", "0",
        ] {
            is_false(s);
        }
    }

    #[test]
    fn case_insensitive() {
        for s in ["TRUE", "True", "T", "YES", "On", "ON"] {
            is_true(s);
        }
        for s in ["FALSE", "False", "F", "NO", "Off", "OFF"] {
            is_false(s);
        }
    }

    #[test]
    fn whitespace_trimmed() {

        is_false("   f           ");
        is_true("   true   ");
        is_true("\tt\n");
        is_true(" 1 ");
    }

    #[test]
    fn rejects() {

        for s in [
            "test", "foo", "yeah", "nay", "o", "on_", "off_", "11", "000", "", "XXX", "  tru e ",
            "1.0", "yeah!", "2",
        ] {
            is_err(s);
        }
    }

    #[test]
    fn reject_message_matches_golden() {

        let e = input(BOOL, "foo").unwrap_err();
        assert_eq!(
            e.message(),
            "invalid input syntax for type boolean: \"foo\""
        );

        let e = input(BOOL, "  tru e ").unwrap_err();
        assert_eq!(
            e.message(),
            "invalid input syntax for type boolean: \"  tru e \""
        );
        let e = input(BOOL, "").unwrap_err();
        assert_eq!(e.message(), "invalid input syntax for type boolean: \"\"");
    }

    #[test]
    fn ambiguous_o_is_error_but_of_is_false() {

        is_err("o");
        is_false("of");
        is_true("on");
    }

    #[test]
    fn output_renders_t_and_f() {
        assert_eq!(output(BOOL, &SqlValue::Int(1)), "t");
        assert_eq!(output(BOOL, &SqlValue::Int(0)), "f");

        assert_eq!(output(BOOL, &SqlValue::Int(7)), "");
        assert_eq!(output(BOOL, &SqlValue::Null), "");
    }

    #[test]
    fn round_trip() {
        assert_eq!(output(BOOL, &t("yes")), "t");
        assert_eq!(output(BOOL, &t("off")), "f");
    }
}
