
use super::PgError;
use sql_core::SqlValue;

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    let is_cidr = oid == super::oid::CIDR;
    match parse(text, is_cidr) {
        Some(canon) => Ok(SqlValue::Text(canon)),
        None => Err(PgError::InvalidInputSyntax {
            typ: super::type_name(oid),
            input: text.to_string(),
        }),
    }
}

pub fn output(_oid: u32, v: &SqlValue) -> String {
    match v {
        SqlValue::Text(s) => s.clone(),
        _ => String::new(),
    }
}

enum Addr {
    V4([u8; 4]),
    V6([u16; 8]),
}

fn parse(text: &str, is_cidr: bool) -> Option<String> {

    let (addr_part, mask_part) = match text.split_once('/') {
        Some((a, m)) => {
            if m.contains('/') {
                return None;
            }
            (a, Some(m))
        }
        None => (text, None),
    };
    if addr_part.is_empty() {
        return None;
    }

    let is_v6 = addr_part.contains(':');
    let (addr, octet_count) = if is_v6 {
        (Addr::V6(parse_v6(addr_part)?), 0usize)
    } else {
        let (o, n) = parse_v4(addr_part)?;
        (Addr::V4(o), n)
    };

    let maxbits: u32 = if is_v6 { 128 } else { 32 };

    let bits: u32 = match mask_part {
        Some(m) => parse_mask(m, maxbits)?,
        None => match &addr {
            Addr::V4(o) => v4_default_bits(o[0], octet_count),
            Addr::V6(_) => 128,
        },
    };

    if is_cidr && !host_bits_zero(&addr, bits) {
        return None;
    }

    Some(match &addr {
        Addr::V4(o) => canon_v4(o, bits, is_cidr),
        Addr::V6(g) => canon_v6(g, bits, is_cidr),
    })
}

fn parse_mask(m: &str, maxbits: u32) -> Option<u32> {
    if m.is_empty() || !m.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let v: u32 = m.parse().ok()?;
    if v > maxbits {
        return None;
    }
    Some(v)
}

fn parse_v4(s: &str) -> Option<([u8; 4], usize)> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.is_empty() || parts.len() > 4 {
        return None;
    }
    let mut octets = [0u8; 4];
    for (i, p) in parts.iter().enumerate() {
        octets[i] = parse_octet(p)?;
    }
    Some((octets, parts.len()))
}

fn parse_v4_full(s: &str) -> Option<[u8; 4]> {
    let (o, n) = parse_v4(s)?;
    if n != 4 {
        return None;
    }
    Some(o)
}

fn parse_octet(p: &str) -> Option<u8> {
    if p.is_empty() || p.len() > 3 || !p.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let v: u32 = p.parse().ok()?;
    if v > 255 {
        return None;
    }
    Some(v as u8)
}

fn v4_default_bits(first: u8, octet_count: usize) -> u32 {
    let classful: u32 = if first >= 240 {
        32
    } else if first >= 224 {
        8
    } else if first >= 192 {
        24
    } else if first >= 128 {
        16
    } else {
        8
    };
    let by_octets = (octet_count as u32) * 8;
    if classful < by_octets {
        by_octets
    } else {
        classful
    }
}

fn parse_v6(s: &str) -> Option<[u16; 8]> {

    let work: String = if s.contains('.') {
        let colon = s.rfind(':')?;
        let v4 = parse_v4_full(&s[colon + 1..])?;
        let g1 = ((v4[0] as u16) << 8) | v4[1] as u16;
        let g2 = ((v4[2] as u16) << 8) | v4[3] as u16;
        format!("{}{:x}:{:x}", &s[..colon + 1], g1, g2)
    } else {
        s.to_string()
    };

    let mut groups = [0u16; 8];
    match work.find("::") {
        Some(idx) => {
            let left = &work[..idx];
            let right = &work[idx + 2..];
            if right.contains("::") {
                return None;
            }
            let lg = split_hex_groups(left)?;
            let rg = split_hex_groups(right)?;
            let total = lg.len() + rg.len();
            if total > 7 {
                return None;
            }
            for (i, g) in lg.iter().enumerate() {
                groups[i] = *g;
            }
            let start = 8 - rg.len();
            for (i, g) in rg.iter().enumerate() {
                groups[start + i] = *g;
            }
            Some(groups)
        }
        None => {
            let gs = split_hex_groups(&work)?;
            if gs.len() != 8 {
                return None;
            }
            for (i, g) in gs.iter().enumerate() {
                groups[i] = *g;
            }
            Some(groups)
        }
    }
}

fn split_hex_groups(s: &str) -> Option<Vec<u16>> {
    if s.is_empty() {
        return Some(vec![]);
    }
    let mut out = Vec::new();
    for tok in s.split(':') {
        if tok.is_empty() || tok.len() > 4 || !tok.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        out.push(u16::from_str_radix(tok, 16).ok()?);
    }
    Some(out)
}

fn host_bits_zero(addr: &Addr, bits: u32) -> bool {
    match addr {
        Addr::V4(o) => bytes_zero_right_of(o, bits),
        Addr::V6(g) => {
            let mut b = [0u8; 16];
            for i in 0..8 {
                b[2 * i] = (g[i] >> 8) as u8;
                b[2 * i + 1] = (g[i] & 0xff) as u8;
            }
            bytes_zero_right_of(&b, bits)
        }
    }
}

fn bytes_zero_right_of(bytes: &[u8], bits: u32) -> bool {
    let total = (bytes.len() as u32) * 8;
    for i in bits..total {
        let byte = bytes[(i / 8) as usize];
        let mask = 1u8 << (7 - (i % 8));
        if byte & mask != 0 {
            return false;
        }
    }
    true
}

fn canon_v4(o: &[u8; 4], bits: u32, is_cidr: bool) -> String {
    let base = format!("{}.{}.{}.{}", o[0], o[1], o[2], o[3]);
    if is_cidr || bits != 32 {
        format!("{base}/{bits}")
    } else {
        base
    }
}

fn canon_v6(g: &[u16; 8], bits: u32, is_cidr: bool) -> String {
    let base = v6_ntop(g);
    if is_cidr || bits != 128 {
        format!("{base}/{bits}")
    } else {
        base
    }
}

fn v6_ntop(w: &[u16; 8]) -> String {

    let (mut best_base, mut best_len): (i32, i32) = (-1, 0);
    let (mut cur_base, mut cur_len): (i32, i32) = (-1, 0);
    for i in 0..8i32 {
        if w[i as usize] == 0 {
            if cur_base == -1 {
                cur_base = i;
                cur_len = 1;
            } else {
                cur_len += 1;
            }
        } else if cur_base != -1 {
            if best_base == -1 || cur_len > best_len {
                best_base = cur_base;
                best_len = cur_len;
            }
            cur_base = -1;
        }
    }
    if cur_base != -1 && (best_base == -1 || cur_len > best_len) {
        best_base = cur_base;
        best_len = cur_len;
    }
    if best_base != -1 && best_len < 2 {
        best_base = -1;
    }

    let mut out = String::new();
    let mut i = 0i32;
    while i < 8 {
        if best_base != -1 && i >= best_base && i < best_base + best_len {
            if i == best_base {
                out.push(':');
            }
            i += 1;
            continue;
        }
        if i != 0 {
            out.push(':');
        }

        if i == 6
            && best_base == 0
            && (best_len == 6
                || (best_len == 7 && w[7] != 0x0001)
                || (best_len == 5 && w[5] == 0xffff))
        {
            out.push_str(&format!(
                "{}.{}.{}.{}",
                w[6] >> 8,
                w[6] & 0xff,
                w[7] >> 8,
                w[7] & 0xff
            ));
            break;
        }
        out.push_str(&format!("{:x}", w[i as usize]));
        i += 1;
    }
    if best_base != -1 && best_base + best_len == 8 {
        out.push(':');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const INET: u32 = super::super::oid::INET;
    const CIDR: u32 = super::super::oid::CIDR;

    fn ok(oid: u32, text: &str) -> String {
        match input(oid, text) {
            Ok(SqlValue::Text(s)) => s,
            other => panic!("expected Text for {text:?}, got {other:?}"),
        }
    }

    fn rejected(oid: u32, text: &str) {
        assert!(input(oid, text).is_err(), "expected rejection for {text:?}");
    }

    #[test]
    fn inet_v4_full_no_mask_omits_slash() {

        assert_eq!(ok(INET, "192.168.1.226"), "192.168.1.226");
        assert_eq!(ok(INET, "10.1.2.3"), "10.1.2.3");
    }

    #[test]
    fn inet_v4_with_mask_shows_slash_and_keeps_host_bits() {
        assert_eq!(ok(INET, "192.168.1.226/24"), "192.168.1.226/24");
        assert_eq!(ok(INET, "192.168.1.0/25"), "192.168.1.0/25");
        assert_eq!(ok(INET, "10.1.2.3/8"), "10.1.2.3/8");

        assert_eq!(ok(INET, "10.1.2.3/32"), "10.1.2.3");
    }

    #[test]
    fn inet_v4_abbreviated_zero_fills_and_uses_classful_default() {

        assert_eq!(ok(INET, "10.1.2"), "10.1.2.0/24");
        assert_eq!(ok(INET, "10.1"), "10.1.0.0/16");
        assert_eq!(ok(INET, "10"), "10.0.0.0/8");
    }

    #[test]
    fn cidr_v4_classful_default_masklen() {
        assert_eq!(ok(CIDR, "192.168.1"), "192.168.1.0/24");
        assert_eq!(ok(CIDR, "10"), "10.0.0.0/8");
        assert_eq!(ok(CIDR, "10.1"), "10.1.0.0/16");
        assert_eq!(ok(CIDR, "10.1.2"), "10.1.2.0/24");
    }

    #[test]
    fn cidr_v4_widen_default_to_octets() {

        assert_eq!(ok(CIDR, "10.0.0.0"), "10.0.0.0/32");
        assert_eq!(ok(CIDR, "10.1.2.3"), "10.1.2.3/32");
    }

    #[test]
    fn cidr_v4_always_shows_mask_even_32() {

        assert_eq!(ok(CIDR, "10.1.2.3/32"), "10.1.2.3/32");
    }

    #[test]
    fn cidr_v4_explicit_mask_kept() {
        assert_eq!(ok(CIDR, "192.168.1.0/26"), "192.168.1.0/26");
    }

    #[test]
    fn cidr_rejects_bits_right_of_mask() {

        rejected(CIDR, "192.168.1.2/30");
        rejected(CIDR, "ffff:ffff:ffff:ffff::/24");
        rejected(CIDR, "192.168.198.200/24");
    }

    #[test]
    fn inet_accepts_bits_right_of_mask() {

        assert_eq!(ok(INET, "192.168.1.2/30"), "192.168.1.2/30");
    }

    #[test]
    fn inet_v6_basic_and_compression() {
        assert_eq!(ok(INET, "10:23::f1/64"), "10:23::f1/64");
        assert_eq!(ok(INET, "10:23::ffff"), "10:23::ffff");
        assert_eq!(ok(INET, "::1"), "::1");
    }

    #[test]
    fn cidr_v6_defaults_128_and_shows_mask() {
        assert_eq!(ok(CIDR, "10:23::f1"), "10:23::f1/128");
        assert_eq!(ok(CIDR, "10:23::8000/113"), "10:23::8000/113");
    }

    #[test]
    fn v6_embedded_ipv4_notation() {

        assert_eq!(ok(CIDR, "::ffff:1.2.3.4"), "::ffff:1.2.3.4/128");
        assert_eq!(ok(INET, "::4.3.2.1/24"), "::4.3.2.1/24");
    }

    #[test]
    fn v6_lowercased_and_all_zero() {
        assert_eq!(ok(INET, "FF80::AB"), "ff80::ab");
        assert_eq!(ok(INET, "0000:0000:0000:0000:0000:0000:0000:0000"), "::");
        assert_eq!(ok(INET, "1:2:3:4:5:6:7:8"), "1:2:3:4:5:6:7:8");
    }

    #[test]
    fn masklen_bounds() {
        assert_eq!(ok(INET, "0.0.0.0/0"), "0.0.0.0/0");
        rejected(INET, "10.0.0.0/33");
        rejected(CIDR, "10.0.0.0/33");
        assert_eq!(ok(INET, "::/0"), "::/0");
        rejected(INET, "::1/129");
        rejected(INET, "10.0.0.0/x");
    }

    #[test]
    fn octet_bounds() {
        assert_eq!(ok(INET, "255.255.255.255"), "255.255.255.255");
        rejected(INET, "256.0.0.1");
        rejected(INET, "1.2.3.4.5");
        rejected(INET, "1.2.3.");
        rejected(INET, "1..2.3");
    }

    #[test]
    fn misc_rejects() {
        rejected(INET, "");
        rejected(CIDR, "1234::1234::1234");
        rejected(INET, "1234::1234::1234");
        rejected(INET, "abcz::1");
        rejected(INET, "1:2:3:4:5:6:7");
        rejected(INET, "10.0.0.0/12/8");
    }

    #[test]
    fn inet_vs_cidr_difference() {

        assert_eq!(ok(CIDR, "192.168.1"), "192.168.1.0/24");
        assert_eq!(ok(INET, "192.168.1"), "192.168.1.0/24");

        assert_eq!(ok(INET, "192.168.1.5"), "192.168.1.5");
        assert_eq!(ok(CIDR, "192.168.1.5"), "192.168.1.5/32");
    }

    #[test]
    fn round_trips() {
        for (oid, text) in [
            (INET, "192.168.1.226/24"),
            (INET, "10.1.2.3"),
            (INET, "10:23::f1/64"),
            (INET, "::4.3.2.1/24"),
            (CIDR, "192.168.1.0/26"),
            (CIDR, "10.0.0.0/8"),
            (CIDR, "::ffff:1.2.3.4/128"),
        ] {
            let first = ok(oid, text);
            let second = ok(oid, &first);
            assert_eq!(first, second, "not idempotent for {text:?}");
            assert_eq!(output(oid, &SqlValue::Text(first.clone())), first);
        }
    }

    #[test]
    fn output_non_text_is_empty() {
        assert_eq!(output(INET, &SqlValue::Null), "");
        assert_eq!(output(CIDR, &SqlValue::Int(5)), "");
    }
}
