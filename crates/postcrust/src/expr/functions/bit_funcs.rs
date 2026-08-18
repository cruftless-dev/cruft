
use crate::types::PgError;
use sql_core::SqlValue;

fn no_such(name: &str) -> Option<Result<SqlValue, PgError>> {
    Some(Err(PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("function {name}(...) does not exist"),
    }))
}

fn range_err(typ: &'static str, n: i64) -> Option<Result<SqlValue, PgError>> {
    Some(Err(PgError::InvalidInputSyntax {
        typ,
        input: n.to_string(),
    }))
}

fn owns(name: &str) -> bool {
    matches!(name, "get_bit" | "set_bit" | "get_byte" | "set_byte")
}

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    if !owns(name) {
        return None;
    }

    if args.iter().any(|a| matches!(a, SqlValue::Null)) {
        return Some(Ok(SqlValue::Null));
    }

    match name {
        "get_bit" => match args {

            [SqlValue::Blob(b), SqlValue::Int(n)] => {
                let total = (b.len() as i64) * 8;
                if *n < 0 || *n >= total {
                    return range_err("get_bit", *n);
                }
                let idx = *n as usize;
                let bit = (b[idx / 8] >> (idx % 8)) & 1;
                Some(Ok(SqlValue::Int(bit as i64)))
            }

            [SqlValue::Text(s), SqlValue::Int(n)] => {
                let bytes = s.as_bytes();
                if *n < 0 || *n >= bytes.len() as i64 {
                    return range_err("get_bit", *n);
                }
                let bit = if bytes[*n as usize] == b'1' { 1 } else { 0 };
                Some(Ok(SqlValue::Int(bit)))
            }
            _ => no_such(name),
        },

        "set_bit" => match args {

            [SqlValue::Blob(b), SqlValue::Int(n), SqlValue::Int(v)] => {
                let total = (b.len() as i64) * 8;
                if *n < 0 || *n >= total {
                    return range_err("set_bit", *n);
                }
                if *v != 0 && *v != 1 {

                    return range_err("set_bit", *v);
                }
                let idx = *n as usize;
                let mut out = b.clone();
                let mask = 1u8 << (idx % 8);
                if *v == 1 {
                    out[idx / 8] |= mask;
                } else {
                    out[idx / 8] &= !mask;
                }
                Some(Ok(SqlValue::Blob(out)))
            }

            [SqlValue::Text(s), SqlValue::Int(n), SqlValue::Int(v)] => {
                let mut bytes = s.clone().into_bytes();
                if *n < 0 || *n >= bytes.len() as i64 {
                    return range_err("set_bit", *n);
                }
                if *v != 0 && *v != 1 {
                    return range_err("set_bit", *v);
                }
                bytes[*n as usize] = if *v == 1 { b'1' } else { b'0' };

                Some(Ok(SqlValue::Text(String::from_utf8(bytes).unwrap())))
            }
            _ => no_such(name),
        },

        "get_byte" => match args {

            [SqlValue::Blob(b), SqlValue::Int(n)] => {
                if *n < 0 || *n >= b.len() as i64 {
                    return range_err("get_byte", *n);
                }
                Some(Ok(SqlValue::Int(b[*n as usize] as i64)))
            }
            _ => no_such(name),
        },

        "set_byte" => match args {

            [SqlValue::Blob(b), SqlValue::Int(n), SqlValue::Int(v)] => {
                if *n < 0 || *n >= b.len() as i64 {
                    return range_err("set_byte", *n);
                }
                let mut out = b.clone();

                out[*n as usize] = (*v & 0xff) as u8;
                Some(Ok(SqlValue::Blob(out)))
            }
            _ => no_such(name),
        },

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::call;
    use crate::types::PgError;
    use sql_core::SqlValue;

    fn blob(bytes: &[u8]) -> SqlValue {
        SqlValue::Blob(bytes.to_vec())
    }

    const GOLDEN: [u8; 9] = [0x12, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd, 0xef, 0x00];

    #[test]
    fn get_bit_bytea_golden() {

        assert_eq!(
            call("get_bit", &[blob(&GOLDEN), SqlValue::Int(43)]),
            Some(Ok(SqlValue::Int(1)))
        );
    }

    #[test]
    fn set_bit_bytea_golden() {

        let mut expect = GOLDEN;
        expect[5] = 0xa3;
        assert_eq!(
            call(
                "set_bit",
                &[blob(&GOLDEN), SqlValue::Int(43), SqlValue::Int(0)]
            ),
            Some(Ok(SqlValue::Blob(expect.to_vec())))
        );
    }

    #[test]
    fn get_bit_bitstring_golden() {

        assert_eq!(
            call(
                "get_bit",
                &[SqlValue::Text("0101011000100".into()), SqlValue::Int(10)]
            ),
            Some(Ok(SqlValue::Int(1)))
        );

        assert_eq!(
            call(
                "get_bit",
                &[SqlValue::Text("0101011000100".into()), SqlValue::Int(0)]
            ),
            Some(Ok(SqlValue::Int(0)))
        );
    }

    #[test]
    fn set_bit_bitstring_golden() {

        assert_eq!(
            call(
                "set_bit",
                &[
                    SqlValue::Text("0101011000100100".into()),
                    SqlValue::Int(15),
                    SqlValue::Int(1)
                ]
            ),
            Some(Ok(SqlValue::Text("0101011000100101".into())))
        );
    }

    #[test]
    fn get_byte_golden() {

        assert_eq!(
            call("get_byte", &[blob(&GOLDEN), SqlValue::Int(3)]),
            Some(Ok(SqlValue::Int(120)))
        );
    }

    #[test]
    fn set_byte_golden() {

        let mut expect = GOLDEN;
        expect[7] = 11;
        assert_eq!(
            call(
                "set_byte",
                &[blob(&GOLDEN), SqlValue::Int(7), SqlValue::Int(11)]
            ),
            Some(Ok(SqlValue::Blob(expect.to_vec())))
        );
    }

    #[test]
    fn out_of_range_index_collapses_to_error() {

        assert!(matches!(
            call("get_bit", &[blob(&GOLDEN), SqlValue::Int(99)]),
            Some(Err(PgError::InvalidInputSyntax { typ: "get_bit", .. }))
        ));
        assert!(matches!(
            call(
                "set_bit",
                &[blob(&GOLDEN), SqlValue::Int(99), SqlValue::Int(0)]
            ),
            Some(Err(PgError::InvalidInputSyntax { typ: "set_bit", .. }))
        ));
        assert!(matches!(
            call("get_byte", &[blob(&GOLDEN), SqlValue::Int(99)]),
            Some(Err(PgError::InvalidInputSyntax {
                typ: "get_byte",
                ..
            }))
        ));

        assert!(matches!(
            call(
                "get_bit",
                &[SqlValue::Text("0101011000100100".into()), SqlValue::Int(16)]
            ),
            Some(Err(PgError::InvalidInputSyntax { typ: "get_bit", .. }))
        ));
    }

    #[test]
    fn null_propagates() {
        assert_eq!(
            call("get_bit", &[SqlValue::Null, SqlValue::Int(0)]),
            Some(Ok(SqlValue::Null))
        );
        assert_eq!(
            call(
                "set_byte",
                &[blob(&GOLDEN), SqlValue::Null, SqlValue::Int(1)]
            ),
            Some(Ok(SqlValue::Null))
        );
    }

    #[test]
    fn wrong_arity_or_type_is_does_not_exist() {

        assert!(matches!(
            call(
                "get_byte",
                &[SqlValue::Text("101".into()), SqlValue::Int(0)]
            ),
            Some(Err(PgError::InvalidInputSyntax {
                typ: "expression",
                ..
            }))
        ));

        assert!(matches!(
            call("get_bit", &[blob(&GOLDEN)]),
            Some(Err(PgError::InvalidInputSyntax {
                typ: "expression",
                ..
            }))
        ));
    }

    #[test]
    fn unclaimed_name_returns_none() {
        assert_eq!(call("length", &[blob(&GOLDEN)]), None);
        assert_eq!(call("upper", &[SqlValue::Text("x".into())]), None);
    }
}
