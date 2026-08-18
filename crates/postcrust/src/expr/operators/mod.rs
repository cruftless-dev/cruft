
use crate::types::PgError;
use sql_core::SqlValue;

mod bitwise;
mod json_access;
mod math_ops;
pub(crate) mod multiranges;
pub(crate) mod ranges;
mod string_ops;
mod text_search;

pub fn binary(op: &str, l: &SqlValue, r: &SqlValue) -> Option<Result<SqlValue, PgError>> {
    string_ops::binary(op, l, r)
        .or_else(|| bitwise::binary(op, l, r))
        .or_else(|| math_ops::binary(op, l, r))
        .or_else(|| json_access::binary(op, l, r))

        .or_else(|| ranges::binary(op, l, r))

        .or_else(|| multiranges::binary(op, l, r))

        .or_else(|| text_search::binary(op, l, r))
}

pub fn unary(op: &str, v: &SqlValue) -> Option<Result<SqlValue, PgError>> {
    bitwise::unary(op, v).or_else(|| math_ops::unary(op, v))
}
