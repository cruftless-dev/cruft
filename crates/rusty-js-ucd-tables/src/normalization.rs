use crate::{
    normalization_tables, L_BASE, L_COUNT, N_COUNT, S_BASE, S_COUNT, T_BASE, T_COUNT, V_BASE,
    V_COUNT,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalizationForm {
    Nfc,
    Nfd,
    Nfkc,
    Nfkd,
}

impl NormalizationForm {
    pub fn parse(form: &str) -> Option<Self> {
        match form {
            "NFC" => Some(Self::Nfc),
            "NFD" => Some(Self::Nfd),
            "NFKC" => Some(Self::Nfkc),
            "NFKD" => Some(Self::Nfkd),
            _ => None,
        }
    }

    fn compatibility(self) -> bool {
        matches!(self, Self::Nfkc | Self::Nfkd)
    }

    fn compose(self) -> bool {
        matches!(self, Self::Nfc | Self::Nfkc)
    }
}

pub fn normalize_str(input: &str, form: NormalizationForm) -> String {
    let mut lowered = Vec::with_capacity(input.len());
    for ch in input.chars() {
        push_decomposition(ch as u32, form.compatibility(), &mut lowered);
    }
    canonical_order(&mut lowered);
    if form.compose() {
        lowered = canonical_compose(lowered);
    }
    lowered
        .into_iter()
        .filter_map(char::from_u32)
        .collect::<String>()
}

fn push_decomposition(cp: u32, compatibility: bool, out: &mut Vec<u32>) {
    if let Some(mapping) = hangul_decomposition(cp) {
        for mapped in mapping {
            push_decomposition(mapped, compatibility, out);
        }
        return;
    }

    let mapping = if compatibility {
        normalization_tables::compatibility_decomposition(cp)
            .or_else(|| normalization_tables::canonical_decomposition(cp))
    } else {
        normalization_tables::canonical_decomposition(cp)
    };

    if let Some(mapping) = mapping {
        for mapped in mapping {
            push_decomposition(*mapped, compatibility, out);
        }
    } else {
        out.push(cp);
    }
}

fn hangul_decomposition(cp: u32) -> Option<Vec<u32>> {
    if !(S_BASE..S_BASE + S_COUNT).contains(&cp) {
        return None;
    }
    let s_index = cp - S_BASE;
    let l = L_BASE + (s_index / N_COUNT);
    let v = V_BASE + ((s_index % N_COUNT) / T_COUNT);
    let t_index = s_index % T_COUNT;
    if t_index == 0 {
        Some(vec![l, v])
    } else {
        Some(vec![l, v, T_BASE + t_index])
    }
}

fn canonical_order(codepoints: &mut [u32]) {
    for i in 1..codepoints.len() {
        let mut j = i;
        while j > 0 {
            let ccc = normalization_tables::canonical_combining_class(codepoints[j]);
            let prev_ccc = normalization_tables::canonical_combining_class(codepoints[j - 1]);
            if ccc == 0 || prev_ccc <= ccc {
                break;
            }
            codepoints.swap(j - 1, j);
            j -= 1;
        }
    }
}

fn canonical_compose(decomposed: Vec<u32>) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::with_capacity(decomposed.len());
    let mut starter_pos: Option<usize> = None;
    let mut last_ccc = 0u8;

    for cp in decomposed {
        let ccc = normalization_tables::canonical_combining_class(cp);
        if let Some(pos) = starter_pos {
            let starter = out[pos];
            if let Some(composed) = compose_pair(starter, cp) {
                if last_ccc == 0 || last_ccc < ccc {
                    out[pos] = composed;
                    continue;
                }
            }
        }

        if ccc == 0 {
            starter_pos = Some(out.len());
        }
        out.push(cp);
        last_ccc = ccc;
    }

    out
}

fn compose_pair(starter: u32, combining: u32) -> Option<u32> {
    hangul_composition(starter, combining)
        .or_else(|| normalization_tables::canonical_composition(starter, combining))
}

fn hangul_composition(starter: u32, combining: u32) -> Option<u32> {
    if (L_BASE..L_BASE + L_COUNT).contains(&starter)
        && (V_BASE..V_BASE + V_COUNT).contains(&combining)
    {
        let l_index = starter - L_BASE;
        let v_index = combining - V_BASE;
        return Some(S_BASE + (l_index * V_COUNT + v_index) * T_COUNT);
    }

    if (S_BASE..S_BASE + S_COUNT).contains(&starter)
        && (starter - S_BASE) % T_COUNT == 0
        && (T_BASE + 1..T_BASE + T_COUNT).contains(&combining)
    {
        return Some(starter + (combining - T_BASE));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_canonical_decomposition_and_composition() {
        assert_eq!(
            normalize_str("\u{00E9}", NormalizationForm::Nfd),
            "e\u{0301}"
        );
        assert_eq!(
            normalize_str("e\u{0301}", NormalizationForm::Nfc),
            "\u{00E9}"
        );
    }

    #[test]
    fn normalizes_compatibility_forms() {
        assert_eq!(normalize_str("\u{2460}", NormalizationForm::Nfkd), "1");
        assert_eq!(normalize_str("\u{FB03}", NormalizationForm::Nfkc), "ffi");
    }

    #[test]
    fn canonical_order_reorders_nonstarters_stably() {
        assert_eq!(
            normalize_str("a\u{0315}\u{0300}", NormalizationForm::Nfd),
            "a\u{0300}\u{0315}"
        );
    }

    #[test]
    fn normalizes_hangul_algorithmically() {
        assert_eq!(
            normalize_str("\u{AC01}", NormalizationForm::Nfd),
            "\u{1100}\u{1161}\u{11A8}"
        );
        assert_eq!(
            normalize_str("\u{1100}\u{1161}\u{11A8}", NormalizationForm::Nfc),
            "\u{AC01}"
        );
    }

    #[test]
    fn parses_ecma_normalization_form_names() {
        assert_eq!(
            NormalizationForm::parse("NFC"),
            Some(NormalizationForm::Nfc)
        );
        assert_eq!(
            NormalizationForm::parse("NFD"),
            Some(NormalizationForm::Nfd)
        );
        assert_eq!(
            NormalizationForm::parse("NFKC"),
            Some(NormalizationForm::Nfkc)
        );
        assert_eq!(
            NormalizationForm::parse("NFKD"),
            Some(NormalizationForm::Nfkd)
        );
        assert_eq!(NormalizationForm::parse("nfc"), None);
    }

    #[test]
    fn optional_official_normalization_test_prefix() {
        let Ok(path) = std::env::var("RUSTY_JS_UCD_NORMALIZATION_TEST") else {
            return;
        };
        let max = std::env::var("RUSTY_JS_UCD_NORMALIZATION_TEST_LIMIT")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(512);
        let text = std::fs::read_to_string(path).expect("read NormalizationTest.txt");
        let mut checked = 0usize;
        for line in text.lines() {
            let body = line.split('#').next().unwrap_or("").trim();
            if body.is_empty() || body.starts_with('@') {
                continue;
            }
            let fields: Vec<String> = body
                .split(';')
                .take(5)
                .map(|field| {
                    field
                        .split_whitespace()
                        .map(|cp| {
                            char::from_u32(u32::from_str_radix(cp, 16).expect("hex scalar"))
                                .expect("Unicode scalar")
                        })
                        .collect()
                })
                .collect();
            if fields.len() != 5 {
                continue;
            }
            let c1 = &fields[0];
            let c2 = &fields[1];
            let c3 = &fields[2];
            let c4 = &fields[3];
            let c5 = &fields[4];
            assert_eq!(normalize_str(c1, NormalizationForm::Nfc), *c2);
            assert_eq!(normalize_str(c2, NormalizationForm::Nfc), *c2);
            assert_eq!(normalize_str(c3, NormalizationForm::Nfc), *c2);
            assert_eq!(normalize_str(c4, NormalizationForm::Nfc), *c4);
            assert_eq!(normalize_str(c5, NormalizationForm::Nfc), *c4);
            assert_eq!(normalize_str(c1, NormalizationForm::Nfd), *c3);
            assert_eq!(normalize_str(c2, NormalizationForm::Nfd), *c3);
            assert_eq!(normalize_str(c3, NormalizationForm::Nfd), *c3);
            assert_eq!(normalize_str(c4, NormalizationForm::Nfd), *c5);
            assert_eq!(normalize_str(c5, NormalizationForm::Nfd), *c5);
            assert_eq!(normalize_str(c1, NormalizationForm::Nfkc), *c4);
            assert_eq!(normalize_str(c2, NormalizationForm::Nfkc), *c4);
            assert_eq!(normalize_str(c3, NormalizationForm::Nfkc), *c4);
            assert_eq!(normalize_str(c4, NormalizationForm::Nfkc), *c4);
            assert_eq!(normalize_str(c5, NormalizationForm::Nfkc), *c4);
            assert_eq!(normalize_str(c1, NormalizationForm::Nfkd), *c5);
            assert_eq!(normalize_str(c2, NormalizationForm::Nfkd), *c5);
            assert_eq!(normalize_str(c3, NormalizationForm::Nfkd), *c5);
            assert_eq!(normalize_str(c4, NormalizationForm::Nfkd), *c5);
            assert_eq!(normalize_str(c5, NormalizationForm::Nfkd), *c5);
            checked += 1;
            if checked >= max {
                break;
            }
        }
        assert_eq!(checked, max);
    }
}
