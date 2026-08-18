
use crate::{FormData, FormDataEntryValue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MultipartError {

    MissingName,

    Malformed(String),
}

const CRLF: &[u8] = b"\r\n";

fn escape_field(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\n' => out.push_str("%0A"),
            '\r' => out.push_str("%0D"),
            '"' => out.push_str("%22"),
            _ => out.push(c),
        }
    }
    out
}

fn unescape_field(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = &s[i + 1..i + 3];
            match hex.to_ascii_uppercase().as_str() {
                "0A" => {
                    out.push('\n');
                    i += 3;
                    continue;
                }
                "0D" => {
                    out.push('\r');
                    i += 3;
                    continue;
                }
                "22" => {
                    out.push('"');
                    i += 3;
                    continue;
                }
                _ => {}
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

impl FormData {

    pub fn to_multipart(&self, boundary: &str) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        for (name, value) in self.entries() {
            out.extend_from_slice(b"--");
            out.extend_from_slice(boundary.as_bytes());
            out.extend_from_slice(CRLF);
            match value {
                FormDataEntryValue::String(s) => {
                    out.extend_from_slice(
                        format!(
                            "Content-Disposition: form-data; name=\"{}\"",
                            escape_field(name)
                        )
                        .as_bytes(),
                    );
                    out.extend_from_slice(CRLF);
                    out.extend_from_slice(CRLF);
                    out.extend_from_slice(s.as_bytes());
                }
                FormDataEntryValue::File {
                    name: filename,
                    content_type,
                    bytes,
                } => {
                    out.extend_from_slice(
                        format!(
                            "Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"",
                            escape_field(name),
                            escape_field(filename)
                        )
                        .as_bytes(),
                    );
                    out.extend_from_slice(CRLF);
                    let ct = if content_type.is_empty() {
                        "application/octet-stream"
                    } else {
                        content_type.as_str()
                    };
                    out.extend_from_slice(format!("Content-Type: {}", ct).as_bytes());
                    out.extend_from_slice(CRLF);
                    out.extend_from_slice(CRLF);
                    out.extend_from_slice(bytes);
                }
            }
            out.extend_from_slice(CRLF);
        }
        out.extend_from_slice(b"--");
        out.extend_from_slice(boundary.as_bytes());
        out.extend_from_slice(b"--");
        out.extend_from_slice(CRLF);
        out
    }

    pub fn from_multipart(bytes: &[u8], boundary: &str) -> Result<FormData, MultipartError> {
        let delim = format!("--{}", boundary).into_bytes();
        let segments = split_on(bytes, &delim);

        let mut fd = FormData::new();
        let mut saw_part = false;
        for seg in segments.iter() {

            if seg.starts_with(b"--") {
                break;
            }

            if !seg.starts_with(CRLF) {
                continue;
            }
            saw_part = true;

            let body = &seg[CRLF.len()..];
            let body = strip_trailing_crlf(body);
            parse_part(body, &mut fd)?;
        }
        if !saw_part {
            return Err(MultipartError::Malformed("no parts found".into()));
        }
        Ok(fd)
    }
}

fn parse_part(part: &[u8], fd: &mut FormData) -> Result<(), MultipartError> {

    let sep = b"\r\n\r\n";
    let split = find(part, sep)
        .ok_or_else(|| MultipartError::Malformed("part missing header/body separator".into()))?;
    let header_block = &part[..split];
    let body = &part[split + sep.len()..];

    let mut name: Option<String> = None;
    let mut filename: Option<String> = None;
    let mut content_type: Option<String> = None;
    for line in split_on(header_block, CRLF) {
        let line = String::from_utf8_lossy(&line);
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("content-disposition:") {
            name = extract_quoted_param(&line, "name").map(|v| unescape_field(&v));
            filename = extract_quoted_param(&line, "filename").map(|v| unescape_field(&v));
        } else if lower.starts_with("content-type:") {
            content_type = Some(line[line.find(':').unwrap() + 1..].trim().to_string());
        }
    }
    let name = name.ok_or(MultipartError::MissingName)?;
    match filename {
        Some(fname) => fd.append_file(
            &name,
            body.to_vec(),
            content_type
                .as_deref()
                .unwrap_or("application/octet-stream"),
            Some(&fname),
        ),
        None => fd.append(&name, &String::from_utf8_lossy(body)),
    }
    Ok(())
}

fn extract_quoted_param(line: &str, key: &str) -> Option<String> {
    let needle = format!("{}=\"", key);
    let start = line.find(&needle)? + needle.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn split_on(hay: &[u8], delim: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut i = 0;
    if delim.is_empty() {
        return vec![hay.to_vec()];
    }
    while i + delim.len() <= hay.len() {
        if &hay[i..i + delim.len()] == delim {
            out.push(hay[start..i].to_vec());
            i += delim.len();
            start = i;
        } else {
            i += 1;
        }
    }
    out.push(hay[start..].to_vec());
    out
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }
    (0..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
}

fn strip_trailing_crlf(b: &[u8]) -> &[u8] {
    if b.ends_with(CRLF) {
        &b[..b.len() - CRLF.len()]
    } else {
        b
    }
}
