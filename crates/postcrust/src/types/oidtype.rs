
use super::PgError;
use sql_core::SqlValue;

fn is_pg_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r')
}

fn split_sign(s: &str) -> (bool, &str) {
    match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    }
}

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    let typ = super::type_name(oid);
    let trimmed = text.trim_matches(is_pg_space);
    let (negative, digits) = split_sign(trimmed);

    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(PgError::InvalidInputSyntax {
            typ,
            input: text.to_string(),
        });
    }

    let mag: u128 = match digits.parse() {
        Ok(m) => m,
        Err(_) => {
            return Err(PgError::OutOfRange {
                typ,
                input: text.to_string(),
            })
        }
    };

    if mag > u64::MAX as u128 {
        return Err(PgError::OutOfRange {
            typ,
            input: text.to_string(),
        });
    }

    let cvt: u64 = if negative {
        0u64.wrapping_sub(mag as u64)
    } else {
        mag as u64
    };
    let result: u32 = cvt as u32;

    if cvt == result as u64 || cvt == (result as i32) as u64 {
        Ok(SqlValue::Int(result as i64))
    } else {
        Err(PgError::OutOfRange {
            typ,
            input: text.to_string(),
        })
    }
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

    const OID: u32 = super::super::oid::OID;

    fn int(v: SqlValue) -> i64 {
        match v {
            SqlValue::Int(i) => i,
            other => panic!("expected Int, got {other:?}"),
        }
    }

    #[test]
    fn zero_and_max() {
        assert_eq!(int(input(OID, "0").unwrap()), 0);
        assert_eq!(int(input(OID, "4294967295").unwrap()), 4_294_967_295);
    }

    #[test]
    fn mid_value() {
        assert_eq!(int(input(OID, "1234").unwrap()), 1234);
        assert_eq!(int(input(OID, "99999999").unwrap()), 99_999_999);
    }

    #[test]
    fn leading_plus_is_accepted() {
        assert_eq!(int(input(OID, "+987").unwrap()), 987);
    }

    #[test]
    fn small_negative_wraps() {

        assert_eq!(int(input(OID, "-1040").unwrap()), 4_294_966_256);
    }

    #[test]
    fn whitespace_is_trimmed() {

        assert_eq!(int(input(OID, "5     ").unwrap()), 5);
        assert_eq!(int(input(OID, "   10  ").unwrap()), 10);
        assert_eq!(int(input(OID, "\t  15 \t  ").unwrap()), 15);
    }

    #[test]
    fn over_max_is_out_of_range() {

        let e = input(OID, "4294967296").unwrap_err();
        assert_eq!(
            e,
            PgError::OutOfRange {
                typ: "oid",
                input: "4294967296".into()
            }
        );
        assert_eq!(
            e.message(),
            "value \"4294967296\" is out of range for type oid"
        );
    }

    #[test]
    fn past_u32_within_u64_is_out_of_range() {

        assert_eq!(
            input(OID, "9999999999").unwrap_err(),
            PgError::OutOfRange {
                typ: "oid",
                input: "9999999999".into()
            }
        );
    }

    #[test]
    fn past_u64_is_out_of_range() {

        assert_eq!(
            input(OID, "32958209582039852935").unwrap_err(),
            PgError::OutOfRange {
                typ: "oid",
                input: "32958209582039852935".into()
            }
        );
        assert_eq!(
            input(OID, "-23582358720398502385").unwrap_err(),
            PgError::OutOfRange {
                typ: "oid",
                input: "-23582358720398502385".into()
            }
        );
    }

    #[test]
    fn empty_and_all_whitespace_are_invalid() {
        let e = input(OID, "").unwrap_err();
        assert_eq!(
            e,
            PgError::InvalidInputSyntax {
                typ: "oid",
                input: "".into()
            }
        );
        assert_eq!(e.message(), "invalid input syntax for type oid: \"\"");
        assert_eq!(
            input(OID, "    ").unwrap_err(),
            PgError::InvalidInputSyntax {
                typ: "oid",
                input: "    ".into()
            }
        );
    }

    #[test]
    fn non_numeric_is_invalid() {
        assert_eq!(
            input(OID, "asdfasd").unwrap_err(),
            PgError::InvalidInputSyntax {
                typ: "oid",
                input: "asdfasd".into()
            }
        );
    }

    #[test]
    fn garbage_and_embedded_space_are_invalid() {
        for bad in ["99asdfasd", "5    d", "    5d", "5    5"] {
            assert_eq!(
                input(OID, bad).unwrap_err(),
                PgError::InvalidInputSyntax {
                    typ: "oid",
                    input: bad.into()
                },
                "input {bad:?}"
            );
        }
    }

    #[test]
    fn space_between_sign_and_digits_is_invalid() {

        let e = input(OID, " - 500").unwrap_err();
        assert_eq!(
            e,
            PgError::InvalidInputSyntax {
                typ: "oid",
                input: " - 500".into()
            }
        );
    }

    #[test]
    fn output_is_plain_decimal() {
        assert_eq!(output(OID, &SqlValue::Int(1234)), "1234");
        assert_eq!(output(OID, &SqlValue::Int(4_294_967_295)), "4294967295");
        assert_eq!(output(OID, &SqlValue::Int(0)), "0");
    }

    #[test]
    fn output_non_int_is_empty() {
        assert_eq!(output(OID, &SqlValue::Null), "");
    }

    #[test]
    fn round_trips() {
        for s in ["0", "1234", "4294967295"] {
            let v = input(OID, s).unwrap();
            assert_eq!(output(OID, &v), s);
        }

        let v = input(OID, "-1040").unwrap();
        assert_eq!(output(OID, &v), "4294966256");
    }
}
