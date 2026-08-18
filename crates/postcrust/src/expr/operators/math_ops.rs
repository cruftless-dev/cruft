
use crate::types::PgError;
use sql_core::SqlValue;

pub fn binary(op: &str, l: &SqlValue, r: &SqlValue) -> Option<Result<SqlValue, PgError>> {
    let _ = (op, l, r);
    None
}

pub fn unary(op: &str, v: &SqlValue) -> Option<Result<SqlValue, PgError>> {

    if !matches!(op, "@" | "|/" | "||/") {
        return None;
    }

    if matches!(v, SqlValue::Null) {
        return Some(Ok(SqlValue::Null));
    }

    match op {

        "@" => match v {
            SqlValue::Int(n) => Some(match n.checked_abs() {
                Some(a) => Ok(SqlValue::Int(a)),
                None => Err(PgError::Overflow { typ: "bigint" }),
            }),
            SqlValue::Real(f) => Some(Ok(SqlValue::Real(f.abs()))),
            _ => None,
        },

        "|/" => {
            let f = crate::expr::arg_f64(v)?;
            if f < 0.0 {

                Some(Err(PgError::InvalidInputSyntax {
                    typ: "|/",
                    input: f.to_string(),
                }))
            } else {
                Some(Ok(SqlValue::Real(f.sqrt())))
            }
        }

        "||/" => {
            let f = crate::expr::arg_f64(v)?;
            Some(Ok(SqlValue::Real(f.cbrt())))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{binary, unary};
    use crate::types::PgError;
    use sql_core::SqlValue;

    #[test]
    fn abs_int() {
        assert_eq!(unary("@", &SqlValue::Int(5)), Some(Ok(SqlValue::Int(5))));
        assert_eq!(unary("@", &SqlValue::Int(-7)), Some(Ok(SqlValue::Int(7))));
    }

    #[test]
    fn abs_int_overflow() {
        assert_eq!(
            unary("@", &SqlValue::Int(i64::MIN)),
            Some(Err(PgError::Overflow { typ: "bigint" }))
        );
    }

    #[test]
    fn abs_real() {
        assert_eq!(
            unary("@", &SqlValue::Real(-3.5)),
            Some(Ok(SqlValue::Real(3.5)))
        );
        assert_eq!(
            unary("@", &SqlValue::Real(2.0)),
            Some(Ok(SqlValue::Real(2.0)))
        );
    }

    #[test]
    fn sqrt_perfect_square() {

        assert_eq!(
            unary("|/", &SqlValue::Real(64.0)),
            Some(Ok(SqlValue::Real(8.0)))
        );
        assert_eq!(
            unary("|/", &SqlValue::Int(9)),
            Some(Ok(SqlValue::Real(3.0)))
        );
    }

    #[test]
    fn sqrt_negative_is_err() {

        assert_eq!(
            unary("|/", &SqlValue::Real(-4.0)),
            Some(Err(PgError::InvalidInputSyntax {
                typ: "|/",
                input: (-4.0f64).to_string()
            }))
        );
    }

    #[test]
    fn cbrt_negative() {

        assert_eq!(
            unary("||/", &SqlValue::Real(-27.0)),
            Some(Ok(SqlValue::Real(-3.0)))
        );
        assert_eq!(
            unary("||/", &SqlValue::Int(27)),
            Some(Ok(SqlValue::Real(3.0)))
        );
    }

    #[test]
    fn null_propagates() {
        assert_eq!(unary("@", &SqlValue::Null), Some(Ok(SqlValue::Null)));
        assert_eq!(unary("|/", &SqlValue::Null), Some(Ok(SqlValue::Null)));
        assert_eq!(unary("||/", &SqlValue::Null), Some(Ok(SqlValue::Null)));
    }

    #[test]
    fn non_numeric_not_claimed() {
        assert_eq!(unary("@", &SqlValue::Text("x".into())), None);
        assert_eq!(unary("|/", &SqlValue::Text("x".into())), None);
    }

    #[test]
    fn unclaimed_op_is_none() {
        assert_eq!(unary("~", &SqlValue::Int(1)), None);
    }

    #[test]
    fn binary_always_none() {
        assert_eq!(binary("@", &SqlValue::Int(1), &SqlValue::Int(2)), None);
    }
}
