
use super::ast::{FromItem, OrderKey, SelectItem, SelectStmt, SetOpArm};
use super::lower::inferred_name;
use crate::catalog::{FuncBody, FunctionDef};
use crate::expr::ast::Expr;
use crate::types::registry::TypeRegistries;
use crate::types::{self, PgError};

const MAX_INLINE_DEPTH: usize = 100;

fn err(msg: String) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "query",
        input: msg,
    }
}

fn contains_user_func(e: &Expr, regs: &TypeRegistries) -> bool {
    let any = |xs: &[Expr]| xs.iter().any(|x| contains_user_func(x, regs));
    match e {
        Expr::Func { name, args, .. } => regs.functions.contains_key(name) || any(args),
        Expr::Unary { expr, .. }
        | Expr::GenUnary { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::IsNull { expr, .. } => contains_user_func(expr, regs),
        Expr::Binary { left, right, .. } => {
            contains_user_func(left, regs) || contains_user_func(right, regs)
        }

        Expr::GenBinary { op, left, right } => {
            regs.operators.contains_key(op)
                || contains_user_func(left, regs)
                || contains_user_func(right, regs)
        }
        Expr::Row(xs) => any(xs),
        Expr::FieldAccess { base, .. } => contains_user_func(base, regs),
        Expr::InSubquery { expr, .. } | Expr::Quantified { expr, .. } => {
            contains_user_func(expr, regs)
        }
        Expr::Case {
            operand,
            whens,
            else_,
        } => {
            operand
                .as_deref()
                .map(|o| contains_user_func(o, regs))
                .unwrap_or(false)
                || whens
                    .iter()
                    .any(|(c, r)| contains_user_func(c, regs) || contains_user_func(r, regs))
                || else_
                    .as_deref()
                    .map(|x| contains_user_func(x, regs))
                    .unwrap_or(false)
        }
        Expr::Window {
            args,
            partition_by,
            order_by,
            ..
        } => {
            any(args)
                || any(partition_by)
                || order_by.iter().any(|k| contains_user_func(&k.expr, regs))
        }

        _ => false,
    }
}

pub(crate) fn inline_stmt(s: &SelectStmt, regs: &TypeRegistries) -> Result<SelectStmt, PgError> {
    let ie = |e: &Expr| inline_expr(e, regs, 0);
    let ik = |k: &OrderKey| -> Result<OrderKey, PgError> {
        Ok(OrderKey {
            expr: inline_expr(&k.expr, regs, 0)?,
            descending: k.descending,
            nulls_first: k.nulls_first,
            comp_oid: k.comp_oid,
        })
    };
    Ok(SelectStmt {
        distinct: s.distinct,
        distinct_on: s.distinct_on.iter().map(&ie).collect::<Result<_, _>>()?,
        projection: s
            .projection
            .iter()
            .map(|it| match it {
                SelectItem::Star => Ok(SelectItem::Star),
                SelectItem::Expr { expr, alias } => {
                    if contains_user_func(expr, regs) {

                        let alias = alias.clone().or_else(|| Some(inferred_name(expr)));
                        Ok(SelectItem::Expr {
                            expr: inline_expr(expr, regs, 0)?,
                            alias,
                        })
                    } else {
                        Ok(it.clone())
                    }
                }
            })
            .collect::<Result<_, PgError>>()?,
        from: s.from.as_ref().map(|f| inline_from(f, regs)).transpose()?,
        filter: s.filter.as_ref().map(&ie).transpose()?,
        group_by: s.group_by.iter().map(&ie).collect::<Result<_, _>>()?,
        grouping_sets: s
            .grouping_sets
            .iter()
            .map(|set| set.iter().map(&ie).collect::<Result<_, _>>())
            .collect::<Result<_, _>>()?,
        having: s.having.as_ref().map(&ie).transpose()?,
        order_by: s.order_by.iter().map(ik).collect::<Result<_, _>>()?,
        limit: s.limit,
        offset: s.offset,
        windows: s.windows.clone(),
        tail: s
            .tail
            .iter()
            .map(|a| {
                Ok(SetOpArm {
                    op: a.op,
                    all: a.all,
                    arm: inline_stmt(&a.arm, regs)?,
                })
            })
            .collect::<Result<_, PgError>>()?,
        locking: s.locking.clone(),
    })
}

fn inline_from(f: &FromItem, regs: &TypeRegistries) -> Result<FromItem, PgError> {
    Ok(match f {
        FromItem::Table { .. } => f.clone(),
        FromItem::Join {
            left,
            right,
            kind,
            on,
            using,
            natural,
        } => FromItem::Join {
            left: Box::new(inline_from(left, regs)?),
            right: Box::new(inline_from(right, regs)?),
            kind: *kind,
            on: on.as_ref().map(|e| inline_expr(e, regs, 0)).transpose()?,
            using: using.clone(),
            natural: *natural,
        },
        FromItem::Subquery {
            query,
            alias,
            lateral,
        } => FromItem::Subquery {
            query: Box::new(inline_stmt(query, regs)?),
            alias: alias.clone(),
            lateral: *lateral,
        },
        FromItem::Function {
            name,
            args,
            alias,
            lateral,
        } => FromItem::Function {
            name: name.clone(),
            args: args
                .iter()
                .map(|a| inline_expr(a, regs, 0))
                .collect::<Result<_, _>>()?,
            alias: alias.clone(),
            lateral: *lateral,
        },
    })
}

pub(crate) fn inline_expr_pub(e: &Expr, regs: &TypeRegistries) -> Result<Expr, PgError> {
    inline_expr(e, regs, 0)
}

fn inline_expr(e: &Expr, regs: &TypeRegistries, depth: usize) -> Result<Expr, PgError> {
    let rec = |x: &Expr| inline_expr(x, regs, depth);
    let ik = |k: &OrderKey| -> Result<OrderKey, PgError> {
        Ok(OrderKey {
            expr: inline_expr(&k.expr, regs, depth)?,
            descending: k.descending,
            nulls_first: k.nulls_first,
            comp_oid: k.comp_oid,
        })
    };
    Ok(match e {
        Expr::Func {
            name,
            args,
            distinct,
            filter,
            order_by,
        } if !*distinct
            && filter.is_none()
            && order_by.is_empty()
            && regs
                .functions
                .get(name)
                .map(|f| f.language == crate::catalog::Lang::Sql)
                .unwrap_or(false) =>
        {
            let fdef = &regs.functions[name];

            if args.len() != fdef.args.len() {
                return Ok(Expr::Func {
                    name: name.clone(),
                    args: args.iter().map(&rec).collect::<Result<_, _>>()?,
                    distinct: *distinct,
                    filter: filter.clone(),
                    order_by: order_by.clone(),
                });
            }
            if depth >= MAX_INLINE_DEPTH {
                return Err(err(format!(
                    "cannot inline recursive or too-deeply-nested SQL function \"{name}\""
                )));
            }
            build_inline(fdef, args, regs, depth)?
        }
        Expr::Func {
            name,
            args,
            distinct,
            filter,
            order_by,
        } => Expr::Func {
            name: name.clone(),
            args: args.iter().map(&rec).collect::<Result<_, _>>()?,
            distinct: *distinct,
            filter: filter.as_deref().map(&rec).transpose()?.map(Box::new),
            order_by: order_by.iter().map(ik).collect::<Result<_, _>>()?,
        },
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: Box::new(rec(expr)?),
        },
        Expr::Binary { op, left, right } => Expr::Binary {
            op: *op,
            left: Box::new(rec(left)?),
            right: Box::new(rec(right)?),
        },

        Expr::GenBinary { op, left, right } if regs.operators.contains_key(op) => {
            let fname = regs.operators[op].clone();
            let call = Expr::Func {
                name: fname,
                args: vec![(**left).clone(), (**right).clone()],
                distinct: false,
                filter: None,
                order_by: Vec::new(),
            };
            inline_expr(&call, regs, depth)?
        }
        Expr::GenBinary { op, left, right } => Expr::GenBinary {
            op: op.clone(),
            left: Box::new(rec(left)?),
            right: Box::new(rec(right)?),
        },
        Expr::GenUnary { op, expr } => Expr::GenUnary {
            op: op.clone(),
            expr: Box::new(rec(expr)?),
        },
        Expr::Cast { expr, type_name } => Expr::Cast {
            expr: Box::new(rec(expr)?),
            type_name: type_name.clone(),
        },
        Expr::IsNull { expr, negated } => Expr::IsNull {
            expr: Box::new(rec(expr)?),
            negated: *negated,
        },
        Expr::Row(xs) => Expr::Row(xs.iter().map(&rec).collect::<Result<_, _>>()?),
        Expr::FieldAccess {
            base,
            field,
            comp_oid,
            field_oid,
        } => Expr::FieldAccess {
            base: Box::new(rec(base)?),
            field: field.clone(),
            comp_oid: *comp_oid,
            field_oid: *field_oid,
        },
        Expr::Case {
            operand,
            whens,
            else_,
        } => Expr::Case {
            operand: operand.as_deref().map(&rec).transpose()?.map(Box::new),
            whens: whens
                .iter()
                .map(|(c, r)| Ok((rec(c)?, rec(r)?)))
                .collect::<Result<_, PgError>>()?,
            else_: else_.as_deref().map(&rec).transpose()?.map(Box::new),
        },
        Expr::ScalarSubquery(q) => Expr::ScalarSubquery(Box::new(inline_stmt(q, regs)?)),
        Expr::Exists { query, negated } => Expr::Exists {
            query: Box::new(inline_stmt(query, regs)?),
            negated: *negated,
        },
        Expr::InSubquery {
            expr,
            query,
            negated,
        } => Expr::InSubquery {
            expr: Box::new(rec(expr)?),
            query: Box::new(inline_stmt(query, regs)?),
            negated: *negated,
        },
        Expr::Quantified {
            expr,
            op,
            quantifier,
            query,
        } => Expr::Quantified {
            expr: Box::new(rec(expr)?),
            op: *op,
            quantifier: *quantifier,
            query: Box::new(inline_stmt(query, regs)?),
        },
        Expr::Window {
            func,
            args,
            partition_by,
            order_by,
            frame,
            window_ref,
        } => Expr::Window {
            func: func.clone(),
            args: args.iter().map(&rec).collect::<Result<_, _>>()?,
            partition_by: partition_by.iter().map(&rec).collect::<Result<_, _>>()?,
            order_by: order_by.iter().map(ik).collect::<Result<_, _>>()?,
            frame: frame.clone(),
            window_ref: window_ref.clone(),
        },

        _ => e.clone(),
    })
}

fn build_inline(
    fdef: &FunctionDef,
    call_args: &[Expr],
    regs: &TypeRegistries,
    depth: usize,
) -> Result<Expr, PgError> {

    let mut subst: Vec<(String, Expr)> = Vec::with_capacity(fdef.args.len() * 2);
    let mut any_null = false;
    for (i, ((argname, oid, _typmod), raw)) in fdef.args.iter().zip(call_args).enumerate() {
        let inlined = inline_expr(raw, regs, depth)?;
        if matches!(inlined, Expr::Null) {
            any_null = true;
        }
        let coerced = cast_to(inlined, *oid);

        subst.push((format!("${}", i + 1), coerced.clone()));

        if let Some(n) = argname {
            subst.push((n.clone(), coerced));
        }
    }

    if fdef.strict && any_null {
        return Ok(cast_to(Expr::Null, fdef.ret_oid));
    }

    let substituted = match &fdef.body {
        FuncBody::Expr(body) => subst_expr(body, &subst),
        FuncBody::Query(q) => Expr::ScalarSubquery(Box::new(subst_stmt(q, &subst))),
    };

    let inlined = inline_expr(&substituted, regs, depth + 1)?;
    Ok(cast_to(inlined, fdef.ret_oid))
}

pub(crate) fn setof_body(fdef: &FunctionDef, call_args: &[Expr]) -> SelectStmt {
    let mut subst: Vec<(String, Expr)> = Vec::with_capacity(fdef.args.len() * 2);
    for (i, ((argname, oid, _typmod), raw)) in fdef.args.iter().zip(call_args).enumerate() {
        let coerced = cast_to(raw.clone(), *oid);
        subst.push((format!("${}", i + 1), coerced.clone()));
        if let Some(n) = argname {
            subst.push((n.clone(), coerced));
        }
    }
    let body = match &fdef.body {
        FuncBody::Query(q) => q.as_ref(),

        FuncBody::Expr(_) => unreachable!("set-returning function body must be a query"),
    };
    subst_stmt(body, &subst)
}

fn cast_to(e: Expr, oid: u32) -> Expr {
    if oid == 0 {
        return e;
    }
    let name = types::type_name(oid);
    if name.is_empty() {
        return e;
    }
    Expr::Cast {
        expr: Box::new(e),
        type_name: name.to_string(),
    }
}

pub(crate) fn subst_expr_pub(e: &Expr, subst: &[(String, Expr)]) -> Expr {
    subst_expr(e, subst)
}

fn subst_expr(e: &Expr, subst: &[(String, Expr)]) -> Expr {
    let rec = |x: &Expr| subst_expr(x, subst);
    let ik = |k: &OrderKey| OrderKey {
        expr: subst_expr(&k.expr, subst),
        descending: k.descending,
        nulls_first: k.nulls_first,
        comp_oid: k.comp_oid,
    };
    match e {
        Expr::Column(name) => subst
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| e.clone()),
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: Box::new(rec(expr)),
        },
        Expr::Binary { op, left, right } => Expr::Binary {
            op: *op,
            left: Box::new(rec(left)),
            right: Box::new(rec(right)),
        },
        Expr::GenBinary { op, left, right } => Expr::GenBinary {
            op: op.clone(),
            left: Box::new(rec(left)),
            right: Box::new(rec(right)),
        },
        Expr::GenUnary { op, expr } => Expr::GenUnary {
            op: op.clone(),
            expr: Box::new(rec(expr)),
        },
        Expr::Cast { expr, type_name } => Expr::Cast {
            expr: Box::new(rec(expr)),
            type_name: type_name.clone(),
        },
        Expr::IsNull { expr, negated } => Expr::IsNull {
            expr: Box::new(rec(expr)),
            negated: *negated,
        },
        Expr::Func {
            name,
            args,
            distinct,
            filter,
            order_by,
        } => Expr::Func {
            name: name.clone(),
            args: args.iter().map(&rec).collect(),
            distinct: *distinct,
            filter: filter.as_deref().map(&rec).map(Box::new),
            order_by: order_by.iter().map(ik).collect(),
        },
        Expr::Row(xs) => Expr::Row(xs.iter().map(&rec).collect()),
        Expr::FieldAccess {
            base,
            field,
            comp_oid,
            field_oid,
        } => Expr::FieldAccess {
            base: Box::new(rec(base)),
            field: field.clone(),
            comp_oid: *comp_oid,
            field_oid: *field_oid,
        },
        Expr::Case {
            operand,
            whens,
            else_,
        } => Expr::Case {
            operand: operand.as_deref().map(&rec).map(Box::new),
            whens: whens.iter().map(|(c, r)| (rec(c), rec(r))).collect(),
            else_: else_.as_deref().map(&rec).map(Box::new),
        },
        Expr::ScalarSubquery(q) => Expr::ScalarSubquery(Box::new(subst_stmt(q, subst))),
        Expr::Exists { query, negated } => Expr::Exists {
            query: Box::new(subst_stmt(query, subst)),
            negated: *negated,
        },
        Expr::InSubquery {
            expr,
            query,
            negated,
        } => Expr::InSubquery {
            expr: Box::new(rec(expr)),
            query: Box::new(subst_stmt(query, subst)),
            negated: *negated,
        },
        Expr::Quantified {
            expr,
            op,
            quantifier,
            query,
        } => Expr::Quantified {
            expr: Box::new(rec(expr)),
            op: *op,
            quantifier: *quantifier,
            query: Box::new(subst_stmt(query, subst)),
        },
        Expr::Window {
            func,
            args,
            partition_by,
            order_by,
            frame,
            window_ref,
        } => Expr::Window {
            func: func.clone(),
            args: args.iter().map(&rec).collect(),
            partition_by: partition_by.iter().map(&rec).collect(),
            order_by: order_by.iter().map(ik).collect(),
            frame: frame.clone(),
            window_ref: window_ref.clone(),
        },
        _ => e.clone(),
    }
}

fn subst_stmt(s: &SelectStmt, subst: &[(String, Expr)]) -> SelectStmt {
    let se = |e: &Expr| subst_expr(e, subst);
    let sk = |k: &OrderKey| OrderKey {
        expr: subst_expr(&k.expr, subst),
        descending: k.descending,
        nulls_first: k.nulls_first,
        comp_oid: k.comp_oid,
    };
    SelectStmt {
        distinct: s.distinct,
        distinct_on: s.distinct_on.iter().map(&se).collect(),
        projection: s
            .projection
            .iter()
            .map(|it| match it {
                SelectItem::Star => SelectItem::Star,
                SelectItem::Expr { expr, alias } => SelectItem::Expr {
                    expr: subst_expr(expr, subst),
                    alias: alias.clone(),
                },
            })
            .collect(),
        from: s.from.as_ref().map(|f| subst_from(f, subst)),
        filter: s.filter.as_ref().map(&se),
        group_by: s.group_by.iter().map(&se).collect(),
        grouping_sets: s
            .grouping_sets
            .iter()
            .map(|set| set.iter().map(&se).collect())
            .collect(),
        having: s.having.as_ref().map(&se),
        order_by: s.order_by.iter().map(sk).collect(),
        limit: s.limit,
        offset: s.offset,
        windows: s.windows.clone(),
        tail: s
            .tail
            .iter()
            .map(|a| SetOpArm {
                op: a.op,
                all: a.all,
                arm: subst_stmt(&a.arm, subst),
            })
            .collect(),
        locking: s.locking.clone(),
    }
}

fn subst_from(f: &FromItem, subst: &[(String, Expr)]) -> FromItem {
    match f {
        FromItem::Table { .. } => f.clone(),
        FromItem::Join {
            left,
            right,
            kind,
            on,
            using,
            natural,
        } => FromItem::Join {
            left: Box::new(subst_from(left, subst)),
            right: Box::new(subst_from(right, subst)),
            kind: *kind,
            on: on.as_ref().map(|e| subst_expr(e, subst)),
            using: using.clone(),
            natural: *natural,
        },
        FromItem::Subquery {
            query,
            alias,
            lateral,
        } => FromItem::Subquery {
            query: Box::new(subst_stmt(query, subst)),
            alias: alias.clone(),
            lateral: *lateral,
        },
        FromItem::Function {
            name,
            args,
            alias,
            lateral,
        } => FromItem::Function {
            name: name.clone(),
            args: args.iter().map(|a| subst_expr(a, subst)).collect(),
            alias: alias.clone(),
            lateral: *lateral,
        },
    }
}
