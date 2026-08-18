
use crate::uts46_table::{uts46_status, Uts46Status};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Disposition {

    Valid,

    Ignored,

    Mapped(String),

    Disallowed,
}

pub trait Uts46Residual {
    fn residual(&self, c: char) -> Disposition;
}

pub struct TableResidual;

impl Uts46Residual for TableResidual {
    fn residual(&self, c: char) -> Disposition {
        status_to_disposition(uts46_status(c as u32))
    }
}

pub struct StdCaseFoldResidual;

impl Uts46Residual for StdCaseFoldResidual {
    fn residual(&self, c: char) -> Disposition {
        TableResidual.residual(c)
    }
}

fn status_to_disposition(status: Uts46Status) -> Disposition {
    match status {
        Uts46Status::Valid => Disposition::Valid,
        Uts46Status::Disallowed => Disposition::Disallowed,
        Uts46Status::Mapped(mapping) => Disposition::Mapped(mapping_to_string(mapping)),
        Uts46Status::Ignored => Disposition::Ignored,

        Uts46Status::Deviation(_) => Disposition::Valid,
        Uts46Status::DisallowedStd3Valid => Disposition::Disallowed,
        Uts46Status::DisallowedStd3Mapped(_) => Disposition::Disallowed,
    }
}

fn mapping_to_string(mapping: &[u32]) -> String {
    mapping
        .iter()
        .filter_map(|cp| char::from_u32(*cp))
        .collect()
}

pub fn map_char_with<R: Uts46Residual>(c: char, residual: &R) -> Disposition {
    match c {

        'a'..='z' | '0'..='9' | '-' => Disposition::Valid,
        '.' => Disposition::Valid,
        'A'..='Z' => Disposition::Mapped(c.to_ascii_lowercase().to_string()),

        '\u{0}'..='\u{2C}' | '/' | '\u{3A}'..='\u{40}' | '\u{5B}'..='\u{60}'
        | '\u{7B}'..='\u{7F}' => Disposition::Disallowed,

        '\u{3002}' | '\u{FF0E}' | '\u{FF61}' => Disposition::Mapped(".".to_string()),

        '\u{00AD}'
        | '\u{200B}'
        | '\u{FEFF}'
        | '\u{2060}'
        => Disposition::Ignored,

        '\u{80}'..='\u{9F}' | '\u{FFFD}' => Disposition::Disallowed,

        '\u{FF21}'..='\u{FF3A}' => {

            let off = c as u32 - 0xFF21;
            Disposition::Mapped(((b'a' + off as u8) as char).to_string())
        }
        '\u{FF41}'..='\u{FF5A}' => {

            let off = c as u32 - 0xFF41;
            Disposition::Mapped(((b'a' + off as u8) as char).to_string())
        }
        '\u{FF10}'..='\u{FF19}' => {

            let off = c as u32 - 0xFF10;
            Disposition::Mapped(((b'0' + off as u8) as char).to_string())
        }

        _ => residual.residual(c),
    }
}

pub fn map_char(c: char) -> Disposition {
    map_char_with(c, &StdCaseFoldResidual)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_ldh_valid() {
        assert_eq!(map_char('a'), Disposition::Valid);
        assert_eq!(map_char('9'), Disposition::Valid);
        assert_eq!(map_char('-'), Disposition::Valid);
    }

    #[test]
    fn ascii_upper_mapped_lower() {
        assert_eq!(map_char('A'), Disposition::Mapped("a".into()));
        assert_eq!(map_char('Z'), Disposition::Mapped("z".into()));
    }

    #[test]
    fn latin_upper_case_folds_via_residual() {

        assert_eq!(map_char('Ü'), Disposition::Mapped("ü".into()));
    }

    #[test]
    fn residual_table_disallows_tag_controls() {
        assert_eq!(map_char('\u{E0001}'), Disposition::Disallowed);
    }

    #[test]
    fn residual_table_maps_compatibility_spaces() {
        assert_eq!(map_char('\u{00A0}'), Disposition::Mapped(" ".into()));
    }

    #[test]
    fn soft_hyphen_ignored() {
        assert_eq!(map_char('\u{00AD}'), Disposition::Ignored);
    }

    #[test]
    fn ascii_space_disallowed() {
        assert_eq!(map_char(' '), Disposition::Disallowed);
        assert_eq!(map_char('\u{0}'), Disposition::Disallowed);
    }

    #[test]
    fn fullwidth_letter_mapped() {
        assert_eq!(map_char('\u{FF21}'), Disposition::Mapped("a".into()));
        assert_eq!(map_char('\u{FF0E}'), Disposition::Mapped(".".into()));
    }
}
