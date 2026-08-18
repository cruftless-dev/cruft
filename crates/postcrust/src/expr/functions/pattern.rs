
use crate::types::PgError;
use sql_core::SqlValue;

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    let ci = match name {
        "like" => false,
        "ilike" => true,
        _ => return None,
    };
    if args.len() != 2 {
        return Some(Err(PgError::InvalidInputSyntax {
            typ: "expression",
            input: format!("function {name}(...) does not exist"),
        }));
    }

    if args.iter().any(|a| matches!(a, SqlValue::Null)) {
        return Some(Ok(SqlValue::Null));
    }
    let (text, pat) = match (&args[0], &args[1]) {
        (SqlValue::Text(t), SqlValue::Text(p)) => (t, p),
        _ => {
            return Some(Err(PgError::InvalidInputSyntax {
                typ: "expression",
                input: format!("function {name}(...) does not exist"),
            }))
        }
    };
    let t: Vec<char> = text.chars().collect();
    let p: Vec<char> = pat.chars().collect();
    Some(Ok(SqlValue::Int(if like_match(&t, &p, ci) {
        1
    } else {
        0
    })))
}

fn like_match(text: &[char], pat: &[char], ci: bool) -> bool {
    let (mut ti, mut pi) = (0usize, 0usize);

    let (mut star_pi, mut star_ti): (Option<usize>, usize) = (None, 0);
    let eq = |a: char, b: char| {
        if ci {
            a.eq_ignore_ascii_case(&b)
        } else {
            a == b
        }
    };

    while ti < text.len() {
        if pi < pat.len() {
            match pat[pi] {
                '%' => {
                    star_pi = Some(pi);
                    star_ti = ti;
                    pi += 1;
                    continue;
                }
                '_' => {
                    pi += 1;
                    ti += 1;
                    continue;
                }
                '\\' if pi + 1 < pat.len() => {
                    if eq(pat[pi + 1], text[ti]) {
                        pi += 2;
                        ti += 1;
                        continue;
                    }
                }
                c => {
                    if eq(c, text[ti]) {
                        pi += 1;
                        ti += 1;
                        continue;
                    }
                }
            }
        }

        if let Some(sp) = star_pi {
            pi = sp + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }

    while pi < pat.len() && pat[pi] == '%' {
        pi += 1;
    }
    pi == pat.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn like(t: &str, p: &str) -> bool {
        matches!(
            call(
                "like",
                &[SqlValue::Text(t.into()), SqlValue::Text(p.into())]
            ),
            Some(Ok(SqlValue::Int(1)))
        )
    }
    fn ilike(t: &str, p: &str) -> bool {
        matches!(
            call(
                "ilike",
                &[SqlValue::Text(t.into()), SqlValue::Text(p.into())]
            ),
            Some(Ok(SqlValue::Int(1)))
        )
    }

    #[test]
    fn wildcards() {
        assert!(like("abc", "abc"));
        assert!(like("abc", "a%"));
        assert!(like("abc", "%c"));
        assert!(like("abc", "a_c"));
        assert!(like("abc", "%"));
        assert!(like("", "%"));
        assert!(!like("abc", "a_"));
        assert!(!like("abc", "abcd"));
        assert!(like("a%b", "a\\%b"));
        assert!(!like("axb", "a\\%b"));
    }

    #[test]
    fn case_insensitive() {
        assert!(ilike("HELLO", "h%o"));
        assert!(!like("HELLO", "h%o"));
    }

    #[test]
    fn null_and_unclaimed() {
        assert_eq!(
            call("like", &[SqlValue::Null, SqlValue::Text("x".into())]),
            Some(Ok(SqlValue::Null))
        );
        assert!(call("nope", &[]).is_none());
    }
}
