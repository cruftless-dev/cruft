
use crate::types::PgError;
use sql_core::SqlValue;

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    match name {
        "host" | "masklen" | "network" | "netmask" | "broadcast" | "family" | "abbrev"
        | "hostmask" => {}
        _ => return None,
    }

    if args.len() != 1 {
        return Some(Err(bad(name)));
    }
    let text = match &args[0] {
        SqlValue::Null => return Some(Ok(SqlValue::Null)),
        SqlValue::Text(s) => s.as_str(),
        _ => return Some(Err(bad(name))),
    };
    let ip = match Ip::parse(text) {
        Some(ip) => ip,
        None => return Some(Err(bad(name))),
    };

    let out = match name {
        "masklen" => SqlValue::Int(ip.bits as i64),
        "family" => SqlValue::Int(if ip.v6 { 6 } else { 4 }),
        "host" => SqlValue::Text(ip.fmt(&ip.addr, Mode::AddrOnly)),
        "abbrev" => SqlValue::Text(ip.fmt(&ip.addr, Mode::Inet)),
        "netmask" => SqlValue::Text(ip.fmt(&ip.netmask(), Mode::AddrOnly)),
        "hostmask" => SqlValue::Text(ip.fmt(&ip.hostmask(), Mode::AddrOnly)),
        "network" => {
            let nm = ip.netmask();
            let mut b = ip.addr;
            for i in 0..ip.len {
                b[i] &= nm[i];
            }
            SqlValue::Text(ip.fmt(&b, Mode::Cidr))
        }
        "broadcast" => {
            let hm = ip.hostmask();
            let mut b = ip.addr;
            for i in 0..ip.len {
                b[i] |= hm[i];
            }
            SqlValue::Text(ip.fmt(&b, Mode::Inet))
        }
        _ => unreachable!(),
    };
    Some(Ok(out))
}

fn bad(name: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("function {name}(...) does not exist"),
    }
}

enum Mode {
    AddrOnly,
    Inet,
    Cidr,
}

struct Ip {
    v6: bool,
    len: usize,
    addr: [u8; 16],
    bits: u32,
}

impl Ip {
    fn maxbits(&self) -> u32 {
        (self.len as u32) * 8
    }

    fn netmask(&self) -> [u8; 16] {
        let mut m = [0u8; 16];
        for i in 0..self.maxbits() {
            if i < self.bits {
                m[(i / 8) as usize] |= 1u8 << (7 - (i % 8));
            }
        }
        m
    }

    fn hostmask(&self) -> [u8; 16] {
        let nm = self.netmask();
        let mut h = [0u8; 16];
        for i in 0..self.len {
            h[i] = !nm[i];
        }
        h
    }

    fn fmt(&self, bytes: &[u8; 16], mode: Mode) -> String {
        let base = if self.v6 {
            let mut g = [0u16; 8];
            for i in 0..8 {
                g[i] = ((bytes[2 * i] as u16) << 8) | bytes[2 * i + 1] as u16;
            }
            v6_ntop(&g)
        } else {
            format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3])
        };
        match mode {
            Mode::AddrOnly => base,
            Mode::Cidr => format!("{base}/{}", self.bits),
            Mode::Inet => {
                if self.bits == self.maxbits() {
                    base
                } else {
                    format!("{base}/{}", self.bits)
                }
            }
        }
    }

    fn parse(text: &str) -> Option<Ip> {
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
        let v6 = addr_part.contains(':');
        let mut addr = [0u8; 16];
        let (len, first_octet, octet_count) = if v6 {
            let g = parse_v6(addr_part)?;
            for i in 0..8 {
                addr[2 * i] = (g[i] >> 8) as u8;
                addr[2 * i + 1] = (g[i] & 0xff) as u8;
            }
            (16usize, 0u8, 0usize)
        } else {
            let (o, n) = parse_v4(addr_part)?;
            addr[..4].copy_from_slice(&o);
            (4usize, o[0], n)
        };
        let maxbits = (len as u32) * 8;
        let bits = match mask_part {
            Some(m) => parse_mask(m, maxbits)?,
            None if v6 => 128,
            None => v4_default_bits(first_octet, octet_count),
        };
        Some(Ip {
            v6,
            len,
            addr,
            bits,
        })
    }
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
            if lg.len() + rg.len() > 7 {
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

    fn t(name: &str, arg: &str) -> SqlValue {
        match super::call(name, &[SqlValue::Text(arg.to_string())]) {
            Some(Ok(v)) => v,
            other => panic!("expected Ok for {name}({arg:?}), got {other:?}"),
        }
    }
    fn s(v: SqlValue) -> String {
        match v {
            SqlValue::Text(s) => s,
            other => panic!("expected Text, got {other:?}"),
        }
    }
    fn n(v: SqlValue) -> i64 {
        match v {
            SqlValue::Int(i) => i,
            other => panic!("expected Int, got {other:?}"),
        }
    }

    #[test]
    fn v4_slash24() {
        assert_eq!(s(t("host", "192.168.1.226/24")), "192.168.1.226");
        assert_eq!(n(t("masklen", "192.168.1.226/24")), 24);
        assert_eq!(n(t("family", "192.168.1.226/24")), 4);
        assert_eq!(s(t("network", "192.168.1.226/24")), "192.168.1.0/24");
        assert_eq!(s(t("broadcast", "192.168.1.226/24")), "192.168.1.255/24");
        assert_eq!(s(t("abbrev", "192.168.1.226/24")), "192.168.1.226/24");
    }

    #[test]
    fn v4_netmask_hostmask() {

        assert_eq!(s(t("netmask", "192.168.1.5/24")), "255.255.255.0");
        assert_eq!(s(t("hostmask", "192.168.1.5/24")), "0.0.0.255");
        assert_eq!(s(t("netmask", "10.0.0.0/8")), "255.0.0.0");
        assert_eq!(s(t("hostmask", "10.0.0.0/8")), "0.255.255.255");
    }

    #[test]
    fn v4_slash32_edge() {
        assert_eq!(n(t("masklen", "10.1.2.3")), 32);
        assert_eq!(s(t("host", "10.1.2.3")), "10.1.2.3");
        assert_eq!(s(t("network", "10.1.2.3")), "10.1.2.3/32");
        assert_eq!(s(t("broadcast", "10.1.2.3")), "10.1.2.3");
        assert_eq!(s(t("abbrev", "10.1.2.3")), "10.1.2.3");
        assert_eq!(s(t("netmask", "10.1.2.3")), "255.255.255.255");
        assert_eq!(s(t("hostmask", "10.1.2.3")), "0.0.0.0");
    }

    #[test]
    fn v6_slash64() {
        assert_eq!(n(t("family", "10:23::f1/64")), 6);
        assert_eq!(n(t("masklen", "10:23::f1/64")), 64);
        assert_eq!(s(t("host", "10:23::f1/64")), "10:23::f1");
        assert_eq!(s(t("network", "10:23::f1/64")), "10:23::/64");
        assert_eq!(
            s(t("broadcast", "10:23::f1/64")),
            "10:23::ffff:ffff:ffff:ffff/64"
        );
        assert_eq!(s(t("netmask", "10:23::f1/64")), "ffff:ffff:ffff:ffff::");
    }

    #[test]
    fn null_propagates() {
        for name in [
            "host",
            "masklen",
            "network",
            "netmask",
            "broadcast",
            "family",
            "abbrev",
            "hostmask",
        ] {
            assert!(matches!(
                super::call(name, &[SqlValue::Null]),
                Some(Ok(SqlValue::Null))
            ));
        }
    }

    #[test]
    fn unclaimed_name_is_none() {
        assert!(super::call("set_masklen", &[SqlValue::Null]).is_none());
        assert!(super::call("sqrt", &[SqlValue::Int(4)]).is_none());
    }

    #[test]
    fn wrong_arity_and_type() {
        assert!(matches!(super::call("host", &[]), Some(Err(_))));
        assert!(matches!(
            super::call("masklen", &[SqlValue::Int(5)]),
            Some(Err(_))
        ));
        assert!(matches!(
            super::call(
                "host",
                &[SqlValue::Text("192.168.1.1".into()), SqlValue::Null]
            ),
            Some(Err(_))
        ));
    }
}
