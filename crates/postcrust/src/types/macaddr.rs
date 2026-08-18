
use super::PgError;
use sql_core::SqlValue;

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    let typ = super::type_name(oid);
    let trimmed = text.trim();
    let bytes = match oid {
        super::oid::MACADDR => parse_macaddr(trimmed),
        super::oid::MACADDR8 => parse_macaddr8(trimmed),
        _ => None,
    };
    match bytes {
        Some(b) => Ok(SqlValue::Text(canon(&b))),
        None => Err(PgError::InvalidInputSyntax {
            typ,
            input: text.to_string(),
        }),
    }
}

pub fn output(oid: u32, v: &SqlValue) -> String {
    let _ = oid;
    match v {
        SqlValue::Text(s) => s.clone(),
        _ => String::new(),
    }
}

fn hexval(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

fn canon(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 {
            s.push(':');
        }
        s.push_str(&format!("{:02x}", b));
    }
    s
}

fn group_bytes(group: &str, nbytes: usize) -> Option<Vec<u8>> {
    let b = group.as_bytes();
    if b.len() != nbytes * 2 {
        return None;
    }
    let mut out = Vec::with_capacity(nbytes);
    for i in 0..nbytes {
        let hi = hexval(b[2 * i])?;
        let lo = hexval(b[2 * i + 1])?;
        out.push((hi << 4) | lo);
    }
    Some(out)
}

fn small_byte(group: &str) -> Option<u8> {
    let b = group.as_bytes();
    if b.is_empty() || b.len() > 2 {
        return None;
    }
    let mut v: u16 = 0;
    for &c in b {
        v = v * 16 + hexval(c)? as u16;
    }
    if v > 0xff {
        None
    } else {
        Some(v as u8)
    }
}

fn parse_macaddr(s: &str) -> Option<Vec<u8>> {
    if s.is_empty() {
        return None;
    }
    if s.contains('.') {

        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        let mut out = Vec::with_capacity(6);
        for p in parts {
            out.extend(group_bytes(p, 2)?);
        }
        return Some(out);
    }
    if s.contains(':') {
        let parts: Vec<&str> = s.split(':').collect();
        return match parts.len() {
            6 => {
                let mut out = Vec::with_capacity(6);
                for p in parts {
                    out.push(small_byte(p)?);
                }
                Some(out)
            }
            2 => {
                let mut out = Vec::with_capacity(6);
                for p in parts {
                    out.extend(group_bytes(p, 3)?);
                }
                Some(out)
            }
            _ => None,
        };
    }
    if s.contains('-') {
        let parts: Vec<&str> = s.split('-').collect();
        return match parts.len() {
            6 => {
                let mut out = Vec::with_capacity(6);
                for p in parts {
                    out.push(small_byte(p)?);
                }
                Some(out)
            }
            2 => {
                let mut out = Vec::with_capacity(6);
                for p in parts {
                    out.extend(group_bytes(p, 3)?);
                }
                Some(out)
            }
            3 => {
                let mut out = Vec::with_capacity(6);
                for p in parts {
                    out.extend(group_bytes(p, 2)?);
                }
                Some(out)
            }
            _ => None,
        };
    }

    group_bytes(s, 6)
}

fn parse_macaddr8(s: &str) -> Option<Vec<u8>> {
    if s.is_empty() {
        return None;
    }
    let mut sep: Option<u8> = None;
    let mut nibbles: Vec<u8> = Vec::with_capacity(16);

    let mut prev_sep = true;
    for &c in s.as_bytes() {
        if let Some(h) = hexval(c) {
            nibbles.push(h);
            prev_sep = false;
        } else if c == b':' || c == b'-' || c == b'.' {
            if prev_sep {
                return None;
            }
            match sep {
                Some(x) if x != c => return None,
                _ => sep = Some(c),
            }
            prev_sep = true;
        } else {
            return None;
        }
    }
    if prev_sep {
        return None;
    }
    let n = nibbles.len();
    if n != 12 && n != 16 {
        return None;
    }
    let mut out: Vec<u8> = Vec::with_capacity(n / 2);
    let mut i = 0;
    while i < n {
        out.push((nibbles[i] << 4) | nibbles[i + 1]);
        i += 2;
    }
    if out.len() == 6 {

        let mut ex = Vec::with_capacity(8);
        ex.extend_from_slice(&out[0..3]);
        ex.push(0xff);
        ex.push(0xfe);
        ex.extend_from_slice(&out[3..6]);
        out = ex;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::oid;

    fn ok(oid: u32, s: &str) -> String {
        match input(oid, s) {
            Ok(SqlValue::Text(t)) => t,
            other => panic!("expected Text for {s:?}, got {other:?}"),
        }
    }

    fn err(oid: u32, s: &str) {
        assert!(
            matches!(input(oid, s), Err(PgError::InvalidInputSyntax { .. })),
            "expected InvalidInputSyntax for {s:?}",
        );
    }

    #[test]
    fn macaddr_all_separator_forms_canonicalize() {
        let canonical = "08:00:2b:01:02:03";
        for form in [
            "08:00:2b:01:02:03",
            "08-00-2b-01-02-03",
            "08002b:010203",
            "08002b-010203",
            "0800.2b01.0203",
            "0800-2b01-0203",
            "08002b010203",
        ] {
            assert_eq!(ok(oid::MACADDR, form), canonical, "form {form:?}");
        }
    }

    #[test]
    fn macaddr_case_insensitive_input_lowercases() {
        assert_eq!(ok(oid::MACADDR, "08:00:2B:01:02:03"), "08:00:2b:01:02:03");
        assert_eq!(ok(oid::MACADDR, "AB:CD:EF:01:02:03"), "ab:cd:ef:01:02:03");
        assert_eq!(ok(oid::MACADDR, "08002BAB01CD"), "08:00:2b:ab:01:cd");
    }

    #[test]
    fn macaddr_surrounding_whitespace_trimmed() {
        assert_eq!(
            ok(oid::MACADDR, "  08:00:2b:01:02:03  "),
            "08:00:2b:01:02:03"
        );
    }

    #[test]
    fn macaddr_colon_three_group_is_rejected() {

        err(oid::MACADDR, "0800:2b01:0203");
    }

    #[test]
    fn macaddr_rejects_bad_input() {
        err(oid::MACADDR, "not even close");
        err(oid::MACADDR, "08:00:2b:01:02:ZZ");
        err(oid::MACADDR, "08:00:2b:01:02:");
        err(oid::MACADDR, "08:00:2b:01:02");
        err(oid::MACADDR, "08:00:2b:01:02:03:04");
        err(oid::MACADDR, "08:00:2b:01:02:100");
        err(oid::MACADDR, "08:00:2b:01:02:03:04:05");
        err(oid::MACADDR, "0800000102030405");
        err(oid::MACADDR, "");
    }

    #[test]
    fn macaddr_round_trips_through_output() {
        let v = input(oid::MACADDR, "08-00-2b-01-02-03").unwrap();
        let rendered = output(oid::MACADDR, &v);
        assert_eq!(rendered, "08:00:2b:01:02:03");

        assert_eq!(ok(oid::MACADDR, &rendered), rendered);
    }

    #[test]
    fn macaddr8_native_8byte_forms_canonicalize() {
        let canonical = "08:00:2b:01:02:03:04:05";
        for form in [
            "08:00:2b:01:02:03:04:05",
            "08-00-2b-01-02-03-04-05",
            "08002b:0102030405",
            "08002b-0102030405",
            "0800.2b01.0203.0405",
            "08002b01:02030405",
            "08002b0102030405",
        ] {
            assert_eq!(ok(oid::MACADDR8, form), canonical, "form {form:?}");
        }
    }

    #[test]
    fn macaddr8_widens_6byte_input_with_ff_fe() {

        let widened = "08:00:2b:ff:fe:01:02:03";
        for form in [
            "08:00:2b:01:02:03",
            "08-00-2b-01-02-03",
            "08002b:010203",
            "08002b-010203",
            "0800.2b01.0203",
            "0800-2b01-0203",
            "08002b010203",
            "0800:2b01:0203",
        ] {
            assert_eq!(ok(oid::MACADDR8, form), widened, "form {form:?}");
        }
    }

    #[test]
    fn macaddr8_case_insensitive_and_trimmed() {
        assert_eq!(
            ok(oid::MACADDR8, "  08:00:2B:01:02:03:04:05  "),
            "08:00:2b:01:02:03:04:05",
        );
    }

    #[test]
    fn macaddr8_rejects_bad_input() {
        err(oid::MACADDR8, "not even close");
        err(oid::MACADDR8, "08:00:2b:01:02:03:04:ZZ");
        err(oid::MACADDR8, "08:00:2b:01:02:03:04:");
        err(oid::MACADDR8, "08:00:2b:01:02:03:04:05:06:07");
        err(oid::MACADDR8, "08-00-2b-01-02-03-04-05-06-07");
        err(oid::MACADDR8, "08002b:01020304050607");
        err(oid::MACADDR8, "08002b01020304050607");
        err(oid::MACADDR8, "0z002b0102030405");
        err(oid::MACADDR8, "08:00-2b:01:02:03:04:05");
        err(oid::MACADDR8, "08:00:2b:01.02:03:04:05");
        err(oid::MACADDR8, ":08:00:2b:01:02:03");
        err(oid::MACADDR8, "");
    }

    #[test]
    fn macaddr8_round_trips_through_output() {
        let v = input(oid::MACADDR8, "08002b0102030405").unwrap();
        let rendered = output(oid::MACADDR8, &v);
        assert_eq!(rendered, "08:00:2b:01:02:03:04:05");
        assert_eq!(ok(oid::MACADDR8, &rendered), rendered);
    }

    #[test]
    fn byte_count_boundary_between_families() {

        assert_eq!(ok(oid::MACADDR, "08:00:2b:01:02:03"), "08:00:2b:01:02:03");
        assert_eq!(
            ok(oid::MACADDR8, "08:00:2b:01:02:03"),
            "08:00:2b:ff:fe:01:02:03"
        );

        err(oid::MACADDR, "08:00:2b:01:02:03:04:05");
        assert_eq!(
            ok(oid::MACADDR8, "08:00:2b:01:02:03:04:05"),
            "08:00:2b:01:02:03:04:05",
        );
    }

    #[test]
    fn output_non_text_is_empty() {
        assert_eq!(output(oid::MACADDR, &SqlValue::Null), "");
        assert_eq!(output(oid::MACADDR8, &SqlValue::Int(5)), "");
    }
}
