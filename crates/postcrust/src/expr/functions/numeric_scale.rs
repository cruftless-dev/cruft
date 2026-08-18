
use crate::types::PgError;
use sql_core::SqlValue;

fn no_such(name: &str) -> Option<Result<SqlValue, PgError>> {
    Some(Err(PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("function {name}(...) does not exist"),
    }))
}

enum Num {
    Nan,

    Inf(bool),

    Fin {
        neg: bool,
        int: String,
        frac: String,
    },
}

fn parse_num(v: &SqlValue) -> Option<Num> {
    match v {
        SqlValue::Int(n) => {
            let neg = *n < 0;

            let mag = (*n as i128).unsigned_abs();
            Some(Num::Fin {
                neg,
                int: mag.to_string(),
                frac: String::new(),
            })
        }
        SqlValue::Text(s) => parse_text(s),
        _ => None,
    }
}

fn parse_text(text: &str) -> Option<Num> {
    let s = text.trim();
    if s.is_empty() {
        return None;
    }
    match s.to_ascii_lowercase().as_str() {
        "nan" => return Some(Num::Nan),
        "inf" | "infinity" | "+inf" | "+infinity" => return Some(Num::Inf(false)),
        "-inf" | "-infinity" => return Some(Num::Inf(true)),
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
    Some(Num::Fin {
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

fn fmt_fin(neg: bool, int: &str, frac: &str) -> String {
    let int_trimmed = int.trim_start_matches('0');
    let int_out = if int_trimmed.is_empty() {
        "0"
    } else {
        int_trimmed
    };
    let is_zero = int_out == "0" && frac.bytes().all(|b| b == b'0');
    let mut out = String::new();
    if neg && !is_zero {
        out.push('-');
    }
    out.push_str(int_out);
    if !frac.is_empty() {
        out.push('.');
        out.push_str(frac);
    }
    out
}

fn scale_to(neg: bool, int: &str, frac: &str, s: i64, half_away: bool) -> String {
    let scale = frac.len() as i64;
    let k = scale - s;
    if k <= 0 {

        let mut f = frac.to_string();
        for _ in 0..(-k) {
            f.push('0');
        }
        return fmt_fin(neg, int, &f);
    }
    let full = format!("{int}{frac}");
    let len = full.len();
    let ku = k as usize;
    let (kept, removed): (&str, &str) = if ku >= len {
        ("", full.as_str())
    } else {
        full.split_at(len - ku)
    };
    let mut kept_s = if kept.is_empty() {
        "0".to_string()
    } else {
        kept.to_string()
    };
    let round_up = half_away && removed.bytes().next().is_some_and(|b| b >= b'5');
    if round_up {
        kept_s = inc_digits(&kept_s);
    }

    if s >= 0 {
        let su = s as usize;
        while kept_s.len() <= su {
            kept_s.insert(0, '0');
        }
        let (i, f) = kept_s.split_at(kept_s.len() - su);
        fmt_fin(neg, i, f)
    } else {
        for _ in 0..(-s) {
            kept_s.push('0');
        }
        fmt_fin(neg, &kept_s, "")
    }
}

fn scaled_i128(neg: bool, int: &str, frac: &str, target: usize) -> Option<i128> {
    let mut digits = format!("{int}{frac}");
    for _ in 0..(target - frac.len()) {
        digits.push('0');
    }
    let trimmed = digits.trim_start_matches('0');
    let mag: i128 = if trimmed.is_empty() {
        0
    } else {
        trimmed.parse().ok()?
    };
    Some(if neg { -mag } else { mag })
}

fn width_bucket(args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {

    let count = match &args[3] {
        SqlValue::Int(c) => *c,
        _ => return no_such("width_bucket"),
    };
    let op = parse_num(&args[0])?;
    let low = parse_num(&args[1])?;
    let high = parse_num(&args[2])?;

    let err = |msg: &str| {
        Some(Err(PgError::InvalidInputSyntax {
            typ: "expression",
            input: format!("width_bucket: {msg}"),
        }))
    };

    if count <= 0 {
        return err("count must be greater than zero");
    }
    if matches!(op, Num::Nan) || matches!(low, Num::Nan) || matches!(high, Num::Nan) {
        return err("operand, lower bound, and upper bound cannot be NaN");
    }
    if matches!(low, Num::Inf(_)) || matches!(high, Num::Inf(_)) {
        return err("lower and upper bounds must be finite");
    }

    let (ln, li, lf) = match &low {
        Num::Fin { neg, int, frac } => (*neg, int.as_str(), frac.as_str()),
        _ => unreachable!(),
    };
    let (hn, hi, hf) = match &high {
        Num::Fin { neg, int, frac } => (*neg, int.as_str(), frac.as_str()),
        _ => unreachable!(),
    };
    let target = lf.len().max(hf.len()).max(match &op {
        Num::Fin { frac, .. } => frac.len(),
        _ => 0,
    });
    let l = scaled_i128(ln, li, lf, target)?;
    let h = scaled_i128(hn, hi, hf, target)?;
    if l == h {
        return err("lower bound cannot equal upper bound");
    }

    let result: i64 = match &op {
        Num::Inf(true) => {

            if l < h {
                0
            } else {
                count + 1
            }
        }
        Num::Inf(false) => {
            if l < h {
                count + 1
            } else {
                0
            }
        }
        Num::Fin { neg, int, frac } => {
            let o = scaled_i128(*neg, int, frac, target)?;
            let c = count as i128;
            if l < h {
                if o < l {
                    0
                } else if o >= h {
                    count + 1
                } else {
                    (1 + (c * (o - l)) / (h - l)) as i64
                }
            } else {

                if o > l {
                    0
                } else if o <= h {
                    count + 1
                } else {
                    (1 + (c * (l - o)) / (l - h)) as i64
                }
            }
        }
        Num::Nan => unreachable!(),
    };
    Some(Ok(SqlValue::Int(result)))
}

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    let claimed = matches!(
        name,
        "round" | "trunc" | "scale" | "min_scale" | "trim_scale" | "width_bucket"
    );
    if !claimed {
        return None;
    }

    match name {
        "round" | "trunc" => {
            if args.len() != 2 {
                return None;
            }
        }
        "scale" | "min_scale" | "trim_scale" => {
            if args.len() != 1 {
                return no_such(name);
            }
        }
        "width_bucket" => {
            if args.len() != 4 {
                return no_such(name);
            }
        }
        _ => unreachable!(),
    }

    if args.iter().any(|a| matches!(a, SqlValue::Null)) {
        return Some(Ok(SqlValue::Null));
    }

    match name {
        "round" | "trunc" => {
            let s = match &args[1] {
                SqlValue::Int(n) => *n,
                _ => return no_such(name),
            };
            let num = parse_num(&args[0])?;
            let half_away = name == "round";
            let out = match num {
                Num::Nan => "NaN".to_string(),
                Num::Inf(neg) => if neg { "-Infinity" } else { "Infinity" }.to_string(),
                Num::Fin { neg, int, frac } => scale_to(neg, &int, &frac, s, half_away),
            };
            Some(Ok(SqlValue::Text(out)))
        }
        "scale" | "min_scale" => {
            let num = parse_num(&args[0])?;
            match num {

                Num::Nan | Num::Inf(_) => Some(Ok(SqlValue::Null)),
                Num::Fin { frac, .. } => {
                    let n = if name == "scale" {
                        frac.len()
                    } else {
                        frac.trim_end_matches('0').len()
                    };
                    Some(Ok(SqlValue::Int(n as i64)))
                }
            }
        }
        "trim_scale" => {
            let num = parse_num(&args[0])?;
            let out = match num {
                Num::Nan => "NaN".to_string(),
                Num::Inf(neg) => if neg { "-Infinity" } else { "Infinity" }.to_string(),
                Num::Fin { neg, int, frac } => {
                    let trimmed = frac.trim_end_matches('0');
                    fmt_fin(neg, &int, trimmed)
                }
            };
            Some(Ok(SqlValue::Text(out)))
        }
        "width_bucket" => width_bucket(args),
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> SqlValue {
        SqlValue::Text(s.to_string())
    }
    fn text_of(r: Option<Result<SqlValue, PgError>>) -> String {
        match r {
            Some(Ok(SqlValue::Text(s))) => s,
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn round_zero_scale_half_away() {
        assert_eq!(text_of(call("round", &[t("5.4"), SqlValue::Int(0)])), "5");
        assert_eq!(text_of(call("round", &[t("5.5"), SqlValue::Int(0)])), "6");
        assert_eq!(text_of(call("round", &[t("-5.5"), SqlValue::Int(0)])), "-6");
        assert_eq!(text_of(call("round", &[t("-0.5"), SqlValue::Int(0)])), "-1");
    }

    #[test]
    fn round_positive_scale() {
        assert_eq!(
            text_of(call("round", &[t("5.45"), SqlValue::Int(1)])),
            "5.5"
        );
        assert_eq!(
            text_of(call("round", &[t("0.005"), SqlValue::Int(2)])),
            "0.01"
        );
        assert_eq!(
            text_of(call("round", &[t("9.99"), SqlValue::Int(1)])),
            "10.0"
        );

        assert_eq!(text_of(call("round", &[t("0"), SqlValue::Int(1)])), "0.0");
    }

    #[test]
    fn round_negative_scale() {
        assert_eq!(
            text_of(call("round", &[t("1234.56"), SqlValue::Int(-2)])),
            "1200"
        );
        assert_eq!(text_of(call("round", &[t("2.5"), SqlValue::Int(0)])), "3");

        assert_eq!(
            text_of(call("round", &[t("950"), SqlValue::Int(-3)])),
            "1000"
        );
    }

    #[test]
    fn trunc_toward_zero() {
        assert_eq!(
            text_of(call("trunc", &[t("5.99"), SqlValue::Int(1)])),
            "5.9"
        );
        assert_eq!(
            text_of(call("trunc", &[t("-5.99"), SqlValue::Int(1)])),
            "-5.9"
        );
        assert_eq!(
            text_of(call("trunc", &[t("1234.56"), SqlValue::Int(-2)])),
            "1200"
        );
        assert_eq!(text_of(call("trunc", &[t("9.99"), SqlValue::Int(0)])), "9");
    }

    #[test]
    fn round_trunc_accept_int_arg() {
        assert_eq!(
            text_of(call("round", &[SqlValue::Int(5), SqlValue::Int(2)])),
            "5.00"
        );
        assert_eq!(
            text_of(call("trunc", &[SqlValue::Int(-7), SqlValue::Int(1)])),
            "-7.0"
        );
    }

    #[test]
    fn scale_family() {
        assert_eq!(call("scale", &[t("123.4500")]), Some(Ok(SqlValue::Int(4))));
        assert_eq!(call("scale", &[t("100")]), Some(Ok(SqlValue::Int(0))));
        assert_eq!(
            call("scale", &[SqlValue::Int(9)]),
            Some(Ok(SqlValue::Int(0)))
        );

        assert_eq!(
            call("min_scale", &[t("123.4500")]),
            Some(Ok(SqlValue::Int(2)))
        );
        assert_eq!(call("min_scale", &[t("0.00")]), Some(Ok(SqlValue::Int(0))));
        assert_eq!(call("min_scale", &[t("1.100")]), Some(Ok(SqlValue::Int(1))));

        assert_eq!(text_of(call("trim_scale", &[t("1.100")])), "1.1");
        assert_eq!(text_of(call("trim_scale", &[t("1.00")])), "1");
        assert_eq!(text_of(call("trim_scale", &[t("100")])), "100");
        assert_eq!(text_of(call("trim_scale", &[t("0.00")])), "0");
    }

    #[test]
    fn scale_of_special_is_null() {
        assert_eq!(call("scale", &[t("NaN")]), Some(Ok(SqlValue::Null)));
        assert_eq!(
            call("min_scale", &[t("Infinity")]),
            Some(Ok(SqlValue::Null))
        );
        assert_eq!(text_of(call("round", &[t("NaN"), SqlValue::Int(2)])), "NaN");
        assert_eq!(text_of(call("trim_scale", &[t("-Infinity")])), "-Infinity");
    }

    #[test]
    fn width_bucket_in_range() {
        let wb = |o: &str| call("width_bucket", &[t(o), t("0"), t("10"), SqlValue::Int(5)]);
        assert_eq!(wb("1"), Some(Ok(SqlValue::Int(1))));
        assert_eq!(wb("2"), Some(Ok(SqlValue::Int(2))));
        assert_eq!(wb("1.99999999999999"), Some(Ok(SqlValue::Int(1))));
        assert_eq!(wb("5"), Some(Ok(SqlValue::Int(3))));
    }

    #[test]
    fn width_bucket_below_and_above() {
        let wb = |o: &str| call("width_bucket", &[t(o), t("0"), t("10"), SqlValue::Int(5)]);
        assert_eq!(wb("-5.2"), Some(Ok(SqlValue::Int(0))));
        assert_eq!(wb("10"), Some(Ok(SqlValue::Int(6))));
        assert_eq!(wb("11"), Some(Ok(SqlValue::Int(6))));
    }

    #[test]
    fn width_bucket_reversed_bounds() {
        let wb = |o: &str| call("width_bucket", &[t(o), t("10"), t("0"), SqlValue::Int(5)]);
        assert_eq!(wb("-5.2"), Some(Ok(SqlValue::Int(6))));
        assert_eq!(wb("10"), Some(Ok(SqlValue::Int(1))));
        assert_eq!(wb("10.0000000000001"), Some(Ok(SqlValue::Int(0))));
    }

    #[test]
    fn width_bucket_errors() {

        assert!(matches!(
            call("width_bucket", &[t("5"), t("3"), t("4"), SqlValue::Int(0)]),
            Some(Err(PgError::InvalidInputSyntax { .. }))
        ));

        assert!(matches!(
            call(
                "width_bucket",
                &[t("3.5"), t("3.0"), t("3.0"), SqlValue::Int(888)]
            ),
            Some(Err(PgError::InvalidInputSyntax { .. }))
        ));

        assert!(matches!(
            call(
                "width_bucket",
                &[t("NaN"), t("3"), t("4"), SqlValue::Int(8)]
            ),
            Some(Err(PgError::InvalidInputSyntax { .. }))
        ));

        assert!(matches!(
            call(
                "width_bucket",
                &[t("2"), t("3"), t("-Infinity"), SqlValue::Int(8)]
            ),
            Some(Err(PgError::InvalidInputSyntax { .. }))
        ));

        assert_eq!(
            call(
                "width_bucket",
                &[t("Infinity"), t("1"), t("10"), SqlValue::Int(10)]
            ),
            Some(Ok(SqlValue::Int(11)))
        );
        assert_eq!(
            call(
                "width_bucket",
                &[t("-Infinity"), t("1"), t("10"), SqlValue::Int(10)]
            ),
            Some(Ok(SqlValue::Int(0)))
        );
    }

    #[test]
    fn null_propagates() {
        assert_eq!(
            call("round", &[SqlValue::Null, SqlValue::Int(2)]),
            Some(Ok(SqlValue::Null))
        );
        assert_eq!(
            call("round", &[t("5.5"), SqlValue::Null]),
            Some(Ok(SqlValue::Null))
        );
        assert_eq!(call("scale", &[SqlValue::Null]), Some(Ok(SqlValue::Null)));
        assert_eq!(
            call("trim_scale", &[SqlValue::Null]),
            Some(Ok(SqlValue::Null))
        );
        assert_eq!(
            call(
                "width_bucket",
                &[SqlValue::Null, t("0"), t("10"), SqlValue::Int(5)]
            ),
            Some(Ok(SqlValue::Null))
        );
    }

    #[test]
    fn one_arg_round_trunc_is_none() {

        assert_eq!(call("round", &[t("5.5")]), None);
        assert_eq!(call("trunc", &[t("5.5")]), None);
        assert_eq!(
            call("round", &[t("5.5"), SqlValue::Int(1), SqlValue::Int(2)]),
            None
        );
    }

    #[test]
    fn unclaimed_name_is_none() {
        assert_eq!(call("nope", &[t("1")]), None);
    }

    #[test]
    fn wrong_arity_and_type_are_claimed_errors() {
        assert!(matches!(
            call("scale", &[t("1"), t("2")]),
            Some(Err(PgError::InvalidInputSyntax { .. }))
        ));
        assert!(matches!(
            call("width_bucket", &[t("1"), t("2")]),
            Some(Err(PgError::InvalidInputSyntax { .. }))
        ));

        assert!(call("scale", &[SqlValue::Blob(vec![1])]).is_none());
    }
}
