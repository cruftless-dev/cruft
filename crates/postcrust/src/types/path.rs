
use super::PgError;
use sql_core::SqlValue;

fn err(input: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: super::type_name(super::oid::PATH),
        input: input.to_string(),
    }
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

fn single_decode(b: &[u8], i: usize) -> Option<(f64, usize)> {
    let start = skip_ws(b, i);
    let mut j = start;
    if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
        j += 1;
    }
    let mut saw_digit = false;
    while j < b.len() && b[j].is_ascii_digit() {
        j += 1;
        saw_digit = true;
    }
    if j < b.len() && b[j] == b'.' {
        j += 1;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
            saw_digit = true;
        }
    }
    if saw_digit && j < b.len() && (b[j] == b'e' || b[j] == b'E') {
        let mut k = j + 1;
        if k < b.len() && (b[k] == b'+' || b[k] == b'-') {
            k += 1;
        }
        if k < b.len() && b[k].is_ascii_digit() {
            while k < b.len() && b[k].is_ascii_digit() {
                k += 1;
            }
            j = k;
        }
    }
    if !saw_digit {
        return None;
    }
    let s = std::str::from_utf8(&b[start..j]).ok()?;
    let v: f64 = s.parse().ok()?;
    Some((v, j))
}

fn pair_decode(b: &[u8], i: usize) -> Option<((f64, f64), usize)> {
    let mut i = skip_ws(b, i);
    let has_delim = i < b.len() && b[i] == b'(';
    if has_delim {
        i += 1;
    }
    let (x, ni) = single_decode(b, i)?;
    i = skip_ws(b, ni);
    if i >= b.len() || b[i] != b',' {
        return None;
    }
    i += 1;
    let (y, ni) = single_decode(b, i)?;
    i = ni;
    if has_delim {
        i = skip_ws(b, i);
        if i >= b.len() || b[i] != b')' {
            return None;
        }
        i += 1;
    }
    i = skip_ws(b, i);
    Some(((x, y), i))
}

fn path_decode(b: &[u8], i: usize, npts: usize) -> Option<(bool, Vec<(f64, f64)>, usize)> {
    let mut i = skip_ws(b, i);
    let isopen = i < b.len() && b[i] == b'[';
    if isopen {
        i = skip_ws(b, i + 1);
    }
    let mut pts = Vec::with_capacity(npts);
    for _ in 0..npts {
        let ((x, y), ni) = pair_decode(b, i)?;
        i = ni;
        if i < b.len() && b[i] == b',' {
            i += 1;
        }
        pts.push((x, y));
    }
    if isopen {
        i = skip_ws(b, i);
        if i >= b.len() || b[i] != b']' {
            return None;
        }
        i = skip_ws(b, i + 1);
    }
    Some((isopen, pts, i))
}

fn fmt_coord(v: f64) -> String {
    format!("{}", v)
}

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    let _ = oid;
    let b = text.as_bytes();

    let ncommas = b.iter().filter(|&&c| c == b',').count();
    if ncommas % 2 == 0 {
        return Err(err(text));
    }
    let npts = (ncommas + 1) / 2;

    let mut i = skip_ws(b, 0);
    let mut depth = 0;
    if i < b.len() && b[i] == b'(' {
        i += 1;
        depth = 1;
    }

    let (isopen, pts, mut i) = match path_decode(b, i, npts) {
        Some(v) => v,
        None => return Err(err(text)),
    };

    while depth > 0 && i < b.len() {
        if b[i] == b')' {
            depth -= 1;
            i = skip_ws(b, i + 1);
        } else {
            break;
        }
    }
    if depth != 0 {
        return Err(err(text));
    }

    i = skip_ws(b, i);
    if i != b.len() {
        return Err(err(text));
    }

    let body: Vec<String> = pts
        .iter()
        .map(|&(x, y)| format!("({},{})", fmt_coord(x), fmt_coord(y)))
        .collect();
    let joined = body.join(",");
    let canon = if isopen {
        format!("[{}]", joined)
    } else {
        format!("({})", joined)
    };
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

    fn parse(s: &str) -> SqlValue {
        input(oid::PATH, s).expect("expected a valid path literal")
    }

    fn text(s: &str) -> SqlValue {
        SqlValue::Text(s.to_string())
    }

    #[test]
    fn open_path_preserved_as_brackets() {

        assert_eq!(parse("[(1,2),(3,4)]"), text("[(1,2),(3,4)]"));
    }

    #[test]
    fn closed_path_preserved_as_parens() {

        assert_eq!(parse(" ( ( 1 , 2 ) , ( 3 , 4 ) ) "), text("((1,2),(3,4))"));

        assert_eq!(parse("((1,2) ,(3,4 ))"), text("((1,2),(3,4))"));
    }

    #[test]
    fn bare_pairs_default_closed() {

        assert_eq!(parse("1,2 ,3,4 "), text("((1,2),(3,4))"));
    }

    #[test]
    fn bare_pairs_open_when_bracketed() {

        assert_eq!(parse(" [1,2,3, 4] "), text("[(1,2),(3,4)]"));
        assert_eq!(parse("[ 11,12,13,14 ]"), text("[(11,12),(13,14)]"));
    }

    #[test]
    fn wrap_paren_bare_pairs_closed() {

        assert_eq!(parse("( 11,12,13,14) "), text("((11,12),(13,14))"));
    }

    #[test]
    fn single_point_closed_is_accepted() {

        assert_eq!(parse("((10,20))"), text("((10,20))"));
    }

    #[test]
    fn float_rendering_integers_have_no_dot_zero() {

        assert_eq!(
            parse("[ (0,0),(3,0),(4,5),(1,6) ]"),
            text("[(0,0),(3,0),(4,5),(1,6)]")
        );
    }

    #[test]
    fn float_rendering_fractional_shortest() {

        assert_eq!(parse("[(1.5,-2.25)]"), text("[(1.5,-2.25)]"));
    }

    #[test]
    fn canonical_forms_round_trip_through_output() {
        for &canon in &["[(1,2),(3,4)]", "((1,2),(3,4))", "((10,20))"] {
            let v = parse(canon);
            assert_eq!(output(oid::PATH, &v), canon);
        }
    }

    #[test]
    fn reject_empty_bracket() {

        assert!(matches!(
            input(oid::PATH, "[]"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_odd_coordinate_count() {

        let bad = "[(1,2),(3)]";
        let e = input(oid::PATH, bad).unwrap_err();
        assert_eq!(
            e,
            PgError::InvalidInputSyntax {
                typ: "path",
                input: bad.to_string()
            }
        );
        assert_eq!(
            e.message(),
            "invalid input syntax for type path: \"[(1,2),(3)]\""
        );
    }

    #[test]
    fn reject_missing_coordinate() {

        assert!(matches!(
            input(oid::PATH, "[(,2),(3,4)]"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_three_coordinate_point() {

        assert!(matches!(
            input(oid::PATH, "[(1,2,6),(3,4,6)]"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_unterminated_open_path() {

        assert!(matches!(
            input(oid::PATH, "[(1,2),(3,4)"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_unbalanced_wrap_paren() {

        assert!(matches!(
            input(oid::PATH, "(1,2,3,4"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_mismatched_brackets() {

        assert!(matches!(
            input(oid::PATH, "(1,2),(3,4)]"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn output_non_text_is_empty() {
        assert_eq!(output(oid::PATH, &SqlValue::Null), "");
        assert_eq!(output(oid::PATH, &SqlValue::Int(42)), "");
    }
}
