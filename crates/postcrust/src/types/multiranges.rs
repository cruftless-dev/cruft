
use super::{oid, ranges, type_name, PgError};
use sql_core::SqlValue;

pub(crate) fn component_range_oid(mr_oid: u32) -> Option<u32> {
    Some(match mr_oid {
        oid::INT4MULTIRANGE => oid::INT4RANGE,
        oid::INT8MULTIRANGE => oid::INT8RANGE,
        oid::NUMMULTIRANGE => oid::NUMRANGE,
        oid::TSMULTIRANGE => oid::TSRANGE,
        oid::DATEMULTIRANGE => oid::DATERANGE,
        _ => return None,
    })
}

pub(crate) fn multirange_of_range(range_oid: u32) -> Option<u32> {
    Some(match range_oid {
        oid::INT4RANGE => oid::INT4MULTIRANGE,
        oid::INT8RANGE => oid::INT8MULTIRANGE,
        oid::NUMRANGE => oid::NUMMULTIRANGE,
        oid::TSRANGE => oid::TSMULTIRANGE,
        oid::DATERANGE => oid::DATEMULTIRANGE,
        _ => return None,
    })
}

fn is_pg_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r')
}

fn split_components(inner: &[char]) -> Option<Vec<String>> {
    let mut comps: Vec<String> = Vec::new();
    let mut i = 0;
    while i < inner.len() {
        let c = inner[i];
        if is_pg_space(c) || c == ',' {
            i += 1;
            continue;
        }

        if c == '[' || c == '(' {
            let start = i;
            let mut in_quote = false;
            i += 1;
            loop {
                if i >= inner.len() {
                    return None;
                }
                let d = inner[i];
                if in_quote {
                    if d == '\\' {
                        i += 2;
                        continue;
                    }
                    if d == '"' {

                        if i + 1 < inner.len() && inner[i + 1] == '"' {
                            i += 2;
                            continue;
                        }
                        in_quote = false;
                    }
                    i += 1;
                } else if d == '"' {
                    in_quote = true;
                    i += 1;
                } else if d == '\\' {
                    i += 2;
                } else if d == ']' || d == ')' {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
            comps.push(inner[start..i].iter().collect());
        } else {

            let start = i;
            while i < inner.len() && inner[i] != ',' && !is_pg_space(inner[i]) {
                i += 1;
            }
            let word: String = inner[start..i].iter().collect();
            if word.eq_ignore_ascii_case("empty") {
                comps.push(word);
            } else {
                return None;
            }
        }
    }
    Some(comps)
}

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    let range_oid = match component_range_oid(oid) {
        Some(r) => r,
        None => {
            return Err(PgError::InvalidInputSyntax {
                typ: type_name(oid),
                input: text.to_string(),
            })
        }
    };
    let malformed = || PgError::InvalidInputSyntax {
        typ: type_name(oid),
        input: text.to_string(),
    };

    let trimmed = text.trim_matches(is_pg_space);
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.first() != Some(&'{') || chars.last() != Some(&'}') || chars.len() < 2 {
        return Err(malformed());
    }
    let inner = &chars[1..chars.len() - 1];
    let comps = split_components(inner).ok_or_else(malformed)?;

    let mut parts = Vec::with_capacity(comps.len());
    for comp in &comps {

        let v = ranges::input(range_oid, comp)?;
        let canon = ranges::output(range_oid, &v);
        if canon.eq_ignore_ascii_case("empty") {
            continue;
        }
        if let Some(p) = ranges::parse_parts(&canon) {
            parts.push(p);
        }
    }
    Ok(SqlValue::Text(render(ranges::normalize_multirange(parts))))
}

pub(crate) fn render(parts: Vec<ranges::RangeParts>) -> String {
    let bodies: Vec<String> = parts.iter().map(ranges::render_parts).collect();
    format!("{{{}}}", bodies.join(","))
}

pub fn output(_oid: u32, v: &SqlValue) -> String {
    match v {
        SqlValue::Text(s) => s.clone(),
        _ => String::new(),
    }
}

pub fn looks_like_multirange(s: &str) -> bool {
    s.trim_matches(is_pg_space).starts_with('{')
}

pub fn parse_components(s: &str) -> Option<Vec<ranges::RangeParts>> {
    let trimmed = s.trim_matches(is_pg_space);
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.first() != Some(&'{') || chars.last() != Some(&'}') || chars.len() < 2 {
        return None;
    }
    let inner = &chars[1..chars.len() - 1];
    let comps = split_components(inner)?;
    let mut parts = Vec::with_capacity(comps.len());
    for comp in &comps {
        if comp.eq_ignore_ascii_case("empty") {
            continue;
        }
        parts.push(ranges::parse_parts(comp)?);
    }
    Some(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    const INT4MR: u32 = oid::INT4MULTIRANGE;
    const NUMMR: u32 = oid::NUMMULTIRANGE;

    fn s(oid: u32, text: &str) -> String {
        match input(oid, text).unwrap() {
            SqlValue::Text(s) => s,
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn empty_and_singleton() {
        assert_eq!(s(INT4MR, "{}"), "{}");
        assert_eq!(s(INT4MR, "  { }  "), "{}");
        assert_eq!(s(INT4MR, "{[1,5)}"), "{[1,5)}");

        assert_eq!(s(INT4MR, "{[1,5]}"), "{[1,6)}");
    }

    #[test]
    fn sorts_components() {
        assert_eq!(s(INT4MR, "{[8,10),[1,5)}"), "{[1,5),[8,10)}");
    }

    #[test]
    fn merges_overlapping_and_adjacent() {

        assert_eq!(s(INT4MR, "{[1,3),[2,5)}"), "{[1,5)}");

        assert_eq!(s(INT4MR, "{[1,3),[3,5)}"), "{[1,5)}");

        assert_eq!(s(NUMMR, "{[1,3),[3,5)}"), "{[1,5)}");

        assert_eq!(s(INT4MR, "{[1,3),[5,8)}"), "{[1,3),[5,8)}");
    }

    #[test]
    fn drops_empty_components() {
        assert_eq!(s(INT4MR, "{[1,5), empty}"), "{[1,5)}");

        assert_eq!(s(INT4MR, "{(1,2)}"), "{}");
    }

    #[test]
    fn bad_component_surfaces_range_error() {
        assert_eq!(
            input(INT4MR, "{[1,zed)}").unwrap_err(),
            PgError::InvalidInputSyntax {
                typ: "integer",
                input: "zed".into()
            }
        );
    }

    #[test]
    fn malformed_rejected() {
        for bad in ["", "[1,5)", "{[1,5)", "{[1,5)}}", "{junk}", "{[1,5) extra}"] {
            assert!(input(INT4MR, bad).is_err(), "expected reject for {bad:?}");
        }
    }

    #[test]
    fn round_trips() {
        for (oid, text) in [
            (INT4MR, "{[1,5),[8,10)}"),
            (NUMMR, "{[1.1,2.2]}"),
            (INT4MR, "{}"),
        ] {
            let canon = s(oid, text);
            assert_eq!(s(oid, &canon), canon);
        }
    }
}
