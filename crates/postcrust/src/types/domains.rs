
use crate::expr::ast::Expr;

#[derive(Debug, Clone)]
pub struct DomainInfo {

    pub base_oid: u32,

    pub base_typmod: i32,

    pub not_null: bool,

    pub checks: Vec<(String, Expr)>,
}
