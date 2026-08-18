
use crate::types::PgError;
use sql_core::SqlValue;

fn wrong(name: &str) -> Option<Result<SqlValue, PgError>> {
    Some(Err(PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("function {name}(...) does not exist"),
    }))
}

fn domain_err(fnname: &'static str, value: f64) -> Option<Result<SqlValue, PgError>> {
    Some(Err(PgError::InvalidInputSyntax {
        typ: fnname,
        input: value.to_string(),
    }))
}

fn any_null(args: &[SqlValue]) -> bool {
    args.iter().any(|v| matches!(v, SqlValue::Null))
}

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    let lname = name.to_ascii_lowercase();
    match lname.as_str() {
        "sqrt" | "cbrt" | "exp" | "ln" | "log10" => {
            if args.len() != 1 {
                return wrong(name);
            }
            if any_null(args) {
                return Some(Ok(SqlValue::Null));
            }
            let x = match crate::expr::arg_f64(&args[0]) {
                Some(x) => x,
                None => return wrong(name),
            };
            let r = match lname.as_str() {
                "sqrt" => {
                    if x < 0.0 {
                        return domain_err("sqrt", x);
                    }
                    x.sqrt()
                }
                "cbrt" => x.cbrt(),
                "exp" => x.exp(),
                "ln" => {
                    if x == 0.0 {
                        return domain_err("ln", x);
                    }
                    if x < 0.0 {
                        return domain_err("ln", x);
                    }
                    x.ln()
                }
                "log10" => {
                    if x == 0.0 {
                        return domain_err("log10", x);
                    }
                    if x < 0.0 {
                        return domain_err("log10", x);
                    }
                    x.log10()
                }
                _ => unreachable!(),
            };
            Some(Ok(SqlValue::Real(r)))
        }

        "log" => match args.len() {
            1 => {
                if any_null(args) {
                    return Some(Ok(SqlValue::Null));
                }
                let x = match crate::expr::arg_f64(&args[0]) {
                    Some(x) => x,
                    None => return wrong(name),
                };
                if x == 0.0 || x < 0.0 {
                    return domain_err("log", x);
                }
                Some(Ok(SqlValue::Real(x.log10())))
            }
            2 => {
                if any_null(args) {
                    return Some(Ok(SqlValue::Null));
                }
                let b = match crate::expr::arg_f64(&args[0]) {
                    Some(b) => b,
                    None => return wrong(name),
                };
                let x = match crate::expr::arg_f64(&args[1]) {
                    Some(x) => x,
                    None => return wrong(name),
                };
                if b == 0.0 || b < 0.0 {
                    return domain_err("log", b);
                }
                if x == 0.0 || x < 0.0 {
                    return domain_err("log", x);
                }
                Some(Ok(SqlValue::Real(x.log(b))))
            }
            _ => wrong(name),
        },
        "power" => {
            if args.len() != 2 {
                return wrong(name);
            }
            if any_null(args) {
                return Some(Ok(SqlValue::Null));
            }
            let x = match crate::expr::arg_f64(&args[0]) {
                Some(x) => x,
                None => return wrong(name),
            };
            let y = match crate::expr::arg_f64(&args[1]) {
                Some(y) => y,
                None => return wrong(name),
            };

            Some(Ok(SqlValue::Real(x.powf(y))))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::call;
    use sql_core::SqlValue;

    fn real(name: &str, args: &[SqlValue]) -> f64 {
        match call(name, args) {
            Some(Ok(SqlValue::Real(r))) => r,
            other => panic!("expected Real for {name}, got {other:?}"),
        }
    }

    #[test]
    fn sqrt_cbrt() {
        assert_eq!(real("sqrt", &[SqlValue::Real(64.0)]), 8.0);
        assert_eq!(real("cbrt", &[SqlValue::Real(27.0)]), 3.0);
    }

    #[test]
    fn exp_ln_roundtrip() {
        assert!((real("exp", &[SqlValue::Int(0)]) - 1.0).abs() < 1e-12);
        assert!((real("ln", &[SqlValue::Real(1.0)]) - 0.0).abs() < 1e-12);
    }

    #[test]
    fn power_and_zero_zero() {
        assert_eq!(
            real("power", &[SqlValue::Int(2), SqlValue::Int(10)]),
            1024.0
        );

        assert_eq!(real("power", &[SqlValue::Int(0), SqlValue::Int(0)]), 1.0);
    }

    #[test]
    fn log_one_arg_is_base10() {
        assert!((real("log", &[SqlValue::Real(1000.0)]) - 3.0).abs() < 1e-12);
        assert!((real("log10", &[SqlValue::Real(1000.0)]) - 3.0).abs() < 1e-12);
    }

    #[test]
    fn log_two_arg_is_base_b() {

        assert!((real("log", &[SqlValue::Real(2.0), SqlValue::Real(8.0)]) - 3.0).abs() < 1e-12);
    }

    #[test]
    fn domain_errors() {

        assert!(matches!(
            call("sqrt", &[SqlValue::Real(-1.0)]),
            Some(Err(_))
        ));

        assert!(matches!(call("ln", &[SqlValue::Real(0.0)]), Some(Err(_))));

        assert!(matches!(call("ln", &[SqlValue::Real(-2.0)]), Some(Err(_))));

        assert!(matches!(call("log", &[SqlValue::Real(0.0)]), Some(Err(_))));
    }

    #[test]
    fn null_propagation() {
        assert!(matches!(
            call("sqrt", &[SqlValue::Null]),
            Some(Ok(SqlValue::Null))
        ));
        assert!(matches!(
            call("power", &[SqlValue::Int(2), SqlValue::Null]),
            Some(Ok(SqlValue::Null))
        ));
        assert!(matches!(
            call("log", &[SqlValue::Null, SqlValue::Real(8.0)]),
            Some(Ok(SqlValue::Null))
        ));
    }

    #[test]
    fn unclaimed_name_returns_none() {
        assert!(call("sin", &[SqlValue::Real(1.0)]).is_none());
        assert!(call("concat", &[]).is_none());
    }

    #[test]
    fn wrong_arity_is_err_not_none() {

        assert!(matches!(
            call("sqrt", &[SqlValue::Int(1), SqlValue::Int(2)]),
            Some(Err(_))
        ));

        assert!(matches!(call("power", &[SqlValue::Int(2)]), Some(Err(_))));
    }
}
