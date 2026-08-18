
use crate::value::{JsString, Value};
use std::rc::Rc;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    OnceLock,
};

static STRING_CONCAT_CALLS: AtomicU64 = AtomicU64::new(0);
static STRING_CONCAT_LHS_BYTES: AtomicU64 = AtomicU64::new(0);
static STRING_CONCAT_RHS_BYTES: AtomicU64 = AtomicU64::new(0);
static STRING_CONCAT_OUTPUT_BYTES: AtomicU64 = AtomicU64::new(0);
static STRING_CONCAT_MAX_LHS_BYTES: AtomicU64 = AtomicU64::new(0);
const LAZY_STRING_CONCAT_MIN_BYTES: usize = 4096;

fn lazy_string_concat_min_bytes() -> usize {
    static MIN: OnceLock<usize> = OnceLock::new();
    *MIN.get_or_init(|| {
        std::env::var("CRUFT_STRING_LAZY_CONCAT_MIN")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(LAZY_STRING_CONCAT_MIN_BYTES)
    })
}

fn string_concat_counters_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_STRING_CONCAT_COUNTERS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn string_concat_counter_report_every() -> u64 {
    static EVERY: OnceLock<u64> = OnceLock::new();
    *EVERY.get_or_init(|| {
        std::env::var("CRUFT_STRING_CONCAT_COUNTERS_EVERY")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(100_000)
    })
}

fn record_string_concat(lhs_bytes: usize, rhs_bytes: usize) {
    if !string_concat_counters_enabled() {
        return;
    }
    let lhs = lhs_bytes as u64;
    let rhs = rhs_bytes as u64;
    let out = lhs + rhs;
    let calls = STRING_CONCAT_CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    STRING_CONCAT_LHS_BYTES.fetch_add(lhs, Ordering::Relaxed);
    STRING_CONCAT_RHS_BYTES.fetch_add(rhs, Ordering::Relaxed);
    STRING_CONCAT_OUTPUT_BYTES.fetch_add(out, Ordering::Relaxed);
    let mut cur = STRING_CONCAT_MAX_LHS_BYTES.load(Ordering::Relaxed);
    while lhs > cur {
        match STRING_CONCAT_MAX_LHS_BYTES.compare_exchange_weak(
            cur,
            lhs,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(next) => cur = next,
        }
    }
    let every = string_concat_counter_report_every();
    if calls % every == 0 {
        eprintln!(
            "[string-concat-counters] calls={} lhs_bytes={} rhs_bytes={} output_bytes={} max_lhs_bytes={}",
            calls,
            STRING_CONCAT_LHS_BYTES.load(Ordering::Relaxed),
            STRING_CONCAT_RHS_BYTES.load(Ordering::Relaxed),
            STRING_CONCAT_OUTPUT_BYTES.load(Ordering::Relaxed),
            STRING_CONCAT_MAX_LHS_BYTES.load(Ordering::Relaxed)
        );
    }
}

pub fn to_boolean(v: &Value) -> bool {
    match v {
        Value::Undefined | Value::Null => false,
        Value::Boolean(b) => *b,
        Value::Number(n) => !(n.is_nan() || *n == 0.0),
        Value::String(s) => !s.is_empty(),
        Value::BigInt(b) => !b.is_zero(),
        Value::Symbol(_) => true,
        Value::Object(_) => true,
    }
}

pub fn to_number(v: &Value) -> f64 {
    match v {
        Value::Undefined => f64::NAN,
        Value::Null => 0.0,
        Value::Boolean(true) => 1.0,
        Value::Boolean(false) => 0.0,
        Value::Number(n) => *n,
        Value::String(s) => parse_string_to_number(s.as_str()),
        Value::BigInt(b) => b.to_f64(),
        Value::Symbol(_) => f64::NAN,
        Value::Object(_) => f64::NAN,
    }
}

fn parse_string_to_number(s: &str) -> f64 {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return 0.0;
    }
    if let Some(rest) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        return u64::from_str_radix(rest, 16)
            .map(|n| n as f64)
            .unwrap_or(f64::NAN);
    }
    if let Some(rest) = trimmed
        .strip_prefix("0b")
        .or_else(|| trimmed.strip_prefix("0B"))
    {
        return u64::from_str_radix(rest, 2)
            .map(|n| n as f64)
            .unwrap_or(f64::NAN);
    }
    if let Some(rest) = trimmed
        .strip_prefix("0o")
        .or_else(|| trimmed.strip_prefix("0O"))
    {
        return u64::from_str_radix(rest, 8)
            .map(|n| n as f64)
            .unwrap_or(f64::NAN);
    }

    let body = trimmed.strip_prefix(['+', '-']).unwrap_or(trimmed);
    if body != "Infinity"
        && body
            .bytes()
            .any(|b| b.is_ascii_alphabetic() && b != b'e' && b != b'E')
    {
        return f64::NAN;
    }
    trimmed.parse::<f64>().unwrap_or(f64::NAN)
}

pub fn to_string(v: &Value) -> Rc<String> {
    Rc::new(match v {
        Value::Undefined => "undefined".to_string(),
        Value::Null => "null".to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::Number(n) => number_to_string(*n),
        Value::String(s) => return std::rc::Rc::new(s.as_str().to_string()),
        Value::BigInt(b) => b.to_decimal(),

        Value::Symbol(s) => {
            if let Some(rest) = s.strip_prefix("@@sym:") {

                let desc = if let Some((_, d)) = rest.split_once(':') {
                    d.to_string()
                } else if rest.chars().all(|c| c.is_ascii_digit()) {
                    String::new()
                } else {
                    rest.to_string()
                };
                return Rc::new(format!("Symbol({desc})"));
            }
            if let Some(name) = s.strip_prefix("@@") {
                return Rc::new(format!("Symbol(Symbol.{name})"));
            }
            return Rc::new(format!("Symbol({})", s.as_str()));
        }
        Value::Object(_) => "[object Object]".to_string(),
    })
}

pub fn to_js_string(v: &Value) -> JsString {
    match v {
        Value::String(s) => (**s).clone(),
        _ => JsString::from((*to_string(v)).clone()),
    }
}

pub fn number_to_string(n: f64) -> String {
    if n.is_nan() {
        return "NaN".to_string();
    }
    if n == f64::INFINITY {
        return "Infinity".to_string();
    }
    if n == f64::NEG_INFINITY {
        return "-Infinity".to_string();
    }
    if n == 0.0 {
        return "0".to_string();
    }

    let neg = n < 0.0;
    let x = n.abs();
    let sci = format!("{:e}", x);
    let (mant, exp_str) = match sci.split_once('e') {
        Some(t) => t,
        None => return sci,
    };
    let e: i64 = exp_str.parse().unwrap_or(0);
    let s: String = mant.chars().filter(|c| *c != '.').collect();
    let s = s.trim_end_matches('0');
    let s = if s.is_empty() { "0" } else { s };
    let k = s.len() as i64;

    let n_pt = e + 1;
    let body = if k <= n_pt && n_pt <= 21 {

        format!("{}{}", s, "0".repeat((n_pt - k) as usize))
    } else if 0 < n_pt && n_pt <= 21 {

        let np = n_pt as usize;
        format!("{}.{}", &s[..np], &s[np..])
    } else if -6 < n_pt && n_pt <= 0 {

        format!("0.{}{}", "0".repeat((-n_pt) as usize), s)
    } else {

        let exp = n_pt - 1;
        let sign = if exp >= 0 { "+" } else { "-" };
        let mantissa = if k == 1 {
            s.to_string()
        } else {
            format!("{}.{}", &s[..1], &s[1..])
        };
        format!("{mantissa}e{sign}{}", exp.abs())
    };
    if neg {
        format!("-{body}")
    } else {
        body
    }
}

pub fn same_value(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            if x.is_nan() && y.is_nan() {
                return true;
            }

            if *x == 0.0 && *y == 0.0 {
                return x.is_sign_positive() == y.is_sign_positive();
            }
            x == y
        }
        _ => is_strictly_equal(a, b),
    }
}

pub fn same_value_zero(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            if x.is_nan() && y.is_nan() {
                return true;
            }
            x == y
        }
        _ => is_strictly_equal(a, b),
    }
}

pub fn is_strictly_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Undefined, Value::Undefined) => true,
        (Value::Null, Value::Null) => true,
        (Value::Boolean(x), Value::Boolean(y)) => x == y,
        (Value::Number(x), Value::Number(y)) => {

            if x.is_nan() || y.is_nan() {
                return false;
            }
            x == y
        }
        (Value::String(x), Value::String(y)) => x.as_str() == y.as_str(),
        (Value::BigInt(x), Value::BigInt(y)) => x == y,

        (Value::Symbol(x), Value::Symbol(y)) => x.as_str() == y.as_str(),
        (Value::Object(x), Value::Object(y)) => x == y,
        _ => false,
    }
}

pub fn is_loosely_equal(a: &Value, b: &Value) -> bool {

    if std::mem::discriminant(a) == std::mem::discriminant(b) {
        return is_strictly_equal(a, b);
    }
    match (a, b) {
        (Value::Null, Value::Undefined) | (Value::Undefined, Value::Null) => true,
        (Value::Number(x), Value::String(s)) | (Value::String(s), Value::Number(x)) => {
            let y = parse_string_to_number(s.as_str());
            !x.is_nan() && !y.is_nan() && *x == y
        }

        (Value::BigInt(b), Value::Number(n)) | (Value::Number(n), Value::BigInt(b)) => {
            if n.is_nan() || n.is_infinite() || n.fract() != 0.0 {
                return false;
            }
            matches!(b.cmp_f64(*n), Some(std::cmp::Ordering::Equal))
        }

        (Value::BigInt(b), Value::String(s)) | (Value::String(s), Value::BigInt(b)) => {
            match crate::bigint::JsBigInt::from_decimal(s.as_str()) {
                Some(parsed) => b.cmp(&parsed) == std::cmp::Ordering::Equal,
                None => false,
            }
        }

        (Value::Boolean(b), other) | (other, Value::Boolean(b)) => {
            let nb = if *b { 1.0 } else { 0.0 };
            is_loosely_equal(&Value::Number(nb), other)
        }
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelOrder {
    Less,
    Greater,
    Equal,
    Undefined,
}

pub fn abstract_relational_compare(x: &Value, y: &Value) -> RelOrder {
    use std::cmp::Ordering::*;

    if let (Value::String(a), Value::String(b)) = (x, y) {

        return match a.code_units().as_ref().cmp(b.code_units().as_ref()) {
            Less => RelOrder::Less,
            Greater => RelOrder::Greater,
            Equal => RelOrder::Equal,
        };
    }

    let ord_to_rel = |o: std::cmp::Ordering| match o {
        Less => RelOrder::Less,
        Greater => RelOrder::Greater,
        Equal => RelOrder::Equal,
    };
    match (x, y) {
        (Value::BigInt(a), Value::BigInt(b)) => return ord_to_rel(a.cmp(b)),
        (Value::BigInt(a), Value::Number(n)) => {
            return match a.cmp_f64(*n) {
                Some(o) => ord_to_rel(o),
                None => RelOrder::Undefined,
            };
        }
        (Value::Number(n), Value::BigInt(b)) => {
            return match b.cmp_f64(*n) {
                Some(o) => ord_to_rel(o.reverse()),
                None => RelOrder::Undefined,
            };
        }

        (Value::BigInt(a), Value::String(s)) => {
            return match crate::bigint::JsBigInt::from_decimal(s.as_str().trim()) {
                Some(b) => ord_to_rel(a.cmp(&b)),
                None => RelOrder::Undefined,
            };
        }
        (Value::String(s), Value::BigInt(b)) => {
            return match crate::bigint::JsBigInt::from_decimal(s.as_str().trim()) {
                Some(a) => ord_to_rel(a.cmp(b)),
                None => RelOrder::Undefined,
            };
        }
        _ => {}
    }
    let nx = to_number(x);
    let ny = to_number(y);
    if nx.is_nan() || ny.is_nan() {
        return RelOrder::Undefined;
    }
    if nx < ny {
        RelOrder::Less
    } else if nx > ny {
        RelOrder::Greater
    } else {
        RelOrder::Equal
    }
}

pub fn to_bigint(
    rt: &mut crate::interp::Runtime,
    v: &Value,
) -> Result<Value, crate::interp::RuntimeError> {
    use crate::bigint::JsBigInt;
    use crate::interp::RuntimeError;
    let prim = match v {
        Value::Object(_) => rt.to_primitive(v, "number")?,
        _ => v.clone(),
    };
    match prim {
        Value::BigInt(b) => Ok(Value::BigInt(b)),
        Value::Boolean(b) => Ok(Value::BigInt(Rc::new(if b {
            JsBigInt::one()
        } else {
            JsBigInt::zero()
        }))),
        Value::String(s) => match JsBigInt::from_decimal(s.trim()) {
            Some(b) => Ok(Value::BigInt(Rc::new(b))),
            None => Err(RuntimeError::SyntaxError(format!(
                "Cannot convert {:?} to a BigInt",
                s.as_str()
            ))),
        },

        Value::Number(n) => {
            if !n.is_finite() || n.fract() != 0.0 {
                return Err(RuntimeError::RangeError(format!(
                    "The number {} cannot be converted to a BigInt because it is not an integer",
                    n
                )));
            }

            Ok(Value::BigInt(Rc::new(JsBigInt::from_f64_trunc(n))))
        }
        Value::Undefined => Err(RuntimeError::TypeError(
            "Cannot convert undefined to a BigInt".into(),
        )),
        Value::Null => Err(RuntimeError::TypeError(
            "Cannot convert null to a BigInt".into(),
        )),
        Value::Symbol(_) => Err(RuntimeError::TypeError(
            "Cannot convert a Symbol value to a BigInt".into(),
        )),
        Value::Object(_) => Err(RuntimeError::TypeError(
            "Cannot convert object to a BigInt".into(),
        )),
    }
}

pub fn to_bigint_value(
    rt: &mut crate::interp::Runtime,
    v: &Value,
) -> Result<Value, crate::interp::RuntimeError> {
    use crate::bigint::JsBigInt;
    use crate::interp::RuntimeError;
    let prim = match v {
        Value::Object(_) => rt.to_primitive(v, "number")?,
        _ => v.clone(),
    };
    match prim {
        Value::BigInt(b) => Ok(Value::BigInt(b)),
        Value::Boolean(b) => Ok(Value::BigInt(Rc::new(if b {
            JsBigInt::one()
        } else {
            JsBigInt::zero()
        }))),
        Value::String(s) => match JsBigInt::from_decimal(s.trim()) {
            Some(b) => Ok(Value::BigInt(Rc::new(b))),
            None => Err(RuntimeError::SyntaxError(format!(
                "Cannot convert {:?} to a BigInt",
                s.as_str()
            ))),
        },
        Value::Number(_) => Err(RuntimeError::TypeError(
            "Cannot convert a Number value to a BigInt".into(),
        )),
        Value::Undefined => Err(RuntimeError::TypeError(
            "Cannot convert undefined to a BigInt".into(),
        )),
        Value::Null => Err(RuntimeError::TypeError(
            "Cannot convert null to a BigInt".into(),
        )),
        Value::Symbol(_) => Err(RuntimeError::TypeError(
            "Cannot convert a Symbol value to a BigInt".into(),
        )),
        Value::Object(_) => Err(RuntimeError::TypeError(
            "Cannot convert object to a BigInt".into(),
        )),
    }
}

pub fn convert_number_to_typed_array_element(v: &Value, kind: &str) -> Value {
    let n = to_number(v);
    match kind {
        "Int8Array" => Value::Number(to_int_n(n, 8) as f64),
        "Uint8Array" => Value::Number(to_uint_n(n, 8) as f64),
        "Uint8ClampedArray" => Value::Number(to_uint8_clamp(n) as f64),
        "Int16Array" => Value::Number(to_int_n(n, 16) as f64),
        "Uint16Array" => Value::Number(to_uint_n(n, 16) as f64),
        "Int32Array" => Value::Number(to_int_n(n, 32) as f64),
        "Uint32Array" => Value::Number(to_uint_n(n, 32) as f64),
        "Float16Array" => Value::Number(f16_to_f64(f64_to_f16_bits(n))),
        _ => Value::Number(n),
    }
}

pub fn to_uint32(n: f64) -> u32 {
    to_uint_n(n, 32) as u32
}

pub fn to_int32(n: f64) -> i32 {
    to_int_n(n, 32) as i32
}

pub fn number_exponentiate(base: f64, exponent: f64) -> f64 {

    if exponent.is_nan() {
        return f64::NAN;
    }
    if base.abs() == 1.0 && exponent.is_infinite() {
        return f64::NAN;
    }
    base.powf(exponent)
}

fn to_uint_n(n: f64, bits: u32) -> u64 {
    if !n.is_finite() {
        return 0;
    }
    let modulus = 2f64.powi(bits as i32);
    n.trunc().rem_euclid(modulus) as u64
}

fn to_int_n(n: f64, bits: u32) -> i64 {
    if !n.is_finite() {
        return 0;
    }
    let modulus = 2f64.powi(bits as i32);
    let half = 2f64.powi(bits as i32 - 1);
    let rem = n.trunc().rem_euclid(modulus);
    if rem >= half {
        (rem - modulus) as i64
    } else {
        rem as i64
    }
}

fn to_uint8_clamp(n: f64) -> u8 {
    if n.is_nan() {
        return 0;
    }
    if n <= 0.0 {
        return 0;
    }
    if n >= 255.0 {
        return 255;
    }
    let floor = n.floor();
    let frac = n - floor;
    if frac < 0.5 {
        floor as u8
    } else if frac > 0.5 {
        (floor + 1.0) as u8
    } else {
        if (floor as i64) % 2 == 0 {
            floor as u8
        } else {
            (floor + 1.0) as u8
        }
    }
}

pub fn f16_to_f64(bits: u16) -> f64 {
    let sign = if bits & 0x8000 == 0 { 1.0 } else { -1.0 };
    let exp = ((bits >> 10) & 0x1f) as i32;
    let frac = (bits & 0x03ff) as u32;
    match exp {
        0 => {
            if frac == 0 {
                sign * 0.0
            } else {
                sign * (frac as f64 / 1024.0) * 2f64.powi(-14)
            }
        }
        0x1f => {
            if frac == 0 {
                sign * f64::INFINITY
            } else {
                f64::NAN
            }
        }
        _ => sign * (1.0 + frac as f64 / 1024.0) * 2f64.powi(exp - 15),
    }
}

pub fn f64_to_f16_bits(n: f64) -> u16 {
    fn round_shift_even(value: u128, shift: u32) -> u128 {
        if shift == 0 {
            return value;
        }
        if shift >= 128 {
            return 0;
        }
        let quotient = value >> shift;
        let remainder = value & ((1_u128 << shift) - 1);
        let halfway = 1_u128 << (shift - 1);
        if remainder > halfway || (remainder == halfway && (quotient & 1) != 0) {
            quotient + 1
        } else {
            quotient
        }
    }

    let bits = n.to_bits();
    let sign = ((bits >> 48) & 0x8000) as u16;
    let exp = ((bits >> 52) & 0x7ff) as i32;
    let frac = bits & 0x000f_ffff_ffff_ffff;

    if exp == 0x7ff {
        return sign | if frac == 0 { 0x7c00 } else { 0x7e00 };
    }
    if exp == 0 && frac == 0 {
        return sign;
    }

    let (mant, unbiased_exp) = if exp == 0 {
        (frac as u128, -1022)
    } else {
        (((1_u64 << 52) | frac) as u128, exp - 1023)
    };
    let mut half_exp = unbiased_exp + 15;
    if half_exp >= 0x1f {
        return sign | 0x7c00;
    }
    if half_exp <= 0 {
        let shift = (28 - unbiased_exp) as u32;
        let rounded = round_shift_even(mant, shift);
        if rounded == 0 {
            return sign;
        }
        if rounded >= 0x400 {
            return sign | 0x0400;
        }
        return sign | rounded as u16;
    }

    let mut rounded = round_shift_even(mant, 42);
    if rounded == 0x800 {
        half_exp += 1;
        rounded = 0x400;
        if half_exp >= 0x1f {
            return sign | 0x7c00;
        }
    }
    sign | ((half_exp as u16) << 10) | ((rounded as u16) & 0x03ff)
}

pub fn number_to_raw_bytes(kind: &str, value: &Value) -> [u8; 8] {
    let mut out = [0u8; 8];
    match kind {
        "Int8Array" => out[0] = to_int_n(to_number(value), 8) as i8 as u8,
        "Uint8Array" | "Uint8ClampedArray" => out[0] = to_uint_n(to_number(value), 8) as u8,
        "Int16Array" => {
            let v = to_int_n(to_number(value), 16) as i16;
            out[..2].copy_from_slice(&v.to_le_bytes());
        }
        "Uint16Array" => {
            let v = to_uint_n(to_number(value), 16) as u16;
            out[..2].copy_from_slice(&v.to_le_bytes());
        }
        "Float16Array" => {
            let v = f64_to_f16_bits(to_number(value));
            out[..2].copy_from_slice(&v.to_le_bytes());
        }
        "Int32Array" => {
            let v = to_int_n(to_number(value), 32) as i32;
            out[..4].copy_from_slice(&v.to_le_bytes());
        }
        "Uint32Array" => {
            let v = to_uint_n(to_number(value), 32) as u32;
            out[..4].copy_from_slice(&v.to_le_bytes());
        }
        "Float32Array" => {
            let v = to_number(value) as f32;
            out[..4].copy_from_slice(&v.to_le_bytes());
        }
        "Float64Array" => out[..8].copy_from_slice(&to_number(value).to_le_bytes()),
        "BigInt64Array" => {
            let v = match value {
                Value::BigInt(b) => b.to_u64_wrapping() as i64,
                _ => to_number(value) as i64,
            };
            out[..8].copy_from_slice(&v.to_le_bytes());
        }
        "BigUint64Array" => {
            let v = match value {
                Value::BigInt(b) => b.to_u64_wrapping(),
                _ => to_number(value) as u64,
            };
            out[..8].copy_from_slice(&v.to_le_bytes());
        }
        _ => {}
    }
    out
}

pub fn typed_array_byte_width(kind: &str) -> usize {
    match kind {
        "Int8Array" | "Uint8Array" | "Uint8ClampedArray" => 1,
        "Int16Array" | "Uint16Array" | "Float16Array" => 2,
        "Int32Array" | "Uint32Array" | "Float32Array" => 4,
        "Float64Array" | "BigInt64Array" | "BigUint64Array" => 8,
        _ => 1,
    }
}

pub fn raw_bytes_to_numeric(kind: &str, bytes: &[u8]) -> Value {
    use crate::bigint::JsBigInt;
    let width = typed_array_byte_width(kind);
    let take: usize = bytes.len().min(width);
    let mut buf = [0u8; 8];
    buf[..take].copy_from_slice(&bytes[..take]);
    match kind {
        "Int8Array" => Value::Number(i8::from_le_bytes([buf[0]]) as f64),
        "Uint8Array" | "Uint8ClampedArray" => Value::Number(buf[0] as f64),
        "Int16Array" => Value::Number(i16::from_le_bytes([buf[0], buf[1]]) as f64),
        "Uint16Array" => Value::Number(u16::from_le_bytes([buf[0], buf[1]]) as f64),
        "Float16Array" => Value::Number(f16_to_f64(u16::from_le_bytes([buf[0], buf[1]]))),
        "Int32Array" => Value::Number(i32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as f64),
        "Uint32Array" => Value::Number(u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as f64),
        "Float32Array" => {
            Value::Number(f32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as f64)
        }
        "Float64Array" => Value::Number(f64::from_le_bytes(buf)),
        "BigInt64Array" => Value::BigInt(Rc::new(JsBigInt::from_i64(i64::from_le_bytes(buf)))),
        "BigUint64Array" => Value::BigInt(Rc::new(JsBigInt::from_u64(u64::from_le_bytes(buf)))),
        _ => Value::Number(0.0),
    }
}

pub fn op_add(x: &Value, y: &Value) -> Value {
    if matches!(x, Value::String(_)) || matches!(y, Value::String(_)) {
        match (x, y) {
            (Value::String(xs), Value::String(ys))
                if xs.is_well_formed() && ys.is_well_formed() =>
            {
                let (lhs_len, rhs_len) = (xs.len(), ys.len());
                record_string_concat(lhs_len, rhs_len);
                if lhs_len + rhs_len >= lazy_string_concat_min_bytes() {
                    return Value::String(Rc::new(crate::value::JsString::Concat {
                        left: xs.clone(),
                        right: ys.clone(),
                        byte_len: lhs_len + rhs_len,
                        flat: std::cell::OnceCell::new(),
                    }));
                }
                let (xs, ys) = (xs.as_str(), ys.as_str());
                let mut concat = String::with_capacity(lhs_len + rhs_len);
                concat.push_str(xs);
                concat.push_str(ys);
                return Value::String(Rc::new(crate::value::JsString::wellformed(concat)));
            }
            (Value::String(xs), other)
                if xs.is_well_formed() && !matches!(other, Value::String(_)) =>
            {
                let ys = to_string(other);
                let xs = xs.as_str();
                let mut concat = String::with_capacity(xs.len() + ys.len());
                concat.push_str(xs);
                concat.push_str(&ys);
                record_string_concat(xs.len(), ys.len());
                return Value::String(Rc::new(crate::value::JsString::wellformed(concat)));
            }
            (other, Value::String(ys))
                if ys.is_well_formed() && !matches!(other, Value::String(_)) =>
            {
                let xs = to_string(other);
                let ys = ys.as_str();
                let mut concat = String::with_capacity(xs.len() + ys.len());
                concat.push_str(&xs);
                concat.push_str(ys);
                record_string_concat(xs.len(), ys.len());
                return Value::String(Rc::new(crate::value::JsString::wellformed(concat)));
            }
            _ => {}
        }

        let xj: Rc<crate::value::JsString> = match x {
            Value::String(js) => js.clone(),
            _ => Rc::new(crate::value::JsString::from(to_string(x))),
        };
        let yj: Rc<crate::value::JsString> = match y {
            Value::String(js) => js.clone(),
            _ => Rc::new(crate::value::JsString::from(to_string(y))),
        };

        if xj.is_well_formed() && yj.is_well_formed() {
            let (xs, ys) = (xj.as_str(), yj.as_str());
            let mut concat = String::with_capacity(xs.len() + ys.len());
            concat.push_str(xs);
            concat.push_str(ys);
            record_string_concat(xs.len(), ys.len());
            return Value::String(Rc::new(crate::value::JsString::wellformed(concat)));
        }

        let mut units = xj.code_units().into_owned();
        units.extend_from_slice(&yj.code_units());
        record_string_concat(xj.len(), yj.len());
        return Value::String(Rc::new(crate::value::JsString::from_code_units(units)));
    }
    if let (Value::BigInt(a), Value::BigInt(b)) = (x, y) {
        return Value::BigInt(Rc::new(a.add(b)));
    }
    Value::Number(to_number(x) + to_number(y))
}
