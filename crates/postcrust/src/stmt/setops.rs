
use super::ast::{SelectStmt, SetOp};
use super::lower::{
    dedup_preserving_order, dedup_preserving_order_typed, rows_eq, rows_eq_typed, run_select,
    QueryResult,
};
use crate::catalog::Catalog;
use crate::expr::lexer::Tok;
use crate::types::PgError;
use sql_core::Row;

fn err(msg: impl Into<String>) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "query",
        input: msg.into(),
    }
}

pub fn has_top_level_setop(toks: &[Tok]) -> bool {
    let mut depth: i32 = 0;
    for t in toks {
        match t {
            Tok::LParen => depth += 1,
            Tok::RParen => depth -= 1,
            Tok::Ident(s) if depth == 0 && SetOp::from_kw(s).is_some() => return true,
            _ => {}
        }
    }
    false
}

pub fn run_query(toks: &[Tok], sql: &str, catalog: &Catalog) -> Result<QueryResult, PgError> {

    if !matches!(toks.first(), Some(Tok::Ident(s)) if s == "select") {
        return Err(err(sql.to_string()));
    }

    let run_arm = |a: &super::ast::SelectStmt| {
        let mut bare = a.clone();
        bare.order_by = Vec::new();
        bare.limit = None;
        bare.offset = 0;
        run_select(&bare, catalog)
    };

    let (first, mut pos) = super::parser::parse_select_at(toks, 0, sql)?;
    let mut result = run_arm(&first)?;

    let mut tail = first;

    loop {

        let op = match toks.get(pos) {
            Some(Tok::Ident(s)) => match SetOp::from_kw(s) {
                Some(op) => op,
                None => return Err(err(sql.to_string())),
            },
            Some(Tok::Eof) | None => break,
            _ => return Err(err(sql.to_string())),
        };
        pos += 1;

        let all = matches!(toks.get(pos), Some(Tok::Ident(s)) if s == "all");
        if all {
            pos += 1;
        }

        if !matches!(toks.get(pos), Some(Tok::Ident(s)) if s == "select") {
            return Err(err(sql.to_string()));
        }
        let (arm, next) = super::parser::parse_select_at(toks, pos, sql)?;
        pos = next;
        let rhs = run_arm(&arm)?;

        if rhs.columns.len() != result.columns.len() {
            return Err(err("each UNION query must have the same number of columns"));
        }

        let rows = combine(
            op,
            all,
            result.rows,
            rhs.rows,
            &result.col_types,
            &catalog.type_registries(),
        );

        result = QueryResult {
            columns: result.columns,
            col_types: result.col_types,
            rows,
        };
        tail = arm;
    }

    if !tail.order_by.is_empty() {
        result.rows = super::lower::order_by_names(result.rows, &tail.order_by, &result.columns)?;
    }
    let off = tail.offset.max(0) as usize;
    result.rows = result.rows.into_iter().skip(off).collect();
    if let Some(n) = tail.limit {
        result.rows.truncate(n.max(0) as usize);
    }

    Ok(result)
}

pub fn run_setop_stmt(first: &SelectStmt, catalog: &Catalog) -> Result<QueryResult, PgError> {
    debug_assert!(
        !first.tail.is_empty(),
        "run_setop_stmt requires a set-op tail"
    );

    let run_arm = |a: &SelectStmt| {
        let mut bare = a.clone();
        bare.order_by = Vec::new();
        bare.limit = None;
        bare.offset = 0;
        bare.tail = Vec::new();
        run_select(&bare, catalog)
    };

    let mut result = run_arm(first)?;

    let mut tail_owner: &SelectStmt = first;

    for arm in &first.tail {
        let rhs = run_arm(&arm.arm)?;
        if rhs.columns.len() != result.columns.len() {
            return Err(err("each UNION query must have the same number of columns"));
        }
        let rows = combine(
            arm.op,
            arm.all,
            result.rows,
            rhs.rows,
            &result.col_types,
            &catalog.type_registries(),
        );

        result = QueryResult {
            columns: result.columns,
            col_types: result.col_types,
            rows,
        };
        tail_owner = &arm.arm;
    }

    if !tail_owner.order_by.is_empty() {
        result.rows =
            super::lower::order_by_names(result.rows, &tail_owner.order_by, &result.columns)?;
    }
    let off = tail_owner.offset.max(0) as usize;
    result.rows = result.rows.into_iter().skip(off).collect();
    if let Some(n) = tail_owner.limit {
        result.rows.truncate(n.max(0) as usize);
    }
    Ok(result)
}

fn combine(
    op: SetOp,
    all: bool,
    left: Vec<Row>,
    right: Vec<Row>,
    col_types: &[u32],
    regs: &crate::types::registry::TypeRegistries,
) -> Vec<Row> {
    match op {
        SetOp::Union => {
            let mut out = left;
            out.extend(right);
            if all {
                out
            } else {
                dedup_preserving_order_typed(out, col_types, regs)
            }
        }
        SetOp::Intersect => {

            let kept: Vec<Row> = left
                .into_iter()
                .filter(|r| right.iter().any(|s| rows_eq_typed(r, s, col_types)))
                .collect();
            dedup_preserving_order_typed(kept, col_types, regs)
        }
        SetOp::Except => {

            let kept: Vec<Row> = left
                .into_iter()
                .filter(|r| !right.iter().any(|s| rows_eq_typed(r, s, col_types)))
                .collect();
            dedup_preserving_order_typed(kept, col_types, regs)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::lexer::lex;
    use sql_core::SqlValue;

    fn i(n: i64) -> SqlValue {
        SqlValue::Int(n)
    }
    fn t(s: &str) -> SqlValue {
        SqlValue::Text(s.into())
    }

    fn catalog() -> Catalog {
        let mut c = Catalog::new();

        c.create(
            "a",
            ["id", "name"],
            vec![vec![i(1), t("x")], vec![i(2), t("y")], vec![i(3), t("z")]],
        );

        c.create(
            "b",
            ["id", "name"],
            vec![vec![i(2), t("y")], vec![i(3), t("z")], vec![i(4), t("w")]],
        );

        c.create("p", ["n"], vec![vec![i(1)], vec![i(2)]]);
        c.create("q", ["n"], vec![vec![i(2)], vec![i(3)]]);
        c
    }

    fn run(sql: &str) -> Result<QueryResult, PgError> {
        let toks = lex(sql).unwrap();
        assert!(has_top_level_setop(&toks), "expected a top-level set-op");
        run_query(&toks, sql, &catalog())
    }

    fn ids(r: &QueryResult) -> Vec<i64> {
        r.rows
            .iter()
            .map(|row| match &row[0] {
                SqlValue::Int(n) => *n,
                v => panic!("not int: {v:?}"),
            })
            .collect()
    }

    #[test]
    fn union_dedups() {

        let r = run("SELECT n FROM p UNION SELECT n FROM q").unwrap();
        assert_eq!(ids(&r), vec![1, 2, 3]);
        assert_eq!(r.columns, vec!["n"]);
    }

    #[test]
    fn union_all_keeps_dups() {

        let r = run("SELECT n FROM p UNION ALL SELECT n FROM q").unwrap();
        assert_eq!(ids(&r), vec![1, 2, 2, 3]);
    }

    #[test]
    fn intersect() {

        let r = run("SELECT n FROM p INTERSECT SELECT n FROM q").unwrap();
        assert_eq!(ids(&r), vec![2]);
    }

    #[test]
    fn except() {

        let r = run("SELECT n FROM p EXCEPT SELECT n FROM q").unwrap();
        assert_eq!(ids(&r), vec![1]);
    }

    #[test]
    fn three_arm_chain() {

        let r = run("SELECT n FROM p UNION SELECT n FROM q UNION ALL SELECT n FROM q").unwrap();

        assert_eq!(ids(&r), vec![1, 2, 3, 2, 3]);
    }

    #[test]
    fn multi_column_union() {

        let r = run("SELECT id, name FROM a UNION SELECT id, name FROM b").unwrap();
        assert_eq!(ids(&r), vec![1, 2, 3, 4]);
        assert_eq!(r.columns, vec!["id", "name"]);
    }

    #[test]
    fn column_count_mismatch_errors() {
        let toks = lex("SELECT id, name FROM a UNION SELECT n FROM p").unwrap();
        let e = run_query(&toks, "…", &catalog()).unwrap_err();
        match e {
            PgError::InvalidInputSyntax { typ, input } => {
                assert_eq!(typ, "query");
                assert!(input.contains("same number of columns"), "{input}");
            }
            other => panic!("wrong error: {other:?}"),
        }
    }

    #[test]
    fn top_level_detection() {

        assert!(has_top_level_setop(
            &lex("SELECT n FROM p UNION SELECT n FROM q").unwrap()
        ));

        assert!(!has_top_level_setop(
            &lex("SELECT * FROM (SELECT n FROM p UNION SELECT n FROM q) s").unwrap()
        ));

        assert!(!has_top_level_setop(&lex("SELECT n FROM p").unwrap()));
    }
}
