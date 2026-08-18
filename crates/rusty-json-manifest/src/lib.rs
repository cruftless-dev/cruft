
use std::fmt;

pub type Value = JsonValue;

#[derive(Clone, Debug, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(JsonNumber),
    String(String),
    Array(Vec<JsonValue>),
    Object(Map),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Map {
    entries: Vec<(String, JsonValue)>,
}

impl Map {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn insert(&mut self, key: String, value: JsonValue) -> Option<JsonValue> {
        if let Some((_, existing)) = self.entries.iter_mut().find(|(k, _)| k == &key) {
            Some(std::mem::replace(existing, value))
        } else {
            self.entries.push((key, value));
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn get_last(&self, key: &str) -> Option<&JsonValue> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &JsonValue)> {
        self.entries.iter().map(|(k, v)| (k, v))
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.entries.iter().map(|(k, _)| k)
    }
}

impl IntoIterator for Map {
    type Item = (String, JsonValue);
    type IntoIter = std::vec::IntoIter<(String, JsonValue)>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl<'a> IntoIterator for &'a Map {
    type Item = (&'a String, &'a JsonValue);
    type IntoIter = std::iter::Map<
        std::slice::Iter<'a, (String, JsonValue)>,
        fn(&(String, JsonValue)) -> (&String, &JsonValue),
    >;

    fn into_iter(self) -> Self::IntoIter {
        fn pair((k, v): &(String, JsonValue)) -> (&String, &JsonValue) {
            (k, v)
        }
        self.entries.iter().map(pair)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct JsonNumber {
    raw: String,
}

impl JsonNumber {
    pub fn raw(&self) -> &str {
        &self.raw
    }

    pub fn as_f64(&self) -> Option<f64> {
        self.raw.parse().ok()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonError {
    pub offset: usize,
    pub message: String,
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at byte {}", self.message, self.offset)
    }
}

impl std::error::Error for JsonError {}

pub const DEFAULT_MAX_JSON_BYTES: usize = 8 * 1024 * 1024;

impl JsonValue {
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            JsonValue::Object(entries) => entries.get(key),
            _ => None,
        }
    }

    pub fn get_last(&self, key: &str) -> Option<&JsonValue> {
        match self {
            JsonValue::Object(entries) => entries.get_last(key),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            JsonValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[JsonValue]> {
        match self {
            JsonValue::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&Map> {
        match self {
            JsonValue::Object(o) => Some(o),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            JsonValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            JsonValue::Number(n) => n.raw.parse().ok(),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            JsonValue::Number(n) => n.as_f64(),
            _ => None,
        }
    }

    pub fn is_string(&self) -> bool {
        matches!(self, JsonValue::String(_))
    }

    pub fn is_array(&self) -> bool {
        matches!(self, JsonValue::Array(_))
    }

    pub fn to_compact_string(&self) -> String {
        let mut out = String::new();
        write_json(self, &mut out, None, 0);
        out
    }

    pub fn to_pretty_string(&self) -> String {
        let mut out = String::new();
        write_json(self, &mut out, Some(2), 0);
        out
    }
}

impl fmt::Display for JsonValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_compact_string())
    }
}

pub trait FromJsonValue: Sized {
    fn from_json_value(value: JsonValue) -> Result<Self, JsonError>;
}

impl FromJsonValue for JsonValue {
    fn from_json_value(value: JsonValue) -> Result<Self, JsonError> {
        Ok(value)
    }
}

pub fn from_str<T: FromJsonValue>(src: &str) -> Result<T, JsonError> {
    T::from_json_value(parse_value_str(src)?)
}

pub fn from_slice<T: FromJsonValue>(src: &[u8]) -> Result<T, JsonError> {
    validate_input_size(src.len())?;
    let s = std::str::from_utf8(src).map_err(|e| JsonError {
        offset: e.valid_up_to(),
        message: "input is not utf-8".into(),
    })?;
    from_str(s)
}

pub fn parse_value_str(src: &str) -> Result<JsonValue, JsonError> {
    validate_input_size(src.len())?;
    let mut p = Parser {
        bytes: src.as_bytes(),
        pos: 0,
        depth: 0,
    };
    p.skip_ws();
    let value = p.parse_value()?;
    p.skip_ws();
    if p.eof() {
        Ok(value)
    } else {
        Err(p.err("trailing characters"))
    }
}

fn validate_input_size(len: usize) -> Result<(), JsonError> {
    if len > DEFAULT_MAX_JSON_BYTES {
        return Err(JsonError {
            offset: DEFAULT_MAX_JSON_BYTES,
            message: format!("json input exceeds parser limit of {DEFAULT_MAX_JSON_BYTES} bytes"),
        });
    }
    Ok(())
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
    depth: usize,
}

impl<'a> Parser<'a> {
    const MAX_DEPTH: usize = 256;

    fn eof(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos += 1;
        Some(b)
    }

    fn err(&self, message: &str) -> JsonError {
        JsonError {
            offset: self.pos,
            message: message.into(),
        }
    }

    fn enter_nested(&mut self) -> Result<(), JsonError> {
        if self.depth >= Self::MAX_DEPTH {
            return Err(self.err("json nesting depth exceeded"));
        }
        self.depth += 1;
        Ok(())
    }

    fn leave_nested(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.pos += 1;
        }
    }

    fn parse_value(&mut self) -> Result<JsonValue, JsonError> {
        match self.peek() {
            Some(b'n') => self.literal(b"null", JsonValue::Null),
            Some(b't') => self.literal(b"true", JsonValue::Bool(true)),
            Some(b'f') => self.literal(b"false", JsonValue::Bool(false)),
            Some(b'"') => self.parse_string().map(JsonValue::String),
            Some(b'[') => self.parse_array(),
            Some(b'{') => self.parse_object(),
            Some(b'-' | b'0'..=b'9') => self.parse_number().map(JsonValue::Number),
            Some(_) => Err(self.err("expected json value")),
            None => Err(self.err("unexpected end of input")),
        }
    }

    fn literal(&mut self, lit: &[u8], value: JsonValue) -> Result<JsonValue, JsonError> {
        if self.bytes.get(self.pos..self.pos + lit.len()) == Some(lit) {
            self.pos += lit.len();
            Ok(value)
        } else {
            Err(self.err("invalid literal"))
        }
    }

    fn parse_array(&mut self) -> Result<JsonValue, JsonError> {
        self.enter_nested()?;
        self.bump();
        let mut values = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.bump();
            self.leave_nested();
            return Ok(JsonValue::Array(values));
        }
        loop {
            self.skip_ws();
            match self.parse_value() {
                Ok(value) => values.push(value),
                Err(err) => {
                    self.leave_nested();
                    return Err(err);
                }
            }
            self.skip_ws();
            match self.bump() {
                Some(b',') => {}
                Some(b']') => {
                    self.leave_nested();
                    return Ok(JsonValue::Array(values));
                }
                _ => {
                    let err = self.err("expected ',' or ']'");
                    self.leave_nested();
                    return Err(err);
                }
            }
        }
    }

    fn parse_object(&mut self) -> Result<JsonValue, JsonError> {
        self.enter_nested()?;
        self.bump();
        let mut entries = Map::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.bump();
            self.leave_nested();
            return Ok(JsonValue::Object(entries));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                let err = self.err("expected object key");
                self.leave_nested();
                return Err(err);
            }
            let key = match self.parse_string() {
                Ok(key) => key,
                Err(err) => {
                    self.leave_nested();
                    return Err(err);
                }
            };
            self.skip_ws();
            if self.bump() != Some(b':') {
                let err = self.err("expected ':'");
                self.leave_nested();
                return Err(err);
            }
            self.skip_ws();
            let value = match self.parse_value() {
                Ok(value) => value,
                Err(err) => {
                    self.leave_nested();
                    return Err(err);
                }
            };
            entries.entries.push((key, value));
            self.skip_ws();
            match self.bump() {
                Some(b',') => {}
                Some(b'}') => {
                    self.leave_nested();
                    return Ok(JsonValue::Object(entries));
                }
                _ => {
                    let err = self.err("expected ',' or '}'");
                    self.leave_nested();
                    return Err(err);
                }
            }
        }
    }

    fn parse_string(&mut self) -> Result<String, JsonError> {
        if self.bump() != Some(b'"') {
            return Err(self.err("expected string"));
        }
        let mut out = String::new();
        loop {
            let b = self.bump().ok_or_else(|| self.err("unterminated string"))?;
            match b {
                b'"' => return Ok(out),
                b'\\' => self.parse_escape(&mut out)?,
                0x00..=0x1f => return Err(self.err("control character in string")),
                _ if b < 0x80 => out.push(b as char),
                _ => {
                    let start = self.pos - 1;
                    let s = std::str::from_utf8(&self.bytes[start..]).map_err(|_| JsonError {
                        offset: start,
                        message: "invalid utf-8 in string".into(),
                    })?;
                    let ch = s.chars().next().ok_or_else(|| self.err("invalid string"))?;
                    self.pos = start + ch.len_utf8();
                    out.push(ch);
                }
            }
        }
    }

    fn parse_escape(&mut self, out: &mut String) -> Result<(), JsonError> {
        match self.bump().ok_or_else(|| self.err("unterminated escape"))? {
            b'"' => out.push('"'),
            b'\\' => out.push('\\'),
            b'/' => out.push('/'),
            b'b' => out.push('\u{0008}'),
            b'f' => out.push('\u{000c}'),
            b'n' => out.push('\n'),
            b'r' => out.push('\r'),
            b't' => out.push('\t'),
            b'u' => {
                let first = self.parse_hex4()?;
                if (0xd800..=0xdbff).contains(&first) {
                    let save = self.pos;
                    if self.bump() == Some(b'\\') && self.bump() == Some(b'u') {
                        let second = self.parse_hex4()?;
                        if (0xdc00..=0xdfff).contains(&second) {
                            let scalar = 0x10000
                                + (((first - 0xd800) as u32) << 10)
                                + (second - 0xdc00) as u32;
                            out.push(
                                char::from_u32(scalar)
                                    .ok_or_else(|| self.err("invalid unicode scalar"))?,
                            );
                        } else {
                            return Err(self.err("invalid low surrogate"));
                        }
                    } else {
                        self.pos = save;
                        return Err(self.err("missing low surrogate"));
                    }
                } else if (0xdc00..=0xdfff).contains(&first) {
                    return Err(self.err("lone low surrogate"));
                } else {
                    out.push(
                        char::from_u32(first as u32)
                            .ok_or_else(|| self.err("invalid unicode scalar"))?,
                    );
                }
            }
            _ => return Err(self.err("invalid escape")),
        }
        Ok(())
    }

    fn parse_hex4(&mut self) -> Result<u16, JsonError> {
        let mut n = 0u16;
        for _ in 0..4 {
            let b = self
                .bump()
                .ok_or_else(|| self.err("short unicode escape"))?;
            n = (n << 4)
                | match b {
                    b'0'..=b'9' => (b - b'0') as u16,
                    b'a'..=b'f' => (b - b'a' + 10) as u16,
                    b'A'..=b'F' => (b - b'A' + 10) as u16,
                    _ => return Err(self.err("invalid unicode escape")),
                };
        }
        Ok(n)
    }

    fn parse_number(&mut self) -> Result<JsonNumber, JsonError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => {
                self.pos += 1;
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.pos += 1;
                }
            }
            _ => return Err(self.err("invalid number")),
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            let digit_start = self.pos;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
            if self.pos == digit_start {
                return Err(self.err("invalid number fraction"));
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            let digit_start = self.pos;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
            if self.pos == digit_start {
                return Err(self.err("invalid number exponent"));
            }
        }
        let raw = std::str::from_utf8(&self.bytes[start..self.pos])
            .unwrap()
            .to_string();
        Ok(JsonNumber { raw })
    }
}

fn write_json(v: &JsonValue, out: &mut String, pretty: Option<usize>, depth: usize) {
    match v {
        JsonValue::Null => out.push_str("null"),
        JsonValue::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        JsonValue::Number(n) => out.push_str(n.raw()),
        JsonValue::String(s) => write_string(s, out),
        JsonValue::Array(values) => {
            out.push('[');
            write_seq(values.iter(), out, pretty, depth);
            out.push(']');
        }
        JsonValue::Object(entries) => {
            out.push('{');
            let mut first = true;
            for (k, val) in entries.iter() {
                write_sep(&mut first, out, pretty, depth);
                write_string(k, out);
                out.push(':');
                if pretty.is_some() {
                    out.push(' ');
                }
                write_json(val, out, pretty, depth + 1);
            }
            write_close(entries.is_empty(), out, pretty, depth);
            out.push('}');
        }
    }
}

fn write_seq<'a, I>(values: I, out: &mut String, pretty: Option<usize>, depth: usize)
where
    I: Iterator<Item = &'a JsonValue>,
{
    let mut first = true;
    let mut empty = true;
    for value in values {
        empty = false;
        write_sep(&mut first, out, pretty, depth);
        write_json(value, out, pretty, depth + 1);
    }
    write_close(empty, out, pretty, depth);
}

fn write_sep(first: &mut bool, out: &mut String, pretty: Option<usize>, depth: usize) {
    if !*first {
        out.push(',');
    }
    *first = false;
    if let Some(spaces) = pretty {
        out.push('\n');
        out.push_str(&" ".repeat((depth + 1) * spaces));
    }
}

fn write_close(empty: bool, out: &mut String, pretty: Option<usize>, depth: usize) {
    if !empty {
        if let Some(spaces) = pretty {
            out.push('\n');
            out.push_str(&" ".repeat(depth * spaces));
        }
    }
}

fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000c}' => out.push_str("\\f"),
            c if c <= '\u{001f}' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

pub trait IntoJsonValue {
    fn into_json_value(self) -> JsonValue;
}

impl IntoJsonValue for JsonValue {
    fn into_json_value(self) -> JsonValue {
        self
    }
}

impl IntoJsonValue for Map {
    fn into_json_value(self) -> JsonValue {
        JsonValue::Object(self)
    }
}

impl IntoJsonValue for &str {
    fn into_json_value(self) -> JsonValue {
        JsonValue::String(self.to_string())
    }
}

impl IntoJsonValue for &&str {
    fn into_json_value(self) -> JsonValue {
        JsonValue::String((*self).to_string())
    }
}

impl IntoJsonValue for String {
    fn into_json_value(self) -> JsonValue {
        JsonValue::String(self)
    }
}

impl IntoJsonValue for &String {
    fn into_json_value(self) -> JsonValue {
        JsonValue::String(self.clone())
    }
}

impl IntoJsonValue for &&String {
    fn into_json_value(self) -> JsonValue {
        JsonValue::String((*self).clone())
    }
}

impl IntoJsonValue for bool {
    fn into_json_value(self) -> JsonValue {
        JsonValue::Bool(self)
    }
}

macro_rules! impl_number_value {
    ($($t:ty),* $(,)?) => {
        $(
            impl IntoJsonValue for $t {
                fn into_json_value(self) -> JsonValue {
                    JsonValue::Number(JsonNumber { raw: self.to_string() })
                }
            }
        )*
    };
}

impl_number_value!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize, f32, f64);

pub fn to_value<T: IntoJsonValue>(value: T) -> JsonValue {
    value.into_json_value()
}

#[macro_export]
macro_rules! json {
    (null) => {
        $crate::JsonValue::Null
    };
    ([ $($items:tt)* ]) => {{
        let mut array = Vec::new();
        $crate::json_array_items!(array, $($items)*);
        $crate::JsonValue::Array(array)
    }};
    ({ $($items:tt)* }) => {{
        let mut object = $crate::Map::new();
        $crate::json_object_items!(object, $($items)*);
        $crate::JsonValue::Object(object)
    }};
    ($other:expr) => {
        $crate::to_value($other)
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! json_object_items {
    ($object:ident,) => {};
    ($object:ident) => {};
    ($object:ident, $key:literal : { $($value:tt)* } $(, $($rest:tt)*)?) => {{
        $object.insert($key.to_string(), $crate::json!({ $($value)* }));
        $($crate::json_object_items!($object, $($rest)*);)?
    }};
    ($object:ident, $key:literal : [ $($value:tt)* ] $(, $($rest:tt)*)?) => {{
        $object.insert($key.to_string(), $crate::json!([ $($value)* ]));
        $($crate::json_object_items!($object, $($rest)*);)?
    }};
    ($object:ident, $key:literal : $value:expr $(, $($rest:tt)*)?) => {{
        $object.insert($key.to_string(), $crate::to_value($value));
        $($crate::json_object_items!($object, $($rest)*);)?
    }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! json_array_items {
    ($array:ident,) => {};
    ($array:ident) => {};
    ($array:ident, { $($value:tt)* } $(, $($rest:tt)*)?) => {{
        $array.push($crate::json!({ $($value)* }));
        $($crate::json_array_items!($array, $($rest)*);)?
    }};
    ($array:ident, [ $($value:tt)* ] $(, $($rest:tt)*)?) => {{
        $array.push($crate::json!([ $($value)* ]));
        $($crate::json_array_items!($array, $($rest)*);)?
    }};
    ($array:ident, $value:expr $(, $($rest:tt)*)?) => {{
        $array.push($crate::to_value($value));
        $($crate::json_array_items!($array, $($rest)*);)?
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_primitives_and_numbers() {
        assert_eq!(from_str::<Value>("null").unwrap(), JsonValue::Null);
        assert_eq!(from_str::<Value>(" true ").unwrap(), JsonValue::Bool(true));
        let n = match from_str::<Value>("-12.5e+2").unwrap() {
            JsonValue::Number(n) => n,
            other => panic!("{other:?}"),
        };
        assert_eq!(n.raw(), "-12.5e+2");
        assert_eq!(n.as_f64(), Some(-1250.0));
    }

    #[test]
    fn rejects_invalid_number_forms() {
        assert!(from_str::<Value>("01").is_err());
        assert!(from_str::<Value>("1.").is_err());
        assert!(from_str::<Value>("1e").is_err());
    }

    #[test]
    fn parses_escapes_and_surrogates() {
        let v = from_str::<Value>(r#""a\n\u00e9\uD834\uDD1E""#).unwrap();
        assert_eq!(v.as_str(), Some("a\né𝄞"));
        assert!(from_str::<Value>(r#""\uD834""#).is_err());
        assert!(from_str::<Value>("\"\u{001f}\"").is_err());
    }

    #[test]
    fn object_order_and_duplicate_keys_are_preserved() {
        let v = from_str::<Value>(r#"{"b":1,"a":2,"b":3}"#).unwrap();
        let obj = v.as_object().unwrap();
        let keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        assert_eq!(keys, vec!["b", "a", "b"]);
        assert_eq!(v.get("b").unwrap().to_compact_string(), "1");
        assert_eq!(v.get_last("b").unwrap().to_compact_string(), "3");
    }

    #[test]
    fn parses_from_bytes_and_nested_lookup() {
        let v = from_slice::<Value>(br##"{"exports":{"default":"./index.js"},"imports":["#x"]}"##)
            .unwrap();
        assert_eq!(
            v.get("exports")
                .and_then(|e| e.get("default"))
                .and_then(JsonValue::as_str),
            Some("./index.js")
        );
        assert_eq!(
            v.get("imports")
                .and_then(JsonValue::as_array)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn compact_writer_round_trips_manifest_shape() {
        let src = r#"{"name":"x","exports":{"default":"./d.js"},"sideEffects":false}"#;
        let v = from_str::<Value>(src).unwrap();
        assert_eq!(from_str::<Value>(&v.to_compact_string()).unwrap(), v);
    }

    #[test]
    fn pretty_writer_is_stable() {
        let v = from_str::<Value>(r#"{"a":[true,null]}"#).unwrap();
        assert_eq!(
            v.to_pretty_string(),
            "{\n  \"a\": [\n    true,\n    null\n  ]\n}"
        );
    }

    #[test]
    fn rejects_trailing_and_malformed_structures() {
        assert!(from_str::<Value>("{} x").is_err());
        assert!(from_str::<Value>("[1,]").is_err());
        assert!(from_str::<Value>("{\"a\" 1}").is_err());
    }

    #[test]
    fn rejects_excessive_nesting_without_stack_overflow() {
        let mut arrays = "[".repeat(1100);
        arrays.push_str("0");
        arrays.push_str(&"]".repeat(1100));
        let err = from_str::<Value>(&arrays).unwrap_err();
        assert!(err.message.contains("nesting depth"));

        let mut objects = String::new();
        for _ in 0..1100 {
            objects.push_str("{\"a\":");
        }
        objects.push('0');
        objects.push_str(&"}".repeat(1100));
        let err = from_str::<Value>(&objects).unwrap_err();
        assert!(err.message.contains("nesting depth"));
    }

    #[test]
    fn rejects_oversized_json_before_parse() {
        let src = " ".repeat(DEFAULT_MAX_JSON_BYTES + 1);
        let err = from_str::<Value>(&src).unwrap_err();
        assert!(err.message.contains("parser limit"));
    }

    #[test]
    fn json_macro_builds_consumed_runtime_shapes() {
        let mut packages = Map::new();
        packages.insert(
            "demo@1.0.0".into(),
            json!({"name": "demo", "version": "1.0.0"}),
        );
        let v = json!({"version": 2, "packages": Value::Object(packages)});
        assert_eq!(v.get("version").and_then(JsonValue::as_u64), Some(2));
        assert_eq!(
            v.get("packages")
                .and_then(|p| p.get("demo@1.0.0"))
                .and_then(|p| p.get("name"))
                .and_then(JsonValue::as_str),
            Some("demo")
        );
    }

    #[test]
    fn json_macro_builds_consumed_apparatus_shapes() {
        let cluster = String::from("built-ins.Array");
        let cluster_ref = &cluster;
        let rec = json!({
            "resolver": "runtime/spec-builtins",
            "cluster": cluster_ref,
            "fail": 12usize,
            "score": 3.5f64,
            "open": true,
        });
        assert_eq!(
            rec.get("cluster").and_then(JsonValue::as_str),
            Some("built-ins.Array")
        );
        assert_eq!(rec.get("fail").and_then(JsonValue::as_u64), Some(12));
        assert_eq!(rec.get("score").and_then(JsonValue::as_f64), Some(3.5));
        assert_eq!(rec.get("open").and_then(JsonValue::as_bool), Some(true));
        assert_eq!(
            rec.to_string(),
            r#"{"resolver":"runtime/spec-builtins","cluster":"built-ins.Array","fail":12,"score":3.5,"open":true}"#
        );
    }
}
