
fn parse_ipv4_number(part: &str) -> Option<u64> {
    if part.is_empty() {
        return None;
    }
    let mut s = part;
    let mut radix = 10u32;
    if s.len() >= 2 && (s.starts_with("0x") || s.starts_with("0X")) {
        s = &s[2..];
        radix = 16;
    } else if s.len() >= 2 && s.starts_with('0') {
        s = &s[1..];
        radix = 8;
    }
    if s.is_empty() {
        return Some(0);
    }
    let mut n: u64 = 0;
    for ch in s.chars() {
        let d = ch.to_digit(radix)?;
        n = n.checked_mul(radix as u64)?.checked_add(d as u64)?;
        if n > 0xFFFF_FFFF {

            n = 0x1_0000_0000;
        }
    }
    Some(n)
}

pub fn ends_in_number(input: &str) -> bool {
    let mut parts: Vec<&str> = input.split('.').collect();
    if parts.last() == Some(&"") && parts.len() > 1 {
        parts.pop();
    }
    let last = match parts.last() {
        Some(l) => *l,
        None => return false,
    };
    if last.is_empty() {
        return false;
    }
    if last.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    parse_ipv4_number(last).is_some() && (last.starts_with("0x") || last.starts_with("0X"))
}

pub fn domain_to_ascii(input: &str) -> Result<String, ()> {

    rusty_js_idna::to_ascii_url(input).map_err(|_| ())
}

pub fn parse_ipv4(input: &str) -> Option<u32> {
    let mut parts: Vec<&str> = input.split('.').collect();
    if parts.last() == Some(&"") && parts.len() > 1 {
        parts.pop();
    }
    if parts.len() > 4 {
        return None;
    }
    let mut numbers: Vec<u64> = Vec::with_capacity(parts.len());
    for part in &parts {
        numbers.push(parse_ipv4_number(part)?);
    }

    let count = numbers.len();
    for n in &numbers[..count - 1] {
        if *n > 255 {
            return None;
        }
    }
    let max_last = 256u64.checked_pow((5 - count) as u32)?;
    if numbers[count - 1] >= max_last {
        return None;
    }
    let mut ipv4 = numbers[count - 1] as u32;
    let mut counter = 0u32;
    for n in &numbers[..count - 1] {
        ipv4 = ipv4.wrapping_add((*n as u32).wrapping_mul(256u32.pow(3 - counter)));
        counter += 1;
    }
    Some(ipv4)
}

pub fn serialize_ipv4(addr: u32) -> String {
    format!(
        "{}.{}.{}.{}",
        (addr >> 24) & 0xff,
        (addr >> 16) & 0xff,
        (addr >> 8) & 0xff,
        addr & 0xff
    )
}

pub fn parse_ipv6(input: &str) -> Option<[u16; 8]> {
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut address = [0u16; 8];
    let mut piece_index = 0usize;
    let mut compress: Option<usize> = None;
    let mut p = 0usize;

    if p < len && chars[p] == ':' {
        if p + 1 >= len || chars[p + 1] != ':' {
            return None;
        }
        p += 2;
        piece_index += 1;
        compress = Some(piece_index);
    }

    while p < len {
        if piece_index == 8 {
            return None;
        }
        if chars[p] == ':' {
            if compress.is_some() {
                return None;
            }
            p += 1;
            piece_index += 1;
            compress = Some(piece_index);
            continue;
        }
        let mut value: u32 = 0;
        let mut length = 0;
        while length < 4 && p < len && chars[p].is_ascii_hexdigit() {
            value = value * 16 + chars[p].to_digit(16).unwrap();
            p += 1;
            length += 1;
        }
        if p < len && chars[p] == '.' {
            if length == 0 {
                return None;
            }
            p -= length;
            if piece_index > 6 {
                return None;
            }
            let mut numbers_seen = 0;
            while p < len {
                let mut ipv4_piece: Option<u32> = None;
                if numbers_seen > 0 {
                    if chars[p] == '.' && numbers_seen < 4 {
                        p += 1;
                    } else {
                        return None;
                    }
                }
                if p >= len || !chars[p].is_ascii_digit() {
                    return None;
                }
                while p < len && chars[p].is_ascii_digit() {
                    let n = chars[p].to_digit(10).unwrap();
                    ipv4_piece = match ipv4_piece {
                        None => Some(n),
                        Some(0) => return None,
                        Some(v) => Some(v * 10 + n),
                    };
                    if ipv4_piece.unwrap() > 255 {
                        return None;
                    }
                    p += 1;
                }
                address[piece_index] = address[piece_index] * 0x100 + ipv4_piece.unwrap() as u16;
                numbers_seen += 1;
                if numbers_seen == 2 || numbers_seen == 4 {
                    piece_index += 1;
                }
            }
            if numbers_seen != 4 {
                return None;
            }
            break;
        } else if p < len && chars[p] == ':' {
            p += 1;
            if p >= len {
                return None;
            }
        } else if p < len {
            return None;
        }
        address[piece_index] = value as u16;
        piece_index += 1;
    }

    if let Some(c) = compress {
        let mut swaps = piece_index as isize - c as isize;
        let mut pi = 7isize;
        while pi != 0 && swaps > 0 {
            address.swap(pi as usize, (c as isize + swaps - 1) as usize);
            pi -= 1;
            swaps -= 1;
        }
    } else if piece_index != 8 {
        return None;
    }
    Some(address)
}

pub fn serialize_ipv6(address: &[u16; 8]) -> String {

    let mut compress: Option<usize> = None;
    let mut best_len = 1usize;
    let mut cur_start: Option<usize> = None;
    let mut cur_len = 0usize;
    for i in 0..8 {
        if address[i] == 0 {
            if cur_start.is_none() {
                cur_start = Some(i);
                cur_len = 0;
            }
            cur_len += 1;
            if cur_len > best_len {
                best_len = cur_len;
                compress = cur_start;
            }
        } else {
            cur_start = None;
            cur_len = 0;
        }
    }
    let mut out = String::from("[");
    let mut ignore0 = false;
    for piece_index in 0..8usize {
        if ignore0 && address[piece_index] == 0 {
            continue;
        } else if ignore0 {
            ignore0 = false;
        }
        if compress == Some(piece_index) {
            out.push_str(if piece_index == 0 { "::" } else { ":" });
            ignore0 = true;
            continue;
        }
        out.push_str(&format!("{:x}", address[piece_index]));
        if piece_index != 7 {
            out.push(':');
        }
    }
    out.push(']');
    out
}
