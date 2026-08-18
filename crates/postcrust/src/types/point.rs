
use super::PgError;
use sql_core::SqlValue;

fn err(input: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: super::type_name(super::oid::POINT),
        input: input.to_string(),
    }
}

fn is_infinity_literal(tok: &str) -> bool {
    let body = tok
        .strip_prefix(['+', '-'])
        .unwrap_or(tok)
        .to_ascii_lowercase();
    body == "inf" || body == "infinity"
}

fn parse_coord(tok: &str, whole: &str) -> Result<f64, PgError> {
    let t = tok.trim();
    if t.is_empty() {
        return Err(err(whole));
    }
    match t.parse::<f64>() {
        Ok(v) if v.is_finite() => Ok(v),

        Ok(v) if v.is_nan() || is_infinity_literal(t) => Ok(v),
        _ => Err(err(whole)),
    }
}

fn render_coord(v: f64) -> String {
    if v.is_nan() {
        "NaN".to_string()
    } else if v.is_infinite() {
        if v < 0.0 {
            "-Infinity".to_string()
        } else {
            "Infinity".to_string()
        }
    } else {

        format!("{v}")
    }
}

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    let _ = oid;

    let s = text.trim();

    let inner = match (s.starts_with('('), s.ends_with(')')) {
        (true, true) => &s[1..s.len() - 1],
        (false, false) => s,
        _ => return Err(err(text)),
    };

    let coords: Vec<&str> = inner.split(',').collect();
    if coords.len() != 2 {
        return Err(err(text));
    }

    let x = parse_coord(coords[0], text)?;
    let y = parse_coord(coords[1], text)?;

    Ok(SqlValue::Text(format!(
        "({},{})",
        render_coord(x),
        render_coord(y)
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
        input(oid::POINT, s).expect("expected a valid point literal")
    }

    fn text(s: &str) -> SqlValue {
        SqlValue::Text(s.to_string())
    }

    #[test]
    fn canonical_input_stored_verbatim() {

        assert_eq!(parse("(5.1,34.5)"), text("(5.1,34.5)"));
    }

    #[test]
    fn space_after_comma_accepted() {

        assert_eq!(parse("(5.1, 34.5)"), text("(5.1,34.5)"));
        assert_eq!(parse("(0.0, 0.0)"), text("(0,0)"));
    }

    #[test]
    fn bare_form_without_parens_accepted() {

        assert_eq!(parse("1,2"), text("(1,2)"));
    }

    #[test]
    fn negative_coordinates_round_trip() {

        assert_eq!(parse("(-10,0)"), text("(-10,0)"));
        assert_eq!(parse("(-5,-12)"), text("(-5,-12)"));
        assert_eq!(parse("(-3,4)"), text("(-3,4)"));
    }

    #[test]
    fn integer_valued_floats_drop_decimal() {

        assert_eq!(parse("(10.0,10.0)"), text("(10,10)"));
    }

    #[test]
    fn decimal_floats_kept() {

        assert_eq!(parse("(1.5,-2.25)"), text("(1.5,-2.25)"));
    }

    #[test]
    fn surrounding_whitespace_tolerated() {
        assert_eq!(parse("  (10,10)  "), text("(10,10)"));
    }

    #[test]
    fn special_float_coordinates() {

        assert_eq!(parse("(NaN,NaN)"), text("(NaN,NaN)"));
        assert_eq!(parse("(Infinity,-Infinity)"), text("(Infinity,-Infinity)"));
    }

    #[test]
    fn reject_not_a_pair() {

        let bad = "asdfasdf";
        let e = input(oid::POINT, bad).unwrap_err();
        assert_eq!(
            e,
            PgError::InvalidInputSyntax {
                typ: "point",
                input: bad.to_string()
            }
        );

        assert_eq!(
            e.message(),
            "invalid input syntax for type point: \"asdfasdf\""
        );
    }

    #[test]
    fn reject_missing_comma() {

        assert!(matches!(
            input(oid::POINT, "(10.0 10.0)"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_trailing_garbage() {

        assert!(matches!(
            input(oid::POINT, "(10.0, 10.0) x"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_unmatched_open_paren() {

        assert!(matches!(
            input(oid::POINT, "(10.0,10.0"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_out_of_range_coordinate() {

        assert!(matches!(
            input(oid::POINT, "(10.0, 1e+500)"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_non_numeric_coordinate() {

        assert!(matches!(
            input(oid::POINT, "1,y"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn output_round_trips_stored_canonical() {
        let v = parse("(10.0, 10.0)");
        assert_eq!(output(oid::POINT, &v), "(10,10)");
        assert_eq!(output(oid::POINT, &parse("(5.1,34.5)")), "(5.1,34.5)");
    }

    #[test]
    fn output_non_text_is_empty() {
        assert_eq!(output(oid::POINT, &SqlValue::Null), "");
        assert_eq!(output(oid::POINT, &SqlValue::Int(42)), "");
    }
}
