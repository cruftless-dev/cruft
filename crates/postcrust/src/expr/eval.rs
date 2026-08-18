
use super::ast::{BinOp, Expr, UnOp};
use super::PgError;
use crate::stmt::ast::OrderKey;
use crate::types::domains::DomainInfo;
use crate::types::registry::TypeRegistries;
use crate::types::{self, oid};
use sql_core::{Row, SqlValue};

#[derive(Clone, Copy)]
pub struct EvalCtx<'a> {
    regs: Option<&'a TypeRegistries>,

    comp_mm: Option<u32>,
}

impl<'a> EvalCtx<'a> {

    pub fn new(regs: &'a TypeRegistries) -> Self {
        EvalCtx {
            regs: Some(regs),
            comp_mm: None,
        }
    }

    pub fn empty() -> Self {
        EvalCtx {
            regs: None,
            comp_mm: None,
        }
    }

    pub fn with_comp_mm(self, oid: Option<u32>) -> Self {
        EvalCtx {
            comp_mm: oid,
            ..self
        }
    }

    fn domain(&self, name: &str) -> Option<&'a DomainInfo> {
        self.regs.and_then(|r| r.domain(name))
    }

    fn enum_labels_by_name(&self, name: &str) -> Option<&'a [String]> {
        self.regs.and_then(|r| r.labels_by_name(name))
    }

    fn composite(&self, oid: u32) -> Option<&'a crate::types::composite::CompositeInfo> {
        self.regs.and_then(|r| r.composite(oid))
    }

    fn composite_by_name(&self, name: &str) -> Option<&'a crate::types::composite::CompositeInfo> {
        self.regs.and_then(|r| r.composite_by_name(name))
    }
}

pub fn eval(e: &Expr) -> Result<SqlValue, PgError> {
    eval_row(e, &[], EvalCtx::empty())
}

pub fn eval_ctx(e: &Expr, ctx: EvalCtx) -> Result<SqlValue, PgError> {
    eval_row(e, &[], ctx)
}

pub fn eval_row(e: &Expr, row: &[SqlValue], ctx: EvalCtx) -> Result<SqlValue, PgError> {
    match e {
        Expr::Null => Ok(SqlValue::Null),
        Expr::Lit(v) => Ok(v.clone()),
        Expr::ScalarSubquery(_)
        | Expr::Exists { .. }
        | Expr::InSubquery { .. }
        | Expr::Quantified { .. }
        | Expr::Window { .. } => Err(unresolved_subquery()),
        Expr::Bool(b) => Ok(SqlValue::Int(if *b { 1 } else { 0 })),
        Expr::Int(n) => Ok(SqlValue::Int(*n)),
        Expr::Float(f) => Ok(SqlValue::Real(*f)),
        Expr::Str(s) => Ok(SqlValue::Text(s.clone())),
        Expr::Column(name) => Err(PgError::InvalidInputSyntax {
            typ: "expression",
            input: format!("column \"{name}\" cannot be evaluated as a constant"),
        }),
        Expr::ColumnRef(i) => row.get(*i).cloned().ok_or(PgError::InvalidInputSyntax {
            typ: "expression",
            input: format!(
                "bound column {i} out of range for row of width {}",
                row.len()
            ),
        }),
        Expr::Unary { op, expr } => eval_unary(*op, &eval_row(expr, row, ctx)?),
        Expr::Binary { op, left, right } => {
            eval_binary(*op, &eval_row(left, row, ctx)?, &eval_row(right, row, ctx)?)
        }
        Expr::Func { name, args, .. } => {
            let vs: Result<Vec<_>, _> = args.iter().map(|a| eval_row(a, row, ctx)).collect();
            eval_func(name, &vs?)
        }
        Expr::GenBinary { op, left, right } => {
            let (l, r) = (eval_row(left, row, ctx)?, eval_row(right, row, ctx)?);
            super::operators::binary(op, &l, &r).unwrap_or_else(|| Err(no_operator(op)))
        }
        Expr::GenUnary { op, expr } => {
            let v = eval_row(expr, row, ctx)?;
            super::operators::unary(op, &v).unwrap_or_else(|| Err(no_operator(op)))
        }
        Expr::Cast { expr, type_name } => eval_cast(&eval_row(expr, row, ctx)?, type_name, ctx),

        Expr::Collate { expr, collation } => {
            crate::collation::validate_for_comparison(collation)?;
            eval_row(expr, row, ctx)
        }

        Expr::Array(elems) => {
            let mut parts = Vec::with_capacity(elems.len());
            for e in elems {
                parts.push(array_element_text(&eval_row(e, row, ctx)?));
            }
            Ok(SqlValue::Text(format!("{{{}}}", parts.join(","))))
        }

        Expr::Row(elems) => {
            let mut fields = Vec::with_capacity(elems.len());
            for e in elems {
                fields.push(render_row_elem(&eval_row(e, row, ctx)?));
            }
            Ok(SqlValue::Text(crate::types::composite::encode(&fields)))
        }

        Expr::FieldAccess {
            base,
            field,
            comp_oid,
            ..
        } => {
            let bv = eval_row(base, row, ctx)?;
            match bv {
                SqlValue::Null => Ok(SqlValue::Null),
                SqlValue::Text(lit) => {
                    let info =
                        ctx.composite(*comp_oid)
                            .ok_or_else(|| PgError::InvalidInputSyntax {
                                typ: "record",
                                input: format!("could not identify column \"{field}\" in record"),
                            })?;
                    let idx =
                        info.field_index(field)
                            .ok_or_else(|| PgError::InvalidInputSyntax {
                                typ: "record",
                                input: format!(
                                    "column \"{field}\" not found in data type {}",
                                    info.name
                                ),
                            })?;
                    let field_oid = info.fields[idx].1;
                    let parts = crate::types::composite::decode(&lit).map_err(|_| {
                        PgError::InvalidInputSyntax {
                            typ: "record",
                            input: format!("malformed record literal: \"{lit}\""),
                        }
                    })?;
                    match parts.get(idx) {
                        Some(Some(t)) => types::input(field_oid, t),
                        _ => Ok(SqlValue::Null),
                    }
                }
                other => Ok(other),
            }
        }
        Expr::IsNull { expr, negated } => {
            let is_null = matches!(eval_row(expr, row, ctx)?, SqlValue::Null);

            Ok(SqlValue::Int(if is_null != *negated { 1 } else { 0 }))
        }
        Expr::Case {
            operand,
            whens,
            else_,
        } => match operand {
            Some(op) => {
                let opv = eval_row(op, row, ctx)?;
                for (val, res) in whens {
                    let vv = eval_row(val, row, ctx)?;
                    if !matches!(opv, SqlValue::Null)
                        && !matches!(vv, SqlValue::Null)
                        && case_operand_eq(&opv, &vv)
                    {
                        return eval_row(res, row, ctx);
                    }
                }
                else_
                    .as_deref()
                    .map_or(Ok(SqlValue::Null), |e| eval_row(e, row, ctx))
            }
            None => {
                for (cond, res) in whens {
                    if matches!(eval_row(cond, row, ctx)?, SqlValue::Int(n) if n != 0) {
                        return eval_row(res, row, ctx);
                    }
                }
                else_
                    .as_deref()
                    .map_or(Ok(SqlValue::Null), |e| eval_row(e, row, ctx))
            }
        },
    }
}

pub fn is_aggregate_name(name: &str) -> bool {
    matches!(
        name,
        "count"
            | "sum"
            | "avg"
            | "min"
            | "max"
            | "string_agg"
            | "array_agg"

            | "json_agg"
            | "jsonb_agg"
            | "json_object_agg"
            | "jsonb_object_agg"
            | "percentile_cont"
            | "percentile_disc"
            | "mode"

            | "var_pop"
            | "var_samp"
            | "variance"
            | "stddev"
            | "stddev_pop"
            | "stddev_samp"
            | "corr"
            | "covar_pop"
            | "covar_samp"
            | "regr_slope"
            | "regr_intercept"
            | "regr_count"
            | "regr_r2"
            | "regr_avgx"
            | "regr_avgy"
            | "regr_sxx"
            | "regr_syy"
            | "regr_sxy"

            | "bool_and"
            | "bool_or"
            | "every"
            | "bit_and"
            | "bit_or"
    )
}

pub fn contains_user_aggregate(e: &Expr, regs: &crate::types::registry::TypeRegistries) -> bool {
    let any = |xs: &[Expr]| xs.iter().any(|x| contains_user_aggregate(x, regs));
    match e {
        Expr::Func { name, args, .. } => regs.aggregates.contains_key(name) || any(args),
        Expr::Unary { expr, .. }
        | Expr::GenUnary { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::Collate { expr, .. }
        | Expr::IsNull { expr, .. } => contains_user_aggregate(expr, regs),
        Expr::Binary { left, right, .. } | Expr::GenBinary { left, right, .. } => {
            contains_user_aggregate(left, regs) || contains_user_aggregate(right, regs)
        }
        Expr::Case {
            operand,
            whens,
            else_,
        } => {
            operand
                .as_deref()
                .is_some_and(|o| contains_user_aggregate(o, regs))
                || whens.iter().any(|(c, r)| {
                    contains_user_aggregate(c, regs) || contains_user_aggregate(r, regs)
                })
                || else_
                    .as_deref()
                    .is_some_and(|x| contains_user_aggregate(x, regs))
        }
        _ => false,
    }
}

pub fn contains_aggregate(e: &Expr) -> bool {
    match e {
        Expr::Func { name, args, .. } => {
            is_aggregate_name(name) || args.iter().any(contains_aggregate)
        }
        Expr::Unary { expr, .. }
        | Expr::GenUnary { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::Collate { expr, .. }
        | Expr::IsNull { expr, .. } => contains_aggregate(expr),
        Expr::Array(elems) => elems.iter().any(contains_aggregate),
        Expr::Binary { left, right, .. } | Expr::GenBinary { left, right, .. } => {
            contains_aggregate(left) || contains_aggregate(right)
        }
        Expr::Case {
            operand,
            whens,
            else_,
        } => {
            operand.as_deref().is_some_and(contains_aggregate)
                || whens
                    .iter()
                    .any(|(c, r)| contains_aggregate(c) || contains_aggregate(r))
                || else_.as_deref().is_some_and(contains_aggregate)
        }
        _ => false,
    }
}

pub fn eval_group(e: &Expr, rows: &[Row], ctx: EvalCtx) -> Result<SqlValue, PgError> {
    if let Expr::Func {
        name,
        args,
        distinct,
        filter,
        order_by,
    } = e
    {
        let is_user_agg = ctx.regs.is_some_and(|r| r.aggregates.contains_key(name));
        if is_aggregate_name(name) || is_user_agg {
            return eval_aggregate(
                name,
                args,
                rows,
                *distinct,
                filter.as_deref(),
                order_by,
                ctx,
            );
        }
    }
    match e {
        Expr::Func { name, args, .. } => {
            let vs: Result<Vec<_>, _> = args.iter().map(|a| eval_group(a, rows, ctx)).collect();
            eval_func(name, &vs?)
        }
        Expr::Binary { op, left, right } => eval_binary(
            *op,
            &eval_group(left, rows, ctx)?,
            &eval_group(right, rows, ctx)?,
        ),
        Expr::Unary { op, expr } => eval_unary(*op, &eval_group(expr, rows, ctx)?),
        Expr::GenBinary { op, left, right } => {
            let (l, r) = (eval_group(left, rows, ctx)?, eval_group(right, rows, ctx)?);
            super::operators::binary(op, &l, &r).unwrap_or_else(|| Err(no_operator(op)))
        }
        Expr::GenUnary { op, expr } => {
            let v = eval_group(expr, rows, ctx)?;
            super::operators::unary(op, &v).unwrap_or_else(|| Err(no_operator(op)))
        }
        Expr::Cast { expr, type_name } => eval_cast(&eval_group(expr, rows, ctx)?, type_name, ctx),
        Expr::Collate { expr, collation } => {
            crate::collation::validate_for_comparison(collation)?;
            eval_group(expr, rows, ctx)
        }
        Expr::Array(elems) => {
            let mut parts = Vec::with_capacity(elems.len());
            for e in elems {
                parts.push(array_element_text(&eval_group(e, rows, ctx)?));
            }
            Ok(SqlValue::Text(format!("{{{}}}", parts.join(","))))
        }
        Expr::IsNull { expr, negated } => {
            let is_null = matches!(eval_group(expr, rows, ctx)?, SqlValue::Null);
            Ok(SqlValue::Int(if is_null != *negated { 1 } else { 0 }))
        }
        Expr::Case {
            operand,
            whens,
            else_,
        } => match operand {
            Some(op) => {
                let opv = eval_group(op, rows, ctx)?;
                for (val, res) in whens {
                    let vv = eval_group(val, rows, ctx)?;
                    if !matches!(opv, SqlValue::Null)
                        && !matches!(vv, SqlValue::Null)
                        && case_operand_eq(&opv, &vv)
                    {
                        return eval_group(res, rows, ctx);
                    }
                }
                else_
                    .as_deref()
                    .map_or(Ok(SqlValue::Null), |e| eval_group(e, rows, ctx))
            }
            None => {
                for (cond, res) in whens {
                    if matches!(eval_group(cond, rows, ctx)?, SqlValue::Int(n) if n != 0) {
                        return eval_group(res, rows, ctx);
                    }
                }
                else_
                    .as_deref()
                    .map_or(Ok(SqlValue::Null), |e| eval_group(e, rows, ctx))
            }
        },

        _ => eval_row(e, rows.first().map(|r| r.as_slice()).unwrap_or(&[]), ctx),
    }
}

fn eval_user_aggregate(
    agg: &crate::catalog::AggregateDef,
    args: &[Expr],
    rows: &[Row],
    ctx: EvalCtx,
) -> Result<SqlValue, PgError> {
    let regs = ctx
        .regs
        .expect("user aggregate requires a registry snapshot");
    let arg = args.first().ok_or_else(|| PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("aggregate {} requires one argument", agg.name),
    })?;
    let call1 = |func: &str, xs: Vec<Expr>| Expr::Func {
        name: func.to_string(),
        args: xs,
        distinct: false,
        filter: None,
        order_by: Vec::new(),
    };
    let mut state = crate::types::input(agg.stype_oid, &agg.initcond)?;
    for r in rows {
        let value = eval_row(arg, r, ctx)?;
        let call = call1(&agg.sfunc, vec![Expr::Lit(state.clone()), Expr::Lit(value)]);
        let inlined = crate::stmt::func_inline::inline_expr_pub(&call, regs)?;
        state = eval_row(&inlined, &[], ctx)?;
    }
    if let Some(ff) = &agg.finalfunc {
        let call = call1(ff, vec![Expr::Lit(state)]);
        let inlined = crate::stmt::func_inline::inline_expr_pub(&call, regs)?;
        state = eval_row(&inlined, &[], ctx)?;
    }
    Ok(state)
}

pub(crate) fn eval_aggregate(
    name: &str,
    args: &[Expr],
    rows: &[Row],
    distinct: bool,
    filter: Option<&Expr>,
    order_by: &[OrderKey],
    ctx: EvalCtx,
) -> Result<SqlValue, PgError> {
    use std::cmp::Ordering;

    let reduced;
    let rows: &[Row] = if distinct || filter.is_some() {
        reduced = filter_distinct_rows(args, rows, distinct, filter, ctx)?;
        &reduced
    } else {
        rows
    };

    match name {
        "percentile_cont" | "percentile_disc" | "mode" => {
            return eval_ordered_set(name, args, rows, order_by, ctx);
        }
        _ => {}
    }

    let sorted;
    let rows: &[Row] = if order_by.is_empty() {
        rows
    } else {
        sorted = sort_group_rows(rows, order_by, ctx)?;
        &sorted
    };

    if let Some(regs) = ctx.regs {
        if let Some(agg) = regs.aggregates.get(name) {
            return eval_user_aggregate(agg, args, rows, ctx);
        }
    }
    match name {
        "count" if args.is_empty() => Ok(SqlValue::Int(rows.len() as i64)),
        "count" => {
            let mut n = 0i64;
            for r in rows {
                if !matches!(eval_row(&args[0], r, ctx)?, SqlValue::Null) {
                    n += 1;
                }
            }
            Ok(SqlValue::Int(n))
        }
        "sum" | "avg" => {

            let mut ints: Vec<i64> = Vec::new();
            let mut reals: Vec<f64> = Vec::new();

            let mut nums: Vec<SqlValue> = Vec::new();
            let mut any_real = false;
            let mut any_num = false;
            let mut count = 0i64;
            for r in rows {
                match eval_row(&args[0], r, ctx)? {
                    SqlValue::Null => {}
                    SqlValue::Int(n) => {
                        ints.push(n);
                        reals.push(n as f64);
                        nums.push(SqlValue::Int(n));
                        count += 1;
                    }
                    SqlValue::Real(f) => {
                        any_real = true;
                        reals.push(f);
                        count += 1;
                    }
                    v @ SqlValue::Text(_) => {

                        if types::numeric::value_cmp(&v, &SqlValue::Int(0)).is_none() {
                            return Err(numeric_type_err());
                        }
                        any_num = true;

                        if let SqlValue::Text(t) = &v {
                            reals.push(t.parse::<f64>().unwrap_or(0.0));
                        }
                        nums.push(v);
                        count += 1;
                    }
                    _ => return Err(numeric_type_err()),
                }
            }
            if count == 0 {
                return Ok(SqlValue::Null);
            }
            if any_real {
                if name == "avg" {
                    Ok(SqlValue::Real(reals.iter().sum::<f64>() / count as f64))
                } else {
                    Ok(SqlValue::Real(reals.iter().sum()))
                }
            } else if any_num {

                let mut acc = SqlValue::Int(0);
                for v in &nums {
                    acc = match types::numeric::arith('+', &acc, v) {
                        Some(res) => res?,
                        None => return Err(numeric_type_err()),
                    };
                }
                if name == "sum" {
                    Ok(acc)
                } else {
                    match types::numeric::arith('/', &acc, &SqlValue::Int(count)) {
                        Some(res) => res,
                        None => Err(numeric_type_err()),
                    }
                }
            } else if name == "avg" {
                Ok(SqlValue::Real(reals.iter().sum::<f64>() / count as f64))
            } else {
                let mut s = 0i64;
                for n in ints {
                    s = s
                        .checked_add(n)
                        .ok_or(PgError::Overflow { typ: "bigint" })?;
                }
                Ok(SqlValue::Int(s))
            }
        }
        "min" | "max" => {
            let want_max = name == "max";

            let comp_fields = ctx
                .comp_mm
                .and_then(|oid| ctx.composite(oid).map(|ci| ci.fields.clone()));
            let mut best: Option<SqlValue> = None;
            for r in rows {
                let v = eval_row(&args[0], r, ctx)?;
                if matches!(v, SqlValue::Null) {
                    continue;
                }
                best = Some(match best {
                    None => v,
                    Some(b) => {

                        let ord = if let (Some(fields), Some(regs)) = (&comp_fields, ctx.regs) {
                            let ka = crate::stmt::lower::composite_order_key(&v, fields, regs);
                            let kb = crate::stmt::lower::composite_order_key(&b, fields, regs);
                            ka.cmp(&kb)
                        } else {
                            types::numeric::value_cmp(&v, &b).unwrap_or_else(|| v.cmp(&b))
                        };
                        let take = if want_max {
                            ord == Ordering::Greater
                        } else {
                            ord == Ordering::Less
                        };
                        if take {
                            v
                        } else {
                            b
                        }
                    }
                });
            }
            Ok(best.unwrap_or(SqlValue::Null))
        }
        "string_agg" => {

            let mut acc: Option<String> = None;
            for r in rows {
                let v = eval_row(&args[0], r, ctx)?;
                if matches!(v, SqlValue::Null) {
                    continue;
                }
                let piece = agg_text(&v);
                match &mut acc {
                    None => acc = Some(piece),
                    Some(s) => {
                        let delim = match args.get(1) {
                            Some(d) => match eval_row(d, r, ctx)? {
                                SqlValue::Null => String::new(),
                                dv => agg_text(&dv),
                            },
                            None => String::new(),
                        };
                        s.push_str(&delim);
                        s.push_str(&piece);
                    }
                }
            }
            Ok(acc.map(SqlValue::Text).unwrap_or(SqlValue::Null))
        }
        "array_agg" => {

            if rows.is_empty() {
                return Ok(SqlValue::Null);
            }
            let mut elems: Vec<String> = Vec::with_capacity(rows.len());
            for r in rows {
                elems.push(array_element_text(&eval_row(&args[0], r, ctx)?));
            }
            Ok(SqlValue::Text(format!("{{{}}}", elems.join(","))))
        }
        "json_agg" | "jsonb_agg" => {

            if rows.is_empty() {
                return Ok(SqlValue::Null);
            }
            let mut elems: Vec<String> = Vec::with_capacity(rows.len());
            for r in rows {
                let v = eval_row(&args[0], r, ctx)?;
                elems.push(super::functions::json_build::value_to_json(&v)?);
            }
            Ok(SqlValue::Text(format!("[{}]", elems.join(", "))))
        }
        "json_object_agg" | "jsonb_object_agg" => {

            if rows.is_empty() {
                return Ok(SqlValue::Null);
            }
            let is_jsonb = name == "jsonb_object_agg";
            let mut pairs: Vec<(String, String)> = Vec::with_capacity(rows.len());
            for r in rows {
                let k =
                    super::functions::json_build::value_to_json_key(&eval_row(&args[0], r, ctx)?)?;
                let v = super::functions::json_build::value_to_json(&eval_row(&args[1], r, ctx)?)?;
                pairs.push((k, v));
            }
            if is_jsonb {
                pairs.sort_by(|a, b| {
                    a.0.len()
                        .cmp(&b.0.len())
                        .then_with(|| a.0.as_bytes().cmp(b.0.as_bytes()))
                });
                let mut deduped: Vec<(String, String)> = Vec::with_capacity(pairs.len());
                for (k, v) in pairs {
                    if let Some(last) = deduped.last_mut() {
                        if last.0 == k {
                            last.1 = v;
                            continue;
                        }
                    }
                    deduped.push((k, v));
                }
                pairs = deduped;
                let body = pairs
                    .iter()
                    .map(|(k, v)| format!("{k}: {v}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                Ok(SqlValue::Text(format!("{{{body}}}")))
            } else {
                let body = pairs
                    .iter()
                    .map(|(k, v)| format!("{k} : {v}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                Ok(SqlValue::Text(format!("{{ {body} }}")))
            }
        }
        "var_pop" | "var_samp" | "variance" | "stddev" | "stddev_pop" | "stddev_samp" => {

            let mut nums: Vec<SqlValue> = Vec::new();
            let mut reals: Vec<f64> = Vec::new();
            let mut any_real = false;
            for r in rows {
                match eval_row(&args[0], r, ctx)? {
                    SqlValue::Null => {}
                    SqlValue::Int(v) => {
                        nums.push(SqlValue::Text(v.to_string()));
                        reals.push(v as f64);
                    }
                    SqlValue::Real(f) => {
                        any_real = true;
                        reals.push(f);
                    }
                    v @ SqlValue::Text(_) => {

                        if types::numeric::value_cmp(&v, &SqlValue::Int(0)).is_none() {
                            return Err(numeric_type_err());
                        }
                        if let SqlValue::Text(t) = &v {
                            reals.push(t.parse::<f64>().unwrap_or(0.0));
                        }
                        nums.push(v);
                    }
                    _ => return Err(numeric_type_err()),
                }
            }
            let n = reals.len();
            if n == 0 {
                return Ok(SqlValue::Null);
            }

            let is_sample = !matches!(name, "var_pop" | "stddev_pop");
            if is_sample && n < 2 {
                return Ok(SqlValue::Null);
            }
            if any_real {

                let mean = reals.iter().sum::<f64>() / n as f64;
                let s: f64 = reals.iter().map(|x| (x - mean) * (x - mean)).sum();
                let var = if is_sample {
                    s / (n as f64 - 1.0)
                } else {
                    s / n as f64
                };
                let out = if name.starts_with("stddev") {
                    var.sqrt()
                } else {
                    var
                };
                return Ok(SqlValue::Real(out));
            }

            let num_arith = |op: char, a: &SqlValue, b: &SqlValue| -> Result<SqlValue, PgError> {
                match types::numeric::arith(op, a, b) {
                    Some(res) => res,
                    None => Err(numeric_type_err()),
                }
            };
            let mut sum_x = SqlValue::Int(0);
            let mut sum_x2 = SqlValue::Int(0);
            for v in &nums {
                sum_x = num_arith('+', &sum_x, v)?;
                let sq = num_arith('*', v, v)?;
                sum_x2 = num_arith('+', &sum_x2, &sq)?;
            }

            let n_sq_terms = num_arith('*', &SqlValue::Int(n as i64), &sum_x2)?;
            let sum_x_sq = num_arith('*', &sum_x, &sum_x)?;
            let numerator = num_arith('-', &n_sq_terms, &sum_x_sq)?;

            let is_zero =
                types::numeric::value_cmp(&numerator, &SqlValue::Int(0)) == Some(Ordering::Equal);
            let variance = if is_zero {
                SqlValue::Text("0".to_string())
            } else {
                let denom = if is_sample {
                    (n as i64) * (n as i64 - 1)
                } else {
                    (n as i64) * (n as i64)
                };
                num_arith('/', &numerator, &SqlValue::Int(denom))?
            };
            if name.starts_with("stddev") {

                let scale = types::numeric::display_scale(&variance);
                match types::numeric::numeric_sqrt(&variance, scale) {
                    Some(sd) => Ok(sd),
                    None => Err(numeric_type_err()),
                }
            } else {
                Ok(variance)
            }
        }
        "corr" | "covar_pop" | "covar_samp" | "regr_slope" | "regr_intercept" | "regr_count"
        | "regr_r2" | "regr_avgx" | "regr_avgy" | "regr_sxx" | "regr_syy" | "regr_sxy" => {

            let mut ys: Vec<f64> = Vec::new();
            let mut xs: Vec<f64> = Vec::new();
            for r in rows {
                let y = agg_f64(&eval_row(&args[0], r, ctx)?)?;
                let x = agg_f64(&eval_row(&args[1], r, ctx)?)?;
                if let (Some(y), Some(x)) = (y, x) {
                    ys.push(y);
                    xs.push(x);
                }
            }
            let n = xs.len();
            if name == "regr_count" {
                return Ok(SqlValue::Int(n as i64));
            }
            if n == 0 {
                return Ok(SqlValue::Null);
            }
            let mx = xs.iter().sum::<f64>() / n as f64;
            let my = ys.iter().sum::<f64>() / n as f64;
            let mut sxx = 0.0;
            let mut syy = 0.0;
            let mut sxy = 0.0;
            for i in 0..n {
                sxx += (xs[i] - mx) * (xs[i] - mx);
                syy += (ys[i] - my) * (ys[i] - my);
                sxy += (xs[i] - mx) * (ys[i] - my);
            }
            let out = match name {
                "regr_avgx" => Some(mx),
                "regr_avgy" => Some(my),
                "regr_sxx" => Some(sxx),
                "regr_syy" => Some(syy),
                "regr_sxy" => Some(sxy),
                "covar_pop" => Some(sxy / n as f64),
                "covar_samp" if n < 2 => None,
                "covar_samp" => Some(sxy / (n as f64 - 1.0)),

                "corr" if sxx == 0.0 || syy == 0.0 => None,
                "corr" => Some(sxy / (sxx * syy).sqrt()),
                "regr_slope" if sxx == 0.0 => None,
                "regr_slope" => Some(sxy / sxx),
                "regr_intercept" if sxx == 0.0 => None,
                "regr_intercept" => Some(my - (sxy / sxx) * mx),
                "regr_r2" if sxx == 0.0 => None,

                "regr_r2" if syy == 0.0 => Some(1.0),
                "regr_r2" => Some((sxy * sxy) / (sxx * syy)),
                _ => unreachable!(),
            };
            Ok(out.map(SqlValue::Real).unwrap_or(SqlValue::Null))
        }
        "bool_and" | "bool_or" | "every" => {

            let want_and = name != "bool_or";
            let mut acc: Option<bool> = None;
            for r in rows {
                match eval_row(&args[0], r, ctx)? {
                    SqlValue::Null => {}
                    SqlValue::Int(b) => {
                        let cur = b != 0;
                        acc = Some(match acc {
                            None => cur,
                            Some(a) if want_and => a && cur,
                            Some(a) => a || cur,
                        });
                    }
                    _ => return Err(numeric_type_err()),
                }
            }
            Ok(acc
                .map(|b| SqlValue::Int(b as i64))
                .unwrap_or(SqlValue::Null))
        }
        "bit_and" | "bit_or" => {

            let and = name == "bit_and";
            let mut acc: Option<i64> = None;
            for r in rows {
                match eval_row(&args[0], r, ctx)? {
                    SqlValue::Null => {}
                    SqlValue::Int(n) => {
                        acc = Some(match acc {
                            None => n,
                            Some(a) if and => a & n,
                            Some(a) => a | n,
                        });
                    }
                    _ => return Err(numeric_type_err()),
                }
            }
            Ok(acc.map(SqlValue::Int).unwrap_or(SqlValue::Null))
        }
        _ => unreachable!("non-aggregate name reached eval_aggregate"),
    }
}

fn agg_f64(v: &SqlValue) -> Result<Option<f64>, PgError> {
    match v {
        SqlValue::Null => Ok(None),
        SqlValue::Int(n) => Ok(Some(*n as f64)),
        SqlValue::Real(f) => Ok(Some(*f)),
        SqlValue::Text(t) => t
            .trim()
            .parse::<f64>()
            .map(Some)
            .map_err(|_| numeric_type_err()),
        _ => Err(numeric_type_err()),
    }
}

fn agg_text(v: &SqlValue) -> String {
    match v {
        SqlValue::Text(s) => s.clone(),
        SqlValue::Int(n) => n.to_string(),
        SqlValue::Real(_) => types::floats::output(types::oid::FLOAT8, v),
        SqlValue::Null => String::new(),
        SqlValue::Blob(b) => {
            let mut s = String::from("\\x");
            for byte in b {
                s.push_str(&format!("{byte:02x}"));
            }
            s
        }
    }
}

fn array_element_text(v: &SqlValue) -> String {
    if matches!(v, SqlValue::Null) {
        return "NULL".to_string();
    }
    let raw = agg_text(v);
    let needs_quote = raw.is_empty()
        || raw.eq_ignore_ascii_case("null")
        || raw
            .chars()
            .any(|c| matches!(c, ',' | '{' | '}' | '"' | '\\') || c.is_whitespace());
    if needs_quote {
        let mut out = String::from("\"");
        for c in raw.chars() {
            if c == '"' || c == '\\' {
                out.push('\\');
            }
            out.push(c);
        }
        out.push('"');
        out
    } else {
        raw
    }
}

fn sort_group_rows(rows: &[Row], order_by: &[OrderKey], ctx: EvalCtx) -> Result<Vec<Row>, PgError> {
    let mut keyed: Vec<(Vec<SqlValue>, Row)> = Vec::with_capacity(rows.len());
    for r in rows {
        let mut keys = Vec::with_capacity(order_by.len());
        for k in order_by {
            keys.push(eval_row(&k.expr, r, ctx)?);
        }
        keyed.push((keys, r.clone()));
    }

    let comp_fields: Vec<Option<Vec<(String, u32, i32)>>> = order_by
        .iter()
        .map(|k| {
            k.comp_oid
                .and_then(|oid| ctx.composite(oid).map(|ci| ci.fields.clone()))
        })
        .collect();
    keyed.sort_by(|a, b| {
        for (i, k) in order_by.iter().enumerate() {
            let ord = match (&comp_fields[i], ctx.regs) {
                (Some(fields), Some(regs)) => {
                    composite_key_cmp(&a.0[i], &b.0[i], fields, regs, k.descending, k.nulls_first)
                }
                _ => ordered_cmp(&a.0[i], &b.0[i], k.descending, k.nulls_first),
            };
            if ord != std::cmp::Ordering::Equal {
                return ord;
            }
        }
        std::cmp::Ordering::Equal
    });
    Ok(keyed.into_iter().map(|(_, r)| r).collect())
}

fn composite_key_cmp(
    a: &SqlValue,
    b: &SqlValue,
    fields: &[(String, u32, i32)],
    regs: &TypeRegistries,
    desc: bool,
    nulls_first: Option<bool>,
) -> std::cmp::Ordering {
    if matches!(a, SqlValue::Null) || matches!(b, SqlValue::Null) {
        return sql_core::sort_cmp_nulls(a, b, desc, Some(nulls_first.unwrap_or(desc)));
    }
    let ka = crate::stmt::lower::composite_order_key(a, fields, regs);
    let kb = crate::stmt::lower::composite_order_key(b, fields, regs);
    let base = ka.cmp(&kb);
    if desc {
        base.reverse()
    } else {
        base
    }
}

fn ordered_cmp(
    a: &SqlValue,
    b: &SqlValue,
    desc: bool,
    nulls_first: Option<bool>,
) -> std::cmp::Ordering {
    let an = matches!(a, SqlValue::Null);
    let bn = matches!(b, SqlValue::Null);
    if an || bn {
        return sql_core::sort_cmp_nulls(a, b, desc, Some(nulls_first.unwrap_or(desc)));
    }
    let base = types::numeric::value_cmp(a, b).unwrap_or_else(|| a.cmp(b));
    if desc {
        base.reverse()
    } else {
        base
    }
}

fn eval_ordered_set(
    name: &str,
    args: &[Expr],
    rows: &[Row],
    order_by: &[OrderKey],
    ctx: EvalCtx,
) -> Result<SqlValue, PgError> {
    let key = order_by
        .first()
        .ok_or_else(|| PgError::InvalidInputSyntax {
            typ: "expression",
            input: format!("{name} requires a WITHIN GROUP (ORDER BY …) clause"),
        })?;

    let comp_fields: Option<Vec<(String, u32, i32)>> = key
        .comp_oid
        .and_then(|oid| ctx.composite(oid).map(|ci| ci.fields.clone()));

    let cmp_key = |a: &SqlValue, b: &SqlValue| -> std::cmp::Ordering {
        match (&comp_fields, ctx.regs) {
            (Some(fields), Some(regs)) => {
                let ka = crate::stmt::lower::composite_order_key(a, fields, regs);
                let kb = crate::stmt::lower::composite_order_key(b, fields, regs);
                ka.cmp(&kb)
            }
            _ => types::numeric::value_cmp(a, b).unwrap_or_else(|| a.cmp(b)),
        }
    };
    if name == "mode" {

        let mut vals: Vec<SqlValue> = Vec::new();
        for r in rows {
            let v = eval_row(&key.expr, r, ctx)?;
            if !matches!(v, SqlValue::Null) {
                vals.push(v);
            }
        }
        if vals.is_empty() {
            return Ok(SqlValue::Null);
        }
        vals.sort_by(|a, b| cmp_key(a, b));
        let mut best: Option<(SqlValue, usize)> = None;
        let mut i = 0;
        while i < vals.len() {
            let mut j = i + 1;
            while j < vals.len() && cmp_key(&vals[j], &vals[i]) == std::cmp::Ordering::Equal {
                j += 1;
            }
            let run = j - i;
            if best.as_ref().map(|(_, c)| run > *c).unwrap_or(true) {
                best = Some((vals[i].clone(), run));
            }
            i = j;
        }
        return Ok(best.map(|(v, _)| v).unwrap_or(SqlValue::Null));
    }

    let fraction = match eval_row(
        &args[0],
        rows.first().map(|r| r.as_slice()).unwrap_or(&[]),
        ctx,
    )? {
        SqlValue::Null => return Ok(SqlValue::Null),
        v => value_f64(&v).ok_or_else(|| PgError::InvalidInputSyntax {
            typ: "double precision",
            input: "percentile fraction must be numeric".into(),
        })?,
    };
    if !(0.0..=1.0).contains(&fraction) {
        return Err(PgError::InvalidInputSyntax {
            typ: "double precision",
            input: format!("percentile value {fraction} is not between 0 and 1"),
        });
    }
    let mut vals: Vec<SqlValue> = Vec::new();
    for r in rows {
        let v = eval_row(&key.expr, r, ctx)?;
        if !matches!(v, SqlValue::Null) {
            vals.push(v);
        }
    }
    if vals.is_empty() {
        return Ok(SqlValue::Null);
    }
    vals.sort_by(|a, b| {
        let ord = cmp_key(a, b);
        if key.descending {
            ord.reverse()
        } else {
            ord
        }
    });
    let n = vals.len();
    if name == "percentile_disc" {

        let idx = if fraction == 0.0 {
            0
        } else {
            ((fraction * n as f64).ceil() as usize)
                .saturating_sub(1)
                .min(n - 1)
        };
        return Ok(vals[idx].clone());
    }

    let rn = fraction * (n as f64 - 1.0);
    let lo = rn.floor() as usize;
    let hi = rn.ceil() as usize;
    let lo_v = value_f64(&vals[lo]).ok_or_else(|| PgError::InvalidInputSyntax {
        typ: "double precision",
        input: "percentile_cont requires numeric input".into(),
    })?;
    if lo == hi {
        return Ok(SqlValue::Real(lo_v));
    }
    let hi_v = value_f64(&vals[hi]).ok_or_else(|| PgError::InvalidInputSyntax {
        typ: "double precision",
        input: "percentile_cont requires numeric input".into(),
    })?;
    let frac = rn - lo as f64;
    Ok(SqlValue::Real(lo_v + (hi_v - lo_v) * frac))
}

fn filter_distinct_rows(
    args: &[Expr],
    rows: &[Row],
    distinct: bool,
    filter: Option<&Expr>,
    ctx: EvalCtx,
) -> Result<Vec<Row>, PgError> {
    let mut out: Vec<Row> = Vec::new();
    let mut seen: Vec<SqlValue> = Vec::new();
    for r in rows {
        if let Some(f) = filter {
            if truthy(&eval_row(f, r, ctx)?) != Some(true) {
                continue;
            }
        }
        if distinct {
            let key = match args.first() {
                Some(a) => eval_row(a, r, ctx)?,
                None => SqlValue::Null,
            };
            let dup = seen.iter().any(|s| {
                types::numeric::value_cmp(s, &key).unwrap_or_else(|| s.cmp(&key))
                    == std::cmp::Ordering::Equal
            });
            if dup {
                continue;
            }
            seen.push(key);
        }
        out.push(r.clone());
    }
    Ok(out)
}

fn case_operand_eq(a: &SqlValue, b: &SqlValue) -> bool {
    match types::numeric::value_cmp(a, b) {
        Some(o) => o == std::cmp::Ordering::Equal,
        None => a.cmp(b) == std::cmp::Ordering::Equal,
    }
}

fn eval_unary(op: UnOp, v: &SqlValue) -> Result<SqlValue, PgError> {
    if matches!(v, SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    match op {
        UnOp::Plus => match v {
            SqlValue::Int(_) | SqlValue::Real(_) => Ok(v.clone()),
            _ => Err(numeric_type_err()),
        },
        UnOp::Neg => match v {
            SqlValue::Int(n) => n
                .checked_neg()
                .map(SqlValue::Int)
                .ok_or(PgError::Overflow { typ: "bigint" }),
            SqlValue::Real(f) => Ok(SqlValue::Real(-f)),

            SqlValue::Text(_) => types::numeric::negate(v).ok_or_else(numeric_type_err),
            _ => Err(numeric_type_err()),
        },
        UnOp::Not => match truthy(v) {
            Some(b) => Ok(SqlValue::Int(if b { 0 } else { 1 })),
            None => Ok(SqlValue::Null),
        },
    }
}

fn eval_binary(op: BinOp, l: &SqlValue, r: &SqlValue) -> Result<SqlValue, PgError> {

    match op {
        BinOp::And => {
            return Ok(match (truthy(l), truthy(r)) {
                (Some(false), _) | (_, Some(false)) => SqlValue::Int(0),
                (Some(true), Some(true)) => SqlValue::Int(1),
                _ => SqlValue::Null,
            });
        }
        BinOp::Or => {
            return Ok(match (truthy(l), truthy(r)) {
                (Some(true), _) | (_, Some(true)) => SqlValue::Int(1),
                (Some(false), Some(false)) => SqlValue::Int(0),
                _ => SqlValue::Null,
            });
        }
        _ => {}
    }

    if matches!(l, SqlValue::Null) || matches!(r, SqlValue::Null) {
        return Ok(SqlValue::Null);
    }

    match op {
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => arith(op, l, r),

        BinOp::Pow => Ok(SqlValue::Real(as_f64(l)?.powf(as_f64(r)?))),
        BinOp::Lt | BinOp::Gt | BinOp::Eq | BinOp::LtEq | BinOp::GtEq | BinOp::NotEq => {
            compare(op, l, r)
        }
        BinOp::And | BinOp::Or => unreachable!("handled above"),
    }
}

fn arith(op: BinOp, l: &SqlValue, r: &SqlValue) -> Result<SqlValue, PgError> {

    let op_ch = match op {
        BinOp::Add => Some('+'),
        BinOp::Sub => Some('-'),
        BinOp::Mul => Some('*'),
        BinOp::Div => Some('/'),
        BinOp::Mod => Some('%'),
        _ => None,
    };
    if let Some(c) = op_ch {

        if let Some(res) = super::operators::ranges::set_op(c, l, r) {
            return res;
        }

        if let Some(res) = super::operators::multiranges::set_op(c, l, r) {
            return res;
        }
        if let Some(res) = types::numeric::arith(c, l, r) {
            return res;
        }
    }

    if let (SqlValue::Int(a), SqlValue::Int(b)) = (l, r) {
        let (a, b) = (*a, *b);
        return match op {
            BinOp::Add => a
                .checked_add(b)
                .map(SqlValue::Int)
                .ok_or(PgError::Overflow { typ: "bigint" }),
            BinOp::Sub => a
                .checked_sub(b)
                .map(SqlValue::Int)
                .ok_or(PgError::Overflow { typ: "bigint" }),
            BinOp::Mul => a
                .checked_mul(b)
                .map(SqlValue::Int)
                .ok_or(PgError::Overflow { typ: "bigint" }),
            BinOp::Div => {
                if b == 0 {
                    return Err(PgError::DivisionByZero);
                }
                a.checked_div(b)
                    .map(SqlValue::Int)
                    .ok_or(PgError::Overflow { typ: "bigint" })
            }
            BinOp::Mod => {
                if b == 0 {
                    return Err(PgError::DivisionByZero);
                }
                Ok(SqlValue::Int(a.wrapping_rem(b)))
            }
            _ => unreachable!(),
        };
    }
    let (a, b) = (as_f64(l)?, as_f64(r)?);
    let out = match op {
        BinOp::Add => a + b,
        BinOp::Sub => a - b,
        BinOp::Mul => a * b,
        BinOp::Div => {
            if b == 0.0 {
                return Err(PgError::DivisionByZero);
            }
            a / b
        }
        BinOp::Mod => {
            if b == 0.0 {
                return Err(PgError::DivisionByZero);
            }
            a % b
        }
        _ => unreachable!(),
    };
    Ok(SqlValue::Real(out))
}

fn compare(op: BinOp, l: &SqlValue, r: &SqlValue) -> Result<SqlValue, PgError> {

    use std::cmp::Ordering;

    let ord = if let Some(o) = types::numeric::value_cmp(l, r) {
        o
    } else {
        match (l, r) {
            (SqlValue::Int(a), SqlValue::Int(b)) => a.cmp(b),
            (SqlValue::Text(a), SqlValue::Text(b)) => a.cmp(b),

            (SqlValue::Int(_) | SqlValue::Real(_), SqlValue::Int(_) | SqlValue::Real(_)) => {
                as_f64(l)?
                    .partial_cmp(&as_f64(r)?)
                    .unwrap_or(Ordering::Less)
            }

            (SqlValue::Text(_), SqlValue::Real(_)) | (SqlValue::Real(_), SqlValue::Text(_)) => {
                match types::numeric::value_cmp_real_bridge(l, r) {
                    Some(o) => o,
                    None => return Err(numeric_type_err()),
                }
            }
            _ => return Err(numeric_type_err()),
        }
    };
    let b = match op {
        BinOp::Lt => ord == Ordering::Less,
        BinOp::Gt => ord == Ordering::Greater,
        BinOp::Eq => ord == Ordering::Equal,
        BinOp::LtEq => ord != Ordering::Greater,
        BinOp::GtEq => ord != Ordering::Less,
        BinOp::NotEq => ord != Ordering::Equal,
        _ => unreachable!(),
    };
    Ok(SqlValue::Int(if b { 1 } else { 0 }))
}

fn eval_func(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    super::functions::call(name, args).unwrap_or_else(|| {
        Err(PgError::InvalidInputSyntax {
            typ: "expression",
            input: format!("function {name}({}) does not exist", args.len()),
        })
    })
}

fn no_operator(op: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("operator does not exist: {op}"),
    }
}

fn unresolved_subquery() -> PgError {
    PgError::InvalidInputSyntax {
        typ: "expression",
        input: "correlated or unresolved subquery is not supported".to_string(),
    }
}

fn render_row_elem(v: &SqlValue) -> Option<String> {
    match v {
        SqlValue::Null => None,
        SqlValue::Text(s) => Some(s.clone()),
        SqlValue::Int(n) => Some(n.to_string()),
        SqlValue::Real(f) => Some(format!("{f}")),
        SqlValue::Blob(_) => Some(String::new()),
    }
}

pub fn coerce_composite(
    info: &crate::types::composite::CompositeInfo,
    text: &str,
) -> Result<SqlValue, PgError> {
    let malformed = || PgError::InvalidInputSyntax {
        typ: "record",
        input: format!("malformed record literal: \"{text}\""),
    };
    let parts = crate::types::composite::decode(text).map_err(|_| malformed())?;
    if parts.len() != info.fields.len() {
        return Err(malformed());
    }
    let mut out = Vec::with_capacity(parts.len());
    for (part, (_, foid, _)) in parts.iter().zip(&info.fields) {
        match part {
            None => out.push(None),
            Some(t) => {
                let v = types::input(*foid, t)?;
                out.push(match v {
                    SqlValue::Null => None,
                    other => Some(types::output(*foid, &other)),
                });
            }
        }
    }
    Ok(SqlValue::Text(crate::types::composite::encode(&out)))
}

fn eval_cast(v: &SqlValue, type_name: &str, ctx: EvalCtx) -> Result<SqlValue, PgError> {

    if let Some(d) = ctx.domain(type_name) {
        let base = cast_to_oid(v, d.base_oid)?;
        if matches!(base, SqlValue::Null) {
            if d.not_null {
                return Err(PgError::InvalidInputSyntax {
                    typ: "expression",
                    input: format!("domain {type_name} does not allow null values"),
                });
            }

            return Ok(base);
        }

        let vrow: [SqlValue; 1] = [base.clone()];
        for (cname, pred) in &d.checks {

            if matches!(eval_row(pred, &vrow, ctx)?, SqlValue::Int(0)) {
                return Err(PgError::InvalidInputSyntax {
                    typ: "expression",
                    input: format!(
                        "value for domain {type_name} violates check constraint \"{cname}\""
                    ),
                });
            }
        }
        return Ok(base);
    }

    if let Some(labels) = ctx.enum_labels_by_name(type_name) {
        return match v {
            SqlValue::Null => Ok(SqlValue::Null),
            SqlValue::Text(s) if labels.iter().any(|l| l == s) => Ok(SqlValue::Text(s.clone())),
            SqlValue::Text(s) => Err(PgError::InvalidEnumInput {
                enum_name: type_name.to_string(),
                input: s.clone(),
            }),
            SqlValue::Int(n) => Err(PgError::InvalidEnumInput {
                enum_name: type_name.to_string(),
                input: n.to_string(),
            }),
            SqlValue::Real(f) => Err(PgError::InvalidEnumInput {
                enum_name: type_name.to_string(),
                input: format!("{f}"),
            }),
            other => Err(PgError::InvalidEnumInput {
                enum_name: type_name.to_string(),
                input: format!("{other:?}"),
            }),
        };
    }

    if let Some(info) = ctx.composite_by_name(type_name) {
        return match v {
            SqlValue::Null => Ok(SqlValue::Null),
            SqlValue::Text(s) => coerce_composite(info, s),
            other => Err(PgError::InvalidInputSyntax {
                typ: "record",
                input: format!("malformed record literal: \"{other:?}\""),
            }),
        };
    }

    let target = resolve_type(type_name).ok_or(PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("type \"{type_name}\" does not exist"),
    })?;
    match cast_to_oid(v, target) {
        Ok(r) => Ok(r),

        Err(e) => {
            let Some(source) = cast_source_oid(v) else {
                return Err(e);
            };
            match ctx.regs.and_then(|r| r.casts.get(&(source, target))) {
                Some(def) => apply_user_cast(&def.func, v, ctx),
                None => Err(e),
            }
        }
    }
}

fn apply_user_cast(func: &str, v: &SqlValue, ctx: EvalCtx) -> Result<SqlValue, PgError> {
    let regs = ctx.regs.ok_or(PgError::InvalidInputSyntax {
        typ: "expression",
        input: "user cast requires a registry snapshot".to_string(),
    })?;
    let call = Expr::Func {
        name: func.to_string(),
        args: vec![Expr::Lit(v.clone())],
        distinct: false,
        filter: None,
        order_by: Vec::new(),
    };
    let inlined = crate::stmt::func_inline::inline_expr_pub(&call, regs)?;
    eval_row(&inlined, &[], ctx)
}

fn cast_to_oid(v: &SqlValue, target: u32) -> Result<SqlValue, PgError> {
    match v {
        SqlValue::Null => Ok(SqlValue::Null),
        SqlValue::Text(s) => types::input(target, s),
        SqlValue::Int(_) => types::cast(v, oid::INT8, target),
        SqlValue::Real(_) => types::cast(v, oid::FLOAT8, target),
        _ => Ok(v.clone()),
    }
}

fn cast_source_oid(v: &SqlValue) -> Option<u32> {
    match v {
        SqlValue::Null => None,
        SqlValue::Text(_) => Some(oid::TEXT),
        SqlValue::Int(_) => Some(oid::INT8),
        SqlValue::Real(_) => Some(oid::FLOAT8),
        SqlValue::Blob(_) => Some(oid::BYTEA),
    }
}

fn resolve_type(name: &str) -> Option<u32> {

    types::oid_for_type_name(name).or(match name {
        "point" => Some(oid::POINT),
        _ => None,
    })
}

fn truthy(v: &SqlValue) -> Option<bool> {
    match v {
        SqlValue::Null => None,
        SqlValue::Int(0) => Some(false),
        SqlValue::Int(_) => Some(true),
        _ => Some(true),
    }
}

fn as_f64(v: &SqlValue) -> Result<f64, PgError> {
    match v {
        SqlValue::Int(n) => Ok(*n as f64),
        SqlValue::Real(f) => Ok(*f),
        _ => Err(numeric_type_err()),
    }
}

fn value_f64(v: &SqlValue) -> Option<f64> {
    match v {
        SqlValue::Int(n) => Some(*n as f64),
        SqlValue::Real(f) => Some(*f),
        SqlValue::Text(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn numeric_type_err() -> PgError {
    PgError::InvalidInputSyntax {
        typ: "expression",
        input: "operator requires numeric operands".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::parser::parse;
    use super::*;

    fn ev(s: &str) -> SqlValue {
        eval(&parse(s).expect("parse ok")).expect("eval ok")
    }

    #[test]
    fn arithmetic_precedence_end_to_end() {

        assert_eq!(ev("1 + 2 * 3"), SqlValue::Int(7));
        assert_eq!(ev("(1 + 2) * 3"), SqlValue::Int(9));
        assert_eq!(ev("10 - 2 - 3"), SqlValue::Int(5));
    }

    #[test]
    fn integer_division_truncates_and_zero_errors() {
        assert_eq!(ev("7 / 2"), SqlValue::Int(3));
        assert_eq!(ev("7 % 2"), SqlValue::Int(1));
        assert_eq!(eval(&parse("1 / 0").unwrap()), Err(PgError::DivisionByZero));
        assert_eq!(eval(&parse("1 % 0").unwrap()), Err(PgError::DivisionByZero));
    }

    #[test]
    fn power_is_float8() {

        assert_eq!(ev("2 ^ 3"), SqlValue::Real(8.0));
        assert_eq!(ev("2 ^ 3 ^ 2"), SqlValue::Real(64.0));
    }

    #[test]
    fn numeric_promotion() {
        assert_eq!(ev("1 + 2.5"), SqlValue::Real(3.5));
        assert_eq!(ev("3 * 2.0"), SqlValue::Real(6.0));
    }

    #[test]
    fn unary_minus_over_pow() {
        assert_eq!(ev("-2 ^ 2"), SqlValue::Real(4.0));
    }

    #[test]
    fn comparisons_yield_boolean() {
        assert_eq!(ev("1 < 2"), SqlValue::Int(1));
        assert_eq!(ev("2 <= 2"), SqlValue::Int(1));
        assert_eq!(ev("3 <> 3"), SqlValue::Int(0));
        assert_eq!(ev("'a' < 'b'"), SqlValue::Int(1));
    }

    #[test]
    fn three_valued_logic() {
        assert_eq!(ev("true and false"), SqlValue::Int(0));
        assert_eq!(ev("true or false"), SqlValue::Int(1));
        assert_eq!(ev("not true"), SqlValue::Int(0));

        assert_eq!(ev("null and false"), SqlValue::Int(0));
        assert_eq!(ev("null and true"), SqlValue::Null);
        assert_eq!(ev("null or true"), SqlValue::Int(1));
        assert_eq!(ev("not null"), SqlValue::Null);
    }

    #[test]
    fn null_propagates_through_arithmetic() {
        assert_eq!(ev("1 + null"), SqlValue::Null);
        assert_eq!(ev("null * 3"), SqlValue::Null);
    }

    #[test]
    fn function_catalog_seam() {
        assert_eq!(ev("abs(-5)"), SqlValue::Int(5));
        assert_eq!(ev("abs(-2.5)"), SqlValue::Real(2.5));
        assert_eq!(ev("length('hello')"), SqlValue::Int(5));
        assert_eq!(ev("upper('abc')"), SqlValue::Text("ABC".into()));
        assert_eq!(ev("lower('ABC')"), SqlValue::Text("abc".into()));
        assert_eq!(ev("abs(null)"), SqlValue::Null);
    }

    #[test]
    fn unknown_function_errors() {
        assert!(eval(&parse("nope(1)").unwrap()).is_err());
    }

    #[test]
    fn integer_overflow_errors() {
        assert_eq!(
            eval(&parse("9223372036854775807 + 1").unwrap()),
            Err(PgError::Overflow { typ: "bigint" })
        );
    }

    #[test]
    fn cast_text_through_type_alphabet() {

        assert_eq!(ev("'(1, 2)'::point"), SqlValue::Text("(1,2)".into()));
        assert_eq!(ev("'42'::int"), SqlValue::Int(42));
    }

    #[test]
    fn free_column_is_an_error_in_const_eval() {
        assert!(eval(&parse("a + 1").unwrap()).is_err());
    }

    #[test]
    fn case_between_in_like() {

        assert_eq!(
            ev("case when 1 > 2 then 'a' else 'b' end"),
            SqlValue::Text("b".into())
        );
        assert_eq!(
            ev("case when 1 < 2 then 10 when 1 < 3 then 20 end"),
            SqlValue::Int(10)
        );

        assert_eq!(ev("case when false then 1 end"), SqlValue::Null);

        assert_eq!(
            ev("case 2 when 1 then 'x' when 2 then 'y' else 'z' end"),
            SqlValue::Text("y".into())
        );

        assert_eq!(ev("5 between 1 and 10"), SqlValue::Int(1));
        assert_eq!(ev("5 between 6 and 10"), SqlValue::Int(0));
        assert_eq!(ev("5 not between 6 and 10"), SqlValue::Int(1));

        assert_eq!(ev("3 in (1, 2, 3)"), SqlValue::Int(1));
        assert_eq!(ev("4 in (1, 2, 3)"), SqlValue::Int(0));
        assert_eq!(ev("4 not in (1, 2, 3)"), SqlValue::Int(1));

        assert_eq!(ev("'hello' like 'h%o'"), SqlValue::Int(1));
        assert_eq!(ev("'hello' like 'H%o'"), SqlValue::Int(0));
        assert_eq!(ev("'hello' ilike 'H%O'"), SqlValue::Int(1));
        assert_eq!(ev("'abc' not like 'x%'"), SqlValue::Int(1));
    }

    #[test]
    fn is_null_predicate() {
        assert_eq!(ev("null is null"), SqlValue::Int(1));
        assert_eq!(ev("1 is null"), SqlValue::Int(0));
        assert_eq!(ev("1 is not null"), SqlValue::Int(1));
        assert_eq!(ev("null is not null"), SqlValue::Int(0));

        assert_eq!(ev("1 = null is null"), SqlValue::Int(1));
    }

    #[test]
    fn catalog_end_to_end() {

        assert_eq!(ev("sqrt(16)"), SqlValue::Real(4.0));
        assert_eq!(ev("gcd(12, 8)"), SqlValue::Int(4));
        assert_eq!(
            ev("upper(concat('ab', 'cd'))"),
            SqlValue::Text("ABCD".into())
        );
        assert_eq!(ev("greatest(3, 7, 2)"), SqlValue::Int(7));

        assert_eq!(ev("'a' || 'b' || 'c'"), SqlValue::Text("abc".into()));
        assert_eq!(ev("6 & 3"), SqlValue::Int(2));

        assert_eq!(ev("|/ 25"), SqlValue::Real(5.0));
        assert_eq!(ev("@ -8"), SqlValue::Int(8));

        assert!(eval(&parse("1 <-> 2").unwrap()).is_err());
        assert!(eval(&parse("bogus(1)").unwrap()).is_err());
    }

    #[test]
    fn empty_ctx_user_type_cast_errors_cleanly() {

        let e = parse("'x'::mood").expect("parse");
        let err = eval_ctx(&e, EvalCtx::empty()).expect_err("must error");
        assert!(matches!(err, PgError::InvalidInputSyntax { .. }));
        let e2 = parse("'5'::posint").expect("parse");
        assert!(eval_ctx(&e2, EvalCtx::empty()).is_err());
    }

    #[test]
    fn catalog_wave5_end_to_end() {

        assert_eq!(ev("array_length('{1,2,3}', 1)"), SqlValue::Int(3));
        assert_eq!(ev("cardinality('{10,20}')"), SqlValue::Int(2));

        assert_eq!(ev("sind(30)"), SqlValue::Real(0.5));
        assert_eq!(ev("cosd(90)"), SqlValue::Real(0.0));

        assert_eq!(
            ev("'{\"a\": 5}'::jsonb ->> 'a'"),
            SqlValue::Text("5".into())
        );

        assert_eq!(
            ev("round('1234.56'::numeric, -2)"),
            SqlValue::Text("1200".into())
        );
    }
}
