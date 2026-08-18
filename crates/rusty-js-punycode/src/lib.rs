
const BASE: u32 = 36;
const TMIN: u32 = 1;
const TMAX: u32 = 26;
const SKEW: u32 = 38;
const DAMP: u32 = 700;
const INITIAL_BIAS: u32 = 72;
const INITIAL_N: u32 = 128;
const DELIMITER: char = '-';

pub const ACE_PREFIX: &str = "xn--";

#[derive(Debug, PartialEq, Eq)]
pub enum PunycodeError {

    NonBasicInput,

    InvalidDigit,

    Overflow,

    UnexpectedEnd,

    InvalidCodePoint,
}

fn adapt(mut delta: u32, num_points: u32, first_time: bool) -> u32 {
    delta /= if first_time { DAMP } else { 2 };
    delta += delta / num_points;
    let mut k = 0u32;
    while delta > ((BASE - TMIN) * TMAX) / 2 {
        delta /= BASE - TMIN;
        k += BASE;
    }
    k + (BASE - TMIN + 1) * delta / (delta + SKEW)
}

fn threshold(k: u32, bias: u32) -> u32 {
    if k <= bias + TMIN {
        TMIN
    } else if k >= bias + TMAX {
        TMAX
    } else {
        k - bias
    }
}

fn encode_digit(d: u32) -> char {
    debug_assert!(d < BASE);
    if d < 26 {
        (b'a' + d as u8) as char
    } else {
        (b'0' + (d - 26) as u8) as char
    }
}

fn decode_digit(c: char) -> Option<u32> {
    match c {
        'a'..='z' => Some(c as u32 - 'a' as u32),
        'A'..='Z' => Some(c as u32 - 'A' as u32),
        '0'..='9' => Some(c as u32 - '0' as u32 + 26),
        _ => None,
    }
}

pub fn encode(input: &str) -> Result<String, PunycodeError> {
    let input: Vec<u32> = input.chars().map(|c| c as u32).collect();
    let mut output = String::new();

    let mut b = 0u32;
    for &c in &input {
        if c < 0x80 {
            output.push(c as u8 as char);
            b += 1;
        }
    }
    let basic = b;
    if basic > 0 {
        output.push(DELIMITER);
    }

    let mut n = INITIAL_N;
    let mut delta = 0u32;
    let mut bias = INITIAL_BIAS;
    let mut h = basic;
    let total = input.len() as u32;

    while h < total {

        let m = input
            .iter()
            .copied()
            .filter(|&c| c >= n)
            .min()
            .ok_or(PunycodeError::UnexpectedEnd)?;

        delta = delta
            .checked_add((m - n).checked_mul(h + 1).ok_or(PunycodeError::Overflow)?)
            .ok_or(PunycodeError::Overflow)?;
        n = m;

        for &c in &input {
            if c < n {
                delta = delta.checked_add(1).ok_or(PunycodeError::Overflow)?;
            }
            if c == n {
                let mut q = delta;
                let mut k = BASE;
                loop {
                    let t = threshold(k, bias);
                    if q < t {
                        break;
                    }
                    output.push(encode_digit(t + (q - t) % (BASE - t)));
                    q = (q - t) / (BASE - t);
                    k += BASE;
                }
                output.push(encode_digit(q));
                bias = adapt(delta, h + 1, h == basic);
                delta = 0;
                h += 1;
            }
        }
        delta += 1;
        n += 1;
    }

    Ok(output)
}

pub fn decode(input: &str) -> Result<String, PunycodeError> {

    if !input.is_ascii() {
        return Err(PunycodeError::NonBasicInput);
    }
    let chars: Vec<char> = input.chars().collect();

    let (basic, mut idx) = match chars.iter().rposition(|&c| c == DELIMITER) {
        Some(pos) => (chars[..pos].to_vec(), pos + 1),
        None => (Vec::new(), 0),
    };
    let mut output: Vec<u32> = basic.iter().map(|&c| c as u32).collect();

    let mut n = INITIAL_N;
    let mut i = 0u32;
    let mut bias = INITIAL_BIAS;

    while idx < chars.len() {
        let old_i = i;
        let mut w = 1u32;
        let mut k = BASE;
        loop {
            let c = *chars.get(idx).ok_or(PunycodeError::UnexpectedEnd)?;
            idx += 1;
            let digit = decode_digit(c).ok_or(PunycodeError::InvalidDigit)?;
            i = i
                .checked_add(digit.checked_mul(w).ok_or(PunycodeError::Overflow)?)
                .ok_or(PunycodeError::Overflow)?;
            let t = threshold(k, bias);
            if digit < t {
                break;
            }
            w = w.checked_mul(BASE - t).ok_or(PunycodeError::Overflow)?;
            k += BASE;
        }
        let out_len = output.len() as u32 + 1;
        bias = adapt(i - old_i, out_len, old_i == 0);
        n = n.checked_add(i / out_len).ok_or(PunycodeError::Overflow)?;
        i %= out_len;

        if n < 0x80 {
            return Err(PunycodeError::InvalidCodePoint);
        }
        if char::from_u32(n).is_none() {
            return Err(PunycodeError::InvalidCodePoint);
        }
        output.insert(i as usize, n);
        i += 1;
    }

    output
        .into_iter()
        .map(|cp| char::from_u32(cp).ok_or(PunycodeError::InvalidCodePoint))
        .collect()
}

pub fn label_to_ascii(label: &str) -> Result<String, PunycodeError> {
    if label.is_ascii() {
        return Ok(label.to_string());
    }
    Ok(format!("{}{}", ACE_PREFIX, encode(label)?))
}

pub fn label_to_unicode(label: &str) -> Result<String, PunycodeError> {
    match label.strip_prefix(ACE_PREFIX) {
        Some(rest) => decode(rest),
        None => Ok(label.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VECTORS: &[(&str, &str)] = &[

        ("他们为什么不说中文", "ihqwcrb4cv8a8dqg056pqjye"),

        (
            "почемужеонинеговорятпорусски",
            "b1abfaaepdrnnbgefbadotcwatmq2g4l",
        ),

        ("bücher", "bcher-kva"),
        ("münchen", "mnchen-3ya"),
        ("ü", "tda"),
    ];

    #[test]
    fn encode_kat_rfc3492() {
        for (u, p) in VECTORS {
            assert_eq!(&encode(u).unwrap(), p, "encode({u:?})");
        }
    }

    #[test]
    fn decode_kat_rfc3492() {
        for (u, p) in VECTORS {
            assert_eq!(&decode(p).unwrap(), u, "decode({p:?})");
        }
    }

    #[test]
    fn round_trip() {
        let samples = [
            "münchen",
            "bücher",
            "ドメイン名例",
            "правда",
            "δοκιμή",
            "mixedÜnicode123",
            "🦀rustacean🦀",
        ];
        for s in samples {
            let enc = encode(s).unwrap();
            assert_eq!(decode(&enc).unwrap(), s, "round-trip {s:?} via {enc:?}");
        }
    }

    #[test]
    fn all_ascii_appends_delimiter() {

        assert_eq!(encode("abc").unwrap(), "abc-");
        assert_eq!(decode("abc-").unwrap(), "abc");
    }

    #[test]
    fn ace_prefix_helpers() {
        assert_eq!(label_to_ascii("münchen").unwrap(), "xn--mnchen-3ya");
        assert_eq!(label_to_unicode("xn--mnchen-3ya").unwrap(), "münchen");

        assert_eq!(label_to_ascii("example").unwrap(), "example");
        assert_eq!(label_to_unicode("example").unwrap(), "example");
    }

    #[test]
    fn decode_rejects_non_ascii() {
        assert_eq!(decode("xÿz"), Err(PunycodeError::NonBasicInput));
    }

    #[test]
    fn decode_rejects_bad_digit() {

        assert!(matches!(
            decode("a-..").unwrap_err(),
            PunycodeError::InvalidDigit
        ));
    }

    #[test]
    fn decode_rejects_integer_overflow() {
        let mut input = String::from("a-");
        input.push_str(&"9".repeat(1024));
        input.push('a');
        assert_eq!(decode(&input), Err(PunycodeError::Overflow));
    }
}
