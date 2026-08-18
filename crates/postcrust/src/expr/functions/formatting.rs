
use crate::types::{self, oid, PgError};
use sql_core::SqlValue;

fn does_not_exist(name: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("function {name}(...) does not exist"),
    }
}

fn any_null(args: &[SqlValue]) -> bool {
    args.iter().any(|a| matches!(a, SqlValue::Null))
}

fn as_text(v: &SqlValue) -> Option<String> {
    match v {
        SqlValue::Text(s) => Some(s.clone()),
        _ => None,
    }
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let m = m as i64;
    let d = d as i64;
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

const MONTH_FULL: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const MONTH_ABBR: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const DAY_FULL: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];
const DAY_ABBR: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

struct Parts {
    year: i64,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    micros: u32,
}

fn digits_i64(s: &str) -> Option<i64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}
fn digits_u32(s: &str) -> Option<u32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn parse_datetime(s: &str) -> Option<Parts> {
    let s = s.trim();
    let (date, time): (&str, Option<&str>) = if let Some((d, t)) = s.split_once('T') {
        (d, Some(t))
    } else if let Some((d, t)) = s.split_once(' ') {
        (d, Some(t.trim_start()))
    } else {
        (s, None)
    };
    let mut dit = date.splitn(3, '-');
    let (y, mo, d) = (dit.next()?, dit.next()?, dit.next()?);
    if dit.next().is_some() {
        return None;
    }
    let year = digits_i64(y)?;
    let month = digits_u32(mo)?;
    let day = digits_u32(d)?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let (hour, minute, second, micros) = match time {
        None => (0, 0, 0, 0),
        Some(t) => {
            let (hms, frac) = match t.split_once('.') {
                Some((h, f)) => (h, Some(f)),
                None => (t, None),
            };
            let mut tit = hms.split(':');
            let h = tit.next()?;
            let mi = tit.next()?;
            let se = tit.next();
            if tit.next().is_some() {
                return None;
            }
            let hour = digits_u32(h)?;
            let minute = digits_u32(mi)?;
            let second = match se {
                Some(ss) => digits_u32(ss)?,
                None => 0,
            };
            let micros = match frac {
                None => 0,
                Some(f) => {
                    if f.is_empty() || !f.bytes().all(|b| b.is_ascii_digit()) {
                        return None;
                    }
                    let bytes = f.as_bytes();
                    let mut m: u32 = 0;
                    for i in 0..6 {
                        let digit = if i < bytes.len() {
                            (bytes[i] - b'0') as u32
                        } else {
                            0
                        };
                        m = m * 10 + digit;
                    }
                    m
                }
            };
            if hour > 23 || minute > 59 || second > 59 {
                return None;
            }
            (hour, minute, second, micros)
        }
    };
    Some(Parts {
        year,
        month,
        day,
        hour,
        minute,
        second,
        micros,
    })
}

fn weekday(p: &Parts) -> usize {
    ((days_from_civil(p.year, p.month, p.day).rem_euclid(7) + 4) % 7) as usize
}

fn weekday_of_days(g: i64) -> usize {
    ((g.rem_euclid(7) + 4) % 7) as usize
}

fn day_of_year(p: &Parts) -> i64 {
    days_from_civil(p.year, p.month, p.day) - days_from_civil(p.year, 1, 1) + 1
}

fn iso_week_date(p: &Parts) -> (i64, i64, i64) {
    let g = days_from_civil(p.year, p.month, p.day);
    let wd = weekday(p) as i64;
    let iso_dow = if wd == 0 { 7 } else { wd };
    let thursday = g + (4 - iso_dow);
    let (ty, _, _) = civil_from_days(thursday);
    let jan1 = days_from_civil(ty, 1, 1);
    let week = (thursday - jan1) / 7 + 1;
    (ty, week, iso_dow)
}

fn iso_to_days(iso_year: i64, iso_week: i64, iso_dow: i64) -> i64 {

    let jan4 = days_from_civil(iso_year, 1, 4);
    let wd = weekday_of_days(jan4) as i64;
    let jan4_dow = if wd == 0 { 7 } else { wd };
    let week1_monday = jan4 - (jan4_dow - 1);
    week1_monday + (iso_week - 1) * 7 + (iso_dow - 1)
}

const ROMAN_MONTH: [&str; 12] = [
    "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI", "XII",
];

fn roman(mut n: i64) -> Option<String> {
    if !(1..=3999).contains(&n) {
        return None;
    }
    const TABLE: [(i64, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut s = String::new();
    for (v, r) in TABLE {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    Some(s)
}

fn ordinal_suffix(n: i64, upper: bool) -> String {
    if n < 0 {
        return String::new();
    }
    let suf = if (11..=13).contains(&(n % 100)) {
        "th"
    } else {
        match n % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        }
    };
    if upper {
        suf.to_uppercase()
    } else {
        suf.to_string()
    }
}

fn group_year(y: i64) -> String {
    let neg = y < 0;
    let s = format!("{:04}", y.abs());
    let len = s.len();
    let mut out = String::new();
    for (idx, b) in s.bytes().enumerate() {
        if idx > 0 && (len - idx) % 3 == 0 {
            out.push(',');
        }
        out.push(b as char);
    }
    if neg {
        format!("-{out}")
    } else {
        out
    }
}

struct DtSource {
    parts: Option<Parts>,

    neg: bool,
    hours: i64,
    minutes: i64,
    seconds: i64,
    micros: i64,
}

impl DtSource {
    fn from_parts(p: Parts) -> Self {
        let (h, mi, s, us) = (
            p.hour as i64,
            p.minute as i64,
            p.second as i64,
            p.micros as i64,
        );
        DtSource {
            hours: h,
            minutes: mi,
            seconds: s,
            micros: us,
            neg: false,
            parts: Some(p),
        }
    }
}

fn parse_interval_time(s: &str) -> Option<DtSource> {

    let tok = s.split_whitespace().find(|t| t.contains(':'))?;
    let (neg, body) = match tok.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, tok),
    };
    let (hms, frac) = match body.split_once('.') {
        Some((a, b)) => (a, Some(b)),
        None => (body, None),
    };
    let mut it = hms.split(':');
    let h: i64 = it.next()?.parse().ok()?;
    let mi: i64 = it.next()?.parse().ok()?;
    let se: i64 = match it.next() {
        Some(x) => x.parse().ok()?,
        None => 0,
    };
    let micros = match frac {
        None => 0,
        Some(f) => {
            let bytes = f.as_bytes();
            let mut m: i64 = 0;
            for i in 0..6 {
                let d = if i < bytes.len() && bytes[i].is_ascii_digit() {
                    (bytes[i] - b'0') as i64
                } else {
                    0
                };
                m = m * 10 + d;
            }
            m
        }
    };
    Some(DtSource {
        neg,
        hours: h,
        minutes: mi,
        seconds: se,
        micros,
        parts: None,
    })
}

fn case_like(sample: &str, upper_word: &str) -> String {

    let first = sample.chars().next().unwrap_or('X');
    let second = sample.chars().nth(1);
    if sample
        .chars()
        .all(|c| c.is_uppercase() || !c.is_alphabetic())
        && first.is_uppercase()
        && second.map(|c| c.is_uppercase()).unwrap_or(true)
    {
        upper_word.to_uppercase()
    } else if first.is_uppercase() {
        upper_word.to_string()
    } else {
        upper_word.to_lowercase()
    }
}

fn pad2(n: i64, fm: bool) -> String {
    if fm {
        n.to_string()
    } else {
        format!("{n:02}")
    }
}

fn format_datetime(src: &DtSource, fmt: &str) -> String {
    let chars: Vec<char> = fmt.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut fm = false;

    let mut last_val: Option<i64> = None;
    let sign = if src.neg { "-" } else { "" };

    let isign = |v: i64| -> String {
        if src.neg && v != 0 {
            "-".to_string()
        } else {
            String::new()
        }
    };
    while i < chars.len() {
        let rest: String = chars[i..].iter().collect();

        let starts = |t: &str| rest.starts_with(t);

        macro_rules! emit {
            ($tok:expr, $val:expr) => {{
                out.push_str(&$val);
                i += $tok.len();
                fm = false;
                continue;
            }};
        }

        macro_rules! emitn {
            ($tok:expr, $val:expr, $num:expr) => {{
                out.push_str(&$val);
                last_val = Some($num);
                i += $tok.len();
                fm = false;
                continue;
            }};
        }

        if starts("FM") {
            fm = true;
            i += 2;
            continue;
        }
        if chars[i] == '"' {

            i += 1;
            while i < chars.len() && chars[i] != '"' {
                out.push(chars[i]);
                i += 1;
            }
            if i < chars.len() {
                i += 1;
            }
            continue;
        }

        if let Some(p) = &src.parts {

            if starts("IYYY") {
                let (iy, _, _) = iso_week_date(p);
                let v = if fm {
                    format!("{iy}")
                } else {
                    format!("{iy:04}")
                };
                emitn!("IYYY", v, iy);
            }
            if starts("IDDD") {
                let (_, iw, id) = iso_week_date(p);
                let v = (iw - 1) * 7 + id;
                let s = if fm {
                    format!("{v}")
                } else {
                    format!("{v:03}")
                };
                emitn!("IDDD", s, v);
            }
            if starts("IYY") {
                let (iy, _, _) = iso_week_date(p);
                emitn!("IYY", format!("{:03}", iy.rem_euclid(1000)), iy);
            }
            if starts("IY") {
                let (iy, _, _) = iso_week_date(p);
                emitn!("IY", format!("{:02}", iy.rem_euclid(100)), iy);
            }
            if starts("IW") {
                let (_, iw, _) = iso_week_date(p);
                emitn!("IW", pad2(iw, fm), iw);
            }
            if starts("ID") {
                let (_, _, id) = iso_week_date(p);
                emitn!("ID", format!("{id}"), id);
            }
            if starts("I") {
                let (iy, _, _) = iso_week_date(p);
                emitn!("I", format!("{}", iy.rem_euclid(10)), iy);
            }

            if starts("Y,YYY") {
                emitn!("Y,YYY", group_year(p.year), p.year);
            }
            if starts("DDD") {
                let v = day_of_year(p);
                let s = if fm {
                    format!("{v}")
                } else {
                    format!("{v:03}")
                };
                emitn!("DDD", s, v);
            }
            if starts("Q") {
                let v = (p.month as i64 - 1) / 3 + 1;
                emitn!("Q", format!("{v}"), v);
            }
            if starts("WW") {
                let v = (day_of_year(p) - 1) / 7 + 1;
                emitn!("WW", pad2(v, fm), v);
            }
            if starts("W") {
                let v = (p.day as i64 - 1) / 7 + 1;
                emitn!("W", format!("{v}"), v);
            }
            if starts("J") {
                let v = days_from_civil(p.year, p.month, p.day) + 2440588;
                emitn!("J", format!("{v}"), v);
            }
            if starts("CC") {
                let v = (p.year - 1) / 100 + 1;
                emitn!("CC", pad2(v, fm), v);
            }
            if starts("RM") {
                let r = ROMAN_MONTH[(p.month - 1) as usize];
                let v = if fm { r.to_string() } else { format!("{r:<4}") };
                emitn!("RM", v, p.month as i64);
            }
            if starts("rm") {
                let r = ROMAN_MONTH[(p.month - 1) as usize].to_lowercase();
                let v = if fm { r } else { format!("{r:<4}") };
                emitn!("rm", v, p.month as i64);
            }
            if starts("YYYY") {
                let v = if fm {
                    format!("{}", p.year)
                } else {
                    format!("{:04}", p.year)
                };
                emit!("YYYY", v);
            }
            if starts("YY") {
                emit!("YY", format!("{:02}", p.year % 100));
            }
            if starts("MONTH") || starts("Month") || starts("month") {
                let tok = &rest[..5];
                let name = case_like(tok, MONTH_FULL[(p.month - 1) as usize]);
                let v = if fm { name } else { format!("{name:9}") };
                emit!("MONTH", v);
            }
            if starts("MON") || starts("Mon") || starts("mon") {
                let tok = &rest[..3];
                emit!("MON", case_like(tok, MONTH_ABBR[(p.month - 1) as usize]));
            }
            if starts("MM") {
                emit!("MM", pad2(p.month as i64, fm));
            }
            if starts("DAY") || starts("Day") || starts("day") {
                let tok = &rest[..3];
                let name = case_like(tok, DAY_FULL[weekday(p)]);
                let v = if fm { name } else { format!("{name:9}") };
                emit!("DAY", v);
            }
            if starts("DY") || starts("Dy") || starts("dy") {
                let tok = &rest[..2];
                emit!("DY", case_like(tok, DAY_ABBR[weekday(p)]));
            }
            if starts("DD") {
                emitn!("DD", pad2(p.day as i64, fm), p.day as i64);
            }
            if starts("D") {

                let v = weekday(p) as i64 + 1;
                emitn!("D", format!("{v}"), v);
            }
        }

        if starts("HH24") {
            let v = format!("{}{}", isign(src.hours), pad2(src.hours.abs(), fm));
            emit!("HH24", v);
        }
        if starts("HH12") || starts("HH") {
            let tok = if starts("HH12") { "HH12" } else { "HH" };
            let h12 = {
                let h = src.hours.abs() % 12;
                if h == 0 {
                    12
                } else {
                    h
                }
            };
            let v = format!("{}{}", isign(src.hours), pad2(h12, fm));
            emit!(tok, v);
        }
        if starts("MI") {
            let v = format!("{}{}", isign(src.minutes), pad2(src.minutes.abs(), fm));
            emit!("MI", v);
        }
        if starts("SSSSS") || starts("SSSS") {

            let tok = if starts("SSSSS") { "SSSSS" } else { "SSSS" };
            let v = src.hours.abs() * 3600 + src.minutes.abs() * 60 + src.seconds.abs();
            emitn!(tok, format!("{v}"), v);
        }
        if starts("SS") {
            let v = format!("{}{}", isign(src.seconds), pad2(src.seconds.abs(), fm));
            emit!("SS", v);
        }
        if starts("MS") {
            emit!("MS", format!("{:03}", (src.micros.abs() / 1000)));
        }
        if starts("US") {
            emit!("US", format!("{:06}", src.micros.abs()));
        }
        if starts("AM") || starts("PM") {
            let pm = src.hours.abs() % 24 >= 12;
            emit!(
                "AM",
                if pm {
                    "PM".to_string()
                } else {
                    "AM".to_string()
                }
            );
        }
        if starts("am") || starts("pm") {
            let pm = src.hours.abs() % 24 >= 12;
            emit!(
                "am",
                if pm {
                    "pm".to_string()
                } else {
                    "am".to_string()
                }
            );
        }
        if starts("A.M.") || starts("P.M.") {
            let pm = src.hours.abs() % 24 >= 12;
            emit!(
                "A.M.",
                if pm {
                    "P.M.".to_string()
                } else {
                    "A.M.".to_string()
                }
            );
        }
        if starts("a.m.") || starts("p.m.") {
            let pm = src.hours.abs() % 24 >= 12;
            emit!(
                "a.m.",
                if pm {
                    "p.m.".to_string()
                } else {
                    "a.m.".to_string()
                }
            );
        }
        if starts("TH") || starts("th") {

            let upper = starts("TH");
            let suf = match last_val {
                Some(n) => ordinal_suffix(n, upper),
                None => String::new(),
            };
            out.push_str(&suf);
            i += 2;
            fm = false;
            continue;
        }
        if starts("TZ") || starts("tz") {

            i += 2;
            continue;
        }

        let _ = sign;
        out.push(chars[i]);
        i += 1;
    }
    out
}

#[derive(Clone, Copy, PartialEq)]
enum NDigit {
    Nine,
    Zero,
}
enum IntTok {
    Digit(NDigit),
    Group,
}

struct NumFmt {
    fm: bool,
    currency: bool,
    lead_s: bool,
    trail_s: bool,
    mi: bool,
    pr: bool,
    int_toks: Vec<IntTok>,
    frac: Vec<NDigit>,
    has_decimal: bool,
    pr_before_digit: bool,
}

fn parse_num_fmt(fmt: &str) -> NumFmt {
    let up = fmt.to_uppercase();
    let bytes: Vec<char> = up.chars().collect();
    let mut f = NumFmt {
        fm: false,
        currency: false,
        lead_s: false,
        trail_s: false,
        mi: false,
        pr: false,
        int_toks: Vec::new(),
        frac: Vec::new(),
        has_decimal: false,
        pr_before_digit: false,
    };
    let mut seen_digit = false;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == 'F' && bytes.get(i + 1) == Some(&'M') {
            f.fm = true;
            i += 2;
            continue;
        }
        if c == 'P' && bytes.get(i + 1) == Some(&'R') {
            f.pr = true;
            if !seen_digit {
                f.pr_before_digit = true;
            }
            i += 2;
            continue;
        }
        if c == 'M' && bytes.get(i + 1) == Some(&'I') {
            f.mi = true;
            i += 2;
            continue;
        }
        match c {
            'L' => f.currency = true,
            'S' => {
                if seen_digit {
                    f.trail_s = true;
                } else {
                    f.lead_s = true;
                }
            }
            '9' | '0' => {
                seen_digit = true;
                let d = if c == '9' { NDigit::Nine } else { NDigit::Zero };
                if f.has_decimal {
                    f.frac.push(d);
                } else {
                    f.int_toks.push(IntTok::Digit(d));
                }
            }
            ',' | 'G' => {
                if !f.has_decimal {
                    f.int_toks.push(IntTok::Group);
                }
            }
            '.' | 'D' => f.has_decimal = true,
            _ => {}
        }
        i += 1;
    }
    f
}

fn value_digits(v: &SqlValue) -> Option<(bool, String, String)> {
    let s = match v {
        SqlValue::Int(n) => {
            let neg = *n < 0;
            return Some((neg, n.unsigned_abs().to_string(), String::new()));
        }
        SqlValue::Real(_) => types::floats::output(oid::FLOAT8, v),
        SqlValue::Text(t) => t.clone(),
        _ => return None,
    };
    let s = s.trim();
    let (neg, rest) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };

    let (int, frac) = match rest.split_once('.') {
        Some((a, b)) => (a, b),
        None => (rest, ""),
    };
    if int.is_empty() && frac.is_empty() {
        return None;
    }
    if !int.bytes().all(|b| b.is_ascii_digit()) || !frac.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((neg, int.to_string(), frac.to_string()))
}

fn inc_digits(d: &str) -> String {
    let mut v: Vec<u8> = d.bytes().collect();
    let mut i = v.len();
    loop {
        if i == 0 {
            let mut r = vec![b'1'];
            r.extend_from_slice(&v);
            return String::from_utf8(r).unwrap();
        }
        i -= 1;
        if v[i] == b'9' {
            v[i] = b'0';
        } else {
            v[i] += 1;
            break;
        }
    }
    String::from_utf8(v).unwrap()
}

fn round_to(int: &str, frac: &str, scale: usize) -> (String, String) {
    let mut all = format!("{int}{frac}");
    let point = int.len();
    if frac.len() > scale {

        let round_pos = point + scale;
        let decide = all.as_bytes()[round_pos];
        all.truncate(round_pos);
        if decide >= b'5' {
            all = inc_digits(&all);
        }
    } else {

        for _ in 0..(scale - frac.len()) {
            all.push('0');
        }
    }

    let total = all.len();
    let point = total - scale;
    let int_part = &all[..point];
    let frac_part = &all[point..];
    let int_trim = int_part.trim_start_matches('0');
    let int_out = if int_trim.is_empty() {
        "0".to_string()
    } else {
        int_trim.to_string()
    };
    (int_out, frac_part.to_string())
}

fn format_number(v: &SqlValue, fmt: &str) -> Option<String> {

    if fmt.contains("RN") || fmt.contains("rn") {
        let lower = fmt.contains("rn");
        let fm = fmt.to_uppercase().contains("FM");
        let (neg, int, frac) = value_digits(v)?;
        let (rint, _) = round_to(&int, &frac, 0);
        let n: i64 = rint.parse().ok()?;
        let n = if neg { -n } else { n };
        let body = match roman(n) {
            Some(r) => {
                let r = if lower { r.to_lowercase() } else { r };
                if fm {
                    r
                } else {
                    format!("{r:>15}")
                }
            }
            None => "#".repeat(15),
        };
        return Some(body);
    }

    let up = fmt.to_uppercase();
    if let Some(vpos) = up.find('V') {
        let shift = up[vpos + 1..]
            .chars()
            .filter(|c| *c == '9' || *c == '0')
            .count();
        let (neg, int, frac) = value_digits(v)?;
        let fchars: Vec<char> = frac.chars().collect();
        let mut int2 = int;
        let mut fi = 0usize;
        for _ in 0..shift {
            if fi < fchars.len() {
                int2.push(fchars[fi]);
                fi += 1;
            } else {
                int2.push('0');
            }
        }
        let rem: String = fchars[fi..].iter().collect();
        let mut scaled = if neg { format!("-{int2}") } else { int2 };
        if !rem.is_empty() {
            scaled.push('.');
            scaled.push_str(&rem);
        }
        let fmt2: String = fmt.chars().filter(|c| *c != 'V' && *c != 'v').collect();
        return format_number(&SqlValue::Text(scaled), &fmt2);
    }

    let th_mode = if fmt.contains("TH") {
        Some(true)
    } else if fmt.contains("th") {
        Some(false)
    } else {
        None
    };

    let f = parse_num_fmt(fmt);
    let (mut neg, int, frac) = value_digits(v)?;
    let int_slots = f
        .int_toks
        .iter()
        .filter(|t| matches!(t, IntTok::Digit(_)))
        .count();
    let scale = f.frac.len();
    let (rint, rfrac) = round_to(&int, &frac, scale);

    if rint.chars().all(|c| c == '0') && rfrac.chars().all(|c| c == '0') {
        neg = false;
    }

    let int_zero = rint == "0";
    let sig = if int_zero {
        if scale > 0 {
            String::new()
        } else {
            "0".to_string()
        }
    } else {
        rint.clone()
    };

    let int_region = if sig.len() > int_slots {

        "#".repeat(int_slots)
    } else {

        let digit_chars: Vec<char> = {
            let leading = int_slots - sig.len();

            let leftmost0 = f
                .int_toks
                .iter()
                .filter_map(|t| match t {
                    IntTok::Digit(d) => Some(*d),
                    _ => None,
                })
                .position(|d| d == NDigit::Zero)
                .unwrap_or(usize::MAX);
            let sig_bytes: Vec<char> = sig.chars().collect();
            (0..int_slots)
                .map(|slot| {
                    if slot < leading {
                        if slot >= leftmost0 {
                            '0'
                        } else {
                            ' '
                        }
                    } else {
                        sig_bytes[slot - leading]
                    }
                })
                .collect()
        };
        let mut region = String::new();
        let mut p = 0usize;
        for tok in &f.int_toks {
            match tok {
                IntTok::Digit(_) => {
                    region.push(digit_chars[p]);
                    p += 1;
                }
                IntTok::Group => {
                    let left_has = region.chars().any(|c| c != ' ');
                    region.push(if left_has { ',' } else { ' ' });
                }
            }
        }
        region
    };

    let mut frac_region = String::new();
    if f.has_decimal {
        frac_region.push('.');
        for (idx, d) in f.frac.iter().enumerate() {
            frac_region.push(rfrac.as_bytes()[idx] as char);
            let _ = d;
        }
        if f.fm {

            let mut keep = f.frac.len();
            while keep > 0 && f.frac[keep - 1] == NDigit::Nine && frac_region.ends_with('0') {
                frac_region.pop();
                keep -= 1;
            }
            if frac_region == "." {
                frac_region.clear();
            }
        }
    }

    let mut body = format!("{int_region}{frac_region}");
    let cs = body
        .char_indices()
        .find(|(_, c)| *c != ' ')
        .map(|(i, _)| i)
        .unwrap_or(body.len());

    let (lead, trail): (Option<char>, Option<char>) = if f.pr {
        if neg {
            (Some('<'), Some('>'))
        } else {
            (Some(' '), Some(' '))
        }
    } else if f.mi {
        (None, Some(if neg { '-' } else { ' ' }))
    } else if f.lead_s {
        (Some(if neg { '-' } else { '+' }), None)
    } else if f.trail_s {
        (None, Some(if neg { '-' } else { '+' }))
    } else {
        (Some(if neg { '-' } else { ' ' }), None)
    };
    if let Some(lc) = lead {
        body.insert(cs, lc);
    }
    if let Some(tc) = trail {
        body.push(tc);
    }
    if f.currency {
        body.insert(0, '$');
    }
    if f.fm {
        body = body.trim_matches(' ').to_string();
    }
    if let Some(upper) = th_mode {
        let n: i64 = rint.parse().unwrap_or(0);
        let n = if neg { -n } else { n };
        body.push_str(&ordinal_suffix(n, upper));
    }
    Some(body)
}

fn to_char(args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() != 2 {
        return Err(does_not_exist("to_char"));
    }
    if any_null(args) {
        return Ok(SqlValue::Null);
    }
    let fmt = as_text(&args[1]).ok_or_else(|| does_not_exist("to_char"))?;
    let out = match &args[0] {
        SqlValue::Int(_) | SqlValue::Real(_) => {
            format_number(&args[0], &fmt).ok_or_else(|| does_not_exist("to_char"))?
        }
        SqlValue::Text(s) => {
            if let Some(p) = parse_datetime(s) {
                format_datetime(&DtSource::from_parts(p), &fmt)
            } else if let Some(iv) = parse_interval_time(s) {
                format_datetime(&iv, &fmt)
            } else {
                format_number(&args[0], &fmt).ok_or_else(|| does_not_exist("to_char"))?
            }
        }
        _ => return Err(does_not_exist("to_char")),
    };
    Ok(SqlValue::Text(out))
}

fn to_number(args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() != 2 {
        return Err(does_not_exist("to_number"));
    }
    if any_null(args) {
        return Ok(SqlValue::Null);
    }
    let input = as_text(&args[0]).ok_or_else(|| does_not_exist("to_number"))?;
    let _fmt = as_text(&args[1]).ok_or_else(|| does_not_exist("to_number"))?;

    let mut num = String::new();
    let mut neg = false;
    let mut seen_dot = false;
    for c in input.chars() {
        match c {
            '0'..='9' => num.push(c),
            '.' if !seen_dot => {
                seen_dot = true;
                num.push('.');
            }
            '-' | '<' => neg = true,
            _ => {}
        }
    }
    if !num.chars().any(|c| c.is_ascii_digit()) {
        return Err(PgError::InvalidInputSyntax {
            typ: "numeric",
            input: input.clone(),
        });
    }
    let signed = if neg { format!("-{num}") } else { num };
    types::numeric::input(oid::NUMERIC, &signed)
}

struct DtFields {
    year: i64,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    micros: u32,
    hour12: bool,
    pm: Option<bool>,

    jday: Option<i64>,
    ddd: Option<i64>,
    iso_year: Option<i64>,
    iso_week: Option<i64>,
    iso_dow: Option<i64>,
}

fn parse_by_format(input: &str, fmt: &str) -> Result<DtFields, PgError> {
    let inb: Vec<char> = input.chars().collect();
    let fb: Vec<char> = fmt.chars().collect();
    let mut ic = 0usize;
    let mut fi = 0usize;
    let mut out = DtFields {
        year: 1,
        month: 1,
        day: 1,
        hour: 0,
        minute: 0,
        second: 0,
        micros: 0,
        hour12: false,
        pm: None,
        jday: None,
        ddd: None,
        iso_year: None,
        iso_week: None,
        iso_dow: None,
    };
    let read_int = |inb: &[char], ic: &mut usize, width: usize| -> Option<i64> {
        while *ic < inb.len() && !inb[*ic].is_ascii_digit() {
            *ic += 1;
        }
        let start = *ic;
        let mut n: i64 = 0;
        let mut cnt = 0;
        while *ic < inb.len() && inb[*ic].is_ascii_digit() && cnt < width {
            n = n * 10 + (inb[*ic] as i64 - '0' as i64);
            *ic += 1;
            cnt += 1;
        }
        if *ic == start {
            None
        } else {
            Some(n)
        }
    };
    let read_alpha = |inb: &[char], ic: &mut usize| -> String {
        while *ic < inb.len() && !inb[*ic].is_alphabetic() {
            *ic += 1;
        }
        let start = *ic;
        while *ic < inb.len() && inb[*ic].is_alphabetic() {
            *ic += 1;
        }
        inb[start..*ic].iter().collect()
    };

    while fi < fb.len() {
        let rest: String = fb[fi..].iter().collect();
        let field = |t: &str| rest.starts_with(t);
        let err = |tok: &str| PgError::InvalidInputSyntax {
            typ: "expression",
            input: format!("invalid value for \"{tok}\""),
        };
        if field("IYYY") {
            out.iso_year = Some(read_int(&inb, &mut ic, 4).ok_or_else(|| err("IYYY"))?);
            fi += 4;
        } else if field("IW") {
            out.iso_week = Some(read_int(&inb, &mut ic, 2).ok_or_else(|| err("IW"))?);
            fi += 2;
        } else if field("ID") {
            out.iso_dow = Some(read_int(&inb, &mut ic, 1).ok_or_else(|| err("ID"))?);
            fi += 2;
        } else if field("DDD") {
            out.ddd = Some(read_int(&inb, &mut ic, 3).ok_or_else(|| err("DDD"))?);
            fi += 3;
        } else if field("J") {
            out.jday = Some(read_int(&inb, &mut ic, 7).ok_or_else(|| err("J"))?);
            fi += 1;
        } else if field("TH") || field("th") {

            while ic < inb.len() && inb[ic].is_alphabetic() {
                ic += 1;
            }
            fi += 2;
        } else if field("YYYY") {
            out.year = read_int(&inb, &mut ic, 4).ok_or_else(|| err("YYYY"))?;
            fi += 4;
        } else if field("YY") {
            let y = read_int(&inb, &mut ic, 2).ok_or_else(|| err("YY"))?;
            out.year = 2000 + y;
            fi += 2;
        } else if field("MON") || field("Mon") || field("mon") {
            let w = read_alpha(&inb, &mut ic).to_lowercase();
            let m = MONTH_ABBR
                .iter()
                .position(|n| n.to_lowercase() == w || w.starts_with(&n.to_lowercase()))
                .or_else(|| {
                    MONTH_FULL
                        .iter()
                        .position(|n| n.to_lowercase().starts_with(&w) && !w.is_empty())
                })
                .ok_or_else(|| err("Mon"))?;
            out.month = (m + 1) as u32;

            if field("MONTH") || field("Month") || field("month") {
                fi += 5;
            } else {
                fi += 3;
            }
        } else if field("MM") {
            out.month = read_int(&inb, &mut ic, 2).ok_or_else(|| err("MM"))? as u32;
            fi += 2;
        } else if field("DD") {
            out.day = read_int(&inb, &mut ic, 2).ok_or_else(|| err("DD"))? as u32;
            fi += 2;
        } else if field("HH24") {
            out.hour = read_int(&inb, &mut ic, 2).ok_or_else(|| err("HH24"))? as u32;
            fi += 4;
        } else if field("HH12") {
            out.hour = read_int(&inb, &mut ic, 2).ok_or_else(|| err("HH12"))? as u32;
            out.hour12 = true;
            fi += 4;
        } else if field("HH") {
            out.hour = read_int(&inb, &mut ic, 2).ok_or_else(|| err("HH"))? as u32;
            out.hour12 = true;
            fi += 2;
        } else if field("MI") {
            out.minute = read_int(&inb, &mut ic, 2).ok_or_else(|| err("MI"))? as u32;
            fi += 2;
        } else if field("SS") {
            out.second = read_int(&inb, &mut ic, 2).ok_or_else(|| err("SS"))? as u32;
            fi += 2;
        } else if field("US") {
            let u = read_int(&inb, &mut ic, 6).ok_or_else(|| err("US"))?;
            out.micros = u as u32;
            fi += 2;
        } else if field("MS") {
            let m = read_int(&inb, &mut ic, 3).ok_or_else(|| err("MS"))?;
            out.micros = (m as u32) * 1000;
            fi += 2;
        } else if field("AM") || field("PM") || field("A.M.") || field("P.M.") {
            let w = read_alpha(&inb, &mut ic).to_uppercase();
            out.pm = Some(w.starts_with('P'));
            fi += if field("A.M.") || field("P.M.") { 4 } else { 2 };
        } else {

            if ic < inb.len() && !inb[ic].is_alphanumeric() {
                ic += 1;
            }
            fi += 1;
        }
    }

    if let Some(j) = out.jday {
        let (y, m, d) = civil_from_days(j - 2440588);
        out.year = y;
        out.month = m;
        out.day = d;
    } else if out.iso_year.is_some() || out.iso_week.is_some() || out.iso_dow.is_some() {
        let iy = out.iso_year.unwrap_or(out.year);
        let iw = out.iso_week.unwrap_or(1);
        let id = out.iso_dow.unwrap_or(1);
        let (y, m, d) = civil_from_days(iso_to_days(iy, iw, id));
        out.year = y;
        out.month = m;
        out.day = d;
    } else if let Some(dd) = out.ddd {
        let (y, m, d) = civil_from_days(days_from_civil(out.year, 1, 1) + dd - 1);
        out.year = y;
        out.month = m;
        out.day = d;
    }
    if out.hour12 {
        if out.pm == Some(true) && out.hour < 12 {
            out.hour += 12;
        } else if out.pm == Some(false) && out.hour == 12 {
            out.hour = 0;
        }
    }
    Ok(out)
}

fn to_date(args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() != 2 {
        return Err(does_not_exist("to_date"));
    }
    if any_null(args) {
        return Ok(SqlValue::Null);
    }
    let input = as_text(&args[0]).ok_or_else(|| does_not_exist("to_date"))?;
    let fmt = as_text(&args[1]).ok_or_else(|| does_not_exist("to_date"))?;
    let f = parse_by_format(&input, &fmt)?;
    let candidate = format!("{:04}-{:02}-{:02}", f.year, f.month, f.day);
    types::date::input(oid::DATE, &candidate)
}

fn epoch_to_tstz(epoch_secs: f64) -> Result<SqlValue, PgError> {
    let total = epoch_secs.floor() as i64;
    let days = total.div_euclid(86400);
    let rem = total.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    let h = rem / 3600;
    let mi = (rem % 3600) / 60;
    let s = rem % 60;
    let frac_us = ((epoch_secs - epoch_secs.floor()) * 1_000_000.0).round() as i64;
    let mut candidate = format!("{y:04}-{m:02}-{d:02} {h:02}:{mi:02}:{s:02}");
    if frac_us > 0 {
        candidate.push('.');
        candidate.push_str(format!("{frac_us:06}").trim_end_matches('0'));
    }
    candidate.push_str("+00");
    types::timestamptz::input(oid::TIMESTAMPTZ, &candidate)
}

fn to_timestamp(args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if any_null(args) {
        return Ok(SqlValue::Null);
    }
    match args.len() {

        1 => {
            let secs = match &args[0] {
                SqlValue::Int(n) => *n as f64,
                SqlValue::Real(r) => *r,
                _ => return Err(does_not_exist("to_timestamp")),
            };
            epoch_to_tstz(secs)
        }
        2 => {
            let input = as_text(&args[0]).ok_or_else(|| does_not_exist("to_timestamp"))?;
            let fmt = as_text(&args[1]).ok_or_else(|| does_not_exist("to_timestamp"))?;
            let f = parse_by_format(&input, &fmt)?;
            let mut candidate = format!(
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                f.year, f.month, f.day, f.hour, f.minute, f.second
            );
            if f.micros > 0 {
                candidate.push('.');
                candidate.push_str(format!("{:06}", f.micros).trim_end_matches('0'));
            }
            candidate.push_str("+00");
            types::timestamptz::input(oid::TIMESTAMPTZ, &candidate)
        }
        _ => Err(does_not_exist("to_timestamp")),
    }
}

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    match name {
        "to_char" => Some(to_char(args)),
        "to_number" => Some(to_number(args)),
        "to_date" => Some(to_date(args)),
        "to_timestamp" => Some(to_timestamp(args)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tc(v: SqlValue, fmt: &str) -> String {
        match to_char(&[v, SqlValue::Text(fmt.into())]) {
            Ok(SqlValue::Text(s)) => s,
            other => panic!("expected text, got {other:?}"),
        }
    }
    fn ts(s: &str) -> SqlValue {
        SqlValue::Text(s.into())
    }

    #[test]
    fn datetime_common() {
        assert_eq!(
            tc(ts("2024-01-15 13:45:09"), "YYYY-MM-DD HH24:MI:SS"),
            "2024-01-15 13:45:09"
        );
        assert_eq!(tc(ts("2024-01-15"), "Mon DD, YYYY"), "Jan 15, 2024");
        assert_eq!(
            tc(ts("2024-01-15 13:45:09"), "HH12:MI:SS AM"),
            "01:45:09 PM"
        );
        assert_eq!(tc(ts("2024-03-05"), "Month DD, YYYY"), "March     05, 2024");
        assert_eq!(tc(ts("2024-03-05"), "FMMonth FMDD, YYYY"), "March 5, 2024");
        assert_eq!(tc(ts("2024-01-15"), "Dy Day D"), "Mon Monday    2");
        assert_eq!(tc(ts("2024-01-15"), "MON mon Mon YY"), "JAN jan Jan 24");
        assert_eq!(
            tc(ts("2024-01-15 13:45:09.123456"), "HH24:MI:SS.MS US"),
            "13:45:09.123 123456"
        );
        assert_eq!(
            tc(ts("2024-01-15 13:45:09"), "YYYY\"y\"MM\"m\"DD\"d\""),
            "2024y01m15d"
        );
        assert_eq!(tc(ts("2024-01-05"), "FMDD FMMon"), "5 Jan");
    }

    #[test]
    fn interval_time() {
        assert_eq!(tc(ts("1 day 02:03:04"), "HH24:MI:SS"), "02:03:04");
        assert_eq!(tc(ts("05:30:00"), "HH24:MI:SS"), "05:30:00");
        assert_eq!(tc(ts("26:03:00"), "HH24:MI:SS"), "26:03:00");
    }

    #[test]
    fn number_common() {
        assert_eq!(tc(SqlValue::Real(1234.5), "9999.99"), " 1234.50");
        assert_eq!(tc(SqlValue::Int(-12), "999PR"), " <12>");
        assert_eq!(tc(SqlValue::Int(12), "999PR"), "  12 ");
        assert_eq!(
            tc(SqlValue::Real(1234567.89), "FM9,999,999.00"),
            "1,234,567.89"
        );
        assert_eq!(
            tc(SqlValue::Real(1234567.89), "9,999,999.00"),
            " 1,234,567.89"
        );
        assert_eq!(tc(SqlValue::Int(-12), "S999"), " -12");
        assert_eq!(tc(SqlValue::Int(12), "S999"), " +12");
        assert_eq!(tc(SqlValue::Real(0.5), "9999.99"), "     .50");
        assert_eq!(tc(SqlValue::Real(0.5), "0000.00"), " 0000.50");
        assert_eq!(tc(SqlValue::Real(1234.5), "FM9999.99"), "1234.5");
        assert_eq!(tc(SqlValue::Int(-12), "999MI"), " 12-");
        assert_eq!(tc(SqlValue::Int(12), "999MI"), " 12 ");
        assert_eq!(tc(SqlValue::Real(1234.56), "9G999D99"), " 1,234.56");
        assert_eq!(tc(SqlValue::Int(0), "9999"), "    0");
        assert_eq!(tc(SqlValue::Int(0), "9999.99"), "     .00");
        assert_eq!(tc(SqlValue::Real(1234.5), "99999999"), "     1235");
        assert_eq!(tc(SqlValue::Real(123.456), "999.99"), " 123.46");
        assert_eq!(tc(SqlValue::Real(1234.5), "L9999.99"), "$ 1234.50");
    }

    #[test]
    fn parse_roundtrip() {
        assert_eq!(
            to_timestamp(&[ts("2024-01-15"), ts("YYYY-MM-DD")]),
            Ok(ts("2024-01-15 00:00:00+00"))
        );
        assert_eq!(
            to_timestamp(&[ts("2024-01-15 13:45:09"), ts("YYYY-MM-DD HH24:MI:SS")]),
            Ok(ts("2024-01-15 13:45:09+00"))
        );
        assert_eq!(
            to_timestamp(&[SqlValue::Int(0)]),
            Ok(ts("1970-01-01 00:00:00+00"))
        );
        assert_eq!(
            to_timestamp(&[SqlValue::Int(1705324509)]),
            Ok(ts("2024-01-15 13:15:09+00"))
        );
        assert_eq!(
            to_date(&[ts("15 Jan 2024"), ts("DD Mon YYYY")]),
            Ok(ts("2024-01-15"))
        );
        assert_eq!(
            to_date(&[ts("2024-01-15"), ts("YYYY-MM-DD")]),
            Ok(ts("2024-01-15"))
        );
        assert_eq!(
            to_number(&[ts("1,234.56"), ts("9,999.99")]),
            Ok(ts("1234.56"))
        );
        assert_eq!(
            to_number(&[ts("12,345.6-"), ts("99,999.9MI")]),
            Ok(ts("-12345.6"))
        );
    }

    #[test]
    fn datetime_exotic() {

        assert_eq!(tc(ts("2024-03-05"), "DDD"), "065");
        assert_eq!(tc(ts("2024-03-05"), "FMDDD"), "65");
        assert_eq!(tc(ts("2024-12-31"), "IDDD"), "002");
        assert_eq!(tc(ts("2024-03-05"), "Q"), "1");
        assert_eq!(tc(ts("2024-11-05"), "Q"), "4");
        assert_eq!(tc(ts("2024-03-05"), "WW"), "10");
        assert_eq!(tc(ts("2024-01-01"), "WW"), "01");
        assert_eq!(tc(ts("2024-03-05"), "W"), "1");

        assert_eq!(tc(ts("2024-01-15"), "J"), "2460325");
        assert_eq!(tc(ts("1970-01-01"), "J"), "2440588");

        assert_eq!(tc(ts("2024-03-05"), "RM"), "III ");
        assert_eq!(tc(ts("2024-12-05"), "RM"), "XII ");
        assert_eq!(tc(ts("2024-08-05"), "RM"), "VIII");
        assert_eq!(tc(ts("2024-03-05"), "rm"), "iii ");
        assert_eq!(tc(ts("2024-03-05"), "FMRM"), "III");

        assert_eq!(tc(ts("2024-03-15"), "FMDDth"), "15th");
        assert_eq!(tc(ts("2024-03-15"), "DDTH"), "15TH");
        assert_eq!(tc(ts("2024-03-01"), "FMDDth"), "1st");

        assert_eq!(tc(ts("2024-12-31"), "IYYY"), "2025");
        assert_eq!(tc(ts("2024-12-31"), "IYY"), "025");
        assert_eq!(tc(ts("2024-12-31"), "IY"), "25");
        assert_eq!(tc(ts("2024-12-31"), "I"), "5");
        assert_eq!(tc(ts("2024-12-31"), "IW"), "01");
        assert_eq!(tc(ts("2024-03-05"), "ID"), "2");

        assert_eq!(tc(ts("2024-03-05"), "CC"), "21");
        assert_eq!(tc(ts("2000-06-01"), "CC"), "20");
        assert_eq!(tc(ts("2024-03-05"), "Y,YYY"), "2,024");
        assert_eq!(tc(ts("2024-03-05 13:45:09"), "SSSS"), "49509");
    }

    #[test]
    fn number_exotic() {

        assert_eq!(tc(SqlValue::Int(2023), "RN"), "        MMXXIII");
        assert_eq!(tc(SqlValue::Int(2023), "rn"), "        mmxxiii");
        assert_eq!(tc(SqlValue::Int(2023), "FMRN"), "MMXXIII");
        assert_eq!(tc(SqlValue::Int(3999), "RN"), "      MMMCMXCIX");
        assert_eq!(tc(SqlValue::Int(0), "RN"), "###############");
        assert_eq!(tc(SqlValue::Int(4000), "RN"), "###############");

        assert_eq!(tc(SqlValue::Real(1.5), "9V9"), " 15");
        assert_eq!(tc(SqlValue::Real(1.5), "9V99"), " 150");
        assert_eq!(tc(SqlValue::Int(12), "9V99"), " ###");
        assert_eq!(tc(SqlValue::Real(1.2345), "9V999"), " 1235");
        assert_eq!(tc(SqlValue::Int(2), "9V9"), " 20");

        assert_eq!(tc(SqlValue::Int(15), "FM99TH"), "15TH");
        assert_eq!(tc(SqlValue::Int(1), "99TH"), "  1ST");
        assert_eq!(tc(SqlValue::Int(111), "999TH"), " 111TH");
        assert_eq!(tc(SqlValue::Int(0), "9TH"), " 0TH");
    }

    #[test]
    fn parse_exotic() {
        assert_eq!(
            to_date(&[ts("2024 065"), ts("YYYY DDD")]),
            Ok(ts("2024-03-05"))
        );
        assert_eq!(to_date(&[ts("2460325"), ts("J")]), Ok(ts("2024-01-15")));
        assert_eq!(
            to_date(&[ts("2024 15th"), ts("YYYY DDth")]),
            Ok(ts("2024-01-15"))
        );
        assert_eq!(
            to_date(&[ts("2025 01 3"), ts("IYYY IW ID")]),
            Ok(ts("2025-01-01"))
        );
        assert_eq!(
            to_timestamp(&[ts("2460325"), ts("J")]),
            Ok(ts("2024-01-15 00:00:00+00"))
        );
    }

    #[test]
    fn unclaimed_and_arity() {
        assert!(call("to_hex", &[SqlValue::Int(1)]).is_none());
        assert!(matches!(call("to_char", &[SqlValue::Int(1)]), Some(Err(_))));
    }
}
