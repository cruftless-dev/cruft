
use super::PgError;
use sql_core::SqlValue;

fn err(input: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: super::type_name(super::oid::UUID),
        input: input.to_string(),
    }
}

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    let _ = oid;

    let body = match (text.starts_with('{'), text.ends_with('}')) {
        (true, true) => &text[1..text.len() - 1],
        (false, false) => text,
        _ => return Err(err(text)),
    };

    let mut hex = String::with_capacity(32);
    for c in body.chars() {
        match c {

            '-' => {
                if !matches!(hex.len(), 8 | 12 | 16 | 20) {
                    return Err(err(text));
                }
            }
            _ if c.is_ascii_hexdigit() => hex.push(c.to_ascii_lowercase()),
            _ => return Err(err(text)),
        }
    }

    if hex.len() != 32 {
        return Err(err(text));
    }

    let mut canon = String::with_capacity(36);
    for (i, c) in hex.chars().enumerate() {
        if matches!(i, 8 | 12 | 16 | 20) {
            canon.push('-');
        }
        canon.push(c);
    }

    Ok(SqlValue::Text(canon))
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

    const CANON: &str = "11111111-1111-1111-1111-111111111111";

    fn parse(s: &str) -> SqlValue {
        input(oid::UUID, s).expect("expected a valid uuid literal")
    }

    #[test]
    fn canonical_input_stored_verbatim() {
        assert_eq!(parse(CANON), SqlValue::Text(CANON.to_string()));
    }

    #[test]
    fn uppercase_normalized_to_lowercase() {

        assert_eq!(
            parse("3F3E3C3B-3A30-3938-3736-353433A2313E"),
            SqlValue::Text("3f3e3c3b-3a30-3938-3736-353433a2313e".to_string())
        );
    }

    #[test]
    fn braces_accepted() {

        assert_eq!(
            parse("{22222222-2222-2222-2222-222222222222}"),
            SqlValue::Text("22222222-2222-2222-2222-222222222222".to_string())
        );
    }

    #[test]
    fn hyphenless_accepted_and_canonicalized() {

        assert_eq!(
            parse("3f3e3c3b3a3039383736353433a2313e"),
            SqlValue::Text("3f3e3c3b-3a30-3938-3736-353433a2313e".to_string())
        );
    }

    #[test]
    fn reject_too_long() {

        let bad = "11111111-1111-1111-1111-111111111111F";
        let e = input(oid::UUID, bad).unwrap_err();
        assert_eq!(
            e,
            PgError::InvalidInputSyntax {
                typ: "uuid",
                input: bad.to_string()
            }
        );

        assert_eq!(
            e.message(),
            "invalid input syntax for type uuid: \"11111111-1111-1111-1111-111111111111F\""
        );
    }

    #[test]
    fn reject_too_short() {

        let bad = "{11111111-1111-1111-1111-11111111111}";
        assert!(matches!(
            input(oid::UUID, bad),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_wrong_hyphen_grouping_is_wrong_count() {

        let bad = "111-11111-1111-1111-1111-111111111111";
        assert!(matches!(
            input(oid::UUID, bad),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_non_hex_chars() {

        assert!(matches!(
            input(oid::UUID, "11111111-1111-1111-G111-111111111111"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
        assert!(matches!(
            input(oid::UUID, "11+11111-1111-1111-1111-111111111111"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_unmatched_brace_and_stray_space() {

        assert!(matches!(
            input(oid::UUID, "{22222222-2222-2222-2222-222222222222 "),
            Err(PgError::InvalidInputSyntax { .. })
        ));

        assert!(matches!(
            input(oid::UUID, "11"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn output_round_trips_stored_canonical() {
        let v = parse("{22222222-2222-2222-2222-222222222222}");
        assert_eq!(
            output(oid::UUID, &v),
            "22222222-2222-2222-2222-222222222222"
        );

        assert_eq!(output(oid::UUID, &parse(CANON)), CANON);
    }

    #[test]
    fn output_non_text_is_empty() {
        assert_eq!(output(oid::UUID, &SqlValue::Null), "");
        assert_eq!(output(oid::UUID, &SqlValue::Int(42)), "");
    }
}
