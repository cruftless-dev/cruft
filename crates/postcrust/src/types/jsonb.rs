
use super::PgError;
use sql_core::SqlValue;

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    match parse_normalize(text) {
        Ok(normalized) => Ok(SqlValue::Text(normalized)),
        Err(()) => Err(PgError::InvalidInputSyntax {
            typ: super::type_name(oid),
            input: text.to_string(),
        }),
    }
}

pub fn output(_oid: u32, v: &SqlValue) -> String {
    match v {
        SqlValue::Text(s) => s.clone(),
        _ => String::new(),
    }
}

fn parse_normalize(text: &str) -> Result<String, ()> {
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
    let mut out = String::new();
    serialize(&v, &mut out);
    Ok(out)
}

pub(crate) fn order_key(canonical: &str) -> SqlValue {
    let mut p = Parser {
        bytes: canonical.as_bytes(),
        pos: 0,
    };
    p.skip_ws();
    let v = match p.value() {
        Ok(v) => v,

        Err(()) => return SqlValue::Text(canonical.to_string()),
    };
    let mut out: Vec<u8> = Vec::new();
    encode_order(&v, &mut out);

    SqlValue::Text(String::from_utf8(out).unwrap_or_else(|_| canonical.to_string()))
}

fn push_count(n: usize, out: &mut Vec<u8>) {
    match crate::types::numeric::sort_key(&SqlValue::Int(n as i64)) {
        SqlValue::Text(k) => out.extend_from_slice(k.as_bytes()),
        _ => out.extend_from_slice(n.to_string().as_bytes()),
    }
    out.push(0x00);
}

fn push_str_key(s: &str, out: &mut Vec<u8>) {
    out.extend_from_slice(s.as_bytes());
    out.push(0x00);
}

fn encode_order(j: &Json, out: &mut Vec<u8>) {
    match j {
        Json::Null => out.push(0),
        Json::Str(s) => {
            out.push(1);
            push_str_key(s, out);
        }
        Json::Num(s) => {
            out.push(2);
            match crate::types::numeric::sort_key(&SqlValue::Text(s.clone())) {
                SqlValue::Text(k) => out.extend_from_slice(k.as_bytes()),
                _ => out.extend_from_slice(s.as_bytes()),
            }
            out.push(0x00);
        }
        Json::Bool(b) => {
            out.push(3);
            out.push(if *b { 1 } else { 0 });
        }
        Json::Arr(items) => {
            out.push(4);
            push_count(items.len(), out);
            for it in items {
                encode_order(it, out);
            }
        }
        Json::Obj(members) => {
            out.push(5);
            push_count(members.len(), out);

            for (k, val) in members {
                push_str_key(k, out);
                encode_order(val, out);
            }
        }
    }
}

const NUM_LIMIT: i64 = 131072;

enum Json {
    Null,
    Bool(bool),

    Num(String),

    Str(String),
    Arr(Vec<Json>),

    Obj(Vec<(String, Json)>),
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
        canonicalize_number(tok).map(Json::Num)
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

fn canonicalize_number(tok: &str) -> Result<String, ()> {
    let b = tok.as_bytes();
    let mut i = 0usize;
    let neg = b[0] == b'-';
    if neg {
        i = 1;
    }

    let int_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let int_digits = &tok[int_start..i];

    let mut frac_digits = "";
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let fs = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        frac_digits = &tok[fs..i];
    }

    let mut exp: i64 = 0;
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        let mut eneg = false;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            eneg = b[i] == b'-';
            i += 1;
        }
        let ds = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        let mag: i64 = tok[ds..i].parse().map_err(|_| ())?;
        exp = if eneg { -mag } else { mag };
    }

    let all: String = format!("{int_digits}{frac_digits}");
    let l = all.len() as i64;

    let pointpos = int_digits.len() as i64 + exp;

    let dscale = (frac_digits.len() as i64 - exp).max(0);

    if pointpos > NUM_LIMIT || dscale > NUM_LIMIT {
        return Err(());
    }

    let (int_part, frac_part): (String, String) = if pointpos <= 0 {
        let zeros = "0".repeat((-pointpos) as usize);
        ("0".to_string(), format!("{zeros}{all}"))
    } else if pointpos >= l {
        let zeros = "0".repeat((pointpos - l) as usize);
        (format!("{all}{zeros}"), String::new())
    } else {
        let p = pointpos as usize;
        (all[..p].to_string(), all[p..].to_string())
    };

    let trimmed = int_part.trim_start_matches('0');
    let int_out = if trimmed.is_empty() { "0" } else { trimmed };

    let is_zero = int_out == "0" && frac_part.bytes().all(|c| c == b'0');

    let mut s = String::new();
    if neg && !is_zero {
        s.push('-');
    }
    s.push_str(int_out);
    if !frac_part.is_empty() {
        s.push('.');
        s.push_str(&frac_part);
    }
    Ok(s)
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
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                serialize(item, out);
            }
            out.push(']');
        }
        Json::Obj(members) => {
            out.push('{');
            for (idx, (k, val)) in members.iter().enumerate() {
                if idx > 0 {
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
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::super::oid::JSONB;
    use super::*;

    fn norm(s: &str) -> String {
        match input(JSONB, s) {
            Ok(SqlValue::Text(t)) => t,
            Ok(other) => panic!("{s:?} expected Text, got {other:?}"),
            Err(e) => panic!("{s:?} should be accepted: {}", e.message()),
        }
    }

    fn norm_eq(s: &str, want: &str) {
        let got = norm(s);
        assert_eq!(got, want, "normalize({s:?})");
        assert_eq!(
            output(JSONB, &SqlValue::Text(got.clone())),
            want,
            "output({s:?})"
        );
    }

    fn is_err(s: &str) {
        match input(JSONB, s) {
            Ok(v) => panic!("{s:?} should be rejected, got {v:?}"),
            Err(e) => assert!(
                matches!(e, PgError::InvalidInputSyntax { typ: "jsonb", .. }),
                "{s:?} should be InvalidInputSyntax(jsonb), got {e:?}"
            ),
        }
    }

    #[test]
    fn key_sorting_length_then_bytewise() {

        norm_eq(
            r#"{"aa":1,"b":2,"cq":3,"fg":false}"#,
            r#"{"b": 2, "aa": 1, "cq": 3, "fg": false}"#,
        );

        norm_eq(
            r#"{"relkind":"r","name":"pg_class"}"#,
            r#"{"name": "pg_class", "relkind": "r"}"#,
        );
    }

    #[test]
    fn duplicate_key_last_wins() {

        norm_eq(
            r#"{"d2":"d2","d1":"d1","d1":"d3"}"#,
            r#"{"d1": "d3", "d2": "d2"}"#,
        );
        norm_eq(r#"{"a":1,"a":2,"a":3}"#, r#"{"a": 3}"#);
    }

    #[test]
    fn number_canonicalization() {
        norm_eq("1", "1");
        norm_eq("0", "0");
        norm_eq("-0", "0");
        norm_eq("0.1", "0.1");
        norm_eq("1.0", "1.0");
        norm_eq("1.50", "1.50");
        norm_eq("1e2", "100");
        norm_eq("-1.5e-2", "-0.015");
        norm_eq("9223372036854775808", "9223372036854775808");

        norm_eq("1e100", &format!("1{}", "0".repeat(100)));

        norm_eq("1.3e100", &format!("13{}", "0".repeat(99)));
    }

    #[test]
    fn whitespace_normalization_exact_spacing() {

        norm_eq(r#"{"abc":1}"#, r#"{"abc": 1}"#);
        norm_eq("[1,2]", "[1, 2]");
        norm_eq(
            r#"{"abc":1,"def":2,"ghi":[3,4],"hij":{"klm":5,"nop":[6]}}"#,
            r#"{"abc": 1, "def": 2, "ghi": [3, 4], "hij": {"klm": 5, "nop": [6]}}"#,
        );

        norm_eq(
            "{\n\t\"a\" :\t1 ,\n \"b\" : [ 2 , 3 ]\n}",
            r#"{"a": 1, "b": [2, 3]}"#,
        );
        norm_eq(" true ", "true");

        norm_eq("{}", "{}");
        norm_eq("[ ]", "[]");
    }

    #[test]
    fn nested_structures() {
        norm_eq("[[1],[2,[3]]]", "[[1], [2, [3]]]");
        norm_eq(
            r#"{"z":[{"b":1,"a":2}],"y":{}}"#,
            r#"{"y": {}, "z": [{"a": 2, "b": 1}]}"#,
        );

        let deep = format!("{}{}", "[".repeat(50), "]".repeat(50));
        norm_eq(&deep, &deep);
    }

    #[test]
    fn unicode_escapes_and_string_output() {

        norm_eq("\"\\u0045\"", r#""E""#);

        norm_eq("\"\\u00a9\"", "\"\u{a9}\"");

        norm_eq("\"\\u0024\"", r#""$""#);

        norm_eq("\"\\ud83d\\ude04\"", "\"\u{1f604}\"");

        norm_eq(r#""😄""#, "\"\u{1f604}\"");

        norm_eq(r#""\n\"\\""#, r#""\n\"\\""#);

        norm_eq("\"café\"", "\"café\"");

        norm_eq(r#""\\u0024""#, r#""\\u0024""#);

        norm_eq(r#""\t\r\b\f""#, r#""\t\r\b\f""#);
        norm_eq(r#""""#, r#""""#);
    }

    #[test]
    fn scalars() {
        norm_eq(r#""""#, r#""""#);
        norm_eq("true", "true");
        norm_eq("false", "false");
        norm_eq("null", "null");
    }

    #[test]
    fn invalid_rejected() {

        is_err("''");
        is_err(r#""abc"#);
        is_err("\"abc\ndef\"");
        is_err(r#""\v""#);
        is_err("01");
        is_err("1f2");
        is_err("1.3ex100");
        is_err("-");
        is_err("+1");
        is_err("[1,2,]");
        is_err("[1,2");
        is_err(r#"{"abc"}"#);
        is_err(r#"{1:"abc"}"#);
        is_err(r#"{"abc":1:2}"#);
        is_err("true false");
        is_err("");
        is_err("    ");

        is_err(r#""\u""#);
        is_err(r#""\u00""#);
        is_err(r#""\u000g""#);
        is_err(r#""\u0000""#);
        is_err(r#""\ud83d\ud83d""#);
        is_err(r#""\ude04\ud83d""#);
        is_err(r#""\ud83dX""#);
        is_err(r#""\ude04""#);

        is_err("1e1000000");
    }

    #[test]
    fn idempotence() {
        for s in [
            r#"{"aa":1,"b":2,"cq":3}"#,
            r#"{"d2":"d2","d1":"d1","d1":"d3"}"#,
            "[1,2,3]",
            "1.3e100",
            r#""©""#,
            "{\n\t\"a\" : [ 1 , {\"z\":true,\"y\":null} ]\n}",
            "-1.5e-2",
        ] {
            let once = norm(s);
            let twice = norm(&once);
            assert_eq!(once, twice, "normalize not idempotent for {s:?}");
        }
    }

    #[test]
    fn output_non_text_is_empty() {
        assert_eq!(output(JSONB, &SqlValue::Null), "");
        assert_eq!(output(JSONB, &SqlValue::Int(1)), "");
    }

    #[test]
    fn order_key_byte_order_equals_jsonb_order() {

        let ascending = [
            "null",
            r#""a""#,
            r#""ab""#,
            r#""b""#,
            "-5",
            "0",
            "2.5",
            "3",
            "10",
            "false",
            "true",
            "[]",
            r#"["a"]"#,
            "[1]",
            "[2]",
            "[10]",
            "[1, 1]",
            "[1, 2]",
            "{}",
            r#"{"a": 1}"#,
            r#"{"a": 2}"#,
            r#"{"b": 1}"#,
            r#"{"a": 1, "b": 2}"#,
        ];
        let keys: Vec<SqlValue> = ascending.iter().map(|s| order_key(s)).collect();
        for w in keys.windows(2) {
            assert!(
                w[0].cmp(&w[1]) == std::cmp::Ordering::Less,
                "order key not strictly ascending between adjacent values"
            );
        }

        assert!(order_key("[1, 2]").cmp(&order_key("[1,2]")) == std::cmp::Ordering::Equal);
    }
}
