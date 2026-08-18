
#[derive(Clone, Copy)]
pub(crate) struct NumberSymbols {
    pub(crate) group: &'static str,
    pub(crate) decimal: &'static str,
    pub(crate) percent: &'static str,
    pub(crate) minus: &'static str,
    pub(crate) percent_glyph: &'static str,
    pub(crate) minus_glyph: &'static str,
}

static NUMBER_SYMBOL_ROWS: &[(&str, NumberSymbols)] = &[
    (
        "af",
        NumberSymbols {
            group: " ",
            decimal: ",",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "am",
        NumberSymbols {
            group: ",",
            decimal: ".",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "ar",
        NumberSymbols {
            group: "٬",
            decimal: "٫",
            percent: "٪؜",
            minus: "؜-",
            percent_glyph: "٪",
            minus_glyph: "-",
        },
    ),
    (
        "az",
        NumberSymbols {
            group: ".",
            decimal: ",",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "bg",
        NumberSymbols {
            group: " ",
            decimal: ",",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "bn",
        NumberSymbols {
            group: ",",
            decimal: ".",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "ca",
        NumberSymbols {
            group: ".",
            decimal: ",",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "cs",
        NumberSymbols {
            group: " ",
            decimal: ",",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "da",
        NumberSymbols {
            group: ".",
            decimal: ",",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "de",
        NumberSymbols {
            group: ".",
            decimal: ",",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "el",
        NumberSymbols {
            group: ".",
            decimal: ",",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "en",
        NumberSymbols {
            group: ",",
            decimal: ".",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "es",
        NumberSymbols {
            group: ".",
            decimal: ",",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "fa",
        NumberSymbols {
            group: "٬",
            decimal: "٫",
            percent: "٪",
            minus: "‎−",
            percent_glyph: "٪",
            minus_glyph: "−",
        },
    ),
    (
        "fi",
        NumberSymbols {
            group: " ",
            decimal: ",",
            percent: "%",
            minus: "−",
            percent_glyph: "%",
            minus_glyph: "−",
        },
    ),
    (
        "fil",
        NumberSymbols {
            group: ",",
            decimal: ".",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "fr",
        NumberSymbols {
            group: " ",
            decimal: ",",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "ha",
        NumberSymbols {
            group: ",",
            decimal: ".",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "he",
        NumberSymbols {
            group: ",",
            decimal: ".",
            percent: "%",
            minus: "‎-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "hi",
        NumberSymbols {
            group: ",",
            decimal: ".",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "id",
        NumberSymbols {
            group: ".",
            decimal: ",",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "it",
        NumberSymbols {
            group: ".",
            decimal: ",",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "ja",
        NumberSymbols {
            group: ",",
            decimal: ".",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "ko",
        NumberSymbols {
            group: ",",
            decimal: ".",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "ku",
        NumberSymbols {
            group: ".",
            decimal: ",",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "nb",
        NumberSymbols {
            group: " ",
            decimal: ",",
            percent: "%",
            minus: "−",
            percent_glyph: "%",
            minus_glyph: "−",
        },
    ),
    (
        "nn",
        NumberSymbols {
            group: " ",
            decimal: ",",
            percent: "%",
            minus: "−",
            percent_glyph: "%",
            minus_glyph: "−",
        },
    ),
    (
        "no",
        NumberSymbols {
            group: " ",
            decimal: ",",
            percent: "%",
            minus: "−",
            percent_glyph: "%",
            minus_glyph: "−",
        },
    ),
    (
        "pa",
        NumberSymbols {
            group: ",",
            decimal: ".",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "pl",
        NumberSymbols {
            group: " ",
            decimal: ",",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "pt",
        NumberSymbols {
            group: ".",
            decimal: ",",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),

    (
        "pt-PT",
        NumberSymbols {
            group: "\u{a0}",
            decimal: ",",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "ro",
        NumberSymbols {
            group: ".",
            decimal: ",",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "ru",
        NumberSymbols {
            group: " ",
            decimal: ",",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "sr",
        NumberSymbols {
            group: ".",
            decimal: ",",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "sv",
        NumberSymbols {
            group: " ",
            decimal: ",",
            percent: "%",
            minus: "−",
            percent_glyph: "%",
            minus_glyph: "−",
        },
    ),
    (
        "th",
        NumberSymbols {
            group: ",",
            decimal: ".",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "tr",
        NumberSymbols {
            group: ".",
            decimal: ",",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "uk",
        NumberSymbols {
            group: " ",
            decimal: ",",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "uz",
        NumberSymbols {
            group: " ",
            decimal: ",",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "vi",
        NumberSymbols {
            group: ".",
            decimal: ",",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "yi",
        NumberSymbols {
            group: ",",
            decimal: ".",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
    (
        "zh",
        NumberSymbols {
            group: ",",
            decimal: ".",
            percent: "%",
            minus: "-",
            percent_glyph: "%",
            minus_glyph: "-",
        },
    ),
];

fn primary_subtag(locale: &str) -> &str {
    locale
        .split(|ch| ch == '-' || ch == '_')
        .next()
        .filter(|part| !part.is_empty())
        .unwrap_or(locale)
}

pub(crate) fn number_symbols(locale: &str) -> Option<NumberSymbols> {

    if let Ok(idx) = NUMBER_SYMBOL_ROWS.binary_search_by(|(candidate, _)| candidate.cmp(&locale)) {
        return Some(NUMBER_SYMBOL_ROWS[idx].1);
    }
    let primary = primary_subtag(locale);
    NUMBER_SYMBOL_ROWS
        .binary_search_by(|(candidate, _)| candidate.cmp(&primary))
        .ok()
        .map(|idx| NUMBER_SYMBOL_ROWS[idx].1)
}
