
const STATIC_TABLE: &[(&str, &str)] = &[
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

const HUFFMAN_TABLE: [(u32, u8); 257] = [
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
    (0x3fffffff, 30),
];

const EOS_SYMBOL: usize = 256;

struct HuffNode {

    children: [usize; 2],

    sym: Option<usize>,
}

impl HuffNode {
    fn empty() -> Self {
        HuffNode {
            children: [usize::MAX, usize::MAX],
            sym: None,
        }
    }
}

fn build_huffman_tree() -> Vec<HuffNode> {
    let mut nodes: Vec<HuffNode> = vec![HuffNode::empty()];
    for (sym, &(code, len)) in HUFFMAN_TABLE.iter().enumerate() {
        let mut cur = 0usize;
        for i in (0..len).rev() {
            let bit = ((code >> i) & 1) as usize;
            let next = nodes[cur].children[bit];
            if next == usize::MAX {
                nodes.push(HuffNode::empty());
                let idx = nodes.len() - 1;
                nodes[cur].children[bit] = idx;
                cur = idx;
            } else {
                cur = next;
            }
        }
        nodes[cur].sym = Some(sym);
    }
    nodes
}

fn huffman_decode(input: &[u8]) -> Result<Vec<u8>, String> {
    let tree = build_huffman_tree();
    let mut out = Vec::new();
    let mut node = 0usize;
    let mut padding_only = true;

    for &byte in input {
        for i in (0..8).rev() {
            let bit = ((byte >> i) & 1) as usize;

            let next = tree[node].children[bit];
            if next == usize::MAX {
                return Err("hpack: invalid huffman code path".to_string());
            }
            node = next;
            if let Some(sym) = tree[node].sym {
                if sym == EOS_SYMBOL {
                    return Err("hpack: huffman EOS symbol decoded".to_string());
                }
                out.push(sym as u8);
                node = 0;
                padding_only = true;
            } else {

                padding_only = padding_only && bit == 1;
            }
        }
    }

    if node != 0 {

        if !padding_only {
            return Err("hpack: invalid huffman padding (not all ones)".to_string());
        }
        let depth = node_depth(&tree, node);
        if depth >= 8 {
            return Err("hpack: huffman padding too long".to_string());
        }
    }

    Ok(out)
}

fn node_depth(tree: &[HuffNode], target: usize) -> usize {

    let mut queue = vec![(0usize, 0usize)];
    let mut head = 0;
    while head < queue.len() {
        let (idx, d) = queue[head];
        head += 1;
        if idx == target {
            return d;
        }
        for &c in &tree[idx].children {
            if c != usize::MAX {
                queue.push((c, d + 1));
            }
        }
    }
    0
}

fn decode_integer(input: &[u8], pos: &mut usize, prefix_bits: u8) -> Result<usize, String> {
    if *pos >= input.len() {
        return Err("hpack: integer: out of input".to_string());
    }
    let max_prefix = (1usize << prefix_bits) - 1;
    let mut value = (input[*pos] as usize) & max_prefix;
    *pos += 1;
    if value < max_prefix {
        return Ok(value);
    }

    let mut m = 0u32;
    loop {
        if *pos >= input.len() {
            return Err("hpack: integer: truncated continuation".to_string());
        }
        let b = input[*pos];
        *pos += 1;
        let add = ((b & 0x7f) as usize)
            .checked_shl(m)
            .ok_or_else(|| "hpack: integer overflow".to_string())?;
        value = value
            .checked_add(add)
            .ok_or_else(|| "hpack: integer overflow".to_string())?;
        m += 7;
        if m > 64 {
            return Err("hpack: integer too large".to_string());
        }
        if b & 0x80 == 0 {
            break;
        }
    }
    Ok(value)
}

fn decode_string(input: &[u8], pos: &mut usize) -> Result<String, String> {
    if *pos >= input.len() {
        return Err("hpack: string: out of input".to_string());
    }
    let huffman = input[*pos] & 0x80 != 0;
    let len = decode_integer(input, pos, 7)?;
    if *pos + len > input.len() {
        return Err("hpack: string: length exceeds input".to_string());
    }
    let raw = &input[*pos..*pos + len];
    *pos += len;
    let bytes = if huffman {
        huffman_decode(raw)?
    } else {
        raw.to_vec()
    };
    String::from_utf8(bytes).map_err(|_| "hpack: string: invalid utf-8".to_string())
}

pub struct HpackDecoder {

    dynamic: std::collections::VecDeque<(String, String)>,

    size: usize,

    max_size: usize,
}

fn entry_size(name: &str, value: &str) -> usize {
    name.len() + value.len() + 32
}

impl HpackDecoder {
    pub fn new() -> Self {
        HpackDecoder {
            dynamic: std::collections::VecDeque::new(),
            size: 0,
            max_size: 4096,
        }
    }

    fn insert_dynamic(&mut self, name: String, value: String) {
        let sz = entry_size(&name, &value);

        if sz > self.max_size {
            self.dynamic.clear();
            self.size = 0;
            return;
        }
        self.size += sz;
        self.dynamic.push_front((name, value));
        self.evict();
    }

    fn evict(&mut self) {
        while self.size > self.max_size {
            if let Some((n, v)) = self.dynamic.pop_back() {
                self.size -= entry_size(&n, &v);
            } else {
                break;
            }
        }
    }

    fn set_max_size(&mut self, new_max: usize) {
        self.max_size = new_max;
        self.evict();
    }

    fn lookup(&self, index: usize) -> Result<(String, String), String> {
        if index == 0 {
            return Err("hpack: index 0 is invalid".to_string());
        }
        if index <= STATIC_TABLE.len() {
            let (n, v) = STATIC_TABLE[index - 1];
            Ok((n.to_string(), v.to_string()))
        } else {
            let dyn_idx = index - STATIC_TABLE.len() - 1;
            self.dynamic
                .get(dyn_idx)
                .cloned()
                .ok_or_else(|| format!("hpack: dynamic index {} out of range", index))
        }
    }

    pub fn decode(&mut self, input: &[u8]) -> Result<Vec<(String, String)>, String> {
        let mut out = Vec::new();
        let mut pos = 0usize;
        while pos < input.len() {
            let first = input[pos];
            if first & 0x80 != 0 {

                let index = decode_integer(input, &mut pos, 7)?;
                let (n, v) = self.lookup(index)?;
                out.push((n, v));
            } else if first & 0x40 != 0 {

                let (name, value) = self.decode_literal(input, &mut pos, 6)?;
                self.insert_dynamic(name.clone(), value.clone());
                out.push((name, value));
            } else if first & 0x20 != 0 {

                let new_max = decode_integer(input, &mut pos, 5)?;
                self.set_max_size(new_max);
            } else {

                let (name, value) = self.decode_literal(input, &mut pos, 4)?;
                out.push((name, value));
            }
        }
        Ok(out)
    }

    fn decode_literal(
        &self,
        input: &[u8],
        pos: &mut usize,
        prefix_bits: u8,
    ) -> Result<(String, String), String> {
        let name_index = decode_integer(input, pos, prefix_bits)?;
        let name = if name_index == 0 {
            decode_string(input, pos)?
        } else {
            self.lookup(name_index)?.0
        };
        let value = decode_string(input, pos)?;
        Ok((name, value))
    }
}

impl Default for HpackDecoder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct HpackEncoder {

    _private: (),
}

impl HpackEncoder {
    pub fn new() -> Self {
        HpackEncoder { _private: () }
    }

    pub fn encode(&mut self, headers: &[(String, String)]) -> Vec<u8> {
        let mut out = Vec::new();
        for (name, value) in headers {

            out.push(0x00);
            let lname = name.to_ascii_lowercase();
            encode_string_raw(&mut out, lname.as_bytes());
            encode_string_raw(&mut out, value.as_bytes());
        }
        out
    }
}

impl Default for HpackEncoder {
    fn default() -> Self {
        Self::new()
    }
}

fn encode_integer(out: &mut Vec<u8>, value: usize, prefix_bits: u8, prefix: u8) {
    let max_prefix = (1usize << prefix_bits) - 1;
    if value < max_prefix {
        out.push(prefix | value as u8);
    } else {
        out.push(prefix | max_prefix as u8);
        let mut v = value - max_prefix;
        while v >= 128 {
            out.push(((v & 0x7f) as u8) | 0x80);
            v >>= 7;
        }
        out.push(v as u8);
    }
}

fn encode_string_raw(out: &mut Vec<u8>, s: &[u8]) {
    encode_integer(out, s.len(), 7, 0x00);
    out.extend_from_slice(s);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_literal() {
        let headers = vec![
            (":method".to_string(), "GET".to_string()),
            (":path".to_string(), "/".to_string()),
            ("user-agent".to_string(), "cruft/1.0".to_string()),
            ("accept".to_string(), "*/*".to_string()),
        ];
        let mut enc = HpackEncoder::new();
        let bytes = enc.encode(&headers);
        let mut dec = HpackDecoder::new();
        let decoded = dec.decode(&bytes).expect("decode");
        assert_eq!(decoded, headers);
    }

    #[test]
    fn round_trip_lowercases_names() {
        let headers = vec![("Content-Type".to_string(), "text/html".to_string())];
        let mut enc = HpackEncoder::new();
        let bytes = enc.encode(&headers);
        let mut dec = HpackDecoder::new();
        let decoded = dec.decode(&bytes).expect("decode");
        assert_eq!(
            decoded,
            vec![("content-type".to_string(), "text/html".to_string())]
        );
    }

    #[test]
    fn rfc7541_c41_first_request_huffman() {

        let encoded: &[u8] = &[
            0x82, 0x86, 0x84, 0x41, 0x8c, 0xf1, 0xe3, 0xc2, 0xe5, 0xf2, 0x3a, 0x6b, 0xa0, 0xab,
            0x90, 0xf4, 0xff,
        ];
        let mut dec = HpackDecoder::new();
        let decoded = dec.decode(encoded).expect("decode");
        assert_eq!(
            decoded,
            vec![
                (":method".to_string(), "GET".to_string()),
                (":scheme".to_string(), "http".to_string()),
                (":path".to_string(), "/".to_string()),
                (":authority".to_string(), "www.example.com".to_string()),
            ]
        );
    }

    #[test]
    fn huffman_string_decode() {

        let huff: &[u8] = &[
            0xf1, 0xe3, 0xc2, 0xe5, 0xf2, 0x3a, 0x6b, 0xa0, 0xab, 0x90, 0xf4, 0xff,
        ];
        let decoded = huffman_decode(huff).expect("huffman decode");
        assert_eq!(decoded, b"www.example.com");
    }

    #[test]
    fn integer_decode_basic() {

        let mut pos = 0;
        let v = decode_integer(&[0x0a], &mut pos, 5).unwrap();
        assert_eq!(v, 10);
        assert_eq!(pos, 1);

        let mut pos = 0;
        let v = decode_integer(&[0x1f, 0x9a, 0x0a], &mut pos, 5).unwrap();
        assert_eq!(v, 1337);
        assert_eq!(pos, 3);
    }

    #[test]
    fn huffman_rejects_eos() {

        let all_ones: &[u8] = &[0xff, 0xff, 0xff, 0xff];
        assert!(huffman_decode(all_ones).is_err());
    }

    #[test]
    fn dynamic_table_indexing() {

        let mut input = Vec::new();
        input.push(0x40);
        encode_string_raw(&mut input, b"custom-key");
        encode_string_raw(&mut input, b"custom-value");

        input.push(0x80 | 62);

        let mut dec = HpackDecoder::new();
        let decoded = dec.decode(&input).expect("decode");
        assert_eq!(
            decoded,
            vec![
                ("custom-key".to_string(), "custom-value".to_string()),
                ("custom-key".to_string(), "custom-value".to_string()),
            ]
        );
    }
}
