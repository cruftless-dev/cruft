
use crate::types::ranges::{looks_like_range, parse_parts, render_parts, RangeParts};
use crate::types::PgError;
use sql_core::SqlValue;
use std::cmp::Ordering;

pub fn binary(op: &str, l: &SqlValue, r: &SqlValue) -> Option<Result<SqlValue, PgError>> {
    match op {
        "@>" | "<@" | "&&" | "-|-" | "<<" | ">>" => {}
        _ => return None,
    }

    if matches!(l, SqlValue::Null) || matches!(r, SqlValue::Null) {
        return Some(Ok(SqlValue::Null));
    }

    let lr = as_range(l);
    let rr = as_range(r);

    match op {
        "@>" => match (lr, rr) {
            (Some(a), Some(b)) => Some(Ok(bool_val(contains_range(&a, &b)))),
            (Some(a), None) => elem_str(r).map(|e| Ok(bool_val(contains_elem(&a, &e)))),
            _ => None,
        },
        "<@" => match (lr, rr) {
            (Some(a), Some(b)) => Some(Ok(bool_val(contains_range(&b, &a)))),
            (None, Some(b)) => elem_str(l).map(|e| Ok(bool_val(contains_elem(&b, &e)))),
            _ => None,
        },
        "&&" => pair(lr, rr, |a, b| overlaps(a, b)),
        "-|-" => pair(lr, rr, |a, b| adjacent(a, b)),
        "<<" => pair(lr, rr, |a, b| strictly_left(a, b)),
        ">>" => pair(lr, rr, |a, b| strictly_left(b, a)),
        _ => unreachable!("guarded above"),
    }
}

pub fn set_op(op: char, l: &SqlValue, r: &SqlValue) -> Option<Result<SqlValue, PgError>> {
    let a = as_range(l)?;
    let b = as_range(r)?;
    let res = match op {
        '+' => union(&a, &b),
        '*' => Ok(intersect(&a, &b)),
        '-' => difference(&a, &b),
        _ => return None,
    };
    Some(res.map(|p| SqlValue::Text(render_parts(&p))))
}

pub fn range_merge(a: &SqlValue, b: &SqlValue) -> Option<Result<SqlValue, PgError>> {
    if matches!(a, SqlValue::Null) || matches!(b, SqlValue::Null) {
        return Some(Ok(SqlValue::Null));
    }
    let a = as_range(a)?;
    let b = as_range(b)?;
    let p = merge(&a, &b);
    Some(Ok(SqlValue::Text(render_parts(&p))))
}

fn as_range(v: &SqlValue) -> Option<RangeParts> {
    match v {
        SqlValue::Text(s) if looks_like_range(s) => parse_parts(s),
        _ => None,
    }
}

fn elem_str(v: &SqlValue) -> Option<String> {
    match v {
        SqlValue::Int(n) => Some(n.to_string()),
        SqlValue::Real(f) => Some(format!("{f}")),
        SqlValue::Text(s) => Some(s.clone()),
        _ => None,
    }
}

fn bool_val(b: bool) -> SqlValue {
    SqlValue::Int(if b { 1 } else { 0 })
}

fn pair(
    a: Option<RangeParts>,
    b: Option<RangeParts>,
    f: impl Fn(&RangeParts, &RangeParts) -> bool,
) -> Option<Result<SqlValue, PgError>> {
    match (a, b) {
        (Some(a), Some(b)) => Some(Ok(bool_val(f(&a, &b)))),
        _ => None,
    }
}

fn cmp_elem(a: &str, b: &str) -> Ordering {
    if let (Ok(x), Ok(y)) = (a.parse::<i128>(), b.parse::<i128>()) {
        return x.cmp(&y);
    }
    if let (Ok(x), Ok(y)) = (a.parse::<f64>(), b.parse::<f64>()) {
        return x.partial_cmp(&y).unwrap_or(Ordering::Equal);
    }
    a.cmp(b)
}

pub(crate) fn contains_elem(r: &RangeParts, e: &str) -> bool {
    if r.empty {
        return false;
    }
    let lower_ok = match &r.lower {
        None => true,
        Some((lo, inc)) => match cmp_elem(e, lo) {
            Ordering::Greater => true,
            Ordering::Equal => *inc,
            Ordering::Less => false,
        },
    };
    let upper_ok = match &r.upper {
        None => true,
        Some((hi, inc)) => match cmp_elem(e, hi) {
            Ordering::Less => true,
            Ordering::Equal => *inc,
            Ordering::Greater => false,
        },
    };
    lower_ok && upper_ok
}

pub(crate) fn contains_range(a: &RangeParts, b: &RangeParts) -> bool {
    if b.empty {
        return true;
    }
    if a.empty {
        return false;
    }
    lower_le(&a.lower, &b.lower) && upper_ge(&a.upper, &b.upper)
}

pub(crate) fn lower_le(a: &Option<(String, bool)>, b: &Option<(String, bool)>) -> bool {
    match (a, b) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some((av, ainc)), Some((bv, binc))) => match cmp_elem(av, bv) {
            Ordering::Less => true,
            Ordering::Greater => false,

            Ordering::Equal => *ainc || !*binc,
        },
    }
}

pub(crate) fn upper_ge(a: &Option<(String, bool)>, b: &Option<(String, bool)>) -> bool {
    match (a, b) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some((av, ainc)), Some((bv, binc))) => match cmp_elem(av, bv) {
            Ordering::Greater => true,
            Ordering::Less => false,
            Ordering::Equal => *ainc || !*binc,
        },
    }
}

fn strictly_left(a: &RangeParts, b: &RangeParts) -> bool {
    if a.empty || b.empty {
        return false;
    }
    let (Some((ahi, ainc)), Some((blo, binc))) = (&a.upper, &b.lower) else {
        return false;
    };
    match cmp_elem(ahi, blo) {
        Ordering::Less => true,
        Ordering::Greater => false,

        Ordering::Equal => !(*ainc && *binc),
    }
}

pub(crate) fn overlaps(a: &RangeParts, b: &RangeParts) -> bool {
    !a.empty && !b.empty && !strictly_left(a, b) && !strictly_left(b, a)
}

fn adjacent(a: &RangeParts, b: &RangeParts) -> bool {
    if a.empty || b.empty {
        return false;
    }
    touch(&a.upper, &b.lower) || touch(&b.upper, &a.lower)
}

fn touch(upper: &Option<(String, bool)>, lower: &Option<(String, bool)>) -> bool {
    match (upper, lower) {
        (Some((uv, uinc)), Some((lv, linc))) => {
            cmp_elem(uv, lv) == Ordering::Equal && (uinc != linc)
        }
        _ => false,
    }
}

fn lower_min(a: &Option<(String, bool)>, b: &Option<(String, bool)>) -> Option<(String, bool)> {
    match (a, b) {
        (None, _) | (_, None) => None,
        (Some(x), Some(y)) => Some(match cmp_elem(&x.0, &y.0) {
            Ordering::Less => x.clone(),
            Ordering::Greater => y.clone(),
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

fn lower_max(a: &Option<(String, bool)>, b: &Option<(String, bool)>) -> Option<(String, bool)> {
    match (a, b) {
        (None, other) | (other, None) => other.clone(),
        (Some(x), Some(y)) => Some(match cmp_elem(&x.0, &y.0) {
            Ordering::Greater => x.clone(),
            Ordering::Less => y.clone(),
            Ordering::Equal => {
                if !x.1 {
                    x.clone()
                } else {
                    y.clone()
                }
            }
        }),
    }
}

fn upper_max(a: &Option<(String, bool)>, b: &Option<(String, bool)>) -> Option<(String, bool)> {
    match (a, b) {
        (None, _) | (_, None) => None,
        (Some(x), Some(y)) => Some(match cmp_elem(&x.0, &y.0) {
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

fn upper_min(a: &Option<(String, bool)>, b: &Option<(String, bool)>) -> Option<(String, bool)> {
    match (a, b) {
        (None, other) | (other, None) => other.clone(),
        (Some(x), Some(y)) => Some(match cmp_elem(&x.0, &y.0) {
            Ordering::Less => x.clone(),
            Ordering::Greater => y.clone(),
            Ordering::Equal => {
                if !x.1 {
                    x.clone()
                } else {
                    y.clone()
                }
            }
        }),
    }
}

pub(crate) fn is_empty_span(
    lower: &Option<(String, bool)>,
    upper: &Option<(String, bool)>,
) -> bool {
    let (Some((lv, linc)), Some((uv, uinc))) = (lower, upper) else {
        return false;
    };
    match cmp_elem(lv, uv) {
        Ordering::Less => false,
        Ordering::Greater => true,
        Ordering::Equal => !(*linc && *uinc),
    }
}

fn union(a: &RangeParts, b: &RangeParts) -> Result<RangeParts, PgError> {
    if a.empty {
        return Ok(b.clone());
    }
    if b.empty {
        return Ok(a.clone());
    }
    if !overlaps(a, b) && !adjacent(a, b) {
        return Err(PgError::InvalidInputSyntax {
            typ: "range",
            input: "result of range union would not be contiguous".to_string(),
        });
    }
    Ok(RangeParts {
        empty: false,
        lower: lower_min(&a.lower, &b.lower),
        upper: upper_max(&a.upper, &b.upper),
    })
}

fn merge(a: &RangeParts, b: &RangeParts) -> RangeParts {
    if a.empty {
        return b.clone();
    }
    if b.empty {
        return a.clone();
    }
    RangeParts {
        empty: false,
        lower: lower_min(&a.lower, &b.lower),
        upper: upper_max(&a.upper, &b.upper),
    }
}

pub(crate) fn intersect(a: &RangeParts, b: &RangeParts) -> RangeParts {
    if a.empty || b.empty || !overlaps(a, b) {
        return RangeParts {
            empty: true,
            lower: None,
            upper: None,
        };
    }
    let lower = lower_max(&a.lower, &b.lower);
    let upper = upper_min(&a.upper, &b.upper);
    if is_empty_span(&lower, &upper) {
        return RangeParts {
            empty: true,
            lower: None,
            upper: None,
        };
    }
    RangeParts {
        empty: false,
        lower,
        upper,
    }
}

fn difference(a: &RangeParts, b: &RangeParts) -> Result<RangeParts, PgError> {

    if a.empty || b.empty || !overlaps(a, b) {
        return Ok(a.clone());
    }
    let a_starts_within_b = lower_le(&b.lower, &a.lower);
    let a_ends_within_b = upper_ge(&b.upper, &a.upper);

    if a_starts_within_b && a_ends_within_b {
        return Ok(RangeParts {
            empty: true,
            lower: None,
            upper: None,
        });
    }

    if !a_starts_within_b && !a_ends_within_b {
        return Err(PgError::InvalidInputSyntax {
            typ: "range",
            input: "result of range difference would not be contiguous".to_string(),
        });
    }
    if a_starts_within_b {

        Ok(RangeParts {
            empty: false,
            lower: flip(&b.upper),
            upper: a.upper.clone(),
        })
    } else {

        Ok(RangeParts {
            empty: false,
            lower: a.lower.clone(),
            upper: flip(&b.lower),
        })
    }
}

pub(crate) fn flip(b: &Option<(String, bool)>) -> Option<(String, bool)> {
    b.as_ref().map(|(v, inc)| (v.clone(), !*inc))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tr(s: &str) -> SqlValue {
        SqlValue::Text(s.to_string())
    }
    fn b(op: &str, l: &SqlValue, r: &SqlValue) -> SqlValue {
        binary(op, l, r).unwrap().unwrap()
    }
    fn t(v: SqlValue) -> bool {
        matches!(v, SqlValue::Int(1))
    }

    #[test]
    fn contains_element_and_range() {
        assert!(t(b("@>", &tr("[1,10)"), &SqlValue::Int(3))));
        assert!(!t(b("@>", &tr("[1,10)"), &SqlValue::Int(10))));
        assert!(t(b("@>", &tr("[1,10)"), &SqlValue::Int(1))));
        assert!(t(b("@>", &tr("[1,10)"), &tr("[2,5)"))));
        assert!(!t(b("@>", &tr("[1,10)"), &tr("[2,15)"))));
        assert!(t(b("@>", &tr("[1,10)"), &tr("empty"))));
    }

    #[test]
    fn contained_by() {
        assert!(t(b("<@", &SqlValue::Int(3), &tr("[1,10)"))));
        assert!(t(b("<@", &tr("[2,5)"), &tr("[1,10)"))));
        assert!(!t(b("<@", &tr("[1,10)"), &tr("[2,5)"))));
    }

    #[test]
    fn overlap_and_position() {
        assert!(t(b("&&", &tr("[1,10)"), &tr("[5,15)"))));
        assert!(!t(b("&&", &tr("[1,5)"), &tr("[5,10)"))));
        assert!(t(b("-|-", &tr("[1,5)"), &tr("[5,10)"))));
        assert!(!t(b("-|-", &tr("[1,5)"), &tr("[6,10)"))));
        assert!(t(b("<<", &tr("[1,5)"), &tr("[6,10)"))));
        assert!(t(b(">>", &tr("[6,10)"), &tr("[1,5)"))));
        assert!(!t(b("<<", &tr("[1,7)"), &tr("[5,10)"))));
    }

    #[test]
    fn set_operations() {
        let u = set_op('+', &tr("[1,5)"), &tr("[4,10)")).unwrap().unwrap();
        assert_eq!(u, tr("[1,10)"));
        let i = set_op('*', &tr("[1,10)"), &tr("[5,15)")).unwrap().unwrap();
        assert_eq!(i, tr("[5,10)"));
        let d = set_op('-', &tr("[1,10)"), &tr("[1,5)")).unwrap().unwrap();
        assert_eq!(d, tr("[5,10)"));

        let e = set_op('*', &tr("[1,3)"), &tr("[5,9)")).unwrap().unwrap();
        assert_eq!(e, tr("empty"));
    }

    #[test]
    fn non_contiguous_errors() {
        assert!(set_op('+', &tr("[1,3)"), &tr("[5,9)")).unwrap().is_err());

        assert!(set_op('-', &tr("[1,10)"), &tr("[3,5)")).unwrap().is_err());
    }

    #[test]
    fn merge_ignores_gap() {
        let m = range_merge(&tr("[1,5)"), &tr("[10,15)")).unwrap().unwrap();
        assert_eq!(m, tr("[1,15)"));
    }

    #[test]
    fn null_propagates_and_declines() {
        assert_eq!(
            binary("@>", &SqlValue::Null, &tr("[1,2)")),
            Some(Ok(SqlValue::Null))
        );

        assert!(binary("@>", &SqlValue::Int(1), &SqlValue::Int(2)).is_none());
        assert!(set_op('+', &SqlValue::Int(1), &SqlValue::Int(2)).is_none());
    }
}
