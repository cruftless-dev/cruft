
use super::PgError;
use sql_core::SqlValue;

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    let _ = oid;
    Ok(SqlValue::Text(text.to_string()))
}

pub fn output(oid: u32, v: &SqlValue) -> String {
    let _ = oid;
    match v {
        SqlValue::Text(s) => s.clone(),
        _ => String::new(),
    }
}

pub fn apply_typmod(oid: u32, typmod: i32, v: SqlValue) -> Result<SqlValue, PgError> {
    let n = (typmod - 4) as i64;
    if n < 0 {
        return Ok(v);
    }
    let n = n as usize;
    let s = match v {
        SqlValue::Text(s) => s,
        other => return Ok(other),
    };
    let chars: Vec<char> = s.chars().collect();
    let is_bpchar = oid == super::oid::BPCHAR;
    if chars.len() > n {

        if chars[n..].iter().all(|&c| c == ' ') {
            let fitted: String = chars[..n].iter().collect();
            return Ok(SqlValue::Text(fitted));
        }
        let disp = if is_bpchar {
            format!("character({n})")
        } else {
            format!("character varying({n})")
        };
        return Err(PgError::ValueTooLong { typ: disp });
    }
    if is_bpchar && chars.len() < n {

        let mut padded = s;
        for _ in 0..(n - chars.len()) {
            padded.push(' ');
        }
        return Ok(SqlValue::Text(padded));
    }
    Ok(SqlValue::Text(s))
}

pub fn rtrim_blanks(v: &SqlValue) -> SqlValue {
    match v {
        SqlValue::Text(s) => SqlValue::Text(s.trim_end_matches(' ').to_string()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::oid;

    fn round_trip(oid: u32, s: &str) {
        let v = input(oid, s).expect("text-family input never errors at this rung");
        assert_eq!(v, SqlValue::Text(s.to_string()));
        assert_eq!(output(oid, &v), s);
    }

    #[test]
    fn text_identity() {

        round_trip(oid::TEXT, "this is a text string");
        round_trip(oid::TEXT, "doh!");
        round_trip(oid::TEXT, "hi de ho neighbor");
    }

    #[test]
    fn varchar_no_typmod_is_text() {
        round_trip(oid::VARCHAR, "a");
        round_trip(oid::VARCHAR, "cd");
        round_trip(oid::VARCHAR, "a longer character varying value");
    }

    #[test]
    fn bpchar_no_typmod_is_text() {

        round_trip(oid::BPCHAR, "c");
        round_trip(oid::BPCHAR, "cd");
        round_trip(oid::BPCHAR, "c     ");
    }

    #[test]
    fn empty_string() {
        round_trip(oid::TEXT, "");
        round_trip(oid::VARCHAR, "");
        round_trip(oid::BPCHAR, "");
    }

    #[test]
    fn unicode_preserved() {
        round_trip(oid::TEXT, "héllo wörld");
        round_trip(oid::TEXT, "日本語のテキスト");
        round_trip(oid::TEXT, "emoji: 🦀🐘");
    }

    #[test]
    fn whitespace_preserved() {
        round_trip(oid::TEXT, "  leading and trailing  ");
        round_trip(oid::TEXT, "tab\tnewline\nreturn\r");
        round_trip(oid::TEXT, "   ");
    }

    #[test]
    fn output_non_text_is_empty() {

        assert_eq!(output(oid::TEXT, &SqlValue::Null), "");
        assert_eq!(output(oid::VARCHAR, &SqlValue::Int(42)), "");
    }

    #[test]
    fn typmod_cases_are_deferred() {

        assert_eq!(
            input(oid::VARCHAR, "cd").unwrap(),
            SqlValue::Text("cd".into())
        );
        assert_eq!(
            input(oid::BPCHAR, "cd").unwrap(),
            SqlValue::Text("cd".into())
        );

        assert_eq!(output(oid::BPCHAR, &input(oid::BPCHAR, "").unwrap()), "");
    }

    fn len_tm(n: i32) -> i32 {
        crate::types::typmod::make_len(n)
    }
    fn ap(oid: u32, n: i32, v: &str) -> Result<SqlValue, PgError> {
        apply_typmod(oid, len_tm(n), SqlValue::Text(v.into()))
    }

    #[test]
    fn varchar_length_enforced() {
        assert_eq!(
            ap(oid::VARCHAR, 10, "hello").unwrap(),
            SqlValue::Text("hello".into())
        );
        assert_eq!(
            ap(oid::VARCHAR, 5, "hello").unwrap(),
            SqlValue::Text("hello".into())
        );

        assert_eq!(
            ap(oid::VARCHAR, 1, "c     ").unwrap(),
            SqlValue::Text("c".into())
        );

        assert_eq!(
            ap(oid::VARCHAR, 10, "ab").unwrap(),
            SqlValue::Text("ab".into())
        );
    }

    #[test]
    fn varchar_too_long_errors() {
        assert_eq!(
            ap(oid::VARCHAR, 1, "cd"),
            Err(PgError::ValueTooLong {
                typ: "character varying(1)".into()
            })
        );
    }

    #[test]
    fn bpchar_pads_and_trims() {

        assert_eq!(
            ap(oid::BPCHAR, 5, "ab").unwrap(),
            SqlValue::Text("ab   ".into())
        );
        assert_eq!(
            ap(oid::BPCHAR, 3, "abc").unwrap(),
            SqlValue::Text("abc".into())
        );

        assert_eq!(
            ap(oid::BPCHAR, 1, "c   ").unwrap(),
            SqlValue::Text("c".into())
        );

        assert_eq!(
            ap(oid::BPCHAR, 3, "").unwrap(),
            SqlValue::Text("   ".into())
        );
    }

    #[test]
    fn bpchar_too_long_errors() {
        assert_eq!(
            ap(oid::BPCHAR, 1, "cd"),
            Err(PgError::ValueTooLong {
                typ: "character(1)".into()
            })
        );
    }

    #[test]
    fn rtrim_blanks_trims_only_trailing_spaces() {
        let t = |s: &str| SqlValue::Text(s.into());

        assert_eq!(rtrim_blanks(&t("ab   ")), t("ab"));
        assert_eq!(rtrim_blanks(&t("ab")), t("ab"));
        assert_eq!(rtrim_blanks(&t("  a b  ")), t("  a b"));
        assert_eq!(rtrim_blanks(&t("     ")), t(""));

        assert_eq!(rtrim_blanks(&t("ab\t")), t("ab\t"));

        assert_eq!(rtrim_blanks(&SqlValue::Null), SqlValue::Null);
        assert_eq!(rtrim_blanks(&SqlValue::Int(5)), SqlValue::Int(5));
    }

    #[test]
    fn typmod_counts_characters_not_bytes() {

        assert_eq!(
            ap(oid::VARCHAR, 3, "héy").unwrap(),
            SqlValue::Text("héy".into())
        );
        assert_eq!(
            ap(oid::BPCHAR, 4, "héy").unwrap(),
            SqlValue::Text("héy ".into())
        );
    }
}
