
use crate::types::PgError;
use sql_core::SqlValue;

fn does_not_exist(name: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("function {name}(...) does not exist"),
    }
}

fn has_null(args: &[SqlValue]) -> bool {
    args.iter().any(|a| matches!(a, SqlValue::Null))
}

fn as_text<'a>(v: &'a SqlValue) -> Option<&'a str> {
    match v {
        SqlValue::Text(s) => Some(s.as_str()),
        _ => None,
    }
}

fn as_int(v: &SqlValue) -> Option<i64> {
    match v {
        SqlValue::Int(n) => Some(*n),
        _ => None,
    }
}

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    match name {
        "strpos" => Some(strpos(name, args)),
        "substr" => Some(substr(name, args)),
        "replace" => Some(replace(name, args)),
        "split_part" => Some(split_part(name, args)),
        "translate" => Some(translate(name, args)),
        "starts_with" => Some(starts_with(name, args)),
        "overlay" => Some(overlay(name, args)),
        _ => None,
    }
}

fn overlay(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() != 3 && args.len() != 4 {
        return Err(does_not_exist(name));
    }
    if has_null(args) {
        return Ok(SqlValue::Null);
    }
    let Some(start) = as_int(&args[2]) else {
        return Err(does_not_exist(name));
    };
    let count = if args.len() == 4 {
        match as_int(&args[3]) {
            Some(c) => Some(c),
            None => return Err(does_not_exist(name)),
        }
    } else {
        None
    };
    let neg = PgError::InvalidInputSyntax {
        typ: "expression",
        input: "negative substring length not allowed".to_string(),
    };

    if let SqlValue::Blob(s) = &args[0] {
        let SqlValue::Blob(sub) = &args[1] else {
            return Err(does_not_exist(name));
        };
        let count = count.unwrap_or(sub.len() as i64);
        let prefix_len = start - 1;
        if prefix_len < 0 || count < 0 {
            return Err(neg);
        }
        let len = s.len() as i64;
        let pre = (prefix_len.min(len)) as usize;
        let suf = ((prefix_len + count).min(len)).max(0) as usize;
        let mut out = Vec::new();
        out.extend_from_slice(&s[..pre]);
        out.extend_from_slice(sub);
        out.extend_from_slice(&s[suf..]);
        return Ok(SqlValue::Blob(out));
    }
    let (Some(s), Some(sub)) = (as_text(&args[0]), as_text(&args[1])) else {
        return Err(does_not_exist(name));
    };
    let chars: Vec<char> = s.chars().collect();
    let sub_chars: Vec<char> = sub.chars().collect();
    let count = count.unwrap_or(sub_chars.len() as i64);
    let prefix_len = start - 1;
    if prefix_len < 0 || count < 0 {
        return Err(neg);
    }
    let len = chars.len() as i64;
    let pre = (prefix_len.min(len)) as usize;
    let suf = ((prefix_len + count).min(len)).max(0) as usize;
    let mut out: String = chars[..pre].iter().collect();
    out.extend(sub_chars.iter());
    out.extend(chars[suf..].iter());
    Ok(SqlValue::Text(out))
}

fn strpos(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() != 2 {
        return Err(does_not_exist(name));
    }
    if has_null(args) {
        return Ok(SqlValue::Null);
    }
    let (Some(hay), Some(needle)) = (as_text(&args[0]), as_text(&args[1])) else {
        return Err(does_not_exist(name));
    };

    let hay_chars: Vec<char> = hay.chars().collect();
    let needle_chars: Vec<char> = needle.chars().collect();
    if needle_chars.is_empty() {
        return Ok(SqlValue::Int(1));
    }
    if needle_chars.len() > hay_chars.len() {
        return Ok(SqlValue::Int(0));
    }
    for start in 0..=(hay_chars.len() - needle_chars.len()) {
        if hay_chars[start..start + needle_chars.len()] == needle_chars[..] {
            return Ok(SqlValue::Int((start as i64) + 1));
        }
    }
    Ok(SqlValue::Int(0))
}

fn substr(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() != 2 && args.len() != 3 {
        return Err(does_not_exist(name));
    }
    if has_null(args) {
        return Ok(SqlValue::Null);
    }
    let Some(s) = as_text(&args[0]) else {
        return Err(does_not_exist(name));
    };
    let Some(from) = as_int(&args[1]) else {
        return Err(does_not_exist(name));
    };
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len() as i64;

    let (lo, hi): (i64, i64) = if args.len() == 3 {
        let Some(count) = as_int(&args[2]) else {
            return Err(does_not_exist(name));
        };
        if count < 0 {
            return Err(PgError::InvalidInputSyntax {
                typ: "expression",
                input: "negative substring length not allowed".to_string(),
            });
        }

        (from, from.saturating_add(count))
    } else {
        (from, len + 1)
    };

    let start = lo.max(1);
    let end = hi.min(len + 1);
    if start >= end {
        return Ok(SqlValue::Text(String::new()));
    }
    let out: String = chars[(start - 1) as usize..(end - 1) as usize]
        .iter()
        .collect();
    Ok(SqlValue::Text(out))
}

fn replace(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() != 3 {
        return Err(does_not_exist(name));
    }
    if has_null(args) {
        return Ok(SqlValue::Null);
    }
    let (Some(s), Some(from), Some(to)) = (as_text(&args[0]), as_text(&args[1]), as_text(&args[2]))
    else {
        return Err(does_not_exist(name));
    };

    if from.is_empty() {
        return Ok(SqlValue::Text(s.to_string()));
    }
    Ok(SqlValue::Text(s.replace(from, to)))
}

fn split_part(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() != 3 {
        return Err(does_not_exist(name));
    }
    if has_null(args) {
        return Ok(SqlValue::Null);
    }
    let (Some(s), Some(delim)) = (as_text(&args[0]), as_text(&args[1])) else {
        return Err(does_not_exist(name));
    };
    let Some(n) = as_int(&args[2]) else {
        return Err(does_not_exist(name));
    };
    if n == 0 {
        return Err(PgError::InvalidInputSyntax {
            typ: "expression",
            input: "field position must not be zero".to_string(),
        });
    }

    if delim.is_empty() {
        if n == 1 || n == -1 {
            return Ok(SqlValue::Text(s.to_string()));
        }
        return Ok(SqlValue::Text(String::new()));
    }
    let fields: Vec<&str> = s.split(delim).collect();
    let count = fields.len() as i64;
    let idx = if n > 0 { n - 1 } else { count + n };
    if idx < 0 || idx >= count {
        return Ok(SqlValue::Text(String::new()));
    }
    Ok(SqlValue::Text(fields[idx as usize].to_string()))
}

fn translate(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() != 3 {
        return Err(does_not_exist(name));
    }
    if has_null(args) {
        return Ok(SqlValue::Null);
    }
    let (Some(s), Some(from), Some(to)) = (as_text(&args[0]), as_text(&args[1]), as_text(&args[2]))
    else {
        return Err(does_not_exist(name));
    };
    let from_chars: Vec<char> = from.chars().collect();
    let to_chars: Vec<char> = to.chars().collect();
    let mut out = String::new();
    for c in s.chars() {
        match from_chars.iter().position(|&fc| fc == c) {
            Some(i) if i < to_chars.len() => out.push(to_chars[i]),
            Some(_) => {}
            None => out.push(c),
        }
    }
    Ok(SqlValue::Text(out))
}

fn starts_with(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() != 2 {
        return Err(does_not_exist(name));
    }
    if has_null(args) {
        return Ok(SqlValue::Null);
    }
    let (Some(s), Some(prefix)) = (as_text(&args[0]), as_text(&args[1])) else {
        return Err(does_not_exist(name));
    };
    Ok(SqlValue::Int(if s.starts_with(prefix) { 1 } else { 0 }))
}

#[cfg(test)]
mod tests {
    use super::call;
    use sql_core::SqlValue;

    fn txt(s: &str) -> SqlValue {
        SqlValue::Text(s.to_string())
    }
    fn ok_text(name: &str, args: &[SqlValue]) -> String {
        match call(name, args).unwrap().unwrap() {
            SqlValue::Text(s) => s,
            other => panic!("expected Text, got {other:?}"),
        }
    }
    fn ok_int(name: &str, args: &[SqlValue]) -> i64 {
        match call(name, args).unwrap().unwrap() {
            SqlValue::Int(n) => n,
            other => panic!("expected Int, got {other:?}"),
        }
    }

    #[test]
    fn strpos_found_absent_empty() {
        assert_eq!(ok_int("strpos", &[txt("abcdef"), txt("cd")]), 3);
        assert_eq!(ok_int("strpos", &[txt("abcdef"), txt("xy")]), 0);
        assert_eq!(ok_int("strpos", &[txt("abcdef"), txt("")]), 1);
        assert_eq!(ok_int("strpos", &[txt(""), txt("xy")]), 0);
        assert_eq!(ok_int("strpos", &[txt(""), txt("")]), 1);
    }

    #[test]
    fn substr_two_arg() {
        assert_eq!(
            ok_text("substr", &[txt("1234567890"), SqlValue::Int(5)]),
            "567890"
        );

        assert_eq!(ok_text("substr", &[txt("abc"), SqlValue::Int(-1)]), "abc");
    }

    #[test]
    fn substr_three_arg_and_from_before_start() {

        assert_eq!(
            ok_text(
                "substr",
                &[txt("1234567890"), SqlValue::Int(-1), SqlValue::Int(5)]
            ),
            "123"
        );

        assert_eq!(
            ok_text(
                "substr",
                &[txt("1234567890"), SqlValue::Int(8), SqlValue::Int(10)]
            ),
            "890"
        );
        assert_eq!(
            ok_text(
                "substr",
                &[txt("hello"), SqlValue::Int(2), SqlValue::Int(3)]
            ),
            "ell"
        );
    }

    #[test]
    fn substr_negative_count_errors() {
        let r = call(
            "substr",
            &[txt("hello"), SqlValue::Int(5), SqlValue::Int(-1)],
        )
        .unwrap();
        assert!(r.is_err(), "negative count must error");
    }

    #[test]
    fn replace_basic() {
        assert_eq!(
            ok_text("replace", &[txt("abcabc"), txt("bc"), txt("X")]),
            "aXaX"
        );

        assert_eq!(ok_text("replace", &[txt("abc"), txt(""), txt("X")]), "abc");
    }

    #[test]
    fn split_part_in_out_and_negative() {
        assert_eq!(
            ok_text(
                "split_part",
                &[txt("joeuser@mydatabase"), txt("@"), SqlValue::Int(1)]
            ),
            "joeuser"
        );
        assert_eq!(
            ok_text(
                "split_part",
                &[txt("joeuser@mydatabase"), txt("@"), SqlValue::Int(2)]
            ),
            "mydatabase"
        );

        assert_eq!(
            ok_text(
                "split_part",
                &[txt("joeuser@mydatabase"), txt("@"), SqlValue::Int(3)]
            ),
            ""
        );

        assert_eq!(
            ok_text(
                "split_part",
                &[txt("joeuser@mydatabase"), txt("@"), SqlValue::Int(-1)]
            ),
            "mydatabase"
        );
        assert_eq!(
            ok_text(
                "split_part",
                &[txt("joeuser@mydatabase"), txt("@"), SqlValue::Int(-2)]
            ),
            "joeuser"
        );
        assert_eq!(
            ok_text(
                "split_part",
                &[txt("joeuser@mydatabase"), txt("@"), SqlValue::Int(-3)]
            ),
            ""
        );

        assert_eq!(
            ok_text(
                "split_part",
                &[txt("joeuser@mydatabase"), txt(""), SqlValue::Int(1)]
            ),
            "joeuser@mydatabase"
        );
        assert_eq!(
            ok_text(
                "split_part",
                &[txt("joeuser@mydatabase"), txt(""), SqlValue::Int(2)]
            ),
            ""
        );

        assert!(
            call("split_part", &[txt("a@b"), txt("@"), SqlValue::Int(0)])
                .unwrap()
                .is_err()
        );
    }

    #[test]
    fn translate_with_drop() {
        assert_eq!(
            ok_text("translate", &[txt("12345"), txt("14"), txt("ax")]),
            "a23x5"
        );

        assert_eq!(
            ok_text("translate", &[txt("12345"), txt("134"), txt("a")]),
            "a25"
        );
        assert_eq!(ok_text("translate", &[txt(""), txt("14"), txt("ax")]), "");
    }

    #[test]
    fn starts_with_true_false() {
        assert_eq!(ok_int("starts_with", &[txt("alphabet"), txt("alph")]), 1);
        assert_eq!(ok_int("starts_with", &[txt("alphabet"), txt("bet")]), 0);
    }

    #[test]
    fn null_propagation() {
        for name in [
            "strpos",
            "replace",
            "split_part",
            "translate",
            "starts_with",
        ] {
            let args = [SqlValue::Null, txt("x"), txt("y")];
            let n = if name == "strpos" || name == "starts_with" {
                2
            } else {
                3
            };
            let r = call(name, &args[..n]).unwrap().unwrap();
            assert!(matches!(r, SqlValue::Null), "{name} should return Null");
        }
        let r = call("substr", &[SqlValue::Null, SqlValue::Int(1)])
            .unwrap()
            .unwrap();
        assert!(matches!(r, SqlValue::Null));
    }

    #[test]
    fn wrong_arity_claimed_errors() {
        assert!(call("strpos", &[txt("a")]).unwrap().is_err());
        assert!(call("starts_with", &[txt("a"), txt("b"), txt("c")])
            .unwrap()
            .is_err());
    }

    #[test]
    fn unclaimed_name_is_none() {
        assert!(call("upper", &[txt("a")]).is_none());
        assert!(call("regexp_substr", &[txt("a"), txt("b")]).is_none());
    }
}
