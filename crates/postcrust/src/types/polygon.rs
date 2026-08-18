
use super::PgError;
use sql_core::SqlValue;

fn err(input: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: super::type_name(super::oid::POLYGON),
        input: input.to_string(),
    }
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

fn parse_num(b: &[u8], i: usize) -> Option<(f64, usize)> {
    let start = i;
    let mut j = i;
    while j < b.len() && matches!(b[j], b'0'..=b'9' | b'.' | b'+' | b'-' | b'e' | b'E') {
        j += 1;
    }
    if j == start {
        return None;
    }
    let tok = std::str::from_utf8(&b[start..j]).ok()?;
    let v: f64 = tok.parse().ok()?;
    Some((v, j))
}

fn parse_point(b: &[u8], i: usize) -> Option<((f64, f64), usize)> {
    let mut i = skip_ws(b, i);
    let has_paren = i < b.len() && b[i] == b'(';
    if has_paren {
        i = skip_ws(b, i + 1);
    }
    let (x, ni) = parse_num(b, i)?;
    i = skip_ws(b, ni);
    if !(i < b.len() && b[i] == b',') {
        return None;
    }
    i = skip_ws(b, i + 1);
    let (y, ni) = parse_num(b, i)?;
    i = skip_ws(b, ni);
    if has_paren {
        if i < b.len() && b[i] == b')' {
            i += 1;
        } else {
            return None;
        }
    }
    Some(((x, y), i))
}

fn parse_polygon(text: &str) -> Option<Vec<(f64, f64)>> {
    let b = text.as_bytes();
    let mut i = skip_ws(b, 0);

    let mut want_close = false;
    if i < b.len() && b[i] == b'(' {
        let j = skip_ws(b, i + 1);
        if j < b.len() && b[j] == b'(' {
            want_close = true;
            i += 1;
        }
    }

    let mut pts = Vec::new();
    loop {
        let ((x, y), ni) = parse_point(b, i)?;
        pts.push((x, y));
        i = skip_ws(b, ni);
        if i < b.len() && b[i] == b',' {
            i = skip_ws(b, i + 1);
            continue;
        }
        break;
    }

    i = skip_ws(b, i);
    if want_close {
        if i < b.len() && b[i] == b')' {
            i += 1;
        } else {
            return None;
        }
    }
    i = skip_ws(b, i);
    if i != b.len() || pts.is_empty() {
        return None;
    }
    Some(pts)
}

fn fmt_f64(v: f64) -> String {
    if v.is_finite() && v == v.trunc() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

fn canonical(pts: &[(f64, f64)]) -> String {
    let mut s = String::from("(");
    for (k, (x, y)) in pts.iter().enumerate() {
        if k > 0 {
            s.push(',');
        }
        s.push('(');
        s.push_str(&fmt_f64(*x));
        s.push(',');
        s.push_str(&fmt_f64(*y));
        s.push(')');
    }
    s.push(')');
    s
}

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    let _ = oid;
    match parse_polygon(text) {
        Some(pts) => Ok(SqlValue::Text(canonical(&pts))),
        None => Err(err(text)),
    }
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
        input(oid::POLYGON, s).expect("expected a valid polygon literal")
    }

    fn text(s: &str) -> SqlValue {
        SqlValue::Text(s.to_string())
    }

    #[test]
    fn canonical_round_trip_three_points() {

        let v = parse("(2.0,0.0),(2.0,4.0),(0.0,0.0)");
        assert_eq!(v, text("((2,0),(2,4),(0,0))"));
        assert_eq!(output(oid::POLYGON, &v), "((2,0),(2,4),(0,0))");
    }

    #[test]
    fn multi_point_four() {

        assert_eq!(
            parse("(1,2),(3,4),(5,6),(7,8)"),
            text("((1,2),(3,4),(5,6),(7,8))")
        );
        assert_eq!(
            parse("(7,8),(5,6),(3,4),(1,2)"),
            text("((7,8),(5,6),(3,4),(1,2))")
        );
    }

    #[test]
    fn negative_coord() {

        assert_eq!(
            parse("(1,2),(7,8),(5,6),(3,-4)"),
            text("((1,2),(7,8),(5,6),(3,-4))")
        );
    }

    #[test]
    fn single_degenerate_point() {

        assert_eq!(parse("(0.0,0.0)"), text("((0,0))"));
    }

    #[test]
    fn two_coincident_points() {

        assert_eq!(parse("(0.0,1.0),(0.0,1.0)"), text("((0,1),(0,1))"));
    }

    #[test]
    fn outer_wrapped_with_spaces() {

        assert_eq!(
            parse("((200, 300),(210, 310),(230, 290))"),
            text("((200,300),(210,310),(230,290))")
        );
    }

    #[test]
    fn float_rendering_shortest() {

        assert_eq!(fmt_f64(2.0), "2");
        assert_eq!(fmt_f64(-4.0), "-4");
        assert_eq!(fmt_f64(3.5), "3.5");
        assert_eq!(parse("(1.5,2.0)"), text("((1.5,2))"));
    }

    #[test]
    fn reject_bare_scalar() {

        let bad = "0.0";
        let e = input(oid::POLYGON, bad).unwrap_err();
        assert_eq!(
            e,
            PgError::InvalidInputSyntax {
                typ: "polygon",
                input: bad.to_string()
            }
        );
        assert_eq!(
            e.message(),
            "invalid input syntax for type polygon: \"0.0\""
        );
    }

    #[test]
    fn reject_missing_comma_and_paren() {

        assert!(matches!(
            input(oid::POLYGON, "(0.0 0.0"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_three_coords() {

        assert!(matches!(
            input(oid::POLYGON, "(0,1,2)"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_unterminated_multi_coord() {

        assert!(matches!(
            input(oid::POLYGON, "(0,1,2,3"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_non_numeric() {

        assert!(matches!(
            input(oid::POLYGON, "asdf"),
            Err(PgError::InvalidInputSyntax { .. })
        ));

        assert!(matches!(
            input(oid::POLYGON, "(2.0,xyz)"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_empty() {
        assert!(matches!(
            input(oid::POLYGON, ""),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn output_non_text_is_empty() {
        assert_eq!(output(oid::POLYGON, &SqlValue::Null), "");
        assert_eq!(output(oid::POLYGON, &SqlValue::Int(42)), "");
    }
}
