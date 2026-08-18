
use crate::catalog::Catalog;
use crate::expr::ast::Expr;
use crate::stmt::ast::{SelectItem, SelectStmt};
use crate::types::PgError;
use sql_core::SqlValue;

pub fn mentions_seq_builtin(sql: &str) -> bool {
    let lower = sql.to_ascii_lowercase();
    lower.contains("nextval") || lower.contains("currval") || lower.contains("setval")
}

fn is_seq_fn(name: &str) -> bool {
    matches!(name, "nextval" | "currval" | "setval")
}

pub fn fold_seq_builtins(stmt: &mut SelectStmt, catalog: &mut Catalog) -> Result<(), PgError> {
    for item in &mut stmt.projection {
        if let SelectItem::Expr { expr, .. } = item {
            fold_expr(expr, catalog)?;
        }
    }
    if let Some(f) = &mut stmt.filter {
        fold_expr(f, catalog)?;
    }
    Ok(())
}

pub fn eval_default(dexpr: &Expr, catalog: &mut Catalog) -> Result<SqlValue, PgError> {
    let mut e = dexpr.clone();
    fold_expr(&mut e, catalog)?;
    crate::expr::eval::eval(&e)
}

fn fold_expr(e: &mut Expr, catalog: &mut Catalog) -> Result<(), PgError> {
    match e {
        Expr::Unary { expr, .. }
        | Expr::GenUnary { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::IsNull { expr, .. } => fold_expr(expr, catalog)?,
        Expr::Binary { left, right, .. } | Expr::GenBinary { left, right, .. } => {
            fold_expr(left, catalog)?;
            fold_expr(right, catalog)?;
        }
        Expr::Case {
            operand,
            whens,
            else_,
        } => {
            if let Some(o) = operand {
                fold_expr(o, catalog)?;
            }
            for (c, r) in whens.iter_mut() {
                fold_expr(c, catalog)?;
                fold_expr(r, catalog)?;
            }
            if let Some(d) = else_ {
                fold_expr(d, catalog)?;
            }
        }
        Expr::Func { name, args, .. } => {
            for a in args.iter_mut() {
                fold_expr(a, catalog)?;
            }
            if is_seq_fn(name) {
                let lit = eval_seq_fn(name, args, catalog)?;
                *e = Expr::Lit(lit);
            }
        }
        _ => {}
    }
    Ok(())
}

fn seq_name(arg: &Expr) -> Result<String, PgError> {
    match crate::expr::eval::eval(arg)? {
        SqlValue::Text(s) => Ok(s),
        other => Err(PgError::InvalidInputSyntax {
            typ: "expression",
            input: format!("sequence name must be text, got {other:?}"),
        }),
    }
}

fn eval_seq_fn(name: &str, args: &[Expr], catalog: &mut Catalog) -> Result<SqlValue, PgError> {
    match name {
        "nextval" => {
            require_arity(name, args, 1)?;
            let n = seq_name(&args[0])?;
            Ok(SqlValue::Int(catalog.seq_nextval(&n)?))
        }
        "currval" => {
            require_arity(name, args, 1)?;
            let n = seq_name(&args[0])?;
            Ok(SqlValue::Int(catalog.seq_currval(&n)?))
        }
        "setval" => {
            if args.len() != 2 && args.len() != 3 {
                return Err(arity_err(name, args.len()));
            }
            let n = seq_name(&args[0])?;
            let val = match crate::expr::eval::eval(&args[1])? {
                SqlValue::Int(v) => v,
                SqlValue::Real(v) => v as i64,
                other => {
                    return Err(PgError::InvalidInputSyntax {
                        typ: "expression",
                        input: format!("setval value must be integer, got {other:?}"),
                    })
                }
            };
            let is_called = if args.len() == 3 {
                matches!(crate::expr::eval::eval(&args[2])?, SqlValue::Int(1))
            } else {
                true
            };
            Ok(SqlValue::Int(catalog.seq_setval(&n, val, is_called)?))
        }
        _ => unreachable!("is_seq_fn gated this set"),
    }
}

fn require_arity(name: &str, args: &[Expr], want: usize) -> Result<(), PgError> {
    if args.len() != want {
        return Err(arity_err(name, args.len()));
    }
    Ok(())
}

fn arity_err(name: &str, got: usize) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("function {name}({got}) does not exist"),
    }
}
