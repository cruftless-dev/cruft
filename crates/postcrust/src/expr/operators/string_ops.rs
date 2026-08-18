
use crate::types::PgError;
use sql_core::SqlValue;

fn as_text(v: &SqlValue) -> Option<String> {
    match v {
        SqlValue::Text(s) => Some(s.clone()),
        SqlValue::Int(i) => Some(i.to_string()),
        SqlValue::Real(f) => Some(f.to_string()),
        _ => None,
    }
}

pub fn binary(op: &str, l: &SqlValue, r: &SqlValue) -> Option<Result<SqlValue, PgError>> {
    if op != "||" {
        return None;
    }

    if matches!(l, SqlValue::Null) || matches!(r, SqlValue::Null) {
        return Some(Ok(SqlValue::Null));
    }

    let has_text = matches!(l, SqlValue::Text(_)) || matches!(r, SqlValue::Text(_));
    if !has_text {
        return Some(Err(PgError::InvalidInputSyntax {
            typ: "expression",
            input: "operator does not exist: || ".to_string(),
        }));
    }
    match (as_text(l), as_text(r)) {
        (Some(a), Some(b)) => Some(Ok(SqlValue::Text(a + &b))),
        _ => Some(Err(PgError::InvalidInputSyntax {
            typ: "expression",
            input: "operator does not exist: || ".to_string(),
        })),
    }
}

pub fn unary(op: &str, v: &SqlValue) -> Option<Result<SqlValue, PgError>> {
    let _ = (op, v);
    None
}

#[cfg(test)]
mod tests {
    use super::{binary, unary};
    use sql_core::SqlValue;

    #[test]
    fn text_concat() {
        assert!(matches!(
            binary("||", &SqlValue::Text("a".into()), &SqlValue::Text("b".into())),
            Some(Ok(SqlValue::Text(ref s))) if s == "ab"
        ));
    }

    #[test]
    fn text_null_is_null() {
        assert!(matches!(
            binary("||", &SqlValue::Text("a".into()), &SqlValue::Null),
            Some(Ok(SqlValue::Null))
        ));
        assert!(matches!(
            binary("||", &SqlValue::Null, &SqlValue::Text("a".into())),
            Some(Ok(SqlValue::Null))
        ));
    }

    #[test]
    fn numeric_coercion() {
        assert!(matches!(
            binary("||", &SqlValue::Text("x".into()), &SqlValue::Int(1)),
            Some(Ok(SqlValue::Text(ref s))) if s == "x1"
        ));
        assert!(matches!(
            binary("||", &SqlValue::Text("four: ".into()), &SqlValue::Real(4.0)),
            Some(Ok(SqlValue::Text(ref s))) if s == "four: 4"
        ));
    }

    #[test]
    fn no_text_operand_errors() {
        assert!(matches!(
            binary("||", &SqlValue::Int(3), &SqlValue::Int(4)),
            Some(Err(_))
        ));
    }

    #[test]
    fn unclaimed_op_falls_through() {
        assert!(binary(
            "&&",
            &SqlValue::Text("a".into()),
            &SqlValue::Text("b".into())
        )
        .is_none());
    }

    #[test]
    fn unary_is_none() {
        assert!(unary("||", &SqlValue::Text("a".into())).is_none());
    }
}
