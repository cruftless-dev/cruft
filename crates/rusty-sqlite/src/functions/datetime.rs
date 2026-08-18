
#![allow(unused_variables)]
use crate::{text_of, Value};

const UNIX_EPOCH_IJD: i64 = 210_866_760_000_000;
const MS_PER_DAY: i64 = 86_400_000;

pub fn call(name: &str, args: &[Value]) -> Option<Result<Value, String>> {
    match name {
        "DATE" | "TIME" | "DATETIME" | "JULIANDAY" | "UNIXEPOCH" | "STRFTIME" => {
            Some(Ok(run(name, args)))
        }
        "TIMEDIFF" => Some(Ok(timediff(args))),
        _ => None,
    }
}

fn timediff(args: &[Value]) -> Value {
    if args.len() != 2 {
        return Value::Null;
    }
    let (d1, d2ijd) = match (compute_ijd(&args[0..1]), compute_ijd(&args[1..2])) {
        (Some(a), Some(b)) => (a, b),
        _ => return Value::Null,
    };
    let d1ymd = ymd_hms(d1);
    let mut d2 = ymd_hms(d2ijd);
    let mut d2jd = d2ijd;
    let recompute = |d: &(i64, i64, i64, i64, i64, i64, i64)| -> i64 {
        compute_jd(
            d.0 as i32,
            d.1 as i32,
            d.2 as i32,
            d.3 as i32,
            d.4 as i32,
            d.5 as f64 + d.6 as f64 / 1000.0,
        )
    };

    const ANCHOR: i64 = 1_486_995_408 * 100_000;
    let (sign, y, m, residual);
    if d1 >= d2ijd {
        sign = '+';
        let mut yy = d1ymd.0 - d2.0;
        if yy != 0 {
            d2.0 = d1ymd.0;
            d2jd = recompute(&d2);
        }
        let mut mm = d1ymd.1 - d2.1;
        if mm < 0 {
            yy -= 1;
            mm += 12;
        }
        if mm != 0 {
            d2.1 = d1ymd.1;
            d2jd = recompute(&d2);
        }
        while d1 < d2jd {
            mm -= 1;
            if mm < 0 {
                mm = 11;
                yy -= 1;
            }
            d2.1 -= 1;
            if d2.1 < 1 {
                d2.1 = 12;
                d2.0 -= 1;
            }
            d2jd = recompute(&d2);
        }
        (y, m, residual) = (yy, mm, d1 - d2jd + ANCHOR);
    } else {
        sign = '-';
        let mut yy = d2.0 - d1ymd.0;
        if yy != 0 {
            d2.0 = d1ymd.0;
            d2jd = recompute(&d2);
        }
        let mut mm = d2.1 - d1ymd.1;
        if mm < 0 {
            yy -= 1;
            mm += 12;
        }
        if mm != 0 {
            d2.1 = d1ymd.1;
            d2jd = recompute(&d2);
        }
        while d1 > d2jd {
            mm -= 1;
            if mm < 0 {
                mm = 11;
                yy -= 1;
            }
            d2.1 += 1;
            if d2.1 > 12 {
                d2.1 = 1;
                d2.0 += 1;
            }
            d2jd = recompute(&d2);
        }
        (y, m, residual) = (yy, mm, d2jd - d1 + ANCHOR);
    }
    let (_, _, day, hour, min, sec, frac) = ymd_hms(residual);
    Value::Text(format!(
        "{}{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        sign,
        y,
        m,
        day - 1,
        hour,
        min,
        sec,
        frac
    ))
}

fn run(name: &str, args: &[Value]) -> Value {

    let (fmt, tv_args): (Option<String>, &[Value]) = if name == "STRFTIME" {
        if args.is_empty() || matches!(args[0], Value::Null) {
            return Value::Null;
        }
        (Some(text_of(&args[0])), &args[1..])
    } else {
        (None, args)
    };
    if tv_args.is_empty() {
        return Value::Null;
    }
    let ijd = match compute_ijd(tv_args) {
        Some(x) => x,
        None => return Value::Null,
    };
    match name {
        "JULIANDAY" => Value::Real(ijd as f64 / MS_PER_DAY as f64),
        "UNIXEPOCH" => Value::Int((ijd - UNIX_EPOCH_IJD).div_euclid(1000)),
        "DATE" => {
            let (y, mo, d, ..) = ymd_hms(ijd);
            Value::Text(format!("{:04}-{:02}-{:02}", y, mo, d))
        }
        "TIME" => {
            let (_, _, _, h, mi, s, _) = ymd_hms(ijd);
            Value::Text(format!("{:02}:{:02}:{:02}", h, mi, s))
        }
        "DATETIME" => {
            let (y, mo, d, h, mi, s, _) = ymd_hms(ijd);
            Value::Text(format!(
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                y, mo, d, h, mi, s
            ))
        }
        "STRFTIME" => match strftime(&fmt.unwrap(), ijd) {
            Some(s) => Value::Text(s),
            None => Value::Null,
        },
        _ => Value::Null,
    }
}

fn jd_to_ijd(num: f64) -> Option<i64> {
    if (0.0..=5_373_484.5).contains(&num) {

        Some((num * MS_PER_DAY as f64 + 0.5) as i64)
    } else {
        None
    }
}

fn now_ijd() -> Option<i64> {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    Some(UNIX_EPOCH_IJD + dur.as_millis() as i64)
}

fn compute_ijd(tv_args: &[Value]) -> Option<i64> {
    let first = &tv_args[0];
    let mods = &tv_args[1..];
    let has_unix = mods
        .iter()
        .any(|m| text_of(m).trim().eq_ignore_ascii_case("unixepoch"));

    let mut ijd: i64 = match first {
        Value::Null => return None,
        Value::Int(i) => {
            if has_unix {
                i.checked_mul(1000)?.checked_add(UNIX_EPOCH_IJD)?
            } else {
                jd_to_ijd(*i as f64)?
            }
        }
        Value::Real(r) => {
            if has_unix {
                (r * 1000.0).round() as i64 + UNIX_EPOCH_IJD
            } else {
                jd_to_ijd(*r)?
            }
        }
        _ => {
            let s = text_of(first);
            let st = s.trim();
            if st.eq_ignore_ascii_case("now") {

                now_ijd()?
            } else if let Ok(num) = st.parse::<f64>() {

                if has_unix {
                    (num * 1000.0).round() as i64 + UNIX_EPOCH_IJD
                } else {
                    jd_to_ijd(num)?
                }
            } else {
                let (y, mo, d, h, mi, sec) = parse_datetime(&s)?;
                compute_jd(y, mo, d, h, mi, sec)
            }
        }
    };

    for m in mods {
        apply_modifier(&mut ijd, &text_of(m))?;
    }
    Some(ijd)
}

fn parse_datetime(s: &str) -> Option<(i32, i32, i32, i32, i32, f64)> {

    let (neg, s) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    let b = s.as_bytes();

    if b.len() >= 10
        && b[4] == b'-'
        && b[7] == b'-'
        && all_digits(&b[0..4])
        && all_digits(&b[5..7])
        && all_digits(&b[8..10])
    {
        let y = int_of(&s[0..4])? * if neg { -1 } else { 1 };
        let mo = int_of(&s[5..7])?;
        let d = int_of(&s[8..10])?;
        let rest = &s[10..];
        if rest.is_empty() {
            return Some((y, mo, d, 0, 0, 0.0));
        }
        let sep = rest.as_bytes()[0];
        if sep == b' ' || sep == b'T' || sep == b't' {
            let (h, mi, sec) = parse_time(&rest[1..])?;
            return Some((y, mo, d, h, mi, sec));
        }
        return None;
    }

    let (h, mi, sec) = parse_time(s)?;
    Some((2000, 1, 1, h, mi, sec))
}

fn parse_time(s: &str) -> Option<(i32, i32, f64)> {
    let b = s.as_bytes();
    if b.len() < 5 || b[2] != b':' || !all_digits(&b[0..2]) || !all_digits(&b[3..5]) {
        return None;
    }
    let h = int_of(&s[0..2])?;
    let mi = int_of(&s[3..5])?;
    if b.len() == 5 {
        return Some((h, mi, 0.0));
    }
    if b[5] != b':' {
        return None;
    }
    let sec_str = &s[6..];

    if sec_str.len() < 2 || !all_digits(&sec_str.as_bytes()[0..2]) {
        return None;
    }
    let sec: f64 = sec_str.parse().ok()?;
    Some((h, mi, sec))
}

fn all_digits(b: &[u8]) -> bool {
    !b.is_empty() && b.iter().all(|c| c.is_ascii_digit())
}

fn int_of(s: &str) -> Option<i32> {
    s.parse().ok()
}

fn compute_jd(y: i32, mo: i32, d: i32, h: i32, mi: i32, s: f64) -> i64 {
    let (mut y, mut m) = (y as i64, mo as i64);
    if m <= 2 {
        y -= 1;
        m += 12;
    }

    let a = (y + 4800) / 100;
    let b = 38 - a + a / 4;
    let x1 = 36525 * (y + 4716) / 100;
    let x2 = 306001 * (m + 1) / 10000;
    (((x1 + x2 + d as i64 + b) as f64 - 1524.5) * MS_PER_DAY as f64) as i64
        + h as i64 * 3_600_000
        + mi as i64 * 60_000
        + (s * 1000.0 + 0.5) as i64
}

fn ymd_hms(ijd: i64) -> (i64, i64, i64, i64, i64, i64, i64) {
    let z = (ijd + 43_200_000).div_euclid(MS_PER_DAY);
    let mut a = ((z as f64 - 1_867_216.25) / 36524.25) as i64;
    a = z + 1 + a - a / 4;
    let b = a + 1524;
    let c = ((b as f64 - 122.1) / 365.25) as i64;
    let dd = 36525 * (c & 32767) / 100;
    let e = ((b - dd) as f64 / 30.6001) as i64;
    let x1 = (30.6001 * e as f64) as i64;
    let day = b - dd - x1;
    let month = if e < 14 { e - 1 } else { e - 13 };
    let year = if month > 2 { c - 4716 } else { c - 4715 };

    let sms = (ijd + 43_200_000).rem_euclid(MS_PER_DAY);
    let h = sms / 3_600_000;
    let mi = (sms % 3_600_000) / 60_000;
    let sec = (sms % 60_000) / 1000;
    let frac = sms % 1000;
    (year, month, day, h, mi, sec, frac)
}

fn apply_modifier(ijd: &mut i64, raw: &str) -> Option<()> {
    let s = raw.trim().to_lowercase();
    if s == "unixepoch" || s == "utc" || s == "localtime" {
        return Some(());
    }
    if s == "start of day" || s == "start of month" || s == "start of year" {
        let (y, mo, d, ..) = ymd_hms(*ijd);
        let (ny, nm, nd) = if s.ends_with("day") {
            (y, mo, d)
        } else if s.ends_with("month") {
            (y, mo, 1)
        } else {
            (y, 1, 1)
        };
        *ijd = compute_jd(ny as i32, nm as i32, nd as i32, 0, 0, 0.0);
        return Some(());
    }
    if let Some(rest) = s.strip_prefix("weekday ") {
        let n: i64 = rest.trim().parse().ok()?;
        if !(0..=6).contains(&n) {
            return None;
        }
        let w = (*ijd + 129_600_000).div_euclid(MS_PER_DAY).rem_euclid(7);
        let delta = (n - w).rem_euclid(7);
        *ijd += delta * MS_PER_DAY;
        return Some(());
    }

    let mut it = s.split_whitespace();
    let num = it.next()?;
    let unit = it.next()?;
    if it.next().is_some() {
        return None;
    }
    let val: f64 = num.parse().ok()?;
    let unit = unit.strip_suffix('s').unwrap_or(unit);
    match unit {
        "day" => *ijd += (val * MS_PER_DAY as f64).round() as i64,
        "hour" => *ijd += (val * 3_600_000.0).round() as i64,
        "minute" => *ijd += (val * 60_000.0).round() as i64,
        "second" => *ijd += (val * 1000.0).round() as i64,
        "month" | "year" => {
            let (y, mo, d, h, mi, sec, frac) = ymd_hms(*ijd);
            let (mut y, mut m) = (y, mo);
            if unit == "year" {
                y += val as i64;
            } else {
                m += val as i64;
                let x = if m > 0 { (m - 1) / 12 } else { (m - 12) / 12 };
                y += x;
                m -= x * 12;
            }
            let secf = sec as f64 + frac as f64 / 1000.0;
            *ijd = compute_jd(y as i32, m as i32, d as i32, h as i32, mi as i32, secf);
        }
        _ => return None,
    }
    Some(())
}

fn strftime(fmt: &str, ijd: i64) -> Option<String> {
    let (y, mo, d, h, mi, sec, frac) = ymd_hms(ijd);
    let mut out = String::new();
    let ch: Vec<char> = fmt.chars().collect();
    let mut i = 0;
    while i < ch.len() {
        if ch[i] == '%' && i + 1 < ch.len() {
            match ch[i + 1] {
                'Y' => out.push_str(&format!("{:04}", y)),
                'm' => out.push_str(&format!("{:02}", mo)),
                'd' => out.push_str(&format!("{:02}", d)),
                'H' => out.push_str(&format!("{:02}", h)),
                'M' => out.push_str(&format!("{:02}", mi)),
                'S' => out.push_str(&format!("{:02}", sec)),
                'j' => out.push_str(&format!("{:03}", day_of_year(y, mo, d))),
                'w' => {
                    let w = (ijd + 129_600_000).div_euclid(MS_PER_DAY).rem_euclid(7);
                    out.push_str(&w.to_string());
                }

                'W' => {
                    let n_day = day_of_year(y, mo, d) - 1;
                    let wd = (ijd + 43_200_000).div_euclid(MS_PER_DAY).rem_euclid(7);
                    out.push_str(&format!("{:02}", (n_day + 7 - wd) / 7));
                }
                's' => out.push_str(&(ijd - UNIX_EPOCH_IJD).div_euclid(1000).to_string()),
                'f' => {
                    let secf = sec as f64 + frac as f64 / 1000.0;
                    out.push_str(&format!("{:06.3}", secf));
                }
                'J' => out.push_str(&fmt_g(ijd as f64 / MS_PER_DAY as f64)),
                '%' => out.push('%'),
                _ => return None,
            }
            i += 2;
        } else {
            out.push(ch[i]);
            i += 1;
        }
    }
    Some(out)
}

fn day_of_year(y: i64, mo: i64, d: i64) -> i64 {
    const CUM: [i64; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let mut doy = CUM[(mo - 1).clamp(0, 11) as usize] + d;
    if mo > 2 && leap {
        doy += 1;
    }
    doy
}

fn fmt_g(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}
