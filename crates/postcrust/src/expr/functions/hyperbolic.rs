
use crate::types::PgError;
use sql_core::SqlValue;

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {

    if name == "pi" {
        if !args.is_empty() {
            return Some(Err(arity_err(name)));
        }
        return Some(Ok(SqlValue::Real(std::f64::consts::PI)));
    }

    let is_member = matches!(
        name,
        "sinh" | "cosh" | "tanh" | "asinh" | "acosh" | "atanh" | "degrees" | "radians"
    );
    if !is_member {
        return None;
    }

    if args.len() != 1 {
        return Some(Err(arity_err(name)));
    }

    if matches!(args[0], SqlValue::Null) {
        return Some(Ok(SqlValue::Null));
    }

    let x = match crate::expr::arg_f64(&args[0]) {
        Some(v) => v,
        None => return Some(Err(arity_err(name))),
    };

    let out = match name {
        "sinh" => x.sinh(),
        "cosh" => x.cosh(),
        "tanh" => x.tanh(),
        "asinh" => x.asinh(),
        "acosh" => {
            if x < 1.0 {
                return Some(Err(domain_err("acosh", x)));
            }
            x.acosh()
        }
        "atanh" => {
            if x.abs() >= 1.0 {
                return Some(Err(domain_err("atanh", x)));
            }
            x.atanh()
        }
        "degrees" => x.to_degrees(),
        "radians" => x.to_radians(),
        _ => unreachable!("membership checked above"),
    };

    Some(Ok(SqlValue::Real(out)))
}

fn arity_err(name: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("function {name}(...) does not exist"),
    }
}

fn domain_err(name: &'static str, value: f64) -> PgError {
    PgError::InvalidInputSyntax {
        typ: name,
        input: value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::call;
    use sql_core::SqlValue;

    fn real(name: &str, x: f64) -> f64 {
        match call(name, &[SqlValue::Real(x)]) {
            Some(Ok(SqlValue::Real(v))) => v,
            other => panic!("expected Real from {name}, got {other:?}"),
        }
    }

    #[test]
    fn sinh_zero() {
        assert_eq!(real("sinh", 0.0), 0.0);
    }

    #[test]
    fn cosh_zero() {
        assert_eq!(real("cosh", 0.0), 1.0);
    }

    #[test]
    fn tanh_zero() {
        assert_eq!(real("tanh", 0.0), 0.0);
    }

    #[test]
    fn asinh_roundtrip() {
        assert!((real("asinh", 0.0)).abs() < 1e-12);
    }

    #[test]
    fn acosh_one() {
        assert!(real("acosh", 1.0).abs() < 1e-12);
    }

    #[test]
    fn atanh_zero() {
        assert_eq!(real("atanh", 0.0), 0.0);
    }

    #[test]
    fn degrees_of_pi() {
        assert!((real("degrees", std::f64::consts::PI) - 180.0).abs() < 1e-9);
    }

    #[test]
    fn radians_of_180() {
        assert!((real("radians", 180.0) - std::f64::consts::PI).abs() < 1e-12);
    }

    #[test]
    fn pi_value() {
        match call("pi", &[]) {
            Some(Ok(SqlValue::Real(v))) => {
                assert!((v - std::f64::consts::PI).abs() < 1e-15)
            }
            other => panic!("expected pi(), got {other:?}"),
        }
    }

    #[test]
    fn acosh_domain_error() {
        match call("acosh", &[SqlValue::Real(0.0)]) {
            Some(Err(_)) => {}
            other => panic!("expected domain error for acosh(0), got {other:?}"),
        }
    }

    #[test]
    fn atanh_domain_error() {
        match call("atanh", &[SqlValue::Real(1.0)]) {
            Some(Err(_)) => {}
            other => panic!("expected domain error for atanh(1), got {other:?}"),
        }
    }

    #[test]
    fn null_propagates() {
        assert!(matches!(
            call("sinh", &[SqlValue::Null]),
            Some(Ok(SqlValue::Null))
        ));
    }

    #[test]
    fn int_arg_coerces() {
        assert_eq!(real_from_int("cosh", 0), 1.0);
    }

    fn real_from_int(name: &str, n: i64) -> f64 {
        match call(name, &[SqlValue::Int(n)]) {
            Some(Ok(SqlValue::Real(v))) => v,
            other => panic!("expected Real, got {other:?}"),
        }
    }

    #[test]
    fn pi_wrong_arity() {
        assert!(matches!(call("pi", &[SqlValue::Real(1.0)]), Some(Err(_))));
    }

    #[test]
    fn unary_wrong_arity() {
        assert!(matches!(call("sinh", &[]), Some(Err(_))));
    }

    #[test]
    fn unclaimed_name_is_none() {
        assert!(call("sin", &[SqlValue::Real(0.0)]).is_none());
    }
}
