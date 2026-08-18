
#[path = "intl_time_zone_display_names_en_generated.rs"]
mod intl_time_zone_display_names_en_generated;

pub type DatePart = (&'static str, String);
pub type RangePart = (&'static str, String, &'static str);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NamedDateProfile {
    EnUs,
    De,
    Fr,
    Ja,
    DayMonth,
    DayMonthNoComma,
    DayMonthArabic,
    DayMonthBengali,
    DayMonthRussian,
    DayMonthThai,
    DayDotMonth,
    DayDotMonthNoComma,
    DayDeMonth,
    CjkMonthDay,
    KoreanMonthDay,
    Persian,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumericDateOrder {
    MonthDayYear,
    DayMonthYear,
    YearMonthDay,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumericDateProfile {
    pub order: NumericDateOrder,
    pub separator: &'static str,
    pub pad_present_fields: bool,
    pub suffix: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DateStyleComponentWidths {
    pub weekday: Option<&'static str>,
    pub year: &'static str,
    pub month: &'static str,
    pub day: &'static str,
    pub numeric_layout: bool,
    pub numeric_profile_override: Option<NumericDateProfile>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeStyleProfile {
    pub hour_width: &'static str,
    pub separator: &'static str,
    pub day_period_before_hour: bool,
}

#[derive(Clone, Debug)]
pub struct TimePartsInput {
    pub hour_width: Option<String>,

    pub hour_h23_pads_numeric: bool,
    pub minute_present: bool,
    pub second_present: bool,
    pub fractional_second_digits: Option<usize>,
    pub day_period_width: Option<String>,
    pub time_zone_name: Option<String>,
    pub hour12: Option<bool>,
    pub hour_cycle: Option<String>,
    pub default_hour12: bool,
    pub time_separator: String,
    pub day_period_before_hour: bool,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub millisecond: u32,
    pub flexible_day_period: Option<String>,
    pub default_day_period: Option<String>,
}

pub fn hour_h23_pads_numeric(locale: Option<&str>) -> bool {
    let lower = locale.map(|s| s.to_ascii_lowercase()).unwrap_or_default();
    let base = lower.split('-').next().unwrap_or("");
    !matches!(base, "ja" | "fi" | "cs" | "he" | "sk" | "el")
}

pub fn named_date_profile(locale: Option<&str>) -> NamedDateProfile {
    let Some(locale) = locale else {
        return NamedDateProfile::EnUs;
    };
    let lower_full = locale.to_ascii_lowercase();

    let lower = match ["-u-", "-t-", "-x-"]
        .iter()
        .filter_map(|s| lower_full.find(s))
        .min()
    {
        Some(idx) => lower_full[..idx].to_string(),
        None => lower_full,
    };

    if lower == "en-ie" {
        return NamedDateProfile::DayMonthNoComma;
    }
    if lower.starts_with("en-") && !matches!(lower.as_str(), "en-us" | "en-ca" | "en-ph") {
        return NamedDateProfile::DayMonth;
    }
    if lower == "de" || lower.starts_with("de-") {
        NamedDateProfile::De
    } else if lower == "fr" || lower.starts_with("fr-") {
        NamedDateProfile::Fr
    } else if lower == "ja" || lower.starts_with("ja-") {
        NamedDateProfile::Ja
    } else if lower == "zh" || lower.starts_with("zh-") {
        NamedDateProfile::CjkMonthDay
    } else if lower == "ko" || lower.starts_with("ko-") {
        NamedDateProfile::KoreanMonthDay
    } else if matches!(lower.as_str(), "es" | "pt")
        || lower.starts_with("es-")
        || lower.starts_with("pt-")
    {
        NamedDateProfile::DayDeMonth
    } else if matches!(lower.as_str(), "fi" | "cs")
        || lower.starts_with("fi-")
        || lower.starts_with("cs-")
    {
        NamedDateProfile::DayDotMonthNoComma
    } else if lower == "ar" || lower.starts_with("ar-") {
        NamedDateProfile::DayMonthArabic
    } else if lower == "bn" || lower.starts_with("bn-") {
        NamedDateProfile::DayMonthBengali
    } else if lower == "ru" || lower.starts_with("ru-") {
        NamedDateProfile::DayMonthRussian
    } else if lower == "th" || lower.starts_with("th-") {
        NamedDateProfile::DayMonthThai
    } else if lower == "fa" || lower.starts_with("fa-") {
        NamedDateProfile::Persian
    } else if matches!(lower.as_str(), "sv" | "it")
        || lower.starts_with("sv-")
        || lower.starts_with("it-")
    {
        NamedDateProfile::DayMonthNoComma
    } else if matches!(lower.as_str(), "pl" | "hi" | "he")
        || lower.starts_with("pl-")
        || lower.starts_with("hi-")
        || lower.starts_with("he-")
    {
        NamedDateProfile::DayMonth
    } else {
        NamedDateProfile::EnUs
    }
}

pub fn numeric_date_profile(locale: Option<&str>) -> NumericDateProfile {
    let Some(locale) = locale else {
        return NumericDateProfile {
            order: NumericDateOrder::MonthDayYear,
            separator: "/",
            pad_present_fields: false,
            suffix: "",
        };
    };
    let lower = locale.to_ascii_lowercase();
    if matches!(lower.as_str(), "af" | "ha" | "sv")
        || lower.starts_with("af-")
        || lower.starts_with("ha-")
        || lower.starts_with("sv-")
    {
        NumericDateProfile {
            order: NumericDateOrder::YearMonthDay,
            separator: "-",
            pad_present_fields: true,
            suffix: "",
        }
    } else if matches!(lower.as_str(), "ja" | "zh")
        || lower.starts_with("ja-")
        || lower.starts_with("zh-")
    {
        NumericDateProfile {
            order: NumericDateOrder::YearMonthDay,
            separator: "/",
            pad_present_fields: false,
            suffix: "",
        }
    } else if lower == "ko" || lower.starts_with("ko-") {
        NumericDateProfile {
            order: NumericDateOrder::YearMonthDay,
            separator: ". ",
            pad_present_fields: false,
            suffix: ".",
        }
    } else if lower == "fa" || lower.starts_with("fa-") {
        NumericDateProfile {
            order: NumericDateOrder::YearMonthDay,
            separator: "/",
            pad_present_fields: false,
            suffix: "",
        }
    } else if matches!(
        lower.as_str(),
        "fr" | "it" | "pt" | "uz" | "en-gb" | "en-ie"
    ) || lower.starts_with("fr-")
        || lower.starts_with("it-")
        || lower.starts_with("pt-")
        || lower.starts_with("uz-")
    {
        NumericDateProfile {
            order: NumericDateOrder::DayMonthYear,
            separator: "/",
            pad_present_fields: true,
            suffix: "",
        }
    } else if matches!(
        lower.as_str(),
        "am" | "bn" | "ca" | "el" | "es" | "hi" | "id" | "pa" | "th" | "vi"
    ) || lower.starts_with("am-")
        || lower.starts_with("bn-")
        || lower.starts_with("ca-")
        || lower.starts_with("el-")
        || lower.starts_with("es-")
        || lower.starts_with("hi-")
        || lower.starts_with("id-")
        || lower.starts_with("pa-")
        || lower.starts_with("th-")
        || lower.starts_with("vi-")
    {
        NumericDateProfile {
            order: NumericDateOrder::DayMonthYear,
            separator: "/",
            pad_present_fields: false,
            suffix: "",
        }
    } else if lower == "ar" || lower.starts_with("ar-") {
        NumericDateProfile {
            order: NumericDateOrder::DayMonthYear,
            separator: "\u{200F}/",
            pad_present_fields: false,
            suffix: "",
        }
    } else if matches!(lower.as_str(), "az" | "pl")
        || lower.starts_with("az-")
        || lower.starts_with("pl-")
    {
        NumericDateProfile {
            order: NumericDateOrder::DayMonthYear,
            separator: ".",
            pad_present_fields: true,
            suffix: "",
        }
    } else if matches!(lower.as_str(), "bg" | "ku" | "ro" | "ru" | "tr" | "uk")
        || lower.starts_with("bg-")
        || lower.starts_with("ku-")
        || lower.starts_with("ro-")
        || lower.starts_with("ru-")
        || lower.starts_with("tr-")
        || lower.starts_with("uk-")
    {
        NumericDateProfile {
            order: NumericDateOrder::DayMonthYear,
            separator: ".",
            pad_present_fields: true,
            suffix: if lower == "bg" || lower.starts_with("bg-") {
                " г."
            } else {
                ""
            },
        }
    } else if matches!(lower.as_str(), "cs" | "sr")
        || lower.starts_with("cs-")
        || lower.starts_with("sr-")
    {
        NumericDateProfile {
            order: NumericDateOrder::DayMonthYear,
            separator: ". ",
            pad_present_fields: false,
            suffix: if lower == "sr" || lower.starts_with("sr-") {
                "."
            } else {
                ""
            },
        }
    } else if lower == "yi" || lower.starts_with("yi-") {
        NumericDateProfile {
            order: NumericDateOrder::DayMonthYear,
            separator: "-",
            pad_present_fields: false,
            suffix: "",
        }
    } else if matches!(lower.as_str(), "da" | "fi" | "he" | "nb" | "nn" | "no")
        || lower.starts_with("da-")
        || lower.starts_with("fi-")
        || lower.starts_with("he-")
        || lower.starts_with("nb-")
        || lower.starts_with("nn-")
        || lower.starts_with("no-")
    {
        NumericDateProfile {
            order: NumericDateOrder::DayMonthYear,
            separator: ".",
            pad_present_fields: false,
            suffix: "",
        }
    } else if lower == "de" || lower.starts_with("de-") {

        NumericDateProfile {
            order: NumericDateOrder::DayMonthYear,
            separator: ".",
            pad_present_fields: false,
            suffix: "",
        }
    } else {
        NumericDateProfile {
            order: NumericDateOrder::MonthDayYear,
            separator: "/",
            pad_present_fields: false,
            suffix: "",
        }
    }
}

pub fn date_style_component_widths(
    locale: Option<&str>,
    date_style: &str,
) -> DateStyleComponentWidths {
    match date_style {
        "full" => DateStyleComponentWidths {
            weekday: Some("long"),
            year: "numeric",
            month: "long",
            day: "numeric",
            numeric_layout: false,
            numeric_profile_override: None,
        },
        "long" => DateStyleComponentWidths {
            weekday: None,
            year: "numeric",
            month: "long",
            day: "numeric",
            numeric_layout: false,
            numeric_profile_override: None,
        },
        "medium" => DateStyleComponentWidths {
            weekday: None,
            year: "numeric",
            month: "short",
            day: "numeric",
            numeric_layout: false,
            numeric_profile_override: None,
        },
        _ => {
            let locale_lower = locale.unwrap_or("en-US").to_ascii_lowercase();
            let year_width = if short_date_style_uses_full_year(&locale_lower) {
                "numeric"
            } else {
                "2-digit"
            };
            let (month_width, day_width) = short_date_style_day_month_widths(&locale_lower);
            DateStyleComponentWidths {
                weekday: None,
                year: year_width,
                month: month_width,
                day: day_width,
                numeric_layout: true,
                numeric_profile_override: short_date_style_numeric_profile(&locale_lower),
            }
        }
    }
}

pub fn time_style_profile(locale: Option<&str>, time_style: &str) -> TimeStyleProfile {
    let lower = locale.map(|s| s.to_ascii_lowercase()).unwrap_or_default();
    let short = time_style == "short";

    let hour_width = if short_time_style_uses_numeric_hour(&lower) {
        "numeric"
    } else {
        "2-digit"
    };
    TimeStyleProfile {
        hour_width,
        separator: if short && (lower == "fi" || lower.starts_with("fi-")) {
            "."
        } else {
            ":"
        },
        day_period_before_hour: short && (lower == "ko" || lower.starts_with("ko-")),
    }
}

pub fn default_hour12_for_locale(locale: Option<&str>) -> bool {
    match locale {
        Some("de") | Some("de-AT") | Some("de-DE") | Some("en-GB") | Some("fr") | Some("fr-FR")
        | Some("pl") | Some("pl-PL") | Some("ja") | Some("ja-JP") | Some("zh") | Some("zh-CN")
        | Some("es") | Some("es-ES") | Some("pt") | Some("pt-BR") | Some("ru") | Some("ru-RU")
        | Some("fa") | Some("fa-IR") | Some("th") | Some("th-TH") | Some("he") | Some("he-IL")
        | Some("sv") | Some("sv-SE") | Some("fi") | Some("fi-FI") | Some("cs") | Some("cs-CZ")
        | Some("it") | Some("it-IT") => false,
        _ => true,
    }
}

fn short_time_style_uses_numeric_hour(locale_lower: &str) -> bool {
    matches!(
        locale_lower,
        "en" | "ar" | "ja" | "ko" | "es" | "hi" | "bn" | "fa" | "he" | "fi" | "cs"
    ) || locale_lower.starts_with("en-")
        || locale_lower.starts_with("ar-")
        || locale_lower.starts_with("ja-")
        || locale_lower.starts_with("ko-")
        || locale_lower.starts_with("es-")
        || locale_lower.starts_with("hi-")
        || locale_lower.starts_with("bn-")
        || locale_lower.starts_with("fa-")
        || locale_lower.starts_with("he-")
        || locale_lower.starts_with("fi-")
        || locale_lower.starts_with("cs-")
}

fn short_date_style_uses_full_year(locale_lower: &str) -> bool {
    matches!(
        locale_lower,
        "ar" | "fa" | "fr" | "he" | "fi" | "ja" | "pl" | "pt" | "ru" | "sv" | "zh"
    ) || locale_lower.starts_with("ar-")
        || locale_lower.starts_with("fa-")
        || locale_lower.starts_with("fr-")
        || locale_lower.starts_with("he-")
        || locale_lower.starts_with("fi-")
        || locale_lower.starts_with("ja-")
        || locale_lower.starts_with("pl-")
        || locale_lower.starts_with("pt-")
        || locale_lower.starts_with("ru-")
        || locale_lower.starts_with("sv-")
        || locale_lower.starts_with("zh-")
}

fn short_date_style_day_month_widths(locale_lower: &str) -> (&'static str, &'static str) {
    if locale_lower == "pl" || locale_lower.starts_with("pl-") {
        ("2-digit", "numeric")
    } else if matches!(locale_lower, "cs" | "de" | "fr" | "ja" | "pt" | "ru" | "sv")
        || locale_lower.starts_with("cs-")
        || locale_lower.starts_with("de-")
        || locale_lower.starts_with("fr-")
        || locale_lower.starts_with("ja-")
        || locale_lower.starts_with("pt-")
        || locale_lower.starts_with("ru-")
        || locale_lower.starts_with("sv-")
    {
        ("2-digit", "2-digit")
    } else {
        ("numeric", "numeric")
    }
}

fn short_date_style_numeric_profile(locale_lower: &str) -> Option<NumericDateProfile> {
    if locale_lower == "pl" || locale_lower.starts_with("pl-") {
        Some(NumericDateProfile {
            order: NumericDateOrder::DayMonthYear,
            separator: ".",
            pad_present_fields: false,
            suffix: "",
        })
    } else if locale_lower == "cs" || locale_lower.starts_with("cs-") {
        Some(NumericDateProfile {
            order: NumericDateOrder::DayMonthYear,
            separator: ".",
            pad_present_fields: false,
            suffix: "",
        })
    } else {
        None
    }
}

pub fn date_time_connector(
    date_style: Option<&str>,
    date_numeric: bool,
    month_width: Option<&str>,
) -> &'static str {
    match date_style {
        Some("full") | Some("long") => " at ",
        Some("medium") => ", ",
        None if !date_numeric && month_width == Some("long") => " at ",
        _ => ", ",
    }
}

pub fn long_time_zone_display_name(
    primary_identifier: &str,
    epoch_ms: i64,
    daylight: bool,
) -> Option<&'static str> {
    intl_time_zone_display_names_en_generated::long_display_name(
        primary_identifier,
        epoch_ms,
        daylight,
    )
}

pub fn era_part_value(
    calendar: &str,
    iso_year: i32,
    raw_era_value: &str,
    width: Option<&str>,
) -> String {
    if matches!(calendar, "gregory" | "iso8601") {
        match width {
            Some("long") => {
                if iso_year <= 0 {
                    "Before Christ".to_string()
                } else {
                    "Anno Domini".to_string()
                }
            }
            Some("short") => {
                if iso_year <= 0 {
                    "BC".to_string()
                } else {
                    "AD".to_string()
                }
            }
            Some("narrow") => {
                if iso_year <= 0 {
                    "B".to_string()
                } else {
                    "A".to_string()
                }
            }
            _ => raw_era_value.to_string(),
        }
    } else {
        raw_era_value.to_string()
    }
}

pub fn emit_numeric_date_parts(
    profile: NumericDateProfile,
    weekday_present: bool,
    year_part: Option<DatePart>,
    month_part_mdy: Option<String>,
    month_part_numeric: Option<String>,
    day_part: Option<String>,
    era_part: Option<String>,
) -> Vec<DatePart> {
    let mut seg: Vec<DatePart> = Vec::new();
    match profile.order {
        NumericDateOrder::YearMonthDay => {
            if let Some(part) = year_part.clone() {
                seg.push(part);
            }
            if let Some(month) = month_part_numeric.clone() {
                seg.push(("month", month));
            }
            if let Some(day) = day_part.clone() {
                seg.push(("day", day));
            }
        }
        NumericDateOrder::DayMonthYear => {
            if let Some(day) = day_part.clone() {
                seg.push(("day", day));
            }
            if let Some(month) = month_part_numeric.clone() {
                seg.push(("month", month));
            }
            if let Some(part) = year_part.clone() {
                seg.push(part);
            }
        }
        NumericDateOrder::MonthDayYear => {
            if let Some(month) = month_part_mdy {
                seg.push(("month", month));
            }
            if let Some(day) = day_part {
                seg.push(("day", day));
            }
            if let Some(part) = year_part {
                seg.push(part);
            }
        }
    }

    let mut out = Vec::new();
    let mut first = true;
    for (i, (ty, value)) in seg.into_iter().enumerate() {
        if i == 0 && weekday_present {
            out.push(("literal", ", ".into()));
        }
        if !first {
            out.push(("literal", profile.separator.into()));
        }
        first = false;
        out.push((ty, value));
    }
    if let Some(era) = era_part {
        if !out.is_empty() {
            out.push(("literal", " ".into()));
        }
        out.push(("era", era));
    } else if !profile.suffix.is_empty() && !out.is_empty() {
        out.push(("literal", profile.suffix.into()));
    }
    out
}

pub fn emit_named_date_parts(
    profile: NamedDateProfile,
    weekday_part: Option<String>,
    month_part: Option<String>,
    month_width: Option<&str>,
    day_part: Option<String>,
    year_part: Option<DatePart>,
    era_part: Option<String>,
) -> Vec<DatePart> {
    let mut out = Vec::new();
    match profile {
        NamedDateProfile::Ja => {
            if let Some(part) = year_part {
                out.push(part);
                out.push(("literal", "年".into()));
            }
            if let Some(month) = month_part {
                out.push(("month", month));
            }
            if let Some(day) = day_part {
                out.push(("day", day));
                out.push(("literal", "日".into()));
            }
            if let Some(weekday) = weekday_part {
                out.push(("weekday", weekday));
            }
            if let Some(era) = era_part {
                if !out.is_empty() {
                    out.push(("literal", " ".into()));
                }
                out.push(("era", era));
            }
        }
        NamedDateProfile::Fr => {
            if let Some(weekday) = weekday_part {
                out.push(("weekday", weekday));
                out.push(("literal", " ".into()));
            }
            if let Some(day) = day_part {
                out.push(("day", day));
                out.push(("literal", " ".into()));
            }
            if let Some(month) = month_part {
                out.push(("month", month));
                if year_part.is_some() || era_part.is_some() {
                    out.push(("literal", " ".into()));
                }
            }
            if let Some(part) = year_part {
                out.push(part);
            }
            if let Some(era) = era_part {
                if !out.is_empty() {
                    out.push(("literal", " ".into()));
                }
                out.push(("era", era));
            }
        }
        NamedDateProfile::DayMonth
        | NamedDateProfile::DayMonthNoComma
        | NamedDateProfile::DayMonthArabic
        | NamedDateProfile::DayMonthBengali
        | NamedDateProfile::DayMonthRussian
        | NamedDateProfile::DayMonthThai
        | NamedDateProfile::DayDotMonth
        | NamedDateProfile::DayDotMonthNoComma
        | NamedDateProfile::DayDeMonth
        | NamedDateProfile::CjkMonthDay
        | NamedDateProfile::KoreanMonthDay
        | NamedDateProfile::Persian => {
            let has_day = day_part.is_some();
            if matches!(profile, NamedDateProfile::Persian) {
                let had_year = year_part.is_some();
                let mut trailing_weekday = None;
                if let Some(part) = year_part {
                    out.push(part);
                    if month_part.is_some() || day_part.is_some() {
                        out.push(("literal", " ".into()));
                    }
                    trailing_weekday = weekday_part;
                } else if let Some(weekday) = weekday_part {
                    out.push(("weekday", weekday));
                    if month_part.is_some() || day_part.is_some() {
                        out.push(("literal", "، ".into()));
                    }
                }
                if had_year {
                    if let Some(month) = month_part {
                        out.push(("month", month));
                        if day_part.is_some() {
                            out.push(("literal", " ".into()));
                        }
                    }
                    if let Some(day) = day_part {
                        out.push(("day", day));
                    }
                } else {
                    if let Some(day) = day_part {
                        out.push(("day", day));
                        if month_part.is_some() {
                            out.push(("literal", " ".into()));
                        }
                    }
                    if let Some(month) = month_part {
                        out.push(("month", month));
                    }
                }
                if let Some(weekday) = trailing_weekday {
                    if !out.is_empty() {
                        out.push(("literal", ", ".into()));
                    }
                    out.push(("weekday", weekday));
                }
                if let Some(era) = era_part {
                    if !out.is_empty() {
                        out.push(("literal", " ".into()));
                    }
                    out.push(("era", era));
                }
                return out;
            }
            if matches!(
                profile,
                NamedDateProfile::CjkMonthDay | NamedDateProfile::KoreanMonthDay
            ) {
                let weekday_after_date = weekday_part.is_some() && year_part.is_some();
                if let Some(part) = year_part {
                    out.push(part);
                    out.push((
                        "literal",
                        if matches!(profile, NamedDateProfile::KoreanMonthDay) {
                            "년 ".into()
                        } else {
                            "年".into()
                        },
                    ));
                }
                if !weekday_after_date {
                    if let Some(weekday) = weekday_part.clone() {
                        out.push(("weekday", weekday));
                        out.push(("literal", " ".into()));
                    }
                }
                if let Some(month) = month_part {
                    out.push(("month", month));
                }
                if let Some(day) = day_part {
                    if matches!(profile, NamedDateProfile::KoreanMonthDay) {
                        out.push(("literal", " ".into()));
                    }
                    out.push(("day", day));
                    out.push((
                        "literal",
                        if matches!(profile, NamedDateProfile::KoreanMonthDay) {
                            "일".into()
                        } else {
                            "日".into()
                        },
                    ));
                }
                if weekday_after_date {
                    if let Some(weekday) = weekday_part {
                        if matches!(profile, NamedDateProfile::KoreanMonthDay) {
                            out.push(("literal", " ".into()));
                        }
                        out.push(("weekday", weekday));
                    }
                }
                if let Some(era) = era_part {
                    if !out.is_empty() {
                        out.push(("literal", " ".into()));
                    }
                    out.push(("era", era));
                }
                return out;
            }
            if let Some(weekday) = weekday_part {
                out.push(("weekday", weekday));
                let weekday_literal = match profile {
                    NamedDateProfile::DayMonthNoComma | NamedDateProfile::DayDotMonthNoComma => " ",
                    NamedDateProfile::DayMonthThai => "ที่ ",
                    NamedDateProfile::DayMonthArabic => "، ",
                    _ => ", ",
                };
                out.push(("literal", weekday_literal.into()));
            }
            let trailing_year = if let Some(part) = year_part {
                if matches!(profile, NamedDateProfile::CjkMonthDay) {
                    out.push(part);
                    out.push(("literal", "年".into()));
                    None
                } else if matches!(profile, NamedDateProfile::KoreanMonthDay) {
                    out.push(part);
                    out.push(("literal", "년 ".into()));
                    None
                } else {
                    Some(part)
                }
            } else {
                None
            };
            if let Some(day) = day_part {
                out.push(("day", day));
                out.push((
                    "literal",
                    match profile {
                        NamedDateProfile::DayDotMonth | NamedDateProfile::DayDotMonthNoComma => {
                            ". ".into()
                        }
                        NamedDateProfile::DayDeMonth => " de ".into(),
                        NamedDateProfile::CjkMonthDay | NamedDateProfile::KoreanMonthDay => {
                            "".into()
                        }
                        _ => " ".into(),
                    },
                ));
            }
            if let Some(month) = month_part {
                out.push(("month", month));
            }
            if matches!(profile, NamedDateProfile::DayMonthBengali) && trailing_year.is_some() {
                out.push(("literal", ", ".into()));
            }
            if matches!(profile, NamedDateProfile::CjkMonthDay) && has_day {
                out.push(("literal", "日".into()));
            } else if matches!(profile, NamedDateProfile::KoreanMonthDay) && has_day {
                out.push(("literal", "일".into()));
            }
            if let Some(part) = trailing_year {
                let year_literal = match profile {
                    NamedDateProfile::DayDeMonth => " de ",
                    NamedDateProfile::DayMonthBengali => "",
                    NamedDateProfile::DayMonthThai => " พ.ศ. ",
                    _ => " ",
                };
                out.push(("literal", year_literal.into()));
                out.push(part);
                if matches!(profile, NamedDateProfile::DayMonthRussian) {
                    out.push(("literal", " г.".into()));
                }
            }
            if let Some(era) = era_part {
                if !out.is_empty() {
                    let era_literal = match profile {
                        NamedDateProfile::DayMonthThai => " พ.ศ. ",
                        _ => " ",
                    };
                    out.push(("literal", era_literal.into()));
                }
                out.push(("era", era));
            }
        }
        NamedDateProfile::De => {
            if let Some(weekday) = weekday_part {
                out.push(("weekday", weekday));
                out.push(("literal", ", ".into()));
            }
            if let Some(day) = day_part {
                out.push(("day", day));
                out.push(("literal", ". ".into()));
            }
            if let Some(month) = month_part {
                out.push(("month", month));
                if year_part.is_some() || era_part.is_some() {
                    if matches!(month_width, Some("short" | "narrow")) {
                        out.push(("literal", ". ".into()));
                    } else {
                        out.push(("literal", " ".into()));
                    }
                }
            }
            if let Some(part) = year_part {
                out.push(part);
            }
            if let Some(era) = era_part {
                if !out.is_empty() {
                    out.push(("literal", " ".into()));
                }
                out.push(("era", era));
            }
        }
        NamedDateProfile::EnUs => {

            let has_day = day_part.is_some();
            let weekday_at_front = has_day;
            if weekday_at_front {
                if let Some(weekday) = weekday_part.clone() {
                    out.push(("weekday", weekday));
                    out.push(("literal", ", ".into()));
                }
            }
            if let Some(month) = month_part {
                out.push(("month", month));
            }
            if let Some(day) = day_part {
                out.push(("literal", " ".into()));
                out.push(("day", day));
            }
            if let Some(part) = year_part {
                if !out.is_empty() {
                    out.push(("literal", if has_day { ", " } else { " " }.into()));
                }
                out.push(part);
            }
            if let Some(era) = era_part {
                if !out.is_empty() {
                    out.push(("literal", " ".into()));
                }
                out.push(("era", era));
            }
            if !weekday_at_front {
                if let Some(weekday) = weekday_part {
                    if !out.is_empty() {
                        out.push(("literal", " ".into()));
                    }
                    out.push(("weekday", weekday));
                }
            }
        }
    }
    out
}

pub fn emit_time_parts(input: TimePartsInput) -> Vec<DatePart> {
    let mut out = Vec::new();
    let has_time_fields = input.hour_width.is_some()
        || input.minute_present
        || input.second_present
        || input.fractional_second_digits.is_some()
        || input.day_period_width.is_some()
        || input.time_zone_name.is_some();
    if !has_time_fields {
        return out;
    }

    let cycle = resolve_hour_cycle(
        input.hour12,
        input.hour_cycle.as_deref(),
        input.default_hour12,
    );
    let day_period = if input.day_period_width.is_some() {
        input.flexible_day_period.clone()
    } else if input.hour_width.is_some() && matches!(cycle, "h11" | "h12") {
        input.default_day_period.clone()
    } else {
        None
    };
    if input.day_period_before_hour {
        if let Some(day_period) = day_period.clone() {
            out.push(("dayPeriod", day_period));
            if input.hour_width.is_some() {
                out.push(("literal", " ".into()));
            }
        }
    }
    if input.hour_width.is_some() {
        let effective_width = if matches!(cycle, "h23" | "h24") && input.hour_h23_pads_numeric {
            Some("2-digit")
        } else {
            input.hour_width.as_deref()
        };
        out.push(("hour", hour_part_value(cycle, effective_width, input.hour)));
    }
    if input.minute_present {
        if input.hour_width.is_some() {
            out.push(("literal", input.time_separator.clone()));
        }
        out.push(("minute", two(input.minute)));
    }
    if input.second_present {
        if input.hour_width.is_some() || input.minute_present {
            out.push(("literal", input.time_separator.clone()));
        }
        out.push(("second", two(input.second)));
    }
    if let Some(digits) = input.fractional_second_digits {
        if input.second_present {
            out.push(("literal", ".".into()));
        }
        let frac = format!("{:03}", input.millisecond);
        out.push(("fractionalSecond", frac[..digits].to_string()));
    }
    if !input.day_period_before_hour && input.day_period_width.is_some() {
        if input.hour_width.is_some() {
            out.push(("literal", " ".into()));
        }
        if let Some(day_period) = day_period {
            out.push(("dayPeriod", day_period));
        }
    } else if !input.day_period_before_hour
        && input.hour_width.is_some()
        && matches!(cycle, "h11" | "h12")
    {
        if let Some(day_period) = day_period {
            out.push(("literal", " ".into()));
            out.push(("dayPeriod", day_period));
        }
    }
    if let Some(time_zone_name) = input.time_zone_name {
        if !out.is_empty() {
            out.push(("literal", " ".into()));
        }
        out.push(("timeZoneName", time_zone_name));
    }
    out
}

pub fn apply_numbering_system<F>(
    parts: &mut Vec<DatePart>,
    numbering_system: Option<&str>,
    mut replace_digits: F,
) where
    F: FnMut(&str, &str) -> String,
{
    let Some(numbering_system) = numbering_system else {
        return;
    };
    if numbering_system == "latn" {
        return;
    }
    for (ty, value) in parts.iter_mut() {
        if matches!(
            *ty,
            "year" | "month" | "day" | "hour" | "minute" | "second" | "fractionalSecond"
        ) {
            *value = replace_digits(value, numbering_system);
        } else if *ty == "literal" && numbering_system == "arab" && value == "." {
            *value = "\u{066B}".into();
        }
    }
}

pub fn join_date_time_parts(
    date_parts: Vec<DatePart>,
    time_parts: Vec<DatePart>,
    date_style: Option<&str>,
    date_numeric: bool,
    month_width: Option<&str>,
) -> Vec<DatePart> {
    let has_date = !date_parts.is_empty();
    let has_time = !time_parts.is_empty();
    let mut parts = date_parts;
    if has_date && has_time {
        parts.push((
            "literal",
            date_time_connector(date_style, date_numeric, month_width).into(),
        ));
    }
    parts.extend(time_parts);
    parts
}

fn resolve_hour_cycle(
    hour12: Option<bool>,
    hour_cycle: Option<&str>,
    default_hour12: bool,
) -> &'static str {
    match (hour12, hour_cycle) {
        (Some(true), Some("h11")) => "h11",
        (Some(true), Some("h12")) => "h12",
        (Some(true), _) => "h12",
        (Some(false), Some("h23")) => "h23",
        (Some(false), Some("h24")) => "h24",
        (Some(false), _) => "h23",
        (None, Some("h11")) => "h11",
        (None, Some("h12")) => "h12",
        (None, Some("h23")) => "h23",
        (None, Some("h24")) => "h24",
        (None, _) if default_hour12 => "h12",
        (None, _) => "h23",
    }
}

fn hour_part_value(cycle: &str, hour_width: Option<&str>, hour: u32) -> String {
    let h = match cycle {
        "h11" => hour % 12,
        "h12" => {
            let x = hour % 12;
            if x == 0 {
                12
            } else {
                x
            }
        }
        "h24" => {
            if hour == 0 {
                24
            } else {
                hour
            }
        }
        _ => hour,
    };
    if hour_width == Some("2-digit") {
        two(h)
    } else {
        h.to_string()
    }
}

fn two(v: u32) -> String {
    format!("{:02}", v)
}

pub fn range_parts(start_parts: Vec<DatePart>, end_parts: Vec<DatePart>) -> Vec<RangePart> {
    if start_parts == end_parts {
        return start_parts
            .into_iter()
            .map(|(ty, value)| (ty, value, "shared"))
            .collect();
    }

    if let Some(compacted) = compact_time_range(&start_parts, &end_parts) {
        return compacted;
    }
    if let Some(compacted) = compact_same_date_time(&start_parts, &end_parts) {
        return compacted;
    }
    if let Some(compacted) = compact_selected_japanese_range_skeleton(&start_parts, &end_parts) {
        return compacted;
    }
    if let Some(compacted) = compact_selected_full_date_range(&start_parts, &end_parts) {
        return compacted;
    }
    if let Some(compacted) = compact_selected_numeric_date_range(&start_parts, &end_parts) {
        return compacted;
    }
    if let Some(compacted) = compact_named_date_range(&start_parts, &end_parts) {
        return compacted;
    }

    let mut combined = Vec::new();
    for (ty, value) in start_parts {
        combined.push((ty, range_literal(value), "startRange"));
    }
    combined.push(("literal", "\u{2009}\u{2013}\u{2009}".into(), "shared"));
    for (ty, value) in end_parts {
        combined.push((ty, range_literal(value), "endRange"));
    }
    combined
}

fn range_literal(value: String) -> String {
    if value == " at " {
        ", ".into()
    } else {
        value
    }
}

fn split_shared_day_period_tail(parts: &[DatePart]) -> (&[DatePart], Option<DatePart>) {
    if parts.len() >= 2
        && parts[parts.len() - 2] == ("literal", " ".to_string())
        && parts[parts.len() - 1].0 == "dayPeriod"
    {
        (
            &parts[..parts.len() - 2],
            Some(parts[parts.len() - 1].clone()),
        )
    } else {
        (parts, None)
    }
}

fn compact_time_range(start_parts: &[DatePart], end_parts: &[DatePart]) -> Option<Vec<RangePart>> {
    let time_only = |parts: &[DatePart]| {
        !parts.is_empty()
            && parts
                .iter()
                .any(|(ty, _)| matches!(*ty, "hour" | "minute" | "second" | "fractionalSecond"))
            && parts.iter().all(|(ty, _)| {
                matches!(
                    *ty,
                    "hour" | "minute" | "second" | "fractionalSecond" | "dayPeriod" | "literal"
                )
            })
    };
    if !time_only(start_parts) || !time_only(end_parts) {
        return None;
    }
    let (start_main_split, start_period) = split_shared_day_period_tail(start_parts);
    let (end_main_split, end_period) = split_shared_day_period_tail(end_parts);
    let share_period = start_period.is_some() && start_period == end_period;

    let (start_main, end_main): (&[DatePart], &[DatePart]) = if share_period {
        (start_main_split, end_main_split)
    } else {
        (start_parts, end_parts)
    };
    let mut out = Vec::new();
    for (ty, value) in start_main {
        out.push((*ty, value.clone(), "startRange"));
    }
    out.push(("literal", "\u{2009}\u{2013}\u{2009}".into(), "shared"));
    for (ty, value) in end_main {
        out.push((*ty, value.clone(), "endRange"));
    }
    if share_period {
        out.push(("literal", " ".into(), "shared"));
        if let Some((ty, value)) = end_period {
            out.push((ty, value, "shared"));
        }
    }
    Some(out)
}

fn compact_same_date_time(
    start_parts: &[DatePart],
    end_parts: &[DatePart],
) -> Option<Vec<RangePart>> {
    let split = start_parts
        .iter()
        .zip(end_parts.iter())
        .position(|(a, b)| a != b)?;
    if split == 0 || start_parts[..split] != end_parts[..split] {
        return None;
    }
    let prefix_has_date = start_parts[..split]
        .iter()
        .any(|(ty, _)| matches!(*ty, "year" | "month" | "day"));
    let prefix_has_day = start_parts[..split].iter().any(|(ty, _)| *ty == "day");
    if !prefix_has_date || !prefix_has_day {
        return None;
    }
    let start_tail = &start_parts[split..];
    let end_tail = &end_parts[split..];
    let tail_has_time = |parts: &[DatePart]| {
        parts
            .iter()
            .any(|(ty, _)| matches!(*ty, "hour" | "minute" | "second" | "fractionalSecond"))
    };
    if !tail_has_time(start_tail) || !tail_has_time(end_tail) {
        return None;
    }
    let mut out = Vec::new();
    for (ty, value) in &start_parts[..split] {
        out.push((*ty, range_literal(value.clone()), "shared"));
    }
    let (start_main_split, start_period) = split_shared_day_period_tail(start_tail);
    let (end_main_split, end_period) = split_shared_day_period_tail(end_tail);
    let share_period = start_period.is_some() && start_period == end_period;

    let (start_main, end_main): (&[DatePart], &[DatePart]) = if share_period {
        (start_main_split, end_main_split)
    } else {
        (start_tail, end_tail)
    };
    for (ty, value) in start_main {
        out.push((*ty, value.clone(), "startRange"));
    }
    out.push(("literal", "\u{2009}\u{2013}\u{2009}".into(), "shared"));
    for (ty, value) in end_main {
        out.push((*ty, value.clone(), "endRange"));
    }
    if share_period {
        out.push(("literal", " ".into(), "shared"));
        if let Some((ty, value)) = end_period {
            out.push((ty, value, "shared"));
        }
    }
    Some(out)
}

fn compact_selected_japanese_range_skeleton(
    start_parts: &[DatePart],
    end_parts: &[DatePart],
) -> Option<Vec<RangePart>> {
    let month_day = |parts: &[DatePart]| -> Option<(String, String)> {
        if parts.len() == 3
            && parts[0].0 == "month"
            && parts[1].0 == "day"
            && parts[2] == ("literal", "日".to_string())
        {
            Some((
                strip_suffix(&parts[0].1, "月").map(two_ascii)?,
                two_ascii(parts[1].1.as_str()),
            ))
        } else {
            None
        }
    };
    if let (Some((sm, sd)), Some((em, ed))) = (month_day(start_parts), month_day(end_parts)) {
        return Some(vec![
            ("month", sm, "startRange"),
            ("literal", "/".into(), "startRange"),
            ("day", sd, "startRange"),
            ("literal", "\u{ff5e}".into(), "shared"),
            ("month", em, "endRange"),
            ("literal", "/".into(), "endRange"),
            ("day", ed, "endRange"),
        ]);
    }

    let full_date = |parts: &[DatePart]| -> Option<(String, String, String, String)> {
        if parts.len() == 6
            && parts[0].0 == "year"
            && parts[1] == ("literal", "年".to_string())
            && parts[2].0 == "month"
            && parts[3].0 == "day"
            && parts[4] == ("literal", "日".to_string())
            && parts[5].0 == "weekday"
        {
            Some((
                parts[0].1.clone(),
                strip_suffix(&parts[2].1, "月").map(two_ascii)?,
                two_ascii(parts[3].1.as_str()),
                parts[5].1.clone(),
            ))
        } else {
            None
        }
    };
    let (sy, sm, sd, sw) = full_date(start_parts)?;
    let (ey, em, ed, ew) = full_date(end_parts)?;
    Some(vec![
        ("year", sy, "startRange"),
        ("literal", "/".into(), "startRange"),
        ("month", sm, "startRange"),
        ("literal", "/".into(), "startRange"),
        ("day", sd, "startRange"),
        ("literal", "(".into(), "startRange"),
        ("weekday", sw, "startRange"),
        ("literal", ")\u{ff5e}".into(), "shared"),
        ("year", ey, "endRange"),
        ("literal", "/".into(), "endRange"),
        ("month", em, "endRange"),
        ("literal", "/".into(), "endRange"),
        ("day", ed, "endRange"),
        ("literal", "(".into(), "endRange"),
        ("weekday", ew, "endRange"),
        ("literal", ")".into(), "shared"),
    ])
}

fn strip_suffix<'a>(value: &'a str, suffix: &str) -> Option<&'a str> {
    value.strip_suffix(suffix)
}

fn two_ascii(value: &str) -> String {
    if value.len() == 1 && value.as_bytes()[0].is_ascii_digit() {
        format!("0{}", value)
    } else {
        value.to_string()
    }
}

fn compact_selected_full_date_range(
    start_parts: &[DatePart],
    end_parts: &[DatePart],
) -> Option<Vec<RangePart>> {
    let weekday_month_day_year = |parts: &[DatePart]| -> Option<(String, String, String, String)> {
        if parts.len() == 7
            && parts[0].0 == "weekday"
            && parts[1] == ("literal", ", ".to_string())
            && parts[2].0 == "month"
            && parts[3] == ("literal", " ".to_string())
            && parts[4].0 == "day"
            && parts[5] == ("literal", ", ".to_string())
            && parts[6].0 == "year"
        {
            Some((
                parts[0].1.clone(),
                parts[2].1.clone(),
                parts[4].1.clone(),
                parts[6].1.clone(),
            ))
        } else {
            None
        }
    };
    if let (Some((sw, sm, sd, sy)), Some((ew, em, ed, ey))) = (
        weekday_month_day_year(start_parts),
        weekday_month_day_year(end_parts),
    ) {
        if sm == em && sy == ey {
            return Some(vec![
                ("weekday", sw, "startRange"),
                ("literal", ", ".into(), "startRange"),
                ("month", sm, "startRange"),
                ("literal", " ".into(), "startRange"),
                ("day", sd, "startRange"),
                ("literal", "\u{2009}\u{2013}\u{2009}".into(), "shared"),
                ("weekday", ew, "endRange"),
                ("literal", ", ".into(), "endRange"),
                ("month", em, "endRange"),
                ("literal", " ".into(), "endRange"),
                ("day", ed, "endRange"),
                ("literal", ", ".into(), "shared"),
                ("year", sy, "shared"),
            ]);
        }
    }

    let weekday_day_month_year =
        |parts: &[DatePart]| -> Option<(String, String, String, String, &'static str)> {
            if parts.len() == 7
                && parts[0].0 == "weekday"
                && parts[1] == ("literal", ", ".to_string())
                && parts[2].0 == "day"
                && parts[3] == ("literal", ". ".to_string())
                && parts[4].0 == "month"
                && parts[5] == ("literal", " ".to_string())
                && parts[6].0 == "year"
            {
                Some((
                    parts[0].1.clone(),
                    parts[2].1.clone(),
                    parts[4].1.clone(),
                    parts[6].1.clone(),
                    ".",
                ))
            } else if parts.len() == 7
                && parts[0].0 == "weekday"
                && parts[1] == ("literal", " ".to_string())
                && parts[2].0 == "day"
                && parts[3] == ("literal", " ".to_string())
                && parts[4].0 == "month"
                && parts[5] == ("literal", " ".to_string())
                && parts[6].0 == "year"
            {
                Some((
                    parts[0].1.clone(),
                    parts[2].1.clone(),
                    parts[4].1.clone(),
                    parts[6].1.clone(),
                    "",
                ))
            } else {
                None
            }
        };
    if let (Some((sw, sd, sm, sy, marker)), Some((ew, ed, em, ey, end_marker))) = (
        weekday_day_month_year(start_parts),
        weekday_day_month_year(end_parts),
    ) {
        if sm == em && sy == ey && marker == end_marker {
            if marker == "." {
                return Some(vec![
                    ("weekday", sw, "startRange"),
                    ("literal", ", ".into(), "startRange"),
                    ("day", sd, "startRange"),
                    ("literal", ".\u{2009}\u{2013}\u{2009}".into(), "shared"),
                    ("weekday", ew, "endRange"),
                    ("literal", ", ".into(), "endRange"),
                    ("day", ed, "endRange"),
                    ("literal", ". ".into(), "shared"),
                    ("month", sm, "shared"),
                    ("literal", " ".into(), "shared"),
                    ("year", sy, "shared"),
                ]);
            }
            return Some(vec![
                ("weekday", sw, "startRange"),
                ("literal", " ".into(), "startRange"),
                ("day", sd, "startRange"),
                ("literal", "\u{2009}\u{2013}\u{2009}".into(), "shared"),
                ("weekday", ew, "endRange"),
                ("literal", " ".into(), "endRange"),
                ("day", ed, "endRange"),
                ("literal", " ".into(), "shared"),
                ("month", sm, "shared"),
                ("literal", " ".into(), "shared"),
                ("year", sy, "shared"),
            ]);
        }
    }

    let arabic_weekday_day_month_year =
        |parts: &[DatePart]| -> Option<(String, String, String, String)> {
            if parts.len() == 7
                && parts[0].0 == "weekday"
                && parts[1] == ("literal", "، ".to_string())
                && parts[2].0 == "day"
                && parts[3] == ("literal", " ".to_string())
                && parts[4].0 == "month"
                && parts[5] == ("literal", " ".to_string())
                && parts[6].0 == "year"
            {
                Some((
                    parts[0].1.clone(),
                    parts[2].1.clone(),
                    parts[4].1.clone(),
                    parts[6].1.clone(),
                ))
            } else {
                None
            }
        };
    if let (Some((sw, sd, sm, sy)), Some((ew, ed, em, ey))) = (
        arabic_weekday_day_month_year(start_parts),
        arabic_weekday_day_month_year(end_parts),
    ) {
        if sm == em && sy == ey {
            return Some(vec![
                ("weekday", sw, "startRange"),
                ("literal", "، ".into(), "startRange"),
                ("day", sd, "startRange"),
                ("literal", " \u{2013} ".into(), "shared"),
                ("weekday", ew, "endRange"),
                ("literal", "، ".into(), "endRange"),
                ("day", ed, "endRange"),
                ("literal", " ".into(), "shared"),
                ("month", sm, "shared"),
                ("literal", "، ".into(), "shared"),
                ("year", sy, "shared"),
            ]);
        }
    }

    let persian_weekday_day_month_year =
        |parts: &[DatePart]| -> Option<(String, String, String, String)> {
            if parts.len() == 7
                && parts[0].0 == "year"
                && parts[1] == ("literal", " ".to_string())
                && parts[2].0 == "month"
                && parts[3] == ("literal", " ".to_string())
                && parts[4].0 == "day"
                && parts[5] == ("literal", ", ".to_string())
                && parts[6].0 == "weekday"
            {
                Some((
                    parts[6].1.clone(),
                    parts[4].1.clone(),
                    parts[2].1.clone(),
                    parts[0].1.clone(),
                ))
            } else {
                None
            }
        };
    let (sw, sd, sm, sy) = persian_weekday_day_month_year(start_parts)?;
    let (ew, ed, em, ey) = persian_weekday_day_month_year(end_parts)?;
    if sm != em || sy != ey {
        return None;
    }
    Some(vec![
        ("weekday", sw, "startRange"),
        ("literal", " ".into(), "startRange"),
        ("day", sd, "startRange"),
        ("literal", " ".into(), "shared"),
        ("month", sm.clone(), "shared"),
        ("literal", " \u{062a}\u{0627} ".into(), "shared"),
        ("weekday", ew, "endRange"),
        ("literal", " ".into(), "endRange"),
        ("day", ed, "endRange"),
        ("literal", " ".into(), "shared"),
        ("month", em, "shared"),
        ("literal", " ".into(), "shared"),
        ("year", sy, "shared"),
    ])
}

fn compact_selected_numeric_date_range(
    start_parts: &[DatePart],
    end_parts: &[DatePart],
) -> Option<Vec<RangePart>> {
    if start_parts.len() != end_parts.len() || start_parts.len() != 5 {
        return None;
    }
    if !start_parts
        .iter()
        .all(|(ty, _)| matches!(*ty, "year" | "month" | "day" | "literal"))
    {
        return None;
    }
    if start_parts
        .iter()
        .zip(end_parts.iter())
        .any(|((sty, sv), (ety, ev))| sty != ety || (*sty != "day" && sv != ev))
    {
        return None;
    }
    if start_parts[0].0 == "day"
        && start_parts[1] == ("literal", ".".to_string())
        && start_parts[2].0 == "month"
        && start_parts[3] == ("literal", ".".to_string())
        && start_parts[4].0 == "year"
    {
        return Some(vec![
            ("day", start_parts[0].1.clone(), "startRange"),
            ("literal", ".\u{2013}".into(), "shared"),
            ("day", end_parts[0].1.clone(), "endRange"),
            ("literal", ".".into(), "shared"),
            ("month", start_parts[2].1.clone(), "shared"),
            ("literal", ".".into(), "shared"),
            ("year", start_parts[4].1.clone(), "shared"),
        ]);
    }
    let sep = if start_parts
        .iter()
        .any(|(_, value)| value.contains('\u{200f}'))
    {
        " \u{2013} "
    } else if start_parts.iter().any(|(_, value)| value.contains('۰')) {
        " \u{062a}\u{0627} "
    } else if start_parts[0].0 == "year"
        && start_parts[1] == ("literal", "/".to_string())
        && start_parts[3] == ("literal", "/".to_string())
    {
        "\u{ff5e}"
    } else {
        return None;
    };
    let mut out = Vec::new();
    for (ty, value) in start_parts {
        out.push((*ty, value.clone(), "startRange"));
    }
    out.push(("literal", sep.into(), "shared"));
    for (ty, value) in end_parts {
        out.push((*ty, value.clone(), "endRange"));
    }
    Some(out)
}

fn compact_named_date_range(
    start_parts: &[DatePart],
    end_parts: &[DatePart],
) -> Option<Vec<RangePart>> {
    let persian_named_month_day = |parts: &[DatePart]| -> Option<(String, String)> {
        const PERSIAN_MONTHS: &[&str] = &[
            "فروردین",
            "اردیبهشت",
            "خرداد",
            "تیر",
            "مرداد",
            "شهریور",
            "مهر",
            "آبان",
            "آذر",
            "دی",
            "بهمن",
            "اسفند",
        ];
        if parts.len() == 3
            && parts[0].0 == "day"
            && parts[1] == ("literal", " ".to_string())
            && parts[2].0 == "month"
            && PERSIAN_MONTHS.contains(&parts[2].1.as_str())
        {
            Some((parts[0].1.clone(), parts[2].1.clone()))
        } else {
            None
        }
    };
    if let (Some((sd, sm)), Some((ed, em))) = (
        persian_named_month_day(start_parts),
        persian_named_month_day(end_parts),
    ) {
        if sm == em {
            return Some(vec![
                ("day", sd, "startRange"),
                ("literal", " ".into(), "startRange"),
                ("month", sm, "startRange"),
                ("literal", " \u{062a}\u{0627} ".into(), "shared"),
                ("day", ed, "endRange"),
                ("literal", " ".into(), "endRange"),
                ("month", em, "endRange"),
            ]);
        }
    }

    let de_named_date = |parts: &[DatePart]| -> Option<(String, String, String)> {
        if parts.len() == 5
            && parts[0].0 == "day"
            && parts[1] == ("literal", ". ".to_string())
            && parts[2].0 == "month"
            && parts[3] == ("literal", ". ".to_string())
            && parts[4].0 == "year"
        {
            Some((parts[0].1.clone(), parts[2].1.clone(), parts[4].1.clone()))
        } else {
            None
        }
    };
    if let (Some((sd, sm, sy)), Some((ed, em, ey))) =
        (de_named_date(start_parts), de_named_date(end_parts))
    {
        if sm == em && sy == ey {
            return Some(vec![
                ("day", sd, "startRange"),
                ("literal", ".\u{2013}".into(), "shared"),
                ("day", ed, "endRange"),
                ("literal", ". ".into(), "shared"),
                ("month", sm, "shared"),
                ("literal", ". ".into(), "shared"),
                ("year", sy, "shared"),
            ]);
        }
    }

    let named_month_day = |parts: &[DatePart]| -> Option<(String, String)> {
        if parts.len() == 3
            && parts[0].0 == "month"
            && parts[1] == ("literal", " ".to_string())
            && parts[2].0 == "day"
        {
            Some((parts[0].1.clone(), parts[2].1.clone()))
        } else {
            None
        }
    };
    if let (Some((sm, sd)), Some((em, ed))) =
        (named_month_day(start_parts), named_month_day(end_parts))
    {
        let mut out = Vec::new();
        if sm == em {
            out.push(("month", sm, "shared"));
            out.push(("literal", " ".into(), "shared"));
            out.push(("day", sd, "startRange"));
            out.push(("literal", "\u{2009}\u{2013}\u{2009}".into(), "shared"));
            out.push(("day", ed, "endRange"));
        } else {
            out.push(("month", sm, "startRange"));
            out.push(("literal", " ".into(), "startRange"));
            out.push(("day", sd, "startRange"));
            out.push(("literal", "\u{2009}\u{2013}\u{2009}".into(), "shared"));
            out.push(("month", em, "endRange"));
            out.push(("literal", " ".into(), "endRange"));
            out.push(("day", ed, "endRange"));
        }
        return Some(out);
    }

    let day_named_month = |parts: &[DatePart]| -> Option<(String, String, &'static str)> {
        if parts.len() == 3 && parts[0].0 == "day" && parts[2].0 == "month" {
            if parts[1] == ("literal", ". ".to_string()) {
                Some((parts[0].1.clone(), parts[2].1.clone(), "."))
            } else if parts[1] == ("literal", " ".to_string()) {
                Some((parts[0].1.clone(), parts[2].1.clone(), ""))
            } else {
                None
            }
        } else {
            None
        }
    };
    if let (Some((sd, sm, marker)), Some((ed, em, end_marker))) =
        (day_named_month(start_parts), day_named_month(end_parts))
    {
        if sm == em && marker == end_marker {
            let mut out = Vec::new();
            out.push(("day", sd, "startRange"));
            if marker == "." {
                out.push(("literal", ".\u{2013}".into(), "shared"));
                out.push(("day", ed, "endRange"));
                out.push(("literal", ". ".into(), "shared"));
            } else {
                out.push(("literal", "\u{2013}".into(), "shared"));
                out.push(("day", ed, "endRange"));
                out.push(("literal", " ".into(), "shared"));
            }
            out.push(("month", sm, "shared"));
            return Some(out);
        }
    }

    let named_date = |parts: &[DatePart]| -> Option<(String, String, String)> {
        if parts.len() == 5
            && parts[0].0 == "month"
            && parts[1] == ("literal", " ".to_string())
            && parts[2].0 == "day"
            && parts[3] == ("literal", ", ".to_string())
            && parts[4].0 == "year"
        {
            Some((parts[0].1.clone(), parts[2].1.clone(), parts[4].1.clone()))
        } else {
            None
        }
    };
    let (sm, sd, sy) = named_date(start_parts)?;
    let (em, ed, ey) = named_date(end_parts)?;
    if sy != ey {
        return None;
    }
    let mut out = Vec::new();
    if sm == em {
        out.push(("month", sm, "shared"));
        out.push(("literal", " ".into(), "shared"));
        out.push(("day", sd, "startRange"));
        out.push(("literal", "\u{2009}\u{2013}\u{2009}".into(), "shared"));
        out.push(("day", ed, "endRange"));
        out.push(("literal", ", ".into(), "shared"));
        out.push(("year", sy, "shared"));
    } else {
        out.push(("month", sm, "startRange"));
        out.push(("literal", " ".into(), "startRange"));
        out.push(("day", sd, "startRange"));
        out.push(("literal", "\u{2009}\u{2013}\u{2009}".into(), "shared"));
        out.push(("month", em, "endRange"));
        out.push(("literal", " ".into(), "endRange"));
        out.push(("day", ed, "endRange"));
        out.push(("literal", ", ".into(), "shared"));
        out.push(("year", sy, "shared"));
    }
    Some(out)
}
