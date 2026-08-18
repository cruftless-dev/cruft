
use super::{oid, type_name, PgError};
use sql_core::SqlValue;

fn element_oid(array_oid: u32) -> u32 {
    match array_oid {
        oid::BOOL_ARRAY => oid::BOOL,
        oid::INT2_ARRAY => oid::INT2,
        oid::INT4_ARRAY => oid::INT4,
        oid::INT8_ARRAY => oid::INT8,
        oid::TEXT_ARRAY => oid::TEXT,
        oid::FLOAT8_ARRAY => oid::FLOAT8,
        oid::NUMERIC_ARRAY => oid::NUMERIC,

        other => other,
    }
}

fn is_pg_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r')
}

enum Node {
    Elem(Option<String>),
    Arr(Vec<Node>),
}

struct Parser<'a> {
    chars: &'a [char],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(c) if is_pg_space(c)) {
            self.pos += 1;
        }
    }

    fn parse_array(&mut self) -> Result<Node, ()> {
        if self.peek() != Some('{') {
            return Err(());
        }
        self.pos += 1;
        self.skip_ws();
        if self.peek() == Some('}') {
            self.pos += 1;
            return Ok(Node::Arr(Vec::new()));
        }
        let mut items = Vec::new();
        loop {
            self.skip_ws();
            let node = if self.peek() == Some('{') {
                self.parse_array()?
            } else {
                self.parse_element()?
            };
            items.push(node);
            self.skip_ws();
            match self.peek() {
                Some(',') => self.pos += 1,
                Some('}') => {
                    self.pos += 1;
                    break;
                }

                _ => return Err(()),
            }
        }
        Ok(Node::Arr(items))
    }

    fn parse_element(&mut self) -> Result<Node, ()> {
        self.skip_ws();
        if self.peek() == Some('"') {

            self.pos += 1;
            let mut buf = String::new();
            loop {
                match self.peek() {
                    None => return Err(()),
                    Some('\\') => {
                        self.pos += 1;
                        match self.peek() {
                            None => return Err(()),
                            Some(c) => {
                                buf.push(c);
                                self.pos += 1;
                            }
                        }
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
            Ok(Node::Elem(Some(buf)))
        } else {

            let start = self.pos;
            let mut buf = String::new();
            let mut sig = 0usize;
            let mut escaped_any = false;
            loop {
                match self.peek() {
                    None => return Err(()),
                    Some('\\') => {
                        self.pos += 1;
                        match self.peek() {
                            None => return Err(()),
                            Some(c) => {
                                buf.push(c);
                                self.pos += 1;
                                sig = buf.len();
                                escaped_any = true;
                            }
                        }
                    }
                    Some('"') => return Err(()),
                    Some(',') | Some('}') | Some('{') => break,
                    Some(c) if is_pg_space(c) => {
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

                return Err(());
            }
            buf.truncate(sig);
            if !escaped_any && buf.eq_ignore_ascii_case("NULL") {
                Ok(Node::Elem(None))
            } else {
                Ok(Node::Elem(Some(buf)))
            }
        }
    }
}

fn count_leaves(node: &Node) -> usize {
    match node {
        Node::Elem(_) => 1,
        Node::Arr(items) => items.iter().map(count_leaves).sum(),
    }
}

fn shape(node: &Node) -> Result<Vec<usize>, ()> {
    match node {
        Node::Elem(_) => Ok(Vec::new()),
        Node::Arr(items) => {
            if items.is_empty() {
                return Ok(vec![0]);
            }
            let first = shape(&items[0])?;
            for it in &items[1..] {
                if shape(it)? != first {
                    return Err(());
                }
            }
            let mut s = vec![items.len()];
            s.extend(first);
            Ok(s)
        }
    }
}

fn build_canon(node: &Node, elem_oid: u32) -> Result<String, PgError> {
    match node {
        Node::Elem(None) => Ok("NULL".to_string()),
        Node::Elem(Some(text)) => {
            let value = super::input(elem_oid, text)?;
            let canon = super::output(elem_oid, &value);
            Ok(requote(&canon))
        }
        Node::Arr(items) => {
            let parts: Result<Vec<String>, PgError> =
                items.iter().map(|i| build_canon(i, elem_oid)).collect();
            Ok(format!("{{{}}}", parts?.join(",")))
        }
    }
}

fn requote(s: &str) -> String {
    let needs = s.is_empty()
        || s.eq_ignore_ascii_case("NULL")
        || s.chars()
            .any(|c| matches!(c, '{' | '}' | ',' | '"' | '\\') || is_pg_space(c));
    if !needs {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

pub fn input(array_oid: u32, text: &str) -> Result<SqlValue, PgError> {
    let typ = type_name(array_oid);
    let malformed = || PgError::InvalidInputSyntax {
        typ,
        input: text.to_string(),
    };
    let elem_oid = element_oid(array_oid);

    let chars: Vec<char> = text.chars().collect();
    let mut p = Parser {
        chars: &chars,
        pos: 0,
    };
    p.skip_ws();

    if p.peek() != Some('{') {
        return Err(malformed());
    }
    let tree = p.parse_array().map_err(|_| malformed())?;
    p.skip_ws();
    if p.pos != chars.len() {
        return Err(malformed());
    }
    shape(&tree).map_err(|_| malformed())?;

    if count_leaves(&tree) == 0 {
        return Ok(SqlValue::Text("{}".to_string()));
    }
    let canon = build_canon(&tree, elem_oid)?;
    Ok(SqlValue::Text(canon))
}

pub fn output(_array_oid: u32, v: &SqlValue) -> String {
    match v {
        SqlValue::Text(s) => s.clone(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOOL_A: u32 = oid::BOOL_ARRAY;
    const INT4_A: u32 = oid::INT4_ARRAY;
    const INT8_A: u32 = oid::INT8_ARRAY;
    const TEXT_A: u32 = oid::TEXT_ARRAY;
    const FLOAT8_A: u32 = oid::FLOAT8_ARRAY;
    const NUM_A: u32 = oid::NUMERIC_ARRAY;

    fn t(v: SqlValue) -> String {
        match v {
            SqlValue::Text(s) => s,
            other => panic!("expected Text, got {other:?}"),
        }
    }

    fn canon(oid: u32, s: &str) -> String {
        t(input(oid, s).expect("valid array literal"))
    }

    #[test]
    fn one_dim_int() {
        assert_eq!(canon(INT4_A, "{1,2,3}"), "{1,2,3}");
        assert_eq!(canon(INT8_A, "{10,-20,30}"), "{10,-20,30}");
    }

    #[test]
    fn one_dim_text() {
        assert_eq!(canon(TEXT_A, "{a,b,c}"), "{a,b,c}");
        assert_eq!(canon(TEXT_A, "{foo,bar}"), "{foo,bar}");
    }

    #[test]
    fn one_dim_bool_canonicalises_via_element_codec() {

        assert_eq!(canon(BOOL_A, "{t,f,true,false}"), "{t,f,t,f}");
    }

    #[test]
    fn one_dim_float_and_numeric() {

        assert_eq!(canon(FLOAT8_A, "{1.5,2}"), "{1.5,2}");
        assert_eq!(canon(NUM_A, "{1.50,2}"), "{1.50,2}");
    }

    #[test]
    fn whitespace_between_elements_tolerated() {
        assert_eq!(canon(INT4_A, "{ 1 , 2 , 3 }"), "{1,2,3}");
        assert_eq!(canon(INT4_A, "\t{\n1,\r2 }"), "{1,2}");
    }

    #[test]
    fn empty_array() {
        assert_eq!(canon(INT4_A, "{}"), "{}");
        assert_eq!(canon(TEXT_A, "{ }"), "{}");
    }

    #[test]
    fn all_empty_multidim_collapses_to_empty() {

        assert_eq!(canon(TEXT_A, "{{},{}}"), "{}");
    }

    #[test]
    fn null_elements() {
        assert_eq!(canon(INT4_A, "{1,NULL,3}"), "{1,NULL,3}");

        assert_eq!(canon(INT4_A, "{1,null,3}"), "{1,NULL,3}");
        assert_eq!(canon(INT4_A, "{NULL,NULL}"), "{NULL,NULL}");
    }

    #[test]
    fn quoted_or_escaped_null_is_literal_text() {

        assert_eq!(
            canon(TEXT_A, r#"{null,n\ull,"null"}"#),
            r#"{NULL,"null","null"}"#
        );
    }

    #[test]
    fn quoted_text_with_special_chars() {

        assert_eq!(canon(TEXT_A, r#"{"a,b","c"}"#), r#"{"a,b",c}"#);
        assert_eq!(canon(TEXT_A, r#"{"{}"}"#), r#"{"{}"}"#);
        assert_eq!(canon(TEXT_A, r#"{"a b"}"#), r#"{"a b"}"#);
    }

    #[test]
    fn empty_text_element_requoted() {
        assert_eq!(canon(TEXT_A, r#"{""}"#), r#"{""}"#);
    }

    #[test]
    fn backslash_and_quote_escapes() {

        assert_eq!(canon(TEXT_A, r#"{ ab\c , "ab\"c" }"#), r#"{abc,"ab\"c"}"#);
    }

    #[test]
    fn nested_multidim() {
        assert_eq!(canon(INT4_A, "{{1,2},{3,4}}"), "{{1,2},{3,4}}");
        assert_eq!(canon(INT4_A, "{ {1, 2}, {3, 4} }"), "{{1,2},{3,4}}");
    }

    #[test]
    fn three_dim_text() {
        let s = "{{{1,2,3,4},{2,3,4,5}},{{3,4,5,6},{4,5,6,7}}}";
        assert_eq!(canon(TEXT_A, s), s);
    }

    #[test]
    fn nested_quoted_element() {

        assert_eq!(canon(TEXT_A, r#"{ { "," } , { 3 } }"#), r#"{{","},{3}}"#);
    }

    #[test]
    fn element_validation_failure_surfaces_element_codec_error() {

        let e = input(INT4_A, "{1,34.5,3}").unwrap_err();
        assert_eq!(
            e,
            PgError::InvalidInputSyntax {
                typ: "integer",
                input: "34.5".into()
            }
        );
        assert_eq!(
            e.message(),
            "invalid input syntax for type integer: \"34.5\""
        );
    }

    #[test]
    fn element_out_of_range_surfaces_element_codec_error() {

        let e = input(oid::INT2_ARRAY, "{1,99999}").unwrap_err();
        assert_eq!(
            e,
            PgError::OutOfRange {
                typ: "smallint",
                input: "99999".into()
            }
        );
    }

    fn assert_malformed(oid: u32, s: &str) {
        let e = input(oid, s).unwrap_err();
        assert_eq!(
            e,
            PgError::InvalidInputSyntax {
                typ: type_name(oid),
                input: s.to_string()
            },
            "expected malformed for {s:?}"
        );
    }

    #[test]
    fn malformed_ragged_multidim() {
        assert_malformed(TEXT_A, "{{1,{2}},{2,3}}");
        assert_malformed(INT4_A, "{{1},{2,3}}");
        assert_malformed(INT4_A, "{{1},{{2}}}");
    }

    #[test]
    fn malformed_unbalanced_and_junk() {
        assert_malformed(INT4_A, "{1,2");
        assert_malformed(TEXT_A, "}{");
        assert_malformed(TEXT_A, "{}}");
        assert_malformed(TEXT_A, "{ }}");
    }

    #[test]
    fn malformed_bad_quoting() {
        assert_malformed(TEXT_A, r#"{a"b"}"#);
        assert_malformed(TEXT_A, r#"{"a"b}"#);
        assert_malformed(TEXT_A, "{foo{}}");
    }

    #[test]
    fn malformed_empty_element() {
        assert_malformed(TEXT_A, "{foo,,bar}");
    }

    #[test]
    fn deferred_dimension_syntax_is_rejected() {

        assert_malformed(INT4_A, "[1:3]={1,2,3}");
        assert_malformed(INT4_A, "[2]={1,7}");
    }

    #[test]
    fn not_opening_with_brace_is_malformed() {
        assert_malformed(INT4_A, "1,2,3");
        assert_malformed(INT4_A, "");
    }

    #[test]
    fn output_returns_stored_text() {
        assert_eq!(output(INT4_A, &SqlValue::Text("{1,2,3}".into())), "{1,2,3}");
    }

    #[test]
    fn output_non_text_is_empty() {
        assert_eq!(output(INT4_A, &SqlValue::Null), "");
        assert_eq!(output(INT4_A, &SqlValue::Int(3)), "");
    }

    #[test]
    fn round_trips() {
        for (oid, s) in [
            (INT4_A, "{1,2,3}"),
            (INT4_A, "{{1,2},{3,4}}"),
            (INT4_A, "{1,NULL,3}"),
            (TEXT_A, "{}"),
            (TEXT_A, r#"{"a,b",c}"#),
            (BOOL_A, "{t,f}"),
        ] {
            let once = canon(oid, s);

            assert_eq!(canon(oid, &once), once, "not a fixed point: {s:?}");

            let v = input(oid, s).unwrap();
            assert_eq!(output(oid, &v), once);
        }
    }
}
