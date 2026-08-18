
use super::PgError;
use sql_core::SqlValue;

const USECS_PER_SEC: i64 = 1_000_000;
const USECS_PER_MIN: i64 = 60 * USECS_PER_SEC;
const USECS_PER_HOUR: i64 = 60 * USECS_PER_MIN;
const USECS_PER_DAY_F: f64 = 86_400.0 * 1_000_000.0;
const DAYS_PER_MONTH_F: f64 = 30.0;

fn invalid(text: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: super::type_name(super::oid::INTERVAL),
        input: text.to_string(),
    }
}

#[derive(Default, Clone, Copy)]
struct Parts {
    months: i64,
    days: i64,
    micros: i64,
}

impl Parts {

    fn add_months(&mut self, val: f64) {
        let whole = val.trunc();
        self.months += whole as i64;
        let frac = val - whole;
        if frac != 0.0 {
            self.add_days(frac * DAYS_PER_MONTH_F);
        }
    }

    fn add_days(&mut self, val: f64) {
        let whole = val.trunc();
        self.days += whole as i64;
        let frac = val - whole;
        if frac != 0.0 {
            self.add_micros_f(frac * USECS_PER_DAY_F);
        }
    }

    fn add_micros_f(&mut self, val: f64) {
        self.micros += val.round() as i64;
    }
}

enum Unit {
    Year,
    Month,
    Week,
    Day,
    Hour,
    Minute,
    Second,
}

fn lookup_unit(word: &str) -> Option<Unit> {
    let w = word.to_ascii_lowercase();
    Some(match w.as_str() {
        "y" | "yr" | "yrs" | "year" | "years" => Unit::Year,
        "mon" | "mons" | "month" | "months" => Unit::Month,
        "w" | "week" | "weeks" => Unit::Week,
        "d" | "day" | "days" => Unit::Day,
        "h" | "hr" | "hrs" | "hour" | "hours" => Unit::Hour,
        "m" | "min" | "mins" | "minute" | "minutes" => Unit::Minute,
        "s" | "sec" | "secs" | "second" | "seconds" => Unit::Second,
        _ => return None,
    })
}

fn apply_unit(p: &mut Parts, unit: Unit, val: f64) {
    match unit {
        Unit::Year => p.add_months(val * 12.0),
        Unit::Month => p.add_months(val),
        Unit::Week => p.add_days(val * 7.0),
        Unit::Day => p.add_days(val),
        Unit::Hour => p.add_micros_f(val * USECS_PER_HOUR as f64),
        Unit::Minute => p.add_micros_f(val * USECS_PER_MIN as f64),
        Unit::Second => p.add_micros_f(val * USECS_PER_SEC as f64),
    }
}

fn is_number(s: &str) -> bool {
    let body = s.strip_prefix(['+', '-']).unwrap_or(s);
    if body.is_empty() {
        return false;
    }
    match body.split_once('.') {
        Some((i, f)) => {
            !i.is_empty()
                && i.bytes().all(|b| b.is_ascii_digit())
                && !f.is_empty()
                && f.bytes().all(|b| b.is_ascii_digit())
        }
        None => body.bytes().all(|b| b.is_ascii_digit()),
    }
}

fn parse_year_month(tok: &str) -> Option<i64> {
    let (neg, rest) = match tok.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, tok.strip_prefix('+').unwrap_or(tok)),
    };
    let (y, m) = rest.split_once('-')?;
    if y.is_empty() || m.is_empty() {
        return None;
    }
    if !y.bytes().all(|b| b.is_ascii_digit()) || !m.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let years: i64 = y.parse().ok()?;
    let months: i64 = m.parse().ok()?;
    let total = years * 12 + months;
    Some(if neg { -total } else { total })
}

fn parse_time_field(tok: &str, text: &str) -> Result<i64, PgError> {
    let (neg, rest) = match tok.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, tok.strip_prefix('+').unwrap_or(tok)),
    };
    let fields: Vec<&str> = rest.split(':').collect();
    if fields.len() != 2 && fields.len() != 3 {
        return Err(invalid(text));
    }
    let hour = parse_uint(fields[0], text)?;
    let minute = parse_uint(fields[1], text)?;
    let (sec, usec) = if fields.len() == 3 {
        parse_seconds(fields[2], text)?
    } else {
        (0i64, 0i64)
    };
    let total = hour * USECS_PER_HOUR + minute * USECS_PER_MIN + sec * USECS_PER_SEC + usec;
    Ok(if neg { -total } else { total })
}

fn parse_uint(field: &str, text: &str) -> Result<i64, PgError> {
    if field.is_empty() || !field.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid(text));
    }
    field.parse::<i64>().map_err(|_| invalid(text))
}

fn parse_seconds(field: &str, text: &str) -> Result<(i64, i64), PgError> {
    let (int_part, frac_part) = match field.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (field, None),
    };
    let sec = parse_uint(int_part, text)?;
    let usec = match frac_part {
        None => 0,
        Some(frac) => {
            if frac.is_empty() || !frac.bytes().all(|b| b.is_ascii_digit()) {
                return Err(invalid(text));
            }
            let bytes = frac.as_bytes();
            let mut m: i64 = 0;
            for i in 0..6 {
                let d = if i < bytes.len() {
                    (bytes[i] - b'0') as i64
                } else {
                    0
                };
                m = m * 10 + d;
            }
            if bytes.len() > 6 && bytes[6] >= b'5' {
                m += 1;
            }
            m
        }
    };
    Ok((sec, usec))
}

fn parse_parts(text: &str) -> Result<Parts, PgError> {
    let mut body = text.trim();
    if body.is_empty() {
        return Err(invalid(text));
    }

    if let Some(rest) = body.strip_prefix('@') {
        body = rest.trim_start();
    }
    if body.is_empty() {
        return Err(invalid(text));
    }

    let mut p = Parts::default();
    let mut pending: Option<f64> = None;
    let mut saw_field = false;

    for tok in body.split_whitespace() {

        if let Some(months) = parse_year_month(tok) {
            if pending.is_some() {
                return Err(invalid(text));
            }
            p.months += months;
            saw_field = true;
            continue;
        }

        if tok.contains(':') {
            if pending.is_some() {
                return Err(invalid(text));
            }
            p.micros += parse_time_field(tok, text)?;
            saw_field = true;
            continue;
        }

        let split = tok
            .find(|c: char| c.is_ascii_alphabetic())
            .unwrap_or(tok.len());
        let (num_str, unit_str) = tok.split_at(split);

        match (num_str.is_empty(), unit_str.is_empty()) {

            (false, true) => {
                if !is_number(num_str) || pending.is_some() {
                    return Err(invalid(text));
                }
                pending = Some(num_str.parse::<f64>().map_err(|_| invalid(text))?);
            }

            (true, false) => {
                let unit = lookup_unit(unit_str).ok_or_else(|| invalid(text))?;
                let val = pending.take().ok_or_else(|| invalid(text))?;
                apply_unit(&mut p, unit, val);
                saw_field = true;
            }

            (false, false) => {
                if pending.is_some() || !is_number(num_str) {
                    return Err(invalid(text));
                }
                let unit = lookup_unit(unit_str).ok_or_else(|| invalid(text))?;
                let val = num_str.parse::<f64>().map_err(|_| invalid(text))?;
                apply_unit(&mut p, unit, val);
                saw_field = true;
            }
            (true, true) => return Err(invalid(text)),
        }
    }

    if pending.is_some() || !saw_field {
        return Err(invalid(text));
    }
    Ok(p)
}

fn add_int_part(out: &mut String, val: i64, unit: &str, is_zero: &mut bool, is_before: &mut bool) {
    if val == 0 {
        return;
    }
    if !*is_zero {
        out.push(' ');
    }
    if *is_before && val > 0 {
        out.push('+');
    }
    out.push_str(&val.to_string());
    out.push(' ');
    out.push_str(unit);
    if val != 1 {
        out.push('s');
    }
    *is_before = val < 0;
    *is_zero = false;
}

fn canonical(p: Parts) -> String {
    let year = p.months / 12;
    let mon = p.months % 12;
    let day = p.days;

    let neg = p.micros < 0;
    let a = p.micros.abs();
    let hour = a / USECS_PER_HOUR;
    let rem = a % USECS_PER_HOUR;
    let minute = rem / USECS_PER_MIN;
    let rem = rem % USECS_PER_MIN;
    let sec = rem / USECS_PER_SEC;
    let usec = rem % USECS_PER_SEC;

    let mut out = String::new();
    let mut is_zero = true;
    let mut is_before = false;
    add_int_part(&mut out, year, "year", &mut is_zero, &mut is_before);
    add_int_part(&mut out, mon, "mon", &mut is_zero, &mut is_before);
    add_int_part(&mut out, day, "day", &mut is_zero, &mut is_before);

    if is_zero || p.micros != 0 {
        if !is_zero {
            out.push(' ');
        }
        if neg {
            out.push('-');
        } else if is_before {
            out.push('+');
        }
        out.push_str(&format!("{hour:02}:{minute:02}:{sec:02}"));
        if usec > 0 {
            let frac = format!("{usec:06}");
            out.push('.');
            out.push_str(frac.trim_end_matches('0'));
        }
    }
    out
}

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    debug_assert_eq!(oid, super::oid::INTERVAL);
    let _ = oid;

    match text.trim().to_ascii_lowercase().as_str() {
        "infinity" | "+infinity" => return Ok(SqlValue::Text("infinity".to_string())),
        "-infinity" => return Ok(SqlValue::Text("-infinity".to_string())),
        _ => {}
    }
    let parts = parse_parts(text)?;
    Ok(SqlValue::Text(canonical(parts)))
}

pub fn output(oid: u32, v: &SqlValue) -> String {
    let _ = oid;
    match v {
        SqlValue::Text(s) => s.clone(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::oid::INTERVAL;
    use super::*;

    fn parse(s: &str) -> Result<String, PgError> {
        match input(INTERVAL, s) {
            Ok(SqlValue::Text(t)) => Ok(t),
            Ok(_) => panic!("interval input must produce Text"),
            Err(e) => Err(e),
        }
    }

    #[test]
    fn unit_word_forms() {
        assert_eq!(
            parse("1 year 2 months 3 days").unwrap(),
            "1 year 2 mons 3 days"
        );
        assert_eq!(parse("6 years").unwrap(), "6 years");
        assert_eq!(parse("5 months").unwrap(), "5 mons");
        assert_eq!(parse("10 day").unwrap(), "10 days");
        assert_eq!(parse("34 year").unwrap(), "34 years");
        assert_eq!(parse("3 months").unwrap(), "3 mons");
    }

    #[test]
    fn unit_abbreviations_and_fused() {

        assert_eq!(parse("5months").unwrap(), "5 mons");
        assert_eq!(parse("2 yrs 3 mon").unwrap(), "2 years 3 mons");
        assert_eq!(parse("90 min").unwrap(), "01:30:00");
        assert_eq!(parse("5 hr").unwrap(), "05:00:00");
        assert_eq!(parse("1 d 1 h 1 m 1 s").unwrap(), "1 day 01:01:01");
    }

    #[test]
    fn month_normalization_to_years() {

        assert_eq!(parse("14 months").unwrap(), "1 year 2 mons");
        assert_eq!(parse("12 months").unwrap(), "1 year");
        assert_eq!(parse("24 mons").unwrap(), "2 years");
    }

    #[test]
    fn hh_mm_ss_time() {
        assert_eq!(parse("04:05:06").unwrap(), "04:05:06");
        assert_eq!(parse("01:00").unwrap(), "01:00:00");
        assert_eq!(parse("+02:00").unwrap(), "02:00:00");
        assert_eq!(parse("-08:00").unwrap(), "-08:00:00");
    }

    #[test]
    fn combined_date_and_time() {
        assert_eq!(parse("1 day 02:03:04").unwrap(), "1 day 02:03:04");
        assert_eq!(
            parse("1 day 2 hours 3 minutes 4 seconds").unwrap(),
            "1 day 02:03:04"
        );
        assert_eq!(parse("5 months 12 hours").unwrap(), "5 mons 12:00:00");
    }

    #[test]
    fn negative_components_and_mixed_signs() {

        assert_eq!(
            parse("10 years -11 month -12 days +13:14").unwrap(),
            "9 years 1 mon -12 days +13:14:00"
        );
        assert_eq!(parse("-1 days +02:03").unwrap(), "-1 days +02:03:00");

        assert_eq!(parse("14 seconds").unwrap(), "00:00:14");
        assert_eq!(parse("-14 seconds").unwrap(), "-00:00:14");
    }

    #[test]
    fn fractional_units_cascade() {

        assert_eq!(parse("1.5 weeks").unwrap(), "10 days 12:00:00");
        assert_eq!(parse("1.5 months").unwrap(), "1 mon 15 days");
    }

    #[test]
    fn fractional_seconds() {
        assert_eq!(parse("04:05:06.5").unwrap(), "04:05:06.5");
        assert_eq!(parse("00:00:00.123456").unwrap(), "00:00:00.123456");

        assert_eq!(parse("00:00:01.100000").unwrap(), "00:00:01.1");

        assert_eq!(parse("00:00:00.0000005").unwrap(), "00:00:00.000001");
        assert_eq!(parse("1.5 seconds").unwrap(), "00:00:01.5");
    }

    #[test]
    fn iso_year_month_field() {
        assert_eq!(parse("1-2").unwrap(), "1 year 2 mons");
        assert_eq!(parse("0-6").unwrap(), "6 mons");
        assert_eq!(parse("2-0").unwrap(), "2 years");
    }

    #[test]
    fn leading_at_marker() {
        assert_eq!(parse("@ 34 year").unwrap(), "34 years");
        assert_eq!(parse("@ 5 hour").unwrap(), "05:00:00");
        assert_eq!(parse("@ 1 minute").unwrap(), "00:01:00");
    }

    #[test]
    fn infinity_sentinels() {
        assert_eq!(parse("infinity").unwrap(), "infinity");
        assert_eq!(parse("-infinity").unwrap(), "-infinity");
        assert_eq!(parse("Infinity").unwrap(), "infinity");
    }

    #[test]
    fn all_zero_renders_time() {
        assert_eq!(parse("0 seconds").unwrap(), "00:00:00");
        assert_eq!(parse("00:00:00").unwrap(), "00:00:00");
    }

    #[test]
    fn large_hours_not_wrapped() {

        assert_eq!(parse("100 hours").unwrap(), "100:00:00");
    }

    #[test]
    fn invalid_rejects() {
        assert!(parse("").is_err());
        assert!(parse("   ").is_err());
        assert!(parse("badly formatted interval").is_err());
        assert!(parse("garbage").is_err());
        assert!(parse("30 eons").is_err());
        assert!(parse("5").is_err());
        assert!(parse("years").is_err());
        assert!(parse("1 2 days").is_err());
        assert!(parse("@").is_err());
        assert!(parse("1 year garbage").is_err());
    }

    #[test]
    fn error_is_invalid_input_syntax() {
        match input(INTERVAL, "garbage") {
            Err(PgError::InvalidInputSyntax { typ, input }) => {
                assert_eq!(typ, "interval");
                assert_eq!(input, "garbage");
            }
            other => panic!("expected InvalidInputSyntax, got {other:?}"),
        }
    }

    #[test]
    fn round_trips() {
        for s in [
            "1 year 2 mons 3 days",
            "9 years 1 mon -12 days +13:14:00",
            "1 mon 15 days",
            "10 days 12:00:00",
            "-1 days +02:03:00",
            "-00:00:14",
            "5 mons 12:00:00",
            "04:05:06.5",
            "00:00:00",
            "infinity",
            "-infinity",
        ] {
            let v = input(INTERVAL, s).unwrap();
            let rendered = output(INTERVAL, &v);
            assert_eq!(rendered, s, "canonical form is not a fixed point");
            let v2 = input(INTERVAL, &rendered).unwrap();
            assert_eq!(output(INTERVAL, &v2), s);
        }
    }

    #[test]
    fn deferred_forms_rejected_this_rung() {

        assert!(parse("14 seconds ago").is_err());
        assert!(parse("@ 14 seconds ago").is_err());

        assert!(parse("P1Y2M3DT4H5M6S").is_err());
        assert!(parse("P1Y2M").is_err());

        assert!(parse("@ 14 secs ago").is_err());
    }

    #[test]
    fn output_non_text_is_empty() {
        assert_eq!(output(INTERVAL, &SqlValue::Null), "");
        assert_eq!(output(INTERVAL, &SqlValue::Int(5)), "");
    }
}
