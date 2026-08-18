
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RoundingMode {
    Ceil,
    Floor,
    Expand,
    Trunc,
    HalfCeil,
    HalfFloor,
    HalfExpand,
    HalfTrunc,
    HalfEven,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UnsignedRoundingMode {
    Zero,
    Infinity,
    HalfZero,
    HalfInfinity,
    HalfEven,
}

impl RoundingMode {

    pub fn negate(self) -> Self {
        use RoundingMode::*;
        match self {
            Ceil => Floor,
            Floor => Ceil,
            HalfCeil => HalfFloor,
            HalfFloor => HalfCeil,
            Trunc => Trunc,
            Expand => Expand,
            HalfTrunc => HalfTrunc,
            HalfExpand => HalfExpand,
            HalfEven => HalfEven,
        }
    }

    pub fn get_unsigned_round_mode(self, is_positive: bool) -> UnsignedRoundingMode {
        use RoundingMode::*;
        use UnsignedRoundingMode as U;
        match self {
            Ceil if is_positive => U::Infinity,
            Ceil | Trunc => U::Zero,
            Floor if is_positive => U::Zero,
            Floor | Expand => U::Infinity,
            HalfCeil if is_positive => U::HalfInfinity,
            HalfCeil | HalfTrunc => U::HalfZero,
            HalfFloor if is_positive => U::HalfZero,
            HalfFloor | HalfExpand => U::HalfInfinity,
            HalfEven => U::HalfEven,
        }
    }
}

fn apply_unsigned_rounding_mode(dividend: i128, divisor: i128, mode: UnsignedRoundingMode) -> i128 {
    let r1 = dividend.div_euclid(divisor);
    let rem = dividend.rem_euclid(divisor);
    if rem == 0 {
        return r1;
    }
    let r2 = r1 + 1;
    match mode {
        UnsignedRoundingMode::Zero => r1,
        UnsignedRoundingMode::Infinity => r2,
        _ => {

            let midway = divisor.div_euclid(2);
            let cmp = rem.cmp(&midway);

            let cmp = if cmp == core::cmp::Ordering::Equal && divisor.rem_euclid(2) != 0 {
                core::cmp::Ordering::Less
            } else {
                cmp
            };
            match cmp {
                core::cmp::Ordering::Less => r1,
                core::cmp::Ordering::Greater => r2,
                core::cmp::Ordering::Equal => match mode {
                    UnsignedRoundingMode::HalfZero => r1,
                    UnsignedRoundingMode::HalfInfinity => r2,
                    UnsignedRoundingMode::HalfEven => {
                        if r1.rem_euclid(2) == 0 {
                            r1
                        } else {
                            r2
                        }
                    }
                    _ => unreachable!("Zero/Infinity handled above"),
                },
            }
        }
    }
}

pub fn round_to_increment(number: i128, increment: i128, mode: RoundingMode) -> i128 {
    debug_assert!(increment > 0);
    let is_positive = number >= 0;
    let unsigned = mode.get_unsigned_round_mode(is_positive);
    let magnitude = number.abs();
    let mut quotient = apply_unsigned_rounding_mode(magnitude, increment, unsigned);
    if !is_positive {
        quotient = -quotient;
    }
    quotient * increment
}

pub const NS_PER_MICROSECOND: i128 = 1_000;
pub const NS_PER_MILLISECOND: i128 = 1_000_000;
pub const NS_PER_SECOND: i128 = 1_000_000_000;
pub const NS_PER_MINUTE: i128 = 60 * NS_PER_SECOND;
pub const NS_PER_HOUR: i128 = 60 * NS_PER_MINUTE;
pub const NS_PER_DAY: i128 = 24 * NS_PER_HOUR;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Unit {
    Year,
    Month,
    Week,
    Day,
    Hour,
    Minute,
    Second,
    Millisecond,
    Microsecond,
    Nanosecond,
}

impl Unit {

    pub fn as_nanoseconds(self) -> Option<i128> {
        match self {
            Unit::Hour => Some(NS_PER_HOUR),
            Unit::Minute => Some(NS_PER_MINUTE),
            Unit::Second => Some(NS_PER_SECOND),
            Unit::Millisecond => Some(NS_PER_MILLISECOND),
            Unit::Microsecond => Some(NS_PER_MICROSECOND),
            Unit::Nanosecond => Some(1),

            Unit::Day => Some(NS_PER_DAY),
            Unit::Year | Unit::Month | Unit::Week => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Hash)]
pub struct TimeDuration(pub i128);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct TimeComponents {
    pub days: i128,
    pub hours: i128,
    pub minutes: i128,
    pub seconds: i128,
    pub milliseconds: i128,
    pub microseconds: i128,
    pub nanoseconds: i128,
}

impl TimeDuration {

    pub fn from_components(
        hours: i64,
        minutes: i64,
        seconds: i64,
        milliseconds: i64,
        microseconds: i128,
        nanoseconds: i128,
    ) -> Self {
        let mut ns: i128 = 0;
        ns += hours as i128 * NS_PER_HOUR;
        ns += minutes as i128 * NS_PER_MINUTE;
        ns += seconds as i128 * NS_PER_SECOND;
        ns += milliseconds as i128 * NS_PER_MILLISECOND;
        ns += microseconds * NS_PER_MICROSECOND;
        ns += nanoseconds;
        TimeDuration(ns)
    }

    pub fn total(self, unit: Unit) -> Option<(i128, i128)> {
        unit.as_nanoseconds().map(|d| (self.0, d))
    }

    pub fn total_f64(self, unit: Unit) -> Option<f64> {
        self.total(unit).map(|(n, d)| rational_to_f64(n, d))
    }

    pub fn round(
        self,
        smallest_unit: Unit,
        increment: i128,
        mode: RoundingMode,
    ) -> Option<TimeDuration> {
        let unit_ns = smallest_unit.as_nanoseconds()?;
        let step = unit_ns.checked_mul(increment)?;
        Some(TimeDuration(round_to_increment(self.0, step, mode)))
    }

    pub fn to_components(self, largest_unit: Unit) -> TimeComponents {
        let sign: i128 = if self.0 < 0 { -1 } else { 1 };
        let mut rem = self.0.abs();

        let scales = [
            NS_PER_DAY,
            NS_PER_HOUR,
            NS_PER_MINUTE,
            NS_PER_SECOND,
            NS_PER_MILLISECOND,
            NS_PER_MICROSECOND,
            1,
        ];

        let start = match largest_unit {
            Unit::Year | Unit::Month | Unit::Week | Unit::Day => 0,
            Unit::Hour => 1,
            Unit::Minute => 2,
            Unit::Second => 3,
            Unit::Millisecond => 4,
            Unit::Microsecond => 5,
            Unit::Nanosecond => 6,
        };
        let mut out = [0i128; 7];
        for i in start..7 {
            out[i] = (rem / scales[i]) * sign;
            rem %= scales[i];
        }
        TimeComponents {
            days: out[0],
            hours: out[1],
            minutes: out[2],
            seconds: out[3],
            milliseconds: out[4],
            microseconds: out[5],
            nanoseconds: out[6],
        }
    }
}

fn rational_to_f64(n: i128, d: i128) -> f64 {
    debug_assert!(d > 0);
    let negative = n < 0;
    let abs_n = n.abs();
    let q = abs_n / d;
    let mut r = abs_n % d;
    if r == 0 {
        let v = q as f64;
        return if negative { -v } else { v };
    }
    let mut s = String::new();
    if negative {
        s.push('-');
    }
    s.push_str(&q.to_string());
    s.push('.');
    for _ in 0..40 {
        r *= 10;
        let digit = r / d;
        s.push(char::from(b'0' + digit as u8));
        r %= d;
        if r == 0 {
            break;
        }
    }
    s.parse::<f64>().unwrap_or_else(|_| {
        let v = q as f64;
        if negative {
            -v
        } else {
            v
        }
    })
}

use rusty_js_temporal_calendar as cal;

impl UnsignedRoundingMode {

    fn apply_value(self, dividend: i128, divisor: i128, r1: i128, r2: i128) -> i128 {
        if dividend == r1 * divisor {
            return r1;
        }
        if self == UnsignedRoundingMode::Zero {
            return r1;
        }
        if self == UnsignedRoundingMode::Infinity {
            return r2;
        }
        let d1 = dividend - r1 * divisor;
        let d2 = r2 * divisor - dividend;
        match d1.cmp(&d2) {
            core::cmp::Ordering::Less => r1,
            core::cmp::Ordering::Greater => r2,
            core::cmp::Ordering::Equal => match self {
                UnsignedRoundingMode::HalfZero => r1,
                UnsignedRoundingMode::HalfInfinity => r2,
                _ => {
                    let diff = r2 - r1;
                    if r1.div_euclid(diff).rem_euclid(2) == 0 {
                        r1
                    } else {
                        r2
                    }
                }
            },
        }
    }
}

pub fn nudge_calendar_unit(
    dd: cal::DateDuration,
    time_ns: i128,
    relative: cal::IsoDate,
    smallest: Unit,
    increment: i128,
    mode: RoundingMode,
) -> Result<cal::DateDuration, cal::RangeError> {

    let add = |dur: &cal::DateDuration| -> Result<cal::IsoDate, cal::RangeError> {
        relative.date_add(dur, cal::Overflow::Constrain)
    };
    let epoch = |d: cal::IsoDate| d.to_epoch_days() as i128 * NS_PER_DAY;

    let origin_epoch = epoch(relative);
    let dest_epoch = epoch(add(&dd)?) + time_ns;
    let sign: i128 = if dest_epoch >= origin_epoch { 1 } else { -1 };

    let trunc = |v: i64| round_to_increment(v as i128, increment, RoundingMode::Trunc);

    let window = |additional_shift: bool| -> Result<
        Option<(i128, i128, cal::DateDuration, cal::DateDuration)>,
        cal::RangeError,
    > {
        let shift = if additional_shift {
            increment * sign
        } else {
            0
        };
        Ok(Some(match smallest {
            Unit::Year => {
                let r1 = trunc(dd.years) + shift;
                let r2 = r1 + increment * sign;
                (
                    r1,
                    r2,
                    cal::DateDuration::new(r1 as i64, 0, 0, 0),
                    cal::DateDuration::new(r2 as i64, 0, 0, 0),
                )
            }
            Unit::Month => {
                let r1 = trunc(dd.months) + shift;
                let r2 = r1 + increment * sign;
                (
                    r1,
                    r2,
                    cal::DateDuration::new(dd.years, r1 as i64, 0, 0),
                    cal::DateDuration::new(dd.years, r2 as i64, 0, 0),
                )
            }
            Unit::Week => {
                let iso_one = add(&cal::DateDuration::new(dd.years, dd.months, 0, 0))?;
                let iso_two = add(&cal::DateDuration::new(dd.years, dd.months, 0, dd.days))?;
                let until = iso_one.date_until(iso_two, cal::Unit::Week);
                let r1 = trunc((dd.weeks + until.weeks) as i64) + shift;
                let r2 = r1 + increment * sign;
                (
                    r1,
                    r2,
                    cal::DateDuration::new(dd.years, dd.months, r1 as i64, 0),
                    cal::DateDuration::new(dd.years, dd.months, r2 as i64, 0),
                )
            }
            Unit::Day => {
                let r1 = trunc(dd.days) + shift;
                let r2 = r1 + increment * sign;
                (
                    r1,
                    r2,
                    cal::DateDuration::new(dd.years, dd.months, dd.weeks, r1 as i64),
                    cal::DateDuration::new(dd.years, dd.months, dd.weeks, r2 as i64),
                )
            }
            _ => return Ok(None),
        }))
    };
    let epoch_of = |dur: &cal::DateDuration| -> Result<i128, cal::RangeError> {
        if *dur == cal::DateDuration::default() {
            return Ok(origin_epoch);
        }
        if smallest == Unit::Day {

            let base = cal::DateDuration::new(dur.years, dur.months, dur.weeks, 0);
            let base_date = if base == cal::DateDuration::default() {
                relative
            } else {
                relative.date_add(&base, cal::Overflow::Constrain)?
            };
            return Ok(epoch(base_date) + dur.days as i128 * NS_PER_DAY);
        }
        Ok(epoch(add(dur)?))
    };

    let (r1, r2, start_dur, end_dur) = match window(false)? {
        Some(w) => w,
        None => return Ok(dd),
    };
    let mut start_epoch = epoch_of(&start_dur)?;

    if dest_epoch == start_epoch {
        return Ok(start_dur);
    }
    let mut end_epoch = epoch_of(&end_dur)?;
    let contained = if sign >= 0 {
        start_epoch <= dest_epoch && dest_epoch <= end_epoch
    } else {
        end_epoch <= dest_epoch && dest_epoch <= start_epoch
    };
    let (r1, r2, start_dur, end_dur) = if contained {
        (r1, r2, start_dur, end_dur)
    } else if let Some((r1s, r2s, sds, eds)) = window(true)? {
        start_epoch = epoch_of(&sds)?;
        end_epoch = epoch_of(&eds)?;
        (r1s, r2s, sds, eds)
    } else {
        (r1, r2, start_dur, end_dur)
    };
    let divisor = end_epoch - start_epoch;
    if divisor == 0 {
        return Ok(start_dur);
    }
    let dividend = dest_epoch - start_epoch;
    let total_times_divisor = r1 * divisor + dividend * increment * sign;

    let unsigned = mode.get_unsigned_round_mode(sign >= 0);
    let total_is_r2 = total_times_divisor.div_euclid(divisor) == r2
        && total_times_divisor.rem_euclid(divisor) == 0;
    let rounded_unit = if total_is_r2 {
        r2.abs()
    } else {
        unsigned.apply_value(total_times_divisor.abs(), divisor.abs(), r1.abs(), r2.abs())
    };
    if rounded_unit == r2.abs() {
        Ok(end_dur)
    } else {
        Ok(start_dur)
    }
}

pub fn total_calendar_unit(
    dd: cal::DateDuration,
    time_ns: i128,
    relative: cal::IsoDate,
    unit: Unit,
) -> Result<Option<f64>, cal::RangeError> {
    let cal_unit = match unit {
        Unit::Year => cal::Unit::Year,
        Unit::Month => cal::Unit::Month,
        Unit::Week => cal::Unit::Week,
        _ => return Ok(None),
    };

    let add = |n: i64| -> Result<cal::IsoDate, cal::RangeError> {
        let dur = match unit {
            Unit::Year => cal::DateDuration::new(n, 0, 0, 0),
            Unit::Month => cal::DateDuration::new(0, n, 0, 0),
            _ => cal::DateDuration::new(0, 0, n, 0),
        };
        relative.date_add(&dur, cal::Overflow::Constrain)
    };
    let epoch = |d: cal::IsoDate| d.to_epoch_days() as i128 * NS_PER_DAY;

    let dest_date = relative.date_add(&dd, cal::Overflow::Constrain)?;
    let dest_epoch = epoch(dest_date) + time_ns;
    let origin_epoch = epoch(relative);
    let sign: i64 = if dest_epoch >= origin_epoch { 1 } else { -1 };

    let mut whole: i64 = match unit {
        Unit::Year => relative.date_until(dest_date, cal_unit).years as i64,
        Unit::Month => relative.date_until(dest_date, cal_unit).months as i64,
        _ => relative.date_until(dest_date, cal_unit).weeks as i64,
    };

    let mut start_epoch = epoch(add(whole)?);
    for _ in 0..100_000 {
        let next_epoch = epoch(add(whole + sign)?);
        let past = if sign >= 0 {
            dest_epoch >= next_epoch
        } else {
            dest_epoch <= next_epoch
        };
        if !past {
            break;
        }
        whole += sign;
        start_epoch = next_epoch;
    }
    let end_epoch = epoch(add(whole + sign)?);
    let divisor = end_epoch - start_epoch;
    if divisor == 0 {
        return Ok(Some(whole as f64));
    }
    let dividend = dest_epoch - start_epoch;

    let denominator = divisor.abs();
    let numerator = whole as i128 * denominator + sign as i128 * dividend.abs();
    Ok(Some(rational_to_f64(numerator, denominator)))
}

pub fn bubble_relative_duration(
    mut dd: cal::DateDuration,
    relative: cal::IsoDate,
    largest: Unit,
    smallest: Unit,
) -> Result<cal::DateDuration, cal::RangeError> {
    fn rank(u: Unit) -> u8 {
        match u {
            Unit::Day => 1,
            Unit::Week => 2,
            Unit::Month => 3,
            Unit::Year => 4,
            _ => 0,
        }
    }
    if rank(largest) <= rank(smallest) || rank(smallest) == 0 {
        return Ok(dd);
    }
    let epoch = |d: cal::IsoDate| d.to_epoch_days() as i128;
    let add = |dur: &cal::DateDuration| -> Result<cal::IsoDate, cal::RangeError> {
        relative.date_add(dur, cal::Overflow::Constrain)
    };
    let nudged_epoch = epoch(add(&dd)?);
    let sign: i128 = if nudged_epoch >= epoch(relative) {
        1
    } else {
        -1
    };

    for larger in [Unit::Week, Unit::Month, Unit::Year] {
        if rank(larger) <= rank(smallest) {
            continue;
        }
        if rank(larger) > rank(largest) {
            break;
        }

        if larger == Unit::Week && largest != Unit::Week {
            continue;
        }
        let cand = match larger {
            Unit::Week => cal::DateDuration::new(dd.years, dd.months, dd.weeks + sign as i64, 0),
            Unit::Month => cal::DateDuration::new(dd.years, dd.months + sign as i64, 0, 0),
            Unit::Year => cal::DateDuration::new(dd.years + sign as i64, 0, 0, 0),
            _ => dd,
        };

        let cand_epoch = match add(&cand) {
            Ok(d) => epoch(d),
            Err(_) => break,
        };
        let reached = if sign >= 0 {
            nudged_epoch >= cand_epoch
        } else {
            nudged_epoch <= cand_epoch
        };
        if reached {
            dd = cand;
        } else {
            break;
        }
    }
    Ok(dd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use RoundingMode::*;

    #[test]
    fn rounding_modes_positive_table() {

        for (m, want) in [
            (Ceil, 20),
            (Floor, 10),
            (Expand, 20),
            (Trunc, 10),
            (HalfCeil, 10),
            (HalfFloor, 10),
            (HalfExpand, 10),
            (HalfTrunc, 10),
            (HalfEven, 10),
        ] {
            assert_eq!(round_to_increment(13, 10, m), want, "13->{:?}", m);
        }

        for (m, want) in [
            (Ceil, 20),
            (Floor, 10),
            (Expand, 20),
            (Trunc, 10),
            (HalfCeil, 20),
            (HalfFloor, 10),
            (HalfExpand, 20),
            (HalfTrunc, 10),
            (HalfEven, 20),
        ] {
            assert_eq!(round_to_increment(15, 10, m), want, "15->{:?}", m);
        }

        assert_eq!(round_to_increment(25, 10, HalfEven), 20);
        assert_eq!(round_to_increment(25, 10, HalfExpand), 30);

        for m in [
            Ceil, Floor, Expand, Trunc, HalfCeil, HalfFloor, HalfExpand, HalfTrunc, HalfEven,
        ] {
            assert_eq!(round_to_increment(20, 10, m), 20, "exact {:?}", m);
        }
    }

    #[test]
    fn rounding_modes_negative_table() {

        assert_eq!(round_to_increment(-13, 10, Ceil), -10);
        assert_eq!(round_to_increment(-13, 10, Floor), -20);
        assert_eq!(round_to_increment(-13, 10, Trunc), -10);
        assert_eq!(round_to_increment(-13, 10, Expand), -20);

        assert_eq!(round_to_increment(-15, 10, HalfExpand), -20);
        assert_eq!(round_to_increment(-15, 10, HalfEven), -20);

        assert_eq!(round_to_increment(-25, 10, HalfEven), -20);
    }

    #[test]
    fn unsigned_mode_resolution() {

        assert_eq!(
            Ceil.get_unsigned_round_mode(true),
            UnsignedRoundingMode::Infinity
        );
        assert_eq!(
            Ceil.get_unsigned_round_mode(false),
            UnsignedRoundingMode::Zero
        );
        assert_eq!(
            Floor.get_unsigned_round_mode(true),
            UnsignedRoundingMode::Zero
        );
        assert_eq!(
            Floor.get_unsigned_round_mode(false),
            UnsignedRoundingMode::Infinity
        );
        assert_eq!(Ceil.negate(), Floor);
        assert_eq!(HalfCeil.negate(), HalfFloor);
        assert_eq!(HalfEven.negate(), HalfEven);
    }

    #[test]
    fn time_duration_exact_components() {
        let td = TimeDuration::from_components(1, 2, 3, 4, 5, 6);
        let expected = NS_PER_HOUR
            + 2 * NS_PER_MINUTE
            + 3 * NS_PER_SECOND
            + 4 * NS_PER_MILLISECOND
            + 5 * NS_PER_MICROSECOND
            + 6;
        assert_eq!(td.0, expected);

        let (num, den) = td.total(Unit::Hour).unwrap();
        assert_eq!((num, den), (expected, NS_PER_HOUR));

        let h90 = TimeDuration::from_components(0, 90, 0, 0, 0, 0);
        assert_eq!(h90.total_f64(Unit::Hour).unwrap(), 1.5);

        assert!(td.total(Unit::Year).is_none());
    }

    #[test]
    fn balance_carries_up_to_largest_unit() {

        let td = TimeDuration::from_components(0, 0, 3661, 0, 0, 0);
        let c = td.to_components(Unit::Hour);
        assert_eq!((c.hours, c.minutes, c.seconds), (1, 1, 1));
        assert_eq!(c.days, 0);

        let c2 = td.to_components(Unit::Minute);
        assert_eq!((c2.minutes, c2.seconds), (61, 1));
        assert_eq!(c2.hours, 0);

        let td2 = TimeDuration::from_components(25, 0, 0, 0, 0, 0);
        let c3 = td2.to_components(Unit::Day);
        assert_eq!((c3.days, c3.hours), (1, 1));

        let neg = TimeDuration::from_components(0, 0, -3661, 0, 0, 0);
        let cn = neg.to_components(Unit::Hour);
        assert_eq!((cn.hours, cn.minutes, cn.seconds), (-1, -1, -1));

        let sub = TimeDuration::from_components(0, 0, 0, 123, 456, 789);
        let cs = sub.to_components(Unit::Millisecond);
        assert_eq!(
            (cs.milliseconds, cs.microseconds, cs.nanoseconds),
            (123, 456, 789)
        );
    }

    #[test]
    fn bubble_carries_months_to_years_and_respects_week_tier() {
        let rel = cal::IsoDate::new_unchecked(2022, 1, 1);

        let b = bubble_relative_duration(
            cal::DateDuration::new(1, 12, 0, 0),
            rel,
            Unit::Year,
            Unit::Month,
        );
        assert_eq!(b.unwrap(), cal::DateDuration::new(2, 0, 0, 0));

        let b2 = bubble_relative_duration(
            cal::DateDuration::new(1, 12, 0, 0),
            rel,
            Unit::Month,
            Unit::Month,
        );
        assert_eq!(b2.unwrap(), cal::DateDuration::new(1, 12, 0, 0));

        let b3 = bubble_relative_duration(
            cal::DateDuration::new(0, 0, 0, 28),
            rel,
            Unit::Month,
            Unit::Day,
        );
        assert_eq!(b3.unwrap().weeks, 0);
    }

    #[test]
    fn nudge_year_rounds_against_reference() {
        let rel = cal::IsoDate::new_unchecked(2024, 1, 1);

        let r = nudge_calendar_unit(
            cal::DateDuration::new(1, 6, 0, 0),
            0,
            rel,
            Unit::Year,
            1,
            HalfExpand,
        );
        assert_eq!(r.unwrap(), cal::DateDuration::new(1, 0, 0, 0));

        let r2 = nudge_calendar_unit(
            cal::DateDuration::new(1, 8, 0, 0),
            0,
            rel,
            Unit::Year,
            1,
            HalfExpand,
        );
        assert_eq!(r2.unwrap(), cal::DateDuration::new(2, 0, 0, 0));

        let r3 = nudge_calendar_unit(
            cal::DateDuration::new(1, 1, 0, 0),
            0,
            rel,
            Unit::Year,
            1,
            Ceil,
        );
        assert_eq!(r3.unwrap(), cal::DateDuration::new(2, 0, 0, 0));

        let r4 = nudge_calendar_unit(
            cal::DateDuration::new(1, 11, 0, 0),
            0,
            rel,
            Unit::Year,
            1,
            Trunc,
        );
        assert_eq!(r4.unwrap(), cal::DateDuration::new(1, 0, 0, 0));
    }

    #[test]
    fn nudge_month_exact_and_fractional() {
        let rel = cal::IsoDate::new_unchecked(2024, 1, 1);

        let r = nudge_calendar_unit(
            cal::DateDuration::new(1, 6, 0, 0),
            0,
            rel,
            Unit::Month,
            1,
            HalfExpand,
        );
        assert_eq!(r.unwrap(), cal::DateDuration::new(1, 6, 0, 0));

        let r2 = nudge_calendar_unit(
            cal::DateDuration::new(0, 1, 0, 15),
            0,
            rel,
            Unit::Month,
            1,
            HalfExpand,
        );
        assert_eq!(r2.unwrap(), cal::DateDuration::new(0, 2, 0, 0));
    }

    #[test]
    fn nudge_day_passthrough() {
        let rel = cal::IsoDate::new_unchecked(2024, 1, 1);

        let r = nudge_calendar_unit(
            cal::DateDuration::new(0, 0, 0, 3),
            18 * NS_PER_HOUR,
            rel,
            Unit::Day,
            1,
            HalfExpand,
        );
        assert_eq!(r.unwrap(), cal::DateDuration::new(0, 0, 0, 4));

        let r2 = nudge_calendar_unit(
            cal::DateDuration::new(0, 0, 0, 3),
            6 * NS_PER_HOUR,
            rel,
            Unit::Day,
            1,
            HalfExpand,
        );
        assert_eq!(r2.unwrap(), cal::DateDuration::new(0, 0, 0, 3));
    }

    #[test]
    fn round_time_duration() {

        let td = TimeDuration::from_components(1, 29, 30, 0, 0, 0);
        let r = td.round(Unit::Hour, 1, HalfExpand).unwrap();
        assert_eq!(r.0, NS_PER_HOUR);

        let r2 = td.round(Unit::Minute, 30, HalfExpand).unwrap();
        assert_eq!(r2.0, NS_PER_HOUR + 30 * NS_PER_MINUTE);

        assert_eq!(td.round(Unit::Hour, 1, Ceil).unwrap().0, 2 * NS_PER_HOUR);

        assert!(td.round(Unit::Month, 1, Trunc).is_none());
    }
}
