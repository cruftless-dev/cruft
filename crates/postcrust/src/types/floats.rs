
use super::PgError;
use sql_core::SqlValue;

fn parse_special(s: &str) -> Option<f64> {
    let lower = s.to_ascii_lowercase();
    match lower.as_str() {
        "nan" => Some(f64::NAN),
        "inf" | "infinity" | "+inf" | "+infinity" => Some(f64::INFINITY),
        "-inf" | "-infinity" => Some(f64::NEG_INFINITY),
        _ => None,
    }
}

fn has_nonzero_mantissa(s: &str) -> bool {
    let mantissa = match s.split_once(['e', 'E']) {
        Some((m, _)) => m,
        None => s,
    };
    mantissa.chars().any(|c| c.is_ascii_digit() && c != '0')
}

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    let typ = super::type_name(oid);
    let is_f4 = oid == super::oid::FLOAT4;
    let s = text.trim();

    if let Some(v) = parse_special(s) {
        return Ok(SqlValue::Real(v));
    }

    let p: f64 = match s.parse::<f64>() {
        Ok(v) => v,
        Err(_) => {
            return Err(PgError::InvalidInputSyntax {
                typ,
                input: text.to_string(),
            });
        }
    };

    if p.is_infinite() {
        return Err(PgError::OutOfRange {
            typ,
            input: text.to_string(),
        });
    }
    if p.is_nan() {
        return Err(PgError::InvalidInputSyntax {
            typ,
            input: text.to_string(),
        });
    }

    if p == 0.0 && has_nonzero_mantissa(s) {
        return Err(PgError::OutOfRange {
            typ,
            input: text.to_string(),
        });
    }

    if is_f4 {
        let f = p as f32;

        if f.is_infinite() {
            return Err(PgError::OutOfRange {
                typ,
                input: text.to_string(),
            });
        }
        if f == 0.0 && p != 0.0 {
            return Err(PgError::OutOfRange {
                typ,
                input: text.to_string(),
            });
        }
        return Ok(SqlValue::Real(f as f64));
    }

    Ok(SqlValue::Real(p))
}

pub fn output(oid: u32, v: &SqlValue) -> String {
    let r = match v {
        SqlValue::Real(r) => *r,
        _ => return String::new(),
    };
    if r.is_nan() {
        return "NaN".to_string();
    }
    if r.is_infinite() {
        return if r < 0.0 { "-Infinity" } else { "Infinity" }.to_string();
    }
    if oid == super::oid::FLOAT4 {

        render(&format!("{:e}", r as f32), 6)
    } else {
        render(&format!("{:e}", r), 15)
    }
}

fn render(sci: &str, cutoff: i32) -> String {
    let (neg, body) = match sci.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, sci),
    };

    let (mant, exp_str) = body.split_once('e').expect("{:e} always has an exponent");
    let exp: i32 = exp_str.parse().expect("{:e} exponent is an integer");
    let digits: String = mant.chars().filter(|c| *c != '.').collect();

    let out = if exp < -4 || exp >= cutoff {

        let mut m = String::new();
        m.push_str(&digits[0..1]);
        if digits.len() > 1 {
            m.push('.');
            m.push_str(&digits[1..]);
        }
        let sign = if exp < 0 { '-' } else { '+' };
        format!("{}e{}{:02}", m, sign, exp.abs())
    } else if exp >= 0 {

        let l = digits.len() as i32;
        if exp + 1 >= l {
            let mut d = digits.clone();
            for _ in 0..(exp + 1 - l) {
                d.push('0');
            }
            d
        } else {
            let split = (exp + 1) as usize;
            format!("{}.{}", &digits[..split], &digits[split..])
        }
    } else {

        let zeros = (-exp - 1) as usize;
        format!("0.{}{}", "0".repeat(zeros), digits)
    };

    if neg {
        format!("-{}", out)
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::oid::{FLOAT4, FLOAT8};

    fn real(v: &SqlValue) -> f64 {
        match v {
            SqlValue::Real(r) => *r,
            _ => panic!("expected Real, got {:?}", v),
        }
    }

    #[test]
    fn input_plain_and_whitespace() {
        assert_eq!(real(&input(FLOAT8, "    0.0").unwrap()), 0.0);
        assert_eq!(real(&input(FLOAT8, "1004.30   ").unwrap()), 1004.3);
        assert_eq!(real(&input(FLOAT8, "     -34.84    ").unwrap()), -34.84);
        assert_eq!(real(&input(FLOAT8, "34.5").unwrap()), 34.5);
    }

    #[test]
    fn input_scientific() {
        assert_eq!(real(&input(FLOAT8, "1e10").unwrap()), 1e10);
        assert_eq!(real(&input(FLOAT8, "1.2e-3").unwrap()), 1.2e-3);
        assert_eq!(
            real(&input(FLOAT8, "1.2345678901234e+200").unwrap()),
            1.2345678901234e200
        );

        assert_eq!(
            real(&input(FLOAT4, "1.2345678901234e+20").unwrap()),
            1.2345678901234e20_f32 as f64
        );
    }

    #[test]
    fn input_specials() {
        assert!(real(&input(FLOAT8, "NaN").unwrap()).is_nan());
        assert!(real(&input(FLOAT4, "nan").unwrap()).is_nan());
        assert!(real(&input(FLOAT4, "   NAN  ").unwrap()).is_nan());
        assert_eq!(real(&input(FLOAT4, "infinity").unwrap()), f64::INFINITY);
        assert_eq!(real(&input(FLOAT8, "Infinity").unwrap()), f64::INFINITY);
        assert_eq!(real(&input(FLOAT8, "inf").unwrap()), f64::INFINITY);
        assert_eq!(
            real(&input(FLOAT4, "          -INFINiTY   ").unwrap()),
            f64::NEG_INFINITY
        );
        assert_eq!(real(&input(FLOAT8, "-inf").unwrap()), f64::NEG_INFINITY);
    }

    #[test]
    fn input_invalid_syntax() {
        for bad in [
            "",
            "       ",
            "xyz",
            "5.0.0",
            "5 . 0",
            "5.   0",
            "     - 3.0",
            "123            5",
        ] {
            match input(FLOAT4, bad) {
                Err(PgError::InvalidInputSyntax { typ, input }) => {
                    assert_eq!(typ, "real");

                    assert_eq!(input, bad);
                }
                other => panic!("expected InvalidInputSyntax for {bad:?}, got {other:?}"),
            }
        }

        for bad in ["N A N", "NaN x", " INFINITY    x"] {
            assert!(matches!(
                input(FLOAT8, bad),
                Err(PgError::InvalidInputSyntax { .. })
            ));
        }
    }

    #[test]
    fn input_float4_out_of_range() {
        for bad in [
            "10e70", "-10e70", "10e-70", "-10e-70", "10e400", "-10e400", "10e-400", "-10e-400",
        ] {
            match input(FLOAT4, bad) {
                Err(PgError::OutOfRange { typ, input }) => {
                    assert_eq!(typ, "real");
                    assert_eq!(input, bad);
                }
                other => panic!("expected OutOfRange for float4 {bad:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn input_float8_out_of_range() {

        assert!(matches!(
            input(FLOAT8, "10e400"),
            Err(PgError::OutOfRange { .. })
        ));
        assert!(matches!(
            input(FLOAT8, "-10e400"),
            Err(PgError::OutOfRange { .. })
        ));
        assert!(matches!(
            input(FLOAT8, "10e-400"),
            Err(PgError::OutOfRange { .. })
        ));

        assert_eq!(real(&input(FLOAT8, "10e70").unwrap()), 1e71);
    }

    #[test]
    fn input_genuine_zero_not_underflow() {

        assert_eq!(real(&input(FLOAT8, "0e-400").unwrap()), 0.0);
        assert_eq!(real(&input(FLOAT4, "0.0e5").unwrap()), 0.0);
    }

    #[test]
    fn output_specials() {
        assert_eq!(output(FLOAT8, &SqlValue::Real(f64::NAN)), "NaN");
        assert_eq!(output(FLOAT4, &SqlValue::Real(f64::INFINITY)), "Infinity");
        assert_eq!(
            output(FLOAT8, &SqlValue::Real(f64::NEG_INFINITY)),
            "-Infinity"
        );
    }

    #[test]
    fn output_float4_golden() {

        assert_eq!(output(FLOAT4, &SqlValue::Real(0.0)), "0");
        assert_eq!(output(FLOAT4, &input(FLOAT4, "1004.30").unwrap()), "1004.3");
        assert_eq!(output(FLOAT4, &input(FLOAT4, "-34.84").unwrap()), "-34.84");
        assert_eq!(
            output(FLOAT4, &input(FLOAT4, "1.2345678901234e+20").unwrap()),
            "1.2345679e+20"
        );
        assert_eq!(
            output(FLOAT4, &input(FLOAT4, "1.2345678901234e-20").unwrap()),
            "1.2345679e-20"
        );

        assert_eq!(output(FLOAT4, &input(FLOAT4, "100000").unwrap()), "100000");
        assert_eq!(
            output(FLOAT4, &input(FLOAT4, "999999.94").unwrap()),
            "999999.94"
        );
        assert_eq!(output(FLOAT4, &input(FLOAT4, "1e6").unwrap()), "1e+06");
        assert_eq!(
            output(FLOAT4, &input(FLOAT4, "100000.01").unwrap()),
            "100000.01"
        );
        assert_eq!(
            output(FLOAT4, &input(FLOAT4, "123456.7").unwrap()),
            "123456.7"
        );

        assert_eq!(output(FLOAT4, &input(FLOAT4, "0.0001").unwrap()), "0.0001");
        assert_eq!(output(FLOAT4, &input(FLOAT4, "1e-05").unwrap()), "1e-05");
    }

    #[test]
    fn output_float8_golden() {
        assert_eq!(output(FLOAT8, &SqlValue::Real(0.0)), "0");
        assert_eq!(output(FLOAT8, &input(FLOAT8, "1004.3").unwrap()), "1004.3");
        assert_eq!(
            output(FLOAT8, &input(FLOAT8, "1.2345678901234e+200").unwrap()),
            "1.2345678901234e+200"
        );
        assert_eq!(
            output(FLOAT8, &input(FLOAT8, "1.2345678901234e-200").unwrap()),
            "1.2345678901234e-200"
        );

        assert_eq!(
            output(FLOAT8, &input(FLOAT8, "999999999999999.9").unwrap()),
            "999999999999999.9"
        );
        assert_eq!(output(FLOAT8, &input(FLOAT8, "1e15").unwrap()), "1e+15");
        assert_eq!(output(FLOAT8, &input(FLOAT8, "1e16").unwrap()), "1e+16");

        assert_eq!(output(FLOAT8, &input(FLOAT8, "0.0001").unwrap()), "0.0001");
        assert_eq!(output(FLOAT8, &input(FLOAT8, "1e-05").unwrap()), "1e-05");
        assert_eq!(output(FLOAT8, &input(FLOAT8, "1e-07").unwrap()), "1e-07");
        assert_eq!(output(FLOAT8, &input(FLOAT8, "1e-10").unwrap()), "1e-10");

        assert_eq!(
            output(FLOAT8, &SqlValue::Real(1008618.4899999999)),
            "1008618.4899999999"
        );
    }

    #[test]
    fn output_round_trip_specials_and_neg() {
        assert_eq!(
            output(FLOAT8, &input(FLOAT8, "-1.2345678901234e+200").unwrap()),
            "-1.2345678901234e+200"
        );
        assert_eq!(
            output(FLOAT4, &input(FLOAT4, "-1004.3").unwrap()),
            "-1004.3"
        );
    }
}
