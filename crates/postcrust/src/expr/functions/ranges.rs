
use crate::expr::operators::ranges as range_ops;
use crate::types::ranges::{looks_like_range, parse_parts};
use crate::types::{oid, PgError};
use sql_core::SqlValue;

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    match name {
        "int4range" => return Some(construct(oid::INT4RANGE, args)),
        "int8range" => return Some(construct(oid::INT8RANGE, args)),
        "numrange" => return Some(construct(oid::NUMRANGE, args)),
        "tsrange" => return Some(construct(oid::TSRANGE, args)),
        "daterange" => return Some(construct(oid::DATERANGE, args)),
        "range_merge" => {
            let [a, b] = args else { return None };
            return range_ops::range_merge(a, b);
        }
        _ => {}
    }

    let [arg] = args else { return None };

    if matches!(arg, SqlValue::Null) {

        return match name {
            "isempty" | "lower_inc" | "upper_inc" | "lower_inf" | "upper_inf" | "lower"
            | "upper" => Some(Ok(SqlValue::Null)),
            _ => None,
        };
    }
    let r = match arg {
        SqlValue::Text(s) if looks_like_range(s) => parse_parts(s)?,

        _ => return None,
    };

    Some(Ok(match name {
        "isempty" => bool_val(r.empty),
        "lower" => bound_value(&r.lower, r.empty),
        "upper" => bound_value(&r.upper, r.empty),
        "lower_inc" => bool_val(!r.empty && matches!(&r.lower, Some((_, inc)) if *inc)),
        "upper_inc" => bool_val(!r.empty && matches!(&r.upper, Some((_, inc)) if *inc)),
        "lower_inf" => bool_val(!r.empty && r.lower.is_none()),
        "upper_inf" => bool_val(!r.empty && r.upper.is_none()),
        _ => return None,
    }))
}

fn bool_val(b: bool) -> SqlValue {
    SqlValue::Int(if b { 1 } else { 0 })
}

fn bound_value(bound: &Option<(String, bool)>, empty: bool) -> SqlValue {
    if empty {
        return SqlValue::Null;
    }
    match bound {
        None => SqlValue::Null,
        Some((v, _)) => match v.parse::<i64>() {
            Ok(n) => SqlValue::Int(n),
            Err(_) => SqlValue::Text(v.clone()),
        },
    }
}

fn construct(range_oid: u32, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    let (a, b, bounds) = match args {
        [a, b] => (a, b, "[)"),
        [a, b, SqlValue::Text(s)] => (a, b, s.as_str()),
        _ => {
            return Err(PgError::InvalidInputSyntax {
                typ: "range",
                input: "constructor requires 2 or 3 arguments".to_string(),
            })
        }
    };
    let mut bchars = bounds.chars();
    let (lb, rb) = (bchars.next().unwrap_or('['), bchars.next().unwrap_or(')'));

    let lo = elem_text(a);
    let hi = elem_text(b);
    let lit = format!(
        "{lb}{},{}{rb}",
        lo.map(|s| quote_if_needed(&s)).unwrap_or_default(),
        hi.map(|s| quote_if_needed(&s)).unwrap_or_default(),
    );
    crate::types::ranges::input(range_oid, &lit)
}

fn elem_text(v: &SqlValue) -> Option<String> {
    match v {
        SqlValue::Null => None,
        SqlValue::Int(n) => Some(n.to_string()),
        SqlValue::Real(f) => Some(format!("{f}")),
        SqlValue::Text(s) => Some(s.clone()),
        SqlValue::Blob(_) => Some(String::new()),
    }
}

fn quote_if_needed(s: &str) -> String {
    let needs = s.is_empty()
        || s.chars()
            .any(|c| matches!(c, '"' | '\\' | '(' | ')' | '[' | ']' | ',') || c.is_whitespace());
    if !needs {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn call1(name: &str, s: &str) -> SqlValue {
        call(name, &[SqlValue::Text(s.to_string())])
            .unwrap()
            .unwrap()
    }

    #[test]
    fn constructors_canonicalize() {
        assert_eq!(
            call("int4range", &[SqlValue::Int(1), SqlValue::Int(5)])
                .unwrap()
                .unwrap(),
            SqlValue::Text("[1,5)".into())
        );
        assert_eq!(
            call(
                "int4range",
                &[
                    SqlValue::Int(1),
                    SqlValue::Int(5),
                    SqlValue::Text("[]".into())
                ]
            )
            .unwrap()
            .unwrap(),
            SqlValue::Text("[1,6)".into())
        );

        assert_eq!(
            call("int4range", &[SqlValue::Null, SqlValue::Int(5)])
                .unwrap()
                .unwrap(),
            SqlValue::Text("(,5)".into())
        );
    }

    #[test]
    fn accessors() {
        assert_eq!(call1("lower", "[1,10)"), SqlValue::Int(1));
        assert_eq!(call1("upper", "[1,10)"), SqlValue::Int(10));
        assert_eq!(call1("isempty", "empty"), SqlValue::Int(1));
        assert_eq!(call1("isempty", "[1,10)"), SqlValue::Int(0));
        assert_eq!(call1("lower_inc", "[1,10)"), SqlValue::Int(1));
        assert_eq!(call1("upper_inc", "[1,10)"), SqlValue::Int(0));
        assert_eq!(call1("lower_inf", "(,5)"), SqlValue::Int(1));
        assert_eq!(call1("upper_inf", "[1,)"), SqlValue::Int(1));
        assert_eq!(call1("lower", "empty"), SqlValue::Null);
    }

    #[test]
    fn lower_upper_decline_non_range() {

        assert!(call("lower", &[SqlValue::Text("ABC".into())]).is_none());
        assert!(call("upper", &[SqlValue::Int(5)]).is_none());
    }
}
