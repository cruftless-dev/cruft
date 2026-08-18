
mod compress;
pub use compress::{compressed_deflate, compressed_gzip_deflate, compressed_zlib_deflate};
pub mod stream;
pub use stream::{CompressionFormat, StreamCodec};

#[derive(Debug)]
pub enum DecodeError {
    UnexpectedEnd,
    InvalidBlockType,
    InvalidStoredLen,
    InvalidHuffmanCode,
    InvalidLengthCode,
    InvalidDistanceCode,
    DistanceTooFar,
    InvalidGzipMagic,
    UnsupportedGzipMethod,
    GzipReservedFlags,
    GzipCrcMismatch,
    GzipSizeMismatch,
    OutputTooLarge,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::UnexpectedEnd => write!(f, "unexpected end of input"),
            DecodeError::InvalidBlockType => write!(f, "invalid DEFLATE block type"),
            DecodeError::InvalidStoredLen => write!(f, "stored block: LEN/NLEN mismatch"),
            DecodeError::InvalidHuffmanCode => write!(f, "invalid Huffman code"),
            DecodeError::InvalidLengthCode => write!(f, "invalid length code"),
            DecodeError::InvalidDistanceCode => write!(f, "invalid distance code"),
            DecodeError::DistanceTooFar => write!(f, "back-reference distance exceeds output"),
            DecodeError::InvalidGzipMagic => write!(f, "invalid gzip magic bytes"),
            DecodeError::UnsupportedGzipMethod => {
                write!(f, "unsupported gzip compression method (only deflate=8)")
            }
            DecodeError::GzipReservedFlags => write!(f, "gzip reserved flags set"),
            DecodeError::GzipCrcMismatch => write!(f, "gzip CRC32 mismatch"),
            DecodeError::GzipSizeMismatch => write!(f, "gzip ISIZE mismatch"),
            DecodeError::OutputTooLarge => write!(f, "decoded output exceeds maximum size"),
        }
    }
}

impl std::error::Error for DecodeError {}

pub const MAX_OUTPUT: usize = 256 * 1024 * 1024;

struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: u32,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    fn read_bits(&mut self, n: u32) -> Result<u32, DecodeError> {
        let mut value: u32 = 0;
        for i in 0..n {
            if self.byte_pos >= self.data.len() {
                return Err(DecodeError::UnexpectedEnd);
            }
            let bit = (self.data[self.byte_pos] >> self.bit_pos) & 1;
            value |= (bit as u32) << i;
            self.bit_pos += 1;
            if self.bit_pos == 8 {
                self.bit_pos = 0;
                self.byte_pos += 1;
            }
        }
        Ok(value)
    }

    fn align_to_byte(&mut self) {
        if self.bit_pos != 0 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
    }

    fn read_aligned_u16_le(&mut self) -> Result<u16, DecodeError> {
        if self.byte_pos + 2 > self.data.len() {
            return Err(DecodeError::UnexpectedEnd);
        }
        let lo = self.data[self.byte_pos] as u16;
        let hi = self.data[self.byte_pos + 1] as u16;
        self.byte_pos += 2;
        Ok(lo | (hi << 8))
    }

    fn read_aligned_bytes(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        if self.byte_pos + n > self.data.len() {
            return Err(DecodeError::UnexpectedEnd);
        }
        let r = &self.data[self.byte_pos..self.byte_pos + n];
        self.byte_pos += n;
        Ok(r)
    }
}

struct HuffmanTable {

    counts: [u16; 16],
    symbols: Vec<u16>,
}

impl HuffmanTable {
    fn from_lengths(lengths: &[u8]) -> Result<Self, DecodeError> {
        let mut counts = [0u16; 16];
        for &l in lengths {
            if l as usize >= 16 {
                return Err(DecodeError::InvalidHuffmanCode);
            }
            counts[l as usize] += 1;
        }
        counts[0] = 0;

        let mut offsets = [0u16; 17];
        for i in 1..16 {
            offsets[i + 1] = offsets[i] + counts[i];
        }
        let total: usize = (1..16).map(|i| counts[i] as usize).sum();
        let mut symbols = vec![0u16; total];
        let mut next = offsets;
        for (sym, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbols[next[l as usize] as usize] = sym as u16;
                next[l as usize] += 1;
            }
        }
        Ok(HuffmanTable { counts, symbols })
    }

    fn decode(&self, br: &mut BitReader) -> Result<u16, DecodeError> {
        let mut code: u32 = 0;
        let mut first: u32 = 0;
        let mut index: u32 = 0;
        for l in 1..16u32 {
            code = (code << 1) | br.read_bits(1)?;
            let count = self.counts[l as usize] as u32;
            if code < first + count {
                let sym_idx = index + (code - first);
                return Ok(self.symbols[sym_idx as usize]);
            }
            index += count;
            first = (first + count) << 1;
        }
        Err(DecodeError::InvalidHuffmanCode)
    }
}

fn fixed_literal_lengths() -> [u8; 288] {
    let mut l = [0u8; 288];

    for i in 0..=143 {
        l[i] = 8;
    }
    for i in 144..=255 {
        l[i] = 9;
    }
    for i in 256..=279 {
        l[i] = 7;
    }
    for i in 280..=287 {
        l[i] = 8;
    }
    l
}

fn fixed_distance_lengths() -> [u8; 30] {
    [5u8; 30]
}

const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DISTANCE_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DISTANCE_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

const CODE_LENGTH_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

fn read_dynamic_tables(br: &mut BitReader) -> Result<(HuffmanTable, HuffmanTable), DecodeError> {
    let hlit = br.read_bits(5)? as usize + 257;
    let hdist = br.read_bits(5)? as usize + 1;
    let hclen = br.read_bits(4)? as usize + 4;

    let mut code_lengths = [0u8; 19];
    for i in 0..hclen {
        code_lengths[CODE_LENGTH_ORDER[i]] = br.read_bits(3)? as u8;
    }
    let cl_table = HuffmanTable::from_lengths(&code_lengths)?;

    let mut combined = vec![0u8; hlit + hdist];
    let mut i = 0;
    while i < combined.len() {
        let sym = cl_table.decode(br)?;
        match sym {
            0..=15 => {
                combined[i] = sym as u8;
                i += 1;
            }
            16 => {
                if i == 0 {
                    return Err(DecodeError::InvalidHuffmanCode);
                }
                let prev = combined[i - 1];
                let repeat = br.read_bits(2)? as usize + 3;
                for _ in 0..repeat {
                    if i >= combined.len() {
                        return Err(DecodeError::InvalidHuffmanCode);
                    }
                    combined[i] = prev;
                    i += 1;
                }
            }
            17 => {
                let repeat = br.read_bits(3)? as usize + 3;
                for _ in 0..repeat {
                    if i >= combined.len() {
                        return Err(DecodeError::InvalidHuffmanCode);
                    }
                    combined[i] = 0;
                    i += 1;
                }
            }
            18 => {
                let repeat = br.read_bits(7)? as usize + 11;
                for _ in 0..repeat {
                    if i >= combined.len() {
                        return Err(DecodeError::InvalidHuffmanCode);
                    }
                    combined[i] = 0;
                    i += 1;
                }
            }
            _ => return Err(DecodeError::InvalidHuffmanCode),
        }
    }

    let lit_table = HuffmanTable::from_lengths(&combined[..hlit])?;
    let dist_table = HuffmanTable::from_lengths(&combined[hlit..])?;
    Ok((lit_table, dist_table))
}

fn decompress_block(
    br: &mut BitReader,
    out: &mut Vec<u8>,
    lit: &HuffmanTable,
    dist: &HuffmanTable,
    max_output: usize,
) -> Result<(), DecodeError> {
    loop {
        if out.len() > max_output {
            return Err(DecodeError::OutputTooLarge);
        }
        let sym = lit.decode(br)?;
        if sym < 256 {
            out.push(sym as u8);
        } else if sym == 256 {
            return Ok(());
        } else {
            let code = (sym - 257) as usize;
            if code >= 29 {
                return Err(DecodeError::InvalidLengthCode);
            }
            let length =
                LENGTH_BASE[code] as usize + br.read_bits(LENGTH_EXTRA[code] as u32)? as usize;
            let dist_sym = dist.decode(br)? as usize;
            if dist_sym >= 30 {
                return Err(DecodeError::InvalidDistanceCode);
            }
            let distance = DISTANCE_BASE[dist_sym] as usize
                + br.read_bits(DISTANCE_EXTRA[dist_sym] as u32)? as usize;
            if distance > out.len() {
                return Err(DecodeError::DistanceTooFar);
            }
            let start = out.len() - distance;
            for i in 0..length {
                let b = out[start + i];
                out.push(b);
                if out.len() > max_output {
                    return Err(DecodeError::OutputTooLarge);
                }
            }
        }
    }
}

pub fn inflate(data: &[u8]) -> Result<Vec<u8>, DecodeError> {
    inflate_with_limit(data, MAX_OUTPUT)
}

pub fn inflate_with_limit(data: &[u8], max_output: usize) -> Result<Vec<u8>, DecodeError> {
    let mut br = BitReader::new(data);
    let mut out = Vec::new();
    loop {
        let bfinal = br.read_bits(1)?;
        let btype = br.read_bits(2)?;
        match btype {
            0 => {
                br.align_to_byte();
                let len = br.read_aligned_u16_le()?;
                let nlen = br.read_aligned_u16_le()?;
                if len ^ nlen != 0xFFFF {
                    return Err(DecodeError::InvalidStoredLen);
                }
                let bytes = br.read_aligned_bytes(len as usize)?;
                let next_len = out
                    .len()
                    .checked_add(bytes.len())
                    .ok_or(DecodeError::OutputTooLarge)?;
                if next_len > max_output {
                    return Err(DecodeError::OutputTooLarge);
                }
                out.extend_from_slice(bytes);
            }
            1 => {
                let lit = HuffmanTable::from_lengths(&fixed_literal_lengths())?;
                let dist = HuffmanTable::from_lengths(&fixed_distance_lengths())?;
                decompress_block(&mut br, &mut out, &lit, &dist, max_output)?;
            }
            2 => {
                let (lit, dist) = read_dynamic_tables(&mut br)?;
                decompress_block(&mut br, &mut out, &lit, &dist, max_output)?;
            }
            _ => return Err(DecodeError::InvalidBlockType),
        }
        if bfinal != 0 {
            break;
        }
    }
    Ok(out)
}

pub fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for n in 0..256u32 {
        let mut c = n;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB88320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        table[n as usize] = c;
    }
    let mut c = 0xFFFFFFFFu32;
    for &b in data {
        c = table[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFFFFFF
}

pub fn gunzip(data: &[u8]) -> Result<Vec<u8>, DecodeError> {
    gunzip_with_limit(data, MAX_OUTPUT)
}

pub fn gunzip_with_limit(data: &[u8], max_output: usize) -> Result<Vec<u8>, DecodeError> {
    if data.len() < 18 {
        return Err(DecodeError::UnexpectedEnd);
    }
    if data[0] != 0x1f || data[1] != 0x8b {
        return Err(DecodeError::InvalidGzipMagic);
    }
    if data[2] != 8 {
        return Err(DecodeError::UnsupportedGzipMethod);
    }
    let flg = data[3];
    if flg & 0xE0 != 0 {
        return Err(DecodeError::GzipReservedFlags);
    }

    let mut p: usize = 10;

    if flg & 0x04 != 0 {
        if p + 2 > data.len() {
            return Err(DecodeError::UnexpectedEnd);
        }
        let xlen = (data[p] as usize) | ((data[p + 1] as usize) << 8);
        p += 2 + xlen;
        if p > data.len() {
            return Err(DecodeError::UnexpectedEnd);
        }
    }

    if flg & 0x08 != 0 {
        while p < data.len() && data[p] != 0 {
            p += 1;
        }
        if p >= data.len() {
            return Err(DecodeError::UnexpectedEnd);
        }
        p += 1;
    }

    if flg & 0x10 != 0 {
        while p < data.len() && data[p] != 0 {
            p += 1;
        }
        if p >= data.len() {
            return Err(DecodeError::UnexpectedEnd);
        }
        p += 1;
    }

    if flg & 0x02 != 0 {
        if p + 2 > data.len() {
            return Err(DecodeError::UnexpectedEnd);
        }
        p += 2;
    }

    if data.len() < p + 8 {
        return Err(DecodeError::UnexpectedEnd);
    }
    let payload = &data[p..data.len() - 8];
    let trailer = &data[data.len() - 8..];

    let out = inflate_with_limit(payload, max_output)?;

    let crc_expected = (trailer[0] as u32)
        | ((trailer[1] as u32) << 8)
        | ((trailer[2] as u32) << 16)
        | ((trailer[3] as u32) << 24);
    let isize_expected = (trailer[4] as u32)
        | ((trailer[5] as u32) << 8)
        | ((trailer[6] as u32) << 16)
        | ((trailer[7] as u32) << 24);
    if crc32(&out) != crc_expected {
        return Err(DecodeError::GzipCrcMismatch);
    }
    if (out.len() as u32) != isize_expected {
        return Err(DecodeError::GzipSizeMismatch);
    }
    Ok(out)
}

pub fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &x in data {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

pub fn zlib_inflate(data: &[u8]) -> Result<Vec<u8>, DecodeError> {
    zlib_inflate_with_limit(data, MAX_OUTPUT)
}

pub fn zlib_inflate_with_limit(data: &[u8], max_output: usize) -> Result<Vec<u8>, DecodeError> {
    if data.len() < 6 {
        return Err(DecodeError::UnexpectedEnd);
    }
    let cmf = data[0];
    let flg = data[1];
    if (cmf & 0x0F) != 8 {
        return Err(DecodeError::UnsupportedGzipMethod);
    }
    if ((cmf as u16) << 8 | flg as u16) % 31 != 0 {
        return Err(DecodeError::InvalidHuffmanCode);
    }
    if flg & 0x20 != 0 {

        return Err(DecodeError::GzipReservedFlags);
    }
    let payload = &data[2..data.len() - 4];
    let trailer = &data[data.len() - 4..];

    let out = inflate_with_limit(payload, max_output)?;

    let adler_expected = ((trailer[0] as u32) << 24)
        | ((trailer[1] as u32) << 16)
        | ((trailer[2] as u32) << 8)
        | (trailer[3] as u32);
    if adler32(&out) != adler_expected {
        return Err(DecodeError::GzipCrcMismatch);
    }
    Ok(out)
}

pub fn http_deflate_inflate(data: &[u8]) -> Result<Vec<u8>, DecodeError> {
    http_deflate_inflate_with_limit(data, MAX_OUTPUT)
}

pub fn http_deflate_inflate_with_limit(
    data: &[u8],
    max_output: usize,
) -> Result<Vec<u8>, DecodeError> {
    match zlib_inflate_with_limit(data, max_output) {
        Ok(v) => Ok(v),
        Err(_) => inflate_with_limit(data, max_output),
    }
}

const STORED_MAX: usize = 0xFFFF;

pub fn deflate_stored(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 5 * (data.len() / STORED_MAX + 1));
    if data.is_empty() {

        out.push(0x01);
        out.extend_from_slice(&[0x00, 0x00, 0xFF, 0xFF]);
        return out;
    }
    let mut i = 0;
    while i < data.len() {
        let chunk_len = std::cmp::min(STORED_MAX, data.len() - i);
        let is_final = i + chunk_len == data.len();

        out.push(if is_final { 0x01 } else { 0x00 });
        let len = chunk_len as u16;
        out.push((len & 0xFF) as u8);
        out.push((len >> 8) as u8);
        out.push((!len & 0xFF) as u8);
        out.push((!len >> 8) as u8);
        out.extend_from_slice(&data[i..i + chunk_len]);
        i += chunk_len;
    }
    out
}

pub fn zlib_deflate_stored(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 11);

    out.push(0x78);

    out.push(0x01);
    out.extend_from_slice(&deflate_stored(data));
    let a = adler32(data);
    out.push((a >> 24) as u8);
    out.push((a >> 16) as u8);
    out.push((a >> 8) as u8);
    out.push((a >> 0) as u8);
    out
}

pub fn gzip_deflate_stored(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 18);
    out.extend_from_slice(&[
        0x1f, 0x8b,
        0x08,
        0x00,
        0x00, 0x00, 0x00, 0x00,
        0x00,
        0xff,
    ]);
    out.extend_from_slice(&deflate_stored(data));
    let c = crc32(data);
    out.push((c >> 0) as u8);
    out.push((c >> 8) as u8);
    out.push((c >> 16) as u8);
    out.push((c >> 24) as u8);
    let isize_le = (data.len() as u32) & 0xFFFFFFFF;
    out.push((isize_le >> 0) as u8);
    out.push((isize_le >> 8) as u8);
    out.push((isize_le >> 16) as u8);
    out.push((isize_le >> 24) as u8);
    out
}
