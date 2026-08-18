
use crate::types::PgError;
use sql_core::SqlValue;

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    let arity = match name {
        "gcd" | "lcm" | "mod" | "div" => 2,
        "factorial" => 1,
        _ => return None,
    };

    if args.len() != arity {
        return Some(Err(PgError::InvalidInputSyntax {
            typ: "expression",
            input: format!("function {name}(...) does not exist"),
        }));
    }

    if args.iter().any(|a| matches!(a, SqlValue::Null)) {
        return Some(Ok(SqlValue::Null));
    }

    if name == "mod" {
        if let Some(res) = crate::types::numeric::arith('%', &args[0], &args[1]) {
            return Some(res);
        }
    }

    let ints: Option<Vec<i64>> = args
        .iter()
        .map(|a| match a {
            SqlValue::Int(i) => Some(*i),
            _ => None,
        })
        .collect();
    let ints = match ints {
        Some(v) => v,

        None => return None,
    };

    let result = match name {
        "gcd" => gcd(ints[0], ints[1]),
        "lcm" => lcm(ints[0], ints[1]),
        "factorial" => factorial(ints[0]),

        "mod" => imod(ints[0], ints[1]),

        "div" => idiv(ints[0], ints[1]),
        _ => unreachable!(),
    };

    Some(result.map(SqlValue::Int))
}

const OVERFLOW: PgError = PgError::Overflow { typ: "bigint" };

fn checked_abs(x: i64) -> Result<i64, PgError> {
    x.checked_abs().ok_or(OVERFLOW)
}

fn gcd(a: i64, b: i64) -> Result<i64, PgError> {

    let mut a = a;
    let mut b = b;
    while b != 0 {
        let r = a.wrapping_rem(b);
        a = b;
        b = r;
    }
    checked_abs(a)
}

fn lcm(a: i64, b: i64) -> Result<i64, PgError> {
    if a == 0 || b == 0 {
        return Ok(0);
    }
    let g = gcd(a, b)?;

    let a_abs = checked_abs(a)?;
    let b_abs = checked_abs(b)?;
    (a_abs / g).checked_mul(b_abs).ok_or(OVERFLOW)
}

fn factorial(n: i64) -> Result<i64, PgError> {
    if n < 0 {
        return Err(PgError::InvalidInputSyntax {
            typ: "expression",
            input: "factorial of a negative number is undefined".to_string(),
        });
    }
    let mut acc: i64 = 1;
    let mut k: i64 = 2;
    while k <= n {
        acc = acc.checked_mul(k).ok_or(OVERFLOW)?;
        k += 1;
    }
    Ok(acc)
}

fn imod(x: i64, y: i64) -> Result<i64, PgError> {
    if y == 0 {
        return Err(PgError::DivisionByZero);
    }
    x.checked_rem(y).ok_or(OVERFLOW)
}

fn idiv(y: i64, x: i64) -> Result<i64, PgError> {
    if x == 0 {
        return Err(PgError::DivisionByZero);
    }
    y.checked_div(x).ok_or(OVERFLOW)
}

#[cfg(test)]
mod tests {
    use super::call;
    use crate::types::PgError;
    use sql_core::SqlValue;

    fn i(n: i64) -> SqlValue {
        SqlValue::Int(n)
    }

    fn int_of(v: Option<Result<SqlValue, PgError>>) -> i64 {
        match v {
            Some(Ok(SqlValue::Int(n))) => n,
            other => panic!("expected Some(Ok(Int)), got {other:?}"),
        }
    }

    #[test]
    fn gcd_basic() {
        assert_eq!(int_of(call("gcd", &[i(0), i(0)])), 0);
        assert_eq!(int_of(call("gcd", &[i(0), i(29893644334)])), 29893644334);
        assert_eq!(
            int_of(call("gcd", &[i(288484263558), i(29893644334)])),
            6835958
        );

        assert_eq!(
            int_of(call("gcd", &[i(-288484263558), i(29893644334)])),
            6835958
        );
        assert_eq!(int_of(call("gcd", &[i(i64::MIN), i(1)])), 1);
    }

    #[test]
    fn gcd_overflow() {

        assert!(matches!(
            call("gcd", &[i(i64::MIN), i(0)]),
            Some(Err(PgError::Overflow { typ: "bigint" }))
        ));
        assert!(matches!(
            call("gcd", &[i(i64::MIN), i(i64::MIN)]),
            Some(Err(PgError::Overflow { typ: "bigint" }))
        ));
    }

    #[test]
    fn lcm_basic() {
        assert_eq!(int_of(call("lcm", &[i(0), i(0)])), 0);
        assert_eq!(int_of(call("lcm", &[i(0), i(29893644334)])), 0);
        assert_eq!(
            int_of(call("lcm", &[i(288484263558), i(29893644334)])),
            1261541684539134
        );
        assert_eq!(
            int_of(call("lcm", &[i(-288484263558), i(29893644334)])),
            1261541684539134
        );
    }

    #[test]
    fn lcm_overflow() {
        assert!(matches!(
            call("lcm", &[i(9223372036854775807), i(9223372036854775806)]),
            Some(Err(PgError::Overflow { typ: "bigint" }))
        ));
    }

    #[test]
    fn factorial_basic() {
        assert_eq!(int_of(call("factorial", &[i(0)])), 1);
        assert_eq!(int_of(call("factorial", &[i(4)])), 24);
        assert_eq!(int_of(call("factorial", &[i(15)])), 1307674368000);
    }

    #[test]
    fn factorial_negative_is_undefined() {
        assert!(matches!(
            call("factorial", &[i(-4)]),
            Some(Err(PgError::InvalidInputSyntax {
                typ: "expression",
                ..
            }))
        ));
    }

    #[test]
    fn factorial_overflow() {
        assert!(matches!(
            call("factorial", &[i(100000)]),
            Some(Err(PgError::Overflow { typ: "bigint" }))
        ));
    }

    #[test]
    fn mod_basic() {
        assert_eq!(int_of(call("mod", &[i(11), i(4)])), 3);

        assert_eq!(int_of(call("mod", &[i(-11), i(4)])), -3);
        assert_eq!(int_of(call("mod", &[i(11), i(-4)])), 3);
    }

    #[test]
    fn div_truncates_toward_zero() {
        assert_eq!(int_of(call("div", &[i(11), i(4)])), 2);
        assert_eq!(int_of(call("div", &[i(-11), i(4)])), -2);
        assert_eq!(int_of(call("div", &[i(11), i(-4)])), -2);
    }

    #[test]
    fn div_and_mod_by_zero() {
        assert!(matches!(
            call("div", &[i(1), i(0)]),
            Some(Err(PgError::DivisionByZero))
        ));
        assert!(matches!(
            call("mod", &[i(1), i(0)]),
            Some(Err(PgError::DivisionByZero))
        ));
    }

    #[test]
    fn null_propagation() {
        assert!(matches!(
            call("gcd", &[SqlValue::Null, i(4)]),
            Some(Ok(SqlValue::Null))
        ));
        assert!(matches!(
            call("factorial", &[SqlValue::Null]),
            Some(Ok(SqlValue::Null))
        ));
        assert!(matches!(
            call("mod", &[i(1), SqlValue::Null]),
            Some(Ok(SqlValue::Null))
        ));
    }

    #[test]
    fn wrong_arity_is_claimed_error() {
        assert!(matches!(
            call("gcd", &[i(1)]),
            Some(Err(PgError::InvalidInputSyntax {
                typ: "expression",
                ..
            }))
        ));
    }

    #[test]
    fn unclaimed_name_returns_none() {
        assert!(call("sqrt", &[i(4)]).is_none());
        assert!(call("abs", &[i(4)]).is_none());
    }
}
