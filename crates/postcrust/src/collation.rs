
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollationKind {

    ByteOrdered,

    IcuGated,
}

pub const BUILTIN: &[(&str, u32, CollationKind)] = &[
    ("default", 100, CollationKind::ByteOrdered),
    ("C", 950, CollationKind::ByteOrdered),
    ("POSIX", 951, CollationKind::ByteOrdered),
    ("ucs_basic", 3450, CollationKind::ByteOrdered),

    ("unicode", 963, CollationKind::IcuGated),
    ("pg_c_utf8", 962, CollationKind::IcuGated),
];

pub fn lookup(name: &str) -> Option<CollationKind> {
    BUILTIN
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, _, k)| *k)
}

pub fn validate_for_comparison(name: &str) -> Result<(), crate::types::PgError> {
    match lookup(name) {
        Some(CollationKind::ByteOrdered) => Ok(()),
        Some(CollationKind::IcuGated) => Err(crate::types::PgError::CollationUnsupported {
            name: name.to_string(),
        }),
        None => Err(crate::types::PgError::CollationDoesNotExist {
            name: name.to_string(),
        }),
    }
}
