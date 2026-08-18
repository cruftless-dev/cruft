
use crate::catalog::Catalog;
use crate::expr::ast::Expr;
use crate::stmt::ast::{SelectItem, SelectStmt};
use crate::types::PgError;
use sql_core::SqlValue;

pub fn mentions_description_builtin(sql: &str) -> bool {
    let l = sql.to_ascii_lowercase();
    l.contains("obj_description") || l.contains("col_description")
}

fn err(msg: String) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "expression",
        input: msg,
    }
}

pub fn fold_description_builtins(stmt: &mut SelectStmt, catalog: &Catalog) -> Result<(), PgError> {
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

fn fold_expr(e: &mut Expr, catalog: &Catalog) -> Result<(), PgError> {
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
            if name == "obj_description" && (args.len() == 1 || args.len() == 2) {
                let rel = regclass_name(&args[0])?;
                *e = Expr::Lit(comment_cell(catalog, &rel, 0));
            } else if name == "col_description" && args.len() == 2 {
                let rel = regclass_name(&args[0])?;
                let sub = int_arg(&args[1])?;
                *e = Expr::Lit(comment_cell(catalog, &rel, sub));
            }
        }
        _ => {}
    }
    Ok(())
}

fn comment_cell(catalog: &Catalog, rel: &str, subid: i32) -> SqlValue {
    catalog
        .get_comment(rel, subid)
        .map(|s| SqlValue::Text(s.clone()))
        .unwrap_or(SqlValue::Null)
}

fn regclass_name(arg: &Expr) -> Result<String, PgError> {
    match arg {
        Expr::Cast { expr, type_name } if type_name.eq_ignore_ascii_case("regclass") => {
            regclass_name(expr)
        }
        Expr::Str(s) => Ok(s.clone()),
        Expr::Lit(SqlValue::Text(s)) => Ok(s.clone()),
        _ => Err(err(
            "obj_description/col_description requires a 'name'::regclass literal argument"
                .to_string(),
        )),
    }
}

fn int_arg(arg: &Expr) -> Result<i32, PgError> {
    match arg {
        Expr::Int(n) => Ok(*n as i32),
        Expr::Lit(SqlValue::Int(n)) => Ok(*n as i32),
        _ => Err(err(
            "col_description requires an integer column number".to_string()
        )),
    }
}
