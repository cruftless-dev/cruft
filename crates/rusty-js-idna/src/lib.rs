
pub mod bidi;
pub mod bidi_tables;
pub mod nfc;
pub mod uts46;
pub mod uts46_table;

use uts46::{map_char_with, Disposition, StdCaseFoldResidual, Uts46Residual};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdnaError {

    DisallowedCodePoint(char),

    BidiRuleViolation,

    Punycode,

    EmptyLabel,

    HyphenRuleViolation,

    LeadingCombiningMark,

    ContextRuleViolation,

    LabelTooLong,

    DomainTooLong,
}

fn map_and_normalize<R: Uts46Residual>(input: &str, residual: &R) -> Result<String, IdnaError> {
    let mut mapped = String::with_capacity(input.len());
    for c in input.chars() {
        match map_char_with(c, residual) {
            Disposition::Valid => mapped.push(c),
            Disposition::Mapped(s) => mapped.push_str(&s),
            Disposition::Ignored => {}
            Disposition::Disallowed => return Err(IdnaError::DisallowedCodePoint(c)),
        }
    }
    Ok(nfc::normalize_nfc(&mapped))
}

fn is_combining_mark(c: char) -> bool {
    matches!(
        c,
        '\u{0300}'..='\u{036F}'
            | '\u{0483}'..='\u{0489}'
            | '\u{0591}'..='\u{05BD}'
            | '\u{05BF}'
            | '\u{05C1}'..='\u{05C2}'
            | '\u{05C4}'..='\u{05C5}'
            | '\u{05C7}'
            | '\u{0610}'..='\u{061A}'
            | '\u{064B}'..='\u{065F}'
            | '\u{0670}'
            | '\u{06D6}'..='\u{06DC}'
            | '\u{06DF}'..='\u{06E4}'
            | '\u{06E7}'..='\u{06E8}'
            | '\u{06EA}'..='\u{06ED}'
    )
}

fn is_greek(c: char) -> bool {
    matches!(c, '\u{0370}'..='\u{03FF}' | '\u{1F00}'..='\u{1FFF}')
}

fn is_hebrew(c: char) -> bool {
    matches!(c, '\u{0590}'..='\u{05FF}' | '\u{FB1D}'..='\u{FB4F}')
}

fn is_hiragana_katakana_or_han(c: char) -> bool {
    matches!(
        c,
        '\u{3040}'..='\u{30FF}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{F900}'..='\u{FAFF}'
    )
}

fn validate_context(label: &str) -> Result<(), IdnaError> {
    let chars: Vec<char> = label.chars().collect();
    for (i, c) in chars.iter().copied().enumerate() {
        match c {

            '\u{200C}' | '\u{200D}' => return Err(IdnaError::ContextRuleViolation),

            '\u{00B7}' => {
                if i == 0 || i + 1 >= chars.len() || chars[i - 1] != 'l' || chars[i + 1] != 'l' {
                    return Err(IdnaError::ContextRuleViolation);
                }
            }

            '\u{0375}' => {
                if i == 0 || !is_greek(chars[i - 1]) {
                    return Err(IdnaError::ContextRuleViolation);
                }
            }

            '\u{05F3}' | '\u{05F4}' => {
                if i == 0 || !is_hebrew(chars[i - 1]) {
                    return Err(IdnaError::ContextRuleViolation);
                }
            }

            '\u{30FB}' => {
                if !chars.iter().copied().any(is_hiragana_katakana_or_han) {
                    return Err(IdnaError::ContextRuleViolation);
                }
            }
            _ => {}
        }
    }

    let has_arabic_indic = chars.iter().any(|c| matches!(c, '\u{0660}'..='\u{0669}'));
    let has_extended_arabic_indic = chars.iter().any(|c| matches!(c, '\u{06F0}'..='\u{06F9}'));
    if has_arabic_indic && has_extended_arabic_indic {
        return Err(IdnaError::ContextRuleViolation);
    }
    Ok(())
}

fn validate_label_before_ace(label: &str, check_hyphens: bool) -> Result<(), IdnaError> {
    if label.is_empty() {
        return Err(IdnaError::EmptyLabel);
    }
    let bytes = label.as_bytes();

    if check_hyphens {
        if bytes.first() == Some(&b'-') || bytes.last() == Some(&b'-') {
            return Err(IdnaError::HyphenRuleViolation);
        }
        if bytes.len() >= 4 && bytes[2] == b'-' && bytes[3] == b'-' {
            return Err(IdnaError::HyphenRuleViolation);
        }
    }
    if label.chars().next().is_some_and(is_combining_mark) {
        return Err(IdnaError::LeadingCombiningMark);
    }
    validate_context(label)
}

pub fn to_ascii(domain: &str) -> Result<String, IdnaError> {
    to_ascii_with(domain, &StdCaseFoldResidual)
}

pub fn to_ascii_with<R: Uts46Residual>(domain: &str, residual: &R) -> Result<String, IdnaError> {
    to_ascii_impl(domain, residual, true)
}

pub fn to_ascii_url(domain: &str) -> Result<String, IdnaError> {
    to_ascii_impl(domain, &StdCaseFoldResidual, false)
}

fn to_ascii_impl<R: Uts46Residual>(
    domain: &str,
    residual: &R,
    check_hyphens: bool,
) -> Result<String, IdnaError> {
    let normalized = map_and_normalize(domain, residual)?;
    let mut out_labels = Vec::new();
    for label in normalized.split('.') {

        if label.is_empty() && !check_hyphens {
            out_labels.push(String::new());
            continue;
        }
        validate_label_before_ace(label, check_hyphens)?;
        if !bidi::check_bidi_label(label) {
            return Err(IdnaError::BidiRuleViolation);
        }

        if let Some(rest) = label.strip_prefix("xn--") {
            let decoded = rusty_js_punycode::decode(rest).map_err(|_| IdnaError::Punycode)?;
            if decoded.is_empty() {
                return Err(IdnaError::Punycode);
            }

            let renorm = map_and_normalize(&decoded, residual)?;
            validate_label_before_ace(&renorm, check_hyphens)?;
            let reencoded =
                rusty_js_punycode::label_to_ascii(&renorm).map_err(|_| IdnaError::Punycode)?;
            if !reencoded.eq_ignore_ascii_case(label) {
                return Err(IdnaError::Punycode);
            }
        }
        let ace = rusty_js_punycode::label_to_ascii(label).map_err(|_| IdnaError::Punycode)?;
        if ace.len() > 63 {
            return Err(IdnaError::LabelTooLong);
        }
        out_labels.push(ace);
    }
    let out = out_labels.join(".");
    if out.len() > 253 {
        return Err(IdnaError::DomainTooLong);
    }
    Ok(out)
}

pub fn to_unicode(domain: &str) -> Result<String, IdnaError> {
    to_unicode_with(domain, &StdCaseFoldResidual)
}

pub fn to_unicode_with<R: Uts46Residual>(domain: &str, residual: &R) -> Result<String, IdnaError> {
    let normalized = map_and_normalize(domain, residual)?;
    let mut out_labels = Vec::new();
    for label in normalized.split('.') {
        let uni = rusty_js_punycode::label_to_unicode(label).map_err(|_| IdnaError::Punycode)?;
        out_labels.push(uni);
    }
    Ok(out_labels.join("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_passthrough() {
        assert_eq!(to_ascii("example.com"), Ok("example.com".to_string()));
    }

    #[test]
    fn case_fold_then_punycode() {

        assert_eq!(
            to_ascii("BÜCHER.example"),
            Ok("xn--bcher-kva.example".to_string())
        );
    }

    #[test]
    fn nfc_compose_then_punycode() {

        assert_eq!(
            to_ascii("mu\u{308}nchen.example.com"),
            Ok("xn--mnchen-3ya.example.com".to_string())
        );
    }

    #[test]
    fn disallowed_code_point_errors() {
        assert_eq!(
            to_ascii("ex ample.com"),
            Err(IdnaError::DisallowedCodePoint(' '))
        );
    }

    #[test]
    fn roundtrip_to_unicode() {
        assert_eq!(
            to_unicode("xn--bcher-kva.example"),
            Ok("bücher.example".to_string())
        );
    }
}
