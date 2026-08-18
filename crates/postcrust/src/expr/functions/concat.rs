
use crate::types::PgError;
use sql_core::SqlValue;

fn text_form(v: &SqlValue) -> Option<String> {
    match v {
        SqlValue::Null => None,
        SqlValue::Text(s) => Some(s.clone()),
        SqlValue::Int(n) => Some(n.to_string()),

        SqlValue::Real(f) => Some(f.to_string()),
        SqlValue::Blob(_) => None,
    }
}

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    match name {
        "concat" => {

            let out: String = args.iter().filter_map(text_form).collect();
            Some(Ok(SqlValue::Text(out)))
        }
        "concat_ws" => {

            let Some((sep, rest)) = args.split_first() else {

                return Some(Ok(SqlValue::Null));
            };
            let sep = match text_form(sep) {
                Some(s) => s,
                None => return Some(Ok(SqlValue::Null)),
            };
            let parts: Vec<String> = rest.iter().filter_map(text_form).collect();
            Some(Ok(SqlValue::Text(parts.join(&sep))))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::call;
    use sql_core::SqlValue;

    fn text(v: SqlValue) -> String {
        match v {
            SqlValue::Text(s) => s,
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn concat_mixed_text_and_int() {
        let r = call(
            "concat",
            &[
                SqlValue::Int(1),
                SqlValue::Int(2),
                SqlValue::Int(3),
                SqlValue::Text("hello".into()),
            ],
        )
        .unwrap()
        .unwrap();
        assert_eq!(text(r), "123hello");
    }

    #[test]
    fn concat_skips_null_not_propagates() {

        let r = call(
            "concat",
            &[
                SqlValue::Text("a".into()),
                SqlValue::Null,
                SqlValue::Text("b".into()),
            ],
        )
        .unwrap()
        .unwrap();
        assert_eq!(text(r), "ab");
    }

    #[test]
    fn concat_ws_skips_nulls() {

        let r = call(
            "concat_ws",
            &[
                SqlValue::Text(",".into()),
                SqlValue::Int(10),
                SqlValue::Int(20),
                SqlValue::Null,
                SqlValue::Int(30),
            ],
        )
        .unwrap()
        .unwrap();
        assert_eq!(text(r), "10,20,30");
    }

    #[test]
    fn concat_ws_null_separator_is_null() {

        let r = call(
            "concat_ws",
            &[SqlValue::Null, SqlValue::Int(10), SqlValue::Int(20)],
        )
        .unwrap()
        .unwrap();
        assert!(matches!(r, SqlValue::Null));
    }

    #[test]
    fn empty_concat_is_empty_string() {
        let r = call("concat", &[]).unwrap().unwrap();
        assert_eq!(text(r), "");
    }

    #[test]
    fn unclaimed_name_returns_none() {
        assert!(call("substr", &[SqlValue::Text("x".into())]).is_none());
    }
}
