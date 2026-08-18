
use std::str;

pub const MAX_HEADER_SECTION_BYTES: usize = 16 * 1024;
pub const MAX_HEADER_LINE_BYTES: usize = 8 * 1024;
pub const MAX_HEADER_COUNT: usize = 100;
pub const MAX_DECODED_BODY_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_CHUNK_COUNT: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRequest {
    pub method: String,
    pub target: String,
    pub version: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,

    pub trailers: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedResponse {
    pub version: String,
    pub status: u16,
    pub reason: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    Truncated,
    BadStartLine(String),
    BadHeader(String),
    BadVersion(String),
    BadStatus(String),
    BadChunkEncoding(String),
    ContentLengthMismatch,
    LimitExceeded(String),
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodecError::Truncated => write!(f, "http-codec: truncated message"),
            CodecError::BadStartLine(s) => write!(f, "http-codec: bad start line: {}", s),
            CodecError::BadHeader(s) => write!(f, "http-codec: bad header: {}", s),
            CodecError::BadVersion(s) => write!(f, "http-codec: bad version: {}", s),
            CodecError::BadStatus(s) => write!(f, "http-codec: bad status: {}", s),
            CodecError::BadChunkEncoding(s) => write!(f, "http-codec: bad chunk-encoding: {}", s),
            CodecError::ContentLengthMismatch => write!(f, "http-codec: content-length mismatch"),
            CodecError::LimitExceeded(s) => write!(f, "http-codec: limit exceeded: {}", s),
        }
    }
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    find_header_end_from(bytes, 0)
}

fn find_header_end_from(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.len() < 4 {
        return None;
    }
    let needle = b"\r\n\r\n";
    let start = start.min(bytes.len().saturating_sub(4));
    for i in start..=bytes.len().saturating_sub(4) {
        if &bytes[i..i + 4] == needle {
            return Some(i + 4);
        }
    }
    None
}

fn validate_header_section(section: &[u8]) -> Result<(), CodecError> {
    if section.len() > MAX_HEADER_SECTION_BYTES {
        return Err(CodecError::LimitExceeded(format!(
            "header section exceeds {MAX_HEADER_SECTION_BYTES} bytes"
        )));
    }
    Ok(())
}

fn validate_header_buffer_len(len: usize) -> Result<(), CodecError> {
    if len > MAX_HEADER_SECTION_BYTES {
        return Err(CodecError::LimitExceeded(format!(
            "header section exceeds {MAX_HEADER_SECTION_BYTES} bytes"
        )));
    }
    Ok(())
}

fn case_insensitive_values<'a>(
    headers: &'a [(String, String)],
    name: &str,
) -> impl Iterator<Item = &'a str> {
    let lower = name.to_ascii_lowercase();
    headers
        .iter()
        .filter(move |(n, _)| n.to_ascii_lowercase() == lower)
        .map(|(_, v)| v.as_str())
}

fn is_field_name_byte(b: u8) -> bool {
    matches!(
        b,
        b'!' | b'#'
            | b'$'
            | b'%'
            | b'&'
            | b'\''
            | b'*'
            | b'+'
            | b'-'
            | b'.'
            | b'^'
            | b'_'
            | b'`'
            | b'|'
            | b'~'
            | b'0'..=b'9'
            | b'A'..=b'Z'
            | b'a'..=b'z'
    )
}

fn validate_header_name(name: &str) -> Result<(), CodecError> {
    if name.is_empty() {
        return Err(CodecError::BadHeader("empty header name".into()));
    }
    if !name.as_bytes().iter().all(|b| is_field_name_byte(*b)) {
        return Err(CodecError::BadHeader(format!(
            "invalid header name {}",
            name
        )));
    }
    Ok(())
}

fn validate_header_value(value: &str) -> Result<(), CodecError> {
    if value
        .as_bytes()
        .iter()
        .any(|b| matches!(*b, b'\r' | b'\n' | 0))
    {
        return Err(CodecError::BadHeader("invalid header value byte".into()));
    }
    Ok(())
}

fn validate_http_version(version: &str) -> Result<(), CodecError> {
    if version == "HTTP/1.1" {
        Ok(())
    } else {
        Err(CodecError::BadVersion(version.to_string()))
    }
}

fn parse_headers(section: &[u8]) -> Result<Vec<(String, String)>, CodecError> {
    validate_header_section(section)?;
    let s = str::from_utf8(section)
        .map_err(|_| CodecError::BadHeader("non-UTF-8 header bytes".into()))?;
    let mut out = Vec::new();
    for line in s.split("\r\n") {
        if line.is_empty() {
            continue;
        }
        if line.len() > MAX_HEADER_LINE_BYTES {
            return Err(CodecError::LimitExceeded(format!(
                "header line exceeds {MAX_HEADER_LINE_BYTES} bytes"
            )));
        }
        if out.len() >= MAX_HEADER_COUNT {
            return Err(CodecError::LimitExceeded(format!(
                "header count exceeds {MAX_HEADER_COUNT}"
            )));
        }
        let colon = line
            .find(':')
            .ok_or_else(|| CodecError::BadHeader(line.into()))?;
        let name_raw = &line[..colon];
        if name_raw != name_raw.trim() {
            return Err(CodecError::BadHeader(
                "whitespace before header colon".into(),
            ));
        }
        validate_header_name(name_raw)?;
        let value = line[colon + 1..].trim().to_string();
        validate_header_value(&value)?;
        let name = name_raw.to_string();
        out.push((name, value));
    }
    Ok(out)
}

fn checked_body_len(len: usize) -> Result<(), CodecError> {
    if len > MAX_DECODED_BODY_BYTES {
        return Err(CodecError::LimitExceeded(format!(
            "decoded body exceeds {MAX_DECODED_BODY_BYTES} bytes"
        )));
    }
    Ok(())
}

fn checked_add_body_len(current: usize, add: usize) -> Result<usize, CodecError> {
    let next = current
        .checked_add(add)
        .ok_or_else(|| CodecError::LimitExceeded("decoded body length overflow".into()))?;
    checked_body_len(next)?;
    Ok(next)
}

fn content_length(headers: &[(String, String)]) -> Result<Option<usize>, CodecError> {
    let mut seen: Option<&str> = None;
    for value in case_insensitive_values(headers, "content-length") {
        let normalized = value.trim();
        if let Some(prev) = seen {
            if prev != normalized {
                return Err(CodecError::BadHeader(
                    "conflicting content-length headers".into(),
                ));
            }
        } else {
            seen = Some(normalized);
        }
    }
    seen.map(|cl| {
        cl.parse()
            .map_err(|_| CodecError::BadHeader(format!("invalid content-length {}", cl)))
    })
    .transpose()
}

fn transfer_encoding(headers: &[(String, String)]) -> Result<Option<Vec<String>>, CodecError> {
    let values: Vec<&str> = case_insensitive_values(headers, "transfer-encoding").collect();
    if values.is_empty() {
        return Ok(None);
    }
    let mut tokens = Vec::new();
    for value in values {
        for token in value.split(',') {
            let token = token.trim();
            if token.is_empty() {
                return Err(CodecError::BadHeader(
                    "empty transfer-encoding token".into(),
                ));
            }
            tokens.push(token.to_ascii_lowercase());
        }
    }
    Ok(Some(tokens))
}

fn parse_chunk_size_line(line: &[u8]) -> Result<usize, CodecError> {
    let size_str = str::from_utf8(line)
        .map_err(|_| CodecError::BadChunkEncoding("non-UTF-8 chunk size".into()))?;
    let size_hex = size_str.split(';').next().unwrap();
    if size_hex.is_empty() || !size_hex.as_bytes().iter().all(|b| b.is_ascii_hexdigit()) {
        return Err(CodecError::BadChunkEncoding(format!(
            "bad chunk size {}",
            size_hex
        )));
    }
    usize::from_str_radix(size_hex, 16)
        .map_err(|_| CodecError::BadChunkEncoding(format!("bad chunk size {}", size_hex)))
}

fn decode_framing(headers: &[(String, String)]) -> Result<Option<BodyFraming>, CodecError> {
    let te = transfer_encoding(headers)?;
    let cl = content_length(headers)?;
    if te.is_some() && cl.is_some() {
        return Err(CodecError::BadHeader(
            "content-length with transfer-encoding".into(),
        ));
    }
    if let Some(tokens) = te {
        if tokens.last().map(String::as_str) != Some("chunked") {
            return Err(CodecError::BadHeader(
                "transfer-encoding chunked must be final".into(),
            ));
        }
        if tokens.iter().any(|t| t != "chunked") {
            return Err(CodecError::BadHeader(
                "unsupported transfer-encoding".into(),
            ));
        }
        return Ok(Some(BodyFraming::Chunked));
    }
    if let Some(n) = cl {
        checked_body_len(n)?;
        Ok(Some(BodyFraming::Length(n)))
    } else {
        Ok(None)
    }
}

pub fn parse_request(bytes: &[u8]) -> Result<ParsedRequest, CodecError> {
    let header_end = find_header_end(bytes).ok_or(CodecError::Truncated)?;
    let header_section = &bytes[..header_end - 4];
    validate_header_section(header_section)?;
    let body_section = &bytes[header_end..];

    let (start_line_bytes, headers_bytes): (&[u8], &[u8]) =
        match header_section.windows(2).position(|w| w == b"\r\n") {
            Some(crlf) => (&header_section[..crlf], &header_section[crlf + 2..]),
            None => (header_section, &[][..]),
        };
    let start_line = str::from_utf8(start_line_bytes)
        .map_err(|_| CodecError::BadStartLine("non-UTF-8 start line".into()))?;
    let parts: Vec<&str> = start_line.splitn(3, ' ').collect();
    if parts.len() != 3 {
        return Err(CodecError::BadStartLine(start_line.into()));
    }
    let method = parts[0].to_string();
    let target = parts[1].to_string();
    let version = parts[2].to_string();
    validate_http_version(&version)?;

    let headers = parse_headers(headers_bytes)?;
    let body = decode_body_exact(&headers, body_section)?;

    let is_chunked = headers.iter().any(|(k, v)| {
        k.eq_ignore_ascii_case("transfer-encoding") && v.to_ascii_lowercase().contains("chunked")
    });
    let trailers = if is_chunked {
        parse_chunked_trailers(body_section)
    } else {
        Vec::new()
    };
    Ok(ParsedRequest {
        method,
        target,
        version,
        headers,
        body,
        trailers,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseHead {
    pub version: String,
    pub status: u16,
    pub reason: String,
    pub headers: Vec<(String, String)>,
}

fn parse_head_section(header_section: &[u8]) -> Result<ResponseHead, CodecError> {
    validate_header_section(header_section)?;
    let (status_line_bytes, headers_bytes): (&[u8], &[u8]) =
        match header_section.windows(2).position(|w| w == b"\r\n") {
            Some(crlf) => (&header_section[..crlf], &header_section[crlf + 2..]),
            None => (header_section, &[][..]),
        };
    let status_line = str::from_utf8(status_line_bytes)
        .map_err(|_| CodecError::BadStartLine("non-UTF-8 status line".into()))?;
    let parts: Vec<&str> = status_line.splitn(3, ' ').collect();
    if parts.len() < 2 {
        return Err(CodecError::BadStartLine(status_line.into()));
    }
    let version = parts[0].to_string();
    validate_http_version(&version)?;
    let status: u16 = parts[1]
        .parse()
        .map_err(|_| CodecError::BadStatus(parts[1].into()))?;
    let reason = if parts.len() == 3 {
        parts[2].to_string()
    } else {
        String::new()
    };
    let headers = parse_headers(headers_bytes)?;
    Ok(ResponseHead {
        version,
        status,
        reason,
        headers,
    })
}

pub fn parse_response(bytes: &[u8]) -> Result<ParsedResponse, CodecError> {
    let header_end = find_header_end(bytes).ok_or(CodecError::Truncated)?;
    let head = parse_head_section(&bytes[..header_end - 4])?;
    let body = decode_body_exact(&head.headers, &bytes[header_end..])?;
    Ok(ParsedResponse {
        version: head.version,
        status: head.status,
        reason: head.reason,
        headers: head.headers,
        body,
    })
}

fn decode_body(headers: &[(String, String)], body_bytes: &[u8]) -> Result<Vec<u8>, CodecError> {
    match decode_framing(headers)? {
        Some(BodyFraming::Chunked) => return chunked_decode(body_bytes),
        Some(BodyFraming::Length(n)) => {
            checked_body_len(n)?;
            if body_bytes.len() < n {
                return Err(CodecError::ContentLengthMismatch);
            }
            return Ok(body_bytes[..n].to_vec());
        }
        Some(BodyFraming::Empty) => return Ok(Vec::new()),
        Some(BodyFraming::Eof) | None => {}
    }
    checked_body_len(body_bytes.len())?;
    Ok(body_bytes.to_vec())
}

fn decode_body_exact(
    headers: &[(String, String)],
    body_bytes: &[u8],
) -> Result<Vec<u8>, CodecError> {
    match decode_framing(headers)? {
        Some(BodyFraming::Length(n)) if body_bytes.len() != n => {
            Err(CodecError::ContentLengthMismatch)
        }
        _ => decode_body(headers, body_bytes),
    }
}

pub fn message_consumed_len(bytes: &[u8]) -> Result<usize, CodecError> {
    let header_end = find_header_end(bytes).ok_or(CodecError::Truncated)?;
    let header_section = &bytes[..header_end - 4];
    validate_header_section(header_section)?;
    let (_, headers_bytes): (&[u8], &[u8]) =
        match header_section.windows(2).position(|w| w == b"\r\n") {
            Some(crlf) => (&header_section[..crlf], &header_section[crlf + 2..]),
            None => (header_section, &[][..]),
        };
    let headers = parse_headers(headers_bytes)?;
    match decode_framing(&headers)? {
        Some(BodyFraming::Length(n)) => {
            if bytes[header_end..].len() < n {
                return Err(CodecError::ContentLengthMismatch);
            }
            Ok(header_end + n)
        }
        Some(BodyFraming::Chunked) => {
            chunked_consumed_len(&bytes[header_end..]).map(|n| header_end + n)
        }
        _ => Ok(header_end),
    }
}

pub fn try_serialize_request(
    method: &str,
    target: &str,
    headers: &[(String, String)],
    body: &[u8],
) -> Result<Vec<u8>, CodecError> {
    let mut out = Vec::new();
    out.extend_from_slice(method.as_bytes());
    out.push(b' ');
    out.extend_from_slice(target.as_bytes());
    out.extend_from_slice(b" HTTP/1.1\r\n");

    let auto_content_length = !(body.is_empty() && method_omits_bodyless_content_length(method));
    write_headers(&mut out, headers, body.len(), auto_content_length)?;
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(body);
    Ok(out)
}

fn method_omits_bodyless_content_length(method: &str) -> bool {
    matches!(
        method.to_ascii_uppercase().as_str(),
        "GET" | "HEAD" | "DELETE" | "OPTIONS" | "TRACE" | "CONNECT"
    )
}

pub fn serialize_request(
    method: &str,
    target: &str,
    headers: &[(String, String)],
    body: &[u8],
) -> Vec<u8> {
    try_serialize_request(method, target, headers, body).expect("invalid HTTP request header")
}

pub fn try_serialize_response(
    status: u16,
    reason: &str,
    headers: &[(String, String)],
    body: &[u8],
) -> Result<Vec<u8>, CodecError> {
    let mut out = Vec::new();
    out.extend_from_slice(b"HTTP/1.1 ");
    out.extend_from_slice(status.to_string().as_bytes());
    out.push(b' ');
    out.extend_from_slice(reason.as_bytes());
    out.extend_from_slice(b"\r\n");

    write_headers(&mut out, headers, body.len(), true)?;
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(body);
    Ok(out)
}

pub fn try_serialize_response_head(
    status: u16,
    reason: &str,
    headers: &[(String, String)],
) -> Result<Vec<u8>, CodecError> {
    let mut out = Vec::new();
    out.extend_from_slice(b"HTTP/1.1 ");
    out.extend_from_slice(status.to_string().as_bytes());
    out.push(b' ');
    out.extend_from_slice(reason.as_bytes());
    out.extend_from_slice(b"\r\n");
    write_headers(&mut out, headers, 0, false)?;
    out.extend_from_slice(b"\r\n");
    Ok(out)
}

pub fn serialize_response(
    status: u16,
    reason: &str,
    headers: &[(String, String)],
    body: &[u8],
) -> Vec<u8> {
    try_serialize_response(status, reason, headers, body).expect("invalid HTTP response header")
}

fn write_headers(
    out: &mut Vec<u8>,
    headers: &[(String, String)],
    body_len: usize,
    auto_content_length: bool,
) -> Result<(), CodecError> {
    let mut has_cl = false;
    let mut has_te = false;
    for (n, v) in headers {
        validate_header_name(n)?;
        validate_header_value(v)?;
        let lower = n.to_ascii_lowercase();
        if lower == "content-length" {
            has_cl = true;
        }
        if lower == "transfer-encoding" {
            has_te = true;
        }
        out.extend_from_slice(n.as_bytes());
        out.extend_from_slice(b": ");
        out.extend_from_slice(v.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    if has_cl && has_te {
        return Err(CodecError::BadHeader(
            "content-length with transfer-encoding".into(),
        ));
    }

    if auto_content_length && !has_cl && !has_te {
        out.extend_from_slice(b"Content-Length: ");
        out.extend_from_slice(body_len.to_string().as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    Ok(())
}

pub fn chunked_encode(chunks: &[&[u8]]) -> Vec<u8> {
    let mut out = Vec::new();
    for c in chunks {
        out.extend_from_slice(format!("{:X}\r\n", c.len()).as_bytes());
        out.extend_from_slice(c);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"0\r\n\r\n");
    out
}

pub fn parse_chunked_trailers(bytes: &[u8]) -> Vec<(String, String)> {
    let mut trailers = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let Some(rel) = bytes[i..].windows(2).position(|w| w == b"\r\n") else {
            return trailers;
        };
        let line_end = rel + i;
        let Ok(size) = parse_chunk_size_line(&bytes[i..line_end]) else {
            return trailers;
        };
        i = line_end + 2;
        if size == 0 {
            loop {
                let Some(rel2) = bytes[i..].windows(2).position(|w| w == b"\r\n") else {
                    break;
                };
                if rel2 == 0 {
                    break;
                }
                let line = &bytes[i..i + rel2];
                if let Some(colon) = line.iter().position(|&b| b == b':') {
                    let name = String::from_utf8_lossy(&line[..colon]).trim().to_string();
                    let val = String::from_utf8_lossy(&line[colon + 1..])
                        .trim()
                        .to_string();
                    if !name.is_empty() {
                        trailers.push((name, val));
                    }
                }
                i += rel2 + 2;
            }
            return trailers;
        }
        i += size;
        if i + 2 > bytes.len() {
            return trailers;
        }
        i += 2;
    }
    trailers
}

pub fn chunked_decode(bytes: &[u8]) -> Result<Vec<u8>, CodecError> {
    let mut out = Vec::new();
    let mut i = 0;
    let mut chunks = 0usize;
    while i < bytes.len() {

        let line_end = bytes[i..]
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or(CodecError::Truncated)?
            + i;
        let size = parse_chunk_size_line(&bytes[i..line_end])?;
        i = line_end + 2;
        if size == 0 {

            if !bytes[i..].starts_with(b"\r\n") {

                bytes[i..]
                    .windows(2)
                    .position(|w| w == b"\r\n")
                    .ok_or_else(|| CodecError::BadChunkEncoding("missing terminator".into()))?;
            }
            return Ok(out);
        }
        if chunks >= MAX_CHUNK_COUNT {
            return Err(CodecError::LimitExceeded(format!(
                "chunk count exceeds {MAX_CHUNK_COUNT}"
            )));
        }
        chunks += 1;
        checked_add_body_len(out.len(), size)?;
        if i + size > bytes.len() {
            return Err(CodecError::BadChunkEncoding(
                "chunk size exceeds remaining bytes".into(),
            ));
        }
        out.extend_from_slice(&bytes[i..i + size]);
        i += size;

        if i + 2 > bytes.len() {
            return Err(CodecError::BadChunkEncoding(
                "incomplete: chunk CRLF not yet received".into(),
            ));
        }
        if &bytes[i..i + 2] != b"\r\n" {
            return Err(CodecError::BadChunkEncoding(
                "chunk not followed by CRLF".into(),
            ));
        }
        i += 2;
    }
    Err(CodecError::BadChunkEncoding(
        "no zero-chunk terminator".into(),
    ))
}

fn chunked_consumed_len(bytes: &[u8]) -> Result<usize, CodecError> {
    let mut i = 0;
    let mut chunks = 0usize;
    let mut decoded = 0usize;
    while i < bytes.len() {
        let line_end = bytes[i..]
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or_else(|| CodecError::BadChunkEncoding("missing chunk-size CRLF".into()))?
            + i;
        let size = parse_chunk_size_line(&bytes[i..line_end])?;
        i = line_end + 2;
        if size == 0 {
            if bytes[i..].starts_with(b"\r\n") {
                return Ok(i + 2);
            }
            let term = bytes[i..]
                .windows(4)
                .position(|w| w == b"\r\n\r\n")
                .ok_or_else(|| CodecError::BadChunkEncoding("missing terminator".into()))?;
            return Ok(i + term + 4);
        }
        if chunks >= MAX_CHUNK_COUNT {
            return Err(CodecError::LimitExceeded(format!(
                "chunk count exceeds {MAX_CHUNK_COUNT}"
            )));
        }
        chunks += 1;
        decoded = checked_add_body_len(decoded, size)?;
        if i + size + 2 > bytes.len() {
            return Err(CodecError::Truncated);
        }
        i += size;
        if &bytes[i..i + 2] != b"\r\n" {
            return Err(CodecError::BadChunkEncoding(
                "chunk not followed by CRLF".into(),
            ));
        }
        i += 2;
    }
    Err(CodecError::Truncated)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyFraming {

    Chunked,

    Length(usize),

    Eof,

    Empty,
}

pub fn body_framing(head: &ResponseHead) -> Result<BodyFraming, CodecError> {

    if head.status < 200 || head.status == 204 || head.status == 304 {
        return Ok(BodyFraming::Empty);
    }
    decode_framing(&head.headers).map(|framing| framing.unwrap_or(BodyFraming::Eof))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChunkState {

    Size,

    Data(usize),

    DataCrlf,

    Trailer,
}

pub struct ResponseDecoder {
    buf: Vec<u8>,
    header_scan_start: usize,
    head: Option<ResponseHead>,
    framing: BodyFraming,

    emitted: usize,
    chunks_seen: usize,
    chunk: ChunkState,
    complete: bool,

    closed: bool,

    trailers: Vec<(String, String)>,
}

impl Default for ResponseDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl ResponseDecoder {
    pub fn new() -> Self {
        ResponseDecoder {
            buf: Vec::new(),
            header_scan_start: 0,
            head: None,
            framing: BodyFraming::Eof,
            emitted: 0,
            chunks_seen: 0,
            chunk: ChunkState::Size,
            complete: false,
            closed: false,
            trailers: Vec::new(),
        }
    }

    pub fn trailers(&self) -> &[(String, String)] {
        &self.trailers
    }

    pub fn push(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    pub fn close(&mut self) {
        self.closed = true;
        if self.head.is_some() && matches!(self.framing, BodyFraming::Eof) && self.buf.is_empty() {
            self.complete = true;
        }
    }

    pub fn head(&mut self) -> Result<Option<&ResponseHead>, CodecError> {
        if self.head.is_none() {
            let Some(end) = find_header_end_from(&self.buf, self.header_scan_start) else {
                validate_header_buffer_len(self.buf.len().saturating_sub(3))?;
                if self.closed && !self.buf.is_empty() {
                    return Err(CodecError::Truncated);
                }
                self.header_scan_start = self.buf.len().saturating_sub(3);
                return Ok(None);
            };
            let head = parse_head_section(&self.buf[..end - 4])?;
            self.framing = body_framing(&head)?;
            self.buf.drain(..end);
            if matches!(self.framing, BodyFraming::Empty | BodyFraming::Length(0)) {
                self.complete = true;
            }
            self.head = Some(head);
        }
        Ok(self.head.as_ref())
    }

    pub fn framing(&mut self) -> Result<Option<BodyFraming>, CodecError> {
        if self.head()?.is_none() {
            return Ok(None);
        }
        Ok(Some(self.framing))
    }

    pub fn is_complete(&self) -> bool {
        self.complete
    }

    pub fn read_body(&mut self) -> Result<Vec<u8>, CodecError> {
        if self.head()?.is_none() || self.complete {
            return Ok(Vec::new());
        }
        match self.framing {
            BodyFraming::Empty => Ok(Vec::new()),
            BodyFraming::Length(n) => {
                checked_body_len(n)?;
                let want = n - self.emitted;
                let take = want.min(self.buf.len());
                let out: Vec<u8> = self.buf.drain(..take).collect();
                self.emitted += take;
                if self.emitted == n {
                    self.complete = true;
                } else if self.closed {
                    return Err(CodecError::ContentLengthMismatch);
                }
                Ok(out)
            }
            BodyFraming::Eof => {
                checked_add_body_len(self.emitted, self.buf.len())?;
                let out: Vec<u8> = self.buf.drain(..).collect();
                self.emitted += out.len();

                if self.closed {
                    self.complete = true;
                }
                Ok(out)
            }
            BodyFraming::Chunked => self.read_chunked(),
        }
    }

    fn read_chunked(&mut self) -> Result<Vec<u8>, CodecError> {
        let mut out = Vec::new();
        loop {
            match self.chunk {
                ChunkState::Size => {
                    let Some(line_end) = self.buf.windows(2).position(|w| w == b"\r\n") else {
                        if self.closed {
                            return Err(CodecError::BadChunkEncoding(
                                "connection closed mid chunk-size".into(),
                            ));
                        }
                        return Ok(out);
                    };
                    let size = parse_chunk_size_line(&self.buf[..line_end])?;
                    self.buf.drain(..line_end + 2);
                    self.chunk = if size == 0 {
                        ChunkState::Trailer
                    } else {
                        if self.chunks_seen >= MAX_CHUNK_COUNT {
                            return Err(CodecError::LimitExceeded(format!(
                                "chunk count exceeds {MAX_CHUNK_COUNT}"
                            )));
                        }
                        self.chunks_seen += 1;
                        checked_add_body_len(self.emitted, size)?;
                        ChunkState::Data(size)
                    };
                }
                ChunkState::Data(remaining) => {
                    if self.buf.is_empty() {
                        if self.closed {
                            return Err(CodecError::BadChunkEncoding(
                                "connection closed mid chunk".into(),
                            ));
                        }
                        return Ok(out);
                    }
                    let take = remaining.min(self.buf.len());
                    checked_add_body_len(self.emitted, take)?;
                    out.extend(self.buf.drain(..take));
                    self.emitted += take;
                    self.chunk = if take == remaining {
                        ChunkState::DataCrlf
                    } else {
                        ChunkState::Data(remaining - take)
                    };
                }
                ChunkState::DataCrlf => {
                    if self.buf.len() < 2 {
                        return Ok(out);
                    }
                    if &self.buf[..2] != b"\r\n" {
                        return Err(CodecError::BadChunkEncoding("missing chunk CRLF".into()));
                    }
                    self.buf.drain(..2);
                    self.chunk = ChunkState::Size;
                }
                ChunkState::Trailer => {

                    loop {
                        let Some(rel) = self.buf.windows(2).position(|w| w == b"\r\n") else {
                            return Ok(out);
                        };
                        if rel == 0 {
                            self.buf.drain(..2);
                            self.complete = true;
                            return Ok(out);
                        }
                        let line: Vec<u8> = self.buf.drain(..rel + 2).collect();
                        if let Some(colon) = line.iter().position(|&b| b == b':') {
                            let name = String::from_utf8_lossy(&line[..colon]).trim().to_string();
                            let val = String::from_utf8_lossy(&line[colon + 1..rel])
                                .trim()
                                .to_string();
                            if !name.is_empty() {
                                self.trailers.push((name, val));
                            }
                        }
                    }
                }
            }
        }
    }
}
