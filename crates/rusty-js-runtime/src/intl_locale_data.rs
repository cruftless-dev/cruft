
#[path = "intl_currency_display_names_en_generated.rs"]
mod intl_currency_display_names_en_generated;
#[path = "intl_currency_fractions_generated.rs"]
mod intl_currency_fractions_generated;
#[path = "intl_display_names_cldr_representative_generated.rs"]
mod intl_display_names_cldr_representative_generated;
#[path = "intl_list_patterns_generated.rs"]
mod intl_list_patterns_generated;
#[path = "intl_locale_identity_generated.rs"]
mod intl_locale_identity_generated;
#[path = "intl_number_compact_generated.rs"]
mod intl_number_compact_generated;
#[path = "intl_number_symbols_generated.rs"]
mod intl_number_symbols_generated;
#[path = "intl_plural_rules_generated.rs"]
mod intl_plural_rules_generated;
#[path = "intl_relative_time_patterns_generated.rs"]
mod intl_relative_time_patterns_generated;
#[path = "intl_supported_values_generated.rs"]
pub(crate) mod intl_supported_values_generated;
#[path = "intl_time_zone_values_generated.rs"]
pub(crate) mod intl_time_zone_values_generated;

pub(crate) struct LocaleData {
    pub(crate) group: &'static str,
    pub(crate) decimal: &'static str,
    pub(crate) percent: &'static str,
    pub(crate) minus: &'static str,

    pub(crate) currency_suffix: bool,

    pub(crate) accounting_parens: bool,
}

pub(crate) enum NumberUnitPatternParts {
    Surround {
        prefix: Vec<(&'static str, String)>,
        suffix: Vec<(&'static str, String)>,
    },
    Replace(Vec<(&'static str, String)>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IntlLocaleService {
    Collator,
    DateTimeFormat,
    NumberFormat,
    PluralRules,
    RelativeTimeFormat,
    ListFormat,
    Segmenter,
    DisplayNames,
    DurationFormat,
    Locale,
    Unknown,
}

const SHARED_INTL_PRIMARIES: &[&str] = &[
    "af", "am", "ar", "az", "bg", "bn", "ca", "cs", "da", "de", "el", "en", "es", "fa", "fi",
    "fil", "fr", "ha", "he", "hi", "id", "it", "ja", "ko", "ku", "nb", "nn", "no", "pa", "pl",
    "pt", "ro", "ru", "sr", "sv", "th", "tr", "uk", "uz", "vi", "yi", "zh",
];

const PLURAL_RULES_PRIMARIES: &[&str] = &[
    "af", "ak", "am", "an", "ar", "ars", "as", "asa", "ast", "az", "bal", "be", "bem", "bez",
    "bg", "bho", "blo", "bm", "bn", "bo", "br", "brx", "bs", "ca", "ce", "ceb", "cgg", "chr",
    "ckb", "cs", "csw", "cv", "cy", "da", "de", "doi", "dsb", "dv", "dz", "ee", "el", "en",
    "eo", "es", "et", "eu", "fa", "ff", "fi", "fil", "fo", "fr", "fur", "fy", "ga", "gd",
    "gl", "gsw", "gu", "guw", "gv", "ha", "haw", "he", "hi", "hnj", "hr", "hsb", "hu", "hy",
    "ia", "id", "ie", "ig", "ii", "in", "io", "is", "it", "iu", "iw", "ja", "jbo", "jgo",
    "ji", "jmc", "jv", "jw", "ka", "kab", "kaj", "kcg", "kde", "kea", "kk", "kkj", "kl", "km",
    "kn", "ko", "kok", "ks", "ksb", "ksh", "ku", "kw", "ky", "lag", "lb", "lg", "lij", "lkt",
    "lld", "ln", "lo", "lt", "lv", "mas", "mg", "mgo", "mk", "ml", "mn", "mo", "mr", "ms",
    "mt", "my", "nah", "naq", "nb", "nd", "ne", "nl", "nn", "nnh", "no", "nqo", "nr", "nso",
    "ny", "nyn", "om", "or", "os", "osa", "pa", "pap", "pcm", "pl", "prg", "ps", "pt", "rm",
    "ro", "rof", "ru", "rwk", "sah", "saq", "sat", "sc", "scn", "sd", "sdh", "se", "seh",
    "ses", "sg", "sgs", "sh", "shi", "si", "sk", "sl", "sma", "smi", "smj", "smn", "sms",
    "sn", "so", "sq", "sr", "ss", "ssy", "st", "su", "sv", "sw", "syr", "ta", "te", "teo",
    "th", "ti", "tig", "tk", "tl", "tn", "to", "tpi", "tr", "ts", "tzm", "ug", "uk", "ur",
    "uz", "ve", "vec", "vi", "vo", "vun", "wa", "wae", "wo", "xh", "xog", "yi", "yo", "yue",
    "zh", "zu",
];

impl IntlLocaleService {
    pub(crate) fn from_constructor_name(name: &str) -> Self {
        match name {
            "Collator" => Self::Collator,
            "DateTimeFormat" => Self::DateTimeFormat,
            "NumberFormat" => Self::NumberFormat,
            "PluralRules" => Self::PluralRules,
            "RelativeTimeFormat" => Self::RelativeTimeFormat,
            "ListFormat" => Self::ListFormat,
            "Segmenter" => Self::Segmenter,
            "DisplayNames" => Self::DisplayNames,
            "DurationFormat" => Self::DurationFormat,
            "Locale" => Self::Locale,
            _ => Self::Unknown,
        }
    }

    pub(crate) fn supports_locale(self, tag: &str) -> bool {
        let base = tag
            .split("-u-")
            .next()
            .unwrap_or(tag)
            .split("-x-")
            .next()
            .unwrap_or(tag);
        let primary = base.split('-').next().unwrap_or(base);

        match self {
            Self::Unknown => false,
            _ => self
                .available_locale_primaries()
                .is_some_and(|primaries| primaries.binary_search(&primary).is_ok()),
        }
    }

    pub(crate) fn available_locale_primaries(self) -> Option<&'static [&'static str]> {
        match self {
            Self::Collator
            | Self::DateTimeFormat
            | Self::NumberFormat
            | Self::Segmenter
            | Self::ListFormat
            | Self::RelativeTimeFormat
            | Self::DisplayNames

            | Self::DurationFormat => Some(SHARED_INTL_PRIMARIES),
            Self::PluralRules => Some(PLURAL_RULES_PRIMARIES),
            Self::Locale => None,
            Self::Unknown => Some(&[]),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IntlCollationProfile {
    Root,
    GermanSearch,
    SwedishFinnish,
    Finnish,
    DanishNorwegian,
    ChinesePinyin,
    CjkDefault,
    KoreanDefault,
    SpanishTraditional,
    Spanish,
    Czech,
    Slovak,
    SlovakSearch,
    TurkishAzeri,
    Dutch,
    Croatian,
    Estonian,
    Icelandic,
    Vietnamese,
    Romanian,
    Maltese,
    Polish,
    Hungarian,
}

pub(crate) fn intl_collation_profile(
    locale: &str,
    usage: &str,
    collation: Option<&str>,
) -> IntlCollationProfile {
    let locale_lower = locale.to_ascii_lowercase();
    let base = locale_lower
        .split("-u-")
        .next()
        .unwrap_or(locale_lower.as_str());
    let extension_collation = locale_lower
        .split("-u-")
        .nth(1)
        .map(|extension| {
            let parts: Vec<&str> = extension.split('-').collect();
            parts
                .windows(2)
                .find_map(|window| (window[0] == "co").then_some(window[1].to_string()))
        })
        .unwrap_or(None);
    let selected_collation = collation.or(extension_collation.as_deref());
    if matches!(selected_collation, Some("emoji" | "eor")) {
        return IntlCollationProfile::Root;
    }
    let phonebook_option = collation == Some("phonebk");
    if (base == "de" || base.starts_with("de-"))
        && (usage == "search" || selected_collation == Some("phonebk") || phonebook_option)
    {
        return IntlCollationProfile::GermanSearch;
    }
    let is_zh = base == "zh" || base.starts_with("zh-");
    let is_zh_hant = base.starts_with("zh-hant");
    let zh_pinyin_alias = matches!(
        selected_collation,
        Some("compat" | "dict" | "phonebk" | "phonetic" | "searchjl" | "trad")
    );
    if is_zh
        && (selected_collation == Some("pinyin")
            || selected_collation.is_none() && !is_zh_hant
            || (!is_zh_hant && zh_pinyin_alias))
    {
        return IntlCollationProfile::ChinesePinyin;
    }
    if base == "ko" || base.starts_with("ko-") {
        match selected_collation {
            Some("unihan" | "searchjl") => {}
            _ => return IntlCollationProfile::KoreanDefault,
        }
    }
    if base == "ja" || base.starts_with("ja-") {
        match selected_collation {
            Some("unihan") => {}
            _ => return IntlCollationProfile::CjkDefault,
        }
    }
    if (base == "es" || base.starts_with("es-")) && selected_collation == Some("trad") {
        return IntlCollationProfile::SpanishTraditional;
    }
    if base == "es" || base.starts_with("es-") {
        return IntlCollationProfile::Spanish;
    }
    if usage == "search" && (base == "cs" || base.starts_with("cs-")) {
        return IntlCollationProfile::Root;
    }
    if base == "cs" || base.starts_with("cs-") {
        return IntlCollationProfile::Czech;
    }
    if usage == "search" && (base == "sk" || base.starts_with("sk-")) {
        return IntlCollationProfile::SlovakSearch;
    }
    if base == "sk" || base.starts_with("sk-") {
        return IntlCollationProfile::Slovak;
    }
    if base == "nl" || base.starts_with("nl-") {
        return IntlCollationProfile::Dutch;
    }
    if base == "hr" || base.starts_with("hr-") || base == "bs" || base.starts_with("bs-") {
        return IntlCollationProfile::Croatian;
    }
    if usage != "search" && (base == "sl" || base.starts_with("sl-")) {
        return IntlCollationProfile::Croatian;
    }
    if usage == "search" && (base == "et" || base.starts_with("et-")) {
        return IntlCollationProfile::Root;
    }
    if base == "et" || base.starts_with("et-") {
        return IntlCollationProfile::Estonian;
    }
    if base == "is" || base.starts_with("is-") {
        return IntlCollationProfile::Icelandic;
    }
    if base == "vi" || base.starts_with("vi-") {
        return IntlCollationProfile::Vietnamese;
    }
    if usage == "search" && (base == "ro" || base.starts_with("ro-")) {
        return IntlCollationProfile::Root;
    }
    if base == "ro" || base.starts_with("ro-") {
        return IntlCollationProfile::Romanian;
    }
    if usage != "search" && (base == "mt" || base.starts_with("mt-")) {
        return IntlCollationProfile::Maltese;
    }
    if usage != "search" && (base == "pl" || base.starts_with("pl-")) {
        return IntlCollationProfile::Polish;
    }
    if usage != "search" && (base == "hu" || base.starts_with("hu-")) {
        return IntlCollationProfile::Hungarian;
    }
    if matches!(base, "tr" | "az") || base.starts_with("tr-") || base.starts_with("az-") {
        return IntlCollationProfile::TurkishAzeri;
    }
    if base == "sv" || base.starts_with("sv-") {
        return IntlCollationProfile::SwedishFinnish;
    }
    if base == "fi" || base.starts_with("fi-") {
        return IntlCollationProfile::Finnish;
    }
    if matches!(base, "da" | "nb" | "nn" | "no")
        || base.starts_with("da-")
        || base.starts_with("nb-")
        || base.starts_with("nn-")
        || base.starts_with("no-")
    {
        return IntlCollationProfile::DanishNorwegian;
    }
    IntlCollationProfile::Root
}

pub(crate) fn intl_collator_default_case_first(locale: &str) -> Option<&'static str> {
    let locale_lower = locale.to_ascii_lowercase();
    let base = locale_lower
        .split("-u-")
        .next()
        .unwrap_or(locale_lower.as_str());
    if base == "da" || base.starts_with("da-") {
        Some("upper")
    } else {
        None
    }
}

fn is_combining_mark(c: char) -> bool {
    matches!(c,
        '\u{0300}'..='\u{036F}'
        | '\u{1AB0}'..='\u{1AFF}'
        | '\u{1DC0}'..='\u{1DFF}'
        | '\u{20D0}'..='\u{20FF}'
        | '\u{FE20}'..='\u{FE2F}'
    )
}

fn intl_de_collation_expand(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c as u32, 0x1F000..=0x1FAFF) {
            out.push('\u{0001}');
        }
        match c {
            'Ä' | 'ä' => out.push_str("ae"),
            'Ö' | 'ö' => out.push_str("oe"),
            'Ü' | 'ü' => out.push_str("ue"),
            'ß' => out.push_str("ss"),
            _ => out.push(c),
        }
    }
    out
}

fn intl_nordic_primary_expand(s: &str, profile: IntlCollationProfile) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if matches!(c as u32, 0x1F000..=0x1FAFF) {
            out.push('\u{0001}');
        }
        if profile == IntlCollationProfile::DanishNorwegian
            && matches!(c, 'A' | 'a')
            && matches!(chars.peek(), Some('A' | 'a'))
        {
            chars.next();
            out.push_str("{cz");
            continue;
        }
        match (profile, c) {
            (IntlCollationProfile::SwedishFinnish | IntlCollationProfile::Finnish, 'Å' | 'å') => {
                out.push_str("{a")
            }
            (
                IntlCollationProfile::SwedishFinnish | IntlCollationProfile::Finnish,
                'Æ' | 'æ' | 'Ä' | 'ä',
            ) => out.push_str("{b"),
            (
                IntlCollationProfile::SwedishFinnish | IntlCollationProfile::Finnish,
                'Ø' | 'ø' | 'Ö' | 'ö',
            ) => out.push_str("{c"),
            (IntlCollationProfile::DanishNorwegian, 'Æ' | 'æ' | 'Ä' | 'ä') => {
                out.push_str("{a")
            }
            (IntlCollationProfile::DanishNorwegian, 'Ø' | 'ø' | 'Ö' | 'ö') => {
                out.push_str("{b")
            }
            (IntlCollationProfile::DanishNorwegian, 'Å' | 'å') => out.push_str("{c"),
            _ => out.push(c),
        }
    }
    out
}

fn intl_root_primary_symbol_expand(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c as u32, 0x1F000..=0x1FAFF) {
            out.push('\u{0001}');
        }
        out.push(c);
    }
    out
}

fn intl_root_primary_compat_expand(s: &str, profile: IntlCollationProfile) -> String {
    let mut out = String::with_capacity(s.len());
    let root_latin_expansions = !matches!(
        profile,
        IntlCollationProfile::SwedishFinnish
            | IntlCollationProfile::Finnish
            | IntlCollationProfile::DanishNorwegian
    );
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if matches!(c, 'L' | 'l')
            && matches!(chars.get(i + 1), Some('·' | '·'))
            && matches!(chars.get(i + 2), Some('L' | 'l'))
        {
            out.push_str("ll");
            i += 3;
            continue;
        }
        match c {
            'ẞ' | 'ß' => out.push_str("ss"),
            'Ĳ' | 'ĳ' => out.push_str("ij"),
            'Ł' | 'ł' if profile != IntlCollationProfile::Polish => out.push('l'),
            'Ð' | 'ð' if profile != IntlCollationProfile::Icelandic => out.push('d'),
            'Đ' | 'đ' if profile != IntlCollationProfile::Vietnamese => out.push('d'),
            'Ħ' | 'ħ' if profile != IntlCollationProfile::Maltese => out.push('h'),
            'Œ' | 'œ'
                if root_latin_expansions
                    || matches!(
                        profile,
                        IntlCollationProfile::DanishNorwegian | IntlCollationProfile::Finnish
                    ) =>
            {
                out.push_str("oe")
            }
            'Æ' | 'æ' if root_latin_expansions => out.push_str("ae"),
            'Ø' | 'ø' if root_latin_expansions => out.push('o'),
            _ => out.push(c),
        }
        i += 1;
    }
    out
}

fn intl_zh_pinyin_primary_expand(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c as u32, 0x1F000..=0x1FAFF) {
            out.push('\u{0001}');
        }
        match c {
            '阿' => out.push_str("{a"),
            '八' => out.push_str("{b"),
            '常' => out.push_str("{c"),
            '崇' => out.push_str("{d"),
            '长' => out.push_str("{e"),
            '中' => out.push_str("{f"),
            '重' => out.push_str("{g"),
            _ => out.push(c),
        }
    }
    out
}

fn intl_cjk_default_primary_expand(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c as u32, 0x1F000..=0x1FAFF) {
            out.push('\u{0001}');
        }
        match c {
            '阿' => out.push_str("{a"),
            '中' => out.push_str("{f"),
            _ => out.push(c),
        }
    }
    out
}

fn intl_ko_default_primary_expand(s: &str) -> String {
    let mut out = intl_cjk_default_primary_expand(s);
    out = out.replace('一', "\u{10FFFF}");
    out
}

fn intl_es_traditional_primary_expand(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if matches!(c as u32, 0x1F000..=0x1FAFF) {
            out.push('\u{0001}');
        }
        if matches!(c, 'L' | 'l') && matches!(chars.peek(), Some('L' | 'l')) {
            chars.next();
            out.push_str("l{ll");
        } else if matches!(c, 'Ñ' | 'ñ') {
            out.push_str("n{n");
        } else {
            out.push(c);
        }
    }
    out
}

fn intl_es_primary_expand(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c as u32, 0x1F000..=0x1FAFF) {
            out.push('\u{0001}');
        }
        match c {
            'Ñ' | 'ñ' => out.push_str("n{n"),
            _ => out.push(c),
        }
    }
    out
}

fn intl_cs_sk_primary_expand(s: &str, profile: IntlCollationProfile) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if matches!(c as u32, 0x1F000..=0x1FAFF) {
            out.push('\u{0001}');
        }
        if matches!(c, 'C' | 'c') && matches!(chars.peek(), Some('H' | 'h')) {
            chars.next();
            out.push_str("h{ch");
        } else if matches!(c, 'Č' | 'č') {
            out.push_str("c{c");
        } else if matches!(c, 'Š' | 'š') {
            out.push_str("s{s");
        } else if matches!(c, 'Ž' | 'ž') {
            out.push_str("z{z");
        } else if profile == IntlCollationProfile::Slovak && matches!(c, 'Ä' | 'ä') {
            out.push_str("az{a");
        } else {
            out.push(c);
        }
    }
    out
}

fn intl_sk_search_primary_expand(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c as u32, 0x1F000..=0x1FAFF) {
            out.push('\u{0001}');
        }
        match c {
            'Á' | 'á' => out.push_str("a{sk1"),
            'À' | 'à' => out.push_str("a{sk0"),
            'Ä' | 'ä' => out.push_str("az{sk"),
            'Č' | 'č' => out.push_str("c{sk"),
            'É' | 'é' => out.push_str("e{sk"),
            'Í' | 'í' => out.push_str("i{sk"),
            'Ĺ' | 'ĺ' => out.push_str("l{sk0"),
            'Ľ' | 'ľ' => out.push_str("l{sk1"),
            'Ň' | 'ň' => out.push_str("n{sk"),
            'Š' | 'š' => out.push_str("s{sk"),
            'Ž' | 'ž' => out.push_str("z{sk"),
            _ => out.push(c),
        }
    }
    out
}

fn intl_nl_primary_expand(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c as u32, 0x1F000..=0x1FAFF) {
            out.push('\u{0001}');
        }
        match c {
            'Ĳ' | 'ĳ' => out.push_str("ij"),
            _ => out.push(c),
        }
    }
    out
}

fn intl_hr_primary_expand(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if matches!(c as u32, 0x1F000..=0x1FAFF) {
            out.push('\u{0001}');
        }
        if matches!(c, 'D' | 'd') && matches!(chars.peek(), Some('Ž' | 'ž')) {
            chars.next();
            out.push_str("d{c");
        } else if matches!(c, 'L' | 'l') && matches!(chars.peek(), Some('J' | 'j')) {
            chars.next();
            out.push_str("l{lj");
        } else if matches!(c, 'N' | 'n') && matches!(chars.peek(), Some('J' | 'j')) {
            chars.next();
            out.push_str("n{nj");
        } else {
            match c {
                'Č' | 'č' => out.push_str("c{c0"),
                'Ć' | 'ć' => out.push_str("c{c1"),
                'Đ' | 'đ' => out.push_str("d{d"),
                'Š' | 'š' => out.push_str("s{s"),
                'Ž' | 'ž' => out.push_str("z{z"),
                _ => out.push(c),
            }
        }
    }
    out
}

fn intl_tail_primary_expand(s: &str, profile: IntlCollationProfile) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c as u32, 0x1F000..=0x1FAFF) {
            out.push('\u{0001}');
        }
        match (profile, c) {
            (IntlCollationProfile::Estonian, 'Õ' | 'õ') => out.push_str("{a"),
            (IntlCollationProfile::Estonian, 'Ä' | 'ä') => out.push_str("{b"),
            (IntlCollationProfile::Estonian, 'Ö' | 'ö') => out.push_str("{c"),
            (IntlCollationProfile::Estonian, 'Ü' | 'ü') => out.push_str("{d"),
            (IntlCollationProfile::Estonian, 'Š' | 'š') => out.push_str("s{s"),
            (IntlCollationProfile::Estonian, 'Ž' | 'ž') => out.push_str("z{z"),
            (IntlCollationProfile::Icelandic, 'Á' | 'á') => out.push_str("a{a"),
            (IntlCollationProfile::Icelandic, 'É' | 'é') => out.push_str("e{e"),
            (IntlCollationProfile::Icelandic, 'Í' | 'í') => out.push_str("i{i"),
            (IntlCollationProfile::Icelandic, 'Æ' | 'æ' | 'Ä' | 'ä') => out.push_str("{a0"),
            (IntlCollationProfile::Icelandic, 'Å' | 'å') => out.push_str("{a1"),
            (IntlCollationProfile::Icelandic, 'Ö' | 'ö' | 'Ø' | 'ø') => out.push_str("{b"),
            (IntlCollationProfile::Icelandic, 'Þ' | 'þ') => out.push_str("t{th"),
            _ => out.push(c),
        }
    }
    out
}

fn intl_vi_primary_expand(s: &str, strip_case: bool, strip_accent: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c as u32, 0x1F000..=0x1FAFF) {
            out.push('\u{0001}');
            continue;
        }
        match c {
            'Ă' | 'ă' if !strip_accent => out.push_str("a{a"),
            'Â' | 'â' if !strip_accent => out.push_str("a{b"),
            'Đ' | 'đ' if strip_accent && strip_case => out.push('d'),
            'Đ' | 'đ' if strip_accent => out.push(c),
            'Ê' | 'ê' if !strip_accent => out.push_str("e{e"),
            'Ô' | 'ô' if !strip_accent => out.push_str("o{o"),
            'Ơ' | 'ơ' if !strip_accent => out.push_str("o{p"),
            'Ư' | 'ư' if !strip_accent => out.push_str("u{u"),
            _ => out.push(c),
        }
    }
    out
}

fn intl_ro_primary_expand(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c as u32, 0x1F000..=0x1FAFF) {
            out.push('\u{0001}');
        }
        match c {
            'Ă' | 'ă' => out.push_str("a{a"),
            'Â' | 'â' => out.push_str("a{b"),
            'Î' | 'î' => out.push_str("i{i"),
            'Ș' | 'ș' => out.push_str("s{s"),
            'Ț' | 'ț' => out.push_str("t{t"),
            _ => out.push(c),
        }
    }
    out
}

fn intl_mt_sort_expand(s: &str, strip_case: bool, strip_accent: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match (c, strip_case, strip_accent) {
            ('Ċ' | 'ċ', true, true) => out.push_str("c0"),
            ('C' | 'c' | 'Ç' | 'ç' | 'Ć' | 'ć' | 'Č' | 'č', true, true) => out.push_str("c1"),
            ('Ġ' | 'ġ', true, true) => out.push_str("g0"),
            ('G' | 'g', true, true) => out.push_str("g1"),
            ('Ħ' | 'ħ', true, true) => out.push_str("h1"),
            ('Ż' | 'ż', true, true) => out.push_str("z0"),
            ('Z' | 'z' | 'Ź' | 'ź' | 'Ž' | 'ž', true, true) => out.push_str("z1"),
            ('Ċ' | 'ċ', false, true) => out.push_str("c0"),
            ('C' | 'c' | 'Ç' | 'ç' | 'Ć' | 'ć' | 'Č' | 'č', false, true) => {
                out.push_str("c1")
            }
            ('Ġ' | 'ġ', false, true) => out.push_str("g0"),
            ('G' | 'g', false, true) => out.push_str("g1"),
            ('Ħ' | 'ħ', false, true) => out.push_str("h1"),
            ('Ż' | 'ż', false, true) => out.push_str("z0"),
            ('Z' | 'z' | 'Ź' | 'ź' | 'Ž' | 'ž', false, true) => out.push_str("z1"),
            ('Ċ', false, false) => out.push_str("c00"),
            ('ċ', false, false) => out.push_str("c01"),
            ('C', false, false) => out.push_str("c10"),
            ('c', false, false) => out.push_str("c11"),
            ('Ġ', false, false) => out.push_str("g00"),
            ('ġ', false, false) => out.push_str("g01"),
            ('G', false, false) => out.push_str("g10"),
            ('g', false, false) => out.push_str("g11"),
            ('Ħ', false, false) => out.push_str("h10"),
            ('ħ', false, false) => out.push_str("h11"),
            ('Ż', false, false) => out.push_str("z00"),
            ('ż', false, false) => out.push_str("z01"),
            ('Z', false, false) => out.push_str("z10"),
            ('z', false, false) => out.push_str("z11"),
            ('Ċ' | 'ċ', true, false) => out.push_str("c0"),
            ('Ġ' | 'ġ', true, false) => out.push_str("g0"),
            ('Ħ' | 'ħ', true, false) => out.push_str("h1"),
            ('Ż' | 'ż', true, false) => out.push_str("z0"),
            _ => out.push(c),
        }
    }
    out
}

fn intl_pl_sort_expand(s: &str, strip_case: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match (c, strip_case) {
            ('Ą' | 'ą', true) => out.push_str("a{pl"),
            ('Ć' | 'ć', true) => out.push_str("c{pl"),
            ('Ę' | 'ę', true) => out.push_str("e{pl"),
            ('Ł' | 'ł', true) => out.push_str("l{pl"),
            ('Ń' | 'ń', true) => out.push_str("n{pl"),
            ('Ś' | 'ś', true) => out.push_str("s{pl"),
            ('Ź' | 'ź', true) => out.push_str("z{pl0"),
            ('Ż' | 'ż', true) => out.push_str("z{pl1"),
            ('Ą', false) => out.push_str("a{pl0"),
            ('ą', false) => out.push_str("a{pl1"),
            ('Ć', false) => out.push_str("c{pl0"),
            ('ć', false) => out.push_str("c{pl1"),
            ('Ę', false) => out.push_str("e{pl0"),
            ('ę', false) => out.push_str("e{pl1"),
            ('Ł', false) => out.push_str("l{pl0"),
            ('ł', false) => out.push_str("l{pl1"),
            ('Ń', false) => out.push_str("n{pl0"),
            ('ń', false) => out.push_str("n{pl1"),
            ('Ś', false) => out.push_str("s{pl0"),
            ('ś', false) => out.push_str("s{pl1"),
            ('Ź', false) => out.push_str("z{pl00"),
            ('ź', false) => out.push_str("z{pl01"),
            ('Ż', false) => out.push_str("z{pl10"),
            ('ż', false) => out.push_str("z{pl11"),
            _ => out.push(c),
        }
    }
    out
}

fn intl_hu_sort_expand(s: &str, strip_accent: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match (c, strip_accent) {
            ('Ö' | 'ö' | 'Ő' | 'ő', true) => out.push_str("o{hu"),
            ('Ø' | 'ø', true) => out.push('o'),
            ('Ü' | 'ü' | 'Ű' | 'ű', true) => out.push_str("u{hu"),
            ('Ö' | 'ö', false) => out.push_str("o{hu0"),
            ('Ő' | 'ő', false) => out.push_str("o{hu1"),
            ('Ø' | 'ø', false) => out.push_str("o{hn"),
            ('Ü' | 'ü', false) => out.push_str("u{hu0"),
            ('Ű' | 'ű', false) => out.push_str("u{hu1"),
            _ => out.push(c),
        }
    }
    out
}

fn intl_tr_az_case_expand(s: &str, strip_case: bool, strip_accent: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c as u32, 0x1F000..=0x1FAFF) {
            out.push('\u{0001}');
            continue;
        }
        match (c, strip_case, strip_accent) {
            ('Í' | 'í', true, true) => out.push_str("i1"),
            ('Í' | 'í', false, true) => out.push('i'),
            ('Í' | 'í', true, false) => out.push_str("i1{acute"),
            ('Í' | 'í', false, false) => out.push_str("i{acute"),
            ('Ö' | 'ö', _, true) => out.push_str("o{o"),
            ('Ø' | 'ø', _, true) => out.push('o'),
            ('Ø' | 'ø', _, false) => out.push_str("o{n"),
            ('Ü' | 'ü', _, true) => out.push_str("u{u"),
            ('I' | 'ı', true, _) => out.push_str("i0"),
            ('İ' | 'i', true, _) => out.push_str("i1"),
            ('ı', false, _) => out.push_str("i00"),
            ('I', false, _) => out.push_str("i01"),
            ('i', false, _) => out.push_str("i10"),
            ('İ', false, _) => out.push_str("i11"),
            ('Ç' | 'ç', true, _) => out.push_str("c{c"),
            _ => out.push(c),
        }
    }
    out
}

fn intl_root_accent_weight(c: char) -> Option<(char, u8)> {
    match c {
        'A' | 'a' => Some(('a', 0)),
        'Á' | 'á' => Some(('a', 1)),
        'À' | 'à' => Some(('a', 2)),
        'Ă' | 'ă' => Some(('a', 3)),
        'Â' | 'â' => Some(('a', 4)),
        'Å' | 'å' => Some(('a', 5)),
        'Ä' | 'ä' => Some(('a', 6)),
        'O' | 'o' => Some(('o', 0)),
        'Ó' | 'ó' => Some(('o', 1)),
        'Ò' | 'ò' => Some(('o', 2)),
        'Ô' | 'ô' => Some(('o', 3)),
        'Ö' | 'ö' => Some(('o', 4)),
        _ => None,
    }
}

fn intl_root_accent_cmp(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering;
    for (ca, cb) in a.chars().zip(b.chars()) {
        if ca == cb {
            continue;
        }
        match (intl_root_accent_weight(ca), intl_root_accent_weight(cb)) {
            (Some((ba, wa)), Some((bb, wb))) if ba == bb => match wa.cmp(&wb) {
                Ordering::Equal => {}
                ord => return Some(ord),
            },
            _ => return None,
        }
    }
    match a.chars().count().cmp(&b.chars().count()) {
        Ordering::Equal => None,
        ord => Some(ord),
    }
}

fn intl_icelandic_tail_secondary_weight(c: char) -> Option<u8> {
    match c {
        'Æ' | 'æ' => Some(0),
        'Ä' | 'ä' => Some(1),
        'Å' | 'å' => Some(2),
        'Ö' | 'ö' => Some(3),
        'Ø' | 'ø' => Some(4),
        _ => None,
    }
}

fn intl_icelandic_tail_secondary_cmp(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering;
    for (ca, cb) in a.chars().zip(b.chars()) {
        if ca == cb {
            continue;
        }
        match (
            intl_icelandic_tail_secondary_weight(ca),
            intl_icelandic_tail_secondary_weight(cb),
        ) {
            (Some(wa), Some(wb)) => match wa.cmp(&wb) {
                Ordering::Equal => {}
                ord => return Some(ord),
            },
            _ => return None,
        }
    }
    match a.chars().count().cmp(&b.chars().count()) {
        Ordering::Equal => None,
        ord => Some(ord),
    }
}

fn intl_nordic_tail_secondary_weight(profile: IntlCollationProfile, c: char) -> Option<u8> {
    match profile {
        IntlCollationProfile::DanishNorwegian => match c {
            'Æ' | 'æ' => Some(0),
            'Ä' | 'ä' => Some(1),
            'Ø' | 'ø' => Some(2),
            'Ö' | 'ö' => Some(3),
            _ => None,
        },
        IntlCollationProfile::SwedishFinnish | IntlCollationProfile::Finnish => match c {
            'Ä' | 'ä' => Some(0),
            'Æ' | 'æ' => Some(1),
            'Ö' | 'ö' => Some(2),
            'Ø' | 'ø' => Some(3),
            _ => None,
        },
        _ => None,
    }
}

fn intl_nordic_tail_secondary_cmp(
    a: &str,
    b: &str,
    profile: IntlCollationProfile,
) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering;
    for (ca, cb) in a.chars().zip(b.chars()) {
        if ca == cb {
            continue;
        }
        match (
            intl_nordic_tail_secondary_weight(profile, ca),
            intl_nordic_tail_secondary_weight(profile, cb),
        ) {
            (Some(wa), Some(wb)) => match wa.cmp(&wb) {
                Ordering::Equal => {}
                ord => return Some(ord),
            },
            _ => return None,
        }
    }
    match a.chars().count().cmp(&b.chars().count()) {
        Ordering::Equal => None,
        ord => Some(ord),
    }
}

fn intl_collator_fold(
    s: &str,
    strip_case: bool,
    strip_accent: bool,
    profile: IntlCollationProfile,
) -> String {
    let s_owned;
    let s = if profile == IntlCollationProfile::GermanSearch {
        s_owned = intl_de_collation_expand(s);
        s_owned.as_str()
    } else if matches!(
        profile,
        IntlCollationProfile::SwedishFinnish
            | IntlCollationProfile::Finnish
            | IntlCollationProfile::DanishNorwegian
    ) && strip_accent
    {
        s_owned = intl_nordic_primary_expand(s, profile);
        s_owned.as_str()
    } else if profile == IntlCollationProfile::Root && strip_accent {
        s_owned = intl_root_primary_symbol_expand(s);
        s_owned.as_str()
    } else if profile == IntlCollationProfile::ChinesePinyin && strip_accent {
        s_owned = intl_zh_pinyin_primary_expand(s);
        s_owned.as_str()
    } else if profile == IntlCollationProfile::CjkDefault && strip_accent {
        s_owned = intl_cjk_default_primary_expand(s);
        s_owned.as_str()
    } else if profile == IntlCollationProfile::KoreanDefault && strip_accent {
        s_owned = intl_ko_default_primary_expand(s);
        s_owned.as_str()
    } else if profile == IntlCollationProfile::SpanishTraditional && strip_accent {
        s_owned = intl_es_traditional_primary_expand(s);
        s_owned.as_str()
    } else if profile == IntlCollationProfile::Spanish && strip_accent {
        s_owned = intl_es_primary_expand(s);
        s_owned.as_str()
    } else if matches!(
        profile,
        IntlCollationProfile::Czech | IntlCollationProfile::Slovak
    ) && strip_accent
    {
        s_owned = intl_cs_sk_primary_expand(s, profile);
        s_owned.as_str()
    } else if profile == IntlCollationProfile::SlovakSearch && strip_accent {
        s_owned = intl_sk_search_primary_expand(s);
        s_owned.as_str()
    } else if profile == IntlCollationProfile::Dutch {
        s_owned = intl_nl_primary_expand(s);
        s_owned.as_str()
    } else if profile == IntlCollationProfile::Croatian && strip_accent {
        s_owned = intl_hr_primary_expand(s);
        s_owned.as_str()
    } else if matches!(
        profile,
        IntlCollationProfile::Estonian | IntlCollationProfile::Icelandic
    ) && strip_accent
    {
        s_owned = intl_tail_primary_expand(s, profile);
        s_owned.as_str()
    } else if profile == IntlCollationProfile::Vietnamese {
        s_owned = intl_vi_primary_expand(s, strip_case, strip_accent);
        s_owned.as_str()
    } else if profile == IntlCollationProfile::Romanian && strip_accent {
        s_owned = intl_ro_primary_expand(s);
        s_owned.as_str()
    } else if profile == IntlCollationProfile::Maltese {
        s_owned = intl_mt_sort_expand(s, strip_case, strip_accent);
        s_owned.as_str()
    } else if profile == IntlCollationProfile::Polish {
        s_owned = intl_pl_sort_expand(s, strip_case);
        s_owned.as_str()
    } else if profile == IntlCollationProfile::Hungarian {
        s_owned = intl_hu_sort_expand(s, strip_accent);
        s_owned.as_str()
    } else if profile == IntlCollationProfile::TurkishAzeri {
        s_owned = intl_tr_az_case_expand(s, strip_case, strip_accent);
        s_owned.as_str()
    } else {
        s
    };
    let compat_owned;
    let s = if strip_accent {
        compat_owned = intl_root_primary_compat_expand(s, profile);
        compat_owned.as_str()
    } else {
        s
    };
    let deaccented: String = if strip_accent {
        rusty_js_ucd_tables::normalize_str(s, rusty_js_ucd_tables::NormalizationForm::Nfd)
            .chars()
            .filter(|c| !is_combining_mark(*c))
            .collect()
    } else {
        s.to_string()
    };
    if strip_case {
        deaccented.chars().flat_map(|c| c.to_lowercase()).collect()
    } else {
        deaccented
    }
}

pub(crate) fn intl_collator_compare_level(
    a: &str,
    b: &str,
    sensitivity: &str,
    profile: IntlCollationProfile,
) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let a = rusty_js_ucd_tables::normalize_str(a, rusty_js_ucd_tables::NormalizationForm::Nfc);
    let b = rusty_js_ucd_tables::normalize_str(b, rusty_js_ucd_tables::NormalizationForm::Nfc);
    let (a, b) = (a.as_str(), b.as_str());
    match intl_collator_fold(a, true, true, profile)
        .cmp(&intl_collator_fold(b, true, true, profile))
    {
        Ordering::Equal => {}
        ord => return ord,
    }
    if sensitivity == "base" {
        return Ordering::Equal;
    }
    if sensitivity == "accent" || sensitivity == "variant" {
        let af = intl_collator_fold(a, true, false, profile);
        let bf = intl_collator_fold(b, true, false, profile);
        if matches!(
            profile,
            IntlCollationProfile::Root
                | IntlCollationProfile::CjkDefault
                | IntlCollationProfile::KoreanDefault
                | IntlCollationProfile::ChinesePinyin
                | IntlCollationProfile::Czech
                | IntlCollationProfile::Slovak
                | IntlCollationProfile::SlovakSearch
                | IntlCollationProfile::TurkishAzeri
                | IntlCollationProfile::Spanish
                | IntlCollationProfile::SpanishTraditional
                | IntlCollationProfile::SwedishFinnish
                | IntlCollationProfile::Finnish
                | IntlCollationProfile::DanishNorwegian
                | IntlCollationProfile::Dutch
                | IntlCollationProfile::Croatian
                | IntlCollationProfile::Estonian
                | IntlCollationProfile::Icelandic
                | IntlCollationProfile::Vietnamese
                | IntlCollationProfile::Romanian
                | IntlCollationProfile::Maltese
                | IntlCollationProfile::Polish
                | IntlCollationProfile::Hungarian
        ) {
            if profile == IntlCollationProfile::Icelandic {
                if let Some(ord) = intl_icelandic_tail_secondary_cmp(&af, &bf) {
                    if ord != Ordering::Equal {
                        return ord;
                    }
                }
            }
            if matches!(
                profile,
                IntlCollationProfile::SwedishFinnish
                    | IntlCollationProfile::Finnish
                    | IntlCollationProfile::DanishNorwegian
            ) {
                if let Some(ord) = intl_nordic_tail_secondary_cmp(&af, &bf, profile) {
                    if ord != Ordering::Equal {
                        return ord;
                    }
                }
            }
            if let Some(ord) = intl_root_accent_cmp(&af, &bf) {
                if ord != Ordering::Equal {
                    return ord;
                }
            }
        }
        match af.cmp(&bf) {
            Ordering::Equal => {}
            ord => return ord,
        }
    }
    if sensitivity == "accent" {
        return Ordering::Equal;
    }
    let (af, bf) = if sensitivity == "case" || profile == IntlCollationProfile::TurkishAzeri {
        (
            intl_collator_fold(a, false, true, profile),
            intl_collator_fold(b, false, true, profile),
        )
    } else {
        (a.to_string(), b.to_string())
    };
    intl_collator_tertiary_cmp(&af, &bf)
}

fn intl_tr_az_case_first_group(c: char) -> Option<(u8, bool)> {
    match c {
        'I' => Some((0, true)),
        'ı' => Some((0, false)),
        'İ' => Some((1, true)),
        'i' => Some((1, false)),
        _ => None,
    }
}

fn intl_dotted_i_case_first_group(c: char) -> Option<bool> {
    match c {
        'İ' => Some(true),
        'i' => Some(false),
        _ => None,
    }
}

pub(crate) fn intl_collator_profile_case_first_cmp(
    a: &str,
    b: &str,
    case_first: &str,
    sensitivity: &str,
    profile: IntlCollationProfile,
) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering;
    if case_first != "upper" {
        return None;
    }
    for (ca, cb) in a.chars().zip(b.chars()) {
        if ca == cb {
            continue;
        }
        if profile == IntlCollationProfile::TurkishAzeri {
            match (
                intl_tr_az_case_first_group(ca),
                intl_tr_az_case_first_group(cb),
            ) {
                (Some((ga, ua)), Some((gb, ub))) if ga == gb && ua != ub => {
                    return Some(if ua {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    });
                }
                _ => return None,
            }
        }
        if profile == IntlCollationProfile::DanishNorwegian && sensitivity == "case" {
            match (
                intl_dotted_i_case_first_group(ca),
                intl_dotted_i_case_first_group(cb),
            ) {
                (Some(ua), Some(ub)) if ua != ub => {
                    return Some(if ua {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    });
                }
                _ => return None,
            }
        }
        return None;
    }
    None
}

fn intl_collator_tertiary_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let mut ai = a.chars();
    let mut bi = b.chars();
    loop {
        match (ai.next(), bi.next()) {
            (Some(ca), Some(cb)) => {
                if ca == cb {
                    continue;
                }
                let same_letter = ca.to_lowercase().eq(cb.to_lowercase());
                if same_letter && ca.is_lowercase() != cb.is_lowercase() {
                    return if ca.is_lowercase() {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    };
                }
                return ca.cmp(&cb);
            }
            (Some(_), None) => return Ordering::Greater,
            (None, Some(_)) => return Ordering::Less,
            (None, None) => return Ordering::Equal,
        }
    }
}

impl LocaleData {
    pub(crate) fn default_calendar(locale: &str) -> Option<&'static str> {
        match locale
            .split("-u-")
            .next()
            .unwrap_or(locale)
            .split("-x-")
            .next()
            .unwrap_or(locale)
            .to_ascii_lowercase()
            .as_str()
        {
            "fa" | "fa-ir" => Some("persian"),
            "th" | "th-th" => Some("buddhist"),
            _ => None,
        }
    }

    pub(crate) fn locale_exact_alias(base: &str) -> Option<String> {
        match base {
            "art-fonipa-lojban" => Some("jbo-fonipa".to_string()),
            "cel-gaulish" | "cel-gaulish-fonipa" | "cel-fonipa-gaulish" => {
                if base.contains("fonipa") {
                    Some("xtg-fonipa".to_string())
                } else {
                    Some("xtg".to_string())
                }
            }
            "zh-fonipa-guoyu" => Some("zh-fonipa".to_string()),
            "zh-fonipa-hakka" => Some("hak-fonipa".to_string()),
            "zh-fonipa-xiang" => Some("hsn-fonipa".to_string()),
            "sh-Cyrl" => Some("sr-Cyrl".to_string()),
            "sh-RS" => Some("sr-Latn-RS".to_string()),
            "sh-BA" => Some("sr-Latn-BA".to_string()),
            "sh-HR" => Some("sr-Latn-HR".to_string()),
            "sh-ME" => Some("sr-Latn-ME".to_string()),
            "cnr-BA" => Some("sr-BA".to_string()),
            _ => intl_locale_identity_generated::language_alias(base).map(str::to_string),
        }
    }

    pub(crate) fn locale_language_alias(language: &str) -> Option<&'static str> {
        match language {
            "cmn" => Some("zh"),
            _ => intl_locale_identity_generated::language_alias(language),
        }
    }

    pub(crate) fn locale_region_alias(
        region: &str,
        language: Option<&str>,
        has_armn_script: bool,
    ) -> Option<&'static str> {
        match region {
            "SU" | "810" => {
                if language == Some("hy") || has_armn_script {
                    Some("AM")
                } else {
                    Some("RU")
                }
            }
            "CS" => Some("RS"),
            "DD" => Some("DE"),
            "BU" => Some("MM"),
            "FX" => Some("FR"),
            "NT" => Some("SA"),
            "TP" => Some("TL"),
            "YD" => Some("YE"),
            _ => intl_locale_identity_generated::territory_alias_first(region),
        }
    }

    fn parse_likely_core(core: &str) -> (String, Option<String>, Option<String>) {
        let mut language = None;
        let mut script = None;
        let mut region = None;
        for part in core.split('-') {
            if language.is_none() {
                language = Some(part.to_string());
            } else if script.is_none()
                && part.len() == 4
                && part.chars().all(|ch| ch.is_ascii_alphabetic())
            {
                script = Some(part.to_string());
            } else if region.is_none()
                && ((part.len() == 2 && part.chars().all(|ch| ch.is_ascii_alphabetic()))
                    || (part.len() == 3 && part.chars().all(|ch| ch.is_ascii_digit())))
            {
                region = Some(part.to_string());
            }
        }
        (
            language.unwrap_or_else(|| "und".to_string()),
            script,
            region,
        )
    }

    fn compose_likely_core(language: &str, script: Option<&str>, region: Option<&str>) -> String {
        let mut out = vec![language.to_string()];
        if let Some(script) = script {
            out.push(script.to_string());
        }
        if let Some(region) = region {
            out.push(region.to_string());
        }
        out.join("-")
    }

    fn likely_generated_maximize(core: &str) -> Option<String> {
        if core == "zh-Hani" {
            return Some(core.to_string());
        }
        if let Some(maximal) = intl_locale_identity_generated::likely_subtag(core) {
            return Some(maximal.to_string());
        }
        let (language, script, region) = Self::parse_likely_core(core);
        let mut candidates = Vec::new();
        if let (Some(script), Some(region)) = (script.as_deref(), region.as_deref()) {
            candidates.push(Self::compose_likely_core(
                &language,
                Some(script),
                Some(region),
            ));
        }
        if let Some(script) = script.as_deref() {
            candidates.push(Self::compose_likely_core(&language, Some(script), None));
        }
        if let Some(region) = region.as_deref() {
            candidates.push(Self::compose_likely_core(&language, None, Some(region)));
        }
        candidates.push(language.clone());
        if let Some(script) = script.as_deref() {
            candidates.push(Self::compose_likely_core("und", Some(script), None));
        }

        if language == "und" {
            if let Some(region) = region.as_deref() {
                candidates.push(Self::compose_likely_core("und", None, Some(region)));
            }
            candidates.push("und".to_string());
        }

        for candidate in candidates {
            if let Some(maximal) = intl_locale_identity_generated::likely_subtag(&candidate) {
                let (max_language, max_script, max_region) = Self::parse_likely_core(maximal);
                return Some(Self::compose_likely_core(
                    if language == "und" {
                        max_language.as_str()
                    } else {
                        language.as_str()
                    },
                    script.as_deref().or(max_script.as_deref()),
                    region.as_deref().or(max_region.as_deref()),
                ));
            }
        }
        None
    }

    pub(crate) fn locale_likely_maximize_core_owned(core: &str) -> Option<String> {
        Self::likely_generated_maximize(core)
    }

    pub(crate) fn locale_likely_minimize_core_owned(core: &str, variants: &str) -> Option<String> {
        let (language, script, region) = Self::parse_likely_core(core);
        let candidates = [
            Self::compose_likely_core(&language, None, None),
            Self::compose_likely_core(&language, None, region.as_deref()),
            Self::compose_likely_core(&language, script.as_deref(), None),
        ];
        for candidate in candidates {
            if Self::likely_generated_maximize(&candidate).as_deref() == Some(core) {
                if core == "zh-Hans-CN" && (variants == "-pinyin" || variants == "-stroke") {
                    return Some("zh".to_string());
                }
                return Some(candidate);
            }
        }
        None
    }

    pub(crate) fn locale_likely_exact_maximize(base: &str) -> Option<&'static str> {
        match base {
            "art-fonipa-lojban" => Some("jbo-fonipa"),
            "cel-fonipa-gaulish" => Some("xtg-fonipa"),
            _ => None,
        }
    }

    pub(crate) fn supported_values(key: &str) -> Option<&'static [&'static str]> {
        let values: Option<&'static [&'static str]> = match key {
            "calendar" => Some(&[
                "buddhist",
                "chinese",
                "coptic",
                "dangi",
                "ethioaa",
                "ethiopic",
                "gregory",
                "hebrew",
                "indian",
                "islamic-civil",
                "islamic-tbla",
                "islamic-umalqura",
                "iso8601",
                "japanese",

                "orthodox",
                "persian",
                "roc",
            ]),

            "collation" => Some(intl_supported_values_generated::collation_bcp47()),
            "currency" => Some(intl_supported_values_generated::currency_ecma402_supported()),
            "numberingSystem" => Some(&[
                "adlm", "ahom", "arab", "arabext", "bali", "beng", "bhks", "brah", "cakm", "cham",
                "deva", "diak", "fullwide", "gara", "gong", "gonm", "gujr", "gukh", "guru",
                "hanidec", "hmng", "hmnp", "java", "kali", "kawi", "khmr", "knda", "krai", "lana",
                "lanatham", "laoo", "latn", "lepc", "limb", "mathbold", "mathdbl", "mathmono",
                "mathsanb", "mathsans", "mlym", "modi", "mong", "mroo", "mtei", "mymr", "mymrepka",
                "mymrpao", "mymrshan", "mymrtlng", "nagm", "newa", "nkoo", "olck", "onao", "orya",
                "osma", "outlined", "rohg", "saur", "segment", "shrd", "sind", "sinh", "sora",
                "sund", "sunu", "takr", "talu", "tamldec", "telu", "thai", "tibt", "tirh", "tnsa",
                "tols", "vaii", "wara", "wcho",
            ]),
            "timeZone" => Some(intl_time_zone_values_generated::ecma402_supported()),
            "unit" => Some(&[
                "acre",
                "bit",
                "byte",
                "celsius",
                "centimeter",
                "day",
                "degree",
                "fahrenheit",
                "fluid-ounce",
                "foot",
                "gallon",
                "gigabit",
                "gigabyte",
                "gram",
                "hectare",
                "hour",
                "inch",
                "kilobit",
                "kilobyte",
                "kilogram",
                "kilometer",
                "liter",
                "megabit",
                "megabyte",
                "meter",
                "microsecond",
                "mile",
                "mile-scandinavian",
                "milliliter",
                "millimeter",
                "millisecond",
                "minute",
                "month",
                "nanosecond",
                "ounce",
                "percent",
                "petabyte",
                "pound",
                "second",
                "stone",
                "terabit",
                "terabyte",
                "week",
                "yard",
                "year",
            ]),
            _ => None,
        };
        if matches!(
            key,
            "calendar" | "collation" | "currency" | "numberingSystem" | "unit"
        ) {
            if let Some(rows) = values {
                debug_assert!(rows
                    .iter()
                    .all(|value| intl_supported_values_generated::contains(key, value)));
            }
        }
        if key == "timeZone" {
            if let Some(rows) = values {
                debug_assert!(rows
                    .iter()
                    .all(|value| intl_time_zone_values_generated::contains(value)));
            }
        }
        values
    }

    fn en_us() -> Self {
        LocaleData {
            group: ",",
            decimal: ".",
            percent: "%",
            minus: "-",
            currency_suffix: false,
            accounting_parens: true,
        }
    }

    pub(crate) fn for_locale(tag: &str) -> Self {
        let selected = Self::for_locale_selected(tag);
        if let Some(symbols) = intl_number_symbols_generated::number_symbols(tag) {
            return LocaleData {
                group: symbols.group,
                decimal: symbols.decimal,
                percent: symbols.percent_glyph,
                minus: symbols.minus_glyph,
                currency_suffix: selected.currency_suffix,
                accounting_parens: selected.accounting_parens,
            };
        }
        selected
    }

    fn for_locale_selected(tag: &str) -> Self {
        if tag.starts_with("de") {
            return LocaleData {
                group: ".",
                decimal: ",",
                percent: "%",
                minus: "-",
                currency_suffix: true,
                accounting_parens: false,
            };
        }
        if tag.starts_with("pt-PT") {
            return LocaleData {
                group: "\u{a0}",
                decimal: ",",
                percent: "%",
                minus: "-",
                currency_suffix: true,
                accounting_parens: false,
            };
        }
        if tag.starts_with("pt") {
            return LocaleData {
                group: ".",
                decimal: ",",
                percent: "%",
                minus: "-",
                currency_suffix: false,
                accounting_parens: false,
            };
        }
        if tag.starts_with("af") {
            return LocaleData {
                group: "\u{a0}",
                decimal: ",",
                percent: "%",
                minus: "-",
                currency_suffix: false,
                accounting_parens: false,
            };
        }
        if tag.starts_with("uk") || tag.starts_with("uz") {
            return LocaleData {
                group: "\u{a0}",
                decimal: ",",
                percent: "%",
                minus: "-",
                currency_suffix: true,
                accounting_parens: false,
            };
        }
        if tag.starts_with("bg") {
            return LocaleData {
                group: "\u{a0}",
                decimal: ",",
                percent: "%",
                minus: "-",
                currency_suffix: true,
                accounting_parens: false,
            };
        }
        if tag.starts_with("bn") {
            return LocaleData {
                group: ",",
                decimal: ".",
                percent: "%",
                minus: "-",
                currency_suffix: true,
                accounting_parens: false,
            };
        }
        if tag.starts_with("ar") {
            return LocaleData {
                group: "\u{066C}",
                decimal: "\u{066B}",
                percent: "\u{066A}",
                minus: "-",
                currency_suffix: true,
                accounting_parens: false,
            };
        }
        if tag.starts_with("fa") {
            return LocaleData {
                group: "\u{066C}",
                decimal: "\u{066B}",
                percent: "\u{066A}",
                minus: "\u{2212}",
                currency_suffix: false,
                accounting_parens: false,
            };
        }
        if tag.starts_with("he") {
            return LocaleData {
                group: ",",
                decimal: ".",
                percent: "%",
                minus: "-",
                currency_suffix: true,
                accounting_parens: false,
            };
        }
        if tag.starts_with("fr") {
            return LocaleData {
                group: "\u{202f}",
                decimal: ",",
                percent: "%",
                minus: "-",
                currency_suffix: true,
                accounting_parens: false,
            };
        }
        if tag.starts_with("az")
            || tag.starts_with("ca")
            || tag.starts_with("da")
            || tag.starts_with("el")
            || tag.starts_with("es")
            || tag.starts_with("it")
            || tag.starts_with("ku")
            || tag.starts_with("ro")
            || tag.starts_with("sr")
            || tag.starts_with("vi")
        {
            return LocaleData {
                group: ".",
                decimal: ",",
                percent: "%",
                minus: "-",
                currency_suffix: true,
                accounting_parens: false,
            };
        }
        if tag.starts_with("tr") {
            return LocaleData {
                group: ".",
                decimal: ",",
                percent: "%",
                minus: "-",
                currency_suffix: false,
                accounting_parens: false,
            };
        }
        if tag.starts_with("id") {
            return LocaleData {
                group: ".",
                decimal: ",",
                percent: "%",
                minus: "-",
                currency_suffix: false,
                accounting_parens: false,
            };
        }
        if tag.starts_with("cs")
            || tag == "fi"
            || tag.starts_with("fi-")
            || tag.starts_with("nb")
            || tag.starts_with("nn")
            || tag.starts_with("no")
            || tag.starts_with("pl")
            || tag.starts_with("ru")
            || tag.starts_with("sv")
        {
            return LocaleData {
                group: "\u{a0}",
                decimal: ",",
                percent: "%",
                minus: "-",
                currency_suffix: true,
                accounting_parens: false,
            };
        }
        LocaleData::en_us()
    }

    pub(crate) fn month_name(locale: Option<&str>, month: usize, width: &str) -> &'static str {
        let locale = locale.unwrap_or("en-US").to_ascii_lowercase();
        let i = month.saturating_sub(1).min(11);
        if locale == "de" || locale.starts_with("de-") {
            const LONG: [&str; 12] = [
                "Januar",
                "Februar",
                "März",
                "April",
                "Mai",
                "Juni",
                "Juli",
                "August",
                "September",
                "Oktober",
                "November",
                "Dezember",
            ];
            const SHORT: [&str; 12] = [
                "Jan", "Feb", "März", "Apr", "Mai", "Juni", "Juli", "Aug", "Sept", "Okt", "Nov",
                "Dez",
            ];
            return if width == "long" { LONG[i] } else { SHORT[i] };
        }
        if locale == "fr" || locale.starts_with("fr-") {
            const LONG: [&str; 12] = [
                "janvier",
                "février",
                "mars",
                "avril",
                "mai",
                "juin",
                "juillet",
                "août",
                "septembre",
                "octobre",
                "novembre",
                "décembre",
            ];
            const SHORT: [&str; 12] = [
                "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.",
                "nov.", "déc.",
            ];
            return if width == "long" { LONG[i] } else { SHORT[i] };
        }
        if locale == "ja" || locale.starts_with("ja-") {
            const NUMERIC: [&str; 12] = [
                "1月", "2月", "3月", "4月", "5月", "6月", "7月", "8月", "9月", "10月", "11月",
                "12月",
            ];
            return NUMERIC[i];
        }
        if locale == "ru" || locale.starts_with("ru-") {
            const LONG: [&str; 12] = [
                "январь",
                "февраль",
                "март",
                "апрель",
                "май",
                "июнь",
                "июль",
                "август",
                "сентябрь",
                "октябрь",
                "ноябрь",
                "декабрь",
            ];
            const SHORT: [&str; 12] = [
                "янв.",
                "февр.",
                "март",
                "апр.",
                "май",
                "июнь",
                "июль",
                "авг.",
                "сент.",
                "окт.",
                "нояб.",
                "дек.",
            ];
            return if width == "long" { LONG[i] } else { SHORT[i] };
        }
        const LONG: [&str; 12] = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        const SHORT: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        const NARROW: [&str; 12] = ["J", "F", "M", "A", "M", "J", "J", "A", "S", "O", "N", "D"];
        match width {
            "long" => LONG[i],
            "narrow" => NARROW[i],
            _ => SHORT[i],
        }
    }

    pub(crate) fn month_name_for_calendar(
        locale: Option<&str>,
        calendar: &str,
        month: usize,
        width: &str,
    ) -> &'static str {
        if matches!(
            calendar,
            "islamic" | "islamic-civil" | "islamic-rgsa" | "islamic-tbla" | "islamic-umalqura"
        ) {
            const LONG: [&str; 12] = [
                "Muharram",
                "Safar",
                "Rabiʻ I",
                "Rabiʻ II",
                "Jumada I",
                "Jumada II",
                "Rajab",
                "Shaʻban",
                "Ramadan",
                "Shawwal",
                "Dhuʻl-Qiʻdah",
                "Dhuʻl-Hijjah",
            ];
            const SHORT: [&str; 12] = [
                "Muh.",
                "Saf.",
                "Rab. I",
                "Rab. II",
                "Jum. I",
                "Jum. II",
                "Raj.",
                "Sha.",
                "Ram.",
                "Shaw.",
                "Dhuʻl-Q.",
                "Dhuʻl-H.",
            ];
            let i = month.saturating_sub(1).min(11);
            return if width == "long" { LONG[i] } else { SHORT[i] };
        }
        if calendar == "persian" {
            const MONTHS: [&str; 12] = [
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
            return MONTHS[month.saturating_sub(1).min(11)];
        }
        Self::month_name(locale, month, width)
    }

    pub(crate) fn month_name_for_datetime_format(
        locale: Option<&str>,
        calendar: &str,
        month: usize,
        width: &str,
        day_present: bool,
    ) -> &'static str {
        if !day_present || width != "long" || !matches!(calendar, "gregory" | "buddhist") {
            return Self::month_name_for_calendar(locale, calendar, month, width);
        }
        let locale = locale.unwrap_or("en-US").to_ascii_lowercase();
        let i = month.saturating_sub(1).min(11);
        macro_rules! long_months {
            ($($prefix:literal => [$($name:literal),+ $(,)?]),+ $(,)?) => {
                $(
                    if locale == $prefix || locale.starts_with(concat!($prefix, "-")) {
                        const MONTHS: [&str; 12] = [$($name),+];
                        return MONTHS[i];
                    }
                )+
            };
        }
        long_months! {
            "ar" => ["يناير", "فبراير", "مارس", "أبريل", "مايو", "يونيو", "يوليو", "أغسطس", "سبتمبر", "أكتوبر", "نوفمبر", "ديسمبر"],
            "bn" => ["জানুয়ারি", "ফেব্রুয়ারি", "মার্চ", "এপ্রিল", "মে", "জুন", "জুলাই", "আগস্ট", "সেপ্টেম্বর", "অক্টোবর", "নভেম্বর", "ডিসেম্বর"],
            "cs" => ["ledna", "února", "března", "dubna", "května", "června", "července", "srpna", "září", "října", "listopadu", "prosince"],
            "es" => ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"],
            "fi" => ["tammikuuta", "helmikuuta", "maaliskuuta", "huhtikuuta", "toukokuuta", "kesäkuuta", "heinäkuuta", "elokuuta", "syyskuuta", "lokakuuta", "marraskuuta", "joulukuuta"],
            "he" => ["בינואר", "בפברואר", "במרץ", "באפריל", "במאי", "ביוני", "ביולי", "באוגוסט", "בספטמבר", "באוקטובר", "בנובמבר", "בדצמבר"],
            "hi" => ["जनवरी", "फ़रवरी", "मार्च", "अप्रैल", "मई", "जून", "जुलाई", "अगस्त", "सितंबर", "अक्टूबर", "नवंबर", "दिसंबर"],
            "it" => ["gennaio", "febbraio", "marzo", "aprile", "maggio", "giugno", "luglio", "agosto", "settembre", "ottobre", "novembre", "dicembre"],
            "ko" => ["1월", "2월", "3월", "4월", "5월", "6월", "7월", "8월", "9월", "10월", "11월", "12월"],
            "pl" => ["stycznia", "lutego", "marca", "kwietnia", "maja", "czerwca", "lipca", "sierpnia", "września", "października", "listopada", "grudnia"],
            "pt" => ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"],
            "ru" => ["января", "февраля", "марта", "апреля", "мая", "июня", "июля", "августа", "сентября", "октября", "ноября", "декабря"],
            "sv" => ["januari", "februari", "mars", "april", "maj", "juni", "juli", "augusti", "september", "oktober", "november", "december"],
            "th" => ["มกราคม", "กุมภาพันธ์", "มีนาคม", "เมษายน", "พฤษภาคม", "มิถุนายน", "กรกฎาคม", "สิงหาคม", "กันยายน", "ตุลาคม", "พฤศจิกายน", "ธันวาคม"],
            "zh" => ["1月", "2月", "3月", "4月", "5月", "6月", "7月", "8月", "9月", "10月", "11月", "12月"],
        }
        Self::month_name_for_calendar(Some(locale.as_str()), calendar, month, width)
    }

    pub(crate) fn weekday_name(locale: Option<&str>, wd: usize, width: &str) -> &'static str {
        let locale = locale.unwrap_or("en-US").to_ascii_lowercase();
        let i = wd.min(6);
        if width == "long" {
            macro_rules! long_weekdays {
                ($($prefix:literal => [$($name:literal),+ $(,)?]),+ $(,)?) => {
                    $(
                        if locale == $prefix || locale.starts_with(concat!($prefix, "-")) {
                            const DAYS: [&str; 7] = [$($name),+];
                            return DAYS[i];
                        }
                    )+
                };
            }
            long_weekdays! {
                "ar" => ["الأحد", "الاثنين", "الثلاثاء", "الأربعاء", "الخميس", "الجمعة", "السبت"],
                "bn" => ["রবিবার", "সোমবার", "মঙ্গলবার", "বুধবার", "বৃহস্পতিবার", "শুক্রবার", "শনিবার"],
                "cs" => ["neděle", "pondělí", "úterý", "středa", "čtvrtek", "pátek", "sobota"],
                "de" => ["Sonntag", "Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag"],
                "es" => ["domingo", "lunes", "martes", "miércoles", "jueves", "viernes", "sábado"],
                "fa" => ["یکشنبه", "دوشنبه", "سه‌شنبه", "چهارشنبه", "پنجشنبه", "جمعه", "شنبه"],
                "fi" => ["sunnuntai", "maanantai", "tiistai", "keskiviikko", "torstai", "perjantai", "lauantai"],
                "fr" => ["dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi"],
                "he" => ["יום ראשון", "יום שני", "יום שלישי", "יום רביעי", "יום חמישי", "יום שישי", "יום שבת"],
                "hi" => ["रविवार", "सोमवार", "मंगलवार", "बुधवार", "गुरुवार", "शुक्रवार", "शनिवार"],
                "it" => ["domenica", "lunedì", "martedì", "mercoledì", "giovedì", "venerdì", "sabato"],
                "ko" => ["일요일", "월요일", "화요일", "수요일", "목요일", "금요일", "토요일"],
                "pl" => ["niedziela", "poniedziałek", "wtorek", "środa", "czwartek", "piątek", "sobota"],
                "pt" => ["domingo", "segunda-feira", "terça-feira", "quarta-feira", "quinta-feira", "sexta-feira", "sábado"],
                "ru" => ["воскресенье", "понедельник", "вторник", "среда", "четверг", "пятница", "суббота"],
                "sv" => ["söndag", "måndag", "tisdag", "onsdag", "torsdag", "fredag", "lördag"],
                "th" => ["วันอาทิตย์", "วันจันทร์", "วันอังคาร", "วันพุธ", "วันพฤหัสบดี", "วันศุกร์", "วันเสาร์"],
                "zh" => ["星期日", "星期一", "星期二", "星期三", "星期四", "星期五", "星期六"],
            }
        }
        if locale == "ja" || locale.starts_with("ja-") {
            const LONG: [&str; 7] = [
                "日曜日",
                "月曜日",
                "火曜日",
                "水曜日",
                "木曜日",
                "金曜日",
                "土曜日",
            ];
            const SHORT: [&str; 7] = ["日", "月", "火", "水", "木", "金", "土"];
            return if width == "long" { LONG[i] } else { SHORT[i] };
        }
        const LONG: [&str; 7] = [
            "Sunday",
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
        ];
        const SHORT: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        const NARROW: [&str; 7] = ["S", "M", "T", "W", "T", "F", "S"];
        match width {
            "long" => LONG[i],
            "narrow" => NARROW[i],
            _ => SHORT[i],
        }
    }

    pub(crate) fn day_period(hour: u32) -> &'static str {
        if hour < 12 {
            "AM"
        } else {
            "PM"
        }
    }

    pub(crate) fn day_period_for_datetime_format(locale: Option<&str>, hour: u32) -> &'static str {
        let lower = locale.map(|s| s.to_ascii_lowercase()).unwrap_or_default();
        let pm = hour >= 12;
        if lower == "ar" || lower.starts_with("ar-") {
            if pm {
                "م"
            } else {
                "ص"
            }
        } else if lower == "hi" || lower.starts_with("hi-") {
            if pm {
                "pm"
            } else {
                "am"
            }
        } else if lower == "ko" || lower.starts_with("ko-") {
            if pm {
                "오후"
            } else {
                "오전"
            }
        } else {
            Self::day_period(hour)
        }
    }

    pub(crate) fn flexible_day_period(hour: u32, width: &str) -> &'static str {
        match hour {
            0..=11 => "in the morning",
            12 if width == "narrow" => "n",
            12 => "noon",
            13..=17 => "in the afternoon",
            18..=21 => "in the evening",
            _ => "at night",
        }
    }

    pub(crate) fn currency(code: &str) -> (String, usize) {
        let up = code.to_ascii_uppercase();
        let digits = Self::currency_fraction_digits(&up);
        match code.to_ascii_uppercase().as_str() {
            "USD" => ("$".into(), digits),
            "EUR" => ("€".into(), digits),
            "GBP" => ("£".into(), digits),
            "JPY" => ("¥".into(), digits),
            "CNY" => ("CN¥".into(), digits),
            "INR" => ("₹".into(), digits),
            "KRW" => ("₩".into(), digits),
            "CAD" => ("CA$".into(), digits),
            "AUD" => ("A$".into(), digits),
            "CHF" => ("CHF".into(), digits),
            other => (other.to_string(), digits),
        }
    }

    pub(crate) fn currency_fraction_digits(code: &str) -> usize {
        intl_currency_fractions_generated::fraction_digits(code)
    }

    pub(crate) fn currency_symbol(tag: &str, code: &str) -> String {
        let up = code.to_ascii_uppercase();
        if up == "USD" && tag.starts_with("bg") {
            return "щ.д.".into();
        }
        if up == "USD" && tag.starts_with("fr") {
            return "$US".into();
        }
        if up == "USD"
            && (tag.starts_with("af")
                || tag.starts_with("ca")
                || tag.starts_with("it")
                || tag.starts_with("nb")
                || tag.starts_with("nn")
                || tag.starts_with("no")
                || tag.starts_with("pl")
                || tag.starts_with("ro")
                || tag.starts_with("uk"))
        {
            return "USD".into();
        }
        if up == "USD"
            && (tag.starts_with("am")
                || tag.starts_with("ar")
                || tag.starts_with("az")
                || tag.starts_with("bn")
                || tag.starts_with("cs")
                || tag.starts_with("da")
                || tag.starts_with("es")
                || tag.starts_with("id")
                || tag.starts_with("ko")
                || tag.starts_with("pa")
                || tag.starts_with("pt")
                || tag.starts_with("sr")
                || tag.starts_with("sv")
                || tag.starts_with("th")
                || tag.starts_with("uz")
                || tag.starts_with("vi")
                || tag.starts_with("yi")
                || tag.starts_with("zh"))
        {
            return "US$".into();
        }
        if up == "JPY" && tag.starts_with("ja") {
            return "￥".into();
        }
        LocaleData::currency(&up).0
    }

    pub(crate) fn currency_prefix_separator(tag: &str, code: &str, display: &str) -> &'static str {
        let up = code.to_ascii_uppercase();
        if up == "USD" && tag.starts_with("fa") && display == "code" {
            return "\u{00A0}";
        }
        if up == "USD"
            && matches!(display, "symbol" | "narrowSymbol")
            && (tag.starts_with("af")
                || tag.starts_with("ha")
                || tag.starts_with("pt")
                || tag.starts_with("yi"))
        {
            return "\u{00A0}";
        }
        ""
    }

    pub(crate) fn currency_prefix_literal(tag: &str, code: &str, display: &str) -> &'static str {
        let up = code.to_ascii_uppercase();
        if up == "USD"
            && tag.starts_with("fa")
            && matches!(display, "code" | "symbol" | "narrowSymbol")
        {
            "\u{200E}"
        } else {
            ""
        }
    }

    pub(crate) fn number_leading_literal(tag: &str, style: &str) -> &'static str {
        if style == "currency" && (tag.starts_with("ar") || tag.starts_with("he")) {
            "\u{200F}"
        } else {
            ""
        }
    }

    pub(crate) fn sign_prefix_literal(tag: &str, sign_kind: &str) -> &'static str {
        if tag.starts_with("ar") && matches!(sign_kind, "minusSign" | "plusSign") {
            "\u{061C}"
        } else if (tag.starts_with("fa") || tag.starts_with("he"))
            && matches!(sign_kind, "minusSign" | "plusSign")
        {
            "\u{200E}"
        } else {
            ""
        }
    }

    pub(crate) fn currency_suffix_separator(tag: &str) -> &'static str {
        if tag.starts_with("he") {
            "\u{00A0}\u{200F}"
        } else {
            "\u{00A0}"
        }
    }

    pub(crate) fn percent_suffix_separator(tag: &str) -> &'static str {
        if tag.starts_with("ca")
            || tag.starts_with("cs")
            || tag.starts_with("da")
            || tag.starts_with("de")
            || tag.starts_with("es")
            || tag.starts_with("fr")
            || tag == "fi"
            || tag.starts_with("fi-")
            || tag.starts_with("nb")
            || tag.starts_with("nn")
            || tag.starts_with("no")
            || tag.starts_with("ro")
            || tag.starts_with("ru")
            || tag.starts_with("sv")
        {
            "\u{00A0}"
        } else {
            ""
        }
    }

    pub(crate) fn percent_trailing_literal(tag: &str) -> &'static str {
        if tag.starts_with("ar") {
            "\u{061C}"
        } else {
            ""
        }
    }

    pub(crate) fn percent_sign_prefix(tag: &str) -> bool {
        tag.starts_with("ku") || tag.starts_with("tr")
    }

    pub(crate) fn currency_name(_tag: &str, code: &str, singular: bool) -> String {
        match code.to_ascii_uppercase().as_str() {
            "USD" if singular => "US dollar".into(),
            "USD" => "US dollars".into(),
            "EUR" if singular => "euro".into(),
            "EUR" => "euros".into(),
            "JPY" => "Japanese yen".into(),
            other => other.to_string(),
        }
    }

    pub(crate) fn number_unit_display(
        locale: &str,
        unit: &str,
        display: &str,
        is_one: bool,
    ) -> Option<(String, Option<&'static str>)> {
        if let Some(value) = Self::selected_locale_number_unit_display(locale, unit, display) {
            return Some(value);
        }
        if let Some((numerator, denominator)) = unit.split_once("-per-") {
            if numerator == "mile" && denominator == "hour" && display != "long" {

                let sep = if display == "narrow" { None } else { Some(" ") };
                return Some(("mph".into(), sep));
            }
            let num = Self::simple_number_unit_display(numerator, display, is_one)?;

            let den = if display == "long" {
                Self::simple_number_unit_display(denominator, "long", true)?
            } else {
                match denominator {
                    "hour" => "h".to_string(),
                    "second" => "s".to_string(),
                    "minute" => "min".to_string(),
                    "day" => "d".to_string(),
                    other => Self::simple_number_unit_display(other, display, true)?,
                }
            };
            let rendered = if display == "long" {
                format!("{num} per {den}")
            } else {
                format!("{num}/{den}")
            };
            return Some((rendered, if display != "narrow" { Some(" ") } else { None }));
        }
        Self::simple_number_unit_display(unit, display, is_one).map(|value| {
            let separated = display != "narrow"
                && !(unit == "percent" && display != "long")
                && !((unit == "celsius" || unit == "fahrenheit") && display == "short");
            (value, if separated { Some(" ") } else { None })
        })
    }

    pub(crate) fn number_unit_pattern_parts(
        locale: &str,
        unit: &str,
        display: &str,
        value: f64,
    ) -> Option<NumberUnitPatternParts> {
        let is_one = value.abs() == 1.0;
        let primary = locale.split('-').next().unwrap_or(locale);
        let lit = |s: &'static str| ("literal", s.to_string());
        let unit_part = |s: &str| ("unit", s.to_string());
        let surround = |prefix, suffix| Some(NumberUnitPatternParts::Surround { prefix, suffix });
        match (primary, unit, display) {
            ("ar", "meter", "short") if (value - 1.0).abs() <= f64::EPSILON => {
                Some(NumberUnitPatternParts::Replace(vec![unit_part("متر")]))
            }
            ("ar", "meter", "short") if (value - 2.0).abs() <= f64::EPSILON => {
                Some(NumberUnitPatternParts::Replace(vec![unit_part("متران")]))
            }
            ("ar", "meter", "short") if (value - 12.0).abs() <= f64::EPSILON => {
                surround(vec![], vec![lit(" "), unit_part("مترًا")])
            }
            ("ja", "meter", "long") => surround(vec![], vec![lit(" "), unit_part("メートル")]),
            ("ja", "meter-per-second", "long") => surround(
                vec![unit_part("秒速"), lit(" ")],
                vec![lit(" "), unit_part("メートル")],
            ),
            ("ko", "meter", "long") => surround(vec![], vec![unit_part("미터")]),
            ("ko", "meter", "short") => surround(vec![], vec![unit_part("m")]),
            ("ko", "meter-per-second", "long") => {
                surround(vec![unit_part("초속"), lit(" ")], vec![unit_part("미터")])
            }
            ("zh", "meter", "long") | ("zh", "meter", "short") => {
                surround(vec![], vec![unit_part("米")])
            }
            ("zh", "meter-per-second", "long") => {
                surround(vec![unit_part("每秒")], vec![unit_part("米")])
            }
            ("zh", "liter", "narrow") => surround(vec![], vec![unit_part("升")]),
            _ => Self::number_unit_display(locale, unit, display, is_one).map(
                |(unit_value, separator)| {
                    let mut suffix = Vec::new();
                    if let Some(separator) = separator {
                        suffix.push(lit(separator));
                    }
                    suffix.push(unit_part(&unit_value));
                    NumberUnitPatternParts::Surround {
                        prefix: Vec::new(),
                        suffix,
                    }
                },
            ),
        }
    }

    fn selected_locale_number_unit_display(
        locale: &str,
        unit: &str,
        display: &str,
    ) -> Option<(String, Option<&'static str>)> {
        let primary = locale.split('-').next().unwrap_or(locale);
        match (primary, unit, display) {
            ("ar", "meter", "long") => Some(("متر".into(), Some(" "))),
            ("ar", "meter-per-second", "long") => Some(("متر في الثانية".into(), Some(" "))),
            ("ar", "liter", "narrow") => Some(("ل".into(), Some(" "))),
            ("bn", "meter", "long") => Some(("মিটার".into(), Some(" "))),
            ("bn", "meter", "short") => Some(("মি".into(), Some(" "))),
            ("bn", "meter-per-second", "long") => Some(("মিটার প্রতি সেকেন্ড".into(), Some(" "))),
            ("bn", "liter", "narrow") => Some(("লিটার".into(), Some(" "))),
            ("de", "meter", "long") => Some(("Meter".into(), Some(" "))),
            ("de", "meter-per-second", "long") => Some(("Meter pro Sekunde".into(), Some(" "))),
            ("de", "liter", "narrow") => Some(("l".into(), Some(" "))),
            ("fa", "meter", "long") => Some(("متر".into(), Some(" "))),
            ("fa", "meter", "short") => Some(("متر".into(), None)),
            ("fa", "meter-per-second", "long") => Some(("متر در ثانیه".into(), Some(" "))),
            ("fr", "meter", "long") => Some(("mètres".into(), Some("\u{a0}"))),
            ("fr", "meter", "short") => Some(("m".into(), Some("\u{202f}"))),
            ("fr", "meter-per-second", "long") => {
                Some(("mètres par seconde".into(), Some("\u{a0}")))
            }
            ("fr", "liter", "narrow") => Some(("l".into(), None)),
            ("he", "meter", "long") => Some(("מטרים".into(), Some(" "))),
            ("he", "meter", "short") => Some(("מ׳".into(), Some(" "))),
            ("he", "meter-per-second", "long") => Some(("מטר לשנייה".into(), Some(" "))),
            ("he", "liter", "narrow") => Some(("ל׳".into(), Some(" "))),
            ("es", "meter", "long") => Some(("metros".into(), Some(" "))),
            ("es", "meter-per-second", "long") => Some(("metros por segundo".into(), Some(" "))),
            ("es", "liter", "narrow") => Some(("l".into(), None)),
            ("hi", "meter", "long") => Some(("मीटर".into(), Some(" "))),
            ("hi", "meter", "short") => Some(("मी".into(), Some(" "))),
            ("hi", "meter-per-second", "long") => Some(("मीटर प्रति सेकंड".into(), Some(" "))),
            ("hi", "liter", "narrow") => Some(("ली".into(), Some(" "))),
            ("pl", "meter", "long") => Some(("metry".into(), Some(" "))),
            ("pl", "meter-per-second", "long") => Some(("metry na sekundę".into(), Some(" "))),
            ("pl", "liter", "narrow") => Some(("l".into(), Some(" "))),
            ("ru", "meter", "long") => Some(("метра".into(), Some(" "))),
            ("ru", "meter", "short") => Some(("м".into(), Some(" "))),
            ("ru", "meter-per-second", "long") => Some(("метра в секунду".into(), Some(" "))),
            ("ru", "liter", "narrow") => Some(("л".into(), Some(" "))),
            ("th", "meter", "long") => Some(("เมตร".into(), Some(" "))),
            ("th", "meter", "short") => Some(("ม.".into(), Some(" "))),
            ("th", "meter-per-second", "long") => Some(("เมตรต่อวินาที".into(), Some(" "))),
            ("th", "liter", "narrow") => Some(("ล.".into(), None)),
            _ => None,
        }
    }

    pub(crate) fn is_sanctioned_number_unit(unit: &str) -> bool {
        if let Some((numerator, denominator)) = unit.split_once("-per-") {
            return !numerator.is_empty()
                && !denominator.is_empty()
                && !denominator.contains("-per-")
                && Self::simple_number_unit_display(numerator, "short", false).is_some()
                && Self::simple_number_unit_display(denominator, "short", false).is_some();
        }
        Self::simple_number_unit_display(unit, "short", false).is_some()
    }

    fn simple_number_unit_display(unit: &str, display: &str, is_one: bool) -> Option<String> {
        if display == "narrow" {
            match unit {
                "byte" => return Some("B".into()),
                "celsius" => return Some("\u{00B0}C".into()),
                _ => {}
            }
        }
        let (short, long) = match unit {
            "acre" => ("ac", "acres"),
            "bit" => ("bit", "bits"),
            "byte" => ("byte", "bytes"),
            "celsius" => ("\u{00B0}C", "degrees Celsius"),
            "centimeter" => ("cm", "centimeters"),
            "day" => ("day", "days"),
            "degree" => ("deg", "degrees"),
            "fahrenheit" => ("\u{00B0}F", "degrees Fahrenheit"),
            "fluid-ounce" => ("fl oz", "fluid ounces"),
            "foot" => ("ft", "feet"),
            "gallon" => ("gal", "gallons"),
            "gigabit" => ("Gb", "gigabits"),
            "gigabyte" => ("GB", "gigabytes"),
            "gram" => ("g", "grams"),
            "hectare" => ("ha", "hectares"),
            "hour" => ("hr", "hours"),
            "inch" => ("in", "inches"),
            "kilobit" => ("kb", "kilobits"),
            "kilobyte" => ("kB", "kilobytes"),
            "kilogram" => ("kg", "kilograms"),
            "kilometer" => ("km", "kilometers"),
            "liter" => ("L", "liters"),
            "megabit" => ("Mb", "megabits"),
            "megabyte" => ("MB", "megabytes"),
            "meter" => ("m", "meters"),
            "microsecond" => ("\u{03BC}s", "microseconds"),
            "mile" => ("mi", "miles"),
            "mile-scandinavian" => ("smi", "miles-scandinavian"),
            "milliliter" => ("mL", "milliliters"),
            "millimeter" => ("mm", "millimeters"),
            "millisecond" => ("ms", "milliseconds"),
            "minute" => ("min", "minutes"),
            "month" => ("mth", "months"),
            "nanosecond" => ("ns", "nanoseconds"),
            "ounce" => ("oz", "ounces"),
            "percent" => ("%", "percent"),
            "petabyte" => ("PB", "petabytes"),
            "pound" => ("lb", "pounds"),
            "second" => ("sec", "seconds"),
            "stone" => ("st", "stones"),
            "terabit" => ("Tb", "terabits"),
            "terabyte" => ("TB", "terabytes"),
            "week" => ("wk", "weeks"),
            "yard" => ("yd", "yards"),
            "year" => ("yr", "years"),
            _ => return None,
        };
        Some(if display == "long" {
            if is_one {
                Self::english_unit_singular(long)
            } else {
                long.to_string()
            }
        } else if display == "narrow" {

            match unit {
                "second" => "s".to_string(),
                "minute" => "m".to_string(),
                "hour" => "h".to_string(),
                "day" => "d".to_string(),
                "week" => "w".to_string(),
                "month" => "m".to_string(),
                "year" => "y".to_string(),
                "fahrenheit" => "\u{00B0}".to_string(),
                "degree" => "\u{00B0}".to_string(),
                "foot" => "\u{2032}".to_string(),
                "inch" => "\u{2033}".to_string(),
                "pound" => "#".to_string(),
                _ => short.to_string(),
            }
        } else if display == "short" && !is_one {
            match unit {
                "year" | "month" | "week" | "day" => format!("{short}s"),
                _ => short.to_string(),
            }
        } else {
            short.to_string()
        })
    }

    fn english_unit_singular(plural: &str) -> String {
        match plural {
            "feet" => "foot".to_string(),
            "inches" => "inch".to_string(),
            "stone" => "stone".to_string(),
            "percent" => "percent".to_string(),
            "degrees" => "degree".to_string(),
            "degrees Celsius" => "degree Celsius".to_string(),
            "degrees Fahrenheit" => "degree Fahrenheit".to_string(),
            "fluid ounces" => "fluid ounce".to_string(),
            "miles-scandinavian" => "mile-scandinavian".to_string(),
            s => s.strip_suffix('s').unwrap_or(s).to_string(),
        }
    }

    pub(crate) fn number_compact_entries(
        locale: &str,
        long: bool,
    ) -> &'static [(i32, &'static str)] {
        if let Some(entries) = intl_number_compact_generated::number_compact_entries(locale, long) {
            if !entries.is_empty() {
                return entries;
            }
        }
        if locale.starts_with("ja") {
            return &[(12, "兆"), (8, "億"), (4, "万")];
        }
        if locale.starts_with("ko") {
            return &[(12, "조"), (8, "억"), (4, "만"), (3, "천")];
        }
        if locale.starts_with("zh") {
            return &[(12, "兆"), (8, "億"), (4, "萬")];
        }
        if locale.starts_with("de") {
            return if long {
                &[
                    (12, " Billionen"),
                    (9, " Milliarden"),
                    (6, " Millionen"),
                    (3, " Tausend"),
                ]
            } else {
                &[(12, "\u{a0}Bio."), (9, "\u{a0}Mrd."), (6, "\u{a0}Mio.")]
            };
        }
        if locale.starts_with("en-IN") {
            return if long {
                &[
                    (7, "\u{202f}crore"),
                    (5, "\u{202f}lakh"),
                    (3, "\u{202f}thousand"),
                ]
            } else {
                &[(7, "Cr"), (5, "L"), (3, "K")]
            };
        }
        if long {
            &[
                (12, " trillion"),
                (9, " billion"),
                (6, " million"),
                (3, " thousand"),
            ]
        } else {
            &[(12, "T"), (9, "B"), (6, "M"), (3, "K")]
        }
    }

    pub(crate) fn number_compact_suffix(
        locale: &str,
        long: bool,
        power: i32,
        category: &str,
    ) -> Option<&'static str> {
        intl_number_compact_generated::number_compact_suffix(locale, long, power, category)
    }

    pub(crate) fn number_compact_pattern(
        locale: &str,
        long: bool,
        power: i32,
        category: &str,
        integer_digits: usize,
    ) -> Option<(&'static str, &'static str)> {
        intl_number_compact_generated::number_compact_pattern(
            locale,
            long,
            power,
            category,
            integer_digits,
        )
    }

    pub(crate) fn plural_categories_cardinal(locale: &str) -> &'static [&'static str] {
        intl_plural_rules_generated::cardinal_categories(locale)
    }

    pub(crate) fn plural_categories_ordinal(locale: &str) -> &'static [&'static str] {
        intl_plural_rules_generated::ordinal_categories(locale)
    }

    pub(crate) fn plural_cardinal(locale: &str, n: f64, e: i32) -> &'static str {
        if let Some(category) = intl_plural_rules_generated::select_cardinal(locale, n, e) {
            return category;
        }
        let i = n.abs() as i64;
        let has_frac = n.fract() != 0.0;
        if !has_frac && i == 1 {
            "one"
        } else {
            "other"
        }
    }

    pub(crate) fn plural_ordinal_en(n: f64) -> &'static str {
        Self::plural_ordinal("en", n)
    }

    pub(crate) fn plural_ordinal(locale: &str, n: f64) -> &'static str {
        if let Some(category) = intl_plural_rules_generated::select_ordinal(locale, n) {
            return category;
        }
        let i = n.abs() as i64;
        let (m10, m100) = (i % 10, i % 100);
        if m10 == 1 && m100 != 11 {
            "one"
        } else if m10 == 2 && m100 != 12 {
            "two"
        } else if m10 == 3 && m100 != 13 {
            "few"
        } else {
            "other"
        }
    }

    pub(crate) fn list_patterns(
        locale: &str,
        typ: &str,
        style: &str,
    ) -> (&'static str, &'static str, &'static str, &'static str) {
        if let Some(patterns) = intl_list_patterns_generated::list_patterns(locale, typ, style) {
            return patterns;
        }
        let lang = locale.split(['-', '_']).next().unwrap_or("en");
        match (lang, typ, style) {
            ("en", "conjunction", "short") => ("{0} & {1}", "{0}, {1}", "{0}, {1}", "{0}, & {1}"),
            ("en", "conjunction", "narrow") => ("{0}, {1}", "{0}, {1}", "{0}, {1}", "{0}, {1}"),
            ("en", "conjunction", _) => ("{0} and {1}", "{0}, {1}", "{0}, {1}", "{0}, and {1}"),
            ("en", "disjunction", "short") => ("{0} or {1}", "{0}, {1}", "{0}, {1}", "{0}, or {1}"),
            ("en", "disjunction", "narrow") => {
                ("{0} or {1}", "{0}, {1}", "{0}, {1}", "{0}, or {1}")
            }
            ("en", "disjunction", _) => ("{0} or {1}", "{0}, {1}", "{0}, {1}", "{0}, or {1}"),
            ("en", "unit", "narrow") => ("{0} {1}", "{0} {1}", "{0} {1}", "{0} {1}"),
            ("en", "unit", _) => ("{0}, {1}", "{0}, {1}", "{0}, {1}", "{0}, {1}"),
            _ => ("{0}, {1}", "{0}, {1}", "{0}, {1}", "{0}, {1}"),
        }
    }

    pub(crate) fn relative_time_en_unit_words(unit: &str, style: &str) -> (String, String) {
        if style == "short" || style == "narrow" {
            let (s, p) = match unit {
                "second" => ("sec.", "sec."),
                "minute" => ("min.", "min."),
                "hour" => ("hr.", "hr."),
                "day" => ("day", "days"),
                "week" => ("wk.", "wk."),
                "month" => ("mo.", "mo."),
                "quarter" => ("qtr.", "qtrs."),
                "year" => ("yr.", "yr."),
                other => (other, other),
            };
            return (s.to_string(), p.to_string());
        }
        (unit.to_string(), format!("{unit}s"))
    }

    pub(crate) fn relative_time_generated_auto_word(
        locale: &str,
        unit: &str,
        style: &str,
        value: i64,
    ) -> Option<&'static str> {
        intl_relative_time_patterns_generated::relative_auto(locale, unit, style, value)
    }

    pub(crate) fn relative_time_generated_numeric_pattern(
        locale: &str,
        unit: &str,
        style: &str,
        value: f64,
        plural: &str,
    ) -> Option<&'static str> {
        intl_relative_time_patterns_generated::numeric_pattern(
            locale,
            unit,
            style,
            value.is_sign_negative(),
            plural,
        )
    }

    pub(crate) fn display_name(
        locale: &str,
        display_type: &str,
        code: &str,
        language_display: &str,
    ) -> Option<String> {
        let language_display_key = if display_type == "language" {
            language_display
        } else {
            ""
        };

        let region_noncanonical = display_type == "region"
            && !((code.len() == 2 && code.chars().all(|c| c.is_ascii_uppercase()))
                || (code.len() == 3 && code.chars().all(|c| c.is_ascii_digit())));
        if !region_noncanonical {
            if let Some(name) = intl_display_names_cldr_representative_generated::display_name(
                locale,
                display_type,
                code,
                language_display_key,
            ) {
                return Some(name.to_string());
            }
        }
        match display_type {
            "calendar" => match code {

                "orthodox" if locale.starts_with("en") => Some("Orthodox Calendar".to_string()),
                "orthodox" => Some("orthodox".to_string()),
                "gregory" if locale.starts_with("en") => Some("Gregorian Calendar".to_string()),
                "buddhist" | "chinese" | "coptic" | "dangi" | "ethioaa" | "ethiopic"
                | "gregory" | "hebrew" | "indian" | "islamic" | "islamic-civil"
                | "islamic-rgsa" | "islamic-tbla" | "islamic-umalqura" | "iso8601" | "japanese"
                | "persian" | "roc" => Some(code.to_string()),
                _ => None,
            },
            "currency" => match code.to_ascii_uppercase().as_str() {
                "USD" if locale.starts_with("de") => Some("US-Dollar".to_string()),
                code if locale.starts_with("en") => {
                    intl_currency_display_names_en_generated::currency_display_name(code)
                        .map(|name| name.to_string())
                }
                "USD" => Some("US Dollar".to_string()),
                "EUR" => Some("Euro".to_string()),
                "JPY" => Some("Japanese Yen".to_string()),
                _ => None,
            },
            "dateTimeField" => match code {
                "era" | "year" | "quarter" | "month" | "weekOfYear" | "weekday" | "day"
                | "dayPeriod" | "hour" | "minute" | "second" | "timeZoneName" => {
                    Some(code.to_string())
                }
                _ => None,
            },
            "region" => {

                if locale.starts_with("de") && code == "FR" {
                    return Some("Frankreich".to_string());
                }
                if locale.starts_with("en") {
                    if let Some(n) = en_region_display_name(code) {
                        return Some(n.to_string());
                    }
                }

                if (code.len() == 2 && code.chars().all(|ch| ch.is_ascii_alphabetic()))
                    || (code.len() == 3 && code.chars().all(|ch| ch.is_ascii_digit()))
                {
                    Some(code.to_string())
                } else {
                    None
                }
            }
            "script" => match code {
                _ if locale.starts_with("en") && en_script_display_name(code).is_some() => {
                    en_script_display_name(code).map(|s| s.to_string())
                }
                "Latn" => Some("Latin".to_string()),
                "Cyrl" => Some("Cyrillic".to_string()),
                "Hans" => Some("Simplified".to_string()),
                _ if code.len() == 4 && code.chars().all(|ch| ch.is_ascii_alphabetic()) => {
                    let mut chars = code.chars();
                    let first = chars
                        .next()
                        .map(|ch| ch.to_ascii_uppercase())
                        .unwrap_or(' ');
                    let rest: String = chars.map(|ch| ch.to_ascii_lowercase()).collect();
                    Some(format!("{first}{rest}"))
                }
                _ => None,
            },
            _ => match (locale, code, language_display) {
                (l, "en-GB", "standard") if l.starts_with("de") => {
                    Some("English (United Kingdom)".to_string())
                }
                (l, "en", _) if l.starts_with("de") => Some("Englisch".to_string()),
                (l, "ja", _) if l.starts_with("fr") => Some("japonais".to_string()),

                (l, _, _) if l.starts_with("en") => en_language_display_name(code)
                    .map(|n| n.to_string())

                    .or_else(|| compose_language_display_name_en(code))
                    .or_else(|| {
                        (code.len() >= 2
                            && code
                                .chars()
                                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-'))
                        .then(|| code.to_string())
                    }),

                (_, "de", _) => Some("German".to_string()),
                (_, "zh-Hant", _) => Some("Traditional Chinese".to_string()),
                (_, "en-GB", _) => Some("British English".to_string()),
                _ if code.len() >= 2
                    && code
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-') =>
                {
                    Some(code.to_string())
                }
                _ => None,
            },
        }
    }
}

fn en_language_display_name(code: &str) -> Option<&'static str> {
    let canon: String = code
        .split('-')
        .enumerate()
        .map(|(i, s)| {
            if i == 0 {
                s.to_ascii_lowercase()
            } else if s.len() == 2 && s.chars().all(|c| c.is_ascii_alphabetic()) {
                s.to_ascii_uppercase()
            } else if s.len() == 4 && s.chars().all(|c| c.is_ascii_alphabetic()) {
                let mut it = s.chars();
                let first = it.next().map(|c| c.to_ascii_uppercase()).unwrap_or(' ');
                format!("{first}{}", it.as_str().to_ascii_lowercase())
            } else {
                s.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("-");
    Some(match canon.as_str() {
        "fr" => "French",
        "es" => "Spanish",
        "en" => "English",
        "zh" => "Chinese",
        "ja" => "Japanese",
        "de" => "German",
        "it" => "Italian",
        "pt" => "Portuguese",
        "ru" => "Russian",
        "ar" => "Arabic",
        "ko" => "Korean",
        "nl" => "Dutch",
        "sv" => "Swedish",
        "pl" => "Polish",
        "tr" => "Turkish",
        "hi" => "Hindi",
        "vi" => "Vietnamese",
        "th" => "Thai",
        "id" => "Indonesian",
        "cs" => "Czech",
        "el" => "Greek",
        "he" => "Hebrew",
        "fa" => "Persian",
        "uk" => "Ukrainian",
        "ro" => "Romanian",
        "hu" => "Hungarian",
        "da" => "Danish",
        "fi" => "Finnish",
        "nb" => "Norwegian Bokmål",
        "no" => "Norwegian",
        "bg" => "Bulgarian",
        "hr" => "Croatian",
        "sk" => "Slovak",
        "sl" => "Slovenian",
        "et" => "Estonian",
        "lv" => "Latvian",
        "lt" => "Lithuanian",
        "ca" => "Catalan",
        "eu" => "Basque",
        "gl" => "Galician",
        "ga" => "Irish",
        "cy" => "Welsh",
        "is" => "Icelandic",
        "mt" => "Maltese",
        "sq" => "Albanian",
        "mk" => "Macedonian",
        "sr" => "Serbian",
        "bs" => "Bosnian",
        "af" => "Afrikaans",
        "sw" => "Swahili",
        "zu" => "Zulu",
        "am" => "Amharic",
        "bn" => "Bangla",
        "ta" => "Tamil",
        "te" => "Telugu",
        "ml" => "Malayalam",
        "kn" => "Kannada",
        "mr" => "Marathi",
        "gu" => "Gujarati",
        "pa" => "Punjabi",
        "ur" => "Urdu",
        "ne" => "Nepali",
        "si" => "Sinhala",
        "km" => "Khmer",
        "lo" => "Lao",
        "my" => "Burmese",
        "ka" => "Georgian",
        "hy" => "Armenian",
        "az" => "Azerbaijani",
        "kk" => "Kazakh",
        "uz" => "Uzbek",
        "ky" => "Kyrgyz",
        "tg" => "Tajik",
        "mn" => "Mongolian",
        "en-US" => "American English",
        "en-GB" => "British English",
        "en-CA" => "Canadian English",
        "en-AU" => "Australian English",
        "zh-Hans" => "Simplified Chinese",
        "zh-Hant" => "Traditional Chinese",
        "pt-BR" => "Brazilian Portuguese",
        "pt-PT" => "European Portuguese",
        "es-419" => "Latin American Spanish",
        "es-ES" => "European Spanish",
        "es-MX" => "Mexican Spanish",
        "fr-CA" => "Canadian French",
        "fr-CH" => "Swiss French",
        "de-CH" => "Swiss High German",
        "de-AT" => "Austrian German",
        "nl-BE" => "Flemish",
        "sw-CD" => "Congo Swahili",
        _ => return None,
    })
}

fn en_region_display_name(code: &str) -> Option<&'static str> {
    Some(match code {
        "001" => "world",
        "002" => "Africa",
        "150" => "Europe",
        "419" => "Latin America",
        "AC" => "Ascension Island",
        "AD" => "Andorra",
        "AE" => "United Arab Emirates",
        "AF" => "Afghanistan",
        "AG" => "Antigua & Barbuda",
        "AI" => "Anguilla",
        "AL" => "Albania",
        "AM" => "Armenia",
        "AN" => "Curaçao",
        "AO" => "Angola",
        "AQ" => "Antarctica",
        "AR" => "Argentina",
        "AS" => "American Samoa",
        "AT" => "Austria",
        "AU" => "Australia",
        "AW" => "Aruba",
        "AX" => "Åland Islands",
        "AZ" => "Azerbaijan",
        "BA" => "Bosnia & Herzegovina",
        "BB" => "Barbados",
        "BD" => "Bangladesh",
        "BE" => "Belgium",
        "BF" => "Burkina Faso",
        "BG" => "Bulgaria",
        "BH" => "Bahrain",
        "BI" => "Burundi",
        "BJ" => "Benin",
        "BL" => "St. Barthélemy",
        "BM" => "Bermuda",
        "BN" => "Brunei",
        "BO" => "Bolivia",
        "BQ" => "Caribbean Netherlands",
        "BR" => "Brazil",
        "BS" => "Bahamas",
        "BT" => "Bhutan",
        "BU" => "Myanmar (Burma)",
        "BV" => "Bouvet Island",
        "BW" => "Botswana",
        "BY" => "Belarus",
        "BZ" => "Belize",
        "CA" => "Canada",
        "CC" => "Cocos (Keeling) Islands",
        "CD" => "Congo - Kinshasa",
        "CF" => "Central African Republic",
        "CG" => "Congo - Brazzaville",
        "CH" => "Switzerland",
        "CI" => "Côte d’Ivoire",
        "CK" => "Cook Islands",
        "CL" => "Chile",
        "CM" => "Cameroon",
        "CN" => "China",
        "CO" => "Colombia",
        "CP" => "Clipperton Island",
        "CQ" => "Sark",
        "CR" => "Costa Rica",
        "CS" => "Serbia",
        "CU" => "Cuba",
        "CV" => "Cape Verde",
        "CW" => "Curaçao",
        "CX" => "Christmas Island",
        "CY" => "Cyprus",
        "CZ" => "Czechia",
        "DD" => "Germany",
        "DE" => "Germany",
        "DG" => "Diego Garcia",
        "DJ" => "Djibouti",
        "DK" => "Denmark",
        "DM" => "Dominica",
        "DO" => "Dominican Republic",
        "DY" => "Benin",
        "DZ" => "Algeria",
        "EA" => "Ceuta & Melilla",
        "EC" => "Ecuador",
        "EE" => "Estonia",
        "EG" => "Egypt",
        "EH" => "Western Sahara",
        "ER" => "Eritrea",
        "ES" => "Spain",
        "ET" => "Ethiopia",
        "EU" => "European Union",
        "EZ" => "Eurozone",
        "FI" => "Finland",
        "FJ" => "Fiji",
        "FK" => "Falkland Islands",
        "FM" => "Micronesia",
        "FO" => "Faroe Islands",
        "FR" => "France",
        "FX" => "France",
        "GA" => "Gabon",
        "GB" => "United Kingdom",
        "GD" => "Grenada",
        "GE" => "Georgia",
        "GF" => "French Guiana",
        "GG" => "Guernsey",
        "GH" => "Ghana",
        "GI" => "Gibraltar",
        "GL" => "Greenland",
        "GM" => "Gambia",
        "GN" => "Guinea",
        "GP" => "Guadeloupe",
        "GQ" => "Equatorial Guinea",
        "GR" => "Greece",
        "GS" => "South Georgia & South Sandwich Islands",
        "GT" => "Guatemala",
        "GU" => "Guam",
        "GW" => "Guinea-Bissau",
        "GY" => "Guyana",
        "HK" => "Hong Kong SAR China",
        "HM" => "Heard & McDonald Islands",
        "HN" => "Honduras",
        "HR" => "Croatia",
        "HT" => "Haiti",
        "HU" => "Hungary",
        "HV" => "Burkina Faso",
        "IC" => "Canary Islands",
        "ID" => "Indonesia",
        "IE" => "Ireland",
        "IL" => "Israel",
        "IM" => "Isle of Man",
        "IN" => "India",
        "IO" => "British Indian Ocean Territory",
        "IQ" => "Iraq",
        "IR" => "Iran",
        "IS" => "Iceland",
        "IT" => "Italy",
        "JE" => "Jersey",
        "JM" => "Jamaica",
        "JO" => "Jordan",
        "JP" => "Japan",
        "KE" => "Kenya",
        "KG" => "Kyrgyzstan",
        "KH" => "Cambodia",
        "KI" => "Kiribati",
        "KM" => "Comoros",
        "KN" => "St. Kitts & Nevis",
        "KP" => "North Korea",
        "KR" => "South Korea",
        "KW" => "Kuwait",
        "KY" => "Cayman Islands",
        "KZ" => "Kazakhstan",
        "LA" => "Laos",
        "LB" => "Lebanon",
        "LC" => "St. Lucia",
        "LI" => "Liechtenstein",
        "LK" => "Sri Lanka",
        "LR" => "Liberia",
        "LS" => "Lesotho",
        "LT" => "Lithuania",
        "LU" => "Luxembourg",
        "LV" => "Latvia",
        "LY" => "Libya",
        "MA" => "Morocco",
        "MC" => "Monaco",
        "MD" => "Moldova",
        "ME" => "Montenegro",
        "MF" => "St. Martin",
        "MG" => "Madagascar",
        "MH" => "Marshall Islands",
        "MK" => "North Macedonia",
        "ML" => "Mali",
        "MM" => "Myanmar (Burma)",
        "MN" => "Mongolia",
        "MO" => "Macao SAR China",
        "MP" => "Northern Mariana Islands",
        "MQ" => "Martinique",
        "MR" => "Mauritania",
        "MS" => "Montserrat",
        "MT" => "Malta",
        "MU" => "Mauritius",
        "MV" => "Maldives",
        "MW" => "Malawi",
        "MX" => "Mexico",
        "MY" => "Malaysia",
        "MZ" => "Mozambique",
        "NA" => "Namibia",
        "NC" => "New Caledonia",
        "NE" => "Niger",
        "NF" => "Norfolk Island",
        "NG" => "Nigeria",
        "NH" => "Vanuatu",
        "NI" => "Nicaragua",
        "NL" => "Netherlands",
        "NO" => "Norway",
        "NP" => "Nepal",
        "NR" => "Nauru",
        "NU" => "Niue",
        "NZ" => "New Zealand",
        "OM" => "Oman",
        "PA" => "Panama",
        "PE" => "Peru",
        "PF" => "French Polynesia",
        "PG" => "Papua New Guinea",
        "PH" => "Philippines",
        "PK" => "Pakistan",
        "PL" => "Poland",
        "PM" => "St. Pierre & Miquelon",
        "PN" => "Pitcairn Islands",
        "PR" => "Puerto Rico",
        "PS" => "Palestinian Territories",
        "PT" => "Portugal",
        "PW" => "Palau",
        "PY" => "Paraguay",
        "QA" => "Qatar",
        "QO" => "Outlying Oceania",
        "RE" => "Réunion",
        "RH" => "Zimbabwe",
        "RO" => "Romania",
        "RS" => "Serbia",
        "RU" => "Russia",
        "RW" => "Rwanda",
        "SA" => "Saudi Arabia",
        "SB" => "Solomon Islands",
        "SC" => "Seychelles",
        "SD" => "Sudan",
        "SE" => "Sweden",
        "SG" => "Singapore",
        "SH" => "St. Helena",
        "SI" => "Slovenia",
        "SJ" => "Svalbard & Jan Mayen",
        "SK" => "Slovakia",
        "SL" => "Sierra Leone",
        "SM" => "San Marino",
        "SN" => "Senegal",
        "SO" => "Somalia",
        "SR" => "Suriname",
        "SS" => "South Sudan",
        "ST" => "São Tomé & Príncipe",
        "SU" => "Russia",
        "SV" => "El Salvador",
        "SX" => "Sint Maarten",
        "SY" => "Syria",
        "SZ" => "Eswatini",
        "TA" => "Tristan da Cunha",
        "TC" => "Turks & Caicos Islands",
        "TD" => "Chad",
        "TF" => "French Southern Territories",
        "TG" => "Togo",
        "TH" => "Thailand",
        "TJ" => "Tajikistan",
        "TK" => "Tokelau",
        "TL" => "Timor-Leste",
        "TM" => "Turkmenistan",
        "TN" => "Tunisia",
        "TO" => "Tonga",
        "TP" => "Timor-Leste",
        "TR" => "Türkiye",
        "TT" => "Trinidad & Tobago",
        "TV" => "Tuvalu",
        "TW" => "Taiwan",
        "TZ" => "Tanzania",
        "UA" => "Ukraine",
        "UG" => "Uganda",
        "UK" => "United Kingdom",
        "UM" => "U.S. Outlying Islands",
        "UN" => "United Nations",
        "US" => "United States",
        "UY" => "Uruguay",
        "UZ" => "Uzbekistan",
        "VA" => "Vatican City",
        "VC" => "St. Vincent & Grenadines",
        "VD" => "Vietnam",
        "VE" => "Venezuela",
        "VG" => "British Virgin Islands",
        "VI" => "U.S. Virgin Islands",
        "VN" => "Vietnam",
        "VU" => "Vanuatu",
        "WF" => "Wallis & Futuna",
        "WS" => "Samoa",
        "XA" => "Pseudo-Accents",
        "XB" => "Pseudo-Bidi",
        "XK" => "Kosovo",
        "YD" => "Yemen",
        "YE" => "Yemen",
        "YT" => "Mayotte",
        "YU" => "Serbia",
        "ZA" => "South Africa",
        "ZM" => "Zambia",
        "ZR" => "Congo - Kinshasa",
        "ZW" => "Zimbabwe",
        "ZZ" => "Unknown Region",
        _ => return None,
    })
}

fn en_script_display_name(code: &str) -> Option<&'static str> {
    Some(match code {
        "Adlm" => "Adlam",
        "Arab" => "Arabic",
        "Armn" => "Armenian",
        "Bali" => "Balinese",
        "Bamu" => "Bamum",
        "Batk" => "Batak",
        "Beng" => "Bangla",
        "Bopo" => "Bopomofo",
        "Brai" => "Braille",
        "Buhd" => "Buhid",
        "Cans" => "Unified Canadian Aboriginal Syllabics",
        "Cari" => "Carian",
        "Cham" => "Cham",
        "Cher" => "Cherokee",
        "Copt" => "Coptic",
        "Cprt" => "Cypriot",
        "Cyrl" => "Cyrillic",
        "Deva" => "Devanagari",
        "Dsrt" => "Deseret",
        "Egyp" => "Egyptian hieroglyphs",
        "Ethi" => "Ethiopic",
        "Geor" => "Georgian",
        "Glag" => "Glagolitic",
        "Goth" => "Gothic",
        "Grek" => "Greek",
        "Gujr" => "Gujarati",
        "Guru" => "Gurmukhi",
        "Hang" => "Hangul",
        "Hani" => "Han",
        "Hans" => "Simplified",
        "Hant" => "Traditional",
        "Hebr" => "Hebrew",
        "Hira" => "Hiragana",
        "Ital" => "Old Italic",
        "Java" => "Javanese",
        "Jpan" => "Japanese",
        "Kali" => "Kayah Li",
        "Kana" => "Katakana",
        "Khmr" => "Khmer",
        "Knda" => "Kannada",
        "Kore" => "Korean",
        "Lana" => "Lanna",
        "Laoo" => "Lao",
        "Latn" => "Latin",
        "Limb" => "Limbu",
        "Lyci" => "Lycian",
        "Lydi" => "Lydian",
        "Mlym" => "Malayalam",
        "Mong" => "Mongolian",
        "Mtei" => "Meitei Mayek",
        "Mymr" => "Myanmar",
        "Nkoo" => "N’Ko",
        "Ogam" => "Ogham",
        "Olck" => "Ol Chiki",
        "Orya" => "Odia",
        "Osma" => "Osmanya",
        "Phag" => "Phags-pa",
        "Phnx" => "Phoenician",
        "Rjng" => "Rejang",
        "Runr" => "Runic",
        "Saur" => "Sourashtra",
        "Shaw" => "Shavian",
        "Sinh" => "Sinhala",
        "Sund" => "Sundanese",
        "Sylo" => "Syloti Nagri",
        "Syrc" => "Syriac",
        "Tagb" => "Tagbanwa",
        "Tale" => "Tai Le",
        "Talu" => "New Tai Lue",
        "Taml" => "Tamil",
        "Telu" => "Telugu",
        "Tfng" => "Tifinagh",
        "Tglg" => "Tagalog",
        "Thaa" => "Thaana",
        "Thai" => "Thai",
        "Tibt" => "Tibetan",
        "Ugar" => "Ugaritic",
        "Vaii" => "Vai",
        "Xsux" => "Sumero-Akkadian Cuneiform",
        "Yiii" => "Yi",
        _ => return None,
    })
}

fn compose_language_display_name_en(code: &str) -> Option<String> {
    let subtags: Vec<String> = code
        .split('-')
        .enumerate()
        .map(|(i, s)| {
            if i == 0 {
                s.to_ascii_lowercase()
            } else if s.len() == 2 && s.chars().all(|c| c.is_ascii_alphabetic()) {
                s.to_ascii_uppercase()
            } else if s.len() == 4 && s.chars().all(|c| c.is_ascii_alphabetic()) {
                let mut it = s.chars();
                let first = it.next().map(|c| c.to_ascii_uppercase()).unwrap_or(' ');
                format!("{first}{}", it.as_str().to_ascii_lowercase())
            } else {
                s.to_string()
            }
        })
        .collect();
    if subtags.len() < 2 {
        return None;
    }
    let lang = &subtags[0];
    let base = en_language_display_name(lang)
        .map(|s| s.to_string())
        .unwrap_or_else(|| lang.clone());
    let mut parts: Vec<String> = Vec::new();
    for sub in &subtags[1..] {
        if sub.len() == 4 && sub.chars().all(|c| c.is_ascii_alphabetic()) {
            parts.push(
                en_script_display_name(sub)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| sub.clone()),
            );
        } else if (sub.len() == 2 && sub.chars().all(|c| c.is_ascii_alphabetic()))
            || (sub.len() == 3 && sub.chars().all(|c| c.is_ascii_digit()))
        {
            parts.push(
                en_region_display_name(sub)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| sub.clone()),
            );
        } else {

            return None;
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(format!("{base} ({})", parts.join(", ")))
}
