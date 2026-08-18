
use crate::expr::operators::multiranges as mr_ops;
use crate::types::multiranges::{looks_like_multirange, parse_components};
use crate::types::{oid, PgError};
use sql_core::SqlValue;

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    match name {
        "int4multirange" => return Some(construct(oid::INT4MULTIRANGE, args)),
        "int8multirange" => return Some(construct(oid::INT8MULTIRANGE, args)),
        "nummultirange" => return Some(construct(oid::NUMMULTIRANGE, args)),
        "tsmultirange" => return Some(construct(oid::TSMULTIRANGE, args)),
        "datemultirange" => return Some(construct(oid::DATEMULTIRANGE, args)),

        "multirange" => {
            let [a] = args else { return None };
            return Some(construct_generic(a));
        }
        "range_merge" => {

            let [a] = args else { return None };
            return mr_ops::range_merge_mr(a);
        }
        _ => {}
    }

    let [arg] = args else { return None };
    if matches!(arg, SqlValue::Null) {
        return match name {
            "isempty" | "lower" | "upper" => Some(Ok(SqlValue::Null)),
            _ => None,
        };
    }
    let parts = match arg {
        SqlValue::Text(s) if looks_like_multirange(s) => parse_components(s)?,
        _ => return None,
    };

    Some(Ok(match name {
        "isempty" => bool_val(parts.is_empty()),

        "lower" => parts
            .first()
            .map(|r| bound_value(&r.lower))
            .unwrap_or(SqlValue::Null),

        "upper" => parts
            .last()
            .map(|r| bound_value(&r.upper))
            .unwrap_or(SqlValue::Null),
        _ => return None,
    }))
}

fn bool_val(b: bool) -> SqlValue {
    SqlValue::Int(if b { 1 } else { 0 })
}

fn bound_value(bound: &Option<(String, bool)>) -> SqlValue {
    match bound {
        None => SqlValue::Null,
        Some((v, _)) => match v.parse::<i64>() {
            Ok(n) => SqlValue::Int(n),
            Err(_) => SqlValue::Text(v.clone()),
        },
    }
}

fn construct(mr_oid: u32, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    let mut bodies = Vec::with_capacity(args.len());
    for a in args {
        match a {
            SqlValue::Null => continue,
            SqlValue::Text(s) => bodies.push(s.clone()),
            _ => {
                return Err(PgError::InvalidInputSyntax {
                    typ: "multirange",
                    input: "constructor arguments must be ranges".to_string(),
                })
            }
        }
    }
    let lit = format!("{{{}}}", bodies.join(","));
    crate::types::multiranges::input(mr_oid, &lit)
}

fn construct_generic(a: &SqlValue) -> Result<SqlValue, PgError> {
    let body = match a {
        SqlValue::Null => String::new(),
        SqlValue::Text(s) => s.clone(),
        _ => {
            return Err(PgError::InvalidInputSyntax {
                typ: "multirange",
                input: "argument must be a range".to_string(),
            })
        }
    };
    let lit = if body.is_empty() {
        "{}".to_string()
    } else {
        format!("{{{body}}}")
    };

    crate::types::multiranges::input(oid::INT4MULTIRANGE, &lit).or_else(|_| {

        use crate::types::ranges::{normalize_multirange, parse_parts};
        let parts = if body.is_empty() {
            vec![]
        } else {
            parse_parts(&body).into_iter().collect::<Vec<_>>()
        };
        Ok(SqlValue::Text(crate::types::multiranges::render(
            normalize_multirange(parts),
        )))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tr(s: &str) -> SqlValue {
        SqlValue::Text(s.to_string())
    }
    fn call1(name: &str, args: &[SqlValue]) -> SqlValue {
        call(name, args).unwrap().unwrap()
    }

    #[test]
    fn constructors() {
        assert_eq!(call1("int4multirange", &[]), tr("{}"));
        assert_eq!(call1("int4multirange", &[tr("[1,5)")]), tr("{[1,5)}"));
        assert_eq!(
            call1("int4multirange", &[tr("[1,5)"), tr("[8,10)")]),
            tr("{[1,5),[8,10)}")
        );

        assert_eq!(
            call1("int4multirange", &[tr("[1,5)"), tr("[4,9)")]),
            tr("{[1,9)}")
        );

        assert_eq!(call1("multirange", &[tr("[1,5)")]), tr("{[1,5)}"));
    }

    #[test]
    fn accessors() {
        assert_eq!(call1("lower", &[tr("{[1,5),[8,10)}")]), SqlValue::Int(1));
        assert_eq!(call1("upper", &[tr("{[1,5),[8,10)}")]), SqlValue::Int(10));
        assert_eq!(call1("isempty", &[tr("{}")]), SqlValue::Int(1));
        assert_eq!(call1("isempty", &[tr("{[1,5)}")]), SqlValue::Int(0));
        assert_eq!(call1("lower", &[tr("{}")]), SqlValue::Null);
    }

    #[test]
    fn range_merge_one_arg() {
        assert_eq!(call1("range_merge", &[tr("{[1,5),[8,10)}")]), tr("[1,10)"));
    }

    #[test]
    fn declines_non_multirange() {

        assert!(call("lower", &[tr("[1,5)")]).is_none());
        assert!(call("isempty", &[tr("[1,5)")]).is_none());

        assert!(call("range_merge", &[tr("[1,5)"), tr("[8,10)")]).is_none());
    }
}
