
use crate::types::PgError;
use sql_core::SqlValue;

fn err(name: &str) -> Result<SqlValue, PgError> {
    Err(PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("function {name}(...) does not exist"),
    })
}

fn text_of(v: &SqlValue) -> String {
    match v {
        SqlValue::Null => String::new(),
        SqlValue::Int(i) => i.to_string(),
        SqlValue::Real(r) => r.to_string(),
        SqlValue::Text(s) => s.clone(),
        SqlValue::Blob(_) => String::new(),
    }
}

fn is_reserved(word: &str) -> bool {
    matches!(
        word,
        "all"
            | "and"
            | "any"
            | "as"
            | "asc"
            | "between"
            | "both"
            | "case"
            | "cast"
            | "check"
            | "collate"
            | "column"
            | "constraint"
            | "create"
            | "current_date"
            | "current_time"
            | "current_timestamp"
            | "current_user"
            | "default"
            | "deferrable"
            | "desc"
            | "distinct"
            | "do"
            | "else"
            | "end"
            | "except"
            | "false"
            | "for"
            | "foreign"
            | "from"
            | "grant"
            | "group"
            | "having"
            | "in"
            | "initially"
            | "intersect"
            | "into"
            | "is"
            | "leading"
            | "limit"
            | "localtime"
            | "localtimestamp"
            | "not"
            | "null"
            | "offset"
            | "on"
            | "only"
            | "or"
            | "order"
            | "placing"
            | "primary"
            | "references"
            | "returning"
            | "select"
            | "session_user"
            | "some"
            | "symmetric"
            | "table"
            | "then"
            | "to"
            | "trailing"
            | "true"
            | "union"
            | "unique"
            | "user"
            | "using"
            | "variadic"
            | "when"
            | "where"
            | "with"
    )
}

fn quote_ident_str(s: &str) -> String {
    let needs_quote = s.is_empty()
        || s.chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
        || !s
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        || is_reserved(s);
    if !needs_quote {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        if c == '"' {
            out.push('"');
        }
        out.push(c);
    }
    out.push('"');
    out
}

fn quote_literal_str(s: &str) -> String {
    let has_backslash = s.contains('\\');
    let mut body = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '\'' => body.push_str("''"),
            '\\' if has_backslash => body.push_str("\\\\"),
            other => body.push(other),
        }
    }
    if has_backslash {
        format!("E'{body}'")
    } else {
        format!("'{body}'")
    }
}

fn quote_nullable_val(v: &SqlValue) -> String {
    match v {
        SqlValue::Null => "NULL".to_string(),
        other => quote_literal_str(&text_of(other)),
    }
}

fn format_impl(fmt: &str, args: &[SqlValue]) -> Result<String, PgError> {
    let bytes: Vec<char> = fmt.chars().collect();
    let mut out = String::new();
    let mut i = 0;

    let mut auto_idx = 0usize;
    let too_few = || {
        Err(PgError::InvalidInputSyntax {
            typ: "expression",
            input: "too few arguments for format()".to_string(),
        })
    };
    while i < bytes.len() {
        let c = bytes[i];
        if c != '%' {
            out.push(c);
            i += 1;
            continue;
        }

        i += 1;
        if i >= bytes.len() {
            return Err(PgError::InvalidInputSyntax {
                typ: "expression",
                input: "unterminated format() type specifier".to_string(),
            });
        }

        let mut explicit: Option<usize> = None;
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == '$' {
                let num: usize = bytes[start..i].iter().collect::<String>().parse().unwrap();
                explicit = Some(num);
                i += 1;
            } else {

                return Err(PgError::InvalidInputSyntax {
                    typ: "expression",
                    input: "unterminated format() type specifier".to_string(),
                });
            }
        }
        if i >= bytes.len() {
            return Err(PgError::InvalidInputSyntax {
                typ: "expression",
                input: "unterminated format() type specifier".to_string(),
            });
        }
        let spec = bytes[i];
        i += 1;
        if spec == '%' {
            if explicit.is_some() {
                return Err(PgError::InvalidInputSyntax {
                    typ: "expression",
                    input: "unrecognized format() type specifier \"%\"".to_string(),
                });
            }
            out.push('%');
            continue;
        }

        let arg_index = match explicit {
            Some(0) => {
                return Err(PgError::InvalidInputSyntax {
                    typ: "expression",
                    input: "format specifies argument 0, but arguments are numbered from 1"
                        .to_string(),
                });
            }
            Some(n) => n - 1,
            None => {
                let idx = auto_idx;
                auto_idx += 1;
                idx
            }
        };
        let arg = match args.get(arg_index) {
            Some(a) => a,
            None => return too_few(),
        };
        match spec {
            's' => out.push_str(&text_of(arg)),
            'I' => {
                if matches!(arg, SqlValue::Null) {
                    return Err(PgError::InvalidInputSyntax {
                        typ: "expression",
                        input: "null values cannot be formatted as an SQL identifier".to_string(),
                    });
                }
                out.push_str(&quote_ident_str(&text_of(arg)));
            }
            'L' => out.push_str(&quote_nullable_val(arg)),
            other => {
                return Err(PgError::InvalidInputSyntax {
                    typ: "expression",
                    input: format!("unrecognized format() type specifier \"{other}\""),
                });
            }
        }
    }
    Ok(out)
}

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    match name {
        "quote_ident" => {
            let [a] = args else { return Some(err(name)) };
            match a {
                SqlValue::Null => Some(Ok(SqlValue::Null)),
                SqlValue::Text(s) => Some(Ok(SqlValue::Text(quote_ident_str(s)))),
                other => Some(Ok(SqlValue::Text(quote_ident_str(&text_of(other))))),
            }
        }
        "quote_literal" => {
            let [a] = args else { return Some(err(name)) };
            match a {
                SqlValue::Null => Some(Ok(SqlValue::Null)),
                other => Some(Ok(SqlValue::Text(quote_literal_str(&text_of(other))))),
            }
        }
        "quote_nullable" => {
            let [a] = args else { return Some(err(name)) };
            Some(Ok(SqlValue::Text(quote_nullable_val(a))))
        }
        "format" => {
            let Some((fmt_val, rest)) = args.split_first() else {
                return Some(err(name));
            };
            match fmt_val {
                SqlValue::Null => Some(Ok(SqlValue::Null)),
                SqlValue::Text(fmt) => Some(match format_impl(fmt, rest) {
                    Ok(s) => Ok(SqlValue::Text(s)),
                    Err(e) => Err(e),
                }),
                other => {
                    let fmt = text_of(other);
                    Some(match format_impl(&fmt, rest) {
                        Ok(s) => Ok(SqlValue::Text(s)),
                        Err(e) => Err(e),
                    })
                }
            }
        }
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
    fn text(r: Option<Result<SqlValue, crate::types::PgError>>) -> String {
        match r {
            Some(Ok(SqlValue::Text(s))) => s,
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn quote_ident_safe_vs_quoted() {
        assert_eq!(text(call("quote_ident", &[t("foo")])), "foo");
        assert_eq!(text(call("quote_ident", &[t("foo_bar1")])), "foo_bar1");
        assert_eq!(text(call("quote_ident", &[t("Foo Bar")])), "\"Foo Bar\"");

        assert_eq!(text(call("quote_ident", &[t("1abc")])), "\"1abc\"");

        assert_eq!(text(call("quote_ident", &[t("select")])), "\"select\"");

        assert_eq!(text(call("quote_ident", &[t("a\"b")])), "\"a\"\"b\"");
    }

    #[test]
    fn quote_literal_golden() {
        assert_eq!(text(call("quote_literal", &[t("")])), "''");
        assert_eq!(text(call("quote_literal", &[t("abc'")])), "'abc'''");

        assert_eq!(text(call("quote_literal", &[t("\\")])), "E'\\\\'");
    }

    #[test]
    fn quote_literal_is_strict() {
        assert!(matches!(
            call("quote_literal", &[SqlValue::Null]),
            Some(Ok(SqlValue::Null))
        ));
        assert!(matches!(
            call("quote_ident", &[SqlValue::Null]),
            Some(Ok(SqlValue::Null))
        ));
    }

    #[test]
    fn quote_nullable_value_and_null() {
        assert_eq!(text(call("quote_nullable", &[t("abc")])), "'abc'");
        assert_eq!(text(call("quote_nullable", &[SqlValue::Null])), "NULL");
        assert_eq!(text(call("quote_nullable", &[SqlValue::Int(10)])), "'10'");
    }

    #[test]
    fn format_basic_specs() {
        assert_eq!(
            text(call("format", &[t("Hello %s"), t("World")])),
            "Hello World"
        );
        assert_eq!(text(call("format", &[t("Hello %%")])), "Hello %");
        assert_eq!(
            text(call(
                "format",
                &[
                    t("INSERT INTO %I VALUES(%L,%L)"),
                    t("mytab"),
                    SqlValue::Int(10),
                    t("Hello")
                ]
            )),
            "INSERT INTO mytab VALUES('10','Hello')"
        );
    }

    #[test]
    fn format_s_of_null_is_empty() {

        assert_eq!(
            text(call(
                "format",
                &[t("%s%s%s"), t("Hello"), SqlValue::Null, t("World")]
            )),
            "HelloWorld"
        );
    }

    #[test]
    fn format_l_of_null_is_unquoted_null() {
        assert_eq!(
            text(call(
                "format",
                &[
                    t("INSERT INTO %I VALUES(%L,%L)"),
                    t("mytab"),
                    SqlValue::Int(10),
                    SqlValue::Null
                ]
            )),
            "INSERT INTO mytab VALUES('10',NULL)"
        );
    }

    #[test]
    fn format_i_of_null_errors() {
        assert!(matches!(
            call(
                "format",
                &[
                    t("INSERT INTO %I VALUES(%L,%L)"),
                    SqlValue::Null,
                    SqlValue::Int(10),
                    t("Hello")
                ]
            ),
            Some(Err(_))
        ));
    }

    #[test]
    fn format_positional() {
        assert_eq!(
            text(call(
                "format",
                &[
                    t("%1$s %3$s"),
                    SqlValue::Int(1),
                    SqlValue::Int(2),
                    SqlValue::Int(3)
                ]
            )),
            "1 3"
        );
    }

    #[test]
    fn format_too_few_args_and_bad_spec() {
        assert!(matches!(call("format", &[t("Hello %s")]), Some(Err(_))));
        assert!(matches!(
            call("format", &[t("Hello %x"), SqlValue::Int(20)]),
            Some(Err(_))
        ));
    }

    #[test]
    fn format_null_fmt_is_null() {
        assert!(matches!(
            call("format", &[SqlValue::Null]),
            Some(Ok(SqlValue::Null))
        ));
    }

    #[test]
    fn unclaimed_name_returns_none() {
        assert!(call("upper", &[t("x")]).is_none());
    }

    #[test]
    fn wrong_arity_is_claimed_error() {
        assert!(matches!(call("quote_ident", &[]), Some(Err(_))));
        assert!(matches!(
            call("quote_literal", &[t("a"), t("b")]),
            Some(Err(_))
        ));
    }
}
