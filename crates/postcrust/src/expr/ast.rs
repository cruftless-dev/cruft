
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {

    Null,

    Lit(sql_core::SqlValue),

    ScalarSubquery(Box<crate::stmt::ast::SelectStmt>),

    Exists {
        query: Box<crate::stmt::ast::SelectStmt>,
        negated: bool,
    },

    InSubquery {
        expr: Box<Expr>,
        query: Box<crate::stmt::ast::SelectStmt>,
        negated: bool,
    },

    Quantified {
        expr: Box<Expr>,
        op: BinOp,
        quantifier: Quantifier,
        query: Box<crate::stmt::ast::SelectStmt>,
    },

    Bool(bool),

    Int(i64),

    Float(f64),

    Str(String),

    Column(String),

    ColumnRef(usize),

    Unary { op: UnOp, expr: Box<Expr> },

    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },

    Func {
        name: String,
        args: Vec<Expr>,
        distinct: bool,
        filter: Option<Box<Expr>>,
        order_by: Vec<crate::stmt::ast::OrderKey>,
    },

    GenBinary {
        op: String,
        left: Box<Expr>,
        right: Box<Expr>,
    },

    GenUnary { op: String, expr: Box<Expr> },

    Cast { expr: Box<Expr>, type_name: String },

    Row(Vec<Expr>),

    FieldAccess {
        base: Box<Expr>,
        field: String,
        comp_oid: u32,
        field_oid: u32,
    },

    IsNull { expr: Box<Expr>, negated: bool },

    Array(Vec<Expr>),

    Collate { expr: Box<Expr>, collation: String },

    Case {
        operand: Option<Box<Expr>>,
        whens: Vec<(Expr, Expr)>,
        else_: Option<Box<Expr>>,
    },

    Window {
        func: String,
        args: Vec<Expr>,
        partition_by: Vec<Expr>,
        order_by: Vec<crate::stmt::ast::OrderKey>,
        frame: Option<WindowFrame>,
        window_ref: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantifier {
    Any,
    All,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowFrame {
    pub mode: FrameMode,
    pub start: FrameBound,
    pub end: FrameBound,

    pub exclude: FrameExclude,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameMode {
    Rows,
    Range,
    Groups,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameExclude {
    NoOthers,
    CurrentRow,
    Group,
    Ties,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameBound {
    UnboundedPreceding,
    Preceding(i64),
    CurrentRow,
    Following(i64),
    UnboundedFollowing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Plus,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {

    Or,
    And,

    Lt,
    Gt,
    Eq,
    LtEq,
    GtEq,
    NotEq,

    Add,
    Sub,

    Mul,
    Div,
    Mod,

    Pow,
}
