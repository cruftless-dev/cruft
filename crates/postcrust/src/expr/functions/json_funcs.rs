
use crate::types::PgError;
use sql_core::SqlValue;

fn does_not_exist(name: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("function {name}(...) does not exist"),
    }
}

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    match name {
        "jsonb_typeof" | "json_typeof" => Some(typeof_fn(name, args)),
        "jsonb_array_length" | "json_array_length" => Some(array_length_fn(name, args)),
        "jsonb_pretty" => Some(pretty_fn(name, args)),
        "jsonb_strip_nulls" => Some(strip_nulls_fn(name, args)),
        _ => None,
    }
}

fn single_text<'a>(name: &str, args: &'a [SqlValue]) -> Result<Option<&'a str>, PgError> {
    if args.len() != 1 {
        return Err(does_not_exist(name));
    }
    match &args[0] {
        SqlValue::Null => Ok(None),
        SqlValue::Text(s) => Ok(Some(s.as_str())),
        _ => Err(does_not_exist(name)),
    }
}

fn typeof_fn(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    let text = match single_text(name, args)? {
        None => return Ok(SqlValue::Null),
        Some(s) => s,
    };
    let v = parse(text).map_err(|()| does_not_exist(name))?;
    let ty = match v {
        Json::Null => "null",
        Json::Bool(_) => "boolean",
        Json::Num(_) => "number",
        Json::Str(_) => "string",
        Json::Arr(_) => "array",
        Json::Obj(_) => "object",
    };
    Ok(SqlValue::Text(ty.to_string()))
}

fn array_length_fn(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    let text = match single_text(name, args)? {
        None => return Ok(SqlValue::Null),
        Some(s) => s,
    };
    let v = parse(text).map_err(|()| does_not_exist(name))?;
    match v {
        Json::Arr(items) => Ok(SqlValue::Int(items.len() as i64)),

        _ => Err(does_not_exist(name)),
    }
}

fn pretty_fn(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    let text = match single_text(name, args)? {
        None => return Ok(SqlValue::Null),
        Some(s) => s,
    };
    let v = parse(text).map_err(|()| does_not_exist(name))?;
    let mut out = String::new();
    pretty(&v, 0, &mut out);
    Ok(SqlValue::Text(out))
}

fn strip_nulls_fn(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    let text = match single_text(name, args)? {
        None => return Ok(SqlValue::Null),
        Some(s) => s,
    };
    let v = parse(text).map_err(|()| does_not_exist(name))?;
    let stripped = strip_nulls(v);
    let mut out = String::new();
    serialize(&stripped, &mut out);
    Ok(SqlValue::Text(out))
}

enum Json {
    Null,
    Bool(bool),

    Num(String),

    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

fn parse(text: &str) -> Result<Json, ()> {
    let mut p = Parser {
        bytes: text.as_bytes(),
        pos: 0,
    };
    p.skip_ws();
    let v = p.value()?;
    p.skip_ws();
    if p.pos != p.bytes.len() {
        return Err(());
    }
    Ok(v)
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let b = self.peek();
        if b.is_some() {
            self.pos += 1;
        }
        b
    }

    fn skip_ws(&mut self) {
        while let Some(b) = self.peek() {
            match b {
                b' ' | b'\t' | b'\n' | b'\r' => self.pos += 1,
                _ => break,
            }
        }
    }

    fn value(&mut self) -> Result<Json, ()> {
        match self.peek().ok_or(())? {
            b'{' => self.object(),
            b'[' => self.array(),
            b'"' => Ok(Json::Str(self.string()?)),
            b't' => {
                self.literal(b"true")?;
                Ok(Json::Bool(true))
            }
            b'f' => {
                self.literal(b"false")?;
                Ok(Json::Bool(false))
            }
            b'n' => {
                self.literal(b"null")?;
                Ok(Json::Null)
            }
            b'-' | b'0'..=b'9' => self.number(),
            _ => Err(()),
        }
    }

    fn literal(&mut self, word: &[u8]) -> Result<(), ()> {
        if self.bytes[self.pos..].starts_with(word) {
            self.pos += word.len();
            Ok(())
        } else {
            Err(())
        }
    }

    fn object(&mut self) -> Result<Json, ()> {
        self.bump();
        let mut members: Vec<(String, Json)> = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.bump();
            return Ok(Json::Obj(members));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(());
            }
            let key = self.string()?;
            self.skip_ws();
            if self.bump() != Some(b':') {
                return Err(());
            }
            self.skip_ws();
            let val = self.value()?;
            members.push((key, val));
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b'}') => break,
                _ => return Err(()),
            }
        }
        Ok(Json::Obj(normalize_members(members)))
    }

    fn array(&mut self) -> Result<Json, ()> {
        self.bump();
        let mut items: Vec<Json> = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.bump();
            return Ok(Json::Arr(items));
        }
        loop {
            self.skip_ws();
            items.push(self.value()?);
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b']') => break,
                _ => return Err(()),
            }
        }
        Ok(Json::Arr(items))
    }

    fn string(&mut self) -> Result<String, ()> {
        self.bump();
        let mut buf: Vec<u8> = Vec::new();
        loop {
            match self.bump().ok_or(())? {
                b'"' => break,
                b'\\' => match self.bump().ok_or(())? {
                    b'"' => buf.push(b'"'),
                    b'\\' => buf.push(b'\\'),
                    b'/' => buf.push(b'/'),
                    b'b' => buf.push(0x08),
                    b'f' => buf.push(0x0c),
                    b'n' => buf.push(b'\n'),
                    b'r' => buf.push(b'\r'),
                    b't' => buf.push(b'\t'),
                    b'u' => {
                        let cp = self.hex4()?;
                        let scalar = if (0xD800..=0xDBFF).contains(&cp) {
                            if self.bump() != Some(b'\\') || self.bump() != Some(b'u') {
                                return Err(());
                            }
                            let lo = self.hex4()?;
                            if !(0xDC00..=0xDFFF).contains(&lo) {
                                return Err(());
                            }
                            0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00)
                        } else if (0xDC00..=0xDFFF).contains(&cp) {
                            return Err(());
                        } else {
                            cp
                        };
                        if scalar == 0 {
                            return Err(());
                        }
                        let ch = char::from_u32(scalar).ok_or(())?;
                        let mut tmp = [0u8; 4];
                        buf.extend_from_slice(ch.encode_utf8(&mut tmp).as_bytes());
                    }
                    _ => return Err(()),
                },
                c if c < 0x20 => return Err(()),
                c => buf.push(c),
            }
        }
        String::from_utf8(buf).map_err(|_| ())
    }

    fn hex4(&mut self) -> Result<u32, ()> {
        let mut v: u32 = 0;
        for _ in 0..4 {
            let h = self.bump().ok_or(())?;
            let d = match h {
                b'0'..=b'9' => (h - b'0') as u32,
                b'a'..=b'f' => (h - b'a' + 10) as u32,
                b'A'..=b'F' => (h - b'A' + 10) as u32,
                _ => return Err(()),
            };
            v = v * 16 + d;
        }
        Ok(v)
    }

    fn number(&mut self) -> Result<Json, ()> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.bump();
        }
        match self.peek().ok_or(())? {
            b'0' => {
                self.bump();
            }
            b'1'..=b'9' => {
                self.bump();
                self.skip_digits();
            }
            _ => return Err(()),
        }
        if self.peek() == Some(b'.') {
            self.bump();
            if !self.at_digit() {
                return Err(());
            }
            self.skip_digits();
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            self.bump();
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.bump();
            }
            if !self.at_digit() {
                return Err(());
            }
            self.skip_digits();
        }
        let tok = core::str::from_utf8(&self.bytes[start..self.pos]).map_err(|_| ())?;
        Ok(Json::Num(tok.to_string()))
    }

    fn at_digit(&self) -> bool {
        matches!(self.peek(), Some(b'0'..=b'9'))
    }

    fn skip_digits(&mut self) {
        while self.at_digit() {
            self.pos += 1;
        }
    }
}

fn normalize_members(mut members: Vec<(String, Json)>) -> Vec<(String, Json)> {
    members.sort_by(|a, b| {
        a.0.len()
            .cmp(&b.0.len())
            .then_with(|| a.0.as_bytes().cmp(b.0.as_bytes()))
    });
    let mut out: Vec<(String, Json)> = Vec::with_capacity(members.len());
    for (k, v) in members {
        if let Some(last) = out.last_mut() {
            if last.0 == k {
                last.1 = v;
                continue;
            }
        }
        out.push((k, v));
    }
    out
}

fn strip_nulls(v: Json) -> Json {
    match v {
        Json::Arr(items) => Json::Arr(items.into_iter().map(strip_nulls).collect()),
        Json::Obj(members) => Json::Obj(
            members
                .into_iter()
                .filter(|(_, val)| !matches!(val, Json::Null))
                .map(|(k, val)| (k, strip_nulls(val)))
                .collect(),
        ),
        other => other,
    }
}

fn serialize(v: &Json, out: &mut String) {
    match v {
        Json::Null => out.push_str("null"),
        Json::Bool(true) => out.push_str("true"),
        Json::Bool(false) => out.push_str("false"),
        Json::Num(n) => out.push_str(n),
        Json::Str(s) => serialize_string(s, out),
        Json::Arr(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                serialize(item, out);
            }
            out.push(']');
        }
        Json::Obj(members) => {
            out.push('{');
            for (i, (k, val)) in members.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                serialize_string(k, out);
                out.push_str(": ");
                serialize(val, out);
            }
            out.push('}');
        }
    }
}

fn pretty(v: &Json, indent: usize, out: &mut String) {
    match v {
        Json::Arr(items) if !items.is_empty() => {
            out.push_str("[\n");
            let inner = indent + 4;
            for (i, item) in items.iter().enumerate() {
                push_spaces(out, inner);
                pretty(item, inner, out);
                if i + 1 < items.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            push_spaces(out, indent);
            out.push(']');
        }
        Json::Obj(members) if !members.is_empty() => {
            out.push_str("{\n");
            let inner = indent + 4;
            for (i, (k, val)) in members.iter().enumerate() {
                push_spaces(out, inner);
                serialize_string(k, out);
                out.push_str(": ");
                pretty(val, inner, out);
                if i + 1 < members.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            push_spaces(out, indent);
            out.push('}');
        }

        _ => serialize(v, out),
    }
}

fn push_spaces(out: &mut String, n: usize) {
    for _ in 0..n {
        out.push(' ');
    }
}

fn serialize_string(s: &str, out: &mut String) {
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
}

#[cfg(test)]
mod tests {
    use super::call;
    use sql_core::SqlValue;

    fn t(s: &str) -> SqlValue {
        SqlValue::Text(s.to_string())
    }

    fn ok(name: &str, arg: &str) -> SqlValue {
        call(name, &[t(arg)])
            .expect("family should claim name")
            .expect("should succeed")
    }

    fn text_of(v: SqlValue) -> String {
        match v {
            SqlValue::Text(s) => s,
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn jsonb_typeof_each_kind() {

        assert_eq!(text_of(ok("jsonb_typeof", "{}")), "object");
        assert_eq!(
            text_of(ok("jsonb_typeof", r#"{"c": 3, "p": "o"}"#)),
            "object"
        );
        assert_eq!(text_of(ok("jsonb_typeof", "[]")), "array");
        assert_eq!(text_of(ok("jsonb_typeof", r#"["a", 1]"#)), "array");
        assert_eq!(text_of(ok("jsonb_typeof", "null")), "null");
        assert_eq!(text_of(ok("jsonb_typeof", "1")), "number");
        assert_eq!(text_of(ok("jsonb_typeof", "-1")), "number");
        assert_eq!(text_of(ok("jsonb_typeof", "1.0")), "number");
        assert_eq!(text_of(ok("jsonb_typeof", "true")), "boolean");
        assert_eq!(text_of(ok("jsonb_typeof", "false")), "boolean");
        assert_eq!(text_of(ok("jsonb_typeof", r#""hello""#)), "string");

        assert_eq!(text_of(ok("jsonb_typeof", r#""true""#)), "string");
        assert_eq!(text_of(ok("jsonb_typeof", r#""1.0""#)), "string");
    }

    #[test]
    fn json_typeof_matches_jsonb() {

        assert_eq!(text_of(ok("json_typeof", "[1, 2]")), "array");
        assert_eq!(text_of(ok("json_typeof", "null")), "null");
        assert_eq!(text_of(ok("json_typeof", r#"{"a":1}"#)), "object");
        assert_eq!(text_of(ok("json_typeof", "123")), "number");
    }

    #[test]
    fn json_null_arg_distinct_from_sql_null() {

        assert_eq!(text_of(ok("jsonb_typeof", "null")), "null");
        let sql_null = call("jsonb_typeof", &[SqlValue::Null]).unwrap().unwrap();
        assert_eq!(sql_null, SqlValue::Null);
    }

    #[test]
    fn array_length_array() {

        assert_eq!(
            ok(
                "jsonb_array_length",
                r#"[1, 2, 3, {"f1": 1, "f2": [5, 6]}, 4]"#
            ),
            SqlValue::Int(5)
        );
        assert_eq!(ok("jsonb_array_length", "[]"), SqlValue::Int(0));
        assert_eq!(
            ok("json_array_length", "[1,2,3,{\"f1\":1,\"f2\":[5,6]},4]"),
            SqlValue::Int(5)
        );
        assert_eq!(ok("json_array_length", "[]"), SqlValue::Int(0));
    }

    #[test]
    fn array_length_non_array_errors() {

        assert!(
            call("jsonb_array_length", &[t(r#"{"f1": 1, "f2": [5, 6]}"#)])
                .unwrap()
                .is_err()
        );
        assert!(call("jsonb_array_length", &[t("4")]).unwrap().is_err());
        assert!(call("json_array_length", &[t(r#"{"f1":1}"#)])
            .unwrap()
            .is_err());
    }

    #[test]
    fn pretty_object_and_nested() {

        let got = text_of(ok(
            "jsonb_pretty",
            r#"{"a": "test", "b": [1, 2, 3], "c": "test3", "d": {"dd": "test4", "dd2": {"ddd": "test5"}}}"#,
        ));
        let want = "{\n    \"a\": \"test\",\n    \"b\": [\n        1,\n        2,\n        3\n    ],\n    \"c\": \"test3\",\n    \"d\": {\n        \"dd\": \"test4\",\n        \"dd2\": {\n            \"ddd\": \"test5\"\n        }\n    }\n}";
        assert_eq!(got, want);
    }

    #[test]
    fn pretty_array_of_containers() {

        let got = text_of(ok(
            "jsonb_pretty",
            r#"[{"f1": 1, "f2": null}, 2, null, [[{"x": true}, 6, 7], 8], 3]"#,
        ));
        let want = "[\n    {\n        \"f1\": 1,\n        \"f2\": null\n    },\n    2,\n    null,\n    [\n        [\n            {\n                \"x\": true\n            },\n            6,\n            7\n        ],\n        8\n    ],\n    3\n]";
        assert_eq!(got, want);
    }

    #[test]
    fn strip_nulls_recursive_object_fields() {

        assert_eq!(
            text_of(ok(
                "jsonb_strip_nulls",
                r#"{"a": 1, "b": null, "c": [2, null, 3], "d": {"e": 4, "f": null}}"#
            )),
            r#"{"a": 1, "c": [2, null, 3], "d": {"e": 4}}"#,
        );

        assert_eq!(
            text_of(ok(
                "jsonb_strip_nulls",
                r#"[1, {"a": 1, "b": null, "c": 2}, 3]"#
            )),
            r#"[1, {"a": 1, "c": 2}, 3]"#,
        );

        assert_eq!(
            text_of(ok(
                "jsonb_strip_nulls",
                r#"{"a": {"b": null, "c": null}, "d": {}}"#
            )),
            r#"{"a": {}, "d": {}}"#,
        );
    }

    #[test]
    fn strip_nulls_scalars_and_arrays_passthrough() {

        assert_eq!(text_of(ok("jsonb_strip_nulls", "1")), "1");
        assert_eq!(
            text_of(ok("jsonb_strip_nulls", r#""a string""#)),
            r#""a string""#
        );
        assert_eq!(text_of(ok("jsonb_strip_nulls", "null")), "null");
        assert_eq!(
            text_of(ok("jsonb_strip_nulls", "[1, 2, null, 3, 4]")),
            "[1, 2, null, 3, 4]"
        );
    }

    #[test]
    fn sql_null_propagates_for_all() {
        for name in [
            "jsonb_typeof",
            "json_typeof",
            "jsonb_array_length",
            "json_array_length",
            "jsonb_pretty",
            "jsonb_strip_nulls",
        ] {
            let got = call(name, &[SqlValue::Null]).expect("claimed").expect("ok");
            assert_eq!(got, SqlValue::Null, "{name} should propagate SQL NULL");
        }
    }

    #[test]
    fn unclaimed_name_is_none() {
        assert!(call("jsonb_object_keys", &[t("{}")]).is_none());
        assert!(call("lpad", &[t("x")]).is_none());
        assert!(call("json_pretty", &[t("{}")]).is_none());
    }

    #[test]
    fn wrong_arity_and_type_error() {

        assert!(call("jsonb_typeof", &[]).unwrap().is_err());
        assert!(call("jsonb_typeof", &[t("1"), t("2")]).unwrap().is_err());

        assert!(call("jsonb_typeof", &[SqlValue::Int(1)]).unwrap().is_err());
    }
}
