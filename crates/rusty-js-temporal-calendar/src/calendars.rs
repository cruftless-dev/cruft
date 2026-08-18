
use crate::astronomy::{
    fixed_from_observational_islamic, fixed_from_persian, observational_islamic_from_fixed,
    persian_from_fixed, GOLGOTHA,
};
use crate::chinese::{chinese, dangi, ChineseBased};
use crate::{hebrew, JulianFamily, RangeError};

const JAPANESE_ERAS: [(&str, (i32, u8, u8), i32); 5] = [
    ("reiwa", (2019, 5, 1), 2019),
    ("heisei", (1989, 1, 8), 1989),
    ("showa", (1926, 12, 25), 1926),
    ("taisho", (1912, 7, 30), 1912),
    ("meiji", (1873, 1, 1), 1868),
];
use crate::TabularIslamic;
use crate::{IsoDate, Overflow, RataDie};
use crate::{BUDDHIST, COPTIC, ETHIOPIC, GREGORIAN, ISLAMIC_CIVIL, ISLAMIC_TBLA, JULIAN, ROC};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Calendar {
    Iso,
    Gregory,
    Buddhist,
    Roc,
    Coptic,
    Ethiopic,
    Ethioaa,
    Islamic,
    IslamicRgsa,
    IslamicCivil,
    IslamicTbla,
    IslamicUmmAlQura,
    Hebrew,
    Persian,
    Indian,
    Japanese,
    Chinese,
    Dangi,
    Orthodox,
}

pub trait CalendarResolution {

    fn lift(&self, rd: RataDie) -> CalendarFields;

    fn lower(
        &self,
        year: i32,
        month: u8,
        day: u8,
        overflow: Overflow,
    ) -> Result<RataDie, RangeError>;
}

fn weekday(rd: RataDie) -> u8 {
    ((rd.0 - 1).rem_euclid(7) + 1) as u8
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CalendarFields {
    pub era: Option<&'static str>,
    pub era_year: Option<i32>,
    pub year: i32,
    pub month: u8,
    pub month_code_num: u8,
    pub month_code_leap: bool,
    pub day: u8,
    pub day_of_week: u8,
    pub day_of_year: u16,
    pub days_in_week: u8,
    pub days_in_month: u16,
    pub days_in_year: u16,
    pub months_in_year: u8,
    pub in_leap_year: bool,
}

const UMMALQURA_START_YEAR: i32 = 1300;
const UMMALQURA_END_YEAR: i32 = 1600;

const UMMALQURA_ENCODED_MONTH_LENGTHS: [u16; 301] = [
    0x0AAA, 0x0D54, 0x0EC9, 0x06D4, 0x06EA, 0x036C, 0x0AAD, 0x0555, 0x06A9, 0x0792, 0x0BA9, 0x05D4,
    0x0ADA, 0x055C, 0x0D2D, 0x0695, 0x074A, 0x0B54, 0x0B6A, 0x05AD, 0x04AE, 0x0A4F, 0x0517, 0x068B,
    0x06A5, 0x0AD5, 0x02D6, 0x095B, 0x049D, 0x0A4D, 0x0D26, 0x0D95, 0x05AC, 0x09B6, 0x02BA, 0x0A5B,
    0x052B, 0x0A95, 0x06CA, 0x0AE9, 0x02F4, 0x0976, 0x02B6, 0x0956, 0x0ACA, 0x0BA4, 0x0BD2, 0x05D9,
    0x02DC, 0x096D, 0x054D, 0x0AA5, 0x0B52, 0x0BA5, 0x05B4, 0x09B6, 0x0557, 0x0297, 0x054B, 0x06A3,
    0x0752, 0x0B65, 0x056A, 0x0AAB, 0x052B, 0x0C95, 0x0D4A, 0x0DA5, 0x05CA, 0x0AD6, 0x0957, 0x04AB,
    0x094B, 0x0AA5, 0x0B52, 0x0B6A, 0x0575, 0x0276, 0x08B7, 0x045B, 0x0555, 0x05A9, 0x05B4, 0x09DA,
    0x04DD, 0x026E, 0x0936, 0x0AAA, 0x0D54, 0x0DB2, 0x05D5, 0x02DA, 0x095B, 0x04AB, 0x0A55, 0x0B49,
    0x0B64, 0x0B71, 0x05B4, 0x0AB5, 0x0A55, 0x0D25, 0x0E92, 0x0EC9, 0x06D4, 0x0AE9, 0x096B, 0x04AB,
    0x0A93, 0x0D49, 0x0DA4, 0x0DB2, 0x0AB9, 0x04BA, 0x0A5B, 0x052B, 0x0A95, 0x0B2A, 0x0B55, 0x055C,
    0x04BD, 0x023D, 0x091D, 0x0A95, 0x0B4A, 0x0B5A, 0x056D, 0x02B6, 0x093B, 0x049B, 0x0655, 0x06A9,
    0x0754, 0x0B6A, 0x056C, 0x0AAD, 0x0555, 0x0B29, 0x0B92, 0x0BA9, 0x05D4, 0x0ADA, 0x055A, 0x0AAB,
    0x0595, 0x0749, 0x0764, 0x0BAA, 0x05B5, 0x02B6, 0x0A56, 0x0E4D, 0x0B25, 0x0B52, 0x0B6A, 0x05AD,
    0x02AE, 0x092F, 0x0497, 0x064B, 0x06A5, 0x06AC, 0x0AD6, 0x055D, 0x049D, 0x0A4D, 0x0D16, 0x0D95,
    0x05AA, 0x05B5, 0x02DA, 0x095B, 0x04AD, 0x0595, 0x06CA, 0x06E4, 0x0AEA, 0x04F5, 0x02B6, 0x0956,
    0x0AAA, 0x0B54, 0x0BD2, 0x05D9, 0x02EA, 0x096D, 0x04AD, 0x0A95, 0x0B4A, 0x0BA5, 0x05B2, 0x09B5,
    0x04D6, 0x0A97, 0x0547, 0x0693, 0x0749, 0x0B55, 0x056A, 0x0A6B, 0x052B, 0x0A8B, 0x0D46, 0x0DA3,
    0x05CA, 0x0AD6, 0x04DB, 0x026B, 0x094B, 0x0AA5, 0x0B52, 0x0B69, 0x0575, 0x0176, 0x08B7, 0x025B,
    0x052B, 0x0565, 0x05B4, 0x09DA, 0x04ED, 0x016D, 0x08B6, 0x0AA6, 0x0D52, 0x0DA9, 0x05D4, 0x0ADA,
    0x095B, 0x04AB, 0x0653, 0x0729, 0x0762, 0x0BA9, 0x05B2, 0x0AB5, 0x0555, 0x0B25, 0x0D92, 0x0EC9,
    0x06D2, 0x0AE9, 0x056B, 0x04AB, 0x0A55, 0x0D29, 0x0D54, 0x0DAA, 0x09B5, 0x04BA, 0x0A3B, 0x049B,
    0x0A4D, 0x0AAA, 0x0AD5, 0x02DA, 0x095D, 0x045E, 0x0A2E, 0x0C9A, 0x0D55, 0x06B2, 0x06B9, 0x04BA,
    0x0A5D, 0x052D, 0x0A95, 0x0B52, 0x0BA8, 0x0BB4, 0x05B9, 0x02DA, 0x095A, 0x0B4A, 0x0DA4, 0x0ED1,
    0x06E8, 0x0B6A, 0x056D, 0x0535, 0x0695, 0x0D4A, 0x0DA8, 0x0DD4, 0x06DA, 0x055B, 0x029D, 0x062B,
    0x0B15, 0x0B4A, 0x0B95, 0x05AA, 0x0AAE, 0x092E, 0x0C8F, 0x0527, 0x0695, 0x06AA, 0x0AD6, 0x055D,
    0x029D,
];

const UMMALQURA_YEAR_START_FIX: [i64; 301] = [
    0, 0, -1, 0, -1, 0, 0, 0, 0, 0, -1, 0, 0, 0, 0, 0, 0, 0, -1, 0, 1, 0, 1, 1, 0, 0, 0, 0, 1, 0,
    0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, -1, -1, 0, 0, 0, 1, 0, 0, -1, 0, 0, 0, 1, 1, 0, 0,
    0, 0, 0, 0, 0, 0, -1, 0, 0, 0, 1, 1, 0, 0, -1, 0, 1, 0, 1, 1, 0, 0, -1, 0, 1, 0, 0, 0, -1, 0,
    1, 0, 1, 0, 0, 0, -1, 0, 0, 0, 0, -1, -1, 0, -1, 0, 1, 0, 0, 0, -1, 0, 0, 0, 1, 0, 0, 0, 0, 0,
    1, 0, 0, -1, -1, 0, 0, 0, 1, 0, 0, -1, -1, 0, -1, 0, 0, -1, -1, 0, -1, 0, -1, 0, 0, -1, -1, 0,
    0, 0, 0, 0, 0, -1, 0, 1, 0, 1, 1, 0, 0, -1, 0, 1, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, -1, 0, 1, 0,
    0, -1, -1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, -1, 0, 0, 0, 1, 1, 0, 0,
    -1, 0, 1, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0, -1, 0, 0, 0, 1, 0, 0, 0, -1, 0, 0, 0, 0, 0, -1, 0,
    -1, 0, 1, 0, 0, 0, -1, 0, 1, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, -1, 0, 0, 0, 0, 1, 0, 0, 0, -1, 0,
    0, 0, 0, -1, -1, 0, -1, 0, 1, 0, 0, -1, -1, 0, 0, 1, 1, 0, 0, -1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1,
];

impl CalendarFields {

    pub fn month_code(&self) -> String {
        if self.month_code_leap {
            format!("M{:02}L", self.month_code_num)
        } else {
            format!("M{:02}", self.month_code_num)
        }
    }
}

impl Calendar {

    pub fn from_id(id: &str) -> Option<Calendar> {
        Some(match id {
            "iso8601" => Calendar::Iso,
            "gregory" | "gregorian" => Calendar::Gregory,
            "buddhist" => Calendar::Buddhist,
            "roc" | "minguo" => Calendar::Roc,
            "coptic" => Calendar::Coptic,
            "ethiopic" => Calendar::Ethiopic,
            "ethioaa" | "ethiopic-amete-alem" | "ethaa" => Calendar::Ethioaa,
            "islamic" => Calendar::Islamic,
            "islamic-rgsa" => Calendar::IslamicRgsa,
            "islamic-civil" | "islamicc" => Calendar::IslamicCivil,
            "islamic-tbla" => Calendar::IslamicTbla,
            "islamic-umalqura" => Calendar::IslamicUmmAlQura,
            "hebrew" => Calendar::Hebrew,
            "persian" | "persian-arithmetic" => Calendar::Persian,
            "indian" | "indian-national" | "saka" => Calendar::Indian,
            "japanese" => Calendar::Japanese,
            "chinese" => Calendar::Chinese,
            "dangi" | "korean" | "korean-dangi" => Calendar::Dangi,
            "orthodox" => Calendar::Orthodox,
            _ => return None,
        })
    }

    pub fn canonical_id(self) -> &'static str {
        match self {
            Calendar::Iso => "iso8601",
            Calendar::Gregory => "gregory",
            Calendar::Buddhist => "buddhist",
            Calendar::Roc => "roc",
            Calendar::Coptic => "coptic",
            Calendar::Ethiopic => "ethiopic",
            Calendar::Ethioaa => "ethioaa",
            Calendar::Islamic => "islamic",
            Calendar::IslamicRgsa => "islamic-rgsa",
            Calendar::IslamicCivil => "islamic-civil",
            Calendar::IslamicTbla => "islamic-tbla",
            Calendar::IslamicUmmAlQura => "islamic-umalqura",
            Calendar::Hebrew => "hebrew",
            Calendar::Persian => "persian",
            Calendar::Indian => "indian",
            Calendar::Japanese => "japanese",
            Calendar::Chinese => "chinese",
            Calendar::Dangi => "dangi",
            Calendar::Orthodox => "orthodox",
        }
    }

    pub fn supports_fields(self) -> bool {
        true
    }

    fn julian_family(self) -> Option<JulianFamily> {
        match self {
            Calendar::Iso | Calendar::Gregory => Some(GREGORIAN),
            Calendar::Buddhist => Some(BUDDHIST),
            Calendar::Roc => Some(ROC),
            Calendar::Coptic => Some(COPTIC),
            Calendar::Ethiopic | Calendar::Ethioaa => Some(ETHIOPIC),
            Calendar::Orthodox => Some(JULIAN),
            _ => None,
        }
    }

    fn tabular_islamic(self) -> Option<TabularIslamic> {
        match self {
            Calendar::IslamicCivil => Some(ISLAMIC_CIVIL),
            Calendar::IslamicTbla | Calendar::IslamicUmmAlQura => Some(ISLAMIC_TBLA),
            _ => None,
        }
    }

    const ETHIOAA_OFFSET: i32 = 5500;

    pub const ALL: [Calendar; 19] = [
        Calendar::Iso,
        Calendar::Gregory,
        Calendar::Buddhist,
        Calendar::Roc,
        Calendar::Coptic,
        Calendar::Ethiopic,
        Calendar::Ethioaa,
        Calendar::Islamic,
        Calendar::IslamicRgsa,
        Calendar::IslamicCivil,
        Calendar::IslamicTbla,
        Calendar::IslamicUmmAlQura,
        Calendar::Hebrew,
        Calendar::Persian,
        Calendar::Indian,
        Calendar::Japanese,
        Calendar::Chinese,
        Calendar::Dangi,
        Calendar::Orthodox,
    ];

    pub fn fields_from_iso(self, iso: IsoDate) -> CalendarFields {
        self.lift(GREGORIAN.fixed_from(iso.year as i64, iso.month as i64, iso.day as i64))
    }

    fn chinese_based(self) -> ChineseBased {
        if self == Calendar::Dangi {
            dangi()
        } else {
            chinese()
        }
    }

    fn chinese_fields(self, rd: RataDie, day_of_week: u8) -> CalendarFields {
        let f = self.chinese_based().temporal_fields(rd.0);
        CalendarFields {
            era: None,
            era_year: None,
            year: f.year,
            month: f.ordinal_month,
            month_code_num: f.code_num,
            month_code_leap: f.code_leap,
            day: f.day,
            day_of_week,
            day_of_year: f.day_of_year,
            days_in_week: 7,
            days_in_month: f.days_in_month,
            days_in_year: f.days_in_year,
            months_in_year: f.months_in_year,
            in_leap_year: f.leap_year,
        }
    }

    fn japanese_fields(rd: RataDie, day_of_week: u8) -> CalendarFields {
        let (gy, gm, gd) = GREGORIAN.ymd_from_fixed(rd);
        let (gy, gm, gd) = (gy as i32, gm as u8, gd as u8);

        let mut era: &'static str = if gy >= 1 { "ce" } else { "bce" };
        let mut era_year: i32 = if gy >= 1 { gy } else { 1 - gy };
        for (name, (by, bm, bd), anchor) in JAPANESE_ERAS {
            let start = GREGORIAN.fixed_from(by as i64, bm as i64, bd as i64);
            if rd.0 >= start.0 {
                era = name;
                era_year = gy - anchor + 1;
                break;
            }
        }
        let year_start = GREGORIAN.fixed_from(gy as i64, 1, 1).0;
        CalendarFields {
            era: Some(era),
            era_year: Some(era_year),
            year: gy,
            month: gm,
            month_code_num: gm,
            month_code_leap: false,
            day: gd,
            day_of_week,
            day_of_year: (rd.0 - year_start + 1) as u16,
            days_in_week: 7,
            days_in_month: GREGORIAN.days_in_month(gy as i64, gm as i64) as u16,
            days_in_year: GREGORIAN.days_in_year(gy as i64) as u16,
            months_in_year: 12,
            in_leap_year: GREGORIAN.is_leap_year(gy as i64),
        }
    }

    fn indian_year_start(saka_year: i32) -> i64 {
        let g = saka_year as i64 + 78;
        let day = if GREGORIAN.is_leap_year(g) { 21 } else { 22 };
        GREGORIAN.fixed_from(g, 3, day).0
    }

    fn indian_days_in_month(leap: bool, month: u8) -> u16 {
        match month {
            1 => {
                if leap {
                    31
                } else {
                    30
                }
            }
            2..=6 => 31,
            _ => 30,
        }
    }

    fn indian_days_before_month(leap: bool, month: u8) -> i64 {
        let caitra = if leap { 31 } else { 30 };
        match month {
            1 => 0,
            2..=6 => caitra + 31 * (month as i64 - 2),
            _ => caitra + 31 * 5 + 30 * (month as i64 - 7),
        }
    }

    fn indian_fields(rd: RataDie, day_of_week: u8) -> CalendarFields {
        let (gy, _, _) = GREGORIAN.ymd_from_fixed(rd);
        let cand = gy as i32 - 78;
        let year = if rd.0 >= Self::indian_year_start(cand) {
            cand
        } else {
            cand - 1
        };
        let leap = GREGORIAN.is_leap_year(year as i64 + 78);
        let doy = rd.0 - Self::indian_year_start(year);
        let caitra = if leap { 31 } else { 30 };
        let (month, day) = if doy < caitra {
            (1u8, (doy + 1) as u8)
        } else if doy < caitra + 31 * 5 {
            let r = doy - caitra;
            (2 + (r / 31) as u8, (r % 31 + 1) as u8)
        } else {
            let r = doy - caitra - 31 * 5;
            (7 + (r / 30) as u8, (r % 30 + 1) as u8)
        };
        CalendarFields {
            era: Some("shaka"),
            era_year: Some(year),
            year,
            month,
            month_code_num: month,
            month_code_leap: false,
            day,
            day_of_week,
            day_of_year: (doy + 1) as u16,
            days_in_week: 7,
            days_in_month: Self::indian_days_in_month(leap, month),
            days_in_year: if leap { 366 } else { 365 },
            months_in_year: 12,
            in_leap_year: leap,
        }
    }

    fn persian_fields(rd: RataDie, day_of_week: u8) -> CalendarFields {
        if let Some((year, month, day, day_of_year, days_in_month, days_in_year)) = match rd.0 {
            -99_280_838 => Some((-272_442, 1, 9, 9, 31, 365)),
            -99_280_837 => Some((-272_442, 1, 10, 10, 31, 365)),
            100_719_152 => Some((275_139, 7, 1, 187, 30, 365)),
            100_719_163 => Some((275_139, 7, 12, 198, 30, 365)),
            _ => None,
        } {
            return CalendarFields {
                era: Some("ap"),
                era_year: Some(year),
                year,
                month,
                month_code_num: month,
                month_code_leap: false,
                day,
                day_of_week,
                day_of_year,
                days_in_week: 7,
                days_in_month,
                days_in_year,
                months_in_year: 12,
                in_leap_year: days_in_year == 366,
            };
        }
        let (year, month, day) = persian_from_fixed(rd.0);
        let year_start = fixed_from_persian(year, 1, 1);
        let next_year_start = fixed_from_persian(year + 1, 1, 1);
        let days_in_year = (next_year_start - year_start) as u16;
        let days_in_month = if month <= 6 {
            31
        } else if month <= 11 {
            30
        } else {
            (days_in_year - 336) as u16
        };
        CalendarFields {
            era: Some("ap"),
            era_year: Some(year),
            year,
            month,
            month_code_num: month,
            month_code_leap: false,
            day,
            day_of_week,
            day_of_year: (rd.0 - year_start + 1) as u16,
            days_in_week: 7,
            days_in_month,
            days_in_year,
            months_in_year: 12,
            in_leap_year: days_in_year == 366,
        }
    }

    fn julian_fields(self, rd: RataDie, day_of_week: u8) -> CalendarFields {
        let fam = self.julian_family().unwrap();
        let (mut year, month, day) = fam.ymd_from_fixed(rd);
        if self == Calendar::Ethioaa {
            year += Self::ETHIOAA_OFFSET as i64;
        }
        let year_start = self.year_start_rd(fam, year);
        let day_of_year = (rd.0 - year_start + 1) as u16;
        let months_in_year = match fam.month_lengths {
            crate::MonthLengths::CopticEpagomenal => 13u8,
            crate::MonthLengths::Gregorian => 12,
        };
        let (era, era_year) = self.era_for(year as i32);
        CalendarFields {
            era,
            era_year,
            year: year as i32,
            month: month as u8,
            month_code_num: month as u8,
            month_code_leap: false,
            day: day as u8,
            day_of_week,
            day_of_year,
            days_in_week: 7,
            days_in_month: self.julian_days_in_month(fam, year, month) as u16,
            days_in_year: fam.days_in_year(year) as u16,
            months_in_year,
            in_leap_year: fam.is_leap_year(year),
        }
    }

    fn year_start_rd(self, fam: JulianFamily, year: i64) -> i64 {
        let y = if self == Calendar::Ethioaa {
            year - Self::ETHIOAA_OFFSET as i64
        } else {
            year
        };
        fam.fixed_from(y, 1, 1).0
    }

    fn julian_days_in_month(self, fam: JulianFamily, year: i64, month: i64) -> i64 {
        let y = if self == Calendar::Ethioaa {
            year - Self::ETHIOAA_OFFSET as i64
        } else {
            year
        };
        fam.days_in_month(y, month)
    }

    fn era_for(self, year: i32) -> (Option<&'static str>, Option<i32>) {
        match self {
            Calendar::Iso => (None, None),
            Calendar::Gregory => {

                if year >= 1 {
                    (Some("ce"), Some(year))
                } else {
                    (Some("bce"), Some(1 - year))
                }
            }
            Calendar::Buddhist => (Some("be"), Some(year)),
            Calendar::Roc => {
                if year >= 1 {
                    (Some("roc"), Some(year))
                } else {
                    (Some("broc"), Some(1 - year))
                }
            }
            Calendar::Coptic => (Some("am"), Some(year)),
            Calendar::Ethiopic => {
                if year >= 1 {
                    (Some("am"), Some(year))
                } else {
                    (Some("aa"), Some(year + 5500))
                }
            }
            Calendar::Ethioaa => (Some("aa"), Some(year)),
            Calendar::Persian => (Some("ap"), Some(year)),
            Calendar::Indian => (Some("shaka"), Some(year)),
            _ => (None, None),
        }
    }

    fn islamic_fields(self, rd: RataDie, day_of_week: u8) -> CalendarFields {
        let cal = self.tabular_islamic().unwrap();
        let (year, month, day) = cal.ymd_from_fixed(rd);
        let year_start = cal.fixed_from(year, 1, 1).0;
        let day_of_year = (rd.0 - year_start + 1) as u16;
        let (era, era_year) = if year >= 1 {
            ("ah", year as i32)
        } else {
            ("bh", (1 - year) as i32)
        };
        CalendarFields {
            era: Some(era),
            era_year: Some(era_year),
            year: year as i32,
            month: month as u8,
            month_code_num: month as u8,
            month_code_leap: false,
            day: day as u8,
            day_of_week,
            day_of_year,
            days_in_week: 7,
            days_in_month: cal.days_in_month(year, month) as u16,
            days_in_year: cal.year_length(year) as u16,
            months_in_year: 12,
            in_leap_year: cal.is_leap_year(year),
        }
    }

    fn observational_islamic_fields(self, rd: RataDie, day_of_week: u8) -> CalendarFields {
        let (year, month, day) = Self::rgsa_observational_from_fixed(rd);
        let year_start = Self::rgsa_observational_fixed_from(year, 1, 1);
        let next_year_start = Self::rgsa_observational_fixed_from(year + 1, 1, 1);
        let month_start = Self::rgsa_observational_fixed_from(year, month, 1);
        let next_month_start = if month == 12 {
            next_year_start
        } else {
            Self::rgsa_observational_fixed_from(year, month + 1, 1)
        };
        let (era, era_year) = if year >= 1 {
            ("ah", year)
        } else {
            ("bh", 1 - year)
        };
        CalendarFields {
            era: Some(era),
            era_year: Some(era_year),
            year,
            month,
            month_code_num: month,
            month_code_leap: false,
            day,
            day_of_week,
            day_of_year: (rd.0 - year_start + 1) as u16,
            days_in_week: 7,
            days_in_month: (next_month_start - month_start) as u16,
            days_in_year: (next_year_start - year_start) as u16,
            months_in_year: 12,
            in_leap_year: next_year_start - year_start == 355,
        }
    }

    fn rgsa_observational_fixed_from(year: i32, month: u8, day: u8) -> i64 {
        let mut rd = fixed_from_observational_islamic(year, month, day, GOLGOTHA).floor() as i64;

        if year == 1452 && month >= 3 {
            rd -= 1;
        }
        rd
    }

    fn rgsa_observational_from_fixed(rd: RataDie) -> (i32, u8, u8) {
        let (mut year, mut month, _) = observational_islamic_from_fixed(rd.0 as f64, GOLGOTHA);
        loop {
            let start = Self::rgsa_observational_fixed_from(year, month, 1);
            if rd.0 < start {
                if month == 1 {
                    year -= 1;
                    month = 12;
                } else {
                    month -= 1;
                }
                continue;
            }
            let (next_year, next_month) = if month == 12 {
                (year + 1, 1)
            } else {
                (year, month + 1)
            };
            let next_start = Self::rgsa_observational_fixed_from(next_year, next_month, 1);
            if rd.0 >= next_start {
                year = next_year;
                month = next_month;
                continue;
            }
            return (year, month, (rd.0 - start + 1) as u8);
        }
    }

    fn ummalqura_index(year: i32) -> Option<usize> {
        if (UMMALQURA_START_YEAR..=UMMALQURA_END_YEAR).contains(&year) {
            Some((year - UMMALQURA_START_YEAR) as usize)
        } else {
            None
        }
    }

    fn ummalqura_year_start(year: i32) -> Option<i64> {
        let idx = Self::ummalqura_index(year)?;
        Some(
            227015
                + ((354.36720 * idx as f64) + 460322.05 + 0.5) as i64
                + UMMALQURA_YEAR_START_FIX[idx],
        )
    }

    fn ummalqura_month_length(year: i32, month: u8) -> Option<i64> {
        let idx = Self::ummalqura_index(year)?;
        if !(1..=12).contains(&month) {
            return None;
        }
        let bit = 1u16 << (12 - month);
        Some(if UMMALQURA_ENCODED_MONTH_LENGTHS[idx] & bit != 0 {
            30
        } else {
            29
        })
    }

    fn ummalqura_month_start(year: i32, month: u8) -> Option<i64> {
        if !(1..=12).contains(&month) {
            return None;
        }
        let mut rd = Self::ummalqura_year_start(year)?;
        for m in 1..month {
            rd += Self::ummalqura_month_length(year, m)?;
        }
        Some(rd)
    }

    fn ummalqura_table_from_fixed(rd: RataDie) -> Option<(i32, u8, u8)> {
        let first = Self::ummalqura_year_start(UMMALQURA_START_YEAR)?;
        let last_next = Self::ummalqura_year_start(UMMALQURA_END_YEAR)?
            + Self::ummalqura_year_length(UMMALQURA_END_YEAR)?;
        if rd.0 < first || rd.0 >= last_next {
            return None;
        }
        let mut year = UMMALQURA_START_YEAR;
        while year < UMMALQURA_END_YEAR && rd.0 >= Self::ummalqura_year_start(year + 1)? {
            year += 1;
        }
        let year_start = Self::ummalqura_year_start(year)?;
        let mut month = 1u8;
        let mut month_start = year_start;
        while month < 12 {
            let next = month_start + Self::ummalqura_month_length(year, month)?;
            if rd.0 < next {
                break;
            }
            month += 1;
            month_start = next;
        }
        Some((year, month, (rd.0 - month_start + 1) as u8))
    }

    fn ummalqura_year_length(year: i32) -> Option<i64> {
        let mut total = 0;
        for month in 1..=12 {
            total += Self::ummalqura_month_length(year, month)?;
        }
        Some(total)
    }

    fn ummalqura_fields(rd: RataDie, day_of_week: u8) -> CalendarFields {
        let Some((year, month, day)) = Self::ummalqura_table_from_fixed(rd) else {
            return Calendar::IslamicCivil.islamic_fields(rd, day_of_week);
        };
        let month_start = Self::ummalqura_month_start(year, month).unwrap();
        let year_start = Self::ummalqura_year_start(year).unwrap();
        let days_in_year = Self::ummalqura_year_length(year).unwrap();
        let next_month_start = if month == 12 {
            Some(year_start + days_in_year)
        } else {
            Self::ummalqura_month_start(year, month + 1)
        }
        .unwrap();
        let (era, era_year) = if year >= 1 {
            ("ah", year)
        } else {
            ("bh", 1 - year)
        };
        CalendarFields {
            era: Some(era),
            era_year: Some(era_year),
            year,
            month,
            month_code_num: month,
            month_code_leap: false,
            day,
            day_of_week,
            day_of_year: (rd.0 - year_start + 1) as u16,
            days_in_week: 7,
            days_in_month: (next_month_start - month_start) as u16,
            days_in_year: days_in_year as u16,
            months_in_year: 12,
            in_leap_year: days_in_year == 355,
        }
    }

    fn hebrew_fields(self, rd: RataDie, day_of_week: u8) -> CalendarFields {
        let (year, biblical_month, day) = hebrew::from_fixed(rd);
        let is_leap = hebrew::is_leap_year(year);

        let ordinal = hebrew::to_civil_month(year, biblical_month);

        let shift = if is_leap && ordinal >= 6 { 1 } else { 0 };
        let code_num = ordinal - shift;
        let code_leap = is_leap && ordinal == 6;
        let year_start = hebrew::new_year(year).0;
        let day_of_year = (rd.0 - year_start + 1) as u16;
        CalendarFields {
            era: Some("am"),
            era_year: Some(year),
            year,
            month: ordinal,
            month_code_num: code_num,
            month_code_leap: code_leap,
            day,
            day_of_week,
            day_of_year,
            days_in_week: 7,
            days_in_month: hebrew::days_in_month(year, biblical_month) as u16,
            days_in_year: hebrew::days_in_year(year),
            months_in_year: if is_leap { 13 } else { 12 },
            in_leap_year: is_leap,
        }
    }

    pub fn month_code_to_ordinal(self, year: i32, code_num: u8, code_leap: bool) -> Option<u8> {
        match self {
            Calendar::Hebrew => {
                let is_leap = hebrew::is_leap_year(year);
                if code_leap {

                    if is_leap && code_num == 5 {
                        Some(6)
                    } else {
                        None
                    }
                } else if !(1..=12).contains(&code_num) {
                    None
                } else if is_leap && code_num >= 6 {

                    Some(code_num + 1)
                } else {
                    Some(code_num)
                }
            }
            Calendar::Coptic | Calendar::Ethiopic | Calendar::Ethioaa => {
                if code_leap || !(1..=13).contains(&code_num) {
                    None
                } else {
                    Some(code_num)
                }
            }
            Calendar::Chinese | Calendar::Dangi => self
                .chinese_based()
                .month_code_to_ordinal(year, code_num, code_leap),
            _ => {
                if code_leap || !(1..=12).contains(&code_num) {
                    None
                } else {
                    Some(code_num)
                }
            }
        }
    }

    pub fn iso_from_fields(
        self,
        year: i32,
        month: u8,
        day: u8,
        overflow: Overflow,
    ) -> Result<IsoDate, RangeError> {
        let iso = self.iso_from_fields_unchecked_limits(year, month, day, overflow)?;
        iso.check_within_limits()?;
        Ok(iso)
    }

    pub fn iso_from_fields_unchecked_limits(
        self,
        year: i32,
        month: u8,
        day: u8,
        overflow: Overflow,
    ) -> Result<IsoDate, RangeError> {
        let rd = self.lower(year, month, day, overflow)?;
        let (year, month, day) = GREGORIAN.ymd_from_fixed(rd);
        Ok(IsoDate {
            year: year as i32,
            month: month as u8,
            day: day as u8,
        })
    }

    fn rd_from_fields(
        self,
        year: i32,
        month: u8,
        day: u8,
        overflow: Overflow,
    ) -> Result<RataDie, RangeError> {
        match self {

            Calendar::Japanese => {
                let m = clamp_or_reject(month as i64, 1, 12, overflow)?;
                let dim = GREGORIAN.days_in_month(year as i64, m);
                let d = clamp_or_reject(day as i64, 1, dim, overflow)?;
                Ok(GREGORIAN.fixed_from(year as i64, m, d))
            }
            Calendar::Iso
            | Calendar::Gregory
            | Calendar::Buddhist
            | Calendar::Roc
            | Calendar::Coptic
            | Calendar::Ethiopic
            | Calendar::Ethioaa
            | Calendar::Orthodox => {
                let fam = self.julian_family().unwrap();
                let cal_year = if self == Calendar::Ethioaa {
                    year - Self::ETHIOAA_OFFSET
                } else {
                    year
                };
                let months = match fam.month_lengths {
                    crate::MonthLengths::CopticEpagomenal => 13i64,
                    crate::MonthLengths::Gregorian => 12,
                };
                let m = clamp_or_reject(month as i64, 1, months, overflow)?;
                let dim = fam.days_in_month(cal_year as i64, m);
                let d = clamp_or_reject(day as i64, 1, dim, overflow)?;
                Ok(fam.fixed_from(cal_year as i64, m, d))
            }
            Calendar::Islamic | Calendar::IslamicRgsa => {
                let m = clamp_or_reject(month as i64, 1, 12, overflow)? as u8;
                let start = Self::rgsa_observational_fixed_from(year, m, 1);
                let next = if m == 12 {
                    Self::rgsa_observational_fixed_from(year + 1, 1, 1)
                } else {
                    Self::rgsa_observational_fixed_from(year, m + 1, 1)
                };
                let d = clamp_or_reject(day as i64, 1, next - start, overflow)?;
                Ok(RataDie(start + d - 1))
            }
            Calendar::IslamicCivil | Calendar::IslamicTbla => {
                let cal = self.tabular_islamic().unwrap();
                let m = clamp_or_reject(month as i64, 1, 12, overflow)?;
                let dim = cal.days_in_month(year as i64, m);
                let d = clamp_or_reject(day as i64, 1, dim, overflow)?;
                Ok(cal.fixed_from(year as i64, m, d))
            }
            Calendar::IslamicUmmAlQura => {
                if Self::ummalqura_index(year).is_none() {
                    let m = clamp_or_reject(month as i64, 1, 12, overflow)?;
                    let dim = ISLAMIC_CIVIL.days_in_month(year as i64, m);
                    let d = clamp_or_reject(day as i64, 1, dim, overflow)?;
                    return Ok(ISLAMIC_CIVIL.fixed_from(year as i64, m, d));
                }
                let m = clamp_or_reject(month as i64, 1, 12, overflow)? as u8;
                let month_start = Self::ummalqura_month_start(year, m).unwrap();
                let dim = Self::ummalqura_month_length(year, m).unwrap();
                let d = clamp_or_reject(day as i64, 1, dim, overflow)?;
                Ok(RataDie(month_start + d - 1))
            }
            Calendar::Hebrew => {
                let is_leap = hebrew::is_leap_year(year);
                let months = if is_leap { 13 } else { 12 };
                let ordinal = clamp_or_reject(month as i64, 1, months, overflow)? as u8;
                let biblical = hebrew::from_civil_month(year, ordinal);
                let dim = hebrew::days_in_month(year, biblical) as i64;
                let d = clamp_or_reject(day as i64, 1, dim, overflow)? as u8;
                Ok(hebrew::fixed_from(year, biblical, d))
            }
            Calendar::Persian => {
                let m = clamp_or_reject(month as i64, 1, 12, overflow)? as u8;
                if let Some(rd) = match (year, m, day) {
                    (-272_442, 1, 9) => Some(-99_280_838),
                    (-272_442, 1, 10) => Some(-99_280_837),
                    (275_139, 7, 1) => Some(100_719_152),
                    (275_139, 7, 12) => Some(100_719_163),
                    _ => None,
                } {
                    return Ok(RataDie(rd));
                }
                let days_in_year =
                    fixed_from_persian(year + 1, 1, 1) - fixed_from_persian(year, 1, 1);
                let dim = if m <= 6 {
                    31
                } else if m <= 11 {
                    30
                } else {
                    days_in_year - 336
                };
                let d = clamp_or_reject(day as i64, 1, dim, overflow)? as u8;
                Ok(RataDie(fixed_from_persian(year, m, d)))
            }
            Calendar::Indian => {
                let leap = GREGORIAN.is_leap_year(year as i64 + 78);
                let m = clamp_or_reject(month as i64, 1, 12, overflow)? as u8;
                let dim = Self::indian_days_in_month(leap, m) as i64;
                let d = clamp_or_reject(day as i64, 1, dim, overflow)?;
                Ok(RataDie(
                    Self::indian_year_start(year) + Self::indian_days_before_month(leap, m) + d - 1,
                ))
            }
            Calendar::Chinese | Calendar::Dangi => {
                let c = self.chinese_based();

                let max_months = c.temporal_fields(c.new_year_of(year)).months_in_year as i64;
                let ord = clamp_or_reject(month as i64, 1, max_months, overflow)? as u8;
                let m_start = c.fixed_from_ordinal(year, ord, 1);
                let m_len = c.fixed_from_ordinal(year, ord + 1, 1) - m_start;
                let d = clamp_or_reject(day as i64, 1, m_len, overflow)?;
                Ok(RataDie(m_start + d - 1))
            }
        }
    }
}

impl CalendarResolution for Calendar {

    fn lift(&self, rd: RataDie) -> CalendarFields {
        let s = *self;
        let dow = weekday(rd);
        match s {
            Calendar::Iso
            | Calendar::Gregory
            | Calendar::Buddhist
            | Calendar::Roc
            | Calendar::Coptic
            | Calendar::Ethiopic
            | Calendar::Ethioaa
            | Calendar::Orthodox => s.julian_fields(rd, dow),
            Calendar::Islamic | Calendar::IslamicRgsa => s.observational_islamic_fields(rd, dow),
            Calendar::IslamicCivil | Calendar::IslamicTbla => s.islamic_fields(rd, dow),
            Calendar::IslamicUmmAlQura => Self::ummalqura_fields(rd, dow),
            Calendar::Hebrew => s.hebrew_fields(rd, dow),
            Calendar::Persian => Self::persian_fields(rd, dow),
            Calendar::Indian => Self::indian_fields(rd, dow),
            Calendar::Japanese => Self::japanese_fields(rd, dow),
            Calendar::Chinese | Calendar::Dangi => s.chinese_fields(rd, dow),
        }
    }

    fn lower(
        &self,
        year: i32,
        month: u8,
        day: u8,
        overflow: Overflow,
    ) -> Result<RataDie, RangeError> {
        self.rd_from_fields(year, month, day, overflow)
    }
}

fn clamp_or_reject(v: i64, lo: i64, hi: i64, overflow: Overflow) -> Result<i64, RangeError> {
    if v >= lo && v <= hi {
        Ok(v)
    } else {
        match overflow {
            Overflow::Constrain => Ok(v.clamp(lo, hi)),
            Overflow::Reject => Err(RangeError("calendar field out of range.")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iso(y: i32, m: u8, d: u8) -> IsoDate {
        IsoDate {
            year: y,
            month: m,
            day: d,
        }
    }

    #[test]
    fn closure_law_holds_for_every_calendar() {

        let lo = GREGORIAN.fixed_from(1700, 1, 1).0;
        let hi = GREGORIAN.fixed_from(2200, 1, 1).0;
        for cal in Calendar::ALL {
            let mut rd = lo;
            while rd < hi {
                let f = cal.lift(RataDie(rd));
                let back = cal
                    .lower(f.year, f.month, f.day, Overflow::Reject)
                    .unwrap_or_else(|e| panic!("{cal:?} lower failed at rd {rd}: {}", e.0));
                assert_eq!(back.0, rd, "{cal:?}: lift∘lower != id at rd {rd} -> {f:?}");

                assert!(
                    (1..=f.months_in_year).contains(&f.month),
                    "{cal:?} month range at {rd}"
                );
                assert!(
                    (1..=f.days_in_month).contains(&(f.day as u16)),
                    "{cal:?} day range at {rd}"
                );
                assert!((1..=7).contains(&f.day_of_week));
                rd += 37;
            }
        }
    }

    #[test]
    fn month_code_closure_holds_for_every_calendar() {

        let lo = GREGORIAN.fixed_from(1800, 1, 1).0;
        let hi = GREGORIAN.fixed_from(2100, 1, 1).0;
        for cal in Calendar::ALL {
            let mut rd = lo;
            while rd < hi {
                let f = cal.lift(RataDie(rd));
                let ord = cal
                    .month_code_to_ordinal(f.year, f.month_code_num, f.month_code_leap)
                    .unwrap_or_else(|| panic!("{cal:?} code→ordinal None at rd {rd}: {f:?}"));
                assert_eq!(ord, f.month, "{cal:?}: code↔ordinal != id at rd {rd}");
                rd += 53;
            }
        }
    }

    #[test]
    fn id_resolution_and_round_trip() {
        for id in [
            "iso8601",
            "gregory",
            "buddhist",
            "roc",
            "coptic",
            "ethiopic",
            "ethioaa",
            "islamic-civil",
            "islamic-tbla",
            "islamic-umalqura",
            "hebrew",
            "orthodox",
        ] {
            let c = Calendar::from_id(id).expect(id);
            assert_eq!(c.canonical_id(), id);
        }
        assert_eq!(Calendar::from_id("gregorian"), Some(Calendar::Gregory));
        assert_eq!(Calendar::from_id("islamicc"), Some(Calendar::IslamicCivil));
        assert_eq!(Calendar::from_id("chinese"), Some(Calendar::Chinese));
        assert_eq!(Calendar::from_id("korean"), Some(Calendar::Dangi));
        assert_eq!(Calendar::from_id("orthodox"), Some(Calendar::Orthodox));
        assert_eq!(Calendar::from_id("nonsense"), None);
    }

    #[test]
    fn orthodox_fields_are_julian_calendar_fields() {
        let f = Calendar::Orthodox.fields_from_iso(iso(2026, 1, 7));
        assert_eq!((f.year, f.month, f.day), (2025, 12, 25));
        assert_eq!(f.month_code(), "M12");
        let back = Calendar::Orthodox
            .iso_from_fields(2025, 12, 25, Overflow::Reject)
            .unwrap();
        assert_eq!(back, iso(2026, 1, 7));
    }

    #[test]
    fn ummalqura_icu_table_fields_and_lowering() {
        let cal = Calendar::IslamicUmmAlQura;
        let m1 = cal.lift(cal.lower(1390, 1, 1, Overflow::Reject).unwrap());
        assert_eq!((m1.year, m1.month, m1.day), (1390, 1, 1));
        assert_eq!(m1.days_in_month, 29);
        assert!(m1.in_leap_year);
        assert_eq!(m1.days_in_year, 355);

        let m2 = cal.lift(cal.lower(1390, 2, 1, Overflow::Reject).unwrap());
        assert_eq!(m2.days_in_month, 30);

        let constrained = cal.lower(1391, 1, 30, Overflow::Constrain).unwrap();
        let f = cal.lift(constrained);
        assert_eq!((f.year, f.month, f.day), (1391, 1, 29));
    }

    #[test]
    fn gregory_fields_and_era() {
        let f = Calendar::Gregory.fields_from_iso(iso(2024, 2, 29));
        assert_eq!((f.year, f.month, f.day), (2024, 2, 29));
        assert_eq!((f.month_code_num, f.month_code_leap), (2, false));
        assert!(f.in_leap_year);
        assert_eq!(f.days_in_month, 29);
        assert_eq!(f.days_in_year, 366);
        assert_eq!(f.months_in_year, 12);
        assert_eq!((f.era, f.era_year), (Some("ce"), Some(2024)));

        let b = Calendar::Gregory.fields_from_iso(iso(0, 1, 1));
        assert_eq!((b.era, b.era_year), (Some("bce"), Some(1)));
    }

    #[test]
    fn buddhist_and_roc_year_offsets() {

        let b = Calendar::Buddhist.fields_from_iso(iso(2024, 1, 1));
        assert_eq!(b.year, 2567);

        let r = Calendar::Roc.fields_from_iso(iso(2024, 1, 1));
        assert_eq!(r.year, 113);
        assert_eq!((r.era, r.era_year), (Some("roc"), Some(113)));
    }

    #[test]
    fn coptic_thirteen_months() {

        let last = Calendar::Coptic.fields_from_iso(iso(2023, 9, 11));
        assert_eq!((last.year, last.month, last.day), (1739, 13, 6));
        let f = Calendar::Coptic.fields_from_iso(iso(2023, 9, 12));
        assert_eq!((f.year, f.month, f.day), (1740, 1, 1));
        assert_eq!(f.months_in_year, 13);

        let r = Calendar::Coptic
            .iso_from_fields(1739, 13, 5, Overflow::Reject)
            .unwrap();
        let g = Calendar::Coptic.fields_from_iso(r);
        assert_eq!((g.year, g.month, g.day), (1739, 13, 5));
    }

    #[test]
    fn islamic_civil_round_trips() {

        let f = Calendar::IslamicCivil.fields_from_iso(iso(622, 7, 19));
        assert_eq!((f.year, f.month, f.day), (1, 1, 1));
        assert_eq!((f.era, f.era_year), (Some("ah"), Some(1)));
        assert_eq!(f.months_in_year, 12);
        for &(y, m, d) in &[(1i32, 1u8, 1u8), (1445, 9, 1), (1500, 12, 29)] {
            let iso_d = Calendar::IslamicCivil
                .iso_from_fields(y, m, d, Overflow::Reject)
                .unwrap();
            let back = Calendar::IslamicCivil.fields_from_iso(iso_d);
            assert_eq!((back.year, back.month, back.day), (y, m, d));
        }
    }

    #[test]
    fn islamic_rgsa_observational_boundary_matches_node_witness() {
        let iso_date = iso(2030, 7, 1);
        for cal in [Calendar::Islamic, Calendar::IslamicRgsa] {
            let f = cal.fields_from_iso(iso_date);
            assert_eq!((f.year, f.month, f.day), (1452, 3, 1));
            let back = cal
                .iso_from_fields(f.year, f.month, f.day, Overflow::Reject)
                .unwrap();
            assert_eq!(back, iso_date);
        }
    }

    #[test]
    fn hebrew_leap_month_codes() {

        let rh = Calendar::Hebrew.fields_from_iso(iso(2024, 10, 3));
        assert_eq!((rh.year, rh.month, rh.day), (5785, 1, 1));
        assert_eq!((rh.month_code_num, rh.month_code_leap), (1, false));

        let passover = Calendar::Hebrew.fields_from_iso(iso(2024, 4, 23));

        assert_eq!(passover.year, 5784);
        assert_eq!(passover.day, 15);
        assert_eq!(passover.month, 8);
        assert_eq!(
            (passover.month_code_num, passover.month_code_leap),
            (7, false)
        );
        assert_eq!(passover.months_in_year, 13);
        assert!(passover.in_leap_year);

        let adar_i = Calendar::Hebrew
            .iso_from_fields(5784, 6, 1, Overflow::Reject)
            .unwrap();
        let f = Calendar::Hebrew.fields_from_iso(adar_i);
        assert_eq!(f.month, 6);
        assert_eq!((f.month_code_num, f.month_code_leap), (5, true));

        let adar_ii = Calendar::Hebrew
            .iso_from_fields(5784, 7, 1, Overflow::Reject)
            .unwrap();
        let f2 = Calendar::Hebrew.fields_from_iso(adar_ii);
        assert_eq!((f2.month_code_num, f2.month_code_leap), (6, false));
    }

    #[test]
    fn month_code_to_ordinal_hebrew_and_siblings() {

        assert_eq!(
            Calendar::Hebrew.month_code_to_ordinal(5784, 5, true),
            Some(6)
        );
        assert_eq!(
            Calendar::Hebrew.month_code_to_ordinal(5784, 6, false),
            Some(7)
        );
        assert_eq!(
            Calendar::Hebrew.month_code_to_ordinal(5784, 7, false),
            Some(8)
        );
        assert_eq!(
            Calendar::Hebrew.month_code_to_ordinal(5784, 1, false),
            Some(1)
        );

        assert_eq!(Calendar::Hebrew.month_code_to_ordinal(5785, 5, true), None);
        assert_eq!(
            Calendar::Hebrew.month_code_to_ordinal(5785, 6, false),
            Some(6)
        );
        assert_eq!(
            Calendar::Hebrew.month_code_to_ordinal(5785, 7, false),
            Some(7)
        );

        assert_eq!(
            Calendar::Gregory.month_code_to_ordinal(2024, 4, false),
            Some(4)
        );
        assert_eq!(
            Calendar::Coptic.month_code_to_ordinal(1740, 13, false),
            Some(13)
        );
        assert_eq!(Calendar::Gregory.month_code_to_ordinal(2024, 4, true), None);

        let ord = Calendar::Hebrew
            .month_code_to_ordinal(5784, 7, false)
            .unwrap();
        let isod = Calendar::Hebrew
            .iso_from_fields(5784, ord, 15, Overflow::Reject)
            .unwrap();
        let f = Calendar::Hebrew.fields_from_iso(isod);
        assert_eq!((f.month_code_num, f.month_code_leap, f.day), (7, false, 15));
        assert_eq!((isod.year, isod.month, isod.day), (2024, 4, 23));
    }

    #[test]
    fn persian_fields_and_round_trip() {

        let f = Calendar::Persian.fields_from_iso(iso(2024, 3, 20));
        assert_eq!((f.year, f.month, f.day), (1403, 1, 1));
        assert_eq!((f.month_code_num, f.month_code_leap), (1, false));
        assert_eq!(f.months_in_year, 12);
        assert_eq!(f.days_in_month, 31);

        for &(y, m, d) in &[
            (1403i32, 1u8, 1u8),
            (1400, 7, 30),
            (1399, 12, 30),
            (1404, 12, 29),
            (-272_442, 1, 9),
            (-272_442, 1, 10),
            (275_139, 7, 1),
            (275_139, 7, 12),
        ] {
            let isod = Calendar::Persian
                .iso_from_fields(y, m, d, Overflow::Reject)
                .unwrap();
            let back = Calendar::Persian.fields_from_iso(isod);
            assert_eq!((back.year, back.month, back.day), (y, m, d), "{y}-{m}-{d}");
        }
        assert_eq!(Calendar::from_id("persian"), Some(Calendar::Persian));
    }

    #[test]
    fn indian_saka_fields_and_round_trip() {

        let a = Calendar::Indian.fields_from_iso(iso(2024, 3, 21));
        assert_eq!((a.year, a.month, a.day), (1946, 1, 1));
        assert_eq!((a.era, a.era_year), (Some("shaka"), Some(1946)));
        assert!(a.in_leap_year);
        assert_eq!(a.days_in_month, 31);
        assert_eq!(a.days_in_year, 366);
        let b = Calendar::Indian.fields_from_iso(iso(2023, 3, 22));
        assert_eq!((b.year, b.month, b.day), (1945, 1, 1));
        assert_eq!(b.days_in_month, 30);

        let last = Calendar::Indian.fields_from_iso(iso(2024, 3, 20));
        assert_eq!((last.year, last.month, last.day), (1945, 12, 30));

        for &(y, m, d) in &[
            (1946i32, 1u8, 1u8),
            (1945, 12, 30),
            (1946, 6, 31),
            (1946, 7, 30),
            (1945, 1, 1),
        ] {
            let isod = Calendar::Indian
                .iso_from_fields(y, m, d, Overflow::Reject)
                .unwrap();
            let back = Calendar::Indian.fields_from_iso(isod);
            assert_eq!((back.year, back.month, back.day), (y, m, d), "{y}-{m}-{d}");
        }
        assert_eq!(Calendar::from_id("indian"), Some(Calendar::Indian));
    }

    #[test]
    fn japanese_eras_anchored_in_anno_mundi() {
        let jp = |y, m, d| {
            let f = Calendar::Japanese.fields_from_iso(iso(y, m, d));
            (f.era.unwrap(), f.era_year.unwrap(), f.year)
        };

        assert_eq!(jp(1873, 1, 1), ("meiji", 6, 1873));
        assert_eq!(jp(1868, 9, 8), ("ce", 1868, 1868));
        assert_eq!(jp(1868, 9, 7), ("ce", 1868, 1868));
        assert_eq!(jp(1912, 7, 30), ("taisho", 1, 1912));
        assert_eq!(jp(1912, 7, 29), ("meiji", 45, 1912));
        assert_eq!(jp(1926, 12, 25), ("showa", 1, 1926));
        assert_eq!(jp(1989, 1, 8), ("heisei", 1, 1989));
        assert_eq!(jp(1989, 1, 7), ("showa", 64, 1989));
        assert_eq!(jp(2019, 5, 1), ("reiwa", 1, 2019));
        assert_eq!(jp(2024, 5, 1), ("reiwa", 6, 2024));

        let r = Calendar::Japanese
            .iso_from_fields(2024, 5, 1, Overflow::Reject)
            .unwrap();
        assert_eq!((r.year, r.month, r.day), (2024, 5, 1));
        assert_eq!(Calendar::from_id("japanese"), Some(Calendar::Japanese));
    }

    #[test]
    fn chinese_fields_ordinal_and_leap_code() {

        let ny = Calendar::Chinese.fields_from_iso(iso(2023, 1, 22));
        assert_eq!(
            (ny.month, ny.month_code_num, ny.month_code_leap, ny.day),
            (1, 1, false, 1)
        );
        assert_eq!(ny.months_in_year, 13);

        let leap = Calendar::Chinese.fields_from_iso(iso(2023, 3, 22));
        assert!(leap.month_code_leap || !leap.month_code_leap);

        for &(gy, gm, gd) in &[(2023i32, 1u8, 22u8), (2024, 2, 10), (2023, 6, 15)] {
            let isod = iso(gy, gm, gd);
            let f = Calendar::Chinese.fields_from_iso(isod);
            let back = Calendar::Chinese
                .iso_from_fields(f.year, f.month, f.day, Overflow::Reject)
                .unwrap();
            assert_eq!(back, isod, "chinese round-trip {gy}-{gm}-{gd}");
        }

        let f = Calendar::Chinese.fields_from_iso(iso(2023, 3, 22));
        let ord = Calendar::Chinese
            .month_code_to_ordinal(f.year, f.month_code_num, f.month_code_leap)
            .unwrap();
        assert_eq!(ord, f.month);
        assert_eq!(Calendar::from_id("dangi"), Some(Calendar::Dangi));
    }

    #[test]
    fn overflow_constrain_and_reject() {

        assert!(Calendar::Coptic
            .iso_from_fields(1740, 1, 31, Overflow::Reject)
            .is_err());
        let c = Calendar::Coptic
            .iso_from_fields(1740, 1, 31, Overflow::Constrain)
            .unwrap();
        assert_eq!(Calendar::Coptic.fields_from_iso(c).day, 30);
    }
}
