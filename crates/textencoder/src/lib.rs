
use std::fmt;

pub struct TextEncoder;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncodeIntoResult {
    pub read: usize,
    pub written: usize,
}

impl TextEncoder {
    pub fn new() -> Self {
        TextEncoder
    }

    pub fn encoding(&self) -> &'static str {
        "utf-8"
    }

    pub fn encode(&self, input: Option<&str>) -> Vec<u8> {
        match input {
            None => Vec::new(),
            Some(s) => s.as_bytes().to_vec(),
        }
    }

    pub fn encode_into(&self, source: &str, destination: &mut [u8]) -> EncodeIntoResult {
        let mut written = 0usize;
        let mut read_utf16 = 0usize;
        for ch in source.chars() {
            let utf8_len = ch.len_utf8();
            if written + utf8_len > destination.len() {
                break;
            }
            ch.encode_utf8(&mut destination[written..]);
            written += utf8_len;
            read_utf16 += ch.len_utf16();
        }
        EncodeIntoResult {
            read: read_utf16,
            written,
        }
    }
}

impl Default for TextEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TextEncoder {

    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[object TextEncoder]")
    }
}

#[derive(Debug, Clone)]
pub struct TextDecoder {
    encoding: &'static str,
    fatal: bool,
    ignore_bom: bool,

    pending: Vec<u8>,

    bom_consumed: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct TextDecoderOptions {
    pub fatal: bool,
    pub ignore_bom: bool,
}

impl Default for TextDecoderOptions {
    fn default() -> Self {
        Self {
            fatal: false,
            ignore_bom: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TextDecodeOptions {
    pub stream: bool,
}

impl Default for TextDecodeOptions {
    fn default() -> Self {
        Self { stream: false }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecoderError {

    UnknownEncoding(String),

    InvalidSequence,
}

impl TextDecoder {

    pub fn new(label: Option<&str>, options: TextDecoderOptions) -> Result<Self, DecoderError> {
        let resolved = resolve_label(label.unwrap_or("utf-8"))?;
        Ok(TextDecoder {
            encoding: resolved,
            fatal: options.fatal,
            ignore_bom: options.ignore_bom,
            pending: Vec::new(),
            bom_consumed: false,
        })
    }

    pub fn encoding(&self) -> &'static str {
        self.encoding
    }
    pub fn fatal(&self) -> bool {
        self.fatal
    }
    pub fn ignore_bom(&self) -> bool {
        self.ignore_bom
    }

    pub fn decode(
        &mut self,
        input: &[u8],
        options: TextDecodeOptions,
    ) -> Result<String, DecoderError> {

        let mut buf: Vec<u8> = Vec::with_capacity(self.pending.len() + input.len());
        buf.extend_from_slice(&self.pending);
        buf.extend_from_slice(input);
        self.pending.clear();

        let mut start = 0;
        if !self.bom_consumed && !self.ignore_bom && self.encoding == "utf-8" {
            if buf.len() >= 3 && &buf[..3] == [0xEF, 0xBB, 0xBF] {
                start = 3;
                self.bom_consumed = true;
            } else if options.stream && (buf == [0xEF].as_slice() || buf == [0xEF, 0xBB].as_slice())
            {
                self.pending = buf;
                return Ok(String::new());
            } else {
                self.bom_consumed = true;
            }
        }
        let body = &buf[start..];

        let (decoded, retained) = utf8_decode(body, self.fatal, options.stream)?;
        if options.stream {
            self.pending = retained;
        } else if !retained.is_empty() {

            if self.fatal {
                return Err(DecoderError::InvalidSequence);
            }

            let mut s = decoded;
            for _ in &retained {
                s.push('\u{FFFD}');
            }
            return Ok(s);
        }
        Ok(decoded)
    }
}

fn resolve_label(label: &str) -> Result<&'static str, DecoderError> {
    let l = label.trim().to_ascii_lowercase();
    match l.as_str() {
        "utf-8" | "utf8" | "unicode-1-1-utf-8" | "unicode11utf8" | "unicode20utf8"
        | "x-unicode20utf8" => Ok("utf-8"),
        _ => Err(DecoderError::UnknownEncoding(label.to_string())),
    }
}

fn utf8_decode(
    bytes: &[u8],
    fatal: bool,
    _stream: bool,
) -> Result<(String, Vec<u8>), DecoderError> {
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        let need = if b < 0x80 {
            1
        } else if b & 0xE0 == 0xC0 {
            2
        } else if b & 0xF0 == 0xE0 {
            3
        } else if b & 0xF8 == 0xF0 {
            4
        } else {
            if fatal {
                return Err(DecoderError::InvalidSequence);
            }
            out.push('\u{FFFD}');
            i += 1;
            continue;
        };
        if i + need > bytes.len() {

            let retained = bytes[i..].to_vec();
            return Ok((out, retained));
        }
        let seq = &bytes[i..i + need];
        match std::str::from_utf8(seq) {
            Ok(s) => out.push_str(s),
            Err(_) => {
                if fatal {
                    return Err(DecoderError::InvalidSequence);
                }
                out.push('\u{FFFD}');
            }
        }
        i += need;
    }
    Ok((out, Vec::new()))
}

impl fmt::Display for TextDecoder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[object TextDecoder]")
    }
}
