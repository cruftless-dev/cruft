
use super::ranges as rops;
use crate::types::multiranges::{looks_like_multirange, parse_components};
use crate::types::ranges::{
    looks_like_range, normalize_multirange, parse_parts, render_parts, RangeParts,
};
use crate::types::{multiranges, PgError};
use sql_core::SqlValue;

pub fn binary(op: &str, l: &SqlValue, r: &SqlValue) -> Option<Result<SqlValue, PgError>> {
    match op {
        "@>" | "<@" | "&&" => {}
        _ => return None,
    }

    if matches!(l, SqlValue::Null) || matches!(r, SqlValue::Null) {

        let touches_mr = as_multirange(l).is_some()
            || as_multirange(r).is_some()
            || (matches!(l, SqlValue::Null) && matches!(r, SqlValue::Null));
        if !touches_mr {
            return None;
        }
        return Some(Ok(SqlValue::Null));
    }

    match op {
        "@>" => {

            let a = as_multirange(l)?;
            Some(Ok(bool_val(contains_right(&a, r)?)))
        }
        "<@" => {

            let b = as_multirange(r)?;
            Some(Ok(bool_val(contains_right(&b, l)?)))
        }
        "&&" => {

            let a = as_multirange_or_range(l)?;
            let b = as_multirange_or_range(r)?;
            if !a.0 && !b.0 {
                return None;
            }
            Some(Ok(bool_val(overlaps_mr(&a.1, &b.1))))
        }
        _ => unreachable!("guarded above"),
    }
}

pub fn set_op(op: char, l: &SqlValue, r: &SqlValue) -> Option<Result<SqlValue, PgError>> {
    let a = as_multirange(l)?;
    let b = as_multirange(r)?;
    let parts = match op {
        '+' => union(a, b),
        '*' => intersect(&a, &b),
        '-' => difference(a, &b),
        _ => return None,
    };
    Some(Ok(SqlValue::Text(multiranges::render(
        normalize_multirange(parts),
    ))))
}

fn as_multirange(v: &SqlValue) -> Option<Vec<RangeParts>> {
    match v {
        SqlValue::Text(s) if looks_like_multirange(s) => parse_components(s),
        _ => None,
    }
}

fn as_multirange_or_range(v: &SqlValue) -> Option<(bool, Vec<RangeParts>)> {
    if let Some(m) = as_multirange(v) {
        return Some((true, m));
    }
    match v {
        SqlValue::Text(s) if looks_like_range(s) => {
            let p = parse_parts(s)?;
            Some((false, if p.empty { vec![] } else { vec![p] }))
        }
        _ => None,
    }
}

fn bool_val(b: bool) -> SqlValue {
    SqlValue::Int(if b { 1 } else { 0 })
}

fn contains_right(a: &[RangeParts], right: &SqlValue) -> Option<bool> {
    if let Some(b) = as_multirange(right) {
        return Some(contains_mr(a, &b));
    }
    match right {
        SqlValue::Text(s) if looks_like_range(s) => {
            let rb = parse_parts(s)?;
            if rb.empty {
                return Some(true);
            }
            Some(a.iter().any(|ra| rops::contains_range(ra, &rb)))
        }
        SqlValue::Int(n) => Some(a.iter().any(|ra| rops::contains_elem(ra, &n.to_string()))),
        SqlValue::Real(f) => Some(a.iter().any(|ra| rops::contains_elem(ra, &format!("{f}")))),
        SqlValue::Text(s) => Some(a.iter().any(|ra| rops::contains_elem(ra, s))),
        _ => None,
    }
}

fn contains_mr(a: &[RangeParts], b: &[RangeParts]) -> bool {
    b.iter()
        .all(|rb| a.iter().any(|ra| rops::contains_range(ra, rb)))
}

fn overlaps_mr(a: &[RangeParts], b: &[RangeParts]) -> bool {
    a.iter().any(|ra| b.iter().any(|rb| rops::overlaps(ra, rb)))
}

fn union(mut a: Vec<RangeParts>, mut b: Vec<RangeParts>) -> Vec<RangeParts> {
    a.append(&mut b);
    a
}

fn intersect(a: &[RangeParts], b: &[RangeParts]) -> Vec<RangeParts> {
    let mut out = Vec::new();
    for ra in a {
        for rb in b {
            let i = rops::intersect(ra, rb);
            if !i.empty {
                out.push(i);
            }
        }
    }
    out
}

fn difference(a: Vec<RangeParts>, b: &[RangeParts]) -> Vec<RangeParts> {
    let mut cur = a;
    for rb in b {
        let mut next = Vec::new();
        for ra in &cur {
            next.extend(range_minus_split(ra, rb));
        }
        cur = next;
    }
    cur
}

fn range_minus_split(a: &RangeParts, b: &RangeParts) -> Vec<RangeParts> {
    if a.empty || b.empty || !rops::overlaps(a, b) {
        return if a.empty { vec![] } else { vec![a.clone()] };
    }
    let a_starts_within_b = rops::lower_le(&b.lower, &a.lower);
    let a_ends_within_b = rops::upper_ge(&b.upper, &a.upper);

    if a_starts_within_b && a_ends_within_b {
        return vec![];
    }
    let mut out = Vec::new();
    if a_starts_within_b {

        push_if_nonempty(&mut out, rops::flip(&b.upper), a.upper.clone());
    } else if a_ends_within_b {

        push_if_nonempty(&mut out, a.lower.clone(), rops::flip(&b.lower));
    } else {

        push_if_nonempty(&mut out, a.lower.clone(), rops::flip(&b.lower));
        push_if_nonempty(&mut out, rops::flip(&b.upper), a.upper.clone());
    }
    out
}

fn push_if_nonempty(
    out: &mut Vec<RangeParts>,
    lower: Option<(String, bool)>,
    upper: Option<(String, bool)>,
) {
    if !rops::is_empty_span(&lower, &upper) {
        out.push(RangeParts {
            empty: false,
            lower,
            upper,
        });
    }
}

pub fn range_merge_mr(v: &SqlValue) -> Option<Result<SqlValue, PgError>> {
    if matches!(v, SqlValue::Null) {
        return Some(Ok(SqlValue::Null));
    }
    let parts = as_multirange(v)?;
    let out = match (parts.first(), parts.last()) {
        (Some(first), Some(last)) => RangeParts {
            empty: false,
            lower: first.lower.clone(),
            upper: last.upper.clone(),
        },
        _ => RangeParts {
            empty: true,
            lower: None,
            upper: None,
        },
    };
    Some(Ok(SqlValue::Text(render_parts(&out))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> SqlValue {
        SqlValue::Text(s.to_string())
    }
    fn b(op: &str, l: &SqlValue, r: &SqlValue) -> bool {
        matches!(binary(op, l, r).unwrap().unwrap(), SqlValue::Int(1))
    }

    #[test]
    fn contains_element_range_multirange() {
        assert!(b("@>", &t("{[1,5),[8,10)}"), &SqlValue::Int(3)));
        assert!(!b("@>", &t("{[1,5),[8,10)}"), &SqlValue::Int(6)));
        assert!(b("@>", &t("{[1,5),[8,10)}"), &t("[2,4)")));
        assert!(!b("@>", &t("{[1,5),[8,10)}"), &t("[4,9)")));
        assert!(b("@>", &t("{[1,10)}"), &t("{[2,5),[6,8)}")));
        assert!(b("<@", &t("[2,4)"), &t("{[1,5)}")));
    }

    #[test]
    fn overlap() {
        assert!(b("&&", &t("{[1,5)}"), &t("{[4,9)}")));
        assert!(!b("&&", &t("{[1,5)}"), &t("{[5,9)}")));
        assert!(b("&&", &t("{[1,5)}"), &t("[4,9)")));
    }

    #[test]
    fn union_intersection_difference() {
        assert_eq!(
            set_op('+', &t("{[1,5)}"), &t("{[8,10)}")).unwrap().unwrap(),
            t("{[1,5),[8,10)}")
        );
        assert_eq!(
            set_op('+', &t("{[1,5)}"), &t("{[4,10)}")).unwrap().unwrap(),
            t("{[1,10)}")
        );
        assert_eq!(
            set_op('*', &t("{[1,10)}"), &t("{[2,5),[8,20)}"))
                .unwrap()
                .unwrap(),
            t("{[2,5),[8,10)}")
        );

        assert_eq!(
            set_op('-', &t("{[1,10)}"), &t("{[3,5)}")).unwrap().unwrap(),
            t("{[1,3),[5,10)}")
        );

        assert_eq!(
            set_op('-', &t("{[1,5)}"), &t("{[1,10)}")).unwrap().unwrap(),
            t("{}")
        );
    }

    #[test]
    fn range_merge_spans_gap() {
        assert_eq!(
            range_merge_mr(&t("{[1,5),[8,10)}")).unwrap().unwrap(),
            t("[1,10)")
        );
        assert_eq!(range_merge_mr(&t("{}")).unwrap().unwrap(), t("empty"));
    }

    #[test]
    fn declines_non_multirange() {
        assert!(set_op('+', &t("[1,5)"), &t("[4,10)")).is_none());
        assert!(binary("@>", &SqlValue::Int(1), &SqlValue::Int(2)).is_none());
    }
}
