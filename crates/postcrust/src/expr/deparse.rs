
use super::ast::{BinOp, Expr, UnOp};

fn binop_sym(op: BinOp) -> &'static str {
    match op {
        BinOp::Or => "OR",
        BinOp::And => "AND",
        BinOp::Lt => "<",
        BinOp::Gt => ">",
        BinOp::Eq => "=",
        BinOp::LtEq => "<=",
        BinOp::GtEq => ">=",
        BinOp::NotEq => "<>",
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
        BinOp::Pow => "^",
    }
}

fn quote_literal(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

pub fn deparse(e: &Expr) -> String {
    match e {
        Expr::Null => "NULL".to_string(),
        Expr::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        Expr::Int(n) => n.to_string(),
        Expr::Float(f) => f.to_string(),
        Expr::Str(s) => quote_literal(s),
        Expr::Column(name) => name.clone(),
        Expr::ColumnRef(i) => format!("${i}"),

        Expr::Binary { op, left, right } => {
            format!("({} {} {})", deparse(left), binop_sym(*op), deparse(right))
        }
        Expr::GenBinary { op, left, right } => {
            format!("({} {} {})", deparse(left), op, deparse(right))
        }
        Expr::Unary { op, expr } => match op {
            UnOp::Neg => format!("-{}", deparse(expr)),
            UnOp::Plus => format!("+{}", deparse(expr)),
            UnOp::Not => format!("(NOT {})", deparse(expr)),
        },
        Expr::GenUnary { op, expr } => format!("{}{}", op, deparse(expr)),
        Expr::Cast { expr, type_name } => format!("{}::{}", deparse(expr), type_name),
        Expr::IsNull { expr, negated } => {
            format!(
                "({} IS {}NULL)",
                deparse(expr),
                if *negated { "NOT " } else { "" }
            )
        }

        Expr::Func { name, args, .. } if name == "nextval" && args.len() == 1 => {
            if let Expr::Str(seq) = &args[0] {
                format!("nextval({}::regclass)", quote_literal(seq))
            } else {
                format!("nextval({})", deparse(&args[0]))
            }
        }

        Expr::Func { name, args, .. } => {
            let rendered: Vec<String> = args.iter().map(deparse).collect();
            format!("{}({})", name, rendered.join(", "))
        }

        other => format!("{other:?}"),
    }
}

pub fn deparse_check(e: &Expr, schema: &super::bind::Schema, col_types: &[u32]) -> String {
    match e {
        Expr::Binary { op, left, right } => format!(
            "({} {} {})",
            check_operand(left, right, schema, col_types),
            binop_sym(*op),
            check_operand(right, left, schema, col_types),
        ),
        Expr::GenBinary { op, left, right } => format!(
            "({} {} {})",
            check_operand(left, right, schema, col_types),
            op,
            check_operand(right, left, schema, col_types),
        ),
        Expr::Unary { op, expr } if matches!(op, UnOp::Not) => {
            format!("(NOT {})", deparse_check(expr, schema, col_types))
        }
        _ => deparse(e),
    }
}

fn check_operand(
    this: &Expr,
    sibling: &Expr,
    schema: &super::bind::Schema,
    col_types: &[u32],
) -> String {
    if let Expr::Str(s) = this {
        if super::infer::infer(sibling, schema, col_types) == Some(crate::types::oid::TEXT) {
            return format!("{}::text", quote_literal(s));
        }
    }
    deparse_check(this, schema, col_types)
}

pub fn column_default_text(default: &Expr, col_oid: u32) -> String {
    match default {
        Expr::Str(s) => format!("{}::{}", quote_literal(s), crate::types::type_name(col_oid)),
        other => deparse(other),
    }
}
