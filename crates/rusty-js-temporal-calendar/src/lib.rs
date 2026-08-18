
pub mod paschalion;

pub mod hebrew;

pub mod astronomy;

pub mod chinese;

pub mod calendars;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct RataDie(pub i64);

impl RataDie {
    pub const EPOCH: RataDie = RataDie(1);

    pub fn add(self, days: i64) -> RataDie {
        RataDie(self.0 + days)
    }

    pub fn since(self, other: RataDie) -> i64 {
        self.0 - other.0
    }

    pub fn until(self, other: RataDie) -> i64 {
        other.0 - self.0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct JulianFamily {

    pub epoch_offset: i64,

    pub century_correction: bool,
    pub leap_rule: LeapRule,
    pub month_lengths: MonthLengths,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LeapRule {
    Julian,
    Gregorian,
    GregorianYearOffset(i64),
    Coptic,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MonthLengths {
    Gregorian,
    CopticEpagomenal,
}

pub const JULIAN: JulianFamily = JulianFamily {
    epoch_offset: -2,
    century_correction: false,
    leap_rule: LeapRule::Julian,
    month_lengths: MonthLengths::Gregorian,
};

pub const GREGORIAN: JulianFamily = JulianFamily {
    epoch_offset: 0,
    century_correction: true,
    leap_rule: LeapRule::Gregorian,
    month_lengths: MonthLengths::Gregorian,
};

pub const BUDDHIST: JulianFamily = JulianFamily {
    epoch_offset: 0,
    century_correction: true,
    leap_rule: LeapRule::GregorianYearOffset(-543),
    month_lengths: MonthLengths::Gregorian,
};

pub const COPTIC: JulianFamily = JulianFamily {
    epoch_offset: 103604,
    century_correction: false,
    leap_rule: LeapRule::Coptic,
    month_lengths: MonthLengths::CopticEpagomenal,
};

pub const ETHIOPIC: JulianFamily = JulianFamily {
    epoch_offset: 2795,
    century_correction: false,
    leap_rule: LeapRule::Coptic,
    month_lengths: MonthLengths::CopticEpagomenal,
};

pub const ROC: JulianFamily = JulianFamily {
    epoch_offset: 0,
    century_correction: true,
    leap_rule: LeapRule::GregorianYearOffset(1911),
    month_lengths: MonthLengths::Gregorian,
};

impl JulianFamily {

    fn epoch(self) -> i64 {
        self.epoch_offset + 1
    }

    pub fn is_leap_year(self, year: i64) -> bool {
        match self.leap_rule {
            LeapRule::Julian => year.rem_euclid(4) == 0,
            LeapRule::Gregorian => Self::is_gregorian_leap_year(year),
            LeapRule::GregorianYearOffset(offset) => Self::is_gregorian_leap_year(year + offset),
            LeapRule::Coptic => year.rem_euclid(4) == 3,
        }
    }

    fn is_gregorian_leap_year(year: i64) -> bool {
        year.rem_euclid(4) == 0 && (year.rem_euclid(100) != 0 || year.rem_euclid(400) == 0)
    }

    fn leap_accumulator(self, year: i64) -> i64 {
        let ym1 = year - 1;
        match self.leap_rule {
            LeapRule::Julian => ym1.div_euclid(4),
            LeapRule::Gregorian => ym1.div_euclid(4) - ym1.div_euclid(100) + ym1.div_euclid(400),
            LeapRule::GregorianYearOffset(offset) => {
                GREGORIAN.day_before_year(year + offset)
                    - GREGORIAN.day_before_year(1 + offset)
                    - 365 * ym1
            }
            LeapRule::Coptic => year.div_euclid(4),
        }
    }

    pub fn day_before_year(self, year: i64) -> i64 {
        365 * (year - 1) + self.leap_accumulator(year)
    }

    pub fn days_before_month(self, year: i64, month: i64) -> i64 {
        match self.month_lengths {
            MonthLengths::Gregorian => match month {
                1 => 0,
                2 => 31,
                _ => {
                    let leap = if self.is_leap_year(year) { 1 } else { 0 };
                    31 + 28 + leap + ((979 * month - 2919) >> 5)
                }
            },
            MonthLengths::CopticEpagomenal => match month {
                1..=13 => (month - 1) * 30,
                _ => panic!("month out of range for Coptic/Ethiopic family"),
            },
        }
    }

    pub fn fixed_from(self, year: i64, month: i64, day: i64) -> RataDie {
        if let LeapRule::GregorianYearOffset(offset) = self.leap_rule {
            return GREGORIAN.fixed_from(year + offset, month, day);
        }
        RataDie(
            self.epoch_offset
                + self.day_before_year(year)
                + self.days_before_month(year, month)
                + day,
        )
    }

    pub fn year_from_fixed(self, rd: RataDie) -> i64 {
        if let LeapRule::GregorianYearOffset(offset) = self.leap_rule {
            return GREGORIAN.year_from_fixed(rd) - offset;
        }
        let d0 = rd.0 - self.epoch();
        if self.leap_rule == LeapRule::Gregorian {
            let (n400, d1) = (d0.div_euclid(146097), d0.rem_euclid(146097));
            let (n100, d2) = (d1.div_euclid(36524), d1.rem_euclid(36524));
            let (n4, d3) = (d2.div_euclid(1461), d2.rem_euclid(1461));
            let n1 = d3.div_euclid(365);
            let year = 400 * n400 + 100 * n100 + 4 * n4 + n1;
            if n100 == 4 || n1 == 4 {
                year
            } else {
                year + 1
            }
        } else if self.leap_rule == LeapRule::Julian {
            let (n4, d1) = (d0.div_euclid(1461), d0.rem_euclid(1461));
            let n1 = d1.div_euclid(365);
            let year = 4 * n4 + n1;
            if n1 == 4 {
                year
            } else {
                year + 1
            }
        } else {
            let estimate = d0.div_euclid(365) + 1;
            let mut lo = estimate - 4;
            let mut hi = estimate + 4;
            let mut span = 8;
            while self.fixed_from(hi, 1, 1).0 <= rd.0 {
                hi += span;
                span *= 2;
            }
            span = 8;
            while self.fixed_from(lo, 1, 1).0 > rd.0 {
                lo -= span;
                span *= 2;
            }
            while lo + 1 < hi {
                let mid = lo + (hi - lo).div_euclid(2);
                if self.fixed_from(mid, 1, 1).0 <= rd.0 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            lo
        }
    }

    pub fn ymd_from_fixed(self, rd: RataDie) -> (i64, i64, i64) {
        if let LeapRule::GregorianYearOffset(offset) = self.leap_rule {
            let (year, month, day) = GREGORIAN.ymd_from_fixed(rd);
            return (year - offset, month, day);
        }
        let year = self.year_from_fixed(rd);
        let day_of_year = rd.0 - self.fixed_from(year, 1, 1).0;
        let month = self.month_from_day_of_year(year, day_of_year);
        let day = day_of_year - self.days_before_month(year, month) + 1;
        (year, month, day)
    }

    fn month_from_day_of_year(self, year: i64, day_of_year: i64) -> i64 {
        match self.month_lengths {
            MonthLengths::Gregorian => {
                let correction = if day_of_year < self.days_before_month(year, 3) {
                    0
                } else if self.is_leap_year(year) {
                    1
                } else {
                    2
                };
                (12 * (day_of_year + correction) + 373) / 367
            }
            MonthLengths::CopticEpagomenal => (day_of_year.div_euclid(30) + 1).min(13),
        }
    }

    pub fn days_in_year(self, year: i64) -> i64 {
        if self.is_leap_year(year) {
            366
        } else {
            365
        }
    }

    pub fn days_in_month(self, year: i64, month: i64) -> i64 {
        match self.month_lengths {
            MonthLengths::Gregorian => {
                let next = self.fixed_from(year, month, 1).0;
                let after = if month == 12 {
                    self.fixed_from(year + 1, 1, 1).0
                } else {
                    self.fixed_from(year, month + 1, 1).0
                };
                after - next
            }
            MonthLengths::CopticEpagomenal => {
                if month == 13 {
                    if self.is_leap_year(year) {
                        6
                    } else {
                        5
                    }
                } else {
                    30
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RangeError(pub &'static str);

pub type CalResult<T> = Result<T, RangeError>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Overflow {
    Constrain,
    Reject,
}

pub const UNIX_EPOCH_RD: i64 = 719163;

pub const MAX_EPOCH_DAYS: i64 = 100_000_000 + 1;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct IsoDate {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

impl IsoDate {

    pub const fn new_unchecked(year: i32, month: u8, day: u8) -> Self {
        IsoDate { year, month, day }
    }

    pub fn days_in_month(year: i32, month: u8) -> u8 {
        GREGORIAN.days_in_month(year as i64, month as i64) as u8
    }

    pub fn is_valid_date(year: i32, month: u8, day: u8) -> bool {
        (1..=12).contains(&month) && (1..=Self::days_in_month(year, month)).contains(&day)
    }

    pub fn is_valid(self) -> bool {
        Self::is_valid_date(self.year, self.month, self.day)
    }

    pub fn check_validity(self) -> CalResult<()> {
        if self.is_valid() {
            Ok(())
        } else {
            Err(RangeError("IsoDate does not have valid fields."))
        }
    }

    pub fn to_epoch_days(self) -> i64 {
        GREGORIAN
            .fixed_from(self.year as i64, self.month as i64, self.day as i64)
            .0
            - UNIX_EPOCH_RD
    }

    pub fn within_limits(self) -> bool {
        self.to_epoch_days().abs() <= MAX_EPOCH_DAYS
    }

    pub fn check_within_limits(self) -> CalResult<()> {
        if self.within_limits() {
            Ok(())
        } else {
            Err(RangeError("date is out of range."))
        }
    }

    pub fn regulate(year: i32, month: u8, day: u8, overflow: Overflow) -> CalResult<IsoDate> {
        match overflow {
            Overflow::Constrain => {
                let month = month.clamp(1, 12);
                let day = day.clamp(1, Self::days_in_month(year, month));
                Ok(IsoDate::new_unchecked(year, month, day))
            }
            Overflow::Reject => {
                if !Self::is_valid_date(year, month, day) {
                    return Err(RangeError("not a valid ISO date."));
                }
                Ok(IsoDate::new_unchecked(year, month, day))
            }
        }
    }

    pub fn new_with_overflow(
        year: i32,
        month: u8,
        day: u8,
        overflow: Overflow,
    ) -> CalResult<IsoDate> {
        let date = Self::regulate(year, month, day, overflow)?;
        date.check_within_limits()?;
        Ok(date)
    }
}

pub fn year_month_within_limits(year: i32, month: u8) -> bool {
    if !(-271821..=275760).contains(&year) {
        false
    } else if year == -271821 && month < 4 {
        false
    } else if year == 275760 && month > 9 {
        false
    } else {
        true
    }
}

pub fn balance_iso_year_month(year: i64, month: i64) -> (i64, u8) {
    let y = year + (month - 1).div_euclid(12);
    let m = ((month - 1).rem_euclid(12) + 1) as u8;
    (y, m)
}

impl IsoDate {

    pub fn balance(year: i64, month: i64, day: i64) -> IsoDate {
        let (y, m) = balance_iso_year_month(year, month);
        let rd = GREGORIAN.fixed_from(y, m as i64, 1).0 + (day - 1);
        let (y2, m2, d2) = GREGORIAN.ymd_from_fixed(RataDie(rd));
        IsoDate::new_unchecked(y2 as i32, m2 as u8, d2 as u8)
    }

    pub fn try_balance(year: i64, month: i64, day: i64) -> CalResult<IsoDate> {
        let (y, m) = balance_iso_year_month(year, month);
        let epoch_days = GREGORIAN.fixed_from(y, m as i64, 1).0 + (day - 1) - UNIX_EPOCH_RD;
        if epoch_days.abs() > MAX_EPOCH_DAYS {
            return Err(RangeError("epoch days exceed maximum range."));
        }
        Ok(Self::balance(year, month, day))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct DateDuration {
    pub years: i64,
    pub months: i64,
    pub weeks: i64,
    pub days: i64,
}

impl DateDuration {
    pub fn new(years: i64, months: i64, weeks: i64, days: i64) -> Self {
        DateDuration {
            years,
            months,
            weeks,
            days,
        }
    }
    fn negated(self) -> Self {
        DateDuration::new(-self.years, -self.months, -self.weeks, -self.days)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Unit {
    Year,
    Month,
    Week,
    Day,
}

pub fn iso_month_code(month: u8) -> [u8; 3] {
    [b'M', b'0' + (month / 10), b'0' + (month % 10)]
}

impl IsoDate {

    fn rd(self) -> RataDie {
        GREGORIAN.fixed_from(self.year as i64, self.month as i64, self.day as i64)
    }

    pub fn day_of_week(self) -> u16 {
        (((self.rd().0 - 1).rem_euclid(7)) + 1) as u16
    }

    pub fn day_of_year(self) -> u16 {
        (self.rd().0 - GREGORIAN.fixed_from(self.year as i64, 1, 1).0 + 1) as u16
    }

    pub fn days_in_week(self) -> u16 {
        7
    }
    pub fn months_in_year(self) -> u16 {
        12
    }
    pub fn in_leap_year(self) -> bool {
        GREGORIAN.is_leap_year(self.year as i64)
    }
    pub fn days_in_month_of(self) -> u16 {
        IsoDate::days_in_month(self.year, self.month) as u16
    }
    pub fn days_in_year_of(self) -> u16 {
        GREGORIAN.days_in_year(self.year as i64) as u16
    }
    pub fn month_code(self) -> [u8; 3] {
        iso_month_code(self.month)
    }

    fn iso_weeks_in_year(year: i64) -> u8 {
        let p =
            |y: i64| (y + y.div_euclid(4) - y.div_euclid(100) + y.div_euclid(400)).rem_euclid(7);
        if p(year) == 4 || p(year - 1) == 3 {
            53
        } else {
            52
        }
    }

    pub fn week_of_year(self) -> (i32, u8) {
        let ordinal = self.day_of_year() as i64;
        let weekday = self.day_of_week() as i64;
        let week = (ordinal - weekday + 10).div_euclid(7);
        if week < 1 {

            (
                self.year - 1,
                IsoDate::iso_weeks_in_year(self.year as i64 - 1),
            )
        } else if week > IsoDate::iso_weeks_in_year(self.year as i64) as i64 {

            (self.year + 1, 1)
        } else {
            (self.year, week as u8)
        }
    }

    fn surpasses(self, other: IsoDate, sign: i8) -> bool {
        (self.cmp(&other) as i8) * sign == 1
    }

    pub fn date_add(self, dur: &DateDuration, overflow: Overflow) -> CalResult<IsoDate> {
        let (iy, im) =
            balance_iso_year_month(self.year as i64 + dur.years, self.month as i64 + dur.months);
        let intermediate = IsoDate::new_with_overflow(iy as i32, im, self.day, overflow)?;
        let additional_days = dur.days + 7 * dur.weeks;
        let intermediate_days = intermediate.day as i64 + additional_days;
        IsoDate::try_balance(
            intermediate.year as i64,
            intermediate.month as i64,
            intermediate_days,
        )
    }

    pub fn date_until(self, other: IsoDate, largest_unit: Unit) -> DateDuration {
        let sign = -(self.cmp(&other) as i8);
        if sign == 0 {
            return DateDuration::default();
        }
        let mut years = 0i32;
        let mut months = 0i32;
        if largest_unit == Unit::Year || largest_unit == Unit::Month {
            let mut candidate_years = other.year - self.year;
            if candidate_years != 0 {
                candidate_years -= sign as i32;
            }
            while !IsoDate::new_unchecked(self.year + candidate_years, self.month, self.day)
                .surpasses(other, sign)
            {
                years = candidate_years;
                candidate_years += sign as i32;
            }
            let mut candidate_months = sign as i32;
            let mut inter = balance_iso_year_month(
                self.year as i64 + years as i64,
                self.month as i64 + candidate_months as i64,
            );
            while !IsoDate::new_unchecked(inter.0 as i32, inter.1, self.day).surpasses(other, sign)
            {
                months = candidate_months;
                candidate_months += sign as i32;
                inter = balance_iso_year_month(inter.0, inter.1 as i64 + sign as i64);
            }
            if largest_unit == Unit::Month {
                months += years * 12;
                years = 0;
            }
        }
        let inter = balance_iso_year_month(
            self.year as i64 + years as i64,
            self.month as i64 + months as i64,
        );
        let constrained = IsoDate::regulate(inter.0 as i32, inter.1, self.day, Overflow::Constrain)
            .expect("constrain never throws for a balanced ISO year/month");
        let days = (other.rd().0 - UNIX_EPOCH_RD) - (constrained.rd().0 - UNIX_EPOCH_RD);
        let (weeks, days) = if largest_unit == Unit::Week {
            (days / 7, days % 7)
        } else {
            (0, days)
        };

        DateDuration::new(years as i64, months as i64, weeks, days)
    }
}

pub const MS_PER_DAY: i64 = 24 * 60 * 60 * 1000;
pub const NS_PER_DAY: i128 = MS_PER_DAY as i128 * 1_000_000;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Hash)]
pub struct IsoTime {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub millisecond: u16,
    pub microsecond: u16,
    pub nanosecond: u16,
}

pub fn is_valid_time(hour: u8, minute: u8, second: u8, ms: u16, mis: u16, ns: u16) -> bool {
    (0..=23).contains(&hour)
        && (0..=59).contains(&minute)
        && (0..=59).contains(&second)
        && (0..=999).contains(&ms)
        && (0..=999).contains(&mis)
        && (0..=999).contains(&ns)
}

impl IsoTime {
    pub const fn new_unchecked(
        hour: u8,
        minute: u8,
        second: u8,
        ms: u16,
        mis: u16,
        ns: u16,
    ) -> Self {
        IsoTime {
            hour,
            minute,
            second,
            millisecond: ms,
            microsecond: mis,
            nanosecond: ns,
        }
    }

    pub const fn noon() -> Self {
        IsoTime::new_unchecked(12, 0, 0, 0, 0, 0)
    }

    pub fn new_with_overflow(
        hour: u8,
        minute: u8,
        second: u8,
        ms: u16,
        mis: u16,
        ns: u16,
        overflow: Overflow,
    ) -> CalResult<IsoTime> {
        match overflow {
            Overflow::Constrain => Ok(IsoTime::new_unchecked(
                hour.clamp(0, 23),
                minute.clamp(0, 59),
                second.clamp(0, 59),
                ms.clamp(0, 999),
                mis.clamp(0, 999),
                ns.clamp(0, 999),
            )),
            Overflow::Reject => {
                if !is_valid_time(hour, minute, second, ms, mis, ns) {
                    return Err(RangeError("time is not valid."));
                }
                Ok(IsoTime::new_unchecked(hour, minute, second, ms, mis, ns))
            }
        }
    }

    pub fn balance(
        hour: i64,
        minute: i64,
        second: i64,
        millisecond: i64,
        microsecond: i128,
        nanosecond: i128,
    ) -> (i64, IsoTime) {
        let microsecond = microsecond + nanosecond.div_euclid(1000);
        let nanosecond = nanosecond.rem_euclid(1000);
        let millisecond = millisecond + microsecond.div_euclid(1000) as i64;
        let microsecond = microsecond.rem_euclid(1000);
        let second = second + millisecond.div_euclid(1000);
        let millisecond = millisecond.rem_euclid(1000);
        let minute = minute + second.div_euclid(60);
        let second = second.rem_euclid(60);
        let hour = hour + minute.div_euclid(60);
        let minute = minute.rem_euclid(60);
        let days = hour.div_euclid(24);
        let hour = hour.rem_euclid(24);
        (
            days,
            IsoTime::new_unchecked(
                hour as u8,
                minute as u8,
                second as u8,
                millisecond as u16,
                microsecond as u16,
                nanosecond as u16,
            ),
        )
    }

    pub fn to_nanoseconds(self) -> i128 {
        let minutes = self.hour as i128 * 60 + self.minute as i128;
        let seconds = minutes * 60 + self.second as i128;
        let millis = seconds * 1000 + self.millisecond as i128;
        let micros = millis * 1000 + self.microsecond as i128;
        micros * 1000 + self.nanosecond as i128
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct IsoDateTime {
    pub date: IsoDate,
    pub time: IsoTime,
}

impl IsoDateTime {
    pub const fn new_unchecked(date: IsoDate, time: IsoTime) -> Self {
        IsoDateTime { date, time }
    }

    pub fn from_epoch_nanos(epoch_nanoseconds: i128) -> IsoDateTime {
        let remainder_nanos = epoch_nanoseconds.rem_euclid(1_000_000);
        let epoch_millis = (epoch_nanoseconds - remainder_nanos).div_euclid(1_000_000) as i64;

        let epoch_days = epoch_millis.div_euclid(MS_PER_DAY);
        let (y, m, d) = GREGORIAN.ymd_from_fixed(RataDie(epoch_days + UNIX_EPOCH_RD));
        let date = IsoDate::new_unchecked(y as i32, m as u8, d as u8);

        let hour = epoch_millis.div_euclid(3_600_000).rem_euclid(24);
        let minute = epoch_millis.div_euclid(60_000).rem_euclid(60);
        let second = epoch_millis.div_euclid(1000).rem_euclid(60);
        let millis = epoch_millis.rem_euclid(1000);
        let micros = remainder_nanos.div_euclid(1_000) as i64;
        let nanos = remainder_nanos.rem_euclid(1000) as i64;

        let (days, time) =
            IsoTime::balance(hour, minute, second, millis, micros.into(), nanos.into());

        let date = if days != 0 {
            IsoDate::balance(date.year as i64, date.month as i64, date.day as i64 + days)
        } else {
            date
        };
        IsoDateTime::new_unchecked(date, time)
    }

    pub fn to_epoch_nanos(self) -> i128 {
        self.date.to_epoch_days() as i128 * NS_PER_DAY + self.time.to_nanoseconds()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NonIsoCalendarStatus {
    DerivableInCruft,
    Icu4xDelegated,
    DeferredPendingArcT2,
    OutOfScopeValueComputation,
}

impl NonIsoCalendarStatus {
    pub const fn flag(self) -> &'static str {
        match self {
            NonIsoCalendarStatus::DerivableInCruft => "DERIVABLE-IN-CRUFT",
            NonIsoCalendarStatus::Icu4xDelegated => "ICU4X-DELEGATED",
            NonIsoCalendarStatus::DeferredPendingArcT2 => "DEFERRED-PENDING-ARC-T.2",
            NonIsoCalendarStatus::OutOfScopeValueComputation => "OUT-OF-SCOPE-VALUE-COMPUTATION",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NonIsoCalendarCluster {
    HebrewKeviyah,
    AstronomicalObservational,
    EraBased,
    JulianFamilySibling,
    IndianNational,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NonIsoCalendarDate {
    pub calendar_id: &'static str,
    pub era: Option<&'static str>,
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

pub trait RataDieCalendarBridge {
    fn from_rata_die(&self, rd: RataDie) -> CalResult<NonIsoCalendarDate>;
    fn to_rata_die(&self, date: NonIsoCalendarDate) -> CalResult<RataDie>;
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NonIsoCalendarDescriptor {
    pub canonical_id: &'static str,
    pub aliases: &'static [&'static str],
    pub cluster: NonIsoCalendarCluster,
    pub status: NonIsoCalendarStatus,
    pub derived_module: Option<&'static str>,
    pub icu4x_type: Option<&'static str>,
    pub note: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SkeletonNonIsoCalendarProvider {
    pub descriptor: &'static NonIsoCalendarDescriptor,
}

impl RataDieCalendarBridge for SkeletonNonIsoCalendarProvider {
    fn from_rata_die(&self, _rd: RataDie) -> CalResult<NonIsoCalendarDate> {
        Err(RangeError(
            "non-ISO calendar value computation is out of scope for this skeleton.",
        ))
    }

    fn to_rata_die(&self, _date: NonIsoCalendarDate) -> CalResult<RataDie> {
        Err(RangeError(
            "non-ISO calendar value computation is out of scope for this skeleton.",
        ))
    }
}

pub const NON_ISO_CALENDAR_DESCRIPTORS: &[NonIsoCalendarDescriptor] = &[
    NonIsoCalendarDescriptor {
        canonical_id: "hebrew",
        aliases: &[],
        cluster: NonIsoCalendarCluster::HebrewKeviyah,
        status: NonIsoCalendarStatus::DerivableInCruft,
        derived_module: Some("crate::hebrew"),
        icu4x_type: None,
        note: "arithmetic keviyah derived natively via paschalion-verified molad substrate",
    },
    NonIsoCalendarDescriptor {
        canonical_id: "chinese",
        aliases: &[],
        cluster: NonIsoCalendarCluster::AstronomicalObservational,
        status: NonIsoCalendarStatus::Icu4xDelegated,
        derived_module: None,
        icu4x_type: Some("icu_calendar::Chinese"),
        note: "astronomical new-moon and solar-term substrate",
    },
    NonIsoCalendarDescriptor {
        canonical_id: "dangi",
        aliases: &["korean-dangi"],
        cluster: NonIsoCalendarCluster::AstronomicalObservational,
        status: NonIsoCalendarStatus::Icu4xDelegated,
        derived_module: None,
        icu4x_type: Some("icu_calendar::Dangi"),
        note: "Korean-Dangi astronomical variant",
    },
    NonIsoCalendarDescriptor {
        canonical_id: "islamic-umalqura",
        aliases: &["islamic", "observational-hijri"],
        cluster: NonIsoCalendarCluster::AstronomicalObservational,
        status: NonIsoCalendarStatus::Icu4xDelegated,
        derived_module: None,
        icu4x_type: Some("icu_calendar::IslamicUmmAlQura"),
        note: "observational Hijri table/source substrate",
    },
    NonIsoCalendarDescriptor {
        canonical_id: "japanese",
        aliases: &["japanese-era"],
        cluster: NonIsoCalendarCluster::EraBased,
        status: NonIsoCalendarStatus::OutOfScopeValueComputation,
        derived_module: None,
        icu4x_type: Some("icu_calendar::Japanese"),
        note: "era table and calendar-protocol era surface carve-out",
    },
    NonIsoCalendarDescriptor {
        canonical_id: "buddhist",
        aliases: &[],
        cluster: NonIsoCalendarCluster::EraBased,
        status: NonIsoCalendarStatus::DerivableInCruft,
        derived_module: None,
        icu4x_type: Some("icu_calendar::Buddhist"),
        note: "Gregorian-derived year offset admitted by ARC-T.2 sibling-table parameterization",
    },
    NonIsoCalendarDescriptor {
        canonical_id: "coptic",
        aliases: &[],
        cluster: NonIsoCalendarCluster::JulianFamilySibling,
        status: NonIsoCalendarStatus::DerivableInCruft,
        derived_module: None,
        icu4x_type: Some("icu_calendar::Coptic"),
        note: "Julian-family arithmetic sibling admitted by ARC-T.2 epoch/month-table extension",
    },
    NonIsoCalendarDescriptor {
        canonical_id: "ethiopic",
        aliases: &["ethiopian", "ethioaa"],
        cluster: NonIsoCalendarCluster::JulianFamilySibling,
        status: NonIsoCalendarStatus::DerivableInCruft,
        derived_module: None,
        icu4x_type: Some("icu_calendar::Ethiopian"),
        note: "Julian-family arithmetic sibling admitted by ARC-T.2 epoch/month-table extension",
    },
    NonIsoCalendarDescriptor {
        canonical_id: "persian",
        aliases: &[],
        cluster: NonIsoCalendarCluster::JulianFamilySibling,
        status: NonIsoCalendarStatus::DerivableInCruft,
        derived_module: None,
        icu4x_type: Some("icu_calendar::Persian"),
        note: "arithmetic Persian candidate, derivable under the future sibling-table pass",
    },
    NonIsoCalendarDescriptor {
        canonical_id: "roc",
        aliases: &[],
        cluster: NonIsoCalendarCluster::JulianFamilySibling,
        status: NonIsoCalendarStatus::DerivableInCruft,
        derived_module: None,
        icu4x_type: Some("icu_calendar::Roc"),
        note:
            "Gregorian-derived era/year offset admitted by ARC-T.2 sibling-table parameterization",
    },
    NonIsoCalendarDescriptor {
        canonical_id: "indian",
        aliases: &["indian-national"],
        cluster: NonIsoCalendarCluster::IndianNational,
        status: NonIsoCalendarStatus::Icu4xDelegated,
        derived_module: None,
        icu4x_type: Some("icu_calendar::Indian"),
        note:
            "Saka/Indian national calendar delegated until a dedicated arithmetic derivation rung",
    },
];

pub fn non_iso_calendar_descriptors() -> &'static [NonIsoCalendarDescriptor] {
    NON_ISO_CALENDAR_DESCRIPTORS
}

pub fn lookup_non_iso_calendar(id: &str) -> Option<&'static NonIsoCalendarDescriptor> {
    NON_ISO_CALENDAR_DESCRIPTORS
        .iter()
        .find(|desc| desc.canonical_id == id || desc.aliases.contains(&id))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TabularIslamic {

    pub epoch_rd: i64,
}

pub const ISLAMIC_CIVIL: TabularIslamic = TabularIslamic { epoch_rd: 227015 };

pub const ISLAMIC_TBLA: TabularIslamic = TabularIslamic { epoch_rd: 227014 };

const ISLAMIC_MEAN_YEAR_X30: i64 = 354 * 30 + 11;

impl TabularIslamic {

    pub fn fixed_from(self, year: i64, month: i64, day: i64) -> RataDie {
        RataDie(
            self.epoch_rd - 1
                + (year - 1) * 354
                + (3 + year * 11).div_euclid(30)
                + 29 * (month - 1)
                + month.div_euclid(2)
                + day,
        )
    }

    pub fn year_from_fixed(self, rd: RataDie) -> i64 {
        (30 * (rd.0 - self.epoch_rd) + 10646).div_euclid(ISLAMIC_MEAN_YEAR_X30)
    }

    pub fn ymd_from_fixed(self, rd: RataDie) -> (i64, i64, i64) {
        let year = self.year_from_fixed(rd);
        let prior_days = rd.0 - self.fixed_from(year, 1, 1).0;
        let month = ((prior_days * 11) + 330) / 325;
        let day = rd.0 - self.fixed_from(year, month, 1).0 + 1;
        (year, month, day)
    }

    pub fn year_length(self, year: i64) -> i64 {
        self.fixed_from(year + 1, 1, 1).0 - self.fixed_from(year, 1, 1).0
    }

    pub fn is_leap_year(self, year: i64) -> bool {
        self.year_length(year) == 355
    }

    pub fn days_in_month(self, year: i64, month: i64) -> i64 {
        if month % 2 == 1 {
            30
        } else if month == 12 && self.is_leap_year(year) {
            30
        } else {
            29
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratadie_arithmetic() {
        let a = RataDie::EPOCH;
        assert_eq!(a.0, 1);
        assert_eq!(a.add(10), RataDie(11));
        assert_eq!(RataDie(20).since(RataDie(5)), 15);
        assert_eq!(RataDie(5).until(RataDie(20)), 15);
    }

    #[test]
    fn gregorian_epoch_anchor() {

        assert_eq!(GREGORIAN.fixed_from(1, 1, 1), RataDie(1));
        assert_eq!(GREGORIAN.fixed_from(1, 12, 31), RataDie(365));
        assert_eq!(GREGORIAN.fixed_from(2, 1, 1), RataDie(366));
    }

    #[test]
    fn gregorian_known_dates() {

        assert_eq!(GREGORIAN.fixed_from(1945, 11, 12), RataDie(710347));
        assert_eq!(GREGORIAN.fixed_from(2000, 1, 1), RataDie(730120));
        assert_eq!(GREGORIAN.ymd_from_fixed(RataDie(710347)), (1945, 11, 12));
        assert_eq!(GREGORIAN.ymd_from_fixed(RataDie(730120)), (2000, 1, 1));
    }

    #[test]
    fn julian_epoch_anchor() {

        assert_eq!(JULIAN.fixed_from(1, 1, 1), RataDie(-1));
    }

    #[test]
    fn leap_rules_diverge_at_centuries() {

        assert!(JULIAN.is_leap_year(1900));
        assert!(!GREGORIAN.is_leap_year(1900));

        assert!(JULIAN.is_leap_year(2000));
        assert!(GREGORIAN.is_leap_year(2000));

        assert!(!JULIAN.is_leap_year(2001));
        assert!(!GREGORIAN.is_leap_year(2001));
    }

    #[test]
    fn days_before_month_closed_form() {

        let expect = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
        for (i, e) in expect.iter().enumerate() {
            assert_eq!(
                GREGORIAN.days_before_month(2001, (i + 1) as i64),
                *e,
                "month {}",
                i + 1
            );
        }

        assert_eq!(GREGORIAN.days_before_month(2000, 3), 60);
        assert_eq!(GREGORIAN.days_before_month(2000, 12), 335);
    }

    #[test]
    fn round_trip_dense() {

        for fam in [JULIAN, GREGORIAN] {
            for rd in (-400_000..400_000).step_by(97) {
                let r = RataDie(rd);
                let (y, m, d) = fam.ymd_from_fixed(r);
                assert_eq!(fam.fixed_from(y, m, d), r, "fam {:?} rd {}", fam, rd);
                assert!((1..=12).contains(&m));
                assert!(d >= 1 && d <= fam.days_in_month(y, m));
            }
        }
    }

    #[test]
    fn islamic_tabular_meet() {

        assert_eq!(
            JULIAN.fixed_from(622, 7, 16),
            RataDie(ISLAMIC_CIVIL.epoch_rd)
        );
        assert_eq!(
            JULIAN.fixed_from(622, 7, 15),
            RataDie(ISLAMIC_TBLA.epoch_rd)
        );

        assert_eq!(ISLAMIC_CIVIL.fixed_from(1, 1, 1), RataDie(227015));
        assert_eq!(GREGORIAN.ymd_from_fixed(RataDie(227015)), (622, 7, 19));

        for rd in (227015..227015 + 200_000).step_by(53) {
            let r = RataDie(rd);
            let (y, m, d) = ISLAMIC_CIVIL.ymd_from_fixed(r);
            assert_eq!(ISLAMIC_CIVIL.fixed_from(y, m, d), r, "civil rd {}", rd);
            assert!((1..=12).contains(&m));
            assert!(d >= 1 && d <= ISLAMIC_CIVIL.days_in_month(y, m));
        }

        let leaps = (1..=30).filter(|&y| ISLAMIC_CIVIL.is_leap_year(y)).count();
        assert_eq!(leaps, 11, "11 leap years per 30-year cycle");

        assert_eq!(ISLAMIC_CIVIL.days_in_month(1, 1), 30);
        assert_eq!(ISLAMIC_CIVIL.days_in_month(1, 2), 29);
        assert_eq!(ISLAMIC_CIVIL.year_length(2), 355);
        assert_eq!(ISLAMIC_CIVIL.days_in_month(2, 12), 30);
        assert_eq!(ISLAMIC_CIVIL.days_in_month(1, 12), 29);
    }

    #[test]
    fn days_in_month_and_year() {
        assert_eq!(GREGORIAN.days_in_month(2001, 2), 28);
        assert_eq!(GREGORIAN.days_in_month(2000, 2), 29);
        assert_eq!(JULIAN.days_in_month(1900, 2), 29);
        assert_eq!(GREGORIAN.days_in_month(1900, 2), 28);
        assert_eq!(GREGORIAN.days_in_year(2000), 366);
        assert_eq!(GREGORIAN.days_in_year(2001), 365);
    }

    #[test]
    fn sibling_month_tables_and_year_offsets() {
        assert_eq!(COPTIC.days_before_month(1, 2), 30);
        assert_eq!(COPTIC.days_before_month(1, 13), 360);
        assert_eq!(COPTIC.days_in_month(1, 13), 5);
        assert_eq!(COPTIC.days_in_month(3, 13), 6);
        assert_eq!(ETHIOPIC.days_in_month(3, 13), 6);
        assert_eq!(
            BUDDHIST.fixed_from(2567, 1, 1),
            GREGORIAN.fixed_from(2024, 1, 1)
        );
        assert_eq!(
            BUDDHIST.ymd_from_fixed(GREGORIAN.fixed_from(2024, 1, 1)),
            (2567, 1, 1)
        );
        assert_eq!(ROC.fixed_from(113, 1, 1), GREGORIAN.fixed_from(2024, 1, 1));
        assert_eq!(
            ROC.ymd_from_fixed(GREGORIAN.fixed_from(2024, 1, 1)),
            (113, 1, 1)
        );
    }

    #[test]
    fn sibling_round_trip_dense() {
        for fam in [BUDDHIST, COPTIC, ETHIOPIC, ROC] {
            for rd in (-400_000..400_000).step_by(97) {
                let r = RataDie(rd);
                let (y, m, d) = fam.ymd_from_fixed(r);
                assert_eq!(fam.fixed_from(y, m, d), r, "fam {:?} rd {}", fam, rd);
                assert!(m >= 1);
                assert!(d >= 1 && d <= fam.days_in_month(y, m));
            }
        }
    }

    #[test]
    fn unix_epoch_rd_anchor() {

        assert_eq!(GREGORIAN.fixed_from(1970, 1, 1), RataDie(UNIX_EPOCH_RD));
        assert_eq!(IsoDate::new_unchecked(1970, 1, 1).to_epoch_days(), 0);
        assert_eq!(IsoDate::new_unchecked(1970, 1, 2).to_epoch_days(), 1);
        assert_eq!(IsoDate::new_unchecked(1969, 12, 31).to_epoch_days(), -1);
    }

    #[test]
    fn is_valid_date_field_ranges() {
        assert!(IsoDate::is_valid_date(2024, 2, 29));
        assert!(!IsoDate::is_valid_date(2023, 2, 29));
        assert!(!IsoDate::is_valid_date(2024, 0, 1));
        assert!(!IsoDate::is_valid_date(2024, 13, 1));
        assert!(!IsoDate::is_valid_date(2024, 4, 31));
        assert!(IsoDate::is_valid_date(2024, 4, 30));
        assert!(!IsoDate::is_valid_date(2024, 1, 0));
    }

    #[test]
    fn regulate_constrain_clamps() {

        assert_eq!(
            IsoDate::regulate(2024, 13, 50, Overflow::Constrain).unwrap(),
            IsoDate::new_unchecked(2024, 12, 31)
        );

        assert_eq!(
            IsoDate::regulate(2024, 2, 31, Overflow::Constrain).unwrap(),
            IsoDate::new_unchecked(2024, 2, 29)
        );
        assert_eq!(
            IsoDate::regulate(2023, 2, 31, Overflow::Constrain).unwrap(),
            IsoDate::new_unchecked(2023, 2, 28)
        );
    }

    #[test]
    fn regulate_reject_throws() {
        assert!(IsoDate::regulate(2023, 2, 29, Overflow::Reject).is_err());
        assert_eq!(
            IsoDate::regulate(2024, 2, 29, Overflow::Reject).unwrap(),
            IsoDate::new_unchecked(2024, 2, 29)
        );
    }

    #[test]
    fn within_limits_gate() {

        assert!(IsoDate::new_unchecked(2024, 1, 1).within_limits());
        assert!(IsoDate::new_unchecked(1, 1, 1).within_limits());

        assert!(IsoDate::new_unchecked(275760, 9, 13).within_limits());
        assert!(!IsoDate::new_unchecked(275760, 9, 30).within_limits());
        assert!(IsoDate::new_unchecked(-271821, 4, 19).within_limits());
        assert!(!IsoDate::new_unchecked(-271821, 4, 1).within_limits());

        assert!(IsoDate::new_with_overflow(280000, 1, 1, Overflow::Reject).is_err());
    }

    #[test]
    fn year_month_within_limits_boundaries() {
        assert!(year_month_within_limits(2024, 6));
        assert!(!year_month_within_limits(-271822, 6));
        assert!(!year_month_within_limits(275761, 6));
        assert!(!year_month_within_limits(-271821, 3));
        assert!(year_month_within_limits(-271821, 4));
        assert!(!year_month_within_limits(275760, 10));
        assert!(year_month_within_limits(275760, 9));
    }

    #[test]
    fn balance_year_month_folds_overflow() {
        assert_eq!(balance_iso_year_month(2024, 13), (2025, 1));
        assert_eq!(balance_iso_year_month(2024, 0), (2023, 12));
        assert_eq!(balance_iso_year_month(2024, 25), (2026, 1));
        assert_eq!(balance_iso_year_month(2024, -1), (2023, 11));
    }

    #[test]
    fn balance_date_normalizes() {

        assert_eq!(
            IsoDate::balance(2024, 1, 32),
            IsoDate::new_unchecked(2024, 2, 1)
        );
        assert_eq!(
            IsoDate::balance(2023, 12, 32),
            IsoDate::new_unchecked(2024, 1, 1)
        );

        assert_eq!(
            IsoDate::balance(2024, 3, 0),
            IsoDate::new_unchecked(2024, 2, 29)
        );

        assert!(IsoDate::try_balance(2024, 1, 200_000_000).is_err());
        assert!(IsoDate::try_balance(2024, 1, 1).is_ok());
    }

    #[test]
    fn getters_iso_anchors() {

        let d = IsoDate::new_unchecked(2000, 1, 1);
        assert_eq!(d.day_of_week(), 6);
        assert_eq!(d.day_of_year(), 1);
        assert_eq!(d.month_code(), *b"M01");
        assert!(d.in_leap_year());
        assert_eq!(d.days_in_year_of(), 366);
        assert_eq!(d.days_in_month_of(), 31);
        assert_eq!(d.months_in_year(), 12);
        assert_eq!(d.days_in_week(), 7);

        let e = IsoDate::new_unchecked(2000, 12, 31);
        assert_eq!(e.day_of_year(), 366);
        assert_eq!(e.day_of_week(), 7);
        assert_eq!(e.month_code(), *b"M12");
    }

    #[test]
    fn iso_week_of_year_anchors() {

        assert_eq!(
            IsoDate::new_unchecked(2005, 1, 1).week_of_year(),
            (2004, 53)
        );

        assert_eq!(IsoDate::new_unchecked(2005, 1, 3).week_of_year(), (2005, 1));

        assert_eq!(IsoDate::new_unchecked(2007, 1, 1).week_of_year(), (2007, 1));

        assert_eq!(
            IsoDate::new_unchecked(2008, 12, 29).week_of_year(),
            (2009, 1)
        );

        assert_eq!(
            IsoDate::new_unchecked(2010, 1, 3).week_of_year(),
            (2009, 53)
        );

        assert_eq!(
            IsoDate::new_unchecked(2024, 6, 3).week_of_year(),
            (2024, 23)
        );
    }

    #[test]
    fn date_add_basic() {
        let d = IsoDate::new_unchecked(2024, 1, 31);

        assert_eq!(
            d.date_add(&DateDuration::new(0, 1, 0, 0), Overflow::Constrain)
                .unwrap(),
            IsoDate::new_unchecked(2024, 2, 29)
        );

        assert!(d
            .date_add(&DateDuration::new(0, 1, 0, 0), Overflow::Reject)
            .is_err());

        let f = IsoDate::new_unchecked(2024, 2, 29);
        assert_eq!(
            f.date_add(&DateDuration::new(1, 0, 1, 3), Overflow::Constrain)
                .unwrap(),
            IsoDate::new_unchecked(2025, 3, 10)
        );
    }

    #[test]
    fn date_until_roundtrips_with_add() {

        let a = IsoDate::new_unchecked(2020, 1, 15);
        let b = IsoDate::new_unchecked(2024, 6, 3);
        for lu in [Unit::Year, Unit::Month, Unit::Week, Unit::Day] {
            let dur = a.date_until(b, lu);
            assert_eq!(
                a.date_add(&dur, Overflow::Constrain).unwrap(),
                b,
                "lu {:?} dur {:?}",
                lu,
                dur
            );
        }

        for lu in [Unit::Year, Unit::Month, Unit::Week, Unit::Day] {
            let dur = b.date_until(a, lu);
            assert_eq!(
                b.date_add(&dur, Overflow::Constrain).unwrap(),
                a,
                "neg lu {:?} dur {:?}",
                lu,
                dur
            );
        }
    }

    #[test]
    fn date_until_known_values() {

        let a = IsoDate::new_unchecked(2020, 1, 15);
        let b = IsoDate::new_unchecked(2024, 6, 3);
        assert_eq!(a.date_until(b, Unit::Year), DateDuration::new(4, 4, 0, 19));

        assert_eq!(
            a.date_until(b, Unit::Month),
            DateDuration::new(0, 52, 0, 19)
        );

        assert_eq!(a.date_until(a, Unit::Year), DateDuration::default());
    }

    #[test]
    fn date_until_allows_internal_edge_intermediates() {
        let receiver = IsoDate::new_unchecked(2000, 5, 2);
        for edge in [
            IsoDate::new_unchecked(-271821, 4, 19),
            IsoDate::new_unchecked(275760, 9, 13),
        ] {
            for lu in [Unit::Year, Unit::Month, Unit::Week, Unit::Day] {
                let _ = edge.date_until(receiver, lu);
                let _ = receiver.date_until(edge, lu);
            }
        }

        let year_month_receiver = IsoDate::new_unchecked(1970, 1, 1);
        for edge in [
            IsoDate::new_unchecked(-271821, 5, 1),
            IsoDate::new_unchecked(275760, 9, 1),
        ] {
            let _ = edge.date_until(year_month_receiver, Unit::Year);
            let _ = edge.date_until(year_month_receiver, Unit::Month);
            let _ = year_month_receiver.date_until(edge, Unit::Year);
            let _ = year_month_receiver.date_until(edge, Unit::Month);
        }
    }

    #[test]
    fn is_valid_time_ranges() {
        assert!(is_valid_time(23, 59, 59, 999, 999, 999));
        assert!(!is_valid_time(24, 0, 0, 0, 0, 0));
        assert!(!is_valid_time(0, 60, 0, 0, 0, 0));
        assert!(!is_valid_time(0, 0, 0, 1000, 0, 0));
    }

    #[test]
    fn time_balance_carry_cascade() {

        assert_eq!(
            IsoTime::balance(0, 0, 0, 1000, 0, 0),
            (0, IsoTime::new_unchecked(0, 0, 1, 0, 0, 0))
        );

        assert_eq!(
            IsoTime::balance(23, 59, 59, 999, 999, 1000),
            (1, IsoTime::new_unchecked(0, 0, 0, 0, 0, 0))
        );

        assert_eq!(
            IsoTime::balance(0, 0, 0, 0, 0, -1),
            (-1, IsoTime::new_unchecked(23, 59, 59, 999, 999, 999))
        );
    }

    #[test]
    fn epoch_nanos_anchors() {

        let dt0 = IsoDateTime::from_epoch_nanos(0);
        assert_eq!(dt0.date, IsoDate::new_unchecked(1970, 1, 1));
        assert_eq!(dt0.time, IsoTime::default());
        assert_eq!(dt0.to_epoch_nanos(), 0);

        let ns = 946_684_800i128 * 1_000_000_000;
        let dt = IsoDateTime::from_epoch_nanos(ns);
        assert_eq!(dt.date, IsoDate::new_unchecked(2000, 1, 1));
        assert_eq!(dt.time, IsoTime::default());

        let ns2 = ns + 123_456_789;
        let dt2 = IsoDateTime::from_epoch_nanos(ns2);
        assert_eq!(dt2.time, IsoTime::new_unchecked(0, 0, 0, 123, 456, 789));
        assert_eq!(dt2.to_epoch_nanos(), ns2);

        let neg = -1i128;
        let dtn = IsoDateTime::from_epoch_nanos(neg);
        assert_eq!(dtn.date, IsoDate::new_unchecked(1969, 12, 31));
        assert_eq!(dtn.time, IsoTime::new_unchecked(23, 59, 59, 999, 999, 999));
        assert_eq!(dtn.to_epoch_nanos(), neg);
    }

    #[test]
    fn epoch_nanos_round_trip_dense() {

        let step = 97_003_001_111i128;
        let mut ns = -500i128 * NS_PER_DAY;
        let end = 500i128 * NS_PER_DAY;
        while ns < end {
            let dt = IsoDateTime::from_epoch_nanos(ns);
            assert!(is_valid_time(
                dt.time.hour,
                dt.time.minute,
                dt.time.second,
                dt.time.millisecond,
                dt.time.microsecond,
                dt.time.nanosecond
            ));
            assert_eq!(dt.to_epoch_nanos(), ns, "round-trip ns {}", ns);
            ns += step;
        }
    }

    #[test]
    fn time_new_with_overflow() {
        assert_eq!(
            IsoTime::new_with_overflow(25, 70, 0, 0, 0, 0, Overflow::Constrain).unwrap(),
            IsoTime::new_unchecked(23, 59, 0, 0, 0, 0)
        );
        assert!(IsoTime::new_with_overflow(25, 0, 0, 0, 0, 0, Overflow::Reject).is_err());
        assert!(IsoTime::new_with_overflow(12, 30, 0, 0, 0, 0, Overflow::Reject).is_ok());
    }

    #[test]
    fn non_iso_registry_covers_wave_ad_ids() {
        let ids = [
            "hebrew",
            "chinese",
            "dangi",
            "observational-hijri",
            "japanese",
            "buddhist",
            "coptic",
            "ethiopic",
            "persian",
            "roc",
            "indian",
        ];
        for id in ids {
            let desc = lookup_non_iso_calendar(id)
                .unwrap_or_else(|| panic!("missing non-ISO calendar id {id}"));
            assert!(desc.icu4x_type.is_some() || desc.derived_module.is_some());
        }
        assert_eq!(non_iso_calendar_descriptors().len(), 11);
    }

    #[test]
    fn non_iso_registry_marks_cluster_statuses() {
        let hebrew = lookup_non_iso_calendar("hebrew").unwrap();
        assert_eq!(hebrew.status.flag(), "DERIVABLE-IN-CRUFT");
        assert_eq!(hebrew.derived_module, Some("crate::hebrew"));
        assert_eq!(hebrew.icu4x_type, None);
        assert_eq!(
            lookup_non_iso_calendar("persian").unwrap().status.flag(),
            "DERIVABLE-IN-CRUFT"
        );
        assert_eq!(
            lookup_non_iso_calendar("chinese").unwrap().status.flag(),
            "ICU4X-DELEGATED"
        );
        assert_eq!(
            lookup_non_iso_calendar("coptic").unwrap().status.flag(),
            "DERIVABLE-IN-CRUFT"
        );
        assert_eq!(
            lookup_non_iso_calendar("japanese-era")
                .unwrap()
                .status
                .flag(),
            "OUT-OF-SCOPE-VALUE-COMPUTATION"
        );
    }

    #[test]
    fn non_iso_skeleton_refuses_value_computation() {
        let descriptor = lookup_non_iso_calendar("chinese").unwrap();
        let provider = SkeletonNonIsoCalendarProvider { descriptor };
        assert!(provider.from_rata_die(RataDie::EPOCH).is_err());
        assert!(provider
            .to_rata_die(NonIsoCalendarDate {
                calendar_id: "chinese",
                era: None,
                year: 1,
                month: 1,
                day: 1,
            })
            .is_err());
    }
}
