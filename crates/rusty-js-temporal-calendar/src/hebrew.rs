
use crate::RataDie;

pub const NISAN: u8 = 1;
pub const TISHRI: u8 = 7;
pub const MARHESHVAN: u8 = 8;
pub const KISLEV: u8 = 9;
pub const ADAR: u8 = 12;
pub const ADAR_II: u8 = 13;

pub const HEBREW_EPOCH_RD: i64 = -1373427;

pub fn is_leap_year(year: i32) -> bool {
    (7 * year as i64 + 1).rem_euclid(19) < 7
}

pub fn last_month(year: i32) -> u8 {
    if is_leap_year(year) {
        ADAR_II
    } else {
        ADAR
    }
}

fn elapsed_days(year: i32) -> i64 {
    let months = (235 * year as i64 - 234).div_euclid(19);
    let parts = 12084 + 13753 * months;
    let days = 29 * months + parts.div_euclid(25920);
    if (3 * (days + 1)).rem_euclid(7) < 3 {
        days + 1
    } else {
        days
    }
}

fn year_length_correction(year: i32) -> i64 {
    let ny0 = elapsed_days(year - 1);
    let ny1 = elapsed_days(year);
    let ny2 = elapsed_days(year + 1);
    if ny2 - ny1 == 356 {
        2
    } else if ny1 - ny0 == 382 {
        1
    } else {
        0
    }
}

pub fn new_year(year: i32) -> RataDie {
    RataDie(HEBREW_EPOCH_RD + elapsed_days(year) + year_length_correction(year))
}

pub fn days_in_year(year: i32) -> u16 {
    (new_year(year + 1).0 - new_year(year).0) as u16
}

fn is_long_marheshvan(year: i32) -> bool {
    matches!(days_in_year(year), 355 | 385)
}
fn is_short_kislev(year: i32) -> bool {
    matches!(days_in_year(year), 353 | 383)
}

pub fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        2 | 4 | 6 | 10 | ADAR_II => 29,
        ADAR if !is_leap_year(year) => 29,
        MARHESHVAN if !is_long_marheshvan(year) => 29,
        KISLEV if is_short_kislev(year) => 29,
        _ => 30,
    }
}

fn calendar_months(year: i32) -> impl Iterator<Item = u8> {
    (TISHRI..=last_month(year)).chain(NISAN..TISHRI)
}

pub fn fixed_from(year: i32, month: u8, day: u8) -> RataDie {
    let mut total = new_year(year).0 + day as i64 - 1;

    for m in calendar_months(year) {
        if m == month {
            break;
        }
        total += days_in_month(year, m) as i64;
    }
    RataDie(total)
}

pub fn from_fixed(rd: RataDie) -> (i32, u8, u8) {

    let mut year = (((rd.0 - HEBREW_EPOCH_RD) * 98496) / 35975351 + 1) as i32;
    while new_year(year).0 > rd.0 {
        year -= 1;
    }
    while new_year(year + 1).0 <= rd.0 {
        year += 1;
    }

    let mut month = TISHRI;
    for m in calendar_months(year) {
        if rd.0 <= fixed_from(year, m, days_in_month(year, m)).0 {
            month = m;
            break;
        }
    }
    let day = (rd.0 - fixed_from(year, month, 1).0 + 1) as u8;
    (year, month, day)
}

pub fn to_civil_month(year: i32, biblical_month: u8) -> u8 {
    let mut civil = (biblical_month + 6) % 12;
    if civil == 0 {
        civil = 12;
    }
    if is_leap_year(year) && biblical_month < TISHRI {
        civil += 1;
    }
    civil
}

pub fn from_civil_month(year: i32, civil_month: u8) -> u8 {
    if civil_month <= 6 {
        civil_month + 6
    } else {
        let mut biblical = civil_month - 6;
        if is_leap_year(year) {
            biblical -= 1;
        }
        if biblical == 0 {
            ADAR_II
        } else {
            biblical
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GREGORIAN;

    #[test]
    fn leap_cycle_and_year_lengths() {

        let leaps: Vec<i32> = (1..=19).filter(|&y| is_leap_year(y)).collect();
        assert_eq!(leaps.len(), 7);

        assert!(is_leap_year(5784));
        assert!(!is_leap_year(5785));

        for y in 5780..5820 {
            assert!(
                matches!(days_in_year(y), 353 | 354 | 355 | 383 | 384 | 385),
                "year {y}"
            );
        }
    }

    #[test]
    fn anchors_against_published_dates() {

        assert_eq!(GREGORIAN.ymd_from_fixed(new_year(5784)), (2023, 9, 16));
        assert_eq!(GREGORIAN.ymd_from_fixed(new_year(5785)), (2024, 10, 3));

        assert_eq!(
            GREGORIAN.ymd_from_fixed(fixed_from(5784, NISAN, 15)),
            (2024, 4, 23)
        );

        assert_eq!(fixed_from(5785, TISHRI, 1), new_year(5785));
    }

    #[test]
    fn round_trip_dense() {

        for rd in (new_year(5700).0..new_year(5830).0).step_by(29) {
            let r = RataDie(rd);
            let (y, m, d) = from_fixed(r);
            assert_eq!(fixed_from(y, m, d), r, "rd {rd} -> ({y},{m},{d})");
            assert!(d >= 1 && d <= days_in_month(y, m));
        }
    }

    #[test]
    fn civil_biblical_mapping() {

        assert_eq!(to_civil_month(5785, TISHRI), 1);

        assert_eq!(to_civil_month(5785, NISAN), 7);
        assert_eq!(to_civil_month(5784, NISAN), 8);

        for &y in &[5784i32, 5785] {
            for bm in calendar_months(y) {
                assert_eq!(
                    from_civil_month(y, to_civil_month(y, bm)),
                    bm,
                    "year {y} month {bm}"
                );
            }
        }
    }
}
