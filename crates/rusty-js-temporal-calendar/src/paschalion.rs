// The Lord is King, He is robed in majesty.

use crate::{IsoDate, RataDie, GREGORIAN, JULIAN};

pub fn orthodox_pascha(year: i32) -> IsoDate {
    let (a, b, c) = (year % 4, year % 7, year % 19);
    let d = (19 * c + 15) % 30;
    let e = (2 * a + 4 * b - d + 34).rem_euclid(7);
    let f = d + e + 114;
    let month = (f / 31) as i64;
    let day = (f % 31 + 1) as i64;
    let rd = JULIAN.fixed_from(year as i64, month, day);
    let (y, m, dd) = GREGORIAN.ymd_from_fixed(rd);
    IsoDate::new_unchecked(y as i32, m as u8, dd as u8)
}

pub fn fixed_julian_feast(year: i32, julian_month: u8, julian_day: u8) -> IsoDate {
    let rd = JULIAN.fixed_from(year as i64, julian_month as i64, julian_day as i64);
    let (y, m, d) = GREGORIAN.ymd_from_fixed(rd);
    IsoDate::new_unchecked(y as i32, m as u8, d as u8)
}

pub const HEBREW_EPOCH_RD: i64 = -1373427;
pub fn hebrew_epoch() -> i64 {
    HEBREW_EPOCH_RD
}

fn hebrew_elapsed_days(h_year: i64) -> i64 {
    let months = (235 * h_year - 234).div_euclid(19);
    let parts = 12084 + 13753 * months;
    let days = 29 * months + parts.div_euclid(25920);

    if (3 * (days + 1)).rem_euclid(7) < 3 {
        days + 1
    } else {
        days
    }
}

pub fn hebrew_rosh_hashanah(h_year: i64) -> RataDie {
    let ny0 = hebrew_elapsed_days(h_year - 1);
    let ny1 = hebrew_elapsed_days(h_year);
    let ny2 = hebrew_elapsed_days(h_year + 1);
    let corr = if ny2 - ny1 == 356 {
        2
    } else if ny1 - ny0 == 382 {
        1
    } else {
        0
    };
    RataDie(hebrew_epoch() + ny1 + corr)
}

pub fn hebrew_passover(greg_year: i32) -> RataDie {
    let h_year = greg_year as i64 + 3760;
    RataDie(hebrew_rosh_hashanah(h_year + 1).0 - 163)
}

pub const CREATION_JULIAN_YEAR: i64 = -5508;

pub fn byzantine_epoch() -> RataDie {
    JULIAN.fixed_from(CREATION_JULIAN_YEAR, 9, 1)
}

pub fn anno_mundi(rd: RataDie) -> i64 {
    let (jy, jm, _) = JULIAN.ymd_from_fixed(rd);
    let base = jy - CREATION_JULIAN_YEAR;
    if jm >= 9 {
        base
    } else {
        base - 1
    }
}

pub fn anno_mundi_traditional(rd: RataDie) -> i64 {
    anno_mundi(rd) + 1
}

pub fn fixed_from_anno_mundi(
    am_traditional_year: i64,
    julian_month: i64,
    julian_day: i64,
) -> RataDie {
    let julian_year = if julian_month >= 9 {
        am_traditional_year - INCARNATION_ANNO_MUNDI
    } else {
        am_traditional_year - INCARNATION_ANNO_MUNDI + 1
    };
    JULIAN.fixed_from(julian_year, julian_month, julian_day)
}

pub const INCARNATION_ANNO_MUNDI: i64 = 5509;

pub fn anno_domini_epoch() -> RataDie {
    JULIAN.fixed_from(1, 1, 1)
}

pub fn anno_domini(rd: RataDie) -> i64 {
    JULIAN.ymd_from_fixed(rd).0
}

pub fn anno_domini_label(rd: RataDie) -> (bool, i64) {
    let y = anno_domini(rd);
    if y >= 1 {
        (true, y)
    } else {
        (false, 1 - y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchors_against_published_dates() {

        assert_eq!(orthodox_pascha(2024), IsoDate::new_unchecked(2024, 5, 5));
        assert_eq!(orthodox_pascha(2025), IsoDate::new_unchecked(2025, 4, 20));

        assert_eq!(
            fixed_julian_feast(2024, 12, 25),
            IsoDate::new_unchecked(2025, 1, 7)
        );

        assert_eq!(
            fixed_julian_feast(2025, 1, 6),
            IsoDate::new_unchecked(2025, 1, 19)
        );

        let pv = GREGORIAN.ymd_from_fixed(hebrew_passover(2024));
        assert_eq!(pv, (2024, 4, 23));
    }

    #[test]
    fn zonaras_proviso_and_deviation_structure() {

        let mut outliers = vec![];
        for y in 2024..=2050 {
            let p = orthodox_pascha(y);
            let pr = GREGORIAN
                .fixed_from(p.year as i64, p.month as i64, p.day as i64)
                .0;
            let dev = pr - hebrew_passover(y).0;
            assert!(
                dev > 0,
                "proviso violated in {y}: Pascha not after Passover"
            );
            if dev >= 30 {
                outliers.push((y, dev));
            }
        }

        assert_eq!(outliers, vec![(2032, 36), (2040, 38)]);
    }

    #[test]
    fn anno_mundi_inverse_round_trips() {

        for &(y, m, d) in &[(2024i64, 9, 1), (2024, 8, 31), (1868, 8, 27), (1, 1, 1)] {
            let rd = JULIAN.fixed_from(y, m, d);
            let am = anno_mundi_traditional(rd);
            assert_eq!(fixed_from_anno_mundi(am, m, d), rd, "{y}-{m}-{d} AM={am}");
        }
    }

    #[test]
    fn byzantine_anno_mundi_year_zero_epoch() {

        assert_eq!(anno_mundi(byzantine_epoch()), 0);
        assert_eq!(anno_mundi_traditional(byzantine_epoch()), 1);

        assert_eq!(JULIAN.ymd_from_fixed(byzantine_epoch()), (-5508, 9, 1));

        let sept1_2024_julian = JULIAN.fixed_from(2024, 9, 1);
        assert_eq!(anno_mundi(sept1_2024_julian), 7532);
        assert_eq!(anno_mundi_traditional(sept1_2024_julian), 7533);

        let aug31_2024_julian = JULIAN.fixed_from(2024, 8, 31);
        assert_eq!(anno_mundi_traditional(aug31_2024_julian), 7532);

        let mut prev = anno_mundi(RataDie(byzantine_epoch().0 - 400));
        for k in (-400..400).step_by(40) {
            let am = anno_mundi(RataDie(byzantine_epoch().0 + k));
            assert!(am >= prev, "AM count must be monotone across BC/AD");
            prev = am;
        }
    }

    #[test]
    fn anno_domini_anchored_within_anno_mundi() {
        let epoch = anno_domini_epoch();

        assert_eq!(epoch, RataDie(-1));
        assert_eq!(JULIAN.ymd_from_fixed(epoch), (1, 1, 1));
        assert_eq!(anno_domini(epoch), 1);
        assert_eq!(anno_domini_label(epoch), (true, 1));

        assert_eq!(anno_mundi_traditional(epoch), INCARNATION_ANNO_MUNDI);
        assert_eq!(anno_mundi_traditional(epoch) - anno_domini(epoch), 5508);

        let mid_year_1bc = JULIAN.fixed_from(0, 6, 1);
        assert_eq!(anno_domini(mid_year_1bc), 0);
        assert_eq!(anno_domini_label(mid_year_1bc), (false, 1));
        let mid_year_2bc = JULIAN.fixed_from(-1, 6, 1);
        assert_eq!(anno_domini_label(mid_year_2bc), (false, 2));

        let jan2024 = JULIAN.fixed_from(2024, 1, 1);
        assert_eq!(anno_domini(jan2024), 2024);
        assert_eq!(anno_mundi_traditional(jan2024), 7532);

        assert_eq!(anno_mundi_traditional(jan2024) - anno_domini(jan2024), 5508);
        let sep2024 = JULIAN.fixed_from(2024, 9, 1);
        assert_eq!(anno_mundi_traditional(sep2024) - anno_domini(sep2024), 5509);
    }
}
