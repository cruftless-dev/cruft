
use super::PgError;
use sql_core::SqlValue;

fn err(input: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: super::type_name(super::oid::TID),
        input: input.to_string(),
    }
}

fn parse_block(field: &str) -> Option<u32> {
    if let Some(mag) = field.strip_prefix('-') {

        let mag: u64 = mag.parse().ok()?;
        if mag == 0 || mag > u32::MAX as u64 {
            return None;
        }
        Some((mag as u32).wrapping_neg())
    } else {

        let val: u64 = field.parse().ok()?;
        if val > u32::MAX as u64 {
            return None;
        }
        Some(val as u32)
    }
}

fn parse_offset(field: &str) -> Option<u16> {
    field.parse().ok()
}

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    let _ = oid;

    let trimmed = text.trim();

    let body = trimmed
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .ok_or_else(|| err(text))?;

    let (block_field, offset_field) = match body.split_once(',') {
        Some((b, o)) if !o.contains(',') => (b.trim(), o.trim()),
        _ => return Err(err(text)),
    };

    let block = parse_block(block_field).ok_or_else(|| err(text))?;
    let offset = parse_offset(offset_field).ok_or_else(|| err(text))?;

    Ok(SqlValue::Text(format!("({block},{offset})")))
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
        input(oid::TID, s).expect("expected a valid tid literal")
    }

    #[test]
    fn canonical_round_trip() {

        assert_eq!(parse("(0,1)"), SqlValue::Text("(0,1)".to_string()));
        assert_eq!(output(oid::TID, &parse("(0,1)")), "(0,1)");

        assert_eq!(parse("(0,0)"), SqlValue::Text("(0,0)".to_string()));
    }

    #[test]
    fn max_block_and_offset() {

        assert_eq!(
            parse("(4294967295,65535)"),
            SqlValue::Text("(4294967295,65535)".to_string())
        );
        assert_eq!(
            output(oid::TID, &parse("(4294967295,65535)")),
            "(4294967295,65535)"
        );
    }

    #[test]
    fn negative_block_wraps() {

        assert_eq!(
            parse("(-1,0)"),
            SqlValue::Text("(4294967295,0)".to_string())
        );
    }

    #[test]
    fn reject_block_overflow() {

        let bad = "(4294967296,1)";
        let e = input(oid::TID, bad).unwrap_err();
        assert_eq!(
            e,
            PgError::InvalidInputSyntax {
                typ: "tid",
                input: bad.to_string()
            }
        );
        assert_eq!(
            e.message(),
            "invalid input syntax for type tid: \"(4294967296,1)\""
        );
    }

    #[test]
    fn reject_offset_overflow() {

        assert!(matches!(
            input(oid::TID, "(1,65536)"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_negative_offset() {

        assert!(matches!(
            input(oid::TID, "(0,-1)"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_missing_paren() {

        assert!(matches!(
            input(oid::TID, "0,1"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
        assert!(matches!(
            input(oid::TID, "(0,1"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_wrong_arity() {

        assert!(matches!(
            input(oid::TID, "(0)"),
            Err(PgError::InvalidInputSyntax { .. })
        ));

        assert!(matches!(
            input(oid::TID, "(0,1,2)"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_non_numeric() {
        assert!(matches!(
            input(oid::TID, "(a,1)"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
        assert!(matches!(
            input(oid::TID, "(0,b)"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn output_non_text_is_empty() {
        assert_eq!(output(oid::TID, &SqlValue::Null), "");
        assert_eq!(output(oid::TID, &SqlValue::Int(42)), "");
    }
}
