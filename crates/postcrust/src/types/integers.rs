
use super::{oid, type_name, PgError};
use sql_core::SqlValue;

fn range(oid: u32) -> (i64, i64) {
    match oid {
        oid::INT2 => (i16::MIN as i64, i16::MAX as i64),
        oid::INT4 => (i32::MIN as i64, i32::MAX as i64),

        _ => (i64::MIN, i64::MAX),
    }
}

fn is_pg_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r')
}

fn is_int_token(s: &str) -> bool {
    let digits = match s.strip_prefix(['+', '-']) {
        Some(rest) => rest,
        None => s,
    };
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    let typ = type_name(oid);
    let trimmed = text.trim_matches(is_pg_space);

    if !is_int_token(trimmed) {
        return Err(PgError::InvalidInputSyntax {
            typ,
            input: text.to_string(),
        });
    }

    let parsed: i64 = match trimmed.parse() {
        Ok(v) => v,
        Err(_) => {
            return Err(PgError::OutOfRange {
                typ,
                input: text.to_string(),
            })
        }
    };

    let (min, max) = range(oid);
    if parsed < min || parsed > max {
        return Err(PgError::OutOfRange {
            typ,
            input: text.to_string(),
        });
    }

    Ok(SqlValue::Int(parsed))
}

pub fn output(_oid: u32, v: &SqlValue) -> String {
    match v {
        SqlValue::Int(i) => i.to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INT2: u32 = oid::INT2;
    const INT4: u32 = oid::INT4;
    const INT8: u32 = oid::INT8;

    fn int(v: SqlValue) -> i64 {
        match v {
            SqlValue::Int(i) => i,
            other => panic!("expected Int, got {other:?}"),
        }
    }

    #[test]
    fn valid_basic_each_type() {
        assert_eq!(int(input(INT2, "1234").unwrap()), 1234);
        assert_eq!(int(input(INT4, "1234567").unwrap()), 1234567);
        assert_eq!(
            int(input(INT8, "4567890123456789").unwrap()),
            4567890123456789
        );
    }

    #[test]
    fn signs() {
        assert_eq!(int(input(INT4, "-1234").unwrap()), -1234);
        assert_eq!(int(input(INT4, "+1234").unwrap()), 1234);
        assert_eq!(int(input(INT2, "-32767").unwrap()), -32767);
        assert_eq!(int(input(INT2, "0").unwrap()), 0);
    }

    #[test]
    fn whitespace_is_trimmed() {

        assert_eq!(int(input(INT4, "  123  ").unwrap()), 123);
        assert_eq!(int(input(INT4, "\t-42\n").unwrap()), -42);
        assert_eq!(int(input(INT8, "\r\n 99 \t").unwrap()), 99);
    }

    #[test]
    fn float_literal_is_invalid_not_truncated() {

        let e = input(INT4, "34.5").unwrap_err();
        assert_eq!(
            e,
            PgError::InvalidInputSyntax {
                typ: "integer",
                input: "34.5".into()
            }
        );

        assert_eq!(
            e.message(),
            "invalid input syntax for type integer: \"34.5\""
        );

        assert_eq!(
            input(INT2, "34.5").unwrap_err().message(),
            "invalid input syntax for type smallint: \"34.5\""
        );
    }

    #[test]
    fn non_numeric_is_invalid() {
        assert_eq!(
            input(INT2, "asdf").unwrap_err(),
            PgError::InvalidInputSyntax {
                typ: "smallint",
                input: "asdf".into()
            }
        );
        assert_eq!(
            input(INT4, "   asdf   ").unwrap_err(),
            PgError::InvalidInputSyntax {
                typ: "integer",
                input: "   asdf   ".into()
            }
        );
    }

    #[test]
    fn empty_and_all_whitespace_are_invalid() {
        assert_eq!(
            input(INT4, "").unwrap_err(),
            PgError::InvalidInputSyntax {
                typ: "integer",
                input: "".into()
            }
        );

        assert_eq!(
            input(INT2, "    ").unwrap_err(),
            PgError::InvalidInputSyntax {
                typ: "smallint",
                input: "    ".into()
            }
        );
    }

    #[test]
    fn space_after_sign_is_invalid() {

        let e = input(INT2, "- 1234").unwrap_err();
        assert_eq!(
            e,
            PgError::InvalidInputSyntax {
                typ: "smallint",
                input: "- 1234".into()
            }
        );
        assert_eq!(
            e.message(),
            "invalid input syntax for type smallint: \"- 1234\""
        );
    }

    #[test]
    fn embedded_space_is_invalid() {
        assert_eq!(
            input(INT4, "123       5").unwrap_err(),
            PgError::InvalidInputSyntax {
                typ: "integer",
                input: "123       5".into()
            }
        );
        assert_eq!(
            input(INT2, "4 444").unwrap_err(),
            PgError::InvalidInputSyntax {
                typ: "smallint",
                input: "4 444".into()
            }
        );
        assert_eq!(
            input(INT2, "123 dt").unwrap_err(),
            PgError::InvalidInputSyntax {
                typ: "smallint",
                input: "123 dt".into()
            }
        );
    }

    #[test]
    fn sign_only_is_invalid() {
        assert_eq!(
            input(INT4, "-").unwrap_err(),
            PgError::InvalidInputSyntax {
                typ: "integer",
                input: "-".into()
            }
        );
    }

    #[test]
    fn int2_boundary() {
        assert_eq!(int(input(INT2, "32767").unwrap()), 32767);
        assert_eq!(int(input(INT2, "-32768").unwrap()), -32768);

        let e = input(INT2, "100000").unwrap_err();
        assert_eq!(
            e,
            PgError::OutOfRange {
                typ: "smallint",
                input: "100000".into()
            }
        );
        assert_eq!(
            e.message(),
            "value \"100000\" is out of range for type smallint"
        );
        assert_eq!(
            input(INT2, "32768").unwrap_err(),
            PgError::OutOfRange {
                typ: "smallint",
                input: "32768".into()
            }
        );
    }

    #[test]
    fn int4_boundary() {
        assert_eq!(int(input(INT4, "2147483647").unwrap()), 2147483647);
        assert_eq!(int(input(INT4, "-2147483648").unwrap()), -2147483648);
        let e = input(INT4, "2147483648").unwrap_err();
        assert_eq!(
            e,
            PgError::OutOfRange {
                typ: "integer",
                input: "2147483648".into()
            }
        );
        assert_eq!(
            input(INT4, "1000000000000").unwrap_err().message(),
            "value \"1000000000000\" is out of range for type integer"
        );
    }

    #[test]
    fn int8_boundary() {
        assert_eq!(
            int(input(INT8, "9223372036854775807").unwrap()),
            9223372036854775807
        );
        assert_eq!(int(input(INT8, "-9223372036854775808").unwrap()), i64::MIN);

        let e = input(INT8, "9223372036854775808").unwrap_err();
        assert_eq!(
            e,
            PgError::OutOfRange {
                typ: "bigint",
                input: "9223372036854775808".into()
            }
        );
        assert_eq!(
            e.message(),
            "value \"9223372036854775808\" is out of range for type bigint"
        );
    }

    #[test]
    fn output_is_plain_decimal() {
        assert_eq!(output(INT4, &SqlValue::Int(1234)), "1234");
        assert_eq!(output(INT4, &SqlValue::Int(-42)), "-42");
        assert_eq!(output(INT2, &SqlValue::Int(0)), "0");
        assert_eq!(
            output(INT8, &SqlValue::Int(9223372036854775807)),
            "9223372036854775807"
        );
    }

    #[test]
    fn output_non_int_is_empty() {
        assert_eq!(output(INT4, &SqlValue::Null), "");
    }

    #[test]
    fn round_trips() {
        for (oid, s) in [
            (INT2, "-32768"),
            (INT4, "2147483647"),
            (INT8, "-9223372036854775808"),
        ] {
            let v = input(oid, s).unwrap();
            assert_eq!(output(oid, &v), s);
        }
    }
}
