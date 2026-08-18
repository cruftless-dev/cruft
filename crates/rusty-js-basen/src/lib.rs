
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseNError {
    InvalidAlphabet,
    InvalidLength,
    InvalidByte(u8),
    NonZeroTrailingBits,
    DataAfterPadding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Padding {
    Required,
    Optional,
    None,
}

#[derive(Debug, Clone, Copy)]
pub struct Encoding {
    alphabet: &'static [u8],
    symbol_bits: u8,
    padding: Padding,
    case_insensitive_decode: bool,
}

pub const BASE16_LOWER: Encoding =
    Encoding::new_unchecked(b"0123456789abcdef", 4, Padding::None, true);
pub const BASE32: Encoding = Encoding::new_unchecked(
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567",
    5,
    Padding::Optional,
    true,
);
pub const BASE64: Encoding = Encoding::new_unchecked(
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/",
    6,
    Padding::Optional,
    false,
);
pub const BASE64URL: Encoding = Encoding::new_unchecked(
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_",
    6,
    Padding::Optional,
    false,
);

impl Encoding {
    pub const fn new_unchecked(
        alphabet: &'static [u8],
        symbol_bits: u8,
        padding: Padding,
        case_insensitive_decode: bool,
    ) -> Self {
        Self {
            alphabet,
            symbol_bits,
            padding,
            case_insensitive_decode,
        }
    }

    pub fn encode(&self, input: &[u8]) -> Result<String, BaseNError> {
        self.validate()?;
        let mut out = String::new();
        let mut acc = 0u32;
        let mut bits = 0u8;
        let mask = (1u32 << self.symbol_bits) - 1;

        for &byte in input {
            acc = (acc << 8) | byte as u32;
            bits += 8;
            while bits >= self.symbol_bits {
                bits -= self.symbol_bits;
                let idx = ((acc >> bits) & mask) as usize;
                out.push(self.alphabet[idx] as char);
            }
        }

        if bits > 0 {
            let idx = ((acc << (self.symbol_bits - bits)) & mask) as usize;
            out.push(self.alphabet[idx] as char);
        }

        if matches!(self.padding, Padding::Required | Padding::Optional) {
            let quantum = output_quantum(self.symbol_bits);
            while out.len() % quantum != 0 {
                out.push('=');
            }
        }

        Ok(out)
    }

    pub fn decode(&self, input: &str) -> Result<Vec<u8>, BaseNError> {
        self.validate()?;
        let mut lut = [255u8; 256];
        for (idx, &b) in self.alphabet.iter().enumerate() {
            lut[b as usize] = idx as u8;
            if self.case_insensitive_decode && b.is_ascii_alphabetic() {
                lut[b.to_ascii_lowercase() as usize] = idx as u8;
                lut[b.to_ascii_uppercase() as usize] = idx as u8;
            }
        }

        let mut out = Vec::with_capacity(input.len() * self.symbol_bits as usize / 8);
        let mut acc = 0u32;
        let mut bits = 0u8;
        let mut symbols = 0usize;
        let mut saw_padding = false;

        for b in input.bytes() {
            if b == b'=' {
                if matches!(self.padding, Padding::None) {
                    return Err(BaseNError::InvalidByte(b));
                }
                saw_padding = true;
                continue;
            }
            if saw_padding {
                return Err(BaseNError::DataAfterPadding);
            }
            let v = lut[b as usize];
            if v == 255 {
                return Err(BaseNError::InvalidByte(b));
            }
            symbols += 1;
            acc = (acc << self.symbol_bits) | v as u32;
            bits += self.symbol_bits;
            while bits >= 8 {
                bits -= 8;
                out.push(((acc >> bits) & 0xff) as u8);
            }
        }

        if invalid_symbol_remainder(self.symbol_bits, symbols) {
            return Err(BaseNError::InvalidLength);
        }
        if bits > 0 {
            let trailing_mask = (1u32 << bits) - 1;
            if (acc & trailing_mask) != 0 {
                return Err(BaseNError::NonZeroTrailingBits);
            }
        }

        Ok(out)
    }

    fn validate(&self) -> Result<(), BaseNError> {
        if self.symbol_bits == 0 || self.symbol_bits >= 8 {
            return Err(BaseNError::InvalidAlphabet);
        }
        if self.alphabet.len() != (1usize << self.symbol_bits) {
            return Err(BaseNError::InvalidAlphabet);
        }
        Ok(())
    }
}

pub fn encode_base16_lower(input: &[u8]) -> String {
    BASE16_LOWER.encode(input).unwrap_or_default()
}

pub fn decode_base16(input: &str) -> Result<Vec<u8>, BaseNError> {
    BASE16_LOWER.decode(input)
}

pub fn encode_base32(input: &[u8]) -> String {
    BASE32.encode(input).unwrap_or_default()
}

pub fn decode_base32(input: &str) -> Result<Vec<u8>, BaseNError> {
    BASE32.decode(input)
}

pub fn encode_base64(input: &[u8]) -> String {
    BASE64.encode(input).unwrap_or_default()
}

pub fn decode_base64(input: &str) -> Result<Vec<u8>, BaseNError> {
    BASE64.decode(input)
}

pub fn encode_base64url(input: &[u8], padding: bool) -> String {
    let enc = Encoding::new_unchecked(
        BASE64URL.alphabet,
        BASE64URL.symbol_bits,
        if padding {
            Padding::Required
        } else {
            Padding::None
        },
        false,
    );
    enc.encode(input).unwrap_or_default()
}

pub fn decode_base64url(input: &str) -> Result<Vec<u8>, BaseNError> {
    BASE64URL.decode(input)
}

fn output_quantum(symbol_bits: u8) -> usize {
    8 / gcd(8, symbol_bits as usize)
}

fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a
}

fn invalid_symbol_remainder(symbol_bits: u8, symbols: usize) -> bool {
    match symbol_bits {
        4 => symbols % 2 == 1,
        5 => matches!(symbols % 8, 1 | 3 | 6),
        6 => symbols % 4 == 1,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RFC_VECTORS: &[(&str, &str, &str, &str)] = &[
        ("", "", "", ""),
        ("f", "66", "MY======", "Zg=="),
        ("fo", "666f", "MZXQ====", "Zm8="),
        ("foo", "666f6f", "MZXW6===", "Zm9v"),
        ("foob", "666f6f62", "MZXW6YQ=", "Zm9vYg=="),
        ("fooba", "666f6f6261", "MZXW6YTB", "Zm9vYmE="),
        ("foobar", "666f6f626172", "MZXW6YTBOI======", "Zm9vYmFy"),
    ];

    #[test]
    fn rfc4648_section_10_encode_vectors() {
        for &(plain, b16, b32, b64) in RFC_VECTORS {
            assert_eq!(encode_base16_lower(plain.as_bytes()), b16);
            assert_eq!(encode_base32(plain.as_bytes()), b32);
            assert_eq!(encode_base64(plain.as_bytes()), b64);
        }
    }

    #[test]
    fn rfc4648_section_10_decode_vectors() {
        for &(plain, b16, b32, b64) in RFC_VECTORS {
            assert_eq!(decode_base16(b16).unwrap(), plain.as_bytes());
            assert_eq!(decode_base32(b32).unwrap(), plain.as_bytes());
            assert_eq!(decode_base64(b64).unwrap(), plain.as_bytes());
        }
    }

    #[test]
    fn decode_accepts_case_projection_for_base16_and_base32() {
        assert_eq!(decode_base16("DEADBEEF").unwrap(), [0xde, 0xad, 0xbe, 0xef]);
        assert_eq!(decode_base32("mzxw6ytboi======").unwrap(), b"foobar");
    }

    #[test]
    fn base64url_projection() {
        assert_eq!(encode_base64url(&[0xfb, 0xff], false), "-_8");
        assert_eq!(decode_base64url("-_8").unwrap(), [0xfb, 0xff]);
    }

    #[test]
    fn invalid_lengths_are_rejected() {
        assert_eq!(decode_base16("f").unwrap_err(), BaseNError::InvalidLength);
        assert_eq!(decode_base64("Z").unwrap_err(), BaseNError::InvalidLength);
        assert_eq!(decode_base32("M").unwrap_err(), BaseNError::InvalidLength);
    }
}
