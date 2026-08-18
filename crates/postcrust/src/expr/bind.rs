
use super::ast::Expr;
use super::eval::{eval_row, EvalCtx};
use super::PgError;
use crate::types::registry::TypeRegistries;
use sql_core::{Pred, Row, Scalar, SqlValue};
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub struct Schema {

    cols: Vec<(Option<String>, String)>,
}

impl Schema {

    pub fn new<I, S>(names: I) -> Schema
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Schema {
            cols: names.into_iter().map(|n| (None, n.into())).collect(),
        }
    }

    pub fn qualified(mut self, table: &str) -> Schema {
        for c in &mut self.cols {
            c.0 = Some(table.to_string());
        }
        self
    }

    pub fn concat(mut self, other: Schema) -> Schema {
        self.cols.extend(other.cols);
        self
    }

    pub fn index_of(&self, name: &str) -> Option<usize> {
        if let Some((t, c)) = name.split_once('.') {
            self.cols
                .iter()
                .position(|(ct, cn)| ct.as_deref() == Some(t) && cn == c)
        } else {
            self.cols.iter().position(|(_, cn)| cn == name)
        }
    }

    pub fn width(&self) -> usize {
        self.cols.len()
    }

    pub fn names(&self) -> Vec<String> {
        self.cols.iter().map(|(_, n)| n.clone()).collect()
    }

    pub fn cols(&self) -> Vec<(Option<String>, String)> {
        self.cols.clone()
    }
}

pub fn resolve(e: &Expr, schema: &Schema) -> Result<Expr, PgError> {
    Ok(match e {
        Expr::Column(name) => {
            let idx = schema
                .index_of(name)
                .ok_or_else(|| PgError::InvalidInputSyntax {
                    typ: "expression",
                    input: format!("column \"{name}\" does not exist"),
                })?;
            Expr::ColumnRef(idx)
        }

        Expr::Null
        | Expr::Bool(_)
        | Expr::Int(_)
        | Expr::Float(_)
        | Expr::Str(_)
        | Expr::ColumnRef(_)
        | Expr::Lit(_)
        | Expr::ScalarSubquery(_)
        | Expr::Exists { .. }
        | Expr::InSubquery { .. }
        | Expr::Quantified { .. }
        | Expr::Window { .. } => e.clone(),
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: Box::new(resolve(expr, schema)?),
        },
        Expr::Binary { op, left, right } => Expr::Binary {
            op: *op,
            left: Box::new(resolve(left, schema)?),
            right: Box::new(resolve(right, schema)?),
        },
        Expr::Func {
            name,
            args,
            distinct,
            filter,
            order_by,
        } => Expr::Func {
            name: name.clone(),
            args: args
                .iter()
                .map(|a| resolve(a, schema))
                .collect::<Result<_, _>>()?,
            distinct: *distinct,
            filter: match filter {
                Some(f) => Some(Box::new(resolve(f, schema)?)),
                None => None,
            },
            order_by: order_by
                .iter()
                .map(|k| {
                    Ok(crate::stmt::ast::OrderKey {
                        expr: resolve(&k.expr, schema)?,
                        descending: k.descending,
                        nulls_first: k.nulls_first,
                        comp_oid: k.comp_oid,
                    })
                })
                .collect::<Result<_, PgError>>()?,
        },
        Expr::GenBinary { op, left, right } => Expr::GenBinary {
            op: op.clone(),
            left: Box::new(resolve(left, schema)?),
            right: Box::new(resolve(right, schema)?),
        },
        Expr::GenUnary { op, expr } => Expr::GenUnary {
            op: op.clone(),
            expr: Box::new(resolve(expr, schema)?),
        },
        Expr::Cast { expr, type_name } => Expr::Cast {
            expr: Box::new(resolve(expr, schema)?),
            type_name: type_name.clone(),
        },
        Expr::Collate { expr, collation } => Expr::Collate {
            expr: Box::new(resolve(expr, schema)?),
            collation: collation.clone(),
        },
        Expr::Array(elems) => Expr::Array(
            elems
                .iter()
                .map(|e| resolve(e, schema))
                .collect::<Result<_, _>>()?,
        ),
        Expr::Row(elems) => Expr::Row(
            elems
                .iter()
                .map(|e| resolve(e, schema))
                .collect::<Result<_, _>>()?,
        ),
        Expr::FieldAccess {
            base,
            field,
            comp_oid,
            field_oid,
        } => Expr::FieldAccess {
            base: Box::new(resolve(base, schema)?),
            field: field.clone(),
            comp_oid: *comp_oid,
            field_oid: *field_oid,
        },
        Expr::IsNull { expr, negated } => Expr::IsNull {
            expr: Box::new(resolve(expr, schema)?),
            negated: *negated,
        },
        Expr::Case {
            operand,
            whens,
            else_,
        } => Expr::Case {
            operand: match operand {
                Some(o) => Some(Box::new(resolve(o, schema)?)),
                None => None,
            },
            whens: whens
                .iter()
                .map(|(c, r)| Ok((resolve(c, schema)?, resolve(r, schema)?)))
                .collect::<Result<_, PgError>>()?,
            else_: match else_ {
                Some(e) => Some(Box::new(resolve(e, schema)?)),
                None => None,
            },
        },
    })
}

pub fn lower(e: &Expr, schema: &Schema, regs: Arc<TypeRegistries>) -> Result<Scalar, PgError> {
    let bound = resolve(e, schema)?;

    Ok(Box::new(move |row: &Row| {
        eval_row(&bound, row, EvalCtx::new(&regs)).map_err(|err| err.message())
    }))
}

pub fn lower_pred(e: &Expr, schema: &Schema, regs: Arc<TypeRegistries>) -> Result<Pred, PgError> {
    let bound = resolve(e, schema)?;
    Ok(Box::new(move |row: &Row| {
        match eval_row(&bound, row, EvalCtx::new(&regs)).map_err(|err| err.message())? {

            SqlValue::Int(0) => Ok(false),
            SqlValue::Int(_) => Ok(true),

            SqlValue::Null => Ok(false),
            other => Err(format!("argument of WHERE must be boolean, not {other:?}")),
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::super::parser::parse;
    use super::*;
    use sql_core::Plan;

    fn schema() -> Schema {
        Schema::new(["id", "v", "name"])
    }

    fn regs() -> Arc<TypeRegistries> {
        Arc::new(TypeRegistries::default())
    }

    fn lower_str(s: &str, sc: &Schema) -> Scalar {
        lower(&parse(s).expect("parse"), sc, regs()).expect("lower")
    }

    #[test]
    fn resolve_binds_names_to_indices() {
        let bound = resolve(&parse("v + 1").unwrap(), &schema()).unwrap();

        assert_eq!(
            bound,
            Expr::Binary {
                op: super::super::ast::BinOp::Add,
                left: Box::new(Expr::ColumnRef(1)),
                right: Box::new(Expr::Int(1)),
            }
        );
    }

    #[test]
    fn unknown_column_errors_at_bind_time() {
        let e = parse("nope + 1").unwrap();
        assert!(matches!(
            resolve(&e, &schema()),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn scalar_reads_the_row() {
        let f = lower_str("v * 10 + id", &schema());

        let row: Row = vec![
            SqlValue::Int(3),
            SqlValue::Int(5),
            SqlValue::Text("x".into()),
        ];
        assert_eq!(f(&row).unwrap(), SqlValue::Int(53));
    }

    #[test]
    fn scalar_runs_catalog_functions_over_columns() {
        let f = lower_str("upper(name)", &schema());
        let row: Row = vec![
            SqlValue::Int(1),
            SqlValue::Int(2),
            SqlValue::Text("abc".into()),
        ];
        assert_eq!(f(&row).unwrap(), SqlValue::Text("ABC".into()));
    }

    #[test]
    fn predicate_three_valued_where() {
        let p = lower_pred(&parse("v >= 2").unwrap(), &schema(), regs()).unwrap();
        let row = |v: i64| vec![SqlValue::Int(0), SqlValue::Int(v), SqlValue::Null];
        assert!(p(&row(5)).unwrap());
        assert!(!p(&row(1)).unwrap());

        let pnull = lower_pred(&parse("v = null").unwrap(), &schema(), regs()).unwrap();
        assert!(!pnull(&row(5)).unwrap());
    }

    #[test]
    fn end_to_end_through_sql_core_executor() {

        let rows: Vec<Row> = vec![
            vec![
                SqlValue::Int(0),
                SqlValue::Int(5),
                SqlValue::Text("a".into()),
            ],
            vec![
                SqlValue::Int(1),
                SqlValue::Int(1),
                SqlValue::Text("b".into()),
            ],
            vec![
                SqlValue::Int(2),
                SqlValue::Int(3),
                SqlValue::Text("c".into()),
            ],
            vec![
                SqlValue::Int(3),
                SqlValue::Int(2),
                SqlValue::Text("d".into()),
            ],
        ];
        let sc = schema();

        let plan = Plan::Project {
            input: Box::new(Plan::Sort {
                input: Box::new(Plan::Filter {
                    input: Box::new(Plan::Scan(rows)),
                    pred: lower_pred(&parse("v >= 2").unwrap(), &sc, regs()).unwrap(),
                }),
                keys: vec![(
                    lower_str("v", &sc),
                    sql_core::SortOptions::with_default(
                        true,
                        None,
                        sql_core::NullsDefault::Postgres,
                        sql_core::TextCollation::Binary,
                    ),
                )],
            }),
            cols: vec![lower_str("upper(name)", &sc), lower_str("v * 2", &sc)],
        };
        let out = plan.execute().expect("execute");
        assert_eq!(
            out,
            vec![
                vec![SqlValue::Text("A".into()), SqlValue::Int(10)],
                vec![SqlValue::Text("C".into()), SqlValue::Int(6)],
                vec![SqlValue::Text("D".into()), SqlValue::Int(4)],
            ]
        );
    }
}
