
use super::PgError;
use sql_core::SqlValue;

fn parse_special(s: &str) -> Option<&'static str> {
    match s.to_ascii_lowercase().as_str() {
        "nan" => Some("NaN"),
        "inf" | "infinity" | "+inf" | "+infinity" => Some("Infinity"),
        "-inf" | "-infinity" => Some("-Infinity"),
        _ => None,
    }
}

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    let typ = super::type_name(oid);
    let invalid = || PgError::InvalidInputSyntax {
        typ,
        input: text.to_string(),
    };

    let s = text.trim();
    if s.is_empty() {
        return Err(invalid());
    }
    if let Some(canon) = parse_special(s) {
        return Ok(SqlValue::Text(canon.to_string()));
    }

    let (neg, body) = match s.as_bytes()[0] {
        b'+' => (false, &s[1..]),
        b'-' => (true, &s[1..]),
        _ => (false, s),
    };

    let (mantissa, exp): (&str, i64) = match body.split_once(['e', 'E']) {
        Some((m, exp_str)) => {

            let digits = match exp_str.as_bytes().first() {
                Some(b'+') | Some(b'-') => &exp_str[1..],
                _ => exp_str,
            };
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return Err(invalid());
            }
            let e: i64 = match exp_str.parse() {
                Ok(e) => e,
                Err(_) => return Err(invalid()),
            };
            (m, e)
        }
        None => (body, 0),
    };

    let (int_part, frac_part) = match mantissa.split_once('.') {
        Some((i, f)) => {
            if f.contains('.') {
                return Err(invalid());
            }
            (i, f)
        }
        None => (mantissa, ""),
    };
    if int_part.is_empty() && frac_part.is_empty() {
        return Err(invalid());
    }
    if !int_part.bytes().all(|b| b.is_ascii_digit())
        || !frac_part.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(invalid());
    }

    Ok(SqlValue::Text(canonicalize(neg, int_part, frac_part, exp)))
}

fn canonicalize(neg: bool, int_part: &str, frac_part: &str, exp: i64) -> String {
    let all_digits: String = format!("{int_part}{frac_part}");
    let total = all_digits.len() as i64;
    let frac_len = frac_part.len() as i64;
    let ef = frac_len - exp;

    let (int_digits, frac_digits): (String, String) = if ef <= 0 {

        let mut d = all_digits;
        for _ in 0..(-ef) {
            d.push('0');
        }
        (d, String::new())
    } else {
        let int_count = total - ef;
        if int_count <= 0 {

            let lead = "0".repeat((-int_count) as usize);
            (String::from("0"), format!("{lead}{all_digits}"))
        } else {
            let split = int_count as usize;
            (
                all_digits[..split].to_string(),
                all_digits[split..].to_string(),
            )
        }
    };

    let int_trimmed = int_digits.trim_start_matches('0');
    let int_out = if int_trimmed.is_empty() {
        "0"
    } else {
        int_trimmed
    };

    let is_zero = int_out == "0" && frac_digits.bytes().all(|b| b == b'0');

    let mut out = String::new();
    if neg && !is_zero {
        out.push('-');
    }
    out.push_str(int_out);
    if !frac_digits.is_empty() {
        out.push('.');
        out.push_str(&frac_digits);
    }
    out
}

pub fn output(_oid: u32, v: &SqlValue) -> String {
    match v {
        SqlValue::Text(s) => s.clone(),
        _ => String::new(),
    }
}

use std::cmp::Ordering;

enum Val {
    Nan,
    PosInf,
    NegInf,
    Fin {
        neg: bool,
        int: String,
        frac: String,
    },
}

fn parse_val(v: &SqlValue) -> Option<Val> {
    match v {
        SqlValue::Int(n) => {
            let neg = *n < 0;
            let int = (*n as i128).unsigned_abs().to_string();
            Some(Val::Fin {
                neg,
                int,
                frac: String::new(),
            })
        }
        SqlValue::Text(s) => parse_val_text(s),
        _ => None,
    }
}

fn parse_val_text(text: &str) -> Option<Val> {
    let s = text.trim();
    if s.is_empty() {
        return None;
    }
    match s.to_ascii_lowercase().as_str() {
        "nan" => return Some(Val::Nan),
        "inf" | "infinity" | "+inf" | "+infinity" => return Some(Val::PosInf),
        "-inf" | "-infinity" => return Some(Val::NegInf),
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

    let int_trim = int_part.trim_start_matches('0');
    let int = if int_trim.is_empty() {
        "0".to_string()
    } else {
        int_trim.to_string()
    };
    let frac = frac_part.trim_end_matches('0').to_string();
    Some(Val::Fin { neg, int, frac })
}

fn rank(v: &Val) -> u8 {
    match v {
        Val::NegInf => 0,
        Val::Fin { .. } => 1,
        Val::PosInf => 2,
        Val::Nan => 3,
    }
}

fn cmp_magnitude(ai: &str, af: &str, bi: &str, bf: &str) -> Ordering {
    match ai.len().cmp(&bi.len()).then_with(|| ai.cmp(bi)) {
        Ordering::Equal => {}
        ord => return ord,
    }
    let n = af.len().max(bf.len());
    let pad = |f: &str| {
        let mut s = f.to_string();
        while s.len() < n {
            s.push('0');
        }
        s
    };
    pad(af).cmp(&pad(bf))
}

fn cmp_val(a: &Val, b: &Val) -> Ordering {
    let (ra, rb) = (rank(a), rank(b));
    if ra != rb {
        return ra.cmp(&rb);
    }
    match (a, b) {
        (
            Val::Fin {
                neg: an,
                int: ai,
                frac: af,
            },
            Val::Fin {
                neg: bn,
                int: bi,
                frac: bf,
            },
        ) => {
            let az = ai == "0" && af.is_empty();
            let bz = bi == "0" && bf.is_empty();

            let sa = if az {
                0
            } else if *an {
                -1
            } else {
                1
            };
            let sb = if bz {
                0
            } else if *bn {
                -1
            } else {
                1
            };
            if sa != sb {
                return sa.cmp(&sb);
            }
            if sa == 0 {
                return Ordering::Equal;
            }
            let mag = cmp_magnitude(ai, af, bi, bf);
            if *an {
                mag.reverse()
            } else {
                mag
            }
        }

        _ => Ordering::Equal,
    }
}

pub fn value_cmp(a: &SqlValue, b: &SqlValue) -> Option<Ordering> {
    Some(cmp_val(&parse_val(a)?, &parse_val(b)?))
}

fn real_to_val(f: f64) -> Option<Val> {
    if f.is_nan() {
        return Some(Val::Nan);
    }
    if f.is_infinite() {
        return Some(if f > 0.0 { Val::PosInf } else { Val::NegInf });
    }
    parse_val_text(&format!("{f}"))
}

fn to_val_any(v: &SqlValue) -> Option<Val> {
    match v {
        SqlValue::Real(f) => real_to_val(*f),
        _ => parse_val(v),
    }
}

pub fn value_cmp_real_bridge(a: &SqlValue, b: &SqlValue) -> Option<Ordering> {
    Some(cmp_val(&to_val_any(a)?, &to_val_any(b)?))
}

fn complement(key: &str) -> String {
    key.bytes().map(|c| (0x63 - c) as char).collect()
}

fn magnitude_key(int: &str, frac: &str) -> String {
    format!("{:07}{int}{frac}*", int.len())
}

pub fn sort_key(v: &SqlValue) -> SqlValue {
    let parsed = match parse_val(v) {
        Some(p) => p,
        None => return v.clone(),
    };
    let key = match parsed {
        Val::NegInf => "0".to_string(),
        Val::PosInf => "3".to_string(),
        Val::Nan => "4".to_string(),
        Val::Fin { neg, int, frac } => {
            let zero = int == "0" && frac.is_empty();
            let mk = magnitude_key(&int, &frac);
            if neg && !zero {
                format!("1{}", complement(&mk))
            } else {
                format!("2{mk}")
            }
        }
    };
    SqlValue::Text(key)
}

struct Dec {
    neg: bool,
    coeff: String,
    scale: usize,
}

fn parse_dec(v: &SqlValue) -> Option<Dec> {
    match v {
        SqlValue::Int(n) => {
            let neg = *n < 0;
            let coeff = (*n as i128).unsigned_abs().to_string();
            let coeff = coeff.trim_start_matches('0').to_string();
            Some(Dec {
                neg,
                coeff,
                scale: 0,
            })
        }
        SqlValue::Text(s) => parse_dec_text(s),
        _ => None,
    }
}

fn parse_dec_text(text: &str) -> Option<Dec> {
    let s = text.trim();
    if s.is_empty() {
        return None;
    }

    match s.to_ascii_lowercase().as_str() {
        "nan" | "inf" | "infinity" | "+inf" | "+infinity" | "-inf" | "-infinity" => return None,
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
    let coeff = format!("{int_part}{frac_part}");
    let coeff = coeff.trim_start_matches('0').to_string();
    Some(Dec {
        neg,
        coeff,
        scale: frac_part.len(),
    })
}

fn coeff_is_zero(coeff: &str) -> bool {
    coeff.bytes().all(|b| b == b'0')
}

fn format_dec(neg: bool, coeff: &str, scale: usize) -> String {

    let mut digits = coeff.trim_start_matches('0').to_string();
    while digits.len() <= scale {
        digits.insert(0, '0');
    }
    let split = digits.len() - scale;
    let int = &digits[..split];
    let frac = &digits[split..];
    let is_zero = coeff_is_zero(int) && coeff_is_zero(frac);
    let mut out = String::new();
    if neg && !is_zero {
        out.push('-');
    }
    out.push_str(int);
    if scale > 0 {
        out.push('.');
        out.push_str(frac);
    }
    out
}

fn ucmp(a: &str, b: &str) -> Ordering {
    let a = a.trim_start_matches('0');
    let b = b.trim_start_matches('0');
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

fn uadd(a: &str, b: &str) -> String {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut out = Vec::new();
    let (mut i, mut j) = (a.len() as isize - 1, b.len() as isize - 1);
    let mut carry = 0u8;
    while i >= 0 || j >= 0 || carry > 0 {
        let da = if i >= 0 { a[i as usize] - b'0' } else { 0 };
        let db = if j >= 0 { b[j as usize] - b'0' } else { 0 };
        let s = da + db + carry;
        out.push(b'0' + s % 10);
        carry = s / 10;
        i -= 1;
        j -= 1;
    }
    out.reverse();
    String::from_utf8(out).unwrap()
}

fn usub(a: &str, b: &str) -> String {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut out = Vec::new();
    let (mut i, mut j) = (a.len() as isize - 1, b.len() as isize - 1);
    let mut borrow = 0i8;
    while i >= 0 {
        let da = (a[i as usize] - b'0') as i8;
        let db = if j >= 0 {
            (b[j as usize] - b'0') as i8
        } else {
            0
        };
        let mut d = da - db - borrow;
        if d < 0 {
            d += 10;
            borrow = 1;
        } else {
            borrow = 0;
        }
        out.push(b'0' + d as u8);
        i -= 1;
        j -= 1;
    }
    out.reverse();
    let trimmed = String::from_utf8(out).unwrap();
    let t = trimmed.trim_start_matches('0');
    if t.is_empty() {
        "0".to_string()
    } else {
        t.to_string()
    }
}

fn umul(a: &str, b: &str) -> String {
    let a = a.trim_start_matches('0');
    let b = b.trim_start_matches('0');
    if a.is_empty() || b.is_empty() {
        return "0".to_string();
    }
    let (ad, bd) = (a.as_bytes(), b.as_bytes());
    let mut acc = vec![0u32; ad.len() + bd.len()];
    for (i, &ca) in ad.iter().rev().enumerate() {
        for (j, &cb) in bd.iter().rev().enumerate() {
            acc[i + j] += (ca - b'0') as u32 * (cb - b'0') as u32;
        }
    }
    let mut carry = 0u32;
    for slot in acc.iter_mut() {
        let cur = *slot + carry;
        *slot = cur % 10;
        carry = cur / 10;
    }
    let mut out: String = acc
        .iter()
        .rev()
        .map(|d| (b'0' + *d as u8) as char)
        .collect();
    let t = out.trim_start_matches('0');
    out = if t.is_empty() {
        "0".to_string()
    } else {
        t.to_string()
    };
    out
}

fn udivmod(numer: &str, denom: &str) -> (String, String) {
    let denom_t = denom.trim_start_matches('0');
    let mut q = String::new();
    let mut rem = String::from("0");
    for ch in numer.chars() {

        rem.push(ch);
        let rem_t = rem.trim_start_matches('0');
        rem = if rem_t.is_empty() {
            "0".to_string()
        } else {
            rem_t.to_string()
        };

        let mut d = 0u8;
        while d < 9 {
            let trial = umul(denom_t, &((d + 1).to_string()));
            if ucmp(&trial, &rem) == Ordering::Greater {
                break;
            }
            d += 1;
        }
        q.push((b'0' + d) as char);
        let sub = umul(denom_t, &d.to_string());
        rem = usub(&rem, &sub);
    }
    let qt = q.trim_start_matches('0');
    let q = if qt.is_empty() {
        "0".to_string()
    } else {
        qt.to_string()
    };
    (q, rem)
}

fn pad_right(s: &str, n: usize) -> String {
    let mut out = s.to_string();
    for _ in 0..n {
        out.push('0');
    }
    out
}

fn add_sub(a: &Dec, b: &Dec, sub: bool) -> Dec {
    let bneg = if sub { !b.neg } else { b.neg };
    let scale = a.scale.max(b.scale);

    let ac = pad_right(&a.coeff, scale - a.scale);
    let bc = pad_right(&b.coeff, scale - b.scale);
    let az = coeff_is_zero(&ac);
    let bz = coeff_is_zero(&bc);
    let aneg = a.neg && !az;
    let bneg = bneg && !bz;
    let (coeff, neg) = if aneg == bneg {
        (uadd(&ac, &bc), aneg)
    } else {

        match ucmp(&ac, &bc) {
            Ordering::Greater | Ordering::Equal => (usub(&ac, &bc), aneg),
            Ordering::Less => (usub(&bc, &ac), bneg),
        }
    };
    Dec { neg, coeff, scale }
}

fn mul(a: &Dec, b: &Dec) -> Dec {
    let coeff = umul(&a.coeff, &b.coeff);
    let neg = (a.neg != b.neg) && !coeff_is_zero(&coeff);
    Dec {
        neg,
        coeff,
        scale: a.scale + b.scale,
    }
}

fn rem(a: &Dec, b: &Dec) -> Dec {
    let scale = a.scale.max(b.scale);
    let ac = pad_right(&a.coeff, scale - a.scale);
    let bc = pad_right(&b.coeff, scale - b.scale);
    let (_q, r) = udivmod(&ac, &bc);
    let neg = a.neg && !coeff_is_zero(&r);
    Dec {
        neg,
        coeff: r,
        scale,
    }
}

fn base10000_weight(d: &Dec) -> (i64, u32) {
    let sig = d.coeff.trim_start_matches('0');
    if sig.is_empty() {
        return (0, 0);
    }

    let dw = d.coeff.len() as i64 - d.scale as i64 - 1 - (d.coeff.len() - sig.len()) as i64;
    let w = dw.div_euclid(4);
    let r = dw.rem_euclid(4) as usize;

    let take = r + 1;
    let mut top = String::new();
    for i in 0..take {
        top.push(sig.as_bytes().get(i).map(|b| *b as char).unwrap_or('0'));
    }
    (w, top.parse().unwrap_or(0))
}

fn div_scale(a: &Dec, b: &Dec) -> usize {
    const MIN_SIG_DIGITS: i64 = 16;
    const DEC_DIGITS: i64 = 4;
    let (w1, f1) = base10000_weight(a);
    let (w2, f2) = base10000_weight(b);
    let mut qweight = w1 - w2;
    if f1 <= f2 {
        qweight -= 1;
    }
    let mut rscale = MIN_SIG_DIGITS - qweight * DEC_DIGITS;
    rscale = rscale.max(a.scale as i64).max(b.scale as i64).max(0);
    rscale as usize
}

fn div(a: &Dec, b: &Dec) -> Dec {
    let rscale = div_scale(a, b);

    let k = rscale as i64 + b.scale as i64 - a.scale as i64;
    let (numer, denom) = if k >= 0 {
        (pad_right(&a.coeff, k as usize), b.coeff.clone())
    } else {
        (a.coeff.clone(), pad_right(&b.coeff, (-k) as usize))
    };

    let numer_g = pad_right(&numer, 1);
    let (mut q, _rem) = udivmod(&numer_g, &denom);

    let last = q.as_bytes().last().map(|b| b - b'0').unwrap_or(0);
    q.pop();
    if q.is_empty() {
        q.push('0');
    }
    if last >= 5 {
        q = uadd(&q, "1");
    }
    let neg = (a.neg != b.neg) && !coeff_is_zero(&q);
    Dec {
        neg,
        coeff: q,
        scale: rscale,
    }
}

fn usqrt(n: &str) -> String {
    let n = n.trim_start_matches('0');
    if n.is_empty() {
        return "0".to_string();
    }
    if n.len() == 1 {

        let d = n.as_bytes()[0] - b'0';
        let r = (d as f64).sqrt().floor() as u8;
        return ((b'0' + r) as char).to_string();
    }

    let mut x = {
        let mut s = String::from("1");
        for _ in 0..n.len().div_ceil(2) {
            s.push('0');
        }
        s
    };
    loop {

        let (q, _) = udivmod(n, &x);
        let sum = uadd(&x, &q);
        let (y, _) = udivmod(&sum, "2");
        if ucmp(&y, &x) != Ordering::Less {
            break;
        }
        x = y;
    }

    let t = x.trim_start_matches('0');
    if t.is_empty() {
        "0".to_string()
    } else {
        t.to_string()
    }
}

pub fn numeric_sqrt(v: &SqlValue, out_scale: usize) -> Option<SqlValue> {
    let d = parse_dec(v)?;
    if d.neg && !coeff_is_zero(&d.coeff) {
        return None;
    }
    if coeff_is_zero(&d.coeff) {
        return Some(SqlValue::Text(format_dec(false, "0", out_scale)));
    }

    let guard = out_scale + 1;
    let e = 2 * guard as i64 - d.scale as i64;
    let radicand = if e >= 0 {
        pad_right(&d.coeff, e as usize)
    } else {

        let (q, _) = udivmod(&d.coeff, &pad_right("1", (-e) as usize));
        q
    };
    let mut rg = usqrt(&radicand);

    let last = rg.as_bytes().last().map(|b| b - b'0').unwrap_or(0);
    rg.pop();
    if rg.is_empty() {
        rg.push('0');
    }
    if last >= 5 {
        rg = uadd(&rg, "1");
    }
    Some(SqlValue::Text(format_dec(false, &rg, out_scale)))
}

pub fn display_scale(v: &SqlValue) -> usize {
    match v {
        SqlValue::Text(s) => parse_dec_text(s).map(|d| d.scale).unwrap_or(0),
        _ => 0,
    }
}

pub fn arith(op: char, l: &SqlValue, r: &SqlValue) -> Option<Result<SqlValue, PgError>> {
    if !matches!(op, '+' | '-' | '*' | '/' | '%') {
        return None;
    }

    if matches!(l, SqlValue::Real(_)) || matches!(r, SqlValue::Real(_)) {
        return None;
    }
    if !matches!(l, SqlValue::Text(_)) && !matches!(r, SqlValue::Text(_)) {
        return None;
    }
    let a = parse_dec(l)?;
    let b = parse_dec(r)?;
    let out = match op {
        '+' => add_sub(&a, &b, false),
        '-' => add_sub(&a, &b, true),
        '*' => mul(&a, &b),
        '/' => {
            if coeff_is_zero(&b.coeff) {
                return Some(Err(PgError::DivisionByZero));
            }
            div(&a, &b)
        }
        '%' => {
            if coeff_is_zero(&b.coeff) {
                return Some(Err(PgError::DivisionByZero));
            }
            rem(&a, &b)
        }
        _ => unreachable!(),
    };
    Some(Ok(SqlValue::Text(format_dec(
        out.neg, &out.coeff, out.scale,
    ))))
}

fn round_dec(d: &Dec, target: usize) -> Dec {

    let mut coeff = if d.coeff.is_empty() {
        "0".to_string()
    } else {
        d.coeff.clone()
    };
    while coeff.len() <= d.scale {
        coeff.insert(0, '0');
    }
    if target >= d.scale {
        return Dec {
            neg: d.neg,
            coeff: pad_right(&coeff, target - d.scale),
            scale: target,
        };
    }
    let drop = d.scale - target;
    let keep_len = coeff.len() - drop;
    let round_up = coeff.as_bytes()[keep_len] >= b'5';
    let mut result = coeff[..keep_len].to_string();
    if round_up {
        result = uadd(&result, "1");
    }
    Dec {
        neg: d.neg,
        coeff: result,
        scale: target,
    }
}

pub fn apply_typmod(typmod: i32, v: SqlValue) -> Result<SqlValue, PgError> {
    let tmp = typmod - 4;
    let precision = ((tmp >> 16) & 0xffff) as i64;
    let scale = (tmp & 0x7ff) as i64;
    if precision <= 0 {
        return Ok(v);
    }
    let d = match parse_dec(&v) {
        Some(d) => d,
        None => return Ok(v),
    };
    let rounded = round_dec(&d, scale as usize);

    let int_len = rounded.coeff.len().saturating_sub(rounded.scale);
    let int_sig = rounded.coeff[..int_len].trim_start_matches('0');
    if (int_sig.len() as i64) > precision - scale {
        return Err(PgError::NumericFieldOverflow);
    }
    Ok(SqlValue::Text(format_dec(
        rounded.neg,
        &rounded.coeff,
        rounded.scale,
    )))
}

pub fn negate(v: &SqlValue) -> Option<SqlValue> {
    let d = parse_dec(v)?;
    let neg = !d.neg && !coeff_is_zero(&d.coeff);
    Some(SqlValue::Text(format_dec(neg, &d.coeff, d.scale)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::oid::NUMERIC;

    fn canon(s: &str) -> String {
        match input(NUMERIC, s) {
            Ok(SqlValue::Text(t)) => t,
            other => panic!("expected Text for {s:?}, got {other:?}"),
        }
    }

    #[test]
    fn input_plain_integers_and_whitespace() {
        assert_eq!(canon(" 123"), "123");
        assert_eq!(canon("   3245874    "), "3245874");
        assert_eq!(canon("  -93853"), "-93853");
        assert_eq!(canon("299792458"), "299792458");
    }

    #[test]
    fn input_fractions_scale_preserved() {

        assert_eq!(canon("555.50"), "555.50");
        assert_eq!(canon("-555.50"), "-555.50");
        assert_eq!(canon("1.0"), "1.0");
        assert_eq!(canon("0.00"), "0.00");
    }

    #[test]
    fn input_exponents() {
        assert_eq!(canon("1e3"), "1000");
        assert_eq!(canon("1.2e-5"), "0.000012");

        assert_eq!(canon("23000000000e-10"), "2.3000000000");
        assert_eq!(canon(".000000000123e10"), "1.23");
        assert_eq!(canon(".000000000123e+11"), "12.3");
        assert_eq!(canon("1.23E2"), "123");
    }

    #[test]
    fn input_canonical_shapes() {
        assert_eq!(canon("0.5"), "0.5");
        assert_eq!(canon(".5"), "0.5");
        assert_eq!(canon("5."), "5");
        assert_eq!(canon("007"), "7");
        assert_eq!(canon("100"), "100");
        assert_eq!(canon("0"), "0");
        assert_eq!(canon("-0"), "0");
        assert_eq!(canon("+42"), "42");
    }

    #[test]
    fn input_specials() {
        assert_eq!(canon("NaN "), "NaN");
        assert_eq!(canon("        nan"), "NaN");
        assert_eq!(canon(" inf "), "Infinity");
        assert_eq!(canon(" +inf "), "Infinity");
        assert_eq!(canon(" -inf "), "-Infinity");
        assert_eq!(canon(" Infinity "), "Infinity");
        assert_eq!(canon(" +inFinity "), "Infinity");
        assert_eq!(canon(" -INFINITY "), "-Infinity");
    }

    #[test]
    fn input_invalid_syntax() {
        for bad in [
            "",
            "     ",
            "   1234   %",
            "xyz",
            "- 1234",
            "5 . 0",
            "5. 0   ",
            " N aN ",
            "+NaN",
            "-NaN",
            "+ infinity",
            "5.0.0",
            "-",
            ".",
            "e5",
            "1e",
            "1e+",
            "1.2e3.4",
        ] {
            match input(NUMERIC, bad) {
                Err(PgError::InvalidInputSyntax { typ, input }) => {
                    assert_eq!(typ, "numeric");
                    assert_eq!(input, bad, "error carries original literal");
                }
                other => panic!("expected InvalidInputSyntax for {bad:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn deferred_underscore_and_radix_reject() {
        for deferred in [
            "12_000_000_000",
            "12_000.123_456",
            "0b1010",
            "0o755",
            "0x1eg",
            "0x1234",
        ] {
            assert!(
                matches!(
                    input(NUMERIC, deferred),
                    Err(PgError::InvalidInputSyntax { .. })
                ),
                "{deferred:?} is deferred-accept; currently rejected"
            );
        }
    }

    #[test]
    fn output_returns_stored_text() {
        assert_eq!(output(NUMERIC, &SqlValue::Text("555.50".into())), "555.50");
        assert_eq!(output(NUMERIC, &SqlValue::Text("NaN".into())), "NaN");

        assert_eq!(output(NUMERIC, &SqlValue::Real(1.0)), "");
        assert_eq!(output(NUMERIC, &SqlValue::Int(1)), "");
        assert_eq!(output(NUMERIC, &SqlValue::Null), "");
    }

    #[test]
    fn round_trip_canonical() {
        for s in [
            "123",
            "555.50",
            "-93853",
            "2.3000000000",
            "1.23",
            "0.00",
            "1000",
        ] {
            let v = input(NUMERIC, s).unwrap();
            assert_eq!(output(NUMERIC, &v), s, "canonical text round-trips");
        }

        let once = canon(" 007.5000e0 ");
        assert_eq!(once, "7.5000");
        assert_eq!(canon(&once), once, "canonicalization is idempotent");
    }

    #[test]
    fn scale_is_still_part_of_display_identity() {

        assert_ne!(canon("1"), canon("1.0"));
        assert_eq!(canon("1"), "1");
        assert_eq!(canon("1.0"), "1.0");

        assert_eq!(canon("1.23456789"), "1.23456789");
    }

    fn vc(a: &str, b: &str) -> Ordering {
        value_cmp(&SqlValue::Text(a.into()), &SqlValue::Text(b.into())).unwrap()
    }

    #[test]
    fn value_cmp_ignores_scale() {
        assert_eq!(vc("1.5", "1.50"), Ordering::Equal);
        assert_eq!(vc("1", "1.0"), Ordering::Equal);
        assert_eq!(vc("0", "0.00"), Ordering::Equal);
        assert_eq!(vc("0", "-0"), Ordering::Equal);
        assert_eq!(vc("100.00", "100"), Ordering::Equal);
    }

    #[test]
    fn value_cmp_orders_by_value_not_bytes() {

        assert_eq!(vc("10", "9"), Ordering::Greater);
        assert_eq!(vc("100", "99"), Ordering::Greater);
        assert_eq!(vc("-42.5", "0.00"), Ordering::Less);
        assert_eq!(vc("-10", "-9"), Ordering::Less);
        assert_eq!(vc("0.001", "0.01"), Ordering::Less);
        assert_eq!(vc("0.5", "0.45"), Ordering::Greater);
        assert_eq!(vc("12345.6789", "100.00"), Ordering::Greater);
    }

    #[test]
    fn value_cmp_specials_pg_order() {

        assert_eq!(vc("-Infinity", "-99999"), Ordering::Less);
        assert_eq!(vc("Infinity", "99999"), Ordering::Greater);
        assert_eq!(vc("NaN", "Infinity"), Ordering::Greater);
        assert_eq!(vc("NaN", "NaN"), Ordering::Equal);
        assert_eq!(vc("Infinity", "Infinity"), Ordering::Equal);
    }

    #[test]
    fn value_cmp_mixed_int_and_none_for_nonnumeric() {
        assert_eq!(
            value_cmp(&SqlValue::Int(10), &SqlValue::Text("9".into())),
            Some(Ordering::Greater)
        );
        assert_eq!(
            value_cmp(&SqlValue::Int(-3), &SqlValue::Text("2.5".into())),
            Some(Ordering::Less)
        );

        assert_eq!(
            value_cmp(&SqlValue::Text("abc".into()), &SqlValue::Int(1)),
            None
        );
        assert_eq!(
            value_cmp(&SqlValue::Text("x".into()), &SqlValue::Text("y".into())),
            None
        );
        assert_eq!(value_cmp(&SqlValue::Null, &SqlValue::Int(1)), None);
    }

    #[test]
    fn sort_key_byte_order_equals_value_order() {
        let corpus = [
            "NaN",
            "Infinity",
            "12345.6789",
            "100.00",
            "100",
            "10",
            "9",
            "1.50",
            "1.5",
            "1",
            "0.10",
            "0.01",
            "0.001",
            "0.00",
            "0",
            "-0",
            "-0.001",
            "-9",
            "-10",
            "-42.5",
            "-100",
            "-Infinity",
        ];
        for a in corpus {
            for b in corpus {
                let ka = sort_key(&SqlValue::Text(a.into()));
                let kb = sort_key(&SqlValue::Text(b.into()));
                let (ka, kb) = match (ka, kb) {
                    (SqlValue::Text(x), SqlValue::Text(y)) => (x, y),
                    _ => panic!("sort_key returned non-Text"),
                };
                let by_key = ka.cmp(&kb);
                let by_val = vc(a, b);
                assert_eq!(
                    by_key, by_val,
                    "sort_key order {a:?}({ka}) vs {b:?}({kb}) = {by_key:?}, value = {by_val:?}"
                );
            }
        }
    }

    fn num(s: &str) -> SqlValue {
        SqlValue::Text(s.into())
    }
    fn ar(op: char, a: &str, b: &str) -> String {
        match arith(op, &num(a), &num(b)) {
            Some(Ok(SqlValue::Text(t))) => t,
            other => panic!("arith {op} {a} {b} = {other:?}"),
        }
    }

    #[test]
    fn add_sub_scale_is_max() {
        assert_eq!(ar('+', "1.5", "1.25"), "2.75");
        assert_eq!(ar('+', "1.50", "1"), "2.50");
        assert_eq!(ar('-', "2.75", "1.25"), "1.50");
        assert_eq!(ar('-', "1", "1.50"), "-0.50");
        assert_eq!(ar('+', "0.00", "0"), "0.00");
        assert_eq!(ar('+', "-1.5", "1.25"), "-0.25");
        assert_eq!(
            ar('+', "999999999999999999999", "1"),
            "1000000000000000000000"
        );
    }

    #[test]
    fn mul_scale_is_sum() {
        assert_eq!(ar('*', "1.5", "1.25"), "1.875");
        assert_eq!(ar('*', "2.00", "3.0"), "6.000");
        assert_eq!(ar('*', "-2.5", "4"), "-10.0");
        assert_eq!(ar('*', "0.00", "5"), "0.00");
        assert_eq!(ar('*', "-1.5", "-2"), "3.0");
    }

    #[test]
    fn div_uses_pg_select_div_scale() {
        assert_eq!(ar('/', "6.0", "2.0"), "3.0000000000000000");
        assert_eq!(ar('/', "10", "4"), "2.5000000000000000");
        assert_eq!(ar('/', "10", "3"), "3.3333333333333333");
        assert_eq!(ar('/', "9.9", "3.3"), "3.0000000000000000");
        assert_eq!(ar('/', "1", "8"), "0.12500000000000000000");
        assert_eq!(ar('/', "-10", "4"), "-2.5000000000000000");
    }

    #[test]
    fn div_by_zero_errors() {
        assert_eq!(
            arith('/', &num("1"), &num("0")),
            Some(Err(PgError::DivisionByZero))
        );
        assert_eq!(
            arith('/', &num("1"), &num("0.00")),
            Some(Err(PgError::DivisionByZero))
        );
    }

    #[test]
    fn arith_path_detection() {

        assert_eq!(arith('+', &SqlValue::Int(1), &SqlValue::Int(2)), None);

        assert_eq!(arith('+', &num("1.5"), &SqlValue::Real(2.0)), None);

        assert_eq!(arith('+', &num("abc"), &SqlValue::Int(1)), None);

        assert_eq!(
            arith('+', &num("1.50"), &SqlValue::Int(1)),
            Some(Ok(num("2.50")))
        );

        assert_eq!(arith('%', &num("7"), &num("3")), Some(Ok(num("1"))));

        assert_eq!(arith('%', &SqlValue::Int(7), &SqlValue::Int(3)), None);
    }

    #[test]
    fn numeric_modulo_matches_pg() {
        assert_eq!(ar('%', "7", "3"), "1");
        assert_eq!(ar('%', "7.5", "2"), "1.5");
        assert_eq!(ar('%', "10.00", "3"), "1.00");
        assert_eq!(ar('%', "-7", "3"), "-1");
        assert_eq!(ar('%', "7", "-3"), "1");
        assert_eq!(ar('%', "5", "5"), "0");
        assert_eq!(
            arith('%', &num("7"), &num("0")),
            Some(Err(PgError::DivisionByZero))
        );
    }

    #[test]
    fn unary_negate_numeric() {
        assert_eq!(negate(&num("1.50")), Some(num("-1.50")));
        assert_eq!(negate(&num("-2.5")), Some(num("2.5")));
        assert_eq!(negate(&num("0.00")), Some(num("0.00")));
        assert_eq!(negate(&SqlValue::Text("abc".into())), None);
    }

    fn tm(p: i32, s: i32) -> i32 {
        crate::types::typmod::make_numeric(p, s)
    }
    fn apply(p: i32, s: i32, v: &str) -> Result<String, PgError> {
        match apply_typmod(tm(p, s), SqlValue::Text(v.into())) {
            Ok(SqlValue::Text(t)) => Ok(t),
            Ok(other) => panic!("expected Text, got {other:?}"),
            Err(e) => Err(e),
        }
    }

    #[test]
    fn typmod_rounds_to_scale() {
        assert_eq!(apply(6, 2, "12.345").unwrap(), "12.35");
        assert_eq!(apply(6, 2, "2.5").unwrap(), "2.50");
        assert_eq!(apply(4, 2, "12.345").unwrap(), "12.35");
        assert_eq!(apply(4, 2, "9.999").unwrap(), "10.00");
        assert_eq!(apply(4, 2, "0.005").unwrap(), "0.01");
        assert_eq!(apply(4, 2, "0.004").unwrap(), "0.00");
        assert_eq!(apply(10, 0, "3.5").unwrap(), "4");
        assert_eq!(apply(10, 0, "-3.5").unwrap(), "-4");
        assert_eq!(apply(6, 2, "-1.005").unwrap(), "-1.01");
    }

    #[test]
    fn typmod_precision_overflow() {

        assert_eq!(apply(4, 2, "123.4"), Err(PgError::NumericFieldOverflow));
        assert_eq!(apply(4, 2, "100"), Err(PgError::NumericFieldOverflow));

        assert_eq!(apply(4, 2, "99.99").unwrap(), "99.99");
        assert_eq!(apply(4, 2, "99.999"), Err(PgError::NumericFieldOverflow));
    }

    #[test]
    fn typmod_specials_pass_through() {
        assert_eq!(apply(4, 2, "NaN").unwrap(), "NaN");
        assert_eq!(apply(4, 2, "Infinity").unwrap(), "Infinity");
    }

    #[test]
    fn sort_key_equal_values_encode_equally() {
        let eq = sort_key(&SqlValue::Text("1.50".into()));
        assert_eq!(eq, sort_key(&SqlValue::Text("1.5".into())));
        assert_eq!(
            sort_key(&SqlValue::Text("0.00".into())),
            sort_key(&SqlValue::Text("0".into()))
        );
        assert_eq!(
            sort_key(&SqlValue::Text("0".into())),
            sort_key(&SqlValue::Text("-0".into()))
        );

        assert_eq!(sort_key(&SqlValue::Null), SqlValue::Null);
        assert_eq!(
            sort_key(&SqlValue::Text("hello".into())),
            SqlValue::Text("hello".into())
        );
    }
}
