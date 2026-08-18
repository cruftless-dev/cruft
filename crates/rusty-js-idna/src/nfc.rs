
pub fn normalize_nfc(input: &str) -> String {
    rusty_js_ucd_tables::normalize_str(input, rusty_js_ucd_tables::NormalizationForm::Nfc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composes_decomposed_u_diaeresis() {
        assert_eq!(normalize_nfc("mu\u{308}nchen"), "münchen");
    }

    #[test]
    fn precomposed_passthrough() {
        assert_eq!(normalize_nfc("münchen"), "münchen");
    }

    #[test]
    fn ascii_passthrough() {
        assert_eq!(normalize_nfc("example.com"), "example.com");
    }

    #[test]
    fn lone_mark_passthrough() {
        assert_eq!(normalize_nfc("\u{308}x"), "\u{308}x");
    }

    #[test]
    fn composes_non_latin_canonical_pair() {
        assert_eq!(normalize_nfc("\u{03B1}\u{0313}"), "\u{1F00}");
    }
}
