
use crate::types::PgError;
use sql_core::SqlValue;

const RADIANS_PER_DEGREE: f64 = std::f64::consts::PI / 180.0;

fn sind_q1(x: f64) -> f64 {
    let sin_30 = (30.0 * RADIANS_PER_DEGREE).sin();
    (x * RADIANS_PER_DEGREE).sin() / sin_30 / 2.0
}

fn cosd_q1(x: f64) -> f64 {
    let one_minus_cos_60 = 1.0 - (60.0 * RADIANS_PER_DEGREE).cos();
    (1.0 - (x * RADIANS_PER_DEGREE).cos()) / one_minus_cos_60 * -0.5 + 1.0
}

fn dsind(mut arg: f64) -> f64 {
    let mut sign = 1.0;

    arg %= 360.0;
    if arg < 0.0 {
        arg = -arg;
        sign = -sign;
    }
    if arg > 180.0 {
        arg -= 180.0;
        sign = -sign;
    }
    if arg > 90.0 {
        arg = 180.0 - arg;
    }
    if arg <= 45.0 {
        sign * sind_q1(arg)
    } else {
        sign * cosd_q1(90.0 - arg)
    }
}

fn dcosd(mut arg: f64) -> f64 {
    let mut sign = 1.0;
    arg %= 360.0;
    if arg < 0.0 {
        arg = -arg;
    }
    if arg > 180.0 {
        arg = 360.0 - arg;
    }
    if arg > 90.0 {
        arg = 180.0 - arg;
        sign = -sign;
    }
    if arg <= 45.0 {
        sign * cosd_q1(arg)
    } else {
        sign * sind_q1(90.0 - arg)
    }
}

fn dtand(mut arg: f64) -> f64 {
    let mut sign = 1.0;
    arg %= 360.0;
    if arg < 0.0 {
        arg = -arg;
        sign = -sign;
    }
    if arg > 180.0 {
        arg -= 180.0;
    }
    if arg > 90.0 {
        arg = 180.0 - arg;
        sign = -sign;
    }
    let tan_arg = if arg <= 45.0 {
        sind_q1(arg) / cosd_q1(arg)
    } else {
        cosd_q1(90.0 - arg) / sind_q1(90.0 - arg)
    };
    let tan_45 = sind_q1(45.0) / cosd_q1(45.0);
    let mut result = sign * (tan_arg / tan_45);

    if result == 0.0 {
        result = 0.0;
    }
    result
}

fn dcotd(mut arg: f64) -> f64 {
    let mut sign = 1.0;
    arg %= 360.0;
    if arg < 0.0 {
        arg = -arg;
        sign = -sign;
    }
    if arg > 180.0 {
        arg -= 180.0;
    }
    if arg > 90.0 {
        arg = 180.0 - arg;
        sign = -sign;
    }
    let cot_arg = if arg <= 45.0 {
        cosd_q1(arg) / sind_q1(arg)
    } else {
        sind_q1(90.0 - arg) / cosd_q1(90.0 - arg)
    };
    let cot_45 = cosd_q1(45.0) / sind_q1(45.0);
    let mut result = sign * (cot_arg / cot_45);
    if result == 0.0 {
        result = 0.0;
    }
    result
}

fn asind_q1(x: f64) -> f64 {
    let asin_0_5 = 0.5_f64.asin();
    let acos_0_5 = 0.5_f64.acos();
    if x <= 0.5 {
        (x.asin() / asin_0_5) * 30.0
    } else {
        90.0 - (x.acos() / acos_0_5) * 60.0
    }
}

fn acosd_q1(x: f64) -> f64 {
    let asin_0_5 = 0.5_f64.asin();
    let acos_0_5 = 0.5_f64.acos();
    if x <= 0.5 {
        90.0 - (x.asin() / asin_0_5) * 30.0
    } else {
        (x.acos() / acos_0_5) * 60.0
    }
}

fn dasind(x: f64) -> f64 {
    if x >= 0.0 {
        asind_q1(x)
    } else {
        -asind_q1(-x)
    }
}

fn dacosd(x: f64) -> f64 {
    if x >= 0.0 {
        acosd_q1(x)
    } else {
        90.0 + asind_q1(-x)
    }
}

fn datand(x: f64) -> f64 {
    let atan_1_0 = 1.0_f64.atan();
    (x.atan() / atan_1_0) * 45.0
}

fn datan2d(y: f64, x: f64) -> f64 {
    let atan_1_0 = 1.0_f64.atan();
    (y.atan2(x) / atan_1_0) * 45.0
}

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {

    let arity: usize = match name {
        "sind" | "cosd" | "tand" | "cotd" | "asind" | "acosd" | "atand" => 1,
        "atan2d" => 2,
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

    if matches!(name, "asind" | "acosd") && !(-1.0..=1.0).contains(&vals[0]) {
        let typ: &'static str = if name == "asind" { "asind" } else { "acosd" };
        return Some(Err(PgError::InvalidInputSyntax {
            typ,
            input: vals[0].to_string(),
        }));
    }

    let result: f64 = match name {
        "sind" => dsind(vals[0]),
        "cosd" => dcosd(vals[0]),
        "tand" => dtand(vals[0]),
        "cotd" => dcotd(vals[0]),
        "asind" => dasind(vals[0]),
        "acosd" => dacosd(vals[0]),
        "atand" => datand(vals[0]),

        "atan2d" => datan2d(vals[0], vals[1]),
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
    fn sind_exact_special_angles() {

        assert_eq!(real(&call("sind", &[SqlValue::Int(0)])), 0.0);
        assert_eq!(real(&call("sind", &[SqlValue::Int(30)])), 0.5);
        assert_eq!(real(&call("sind", &[SqlValue::Int(90)])), 1.0);
        assert_eq!(real(&call("sind", &[SqlValue::Int(150)])), 0.5);
        assert_eq!(real(&call("sind", &[SqlValue::Int(210)])), -0.5);
        assert_eq!(real(&call("sind", &[SqlValue::Int(270)])), -1.0);
    }

    #[test]
    fn cosd_exact_special_angles() {
        assert_eq!(real(&call("cosd", &[SqlValue::Int(0)])), 1.0);
        assert_eq!(real(&call("cosd", &[SqlValue::Int(60)])), 0.5);

        assert_eq!(real(&call("cosd", &[SqlValue::Int(90)])), 0.0);
        assert_eq!(real(&call("cosd", &[SqlValue::Int(120)])), -0.5);
        assert_eq!(real(&call("cosd", &[SqlValue::Int(180)])), -1.0);
        assert_eq!(real(&call("cosd", &[SqlValue::Int(270)])), 0.0);
    }

    #[test]
    fn tand_cotd_special_angles() {
        assert_eq!(real(&call("tand", &[SqlValue::Int(0)])), 0.0);
        assert_eq!(real(&call("tand", &[SqlValue::Int(45)])), 1.0);
        assert!(real(&call("tand", &[SqlValue::Int(90)])).is_infinite());
        assert_eq!(real(&call("tand", &[SqlValue::Int(135)])), -1.0);
        assert_eq!(real(&call("tand", &[SqlValue::Int(180)])), 0.0);
        assert!(real(&call("cotd", &[SqlValue::Int(0)])).is_infinite());
        assert_eq!(real(&call("cotd", &[SqlValue::Int(45)])), 1.0);
        assert_eq!(real(&call("cotd", &[SqlValue::Int(90)])), 0.0);
    }

    #[test]
    fn inverse_degree_exact() {
        assert_eq!(real(&call("asind", &[SqlValue::Int(-1)])), -90.0);
        assert_eq!(real(&call("asind", &[SqlValue::Real(-0.5)])), -30.0);
        assert_eq!(real(&call("asind", &[SqlValue::Int(0)])), 0.0);
        assert_eq!(real(&call("asind", &[SqlValue::Real(0.5)])), 30.0);
        assert_eq!(real(&call("asind", &[SqlValue::Int(1)])), 90.0);
        assert_eq!(real(&call("acosd", &[SqlValue::Int(-1)])), 180.0);
        assert_eq!(real(&call("acosd", &[SqlValue::Real(-0.5)])), 120.0);
        assert_eq!(real(&call("acosd", &[SqlValue::Int(0)])), 90.0);
        assert_eq!(real(&call("acosd", &[SqlValue::Real(0.5)])), 60.0);
        assert_eq!(real(&call("acosd", &[SqlValue::Int(1)])), 0.0);
        assert_eq!(real(&call("atand", &[SqlValue::Int(-1)])), -45.0);
        assert_eq!(real(&call("atand", &[SqlValue::Int(0)])), 0.0);
        assert_eq!(real(&call("atand", &[SqlValue::Int(1)])), 45.0);
        assert_eq!(real(&call("atand", &[SqlValue::Real(f64::INFINITY)])), 90.0);
        assert_eq!(
            real(&call("atand", &[SqlValue::Real(f64::NEG_INFINITY)])),
            -90.0
        );
    }

    #[test]
    fn atan2d_two_arg_exact() {

        assert_eq!(
            real(&call("atan2d", &[SqlValue::Int(0), SqlValue::Int(10)])),
            0.0
        );
        assert_eq!(
            real(&call("atan2d", &[SqlValue::Int(10), SqlValue::Int(0)])),
            90.0
        );
        assert_eq!(
            real(&call("atan2d", &[SqlValue::Int(0), SqlValue::Int(-10)])),
            180.0
        );
        assert_eq!(
            real(&call("atan2d", &[SqlValue::Int(-10), SqlValue::Int(0)])),
            -90.0
        );
    }

    #[test]
    fn asind_domain_error() {
        assert!(matches!(
            call("asind", &[SqlValue::Real(2.0)]),
            Some(Err(crate::types::PgError::InvalidInputSyntax {
                typ: "asind",
                ..
            }))
        ));
        assert!(matches!(
            call("acosd", &[SqlValue::Real(-1.5)]),
            Some(Err(crate::types::PgError::InvalidInputSyntax {
                typ: "acosd",
                ..
            }))
        ));
    }

    #[test]
    fn null_propagation() {
        assert!(matches!(
            call("sind", &[SqlValue::Null]),
            Some(Ok(SqlValue::Null))
        ));
        assert!(matches!(
            call("atan2d", &[SqlValue::Null, SqlValue::Int(1)]),
            Some(Ok(SqlValue::Null))
        ));
        assert!(matches!(
            call("atan2d", &[SqlValue::Int(1), SqlValue::Null]),
            Some(Ok(SqlValue::Null))
        ));
    }

    #[test]
    fn wrong_arity_is_error() {
        assert!(matches!(
            call("sind", &[SqlValue::Int(0), SqlValue::Int(1)]),
            Some(Err(crate::types::PgError::InvalidInputSyntax {
                typ: "expression",
                ..
            }))
        ));
        assert!(matches!(
            call("atan2d", &[SqlValue::Int(1)]),
            Some(Err(crate::types::PgError::InvalidInputSyntax {
                typ: "expression",
                ..
            }))
        ));
    }

    #[test]
    fn unclaimed_names_return_none() {

        assert!(call("sin", &[SqlValue::Int(0)]).is_none());
        assert!(call("atan2", &[SqlValue::Int(1), SqlValue::Int(1)]).is_none());
        assert!(call("sqrt", &[SqlValue::Int(4)]).is_none());
    }
}
