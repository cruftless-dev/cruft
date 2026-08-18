
use super::{oid, type_name, PgError};
use sql_core::SqlValue;

fn element_oid(range_oid: u32) -> Option<u32> {
    Some(match range_oid {
        oid::INT4RANGE => oid::INT4,
        oid::INT8RANGE => oid::INT8,
        oid::NUMRANGE => oid::NUMERIC,
        oid::TSRANGE => oid::TIMESTAMP,
        oid::DATERANGE => oid::DATE,
        _ => return None,
    })
}

fn is_discrete(element_oid: u32) -> bool {
    matches!(element_oid, oid::INT4 | oid::INT8 | oid::DATE)
}

fn is_pg_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r')
}

#[derive(Debug, Clone)]
struct Bound {

    value: String,
    inclusive: bool,
    infinite: bool,
}

fn scan_bound(chars: &[char], start: usize) -> Result<(String, bool, usize), ()> {
    let mut buf = String::new();
    let mut in_quote = false;
    let mut saw_quote = false;
    let mut i = start;
    while i < chars.len() {
        let c = chars[i];
        if in_quote {
            if c == '\\' {
                i += 1;
                if i >= chars.len() {
                    return Err(());
                }
                buf.push(chars[i]);
                i += 1;
            } else if c == '"' {

                if i + 1 < chars.len() && chars[i + 1] == '"' {
                    buf.push('"');
                    i += 2;
                } else {
                    in_quote = false;
                    i += 1;
                }
            } else {
                buf.push(c);
                i += 1;
            }
        } else if c == '"' {
            in_quote = true;
            saw_quote = true;
            i += 1;
        } else if c == '\\' {
            i += 1;
            if i >= chars.len() {
                return Err(());
            }
            buf.push(chars[i]);
            i += 1;
        } else if c == ',' || c == ')' || c == ']' {
            break;
        } else {
            buf.push(c);
            i += 1;
        }
    }
    if in_quote {
        return Err(());
    }
    Ok((buf, saw_quote, i))
}

fn increment(element_oid: u32, value: &str) -> Result<String, PgError> {
    let stepped = match element_oid {
        oid::INT4 | oid::INT8 => {
            let n: i128 = value.parse().map_err(|_| PgError::InvalidInputSyntax {
                typ: type_name(element_oid),
                input: value.to_string(),
            })?;
            (n + 1).to_string()
        }
        oid::DATE => date_add_one_day(value)?,
        _ => return Ok(value.to_string()),
    };

    let v = super::input(element_oid, &stepped)?;
    Ok(super::output(element_oid, &v))
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn date_add_one_day(iso: &str) -> Result<String, PgError> {
    let bad = || PgError::InvalidInputSyntax {
        typ: "date",
        input: iso.to_string(),
    };
    let mut it = iso.split('-');
    let (Some(y), Some(m), Some(d), None) = (it.next(), it.next(), it.next(), it.next()) else {
        return Err(bad());
    };
    let (Ok(year), Ok(month), Ok(day)) = (y.parse::<i64>(), m.parse::<u32>(), d.parse::<u32>())
    else {
        return Err(bad());
    };
    let (mut year, mut month, mut day) = (year, month, day);
    day += 1;
    if day > days_in_month(year, month) {
        day = 1;
        month += 1;
        if month > 12 {
            month = 1;
            year += 1;
        }
    }
    Ok(format!("{year:04}-{month:02}-{day:02}"))
}

fn compare(element_oid: u32, a: &str, b: &str) -> Option<std::cmp::Ordering> {
    match element_oid {
        oid::INT4 | oid::INT8 => {
            let x: i128 = a.parse().ok()?;
            let y: i128 = b.parse().ok()?;
            Some(x.cmp(&y))
        }
        oid::DATE | oid::TIMESTAMP => Some(a.cmp(b)),
        oid::NUMERIC => {
            let x: f64 = a.parse().ok()?;
            let y: f64 = b.parse().ok()?;
            x.partial_cmp(&y)
        }
        _ => None,
    }
}

fn needs_quoting(s: &str) -> bool {
    s.is_empty()
        || s.chars()
            .any(|c| matches!(c, '"' | '\\' | '(' | ')' | '[' | ']' | ',') || is_pg_space(c))
}

fn render_value(s: &str) -> String {
    if !needs_quoting(s) {
        return s.to_string();
    }
    let mut out = String::from('"');
    for c in s.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    let elem = match element_oid(oid) {
        Some(e) => e,
        None => {
            return Err(PgError::InvalidInputSyntax {
                typ: type_name(oid),
                input: text.to_string(),
            })
        }
    };
    let malformed = || PgError::InvalidInputSyntax {
        typ: type_name(oid),
        input: text.to_string(),
    };

    let trimmed = text.trim_matches(is_pg_space);
    if trimmed.eq_ignore_ascii_case("empty") {
        return Ok(SqlValue::Text("empty".to_string()));
    }

    let chars: Vec<char> = trimmed.chars().collect();
    if chars.is_empty() {
        return Err(malformed());
    }

    let lower_inc = match chars[0] {
        '[' => true,
        '(' => false,
        _ => return Err(malformed()),
    };

    let (lower_raw, lower_quoted, comma_idx) = scan_bound(&chars, 1).map_err(|_| malformed())?;
    if comma_idx >= chars.len() || chars[comma_idx] != ',' {
        return Err(malformed());
    }

    let (upper_raw, upper_quoted, close_idx) =
        scan_bound(&chars, comma_idx + 1).map_err(|_| malformed())?;
    if close_idx >= chars.len() {
        return Err(malformed());
    }
    let upper_inc = match chars[close_idx] {
        ']' => true,
        ')' => false,
        _ => return Err(malformed()),
    };

    if close_idx + 1 != chars.len() {
        return Err(malformed());
    }

    let mut lower = if lower_raw.is_empty() && !lower_quoted {
        Bound {
            value: String::new(),
            inclusive: false,
            infinite: true,
        }
    } else {
        let v = super::input(elem, &lower_raw)?;
        Bound {
            value: super::output(elem, &v),
            inclusive: lower_inc,
            infinite: false,
        }
    };
    let mut upper = if upper_raw.is_empty() && !upper_quoted {
        Bound {
            value: String::new(),
            inclusive: false,
            infinite: true,
        }
    } else {
        let v = super::input(elem, &upper_raw)?;
        Bound {
            value: super::output(elem, &v),
            inclusive: upper_inc,
            infinite: false,
        }
    };

    if !lower.infinite && !upper.infinite {
        match compare(elem, &lower.value, &upper.value) {
            Some(std::cmp::Ordering::Greater) => return Err(malformed()),
            Some(std::cmp::Ordering::Equal) if !(lower.inclusive && upper.inclusive) => {
                return Ok(SqlValue::Text("empty".to_string()));
            }
            _ => {}
        }
    }

    if is_discrete(elem) {
        if !lower.infinite && !lower.inclusive {
            lower.value = increment(elem, &lower.value)?;
            lower.inclusive = true;
        }
        if !upper.infinite && upper.inclusive {
            upper.value = increment(elem, &upper.value)?;
            upper.inclusive = false;
        }

        if !lower.infinite
            && !upper.infinite
            && matches!(
                compare(elem, &lower.value, &upper.value),
                Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
            )
        {
            return Ok(SqlValue::Text("empty".to_string()));
        }
    }

    Ok(SqlValue::Text(render(&lower, &upper)))
}

fn render(lower: &Bound, upper: &Bound) -> String {
    let lb = if lower.inclusive { '[' } else { '(' };
    let rb = if upper.inclusive { ']' } else { ')' };
    let ls = if lower.infinite {
        String::new()
    } else {
        render_value(&lower.value)
    };
    let us = if upper.infinite {
        String::new()
    } else {
        render_value(&upper.value)
    };
    format!("{lb}{ls},{us}{rb}")
}

pub fn range_element_oid(range_oid: u32) -> Option<u32> {
    element_oid(range_oid)
}

#[derive(Debug, Clone, PartialEq)]
pub struct RangeParts {
    pub empty: bool,

    pub lower: Option<(String, bool)>,

    pub upper: Option<(String, bool)>,
}

pub fn looks_like_range(s: &str) -> bool {
    let t = s.trim_matches(is_pg_space);
    t.eq_ignore_ascii_case("empty") || matches!(t.chars().next(), Some('[') | Some('('))
}

pub fn parse_parts(text: &str) -> Option<RangeParts> {
    let trimmed = text.trim_matches(is_pg_space);
    if trimmed.eq_ignore_ascii_case("empty") {
        return Some(RangeParts {
            empty: true,
            lower: None,
            upper: None,
        });
    }
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.is_empty() {
        return None;
    }
    let lower_inc = match chars[0] {
        '[' => true,
        '(' => false,
        _ => return None,
    };
    let (lower_raw, lower_quoted, comma_idx) = scan_bound(&chars, 1).ok()?;
    if comma_idx >= chars.len() || chars[comma_idx] != ',' {
        return None;
    }
    let (upper_raw, upper_quoted, close_idx) = scan_bound(&chars, comma_idx + 1).ok()?;
    if close_idx >= chars.len() {
        return None;
    }
    let upper_inc = match chars[close_idx] {
        ']' => true,
        ')' => false,
        _ => return None,
    };
    if close_idx + 1 != chars.len() {
        return None;
    }
    let lower = if lower_raw.is_empty() && !lower_quoted {
        None
    } else {
        Some((lower_raw, lower_inc))
    };
    let upper = if upper_raw.is_empty() && !upper_quoted {
        None
    } else {
        Some((upper_raw, upper_inc))
    };
    Some(RangeParts {
        empty: false,
        lower,
        upper,
    })
}

pub fn render_parts(p: &RangeParts) -> String {
    if p.empty {
        return "empty".to_string();
    }
    let lower = match &p.lower {
        Some((v, inc)) => Bound {
            value: v.clone(),
            inclusive: *inc,
            infinite: false,
        },
        None => Bound {
            value: String::new(),
            inclusive: false,
            infinite: true,
        },
    };
    let upper = match &p.upper {
        Some((v, inc)) => Bound {
            value: v.clone(),
            inclusive: *inc,
            infinite: false,
        },
        None => Bound {
            value: String::new(),
            inclusive: false,
            infinite: true,
        },
    };
    render(&lower, &upper)
}

pub(crate) fn cmp_elem_text(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    if let (Ok(x), Ok(y)) = (a.parse::<i128>(), b.parse::<i128>()) {
        return x.cmp(&y);
    }
    if let (Ok(x), Ok(y)) = (a.parse::<f64>(), b.parse::<f64>()) {
        return x.partial_cmp(&y).unwrap_or(Ordering::Equal);
    }
    a.cmp(b)
}

fn cmp_lower(a: &Option<(String, bool)>, b: &Option<(String, bool)>) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some((av, ainc)), Some((bv, binc))) => match cmp_elem_text(av, bv) {
            Ordering::Equal => match (ainc, binc) {
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
                _ => Ordering::Equal,
            },
            other => other,
        },
    }
}

fn upper_max(a: &Option<(String, bool)>, b: &Option<(String, bool)>) -> Option<(String, bool)> {
    use std::cmp::Ordering;
    match (a, b) {
        (None, _) | (_, None) => None,
        (Some(x), Some(y)) => Some(match cmp_elem_text(&x.0, &y.0) {
            Ordering::Greater => x.clone(),
            Ordering::Less => y.clone(),
            Ordering::Equal => {
                if x.1 {
                    x.clone()
                } else {
                    y.clone()
                }
            }
        }),
    }
}

fn meets(a_upper: &Option<(String, bool)>, b_lower: &Option<(String, bool)>) -> bool {
    use std::cmp::Ordering;
    match (a_upper, b_lower) {
        (None, _) | (_, None) => true,
        (Some((uv, uinc)), Some((lv, linc))) => match cmp_elem_text(uv, lv) {
            Ordering::Greater => true,
            Ordering::Less => false,
            Ordering::Equal => *uinc || *linc,
        },
    }
}

pub(crate) fn normalize_multirange(mut parts: Vec<RangeParts>) -> Vec<RangeParts> {
    parts.retain(|p| !p.empty);
    parts.sort_by(|a, b| cmp_lower(&a.lower, &b.lower));
    let mut out: Vec<RangeParts> = Vec::new();
    for r in parts {
        if let Some(last) = out.last_mut() {

            if meets(&last.upper, &r.lower) {
                last.upper = upper_max(&last.upper, &r.upper);
                continue;
            }
        }
        out.push(r);
    }
    out
}

pub fn output(_oid: u32, v: &SqlValue) -> String {
    match v {
        SqlValue::Text(s) => s.clone(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INT4RANGE: u32 = oid::INT4RANGE;
    const INT8RANGE: u32 = oid::INT8RANGE;
    const NUMRANGE: u32 = oid::NUMRANGE;
    const TSRANGE: u32 = oid::TSRANGE;
    const DATERANGE: u32 = oid::DATERANGE;

    fn s(oid: u32, text: &str) -> String {
        match input(oid, text).unwrap() {
            SqlValue::Text(s) => s,
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn numrange_bracket_combos_preserved() {
        assert_eq!(s(NUMRANGE, "[1.1,2.2)"), "[1.1,2.2)");
        assert_eq!(s(NUMRANGE, "(1.1,2.2]"), "(1.1,2.2]");
        assert_eq!(s(NUMRANGE, "[1.1,2.2]"), "[1.1,2.2]");
        assert_eq!(s(NUMRANGE, "(1.1,2.2)"), "(1.1,2.2)");

        assert_eq!(s(NUMRANGE, "[1.7,1.7]"), "[1.7,1.7]");
    }

    #[test]
    fn whitespace_tolerance() {

        assert_eq!(s(NUMRANGE, "  [1.1, 2.2)  "), "[1.1,2.2)");
        assert_eq!(s(INT4RANGE, "[ 1 , 10 )"), "[1,10)");
    }

    #[test]
    fn int4range_canonicalizes_to_half_open() {

        assert_eq!(s(INT4RANGE, "[1,10]"), "[1,11)");
        assert_eq!(s(INT4RANGE, "[1,10)"), "[1,10)");
        assert_eq!(s(INT4RANGE, "(1,10]"), "[2,11)");
        assert_eq!(s(INT4RANGE, "(1,10)"), "[2,10)");
    }

    #[test]
    fn int8range_canonicalizes_to_half_open() {

        assert_eq!(
            s(INT8RANGE, "(10000000000,20000000000]"),
            "[10000000001,20000000001)"
        );
        assert_eq!(s(INT8RANGE, "[5,5]"), "[5,6)");
    }

    #[test]
    fn daterange_canonicalizes_by_one_day() {

        assert_eq!(
            s(DATERANGE, "[2000-01-10,2000-01-20]"),
            "[2000-01-10,2000-01-21)"
        );
        assert_eq!(
            s(DATERANGE, "[2000-01-10,2000-01-20)"),
            "[2000-01-10,2000-01-20)"
        );
        assert_eq!(
            s(DATERANGE, "(2000-01-10,2000-01-20]"),
            "[2000-01-11,2000-01-21)"
        );
        assert_eq!(
            s(DATERANGE, "(2000-01-10,2000-01-20)"),
            "[2000-01-11,2000-01-20)"
        );

        assert_eq!(
            s(DATERANGE, "[2000-01-31,2000-01-31]"),
            "[2000-01-31,2000-02-01)"
        );
        assert_eq!(
            s(DATERANGE, "[2000-12-31,2000-12-31]"),
            "[2000-12-31,2001-01-01)"
        );

        assert_eq!(
            s(DATERANGE, "[2000-02-29,2000-02-29]"),
            "[2000-02-29,2000-03-01)"
        );
    }

    #[test]
    fn unbounded_bounds_forced_exclusive() {

        assert_eq!(s(NUMRANGE, "[,)"), "(,)");
        assert_eq!(s(NUMRANGE, "[3,]"), "[3,)");
        assert_eq!(s(NUMRANGE, "[,5)"), "(,5)");
        assert_eq!(s(NUMRANGE, "(,)"), "(,)");

        assert_eq!(s(INT4RANGE, "[1,)"), "[1,)");
        assert_eq!(s(INT4RANGE, "(,10]"), "(,11)");
    }

    #[test]
    fn empty_literal() {
        assert_eq!(s(NUMRANGE, "empty"), "empty");
        assert_eq!(s(NUMRANGE, "  EMPTY  "), "empty");
        assert_eq!(s(INT4RANGE, "Empty"), "empty");
    }

    #[test]
    fn equal_bounds_with_exclusive_end_is_empty() {

        assert_eq!(s(NUMRANGE, "(1,1)"), "empty");
        assert_eq!(s(NUMRANGE, "[1,1)"), "empty");
        assert_eq!(s(NUMRANGE, "(1,1]"), "empty");
    }

    #[test]
    fn discrete_collapse_to_empty() {

        assert_eq!(s(INT4RANGE, "(1,2)"), "empty");
        assert_eq!(s(INT4RANGE, "[1,1)"), "empty");
        assert_eq!(s(DATERANGE, "(2000-01-10,2000-01-11)"), "empty");
    }

    #[test]
    fn lower_greater_than_upper_rejected() {
        assert_eq!(
            input(INT4RANGE, "(4,1)").unwrap_err(),
            PgError::InvalidInputSyntax {
                typ: "int4range",
                input: "(4,1)".into()
            }
        );
        assert_eq!(
            input(NUMRANGE, "[2.0,1.0)").unwrap_err(),
            PgError::InvalidInputSyntax {
                typ: "numrange",
                input: "[2.0,1.0)".into()
            }
        );
        assert_eq!(
            input(DATERANGE, "[2000-02-01,2000-01-01]").unwrap_err(),
            PgError::InvalidInputSyntax {
                typ: "daterange",
                input: "[2000-02-01,2000-01-01]".into()
            }
        );
    }

    #[test]
    fn bad_bound_surfaces_element_error() {

        assert_eq!(
            input(INT4RANGE, "(4,zed)").unwrap_err(),
            PgError::InvalidInputSyntax {
                typ: "integer",
                input: "zed".into()
            }
        );

        assert_eq!(
            input(DATERANGE, "[not-a-date,2000-01-01]").unwrap_err(),
            PgError::InvalidInputSyntax {
                typ: "date",
                input: "not-a-date".into()
            }
        );
    }

    #[test]
    fn overflow_on_canonicalization_surfaces_element_error() {

        let e = input(INT4RANGE, "[1,2147483647]").unwrap_err();
        assert_eq!(
            e,
            PgError::OutOfRange {
                typ: "integer",
                input: "2147483648".into()
            }
        );
    }

    #[test]
    fn malformed_literals_rejected() {
        for bad in [
            "",
            "1,10)",
            "(1,10",
            "[1,10",
            "abc",
            "(1,10) junk",
            "[1,2,3)",
        ] {
            let e = input(INT4RANGE, bad).unwrap_err();
            assert_eq!(
                e,
                PgError::InvalidInputSyntax {
                    typ: "int4range",
                    input: bad.into()
                },
                "expected malformed reject for {bad:?}"
            );
        }
    }

    #[test]
    fn unknown_oid_rejected() {
        assert_eq!(
            input(9999, "[1,10)").unwrap_err(),
            PgError::InvalidInputSyntax {
                typ: "unknown",
                input: "[1,10)".into()
            }
        );
    }

    #[test]
    fn tsrange_quotes_and_round_trips() {

        let out = s(TSRANGE, "[2010-01-02 10:00:00,2010-01-02 11:00:00)");
        assert_eq!(out, "[\"2010-01-02 10:00:00\",\"2010-01-02 11:00:00\")");

        assert_eq!(s(TSRANGE, &out), out);
    }

    #[test]
    fn round_trips() {
        let cases = [
            (INT4RANGE, "[1,10]"),
            (INT4RANGE, "(1,10)"),
            (INT8RANGE, "[100,200)"),
            (NUMRANGE, "[1.1,2.2]"),
            (NUMRANGE, "(,)"),
            (DATERANGE, "[2000-01-10,2000-01-20]"),
            (NUMRANGE, "empty"),
        ];
        for (oid, text) in cases {
            let v = input(oid, text).unwrap();
            let canon = output(oid, &v);

            assert_eq!(canon, s(oid, text));

            assert_eq!(s(oid, &canon), canon);
        }
    }

    #[test]
    fn output_non_text_is_empty() {
        assert_eq!(output(INT4RANGE, &SqlValue::Null), "");
        assert_eq!(output(INT4RANGE, &SqlValue::Int(5)), "");
    }

    #[test]
    fn error_message_wording() {
        let e = input(INT4RANGE, "(4,1)").unwrap_err();
        assert_eq!(
            e.message(),
            "invalid input syntax for type int4range: \"(4,1)\""
        );
    }
}
