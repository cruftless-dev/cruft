
use super::lower::{dedup_preserving_order, rows_eq, run_select, QueryResult};
use super::parser::parse_select_at;
use super::setops::{has_top_level_setop, run_query};
use crate::catalog::Catalog;
use crate::expr::lexer::Tok;
use crate::types::PgError;

const RECURSION_CAP: usize = 10_000;

fn err(msg: impl Into<String>) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "query",
        input: msg.into(),
    }
}

pub fn has_with(toks: &[Tok]) -> bool {
    matches!(toks.first(), Some(Tok::Ident(s)) if s == "with")
}

fn run_toks(toks: &[Tok], sql: &str, catalog: &Catalog) -> Result<QueryResult, PgError> {
    if has_top_level_setop(toks) {
        run_query(toks, sql, catalog)
    } else {
        let (s, _) = parse_select_at(toks, 0, sql)?;
        run_select(&s, catalog)
    }
}

struct CteDef {
    name: String,
    col_aliases: Option<Vec<String>>,
    body: Vec<Tok>,
    union: Option<(usize, bool)>,
}

pub fn run_with(toks: &[Tok], sql: &str, catalog: &Catalog) -> Result<QueryResult, PgError> {
    let mut pos = 1;

    let recursive = matches!(toks.get(pos), Some(Tok::Ident(s)) if s == "recursive");
    if recursive {
        pos += 1;
    }

    let mut defs: Vec<CteDef> = Vec::new();
    loop {

        let name = match toks.get(pos) {
            Some(Tok::Ident(n)) if !is_reserved(n) => {
                let n = n.clone();
                pos += 1;
                n
            }
            _ => return Err(err(sql.to_string())),
        };

        let mut col_aliases: Option<Vec<String>> = None;
        if matches!(toks.get(pos), Some(Tok::LParen)) {
            pos += 1;
            let mut cols = Vec::new();
            loop {
                match toks.get(pos) {
                    Some(Tok::Ident(c)) => {
                        cols.push(c.clone());
                        pos += 1;
                    }
                    _ => return Err(err(sql.to_string())),
                }
                match toks.get(pos) {
                    Some(Tok::Comma) => pos += 1,
                    Some(Tok::RParen) => {
                        pos += 1;
                        break;
                    }
                    _ => return Err(err(sql.to_string())),
                }
            }
            col_aliases = Some(cols);
        }

        if !matches!(toks.get(pos), Some(Tok::Ident(s)) if s == "as") {
            return Err(err(sql.to_string()));
        }
        pos += 1;
        if !matches!(toks.get(pos), Some(Tok::LParen)) {
            return Err(err(sql.to_string()));
        }
        pos += 1;

        let start = pos;
        let mut depth = 1i32;
        while depth > 0 {
            match toks.get(pos) {
                Some(Tok::LParen) => depth += 1,
                Some(Tok::RParen) => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                Some(Tok::Eof) | None => return Err(err(sql.to_string())),
                _ => {}
            }
            pos += 1;
        }
        let body: Vec<Tok> = toks[start..pos].to_vec();
        pos += 1;

        let union = if recursive {
            top_level_union(&body)
        } else {
            None
        };
        defs.push(CteDef {
            name,
            col_aliases,
            body,
            union,
        });

        match toks.get(pos) {
            Some(Tok::Comma) => {
                pos += 1;
                continue;
            }
            _ => break,
        }
    }

    let mut scoped = catalog.clone();

    let cand_names: Vec<String> = defs
        .iter()
        .filter(|d| d.union.is_some())
        .map(|d| d.name.clone())
        .collect();
    let rec_flags: Vec<bool> = defs
        .iter()
        .map(|d| d.union.is_some() && !refs_in(&d.body, &cand_names).is_empty())
        .collect();
    let rec_names: Vec<String> = defs
        .iter()
        .zip(&rec_flags)
        .filter(|(_, &r)| r)
        .map(|(d, _)| d.name.clone())
        .collect();

    for (d, &is_rec) in defs.iter().zip(&rec_flags) {
        if !is_rec && refs_in(&d.body, &rec_names).is_empty() {
            materialize_nonrec(d, sql, &mut scoped)?;
        }
    }

    let group: Vec<&CteDef> = defs
        .iter()
        .zip(&rec_flags)
        .filter(|(_, &r)| r)
        .map(|(d, _)| d)
        .collect();
    if !group.is_empty() {
        for (name, cols, rows) in materialize_recursive_group(&group, sql, &scoped)? {
            scoped.create_query_typed(&name, cols, rows);
        }
    }

    for (d, &is_rec) in defs.iter().zip(&rec_flags) {
        if !is_rec && !refs_in(&d.body, &rec_names).is_empty() {
            materialize_nonrec(d, sql, &mut scoped)?;
        }
    }

    run_toks(&toks[pos..], sql, &scoped)
}

pub fn with_is_data_modifying(toks: &[Tok]) -> bool {
    toks.iter().any(is_dml_kw)
}

fn is_dml_kw(t: &Tok) -> bool {
    matches!(t, Tok::Ident(s) if matches!(s.as_str(), "insert" | "update" | "delete" | "merge"))
}

fn parse_cte_defs(toks: &[Tok], sql: &str) -> Result<(bool, Vec<CteDef>, usize), PgError> {
    let mut pos = 1;
    let recursive = matches!(toks.get(pos), Some(Tok::Ident(s)) if s == "recursive");
    if recursive {
        pos += 1;
    }
    let mut defs: Vec<CteDef> = Vec::new();
    loop {
        let name = match toks.get(pos) {
            Some(Tok::Ident(n)) if !is_reserved(n) => {
                let n = n.clone();
                pos += 1;
                n
            }
            _ => return Err(err(sql.to_string())),
        };
        let mut col_aliases: Option<Vec<String>> = None;
        if matches!(toks.get(pos), Some(Tok::LParen)) {
            pos += 1;
            let mut cols = Vec::new();
            loop {
                match toks.get(pos) {
                    Some(Tok::Ident(c)) => {
                        cols.push(c.clone());
                        pos += 1;
                    }
                    _ => return Err(err(sql.to_string())),
                }
                match toks.get(pos) {
                    Some(Tok::Comma) => pos += 1,
                    Some(Tok::RParen) => {
                        pos += 1;
                        break;
                    }
                    _ => return Err(err(sql.to_string())),
                }
            }
            col_aliases = Some(cols);
        }
        if !matches!(toks.get(pos), Some(Tok::Ident(s)) if s == "as") {
            return Err(err(sql.to_string()));
        }
        pos += 1;
        if !matches!(toks.get(pos), Some(Tok::LParen)) {
            return Err(err(sql.to_string()));
        }
        pos += 1;
        let start = pos;
        let mut depth = 1i32;
        while depth > 0 {
            match toks.get(pos) {
                Some(Tok::LParen) => depth += 1,
                Some(Tok::RParen) => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                Some(Tok::Eof) | None => return Err(err(sql.to_string())),
                _ => {}
            }
            pos += 1;
        }
        let body: Vec<Tok> = toks[start..pos].to_vec();
        pos += 1;
        let union = if recursive {
            top_level_union(&body)
        } else {
            None
        };
        defs.push(CteDef {
            name,
            col_aliases,
            body,
            union,
        });
        match toks.get(pos) {
            Some(Tok::Comma) => {
                pos += 1;
                continue;
            }
            _ => break,
        }
    }
    Ok((recursive, defs, pos))
}

pub fn run_with_mut(
    toks: &[Tok],
    sql: &str,
    catalog: &mut Catalog,
) -> Result<QueryResult, PgError> {
    let (_recursive, defs, pos) = parse_cte_defs(toks, sql)?;
    let main = &toks[pos..];
    if is_dml_body(main) {
        return run_with_dml_main(&defs, main, sql, catalog);
    }

    let mut scoped = catalog.clone();
    for def in &defs {
        if is_dml_body(&def.body) {
            let result = run_dml_body(def, sql, catalog)?;
            register_cte(&mut scoped, def, result)?;
        } else {
            materialize_nonrec(def, sql, &mut scoped)?;
        }
    }
    run_toks(main, sql, &scoped)
}

fn run_with_dml_main(
    defs: &[CteDef],
    main: &[Tok],
    sql: &str,
    catalog: &mut Catalog,
) -> Result<QueryResult, PgError> {
    for def in defs {
        if catalog.get(&def.name).is_some() {
            return Err(err(format!(
                "WITH query name \"{}\" conflicts with an existing table (data-modifying CTE)",
                def.name
            )));
        }
    }
    for def in defs {
        if is_dml_body(&def.body) {
            let result = run_dml_body(def, sql, catalog)?;
            register_cte(catalog, def, result)?;
        } else {
            materialize_nonrec(def, sql, catalog)?;
        }
    }
    let result = crate::stmt::ddl::run(main, sql, catalog);

    for def in defs {
        catalog.drop_table(&def.name);
    }
    result
}

fn run_dml_body(def: &CteDef, sql: &str, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    let mut body = def.body.clone();
    body.push(Tok::Eof);
    crate::stmt::ddl::run(&body, sql, catalog)
}

fn register_cte(target: &mut Catalog, def: &CteDef, result: QueryResult) -> Result<(), PgError> {
    let names = resolve_names(&def.name, def.col_aliases.clone(), &result.columns)?;
    let mut oids = result.col_types.clone();
    oids.resize(names.len(), 0);
    let cols: Vec<(String, u32)> = names.into_iter().zip(oids).collect();
    target.create_query_typed(&def.name, cols, result.rows);
    Ok(())
}

fn is_dml_body(body: &[Tok]) -> bool {
    body.first().is_some_and(is_dml_kw)
}

fn refs_in(body: &[Tok], pool: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in body {
        if let Tok::Ident(s) = t {
            if pool.iter().any(|n| n == s) && !out.iter().any(|n| n == s) {
                out.push(s.clone());
            }
        }
    }
    out
}

fn materialize_nonrec(def: &CteDef, sql: &str, scoped: &mut Catalog) -> Result<(), PgError> {
    let mut with_eof: Vec<Tok> = def.body.clone();
    with_eof.push(Tok::Eof);
    let r = run_toks(&with_eof, sql, scoped)?;
    let names = resolve_names(&def.name, def.col_aliases.clone(), &r.columns)?;
    let mut oids = r.col_types.clone();
    oids.resize(names.len(), 0);
    let cols: Vec<(String, u32)> = names.into_iter().zip(oids).collect();
    scoped.create_query_typed(&def.name, cols, r.rows);
    Ok(())
}

fn resolve_names(
    name: &str,
    col_aliases: Option<Vec<String>>,
    body_cols: &[String],
) -> Result<Vec<String>, PgError> {
    match col_aliases {
        Some(aliases) => {
            if aliases.len() != body_cols.len() {
                return Err(err(format!(
                    "WITH query \"{name}\" has {} columns available but {} columns specified",
                    body_cols.len(),
                    aliases.len()
                )));
            }
            Ok(aliases)
        }
        None => Ok(body_cols.to_vec()),
    }
}

fn top_level_union(body: &[Tok]) -> Option<(usize, bool)> {
    let mut depth = 0i32;
    for (i, t) in body.iter().enumerate() {
        match t {
            Tok::LParen => depth += 1,
            Tok::RParen => depth -= 1,
            Tok::Ident(s) if depth == 0 && s == "union" => {
                let all = matches!(body.get(i + 1), Some(Tok::Ident(a)) if a == "all");
                return Some((i, all));
            }
            _ => {}
        }
    }
    None
}

struct RecState {
    name: String,
    all: bool,
    rec_term: Vec<Tok>,
    cols: Vec<(String, u32)>,

    result: Vec<sql_core::Row>,

    delta: Vec<sql_core::Row>,
}

fn materialize_recursive_group(
    group: &[&CteDef],
    sql: &str,
    scoped: &Catalog,
) -> Result<Vec<(String, Vec<(String, u32)>, Vec<sql_core::Row>)>, PgError> {

    let mut states: Vec<RecState> = Vec::with_capacity(group.len());
    for def in group {
        let (union_at, all) = def.union.expect("recursive CTE has a top-level UNION");
        let mut anchor: Vec<Tok> = def.body[..union_at].to_vec();
        anchor.push(Tok::Eof);
        let rec_start = union_at + 1 + if all { 1 } else { 0 };
        let mut rec_term: Vec<Tok> = def.body[rec_start..].to_vec();
        rec_term.push(Tok::Eof);

        let seed = run_toks(&anchor, sql, scoped)?;
        let names = resolve_names(&def.name, def.col_aliases.clone(), &seed.columns)?;
        let mut oids = seed.col_types.clone();
        oids.resize(names.len(), 0);
        let cols: Vec<(String, u32)> = names.into_iter().zip(oids).collect();

        let result: Vec<sql_core::Row> = if all {
            seed.rows
        } else {
            dedup_preserving_order(seed.rows)
        };
        let delta = result.clone();
        states.push(RecState {
            name: def.name.clone(),
            all,
            rec_term,
            cols,
            result,
            delta,
        });
    }

    let mut iters = 0usize;
    loop {
        if states.iter().all(|s| s.delta.is_empty()) {
            break;
        }
        iters += 1;
        if iters > RECURSION_CAP {
            let names = states
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
                .join("\", \"");
            return Err(err(format!(
                "recursive query \"{names}\" exceeded the iteration limit ({RECURSION_CAP})"
            )));
        }

        let mut step_scope = scoped.clone();
        for s in &states {
            step_scope.create_query_typed(&s.name, s.cols.clone(), s.delta.clone());
        }

        let mut next: Vec<Vec<sql_core::Row>> = Vec::with_capacity(states.len());
        for s in &states {
            let step = run_toks(&s.rec_term, sql, &step_scope)?;
            if step.columns.len() != s.cols.len() {
                return Err(err(
                    "each UNION query must have the same number of columns".to_string()
                ));
            }
            if s.all {
                next.push(step.rows);
            } else {

                let mut fresh: Vec<sql_core::Row> = Vec::new();
                for r in step.rows {
                    if !s.result.iter().any(|x| rows_eq(x, &r))
                        && !fresh.iter().any(|x| rows_eq(x, &r))
                    {
                        fresh.push(r);
                    }
                }
                next.push(fresh);
            }
        }

        for (s, new_rows) in states.iter_mut().zip(next) {
            s.result.extend(new_rows.clone());
            s.delta = new_rows;
        }
    }

    Ok(states
        .into_iter()
        .map(|s| (s.name, s.cols, s.result))
        .collect())
}

fn is_reserved(s: &str) -> bool {
    matches!(s, "select" | "with" | "as" | "recursive" | "from" | "where")
}

#[cfg(test)]
mod tests {
    use crate::catalog::Catalog;
    use crate::stmt::run;
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
            "emp",
            ["id", "name", "dept_id"],
            vec![
                vec![i(1), t("alice"), i(10)],
                vec![i(2), t("bob"), i(20)],
                vec![i(3), t("carol"), i(10)],
                vec![i(4), t("dave"), i(99)],
            ],
        );
        c.create(
            "dept",
            ["did", "dname"],
            vec![vec![i(10), t("eng")], vec![i(20), t("sales")]],
        );
        c
    }

    fn ok(sql: &str) -> crate::stmt::QueryResult {
        run(sql, &catalog()).expect("query ok")
    }

    #[test]
    fn single_cte_in_from() {
        let r = ok("WITH e AS (SELECT id, name FROM emp WHERE dept_id = 10) \
                    SELECT name FROM e ORDER BY id");
        assert_eq!(r.columns, vec!["name"]);
        assert_eq!(r.rows, vec![vec![t("alice")], vec![t("carol")]]);
    }

    #[test]
    fn cte_references_earlier_cte() {
        let r = ok("WITH a AS (SELECT id, dept_id FROM emp), \
                         b AS (SELECT id FROM a WHERE dept_id = 10) \
                    SELECT id FROM b ORDER BY id");
        assert_eq!(r.rows, vec![vec![i(1)], vec![i(3)]]);
    }

    #[test]
    fn cte_referenced_multiple_times_via_join() {

        let r = ok("WITH e AS (SELECT id, name, dept_id FROM emp) \
                    SELECT x.name, y.name FROM e x JOIN e y ON x.dept_id = y.dept_id \
                    WHERE x.id = 1 ORDER BY y.id");
        assert_eq!(
            r.rows,
            vec![vec![t("alice"), t("alice")], vec![t("alice"), t("carol")]]
        );
    }

    #[test]
    fn cte_used_in_subquery() {
        let r = ok("WITH tens AS (SELECT id FROM emp WHERE dept_id = 10) \
                    SELECT name FROM emp WHERE id IN (SELECT id FROM tens) ORDER BY id");
        assert_eq!(r.rows, vec![vec![t("alice")], vec![t("carol")]]);
    }

    #[test]
    fn cte_join_two_ctes() {
        let r = ok("WITH e AS (SELECT id, name, dept_id FROM emp), \
                         d AS (SELECT did, dname FROM dept) \
                    SELECT e.name, d.dname FROM e JOIN d ON e.dept_id = d.did ORDER BY e.id");
        assert_eq!(
            r.rows,
            vec![
                vec![t("alice"), t("eng")],
                vec![t("bob"), t("sales")],
                vec![t("carol"), t("eng")],
            ]
        );
    }

    #[test]
    fn cte_column_aliases() {
        let r =
            ok("WITH e(k, who) AS (SELECT id, name FROM emp WHERE id = 1) SELECT k, who FROM e");
        assert_eq!(r.columns, vec!["k", "who"]);
        assert_eq!(r.rows, vec![vec![i(1), t("alice")]]);
    }

    #[test]
    fn cte_body_is_setop() {
        let r = ok("WITH u AS (SELECT id FROM emp WHERE id = 1 \
                                UNION SELECT id FROM emp WHERE id = 2) \
                    SELECT id FROM u ORDER BY id");
        assert_eq!(r.rows, vec![vec![i(1)], vec![i(2)]]);
    }

    #[test]
    fn cte_shadows_base_table() {

        let r = ok("WITH emp AS (SELECT 99 AS id) SELECT id FROM emp");
        assert_eq!(r.rows, vec![vec![i(99)]]);
    }

    #[test]
    fn recursive_counting_series() {
        let r = ok(
            "WITH RECURSIVE t(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM t WHERE n < 5) \
             SELECT n FROM t ORDER BY n",
        );
        assert_eq!(r.columns, vec!["n"]);
        assert_eq!(
            r.rows,
            vec![vec![i(1)], vec![i(2)], vec![i(3)], vec![i(4)], vec![i(5)]]
        );
    }

    #[test]
    fn recursive_union_distinct_terminates() {

        let r = ok(
            "WITH RECURSIVE t(n) AS (SELECT 1 UNION SELECT n+1 FROM t WHERE n < 3) \
             SELECT n FROM t ORDER BY n",
        );
        assert_eq!(r.rows, vec![vec![i(1)], vec![i(2)], vec![i(3)]]);
    }

    #[test]
    fn recursive_runaway_hits_cap() {

        let e = run(
            "WITH RECURSIVE t(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM t) SELECT n FROM t",
            &catalog(),
        )
        .unwrap_err();
        assert!(e.message().contains("iteration limit"), "{}", e.message());
    }

    #[test]
    fn recursive_still_allows_nonrecursive_body() {

        let r = ok("WITH RECURSIVE t(x) AS (SELECT 1) SELECT x FROM t");
        assert_eq!(r.rows, vec![vec![i(1)]]);
    }

    #[test]
    fn mutual_recursion_even_odd() {

        let r = ok("WITH RECURSIVE \
               evens(n) AS (SELECT 0 UNION SELECT n+1 FROM odds WHERE n < 10), \
               odds(n)  AS (SELECT 1 UNION SELECT n+1 FROM evens WHERE n < 10) \
             SELECT n FROM evens ORDER BY n");
        assert_eq!(
            r.rows,
            vec![
                vec![i(0)],
                vec![i(2)],
                vec![i(4)],
                vec![i(6)],
                vec![i(8)],
                vec![i(10)],
            ]
        );
    }

    #[test]
    fn mutual_recursion_odd_side() {
        let r = ok("WITH RECURSIVE \
               evens(n) AS (SELECT 0 UNION SELECT n+1 FROM odds WHERE n < 10), \
               odds(n)  AS (SELECT 1 UNION SELECT n+1 FROM evens WHERE n < 10) \
             SELECT n FROM odds ORDER BY n");
        assert_eq!(
            r.rows,
            vec![vec![i(1)], vec![i(3)], vec![i(5)], vec![i(7)], vec![i(9)],]
        );
    }

    #[test]
    fn nonrecursive_cte_feeds_recursive() {

        let r = ok("WITH RECURSIVE \
               seed(x) AS (SELECT 2), \
               r(n) AS (SELECT x FROM seed UNION ALL SELECT n+1 FROM r WHERE n < 5) \
             SELECT n FROM r ORDER BY n");
        assert_eq!(r.rows, vec![vec![i(2)], vec![i(3)], vec![i(4)], vec![i(5)]]);
    }

    #[test]
    fn recursive_consumed_by_later_nonrecursive() {

        let r = ok("WITH RECURSIVE \
               r(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM r WHERE n < 6), \
               evens_only(n) AS (SELECT n FROM r WHERE n % 2 = 0) \
             SELECT n FROM evens_only ORDER BY n");
        assert_eq!(r.rows, vec![vec![i(2)], vec![i(4)], vec![i(6)]]);
    }

    #[test]
    fn out_of_scope_cte_name_errors() {

        assert!(run("WITH a AS (SELECT 1 AS x) SELECT * FROM b", &catalog()).is_err());
    }
}
