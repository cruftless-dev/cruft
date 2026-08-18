
use crate::types::{self, oid, PgError};
use sql_core::SqlValue;

fn does_not_exist(name: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("function {name}(...) does not exist"),
    }
}

fn as_int(v: &SqlValue) -> Option<i64> {
    match v {
        SqlValue::Int(i) => Some(*i),
        _ => None,
    }
}

fn as_f64(v: &SqlValue) -> Option<f64> {
    match v {
        SqlValue::Real(r) => Some(*r),
        SqlValue::Int(i) => Some(*i as f64),
        _ => None,
    }
}

fn any_null(args: &[SqlValue]) -> bool {
    args.iter().any(|a| matches!(a, SqlValue::Null))
}

fn seconds_field(sec: f64) -> String {
    if !sec.is_finite() || sec < 0.0 {
        return "xx".to_string();
    }
    let total_us = (sec * 1_000_000.0).round() as i64;
    let whole = total_us / 1_000_000;
    let frac_us = total_us % 1_000_000;
    let mut s = format!("{whole:02}");
    if frac_us > 0 {
        s.push('.');
        s.push_str(format!("{frac_us:06}").trim_end_matches('0'));
    }
    s
}

fn make_date(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() != 3 {
        return Err(does_not_exist(name));
    }
    if any_null(args) {
        return Ok(SqlValue::Null);
    }
    let (Some(year), Some(month), Some(day)) =
        (as_int(&args[0]), as_int(&args[1]), as_int(&args[2]))
    else {
        return Err(does_not_exist(name));
    };

    let candidate = format!("{year}-{month}-{day}");
    types::date::input(oid::DATE, &candidate)
}

fn make_time(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() != 3 {
        return Err(does_not_exist(name));
    }
    if any_null(args) {
        return Ok(SqlValue::Null);
    }
    let (Some(hour), Some(min), Some(sec)) = (as_int(&args[0]), as_int(&args[1]), as_f64(&args[2]))
    else {
        return Err(does_not_exist(name));
    };
    let candidate = format!("{hour:02}:{min:02}:{}", seconds_field(sec));
    types::time::input(oid::TIME, &candidate)
}

fn make_timestamp(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() != 6 {
        return Err(does_not_exist(name));
    }
    if any_null(args) {
        return Ok(SqlValue::Null);
    }
    let (Some(year), Some(month), Some(mday), Some(hour), Some(min), Some(sec)) = (
        as_int(&args[0]),
        as_int(&args[1]),
        as_int(&args[2]),
        as_int(&args[3]),
        as_int(&args[4]),
        as_f64(&args[5]),
    ) else {
        return Err(does_not_exist(name));
    };

    let candidate = format!(
        "{year:04}-{month:02}-{mday:02} {hour:02}:{min:02}:{}",
        seconds_field(sec)
    );
    types::timestamp::input(oid::TIMESTAMP, &candidate)
}

fn make_interval(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {

    if args.len() != 7 {
        return Err(does_not_exist(name));
    }
    if any_null(args) {
        return Ok(SqlValue::Null);
    }
    let (Some(years), Some(months), Some(weeks), Some(days), Some(hours), Some(mins), Some(secs)) = (
        as_int(&args[0]),
        as_int(&args[1]),
        as_int(&args[2]),
        as_int(&args[3]),
        as_int(&args[4]),
        as_int(&args[5]),
        as_f64(&args[6]),
    ) else {
        return Err(does_not_exist(name));
    };
    if !secs.is_finite() {

        return Err(does_not_exist(name));
    }

    let candidate = format!(
        "{years} years {months} months {weeks} weeks {days} days \
         {hours} hours {mins} mins {secs} secs"
    );
    types::interval::input(oid::INTERVAL, &candidate)
}

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    match name {
        "make_date" => Some(make_date(name, args)),
        "make_time" => Some(make_time(name, args)),
        "make_timestamp" => Some(make_timestamp(name, args)),
        "make_interval" => Some(make_interval(name, args)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn txt(r: Option<Result<SqlValue, PgError>>) -> String {
        match r {
            Some(Ok(SqlValue::Text(s))) => s,
            other => panic!("expected Some(Ok(Text)), got {other:?}"),
        }
    }

    fn i(n: i64) -> SqlValue {
        SqlValue::Int(n)
    }
    fn r(n: f64) -> SqlValue {
        SqlValue::Real(n)
    }

    #[test]
    fn make_date_canonical() {
        assert_eq!(
            txt(call("make_date", &[i(2013), i(7), i(15)])),
            "2013-07-15"
        );

        assert_eq!(txt(call("make_date", &[i(500), i(3), i(2)])), "0500-03-02");
    }

    #[test]
    fn make_time_canonical() {
        assert_eq!(txt(call("make_time", &[i(8), i(20), r(0.0)])), "08:20:00");
        assert_eq!(
            txt(call("make_time", &[i(8), i(15), r(23.5)])),
            "08:15:23.5"
        );
    }

    #[test]
    fn make_timestamp_canonical() {
        assert_eq!(
            txt(call(
                "make_timestamp",
                &[i(2013), i(7), i(15), i(8), i(15), r(23.5)]
            )),
            "2013-07-15 08:15:23.5"
        );

        assert_eq!(
            txt(call(
                "make_timestamp",
                &[i(1997), i(1), i(2), i(3), i(4), r(5.0)]
            )),
            "1997-01-02 03:04:05"
        );
    }

    #[test]
    fn make_interval_canonical() {

        assert_eq!(
            txt(call(
                "make_interval",
                &[i(2), i(0), i(0), i(0), i(0), i(0), r(0.0)]
            )),
            "2 years"
        );

        assert_eq!(
            txt(call(
                "make_interval",
                &[i(1), i(6), i(0), i(0), i(0), i(0), r(0.0)]
            )),
            "1 year 6 mons"
        );

        assert_eq!(
            txt(call(
                "make_interval",
                &[i(1), i(-1), i(5), i(-7), i(25), i(-180), r(0.0)]
            )),
            "11 mons 28 days 22:00:00"
        );

        assert_eq!(
            txt(call(
                "make_interval",
                &[i(0), i(0), i(0), i(0), i(0), i(0), r(0.0)]
            )),
            "00:00:00"
        );

        assert_eq!(
            txt(call(
                "make_interval",
                &[i(0), i(0), i(0), i(0), i(-2), i(-10), r(-25.3)]
            )),
            "-02:10:25.3"
        );
    }

    #[test]
    fn fractional_seconds_round_to_micros() {
        assert_eq!(
            txt(call("make_time", &[i(13), i(30), r(25.575401)])),
            "13:30:25.575401"
        );
        assert_eq!(
            txt(call(
                "make_interval",
                &[i(0), i(0), i(0), i(0), i(0), i(0), r(1.5)]
            )),
            "00:00:01.5"
        );
    }

    #[test]
    fn make_date_leap_day() {

        assert_eq!(
            txt(call("make_date", &[i(1996), i(2), i(29)])),
            "1996-02-29"
        );
        assert_eq!(
            txt(call("make_date", &[i(2000), i(2), i(29)])),
            "2000-02-29"
        );
        assert!(matches!(
            call("make_date", &[i(1997), i(2), i(29)]),
            Some(Err(_))
        ));
        assert!(matches!(
            call("make_date", &[i(1900), i(2), i(29)]),
            Some(Err(_))
        ));
    }

    #[test]
    fn field_out_of_range_rejects() {

        assert!(matches!(
            call("make_date", &[i(2013), i(13), i(1)]),
            Some(Err(_))
        ));
        assert!(matches!(
            call("make_date", &[i(2013), i(2), i(30)]),
            Some(Err(_))
        ));
        assert!(matches!(
            call("make_date", &[i(2013), i(11), i(-1)]),
            Some(Err(_))
        ));
        assert!(matches!(
            call("make_time", &[i(10), i(55), r(100.1)]),
            Some(Err(_))
        ));
        assert!(matches!(
            call("make_time", &[i(24), i(0), r(2.1)]),
            Some(Err(_))
        ));

        assert!(matches!(
            call("make_date", &[i(0), i(7), i(15)]),
            Some(Err(_))
        ));
        assert!(matches!(
            call("make_date", &[i(-44), i(3), i(15)]),
            Some(Err(_))
        ));
    }

    #[test]
    fn null_argument_yields_null() {
        assert!(matches!(
            call("make_date", &[SqlValue::Null, i(7), i(15)]),
            Some(Ok(SqlValue::Null))
        ));
        assert!(matches!(
            call("make_time", &[i(8), SqlValue::Null, r(0.0)]),
            Some(Ok(SqlValue::Null))
        ));
        assert!(matches!(
            call(
                "make_timestamp",
                &[i(2013), i(7), i(15), i(8), i(15), SqlValue::Null]
            ),
            Some(Ok(SqlValue::Null))
        ));
        assert!(matches!(
            call(
                "make_interval",
                &[i(1), i(0), i(0), i(0), i(0), i(0), SqlValue::Null]
            ),
            Some(Ok(SqlValue::Null))
        ));
    }

    #[test]
    fn wrong_arity_or_type_is_does_not_exist() {

        match call("make_date", &[i(2013), i(7)]) {
            Some(Err(PgError::InvalidInputSyntax { typ, input })) => {
                assert_eq!(typ, "expression");
                assert_eq!(input, "function make_date(...) does not exist");
            }
            other => panic!("expected does-not-exist, got {other:?}"),
        }

        assert!(matches!(
            call("make_date", &[SqlValue::Text("x".into()), i(7), i(15)]),
            Some(Err(PgError::InvalidInputSyntax {
                typ: "expression",
                ..
            }))
        ));

        assert!(matches!(
            call("make_interval", &[i(2)]),
            Some(Err(PgError::InvalidInputSyntax {
                typ: "expression",
                ..
            }))
        ));
    }

    #[test]
    fn unclaimed_name_returns_none() {
        assert!(call("make_timestamptz", &[]).is_none());
        assert!(call("now", &[]).is_none());
        assert!(call("age", &[i(1)]).is_none());
    }
}
