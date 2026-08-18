
use crate::types::PgError;
use sql_core::SqlValue;

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    match name {
        "greatest" => Some(extremum(args, Ordering::Greater)),
        "least" => Some(extremum(args, Ordering::Less)),
        "num_nonnulls" => Some(Ok(SqlValue::Int(
            args.iter().filter(|v| !matches!(v, SqlValue::Null)).count() as i64,
        ))),
        "num_nulls" => Some(Ok(SqlValue::Int(
            args.iter().filter(|v| matches!(v, SqlValue::Null)).count() as i64,
        ))),

        "coalesce" => Some(Ok(args
            .iter()
            .find(|v| !matches!(v, SqlValue::Null))
            .cloned()
            .unwrap_or(SqlValue::Null))),

        "nullif" if args.len() == 2 => Some(Ok(if values_equal(&args[0], &args[1]) {
            SqlValue::Null
        } else {
            args[0].clone()
        })),
        _ => None,
    }
}

fn values_equal(a: &SqlValue, b: &SqlValue) -> bool {
    if matches!(a, SqlValue::Null) || matches!(b, SqlValue::Null) {
        return false;
    }
    if let (Some(x), Some(y)) = (as_f64(a), as_f64(b)) {
        return x == y;
    }
    a == b
}

#[derive(Clone, Copy)]
enum Ordering {
    Greater,
    Less,
}

fn err() -> PgError {
    PgError::InvalidInputSyntax {
        typ: "expression",
        input: "incomparable argument types".to_string(),
    }
}

fn extremum(args: &[SqlValue], dir: Ordering) -> Result<SqlValue, PgError> {
    let mut best: Option<&SqlValue> = None;
    for v in args {
        if matches!(v, SqlValue::Null) {
            continue;
        }
        best = match best {
            None => Some(v),
            Some(cur) => {
                let ord = compare(cur, v)?;
                let take = match dir {
                    Ordering::Greater => ord == core::cmp::Ordering::Less,
                    Ordering::Less => ord == core::cmp::Ordering::Greater,
                };
                Some(if take { v } else { cur })
            }
        };
    }
    Ok(best.cloned().unwrap_or(SqlValue::Null))
}

fn compare(a: &SqlValue, b: &SqlValue) -> Result<core::cmp::Ordering, PgError> {
    if let (Some(x), Some(y)) = (as_f64(a), as_f64(b)) {
        return x.partial_cmp(&y).ok_or_else(err);
    }
    match (a, b) {
        (SqlValue::Text(x), SqlValue::Text(y)) => Ok(x.cmp(y)),
        _ => Err(err()),
    }
}

fn as_f64(v: &SqlValue) -> Option<f64> {
    match v {
        SqlValue::Int(i) => Some(*i as f64),
        SqlValue::Real(r) => Some(*r),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::call;
    use sql_core::SqlValue;

    fn i(n: i64) -> SqlValue {
        SqlValue::Int(n)
    }
    fn t(s: &str) -> SqlValue {
        SqlValue::Text(s.to_string())
    }

    #[test]
    fn greatest_least_over_ints() {
        assert_eq!(
            call("greatest", &[i(3), i(9), i(1)]).unwrap().unwrap(),
            i(9)
        );
        assert_eq!(call("least", &[i(3), i(9), i(1)]).unwrap().unwrap(), i(1));
    }

    #[test]
    fn greatest_preserves_real_variant() {
        assert_eq!(
            call("greatest", &[SqlValue::Real(2.5), i(2)])
                .unwrap()
                .unwrap(),
            SqlValue::Real(2.5)
        );
    }

    #[test]
    fn greatest_least_over_text() {
        assert_eq!(
            call("greatest", &[t("apple"), t("pear"), t("fig")])
                .unwrap()
                .unwrap(),
            t("pear")
        );
        assert_eq!(
            call("least", &[t("apple"), t("pear"), t("fig")])
                .unwrap()
                .unwrap(),
            t("apple")
        );
    }

    #[test]
    fn extremum_ignores_null() {
        assert_eq!(
            call("greatest", &[i(4), SqlValue::Null, i(7)])
                .unwrap()
                .unwrap(),
            i(7)
        );
        assert_eq!(
            call("least", &[SqlValue::Null, i(4), i(7)])
                .unwrap()
                .unwrap(),
            i(4)
        );
    }

    #[test]
    fn all_null_extremum_is_null() {
        assert_eq!(
            call("greatest", &[SqlValue::Null, SqlValue::Null])
                .unwrap()
                .unwrap(),
            SqlValue::Null
        );
        assert_eq!(
            call("least", &[SqlValue::Null]).unwrap().unwrap(),
            SqlValue::Null
        );
    }

    #[test]
    fn incomparable_extremum_errors() {
        assert!(call("greatest", &[i(1), t("x")]).unwrap().is_err());
    }

    #[test]
    fn null_counting() {
        let args = [i(1), SqlValue::Null, t("a"), SqlValue::Null, SqlValue::Null];
        assert_eq!(call("num_nulls", &args).unwrap().unwrap(), i(3));
        assert_eq!(call("num_nonnulls", &args).unwrap().unwrap(), i(2));
    }

    #[test]
    fn counting_all_null_and_none_null() {
        assert_eq!(
            call("num_nulls", &[SqlValue::Null, SqlValue::Null])
                .unwrap()
                .unwrap(),
            i(2)
        );
        assert_eq!(call("num_nonnulls", &[i(0), i(0)]).unwrap().unwrap(), i(2));
    }

    #[test]
    fn unclaimed_name_returns_none() {
        assert!(call("no_such_conditional_fn", &[i(1)]).is_none());
    }

    #[test]
    fn coalesce_and_nullif() {
        assert_eq!(
            call("coalesce", &[SqlValue::Null, i(5), i(6)])
                .unwrap()
                .unwrap(),
            i(5)
        );
        assert_eq!(
            call("coalesce", &[SqlValue::Null, SqlValue::Null])
                .unwrap()
                .unwrap(),
            SqlValue::Null
        );
        assert_eq!(
            call("nullif", &[i(5), i(5)]).unwrap().unwrap(),
            SqlValue::Null
        );
        assert_eq!(call("nullif", &[i(5), i(6)]).unwrap().unwrap(), i(5));

        assert_eq!(
            call("nullif", &[SqlValue::Null, i(6)]).unwrap().unwrap(),
            SqlValue::Null
        );
        assert_eq!(
            call("nullif", &[i(6), SqlValue::Null]).unwrap().unwrap(),
            i(6)
        );
    }
}
