
use crate::types::PgError;
use sql_core::SqlValue;

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {

    let arity: usize = match name {
        "sin" | "cos" | "tan" | "cot" | "asin" | "acos" | "atan" => 1,
        "atan2" => 2,
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

    let mut vals: [f64; 2] = [0.0; 2];
    for (i, a) in args.iter().enumerate() {
        match crate::expr::arg_f64(a) {
            Some(f) => vals[i] = f,
            None => {
                return Some(Err(PgError::InvalidInputSyntax {
                    typ: "expression",
                    input: format!("function {name}(...) does not exist"),
                }));
            }
        }
    }

    let result: f64 = match name {
        "sin" => vals[0].sin(),
        "cos" => vals[0].cos(),
        "tan" => vals[0].tan(),
        "cot" => 1.0 / vals[0].tan(),
        "asin" => {
            let x = vals[0];
            if !(-1.0..=1.0).contains(&x) {
                return Some(Err(PgError::InvalidInputSyntax {
                    typ: "asin",
                    input: x.to_string(),
                }));
            }
            x.asin()
        }
        "acos" => {
            let x = vals[0];
            if !(-1.0..=1.0).contains(&x) {
                return Some(Err(PgError::InvalidInputSyntax {
                    typ: "acos",
                    input: x.to_string(),
                }));
            }
            x.acos()
        }
        "atan" => vals[0].atan(),

        "atan2" => vals[0].atan2(vals[1]),
        _ => unreachable!("arity table already filtered non-family names"),
    };

    Some(Ok(SqlValue::Real(result)))
}

#[cfg(test)]
mod tests {
    use super::call;
    use sql_core::SqlValue;

    fn real(v: &Option<Result<SqlValue, crate::types::PgError>>) -> f64 {
        match v {
            Some(Ok(SqlValue::Real(f))) => *f,
            other => panic!("expected Real, got {other:?}"),
        }
    }

    #[test]
    fn sin_cos_tan_at_zero() {
        assert_eq!(real(&call("sin", &[SqlValue::Int(0)])), 0.0);
        assert_eq!(real(&call("cos", &[SqlValue::Int(0)])), 1.0);
        assert_eq!(real(&call("tan", &[SqlValue::Int(0)])), 0.0);
    }

    #[test]
    fn cot_is_reciprocal_of_tan() {

        let x = std::f64::consts::FRAC_PI_4;
        let got = real(&call("cot", &[SqlValue::Real(x)]));
        assert!((got - 1.0).abs() < 1e-9, "cot(pi/4) = {got}");
    }

    #[test]
    fn inverse_functions() {
        assert!((real(&call("asin", &[SqlValue::Int(0)])) - 0.0).abs() < 1e-12);
        assert!((real(&call("acos", &[SqlValue::Int(1)])) - 0.0).abs() < 1e-12);
        assert!((real(&call("atan", &[SqlValue::Int(0)])) - 0.0).abs() < 1e-12);
    }

    #[test]
    fn atan2_two_arg() {

        let got = real(&call("atan2", &[SqlValue::Int(1), SqlValue::Int(1)]));
        assert!(
            (got - std::f64::consts::FRAC_PI_4).abs() < 1e-12,
            "got {got}"
        );
    }

    #[test]
    fn asin_acos_out_of_range() {
        assert!(matches!(
            call("asin", &[SqlValue::Real(2.0)]),
            Some(Err(crate::types::PgError::InvalidInputSyntax {
                typ: "asin",
                ..
            }))
        ));
        assert!(matches!(
            call("acos", &[SqlValue::Real(-1.5)]),
            Some(Err(crate::types::PgError::InvalidInputSyntax {
                typ: "acos",
                ..
            }))
        ));
    }

    #[test]
    fn null_propagation() {
        assert!(matches!(
            call("sin", &[SqlValue::Null]),
            Some(Ok(SqlValue::Null))
        ));
        assert!(matches!(
            call("atan2", &[SqlValue::Null, SqlValue::Int(1)]),
            Some(Ok(SqlValue::Null))
        ));
        assert!(matches!(
            call("atan2", &[SqlValue::Int(1), SqlValue::Null]),
            Some(Ok(SqlValue::Null))
        ));
    }

    #[test]
    fn wrong_arity_is_error() {
        assert!(matches!(
            call("sin", &[SqlValue::Int(0), SqlValue::Int(1)]),
            Some(Err(crate::types::PgError::InvalidInputSyntax {
                typ: "expression",
                ..
            }))
        ));
        assert!(matches!(
            call("atan2", &[SqlValue::Int(1)]),
            Some(Err(crate::types::PgError::InvalidInputSyntax {
                typ: "expression",
                ..
            }))
        ));
    }

    #[test]
    fn unclaimed_names_return_none() {

        assert!(call("sind", &[SqlValue::Int(0)]).is_none());
        assert!(call("cosd", &[SqlValue::Int(0)]).is_none());
        assert!(call("sqrt", &[SqlValue::Int(4)]).is_none());
    }
}
