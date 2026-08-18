
use super::{type_name, PgError};
use sql_core::SqlValue;

fn is_pg_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r')
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn parse_field(s: &str) -> Option<i64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    let invalid = || PgError::InvalidInputSyntax {
        typ: type_name(oid),
        input: text.to_string(),
    };
    let trimmed = text.trim_matches(is_pg_space);

    let mut parts = trimmed.split('-');
    let (Some(y), Some(m), Some(d), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(invalid());
    };

    let (Some(year), Some(month), Some(day)) = (parse_field(y), parse_field(m), parse_field(d))
    else {
        return Err(invalid());
    };

    if !(1..=9999).contains(&year) {
        return Err(invalid());
    }
    if !(1..=12).contains(&month) {
        return Err(invalid());
    }
    let month = month as u32;
    if day < 1 || day > days_in_month(year, month) as i64 {
        return Err(invalid());
    }
    let day = day as u32;

    Ok(SqlValue::Text(format!("{year:04}-{month:02}-{day:02}")))
}

pub fn output(_oid: u32, v: &SqlValue) -> String {
    match v {
        SqlValue::Text(s) => s.clone(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATE: u32 = super::super::oid::DATE;

    fn txt(v: SqlValue) -> String {
        match v {
            SqlValue::Text(s) => s,
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn valid_iso_dates() {
        assert_eq!(txt(input(DATE, "1999-01-08").unwrap()), "1999-01-08");
        assert_eq!(txt(input(DATE, "1957-04-09").unwrap()), "1957-04-09");
        assert_eq!(txt(input(DATE, "2040-04-10").unwrap()), "2040-04-10");
        assert_eq!(txt(input(DATE, "0001-01-01").unwrap()), "0001-01-01");
        assert_eq!(txt(input(DATE, "9999-12-31").unwrap()), "9999-12-31");
    }

    #[test]
    fn unpadded_fields_are_repadded() {

        assert_eq!(txt(input(DATE, "1999-1-8").unwrap()), "1999-01-08");
        assert_eq!(txt(input(DATE, "500-3-2").unwrap()), "0500-03-02");
    }

    #[test]
    fn whitespace_is_trimmed() {
        assert_eq!(txt(input(DATE, "  1999-01-08  ").unwrap()), "1999-01-08");
        assert_eq!(txt(input(DATE, "\t2000-04-01\n").unwrap()), "2000-04-01");
    }

    #[test]
    fn leap_year_feb29_valid() {

        assert_eq!(txt(input(DATE, "1996-02-29").unwrap()), "1996-02-29");
        assert_eq!(txt(input(DATE, "2000-02-29").unwrap()), "2000-02-29");
    }

    #[test]
    fn non_leap_feb29_rejected() {

        for s in ["1997-02-29", "1900-02-29", "2100-02-29"] {
            let e = input(DATE, s).unwrap_err();
            assert_eq!(
                e,
                PgError::InvalidInputSyntax {
                    typ: "date",
                    input: s.into()
                }
            );
        }

        assert_eq!(txt(input(DATE, "1997-02-28").unwrap()), "1997-02-28");
    }

    #[test]
    fn month_range_rejects() {
        for s in ["1999-00-10", "1999-13-01"] {
            assert_eq!(
                input(DATE, s).unwrap_err(),
                PgError::InvalidInputSyntax {
                    typ: "date",
                    input: s.into()
                }
            );
        }
    }

    #[test]
    fn day_range_rejects() {

        for s in ["1999-01-00", "1999-04-31", "1999-06-31", "1999-01-32"] {
            assert_eq!(
                input(DATE, s).unwrap_err(),
                PgError::InvalidInputSyntax {
                    typ: "date",
                    input: s.into()
                }
            );
        }

        assert_eq!(txt(input(DATE, "1999-01-31").unwrap()), "1999-01-31");
    }

    #[test]
    fn year_range_rejects() {
        for s in ["0000-01-01", "10000-01-01"] {
            assert_eq!(
                input(DATE, s).unwrap_err(),
                PgError::InvalidInputSyntax {
                    typ: "date",
                    input: s.into()
                }
            );
        }
    }

    #[test]
    fn non_date_text_rejected() {
        for s in [
            "",
            "   ",
            "asdf",
            "1999",
            "1999-01",
            "1999-01-08-1",
            "1999/01/08",
            "-1999-01-08",
        ] {
            let e = input(DATE, s).unwrap_err();
            assert_eq!(
                e,
                PgError::InvalidInputSyntax {
                    typ: "date",
                    input: s.into()
                }
            );
            assert_eq!(
                e.message(),
                format!("invalid input syntax for type date: \"{s}\"")
            );
        }
    }

    #[test]
    fn non_numeric_fields_rejected() {
        for s in ["1999-Jan-08", "abcd-01-08", "1999-01-0x"] {
            assert_eq!(
                input(DATE, s).unwrap_err(),
                PgError::InvalidInputSyntax {
                    typ: "date",
                    input: s.into()
                }
            );
        }
    }

    #[test]
    fn output_returns_canonical_text() {
        assert_eq!(
            output(DATE, &SqlValue::Text("1999-01-08".into())),
            "1999-01-08"
        );
    }

    #[test]
    fn output_non_text_is_empty() {
        assert_eq!(output(DATE, &SqlValue::Null), "");
        assert_eq!(output(DATE, &SqlValue::Int(5)), "");
    }

    #[test]
    fn round_trips() {
        for s in ["1957-04-09", "1996-02-29", "2000-04-03", "9999-12-31"] {
            let v = input(DATE, s).unwrap();
            assert_eq!(output(DATE, &v), s);
        }
    }

    #[test]
    fn canonical_text_sorts_chronologically() {
        let mut got: Vec<String> = ["2040-04-10", "1957-04-09", "1996-02-29", "1996-03-01"]
            .iter()
            .map(|s| txt(input(DATE, s).unwrap()))
            .collect();
        got.sort();
        assert_eq!(
            got,
            ["1957-04-09", "1996-02-29", "1996-03-01", "2040-04-10"]
        );
    }

    #[test]
    fn deferred_formats_are_rejected_for_now() {

        let deferred = [
            "January 8, 1999",
            "1/8/1999",
            "18/1/1999",
            "01/02/03",
            "19990108",
            "990108",
            "1999.008",
            "J2451187",
            "99-Jan-08",
            "08-Jan-1999",
            "2040-04-10 BC",
            "today",
            "infinity",
        ];
        for s in deferred {
            assert_eq!(
                input(DATE, s).unwrap_err(),
                PgError::InvalidInputSyntax {
                    typ: "date",
                    input: s.into()
                },
                "deferred format {s:?} should reject as InvalidInputSyntax"
            );
        }
    }
}
