
pub use rusty_js_ast::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,

    pub preceded_by_line_terminator: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {

    Ident(String),

    PrivateIdent(String),

    Number(f64, NumberKind),

    BigInt(String, NumberKind),

    String(String),

    WtfString(Vec<u16>),

    Template {
        cooked: Option<String>,
        raw: String,
        part: TemplatePart,
    },

    Regex { body: String, flags: String },

    Punct(Punct),

    Hashbang(String),

    Eof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberKind {
    Decimal,
    Hex,
    Binary,
    Octal,

    LegacyOctal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplatePart {
    NoSubstitution,
    Head,
    Middle,
    Tail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Punct {

    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Semicolon,
    Comma,
    Colon,
    At,
    Dot,
    Spread,
    Arrow,
    OptionalChain,

    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
    StrictEq,
    StrictNe,

    Plus,
    Minus,
    Star,
    Percent,
    StarStar,
    Slash,
    Inc,
    Dec,
    Shl,
    Shr,
    UShr,
    BitAnd,
    BitOr,
    BitXor,
    BitNot,
    LogicalNot,
    LogicalAnd,
    LogicalOr,
    NullishCoalesce,
    Question,

    Assign,
    PlusAssign,
    MinusAssign,
    StarAssign,
    PercentAssign,
    StarStarAssign,
    SlashAssign,
    ShlAssign,
    ShrAssign,
    UShrAssign,
    BitAndAssign,
    BitOrAssign,
    BitXorAssign,
    LogicalAndAssign,
    LogicalOrAssign,
    NullishAssign,
}
