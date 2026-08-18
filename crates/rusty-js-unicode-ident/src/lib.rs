
use rusty_js_ucd_tables::identifier_tables;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentifierDecision {
    Accept,
    Reject,
}

impl IdentifierDecision {
    pub fn as_bool(self) -> bool {
        match self {
            Self::Accept => true,
            Self::Reject => false,
        }
    }
}

pub fn is_ascii_identifier_start(cp: u32) -> bool {
    cp < 0x80 && {
        let b = cp as u8;
        b.is_ascii_alphabetic() || b == b'_' || b == b'$'
    }
}

pub fn is_ascii_identifier_continue(cp: u32) -> bool {
    cp < 0x80 && {
        let b = cp as u8;
        b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
    }
}

pub fn is_ecmascript_join_control(cp: u32) -> bool {
    matches!(cp, 0x200C | 0x200D)
}

pub fn is_id_xid_delta(cp: u32) -> bool {
    matches!(
        cp,
        0x037A | 0x0E33 | 0x0EB3 | 0x309B | 0x309C | 0xFC5E
            ..=0xFC63
                | 0xFDFA
                | 0xFDFB
                | 0xFE70
                | 0xFE72
                | 0xFE74
                | 0xFE76
                | 0xFE78
                | 0xFE7A
                | 0xFE7C
                | 0xFE7E
                | 0xFF9E
                | 0xFF9F
    )
}

pub fn is_id_start(cp: u32) -> bool {
    if cp < 0x80 {
        is_ascii_identifier_start(cp)
    } else {
        identifier_tables::is_id_start(cp)
    }
}

pub fn is_id_continue(cp: u32) -> bool {
    if cp < 0x80 {
        is_ascii_identifier_continue(cp)
    } else {
        is_ecmascript_join_control(cp) || identifier_tables::is_id_continue(cp)
    }
}

pub fn is_xid_start(cp: u32) -> bool {
    if cp < 0x80 {
        is_ascii_identifier_start(cp)
    } else {
        identifier_tables::is_xid_start(cp)
    }
}

pub fn is_xid_continue(cp: u32) -> bool {
    if cp < 0x80 {
        is_ascii_identifier_continue(cp)
    } else {
        identifier_tables::is_xid_continue(cp)
    }
}

pub fn identifier_start_decision(cp: u32) -> IdentifierDecision {
    if cp < 0x80 {
        if is_ascii_identifier_start(cp) {
            IdentifierDecision::Accept
        } else {
            IdentifierDecision::Reject
        }
    } else if is_id_start(cp) {
        IdentifierDecision::Accept
    } else {
        IdentifierDecision::Reject
    }
}

pub fn identifier_continue_decision(cp: u32) -> IdentifierDecision {
    if cp < 0x80 {
        if is_ascii_identifier_continue(cp) {
            IdentifierDecision::Accept
        } else {
            IdentifierDecision::Reject
        }
    } else if is_id_continue(cp) {
        IdentifierDecision::Accept
    } else {
        IdentifierDecision::Reject
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_start_and_continue_match_ecma_cells() {
        for cp in 0..0x80 {
            let ch = char::from_u32(cp).unwrap();
            let start = ch.is_ascii_alphabetic() || ch == '_' || ch == '$';
            let cont = ch.is_ascii_alphanumeric() || ch == '_' || ch == '$';
            assert_eq!(is_ascii_identifier_start(cp), start, "U+{cp:04X}");
            assert_eq!(is_ascii_identifier_continue(cp), cont, "U+{cp:04X}");
            assert_eq!(is_id_start(cp), start, "U+{cp:04X}");
            assert_eq!(is_id_continue(cp), cont, "U+{cp:04X}");
        }
    }

    #[test]
    fn unicode_version_matches_shared_ucd_tables() {
        assert_eq!(rusty_js_ucd_tables::UNICODE_VERSION, "17.0.0");
        assert_eq!(
            identifier_tables::UNICODE_VERSION,
            rusty_js_ucd_tables::UNICODE_VERSION
        );
    }

    #[test]
    fn ecmascript_join_controls_are_continue_only() {
        for cp in [0x200C, 0x200D] {
            assert!(!is_id_start(cp));
            assert!(is_id_continue(cp));
        }
    }

    #[test]
    fn id_minus_xid_delta_is_start_and_continue() {
        let deltas = [
            0x037A, 0x0E33, 0x0EB3, 0x309B, 0x309C, 0xFC5E, 0xFC63, 0xFDFA, 0xFDFB, 0xFE70, 0xFE72,
            0xFE74, 0xFE76, 0xFE78, 0xFE7A, 0xFE7C, 0xFE7E, 0xFF9E, 0xFF9F,
        ];
        for cp in deltas {
            assert!(is_id_xid_delta(cp), "U+{cp:04X}");
            assert!(is_id_start(cp), "U+{cp:04X}");
            assert!(is_id_continue(cp), "U+{cp:04X}");
            assert!(!is_xid_start(cp), "U+{cp:04X}");
        }
    }

    #[test]
    fn broad_non_ascii_membership_uses_shared_ucd_tables() {
        assert!(is_id_start('π' as u32));
        assert!(is_id_continue('π' as u32));
        assert!(is_id_start('中' as u32));
        assert!(is_id_continue('中' as u32));
        assert!(!is_id_start(0x2E2F));
        assert!(!is_id_continue(0x2E2F));
    }

    #[test]
    fn prop_list_residual_pins() {
        assert!(is_id_start(0x2118));
        assert!(is_id_continue(0x00B7));
        assert!(!is_id_start(0x00B7));
        assert!(!is_id_start(0x200E));
        assert!(!is_id_continue(0x200E));
    }
}
