
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiClass {
    L,
    R,
    AL,
    EN,
    AN,
    NSM,
    ES,
    CS,
    ET,
    ON,
    BN,
    Other,
}

pub fn bidi_class(c: char) -> BidiClass {
    crate::bidi_tables::bidi_class(c as u32)
}

pub fn is_bidi_label(label: &str) -> bool {
    label
        .chars()
        .any(|c| matches!(bidi_class(c), BidiClass::R | BidiClass::AL | BidiClass::AN))
}

pub fn check_bidi_label(label: &str) -> bool {
    if label.is_empty() {
        return true;
    }
    if !is_bidi_label(label) {

        return true;
    }

    let classes: Vec<BidiClass> = label.chars().map(bidi_class).collect();
    let first = classes[0];

    let rtl = match first {
        BidiClass::R | BidiClass::AL => true,
        BidiClass::L => false,
        _ => return false,
    };

    let last_strong = classes
        .iter()
        .rev()
        .find(|c| **c != BidiClass::NSM)
        .copied()
        .unwrap_or(first);

    if rtl {

        for c in &classes {
            match c {
                BidiClass::L | BidiClass::Other => return false,
                _ => {}
            }
        }

        if !matches!(
            last_strong,
            BidiClass::R | BidiClass::AL | BidiClass::EN | BidiClass::AN
        ) {
            return false;
        }

        let has_en = classes.iter().any(|c| *c == BidiClass::EN);
        let has_an = classes.iter().any(|c| *c == BidiClass::AN);
        if has_en && has_an {
            return false;
        }
    } else {

        for c in &classes {
            match c {
                BidiClass::R | BidiClass::AL | BidiClass::AN | BidiClass::Other => return false,
                _ => {}
            }
        }

        if !matches!(last_strong, BidiClass::L | BidiClass::EN) {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_ltr_ok() {
        assert!(check_bidi_label("bücher"));
        assert!(check_bidi_label("example"));
    }

    #[test]
    fn non_bidi_label_trivially_ok() {
        assert!(!is_bidi_label("münchen"));
        assert!(check_bidi_label("münchen"));
    }

    #[test]
    fn rtl_hebrew_label_detected() {

        assert!(is_bidi_label("\u{05D0}\u{05D1}"));
        assert!(check_bidi_label("\u{05D0}\u{05D1}"));
    }

    #[test]
    fn rtl_label_with_ltr_char_violates() {

        assert!(!check_bidi_label("\u{05D0}a\u{05D1}"));
    }

    #[test]
    fn rtl_label_en_and_an_violates() {

        assert!(!check_bidi_label("\u{05D0}1\u{0661}"));
    }
}
