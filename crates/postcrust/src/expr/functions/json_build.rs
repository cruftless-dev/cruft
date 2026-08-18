
use crate::types::PgError;
use sql_core::SqlValue;

fn does_not_exist(name: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("function {name}(...) does not exist"),
    }
}

fn build_error(msg: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "expression",
        input: msg.to_string(),
    }
}

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    match name {
        "to_json" | "to_jsonb" => {
            if args.len() != 1 {
                return Some(Err(does_not_exist(name)));
            }
            Some(json_element(&args[0]).map(SqlValue::Text))
        }
        "json_build_array" | "jsonb_build_array" => Some(build_array(args)),
        "json_build_object" => Some(build_object(args, false)),
        "jsonb_build_object" => Some(build_object(args, true)),
        _ => None,
    }
}

pub(crate) fn value_to_json(v: &SqlValue) -> Result<String, PgError> {
    json_element(v)
}

pub(crate) fn value_to_json_key(v: &SqlValue) -> Result<String, PgError> {
    match v {
        SqlValue::Null => Err(build_error("null value not allowed for object key")),
        SqlValue::Int(n) => Ok(quote_string(&n.to_string())),
        SqlValue::Real(f) => Ok(quote_string(&real_to_json(*f))),
        SqlValue::Text(s) => Ok(quote_string(s)),
        SqlValue::Blob(_) => Err(build_error(
            "key value must be scalar, not array, composite, or json",
        )),
    }
}

fn json_element(v: &SqlValue) -> Result<String, PgError> {
    match v {
        SqlValue::Null => Ok("null".to_string()),

        SqlValue::Int(n) => Ok(n.to_string()),
        SqlValue::Real(f) => Ok(real_to_json(*f)),
        SqlValue::Text(s) => Ok(quote_string(s)),
        SqlValue::Blob(_) => Err(build_error("cannot convert bytea to json")),
    }
}

fn real_to_json(f: f64) -> String {
    format!("{f}")
}

fn quote_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn build_array(args: &[SqlValue]) -> Result<SqlValue, PgError> {
    let mut out = String::from("[");
    for (i, a) in args.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(&json_element(a)?);
    }
    out.push(']');
    Ok(SqlValue::Text(out))
}

fn build_object(args: &[SqlValue], jsonb: bool) -> Result<SqlValue, PgError> {
    if args.len() % 2 != 0 {
        return Err(build_error(
            "argument list must have even number of elements",
        ));
    }
    let mut pairs: Vec<(String, String)> = Vec::with_capacity(args.len() / 2);
    for kv in args.chunks(2) {
        let key = match &kv[0] {
            SqlValue::Null => return Err(build_error("null value not allowed for object key")),

            SqlValue::Int(n) => quote_string(&n.to_string()),
            SqlValue::Real(f) => quote_string(&real_to_json(*f)),
            SqlValue::Text(s) => quote_string(s),
            SqlValue::Blob(_) => {
                return Err(build_error(
                    "key value must be scalar, not array, composite, or json",
                ))
            }
        };
        pairs.push((key, json_element(&kv[1])?));
    }
    if jsonb {

        pairs.sort_by(|a, b| {
            a.0.len()
                .cmp(&b.0.len())
                .then_with(|| a.0.as_bytes().cmp(b.0.as_bytes()))
        });
        let mut deduped: Vec<(String, String)> = Vec::with_capacity(pairs.len());
        for (k, v) in pairs {
            if let Some(last) = deduped.last_mut() {
                if last.0 == k {
                    last.1 = v;
                    continue;
                }
            }
            deduped.push((k, v));
        }
        pairs = deduped;
    }
    let sep = if jsonb { ": " } else { " : " };
    let mut out = String::from("{");
    for (i, (k, v)) in pairs.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(k);
        out.push_str(sep);
        out.push_str(v);
    }
    out.push('}');
    Ok(SqlValue::Text(out))
}

#[cfg(test)]
mod tests {
    use super::call;
    use sql_core::SqlValue;

    fn text(s: &str) -> SqlValue {
        SqlValue::Text(s.to_string())
    }

    fn built(name: &str, args: &[SqlValue]) -> String {
        match call(name, args) {
            Some(Ok(SqlValue::Text(t))) => t,
            other => panic!("{name}: expected Text, got {other:?}"),
        }
    }

    #[test]
    fn to_json_scalars() {

        assert_eq!(built("to_json", &[SqlValue::Int(42)]), "42");
        assert_eq!(built("to_json", &[text("a")]), r#""a""#);
        assert_eq!(built("to_jsonb", &[SqlValue::Int(42)]), "42");
        assert_eq!(built("to_jsonb", &[text("a")]), r#""a""#);

        assert_eq!(built("to_json", &[SqlValue::Null]), "null");
        assert_eq!(built("to_json", &[SqlValue::Real(1.2)]), "1.2");
    }

    #[test]
    fn to_json_string_escaping() {

        assert_eq!(built("to_json", &[text("a\"b\\c\n")]), r#""a\"b\\c\n""#);
        assert_eq!(built("to_jsonb", &[text("café")]), "\"café\"");
    }

    #[test]
    fn build_array_basic() {

        assert_eq!(
            built(
                "json_build_array",
                &[text("a"), SqlValue::Int(1), text("b")]
            ),
            r#"["a", 1, "b"]"#,
        );
        assert_eq!(
            built("jsonb_build_array", &[text("a"), SqlValue::Null]),
            r#"["a", null]"#
        );

        assert_eq!(built("json_build_array", &[]), "[]");
        assert_eq!(built("jsonb_build_array", &[]), "[]");
    }

    #[test]
    fn build_object_json_spacing_preserves_order() {

        assert_eq!(
            built(
                "json_build_object",
                &[text("k"), SqlValue::Int(1), text("k2"), text("v")]
            ),
            r#"{"k" : 1, "k2" : "v"}"#,
        );

        assert_eq!(
            built(
                "json_build_object",
                &[text("z"), SqlValue::Int(1), text("a"), SqlValue::Int(2)]
            ),
            r#"{"z" : 1, "a" : 2}"#,
        );
        assert_eq!(built("json_build_object", &[]), "{}");
    }

    #[test]
    fn build_object_jsonb_canonical() {

        assert_eq!(
            built(
                "jsonb_build_object",
                &[text("z"), SqlValue::Int(1), text("a"), SqlValue::Int(2)]
            ),
            r#"{"a": 2, "z": 1}"#,
        );

        assert_eq!(
            built(
                "jsonb_build_object",
                &[text("aa"), SqlValue::Int(1), text("b"), SqlValue::Int(2)],
            ),
            r#"{"b": 2, "aa": 1}"#,
        );

        assert_eq!(
            built(
                "jsonb_build_object",
                &[text("k"), SqlValue::Int(1), text("k"), SqlValue::Int(9)],
            ),
            r#"{"k": 9}"#,
        );
    }

    #[test]
    fn odd_arg_count_is_even_number_error() {
        for name in ["json_build_object", "jsonb_build_object"] {
            match call(name, &[text("a"), SqlValue::Int(1), text("c")]) {
                Some(Err(crate::types::PgError::InvalidInputSyntax {
                    typ: "expression",
                    input,
                })) => {
                    assert!(input.contains("even number"), "{name}: {input}");
                }
                other => panic!("{name}: expected even-number error, got {other:?}"),
            }
        }
    }

    #[test]
    fn null_key_is_rejected() {
        for name in ["json_build_object", "jsonb_build_object"] {
            match call(name, &[SqlValue::Null, SqlValue::Int(1)]) {
                Some(Err(crate::types::PgError::InvalidInputSyntax {
                    typ: "expression",
                    input,
                })) => {
                    assert!(input.contains("object key"), "{name}: {input}");
                }
                other => panic!("{name}: expected null-key error, got {other:?}"),
            }
        }
    }

    #[test]
    fn wrong_arity_to_json_is_claimed_error() {

        for name in ["to_json", "to_jsonb"] {
            match call(name, &[SqlValue::Int(1), SqlValue::Int(2)]) {
                Some(Err(crate::types::PgError::InvalidInputSyntax {
                    typ: "expression",
                    input,
                })) => {
                    assert_eq!(input, format!("function {name}(...) does not exist"));
                }
                other => panic!("{name}: expected does-not-exist, got {other:?}"),
            }
            assert!(call(name, &[]).unwrap().is_err());
        }
    }

    #[test]
    fn unclaimed_name_returns_none() {
        assert!(call("array_to_json", &[SqlValue::Null]).is_none());
        assert!(call("json_object", &[]).is_none());
        assert!(call("lpad", &[text("x")]).is_none());
    }
}
