
use crate::types::PgError;
use sql_core::SqlValue;

pub fn binary(op: &str, l: &SqlValue, r: &SqlValue) -> Option<Result<SqlValue, PgError>> {

    match op {
        "&" | "|" | "#" | "<<" | ">>" => {}
        _ => return None,
    }

    if matches!(l, SqlValue::Null) || matches!(r, SqlValue::Null) {
        return Some(Ok(SqlValue::Null));
    }

    let (a, b) = match (l, r) {
        (SqlValue::Int(a), SqlValue::Int(b)) => (*a, *b),
        _ => return None,
    };
    let out = match op {
        "&" => a & b,
        "|" => a | b,
        "#" => a ^ b,
        "<<" => a.wrapping_shl(b as u32),
        ">>" => a.wrapping_shr(b as u32),
        _ => unreachable!("guarded above"),
    };
    Some(Ok(SqlValue::Int(out)))
}

pub fn unary(op: &str, v: &SqlValue) -> Option<Result<SqlValue, PgError>> {
    if op != "~" {
        return None;
    }
    if matches!(v, SqlValue::Null) {
        return Some(Ok(SqlValue::Null));
    }
    match v {
        SqlValue::Int(a) => Some(Ok(SqlValue::Int(!a))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{binary, unary};
    use sql_core::SqlValue;

    fn int(i: i64) -> SqlValue {
        SqlValue::Int(i)
    }

    #[test]
    fn and_or_xor() {
        assert_eq!(
            binary("&", &int(123456), &int(-123456)),
            Some(Ok(int(123456 & -123456)))
        );
        assert_eq!(
            binary("|", &int(123456), &int(-123456)),
            Some(Ok(int(123456 | -123456)))
        );
        assert_eq!(
            binary("#", &int(123456), &int(-123456)),
            Some(Ok(int(123456 ^ -123456)))
        );

        assert_eq!(binary("&", &int(6), &int(3)), Some(Ok(int(2))));
        assert_eq!(binary("|", &int(6), &int(3)), Some(Ok(int(7))));
        assert_eq!(binary("#", &int(6), &int(3)), Some(Ok(int(5))));
    }

    #[test]
    fn shifts() {
        assert_eq!(binary("<<", &int(1), &int(4)), Some(Ok(int(16))));
        assert_eq!(binary(">>", &int(16), &int(2)), Some(Ok(int(4))));

        assert_eq!(binary("<<", &int(-1), &int(31)), Some(Ok(int(-2147483648))));
        assert_eq!(binary(">>", &int(-8), &int(1)), Some(Ok(int(-4))));
    }

    #[test]
    fn not_prefix() {
        assert_eq!(unary("~", &int(0)), Some(Ok(int(-1))));
        assert_eq!(unary("~", &int(-1)), Some(Ok(int(0))));
        assert_eq!(unary("~", &int(123456)), Some(Ok(int(!123456i64))));
    }

    #[test]
    fn null_propagation() {
        assert_eq!(
            binary("&", &SqlValue::Null, &int(5)),
            Some(Ok(SqlValue::Null))
        );
        assert_eq!(
            binary("<<", &int(5), &SqlValue::Null),
            Some(Ok(SqlValue::Null))
        );
        assert_eq!(unary("~", &SqlValue::Null), Some(Ok(SqlValue::Null)));
    }

    #[test]
    fn non_integer_declines() {
        assert_eq!(binary("&", &SqlValue::Real(1.0), &int(5)), None);
        assert_eq!(binary("|", &int(5), &SqlValue::Text("x".into())), None);
        assert_eq!(unary("~", &SqlValue::Real(1.0)), None);
    }

    #[test]
    fn unclaimed_ops_decline() {
        assert_eq!(binary("+", &int(1), &int(2)), None);
        assert_eq!(binary("~", &int(1), &int(2)), None);
        assert_eq!(unary("-", &int(1)), None);
    }
}
