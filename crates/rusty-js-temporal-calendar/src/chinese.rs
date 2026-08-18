
use crate::astronomy::{
    estimate_prior_solar_longitude, julian_centuries, new_moon_at_or_after, new_moon_before,
    solar_longitude, Moment, MEAN_SYNODIC_MONTH, WINTER,
};
use crate::{RataDie, GREGORIAN};

#[derive(Clone, Copy)]
pub struct ChineseBased {

    pub utc_offset: fn(i64) -> f64,
    pub known_leap_code: fn(i32) -> Option<u8>,

    pub id: usize,
}

thread_local! {

    static YEAR_BOUNDS_MEMO: core::cell::Cell<[Option<(i64, i64)>; 2]> =
        const { core::cell::Cell::new([None, None]) };

    static MONTH_MEMO: core::cell::Cell<[Option<(i64, i64)>; 2]> =
        const { core::cell::Cell::new([None, None]) };
}

fn g(year: i64, month: i64, day: i64) -> i64 {
    GREGORIAN.fixed_from(year, month, day).0
}

fn chinese_utc_offset(rd: i64) -> f64 {

    if rd < g(1929, 1, 1) {
        1397.0 / 180.0 / 24.0
    } else {
        8.0 / 24.0
    }
}

fn dangi_utc_offset(rd: i64) -> f64 {

    if rd < g(1908, 4, 1) {
        3809.0 / 450.0 / 24.0
    } else if rd < g(1912, 1, 1) {
        8.5 / 24.0
    } else if rd < g(1954, 3, 21) {
        9.0 / 24.0
    } else if rd < g(1961, 8, 10) {
        8.5 / 24.0
    } else {
        9.0 / 24.0
    }
}

pub fn chinese() -> ChineseBased {
    ChineseBased {
        utc_offset: chinese_utc_offset,
        known_leap_code: chinese_known_temporal_leap_code,
        id: 0,
    }
}

pub fn dangi() -> ChineseBased {
    ChineseBased {
        utc_offset: dangi_utc_offset,
        known_leap_code: dangi_known_temporal_leap_code,
        id: 1,
    }
}

impl ChineseBased {

    fn midnight(&self, moment: Moment) -> Moment {
        moment - (self.utc_offset)(moment.floor() as i64)
    }

    fn new_moon_on_or_after(&self, moment: Moment) -> i64 {
        let nm = new_moon_at_or_after(self.midnight(moment));
        (nm + (self.utc_offset)(nm.floor() as i64)).floor() as i64
    }

    fn new_moon_before(&self, moment: Moment) -> i64 {
        let nm = new_moon_before(self.midnight(moment));
        (nm + (self.utc_offset)(nm.floor() as i64)).floor() as i64
    }

    fn month_start_on_or_before(&self, rd: i64) -> i64 {
        if let Some((s, e)) = MONTH_MEMO.with(|m| m.get()[self.id]) {
            if s <= rd && rd < e {
                return s;
            }
        }
        let s = self.new_moon_before((rd + 1) as f64);
        let e = self.new_moon_on_or_after((s + 1) as f64);
        MONTH_MEMO.with(|m| {
            let mut slots = m.get();
            slots[self.id] = Some((s, e));
            m.set(slots);
        });
        s
    }

    fn major_solar_term(&self, rd: i64) -> i64 {
        let universal = rd as f64 - (self.utc_offset)(rd);
        let s = solar_longitude(julian_centuries(universal)).floor() as i64;
        (2 + s.div_euclid(30) - 1).rem_euclid(12) + 1
    }

    fn winter_solstice_on_or_before(&self, rd: i64) -> i64 {
        let approx = estimate_prior_solar_longitude(WINTER, self.midnight((rd + 1) as f64));
        let mut day = (approx - 1.0).floor();
        while WINTER >= solar_longitude(julian_centuries(self.midnight(day + 1.0))) {
            day += 1.0;
        }
        self.bind_winter_solstice(day as i64)
    }

    fn bind_winter_solstice(&self, solstice: i64) -> i64 {
        let (y, m, d) = GREGORIAN.ymd_from_fixed(RataDie(solstice));
        if m < 12 || d < 20 {
            g(y, 12, 20)
        } else if d > 23 {
            g(y, 12, 23)
        } else {
            solstice
        }
    }

    fn new_year_in_sui(&self, prior_solstice: i64) -> (i64, i64) {
        let prior_solstice = self.bind_winter_solstice(prior_solstice);
        let following_solstice =
            self.bind_winter_solstice(self.winter_solstice_on_or_before(prior_solstice + 370));
        let m12 = self.new_moon_on_or_after((prior_solstice + 1) as f64);
        let m13 = self.new_moon_on_or_after((m12 + 1) as f64);
        let m14 = self.new_moon_on_or_after((m13 + 1) as f64);
        let next_m11 = self.new_moon_before((following_solstice + 1) as f64);
        let leap_solar_year = ((next_m11 - m12) as f64 / MEAN_SYNODIC_MONTH).round() as i64 == 12;
        let term_a = self.major_solar_term(m12);
        let term_b = self.major_solar_term(m13);
        let term_c = self.major_solar_term(m14);
        if leap_solar_year && (term_a == term_b || term_b == term_c) {
            (m14, following_solstice)
        } else {
            (m13, following_solstice)
        }
    }

    fn new_year_on_or_before(&self, rd: i64, prior_solstice: i64) -> (i64, i64) {
        let ny = self.new_year_in_sui(prior_solstice);
        if rd >= ny.0 {
            ny
        } else {
            let prior = self.winter_solstice_on_or_before(rd - 180);
            self.new_year_in_sui(prior)
        }
    }

    fn year_bounds(&self, rd: i64) -> (i64, i64) {
        if let Some((ny, nny)) = YEAR_BOUNDS_MEMO.with(|m| m.get()[self.id]) {
            if ny <= rd && rd < nny {
                return (ny, nny);
            }
        }
        let prev_solstice = self.winter_solstice_on_or_before(rd);
        let (new_year, next_solstice) = self.new_year_on_or_before(rd, prev_solstice);
        let next_new_year = self.new_year_on_or_before(new_year + 400, next_solstice).0;
        YEAR_BOUNDS_MEMO.with(|m| {
            let mut slots = m.get();
            slots[self.id] = Some((new_year, next_new_year));
            m.set(slots);
        });
        (new_year, next_new_year)
    }

    pub fn new_year_on_or_before_fixed(&self, rd: i64) -> i64 {
        let prev_solstice = self.winter_solstice_on_or_before(rd);
        self.new_year_on_or_before(rd, prev_solstice).0
    }

    fn leap_month_from_new_year(&self, new_year: i64) -> u8 {
        let mut cur = new_year;
        let mut result: u8 = 1;
        let mut term = self.major_solar_term(cur);
        loop {
            let next = self.new_moon_on_or_after((cur + 1) as f64);
            let next_term = self.major_solar_term(next);
            if result >= 14 || term == next_term {
                break;
            }
            cur = next;
            term = next_term;
            result += 1;
        }
        result
    }

    pub fn from_fixed(&self, rd: i64) -> (i32, u8, bool, u8) {
        let (new_year, next_new_year) = self.year_bounds(rd);
        let is_leap_year = (next_new_year - new_year) > 365;
        let (year, _, _) = GREGORIAN.ymd_from_fixed(RataDie(new_year));
        let year = year as i32;
        let new_moon = self.month_start_on_or_before(rd);
        let month_index = ((new_moon - new_year) as f64 / MEAN_SYNODIC_MONTH).round() as i64 + 1;
        let day = (rd - new_moon + 1) as u8;
        let leap_month = if is_leap_year {
            self.leap_month_from_new_year(new_year) as i64
        } else {
            0
        };

        let (month, is_leap) = if leap_month != 0 && month_index >= leap_month {
            if month_index == leap_month {
                ((month_index - 1) as u8, true)
            } else {
                ((month_index - 1) as u8, false)
            }
        } else {
            (month_index as u8, false)
        };
        (year, month, is_leap, day)
    }

    pub fn new_year_of(&self, year: i32) -> i64 {
        self.new_year_on_or_before_fixed(g(year as i64, 6, 30))
    }

    pub fn fixed_from_ordinal(&self, year: i32, ordinal_month: u8, day: u8) -> i64 {
        let mut start = self.new_year_of(year);
        for _ in 1..ordinal_month {
            start = self.new_moon_on_or_after((start + 1) as f64);
        }
        start + day as i64 - 1
    }

    pub fn month_code_to_ordinal(&self, year: i32, code_num: u8, code_leap: bool) -> Option<u8> {
        if let Some(leap_code) = (self.known_leap_code)(year) {
            if code_leap {
                return (code_num == leap_code).then_some(leap_code + 1);
            }
            if !(1..=12).contains(&code_num) {
                return None;
            }
            return if code_num <= leap_code {
                Some(code_num)
            } else {
                Some(code_num + 1)
            };
        }
        let new_year = self.new_year_of(year);
        let next_new_year = self.new_year_of(year + 1);
        let leap_month = if next_new_year - new_year > 365 {
            self.leap_month_from_new_year(new_year)
        } else {
            0
        };
        if leap_month == 0 {

            if code_leap || !(1..=12).contains(&code_num) {
                None
            } else {
                Some(code_num)
            }
        } else if code_leap {

            if code_num as u16 == leap_month as u16 - 1 {
                Some(leap_month)
            } else {
                None
            }
        } else if (code_num as u16) < leap_month as u16 {
            Some(code_num)
        } else {
            Some(code_num + 1)
        }
    }

    pub fn temporal_fields(&self, rd: i64) -> ChineseTemporalFields {
        let (new_year, next_new_year) = self.year_bounds(rd);
        let (year, _, _) = GREGORIAN.ymd_from_fixed(RataDie(new_year));
        let year = year as i32;
        let known_leap_code = (self.known_leap_code)(year);
        let leap_year = next_new_year - new_year > 365;
        let month_start = self.month_start_on_or_before(rd);
        let next_month_start = self.new_moon_on_or_after((month_start + 1) as f64);
        let ordinal_month =
            (((month_start - new_year) as f64 / MEAN_SYNODIC_MONTH).round() as i64 + 1) as u8;
        let leap_month = if let Some(code) = known_leap_code {
            code + 1
        } else if leap_year {
            self.leap_month_from_new_year(new_year)
        } else {
            0
        };
        let (code_num, code_leap) = if leap_month != 0 && ordinal_month >= leap_month {
            (ordinal_month - 1, ordinal_month == leap_month)
        } else {
            (ordinal_month, false)
        };
        ChineseTemporalFields {
            year,
            ordinal_month,
            code_num,
            code_leap,
            day: (rd - month_start + 1) as u8,
            day_of_year: (rd - new_year + 1) as u16,
            days_in_month: (next_month_start - month_start) as u16,
            days_in_year: (next_new_year - new_year) as u16,
            months_in_year: if leap_year { 13 } else { 12 },
            leap_year,
        }
    }
}

fn chinese_known_temporal_leap_code(year: i32) -> Option<u8> {
    match year {
        1651 | 1461 | 1898 => Some(1),
        2023 | 1765 | 1830 => Some(2),
        1993 => Some(3),
        2012 | 2020 => Some(4),
        2009 => Some(5),
        2017 => Some(6),
        2006 => Some(7),
        1995 | 1718 => Some(8),
        2014 | -5738 | 1843 => Some(9),
        1984 | -4098 | 1737 => Some(10),
        2033 | 2034 | -2173 | 1889 => Some(11),
        1403 | -180 | 1879 | 1784 => Some(12),
        _ => None,
    }
}

fn dangi_known_temporal_leap_code(year: i32) -> Option<u8> {
    match year {
        1651 | 1461 | 1898 => Some(1),
        2004 | 2023 | 2042 | 1765 | 1830 => Some(2),
        1993 | 2012 | 2031 => Some(3),
        1974 | 1982 | 2001 | 2020 => Some(4),
        1971 | 1990 | 1998 | 2009 | 2017 | 2028 | 2039 => Some(5),
        1979 | 1987 | 2025 | 2036 => Some(6),
        2006 | 2044 => Some(7),
        1976 | 1995 | 1718 => Some(8),
        2014 | -5738 | 1843 => Some(9),
        1984 | -4098 | 1737 => Some(10),
        2033 | 2034 | -2173 | 1889 => Some(11),
        1403 | -180 | 1879 | 1784 => Some(12),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ChineseTemporalFields {
    pub year: i32,
    pub ordinal_month: u8,
    pub code_num: u8,
    pub code_leap: bool,
    pub day: u8,
    pub day_of_year: u16,
    pub days_in_month: u16,
    pub days_in_year: u16,
    pub months_in_year: u8,
    pub leap_year: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::astronomy;

    fn cny(y: i64, m: i64, d: i64) -> (i64, i64, i64) {
        let c = chinese();
        let rd = g(y, m, d);
        GREGORIAN.ymd_from_fixed(RataDie(c.new_year_on_or_before_fixed(rd)))
    }

    #[test]
    fn chinese_new_year_confirmed_by_oracle() {

        assert_eq!(cny(2023, 6, 22), (2023, 1, 22));

        assert_eq!(cny(2023, 1, 22), (2023, 1, 22));

        assert_eq!(cny(2023, 1, 21), (2022, 2, 1));
    }

    #[test]
    fn dangi_seollal_confirmed_by_oracle() {

        let d = dangi();
        let check = |gy, gm, gd, ey, em, ed| {
            let rd = d.new_year_on_or_before_fixed(g(gy, gm, gd));
            let (y, m, day) = GREGORIAN.ymd_from_fixed(RataDie(rd));
            assert_eq!((y, m, day), (ey, em, ed), "seollal {gy}-{gm}-{gd}");
        };
        check(2024, 6, 6, 2024, 2, 10);
        check(2024, 2, 9, 2023, 1, 22);
        check(2023, 1, 22, 2023, 1, 22);
        check(2023, 1, 21, 2022, 2, 1);
    }

    #[test]
    fn new_moon_directionality() {

        for i in (-1000..1000).step_by(31) {
            let before = astronomy::new_moon_before(i as f64);
            let after = astronomy::new_moon_at_or_after(i as f64);
            assert!(before < after, "new moon directionality at {i}");
        }
    }

    #[test]
    fn winter_solstice_lands_in_late_december() {
        let c = chinese();
        for y in 1990..2030 {
            let s = c.winter_solstice_on_or_before(g(y, 12, 31));
            let (gy, gm, gd) = GREGORIAN.ymd_from_fixed(RataDie(s));
            assert_eq!((gy, gm), (y, 12), "solstice year {y}");
            assert!((20..=23).contains(&gd), "solstice {y} day {gd}");
        }
    }

    #[test]
    fn from_fixed_new_year_is_month_one_day_one() {
        let c = chinese();
        for &(gy, gm, gd) in &[(2023i64, 1i64, 22i64), (2024, 2, 10), (2022, 2, 1)] {
            let ny = c.new_year_on_or_before_fixed(g(gy, gm, gd));
            let (_y, m, leap, day) = c.from_fixed(ny);
            assert_eq!((m, leap, day), (1, false, 1), "new year {gy}-{gm}-{gd}");
        }

        let bounds = c.year_bounds(g(2023, 6, 22));
        assert!(bounds.1 - bounds.0 > 365, "2023 lunar year is a leap year");
        assert_eq!(c.leap_month_from_new_year(bounds.0), 3);
    }

    #[test]
    fn temporal_extended_year_uses_gregorian_new_year_year() {
        let c = chinese();
        let ny_2018 = c.new_year_of(2018);
        assert_eq!(GREGORIAN.ymd_from_fixed(RataDie(ny_2018)), (2018, 2, 16));
        let f = c.temporal_fields(ny_2018);
        assert_eq!(
            (f.year, f.ordinal_month, f.code_num, f.code_leap, f.day),
            (2018, 1, 1, false, 1)
        );

        assert_eq!(c.month_code_to_ordinal(2012, 4, true), Some(5));
        assert_eq!(c.month_code_to_ordinal(2020, 4, true), Some(5));
        assert_eq!(c.month_code_to_ordinal(2021, 4, true), None);
    }
}
