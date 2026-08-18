
use crate::types::PgError;
use sql_core::SqlValue;

pub fn binary(op: &str, l: &SqlValue, r: &SqlValue) -> Option<Result<SqlValue, PgError>> {

    match op {
        "@?" | "@@" if matches!(l, SqlValue::Null) || matches!(r, SqlValue::Null) => {
            return Some(Ok(SqlValue::Null));
        }
        _ => {}
    }
    match op {
        "@?" if crate::expr::functions::jsonpath::is_json_target(l) => {
            return Some(crate::expr::functions::jsonpath::op_exists(l, r))
        }

        "@@" if crate::expr::functions::jsonpath::is_json_target(l)
            && crate::expr::functions::jsonpath::looks_like_jsonpath(r) =>
        {
            return Some(crate::expr::functions::jsonpath::op_match(l, r))
        }
        _ => {}
    }
    match op {
        "->" | "->>" | "#>" | "#>>" => {}
        _ => return None,
    }

    if matches!(l, SqlValue::Null) {
        return Some(Ok(SqlValue::Null));
    }

    let root = match l {
        SqlValue::Text(s) => match parse_json(s) {
            Some(j) => j,
            None => return Some(Ok(SqlValue::Null)),
        },
        _ => return Some(Ok(SqlValue::Null)),
    };

    let selected: Option<Json> = match op {
        "->" | "->>" => match r {
            SqlValue::Null => return Some(Ok(SqlValue::Null)),
            SqlValue::Int(i) => index_get(&root, *i),
            SqlValue::Text(k) => key_get(&root, k),

            _ => None,
        },

        _ => match r {
            SqlValue::Null => return Some(Ok(SqlValue::Null)),
            SqlValue::Text(lit) => match parse_text_path(lit) {

                Some(path) => {
                    if path.iter().any(|e| e.is_none()) {
                        return Some(Ok(SqlValue::Null));
                    }
                    path_get(&root, &path)
                }
                None => return Some(Ok(SqlValue::Null)),
            },
            _ => return Some(Ok(SqlValue::Null)),
        },
    };

    let node = match selected {
        Some(n) => n,
        None => return Some(Ok(SqlValue::Null)),
    };

    let text_variant = op == "->>" || op == "#>>";
    if text_variant {

        match text_of(&node) {
            Some(t) => Some(Ok(SqlValue::Text(t))),
            None => Some(Ok(SqlValue::Null)),
        }
    } else {

        let mut out = String::new();
        serialize(&node, &mut out);
        Some(Ok(SqlValue::Text(out)))
    }
}

pub fn unary(op: &str, v: &SqlValue) -> Option<Result<SqlValue, PgError>> {
    let _ = (op, v);
    None
}

#[derive(Clone)]
enum Json {
    Null,
    Bool(bool),
    Num(String),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

fn text_of(j: &Json) -> Option<String> {
    match j {
        Json::Null => None,
        Json::Str(s) => Some(s.clone()),
        other => {
            let mut out = String::new();
            serialize(other, &mut out);
            Some(out)
        }
    }
}

fn index_get(root: &Json, i: i64) -> Option<Json> {
    if let Json::Arr(items) = root {
        let len = items.len() as i64;
        let idx = if i < 0 { len + i } else { i };
        if idx >= 0 && idx < len {
            return Some(items[idx as usize].clone());
        }
    }
    None
}

fn key_get(root: &Json, key: &str) -> Option<Json> {
    if let Json::Obj(members) = root {
        for (k, v) in members {
            if k == key {
                return Some(v.clone());
            }
        }
    }
    None
}

fn path_get(root: &Json, path: &[Option<String>]) -> Option<Json> {
    let mut cur = root.clone();
    for step in path {
        let key = step.as_ref()?;
        cur = match &cur {
            Json::Obj(_) => key_get(&cur, key)?,
            Json::Arr(_) => {
                let i: i64 = key.parse().ok()?;
                index_get(&cur, i)?
            }

            _ => return None,
        };
    }
    Some(cur)
}

fn parse_json(text: &str) -> Option<Json> {
    let mut p = JParser {
        b: text.as_bytes(),
        pos: 0,
    };
    p.skip_ws();
    let v = p.value()?;
    p.skip_ws();
    if p.pos != p.b.len() {
        return None;
    }
    Some(v)
}

struct JParser<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> JParser<'a> {
    fn peek(&self) -> Option<u8> {
        self.b.get(self.pos).copied()
    }
    fn bump(&mut self) -> Option<u8> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }
    fn skip_ws(&mut self) {
        while matches!(
            self.peek(),
            Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r')
        ) {
            self.pos += 1;
        }
    }
    fn value(&mut self) -> Option<Json> {
        match self.peek()? {
            b'{' => self.object(),
            b'[' => self.array(),
            b'"' => Some(Json::Str(self.string()?)),
            b't' => {
                self.literal(b"true")?;
                Some(Json::Bool(true))
            }
            b'f' => {
                self.literal(b"false")?;
                Some(Json::Bool(false))
            }
            b'n' => {
                self.literal(b"null")?;
                Some(Json::Null)
            }
            b'-' | b'0'..=b'9' => self.number(),
            _ => None,
        }
    }
    fn literal(&mut self, word: &[u8]) -> Option<()> {
        if self.b[self.pos..].starts_with(word) {
            self.pos += word.len();
            Some(())
        } else {
            None
        }
    }
    fn object(&mut self) -> Option<Json> {
        self.bump();
        let mut members: Vec<(String, Json)> = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.bump();
            return Some(Json::Obj(members));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return None;
            }
            let key = self.string()?;
            self.skip_ws();
            if self.bump() != Some(b':') {
                return None;
            }
            self.skip_ws();
            let val = self.value()?;
            members.push((key, val));
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b'}') => break,
                _ => return None,
            }
        }
        Some(Json::Obj(members))
    }
    fn array(&mut self) -> Option<Json> {
        self.bump();
        let mut items: Vec<Json> = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.bump();
            return Some(Json::Arr(items));
        }
        loop {
            self.skip_ws();
            items.push(self.value()?);
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b']') => break,
                _ => return None,
            }
        }
        Some(Json::Arr(items))
    }
    fn string(&mut self) -> Option<String> {
        self.bump();
        let mut buf: Vec<u8> = Vec::new();
        loop {
            match self.bump()? {
                b'"' => break,
                b'\\' => match self.bump()? {
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
                                return None;
                            }
                            let lo = self.hex4()?;
                            if !(0xDC00..=0xDFFF).contains(&lo) {
                                return None;
                            }
                            0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00)
                        } else if (0xDC00..=0xDFFF).contains(&cp) {
                            return None;
                        } else {
                            cp
                        };
                        let ch = char::from_u32(scalar)?;
                        let mut tmp = [0u8; 4];
                        buf.extend_from_slice(ch.encode_utf8(&mut tmp).as_bytes());
                    }
                    _ => return None,
                },
                c if c < 0x20 => return None,
                c => buf.push(c),
            }
        }
        String::from_utf8(buf).ok()
    }
    fn hex4(&mut self) -> Option<u32> {
        let mut v: u32 = 0;
        for _ in 0..4 {
            let h = self.bump()?;
            let d = match h {
                b'0'..=b'9' => (h - b'0') as u32,
                b'a'..=b'f' => (h - b'a' + 10) as u32,
                b'A'..=b'F' => (h - b'A' + 10) as u32,
                _ => return None,
            };
            v = v * 16 + d;
        }
        Some(v)
    }
    fn number(&mut self) -> Option<Json> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.bump();
        }
        match self.peek()? {
            b'0' => {
                self.bump();
            }
            b'1'..=b'9' => {
                self.bump();
                self.skip_digits();
            }
            _ => return None,
        }
        if self.peek() == Some(b'.') {
            self.bump();
            if !self.at_digit() {
                return None;
            }
            self.skip_digits();
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            self.bump();
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.bump();
            }
            if !self.at_digit() {
                return None;
            }
            self.skip_digits();
        }
        let tok = core::str::from_utf8(&self.b[start..self.pos]).ok()?;
        Some(Json::Num(tok.to_string()))
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

fn parse_text_path(lit: &str) -> Option<Vec<Option<String>>> {
    let chars: Vec<char> = lit.chars().collect();
    let mut p = APaser {
        chars: &chars,
        pos: 0,
    };
    p.skip_ws();
    if p.peek() != Some('{') {
        return None;
    }
    p.pos += 1;
    let mut out: Vec<Option<String>> = Vec::new();
    p.skip_ws();
    if p.peek() == Some('}') {
        p.pos += 1;
        p.skip_ws();
        return if p.pos == chars.len() {
            Some(out)
        } else {
            None
        };
    }
    loop {
        p.skip_ws();

        if p.peek() == Some('{') {
            return None;
        }
        out.push(p.element()?);
        p.skip_ws();
        match p.peek() {
            Some(',') => p.pos += 1,
            Some('}') => {
                p.pos += 1;
                break;
            }
            _ => return None,
        }
    }
    p.skip_ws();
    if p.pos == chars.len() {
        Some(out)
    } else {
        None
    }
}

struct APaser<'a> {
    chars: &'a [char],
    pos: usize,
}

impl<'a> APaser<'a> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }
    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(c) if matches!(c, ' ' | '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r'))
        {
            self.pos += 1;
        }
    }
    fn element(&mut self) -> Option<Option<String>> {
        if self.peek() == Some('"') {
            self.pos += 1;
            let mut buf = String::new();
            loop {
                match self.peek() {
                    None => return None,
                    Some('\\') => {
                        self.pos += 1;
                        let c = self.peek()?;
                        buf.push(c);
                        self.pos += 1;
                    }
                    Some('"') => {
                        self.pos += 1;
                        break;
                    }
                    Some(c) => {
                        buf.push(c);
                        self.pos += 1;
                    }
                }
            }
            Some(Some(buf))
        } else {
            let start = self.pos;
            let mut buf = String::new();
            let mut sig = 0usize;
            let mut escaped_any = false;
            loop {
                match self.peek() {
                    None => return None,
                    Some('\\') => {
                        self.pos += 1;
                        let c = self.peek()?;
                        buf.push(c);
                        self.pos += 1;
                        sig = buf.len();
                        escaped_any = true;
                    }
                    Some('"') => return None,
                    Some(',') | Some('}') | Some('{') => break,
                    Some(c) if matches!(c, ' ' | '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r') => {
                        buf.push(c);
                        self.pos += 1;
                    }
                    Some(c) => {
                        buf.push(c);
                        self.pos += 1;
                        sig = buf.len();
                    }
                }
            }
            if self.pos == start {
                return None;
            }
            buf.truncate(sig);
            if !escaped_any && buf.eq_ignore_ascii_case("NULL") {
                Some(None)
            } else {
                Some(Some(buf))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::binary;
    use sql_core::SqlValue;

    fn txt(s: &str) -> SqlValue {
        SqlValue::Text(s.to_string())
    }

    fn t(op: &str, l: &str, r: SqlValue, want: &str) {
        match binary(op, &txt(l), &r) {
            Some(Ok(SqlValue::Text(got))) => assert_eq!(got, want, "{l} {op} {r:?}"),
            other => panic!("{l} {op} {r:?} => {other:?}, wanted Text({want:?})"),
        }
    }

    fn n(op: &str, l: &str, r: SqlValue) {
        match binary(op, &txt(l), &r) {
            Some(Ok(SqlValue::Null)) => {}
            other => panic!("{l} {op} {r:?} => {other:?}, wanted Null"),
        }
    }

    #[test]
    fn arrow_object_key() {

        t("->", r#"{"a": 1}"#, txt("a"), "1");
        t("->", r#"{"a": {"b": 1}}"#, txt("a"), r#"{"b": 1}"#);

        n("->", r#"{"a": 1}"#, txt("z"));

        n("->", "[1, 2, 3]", txt("a"));

        t("->", r#"{"a": "c", "b": null}"#, txt("b"), "null");
    }

    #[test]
    fn arrow_array_index() {

        t("->", "[10, 20]", SqlValue::Int(1), "20");

        t("->", "[10, 20, 30]", SqlValue::Int(-1), "30");
        t("->", "[10, 20, 30]", SqlValue::Int(-3), "10");

        n("->", "[10, 20]", SqlValue::Int(5));
        n("->", "[10, 20]", SqlValue::Int(-3));

        n("->", r#"{"a": 1}"#, SqlValue::Int(0));
    }

    #[test]
    fn double_arrow_returns_unquoted_text() {

        t("->>", r#"{"a": "x"}"#, txt("a"), "x");

        t("->>", r#"{"a": 1}"#, txt("a"), "1");

        t(
            "->>",
            r#"[{"b": "c"}, {"b": "cc"}]"#,
            SqlValue::Int(1),
            r#"{"b": "cc"}"#,
        );

        n("->>", r#"{"a": "c", "b": null}"#, txt("b"));

        n("->>", r#"{"a": "c"}"#, txt("z"));
    }

    #[test]
    fn hash_arrow_path_json() {

        t(
            "#>",
            r#"{"a": {"b": {"c": "foo"}}}"#,
            txt("{a,b}"),
            r#"{"c": "foo"}"#,
        );

        t(
            "#>",
            r#"{"a": [{"b": "c"}, {"b": "cc"}]}"#,
            txt("{a,1,b}"),
            r#""cc""#,
        );

        t("#>", "[1, 2, 3]", txt("{}"), "[1, 2, 3]");

        n("#>", r#"[{"b": "c"}]"#, txt("{z,b}"));

        n("#>", r#"{"a": {"b": 1}}"#, txt("{a,z}"));
    }

    #[test]
    fn hash_double_arrow_path_text() {

        t(
            "#>>",
            r#"{"a": {"b": {"c": "foo"}}}"#,
            txt("{a,b,c}"),
            "foo",
        );

        t("#>>", r#"{"f2": {"f3": 1}}"#, txt("{f2}"), r#"{"f3": 1}"#);

        t("#>>", "42", txt("{}"), "42");

        n("#>>", "null", txt("{}"));
    }

    #[test]
    fn strict_null_propagation() {

        n("->", "", SqlValue::Null);
        match binary("->", &SqlValue::Null, &txt("a")) {
            Some(Ok(SqlValue::Null)) => {}
            other => panic!("NULL left => {other:?}"),
        }

        n("->>", r#"{"a": 1}"#, SqlValue::Null);
        n("#>", r#"{"a": 1}"#, SqlValue::Null);

        n("#>", r#"{"a": {"b": 1}}"#, txt("{a,NULL}"));
    }

    #[test]
    fn jsonpath_ops_strict_null_and_routing() {

        for op in ["@?", "@@"] {
            match binary(op, &SqlValue::Null, &txt("$.a")) {
                Some(Ok(SqlValue::Null)) => {}
                other => panic!("{op} NULL-left => {other:?}"),
            }
            match binary(op, &txt(r#"{"a": 1}"#), &SqlValue::Null) {
                Some(Ok(SqlValue::Null)) => {}
                other => panic!("{op} NULL-right => {other:?}"),
            }
        }

        assert!(binary("@@", &txt("5"), &txt("cat")).is_none());
        assert!(binary("@@", &txt("true"), &txt("cat")).is_none());
        assert!(binary("@@", &txt("[1, 2, 3]"), &txt("fat & cat")).is_none());

        match binary("@@", &txt(r#"{"a": 1}"#), &txt("$.a == 1")) {
            Some(Ok(SqlValue::Int(1))) => {}
            other => panic!("jsonb @@ jsonpath => {other:?}"),
        }

        match binary("@?", &txt(r#"{"a": 1}"#), &txt("$.a")) {
            Some(Ok(SqlValue::Int(1))) => {}
            other => panic!("jsonb @? path => {other:?}"),
        }
    }

    #[test]
    fn unclaimed_op_falls_through() {

        assert!(binary("@>", &txt(r#"{"a": 1}"#), &txt(r#"{"a": 1}"#)).is_none());
        assert!(binary("+", &SqlValue::Int(1), &SqlValue::Int(2)).is_none());
    }

    #[test]
    fn unary_is_none() {
        assert!(super::unary("->", &txt(r#"{"a": 1}"#)).is_none());
        assert!(super::unary("#>", &SqlValue::Null).is_none());
    }
}
