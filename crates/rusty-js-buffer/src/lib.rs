
pub mod num;
pub mod ops;
pub mod pool;
pub use num::NumError;
pub use ops::SwapError;
pub use pool::{BufferPool, PooledBuffer, DEFAULT_POOL_SIZE};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Latin1,
    Hex,
    Base64,

    Base64Url,
    Ascii,
    Ucs2,
}

impl Encoding {
    pub fn from_name(name: Option<&str>) -> Option<Self> {
        match name.unwrap_or("utf8").to_ascii_lowercase().as_str() {
            "utf8" | "utf-8" => Some(Self::Utf8),
            "latin1" | "binary" => Some(Self::Latin1),
            "hex" => Some(Self::Hex),
            "base64" => Some(Self::Base64),
            "base64url" => Some(Self::Base64Url),
            "ascii" => Some(Self::Ascii),
            "ucs2" | "ucs-2" | "utf16le" | "utf-16le" => Some(Self::Ucs2),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Buffer {
    bytes: Vec<u8>,
}

impl Buffer {
    pub fn from_string(s: &str, encoding: Encoding) -> Self {
        Self {
            bytes: encode(s, encoding),
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            bytes: bytes.to_vec(),
        }
    }

    pub fn from_array_buffer(bytes: &[u8]) -> Self {
        Self::from_bytes(bytes)
    }

    pub fn from_uint8_array(bytes: &[u8]) -> Self {
        Self::from_bytes(bytes)
    }

    pub fn from_buffer(buffer: &Buffer) -> Self {
        buffer.clone()
    }

    pub fn alloc(size: usize) -> Self {
        Self {
            bytes: vec![0; size],
        }
    }

    pub fn alloc_filled(size: usize, fill: &[u8]) -> Self {
        if fill.is_empty() {
            return Self::alloc(size);
        }
        let mut bytes = Vec::with_capacity(size);
        for i in 0..size {
            bytes.push(fill[i % fill.len()]);
        }
        Self { bytes }
    }

    pub fn alloc_unsafe(size: usize) -> Self {
        Self::alloc(size)
    }

    pub fn byte_length(s: &str, encoding: Encoding) -> usize {
        encode(s, encoding).len()
    }

    pub fn to_string(&self, encoding: Encoding) -> String {
        decode(&self.bytes, encoding)
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    pub fn is_uint8array_family(&self) -> bool {
        true
    }

    pub fn concat(list: &[Buffer], total_length: Option<usize>) -> Self {
        if list.is_empty() {
            return Self::alloc(0);
        }
        let sum: usize = list.iter().map(Buffer::len).sum();
        let total = total_length.unwrap_or(sum);
        let mut bytes = vec![0u8; total];
        let mut pos = 0;
        for b in list {
            if pos >= total {
                break;
            }
            let take = b.len().min(total - pos);
            bytes[pos..pos + take].copy_from_slice(&b.bytes[..take]);
            pos += take;
        }
        Self { bytes }
    }

    pub fn get(&self, index: i64) -> Option<u8> {
        if index < 0 {
            return None;
        }
        self.bytes.get(index as usize).copied()
    }

    pub fn set(&mut self, index: i64, value: i32) -> Option<u8> {
        if index < 0 {
            return None;
        }
        let idx = index as usize;
        if idx >= self.bytes.len() {
            return None;
        }
        let byte = (value & 0xFF) as u8;
        self.bytes[idx] = byte;
        Some(byte)
    }

    pub fn copy(
        &self,
        target: &mut Buffer,
        target_start: usize,
        source_start: usize,
        source_end: usize,
    ) -> usize {
        let src_start = source_start.min(self.bytes.len());
        let src_end = source_end.clamp(src_start, self.bytes.len());
        let tgt_start = target_start.min(target.bytes.len());
        let avail_src = src_end - src_start;
        let avail_tgt = target.bytes.len() - tgt_start;
        let n = avail_src.min(avail_tgt);
        if n == 0 {
            return 0;
        }

        let snapshot = self.bytes[src_start..src_start + n].to_vec();
        target.bytes[tgt_start..tgt_start + n].copy_from_slice(&snapshot);
        n
    }

    pub fn fill_byte(&mut self, value: u8, offset: usize, end: usize) {
        let start = offset.min(self.bytes.len());
        let stop = end.clamp(start, self.bytes.len());
        for b in &mut self.bytes[start..stop] {
            *b = value;
        }
    }

    pub fn fill_bytes(&mut self, pattern: &[u8], offset: usize, end: usize) {
        if pattern.is_empty() {
            return;
        }
        let start = offset.min(self.bytes.len());
        let stop = end.clamp(start, self.bytes.len());
        for (k, b) in self.bytes[start..stop].iter_mut().enumerate() {
            *b = pattern[k % pattern.len()];
        }
    }

    pub fn write(&mut self, src: &[u8], offset: usize, length: Option<usize>) -> usize {
        let off = offset.min(self.bytes.len());
        let remaining = self.bytes.len() - off;
        let n = src.len().min(remaining).min(length.unwrap_or(remaining));
        if n == 0 {
            return 0;
        }
        self.bytes[off..off + n].copy_from_slice(&src[..n]);
        n
    }

    pub fn write_string(
        &mut self,
        s: &str,
        offset: usize,
        length: Option<usize>,
        encoding: Encoding,
    ) -> usize {
        let encoded = encode(s, encoding);
        self.write(&encoded, offset, length)
    }

    pub fn to_shared(&self) -> PooledBuffer {
        PooledBuffer::standalone(self.bytes.clone())
    }

    pub fn concat_pooled(
        pool: &mut BufferPool,
        list: &[Buffer],
        total_length: Option<usize>,
    ) -> PooledBuffer {
        let sum: usize = list.iter().map(Buffer::len).sum();
        let total = total_length.unwrap_or(sum);
        let out = pool.alloc_unsafe(total);
        let mut pos = 0;
        for b in list {
            if pos >= total {
                break;
            }
            let take = b.len().min(total - pos);
            out.copy_at(pos, &b.bytes[..take]);
            pos += take;
        }

        out.zero_range(pos, total);
        out
    }
}

pub fn encode(s: &str, encoding: Encoding) -> Vec<u8> {
    match encoding {
        Encoding::Utf8 => s.as_bytes().to_vec(),
        Encoding::Latin1 => s.chars().map(|c| (c as u32 & 0xff) as u8).collect(),
        Encoding::Hex => decode_hex(s),
        Encoding::Base64 => decode_base64(s),

        Encoding::Base64Url => decode_base64(&normalize_base64url(s)),
        Encoding::Ascii => s.chars().map(|c| (c as u32 & 0xff) as u8).collect(),

        Encoding::Ucs2 => {
            let mut out = Vec::new();
            for u in s.encode_utf16() {
                out.extend_from_slice(&u.to_le_bytes());
            }
            out
        }
    }
}

pub fn decode(bytes: &[u8], encoding: Encoding) -> String {
    match encoding {
        Encoding::Utf8 => String::from_utf8_lossy(bytes).into_owned(),
        Encoding::Latin1 => bytes.iter().map(|&b| b as char).collect(),
        Encoding::Hex => encode_hex(bytes),
        Encoding::Base64 => encode_base64(bytes),

        Encoding::Base64Url => base64_to_base64url(&encode_base64(bytes)),
        Encoding::Ascii => bytes.iter().map(|&b| (b & 0x7f) as char).collect(),

        Encoding::Ucs2 => {
            let units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16_lossy(&units)
        }
    }
}

fn normalize_base64url(s: &str) -> String {
    s.replace('-', "+").replace('_', "/")
}

fn base64_to_base64url(s: &str) -> String {
    s.replace('+', "-").replace('/', "_").replace('=', "")
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn decode_hex(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    let mut i = 0;
    while i + 1 < bytes.len() {
        match (hex_val(bytes[i]), hex_val(bytes[i + 1])) {
            (Some(hi), Some(lo)) => out.push((hi << 4) | lo),
            _ => break,
        }
        i += 2;
    }
    out
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn b64_val(b: u8) -> Option<u8> {
    match b {
        b'A'..=b'Z' => Some(b - b'A'),
        b'a'..=b'z' => Some(b - b'a' + 26),
        b'0'..=b'9' => Some(b - b'0' + 52),
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        _ => None,
    }
}

fn decode_base64(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut quartet = [0u8; 4];
    let mut q_len = 0usize;
    let mut pad = 0usize;

    for b in s.bytes() {
        if b.is_ascii_whitespace() {
            continue;
        }
        if b == b'=' {
            quartet[q_len] = 0;
            q_len += 1;
            pad += 1;
            if q_len == 4 {
                out.push((quartet[0] << 2) | (quartet[1] >> 4));
                if pad < 2 {
                    out.push((quartet[1] << 4) | (quartet[2] >> 2));
                }
                if pad == 0 {
                    out.push((quartet[2] << 6) | quartet[3]);
                }
                q_len = 0;
                pad = 0;
            }
            break;
        } else if let Some(v) = b64_val(b) {
            quartet[q_len] = v;
            q_len += 1;
        } else {

            continue;
        }
        if q_len == 4 {
            out.push((quartet[0] << 2) | (quartet[1] >> 4));
            if pad < 2 {
                out.push((quartet[1] << 4) | (quartet[2] >> 2));
            }
            if pad == 0 {
                out.push((quartet[2] << 6) | quartet[3]);
            }
            q_len = 0;
            pad = 0;
        }
    }

    let data_len = q_len.saturating_sub(pad);
    if data_len >= 2 {
        for slot in quartet.iter_mut().skip(q_len) {
            *slot = 0;
        }
        out.push((quartet[0] << 2) | (quartet[1] >> 4));
        if data_len >= 3 {
            out.push((quartet[1] << 4) | (quartet[2] >> 2));
        }
    }

    out
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let mut i = 0;
    while i < bytes.len() {
        let b0 = bytes[i];
        let b1 = *bytes.get(i + 1).unwrap_or(&0);
        let b2 = *bytes.get(i + 2).unwrap_or(&0);
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if i + 1 < bytes.len() {
            out.push(TABLE[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if i + 2 < bytes.len() {
            out.push(TABLE[(b2 & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        i += 3;
    }
    out
}
