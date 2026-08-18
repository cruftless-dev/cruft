
use crate::types::PgError;
use sql_core::SqlValue;

fn owns(name: &str) -> bool {
    matches!(name, "levenshtein" | "levenshtein_less_equal")
}

fn as_text(v: &SqlValue) -> Option<String> {
    match v {
        SqlValue::Text(s) => Some(s.clone()),
        SqlValue::Int(n) => Some(n.to_string()),
        SqlValue::Real(f) => Some(f.to_string()),
        _ => None,
    }
}

fn distance(a: &[char], b: &[char]) -> i64 {
    if a.is_empty() {
        return b.len() as i64;
    }
    if b.is_empty() {
        return a.len() as i64;
    }

    let mut prev: Vec<i64> = (0..=b.len() as i64).collect();
    let mut cur = vec![0i64; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        cur[0] = i as i64 + 1;
        for (j, &cb) in b.iter().enumerate() {
            let sub_cost = if ca == cb { 0 } else { 1 };
            cur[j + 1] = (prev[j] + sub_cost)
                .min(prev[j + 1] + 1)
                .min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

fn arity_err(name: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("function {name}(...) does not exist"),
    }
}

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    if !owns(name) {
        return None;
    }

    if args.iter().any(|a| matches!(a, SqlValue::Null)) {
        return Some(Ok(SqlValue::Null));
    }
    Some(match name {
        "levenshtein" => {
            if args.len() != 2 {
                return Some(Err(arity_err("levenshtein")));
            }
            let a = match as_text(&args[0]) {
                Some(s) => s,
                None => return Some(Err(arity_err("levenshtein"))),
            };
            let b = match as_text(&args[1]) {
                Some(s) => s,
                None => return Some(Err(arity_err("levenshtein"))),
            };
            let ac: Vec<char> = a.chars().collect();
            let bc: Vec<char> = b.chars().collect();
            Ok(SqlValue::Int(distance(&ac, &bc)))
        }
        "levenshtein_less_equal" => {
            if args.len() != 3 {
                return Some(Err(arity_err("levenshtein_less_equal")));
            }
            let a = match as_text(&args[0]) {
                Some(s) => s,
                None => return Some(Err(arity_err("levenshtein_less_equal"))),
            };
            let b = match as_text(&args[1]) {
                Some(s) => s,
                None => return Some(Err(arity_err("levenshtein_less_equal"))),
            };
            let max = match &args[2] {
                SqlValue::Int(n) => *n,
                _ => return Some(Err(arity_err("levenshtein_less_equal"))),
            };
            if max < 0 {
                return Some(Err(PgError::InvalidInputSyntax {
                    typ: "levenshtein_less_equal",
                    input: "max_d must be a non-negative integer".to_string(),
                }));
            }
            let ac: Vec<char> = a.chars().collect();
            let bc: Vec<char> = b.chars().collect();
            let d = distance(&ac, &bc);

            Ok(SqlValue::Int(d.min(max + 1)))
        }
        _ => unreachable!(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lev(a: &str, b: &str) -> i64 {
        match super::call(
            "levenshtein",
            &[SqlValue::Text(a.into()), SqlValue::Text(b.into())],
        ) {
            Some(Ok(SqlValue::Int(n))) => n,
            other => panic!("unexpected {other:?}"),
        }
    }
    fn lle(a: &str, b: &str, m: i64) -> i64 {
        match super::call(
            "levenshtein_less_equal",
            &[
                SqlValue::Text(a.into()),
                SqlValue::Text(b.into()),
                SqlValue::Int(m),
            ],
        ) {
            Some(Ok(SqlValue::Int(n))) => n,
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn classic_distances() {
        assert_eq!(lev("kitten", "sitting"), 3);
        assert_eq!(lev("sunday", "saturday"), 3);
        assert_eq!(lev("flaw", "lawn"), 2);
        assert_eq!(lev("gumbo", "gambol"), 2);
        assert_eq!(lev("book", "back"), 2);
    }

    #[test]
    fn edge_cases() {
        assert_eq!(lev("", "abc"), 3);
        assert_eq!(lev("abc", ""), 3);
        assert_eq!(lev("abc", "abc"), 0);
        assert_eq!(lev("", ""), 0);
        assert_eq!(lev("a", ""), 1);
    }

    #[test]
    fn less_equal_bound() {
        assert_eq!(lle("kitten", "sitting", 5), 3);
        assert_eq!(lle("abcdef", "xxxxxx", 2), 3);
        assert_eq!(lle("kitten", "kitten", 3), 0);
        assert_eq!(lle("a", "abc", 5), 2);
    }

    #[test]
    fn null_is_null() {
        assert_eq!(
            super::call("levenshtein", &[SqlValue::Null, SqlValue::Text("x".into())]),
            Some(Ok(SqlValue::Null))
        );
    }
}
