
use crate::types::PgError;
use sql_core::SqlValue;

fn does_not_exist(name: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("function {name}(...) does not exist"),
    }
}

fn as_text(v: &SqlValue) -> Option<&str> {
    match v {
        SqlValue::Text(s) => Some(s.as_str()),
        _ => None,
    }
}

fn as_int(v: &SqlValue) -> Option<i64> {
    match v {
        SqlValue::Int(i) => Some(*i),
        _ => None,
    }
}

fn any_null(args: &[SqlValue]) -> bool {
    args.iter().any(|a| matches!(a, SqlValue::Null))
}

fn pad(s: &str, len: i64, fill: &str, left_side: bool) -> String {
    let chars: Vec<char> = s.chars().collect();
    let target = if len < 0 { 0usize } else { len as usize };

    if target <= chars.len() {

        return chars.into_iter().take(target).collect();
    }

    let fill_chars: Vec<char> = fill.chars().collect();
    if fill_chars.is_empty() {

        return s.to_string();
    }

    let pad_needed = target - chars.len();
    let mut padding: Vec<char> = Vec::with_capacity(pad_needed);
    for i in 0..pad_needed {
        padding.push(fill_chars[i % fill_chars.len()]);
    }

    if left_side {
        padding.into_iter().chain(chars).collect()
    } else {
        chars.into_iter().chain(padding).collect()
    }
}

fn trim(s: &str, set: &str, from_left: bool, from_right: bool) -> String {
    let set_chars: Vec<char> = set.chars().collect();
    let mut chars: Vec<char> = s.chars().collect();

    if from_left {
        let mut start = 0;
        while start < chars.len() && set_chars.contains(&chars[start]) {
            start += 1;
        }
        chars.drain(0..start);
    }
    if from_right {
        while let Some(last) = chars.last() {
            if set_chars.contains(last) {
                chars.pop();
            } else {
                break;
            }
        }
    }
    chars.into_iter().collect()
}

fn left(s: &str, n: i64) -> String {
    let chars: Vec<char> = s.chars().collect();
    let take = if n < 0 {

        (chars.len() as i64 + n).max(0) as usize
    } else {
        (n as usize).min(chars.len())
    };
    chars.into_iter().take(take).collect()
}

fn right(s: &str, n: i64) -> String {
    let chars: Vec<char> = s.chars().collect();
    let skip = if n < 0 {

        (-n).min(chars.len() as i64) as usize
    } else {
        chars.len().saturating_sub(n as usize)
    };
    chars.into_iter().skip(skip).collect()
}

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    match name {
        "lpad" | "rpad" => {
            if !(args.len() == 2 || args.len() == 3) {
                return Some(Err(does_not_exist(name)));
            }
            if any_null(args) {
                return Some(Ok(SqlValue::Null));
            }
            let s = match as_text(&args[0]) {
                Some(s) => s,
                None => return Some(Err(does_not_exist(name))),
            };
            let len = match as_int(&args[1]) {
                Some(i) => i,
                None => return Some(Err(does_not_exist(name))),
            };
            let fill = if args.len() == 3 {
                match as_text(&args[2]) {
                    Some(f) => f,
                    None => return Some(Err(does_not_exist(name))),
                }
            } else {
                " "
            };
            let left_side = name == "lpad";
            Some(Ok(SqlValue::Text(pad(s, len, fill, left_side))))
        }
        "ltrim" | "rtrim" | "btrim" | "trim" => {
            if !(args.len() == 1 || args.len() == 2) {
                return Some(Err(does_not_exist(name)));
            }
            if any_null(args) {
                return Some(Ok(SqlValue::Null));
            }
            let s = match as_text(&args[0]) {
                Some(s) => s,
                None => return Some(Err(does_not_exist(name))),
            };
            let set = if args.len() == 2 {
                match as_text(&args[1]) {
                    Some(c) => c,
                    None => return Some(Err(does_not_exist(name))),
                }
            } else {
                " "
            };
            let (from_left, from_right) = match name {
                "ltrim" => (true, false),
                "rtrim" => (false, true),
                _ => (true, true),
            };
            Some(Ok(SqlValue::Text(trim(s, set, from_left, from_right))))
        }
        "left" | "right" => {
            if args.len() != 2 {
                return Some(Err(does_not_exist(name)));
            }
            if any_null(args) {
                return Some(Ok(SqlValue::Null));
            }
            let s = match as_text(&args[0]) {
                Some(s) => s,
                None => return Some(Err(does_not_exist(name))),
            };
            let n = match as_int(&args[1]) {
                Some(i) => i,
                None => return Some(Err(does_not_exist(name))),
            };
            let out = if name == "left" {
                left(s, n)
            } else {
                right(s, n)
            };
            Some(Ok(SqlValue::Text(out)))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::call;
    use sql_core::SqlValue;

    fn t(s: &str) -> SqlValue {
        SqlValue::Text(s.to_string())
    }
    fn i(n: i64) -> SqlValue {
        SqlValue::Int(n)
    }
    fn text_of(r: Option<Result<SqlValue, crate::types::PgError>>) -> String {
        match r {
            Some(Ok(SqlValue::Text(s))) => s,
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn lpad_pad_and_truncate() {
        assert_eq!(text_of(call("lpad", &[t("hi"), i(5), t("xy")])), "xyxhi");
        assert_eq!(text_of(call("lpad", &[t("hi"), i(5)])), "   hi");
        assert_eq!(text_of(call("lpad", &[t("hi"), i(-5), t("xy")])), "");
        assert_eq!(text_of(call("lpad", &[t("hello"), i(2)])), "he");
        assert_eq!(text_of(call("lpad", &[t("hi"), i(5), t("")])), "hi");
    }

    #[test]
    fn rpad_pad_and_truncate() {
        assert_eq!(text_of(call("rpad", &[t("hi"), i(5), t("xy")])), "hixyx");
        assert_eq!(text_of(call("rpad", &[t("hi"), i(5)])), "hi   ");
        assert_eq!(text_of(call("rpad", &[t("hi"), i(-5), t("xy")])), "");
        assert_eq!(text_of(call("rpad", &[t("hello"), i(2)])), "he");
        assert_eq!(text_of(call("rpad", &[t("hi"), i(5), t("")])), "hi");
    }

    #[test]
    fn trims_default_and_custom() {
        assert_eq!(text_of(call("ltrim", &[t("zzzytrim"), t("xyz")])), "trim");
        assert_eq!(text_of(call("ltrim", &[t("   trim")])), "trim");
        assert_eq!(text_of(call("rtrim", &[t("trimzzz"), t("xyz")])), "trim");
        assert_eq!(text_of(call("rtrim", &[t("trim   ")])), "trim");
        assert_eq!(text_of(call("btrim", &[t("xyzTomxyz"), t("xyz")])), "Tom");
        assert_eq!(text_of(call("btrim", &[t("   Tom   ")])), "Tom");

        assert_eq!(text_of(call("trim", &[t("xxTomxx"), t("x")])), "Tom");
    }

    #[test]
    fn left_right_positive_and_negative() {

        assert_eq!(text_of(call("left", &[t("ahoj"), i(-5)])), "");
        assert_eq!(text_of(call("left", &[t("ahoj"), i(-3)])), "a");
        assert_eq!(text_of(call("left", &[t("ahoj"), i(0)])), "");
        assert_eq!(text_of(call("left", &[t("ahoj"), i(2)])), "ah");
        assert_eq!(text_of(call("left", &[t("ahoj"), i(5)])), "ahoj");

        assert_eq!(text_of(call("right", &[t("ahoj"), i(-5)])), "");
        assert_eq!(text_of(call("right", &[t("ahoj"), i(-3)])), "j");
        assert_eq!(text_of(call("right", &[t("ahoj"), i(0)])), "");
        assert_eq!(text_of(call("right", &[t("ahoj"), i(2)])), "oj");
        assert_eq!(text_of(call("right", &[t("ahoj"), i(5)])), "ahoj");
    }

    #[test]
    fn unicode_by_codepoint() {
        assert_eq!(text_of(call("left", &[t("héllo"), i(2)])), "hé");
        assert_eq!(text_of(call("lpad", &[t("é"), i(3), t("λ")])), "λλé");
    }

    #[test]
    fn null_propagates() {
        assert!(matches!(
            call("lpad", &[SqlValue::Null, i(5)]),
            Some(Ok(SqlValue::Null))
        ));
        assert!(matches!(
            call("btrim", &[t("x"), SqlValue::Null]),
            Some(Ok(SqlValue::Null))
        ));
        assert!(matches!(
            call("left", &[t("x"), SqlValue::Null]),
            Some(Ok(SqlValue::Null))
        ));
    }

    #[test]
    fn wrong_arity_and_type_claimed() {
        assert!(matches!(call("lpad", &[t("x")]), Some(Err(_))));
        assert!(matches!(call("left", &[t("x")]), Some(Err(_))));
        assert!(matches!(call("lpad", &[i(1), i(2)]), Some(Err(_))));
    }

    #[test]
    fn unclaimed_name_returns_none() {
        assert!(call("upper", &[t("x")]).is_none());
        assert!(call("substr", &[t("x"), i(1)]).is_none());
    }
}
