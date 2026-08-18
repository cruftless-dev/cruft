
use crate::expr::ast::Expr;

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Select(SelectStmt),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectStmt {
    pub distinct: bool,

    pub distinct_on: Vec<Expr>,
    pub projection: Vec<SelectItem>,

    pub from: Option<FromItem>,
    pub filter: Option<Expr>,
    pub group_by: Vec<Expr>,

    pub grouping_sets: Vec<Vec<Expr>>,
    pub having: Option<Expr>,
    pub order_by: Vec<OrderKey>,
    pub limit: Option<i64>,
    pub offset: i64,

    pub windows: Vec<NamedWindow>,

    pub tail: Vec<SetOpArm>,

    pub locking: Vec<LockClause>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockStrength {
    Update,
    NoKeyUpdate,
    Share,
    KeyShare,
}

impl LockStrength {

    pub fn as_str(self) -> &'static str {
        match self {
            LockStrength::Update => "FOR UPDATE",
            LockStrength::NoKeyUpdate => "FOR NO KEY UPDATE",
            LockStrength::Share => "FOR SHARE",
            LockStrength::KeyShare => "FOR KEY SHARE",
        }
    }

    pub fn is_exclusive(self) -> bool {
        matches!(self, LockStrength::Update | LockStrength::NoKeyUpdate)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockWait {
    Wait,
    NoWait,
    SkipLocked,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LockClause {
    pub strength: LockStrength,

    pub of: Vec<String>,
    pub wait: LockWait,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetOp {
    Union,
    Intersect,
    Except,
}

impl SetOp {

    pub fn from_kw(kw: &str) -> Option<SetOp> {
        match kw {
            "union" => Some(SetOp::Union),
            "intersect" => Some(SetOp::Intersect),
            "except" => Some(SetOp::Except),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SetOpArm {
    pub op: SetOp,
    pub all: bool,
    pub arm: SelectStmt,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NamedWindow {
    pub name: String,
    pub partition_by: Vec<Expr>,
    pub order_by: Vec<OrderKey>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelectItem {

    Star,

    Expr { expr: Expr, alias: Option<String> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum FromItem {
    Table {
        name: String,
        alias: Option<String>,
    },

    Join {
        left: Box<FromItem>,
        right: Box<FromItem>,
        kind: JoinKind,
        on: Option<Expr>,
        using: Vec<String>,
        natural: bool,
    },

    Subquery {
        query: Box<SelectStmt>,
        alias: String,
        lateral: bool,
    },

    Function {
        name: String,
        args: Vec<Expr>,
        alias: Option<String>,
        lateral: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinKind {
    Inner,
    Left,
    Cross,
    Right,
    Full,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrderKey {
    pub expr: Expr,
    pub descending: bool,
    pub nulls_first: Option<bool>,

    pub comp_oid: Option<u32>,
}
