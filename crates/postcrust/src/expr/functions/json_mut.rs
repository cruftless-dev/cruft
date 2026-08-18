
use crate::types::PgError;
use sql_core::SqlValue;

fn does_not_exist(name: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("function {name}(...) does not exist"),
    }
}

fn json_err(msg: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "expression",
        input: msg.to_string(),
    }
}

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    match name {
        "jsonb_set" => Some(set_fn(name, args)),
        "jsonb_insert" => Some(insert_fn(name, args)),

        "%row_to_json" => Some(row_to_json_value(args)),
        _ => None,
    }
}

fn set_fn(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() < 3 || args.len() > 4 {
        return Err(does_not_exist(name));
    }
    let create_if_missing = match args.get(3) {
        None => true,
        Some(SqlValue::Int(n)) => *n != 0,
        Some(SqlValue::Null) => return Ok(SqlValue::Null),
        Some(_) => return Err(does_not_exist(name)),
    };
    mutate(name, args, MutKind::Set { create_if_missing })
}

fn insert_fn(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() < 3 || args.len() > 4 {
        return Err(does_not_exist(name));
    }
    let insert_after = match args.get(3) {
        None => false,
        Some(SqlValue::Int(n)) => *n != 0,
        Some(SqlValue::Null) => return Ok(SqlValue::Null),
        Some(_) => return Err(does_not_exist(name)),
    };
    mutate(name, args, MutKind::Insert { insert_after })
}

enum MutKind {
    Set { create_if_missing: bool },
    Insert { insert_after: bool },
}

fn mutate(name: &str, args: &[SqlValue], kind: MutKind) -> Result<SqlValue, PgError> {
    let target = match &args[0] {
        SqlValue::Null => return Ok(SqlValue::Null),
        SqlValue::Text(s) => s.as_str(),
        _ => return Err(does_not_exist(name)),
    };
    let path_txt = match &args[1] {
        SqlValue::Null => return Ok(SqlValue::Null),
        SqlValue::Text(s) => s.as_str(),
        _ => return Err(does_not_exist(name)),
    };
    let new_txt = match &args[2] {
        SqlValue::Null => return Ok(SqlValue::Null),
        SqlValue::Text(s) => s.as_str(),
        _ => return Err(does_not_exist(name)),
    };
    let mut node = parse(target).map_err(|()| does_not_exist(name))?;
    let new_value = parse(new_txt).map_err(|()| does_not_exist(name))?;
    let path = parse_path(path_txt).ok_or_else(|| json_err("malformed array literal in path"))?;
    if path.is_empty() {

        let mut out = String::new();
        serialize(&node, &mut out);
        return Ok(SqlValue::Text(out));
    }
    apply(&mut node, &path, new_value, &kind)?;
    let mut out = String::new();
    serialize(&node, &mut out);
    Ok(SqlValue::Text(out))
}

fn array_index(elem: &str, len: usize) -> Result<i64, ()> {
    let raw: i64 = elem.trim().parse().map_err(|_| ())?;
    Ok(if raw < 0 { len as i64 + raw } else { raw })
}

fn apply(node: &mut Json, path: &[String], new: Json, kind: &MutKind) -> Result<(), PgError> {
    let (head, rest) = path.split_first().expect("non-empty path");
    let last = rest.is_empty();
    match node {
        Json::Obj(members) => {
            let pos = members.iter().position(|(k, _)| k == head);
            if last {
                match (pos, kind) {
                    (Some(i), MutKind::Set { .. }) => members[i].1 = new,
                    (Some(_), MutKind::Insert { .. }) => {

                        return Err(json_err("cannot replace existing key"));
                    }
                    (None, MutKind::Set { create_if_missing }) => {
                        if *create_if_missing {
                            members.push((head.clone(), new));
                        }
                    }
                    (None, MutKind::Insert { .. }) => members.push((head.clone(), new)),
                }
            } else if let Some(i) = pos {
                apply(&mut members[i].1, rest, new, kind)?;
            }

            Ok(())
        }
        Json::Arr(items) => {
            let len = items.len();
            let idx =
                array_index(head, len).map_err(|()| json_err("path element is not an integer"))?;
            if last {
                match kind {
                    MutKind::Set { create_if_missing } => {
                        if idx >= 0 && (idx as usize) < len {
                            items[idx as usize] = new;
                        } else if *create_if_missing {
                            if idx < 0 {
                                items.insert(0, new);
                            } else {
                                items.push(new);
                            }
                        }
                    }
                    MutKind::Insert { insert_after } => {

                        let base = idx.clamp(0, len as i64) as usize;
                        let at = if *insert_after {
                            (base + 1).min(len)
                        } else {
                            base
                        };
                        items.insert(at, new);
                    }
                }
            } else if idx >= 0 && (idx as usize) < len {
                apply(&mut items[idx as usize], rest, new, kind)?;
            }
            Ok(())
        }

        _ => Ok(()),
    }
}

fn parse_path(text: &str) -> Option<Vec<String>> {
    let cs: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < cs.len() && cs[i].is_whitespace() {
        i += 1;
    }
    if cs.get(i) != Some(&'{') {
        return None;
    }
    i += 1;
    let mut out = Vec::new();

    while i < cs.len() && cs[i].is_whitespace() {
        i += 1;
    }
    if cs.get(i) == Some(&'}') {
        return Some(out);
    }
    loop {
        let mut buf = String::new();
        if cs.get(i) == Some(&'"') {
            i += 1;
            loop {
                match cs.get(i) {
                    None => return None,
                    Some('\\') => {
                        i += 1;
                        buf.push(*cs.get(i)?);
                        i += 1;
                    }
                    Some('"') => {
                        i += 1;
                        break;
                    }
                    Some(c) => {
                        buf.push(*c);
                        i += 1;
                    }
                }
            }
        } else {
            while let Some(&c) = cs.get(i) {
                if c == ',' || c == '}' {
                    break;
                }
                buf.push(c);
                i += 1;
            }
            buf = buf.trim().to_string();
        }
        out.push(buf);
        match cs.get(i) {
            Some(',') => {
                i += 1;
            }
            Some('}') => break,
            _ => return None,
        }
    }
    Some(out)
}

pub fn object_keys_rows(name: &str, args: &[SqlValue]) -> Result<Vec<SqlValue>, PgError> {
    if args.len() != 1 {
        return Err(json_err(&format!("{name}(jsonb) expects 1 argument")));
    }
    let text = match &args[0] {
        SqlValue::Null => return Ok(Vec::new()),
        SqlValue::Text(s) => s.as_str(),
        _ => return Err(json_err(&format!("{name}: argument is not json"))),
    };
    let keys = top_level_keys(text)
        .ok_or_else(|| json_err(&format!("cannot call {name} on a non-object")))?;
    Ok(keys.into_iter().map(SqlValue::Text).collect())
}

fn top_level_keys(text: &str) -> Option<Vec<String>> {
    let mut p = Parser {
        bytes: text.as_bytes(),
        pos: 0,
    };
    p.skip_ws();
    if p.peek() != Some(b'{') {

        return None;
    }
    p.bump();
    let mut keys = Vec::new();
    p.skip_ws();
    if p.peek() == Some(b'}') {
        return Some(keys);
    }
    loop {
        p.skip_ws();
        if p.peek() != Some(b'"') {
            return None;
        }
        let key = p.string().ok()?;
        p.skip_ws();
        if p.bump() != Some(b':') {
            return None;
        }
        p.skip_ws();
        p.value().ok()?;
        keys.push(key);
        p.skip_ws();
        match p.bump() {
            Some(b',') => continue,
            Some(b'}') => break,
            _ => return None,
        }
    }
    Some(keys)
}

pub fn each_rows(
    name: &str,
    arg: &SqlValue,
    jsonb: bool,
    as_text: bool,
) -> Result<Vec<(String, SqlValue)>, PgError> {
    let raw = match arg {
        SqlValue::Null => return Ok(Vec::new()),
        SqlValue::Text(s) => s.clone(),
        _ => return Err(json_err(&format!("{name}: argument is not json"))),
    };

    let source = if jsonb {
        match crate::types::jsonb::input(crate::types::oid::JSONB, &raw)? {
            SqlValue::Text(s) => s,
            _ => raw,
        }
    } else {
        crate::types::json::input(crate::types::oid::JSON, &raw)?;
        raw
    };
    let members = slice_members(&source)
        .ok_or_else(|| json_err(&format!("cannot call {name} on a non-object")))?;
    let mut out = Vec::with_capacity(members.len());
    for (key, val_src) in members {
        let value = if as_text {
            match parse(&val_src) {
                Ok(Json::Str(s)) => SqlValue::Text(s),
                _ => SqlValue::Text(val_src),
            }
        } else {
            SqlValue::Text(val_src)
        };
        out.push((key, value));
    }
    Ok(out)
}

fn slice_members(text: &str) -> Option<Vec<(String, String)>> {
    let mut p = Parser {
        bytes: text.as_bytes(),
        pos: 0,
    };
    p.skip_ws();
    if p.peek() != Some(b'{') {
        return None;
    }
    p.bump();
    let mut members = Vec::new();
    p.skip_ws();
    if p.peek() == Some(b'}') {
        return Some(members);
    }
    loop {
        p.skip_ws();
        if p.peek() != Some(b'"') {
            return None;
        }
        let key = p.string().ok()?;
        p.skip_ws();
        if p.bump() != Some(b':') {
            return None;
        }
        p.skip_ws();
        let vstart = p.pos;
        p.value().ok()?;
        let vsrc = core::str::from_utf8(&p.bytes[vstart..p.pos])
            .ok()?
            .to_string();
        members.push((key, vsrc));
        p.skip_ws();
        match p.bump() {
            Some(b',') => continue,
            Some(b'}') => break,
            _ => return None,
        }
    }
    Some(members)
}

pub fn row_to_json_value(args: &[SqlValue]) -> Result<SqlValue, PgError> {
    let value = match args.first() {
        None => return Err(does_not_exist("row_to_json")),
        Some(SqlValue::Null) => return Ok(SqlValue::Null),
        Some(SqlValue::Text(s)) => s.as_str(),
        Some(_) => return Err(does_not_exist("row_to_json")),
    };

    let mut fields: Vec<(String, u32)> = Vec::new();
    let mut rest = &args[1..];
    while rest.len() >= 2 {
        let name = match &rest[0] {
            SqlValue::Text(s) => s.clone(),
            _ => return Err(does_not_exist("row_to_json")),
        };
        let oid = match &rest[1] {
            SqlValue::Int(n) => *n as u32,
            _ => return Err(does_not_exist("row_to_json")),
        };
        fields.push((name, oid));
        rest = &rest[2..];
    }
    let parts = crate::types::composite::decode(value)
        .map_err(|()| json_err(&format!("malformed record literal: \"{value}\"")))?;
    if parts.len() != fields.len() {
        return Err(json_err("row_to_json: field count mismatch"));
    }
    let mut out = String::from("{");
    for (i, ((name, oid), part)) in fields.iter().zip(parts.iter()).enumerate() {
        if i > 0 {
            out.push(',');
        }
        quote_string(name, &mut out);
        out.push(':');
        out.push_str(&field_to_json(part.as_deref(), *oid));
    }
    out.push('}');
    Ok(SqlValue::Text(out))
}

fn field_to_json(part: Option<&str>, oid: u32) -> String {
    use crate::types::oid;
    let s = match part {
        None => return "null".to_string(),
        Some(s) => s,
    };
    match oid {

        oid::INT2 | oid::INT4 | oid::INT8 | oid::FLOAT4 | oid::FLOAT8 | oid::NUMERIC => {
            s.to_string()
        }

        oid::BOOL => {
            if s == "t" {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }

        oid::JSON | oid::JSONB => s.to_string(),

        _ => {
            let mut out = String::new();
            quote_string(s, &mut out);
            out
        }
    }
}

fn quote_string(s: &str, out: &mut String) {
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
        Ok(Json::Obj(members))
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
            let mut idx: Vec<usize> = (0..members.len()).collect();
            idx.sort_by(|&a, &b| {
                let (ka, kb) = (&members[a].0, &members[b].0);
                ka.len()
                    .cmp(&kb.len())
                    .then_with(|| ka.as_bytes().cmp(kb.as_bytes()))
            });

            let mut kept: Vec<usize> = Vec::with_capacity(idx.len());
            for &i in &idx {
                if let Some(&last) = kept.last() {
                    if members[last].0 == members[i].0 {
                        *kept.last_mut().unwrap() = i;
                        continue;
                    }
                }
                kept.push(i);
            }
            out.push('{');
            for (n, &i) in kept.iter().enumerate() {
                if n > 0 {
                    out.push_str(", ");
                }
                serialize_string(&members[i].0, out);
                out.push_str(": ");
                serialize(&members[i].1, out);
            }
            out.push('}');
        }
    }
}

fn serialize_string(s: &str, out: &mut String) {
    quote_string(s, out);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> SqlValue {
        SqlValue::Text(s.to_string())
    }

    fn set(args: &[SqlValue]) -> String {
        match call("jsonb_set", args).unwrap().unwrap() {
            SqlValue::Text(s) => s,
            other => panic!("expected text, got {other:?}"),
        }
    }

    fn ins(args: &[SqlValue]) -> String {
        match call("jsonb_insert", args).unwrap().unwrap() {
            SqlValue::Text(s) => s,
            other => panic!("expected text, got {other:?}"),
        }
    }

    #[test]
    fn set_replace_existing_key() {
        assert_eq!(
            set(&[t(r#"{"a": 1, "b": 2}"#), t("{b}"), t("9")]),
            r#"{"a": 1, "b": 9}"#
        );
    }

    #[test]
    fn set_creates_missing_key_by_default() {
        assert_eq!(
            set(&[t(r#"{"a": 1}"#), t("{c}"), t("3")]),
            r#"{"a": 1, "c": 3}"#
        );

        assert_eq!(
            set(&[t(r#"{"a": 1}"#), t("{c}"), t("3"), SqlValue::Int(0)]),
            r#"{"a": 1}"#
        );
    }

    #[test]
    fn set_nested_path() {
        assert_eq!(
            set(&[t(r#"{"a": {"b": 1}}"#), t("{a,b}"), t("9")]),
            r#"{"a": {"b": 9}}"#
        );
    }

    #[test]
    fn set_array_index_and_negative() {
        assert_eq!(
            set(&[t(r#"{"a": [1, 2, 3]}"#), t("{a,1}"), t("9")]),
            r#"{"a": [1, 9, 3]}"#
        );

        assert_eq!(set(&[t(r#"[1, 2, 3]"#), t("{-1}"), t("9")]), "[1, 2, 9]");
    }

    #[test]
    fn insert_array_before_and_after() {
        assert_eq!(
            ins(&[t(r#"{"a": [1, 2]}"#), t("{a,1}"), t("9")]),
            r#"{"a": [1, 9, 2]}"#
        );
        assert_eq!(
            ins(&[t(r#"{"a": [1, 2]}"#), t("{a,1}"), t("9"), SqlValue::Int(1)]),
            r#"{"a": [1, 2, 9]}"#
        );
    }

    #[test]
    fn insert_object_key_new_ok_existing_errors() {
        assert_eq!(
            ins(&[t(r#"{"a": 1}"#), t("{b}"), t("2")]),
            r#"{"a": 1, "b": 2}"#
        );
        assert!(call("jsonb_insert", &[t(r#"{"a": 1}"#), t("{a}"), t("2")])
            .unwrap()
            .is_err());
    }

    #[test]
    fn strict_null_propagation() {
        assert_eq!(
            call("jsonb_set", &[SqlValue::Null, t("{a}"), t("1")])
                .unwrap()
                .unwrap(),
            SqlValue::Null
        );
        assert_eq!(
            call("jsonb_set", &[t("{}"), t("{a}"), SqlValue::Null])
                .unwrap()
                .unwrap(),
            SqlValue::Null
        );
    }

    #[test]
    fn object_keys_order_and_errors() {
        let ks = object_keys_rows("jsonb_object_keys", &[t(r#"{"x": 1, "y": 2}"#)]).unwrap();
        assert_eq!(ks, vec![t("x"), t("y")]);

        assert!(object_keys_rows("jsonb_object_keys", &[SqlValue::Null])
            .unwrap()
            .is_empty());
        assert!(object_keys_rows("jsonb_object_keys", &[t("[1, 2]")]).is_err());
        assert!(object_keys_rows("jsonb_object_keys", &[t("5")]).is_err());
    }

    #[test]
    fn row_to_json_named_and_anon() {
        use crate::types::oid;

        let v = row_to_json_value(&[
            t("(1,2)"),
            t("x"),
            SqlValue::Int(oid::INT4 as i64),
            t("y"),
            SqlValue::Int(oid::INT4 as i64),
        ])
        .unwrap();
        assert_eq!(v, t(r#"{"x":1,"y":2}"#));

        let v = row_to_json_value(&[
            t("(1,foo)"),
            t("f1"),
            SqlValue::Int(oid::INT4 as i64),
            t("f2"),
            SqlValue::Int(oid::TEXT as i64),
        ])
        .unwrap();
        assert_eq!(v, t(r#"{"f1":1,"f2":"foo"}"#));
    }

    #[test]
    fn unclaimed_name_is_none() {
        assert!(call("jsonb_typeof", &[t("{}")]).is_none());
    }
}
