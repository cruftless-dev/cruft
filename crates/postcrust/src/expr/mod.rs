
use crate::types::PgError;

pub mod ast;
pub mod bind;
pub mod deparse;
pub mod eval;
pub mod functions;
pub mod infer;
pub mod lexer;
pub mod operators;
pub mod parser;

pub use bind::{lower, lower_pred, Schema};

pub(crate) fn arg_f64(v: &sql_core::SqlValue) -> Option<f64> {
    match v {
        sql_core::SqlValue::Int(n) => Some(*n as f64),
        sql_core::SqlValue::Real(f) => Some(*f),
        _ => None,
    }
}

pub use ast::Expr;
pub use eval::eval;
pub use parser::parse;

pub fn eval_str(src: &str) -> Result<sql_core::SqlValue, PgError> {
    eval(&parse(src)?)
}
