
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PercentError {
    TruncatedTriplet,
    InvalidHex,
}

#[derive(Clone, Copy)]
pub struct EncodeSet {
    table: [bool; 256],
}

impl EncodeSet {
    pub const fn from_table(table: [bool; 256]) -> Self {
        Self { table }
    }

    pub fn contains(&self, byte: u8) -> bool {
        self.table[byte as usize]
    }

    pub fn with(mut self, byte: u8) -> Self {
        self.table[byte as usize] = true;
        self
    }
}

pub const UNRESERVED: EncodeSet = EncodeSet::from_table(unreserved_table());
pub const RESERVED: EncodeSet = EncodeSet::from_table(reserved_table());
pub const CONTROLS: EncodeSet = EncodeSet::from_table(controls_table());
pub const FRAGMENT: EncodeSet = EncodeSet::from_table(fragment_table());
pub const PATH: EncodeSet = EncodeSet::from_table(path_table());
pub const USERINFO: EncodeSet = EncodeSet::from_table(userinfo_table());
pub const SPECIAL_QUERY: EncodeSet = EncodeSet::from_table(special_query_table());
pub const COMPONENT: EncodeSet = EncodeSet::from_table(component_table());

pub fn encode(bytes: &[u8], encode_set: &EncodeSet) -> String {
    let mut out = String::new();
    for &byte in bytes {
        if byte >= 0x80 || encode_set.contains(byte) {
            out.push('%');
            out.push_str(&rusty_js_basen::encode_base16_lower(&[byte]).to_ascii_uppercase());
        } else {
            out.push(byte as char);
        }
    }
    out
}

pub fn decode(input: &str) -> Result<Vec<u8>, PercentError> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'%' {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        if i + 2 >= bytes.len() {
            return Err(PercentError::TruncatedTriplet);
        }
        let hex =
            std::str::from_utf8(&bytes[i + 1..i + 3]).map_err(|_| PercentError::InvalidHex)?;
        let decoded = rusty_js_basen::decode_base16(hex).map_err(|_| PercentError::InvalidHex)?;
        out.push(decoded[0]);
        i += 3;
    }
    Ok(out)
}

pub fn decode_lenient(input: &str) -> Vec<u8> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'%' {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        if i + 2 >= bytes.len() {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        let hex = &bytes[i + 1..i + 3];
        if is_ascii_hex(hex[0]) && is_ascii_hex(hex[1]) {
            out.push((hex_value(hex[0]) << 4) | hex_value(hex[1]));
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    out
}

fn is_ascii_hex(byte: u8) -> bool {
    byte.is_ascii_hexdigit()
}

fn hex_value(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => 0,
    }
}

const fn unreserved_table() -> [bool; 256] {
    let mut t = [true; 256];
    let mut b = b'A';
    while b <= b'Z' {
        t[b as usize] = false;
        b += 1;
    }
    b = b'a';
    while b <= b'z' {
        t[b as usize] = false;
        b += 1;
    }
    b = b'0';
    while b <= b'9' {
        t[b as usize] = false;
        b += 1;
    }
    t[b'-' as usize] = false;
    t[b'.' as usize] = false;
    t[b'_' as usize] = false;
    t[b'~' as usize] = false;
    t
}

const fn reserved_table() -> [bool; 256] {
    let mut t = [false; 256];
    let reserved = *b":/?#[]@!$&'()*+,;=";
    let mut i = 0;
    while i < reserved.len() {
        t[reserved[i] as usize] = true;
        i += 1;
    }
    t
}

const fn controls_table() -> [bool; 256] {
    let mut t = [false; 256];
    let mut i = 0;
    while i <= 0x1f {
        t[i] = true;
        i += 1;
    }
    t[0x7f] = true;
    t
}

const fn merge(mut a: [bool; 256], b: [bool; 256]) -> [bool; 256] {
    let mut i = 0;
    while i < 256 {
        a[i] = a[i] || b[i];
        i += 1;
    }
    a
}

const fn add_bytes(mut t: [bool; 256], bytes: &[u8]) -> [bool; 256] {
    let mut i = 0;
    while i < bytes.len() {
        t[bytes[i] as usize] = true;
        i += 1;
    }
    t
}

const fn fragment_table() -> [bool; 256] {
    add_bytes(controls_table(), b" \"<>`")
}

const fn path_table() -> [bool; 256] {
    add_bytes(fragment_table(), b"?#{}^")
}

const fn userinfo_table() -> [bool; 256] {
    add_bytes(path_table(), b"/:;=@[\\]^|")
}

const fn special_query_table() -> [bool; 256] {
    add_bytes(fragment_table(), b"'")
}

const fn component_table() -> [bool; 256] {
    merge(unreserved_table(), add_bytes([false; 256], b"$%&+,/:;=?@"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3986_triplet_decode() {
        assert_eq!(decode("A%20B%2FC%7E").unwrap(), b"A B/C~");
        assert_eq!(decode("%ff").unwrap(), vec![0xff]);
    }

    #[test]
    fn rfc3986_unreserved_encode() {
        assert_eq!(encode(b"AZaz09-._~", &UNRESERVED), "AZaz09-._~");
        assert_eq!(encode(b" /?#", &UNRESERVED), "%20%2F%3F%23");
    }

    #[test]
    fn whatwg_path_set_smoke() {
        assert_eq!(encode(b"/a b?c#d", &PATH), "/a%20b%3Fc%23d");
        assert_eq!(encode("x\u{2025}y".as_bytes(), &PATH), "x%E2%80%A5y");
        assert_eq!(encode(b"^", &PATH), "%5E");
    }

    #[test]
    fn whatwg_userinfo_set_smoke() {
        assert_eq!(encode(b"user:pa/ss@[", &USERINFO), "user%3Apa%2Fss%40%5B");
    }

    #[test]
    fn malformed_triplets_reject() {
        assert_eq!(decode("%").unwrap_err(), PercentError::TruncatedTriplet);
        assert_eq!(decode("%G0").unwrap_err(), PercentError::InvalidHex);
    }

    #[test]
    fn lenient_decode_accepts_valid_triplets() {
        assert_eq!(decode_lenient("%2B"), b"+");
        assert_eq!(decode_lenient("%E2%82%AC"), "€".as_bytes());
        assert_eq!(decode_lenient("hello"), b"hello");
    }

    #[test]
    fn lenient_decode_passes_malformed_percent_through() {
        assert_eq!(decode_lenient("a%2"), b"a%2");
        assert_eq!(decode_lenient("%Xy"), b"%Xy");
        assert_eq!(decode_lenient("100%"), b"100%");
        assert_eq!(decode_lenient("%G0"), b"%G0");
    }

    #[test]
    fn lenient_decode_handles_empty_and_trailing_partial() {
        assert_eq!(decode_lenient(""), b"");
        assert_eq!(decode_lenient("%"), b"%");
        assert_eq!(decode_lenient("abc%2"), b"abc%2");
    }
}
