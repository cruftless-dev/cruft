
use crate::types::PgError;
use sql_core::SqlValue;

fn err(name: &str) -> Option<Result<SqlValue, PgError>> {
    Some(Err(PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("function {name}(...) does not exist"),
    }))
}

fn owns(name: &str) -> bool {
    matches!(
        name,
        "length"
            | "char_length"
            | "character_length"
            | "octet_length"
            | "bit_length"
            | "upper"
            | "lower"
            | "initcap"
            | "reverse"
            | "repeat"
            | "ascii"
            | "chr"
    )
}

fn initcap(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_word = false;
    for c in s.chars() {
        if c.is_alphanumeric() {
            if in_word {
                out.extend(c.to_lowercase());
            } else {
                out.extend(c.to_uppercase());
            }
            in_word = true;
        } else {
            out.push(c);
            in_word = false;
        }
    }
    out
}

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    if !owns(name) {
        return None;
    }

    if args.iter().any(|a| matches!(a, SqlValue::Null)) {
        return Some(Ok(SqlValue::Null));
    }

    match name {
        "length" | "char_length" | "character_length" => match args {
            [SqlValue::Text(s)] => {

                if name == "length" {
                    if let Some(n) = super::text_search::tsvector_length(s) {
                        return Some(Ok(SqlValue::Int(n)));
                    }
                }
                Some(Ok(SqlValue::Int(s.chars().count() as i64)))
            }
            _ => err(name),
        },
        "octet_length" => match args {
            [SqlValue::Text(s)] => Some(Ok(SqlValue::Int(s.len() as i64))),
            [SqlValue::Blob(b)] => Some(Ok(SqlValue::Int(b.len() as i64))),
            _ => err(name),
        },
        "bit_length" => match args {
            [SqlValue::Text(s)] => Some(Ok(SqlValue::Int((s.len() * 8) as i64))),
            [SqlValue::Blob(b)] => Some(Ok(SqlValue::Int((b.len() * 8) as i64))),
            _ => err(name),
        },
        "upper" => match args {
            [SqlValue::Text(s)] => Some(Ok(SqlValue::Text(s.to_uppercase()))),
            _ => err(name),
        },
        "lower" => match args {
            [SqlValue::Text(s)] => Some(Ok(SqlValue::Text(s.to_lowercase()))),
            _ => err(name),
        },
        "initcap" => match args {
            [SqlValue::Text(s)] => Some(Ok(SqlValue::Text(initcap(s)))),
            _ => err(name),
        },
        "reverse" => match args {
            [SqlValue::Text(s)] => Some(Ok(SqlValue::Text(s.chars().rev().collect()))),
            _ => err(name),
        },
        "repeat" => match args {
            [SqlValue::Text(s), SqlValue::Int(n)] => {
                let count = if *n > 0 { *n as usize } else { 0 };
                Some(Ok(SqlValue::Text(s.repeat(count))))
            }
            _ => err(name),
        },
        "ascii" => match args {
            [SqlValue::Text(s)] => {
                let code = s.chars().next().map(|c| c as i64).unwrap_or(0);
                Some(Ok(SqlValue::Int(code)))
            }
            _ => err(name),
        },
        "chr" => match args {
            [SqlValue::Int(n)] => {
                if *n == 0 {
                    return Some(Err(PgError::InvalidInputSyntax {
                        typ: "expression",
                        input: "null character not permitted".to_string(),
                    }));
                }
                match u32::try_from(*n).ok().and_then(char::from_u32) {
                    Some(c) => Some(Ok(SqlValue::Text(c.to_string()))),
                    None => Some(Err(PgError::InvalidInputSyntax {
                        typ: "expression",
                        input: format!("requested character too large for encoding: {n}"),
                    })),
                }
            }
            _ => err(name),
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::call;
    use sql_core::SqlValue;

    fn t(s: &str) -> SqlValue {
        SqlValue::Text(s.to_string())
    }

    #[test]
    fn rehomed_length_upper_lower() {
        assert_eq!(
            call("length", &[t("hello")]).unwrap().unwrap(),
            SqlValue::Int(5)
        );
        assert_eq!(call("upper", &[t("aBc")]).unwrap().unwrap(), t("ABC"));
        assert_eq!(call("lower", &[t("aBc")]).unwrap().unwrap(), t("abc"));
    }

    #[test]
    fn char_vs_octet_multibyte() {

        let s = "café";
        assert_eq!(
            call("char_length", &[t(s)]).unwrap().unwrap(),
            SqlValue::Int(4)
        );
        assert_eq!(
            call("character_length", &[t(s)]).unwrap().unwrap(),
            SqlValue::Int(4)
        );
        assert_eq!(call("length", &[t(s)]).unwrap().unwrap(), SqlValue::Int(4));
        assert_eq!(
            call("octet_length", &[t(s)]).unwrap().unwrap(),
            SqlValue::Int(5)
        );
        assert_eq!(
            call("bit_length", &[t(s)]).unwrap().unwrap(),
            SqlValue::Int(40)
        );
    }

    #[test]
    fn initcap_golden() {
        assert_eq!(
            call("initcap", &[t("hi THOMAS")]).unwrap().unwrap(),
            t("Hi Thomas")
        );
    }

    #[test]
    fn reverse_by_char() {
        assert_eq!(call("reverse", &[t("abcé")]).unwrap().unwrap(), t("écba"));
    }

    #[test]
    fn repeat_semantics() {
        assert_eq!(
            call("repeat", &[t("Pg"), SqlValue::Int(4)])
                .unwrap()
                .unwrap(),
            t("PgPgPgPg")
        );
        assert_eq!(
            call("repeat", &[t("Pg"), SqlValue::Int(-4)])
                .unwrap()
                .unwrap(),
            t("")
        );
        assert_eq!(
            call("repeat", &[t("Pg"), SqlValue::Int(0)])
                .unwrap()
                .unwrap(),
            t("")
        );
    }

    #[test]
    fn ascii_and_chr() {
        assert_eq!(
            call("ascii", &[t("A")]).unwrap().unwrap(),
            SqlValue::Int(65)
        );
        assert_eq!(call("ascii", &[t("")]).unwrap().unwrap(), SqlValue::Int(0));
        assert_eq!(call("chr", &[SqlValue::Int(65)]).unwrap().unwrap(), t("A"));
    }

    #[test]
    fn chr_zero_errors() {
        assert!(call("chr", &[SqlValue::Int(0)]).unwrap().is_err());
    }

    #[test]
    fn null_propagation() {
        assert_eq!(
            call("upper", &[SqlValue::Null]).unwrap().unwrap(),
            SqlValue::Null
        );
        assert_eq!(
            call("repeat", &[SqlValue::Null, SqlValue::Int(3)])
                .unwrap()
                .unwrap(),
            SqlValue::Null
        );
    }

    #[test]
    fn wrong_arity_claimed() {

        assert!(call("upper", &[SqlValue::Int(1)]).unwrap().is_err());
        assert!(call("repeat", &[t("x")]).unwrap().is_err());
    }

    #[test]
    fn unclaimed_name_is_none() {
        assert!(call("substr", &[t("x")]).is_none());
        assert!(call("trim", &[t("x")]).is_none());
    }
}
