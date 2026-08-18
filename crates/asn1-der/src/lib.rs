
#[derive(Debug, Clone, PartialEq)]
pub enum DerError {
    UnexpectedEnd,
    InvalidLength,
    UnknownTag(u8),
    WrongTag { expected: u8, actual: u8 },
    NotConstructed,
    NotPrimitive,
    InvalidOid,
    InvalidInteger,
    TrailingData,
    MaxDepthExceeded,
}

impl std::fmt::Display for DerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DerError::UnexpectedEnd => write!(f, "unexpected end of DER input"),
            DerError::InvalidLength => write!(f, "invalid DER length encoding"),
            DerError::UnknownTag(t) => write!(f, "unknown DER tag 0x{:02x}", t),
            DerError::WrongTag { expected, actual } => write!(
                f,
                "DER tag mismatch: expected 0x{:02x}, got 0x{:02x}",
                expected, actual
            ),
            DerError::NotConstructed => write!(f, "DER value is not constructed (no inner)"),
            DerError::NotPrimitive => write!(f, "DER value is primitive (no constructed inner)"),
            DerError::InvalidOid => write!(f, "invalid OID encoding"),
            DerError::InvalidInteger => write!(f, "invalid INTEGER encoding"),
            DerError::TrailingData => write!(f, "unexpected trailing data after DER value"),
            DerError::MaxDepthExceeded => write!(f, "DER nesting exceeds configured depth"),
        }
    }
}

impl std::error::Error for DerError {}

pub const TAG_BOOLEAN: u8 = 0x01;
pub const TAG_INTEGER: u8 = 0x02;
pub const TAG_BIT_STRING: u8 = 0x03;
pub const TAG_OCTET_STRING: u8 = 0x04;
pub const TAG_NULL: u8 = 0x05;
pub const TAG_OID: u8 = 0x06;
pub const TAG_UTF8_STRING: u8 = 0x0C;
pub const TAG_SEQUENCE: u8 = 0x30;
pub const TAG_SET: u8 = 0x31;
pub const TAG_PRINTABLE_STRING: u8 = 0x13;
pub const TAG_TELETEX_STRING: u8 = 0x14;
pub const TAG_IA5_STRING: u8 = 0x16;
pub const TAG_UTC_TIME: u8 = 0x17;
pub const TAG_GENERALIZED_TIME: u8 = 0x18;
pub const TAG_UNIVERSAL_STRING: u8 = 0x1C;
pub const TAG_BMP_STRING: u8 = 0x1E;

#[derive(Debug, Clone)]
pub struct DerValue<'a> {
    pub tag: u8,
    pub content: &'a [u8],
}

impl<'a> DerValue<'a> {
    pub fn is_constructed(&self) -> bool {
        (self.tag & 0x20) != 0
    }
    pub fn is_context_specific(&self) -> bool {
        (self.tag & 0xC0) == 0x80
    }
    pub fn context_tag_number(&self) -> u8 {
        self.tag & 0x1F
    }
}

pub struct DerReader<'a> {
    buf: &'a [u8],
}

impl<'a> DerReader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        DerReader { buf }
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
    pub fn remaining(&self) -> &'a [u8] {
        self.buf
    }

    pub fn read_tlv(&mut self) -> Result<DerValue<'a>, DerError> {
        if self.buf.is_empty() {
            return Err(DerError::UnexpectedEnd);
        }
        let tag = self.buf[0];
        validate_tag(tag)?;
        let (length, header_len) = parse_length(&self.buf[1..])?;
        let total = 1 + header_len + length;
        if total > self.buf.len() {
            return Err(DerError::UnexpectedEnd);
        }
        let content = &self.buf[1 + header_len..1 + header_len + length];
        self.buf = &self.buf[total..];
        Ok(DerValue { tag, content })
    }

    pub fn read_tag(&mut self, expected: u8) -> Result<DerValue<'a>, DerError> {
        let v = self.read_tlv()?;
        if v.tag != expected {
            return Err(DerError::WrongTag {
                expected,
                actual: v.tag,
            });
        }
        Ok(v)
    }

    pub fn peek_tag(&self) -> Option<u8> {
        self.buf.first().copied()
    }
}

fn validate_tag(tag: u8) -> Result<(), DerError> {
    let class = tag & 0xC0;
    let constructed = (tag & 0x20) != 0;
    let number = tag & 0x1F;
    if number == 0x1F {
        return Err(DerError::UnknownTag(tag));
    }
    if class != 0 {
        return Ok(());
    }
    match number {
        0x01 | 0x02 | 0x03 | 0x04 | 0x05 | 0x06 | 0x0C | 0x13 | 0x14 | 0x16 | 0x17 | 0x18
        | 0x1C | 0x1E => {
            if constructed {
                Err(DerError::NotPrimitive)
            } else {
                Ok(())
            }
        }
        0x10 | 0x11 => {
            if constructed {
                Ok(())
            } else {
                Err(DerError::NotConstructed)
            }
        }
        _ => Err(DerError::UnknownTag(tag)),
    }
}

fn parse_length(buf: &[u8]) -> Result<(usize, usize), DerError> {
    if buf.is_empty() {
        return Err(DerError::UnexpectedEnd);
    }
    let first = buf[0];
    if first < 0x80 {

        Ok((first as usize, 1))
    } else {

        let n = (first & 0x7F) as usize;
        if n == 0 || n > 4 {

            return Err(DerError::InvalidLength);
        }
        if buf.len() < 1 + n {
            return Err(DerError::UnexpectedEnd);
        }
        let mut length: usize = 0;
        for i in 0..n {
            length = (length << 8) | (buf[1 + i] as usize);
        }

        if (n == 1 && length < 128) || (n > 1 && buf[1] == 0) {
            return Err(DerError::InvalidLength);
        }
        Ok((length, 1 + n))
    }
}

impl<'a> DerValue<'a> {

    pub fn into_reader(self) -> Result<DerReader<'a>, DerError> {
        if !self.is_constructed() {
            return Err(DerError::NotConstructed);
        }
        Ok(DerReader::new(self.content))
    }

    pub fn as_bytes(&self) -> &'a [u8] {
        self.content
    }

    pub fn as_integer_bytes(&self) -> Result<&'a [u8], DerError> {
        if self.tag != TAG_INTEGER {
            return Err(DerError::WrongTag {
                expected: TAG_INTEGER,
                actual: self.tag,
            });
        }
        if self.content.is_empty() {
            return Err(DerError::InvalidInteger);
        }

        if self.content.len() > 1 {
            let b0 = self.content[0];
            let b1 = self.content[1];
            if (b0 == 0x00 && (b1 & 0x80) == 0) || (b0 == 0xFF && (b1 & 0x80) != 0) {
                return Err(DerError::InvalidInteger);
            }
        }
        Ok(self.content)
    }

    pub fn as_unsigned_integer(&self) -> Result<&'a [u8], DerError> {
        let bytes = self.as_integer_bytes()?;

        if !bytes.is_empty() && (bytes[0] & 0x80) != 0 {
            return Err(DerError::InvalidInteger);
        }

        if bytes.len() > 1 && bytes[0] == 0x00 {
            Ok(&bytes[1..])
        } else {
            Ok(bytes)
        }
    }

    pub fn as_i64(&self) -> Result<i64, DerError> {
        let bytes = self.as_integer_bytes()?;
        if bytes.len() > 8 {
            return Err(DerError::InvalidInteger);
        }
        let mut v: i64 = if (bytes[0] & 0x80) != 0 { -1 } else { 0 };
        for &b in bytes {
            v = (v << 8) | (b as i64 & 0xff);
        }
        Ok(v)
    }

    pub fn as_bit_string(&self) -> Result<(u8, &'a [u8]), DerError> {
        if self.tag != TAG_BIT_STRING {
            return Err(DerError::WrongTag {
                expected: TAG_BIT_STRING,
                actual: self.tag,
            });
        }
        if self.content.is_empty() {
            return Err(DerError::InvalidLength);
        }
        let unused = self.content[0];
        if unused > 7 {
            return Err(DerError::InvalidLength);
        }
        let data = &self.content[1..];
        if unused != 0 {
            let Some(&last) = data.last() else {
                return Err(DerError::InvalidLength);
            };
            let unused_mask = (1u8 << unused) - 1;
            if last & unused_mask != 0 {
                return Err(DerError::InvalidLength);
            }
        }
        Ok((unused, data))
    }

    pub fn as_oid(&self) -> Result<Vec<u64>, DerError> {
        if self.tag != TAG_OID {
            return Err(DerError::WrongTag {
                expected: TAG_OID,
                actual: self.tag,
            });
        }
        if self.content.is_empty() {
            return Err(DerError::InvalidOid);
        }
        let mut out = Vec::new();
        let first_byte = self.content[0];
        out.push((first_byte / 40) as u64);
        out.push((first_byte % 40) as u64);
        let mut value: u64 = 0;
        for &b in &self.content[1..] {

            if value > (u64::MAX >> 7) {
                return Err(DerError::InvalidOid);
            }
            value = (value << 7) | ((b & 0x7F) as u64);
            if (b & 0x80) == 0 {
                out.push(value);
                value = 0;
            }
        }
        if value != 0 {

            return Err(DerError::InvalidOid);
        }
        Ok(out)
    }

    pub fn as_string(&self) -> Result<&'a str, DerError> {
        match self.tag {
            TAG_UTF8_STRING | TAG_PRINTABLE_STRING | TAG_IA5_STRING => {
                std::str::from_utf8(self.content).map_err(|_| DerError::InvalidLength)
            }
            _ => Err(DerError::WrongTag {
                expected: TAG_UTF8_STRING,
                actual: self.tag,
            }),
        }
    }

    pub fn as_utc_time(&self) -> Result<&'a [u8], DerError> {
        if self.tag != TAG_UTC_TIME {
            return Err(DerError::WrongTag {
                expected: TAG_UTC_TIME,
                actual: self.tag,
            });
        }
        Ok(self.content)
    }

    pub fn as_generalized_time(&self) -> Result<&'a [u8], DerError> {
        if self.tag != TAG_GENERALIZED_TIME {
            return Err(DerError::WrongTag {
                expected: TAG_GENERALIZED_TIME,
                actual: self.tag,
            });
        }
        Ok(self.content)
    }

    pub fn as_bool(&self) -> Result<bool, DerError> {
        if self.tag != TAG_BOOLEAN {
            return Err(DerError::WrongTag {
                expected: TAG_BOOLEAN,
                actual: self.tag,
            });
        }
        if self.content.len() != 1 {
            return Err(DerError::InvalidLength);
        }
        match self.content[0] {
            0x00 => Ok(false),
            0xFF => Ok(true),
            _ => Err(DerError::InvalidLength),
        }
    }
}

pub fn oid_to_string(arcs: &[u64]) -> String {
    let mut s = String::new();
    for (i, a) in arcs.iter().enumerate() {
        if i > 0 {
            s.push('.');
        }
        s.push_str(&a.to_string());
    }
    s
}

pub fn parse_single<'a>(buf: &'a [u8]) -> Result<DerValue<'a>, DerError> {
    let mut r = DerReader::new(buf);
    let v = r.read_tlv()?;
    if !r.is_empty() {
        return Err(DerError::TrailingData);
    }
    Ok(v)
}

pub fn validate_der_tree(buf: &[u8], max_depth: usize) -> Result<(), DerError> {
    fn walk(buf: &[u8], depth: usize, max_depth: usize) -> Result<(), DerError> {
        if depth > max_depth {
            return Err(DerError::MaxDepthExceeded);
        }
        let mut reader = DerReader::new(buf);
        while !reader.is_empty() {
            let value = reader.read_tlv()?;
            if value.is_constructed() {
                walk(value.content, depth + 1, max_depth)?;
            }
        }
        Ok(())
    }

    let value = parse_single(buf)?;
    if value.is_constructed() {
        walk(value.content, 1, max_depth)?;
    }
    Ok(())
}

pub fn enc_len(len: usize) -> Vec<u8> {
    if len < 0x80 {
        return vec![len as u8];
    }
    let mut be = Vec::new();
    let mut n = len;
    while n > 0 {
        be.insert(0, (n & 0xff) as u8);
        n >>= 8;
    }
    let mut out = vec![0x80 | be.len() as u8];
    out.extend_from_slice(&be);
    out
}

pub fn enc_tlv(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + 4 + content.len());
    out.push(tag);
    out.extend_from_slice(&enc_len(content.len()));
    out.extend_from_slice(content);
    out
}

pub fn enc_sequence(items: &[Vec<u8>]) -> Vec<u8> {
    let mut content = Vec::new();
    for it in items {
        content.extend_from_slice(it);
    }
    enc_tlv(TAG_SEQUENCE, &content)
}

pub fn enc_integer_unsigned(magnitude: &[u8]) -> Vec<u8> {
    let mut i = 0;
    while i + 1 < magnitude.len() && magnitude[i] == 0 {
        i += 1;
    }
    let trimmed = if magnitude.is_empty() {
        &[0u8][..]
    } else {
        &magnitude[i..]
    };
    let mut body = Vec::with_capacity(trimmed.len() + 1);
    if trimmed[0] & 0x80 != 0 {
        body.push(0x00);
    }
    body.extend_from_slice(trimmed);
    enc_tlv(TAG_INTEGER, &body)
}

pub fn enc_integer_small(v: u64) -> Vec<u8> {
    enc_integer_unsigned(&v.to_be_bytes())
}

pub fn enc_octet_string(data: &[u8]) -> Vec<u8> {
    enc_tlv(TAG_OCTET_STRING, data)
}

pub fn enc_bit_string(data: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(data.len() + 1);
    body.push(0x00);
    body.extend_from_slice(data);
    enc_tlv(TAG_BIT_STRING, &body)
}

pub fn enc_null() -> Vec<u8> {
    enc_tlv(TAG_NULL, &[])
}

pub fn enc_oid(arcs: &[u64]) -> Vec<u8> {
    let mut body = Vec::new();
    if arcs.len() >= 2 {
        body.push((arcs[0] * 40 + arcs[1]) as u8);
        for &arc in &arcs[2..] {
            let mut stack = Vec::new();
            let mut n = arc;
            stack.push((n & 0x7f) as u8);
            n >>= 7;
            while n > 0 {
                stack.push((n & 0x7f) as u8 | 0x80);
                n >>= 7;
            }
            stack.reverse();
            body.extend_from_slice(&stack);
        }
    }
    enc_tlv(TAG_OID, &body)
}

pub fn enc_context_constructed(tag_num: u8, content: &[u8]) -> Vec<u8> {
    enc_tlv(0xA0 | (tag_num & 0x1f), content)
}

#[cfg(test)]
mod writer_tests {
    use super::*;

    #[test]
    fn integer_strips_and_pads() {

        assert_eq!(
            enc_integer_unsigned(&[0x00, 0x00, 0x01]),
            vec![0x02, 0x01, 0x01]
        );
        assert_eq!(enc_integer_unsigned(&[0x80]), vec![0x02, 0x02, 0x00, 0x80]);
        assert_eq!(enc_integer_small(0), vec![0x02, 0x01, 0x00]);
    }

    #[test]
    fn oid_roundtrips_through_reader() {
        let arcs = vec![1u64, 2, 840, 113549, 1, 1, 1];
        let enc = enc_oid(&arcs);
        let v = parse_single(&enc).unwrap();
        assert_eq!(v.as_oid().unwrap(), arcs);
    }

    #[test]
    fn sequence_of_integers_roundtrips() {
        let seq = enc_sequence(&[
            enc_integer_unsigned(&[0xab, 0xcd]),
            enc_integer_small(65537),
        ]);
        let v = parse_single(&seq).unwrap();
        let mut r = v.into_reader().unwrap();
        assert_eq!(
            r.read_tag(TAG_INTEGER)
                .unwrap()
                .as_unsigned_integer()
                .unwrap(),
            &[0xab, 0xcd]
        );
        assert_eq!(
            r.read_tag(TAG_INTEGER)
                .unwrap()
                .as_unsigned_integer()
                .unwrap(),
            &[0x01, 0x00, 0x01]
        );
    }

    #[test]
    fn long_length_form() {

        let os = enc_octet_string(&vec![0u8; 200]);
        assert_eq!(&os[0..3], &[TAG_OCTET_STRING, 0x81, 0xC8]);
        let v = parse_single(&os).unwrap();
        assert_eq!(v.as_bytes().len(), 200);
    }

    #[test]
    fn bit_string_roundtrips() {
        let bs = enc_bit_string(&[0xde, 0xad, 0xbe, 0xef]);
        let v = parse_single(&bs).unwrap();
        let (unused, data) = v.as_bit_string().unwrap();
        assert_eq!(unused, 0);
        assert_eq!(data, &[0xde, 0xad, 0xbe, 0xef]);
    }
}
