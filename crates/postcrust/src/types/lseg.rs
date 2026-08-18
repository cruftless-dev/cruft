
use super::PgError;
use sql_core::SqlValue;

fn err(input: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: super::type_name(super::oid::LSEG),
        input: input.to_string(),
    }
}

struct Scanner<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Scanner<'a> {
    fn new(s: &'a str) -> Self {
        Scanner {
            s: s.as_bytes(),
            i: 0,
        }
    }

    fn skip_ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn eat(&mut self, b: u8) -> bool {
        if self.peek() == Some(b) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn parse_float(&mut self) -> Option<f64> {
        self.skip_ws();
        let start = self.i;
        while let Some(c) = self.peek() {
            if c.is_ascii_whitespace() || matches!(c, b',' | b'(' | b')' | b'[' | b']') {
                break;
            }
            self.i += 1;
        }
        let tok = std::str::from_utf8(&self.s[start..self.i]).ok()?;
        if tok.is_empty() {
            return None;
        }
        tok.parse::<f64>().ok()
    }

    fn parse_point(&mut self) -> Option<(f64, f64)> {
        self.skip_ws();
        let had_paren = self.eat(b'(');
        let x = self.parse_float()?;
        self.skip_ws();
        if !self.eat(b',') {
            return None;
        }
        let y = self.parse_float()?;
        self.skip_ws();
        if had_paren && !self.eat(b')') {
            return None;
        }
        Some((x, y))
    }

    fn at_end(&mut self) -> bool {
        self.skip_ws();
        self.i >= self.s.len()
    }
}

fn fmt_coord(x: f64) -> String {
    format!("{x}")
}

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    let _ = oid;

    let t = text.trim();

    let body = match (t.starts_with('['), t.ends_with(']')) {
        (true, true) => &t[1..t.len() - 1],
        (false, false) => t,
        _ => return Err(err(text)),
    };

    let mut sc = Scanner::new(body);
    let p1 = sc.parse_point().ok_or_else(|| err(text))?;
    sc.skip_ws();
    if !sc.eat(b',') {
        return Err(err(text));
    }
    let p2 = sc.parse_point().ok_or_else(|| err(text))?;
    if !sc.at_end() {
        return Err(err(text));
    }

    let canon = format!(
        "[({},{}),({},{})]",
        fmt_coord(p1.0),
        fmt_coord(p1.1),
        fmt_coord(p2.0),
        fmt_coord(p2.1)
    );
    Ok(SqlValue::Text(canon))
}

pub fn output(oid: u32, v: &SqlValue) -> String {
    let _ = oid;
    match v {
        SqlValue::Text(s) => s.clone(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::oid;

    const CANON: &str = "[(1,2),(3,4)]";

    fn parse(s: &str) -> SqlValue {
        input(oid::LSEG, s).expect("expected a valid lseg literal")
    }

    fn text(s: &str) -> SqlValue {
        SqlValue::Text(s.to_string())
    }

    #[test]
    fn canonical_round_trip() {

        let v = parse(CANON);
        assert_eq!(v, text(CANON));
        assert_eq!(output(oid::LSEG, &v), CANON);
    }

    #[test]
    fn parenthesized_no_brackets_accepted() {

        assert_eq!(parse("(0,0),(6,6)"), text("[(0,0),(6,6)]"));
    }

    #[test]
    fn bare_numbers_with_spaces_accepted() {

        assert_eq!(parse("10,-10 ,-3,-4"), text("[(10,-10),(-3,-4)]"));
    }

    #[test]
    fn bracketed_scientific_notation_accepted_and_rerendered() {

        assert_eq!(
            parse("[-1e6,2e2,3e5, -4e1]"),
            text("[(-1000000,200),(300000,-40)]")
        );
    }

    #[test]
    fn vertical_and_horizontal_accepted() {

        assert_eq!(parse("[(-10,2),(-10,3)]"), text("[(-10,2),(-10,3)]"));
        assert_eq!(parse("[(0,-20),(30,-20)]"), text("[(0,-20),(30,-20)]"));
    }

    #[test]
    fn nan_coordinate_accepted_and_rendered() {

        assert_eq!(parse("[(NaN,1),(NaN,90)]"), text("[(NaN,1),(NaN,90)]"));
    }

    #[test]
    fn float_rendering_drops_trailing_dot_zero() {

        assert_eq!(parse("[(2e2,1),(3e5,4)]"), text("[(200,1),(300000,4)]"));
    }

    #[test]
    fn reject_garbage_number_tokens() {

        let bad = "(3asdf,2 ,3,4r2)";
        let e = input(oid::LSEG, bad).unwrap_err();
        assert_eq!(
            e,
            PgError::InvalidInputSyntax {
                typ: "lseg",
                input: bad.to_string()
            }
        );

        assert_eq!(
            e.message(),
            "invalid input syntax for type lseg: \"(3asdf,2 ,3,4r2)\""
        );
    }

    #[test]
    fn reject_unmatched_open_bracket_bare() {

        assert!(matches!(
            input(oid::LSEG, "[1,2,3, 4"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_unmatched_open_bracket_paren() {

        assert!(matches!(
            input(oid::LSEG, "[(1,2),(3,4)"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_empty_coordinate() {

        assert!(matches!(
            input(oid::LSEG, "[(,2),(3,4)]"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_point_with_single_coordinate() {

        let bad = "[(1,2),(3)]";
        let e = input(oid::LSEG, bad).unwrap_err();
        assert_eq!(
            e.message(),
            "invalid input syntax for type lseg: \"[(1,2),(3)]\""
        );
    }

    #[test]
    fn output_non_text_is_empty() {
        assert_eq!(output(oid::LSEG, &SqlValue::Null), "");
        assert_eq!(output(oid::LSEG, &SqlValue::Int(42)), "");
    }
}
