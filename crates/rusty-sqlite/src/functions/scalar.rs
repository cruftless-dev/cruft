
#![allow(unused_variables)]
use crate::{coerce, text_of, Affinity, Value};

fn err(msg: String) -> Option<Result<Value, String>> {
    Some(Err(msg))
}
fn ok(v: Value) -> Option<Result<Value, String>> {
    Some(Ok(v))
}

fn num(v: &Value) -> Option<f64> {
    match v {
        Value::Int(i) => Some(*i as f64),
        Value::Real(r) => Some(*r),
        Value::Text(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn num_affinity(v: &Value) -> Option<f64> {
    match v {
        Value::Int(i) => Some(*i as f64),
        Value::Real(r) => Some(*r),
        Value::Text(s) => Some(crate::numeric_prefix(s)),
        _ => None,
    }
}

pub fn call(name: &str, args: &[Value]) -> Option<Result<Value, String>> {
    match name {
        "SUBSTR" | "SUBSTRING" => substr(args),
        "REPLACE" => replace(args),
        "TRIM" => trim_fn(args, true, true),
        "LTRIM" => trim_fn(args, true, false),
        "RTRIM" => trim_fn(args, false, true),
        "INSTR" => instr(args),
        "ROUND" => round(args),
        "ABS" => abs(args),
        "HEX" => hex(args),
        "QUOTE" => quote(args),
        "CHAR" => char_fn(args),
        "UNICODE" => unicode(args),
        "SIGN" => sign(args),
        "NULLIF" => nullif(args),
        "IFNULL" => ifnull(args),
        "COALESCE" => coalesce(args),
        "LENGTH" => length(args),
        "UPPER" => upper(args, true),
        "LOWER" => upper(args, false),
        "TYPEOF" => typeof_fn(args),
        "PRINTF" | "FORMAT" => printf(args),
        "CAST" => cast(args),

        "POWER" | "POW" => math2(args, |a, b| a.powf(b)),
        "SQRT" => math1(args, f64::sqrt),
        "EXP" => math1(args, f64::exp),
        "LN" => math1(args, f64::ln),
        "LOG10" => math1(args, f64::log10),
        "LOG2" => math1(args, f64::log2),
        "LOG" => log_fn(args),
        "MOD" => mod_fn(args),
        "FLOOR" => math1(args, f64::floor),
        "CEIL" | "CEILING" => math1(args, f64::ceil),
        "TRUNC" => math1(args, f64::trunc),
        "PI" => ok(Value::Real(std::f64::consts::PI)),
        "SIN" => math1(args, f64::sin),
        "COS" => math1(args, f64::cos),
        "TAN" => math1(args, f64::tan),
        "ASIN" => math1(args, f64::asin),
        "ACOS" => math1(args, f64::acos),
        "ATAN" => math1(args, f64::atan),
        "ATAN2" => math2(args, |a, b| a.atan2(b)),
        "SINH" => math1(args, f64::sinh),
        "COSH" => math1(args, f64::cosh),
        "TANH" => math1(args, f64::tanh),
        "ASINH" => math1(args, f64::asinh),
        "ACOSH" => math1(args, f64::acosh),
        "ATANH" => math1(args, f64::atanh),
        "DEGREES" => math1(args, f64::to_degrees),
        "RADIANS" => math1(args, f64::to_radians),
        "CONCAT" => concat(args),
        "CONCAT_WS" => concat_ws(args),
        "IIF" => iif(args),
        "UNHEX" => unhex(args),
        "ZEROBLOB" => zeroblob(args),
        "SQLITE_VERSION" => ok(Value::Text("3.45.0".into())),

        "LIKELIHOOD" | "LIKELY" | "UNLIKELY" => ok(args.first().cloned().unwrap_or(Value::Null)),
        _ => None,
    }
}

fn math1(args: &[Value], f: impl Fn(f64) -> f64) -> Option<Result<Value, String>> {
    match args.first() {
        Some(Value::Null) | None => ok(Value::Null),
        Some(v) => match num(v) {
            Some(x) => ok(Value::Real(f(x))),
            None => ok(Value::Null),
        },
    }
}

fn math2(args: &[Value], f: impl Fn(f64, f64) -> f64) -> Option<Result<Value, String>> {
    if args.len() < 2 || matches!(args[0], Value::Null) || matches!(args[1], Value::Null) {
        return ok(Value::Null);
    }
    match (num(&args[0]), num(&args[1])) {
        (Some(a), Some(b)) => ok(Value::Real(f(a, b))),
        _ => ok(Value::Null),
    }
}

fn log_fn(args: &[Value]) -> Option<Result<Value, String>> {
    match args.len() {
        1 => math1(args, f64::log10),
        _ => match (num(&args[0]), num(&args[1])) {
            (Some(b), Some(x))
                if !matches!(args[0], Value::Null) && !matches!(args[1], Value::Null) =>
            {
                ok(Value::Real(x.ln() / b.ln()))
            }
            _ => ok(Value::Null),
        },
    }
}

fn mod_fn(args: &[Value]) -> Option<Result<Value, String>> {
    if args.len() < 2 || matches!(args[0], Value::Null) || matches!(args[1], Value::Null) {
        return ok(Value::Null);
    }
    match (num(&args[0]), num(&args[1])) {
        (Some(_), Some(0.0)) => ok(Value::Null),
        (Some(a), Some(b)) => ok(Value::Real(a % b)),
        _ => ok(Value::Null),
    }
}

fn concat(args: &[Value]) -> Option<Result<Value, String>> {
    let s: String = args
        .iter()
        .map(|v| {
            if matches!(v, Value::Null) {
                String::new()
            } else {
                text_of(v)
            }
        })
        .collect();
    ok(Value::Text(s))
}

fn concat_ws(args: &[Value]) -> Option<Result<Value, String>> {
    match args.first() {
        None => err("wrong number of arguments to function concat_ws()".into()),
        Some(Value::Null) => ok(Value::Null),
        Some(sep_v) => {
            let sep = text_of(sep_v);
            let parts: Vec<String> = args[1..]
                .iter()
                .filter(|v| !matches!(v, Value::Null))
                .map(text_of)
                .collect();
            ok(Value::Text(parts.join(&sep)))
        }
    }
}

fn iif(args: &[Value]) -> Option<Result<Value, String>> {
    if args.len() < 3 {
        return err("wrong number of arguments to function iif()".into());
    }
    if args[0].truthy() {
        ok(args[1].clone())
    } else {
        ok(args[2].clone())
    }
}

fn unhex(args: &[Value]) -> Option<Result<Value, String>> {
    match args.first() {
        Some(Value::Null) | None => ok(Value::Null),
        Some(v) => {
            let cs: Vec<char> = text_of(v).chars().collect();
            if cs.len() % 2 != 0 {
                return ok(Value::Null);
            }
            let mut bytes = Vec::with_capacity(cs.len() / 2);
            let mut i = 0;
            while i < cs.len() {
                match (cs[i].to_digit(16), cs[i + 1].to_digit(16)) {
                    (Some(h), Some(l)) => bytes.push((h * 16 + l) as u8),
                    _ => return ok(Value::Null),
                }
                i += 2;
            }
            ok(Value::Blob(bytes))
        }
    }
}

fn zeroblob(args: &[Value]) -> Option<Result<Value, String>> {
    match args.first().and_then(num) {
        Some(n) if n >= 0.0 => ok(Value::Blob(vec![0u8; n as usize])),
        _ => ok(Value::Blob(Vec::new())),
    }
}

fn substr(args: &[Value]) -> Option<Result<Value, String>> {
    if args.len() != 2 && args.len() != 3 {
        return err("wrong number of arguments to function substr()".into());
    }
    if matches!(args[0], Value::Null) {
        return ok(Value::Null);
    }
    let s: Vec<char> = text_of(&args[0]).chars().collect();
    let n = s.len() as i64;
    let start = match num(&args[1]) {
        Some(f) => f as i64,
        None => return ok(Value::Null),
    };
    let has_len = args.len() == 3;
    if has_len && matches!(args[2], Value::Null) {
        return ok(Value::Null);
    }
    let mut y = start;
    if y < 0 {
        y = n + y + 1;
    } else if y == 0 {
        y = 1;
    }

    let mut z: i64 = if has_len {
        num(&args[2]).map(|f| f as i64).unwrap_or(0)
    } else {
        n - y + 1
    };
    if z < 0 {

        y += z;
        z = -z;
    }
    if y < 1 {
        z += y - 1;
        y = 1;
    }
    if z < 0 {
        z = 0;
    }
    let start_idx = (y - 1).max(0) as usize;
    let out: String = s.iter().skip(start_idx).take(z as usize).collect();
    ok(Value::Text(out))
}

fn replace(args: &[Value]) -> Option<Result<Value, String>> {
    if args.len() != 3 {
        return err("wrong number of arguments to function replace()".into());
    }
    if args.iter().any(|a| matches!(a, Value::Null)) {
        return ok(Value::Null);
    }
    let s = text_of(&args[0]);
    let from = text_of(&args[1]);
    let to = text_of(&args[2]);
    if from.is_empty() {
        return ok(Value::Text(s));
    }
    ok(Value::Text(s.replace(&from, &to)))
}

fn trim_fn(args: &[Value], left: bool, right: bool) -> Option<Result<Value, String>> {
    if args.is_empty() || args.len() > 2 {
        return err("wrong number of arguments to function trim()".into());
    }
    if matches!(args[0], Value::Null) {
        return ok(Value::Null);
    }
    let s = text_of(&args[0]);
    let set: Vec<char> = if args.len() == 2 {
        if matches!(args[1], Value::Null) {
            return ok(Value::Null);
        }
        text_of(&args[1]).chars().collect()
    } else {
        vec![' ', '\t', '\n', '\r', '\x0b', '\x0c']
    };
    let chars: Vec<char> = s.chars().collect();
    let mut lo = 0usize;
    let mut hi = chars.len();
    if left {
        while lo < hi && set.contains(&chars[lo]) {
            lo += 1;
        }
    }
    if right {
        while hi > lo && set.contains(&chars[hi - 1]) {
            hi -= 1;
        }
    }
    ok(Value::Text(chars[lo..hi].iter().collect()))
}

fn instr(args: &[Value]) -> Option<Result<Value, String>> {
    if args.len() != 2 {
        return err("wrong number of arguments to function instr()".into());
    }
    if args.iter().any(|a| matches!(a, Value::Null)) {
        return ok(Value::Null);
    }
    let hay: Vec<char> = text_of(&args[0]).chars().collect();
    let needle: Vec<char> = text_of(&args[1]).chars().collect();
    if needle.is_empty() {
        return ok(Value::Int(1));
    }
    if needle.len() > hay.len() {
        return ok(Value::Int(0));
    }
    for i in 0..=(hay.len() - needle.len()) {
        if hay[i..i + needle.len()] == needle[..] {
            return ok(Value::Int(i as i64 + 1));
        }
    }
    ok(Value::Int(0))
}

fn round(args: &[Value]) -> Option<Result<Value, String>> {
    if args.is_empty() || args.len() > 2 {
        return err("wrong number of arguments to function round()".into());
    }
    if matches!(args[0], Value::Null) {
        return ok(Value::Null);
    }
    let x = match num_affinity(&args[0]) {
        Some(f) => f,
        None => return ok(Value::Real(0.0)),
    };
    let digits: i64 = if args.len() == 2 {
        if matches!(args[1], Value::Null) {
            return ok(Value::Null);
        }
        num(&args[1]).map(|f| f as i64).unwrap_or(0)
    } else {
        0
    };
    let d = digits.max(0) as usize;
    ok(Value::Real(round_half_away(x, d)))
}

fn round_half_away(x: f64, d: usize) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let neg = x.is_sign_negative();
    let ax = x.abs();

    let s = format!("{:.*}", d + 25, ax);
    let dot = s.find('.').unwrap();

    let mut digits: Vec<u8> = s.bytes().filter(|&b| b != b'.').map(|b| b - b'0').collect();
    let keep = dot + d;
    let round_up = digits.get(keep).is_some_and(|&g| g >= 5);
    digits.truncate(keep);
    if round_up {

        let mut i = keep;
        loop {
            if i == 0 {
                digits.insert(0, 1);
                break;
            }
            i -= 1;
            if digits[i] == 9 {
                digits[i] = 0;
            } else {
                digits[i] += 1;
                break;
            }
        }
    }

    let frac_start = digits.len() - d;
    let int_str: String = digits[..frac_start]
        .iter()
        .map(|n| (n + b'0') as char)
        .collect();
    let frac_str: String = digits[frac_start..]
        .iter()
        .map(|n| (n + b'0') as char)
        .collect();
    let joined = if d == 0 {
        int_str
    } else {
        format!("{int_str}.{frac_str}")
    };
    let mut r: f64 = joined.parse().unwrap_or(0.0);
    if neg {
        r = -r;
    }
    r
}

fn abs(args: &[Value]) -> Option<Result<Value, String>> {
    if args.len() != 1 {
        return err("wrong number of arguments to function abs()".into());
    }
    match &args[0] {
        Value::Null => ok(Value::Null),
        Value::Int(i) => ok(Value::Int(i.abs())),
        v => match num_affinity(v) {
            Some(f) => ok(Value::Real(f.abs())),
            None => ok(Value::Real(0.0)),
        },
    }
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02X}", b));
    }
    s
}

fn hex(args: &[Value]) -> Option<Result<Value, String>> {
    if args.len() != 1 {
        return err("wrong number of arguments to function hex()".into());
    }
    match &args[0] {
        Value::Null => ok(Value::Text(String::new())),
        Value::Blob(b) => ok(Value::Text(to_hex(b))),
        v => ok(Value::Text(to_hex(text_of(v).as_bytes()))),
    }
}

fn quote(args: &[Value]) -> Option<Result<Value, String>> {
    if args.len() != 1 {
        return err("wrong number of arguments to function quote()".into());
    }
    let out = match &args[0] {
        Value::Null => "NULL".to_string(),
        Value::Int(i) => i.to_string(),
        Value::Real(_) => text_of(&args[0]),
        Value::Text(s) => format!("'{}'", s.replace('\'', "''")),
        Value::Blob(b) => format!("X'{}'", to_hex(b)),
    };
    ok(Value::Text(out))
}

fn char_fn(args: &[Value]) -> Option<Result<Value, String>> {
    let mut s = String::new();
    for a in args {
        let cp = num(a).map(|f| f as u32).unwrap_or(0);
        if let Some(c) = char::from_u32(cp) {
            s.push(c);
        }
    }
    ok(Value::Text(s))
}

fn unicode(args: &[Value]) -> Option<Result<Value, String>> {
    if args.len() != 1 {
        return err("wrong number of arguments to function unicode()".into());
    }
    if matches!(args[0], Value::Null) {
        return ok(Value::Null);
    }
    match text_of(&args[0]).chars().next() {
        Some(c) => ok(Value::Int(c as i64)),
        None => ok(Value::Null),
    }
}

fn sign(args: &[Value]) -> Option<Result<Value, String>> {
    if args.len() != 1 {
        return err("wrong number of arguments to function sign()".into());
    }
    match &args[0] {
        Value::Int(_) | Value::Real(_) => {
            let f = num(&args[0]).unwrap();
            ok(Value::Int(if f > 0.0 {
                1
            } else if f < 0.0 {
                -1
            } else {
                0
            }))
        }
        _ => ok(Value::Null),
    }
}

fn nullif(args: &[Value]) -> Option<Result<Value, String>> {
    if args.len() != 2 {
        return err("wrong number of arguments to function nullif()".into());
    }
    if args[0].compare(&args[1]) == std::cmp::Ordering::Equal {
        ok(Value::Null)
    } else {
        ok(args[0].clone())
    }
}

fn ifnull(args: &[Value]) -> Option<Result<Value, String>> {
    if args.len() != 2 {
        return err("wrong number of arguments to function ifnull()".into());
    }
    coalesce(args)
}

fn coalesce(args: &[Value]) -> Option<Result<Value, String>> {
    if args.len() < 2 {
        return err("wrong number of arguments to function coalesce()".into());
    }
    ok(args
        .iter()
        .find(|v| !matches!(v, Value::Null))
        .cloned()
        .unwrap_or(Value::Null))
}

fn length(args: &[Value]) -> Option<Result<Value, String>> {
    if args.len() != 1 {
        return err("wrong number of arguments to function length()".into());
    }
    match &args[0] {
        Value::Null => ok(Value::Null),
        Value::Blob(b) => ok(Value::Int(b.len() as i64)),
        v => ok(Value::Int(text_of(v).chars().count() as i64)),
    }
}

fn upper(args: &[Value], up: bool) -> Option<Result<Value, String>> {
    if args.len() != 1 {
        return err("wrong number of arguments".into());
    }
    match &args[0] {
        Value::Null => ok(Value::Null),
        v => {
            let s = text_of(v);
            ok(Value::Text(if up {
                s.to_uppercase()
            } else {
                s.to_lowercase()
            }))
        }
    }
}

fn typeof_fn(args: &[Value]) -> Option<Result<Value, String>> {
    if args.len() != 1 {
        return err("wrong number of arguments to function typeof()".into());
    }
    ok(Value::Text(
        match &args[0] {
            Value::Null => "null",
            Value::Int(_) => "integer",
            Value::Real(_) => "real",
            Value::Text(_) => "text",
            Value::Blob(_) => "blob",
        }
        .into(),
    ))
}

fn cast(args: &[Value]) -> Option<Result<Value, String>> {
    if args.len() != 2 {
        return err("wrong number of arguments to CAST".into());
    }
    let ty = text_of(&args[1]).to_ascii_uppercase();
    let aff = if ty.contains("INT") {
        Affinity::Integer
    } else if ty.contains("REAL") || ty.contains("FLOA") || ty.contains("DOUB") {
        Affinity::Real
    } else if ty.contains("TEXT") || ty.contains("CHAR") || ty.contains("CLOB") {
        Affinity::Text
    } else if ty.contains("BLOB") || ty.is_empty() {
        Affinity::Blob
    } else {
        Affinity::Numeric
    };

    let v = args[0].clone();
    let out = match aff {
        Affinity::Integer => match &v {
            Value::Null => Value::Null,
            Value::Int(_) => v,
            Value::Real(r) => Value::Int(*r as i64),
            other => Value::Int(parse_leading_real(&text_of(other)) as i64),
        },
        Affinity::Real => match &v {
            Value::Null => Value::Null,
            Value::Real(_) => v,
            Value::Int(i) => Value::Real(*i as f64),
            other => Value::Real(parse_leading_real(&text_of(other))),
        },
        _ => coerce(v, aff),
    };
    ok(out)
}

fn parse_leading_real(s: &str) -> f64 {
    let t = s.trim_start();
    let bytes = t.as_bytes();
    let mut i = 0;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    let mut seen_dot = false;
    let mut seen_exp = false;
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_digit() {
            i += 1;
        } else if c == b'.' && !seen_dot && !seen_exp {
            seen_dot = true;
            i += 1;
        } else if (c == b'e' || c == b'E') && !seen_exp && i > 0 {
            seen_exp = true;
            i += 1;
            if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
                i += 1;
            }
        } else {
            break;
        }
    }
    t[..i].parse::<f64>().unwrap_or(0.0)
}

fn printf(args: &[Value]) -> Option<Result<Value, String>> {
    if args.is_empty() {
        return ok(Value::Null);
    }
    if matches!(args[0], Value::Null) {
        return ok(Value::Null);
    }
    let fmt: Vec<char> = text_of(&args[0]).chars().collect();
    let mut out = String::new();
    let mut ai = 1usize;
    let mut i = 0usize;
    while i < fmt.len() {
        if fmt[i] != '%' {
            out.push(fmt[i]);
            i += 1;
            continue;
        }
        i += 1;
        if i >= fmt.len() {
            break;
        }
        if fmt[i] == '%' {
            out.push('%');
            i += 1;
            continue;
        }

        let mut left = false;
        let mut zero = false;
        let mut plus = false;
        let mut space = false;
        let mut alt = false;
        loop {
            match fmt.get(i) {
                Some('-') => left = true,
                Some('0') => zero = true,
                Some('+') => plus = true,
                Some(' ') => space = true,
                Some('#') => alt = true,
                Some('!') => {}
                _ => break,
            }
            i += 1;
        }

        let mut width = 0usize;
        let mut has_width = false;
        while let Some(c) = fmt.get(i) {
            if c.is_ascii_digit() {
                width = width * 10 + (*c as usize - '0' as usize);
                has_width = true;
                i += 1;
            } else {
                break;
            }
        }

        let mut prec: Option<usize> = None;
        if fmt.get(i) == Some(&'.') {
            i += 1;
            let mut p = 0usize;
            while let Some(c) = fmt.get(i) {
                if c.is_ascii_digit() {
                    p = p * 10 + (*c as usize - '0' as usize);
                    i += 1;
                } else {
                    break;
                }
            }
            prec = Some(p);
        }

        while matches!(fmt.get(i), Some('l') | Some('h') | Some('L')) {
            i += 1;
        }
        let conv = match fmt.get(i) {
            Some(c) => *c,
            None => break,
        };
        i += 1;
        let arg = args.get(ai);
        let mut body: String;
        let mut is_num_signed = false;
        let mut negative = false;
        match conv {
            'd' | 'i' | 'u' => {
                ai += 1;
                let n = arg.and_then(num).map(|f| f as i64).unwrap_or(0);
                is_num_signed = true;
                negative = n < 0;
                body = n.unsigned_abs().to_string();
            }
            'x' | 'X' => {
                ai += 1;
                let n = arg.and_then(num).map(|f| f as i64).unwrap_or(0) as u64;
                body = if conv == 'x' {
                    format!("{:x}", n)
                } else {
                    format!("{:X}", n)
                };
                if alt && n != 0 {
                    body = format!("0{}{}", if conv == 'x' { "x" } else { "X" }, body);
                }
            }
            'o' => {
                ai += 1;
                let n = arg.and_then(num).map(|f| f as i64).unwrap_or(0) as u64;
                body = format!("{:o}", n);
            }
            'f' | 'F' | 'e' | 'E' | 'g' | 'G' => {
                ai += 1;
                let f = arg.and_then(num).unwrap_or(0.0);
                is_num_signed = true;
                negative = f.is_sign_negative() && f != 0.0;
                let p = prec.unwrap_or(6);
                let mag = f.abs();
                body = match conv {
                    'e' => format!("{:.*e}", p, mag),
                    'E' => format!("{:.*E}", p, mag),
                    'g' | 'G' => format!("{}", mag),
                    _ => format!("{:.*}", p, mag),
                };
            }
            's' | 'z' => {

                ai += 1;
                let mut s = arg.map(text_of).unwrap_or_default();
                if let Some(p) = prec {
                    s = s.chars().take(p).collect();
                }
                body = s;
            }

            'q' => {

                ai += 1;
                body = match arg {
                    Some(Value::Null) | None => "(NULL)".to_string(),
                    Some(v) => text_of(v).replace('\'', "''"),
                };
            }
            'Q' => {

                ai += 1;
                body = match arg {
                    Some(Value::Null) | None => "NULL".to_string(),
                    Some(v) => format!("'{}'", text_of(v).replace('\'', "''")),
                };
            }
            'w' => {

                ai += 1;
                body = match arg {
                    Some(Value::Null) | None => "(NULL)".to_string(),
                    Some(v) => text_of(v).replace('"', "\"\""),
                };
            }
            'c' => {
                ai += 1;
                let cp = arg.and_then(num).map(|f| f as u32).unwrap_or(0);
                body = char::from_u32(cp)
                    .map(|c| c.to_string())
                    .unwrap_or_default();
            }
            other => {
                out.push('%');
                out.push(other);
                continue;
            }
        }

        if matches!(conv, 'd' | 'i' | 'u' | 'x' | 'X' | 'o') {
            if let Some(p) = prec {
                while body.len() < p {
                    body.insert(0, '0');
                }
            }
        }
        let mut sign = String::new();
        if is_num_signed {
            if negative {
                sign.push('-');
            } else if plus {
                sign.push('+');
            } else if space {
                sign.push(' ');
            }
        }
        let content_len = sign.len() + body.len();
        if has_width && content_len < width {
            let pad = width - content_len;
            if left {
                out.push_str(&sign);
                out.push_str(&body);
                out.extend(std::iter::repeat(' ').take(pad));
            } else if zero && (prec.is_none() || matches!(conv, 'f' | 'F' | 'e' | 'E' | 'g' | 'G'))
            {

                out.push_str(&sign);
                out.extend(std::iter::repeat('0').take(pad));
                out.push_str(&body);
            } else {
                out.extend(std::iter::repeat(' ').take(pad));
                out.push_str(&sign);
                out.push_str(&body);
            }
        } else {
            out.push_str(&sign);
            out.push_str(&body);
        }
    }
    ok(Value::Text(out))
}
