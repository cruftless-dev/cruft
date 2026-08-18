
use super::PgError;
use sql_core::SqlValue;

fn err(input: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: super::type_name(super::oid::BYTEA),
        input: input.to_string(),
    }
}

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    let _ = oid;
    let bytes = text.as_bytes();

    if bytes.starts_with(b"\\x") {
        decode_hex(&bytes[2..])
            .map(SqlValue::Blob)
            .ok_or_else(|| err(text))
    } else {
        decode_escape(bytes)
            .map(SqlValue::Blob)
            .ok_or_else(|| err(text))
    }
}

fn decode_hex(body: &[u8]) -> Option<Vec<u8>> {
    let mut nibbles: Vec<u8> = Vec::with_capacity(body.len());
    for &b in body {
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

fn decode_escape(body: &[u8]) -> Option<Vec<u8>> {
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

pub fn output(oid: u32, v: &SqlValue) -> String {
    let _ = oid;
    match v {
        SqlValue::Blob(bytes) => {
            let mut s = String::with_capacity(2 + bytes.len() * 2);
            s.push_str("\\x");
            for b in bytes {
                s.push(char::from_digit((b >> 4) as u32, 16).unwrap());
                s.push(char::from_digit((b & 0x0f) as u32, 16).unwrap());
            }
            s
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::oid;

    fn parse(s: &str) -> Vec<u8> {
        match input(oid::BYTEA, s).expect("expected a valid bytea literal") {
            SqlValue::Blob(b) => b,
            other => panic!("expected Blob, got {other:?}"),
        }
    }

    #[test]
    fn hex_input_mixed_case() {

        assert_eq!(parse("\\xDeAdBeEf"), vec![0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn hex_input_uppercase() {
        assert_eq!(parse("\\xDEADBEEF"), vec![0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn hex_input_whitespace_skipped() {

        assert_eq!(parse("\\x De Ad Be Ef "), vec![0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn hex_input_with_embedded_null() {

        assert_eq!(parse("\\xDe00BeEf"), vec![0xde, 0x00, 0xbe, 0xef]);
    }

    #[test]
    fn hex_input_empty_body() {

        assert_eq!(parse("\\x"), Vec::<u8>::new());
    }

    #[test]
    fn hex_reject_odd_length() {

        let bad = "\\xDeAdBeE";
        let e = input(oid::BYTEA, bad).unwrap_err();
        assert_eq!(
            e,
            PgError::InvalidInputSyntax {
                typ: "bytea",
                input: bad.to_string()
            }
        );
    }

    #[test]
    fn hex_reject_non_hex_digit() {

        let bad = "\\xDeAdBeEx";
        assert!(matches!(
            input(oid::BYTEA, bad),
            Err(PgError::InvalidInputSyntax { typ: "bytea", .. })
        ));
    }

    #[test]
    fn escape_input_all_literal_ascii() {

        assert_eq!(parse("DeAdBeEf"), b"DeAdBeEf".to_vec());
    }

    #[test]
    fn escape_input_octal_null() {

        assert_eq!(
            parse("De\\000dBeEf"),
            vec![b'D', b'e', 0x00, b'd', b'B', b'e', b'E', b'f']
        );
    }

    #[test]
    fn escape_input_octal_byte() {

        assert_eq!(parse("De\\123dBeEf"), b"DeSdBeEf".to_vec());
    }

    #[test]
    fn escape_input_literal_backslash() {

        assert_eq!(parse("a\\\\b"), vec![b'a', b'\\', b'b']);
    }

    #[test]
    fn escape_reject_bad_octal_run() {

        assert!(matches!(
            input(oid::BYTEA, "De\\678dBeEf"),
            Err(PgError::InvalidInputSyntax { typ: "bytea", .. })
        ));

        assert!(matches!(
            input(oid::BYTEA, "foo\\99bar"),
            Err(PgError::InvalidInputSyntax { typ: "bytea", .. })
        ));
    }

    #[test]
    fn escape_reject_trailing_backslash() {
        assert!(matches!(
            input(oid::BYTEA, "abc\\"),
            Err(PgError::InvalidInputSyntax { typ: "bytea", .. })
        ));
    }

    #[test]
    fn output_hex_lowercase_with_prefix() {

        let v = SqlValue::Blob(vec![0xde, 0xad, 0xbe, 0xef]);
        assert_eq!(output(oid::BYTEA, &v), "\\xdeadbeef");
    }

    #[test]
    fn output_empty_blob_is_bare_prefix() {
        assert_eq!(output(oid::BYTEA, &SqlValue::Blob(Vec::new())), "\\x");
    }

    #[test]
    fn output_round_trips_hex_input() {

        let v = input(oid::BYTEA, "\\xDeAdBeEf").unwrap();
        assert_eq!(output(oid::BYTEA, &v), "\\xdeadbeef");
    }

    #[test]
    fn output_round_trips_escape_input() {

        let v = input(oid::BYTEA, "DeAdBeEf").unwrap();
        assert_eq!(output(oid::BYTEA, &v), "\\x4465416442654566");
    }

    #[test]
    fn output_non_blob_is_empty() {
        assert_eq!(output(oid::BYTEA, &SqlValue::Null), "");
        assert_eq!(output(oid::BYTEA, &SqlValue::Int(42)), "");
    }
}
