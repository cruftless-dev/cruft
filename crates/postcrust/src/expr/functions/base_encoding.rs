
use crate::types::PgError;
use sql_core::SqlValue;

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    match name {
        "encode" => Some(encode_call(args)),
        "decode" => Some(decode_call(args)),
        _ => None,
    }
}

fn wrong(name: &str) -> Result<SqlValue, PgError> {
    Err(PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("function {name}(...) does not exist"),
    })
}

fn encode_call(args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() != 2 {
        return wrong("encode");
    }

    if matches!(args[0], SqlValue::Null) || matches!(args[1], SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let bytes = match &args[0] {
        SqlValue::Blob(b) => b.as_slice(),
        _ => return wrong("encode"),
    };
    let format = match &args[1] {
        SqlValue::Text(s) => s.as_str(),
        _ => return wrong("encode"),
    };
    match format {
        "hex" => Ok(SqlValue::Text(encode_hex(bytes))),
        "base64" => Ok(SqlValue::Text(encode_base64(bytes))),
        "escape" => Ok(SqlValue::Text(encode_escape(bytes))),
        _ => wrong("encode"),
    }
}

fn decode_call(args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() != 2 {
        return wrong("decode");
    }
    if matches!(args[0], SqlValue::Null) || matches!(args[1], SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let text = match &args[0] {
        SqlValue::Text(s) => s.as_str(),
        _ => return wrong("decode"),
    };
    let format = match &args[1] {
        SqlValue::Text(s) => s.as_str(),
        _ => return wrong("decode"),
    };
    let decoded = match format {
        "hex" => decode_hex(text),
        "base64" => decode_base64(text),
        "escape" => decode_escape(text),
        _ => return wrong("decode"),
    };
    decoded
        .map(SqlValue::Blob)
        .ok_or_else(|| PgError::InvalidInputSyntax {
            typ: "expression",
            input: "function decode(...) does not exist".to_string(),
        })
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        s.push(char::from_digit((b & 0x0f) as u32, 16).unwrap());
    }
    s
}

fn decode_hex(text: &str) -> Option<Vec<u8>> {
    let mut nibbles: Vec<u8> = Vec::with_capacity(text.len());
    for &b in text.as_bytes() {
        if b.is_ascii_whitespace() {
            continue;
        }
        let n = match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            b'A'..=b'F' => b - b'A' + 10,
            _ => return None,
        };
        nibbles.push(n);
    }
    if nibbles.len() % 2 != 0 {
        return None;
    }
    Some(
        nibbles
            .chunks_exact(2)
            .map(|p| (p[0] << 4) | p[1])
            .collect(),
    )
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn encode_base64(bytes: &[u8]) -> String {

    let mut raw = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        raw.push(B64[(n >> 18 & 0x3f) as usize] as char);
        raw.push(B64[(n >> 12 & 0x3f) as usize] as char);
        raw.push(if chunk.len() >= 2 {
            B64[(n >> 6 & 0x3f) as usize] as char
        } else {
            '='
        });
        raw.push(if chunk.len() >= 3 {
            B64[(n & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    let mut out = String::with_capacity(raw.len() + raw.len() / 76 + 1);
    for (i, c) in raw.chars().enumerate() {
        if i > 0 && i % 76 == 0 {
            out.push('\n');
        }
        out.push(c);
    }
    out
}

fn decode_base64(text: &str) -> Option<Vec<u8>> {
    fn val(b: u8) -> Option<u32> {
        match b {
            b'A'..=b'Z' => Some((b - b'A') as u32),
            b'a'..=b'z' => Some((b - b'a' + 26) as u32),
            b'0'..=b'9' => Some((b - b'0' + 52) as u32),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let mut acc = 0u32;
    let mut bits = 0u32;
    let mut out: Vec<u8> = Vec::new();
    for &b in text.as_bytes() {
        if b.is_ascii_whitespace() || b == b'=' {
            continue;
        }
        let v = val(b)?;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

fn encode_escape(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len());
    for &b in bytes {
        if b == 0 || b >= 0x80 {
            s.push('\\');
            s.push(char::from_digit((b >> 6) as u32, 8).unwrap());
            s.push(char::from_digit((b >> 3 & 0x7) as u32, 8).unwrap());
            s.push(char::from_digit((b & 0x7) as u32, 8).unwrap());
        } else if b == b'\\' {
            s.push_str("\\\\");
        } else {
            s.push(b as char);
        }
    }
    s
}

fn decode_escape(text: &str) -> Option<Vec<u8>> {
    let body = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(body.len());
    let mut i = 0;
    while i < body.len() {
        if body[i] != b'\\' {
            out.push(body[i]);
            i += 1;
            continue;
        }
        match body.get(i + 1) {
            Some(b'\\') => {
                out.push(b'\\');
                i += 2;
            }
            Some(&d0 @ b'0'..=b'3') => {
                let d1 = *body.get(i + 2)?;
                let d2 = *body.get(i + 3)?;
                if !(b'0'..=b'7').contains(&d1) || !(b'0'..=b'7').contains(&d2) {
                    return None;
                }
                let val = ((d0 - b'0') << 6) | ((d1 - b'0') << 3) | (d2 - b'0');
                out.push(val);
                i += 4;
            }
            _ => return None,
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> SqlValue {
        SqlValue::Text(s.to_string())
    }
    fn blob(b: &[u8]) -> SqlValue {
        SqlValue::Blob(b.to_vec())
    }
    fn as_text(v: SqlValue) -> String {
        match v {
            SqlValue::Text(s) => s,
            other => panic!("expected Text, got {other:?}"),
        }
    }
    fn as_blob(v: SqlValue) -> Vec<u8> {
        match v {
            SqlValue::Blob(b) => b,
            other => panic!("expected Blob, got {other:?}"),
        }
    }

    const SAMPLE: [u8; 9] = [0x12, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd, 0xef, 0x00];

    #[test]
    fn hex_encode_golden() {

        let r = call("encode", &[blob(&SAMPLE), text("hex")])
            .unwrap()
            .unwrap();
        assert_eq!(as_text(r), "1234567890abcdef00");
    }

    #[test]
    fn hex_decode_golden() {

        let r = call("decode", &[text("1234567890abcdef00"), text("hex")])
            .unwrap()
            .unwrap();
        assert_eq!(as_blob(r), SAMPLE.to_vec());
    }

    #[test]
    fn hex_round_trip_case_insensitive() {
        let enc = as_text(
            call("encode", &[blob(&SAMPLE), text("hex")])
                .unwrap()
                .unwrap(),
        );
        let back = as_blob(
            call("decode", &[text(&enc.to_uppercase()), text("hex")])
                .unwrap()
                .unwrap(),
        );
        assert_eq!(back, SAMPLE.to_vec());
    }

    #[test]
    fn hex_decode_bad() {

        assert!(call("decode", &[text("12zz"), text("hex")])
            .unwrap()
            .is_err());

        assert!(call("decode", &[text("abc"), text("hex")])
            .unwrap()
            .is_err());
    }

    #[test]
    fn base64_encode_wraps_at_76() {

        let unit = [0x12u8, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd, 0xef, 0x00, 0x01];
        let mut bytes = Vec::new();
        for _ in 0..7 {
            bytes.extend_from_slice(&unit);
        }
        assert_eq!(bytes.len(), 70);
        let enc = as_text(
            call("encode", &[blob(&bytes), text("base64")])
                .unwrap()
                .unwrap(),
        );
        let lines: Vec<&str> = enc.split('\n').collect();
        assert_eq!(lines.len(), 2, "expected a single 76-char wrap");
        assert_eq!(lines[0].len(), 76);
        assert_eq!(
            enc,
            "EjRWeJCrze8AARI0VniQq83vAAESNFZ4kKvN7wABEjRWeJCrze8AARI0VniQq83vAAESNFZ4kKvN\n\
             7wABEjRWeJCrze8AAQ=="
        );
    }

    #[test]
    fn base64_round_trip_ignores_newlines_and_padding() {

        let unit = [0x12u8, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd, 0xef, 0x00, 0x01];
        let mut bytes = Vec::new();
        for _ in 0..7 {
            bytes.extend_from_slice(&unit);
        }
        let enc = as_text(
            call("encode", &[blob(&bytes), text("base64")])
                .unwrap()
                .unwrap(),
        );
        let back = as_blob(
            call("decode", &[text(&enc), text("base64")])
                .unwrap()
                .unwrap(),
        );
        assert_eq!(back, bytes);
    }

    #[test]
    fn base64_padding_shapes() {

        assert_eq!(
            as_text(
                call("encode", &[blob(&[0x00]), text("base64")])
                    .unwrap()
                    .unwrap()
            ),
            "AA=="
        );
        assert_eq!(
            as_text(
                call("encode", &[blob(&[0x00, 0x00]), text("base64")])
                    .unwrap()
                    .unwrap()
            ),
            "AAA="
        );
        assert_eq!(
            as_text(
                call("encode", &[blob(b"Man"), text("base64")])
                    .unwrap()
                    .unwrap()
            ),
            "TWFu"
        );
    }

    #[test]
    fn escape_encode_golden_bytes() {

        let r = as_text(
            call("encode", &[blob(&SAMPLE), text("escape")])
                .unwrap()
                .unwrap(),
        );
        let expected = format!("{}4Vx\\220\\253\\315\\357\\000", '\u{12}');
        assert_eq!(r, expected);

        assert!(r.contains("\\220"));
    }

    #[test]
    fn escape_encode_backslash_doubled() {
        assert_eq!(
            as_text(
                call("encode", &[blob(b"a\\b"), text("escape")])
                    .unwrap()
                    .unwrap()
            ),
            "a\\\\b"
        );
    }

    #[test]
    fn escape_round_trip() {

        let enc = as_text(
            call("encode", &[blob(&SAMPLE), text("escape")])
                .unwrap()
                .unwrap(),
        );
        let back = as_blob(
            call("decode", &[SqlValue::Text(enc), text("escape")])
                .unwrap()
                .unwrap(),
        );
        assert_eq!(back, SAMPLE.to_vec());
    }

    #[test]
    fn escape_decode_bad_backslash() {

        assert!(call("decode", &[text("abc\\"), text("escape")])
            .unwrap()
            .is_err());
    }

    #[test]
    fn unknown_format_errors() {
        assert!(call("encode", &[blob(&SAMPLE), text("uuencode")])
            .unwrap()
            .is_err());
        assert!(call("decode", &[text("00"), text("uuencode")])
            .unwrap()
            .is_err());
    }

    #[test]
    fn null_propagates() {
        assert_eq!(
            call("encode", &[SqlValue::Null, text("hex")])
                .unwrap()
                .unwrap(),
            SqlValue::Null
        );
        assert_eq!(
            call("encode", &[blob(&SAMPLE), SqlValue::Null])
                .unwrap()
                .unwrap(),
            SqlValue::Null
        );
        assert_eq!(
            call("decode", &[SqlValue::Null, text("hex")])
                .unwrap()
                .unwrap(),
            SqlValue::Null
        );
    }

    #[test]
    fn wrong_arity_and_type() {
        assert!(call("encode", &[blob(&SAMPLE)]).unwrap().is_err());

        assert!(call("encode", &[text("nope"), text("hex")])
            .unwrap()
            .is_err());

        assert!(call("decode", &[blob(&SAMPLE), text("hex")])
            .unwrap()
            .is_err());
    }

    #[test]
    fn unclaimed_name_is_none() {
        assert!(call("md5", &[text("x")]).is_none());
        assert!(call("to_hex", &[SqlValue::Int(1)]).is_none());
    }
}
