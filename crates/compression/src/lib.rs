
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
    Brotli(String),

    BrotliTruncated,
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
            DecodeError::Brotli(s) => write!(f, "brotli: {}", s),
            DecodeError::BrotliTruncated => write!(f, "unexpected end of file"),
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

    fn position(&self) -> usize {
        self.byte_pos
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
    inflate_with_limit_consumed(data, max_output).map(|(out, _)| out)
}

pub fn inflate_with_limit_consumed(
    data: &[u8],
    max_output: usize,
) -> Result<(Vec<u8>, usize), DecodeError> {
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
    br.align_to_byte();
    Ok((out, br.byte_pos))
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

    let mut result = Vec::new();
    let mut offset = 0;
    while offset + 2 <= data.len() && data[offset] == 0x1f && data[offset + 1] == 0x8b {
        let remaining_budget = max_output.saturating_sub(result.len());
        let (out, consumed) = gunzip_member(&data[offset..], remaining_budget)?;
        result.extend_from_slice(&out);
        offset += consumed;
    }
    if offset == 0 {

        return gunzip_member(data, max_output).map(|(out, _)| out);
    }
    Ok(result)
}

fn gunzip_member(data: &[u8], max_output: usize) -> Result<(Vec<u8>, usize), DecodeError> {
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

    if data.len() < p {
        return Err(DecodeError::UnexpectedEnd);
    }
    let (out, deflate_consumed) = inflate_with_limit_consumed(&data[p..], max_output)?;
    let trailer_start = p + deflate_consumed;
    if data.len() < trailer_start + 8 {
        return Err(DecodeError::UnexpectedEnd);
    }
    let trailer = &data[trailer_start..trailer_start + 8];

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
    Ok((out, trailer_start + 8))
}

fn adler32(data: &[u8]) -> u32 {
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

struct BitWriter {
    out: Vec<u8>,
    bit_buf: u32,
    bit_cnt: u32,
}
impl BitWriter {
    fn new() -> Self {
        BitWriter {
            out: Vec::new(),
            bit_buf: 0,
            bit_cnt: 0,
        }
    }
    fn write_bits(&mut self, val: u32, n: u32) {
        self.bit_buf |= val << self.bit_cnt;
        self.bit_cnt += n;
        while self.bit_cnt >= 8 {
            self.out.push((self.bit_buf & 0xFF) as u8);
            self.bit_buf >>= 8;
            self.bit_cnt -= 8;
        }
    }
    fn write_huff(&mut self, code: u32, n: u32) {
        for i in (0..n).rev() {
            self.write_bits((code >> i) & 1, 1);
        }
    }
    fn finish(mut self) -> Vec<u8> {
        if self.bit_cnt > 0 {
            self.out.push((self.bit_buf & 0xFF) as u8);
        }
        self.out
    }
}

fn fixed_litlen(sym: u16) -> (u32, u32) {
    match sym {
        0..=143 => (0x30 + sym as u32, 8),
        144..=255 => (0x190 + (sym as u32 - 144), 9),
        256..=279 => (sym as u32 - 256, 7),
        _ => (0xC0 + (sym as u32 - 280), 8),
    }
}
fn enc_length_code(len: usize) -> usize {
    let mut c = 0;
    for (i, &b) in LENGTH_BASE.iter().enumerate() {
        if (b as usize) <= len {
            c = i;
        } else {
            break;
        }
    }
    c
}
fn enc_distance_code(dist: usize) -> usize {
    let mut c = 0;
    for (i, &b) in DISTANCE_BASE.iter().enumerate() {
        if (b as usize) <= dist {
            c = i;
        } else {
            break;
        }
    }
    c
}

const ENC_MIN_MATCH: usize = 3;
const ENC_MAX_MATCH: usize = 258;
const ENC_WSIZE: usize = 32768;

fn enc_hash3(d: &[u8], i: usize) -> usize {
    (((d[i] as usize) << 10) ^ ((d[i + 1] as usize) << 5) ^ (d[i + 2] as usize)) & 0x7FFF
}

pub fn deflate_fixed(data: &[u8]) -> Vec<u8> {
    let mut bw = BitWriter::new();
    bw.write_bits(1, 1);
    bw.write_bits(1, 2);
    if data.len() < ENC_MIN_MATCH {
        for &b in data {
            let (c, n) = fixed_litlen(b as u16);
            bw.write_huff(c, n);
        }
        let (c, n) = fixed_litlen(256);
        bw.write_huff(c, n);
        return bw.finish();
    }
    let mut head = vec![-1i32; 1 << 15];
    let mut prev = vec![-1i32; data.len()];
    let mut i = 0;
    while i < data.len() {
        let mut best_len = 0usize;
        let mut best_dist = 0usize;
        if i + ENC_MIN_MATCH <= data.len() {
            let h = enc_hash3(data, i);
            let mut j = head[h];
            let mut chain = 0;
            while j >= 0 && (i - j as usize) <= ENC_WSIZE && chain < 256 {
                let jj = j as usize;
                let max = (data.len() - i).min(ENC_MAX_MATCH);
                let mut l = 0;
                while l < max && data[jj + l] == data[i + l] {
                    l += 1;
                }
                if l > best_len {
                    best_len = l;
                    best_dist = i - jj;
                    if l >= max {
                        break;
                    }
                }
                j = prev[jj];
                chain += 1;
            }
        }
        if best_len >= ENC_MIN_MATCH {
            let lc = enc_length_code(best_len);
            let (c, n) = fixed_litlen(257 + lc as u16);
            bw.write_huff(c, n);
            bw.write_bits(
                (best_len - LENGTH_BASE[lc] as usize) as u32,
                LENGTH_EXTRA[lc] as u32,
            );
            let dc = enc_distance_code(best_dist);
            bw.write_huff(dc as u32, 5);
            bw.write_bits(
                (best_dist - DISTANCE_BASE[dc] as usize) as u32,
                DISTANCE_EXTRA[dc] as u32,
            );
            let end = i + best_len;
            while i < end {
                if i + ENC_MIN_MATCH <= data.len() {
                    let h = enc_hash3(data, i);
                    prev[i] = head[h];
                    head[h] = i as i32;
                }
                i += 1;
            }
        } else {
            let (c, n) = fixed_litlen(data[i] as u16);
            bw.write_huff(c, n);
            if i + ENC_MIN_MATCH <= data.len() {
                let h = enc_hash3(data, i);
                prev[i] = head[h];
                head[h] = i as i32;
            }
            i += 1;
        }
    }
    let (c, n) = fixed_litlen(256);
    bw.write_huff(c, n);
    bw.finish()
}

pub fn deflate_fixed_literals(data: &[u8]) -> Vec<u8> {
    let mut bw = BitWriter::new();
    bw.write_bits(1, 1);
    bw.write_bits(1, 2);
    for &b in data {
        let (c, n) = fixed_litlen(b as u16);
        bw.write_huff(c, n);
    }
    let (c, n) = fixed_litlen(256);
    bw.write_huff(c, n);
    bw.finish()
}

fn small_best_match(data: &[u8], i: usize) -> (usize, usize) {
    let mut best_len = 0usize;
    let mut best_dist = 0usize;
    for j in 0..i {
        let max = (data.len() - i).min(ENC_MAX_MATCH);
        let mut len = 0usize;
        while len < max && data[j + len] == data[i + len] {
            len += 1;
        }
        if len > best_len {
            best_len = len;
            best_dist = i - j;
        }
    }
    if best_len >= ENC_MIN_MATCH {
        (best_len, best_dist)
    } else {
        (0, 0)
    }
}

fn write_fixed_literal(bw: &mut BitWriter, byte: u8) {
    let (c, n) = fixed_litlen(byte as u16);
    bw.write_huff(c, n);
}

fn write_fixed_match(bw: &mut BitWriter, len: usize, dist: usize) {
    let lc = enc_length_code(len);
    let (c, n) = fixed_litlen(257 + lc as u16);
    bw.write_huff(c, n);
    bw.write_bits(
        (len - LENGTH_BASE[lc] as usize) as u32,
        LENGTH_EXTRA[lc] as u32,
    );
    let dc = enc_distance_code(dist);
    bw.write_huff(dc as u32, 5);
    bw.write_bits(
        (dist - DISTANCE_BASE[dc] as usize) as u32,
        DISTANCE_EXTRA[dc] as u32,
    );
}

fn deflate_fixed_small_lazy(data: &[u8]) -> Vec<u8> {
    let mut bw = BitWriter::new();
    bw.write_bits(1, 1);
    bw.write_bits(1, 2);
    let mut i = 0usize;
    let mut deferred_match = false;
    while i < data.len() {
        let (best_len, best_dist) = small_best_match(data, i);
        if best_len >= ENC_MIN_MATCH {
            let (next_len, _) = if i + 1 < data.len() {
                small_best_match(data, i + 1)
            } else {
                (0, 0)
            };
            if !deferred_match && next_len + 1 >= best_len {
                write_fixed_literal(&mut bw, data[i]);
                i += 1;
                deferred_match = true;
                continue;
            }
            write_fixed_match(&mut bw, best_len, best_dist);
            i += best_len;
            deferred_match = false;
        } else {
            write_fixed_literal(&mut bw, data[i]);
            i += 1;
            deferred_match = false;
        }
    }
    let (c, n) = fixed_litlen(256);
    bw.write_huff(c, n);
    bw.finish()
}

const CL_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

enum Tok {
    Lit(u8),
    Match { len: u16, dist: u16 },
}

fn lz77_tokens(data: &[u8]) -> Vec<Tok> {
    let mut toks = Vec::new();
    if data.len() < ENC_MIN_MATCH {
        toks.extend(data.iter().map(|&b| Tok::Lit(b)));
        return toks;
    }
    let mut head = vec![-1i32; 1 << 15];
    let mut prev = vec![-1i32; data.len()];
    let mut i = 0;
    while i < data.len() {
        let mut best_len = 0usize;
        let mut best_dist = 0usize;
        if i + ENC_MIN_MATCH <= data.len() {
            let h = enc_hash3(data, i);
            let mut j = head[h];
            let mut chain = 0;
            while j >= 0 && (i - j as usize) <= ENC_WSIZE && chain < 256 {
                let jj = j as usize;
                let max = (data.len() - i).min(ENC_MAX_MATCH);
                let mut l = 0;
                while l < max && data[jj + l] == data[i + l] {
                    l += 1;
                }
                if l > best_len {
                    best_len = l;
                    best_dist = i - jj;
                    if l >= max {
                        break;
                    }
                }
                j = prev[jj];
                chain += 1;
            }
        }
        if best_len >= ENC_MIN_MATCH {
            toks.push(Tok::Match {
                len: best_len as u16,
                dist: best_dist as u16,
            });
            let end = i + best_len;
            while i < end {
                if i + ENC_MIN_MATCH <= data.len() {
                    let h = enc_hash3(data, i);
                    prev[i] = head[h];
                    head[h] = i as i32;
                }
                i += 1;
            }
        } else {
            toks.push(Tok::Lit(data[i]));
            if i + ENC_MIN_MATCH <= data.len() {
                let h = enc_hash3(data, i);
                prev[i] = head[h];
                head[h] = i as i32;
            }
            i += 1;
        }
    }
    toks
}

fn package_merge(freqs: &[usize], max_bits: usize) -> Vec<u8> {
    let n = freqs.len();
    let mut lengths = vec![0u8; n];
    let syms: Vec<usize> = (0..n).filter(|&i| freqs[i] > 0).collect();
    if syms.is_empty() {
        return lengths;
    }
    if syms.len() == 1 {
        lengths[syms[0]] = 1;
        return lengths;
    }
    #[derive(Clone)]
    struct Coin {
        weight: usize,
        syms: Vec<usize>,
    }
    let mut leaves: Vec<Coin> = syms
        .iter()
        .map(|&s| Coin {
            weight: freqs[s],
            syms: vec![s],
        })
        .collect();
    leaves.sort_by_key(|c| c.weight);
    let mut prev = leaves.clone();
    for _ in 0..max_bits - 1 {
        let mut packaged: Vec<Coin> = Vec::new();
        let mut i = 0;
        while i + 1 < prev.len() {
            let mut sv = prev[i].syms.clone();
            sv.extend_from_slice(&prev[i + 1].syms);
            packaged.push(Coin {
                weight: prev[i].weight + prev[i + 1].weight,
                syms: sv,
            });
            i += 2;
        }
        let mut merged = leaves.clone();
        merged.extend(packaged);
        merged.sort_by_key(|c| c.weight);
        prev = merged;
    }
    let take = 2 * syms.len() - 2;
    for coin in prev.iter().take(take) {
        for &s in &coin.syms {
            lengths[s] += 1;
        }
    }
    lengths
}

fn canonical_codes(lengths: &[u8]) -> Vec<u32> {
    let max_bits = *lengths.iter().max().unwrap_or(&0) as usize;
    let mut bl_count = vec![0u32; max_bits + 1];
    for &l in lengths {
        if l > 0 {
            bl_count[l as usize] += 1;
        }
    }
    let mut next_code = vec![0u32; max_bits + 2];
    let mut code = 0u32;
    for bits in 1..=max_bits {
        code = (code + bl_count[bits - 1]) << 1;
        next_code[bits] = code;
    }
    let mut codes = vec![0u32; lengths.len()];
    for (i, &l) in lengths.iter().enumerate() {
        if l > 0 {
            codes[i] = next_code[l as usize];
            next_code[l as usize] += 1;
        }
    }
    codes
}

pub fn deflate_dynamic(data: &[u8]) -> Vec<u8> {
    let toks = lz77_tokens(data);
    let mut ll_freq = vec![0usize; 286];
    let mut d_freq = vec![0usize; 30];
    for t in &toks {
        match t {
            Tok::Lit(b) => ll_freq[*b as usize] += 1,
            Tok::Match { len, dist } => {
                ll_freq[257 + enc_length_code(*len as usize)] += 1;
                d_freq[enc_distance_code(*dist as usize)] += 1;
            }
        }
    }
    ll_freq[256] += 1;

    let mut ll_len = package_merge(&ll_freq, 15);
    let mut d_len = package_merge(&d_freq, 15);

    if d_len.iter().all(|&l| l == 0) {
        d_len[0] = 1;
    }

    let hlit = (257..=286)
        .rev()
        .find(|&i| ll_len[i - 1] != 0)
        .unwrap_or(257)
        .max(257);
    let hdist = (1..=30)
        .rev()
        .find(|&i| d_len[i - 1] != 0)
        .unwrap_or(1)
        .max(1);
    ll_len.truncate(hlit);
    d_len.truncate(hdist);

    let ll_codes = canonical_codes(&ll_len);
    let d_codes = canonical_codes(&d_len);

    let mut all_len: Vec<u8> = Vec::new();
    all_len.extend_from_slice(&ll_len);
    all_len.extend_from_slice(&d_len);

    let mut cl_syms: Vec<(u8, u32, u32)> = Vec::new();
    let mut cl_freq = vec![0usize; 19];
    let mut i = 0;
    while i < all_len.len() {
        let v = all_len[i];
        let mut run = 1;
        while i + run < all_len.len() && all_len[i + run] == v {
            run += 1;
        }
        if v == 0 {

            let mut r = run;
            while r >= 11 {
                let take = r.min(138);
                cl_syms.push((18, (take - 11) as u32, 7));
                cl_freq[18] += 1;
                r -= take;
            }
            while r >= 3 {
                let take = r.min(10);
                cl_syms.push((17, (take - 3) as u32, 3));
                cl_freq[17] += 1;
                r -= take;
            }
            for _ in 0..r {
                cl_syms.push((0, 0, 0));
                cl_freq[0] += 1;
            }
        } else {

            cl_syms.push((v, 0, 0));
            cl_freq[v as usize] += 1;
            let mut r = run - 1;
            while r >= 3 {
                let take = r.min(6);
                cl_syms.push((16, (take - 3) as u32, 2));
                cl_freq[16] += 1;
                r -= take;
            }
            for _ in 0..r {
                cl_syms.push((v, 0, 0));
                cl_freq[v as usize] += 1;
            }
        }
        i += run;
    }
    let cl_len = package_merge(&cl_freq, 7);
    let cl_codes = canonical_codes(&cl_len);

    let hclen = (4..=19)
        .rev()
        .find(|&n| cl_len[CL_ORDER[n - 1]] != 0)
        .unwrap_or(4)
        .max(4);

    let mut bw = BitWriter::new();
    bw.write_bits(1, 1);
    bw.write_bits(2, 2);
    bw.write_bits((hlit - 257) as u32, 5);
    bw.write_bits((hdist - 1) as u32, 5);
    bw.write_bits((hclen - 4) as u32, 4);
    for k in 0..hclen {
        bw.write_bits(cl_len[CL_ORDER[k]] as u32, 3);
    }
    for &(sym, extra, nbits) in &cl_syms {
        bw.write_huff(cl_codes[sym as usize], cl_len[sym as usize] as u32);
        if nbits > 0 {
            bw.write_bits(extra, nbits);
        }
    }

    for t in &toks {
        match t {
            Tok::Lit(b) => bw.write_huff(ll_codes[*b as usize], ll_len[*b as usize] as u32),
            Tok::Match { len, dist } => {
                let lc = enc_length_code(*len as usize);
                let sym = 257 + lc;
                bw.write_huff(ll_codes[sym], ll_len[sym] as u32);
                bw.write_bits(
                    (*len as usize - LENGTH_BASE[lc] as usize) as u32,
                    LENGTH_EXTRA[lc] as u32,
                );
                let dc = enc_distance_code(*dist as usize);
                bw.write_huff(d_codes[dc], d_len[dc] as u32);
                bw.write_bits(
                    (*dist as usize - DISTANCE_BASE[dc] as usize) as u32,
                    DISTANCE_EXTRA[dc] as u32,
                );
            }
        }
    }
    bw.write_huff(ll_codes[256], ll_len[256] as u32);
    bw.finish()
}

pub fn deflate_best(data: &[u8]) -> Vec<u8> {
    let dynamic = deflate_dynamic(data);
    let fixed = deflate_fixed(data);
    let mut best = if dynamic.len() <= fixed.len() {
        dynamic
    } else {
        fixed
    };
    let stored = deflate_stored(data);
    if stored.len() < best.len() {
        best = stored;
    }
    best
}

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
    out.extend_from_slice(&deflate_best(data));
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
    out.extend_from_slice(&deflate_best(data));
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

pub fn gzip_deflate_node_default(data: &[u8]) -> Vec<u8> {
    let all_same_byte = data
        .first()
        .is_some_and(|first| data.iter().all(|b| b == first));
    let deflated = if data.len() <= 16 && !all_same_byte {
        deflate_fixed_small_lazy(data)
    } else {
        deflate_best(data)
    };

    let mut out = Vec::with_capacity(deflated.len() + 18);
    out.extend_from_slice(&[
        0x1f, 0x8b,
        0x08,
        0x00,
        0x00, 0x00, 0x00, 0x00,
        0x00,
        0x03,
    ]);
    out.extend_from_slice(&deflated);
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

fn map_brotli_error(e: rusty_js_brotli::BrotliError) -> DecodeError {

    match e {
        rusty_js_brotli::BrotliError::UnexpectedEnd
        | rusty_js_brotli::BrotliError::UnsupportedStream => DecodeError::BrotliTruncated,
        other => DecodeError::Brotli(format!("{other}")),
    }
}

pub fn brotli_decode(data: &[u8]) -> Result<Vec<u8>, DecodeError> {
    rusty_js_brotli::decode(data).map_err(map_brotli_error)
}

pub fn brotli_decode_with_limit(data: &[u8], max_output: usize) -> Result<Vec<u8>, DecodeError> {
    rusty_js_brotli::decode_with_limit(data, max_output).map_err(map_brotli_error)
}

pub fn brotli_encode(data: &[u8], quality: u32, lgwin: u32) -> Result<Vec<u8>, DecodeError> {
    brotli_encode_params(
        data,
        &BrotliParams {
            quality,
            lgwin,
            ..Default::default()
        },
    )
}

pub fn brotli_compress(data: &[u8]) -> Result<Vec<u8>, DecodeError> {
    brotli_encode(data, 11, 22)
}

#[derive(Clone, Copy, Debug)]
pub struct BrotliParams {
    pub quality: u32,
    pub lgwin: u32,
    pub mode: u32,
    pub size_hint: usize,
    pub large_window: bool,
}

impl Default for BrotliParams {
    fn default() -> Self {

        BrotliParams {
            quality: 11,
            lgwin: 22,
            mode: 0,
            size_hint: 0,
            large_window: false,
        }
    }
}

pub fn brotli_encode_params(data: &[u8], p: &BrotliParams) -> Result<Vec<u8>, DecodeError> {
    let params = rusty_js_brotli::BrotliParams {
        quality: p.quality.min(11),
        lgwin: p.lgwin.clamp(10, if p.large_window { 30 } else { 24 }),
        mode: p.mode,
        size_hint: p.size_hint,
        large_window: p.large_window,
    };
    rusty_js_brotli::encode(data, &params).map_err(|e| DecodeError::Brotli(format!("{e}")))
}

#[cfg(test)]
mod brotli_tests {
    use super::*;

    #[test]
    fn inflate_with_limit_rejects_output_bomb_before_growth() {
        let data = [0x01, 0x05, 0x00, 0xFA, 0xFF, b'h', b'e', b'l', b'l', b'o'];
        assert!(matches!(
            inflate_with_limit(&data, 4),
            Err(DecodeError::OutputTooLarge)
        ));
    }

    #[test]
    fn inflate_rejects_bad_back_reference_before_output() {
        let bad = [0x03, 0x02, 0x00];
        assert!(matches!(inflate(&bad), Err(DecodeError::DistanceTooFar)));
    }

    #[test]
    fn brotli_encode_decode_roundtrip() {

        let original = b"Hello, World!";
        let encoded = brotli_encode(original, 11, 22).expect("brotli encode");
        let decoded = brotli_decode(&encoded).expect("brotli decode");
        assert_eq!(decoded, original, "brotli encode->decode must round-trip");
    }

    #[test]
    fn brotli_compress_default_roundtrip() {
        let original = b"the quick brown fox jumps over the lazy dog, repeatedly. \
                         the quick brown fox jumps over the lazy dog, repeatedly.";
        let encoded = brotli_compress(original).expect("brotli compress");
        let decoded = brotli_decode(&encoded).expect("brotli decode");
        assert_eq!(decoded, original);
    }

    #[test]
    fn brotli_encode_empty() {
        let encoded = brotli_encode(b"", 11, 22).expect("encode empty");
        let decoded = brotli_decode(&encoded).expect("decode empty");
        assert_eq!(decoded, Vec::<u8>::new());
    }

    #[test]
    fn brotli_params_quality_is_accepted() {
        let mut original = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            original.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        original.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            original.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        original.extend_from_slice(br#""}"#);

        let q0 = brotli_encode_params(
            &original,
            &BrotliParams {
                quality: 0,
                ..Default::default()
            },
        )
        .expect("q0");
        let q11 = brotli_encode_params(&original, &BrotliParams::default()).expect("q11");
        assert_ne!(q0, q11);
        assert_eq!(brotli_decode(&q0).unwrap(), original);
        assert_eq!(brotli_decode(&q11).unwrap(), original);
    }

    #[test]
    fn brotli_params_mode_and_size_hint_roundtrip() {
        let original = b"text-mode brotli content with a size hint provided to the encoder";
        for mode in 0..=2u32 {
            let enc = brotli_encode_params(
                original,
                &BrotliParams {
                    mode,
                    size_hint: original.len(),
                    ..Default::default()
                },
            )
            .unwrap_or_else(|e| panic!("mode {mode}: {e:?}"));
            assert_eq!(
                brotli_decode(&enc).unwrap(),
                original,
                "mode {mode} round-trip"
            );
        }
    }

    #[test]
    fn brotli_params_large_window_roundtrip() {

        let original = b"large window brotli frame should round-trip through brotliDecompress";
        let enc = brotli_encode_params(
            original,
            &BrotliParams {
                lgwin: 30,
                large_window: true,
                ..Default::default()
            },
        )
        .expect("large-window encode");
        assert_eq!(
            brotli_decode(&enc).unwrap(),
            original,
            "large-window round-trip"
        );
    }

    #[test]
    fn brotli_encode_quality_levels() {

        let original = b"compression quality sweep across all brotli levels 0 to 11";
        for q in 0..=11u32 {
            let encoded =
                brotli_encode(original, q, 22).unwrap_or_else(|e| panic!("encode q={q}: {e:?}"));
            let decoded = brotli_decode(&encoded).unwrap_or_else(|e| panic!("decode q={q}: {e:?}"));
            assert_eq!(decoded, original, "round-trip at quality {q}");
        }
    }

    #[test]
    fn brotli_decode_empty() {

        let empty_stream = [0x06];
        let r = brotli_decode(&empty_stream);
        assert!(r.is_ok(), "brotli empty: {:?}", r);
        assert_eq!(r.unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn brotli_decode_hello_roundtrip() {

        let encoded = [
            0x0b, 0x06, 0x80, 0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x2c, 0x20, 0x57, 0x6f, 0x72, 0x6c,
            0x64, 0x21, 0x03,
        ];
        let r = brotli_decode(&encoded).expect("brotli decode");
        assert_eq!(r, b"Hello, World!");
    }

    #[test]
    fn brotli_decode_with_limit_preserves_output_cap() {
        let encoded = [
            0x0b, 0x06, 0x80, 0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x2c, 0x20, 0x57, 0x6f, 0x72, 0x6c,
            0x64, 0x21, 0x03,
        ];
        let err = brotli_decode_with_limit(&encoded, b"Hello, World".len()).unwrap_err();
        assert!(
            matches!(&err, DecodeError::Brotli(message) if message.contains("maximum size")),
            "unexpected brotli cap error: {err:?}"
        );
    }
}
