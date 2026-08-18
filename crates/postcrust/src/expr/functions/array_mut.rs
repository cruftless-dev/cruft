
use crate::types::PgError;
use sql_core::SqlValue;

const FAMILY: &[&str] = &[
    "array_append",
    "array_prepend",
    "array_cat",
    "array_position",
    "array_positions",
    "array_remove",
    "array_replace",
];

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    if !FAMILY.contains(&name) {
        return None;
    }
    Some(dispatch(name, args))
}

fn wrong(name: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("function {name}(...) does not exist"),
    }
}

fn dispatch(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    match name {
        "array_append" => {
            let [arr, elem] = two(name, args)?;

            let mut items = parse_array_or_empty(name, arr)?;
            items.push(render_elem(name, elem)?);
            Ok(serialize(&items))
        }
        "array_prepend" => {

            let [elem, arr] = two(name, args)?;
            let mut items = parse_array_or_empty(name, arr)?;
            items.insert(0, render_elem(name, elem)?);
            Ok(serialize(&items))
        }
        "array_cat" => {
            let [a, b] = two(name, args)?;
            let mut items = parse_array_or_empty(name, a)?;
            items.extend(parse_array_or_empty(name, b)?);
            Ok(serialize(&items))
        }
        "array_position" => {
            let [arr, elem] = two(name, args)?;

            let items = match parse_array_strict(name, arr)? {
                None => return Ok(SqlValue::Null),
                Some(v) => v,
            };
            let needle = render_elem(name, elem)?;
            match items.iter().position(|it| elem_eq(it, &needle)) {
                Some(i) => Ok(SqlValue::Int((i as i64) + 1)),
                None => Ok(SqlValue::Null),
            }
        }
        "array_positions" => {
            let [arr, elem] = two(name, args)?;
            let items = match parse_array_strict(name, arr)? {
                None => return Ok(SqlValue::Null),
                Some(v) => v,
            };
            let needle = render_elem(name, elem)?;
            let hits: Vec<Option<String>> = items
                .iter()
                .enumerate()
                .filter(|(_, it)| elem_eq(it, &needle))
                .map(|(i, _)| Some((i + 1).to_string()))
                .collect();
            Ok(serialize(&hits))
        }
        "array_remove" => {
            let [arr, elem] = two(name, args)?;
            let items = match parse_array_strict(name, arr)? {
                None => return Ok(SqlValue::Null),
                Some(v) => v,
            };
            let needle = render_elem(name, elem)?;
            let kept: Vec<Option<String>> = items
                .into_iter()
                .filter(|it| !elem_eq(it, &needle))
                .collect();
            Ok(serialize(&kept))
        }
        "array_replace" => {
            let [arr, from, to] = three(name, args)?;
            let items = match parse_array_strict(name, arr)? {
                None => return Ok(SqlValue::Null),
                Some(v) => v,
            };
            let from = render_elem(name, from)?;
            let to = render_elem(name, to)?;
            let out: Vec<Option<String>> = items
                .into_iter()
                .map(|it| if elem_eq(&it, &from) { to.clone() } else { it })
                .collect();
            Ok(serialize(&out))
        }
        _ => Err(wrong(name)),
    }
}

fn two<'a>(name: &str, args: &'a [SqlValue]) -> Result<[&'a SqlValue; 2], PgError> {
    match args {
        [a, b] => Ok([a, b]),
        _ => Err(wrong(name)),
    }
}

fn three<'a>(name: &str, args: &'a [SqlValue]) -> Result<[&'a SqlValue; 3], PgError> {
    match args {
        [a, b, c] => Ok([a, b, c]),
        _ => Err(wrong(name)),
    }
}

fn render_elem(name: &str, v: &SqlValue) -> Result<Option<String>, PgError> {
    match v {
        SqlValue::Null => Ok(None),
        SqlValue::Int(i) => Ok(Some(i.to_string())),
        SqlValue::Real(f) => Ok(Some(fmt_real(*f))),
        SqlValue::Text(s) => Ok(Some(s.clone())),
        SqlValue::Blob(_) => Err(wrong(name)),
    }
}

fn fmt_real(f: f64) -> String {
    if f == f.trunc() && f.is_finite() {
        format!("{}", f as i64)
    } else {
        format!("{f}")
    }
}

fn elem_eq(a: &Option<String>, b: &Option<String>) -> bool {
    match (a, b) {
        (None, None) => true,
        (None, Some(_)) | (Some(_), None) => false,
        (Some(x), Some(y)) => {
            if x == y {
                return true;
            }
            match (x.trim().parse::<f64>(), y.trim().parse::<f64>()) {
                (Ok(nx), Ok(ny)) => nx == ny,
                _ => false,
            }
        }
    }
}

fn parse_array_strict(name: &str, v: &SqlValue) -> Result<Option<Vec<Option<String>>>, PgError> {
    match v {
        SqlValue::Null => Ok(None),
        SqlValue::Text(s) => parse_1d(s).map(Some).map_err(|_| wrong(name)),
        _ => Err(wrong(name)),
    }
}

fn parse_array_or_empty(name: &str, v: &SqlValue) -> Result<Vec<Option<String>>, PgError> {
    match v {
        SqlValue::Null => Ok(Vec::new()),
        SqlValue::Text(s) => parse_1d(s).map_err(|_| wrong(name)),
        _ => Err(wrong(name)),
    }
}

fn is_pg_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r')
}

struct P1<'a> {
    chars: &'a [char],
    pos: usize,
}

impl<'a> P1<'a> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }
    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(c) if is_pg_space(c)) {
            self.pos += 1;
        }
    }

    fn element(&mut self) -> Result<Option<String>, ()> {
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
            Ok(Some(buf))
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
                Ok(None)
            } else {
                Ok(Some(buf))
            }
        }
    }
}

fn parse_1d(text: &str) -> Result<Vec<Option<String>>, ()> {
    let chars: Vec<char> = text.chars().collect();
    let mut p = P1 {
        chars: &chars,
        pos: 0,
    };
    p.skip_ws();
    if p.peek() != Some('{') {
        return Err(());
    }
    p.pos += 1;
    p.skip_ws();
    let mut items = Vec::new();
    if p.peek() == Some('}') {
        p.pos += 1;
    } else {
        loop {
            p.skip_ws();
            if p.peek() == Some('{') {
                return Err(());
            }
            items.push(p.element()?);
            p.skip_ws();
            match p.peek() {
                Some(',') => p.pos += 1,
                Some('}') => {
                    p.pos += 1;
                    break;
                }
                _ => return Err(()),
            }
        }
    }
    p.skip_ws();
    if p.pos != chars.len() {
        return Err(());
    }
    Ok(items)
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

fn serialize(items: &[Option<String>]) -> SqlValue {
    let body = items
        .iter()
        .map(|it| match it {
            None => "NULL".to_string(),
            Some(s) => requote(s),
        })
        .collect::<Vec<_>>()
        .join(",");
    SqlValue::Text(format!("{{{body}}}"))
}

#[cfg(test)]
mod tests {
    use super::call;
    use sql_core::SqlValue;

    fn txt(s: &str) -> SqlValue {
        SqlValue::Text(s.to_string())
    }
    fn out(name: &str, args: &[SqlValue]) -> SqlValue {
        call(name, args).expect("claimed").expect("ok")
    }
    fn out_txt(name: &str, args: &[SqlValue]) -> String {
        match out(name, args) {
            SqlValue::Text(s) => s,
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn unclaimed_returns_none() {
        assert!(call("array_length", &[txt("{1,2}")]).is_none());
        assert!(call("upper", &[txt("x")]).is_none());
    }

    #[test]
    fn append_prepend_cat() {
        assert_eq!(
            out_txt("array_append", &[txt("{42}"), SqlValue::Int(6)]),
            "{42,6}"
        );
        assert_eq!(
            out_txt("array_append", &[txt("{1,2}"), SqlValue::Int(3)]),
            "{1,2,3}"
        );

        assert_eq!(
            out_txt("array_prepend", &[SqlValue::Int(6), txt("{42}")]),
            "{6,42}"
        );
        assert_eq!(
            out_txt("array_prepend", &[SqlValue::Int(0), txt("{1,2}")]),
            "{0,1,2}"
        );
        assert_eq!(
            out_txt("array_cat", &[txt("{1,2}"), txt("{3,4}")]),
            "{1,2,3,4}"
        );
        assert_eq!(out_txt("array_cat", &[txt("{}"), txt("{3,4}")]), "{3,4}");
    }

    #[test]
    fn append_null_element_and_null_array() {

        assert_eq!(
            out_txt("array_append", &[txt("{1,2}"), SqlValue::Null]),
            "{1,2,NULL}"
        );

        assert_eq!(
            out_txt("array_append", &[SqlValue::Null, SqlValue::Int(3)]),
            "{3}"
        );
        assert_eq!(
            out_txt("array_append", &[SqlValue::Null, SqlValue::Null]),
            "{NULL}"
        );
        assert_eq!(
            out_txt("array_prepend", &[SqlValue::Int(1), SqlValue::Null]),
            "{1}"
        );
        assert_eq!(
            out_txt("array_cat", &[SqlValue::Null, txt("{1,2}")]),
            "{1,2}"
        );
        assert_eq!(
            out_txt("array_cat", &[txt("{1,2}"), SqlValue::Null]),
            "{1,2}"
        );
    }

    #[test]
    fn position_found_and_absent() {
        assert_eq!(
            out("array_position", &[txt("{1,2,3,4,5}"), SqlValue::Int(4)]),
            SqlValue::Int(4)
        );
        assert_eq!(
            out("array_position", &[txt("{5,3,4,2,1}"), SqlValue::Int(4)]),
            SqlValue::Int(3)
        );
        assert_eq!(
            out(
                "array_position",
                &[txt("{sun,mon,tue,wed,thu,fri,sat}"), txt("mon")]
            ),
            SqlValue::Int(2)
        );
        assert_eq!(
            out(
                "array_position",
                &[txt("{sun,mon,tue,wed,thu,fri,sat}"), txt("sat")]
            ),
            SqlValue::Int(7)
        );

        assert_eq!(
            out("array_position", &[txt("{1,2,3}"), SqlValue::Int(9)]),
            SqlValue::Null
        );
    }

    #[test]
    fn position_null_semantics() {

        assert_eq!(
            out(
                "array_position",
                &[txt("{sun,mon,tue,wed,thu,fri,sat}"), SqlValue::Null]
            ),
            SqlValue::Null
        );

        assert_eq!(
            out(
                "array_position",
                &[txt("{sun,mon,tue,wed,thu,NULL,fri,sat}"), SqlValue::Null]
            ),
            SqlValue::Int(6)
        );
        assert_eq!(
            out(
                "array_position",
                &[txt("{sun,mon,tue,wed,thu,NULL,fri,sat}"), txt("sat")]
            ),
            SqlValue::Int(8)
        );

        assert_eq!(
            out("array_position", &[SqlValue::Null, SqlValue::Int(1)]),
            SqlValue::Null
        );
    }

    #[test]
    fn positions_all_matches() {
        assert_eq!(
            out_txt(
                "array_positions",
                &[txt("{1,2,3,4,5,6,1,2,3,4,5,6}"), SqlValue::Int(4)]
            ),
            "{4,10}"
        );

        assert_eq!(
            out_txt(
                "array_positions",
                &[txt("{1,2,3,4,5,6,1,2,3,4,5,6}"), SqlValue::Null]
            ),
            "{}"
        );

        assert_eq!(
            out_txt(
                "array_positions",
                &[txt("{1,2,3,NULL,5,6,1,2,3,NULL,5,6}"), SqlValue::Null]
            ),
            "{4,10}"
        );

        assert_eq!(
            out("array_positions", &[SqlValue::Null, SqlValue::Int(10)]),
            SqlValue::Null
        );
    }

    #[test]
    fn remove_drops_all_matches() {
        assert_eq!(
            out_txt("array_remove", &[txt("{1,2,2,3}"), SqlValue::Int(2)]),
            "{1,3}"
        );
        assert_eq!(
            out_txt("array_remove", &[txt("{1,2,2,3}"), SqlValue::Int(5)]),
            "{1,2,2,3}"
        );

        assert_eq!(
            out_txt("array_remove", &[txt("{1,NULL,NULL,3}"), SqlValue::Null]),
            "{1,3}"
        );
        assert_eq!(
            out_txt("array_remove", &[txt("{A,CC,D,C,RR}"), txt("RR")]),
            "{A,CC,D,C}"
        );

        assert_eq!(
            out_txt("array_remove", &[txt("{1.0,2.1,3.3}"), SqlValue::Int(1)]),
            "{2.1,3.3}"
        );

        assert_eq!(out_txt("array_remove", &[txt("{X,X,X}"), txt("X")]), "{}");

        assert_eq!(
            out("array_remove", &[SqlValue::Null, SqlValue::Int(1)]),
            SqlValue::Null
        );
    }

    #[test]
    fn replace_swaps_all_matches() {
        assert_eq!(
            out_txt(
                "array_replace",
                &[txt("{1,2,5,4}"), SqlValue::Int(5), SqlValue::Int(3)]
            ),
            "{1,2,3,4}"
        );

        assert_eq!(
            out_txt(
                "array_replace",
                &[txt("{1,2,5,4}"), SqlValue::Int(5), SqlValue::Null]
            ),
            "{1,2,NULL,4}"
        );

        assert_eq!(
            out_txt(
                "array_replace",
                &[txt("{1,2,NULL,4,NULL}"), SqlValue::Null, SqlValue::Int(5)]
            ),
            "{1,2,5,4,5}"
        );
        assert_eq!(
            out_txt("array_replace", &[txt("{A,B,DD,B}"), txt("B"), txt("CC")]),
            "{A,CC,DD,CC}"
        );

        assert_eq!(
            out_txt(
                "array_replace",
                &[txt("{1,NULL,3}"), SqlValue::Null, SqlValue::Null]
            ),
            "{1,NULL,3}"
        );
        assert_eq!(
            out_txt(
                "array_replace",
                &[txt("{AB,NULL,CDE}"), SqlValue::Null, txt("12")]
            ),
            "{AB,12,CDE}"
        );

        assert_eq!(
            out(
                "array_replace",
                &[SqlValue::Null, SqlValue::Int(1), SqlValue::Int(2)]
            ),
            SqlValue::Null
        );
    }

    #[test]
    fn requoting_on_reemission() {

        assert_eq!(
            out_txt("array_append", &[txt("{a}"), txt("b,c")]),
            r#"{a,"b,c"}"#
        );

        assert_eq!(
            out_txt("array_append", &[txt("{a}"), txt("null")]),
            r#"{a,"null"}"#
        );

        assert_eq!(
            out_txt("array_append", &[txt("{}"), txt(r#"a"b"#)]),
            r#"{"a\"b"}"#
        );

        assert_eq!(
            out_txt("array_cat", &[txt(r#"{"a,b",c}"#), txt("{d}")]),
            r#"{"a,b",c,d}"#
        );

        assert_eq!(out_txt("array_append", &[txt("{}"), txt("")]), r#"{""}"#);
    }

    #[test]
    fn multidim_and_malformed_error() {
        for v in [
            call(
                "array_remove",
                &[txt("{{1,2,2},{1,4,3}}"), SqlValue::Int(2)],
            ),
            call("array_position", &[txt("{{1,2},{3,4}}"), SqlValue::Int(3)]),
            call("array_append", &[txt("{1,2"), SqlValue::Int(3)]),
        ] {
            let e = v.expect("claimed").unwrap_err();
            match e {
                crate::types::PgError::InvalidInputSyntax { typ, .. } => {
                    assert_eq!(typ, "expression")
                }
                other => panic!("unexpected {other:?}"),
            }
        }
    }

    #[test]
    fn wrong_arity_and_type() {
        assert!(call("array_append", &[txt("{1}")]).unwrap().is_err());
        assert!(call("array_replace", &[txt("{1}"), SqlValue::Int(1)])
            .unwrap()
            .is_err());

        assert!(
            call("array_position", &[SqlValue::Int(5), SqlValue::Int(1)])
                .unwrap()
                .is_err()
        );

        assert!(call("array_append", &[txt("{1}"), SqlValue::Blob(vec![1])])
            .unwrap()
            .is_err());
    }
}
