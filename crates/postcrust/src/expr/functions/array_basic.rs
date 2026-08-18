
use crate::types::PgError;
use sql_core::SqlValue;

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    match name {
        "array_length" => Some(array_length(name, args)),
        "array_upper" => Some(array_bound(name, args, true)),
        "array_lower" => Some(array_bound(name, args, false)),
        "cardinality" => Some(cardinality(name, args)),
        "array_ndims" => Some(array_ndims(name, args)),
        "array_dims" => Some(array_dims(name, args)),
        "array_to_string" => Some(array_to_string(name, args)),
        "string_to_array" => Some(string_to_array(name, args)),
        _ => None,
    }
}

fn does_not_exist(name: &str) -> Result<SqlValue, PgError> {
    Err(PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("function {name}(...) does not exist"),
    })
}

enum Node {
    Elem(Option<String>),
    Arr(Vec<Node>),
}

fn is_pg_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r')
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

fn parse_array(text: &str) -> Result<Node, ()> {
    let chars: Vec<char> = text.chars().collect();
    let mut p = Parser {
        chars: &chars,
        pos: 0,
    };
    p.skip_ws();
    if p.peek() != Some('{') {
        return Err(());
    }
    let tree = p.parse_array()?;
    p.skip_ws();
    if p.pos != chars.len() {
        return Err(());
    }
    shape(&tree)?;
    Ok(tree)
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

fn count_leaves(node: &Node) -> usize {
    match node {
        Node::Elem(_) => 1,
        Node::Arr(items) => items.iter().map(count_leaves).sum(),
    }
}

fn collect_leaves(node: &Node, out: &mut Vec<Option<String>>) {
    match node {
        Node::Elem(e) => out.push(e.clone()),
        Node::Arr(items) => {
            for it in items {
                collect_leaves(it, out);
            }
        }
    }
}

fn dimensions(tree: &Node) -> Vec<usize> {
    if count_leaves(tree) == 0 {
        Vec::new()
    } else {
        shape(tree).unwrap_or_default()
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

fn build_canon(elems: &[Option<String>]) -> String {
    if elems.is_empty() {
        return "{}".to_string();
    }
    let parts: Vec<String> = elems
        .iter()
        .map(|e| match e {
            None => "NULL".to_string(),
            Some(s) => requote(s),
        })
        .collect();
    format!("{{{}}}", parts.join(","))
}

enum ArrArg {
    Null,
    Tree(Node),
    Bad,
}

fn array_arg(v: &SqlValue) -> ArrArg {
    match v {
        SqlValue::Null => ArrArg::Null,
        SqlValue::Text(s) => match parse_array(s) {
            Ok(t) => ArrArg::Tree(t),
            Err(()) => ArrArg::Bad,
        },
        _ => ArrArg::Bad,
    }
}

fn array_length(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() != 2 {
        return does_not_exist(name);
    }

    let dim = match &args[1] {
        SqlValue::Null => return Ok(SqlValue::Null),
        SqlValue::Int(d) => *d,
        _ => return does_not_exist(name),
    };
    let tree = match array_arg(&args[0]) {
        ArrArg::Null => return Ok(SqlValue::Null),
        ArrArg::Bad => return does_not_exist(name),
        ArrArg::Tree(t) => t,
    };
    let dims = dimensions(&tree);
    if dim < 1 || (dim as usize) > dims.len() {
        return Ok(SqlValue::Null);
    }
    Ok(SqlValue::Int(dims[(dim as usize) - 1] as i64))
}

fn array_bound(name: &str, args: &[SqlValue], upper: bool) -> Result<SqlValue, PgError> {
    if args.len() != 2 {
        return does_not_exist(name);
    }
    let dim = match &args[1] {
        SqlValue::Null => return Ok(SqlValue::Null),
        SqlValue::Int(d) => *d,
        _ => return does_not_exist(name),
    };
    let tree = match array_arg(&args[0]) {
        ArrArg::Null => return Ok(SqlValue::Null),
        ArrArg::Bad => return does_not_exist(name),
        ArrArg::Tree(t) => t,
    };
    let dims = dimensions(&tree);
    if dim < 1 || (dim as usize) > dims.len() {
        return Ok(SqlValue::Null);
    }
    Ok(SqlValue::Int(if upper {
        dims[(dim as usize) - 1] as i64
    } else {
        1
    }))
}

fn cardinality(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() != 1 {
        return does_not_exist(name);
    }
    match array_arg(&args[0]) {
        ArrArg::Null => Ok(SqlValue::Null),
        ArrArg::Bad => does_not_exist(name),
        ArrArg::Tree(t) => Ok(SqlValue::Int(count_leaves(&t) as i64)),
    }
}

fn array_ndims(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() != 1 {
        return does_not_exist(name);
    }
    let tree = match array_arg(&args[0]) {
        ArrArg::Null => return Ok(SqlValue::Null),
        ArrArg::Bad => return does_not_exist(name),
        ArrArg::Tree(t) => t,
    };
    let dims = dimensions(&tree);
    if dims.is_empty() {
        Ok(SqlValue::Null)
    } else {
        Ok(SqlValue::Int(dims.len() as i64))
    }
}

fn array_dims(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() != 1 {
        return does_not_exist(name);
    }
    let tree = match array_arg(&args[0]) {
        ArrArg::Null => return Ok(SqlValue::Null),
        ArrArg::Bad => return does_not_exist(name),
        ArrArg::Tree(t) => t,
    };
    let dims = dimensions(&tree);
    if dims.is_empty() {
        return Ok(SqlValue::Null);
    }
    let mut s = String::new();
    for d in dims {
        s.push_str(&format!("[1:{d}]"));
    }
    Ok(SqlValue::Text(s))
}

fn array_to_string(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() != 2 && args.len() != 3 {
        return does_not_exist(name);
    }

    let sep = match &args[1] {
        SqlValue::Null => return Ok(SqlValue::Null),
        SqlValue::Text(s) => s.clone(),
        _ => return does_not_exist(name),
    };

    let null_str: Option<String> = match args.get(2) {
        None | Some(SqlValue::Null) => None,
        Some(SqlValue::Text(s)) => Some(s.clone()),
        Some(_) => return does_not_exist(name),
    };
    let tree = match array_arg(&args[0]) {
        ArrArg::Null => return Ok(SqlValue::Null),
        ArrArg::Bad => return does_not_exist(name),
        ArrArg::Tree(t) => t,
    };
    let mut leaves = Vec::new();
    collect_leaves(&tree, &mut leaves);
    let mut pieces: Vec<String> = Vec::new();
    for leaf in leaves {
        match leaf {
            Some(s) => pieces.push(s),
            None => {
                if let Some(ns) = &null_str {
                    pieces.push(ns.clone());
                }

            }
        }
    }
    Ok(SqlValue::Text(pieces.join(&sep)))
}

fn string_to_array(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() != 2 && args.len() != 3 {
        return does_not_exist(name);
    }

    let text = match &args[0] {
        SqlValue::Null => return Ok(SqlValue::Null),
        SqlValue::Text(s) => s.clone(),
        _ => return does_not_exist(name),
    };

    let sep: Option<String> = match &args[1] {
        SqlValue::Null => None,
        SqlValue::Text(s) => Some(s.clone()),
        _ => return does_not_exist(name),
    };

    let null_str: Option<String> = match args.get(2) {
        None | Some(SqlValue::Null) => None,
        Some(SqlValue::Text(s)) => Some(s.clone()),
        Some(_) => return does_not_exist(name),
    };

    let tokens: Vec<String> = if text.is_empty() {
        Vec::new()
    } else {
        match &sep {
            None => text.chars().map(|c| c.to_string()).collect(),
            Some(s) if s.is_empty() => vec![text.clone()],
            Some(s) => text.split(s.as_str()).map(|p| p.to_string()).collect(),
        }
    };

    let elems: Vec<Option<String>> = tokens
        .into_iter()
        .map(|tok| match &null_str {
            Some(ns) if *ns == tok => None,
            _ => Some(tok),
        })
        .collect();

    Ok(SqlValue::Text(build_canon(&elems)))
}

#[cfg(test)]
mod tests {
    use super::call;
    use sql_core::SqlValue;

    fn txt(s: &str) -> SqlValue {
        SqlValue::Text(s.into())
    }

    fn ok_int(name: &str, args: &[SqlValue]) -> i64 {
        match call(name, args) {
            Some(Ok(SqlValue::Int(i))) => i,
            other => panic!("expected Int, got {other:?}"),
        }
    }

    fn ok_text(name: &str, args: &[SqlValue]) -> String {
        match call(name, args) {
            Some(Ok(SqlValue::Text(s))) => s,
            other => panic!("expected Text, got {other:?}"),
        }
    }

    fn is_null(name: &str, args: &[SqlValue]) -> bool {
        matches!(call(name, args), Some(Ok(SqlValue::Null)))
    }

    #[test]
    fn array_length_1d() {
        assert_eq!(
            ok_int("array_length", &[txt("{1,2,3}"), SqlValue::Int(1)]),
            3
        );
    }

    #[test]
    fn array_length_multidim() {
        let a = txt("{{1,2,3},{4,5,6}}");
        assert_eq!(ok_int("array_length", &[a.clone(), SqlValue::Int(1)]), 2);
        assert_eq!(ok_int("array_length", &[a.clone(), SqlValue::Int(2)]), 3);

        assert!(is_null("array_length", &[a.clone(), SqlValue::Int(0)]));
        assert!(is_null("array_length", &[a, SqlValue::Int(3)]));
    }

    #[test]
    fn array_length_empty_is_null() {
        assert!(is_null("array_length", &[txt("{}"), SqlValue::Int(1)]));
    }

    #[test]
    fn array_length_null_propagation() {
        assert!(is_null("array_length", &[SqlValue::Null, SqlValue::Int(1)]));
        assert!(is_null("array_length", &[txt("{1,2}"), SqlValue::Null]));
    }

    #[test]
    fn cardinality_cases() {
        assert_eq!(ok_int("cardinality", &[txt("{1,2,3}")]), 3);
        assert_eq!(ok_int("cardinality", &[txt("{{1,2}}")]), 2);
        assert_eq!(ok_int("cardinality", &[txt("{{1,2},{3,4},{5,6}}")]), 6);
        assert_eq!(
            ok_int("cardinality", &[txt("{{{1,9},{5,6}},{{2,3},{3,4}}}")]),
            8
        );
        assert_eq!(ok_int("cardinality", &[txt("{}")]), 0);
        assert!(is_null("cardinality", &[SqlValue::Null]));
    }

    #[test]
    fn array_ndims_cases() {
        assert_eq!(ok_int("array_ndims", &[txt("{1,2,3}")]), 1);
        assert_eq!(ok_int("array_ndims", &[txt("{{1,2},{3,4}}")]), 2);
        assert_eq!(
            ok_int("array_ndims", &[txt("{{{1,2},{3,4}},{{5,6},{7,8}}}")]),
            3
        );
        assert!(is_null("array_ndims", &[txt("{}")]));
        assert!(is_null("array_ndims", &[SqlValue::Null]));
    }

    #[test]
    fn array_dims_cases() {
        assert_eq!(ok_text("array_dims", &[txt("{1,2,3}")]), "[1:3]");
        assert_eq!(ok_text("array_dims", &[txt("{{1,2},{3,4}}")]), "[1:2][1:2]");
        assert_eq!(
            ok_text("array_dims", &[txt("{{1,2,3},{4,5,6}}")]),
            "[1:2][1:3]"
        );
        assert!(is_null("array_dims", &[txt("{}")]));
        assert!(is_null("array_dims", &[SqlValue::Null]));
    }

    #[test]
    fn array_to_string_basic() {
        assert_eq!(
            ok_text("array_to_string", &[txt("{1,2,3,4,NULL,6}"), txt(",")]),
            "1,2,3,4,6"
        );
        assert_eq!(
            ok_text(
                "array_to_string",
                &[txt("{1,2,3,4,NULL,6}"), txt(","), txt("*")]
            ),
            "1,2,3,4,*,6"
        );

        assert_eq!(
            ok_text(
                "array_to_string",
                &[txt("{1,2,3,4,NULL,6}"), txt(","), SqlValue::Null]
            ),
            "1,2,3,4,6"
        );
    }

    #[test]
    fn array_to_string_empty_and_null() {
        assert_eq!(ok_text("array_to_string", &[txt("{}"), txt(",")]), "");
        assert!(is_null("array_to_string", &[SqlValue::Null, txt(",")]));

        assert!(is_null(
            "array_to_string",
            &[txt("{1,2,3}"), SqlValue::Null]
        ));
    }

    #[test]
    fn array_to_string_multidim_flattens() {
        assert_eq!(
            ok_text("array_to_string", &[txt("{{1,2,3},{4,5,6}}"), txt(",")]),
            "1,2,3,4,5,6"
        );
    }

    #[test]
    fn string_to_array_basic() {
        assert_eq!(
            ok_text("string_to_array", &[txt("1|2|3"), txt("|")]),
            "{1,2,3}"
        );
        assert_eq!(
            ok_text("string_to_array", &[txt("1|2|3|"), txt("|")]),
            "{1,2,3,\"\"}"
        );
        assert_eq!(
            ok_text("string_to_array", &[txt("1||2|3||"), txt("||")]),
            "{1,2|3,\"\"}"
        );
    }

    #[test]
    fn string_to_array_empty_sep_and_input() {

        assert_eq!(
            ok_text("string_to_array", &[txt("1|2|3"), txt("")]),
            "{1|2|3}"
        );
        assert_eq!(ok_text("string_to_array", &[txt("abc"), txt("")]), "{abc}");

        assert_eq!(ok_text("string_to_array", &[txt(""), txt("|")]), "{}");
    }

    #[test]
    fn string_to_array_null_sep_splits_chars() {
        assert_eq!(
            ok_text("string_to_array", &[txt("1|2|3"), SqlValue::Null]),
            "{1,|,2,|,3}"
        );
    }

    #[test]
    fn string_to_array_null_string_mapping() {
        assert_eq!(
            ok_text("string_to_array", &[txt("abc"), txt(""), txt("abc")]),
            "{NULL}"
        );
        assert_eq!(
            ok_text("string_to_array", &[txt("abc"), txt(","), txt("abc")]),
            "{NULL}"
        );

        assert_eq!(
            ok_text("string_to_array", &[txt("1,2,3,4,,6"), txt(","), txt("")]),
            "{1,2,3,4,NULL,6}"
        );
        assert_eq!(
            ok_text("string_to_array", &[txt("1,2,3,4,*,6"), txt(","), txt("*")]),
            "{1,2,3,4,NULL,6}"
        );

        assert_eq!(
            ok_text("string_to_array", &[txt("1,2,3,4,,6"), txt(",")]),
            "{1,2,3,4,\"\",6}"
        );
    }

    #[test]
    fn string_to_array_null_input() {
        assert!(is_null("string_to_array", &[SqlValue::Null, txt("|")]));
    }

    #[test]
    fn round_trip_string_array() {
        let arr = ok_text("string_to_array", &[txt("1|2|3"), txt("|")]);
        assert_eq!(ok_text("array_to_string", &[txt(&arr), txt("|")]), "1|2|3");
    }

    #[test]
    fn wrong_arity_is_does_not_exist() {
        assert!(matches!(
            call("cardinality", &[txt("{1}"), txt("{2}")]),
            Some(Err(_))
        ));
        assert!(matches!(call("array_length", &[txt("{1}")]), Some(Err(_))));
        assert!(matches!(
            call("array_to_string", &[txt("{1}")]),
            Some(Err(_))
        ));
    }

    #[test]
    fn unclaimed_name_returns_none() {
        assert!(call("array_frobnicate", &[txt("{1,2}")]).is_none());
        assert!(call("substr", &[txt("abc")]).is_none());
    }

    use crate::types::PgError;

    #[test]
    fn wrong_type_array_arg() {
        match call("cardinality", &[SqlValue::Int(5)]) {
            Some(Err(PgError::InvalidInputSyntax { typ, .. })) => assert_eq!(typ, "expression"),
            other => panic!("expected InvalidInputSyntax, got {other:?}"),
        }
    }
}
