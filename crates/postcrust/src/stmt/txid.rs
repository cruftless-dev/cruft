
use crate::catalog::Catalog;
use crate::expr::ast::Expr;
use crate::stmt::ast::{SelectItem, SelectStmt};
use crate::types::PgError;
use sql_core::SqlValue;

pub fn mentions_txid_builtin(sql: &str) -> bool {
    let lower = sql.to_ascii_lowercase();
    lower.contains("pg_current_xact_id")
        || lower.contains("txid_current")
        || lower.contains("txid_status")
}

fn is_txid_fn(name: &str) -> bool {
    matches!(
        name,
        "pg_current_xact_id"
            | "pg_current_xact_id_if_assigned"
            | "txid_current"
            | "txid_current_if_assigned"
            | "txid_status"
    )
}

pub fn fold_txid_builtins(stmt: &mut SelectStmt, catalog: &mut Catalog) -> Result<(), PgError> {
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
            if is_txid_fn(name) {
                let lit = eval_txid_fn(name, args, catalog)?;
                *e = Expr::Lit(lit);
            }
        }
        _ => {}
    }
    Ok(())
}

fn eval_txid_fn(name: &str, args: &[Expr], catalog: &mut Catalog) -> Result<SqlValue, PgError> {
    match name {

        "pg_current_xact_id" | "txid_current" => {
            require_arity(name, args, 0)?;
            Ok(SqlValue::Int(catalog.assign_current_xid() as i64))
        }

        "pg_current_xact_id_if_assigned" | "txid_current_if_assigned" => {
            require_arity(name, args, 0)?;
            Ok(match catalog.current_xid_if_assigned() {
                Some(x) => SqlValue::Int(x as i64),
                None => SqlValue::Null,
            })
        }

        "txid_status" => {
            require_arity(name, args, 1)?;
            let v = crate::expr::eval::eval(&args[0])?;
            let xid = match v {
                SqlValue::Null => return Ok(SqlValue::Null),
                SqlValue::Int(n) if n >= 0 => n as u64,
                SqlValue::Int(_) => return Ok(SqlValue::Null),
                other => {

                    match other.as_i64_lenient() {
                        Some(n) if n >= 0 => n as u64,
                        _ => return Ok(SqlValue::Null),
                    }
                }
            };
            Ok(match catalog.txid_status(xid) {
                Some(s) => SqlValue::Text(s.to_string()),
                None => SqlValue::Null,
            })
        }
        _ => unreachable!("is_txid_fn gated this set"),
    }
}

fn require_arity(name: &str, args: &[Expr], want: usize) -> Result<(), PgError> {
    if args.len() != want {
        return Err(PgError::InvalidInputSyntax {
            typ: "expression",
            input: format!("function {name}({}) does not exist", args.len()),
        });
    }
    Ok(())
}

trait AsI64Lenient {
    fn as_i64_lenient(&self) -> Option<i64>;
}
impl AsI64Lenient for SqlValue {
    fn as_i64_lenient(&self) -> Option<i64> {
        match self {
            SqlValue::Int(n) => Some(*n),
            SqlValue::Real(r) => Some(*r as i64),
            SqlValue::Text(s) => s.trim().parse().ok(),
            _ => None,
        }
    }
}
