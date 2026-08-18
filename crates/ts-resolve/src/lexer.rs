
pub use rusty_js_parser::{LexError, LexErrorKind, Lexer, LexerGoal};
pub use rusty_js_parser::{NumberKind, Punct, Span, TemplatePart, Token, TokenKind};

pub const TS_CONTEXTUAL_KEYWORDS: &[&str] = &[
    "type",
    "interface",
    "keyof",
    "as",
    "is",
    "readonly",
    "unique",
    "infer",
    "satisfies",
    "namespace",
    "module",
    "declare",
    "abstract",
    "override",
    "public",
    "private",
    "protected",
    "implements",
    "out",
    "asserts",
    "global",
];

#[inline]
pub fn is_ts_contextual_keyword(name: &str) -> bool {
    TS_CONTEXTUAL_KEYWORDS.iter().any(|k| *k == name)
}
