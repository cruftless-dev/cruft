
use super::PgError;
use sql_core::SqlValue;

fn err(input: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: super::type_name(super::oid::BOX),
        input: input.to_string(),
    }
}

fn fmt_coord(x: f64) -> String {
    format!("{}", x)
}

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    let _ = oid;

    if text.contains('[') || text.contains(']') {
        return Err(err(text));
    }

    let stripped: String = text.chars().filter(|&c| c != '(' && c != ')').collect();
    let mut nums = [0.0f64; 4];
    let mut count = 0usize;
    for tok in stripped.split(',') {
        if count == 4 {
            return Err(err(text));
        }
        match tok.trim().parse::<f64>() {
            Ok(v) => {
                nums[count] = v;
                count += 1;
            }
            Err(_) => return Err(err(text)),
        }
    }
    if count != 4 {
        return Err(err(text));
    }

    let (x1, y1, x2, y2) = (nums[0], nums[1], nums[2], nums[3]);

    let hx = x1.max(x2);
    let hy = y1.max(y2);
    let lx = x1.min(x2);
    let ly = y1.min(y2);

    let canon = format!(
        "({},{}),({},{})",
        fmt_coord(hx),
        fmt_coord(hy),
        fmt_coord(lx),
        fmt_coord(ly)
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

    fn parse(s: &str) -> SqlValue {
        input(oid::BOX, s).expect("expected a valid box literal")
    }

    fn text(s: &str) -> SqlValue {
        SqlValue::Text(s.to_string())
    }

    #[test]
    fn corners_reordered_high_then_low() {

        assert_eq!(parse("(0,0),(1,1)"), text("(1,1),(0,0)"));
    }

    #[test]
    fn golden_box_tbl_rows_canonicalize() {

        assert_eq!(parse("(2.0,2.0,0.0,0.0)"), text("(2,2),(0,0)"));
        assert_eq!(parse("(1.0,1.0,3.0,3.0)"), text("(3,3),(1,1)"));
        assert_eq!(parse("((-8, 2), (-2, -10))"), text("(-2,2),(-8,-10)"));
        assert_eq!(parse("(2.5, 2.5, 2.5,3.5)"), text("(2.5,3.5),(2.5,2.5)"));
        assert_eq!(parse("(3.0, 3.0,3.0,3.0)"), text("(3,3),(3,3)"));
    }

    #[test]
    fn per_coordinate_not_whole_point_swap() {

        assert_eq!(parse("((-8, 2), (-2, -10))"), text("(-2,2),(-8,-10)"));
    }

    #[test]
    fn canonical_form_round_trips() {

        let v = parse("(2,2),(0,0)");
        assert_eq!(v, text("(2,2),(0,0)"));
        assert_eq!(output(oid::BOX, &v), "(2,2),(0,0)");
    }

    #[test]
    fn float_rendering_matches_pg() {

        assert_eq!(parse("(2.5, 2.5, 2.5,3.5)"), text("(2.5,3.5),(2.5,2.5)"));
        assert_eq!(parse("(1.0,1.0,3.0,3.0)"), text("(3,3),(1,1)"));
    }

    #[test]
    fn accepts_bare_and_nested_paren_forms() {

        assert_eq!(parse("2.0,2.0,0.0,0.0"), text("(2,2),(0,0)"));
        assert_eq!(parse("((0,0),(1,1))"), text("(1,1),(0,0)"));
    }

    #[test]
    fn reject_too_few_numbers() {

        let bad = "(2.3, 4.5)";
        let e = input(oid::BOX, bad).unwrap_err();
        assert_eq!(
            e,
            PgError::InvalidInputSyntax {
                typ: "box",
                input: bad.to_string()
            }
        );
        assert_eq!(
            e.message(),
            "invalid input syntax for type box: \"(2.3, 4.5)\""
        );
        assert!(matches!(
            input(oid::BOX, "200"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_brackets() {

        assert!(matches!(
            input(oid::BOX, "[1, 2, 3, 4)"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
        assert!(matches!(
            input(oid::BOX, "(1, 2, 3, 4]"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_trailing_garbage_and_nonnumeric() {

        assert!(matches!(
            input(oid::BOX, "(1, 2, 3, 4) x"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
        assert!(matches!(
            input(oid::BOX, "asdfasdf(ad"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
        assert!(matches!(
            input(oid::BOX, "((200,300),(500, xyz))"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn reject_too_many_numbers() {

        assert!(matches!(
            input(oid::BOX, "(1,2,3,4,5)"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn output_non_text_is_empty() {
        assert_eq!(output(oid::BOX, &SqlValue::Null), "");
        assert_eq!(output(oid::BOX, &SqlValue::Int(42)), "");
    }
}
