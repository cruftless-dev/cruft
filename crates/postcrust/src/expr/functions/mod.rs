
use crate::types::PgError;
use sql_core::SqlValue;

mod array_basic;
mod array_mut;
mod base_encoding;
mod bit_funcs;
mod concat;
mod conditional;
mod datetime_make;
mod datetime_part;
mod encoding;
mod formatting;
mod fuzzy_match;
mod hyperbolic;
mod intmath;
pub(crate) mod json_build;
mod json_funcs;
mod json_mut;
pub mod jsonpath;
mod math_exp;
mod math_round;
mod multiranges;
mod network_inet;
mod numeric_scale;
mod pattern;
mod quote_format;
mod ranges;
mod regexp;
mod sha_hash;
mod string_case;
mod string_pad;
mod string_search;
mod text_search;
mod trig;
mod trig_degrees;

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {

    ranges::call(name, args)

        .or_else(|| multiranges::call(name, args))
        .or_else(|| math_round::call(name, args))
        .or_else(|| math_exp::call(name, args))
        .or_else(|| trig::call(name, args))
        .or_else(|| hyperbolic::call(name, args))
        .or_else(|| intmath::call(name, args))
        .or_else(|| string_case::call(name, args))
        .or_else(|| string_pad::call(name, args))
        .or_else(|| string_search::call(name, args))
        .or_else(|| concat::call(name, args))
        .or_else(|| conditional::call(name, args))
        .or_else(|| encoding::call(name, args))

        .or_else(|| array_basic::call(name, args))
        .or_else(|| array_mut::call(name, args))
        .or_else(|| json_funcs::call(name, args))
        .or_else(|| json_build::call(name, args))
        .or_else(|| json_mut::call(name, args))
        .or_else(|| jsonpath::call(name, args))
        .or_else(|| datetime_make::call(name, args))
        .or_else(|| datetime_part::call(name, args))
        .or_else(|| numeric_scale::call(name, args))
        .or_else(|| sha_hash::call(name, args))
        .or_else(|| quote_format::call(name, args))
        .or_else(|| network_inet::call(name, args))
        .or_else(|| bit_funcs::call(name, args))
        .or_else(|| trig_degrees::call(name, args))
        .or_else(|| base_encoding::call(name, args))
        .or_else(|| pattern::call(name, args))
        .or_else(|| fuzzy_match::call(name, args))
        .or_else(|| regexp::call(name, args))
        .or_else(|| text_search::call(name, args))
        .or_else(|| formatting::call(name, args))
}

pub fn json_each_rows(
    name: &str,
    arg: &SqlValue,
    jsonb: bool,
    as_text: bool,
) -> Result<Vec<(String, SqlValue)>, PgError> {
    json_mut::each_rows(name, arg, jsonb, as_text)
}

pub fn call_srf(name: &str, args: &[SqlValue]) -> Option<Result<Vec<SqlValue>, PgError>> {
    match name {
        "regexp_matches" => Some(regexp::regexp_matches_rows(args)),
        "regexp_split_to_table" => Some(regexp::regexp_split_to_table_rows(args)),
        "jsonb_object_keys" | "json_object_keys" => Some(json_mut::object_keys_rows(name, args)),
        _ => None,
    }
}
