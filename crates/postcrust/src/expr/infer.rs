
use super::ast::{BinOp, Expr, UnOp};
use super::bind::Schema;
use crate::types::{oid, oid_for_type_name};

pub fn infer(e: &Expr, schema: &Schema, col_types: &[u32]) -> Option<u32> {
    match e {
        Expr::Null => None,
        Expr::Lit(v) => match v {
            sql_core::SqlValue::Int(n) if *n >= i32::MIN as i64 && *n <= i32::MAX as i64 => {
                Some(oid::INT4)
            }
            sql_core::SqlValue::Int(_) => Some(oid::INT8),
            sql_core::SqlValue::Real(_) => Some(oid::FLOAT8),
            sql_core::SqlValue::Text(_) => Some(oid::TEXT),
            sql_core::SqlValue::Blob(_) => Some(oid::BYTEA),
            sql_core::SqlValue::Null => None,
        },
        Expr::ScalarSubquery(_)
        | Expr::Exists { .. }
        | Expr::InSubquery { .. }
        | Expr::Quantified { .. }
        | Expr::Window { .. } => None,
        Expr::Bool(_) => Some(oid::BOOL),
        Expr::Int(n) => Some(if *n >= i32::MIN as i64 && *n <= i32::MAX as i64 {
            oid::INT4
        } else {
            oid::INT8
        }),
        Expr::Float(_) => Some(oid::FLOAT8),
        Expr::Str(_) => Some(oid::TEXT),
        Expr::Column(name) => schema
            .index_of(name)
            .and_then(|i| col_types.get(i).copied())
            .filter(|&o| o != 0),
        Expr::ColumnRef(i) => col_types.get(*i).copied().filter(|&o| o != 0),

        Expr::Collate { expr, .. } => infer(expr, schema, col_types),

        Expr::Array(elems) => elems
            .first()
            .and_then(|e| infer(e, schema, col_types))
            .and_then(|eo| match eo {
                oid::INT2 => Some(oid::INT2_ARRAY),
                oid::INT4 => Some(oid::INT4_ARRAY),
                oid::INT8 => Some(oid::INT8_ARRAY),
                oid::TEXT => Some(oid::TEXT_ARRAY),
                oid::NUMERIC => Some(oid::NUMERIC_ARRAY),
                oid::FLOAT8 => Some(oid::FLOAT8_ARRAY),
                oid::BOOL => Some(oid::BOOL_ARRAY),
                _ => None,
            }),
        Expr::Cast { type_name, .. } => oid_for_type_name(type_name).or(match type_name.as_str() {
            "point" => Some(oid::POINT),
            _ => None,
        }),

        Expr::Row(_) => None,

        Expr::FieldAccess { field_oid, .. } => Some(*field_oid).filter(|&o| o != 0),
        Expr::Unary { op, expr } => match op {
            UnOp::Not => Some(oid::BOOL),
            UnOp::Neg | UnOp::Plus => infer(expr, schema, col_types),
        },
        Expr::Binary { op, left, right } => match op {
            BinOp::Lt
            | BinOp::Gt
            | BinOp::Eq
            | BinOp::LtEq
            | BinOp::GtEq
            | BinOp::NotEq
            | BinOp::And
            | BinOp::Or => Some(oid::BOOL),
            BinOp::Pow => Some(oid::FLOAT8),
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                let lt = infer(left, schema, col_types);

                if matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul) {
                    if let Some(o) = lt.filter(|&o| is_range_oid(o) || is_multirange_oid(o)) {
                        return Some(o);
                    }
                }
                numeric_result(lt, infer(right, schema, col_types))
            }
        },
        Expr::GenBinary { op, left, right } => match op.as_str() {
            "||" => Some(oid::TEXT),

            "@>" | "<@" | "&&" | "-|-" | "@?" | "@@" => Some(oid::BOOL),
            "<<" | ">>" => {

                let lt = infer(left, schema, col_types);
                if lt.map(is_range_oid).unwrap_or(false) {
                    Some(oid::BOOL)
                } else {
                    lt.or(Some(oid::INT4))
                }
            }
            "&" | "|" | "#" => infer(left, schema, col_types).or(Some(oid::INT4)),
            _ => {
                let _ = right;
                None
            }
        },
        Expr::GenUnary { op, expr } => match op.as_str() {
            "@" | "~" => infer(expr, schema, col_types),
            "|/" | "||/" => Some(oid::FLOAT8),
            _ => None,
        },
        Expr::IsNull { .. } => Some(oid::BOOL),

        Expr::Case { whens, else_, .. } => whens
            .first()
            .and_then(|(_, r)| infer(r, schema, col_types))
            .or_else(|| else_.as_deref().and_then(|e| infer(e, schema, col_types))),

        Expr::Func { name, order_by, .. }
            if matches!(name.as_str(), "percentile_disc" | "mode") =>
        {
            order_by
                .first()
                .and_then(|k| infer(&k.expr, schema, col_types))
        }
        Expr::Func { name, args, .. } => func_ret(name, args, schema, col_types),
    }
}

fn is_range_oid(o: u32) -> bool {
    matches!(
        o,
        oid::INT4RANGE | oid::INT8RANGE | oid::NUMRANGE | oid::TSRANGE | oid::DATERANGE
    )
}

fn is_multirange_oid(o: u32) -> bool {
    matches!(
        o,
        oid::INT4MULTIRANGE
            | oid::INT8MULTIRANGE
            | oid::NUMMULTIRANGE
            | oid::TSMULTIRANGE
            | oid::DATEMULTIRANGE
    )
}

fn multirange_element_oid(o: u32) -> Option<u32> {
    crate::types::multiranges::component_range_oid(o)
        .and_then(crate::types::ranges::range_element_oid)
}

fn numeric_result(l: Option<u32>, r: Option<u32>) -> Option<u32> {
    let (l, r) = (l?, r?);
    let is_int = |o: u32| matches!(o, oid::INT2 | oid::INT4 | oid::INT8);
    if l == oid::NUMERIC || r == oid::NUMERIC {
        Some(oid::NUMERIC)
    } else if l == oid::FLOAT8 || r == oid::FLOAT8 || l == oid::FLOAT4 || r == oid::FLOAT4 {
        Some(oid::FLOAT8)
    } else if is_int(l) && is_int(r) {
        Some(if l == oid::INT8 || r == oid::INT8 {
            oid::INT8
        } else {
            oid::INT4
        })
    } else {
        None
    }
}

fn func_ret(name: &str, args: &[Expr], schema: &Schema, col_types: &[u32]) -> Option<u32> {

    match name {
        "int4range" => return Some(oid::INT4RANGE),
        "int8range" => return Some(oid::INT8RANGE),
        "numrange" => return Some(oid::NUMRANGE),
        "tsrange" => return Some(oid::TSRANGE),
        "daterange" => return Some(oid::DATERANGE),

        "int4multirange" => return Some(oid::INT4MULTIRANGE),
        "int8multirange" => return Some(oid::INT8MULTIRANGE),
        "nummultirange" => return Some(oid::NUMMULTIRANGE),
        "tsmultirange" => return Some(oid::TSMULTIRANGE),
        "datemultirange" => return Some(oid::DATEMULTIRANGE),

        "multirange" => {
            return args
                .first()
                .and_then(|a| infer(a, schema, col_types))
                .and_then(crate::types::multiranges::multirange_of_range)
        }

        "range_merge" => {
            let first = args.first().and_then(|a| infer(a, schema, col_types));
            return match first {
                Some(o) if is_multirange_oid(o) => {
                    crate::types::multiranges::component_range_oid(o)
                }
                other => other,
            };
        }

        "isempty" | "lower_inc" | "upper_inc" | "lower_inf" | "upper_inf" => {
            return Some(oid::BOOL)
        }
        _ => {}
    }

    if matches!(name, "lower" | "upper") {
        if let Some(argt) = args.first().and_then(|a| infer(a, schema, col_types)) {
            if is_range_oid(argt) {
                if let Some(elem) = crate::types::ranges::range_element_oid(argt) {
                    return Some(elem);
                }
            } else if is_multirange_oid(argt) {
                if let Some(elem) = multirange_element_oid(argt) {
                    return Some(elem);
                }
            }
        }
    }

    if matches!(
        name,
        "abs" | "ceil" | "ceiling" | "floor" | "sign" | "greatest" | "least" | "min" | "max"
            | "sum" | "coalesce" | "nullif"

            | "bool_and" | "bool_or" | "every" | "bit_and" | "bit_or"
    ) {
        return args.first().and_then(|a| infer(a, schema, col_types));
    }

    if matches!(
        name,
        "var_pop" | "var_samp" | "variance" | "stddev" | "stddev_pop" | "stddev_samp"
    ) {
        let argt = args.first().and_then(|a| infer(a, schema, col_types));
        return Some(match argt {
            Some(oid::FLOAT4) | Some(oid::FLOAT8) => oid::FLOAT8,
            _ => oid::NUMERIC,
        });
    }

    if matches!(name, "round" | "trunc") {
        return if args.len() == 2 {
            Some(oid::NUMERIC)
        } else {
            args.first().and_then(|a| infer(a, schema, col_types))
        };
    }
    Some(match name {

        "upper" | "lower" | "initcap" | "reverse" | "repeat" | "substr" | "replace"
        | "split_part" | "translate" | "lpad" | "rpad" | "ltrim" | "rtrim" | "btrim" | "trim"
        | "overlay" | "left" | "right" | "concat" | "concat_ws" | "md5" | "to_hex" | "chr"
        | "quote_ident" | "string_agg" | "quote_literal" | "quote_nullable" | "format"
        | "array_to_string" | "jsonb_typeof" | "json_typeof" | "encode" | "host" | "abbrev"
        | "regexp_replace" | "regexp_substr" => oid::TEXT,

        "length"
        | "char_length"
        | "character_length"
        | "octet_length"
        | "bit_length"
        | "ascii"
        | "strpos"
        | "position"
        | "array_length"
        | "array_upper"
        | "array_lower"
        | "cardinality"
        | "array_ndims"
        | "jsonb_array_length"
        | "json_array_length"
        | "num_nulls"
        | "num_nonnulls"
        | "masklen"
        | "family"
        | "get_bit"
        | "get_byte"
        | "gcd"
        | "lcm"
        | "factorial"
        | "mod"
        | "div"
        | "width_bucket"
        | "scale"
        | "min_scale"
        | "regexp_count"
        | "regexp_instr"
        | "levenshtein"
        | "levenshtein_less_equal" => oid::INT4,

        "sqrt" | "cbrt" | "power" | "exp" | "ln" | "log" | "log10" | "sin" | "cos" | "tan"
        | "cot" | "asin" | "acos" | "atan" | "atan2" | "sinh" | "cosh" | "tanh" | "asinh"
        | "acosh" | "atanh" | "degrees" | "radians" | "pi" | "sind" | "cosd" | "tand" | "cotd"
        | "asind" | "acosd" | "atand" | "atan2d" | "date_part" => oid::FLOAT8,

        "sha224" | "sha256" | "sha384" | "sha512" | "decode" | "set_bit" | "set_byte" => oid::BYTEA,

        "count" => oid::INT8,
        "avg" => oid::FLOAT8,

        "percentile_cont" => oid::FLOAT8,

        "corr" | "covar_pop" | "covar_samp" | "regr_slope" | "regr_intercept" | "regr_r2"
        | "regr_avgx" | "regr_avgy" | "regr_sxx" | "regr_syy" | "regr_sxy" => oid::FLOAT8,

        "regr_count" => oid::INT8,

        "starts_with" | "like" | "ilike" | "regexp_like" | "jsonb_path_exists"
        | "jsonb_path_match" => oid::BOOL,

        "jsonb_path_query" | "jsonb_path_query_first" | "jsonb_path_query_array" => oid::JSONB,

        "trim_scale" => oid::NUMERIC,

        "to_jsonb" | "jsonb_build_array" | "jsonb_build_object" | "jsonb_strip_nulls"
        | "jsonb_pretty" | "jsonb_set" | "jsonb_insert" | "jsonb_agg" | "jsonb_object_agg" => {
            oid::JSONB
        }
        "to_json" | "json_build_array" | "json_build_object" | "row_to_json" | "%row_to_json"
        | "json_agg" | "json_object_agg" => oid::JSON,

        "make_date" => oid::DATE,
        "make_time" => oid::TIME,
        "make_timestamp" => oid::TIMESTAMP,
        "make_interval" => oid::INTERVAL,
        "date_trunc" => oid::TIMESTAMP,

        "to_char" => oid::TEXT,
        "to_number" => oid::NUMERIC,
        "to_date" => oid::DATE,
        "to_timestamp" => oid::TIMESTAMPTZ,

        "to_tsvector" | "strip" | "setweight" => oid::TSVECTOR,
        "to_tsquery" | "plainto_tsquery" | "phraseto_tsquery" => oid::TSQUERY,
        "numnode" => oid::INT4,
        "ts_rank" | "ts_rank_cd" => oid::FLOAT4,
        "ts_headline" => oid::TEXT,

        "network" | "netmask" | "broadcast" | "hostmask" => oid::INET,

        "regexp_match" | "regexp_split_to_array" => oid::TEXT_ARRAY,
        _ => return None,
    })
}
