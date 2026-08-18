
use crate::types::{tsquery, tsvector, PgError};
use sql_core::SqlValue;

pub fn binary(op: &str, l: &SqlValue, r: &SqlValue) -> Option<Result<SqlValue, PgError>> {
    if op != "@@" {
        return None;
    }

    if matches!(l, SqlValue::Null) || matches!(r, SqlValue::Null) {
        return Some(Ok(SqlValue::Null));
    }
    let (lt, rt) = match (l, r) {
        (SqlValue::Text(a), SqlValue::Text(b)) => (a, b),

        _ => return None,
    };
    let b = |matched: bool| SqlValue::Int(if matched { 1 } else { 0 });

    match (tsvector::parse(lt), tsquery::parse(rt)) {
        (Ok(entries), Ok(query)) => Some(Ok(b(tsquery::matches(&query, &entries)))),
        normal => {

            if let (Ok(query), Ok(entries)) = (tsquery::parse(lt), tsvector::parse(rt)) {
                return Some(Ok(b(tsquery::matches(&query, &entries))));
            }

            Some(Err(match normal {
                (Err(e), _) | (_, Err(e)) => e,
                (Ok(_), Ok(_)) => unreachable!(),
            }))
        }
    }
}

pub fn unary(op: &str, v: &SqlValue) -> Option<Result<SqlValue, PgError>> {
    let _ = (op, v);
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(vec: &str, q: &str) -> bool {
        match binary("@@", &SqlValue::Text(vec.into()), &SqlValue::Text(q.into())) {
            Some(Ok(SqlValue::Int(n))) => n == 1,
            other => panic!("@@ => {other:?}"),
        }
    }

    #[test]
    fn matches_canonical_forms() {
        assert!(m("'a':1 'cat':3 'fat':2", "'cat' & 'fat'"));
        assert!(!m("'a':1 'cat':3 'fat':2", "'cat' & 'dog'"));
        assert!(m("'a':1 'cat':3 'fat':2", "'fat' <-> 'cat'"));
    }

    #[test]
    fn text_coercion_bare_words() {

        assert!(m("a fat cat", "'cat'"));
    }

    fn m_rev(q: &str, vec: &str) -> bool {
        match binary("@@", &SqlValue::Text(q.into()), &SqlValue::Text(vec.into())) {
            Some(Ok(SqlValue::Int(n))) => n == 1,
            other => panic!("@@ => {other:?}"),
        }
    }

    #[test]
    fn reversed_operand_order_commutes() {

        assert!(m_rev("'cat'", "'a':1 'cat':3 'fat':2"));
        assert!(!m_rev("'dog'", "'a':1 'cat':3 'fat':2"));
        assert!(m_rev("'cat' & 'fat'", "'a':1 'cat':3 'fat':2"));
        assert!(!m_rev("'cat' & 'dog'", "'a':1 'cat':3 'fat':2"));
        assert!(m_rev("'fat' <-> 'cat'", "'a':1 'cat':3 'fat':2"));

        assert!(m_rev("'cat'", "'a' 'cat' 'fat'"));
    }

    #[test]
    fn strict_null() {
        assert_eq!(
            binary("@@", &SqlValue::Null, &SqlValue::Text("'a'".into())),
            Some(Ok(SqlValue::Null))
        );
    }

    #[test]
    fn other_op_falls_through() {
        assert!(binary(
            "->",
            &SqlValue::Text("a".into()),
            &SqlValue::Text("b".into())
        )
        .is_none());
    }
}
