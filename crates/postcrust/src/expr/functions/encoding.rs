
use crate::types::PgError;
use sql_core::SqlValue;

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    match name {
        "md5" => Some(md5_call(args)),
        "to_hex" => Some(to_hex_call(args)),

        _ => None,
    }
}

fn wrong(name: &str) -> Result<SqlValue, PgError> {
    Err(PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("function {name}(...) does not exist"),
    })
}

fn md5_call(args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() != 1 {
        return wrong("md5");
    }
    match &args[0] {
        SqlValue::Null => Ok(SqlValue::Null),
        SqlValue::Text(s) => Ok(SqlValue::Text(md5_hex(s.as_bytes()))),
        _ => wrong("md5"),
    }
}

fn to_hex_call(args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() != 1 {
        return wrong("to_hex");
    }
    match &args[0] {
        SqlValue::Null => Ok(SqlValue::Null),

        SqlValue::Int(n) => Ok(SqlValue::Text(format!("{:x}", *n as u64))),
        _ => wrong("to_hex"),
    }
}

fn md5_hex(msg: &[u8]) -> String {

    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];

    const K: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613,
        0xfd469501, 0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193,
        0xa679438e, 0x49b40821, 0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d,
        0x02441453, 0xd8a1e681, 0xe7d3fbc8, 0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
        0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a, 0xfffa3942, 0x8771f681, 0x6d9d6122,
        0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, 0x289b7ec6, 0xeaa127fa,
        0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665, 0xf4292244,
        0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb,
        0xeb86d391,
    ];

    let mut a0: u32 = 0x67452301;
    let mut b0: u32 = 0xefcdab89;
    let mut c0: u32 = 0x98badcfe;
    let mut d0: u32 = 0x10325476;

    let bit_len = (msg.len() as u64).wrapping_mul(8);
    let mut data = msg.to_vec();
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bit_len.to_le_bytes());

    for chunk in data.chunks_exact(64) {
        let mut m = [0u32; 16];
        for (i, word) in m.iter_mut().enumerate() {
            let j = i * 4;
            *word = u32::from_le_bytes([chunk[j], chunk[j + 1], chunk[j + 2], chunk[j + 3]]);
        }

        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i {
                0..=15 => ((b & c) | (!b & d), i),
                16..=31 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let tmp = d;
            d = c;
            c = b;
            let sum = a.wrapping_add(f).wrapping_add(K[i]).wrapping_add(m[g]);
            b = b.wrapping_add(sum.rotate_left(S[i]));
            a = tmp;
        }

        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }

    let mut out = String::with_capacity(32);
    for word in [a0, b0, c0, d0] {
        for byte in word.to_le_bytes() {
            out.push_str(&format!("{byte:02x}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::call;
    use sql_core::SqlValue;

    fn text(s: &str) -> SqlValue {
        SqlValue::Text(s.to_string())
    }

    fn unwrap_text(v: Option<Result<SqlValue, crate::types::PgError>>) -> String {
        match v {
            Some(Ok(SqlValue::Text(s))) => s,
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn md5_known_vectors() {
        assert_eq!(
            unwrap_text(call("md5", &[text("")])),
            "d41d8cd98f00b204e9800998ecf8427e"
        );
        assert_eq!(
            unwrap_text(call("md5", &[text("abc")])),
            "900150983cd24fb0d6963f7d28e17f72"
        );

        assert_eq!(
            unwrap_text(call(
                "md5",
                &[text("The quick brown fox jumps over the lazy dog")]
            )),
            "9e107d9d372bb6826bd81d3542a419d6"
        );
    }

    #[test]
    fn to_hex_positive() {
        assert_eq!(unwrap_text(call("to_hex", &[SqlValue::Int(255)])), "ff");

        assert_eq!(
            unwrap_text(call("to_hex", &[SqlValue::Int(256 * 256 * 256 - 1)])),
            "ffffff"
        );
    }

    #[test]
    fn to_hex_negative_twos_complement() {

        assert_eq!(
            unwrap_text(call("to_hex", &[SqlValue::Int(-1234)])),
            "fffffffffffffb2e"
        );
        assert_eq!(
            unwrap_text(call("to_hex", &[SqlValue::Int(-1)])),
            "ffffffffffffffff"
        );
    }

    #[test]
    fn null_propagation() {
        assert!(matches!(
            call("md5", &[SqlValue::Null]),
            Some(Ok(SqlValue::Null))
        ));
        assert!(matches!(
            call("to_hex", &[SqlValue::Null]),
            Some(Ok(SqlValue::Null))
        ));
    }

    #[test]
    fn wrong_arity_or_type() {
        assert!(matches!(call("md5", &[]), Some(Err(_))));
        assert!(matches!(call("to_hex", &[text("x")]), Some(Err(_))));
    }

    #[test]
    fn unclaimed_name_falls_through() {
        assert!(call("sha256", &[text("x")]).is_none());
        assert!(call("encode", &[text("x"), text("hex")]).is_none());
    }
}
