
use super::PgError;
use sql_core::SqlValue;

fn err(input: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: super::type_name(super::oid::CIRCLE),
        input: input.to_string(),
    }
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn new(s: &str) -> Self {
        Parser {
            chars: s.chars().collect(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) {
        self.pos += 1;
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_whitespace()) {
            self.pos += 1;
        }
    }

    fn eof(&self) -> bool {
        self.pos >= self.chars.len()
    }

    fn read_number(&mut self) -> Option<f64> {
        self.skip_ws();
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_whitespace() || c == ',' || c == ')' || c == '>' {
                break;
            }
            self.pos += 1;
        }
        if self.pos == start {
            return None;
        }
        let tok: String = self.chars[start..self.pos].iter().collect();

        tok.parse::<f64>().ok()
    }

    fn read_point(&mut self) -> Option<(f64, f64)> {
        self.skip_ws();
        let paren = self.peek() == Some('(');
        if paren {
            self.bump();
        }
        let x = self.read_number()?;
        self.skip_ws();
        if self.peek() != Some(',') {
            return None;
        }
        self.bump();
        let y = self.read_number()?;
        if paren {
            self.skip_ws();
            if self.peek() != Some(')') {
                return None;
            }
            self.bump();
        }
        Some((x, y))
    }
}

fn fmt_f64(v: f64) -> String {
    if v.is_nan() {
        return "NaN".to_string();
    }
    if v.is_infinite() {
        return if v > 0.0 {
            "Infinity".to_string()
        } else {
            "-Infinity".to_string()
        };
    }
    format!("{v}")
}

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    let _ = oid;

    let mut p = Parser::new(text);
    p.skip_ws();

    let mut wrapped = false;
    match p.peek() {
        Some('<') => {
            wrapped = true;
            p.bump();
        }
        Some('(') => {
            let save = p.pos;
            p.bump();
            p.skip_ws();
            if p.peek() == Some('(') {
                wrapped = true;
            } else {
                p.pos = save;
            }
        }
        _ => {}
    }

    let (x, y) = match p.read_point() {
        Some(pt) => pt,
        None => return Err(err(text)),
    };

    p.skip_ws();
    if p.peek() == Some(',') {
        p.bump();
    }

    let r = match p.read_number() {
        Some(r) => r,
        None => return Err(err(text)),
    };

    p.skip_ws();
    if wrapped {

        match p.peek() {
            Some('>') | Some(')') => {
                p.bump();
                p.skip_ws();
            }
            _ => return Err(err(text)),
        }
    }

    if !p.eof() {
        return Err(err(text));
    }

    if r < 0.0 {
        return Err(err(text));
    }

    Ok(SqlValue::Text(format!(
        "<({},{}),{}>",
        fmt_f64(x),
        fmt_f64(y),
        fmt_f64(r)
    )))
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

    fn parse(s: &str) -> SqlValue {
        input(oid::CIRCLE, s).expect("expected a valid circle literal")
    }

    fn canon(s: &str) -> String {
        match parse(s) {
            SqlValue::Text(t) => t,
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn canonical_round_trip() {

        assert_eq!(canon("<(5,1),3>"), "<(5,1),3>");
        let v = parse("<(5,1),3>");
        assert_eq!(output(oid::CIRCLE, &v), "<(5,1),3>");
    }

    #[test]
    fn double_paren_collapses() {

        assert_eq!(canon("((1,2),100)"), "<(1,2),100>");
    }

    #[test]
    fn bare_triple_collapses() {

        assert_eq!(canon(" 1 , 3 , 5 "), "<(1,3),5>");
    }

    #[test]
    fn spaced_double_paren_collapses() {

        assert_eq!(canon(" ( ( 1 , 2 ) , 3 ) "), "<(1,2),3>");
    }

    #[test]
    fn paren_point_only_collapses() {

        assert_eq!(canon(" ( 100 , 200 ) , 10 "), "<(100,200),10>");
    }

    #[test]
    fn spaced_angle_collapses() {

        assert_eq!(canon(" < ( 100 , 1 ) , 115 > "), "<(100,1),115>");
    }

    #[test]
    fn zero_radius_accepted() {

        assert_eq!(canon("<(3,5),0>"), "<(3,5),0>");
    }

    #[test]
    fn nan_radius_accepted() {

        assert_eq!(canon("<(3,5),NaN>"), "<(3,5),NaN>");
    }

    #[test]
    fn reject_negative_radius() {

        let bad = "<(-100,0),-100>";
        let e = input(oid::CIRCLE, bad).unwrap_err();
        assert_eq!(
            e,
            PgError::InvalidInputSyntax {
                typ: "circle",
                input: bad.to_string()
            }
        );
        assert_eq!(
            e.message(),
            "invalid input syntax for type circle: \"<(-100,0),-100>\""
        );
    }

    #[test]
    fn reject_unclosed_wrapper() {

        assert!(matches!(
            input(oid::CIRCLE, "<(100,200),10"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_trailing_junk() {

        assert!(matches!(
            input(oid::CIRCLE, "<(100,200),10> x"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_non_numeric_field() {

        assert!(matches!(
            input(oid::CIRCLE, "1abc,3,5"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_point_where_coordinate_belongs() {

        assert!(matches!(
            input(oid::CIRCLE, "(3,(1,2),3)"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn float_rendering_integers_have_no_fraction() {

        assert_eq!(canon("<(115,0),230>"), "<(115,0),230>");
    }

    #[test]
    fn output_non_text_is_empty() {
        assert_eq!(output(oid::CIRCLE, &SqlValue::Null), "");
        assert_eq!(output(oid::CIRCLE, &SqlValue::Int(42)), "");
    }
}
