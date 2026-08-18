
use crate::types::PgError;
use sql_core::SqlValue;

fn no_such(name: &str) -> Option<Result<SqlValue, PgError>> {
    Some(Err(PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("function {name}(...) does not exist"),
    }))
}

enum Dec {
    Nan,

    Inf(bool),

    Fin {
        neg: bool,
        int: String,
        frac: String,
    },
}

fn parse_numeric_text(text: &str) -> Option<Dec> {
    let s = text.trim();
    if s.is_empty() {
        return None;
    }
    match s.to_ascii_lowercase().as_str() {
        "nan" => return Some(Dec::Nan),
        "inf" | "infinity" | "+inf" | "+infinity" => return Some(Dec::Inf(false)),
        "-inf" | "-infinity" => return Some(Dec::Inf(true)),
        _ => {}
    }
    let (neg, body) = match s.as_bytes()[0] {
        b'+' => (false, &s[1..]),
        b'-' => (true, &s[1..]),
        _ => (false, s),
    };
    let (int_part, frac_part) = match body.split_once('.') {
        Some((i, f)) => {
            if f.contains('.') {
                return None;
            }
            (i, f)
        }
        None => (body, ""),
    };
    if int_part.is_empty() && frac_part.is_empty() {
        return None;
    }
    if !int_part.bytes().all(|b| b.is_ascii_digit())
        || !frac_part.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let int = if int_part.is_empty() {
        "0".to_string()
    } else {
        int_part.to_string()
    };
    Some(Dec::Fin {
        neg,
        int,
        frac: frac_part.to_string(),
    })
}

fn inc_digits(d: &str) -> String {
    let mut bytes: Vec<u8> = d.bytes().collect();
    let mut i = bytes.len();
    loop {
        if i == 0 {
            let mut out = vec![b'1'];
            out.extend_from_slice(&bytes);
            return String::from_utf8(out).unwrap();
        }
        i -= 1;
        if bytes[i] == b'9' {
            bytes[i] = b'0';
        } else {
            bytes[i] += 1;
            return String::from_utf8(bytes).unwrap();
        }
    }
}

fn fmt_int(neg: bool, mag: &str) -> String {
    let trimmed = mag.trim_start_matches('0');
    let m = if trimmed.is_empty() { "0" } else { trimmed };
    if m == "0" {
        "0".to_string()
    } else if neg {
        format!("-{m}")
    } else {
        m.to_string()
    }
}

fn special_passthrough(d: &Dec) -> Option<String> {
    match d {
        Dec::Nan => Some("NaN".to_string()),
        Dec::Inf(neg) => Some(if *neg { "-Infinity" } else { "Infinity" }.to_string()),
        Dec::Fin { .. } => None,
    }
}

fn ceil_numeric(d: &Dec) -> String {
    if let Some(s) = special_passthrough(d) {
        return s;
    }
    let Dec::Fin { neg, int, frac } = d else {
        unreachable!()
    };
    let has_frac = frac.bytes().any(|b| b != b'0');

    let mag = if has_frac && !neg {
        inc_digits(int)
    } else {
        int.clone()
    };
    fmt_int(*neg, &mag)
}

fn floor_numeric(d: &Dec) -> String {
    if let Some(s) = special_passthrough(d) {
        return s;
    }
    let Dec::Fin { neg, int, frac } = d else {
        unreachable!()
    };
    let has_frac = frac.bytes().any(|b| b != b'0');

    let mag = if has_frac && *neg {
        inc_digits(int)
    } else {
        int.clone()
    };
    fmt_int(*neg, &mag)
}

fn trunc_numeric(d: &Dec) -> String {
    if let Some(s) = special_passthrough(d) {
        return s;
    }
    let Dec::Fin { neg, int, .. } = d else {
        unreachable!()
    };
    fmt_int(*neg, int)
}

fn round_numeric(d: &Dec) -> String {
    if let Some(s) = special_passthrough(d) {
        return s;
    }
    let Dec::Fin { neg, int, frac } = d else {
        unreachable!()
    };
    let round_up = frac.bytes().next().is_some_and(|b| b >= b'5');
    let mag = if round_up {
        inc_digits(int)
    } else {
        int.clone()
    };
    fmt_int(*neg, &mag)
}

fn sign_numeric(d: &Dec) -> String {
    match d {
        Dec::Nan => "NaN".to_string(),
        Dec::Inf(neg) => if *neg { "-1" } else { "1" }.to_string(),
        Dec::Fin { neg, int, frac } => {
            let is_zero = int.bytes().all(|b| b == b'0') && frac.bytes().all(|b| b == b'0');
            if is_zero {
                "0".to_string()
            } else if *neg {
                "-1".to_string()
            } else {
                "1".to_string()
            }
        }
    }
}

fn abs_numeric(d: &Dec) -> String {
    match d {
        Dec::Nan => "NaN".to_string(),
        Dec::Inf(_) => "Infinity".to_string(),
        Dec::Fin { int, frac, .. } => {
            let mut out = int.clone();
            if !frac.is_empty() {
                out.push('.');
                out.push_str(frac);
            }
            out
        }
    }
}

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {

    let claimed = matches!(
        name,
        "abs" | "ceil" | "ceiling" | "floor" | "round" | "trunc" | "sign"
    );
    if !claimed {
        return None;
    }

    if args.len() != 1 {
        if (name == "round" || name == "trunc") && args.len() == 2 {
            return None;
        }
        return no_such(name);
    }

    let arg = &args[0];

    if matches!(arg, SqlValue::Null) {
        return Some(Ok(SqlValue::Null));
    }

    match name {
        "abs" => match arg {
            SqlValue::Int(n) => Some(
                n.checked_abs()
                    .map(SqlValue::Int)
                    .ok_or(PgError::Overflow { typ: "bigint" }),
            ),
            SqlValue::Real(f) => Some(Ok(SqlValue::Real(f.abs()))),

            SqlValue::Text(s) => match parse_numeric_text(s) {
                Some(d) => Some(Ok(SqlValue::Text(abs_numeric(&d)))),
                None => no_such(name),
            },
            _ => no_such(name),
        },

        "ceil" | "ceiling" => match arg {
            SqlValue::Int(n) => Some(Ok(SqlValue::Int(*n))),
            SqlValue::Real(f) => Some(Ok(SqlValue::Real(f.ceil()))),

            SqlValue::Text(s) => match parse_numeric_text(s) {
                Some(d) => Some(Ok(SqlValue::Text(ceil_numeric(&d)))),
                None => no_such(name),
            },
            _ => no_such(name),
        },
        "floor" => match arg {
            SqlValue::Int(n) => Some(Ok(SqlValue::Int(*n))),
            SqlValue::Real(f) => Some(Ok(SqlValue::Real(f.floor()))),

            SqlValue::Text(s) => match parse_numeric_text(s) {
                Some(d) => Some(Ok(SqlValue::Text(floor_numeric(&d)))),
                None => no_such(name),
            },
            _ => no_such(name),
        },
        "round" => match arg {
            SqlValue::Int(n) => Some(Ok(SqlValue::Int(*n))),

            SqlValue::Real(f) => Some(Ok(SqlValue::Real(f.round()))),

            SqlValue::Text(s) => match parse_numeric_text(s) {
                Some(d) => Some(Ok(SqlValue::Text(round_numeric(&d)))),
                None => no_such(name),
            },
            _ => no_such(name),
        },
        "trunc" => match arg {
            SqlValue::Int(n) => Some(Ok(SqlValue::Int(*n))),
            SqlValue::Real(f) => Some(Ok(SqlValue::Real(f.trunc()))),

            SqlValue::Text(s) => match parse_numeric_text(s) {
                Some(d) => Some(Ok(SqlValue::Text(trunc_numeric(&d)))),
                None => no_such(name),
            },
            _ => no_such(name),
        },
        "sign" => match arg {
            SqlValue::Int(n) => Some(Ok(SqlValue::Int(n.signum()))),

            SqlValue::Real(f) => Some(Ok(SqlValue::Real(if f.is_nan() {
                f64::NAN
            } else if *f > 0.0 {
                1.0
            } else if *f < 0.0 {
                -1.0
            } else {

                0.0
            }))),

            SqlValue::Text(s) => match parse_numeric_text(s) {
                Some(d) => Some(Ok(SqlValue::Text(sign_numeric(&d)))),
                None => no_such(name),
            },
            _ => no_such(name),
        },

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abs_int() {
        assert_eq!(
            call("abs", &[SqlValue::Int(-5)]),
            Some(Ok(SqlValue::Int(5)))
        );
        assert_eq!(call("abs", &[SqlValue::Int(5)]), Some(Ok(SqlValue::Int(5))));
    }

    #[test]
    fn abs_int_overflow() {
        assert_eq!(
            call("abs", &[SqlValue::Int(i64::MIN)]),
            Some(Err(PgError::Overflow { typ: "bigint" }))
        );
    }

    #[test]
    fn abs_real() {
        assert_eq!(
            call("abs", &[SqlValue::Real(-2.5)]),
            Some(Ok(SqlValue::Real(2.5)))
        );
    }

    #[test]
    fn ceil_floor() {
        assert_eq!(
            call("ceil", &[SqlValue::Real(2.1)]),
            Some(Ok(SqlValue::Real(3.0)))
        );
        assert_eq!(
            call("ceiling", &[SqlValue::Real(2.1)]),
            Some(Ok(SqlValue::Real(3.0)))
        );
        assert_eq!(
            call("floor", &[SqlValue::Real(2.9)]),
            Some(Ok(SqlValue::Real(2.0)))
        );

        assert_eq!(
            call("ceil", &[SqlValue::Int(7)]),
            Some(Ok(SqlValue::Int(7)))
        );
        assert_eq!(
            call("floor", &[SqlValue::Int(7)]),
            Some(Ok(SqlValue::Int(7)))
        );
    }

    #[test]
    fn round_half_away_from_zero() {
        assert_eq!(
            call("round", &[SqlValue::Real(2.5)]),
            Some(Ok(SqlValue::Real(3.0)))
        );
        assert_eq!(
            call("round", &[SqlValue::Real(-2.5)]),
            Some(Ok(SqlValue::Real(-3.0)))
        );
        assert_eq!(
            call("round", &[SqlValue::Real(0.5)]),
            Some(Ok(SqlValue::Real(1.0)))
        );
        assert_eq!(
            call("round", &[SqlValue::Int(4)]),
            Some(Ok(SqlValue::Int(4)))
        );
    }

    #[test]
    fn trunc_toward_zero() {
        assert_eq!(
            call("trunc", &[SqlValue::Real(2.9)]),
            Some(Ok(SqlValue::Real(2.0)))
        );
        assert_eq!(
            call("trunc", &[SqlValue::Real(-2.9)]),
            Some(Ok(SqlValue::Real(-2.0)))
        );
        assert_eq!(
            call("trunc", &[SqlValue::Int(9)]),
            Some(Ok(SqlValue::Int(9)))
        );
    }

    #[test]
    fn sign_int_and_real() {
        assert_eq!(
            call("sign", &[SqlValue::Int(-9)]),
            Some(Ok(SqlValue::Int(-1)))
        );
        assert_eq!(
            call("sign", &[SqlValue::Int(0)]),
            Some(Ok(SqlValue::Int(0)))
        );
        assert_eq!(
            call("sign", &[SqlValue::Int(9)]),
            Some(Ok(SqlValue::Int(1)))
        );
        assert_eq!(
            call("sign", &[SqlValue::Real(-3.2)]),
            Some(Ok(SqlValue::Real(-1.0)))
        );
        assert_eq!(
            call("sign", &[SqlValue::Real(0.0)]),
            Some(Ok(SqlValue::Real(0.0)))
        );
        assert_eq!(
            call("sign", &[SqlValue::Real(3.2)]),
            Some(Ok(SqlValue::Real(1.0)))
        );
    }

    fn t(s: &str) -> SqlValue {
        SqlValue::Text(s.to_string())
    }

    #[test]
    fn ceil_numeric_arg() {
        assert_eq!(call("ceil", &[t("4.2")]), Some(Ok(t("5"))));
        assert_eq!(call("ceiling", &[t("4.2")]), Some(Ok(t("5"))));

        assert_eq!(call("ceil", &[t("-0.1")]), Some(Ok(t("0"))));

        assert_eq!(call("ceil", &[t("100")]), Some(Ok(t("100"))));
    }

    #[test]
    fn floor_numeric_arg() {
        assert_eq!(call("floor", &[t("4.8")]), Some(Ok(t("4"))));

        assert_eq!(call("floor", &[t("-0.1")]), Some(Ok(t("-1"))));
    }

    #[test]
    fn trunc_numeric_arg() {
        assert_eq!(call("trunc", &[t("9.9")]), Some(Ok(t("9"))));
        assert_eq!(call("trunc", &[t("-9.9")]), Some(Ok(t("-9"))));
    }

    #[test]
    fn round_numeric_arg_half_away() {
        assert_eq!(call("round", &[t("2.5")]), Some(Ok(t("3"))));
        assert_eq!(call("round", &[t("-2.5")]), Some(Ok(t("-3"))));
        assert_eq!(call("round", &[t("4.2")]), Some(Ok(t("4"))));
    }

    #[test]
    fn sign_numeric_arg() {
        assert_eq!(call("sign", &[t("-3.5")]), Some(Ok(t("-1"))));
        assert_eq!(call("sign", &[t("0.00")]), Some(Ok(t("0"))));
        assert_eq!(call("sign", &[t("3.5")]), Some(Ok(t("1"))));
    }

    #[test]
    fn abs_numeric_arg_preserves_scale() {
        assert_eq!(call("abs", &[t("-12.50")]), Some(Ok(t("12.50"))));
        assert_eq!(call("abs", &[t("12.50")]), Some(Ok(t("12.50"))));
    }

    #[test]
    fn numeric_specials_pass_through() {
        assert_eq!(call("ceil", &[t("NaN")]), Some(Ok(t("NaN"))));
        assert_eq!(call("floor", &[t("-Infinity")]), Some(Ok(t("-Infinity"))));
        assert_eq!(call("round", &[t("Infinity")]), Some(Ok(t("Infinity"))));
        assert_eq!(call("sign", &[t("NaN")]), Some(Ok(t("NaN"))));
        assert_eq!(call("abs", &[t("-Infinity")]), Some(Ok(t("Infinity"))));
    }

    #[test]
    fn non_numeric_text_is_claimed_error() {
        assert!(matches!(
            call("ceil", &[t("not a number")]),
            Some(Err(PgError::InvalidInputSyntax { .. }))
        ));
    }

    #[test]
    fn two_arg_numeric_round_trunc_still_fall_through() {
        assert_eq!(call("round", &[t("2.567"), SqlValue::Int(2)]), None);
        assert_eq!(call("trunc", &[t("2.567"), SqlValue::Int(2)]), None);
    }

    #[test]
    fn null_propagates() {
        assert_eq!(call("abs", &[SqlValue::Null]), Some(Ok(SqlValue::Null)));
        assert_eq!(call("round", &[SqlValue::Null]), Some(Ok(SqlValue::Null)));
        assert_eq!(call("sign", &[SqlValue::Null]), Some(Ok(SqlValue::Null)));
    }

    #[test]
    fn two_arg_round_falls_through() {
        assert_eq!(
            call("round", &[SqlValue::Real(2.567), SqlValue::Int(2)]),
            None
        );
    }

    #[test]
    fn wrong_arity_is_claimed_error() {
        assert!(matches!(
            call("abs", &[]),
            Some(Err(PgError::InvalidInputSyntax { .. }))
        ));
    }

    #[test]
    fn unclaimed_name_is_none() {
        assert_eq!(call("nope", &[SqlValue::Int(1)]), None);
    }
}
