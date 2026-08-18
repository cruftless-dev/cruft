
use super::PgError;
use sql_core::SqlValue;

fn err(input: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: super::type_name(super::oid::LINE),
        input: input.to_string(),
    }
}

fn parse_f64(tok: &str, whole: &str) -> Result<f64, PgError> {
    let t = tok.trim();
    if t.is_empty() {
        return Err(err(whole));
    }
    match t.parse::<f64>() {

        Ok(v) if v.is_finite() || v.is_nan() => Ok(v),
        _ => Err(err(whole)),
    }
}

fn fmt_f64(v: f64) -> String {
    if v == 0.0 {
        "0".to_string()
    } else {
        format!("{v}")
    }
}

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    let _ = oid;

    let trimmed = text.trim();

    if trimmed.starts_with('{') {
        input_coeffs(trimmed, text)
    } else {
        input_two_point(trimmed, text)
    }
}

fn input_coeffs(body: &str, whole: &str) -> Result<SqlValue, PgError> {

    if !body.ends_with('}') {
        return Err(err(whole));
    }
    let inner = &body[1..body.len() - 1];
    let parts: Vec<&str> = inner.split(',').collect();
    if parts.len() != 3 {
        return Err(err(whole));
    }
    let a = parse_f64(parts[0], whole)?;
    let b = parse_f64(parts[1], whole)?;
    let c = parse_f64(parts[2], whole)?;

    if a == 0.0 && b == 0.0 {
        return Err(err(whole));
    }

    Ok(canonical(a, b, c))
}

fn input_two_point(body: &str, whole: &str) -> Result<SqlValue, PgError> {
    let unwrapped = match (body.starts_with('['), body.ends_with(']')) {
        (true, true) => &body[1..body.len() - 1],
        (false, false) => body,

        _ => return Err(err(whole)),
    };

    let flat: String = unwrapped
        .chars()
        .filter(|&c| c != '(' && c != ')')
        .collect();

    let parts: Vec<&str> = flat.split(',').collect();
    if parts.len() != 4 {
        return Err(err(whole));
    }
    let x1 = parse_f64(parts[0], whole)?;
    let y1 = parse_f64(parts[1], whole)?;
    let x2 = parse_f64(parts[2], whole)?;
    let y2 = parse_f64(parts[3], whole)?;

    if x1 == x2 && y1 == y2 {
        return Err(err(whole));
    }

    let (a, b, c) = if x1 == x2 {
        (-1.0, 0.0, x1)
    } else {
        let slope = (y2 - y1) / (x2 - x1);
        (slope, -1.0, y1 - slope * x1)
    };

    Ok(canonical(a, b, c))
}

fn canonical(a: f64, b: f64, c: f64) -> SqlValue {
    SqlValue::Text(format!("{{{},{},{}}}", fmt_f64(a), fmt_f64(b), fmt_f64(c)))
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
        input(oid::LINE, s).expect("expected a valid line literal")
    }

    fn text(s: &str) -> SqlValue {
        SqlValue::Text(s.to_string())
    }

    #[test]
    fn direct_coeff_form_round_trips() {

        assert_eq!(parse("{0,-1,5}"), text("{0,-1,5}"));
        assert_eq!(parse("{1,0,5}"), text("{1,0,5}"));
        assert_eq!(parse("{0,3,0}"), text("{0,3,0}"));
    }

    #[test]
    fn direct_coeff_nan_preserved() {

        assert_eq!(parse("{3,NaN,5}"), text("{3,NaN,5}"));
        assert_eq!(parse("{NaN,NaN,NaN}"), text("{NaN,NaN,NaN}"));

        assert_eq!(parse("{nan, 1, nan}"), text("{NaN,1,NaN}"));
    }

    #[test]
    fn two_point_paren_form() {

        assert_eq!(parse(" (0,0), (6,6)"), text("{1,-1,0}"));
    }

    #[test]
    fn two_point_bare_four_floats() {

        assert_eq!(parse("10,-10 ,-5,-4"), text("{-0.4,-1,-6}"));
    }

    #[test]
    fn two_point_bracket_scientific() {

        assert_eq!(
            parse("[-1e6,2e2,3e5, -4e1]"),
            text("{-0.0001846153846153846,-1,15.384615384615387}")
        );
    }

    #[test]
    fn two_point_horizontal() {

        assert_eq!(parse("[(1,3),(2,3)]"), text("{0,-1,3}"));
    }

    #[test]
    fn two_point_vertical() {

        assert_eq!(parse("[(3,1),(3,2)]"), text("{-1,0,3}"));
    }

    #[test]
    fn reject_degenerate_two_point() {

        assert!(matches!(
            input(oid::LINE, "[(1,2),(1,2)]"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
        assert!(matches!(
            input(oid::LINE, "(1,0),(1,0)"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_both_ab_zero() {

        let e = input(oid::LINE, "{0,0,1}").unwrap_err();
        assert_eq!(
            e,
            PgError::InvalidInputSyntax {
                typ: "line",
                input: "{0,0,1}".to_string()
            }
        );

        assert!(matches!(
            input(oid::LINE, "{0, 0, 0}"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_malformed_brace_forms() {

        for bad in ["{}", "{0", "{0,0}", "{0,0,1", "{0,0,1} x", "{1, 1}"] {
            assert!(
                matches!(
                    input(oid::LINE, bad),
                    Err(PgError::InvalidInputSyntax { .. })
                ),
                "expected reject for {bad:?}"
            );
        }
    }

    #[test]
    fn reject_malformed_two_point_forms() {

        for bad in [
            "(3asdf,2 ,3,4r2)",
            "[1,2,3, 4",
            "[(,2),(3,4)]",
            "[(1,2),(3,4)",
            "{1, 1, a}",
        ] {
            assert!(
                matches!(
                    input(oid::LINE, bad),
                    Err(PgError::InvalidInputSyntax { .. })
                ),
                "expected reject for {bad:?}"
            );
        }
    }

    #[test]
    fn reject_out_of_range_float() {

        assert!(matches!(
            input(oid::LINE, "{1, 1, 1e400}"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
        assert!(matches!(
            input(oid::LINE, "(1, 1), (1, 1e400)"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn float_rendering_matches_golden() {

        assert_eq!(fmt_f64(1.0), "1");
        assert_eq!(fmt_f64(-1.0), "-1");
        assert_eq!(fmt_f64(-0.0), "0");
        assert_eq!(fmt_f64(-0.4), "-0.4");
        assert_eq!(fmt_f64(15.384615384615387), "15.384615384615387");
        assert_eq!(fmt_f64(f64::NAN), "NaN");
    }

    #[test]
    fn output_round_trips_and_non_text_empty() {
        let v = parse("{0,-1,5}");
        assert_eq!(output(oid::LINE, &v), "{0,-1,5}");
        assert_eq!(output(oid::LINE, &parse("(0,0), (6,6)")), "{1,-1,0}");
        assert_eq!(output(oid::LINE, &SqlValue::Null), "");
        assert_eq!(output(oid::LINE, &SqlValue::Int(42)), "");
    }
}
