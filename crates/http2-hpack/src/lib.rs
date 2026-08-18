
pub fn encode_integer(value: u64, n: u8, prefix_bits: u8) -> Vec<u8> {
    let max_prefix = (1u64 << n) - 1;
    let mut out = Vec::new();
    if value < max_prefix {
        out.push(prefix_bits | value as u8);
        return out;
    }
    out.push(prefix_bits | max_prefix as u8);
    let mut v = value - max_prefix;
    while v >= 128 {
        out.push(((v % 128) as u8) | 0x80);
        v /= 128;
    }
    out.push(v as u8);
    out
}

pub fn decode_integer(buf: &[u8], n: u8) -> Option<(u64, usize)> {
    let max_prefix = (1u64 << n) - 1;
    let first = *buf.first()? as u64 & max_prefix;
    if first < max_prefix {
        return Some((first, 1));
    }
    let mut value = max_prefix;
    let mut m = 0u32;
    let mut i = 1;
    loop {
        let b = *buf.get(i)? as u64;
        i += 1;
        value += (b & 0x7f) << m;
        m += 7;
        if b & 0x80 == 0 {
            break;
        }
        if m > 63 {
            return None;
        }
    }
    Some((value, i))
}

pub fn encode_string(s: &[u8]) -> Vec<u8> {
    let mut out = encode_integer(s.len() as u64, 7, 0x00);
    out.extend_from_slice(s);
    out
}

pub fn decode_string(buf: &[u8]) -> Option<(Vec<u8>, usize)> {
    let huffman = buf.first()? & 0x80 != 0;
    let (len, n) = decode_integer(buf, 7)?;
    let len = len as usize;
    let end = n + len;
    if buf.len() < end {
        return None;
    }
    if huffman {
        return huffman::decode(&buf[n..end]).map(|s| (s, end));
    }
    Some((buf[n..end].to_vec(), end))
}

pub mod huffman {

    pub(super) const TABLE: [(u32, u8); 256] = [
        (0x1ff8, 13),
        (0x7fffd8, 23),
        (0xfffffe2, 28),
        (0xfffffe3, 28),
        (0xfffffe4, 28),
        (0xfffffe5, 28),
        (0xfffffe6, 28),
        (0xfffffe7, 28),
        (0xfffffe8, 28),
        (0xffffea, 24),
        (0x3ffffffc, 30),
        (0xfffffe9, 28),
        (0xfffffea, 28),
        (0x3ffffffd, 30),
        (0xfffffeb, 28),
        (0xfffffec, 28),
        (0xfffffed, 28),
        (0xfffffee, 28),
        (0xfffffef, 28),
        (0xffffff0, 28),
        (0xffffff1, 28),
        (0xffffff2, 28),
        (0x3ffffffe, 30),
        (0xffffff3, 28),
        (0xffffff4, 28),
        (0xffffff5, 28),
        (0xffffff6, 28),
        (0xffffff7, 28),
        (0xffffff8, 28),
        (0xffffff9, 28),
        (0xffffffa, 28),
        (0xffffffb, 28),
        (0x14, 6),
        (0x3f8, 10),
        (0x3f9, 10),
        (0xffa, 12),
        (0x1ff9, 13),
        (0x15, 6),
        (0xf8, 8),
        (0x7fa, 11),
        (0x3fa, 10),
        (0x3fb, 10),
        (0xf9, 8),
        (0x7fb, 11),
        (0xfa, 8),
        (0x16, 6),
        (0x17, 6),
        (0x18, 6),
        (0x0, 5),
        (0x1, 5),
        (0x2, 5),
        (0x19, 6),
        (0x1a, 6),
        (0x1b, 6),
        (0x1c, 6),
        (0x1d, 6),
        (0x1e, 6),
        (0x1f, 6),
        (0x5c, 7),
        (0xfb, 8),
        (0x7ffc, 15),
        (0x20, 6),
        (0xffb, 12),
        (0x3fc, 10),
        (0x1ffa, 13),
        (0x21, 6),
        (0x5d, 7),
        (0x5e, 7),
        (0x5f, 7),
        (0x60, 7),
        (0x61, 7),
        (0x62, 7),
        (0x63, 7),
        (0x64, 7),
        (0x65, 7),
        (0x66, 7),
        (0x67, 7),
        (0x68, 7),
        (0x69, 7),
        (0x6a, 7),
        (0x6b, 7),
        (0x6c, 7),
        (0x6d, 7),
        (0x6e, 7),
        (0x6f, 7),
        (0x70, 7),
        (0x71, 7),
        (0x72, 7),
        (0xfc, 8),
        (0x73, 7),
        (0xfd, 8),
        (0x1ffb, 13),
        (0x7fff0, 19),
        (0x1ffc, 13),
        (0x3ffc, 14),
        (0x22, 6),
        (0x7ffd, 15),
        (0x3, 5),
        (0x23, 6),
        (0x4, 5),
        (0x24, 6),
        (0x5, 5),
        (0x25, 6),
        (0x26, 6),
        (0x27, 6),
        (0x6, 5),
        (0x74, 7),
        (0x75, 7),
        (0x28, 6),
        (0x29, 6),
        (0x2a, 6),
        (0x7, 5),
        (0x2b, 6),
        (0x76, 7),
        (0x2c, 6),
        (0x8, 5),
        (0x9, 5),
        (0x2d, 6),
        (0x77, 7),
        (0x78, 7),
        (0x79, 7),
        (0x7a, 7),
        (0x7b, 7),
        (0x7ffe, 15),
        (0x7fc, 11),
        (0x3ffd, 14),
        (0x1ffd, 13),
        (0xffffffc, 28),
        (0xfffe6, 20),
        (0x3fffd2, 22),
        (0xfffe7, 20),
        (0xfffe8, 20),
        (0x3fffd3, 22),
        (0x3fffd4, 22),
        (0x3fffd5, 22),
        (0x7fffd9, 23),
        (0x3fffd6, 22),
        (0x7fffda, 23),
        (0x7fffdb, 23),
        (0x7fffdc, 23),
        (0x7fffdd, 23),
        (0x7fffde, 23),
        (0xffffeb, 24),
        (0x7fffdf, 23),
        (0xffffec, 24),
        (0xffffed, 24),
        (0x3fffd7, 22),
        (0x7fffe0, 23),
        (0xffffee, 24),
        (0x7fffe1, 23),
        (0x7fffe2, 23),
        (0x7fffe3, 23),
        (0x7fffe4, 23),
        (0x1fffdc, 21),
        (0x3fffd8, 22),
        (0x7fffe5, 23),
        (0x3fffd9, 22),
        (0x7fffe6, 23),
        (0x7fffe7, 23),
        (0xffffef, 24),
        (0x3fffda, 22),
        (0x1fffdd, 21),
        (0xfffe9, 20),
        (0x3fffdb, 22),
        (0x3fffdc, 22),
        (0x7fffe8, 23),
        (0x7fffe9, 23),
        (0x1fffde, 21),
        (0x7fffea, 23),
        (0x3fffdd, 22),
        (0x3fffde, 22),
        (0xfffff0, 24),
        (0x1fffdf, 21),
        (0x3fffdf, 22),
        (0x7fffeb, 23),
        (0x7fffec, 23),
        (0x1fffe0, 21),
        (0x1fffe1, 21),
        (0x3fffe0, 22),
        (0x1fffe2, 21),
        (0x7fffed, 23),
        (0x3fffe1, 22),
        (0x7fffee, 23),
        (0x7fffef, 23),
        (0xfffea, 20),
        (0x3fffe2, 22),
        (0x3fffe3, 22),
        (0x3fffe4, 22),
        (0x7ffff0, 23),
        (0x3fffe5, 22),
        (0x3fffe6, 22),
        (0x7ffff1, 23),
        (0x3ffffe0, 26),
        (0x3ffffe1, 26),
        (0xfffeb, 20),
        (0x7fff1, 19),
        (0x3fffe7, 22),
        (0x7ffff2, 23),
        (0x3fffe8, 22),
        (0x1ffffec, 25),
        (0x3ffffe2, 26),
        (0x3ffffe3, 26),
        (0x3ffffe4, 26),
        (0x7ffffde, 27),
        (0x7ffffdf, 27),
        (0x3ffffe5, 26),
        (0xfffff1, 24),
        (0x1ffffed, 25),
        (0x7fff2, 19),
        (0x1fffe3, 21),
        (0x3ffffe6, 26),
        (0x7ffffe0, 27),
        (0x7ffffe1, 27),
        (0x3ffffe7, 26),
        (0x7ffffe2, 27),
        (0xfffff2, 24),
        (0x1fffe4, 21),
        (0x1fffe5, 21),
        (0x3ffffe8, 26),
        (0x3ffffe9, 26),
        (0xffffffd, 28),
        (0x7ffffe3, 27),
        (0x7ffffe4, 27),
        (0x7ffffe5, 27),
        (0xfffec, 20),
        (0xfffff3, 24),
        (0xfffed, 20),
        (0x1fffe6, 21),
        (0x3fffe9, 22),
        (0x1fffe7, 21),
        (0x1fffe8, 21),
        (0x7ffff3, 23),
        (0x3fffea, 22),
        (0x3fffeb, 22),
        (0x1ffffee, 25),
        (0x1ffffef, 25),
        (0xfffff4, 24),
        (0xfffff5, 24),
        (0x3ffffea, 26),
        (0x7ffff4, 23),
        (0x3ffffeb, 26),
        (0x7ffffe6, 27),
        (0x3ffffec, 26),
        (0x3ffffed, 26),
        (0x7ffffe7, 27),
        (0x7ffffe8, 27),
        (0x7ffffe9, 27),
        (0x7ffffea, 27),
        (0x7ffffeb, 27),
        (0xffffffe, 28),
        (0x7ffffec, 27),
        (0x7ffffed, 27),
        (0x7ffffee, 27),
        (0x7ffffef, 27),
        (0x7fffff0, 27),
        (0x3ffffee, 26),
    ];

    pub fn encode(input: &[u8]) -> Vec<u8> {
        let mut bits: u64 = 0;
        let mut nbits: u32 = 0;
        let mut out = Vec::new();
        for &b in input {
            let (code, len) = TABLE[b as usize];
            bits = (bits << len) | code as u64;
            nbits += len as u32;
            while nbits >= 8 {
                nbits -= 8;
                out.push((bits >> nbits) as u8);
            }
        }
        if nbits > 0 {
            let pad = 8 - nbits;
            bits = (bits << pad) | ((1u64 << pad) - 1);
            out.push(bits as u8);
        }
        out
    }

    pub fn decode(input: &[u8]) -> Option<Vec<u8>> {
        let mut map = std::collections::HashMap::with_capacity(256);
        for (sym, &(code, len)) in TABLE.iter().enumerate() {
            map.insert((code, len), sym as u8);
        }
        let mut out = Vec::new();
        let mut code: u32 = 0;
        let mut len: u8 = 0;
        for &byte in input {
            for bit in (0..8).rev() {
                code = (code << 1) | ((byte >> bit) & 1) as u32;
                len += 1;
                if let Some(&sym) = map.get(&(code, len)) {
                    out.push(sym);
                    code = 0;
                    len = 0;
                } else if len > 30 {
                    return None;
                }
            }
        }

        if len >= 8 {
            return None;
        }
        if len > 0 {
            let mask = (1u32 << len) - 1;
            if code & mask != mask {
                return None;
            }
        }
        Some(out)
    }
}

pub const STATIC_TABLE: &[(&str, &str)] = &[
    (":authority", ""),
    (":method", "GET"),
    (":method", "POST"),
    (":path", "/"),
    (":path", "/index.html"),
    (":scheme", "http"),
    (":scheme", "https"),
    (":status", "200"),
    (":status", "204"),
    (":status", "206"),
    (":status", "304"),
    (":status", "400"),
    (":status", "404"),
    (":status", "500"),
    ("accept-charset", ""),
    ("accept-encoding", "gzip, deflate"),
    ("accept-language", ""),
    ("accept-ranges", ""),
    ("accept", ""),
    ("access-control-allow-origin", ""),
    ("age", ""),
    ("allow", ""),
    ("authorization", ""),
    ("cache-control", ""),
    ("content-disposition", ""),
    ("content-encoding", ""),
    ("content-language", ""),
    ("content-length", ""),
    ("content-location", ""),
    ("content-range", ""),
    ("content-type", ""),
    ("cookie", ""),
    ("date", ""),
    ("etag", ""),
    ("expect", ""),
    ("expires", ""),
    ("from", ""),
    ("host", ""),
    ("if-match", ""),
    ("if-modified-since", ""),
    ("if-none-match", ""),
    ("if-range", ""),
    ("if-unmodified-since", ""),
    ("last-modified", ""),
    ("link", ""),
    ("location", ""),
    ("max-forwards", ""),
    ("proxy-authenticate", ""),
    ("proxy-authorization", ""),
    ("range", ""),
    ("referer", ""),
    ("refresh", ""),
    ("retry-after", ""),
    ("server", ""),
    ("set-cookie", ""),
    ("strict-transport-security", ""),
    ("transfer-encoding", ""),
    ("user-agent", ""),
    ("vary", ""),
    ("via", ""),
    ("www-authenticate", ""),
];

#[derive(Debug, Clone)]
pub struct DynamicTable {
    entries: std::collections::VecDeque<(String, String)>,
    size: usize,
    max_size: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeLimits {
    pub max_header_list_size: usize,
    pub max_dynamic_table_size: usize,
}

impl Default for DecodeLimits {
    fn default() -> Self {
        DecodeLimits {
            max_header_list_size: 65_536,
            max_dynamic_table_size: 4_096,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    Malformed,
    HeaderListTooLarge,
    DynamicTableSizeUpdateTooLarge,
}

impl DynamicTable {
    pub fn new(max_size: usize) -> Self {
        DynamicTable {
            entries: std::collections::VecDeque::new(),
            size: 0,
            max_size,
        }
    }
    fn entry_size(name: &str, value: &str) -> usize {
        name.len() + value.len() + 32
    }
    pub fn insert(&mut self, name: &str, value: &str) {
        let es = Self::entry_size(name, value);
        self.entries
            .push_front((name.to_string(), value.to_string()));
        self.size += es;
        self.evict();
    }
    pub fn set_max_size(&mut self, max: usize) {
        self.max_size = max;
        self.evict();
    }
    fn evict(&mut self) {
        while self.size > self.max_size {
            if let Some((n, v)) = self.entries.pop_back() {
                self.size -= Self::entry_size(&n, &v);
            } else {
                break;
            }
        }
    }

    fn get(&self, index: usize) -> Option<(String, String)> {
        if index == 0 {
            return None;
        }
        if index <= STATIC_TABLE.len() {
            let (n, v) = STATIC_TABLE[index - 1];
            return Some((n.to_string(), v.to_string()));
        }
        let dyn_idx = index - STATIC_TABLE.len() - 1;
        self.entries.get(dyn_idx).cloned()
    }
}

pub fn decode_header_block(
    block: &[u8],
    table: &mut DynamicTable,
) -> Option<Vec<(String, String)>> {
    decode_header_block_limited(block, table, DecodeLimits::default()).ok()
}

pub fn decode_header_block_limited(
    block: &[u8],
    table: &mut DynamicTable,
    limits: DecodeLimits,
) -> Result<Vec<(String, String)>, DecodeError> {
    let mut out = Vec::new();
    let mut decoded_size = 0usize;
    let mut p = block;
    while !p.is_empty() {
        let b0 = p[0];
        if b0 & 0x80 != 0 {

            let (idx, n) = decode_integer(p, 7).ok_or(DecodeError::Malformed)?;
            let (name, value) = table.get(idx as usize).ok_or(DecodeError::Malformed)?;
            decoded_size = checked_header_list_size(decoded_size, &name, &value, limits)?;
            out.push((name, value));
            p = &p[n..];
        } else if b0 & 0x40 != 0 {

            let (name, value, used) = decode_literal(p, 6, table).ok_or(DecodeError::Malformed)?;
            decoded_size = checked_header_list_size(decoded_size, &name, &value, limits)?;
            table.insert(&name, &value);
            out.push((name, value));
            p = &p[used..];
        } else if b0 & 0x20 != 0 {

            let (max, n) = decode_integer(p, 5).ok_or(DecodeError::Malformed)?;
            if max as usize > limits.max_dynamic_table_size {
                return Err(DecodeError::DynamicTableSizeUpdateTooLarge);
            }
            table.set_max_size(max as usize);
            p = &p[n..];
        } else {

            let (name, value, used) = decode_literal(p, 4, table).ok_or(DecodeError::Malformed)?;
            decoded_size = checked_header_list_size(decoded_size, &name, &value, limits)?;
            out.push((name, value));
            p = &p[used..];
        }
    }
    Ok(out)
}

fn checked_header_list_size(
    current: usize,
    name: &str,
    value: &str,
    limits: DecodeLimits,
) -> Result<usize, DecodeError> {
    let next = current
        .checked_add(name.len())
        .and_then(|n| n.checked_add(value.len()))
        .and_then(|n| n.checked_add(32))
        .ok_or(DecodeError::HeaderListTooLarge)?;
    if next > limits.max_header_list_size {
        return Err(DecodeError::HeaderListTooLarge);
    }
    Ok(next)
}

fn decode_literal(
    p: &[u8],
    prefix_bits: u8,
    table: &DynamicTable,
) -> Option<(String, String, usize)> {
    let (name_idx, n) = decode_integer(p, prefix_bits)?;
    let mut used = n;
    let name = if name_idx == 0 {
        let (nb, ns) = decode_string(&p[used..])?;
        used += ns;
        String::from_utf8(nb).ok()?
    } else {
        table.get(name_idx as usize)?.0
    };
    let (vb, vs) = decode_string(&p[used..])?;
    used += vs;
    let value = String::from_utf8(vb).ok()?;
    Some((name, value, used))
}

pub fn encode_header_block(headers: &[(String, String)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (name, value) in headers {

        if let Some(i) = STATIC_TABLE
            .iter()
            .position(|(n, v)| *n == name.as_str() && *v == value.as_str())
        {
            out.extend_from_slice(&encode_integer((i + 1) as u64, 7, 0x80));
            continue;
        }

        let name_idx = STATIC_TABLE.iter().position(|(n, _)| *n == name.as_str());
        match name_idx {
            Some(i) => out.extend_from_slice(&encode_integer((i + 1) as u64, 4, 0x00)),
            None => {
                out.extend_from_slice(&encode_integer(0, 4, 0x00));
                out.extend_from_slice(&encode_string(name.as_bytes()));
            }
        }
        out.extend_from_slice(&encode_string(value.as_bytes()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_round_trips() {

        assert_eq!(encode_integer(10, 5, 0x00), vec![0x0a]);
        assert_eq!(decode_integer(&[0x0a], 5), Some((10, 1)));
        assert_eq!(encode_integer(1337, 5, 0x00), vec![0x1f, 0x9a, 0x0a]);
        assert_eq!(decode_integer(&[0x1f, 0x9a, 0x0a], 5), Some((1337, 3)));

        assert_eq!(
            decode_integer(&encode_integer(42, 8, 0x00), 8),
            Some((42, 1))
        );
    }

    #[test]
    fn string_round_trips() {
        let (s, n) = decode_string(&encode_string(b"custom-key")).unwrap();
        assert_eq!(s, b"custom-key");
        assert_eq!(n, 1 + 10);
    }

    #[test]
    fn huffman_round_trips_and_rfc_vector() {

        let encoded = [
            0xf1, 0xe3, 0xc2, 0xe5, 0xf2, 0x3a, 0x6b, 0xa0, 0xab, 0x90, 0xf4, 0xff,
        ];
        assert_eq!(huffman::encode(b"www.example.com"), encoded);
        assert_eq!(huffman::decode(&encoded).unwrap(), b"www.example.com");

        assert_eq!(
            huffman::decode(&huffman::encode(b"no-cache")).unwrap(),
            b"no-cache"
        );

        for s in [
            &b"custom-key"[..],
            b"/sample/path",
            b":status",
            b"200",
            b"!@#$%^&*()_+",
            &[0u8, 255, 128, 10],
        ] {
            assert_eq!(huffman::decode(&huffman::encode(s)).unwrap(), s);
        }

        let mut lit = encode_integer(encoded.len() as u64, 7, 0x80);
        lit.extend_from_slice(&encoded);
        let (s, _) = decode_string(&lit).unwrap();
        assert_eq!(s, b"www.example.com");
    }

    #[test]
    fn decode_rfc_c21_literal_with_indexing() {

        let block = [
            0x40, 0x0a, b'c', b'u', b's', b't', b'o', b'm', b'-', b'k', b'e', b'y', 0x0d, b'c',
            b'u', b's', b't', b'o', b'm', b'-', b'h', b'e', b'a', b'd', b'e', b'r',
        ];
        let mut t = DynamicTable::new(4096);
        let hs = decode_header_block(&block, &mut t).unwrap();
        assert_eq!(
            hs,
            vec![("custom-key".to_string(), "custom-header".to_string())]
        );

        assert_eq!(
            t.get(STATIC_TABLE.len() + 1),
            Some(("custom-key".into(), "custom-header".into()))
        );
    }

    #[test]
    fn decode_rfc_c23_indexed_and_path() {

        let block = [0x82, 0x85];
        let mut t = DynamicTable::new(4096);
        let hs = decode_header_block(&block, &mut t).unwrap();
        assert_eq!(
            hs,
            vec![
                (":method".to_string(), "GET".to_string()),
                (":path".to_string(), "/index.html".to_string())
            ]
        );
    }

    #[test]
    fn encode_then_decode_response_headers() {
        let headers = vec![
            (":status".to_string(), "200".to_string()),
            ("content-type".to_string(), "text/plain".to_string()),
            ("x-custom".to_string(), "hello".to_string()),
        ];
        let block = encode_header_block(&headers);
        let mut t = DynamicTable::new(4096);
        let decoded = decode_header_block(&block, &mut t).unwrap();
        assert_eq!(decoded, headers);
    }

    #[test]
    fn decode_rejects_header_list_expansion_past_limit() {
        let block = encode_header_block(&[
            ("x-a".to_string(), "1234567890".to_string()),
            ("x-b".to_string(), "1234567890".to_string()),
        ]);
        let mut t = DynamicTable::new(4096);
        assert_eq!(
            decode_header_block_limited(
                &block,
                &mut t,
                DecodeLimits {
                    max_header_list_size: 40,
                    max_dynamic_table_size: 4096,
                },
            ),
            Err(DecodeError::HeaderListTooLarge)
        );
    }

    #[test]
    fn decode_rejects_dynamic_table_size_update_above_advertised_limit() {
        let update = encode_integer(8192, 5, 0x20);
        let mut t = DynamicTable::new(4096);
        assert_eq!(
            decode_header_block_limited(
                &update,
                &mut t,
                DecodeLimits {
                    max_header_list_size: 65_536,
                    max_dynamic_table_size: 4096,
                },
            ),
            Err(DecodeError::DynamicTableSizeUpdateTooLarge)
        );
    }
}
