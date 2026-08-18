
use super::ast::{
    FromItem, LockClause, LockStrength, LockWait, OrderKey, SelectItem, SelectStmt, Stmt,
};
use crate::catalog::{
    Catalog, Cursor, FunctionDef, IsolationLevel, LockMode, PreparedStmt, RetShape,
};
use crate::expr::ast::{BinOp, Expr};
use crate::expr::bind::{lower, lower_pred, Schema};
use crate::expr::eval::EvalCtx;
use crate::types::registry::TypeRegistries;
use crate::types::PgError;
use sql_core::{NullsDefault, Plan, Row, Scalar, SortOptions, SqlValue, TextCollation};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub col_types: Vec<u32>,
    pub rows: Vec<Row>,
}

pub fn run(sql: &str, catalog: &Catalog) -> Result<QueryResult, PgError> {
    let toks = crate::expr::lexer::lex(sql)?;
    if super::cte::has_with(&toks) {
        return super::cte::run_with(&toks, sql, catalog);
    }
    if super::setops::has_top_level_setop(&toks) {
        return super::setops::run_query(&toks, sql, catalog);
    }
    match super::parser::parse(sql)? {
        Stmt::Select(s) => run_select(&s, catalog),
    }
}

pub fn run_mut(sql: &str, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    let toks = crate::expr::lexer::lex(sql)?;

    let kw = match toks.first() {
        Some(crate::expr::lexer::Tok::Ident(k)) => Some(k.as_str()),
        _ => None,
    };

    if catalog.is_aborted() {
        match kw {
            Some("commit") | Some("end") => {
                catalog.txn_rollback();
                return Ok(empty_result());
            }
            Some("rollback") | Some("abort") | Some("release") => {

            }
            _ => return Err(PgError::TransactionAborted),
        }
    }

    let in_txn = catalog.in_transaction();

    if !is_txn_control_kw(kw) && kw != Some("set") {
        catalog.stmt_snapshot_hook();
    }

    let stmt_snap = if is_mutating_kw(kw) {
        Some(catalog.stmt_snapshot())
    } else {
        None
    };

    let autocommit_write = is_mutating_kw(kw) && !in_txn && !catalog.in_autocommit_stmt();
    if autocommit_write {
        catalog.stmt_write_begin();
    }

    let result = run_mut_dispatch(sql, &toks, kw, catalog);

    if result.is_err() {
        if let Some(snap) = &stmt_snap {
            catalog.stmt_restore(snap);
        }
    }
    if autocommit_write {
        catalog.stmt_write_end(result.is_ok());
    }

    if result.is_err() && in_txn && !is_txn_control_kw(kw) {
        catalog.set_aborted();
    }
    result
}

fn is_mutating_kw(kw: Option<&str>) -> bool {
    matches!(
        kw,
        Some("create")
            | Some("insert")
            | Some("update")
            | Some("delete")
            | Some("drop")
            | Some("merge")
            | Some("alter")
            | Some("refresh")
    )
}

fn is_txn_control_kw(kw: Option<&str>) -> bool {
    matches!(
        kw,
        Some("begin")
            | Some("start")
            | Some("commit")
            | Some("end")
            | Some("rollback")
            | Some("abort")
            | Some("savepoint")
            | Some("release")
    )
}

fn run_mut_dispatch(
    sql: &str,
    toks: &[crate::expr::lexer::Tok],
    _kw: Option<&str>,
    catalog: &mut Catalog,
) -> Result<QueryResult, PgError> {

    if super::cte::has_with(toks) && super::cte::with_is_data_modifying(toks) {
        return super::cte::run_with_mut(toks, sql, catalog);
    }
    match toks.first() {

        Some(crate::expr::lexer::Tok::Ident(kw))
            if matches!(
                kw.as_str(),
                "begin"
                    | "start"
                    | "commit"
                    | "end"
                    | "rollback"
                    | "abort"
                    | "savepoint"
                    | "release"
            ) =>
        {
            run_txn(&toks, kw.as_str(), catalog)
        }

        Some(crate::expr::lexer::Tok::Ident(kw)) if kw == "explain" => {
            super::explain::run(sql, catalog)
        }

        Some(crate::expr::lexer::Tok::Ident(kw)) if kw == "prepare" => run_prepare(sql, catalog),
        Some(crate::expr::lexer::Tok::Ident(kw)) if kw == "execute" => run_execute(sql, catalog),
        Some(crate::expr::lexer::Tok::Ident(kw)) if kw == "deallocate" => {
            run_deallocate(sql, catalog)
        }

        Some(crate::expr::lexer::Tok::Ident(kw)) if kw == "declare" => run_declare(sql, catalog),
        Some(crate::expr::lexer::Tok::Ident(kw)) if kw == "fetch" => {
            run_fetch_move(sql, catalog, false)
        }
        Some(crate::expr::lexer::Tok::Ident(kw)) if kw == "move" => {
            run_fetch_move(sql, catalog, true)
        }
        Some(crate::expr::lexer::Tok::Ident(kw)) if kw == "close" => run_close(sql, catalog),

        Some(crate::expr::lexer::Tok::Ident(kw))
            if kw == "set"
                && matches!(toks.get(1), Some(crate::expr::lexer::Tok::Ident(k)) if k == "constraints") =>
        {
            run_set_constraints(toks, catalog)
        }

        Some(crate::expr::lexer::Tok::Ident(kw))
            if kw == "set"
                && matches!(toks.get(1), Some(crate::expr::lexer::Tok::Ident(k)) if k == "transaction") =>
        {
            run_set_transaction(&toks, catalog)
        }

        Some(crate::expr::lexer::Tok::Ident(kw))
            if kw == "set"
                && matches!(toks.get(1), Some(crate::expr::lexer::Tok::Ident(k)) if k == "session")
                && matches!(toks.get(2), Some(crate::expr::lexer::Tok::Ident(k)) if k == "characteristics") =>
        {
            run_set_session_characteristics(&toks, catalog)
        }
        Some(crate::expr::lexer::Tok::Ident(kw))
            if matches!(
                kw.as_str(),
                "create"
                    | "insert"
                    | "update"
                    | "delete"
                    | "drop"
                    | "merge"
                    | "refresh"
                    | "alter"
                    | "comment"
            ) =>
        {
            super::ddl::run(&toks, sql, catalog)
        }

        _ if lock_clause_strength(&toks).is_some() => {
            let strength = lock_clause_strength(&toks).expect("just checked");
            if super::setops::has_top_level_setop(&toks) {
                return Err(exec_err(format!(
                    "{} is not allowed with UNION/INTERSECT/EXCEPT",
                    strength.as_str()
                )));
            }
            if super::cte::has_with(&toks) {

                return run(sql, catalog);
            }
            run_select_locking(sql, catalog)
        }

        Some(crate::expr::lexer::Tok::Ident(kw)) if kw == "analyze" => {
            super::analyze::run(&toks, catalog)
        }
        _ if super::txid::mentions_txid_builtin(sql)
            || super::seq::mentions_seq_builtin(sql)
            || super::descr::mentions_description_builtin(sql) =>
        {
            run_select_stateful(sql, catalog)
        }

        _ if catalog.any_plpgsql_function() && stmt_calls_plpgsql(toks, catalog) => {
            run_query_plpgsql(sql, catalog)
        }
        _ => run(sql, catalog),
    }
}

fn pp_skip_ws(cs: &[char], mut i: usize) -> usize {
    while i < cs.len() {
        if cs[i].is_whitespace() {
            i += 1;
        } else if cs[i] == '-' && i + 1 < cs.len() && cs[i + 1] == '-' {
            while i < cs.len() && cs[i] != '\n' {
                i += 1;
            }
        } else {
            break;
        }
    }
    i
}

fn pp_match_kw(cs: &[char], i: usize, kw: &str) -> Option<usize> {
    let kwc: Vec<char> = kw.chars().collect();
    if i + kwc.len() > cs.len() {
        return None;
    }
    for (k, &kc) in kwc.iter().enumerate() {
        if cs[i + k].to_ascii_lowercase() != kc {
            return None;
        }
    }
    let j = i + kwc.len();
    if j < cs.len() && (cs[j].is_alphanumeric() || cs[j] == '_' || cs[j] == '$') {
        return None;
    }
    Some(j)
}

fn pp_parse_ident(cs: &[char], i: usize) -> Result<(String, usize), PgError> {
    let i = pp_skip_ws(cs, i);
    if i < cs.len() && cs[i] == '"' {
        let mut j = i + 1;
        let mut s = String::new();
        while j < cs.len() {
            if cs[j] == '"' {
                if j + 1 < cs.len() && cs[j + 1] == '"' {
                    s.push('"');
                    j += 2;
                    continue;
                }
                return Ok((s, j + 1));
            }
            s.push(cs[j]);
            j += 1;
        }
        return Err(exec_err("unterminated quoted identifier".to_string()));
    }
    let start = i;
    let mut j = i;
    while j < cs.len() && (cs[j].is_alphanumeric() || cs[j] == '_' || cs[j] == '$') {
        j += 1;
    }
    if j == start {
        return Err(exec_err("syntax error: expected identifier".to_string()));
    }
    let s: String = cs[start..j].iter().collect::<String>().to_ascii_lowercase();
    Ok((s, j))
}

fn pp_parse_paren_list(cs: &[char], i: usize) -> Option<(Vec<String>, usize)> {
    let i = pp_skip_ws(cs, i);
    if i >= cs.len() || cs[i] != '(' {
        return None;
    }
    let mut depth = 0usize;
    let mut j = i;
    let mut items: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_str = false;
    while j < cs.len() {
        let c = cs[j];
        if in_str {
            cur.push(c);
            if c == '\'' {
                if j + 1 < cs.len() && cs[j + 1] == '\'' {
                    cur.push('\'');
                    j += 2;
                    continue;
                }
                in_str = false;
            }
            j += 1;
            continue;
        }
        match c {
            '\'' => {
                in_str = true;
                cur.push(c);
            }
            '(' => {
                depth += 1;
                if depth > 1 {
                    cur.push(c);
                }
            }
            ')' => {
                depth -= 1;
                if depth == 0 {
                    if !cur.trim().is_empty() || !items.is_empty() {
                        items.push(cur.trim().to_string());
                    }
                    return Some((items, j + 1));
                }
                cur.push(c);
            }
            ',' if depth == 1 => {
                items.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
        j += 1;
    }
    None
}

fn pp_max_param(inner: &str) -> usize {
    let cs: Vec<char> = inner.chars().collect();
    let mut i = 0;
    let mut max = 0usize;
    let mut in_str = false;
    while i < cs.len() {
        let c = cs[i];
        if in_str {
            if c == '\'' {
                if i + 1 < cs.len() && cs[i + 1] == '\'' {
                    i += 2;
                    continue;
                }
                in_str = false;
            }
            i += 1;
            continue;
        }
        if c == '\'' {
            in_str = true;
            i += 1;
            continue;
        }
        if c == '$' && i + 1 < cs.len() && cs[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            while j < cs.len() && cs[j].is_ascii_digit() {
                j += 1;
            }
            let n: usize = cs[i + 1..j].iter().collect::<String>().parse().unwrap_or(0);
            if n > max {
                max = n;
            }
            i = j;
            continue;
        }
        i += 1;
    }
    max
}

fn pp_substitute(inner: &str, repls: &[String]) -> String {
    let cs: Vec<char> = inner.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut in_str = false;
    let mut dollar_tag: Option<String> = None;
    while i < cs.len() {
        let c = cs[i];
        if let Some(tag) = &dollar_tag {
            let tc: Vec<char> = tag.chars().collect();
            if cs[i..].starts_with(tc.as_slice()) {
                out.push_str(tag);
                i += tc.len();
                dollar_tag = None;
                continue;
            }
            out.push(c);
            i += 1;
            continue;
        }
        if in_str {
            out.push(c);
            if c == '\'' {
                if i + 1 < cs.len() && cs[i + 1] == '\'' {
                    out.push('\'');
                    i += 2;
                    continue;
                }
                in_str = false;
            }
            i += 1;
            continue;
        }
        if c == '\'' {
            in_str = true;
            out.push(c);
            i += 1;
            continue;
        }
        if c == '"' {
            out.push(c);
            i += 1;
            while i < cs.len() {
                out.push(cs[i]);
                let q = cs[i] == '"';
                i += 1;
                if q {
                    break;
                }
            }
            continue;
        }
        if c == '$' {
            if i + 1 < cs.len() && cs[i + 1].is_ascii_digit() {
                let mut j = i + 1;
                while j < cs.len() && cs[j].is_ascii_digit() {
                    j += 1;
                }
                let num: usize = cs[i + 1..j].iter().collect::<String>().parse().unwrap_or(0);
                if num >= 1 && num <= repls.len() {
                    out.push_str(&repls[num - 1]);
                } else {
                    out.push_str(&cs[i..j].iter().collect::<String>());
                }
                i = j;
                continue;
            }

            let mut j = i + 1;
            while j < cs.len() && (cs[j].is_alphanumeric() || cs[j] == '_') {
                j += 1;
            }
            if j < cs.len() && cs[j] == '$' {
                let tag: String = cs[i..=j].iter().collect();
                out.push_str(&tag);
                dollar_tag = Some(tag);
                i = j + 1;
                continue;
            }
            out.push(c);
            i += 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

fn run_prepare(sql: &str, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    let cs: Vec<char> = sql.chars().collect();
    let i = pp_skip_ws(&cs, 0);
    let i = pp_match_kw(&cs, i, "prepare")
        .ok_or_else(|| exec_err("syntax error at or near \"PREPARE\"".to_string()))?;
    let (name, i) = pp_parse_ident(&cs, i)?;
    let (param_types, i) = match pp_parse_paren_list(&cs, i) {
        Some((types, ni)) => (types, ni),
        None => (Vec::new(), i),
    };
    let i = pp_skip_ws(&cs, i);
    let i = pp_match_kw(&cs, i, "as")
        .ok_or_else(|| exec_err("syntax error in PREPARE: expected AS".to_string()))?;
    let inner_sql: String = cs[i..]
        .iter()
        .collect::<String>()
        .trim()
        .trim_end_matches(';')
        .trim()
        .to_string();
    if inner_sql.is_empty() {
        return Err(exec_err(
            "syntax error in PREPARE: missing statement".to_string(),
        ));
    }
    let param_count = if param_types.is_empty() {
        pp_max_param(&inner_sql)
    } else {
        param_types.len()
    };
    catalog.prepare_stmt(
        &name,
        PreparedStmt {
            param_types,
            param_count,
            inner_sql,
        },
    )?;
    Ok(empty_result())
}

fn run_execute(sql: &str, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    let cs: Vec<char> = sql.chars().collect();
    let i = pp_skip_ws(&cs, 0);
    let i = pp_match_kw(&cs, i, "execute")
        .ok_or_else(|| exec_err("syntax error at or near \"EXECUTE\"".to_string()))?;
    let (name, i) = pp_parse_ident(&cs, i)?;
    let args = pp_parse_paren_list(&cs, i)
        .map(|(a, _)| a)
        .unwrap_or_default();

    let prep = catalog
        .get_prepared(&name)
        .ok_or_else(|| exec_err(format!("prepared statement \"{name}\" does not exist")))?;

    if args.len() != prep.param_count {
        return Err(exec_err(format!(
            "wrong number of parameters for prepared statement \"{name}\""
        )));
    }

    let repls: Vec<String> = args
        .iter()
        .enumerate()
        .map(|(k, arg)| match prep.param_types.get(k) {
            Some(ty) if !ty.is_empty() => format!("(({arg})::{ty})"),
            _ => format!("({arg})"),
        })
        .collect();

    let substituted = pp_substitute(&prep.inner_sql, &repls);
    run_mut(&substituted, catalog)
}

fn run_deallocate(sql: &str, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    let cs: Vec<char> = sql.chars().collect();
    let i = pp_skip_ws(&cs, 0);
    let i = pp_match_kw(&cs, i, "deallocate")
        .ok_or_else(|| exec_err("syntax error at or near \"DEALLOCATE\"".to_string()))?;

    let i = pp_match_kw(&cs, pp_skip_ws(&cs, i), "prepare").unwrap_or(i);

    let after_ws = pp_skip_ws(&cs, i);
    if let Some(j) = pp_match_kw(&cs, after_ws, "all") {
        let _ = j;
        catalog.deallocate_all();
        return Ok(empty_result());
    }
    let (name, _) = pp_parse_ident(&cs, i)?;
    catalog.deallocate(&name)?;
    Ok(empty_result())
}

#[derive(Debug, Clone, Copy)]
enum FetchDir {
    Forward(usize),
    Backward(usize),
    Absolute(i64),
    Relative(i64),
}

fn pp_peek_word(cs: &[char], i: usize) -> Option<(String, usize)> {
    let i = pp_skip_ws(cs, i);
    let start = i;
    let mut j = i;
    while j < cs.len() && (cs[j].is_alphanumeric() || cs[j] == '_') {
        j += 1;
    }
    if j == start {
        return None;
    }
    Some((
        cs[start..j].iter().collect::<String>().to_ascii_lowercase(),
        j,
    ))
}

fn pp_parse_signed_int(cs: &[char], i: usize) -> Option<(i64, usize)> {
    let i = pp_skip_ws(cs, i);
    let mut j = i;
    let mut neg = false;
    if j < cs.len() && (cs[j] == '-' || cs[j] == '+') {
        neg = cs[j] == '-';
        j = pp_skip_ws(cs, j + 1);
    }
    let start = j;
    while j < cs.len() && cs[j].is_ascii_digit() {
        j += 1;
    }
    if j == start {
        return None;
    }
    let n: i64 = cs[start..j].iter().collect::<String>().parse().ok()?;
    Some((if neg { -n } else { n }, j))
}

fn pp_parse_fb(cs: &[char], i: usize, backward: bool) -> (FetchDir, usize) {
    if let Some((w, nj)) = pp_peek_word(cs, i) {
        if w == "all" {
            let d = if backward {
                FetchDir::Backward(usize::MAX)
            } else {
                FetchDir::Forward(usize::MAX)
            };
            return (d, nj);
        }
    }
    if let Some((n, nj)) = pp_parse_signed_int(cs, i) {
        let mag = n.unsigned_abs() as usize;
        let go_back = backward ^ (n < 0);
        let d = if go_back {
            FetchDir::Backward(mag)
        } else {
            FetchDir::Forward(mag)
        };
        return (d, nj);
    }
    let d = if backward {
        FetchDir::Backward(1)
    } else {
        FetchDir::Forward(1)
    };
    (d, i)
}

fn pp_parse_fetch_dir(cs: &[char], i: usize) -> (FetchDir, usize) {
    let i0 = pp_skip_ws(cs, i);

    if i0 < cs.len() && (cs[i0] == '-' || cs[i0] == '+' || cs[i0].is_ascii_digit()) {
        if let Some((n, nj)) = pp_parse_signed_int(cs, i0) {
            let d = if n >= 0 {
                FetchDir::Forward(n as usize)
            } else {
                FetchDir::Backward((-n) as usize)
            };
            return (d, nj);
        }
    }
    let Some((w, ni)) = pp_peek_word(cs, i0) else {
        return (FetchDir::Forward(1), i0);
    };
    match w.as_str() {
        "next" => (FetchDir::Forward(1), ni),
        "prior" => (FetchDir::Backward(1), ni),
        "first" => (FetchDir::Absolute(1), ni),
        "last" => (FetchDir::Absolute(-1), ni),
        "all" => (FetchDir::Forward(usize::MAX), ni),
        "absolute" => match pp_parse_signed_int(cs, ni) {
            Some((n, nj)) => (FetchDir::Absolute(n), nj),
            None => (FetchDir::Absolute(1), ni),
        },
        "relative" => match pp_parse_signed_int(cs, ni) {
            Some((n, nj)) => (FetchDir::Relative(n), nj),
            None => (FetchDir::Relative(0), ni),
        },
        "forward" => pp_parse_fb(cs, ni, false),
        "backward" => pp_parse_fb(cs, ni, true),

        _ => (FetchDir::Forward(1), i0),
    }
}

fn cursor_scroll_err() -> PgError {
    PgError::InvalidInputSyntax {
        typ: "query",
        input: "cursor can only scan forward".to_string(),
    }
}

fn cursor_fetch(cur: &mut Cursor, dir: &FetchDir) -> Result<Vec<Row>, PgError> {
    let len = cur.rows.len();
    let pos = cur.pos;
    let (rows, new_pos): (Vec<Row>, usize) = match *dir {
        FetchDir::Forward(k) => {
            let mut out = Vec::new();
            let mut p = pos;
            let mut s = 0usize;
            while s < k && p < len {
                p += 1;
                out.push(cur.rows[p - 1].clone());
                s += 1;
            }
            (out, p)
        }
        FetchDir::Backward(k) => {
            if k > 0 && !cur.scroll {
                return Err(cursor_scroll_err());
            }
            let mut out = Vec::new();
            let mut p = pos;
            let mut s = 0usize;
            while s < k && p > 0 {
                p -= 1;
                if p >= 1 {
                    out.push(cur.rows[p - 1].clone());
                }
                s += 1;
            }
            (out, p)
        }
        FetchDir::Absolute(n) => {
            let target: i64 = if n < 0 { len as i64 + n + 1 } else { n };
            if target < pos as i64 && !cur.scroll {
                return Err(cursor_scroll_err());
            }
            if target >= 1 && target <= len as i64 {
                (
                    vec![cur.rows[(target - 1) as usize].clone()],
                    target as usize,
                )
            } else if target <= 0 {
                (Vec::new(), 0)
            } else {
                (Vec::new(), len)
            }
        }
        FetchDir::Relative(n) => {
            if n < 0 && !cur.scroll {
                return Err(cursor_scroll_err());
            }
            let target: i64 = pos as i64 + n;
            if target >= 1 && target <= len as i64 {
                (
                    vec![cur.rows[(target - 1) as usize].clone()],
                    target as usize,
                )
            } else if target <= 0 {
                (Vec::new(), 0)
            } else {
                (Vec::new(), len)
            }
        }
    };
    cur.pos = new_pos;
    Ok(rows)
}

fn run_declare(sql: &str, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    let cs: Vec<char> = sql.chars().collect();
    let i = pp_skip_ws(&cs, 0);
    let i = pp_match_kw(&cs, i, "declare")
        .ok_or_else(|| exec_err("syntax error at or near \"DECLARE\"".to_string()))?;
    let (name, i) = pp_parse_ident(&cs, i)?;
    let mut j = pp_skip_ws(&cs, i);
    if let Some(nj) = pp_match_kw(&cs, j, "binary") {
        j = pp_skip_ws(&cs, nj);
    }
    if let Some(nj) =
        pp_match_kw(&cs, j, "insensitive").or_else(|| pp_match_kw(&cs, j, "asensitive"))
    {
        j = pp_skip_ws(&cs, nj);
    }
    let mut scroll = false;
    if let Some(nj) = pp_match_kw(&cs, j, "no") {
        let nj2 = pp_skip_ws(&cs, nj);
        if let Some(nj3) = pp_match_kw(&cs, nj2, "scroll") {
            scroll = false;
            j = pp_skip_ws(&cs, nj3);
        }
    } else if let Some(nj) = pp_match_kw(&cs, j, "scroll") {
        scroll = true;
        j = pp_skip_ws(&cs, nj);
    }
    let j = pp_match_kw(&cs, j, "cursor")
        .ok_or_else(|| exec_err("syntax error in DECLARE: expected CURSOR".to_string()))?;
    let mut j = pp_skip_ws(&cs, j);
    if let Some(nj) = pp_match_kw(&cs, j, "with").or_else(|| pp_match_kw(&cs, j, "without")) {
        let nj2 = pp_skip_ws(&cs, nj);
        j = pp_match_kw(&cs, nj2, "hold")
            .map(|x| pp_skip_ws(&cs, x))
            .unwrap_or(nj2);
    }
    let j = pp_match_kw(&cs, j, "for")
        .ok_or_else(|| exec_err("syntax error in DECLARE: expected FOR".to_string()))?;
    let inner_sql: String = cs[j..]
        .iter()
        .collect::<String>()
        .trim()
        .trim_end_matches(';')
        .trim()
        .to_string();
    if inner_sql.is_empty() {
        return Err(exec_err(
            "syntax error in DECLARE: missing query".to_string(),
        ));
    }

    let result = run(&inner_sql, catalog)?;
    let cursor = Cursor {
        cols: result.columns,
        oids: result.col_types,
        rows: result.rows,
        pos: 0,
        scroll,
    };
    catalog.declare_cursor(&name, cursor)?;
    Ok(empty_result())
}

fn run_fetch_move(sql: &str, catalog: &mut Catalog, is_move: bool) -> Result<QueryResult, PgError> {
    let cs: Vec<char> = sql.chars().collect();
    let kw = if is_move { "move" } else { "fetch" };
    let i = pp_skip_ws(&cs, 0);
    let i = pp_match_kw(&cs, i, kw)
        .ok_or_else(|| exec_err(format!("syntax error at or near \"{}\"", kw.to_uppercase())))?;
    let (dir, i) = pp_parse_fetch_dir(&cs, i);
    let i = pp_skip_ws(&cs, i);
    let i = pp_match_kw(&cs, i, "from")
        .or_else(|| pp_match_kw(&cs, i, "in"))
        .unwrap_or(i);
    let (name, _) = pp_parse_ident(&cs, i)?;
    let cur = catalog
        .get_cursor_mut(&name)
        .ok_or_else(|| PgError::InvalidInputSyntax {
            typ: "query",
            input: format!("cursor \"{name}\" does not exist"),
        })?;
    let cols = cur.cols.clone();
    let oids = cur.oids.clone();
    let rows = cursor_fetch(cur, &dir)?;
    if is_move {
        Ok(QueryResult {
            columns: cols,
            col_types: oids,
            rows: Vec::new(),
        })
    } else {
        Ok(QueryResult {
            columns: cols,
            col_types: oids,
            rows,
        })
    }
}

fn run_close(sql: &str, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    let cs: Vec<char> = sql.chars().collect();
    let i = pp_skip_ws(&cs, 0);
    let i = pp_match_kw(&cs, i, "close")
        .ok_or_else(|| exec_err("syntax error at or near \"CLOSE\"".to_string()))?;
    let after = pp_skip_ws(&cs, i);
    if pp_match_kw(&cs, after, "all").is_some() {
        catalog.close_all_cursors();
        return Ok(empty_result());
    }
    let (name, _) = pp_parse_ident(&cs, i)?;
    catalog.close_cursor(&name)?;
    Ok(empty_result())
}

fn lock_clause_strength(toks: &[crate::expr::lexer::Tok]) -> Option<LockStrength> {
    use crate::expr::lexer::Tok;
    let ident_at = |i: usize| -> Option<&str> {
        match toks.get(i) {
            Some(Tok::Ident(s)) => Some(s.as_str()),
            _ => None,
        }
    };
    for i in 0..toks.len() {
        if ident_at(i) != Some("for") {
            continue;
        }
        return match ident_at(i + 1) {
            Some("update") => Some(LockStrength::Update),
            Some("no") => Some(LockStrength::NoKeyUpdate),
            Some("share") => Some(LockStrength::Share),
            Some("key") => Some(LockStrength::KeyShare),
            _ => continue,
        };
    }
    None
}

fn run_select_locking(sql: &str, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    let stmt = match super::parser::parse(sql)? {
        Stmt::Select(s) => s,
    };
    let lock = stmt.locking.first().cloned();
    if let Some(lc) = &lock {
        reject_illegal_locking(&stmt, lc)?;
    }

    if !catalog.in_transaction() {
        return run_select(&stmt, catalog);
    }

    let mut skip_arm: Option<(String, std::collections::HashSet<usize>)> = None;

    let regs = Arc::new(catalog.type_registries());
    let regs = &regs;
    if let (Some(lc), Some(FromItem::Table { name, alias })) = (&lock, &stmt.from) {
        let bare = name
            .split_once('.')
            .map(|(_, t)| t)
            .unwrap_or(name.as_str());
        let target = alias.clone().unwrap_or_else(|| bare.to_string());
        let applies = lc.of.is_empty() || lc.of.iter().any(|x| x == &target || x == bare);

        let schema = catalog
            .get(bare)
            .map(|t| t.schema.clone().qualified(&target));
        if applies {
            if let Some(schema) = schema {
                let skip =
                    acquire_row_locks(catalog, bare, stmt.filter.as_ref(), &schema, lc, regs)?;
                if lc.wait == LockWait::SkipLocked && !skip.is_empty() {
                    skip_arm = Some((bare.to_string(), skip));
                }
            }
        }
    }

    catalog.set_lock_skip(skip_arm);
    let result = run_select(&stmt, catalog);
    catalog.set_lock_skip(None);
    result
}

fn reject_illegal_locking(s: &SelectStmt, lc: &LockClause) -> Result<(), PgError> {
    let name = lc.strength.as_str();
    if s.distinct || !s.distinct_on.is_empty() {
        return Err(exec_err(format!(
            "{name} is not allowed with DISTINCT clause"
        )));
    }
    if !s.group_by.is_empty() || !s.grouping_sets.is_empty() {
        return Err(exec_err(format!(
            "{name} is not allowed with GROUP BY clause"
        )));
    }
    if s.having.is_some() {
        return Err(exec_err(format!(
            "{name} is not allowed with HAVING clause"
        )));
    }
    let has_aggregate = s.projection.iter().any(|it| {
        matches!(it, SelectItem::Expr { expr, .. } if crate::expr::eval::contains_aggregate(expr))
    });
    if has_aggregate {
        return Err(exec_err(format!(
            "{name} is not allowed with aggregate functions"
        )));
    }
    if stmt_has_window(s) {
        return Err(exec_err(format!(
            "{name} is not allowed with window functions"
        )));
    }
    Ok(())
}

fn acquire_row_locks(
    catalog: &mut Catalog,
    bare: &str,
    where_: Option<&Expr>,
    schema: &Schema,
    lc: &LockClause,
    regs: &Arc<TypeRegistries>,
) -> Result<std::collections::HashSet<usize>, PgError> {
    let mode = if lc.strength.is_exclusive() {
        LockMode::Exclusive
    } else {
        LockMode::Shared
    };
    let pred = match where_ {
        Some(e) => Some(lower_pred(e, schema, regs.clone())?),
        None => None,
    };
    let xid = catalog.lock_xid();
    let my_read = catalog.read_visibility_xid();

    let mut candidates: Vec<usize> = Vec::new();
    if let Some(t) = catalog.get(bare) {
        for (pos, (row, h)) in t.rows.iter().zip(&t.versions).enumerate() {
            if !catalog.tuple_visible(h, my_read) {
                continue;
            }
            let matched = match &pred {
                Some(p) => p(row).map_err(exec_err)?,
                None => true,
            };
            if matched {
                candidates.push(pos);
            }
        }
    }

    let mut skip = std::collections::HashSet::new();
    for &pos in &candidates {
        if catalog.row_lock_conflict(bare, pos, xid, mode) {
            match lc.wait {
                LockWait::SkipLocked => {
                    skip.insert(pos);
                }
                LockWait::NoWait => {
                    return Err(PgError::LockNotAvailable {
                        rel: bare.to_string(),
                    })
                }
                LockWait::Wait => return Err(PgError::SerializationFailure),
            }
        }
    }

    for &pos in &candidates {
        if !skip.contains(&pos) {
            catalog.acquire_row_lock(bare, pos, xid, mode);
        }
    }
    Ok(skip)
}

fn run_select_stateful(sql: &str, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    let toks = crate::expr::lexer::lex(sql)?;
    if super::cte::has_with(&toks) || super::setops::has_top_level_setop(&toks) {
        return run(sql, catalog);
    }
    let mut stmt = match super::parser::parse(sql)? {
        Stmt::Select(s) => s,
    };
    let in_txn = catalog.in_transaction();
    if !in_txn {
        catalog.stmt_write_begin();
    }
    let folded = super::txid::fold_txid_builtins(&mut stmt, catalog)
        .and_then(|()| super::seq::fold_seq_builtins(&mut stmt, catalog))
        .and_then(|()| super::descr::fold_description_builtins(&mut stmt, catalog));
    let result = folded.and_then(|()| run_select(&stmt, catalog));
    if !in_txn {
        catalog.stmt_write_end(result.is_ok());
    }
    result
}

fn stmt_calls_plpgsql(toks: &[crate::expr::lexer::Tok], catalog: &Catalog) -> bool {
    use crate::expr::lexer::Tok;
    toks.windows(2).any(|w| {
        matches!(&w[0], Tok::Ident(name) if catalog.is_plpgsql_function(name))
            && matches!(w[1], Tok::LParen)
    })
}

fn run_query_plpgsql(sql: &str, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    let toks = crate::expr::lexer::lex(sql)?;

    if super::cte::has_with(&toks) || super::setops::has_top_level_setop(&toks) {
        return run(sql, catalog);
    }
    let mut stmt = match super::parser::parse(sql)? {
        Stmt::Select(s) => s,
    };
    let in_txn = catalog.in_transaction();
    let wrap = !in_txn && !catalog.in_autocommit_stmt();
    if wrap {
        catalog.stmt_write_begin();
    }
    let result = run_query_plpgsql_inner(&mut stmt, catalog);
    if wrap {
        catalog.stmt_write_end(result.is_ok());
    }
    result
}

fn run_query_plpgsql_inner(
    stmt: &mut SelectStmt,
    catalog: &mut Catalog,
) -> Result<QueryResult, PgError> {
    let regs = Arc::new(catalog.type_registries());

    for item in &mut stmt.projection {
        if let SelectItem::Expr { expr, alias } = item {
            if alias.is_none() && crate::stmt::plpgsql::expr_calls_plpgsql(expr, &regs) {
                *alias = Some(inferred_name(expr));
            }
        }
    }

    fold_const_plpgsql_stmt(stmt, catalog, &regs)?;

    let regs = Arc::new(catalog.type_registries());

    run_projection_plpgsql(stmt, catalog, &regs)
}

fn fold_const_plpgsql_stmt(
    stmt: &mut SelectStmt,
    catalog: &mut Catalog,
    regs: &Arc<TypeRegistries>,
) -> Result<(), PgError> {
    let fold = |e: &Expr, cat: &mut Catalog| crate::stmt::plpgsql::fold_const_plpgsql(e, cat, regs);
    for item in &mut stmt.projection {
        if let SelectItem::Expr { expr, .. } = item {
            *expr = fold(expr, catalog)?;
        }
    }
    if let Some(f) = &mut stmt.filter {
        *f = fold(f, catalog)?;
    }
    for e in &mut stmt.group_by {
        *e = fold(e, catalog)?;
    }
    if let Some(h) = &mut stmt.having {
        *h = fold(h, catalog)?;
    }
    for e in &mut stmt.distinct_on {
        *e = fold(e, catalog)?;
    }
    for k in &mut stmt.order_by {
        k.expr = fold(&k.expr, catalog)?;
    }
    Ok(())
}

fn run_projection_plpgsql(
    stmt: &SelectStmt,
    catalog: &mut Catalog,
    regs: &Arc<TypeRegistries>,
) -> Result<QueryResult, PgError> {
    let impure = stmt.projection.iter().any(|it| {
        matches!(it, SelectItem::Expr { expr, .. }
            if crate::stmt::plpgsql::expr_calls_plpgsql(expr, regs))
    });
    if !impure {

        return run_select(stmt, catalog);
    }

    if !stmt.group_by.is_empty()
        || !stmt.grouping_sets.is_empty()
        || stmt.having.is_some()
        || stmt.projection.iter().any(|it| {
            matches!(it, SelectItem::Expr { expr, .. }
                if crate::expr::eval::contains_aggregate(expr))
        })
    {
        return Err(exec_err(
            "a per-row plpgsql function call in a grouped/aggregate query is not supported"
                .to_string(),
        ));
    }

    enum Slot {

        Pure(usize),

        Impure {
            expr: Expr,
            arg_idxs: Vec<Vec<usize>>,
            name: String,
        },
    }
    let mut pure_proj: Vec<SelectItem> = Vec::new();
    let mut slots: Vec<Slot> = Vec::new();
    for item in &stmt.projection {
        match item {
            SelectItem::Star => {

                return Err(exec_err(
                    "`*` combined with a per-row plpgsql call is not supported".to_string(),
                ));
            }
            SelectItem::Expr { expr, alias } => {
                if crate::stmt::plpgsql::expr_calls_plpgsql(expr, regs) {
                    let calls = crate::stmt::plpgsql::collect_plpgsql_calls(expr, regs);
                    let mut arg_idxs = Vec::with_capacity(calls.len());
                    for call in &calls {
                        let args = match call {
                            Expr::Func { args, .. } => args,
                            _ => unreachable!("collect returns only Func nodes"),
                        };
                        let mut idxs = Vec::with_capacity(args.len());
                        for a in args {
                            pure_proj.push(SelectItem::Expr {
                                expr: a.clone(),
                                alias: None,
                            });
                            idxs.push(pure_proj.len() - 1);
                        }
                        arg_idxs.push(idxs);
                    }
                    let name = alias.clone().unwrap_or_else(|| inferred_name(expr));
                    slots.push(Slot::Impure {
                        expr: expr.clone(),
                        arg_idxs,
                        name,
                    });
                } else {
                    pure_proj.push(item.clone());
                    slots.push(Slot::Pure(pure_proj.len() - 1));
                }
            }
        }
    }

    let mut pure_stmt = stmt.clone();
    pure_stmt.projection = pure_proj;
    let pure = run_select(&pure_stmt, catalog)?;

    let empty_schema = Schema::default();
    let mut columns = Vec::with_capacity(slots.len());
    let mut col_types = Vec::with_capacity(slots.len());
    for slot in &slots {
        match slot {
            Slot::Pure(i) => {
                columns.push(pure.columns[*i].clone());
                col_types.push(pure.col_types.get(*i).copied().unwrap_or(0));
            }
            Slot::Impure {
                expr,
                arg_idxs,
                name,
            } => {
                columns.push(name.clone());
                let nulls = vec![SqlValue::Null; arg_idxs.len()];
                let mut idx = 0usize;
                let probe = crate::stmt::plpgsql::subst_plpgsql_calls(expr, &nulls, regs, &mut idx);
                col_types.push(crate::expr::infer::infer(&probe, &empty_schema, &[]).unwrap_or(0));
            }
        }
    }

    let mut out_rows = Vec::with_capacity(pure.rows.len());
    for row in &pure.rows {
        let mut out = Vec::with_capacity(slots.len());
        for slot in &slots {
            match slot {
                Slot::Pure(i) => out.push(row[*i].clone()),
                Slot::Impure { expr, arg_idxs, .. } => {

                    let mut results = Vec::with_capacity(arg_idxs.len());
                    for (call, idxs) in crate::stmt::plpgsql::collect_plpgsql_calls(expr, regs)
                        .iter()
                        .zip(arg_idxs)
                    {
                        let fname = match call {
                            Expr::Func { name, .. } => name,
                            _ => unreachable!(),
                        };
                        let fdef =
                            regs.functions.get(fname).cloned().ok_or_else(|| {
                                exec_err(format!("function {fname} does not exist"))
                            })?;
                        let argv: Vec<SqlValue> = idxs.iter().map(|&i| row[i].clone()).collect();
                        results.push(crate::stmt::plpgsql::run_function(
                            &fdef, &argv, catalog, regs,
                        )?);
                    }

                    let mut idx = 0usize;
                    let substituted =
                        crate::stmt::plpgsql::subst_plpgsql_calls(expr, &results, regs, &mut idx);
                    out.push(crate::expr::eval::eval_ctx(
                        &substituted,
                        crate::expr::eval::EvalCtx::new(regs),
                    )?);
                }
            }
        }
        out_rows.push(out);
    }

    Ok(QueryResult {
        columns,
        col_types,
        rows: out_rows,
    })
}

fn empty_result() -> QueryResult {
    QueryResult {
        columns: Vec::new(),
        col_types: Vec::new(),
        rows: Vec::new(),
    }
}

fn run_txn(
    toks: &[crate::expr::lexer::Tok],
    kw: &str,
    catalog: &mut Catalog,
) -> Result<QueryResult, PgError> {
    use crate::expr::lexer::Tok;
    let ident_at = |i: usize| -> Option<&str> {
        match toks.get(i) {
            Some(Tok::Ident(s)) => Some(s.as_str()),
            _ => None,
        }
    };

    let savepoint_name = |start: usize| -> Result<&str, PgError> {
        let mut i = start;
        if ident_at(i) == Some("savepoint") {
            i += 1;
        }
        ident_at(i).ok_or_else(|| exec_err("expected a savepoint name".into()))
    };

    match kw {
        "begin" | "start" => {
            let was_in_txn = catalog.in_transaction();
            catalog.txn_begin();

            if !was_in_txn {
                if let Some(level) = find_isolation_level(toks) {
                    catalog.set_txn_isolation(level);
                }
            }
        }
        "commit" | "end" => {

            if catalog.in_transaction() {
                if let Err(e) = super::ddl::validate_deferred_foreign_keys(catalog) {
                    catalog.txn_rollback();
                    return Err(e);
                }

                if catalog.ser_write_skew_conflict() {
                    catalog.txn_rollback();
                    return Err(PgError::SerializationFailure);
                }
            }
            catalog.txn_commit();
        }
        "abort" => catalog.txn_rollback(),
        "rollback" => {
            if ident_at(1) == Some("to") {
                let name = savepoint_name(2)?;
                catalog.txn_rollback_to(name)?;
            } else {
                catalog.txn_rollback();
            }
        }
        "savepoint" => {
            let name = ident_at(1)
                .ok_or_else(|| exec_err("SAVEPOINT requires a savepoint name".into()))?;
            catalog.txn_savepoint(name);
        }
        "release" => {
            let name = savepoint_name(1)?;
            catalog.txn_release(name)?;
        }
        _ => unreachable!("run_txn dispatched on unexpected keyword {kw:?}"),
    }
    Ok(empty_result())
}

fn run_set_constraints(
    toks: &[crate::expr::lexer::Tok],
    catalog: &mut Catalog,
) -> Result<QueryResult, PgError> {
    use crate::expr::lexer::Tok;

    let mode = match toks.iter().rev().find_map(|t| match t {
        Tok::Ident(s) => Some(s.as_str()),
        _ => None,
    }) {
        Some("deferred") => true,
        Some("immediate") => false,
        _ => {
            return Err(exec_err(
                "SET CONSTRAINTS requires DEFERRED or IMMEDIATE".into(),
            ))
        }
    };
    catalog.set_constraints_deferred(mode);

    if !mode {
        super::ddl::validate_deferred_foreign_keys(catalog)?;
    }
    Ok(empty_result())
}

fn find_isolation_level(toks: &[crate::expr::lexer::Tok]) -> Option<IsolationLevel> {
    use crate::expr::lexer::Tok;
    let ident_at = |i: usize| -> Option<&str> {
        match toks.get(i) {
            Some(Tok::Ident(s)) => Some(s.as_str()),
            _ => None,
        }
    };
    let i = toks
        .iter()
        .position(|t| matches!(t, Tok::Ident(s) if s == "isolation"))?;
    if ident_at(i + 1) != Some("level") {
        return None;
    }
    match ident_at(i + 2)? {
        "serializable" => Some(IsolationLevel::Serializable),
        "repeatable" if ident_at(i + 3) == Some("read") => Some(IsolationLevel::RepeatableRead),
        "read" => match ident_at(i + 3) {

            Some("committed") | Some("uncommitted") => Some(IsolationLevel::ReadCommitted),
            _ => None,
        },
        _ => None,
    }
}

fn run_set_transaction(
    toks: &[crate::expr::lexer::Tok],
    catalog: &mut Catalog,
) -> Result<QueryResult, PgError> {
    if let Some(level) = find_isolation_level(toks) {
        catalog.set_transaction_isolation(level)?;
    }
    Ok(empty_result())
}

fn run_set_session_characteristics(
    toks: &[crate::expr::lexer::Tok],
    catalog: &mut Catalog,
) -> Result<QueryResult, PgError> {
    if let Some(level) = find_isolation_level(toks) {
        catalog.set_session_isolation(level);
    }
    Ok(empty_result())
}

pub(crate) fn exec_err(msg: String) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "query",
        input: msg,
    }
}

pub(crate) fn run_select(s: &SelectStmt, catalog: &Catalog) -> Result<QueryResult, PgError> {

    let regs = Arc::new(catalog.type_registries());

    let inlined_owned;
    let s = if regs.functions.is_empty() {
        s
    } else {
        inlined_owned = super::func_inline::inline_stmt(s, &regs)?;
        &inlined_owned
    };

    if !s.tail.is_empty() {
        return super::setops::run_setop_stmt(s, catalog);
    }

    if let Some(from) = &s.from {
        if stmt_has_subquery(s) {
            let (schema, col_oids, plan) = plan_from(from, catalog, &regs)?;
            if stmt_is_correlated(s, &schema, catalog)? {
                let outer_aggregate = !s.group_by.is_empty()
                    || !s.grouping_sets.is_empty()
                    || s.having.is_some()
                    || s.projection.iter().any(|it| {
                        matches!(it, SelectItem::Expr { expr, .. }
                            if crate::expr::eval::contains_aggregate(expr))
                    });
                if outer_aggregate {

                    if stmt_has_window(s) {
                        return run_aggregate_correlated_windowed(
                            s, &schema, &col_oids, plan, catalog, &regs,
                        );
                    }
                    return run_aggregate_correlated(s, &schema, &col_oids, plan, catalog, &regs);
                } else {
                    return run_correlated(s, &schema, &col_oids, plan, catalog, &regs);
                }
            }
        }
    }

    let resolved = resolve_stmt(s, catalog)?;

    let resolved = resolve_named_windows(&resolved)?;
    let s = &resolved;

    let (schema, col_oids, mut plan) = match &s.from {
        Some(item) => plan_from_where(item, catalog, s.filter.as_ref(), &regs)?,
        None => (
            Schema::default(),
            Vec::new(),
            Plan::Values(vec![Vec::new()]),
        ),
    };

    if let Some(f) = &s.filter {
        let f = rewrite_bpchar_cmp(f, &schema, &col_oids);

        let f = rewrite_composite_cmp(&f, &schema, &col_oids, &regs)?;

        let f = rewrite_enum(&f, &schema, &col_oids, &regs);
        plan = Plan::Filter {
            input: Box::new(plan),
            pred: lower_pred(&f, &schema, regs.clone())?,
        };
    }

    let is_aggregate = !s.group_by.is_empty()
        || !s.grouping_sets.is_empty()
        || s.having.is_some()
        || s.projection.iter().any(|it| {
            matches!(it, SelectItem::Expr { expr, .. }
                if crate::expr::eval::contains_aggregate(expr)
                    || crate::expr::eval::contains_user_aggregate(expr, &regs))
        });
    if is_aggregate {

        if stmt_has_window(s) {
            return finish_aggregate_windowed(s, &schema, &col_oids, plan, catalog, &regs);
        }
        return finish_aggregate(s, &schema, &col_oids, plan, &regs);
    }

    if select_srf_index(&s.projection).is_some() {
        if stmt_has_window(s) {
            let (ws, wsch, woid, wplan) =
                apply_windows(s, &schema, &col_oids, plan, catalog, &regs)?;
            return run_srf_select(&ws, &wsch, &woid, wplan, &regs);
        }
        return run_srf_select(s, &schema, &col_oids, plan, &regs);
    }

    finish_projection(s, &schema, &col_oids, plan, catalog, &regs)
}

fn finish_projection(
    s: &SelectStmt,
    schema: &Schema,
    col_oids: &[u32],
    plan: Plan,
    catalog: &Catalog,
    regs: &Arc<TypeRegistries>,
) -> Result<QueryResult, PgError> {
    let (mut schema, mut col_oids, mut plan) = (schema.clone(), col_oids.to_vec(), plan);

    let window_stmt;
    let s = if stmt_has_window(s) {
        let (ns, nsch, noid, nplan) = apply_windows(s, &schema, &col_oids, plan, catalog, regs)?;
        schema = nsch;
        col_oids = noid;
        plan = nplan;
        window_stmt = ns;
        &window_stmt
    } else {
        s
    };

    if !s.order_by.is_empty() {
        let mut keys = Vec::with_capacity(s.order_by.len());
        for k in &s.order_by {
            let expr = match &k.expr {

                Expr::Int(n) if *n >= 1 => match s.projection.get(*n as usize - 1) {
                    Some(SelectItem::Expr { expr, .. }) => expr.clone(),
                    _ => k.expr.clone(),
                },

                Expr::Column(name) if schema.index_of(name).is_none() => {
                    let want = name.rsplit('.').next().unwrap_or(name);
                    s.projection
                        .iter()
                        .find_map(|it| match it {
                            SelectItem::Expr {
                                expr,
                                alias: Some(a),
                            } if a == want => Some(expr.clone()),
                            _ => None,
                        })
                        .unwrap_or_else(|| k.expr.clone())
                }
                _ => k.expr.clone(),
            };
            let mut key = lower(&expr, &schema, regs.clone())?;

            match crate::expr::infer::infer(&expr, &schema, &col_oids) {
                Some(crate::types::oid::NUMERIC) => key = numeric_sort_key(key),
                Some(crate::types::oid::JSONB) => key = jsonb_sort_key(key),

                Some(crate::types::oid::BPCHAR) => key = bpchar_sort_key(key),

                Some(oid) if regs.is_enum(oid) => {
                    if let Some(labels) = regs.labels(oid) {
                        key = enum_sort_key(key, labels.to_vec());
                    }
                }

                Some(oid) if regs.composite(oid).is_some() => {
                    let fields = regs.composite(oid).unwrap().fields.clone();
                    key = composite_sort_key(key, fields, regs.clone());
                }
                _ => {}
            }
            keys.push((
                key,
                SortOptions::with_default(
                    k.descending,
                    k.nulls_first,
                    NullsDefault::Postgres,
                    TextCollation::Binary,
                ),
            ));
        }
        plan = Plan::Sort {
            input: Box::new(plan),
            keys,
        };
    }

    let mut cols = Vec::new();
    let mut columns = Vec::new();
    let mut col_types = Vec::new();
    for item in &s.projection {
        match item {
            SelectItem::Star => {
                for (i, name) in schema.names().iter().enumerate() {
                    cols.push(lower(&Expr::ColumnRef(i), &schema, regs.clone())?);
                    columns.push(name.clone());
                    col_types.push(col_oids.get(i).copied().unwrap_or(0));
                }
            }
            SelectItem::Expr { expr, alias } => {

                let base = rewrite_row_to_json(expr, &schema, &col_oids, regs);
                let rexpr = rewrite_composite_cmp(&base, &schema, &col_oids, regs)?;
                let rexpr = rewrite_enum(&rexpr, &schema, &col_oids, regs);
                cols.push(lower(&rexpr, &schema, regs.clone())?);
                columns.push(alias.clone().unwrap_or_else(|| inferred_name(expr)));
                col_types.push(crate::expr::infer::infer(&rexpr, &schema, &col_oids).unwrap_or(0));
            }
        }
    }

    let n_real = columns.len();
    for e in &s.distinct_on {
        cols.push(lower(e, &schema, regs.clone())?);
    }
    plan = Plan::Project {
        input: Box::new(plan),
        cols,
    };

    let mut rows = plan.execute().map_err(exec_err)?;

    if !s.distinct_on.is_empty() {
        rows = dedup_on_keys(rows, n_real);
        for r in &mut rows {
            r.truncate(n_real);
        }
    }

    if s.distinct {
        rows = dedup_preserving_order_typed(rows, &col_types, regs);
    }

    let off = s.offset.max(0) as usize;
    rows = rows.into_iter().skip(off).collect();
    if let Some(n) = s.limit {
        rows.truncate(n.max(0) as usize);
    }

    Ok(QueryResult {
        columns,
        col_types,
        rows,
    })
}

fn view_columns(v: &crate::catalog::View, r: &QueryResult) -> (Vec<String>, Vec<u32>) {
    let mut cols = r.columns.clone();
    if let Some(aliases) = &v.columns {
        for (i, a) in aliases.iter().enumerate() {
            if i < cols.len() {
                cols[i] = a.clone();
            }
        }
    }
    let mut oids = r.col_types.clone();
    oids.resize(cols.len(), 0);
    (cols, oids)
}

fn eq_index_safe_oid(oid: u32) -> bool {
    use crate::types::oid;
    matches!(
        oid,
        oid::BOOL | oid::INT2 | oid::INT4 | oid::INT8 | oid::TEXT | oid::VARCHAR
    )
}

fn find_indexable_eq(where_: &Expr, schema: &Schema) -> Option<(usize, SqlValue)> {

    find_indexable_eqs(where_, schema).into_iter().next()
}

fn find_indexable_eqs(where_: &Expr, schema: &Schema) -> Vec<(usize, SqlValue)> {
    let mut found: Vec<(usize, SqlValue)> = Vec::new();
    let mut stack = vec![where_];
    while let Some(cur) = stack.pop() {
        match cur {
            Expr::Binary {
                op: BinOp::And,
                left,
                right,
            } => {
                stack.push(left);
                stack.push(right);
            }
            Expr::Binary {
                op: BinOp::Eq,
                left,
                right,
            } => {
                for (col_side, val_side) in [
                    (left.as_ref(), right.as_ref()),
                    (right.as_ref(), left.as_ref()),
                ] {
                    if let Expr::Column(name) = col_side {
                        if let Some(ci) = schema.index_of(name) {

                            if let Ok(v) = crate::expr::eval::eval(val_side) {
                                if !matches!(v, SqlValue::Null)
                                    && !found.iter().any(|(c, _)| *c == ci)
                                {
                                    found.push((ci, v));
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    found.sort_by_key(|(ci, _)| *ci);
    found
}

fn find_indexable_range(
    where_: &Expr,
    schema: &Schema,
) -> Option<(usize, std::ops::Bound<SqlValue>, std::ops::Bound<SqlValue>)> {
    use std::ops::Bound;

    let mut lowers: std::collections::BTreeMap<usize, Bound<SqlValue>> =
        std::collections::BTreeMap::new();
    let mut uppers: std::collections::BTreeMap<usize, Bound<SqlValue>> =
        std::collections::BTreeMap::new();
    let mut stack = vec![where_];
    while let Some(cur) = stack.pop() {
        match cur {
            Expr::Binary {
                op: BinOp::And,
                left,
                right,
            } => {
                stack.push(left);
                stack.push(right);
            }
            Expr::Binary { op, left, right }
                if matches!(op, BinOp::Lt | BinOp::LtEq | BinOp::Gt | BinOp::GtEq) =>
            {
                for (col_side, val_side, flip) in [
                    (left.as_ref(), right.as_ref(), false),
                    (right.as_ref(), left.as_ref(), true),
                ] {
                    let Expr::Column(name) = col_side else {
                        continue;
                    };
                    let Some(ci) = schema.index_of(name) else {
                        continue;
                    };

                    let v = match crate::expr::eval::eval(val_side) {
                        Ok(v) if !matches!(v, SqlValue::Null) => v,
                        _ => continue,
                    };

                    let eff = match (op, flip) {
                        (BinOp::Gt, false) | (BinOp::Lt, true) => (true, false),
                        (BinOp::GtEq, false) | (BinOp::LtEq, true) => (true, true),
                        (BinOp::Lt, false) | (BinOp::Gt, true) => (false, false),
                        (BinOp::LtEq, false) | (BinOp::GtEq, true) => (false, true),
                        _ => continue,
                    };
                    let (is_lower, inclusive) = eff;
                    let bound = if inclusive {
                        Bound::Included(v)
                    } else {
                        Bound::Excluded(v)
                    };
                    let slot = if is_lower { &mut lowers } else { &mut uppers };
                    slot.entry(ci).or_insert(bound);
                    break;
                }
            }
            _ => {}
        }
    }

    let ci = lowers.keys().chain(uppers.keys()).copied().min()?;
    let lo = lowers.get(&ci).cloned().unwrap_or(Bound::Unbounded);
    let hi = uppers.get(&ci).cloned().unwrap_or(Bound::Unbounded);
    Some((ci, lo, hi))
}

fn index_scan_rows(
    t: &crate::catalog::Table,
    schema: &Schema,
    where_: Option<&Expr>,
    catalog: &Catalog,
    bare: &str,
) -> Option<Vec<Row>> {
    let regs = Arc::new(catalog.type_registries());
    let regs = &regs;

    let mut cols: Vec<usize> = Vec::new();
    let mut keys: Vec<SqlValue> = Vec::new();
    for (ci, key) in find_indexable_eqs(where_?, schema) {
        let oid = match t.col_types.get(ci) {
            Some(&o) if eq_index_safe_oid(o) => o,
            _ => continue,
        };
        let typmod = t
            .col_typmods
            .get(ci)
            .copied()
            .unwrap_or(crate::types::typmod::NONE);
        let coerced = match crate::stmt::ddl::coerce(key, Some(oid), typmod, regs) {
            Ok(v) if !matches!(v, SqlValue::Null) => v,
            _ => continue,
        };
        cols.push(ci);
        keys.push(coerced);
    }
    let positions: Vec<usize> = match cols.len() {

        0 => {
            use std::ops::Bound;
            let (ci, lo, hi) = find_indexable_range(where_?, schema)?;
            let oid = match t.col_types.get(ci) {
                Some(&o) if eq_index_safe_oid(o) => o,
                _ => return None,
            };
            let typmod = t
                .col_typmods
                .get(ci)
                .copied()
                .unwrap_or(crate::types::typmod::NONE);

            let coerce_bound = |b: Bound<SqlValue>| -> Option<Bound<SqlValue>> {
                match b {
                    Bound::Unbounded => Some(Bound::Unbounded),
                    Bound::Included(v) => {
                        match crate::stmt::ddl::coerce(v, Some(oid), typmod, regs) {
                            Ok(v) if !matches!(v, SqlValue::Null) => Some(Bound::Included(v)),
                            _ => None,
                        }
                    }
                    Bound::Excluded(v) => {
                        match crate::stmt::ddl::coerce(v, Some(oid), typmod, regs) {
                            Ok(v) if !matches!(v, SqlValue::Null) => Some(Bound::Excluded(v)),
                            _ => None,
                        }
                    }
                }
            };
            let lo = coerce_bound(lo)?;
            let hi = coerce_bound(hi)?;

            let mut cache = t.eq_indexes.borrow_mut();
            let idx = cache
                .entry(ci)
                .or_insert_with(|| sql_core::EqIndex::build(t.rows.iter().map(|r| r[ci].clone())));
            idx.range(lo.as_ref(), hi.as_ref())
        }

        1 => {
            let ci = cols[0];
            let mut cache = t.eq_indexes.borrow_mut();
            let idx = cache
                .entry(ci)
                .or_insert_with(|| sql_core::EqIndex::build(t.rows.iter().map(|r| r[ci].clone())));
            idx.probe(&keys[0]).to_vec()
        }

        _ => {
            let mut cache = t.eq_indexes_multi.borrow_mut();
            let idx = cache.entry(cols.clone()).or_insert_with(|| {
                sql_core::EqIndexN::build(
                    t.rows
                        .iter()
                        .map(|r| cols.iter().map(|&ci| r[ci].clone()).collect()),
                )
            });
            idx.probe(&keys).to_vec()
        }
    };

    let my_xid = catalog.read_visibility_xid();

    let mut vis: Vec<(u64, usize)> = positions
        .iter()
        .filter(|&&p| catalog.tuple_visible(&t.versions[p], my_xid))
        .map(|&p| (t.rids[p], p))
        .collect();
    vis.sort_by_key(|&(rid, _)| rid);
    Some(
        vis.into_iter()
            .map(|(_, p)| {

                catalog.note_ser_read(bare, p);
                t.rows[p].clone()
            })
            .collect(),
    )
}

pub(crate) fn index_access_path(
    t: &crate::catalog::Table,
    schema: &Schema,
    where_: Option<&Expr>,
    regs: &Arc<TypeRegistries>,
) -> Option<(String, SqlValue)> {
    let (ci, key) = find_indexable_eq(where_?, schema)?;
    let oid = *t.col_types.get(ci)?;
    if !eq_index_safe_oid(oid) {
        return None;
    }
    let typmod = t
        .col_typmods
        .get(ci)
        .copied()
        .unwrap_or(crate::types::typmod::NONE);
    let coerced = crate::stmt::ddl::coerce(key, Some(oid), typmod, regs).ok()?;
    if matches!(coerced, SqlValue::Null) {
        return None;
    }
    let name = schema.names().get(ci)?.clone();
    Some((name, coerced))
}

pub(crate) fn range_access_path(
    t: &crate::catalog::Table,
    schema: &Schema,
    where_: Option<&Expr>,
    regs: &Arc<TypeRegistries>,
) -> Option<(String, std::ops::Bound<SqlValue>, std::ops::Bound<SqlValue>)> {
    use std::ops::Bound;
    let w = where_?;

    if index_access_path(t, schema, where_, regs).is_some() {
        return None;
    }
    let (ci, lo, hi) = find_indexable_range(w, schema)?;
    let oid = *t.col_types.get(ci)?;
    if !eq_index_safe_oid(oid) {
        return None;
    }
    let typmod = t
        .col_typmods
        .get(ci)
        .copied()
        .unwrap_or(crate::types::typmod::NONE);
    let coerce_bound = |b: Bound<SqlValue>| -> Option<Bound<SqlValue>> {
        match b {
            Bound::Unbounded => Some(Bound::Unbounded),
            Bound::Included(v) => match crate::stmt::ddl::coerce(v, Some(oid), typmod, regs) {
                Ok(v) if !matches!(v, SqlValue::Null) => Some(Bound::Included(v)),
                _ => None,
            },
            Bound::Excluded(v) => match crate::stmt::ddl::coerce(v, Some(oid), typmod, regs) {
                Ok(v) if !matches!(v, SqlValue::Null) => Some(Bound::Excluded(v)),
                _ => None,
            },
        }
    };
    let lo = coerce_bound(lo)?;
    let hi = coerce_bound(hi)?;
    let name = schema.names().get(ci)?.clone();
    Some((name, lo, hi))
}

fn plan_from(
    item: &FromItem,
    catalog: &Catalog,
    regs: &Arc<TypeRegistries>,
) -> Result<(Schema, Vec<u32>, Plan), PgError> {
    plan_from_where(item, catalog, None, regs)
}

fn plan_from_where(
    item: &FromItem,
    catalog: &Catalog,
    where_: Option<&Expr>,
    regs: &Arc<TypeRegistries>,
) -> Result<(Schema, Vec<u32>, Plan), PgError> {
    match item {
        FromItem::Table { name, alias } => {

            let (schema_q, bare) = match name.split_once('.') {
                Some((s, t)) => (Some(s), t),
                None => (None, name.as_str()),
            };
            if matches!(schema_q, Some("information_schema") | Some("pg_catalog")) {
                return synth_system_view(schema_q.unwrap(), bare, alias, catalog);
            }

            if let Some(t) = catalog.get(bare) {
                let qual = alias.clone().unwrap_or_else(|| bare.to_string());
                let schema = t.schema.clone().qualified(&qual);
                let mut oids = t.col_types.clone();
                oids.resize(t.schema.width(), 0);

                if let Some(pinfo) = catalog.partition_info(bare) {
                    let mut rows: Vec<Row> = Vec::new();
                    for child in &pinfo.children {
                        if let Some(crows) = catalog.visible_rows(child) {
                            rows.extend(crows);
                        }
                    }
                    return Ok((schema, oids, Plan::Scan(rows)));
                }

                if !catalog.lock_skip_table(bare) {
                    if let Some(rows) = index_scan_rows(t, &schema, where_, catalog, bare) {
                        if std::env::var_os("CRUFT_SQL_PLAN").is_some() {
                            eprintln!("[sql-plan] IndexScan {bare} -> {} row(s)", rows.len());
                        }
                        return Ok((schema, oids, Plan::Scan(rows)));
                    }
                }

                let my_xid = catalog.read_visibility_xid();

                let mut vis: Vec<(u64, usize)> = t
                    .versions
                    .iter()
                    .enumerate()
                    .filter(|(i, h)| {
                        catalog.tuple_visible(h, my_xid) && !catalog.scan_skips(bare, *i)
                    })
                    .map(|(i, _)| (t.rids[i], i))
                    .collect();
                vis.sort_by_key(|&(rid, _)| rid);
                let rows: Vec<Row> = vis
                    .into_iter()
                    .map(|(_, i)| {

                        catalog.note_ser_read(bare, i);
                        t.rows[i].clone()
                    })
                    .collect();
                return Ok((schema, oids, Plan::Scan(rows)));
            }
            if let Some(v) = catalog.get_view(bare) {
                let qual = alias.clone().unwrap_or_else(|| bare.to_string());

                if v.materialized {
                    let schema = Schema::new(v.mat_columns.clone()).qualified(&qual);
                    return Ok((
                        schema,
                        v.mat_col_types.clone(),
                        Plan::Scan(v.mat_rows.clone()),
                    ));
                }
                let r = run_select(&v.query, catalog)?;
                let (cols, oids) = view_columns(v, &r);
                let schema = Schema::new(cols).qualified(&qual);
                return Ok((schema, oids, Plan::Scan(r.rows)));
            }
            Err(PgError::InvalidInputSyntax {
                typ: "query",
                input: format!("relation \"{name}\" does not exist"),
            })
        }
        FromItem::Join {
            left,
            right,
            kind,
            on,
            using,
            natural,
        } => {

            if from_is_lateral(right) {
                return plan_lateral_join(left, right, *kind, on.as_ref(), catalog, regs);
            }
            let (ls, lt, lp) = plan_from(left, catalog, regs)?;
            let (rs, rt, rp) = plan_from(right, catalog, regs)?;
            let left_width = ls.width();
            let right_width = rs.width();
            let core_kind = match kind {
                super::ast::JoinKind::Inner => sql_core::JoinKind::Inner,
                super::ast::JoinKind::Left => sql_core::JoinKind::Left,
                super::ast::JoinKind::Cross => sql_core::JoinKind::Cross,
                super::ast::JoinKind::Right => sql_core::JoinKind::Right,
                super::ast::JoinKind::Full => sql_core::JoinKind::Full,
            };

            let join_cols: Vec<String> = if *natural {
                let rnames = rs.names();
                ls.names()
                    .into_iter()
                    .filter(|n| rnames.contains(n))
                    .collect()
            } else {
                using.clone()
            };

            let raw = ls.clone().concat(rs.clone());
            let mut raw_types = lt.clone();
            raw_types.extend(rt.clone());

            if join_cols.is_empty() {

                if let (Some(e), Some(hk)) = (on, hashable_kind(*kind)) {
                    let resolved = crate::expr::bind::resolve(e, &raw)?;
                    let (keys, residual) = split_equi_on(&resolved, left_width);
                    if !keys.is_empty() {
                        let mut left_keys: Vec<Scalar> = Vec::with_capacity(keys.len());
                        let mut right_keys: Vec<Scalar> = Vec::with_capacity(keys.len());
                        for &(li, ri) in &keys {

                            left_keys.push(lower(&Expr::ColumnRef(li), &ls, regs.clone())?);
                            right_keys.push(lower(
                                &Expr::ColumnRef(ri - left_width),
                                &rs,
                                regs.clone(),
                            )?);
                        }
                        let extra = match residual {
                            Some(r) => Some(lower_pred(&r, &raw, regs.clone())?),
                            None => None,
                        };
                        let plan = Plan::HashJoin {
                            left: Box::new(lp),
                            right: Box::new(rp),
                            left_width,
                            right_width,
                            kind: hk,
                            left_keys,
                            right_keys,
                            extra,
                        };
                        return Ok((raw, raw_types, plan));
                    }
                }
                let pred = match on {
                    Some(e) => Some(lower_pred(e, &raw, regs.clone())?),
                    None => None,
                };
                let plan = Plan::NestedLoopJoin {
                    left: Box::new(lp),
                    right: Box::new(rp),
                    left_width,
                    right_width,
                    kind: core_kind,
                    pred,
                };
                return Ok((raw, raw_types, plan));
            }

            let mut left_idx = Vec::with_capacity(join_cols.len());
            let mut right_idx = Vec::with_capacity(join_cols.len());
            for c in &join_cols {
                let li = ls.index_of(c).ok_or_else(|| {
                    exec_err(format!(
                        "column \"{c}\" specified in USING clause does not exist in left table"
                    ))
                })?;
                let ri = rs.index_of(c).ok_or_else(|| {
                    exec_err(format!(
                        "column \"{c}\" specified in USING clause does not exist in right table"
                    ))
                })?;
                left_idx.push(li);
                right_idx.push(left_width + ri);
            }

            let mut pred_expr: Option<Expr> = None;
            for (&li, &ri) in left_idx.iter().zip(&right_idx) {
                let eq = Expr::Binary {
                    op: BinOp::Eq,
                    left: Box::new(Expr::ColumnRef(li)),
                    right: Box::new(Expr::ColumnRef(ri)),
                };
                pred_expr = Some(match pred_expr {
                    Some(acc) => Expr::Binary {
                        op: BinOp::And,
                        left: Box::new(acc),
                        right: Box::new(eq),
                    },
                    None => eq,
                });
            }

            let join_plan = if let Some(hk) = hashable_kind(*kind) {
                let mut left_keys: Vec<Scalar> = Vec::with_capacity(left_idx.len());
                let mut right_keys: Vec<Scalar> = Vec::with_capacity(right_idx.len());
                for (&li, &ri) in left_idx.iter().zip(&right_idx) {
                    left_keys.push(lower(&Expr::ColumnRef(li), &ls, regs.clone())?);

                    right_keys.push(lower(&Expr::ColumnRef(ri - left_width), &rs, regs.clone())?);
                }
                Plan::HashJoin {
                    left: Box::new(lp),
                    right: Box::new(rp),
                    left_width,
                    right_width,
                    kind: hk,
                    left_keys,
                    right_keys,
                    extra: None,
                }
            } else {
                let pred = Some(lower_pred(&pred_expr.unwrap(), &raw, regs.clone())?);
                Plan::NestedLoopJoin {
                    left: Box::new(lp),
                    right: Box::new(rp),
                    left_width,
                    right_width,
                    kind: core_kind,
                    pred,
                }
            };

            let mut out_schema = Schema::default();
            let mut out_types: Vec<u32> = Vec::new();
            let mut cols: Vec<Expr> = Vec::new();
            for (k, c) in join_cols.iter().enumerate() {
                let (li, ri) = (left_idx[k], right_idx[k]);

                cols.push(Expr::Case {
                    operand: None,
                    whens: vec![(
                        Expr::IsNull {
                            expr: Box::new(Expr::ColumnRef(li)),
                            negated: true,
                        },
                        Expr::ColumnRef(li),
                    )],
                    else_: Some(Box::new(Expr::ColumnRef(ri))),
                });
                out_schema = out_schema.concat(Schema::new([c.clone()]));
                let lo = raw_types.get(li).copied().unwrap_or(0);
                out_types.push(if lo != 0 {
                    lo
                } else {
                    raw_types.get(ri).copied().unwrap_or(0)
                });
            }
            let raw_cols = raw.cols();
            for (i, (qual, name)) in raw_cols.iter().enumerate() {

                if left_idx.contains(&i) || right_idx.contains(&i) {
                    continue;
                }
                cols.push(Expr::ColumnRef(i));
                let mut s = Schema::new([name.clone()]);
                if let Some(q) = qual {
                    s = s.qualified(q);
                }
                out_schema = out_schema.concat(s);
                out_types.push(raw_types.get(i).copied().unwrap_or(0));
            }

            let projected: Vec<_> = cols
                .iter()
                .map(|e| lower(e, &raw, regs.clone()))
                .collect::<Result<_, PgError>>()?;
            let plan = Plan::Project {
                input: Box::new(join_plan),
                cols: projected,
            };
            Ok((out_schema, out_types, plan))
        }
        FromItem::Subquery {
            query,
            alias,
            lateral: _,
        } => {

            let r = run_select(query, catalog)?;
            let schema = Schema::new(r.columns.clone()).qualified(alias);
            let mut oids = r.col_types.clone();
            oids.resize(r.columns.len(), 0);
            Ok((schema, oids, Plan::Scan(r.rows)))
        }
        FromItem::Function {
            name,
            args,
            alias,
            lateral: _,
        } => {

            let (cols, oids, rows) = match regs.functions.get(name) {
                Some(fdef) if fdef.returns.is_setof() => {
                    eval_setof_func(fdef, args, alias, name, catalog)?
                }
                _ => eval_srf(name, args, regs)?,
            };

            let qual = alias.clone().unwrap_or_else(|| name.clone());
            let cols = srf_alias_cols(cols, alias);
            let schema = Schema::new(cols).qualified(&qual);
            Ok((schema, oids, Plan::Scan(rows)))
        }
    }
}

fn eval_setof_func(
    fdef: &FunctionDef,
    args: &[Expr],
    _alias: &Option<String>,
    name: &str,
    catalog: &Catalog,
) -> Result<(Vec<String>, Vec<u32>, Vec<Row>), PgError> {
    let srf_err = |msg: String| PgError::InvalidInputSyntax {
        typ: "query",
        input: msg,
    };

    if args.len() != fdef.args.len() {
        return Err(srf_err(format!("function {name}(...) does not exist")));
    }
    let body = super::func_inline::setof_body(fdef, args);
    let r = run_select(&body, catalog)?;

    match &fdef.returns {
        RetShape::SetofScalar { oid } => {

            let from = r.col_types.first().copied().unwrap_or(0);
            let rows = r
                .rows
                .into_iter()
                .map(|mut row| {
                    if let Some(v) = row.first_mut() {
                        *v = crate::types::cast(v, from, *oid)?;
                    }
                    Ok(row)
                })
                .collect::<Result<Vec<_>, PgError>>()?;
            Ok((vec![name.to_string()], vec![*oid], rows))
        }
        RetShape::SetofTable(decl) => {

            let names: Vec<String> = decl.iter().map(|(n, _, _)| n.clone()).collect();
            let oids: Vec<u32> = decl.iter().map(|(_, o, _)| *o).collect();
            let rows = r
                .rows
                .into_iter()
                .map(|row| {
                    row.into_iter()
                        .enumerate()
                        .map(|(i, v)| {
                            let from = r.col_types.get(i).copied().unwrap_or(0);
                            let to = oids.get(i).copied().unwrap_or(from);
                            crate::types::cast(&v, from, to)
                        })
                        .collect::<Result<Vec<_>, PgError>>()
                })
                .collect::<Result<Vec<_>, PgError>>()?;
            Ok((names, oids, rows))
        }
        RetShape::SetofRel => {

            Ok((r.columns, r.col_types, r.rows))
        }
        RetShape::Scalar => unreachable!("eval_setof_func on a scalar function"),
    }
}

fn synth_system_view(
    schema: &str,
    table: &str,
    alias: &Option<String>,
    catalog: &Catalog,
) -> Result<(Schema, Vec<u32>, Plan), PgError> {
    use crate::types::oid::{INT4, TEXT};
    let txt = |s: &str| SqlValue::Text(s.to_string());

    let (cols, oids, rows): (Vec<&str>, Vec<u32>, Vec<Row>) = match (schema, table) {

        ("information_schema", "tables") => {
            let mut rows = Vec::new();
            for (name, _) in catalog.tables_iter() {
                rows.push(vec![
                    txt("postcrust"),
                    txt("public"),
                    txt(name),
                    txt("BASE TABLE"),
                ]);
            }
            for (name, v) in catalog.views_iter() {

                if v.materialized {
                    continue;
                }
                rows.push(vec![
                    txt("postcrust"),
                    txt("public"),
                    txt(name),
                    txt("VIEW"),
                ]);
            }
            (
                vec!["table_catalog", "table_schema", "table_name", "table_type"],
                vec![TEXT, TEXT, TEXT, TEXT],
                rows,
            )
        }

        ("information_schema", "columns") => {
            use crate::types::{oid as toid, typmod};

            let ocell =
                |o: Option<i32>| o.map(|n| SqlValue::Int(n as i64)).unwrap_or(SqlValue::Null);
            let mut rows = Vec::new();
            for (name, tab) in catalog.tables_iter() {
                for (i, cname) in tab.schema.names().iter().enumerate() {
                    let oid = tab.col_types.get(i).copied().unwrap_or(0);
                    let tm = tab.col_typmods.get(i).copied().unwrap_or(typmod::NONE);

                    let data_type = if crate::types::is_array(oid) {
                        "ARRAY"
                    } else {
                        crate::types::type_name(oid)
                    };

                    let nullable = if tab.constraints.not_null.get(i).copied().unwrap_or(false) {
                        "NO"
                    } else {
                        "YES"
                    };

                    let char_len = if matches!(oid, toid::VARCHAR | toid::BPCHAR) {
                        typmod::char_len(tm)
                    } else {
                        None
                    };

                    let (num_prec, num_radix, num_scale) = match oid {
                        toid::INT2 => (Some(16), Some(2), Some(0)),
                        toid::INT4 => (Some(32), Some(2), Some(0)),
                        toid::INT8 => (Some(64), Some(2), Some(0)),
                        toid::FLOAT4 => (Some(24), Some(2), None),
                        toid::FLOAT8 => (Some(53), Some(2), None),
                        toid::NUMERIC => (
                            typmod::numeric_precision(tm),
                            Some(10),
                            typmod::numeric_scale(tm),
                        ),
                        _ => (None, None, None),
                    };
                    let ident_kind = tab.identity.get(i).copied().unwrap_or_default();
                    let (is_identity, ident_gen) = match ident_kind {
                        crate::catalog::IdentityKind::None => ("NO", SqlValue::Null),
                        crate::catalog::IdentityKind::Always => ("YES", txt("ALWAYS")),
                        crate::catalog::IdentityKind::ByDefault => ("YES", txt("BY DEFAULT")),
                    };

                    let default_cell = match tab.defaults.get(i).and_then(|d| d.as_ref()) {
                        Some(expr) if ident_kind == crate::catalog::IdentityKind::None => {
                            txt(&crate::expr::deparse::column_default_text(expr, oid))
                        }
                        _ => SqlValue::Null,
                    };
                    rows.push(vec![
                        txt("postcrust"),
                        txt("public"),
                        txt(name),
                        txt(cname),
                        SqlValue::Int((i + 1) as i64),
                        txt(data_type),
                        txt(nullable),
                        txt(crate::types::udt_name(oid)),
                        ocell(char_len),
                        ocell(num_prec),
                        ocell(num_radix),
                        ocell(num_scale),
                        txt(is_identity),
                        ident_gen,
                        default_cell,
                    ]);
                }
            }
            (
                vec![
                    "table_catalog",
                    "table_schema",
                    "table_name",
                    "column_name",
                    "ordinal_position",
                    "data_type",
                    "is_nullable",
                    "udt_name",
                    "character_maximum_length",
                    "numeric_precision",
                    "numeric_precision_radix",
                    "numeric_scale",
                    "is_identity",
                    "identity_generation",
                    "column_default",
                ],
                vec![
                    TEXT, TEXT, TEXT, TEXT, INT4, TEXT, TEXT, TEXT, INT4, INT4, INT4, INT4, TEXT,
                    TEXT, TEXT,
                ],
                rows,
            )
        }

        ("information_schema", "table_constraints") => {
            let mut rows = Vec::new();
            for (tname, tab) in catalog.tables_iter() {
                let names = tab.schema.names();
                for uk in &tab.constraints.uniques {
                    let ctype = if uk.is_primary {
                        "PRIMARY KEY"
                    } else {
                        "UNIQUE"
                    };
                    let cname = uk.name.clone().unwrap_or_else(|| {
                        format!("{tname}_{}", if uk.is_primary { "pkey" } else { "key" })
                    });
                    rows.push(vec![
                        txt("postcrust"),
                        txt("public"),
                        txt(&cname),
                        txt("postcrust"),
                        txt("public"),
                        txt(tname),
                        txt(ctype),
                        txt("NO"),
                        txt("NO"),
                    ]);
                }
                for fk in &tab.constraints.foreign_keys {
                    let cname = fk.name.clone().unwrap_or_else(|| format!("{tname}_fkey"));
                    rows.push(vec![
                        txt("postcrust"),
                        txt("public"),
                        txt(&cname),
                        txt("postcrust"),
                        txt("public"),
                        txt(tname),
                        txt("FOREIGN KEY"),
                        txt(if fk.deferrable { "YES" } else { "NO" }),
                        txt(if fk.initially_deferred { "YES" } else { "NO" }),
                    ]);
                }
                for ck in &tab.constraints.checks {
                    if let Some(cname) = &ck.name {
                        let _ = names;
                        rows.push(vec![
                            txt("postcrust"),
                            txt("public"),
                            txt(cname),
                            txt("postcrust"),
                            txt("public"),
                            txt(tname),
                            txt("CHECK"),
                            txt("NO"),
                            txt("NO"),
                        ]);
                    }
                }
            }
            (
                vec![
                    "constraint_catalog",
                    "constraint_schema",
                    "constraint_name",
                    "table_catalog",
                    "table_schema",
                    "table_name",
                    "constraint_type",
                    "is_deferrable",
                    "initially_deferred",
                ],
                vec![TEXT, TEXT, TEXT, TEXT, TEXT, TEXT, TEXT, TEXT, TEXT],
                rows,
            )
        }

        ("information_schema", "check_constraints") => {
            let mut rows = Vec::new();
            for (_tname, tab) in catalog.tables_iter() {
                for ck in &tab.constraints.checks {
                    if let Some(cname) = &ck.name {

                        let clause = crate::expr::deparse::deparse_check(
                            &ck.expr,
                            &tab.schema,
                            &tab.col_types,
                        );
                        rows.push(vec![
                            txt("postcrust"),
                            txt("public"),
                            txt(cname),
                            txt(&clause),
                        ]);
                    }
                }
            }
            (
                vec![
                    "constraint_catalog",
                    "constraint_schema",
                    "constraint_name",
                    "check_clause",
                ],
                vec![TEXT, TEXT, TEXT, TEXT],
                rows,
            )
        }

        ("information_schema", "sequences") => {
            let mut rows: Vec<Vec<SqlValue>> = catalog
                .sequences_iter()
                .map(|(name, s)| {
                    vec![
                        txt("postcrust"),
                        txt("public"),
                        txt(name),
                        txt("bigint"),
                        SqlValue::Int(64),
                        SqlValue::Int(2),
                        SqlValue::Int(0),
                        txt(&s.start.to_string()),
                        txt(&s.min.to_string()),
                        txt(&s.max.to_string()),
                        txt(&s.increment.to_string()),
                        txt(if s.cycle { "YES" } else { "NO" }),
                    ]
                })
                .collect();
            rows.sort_by(|a, b| a[2].cmp(&b[2]));
            (
                vec![
                    "sequence_catalog",
                    "sequence_schema",
                    "sequence_name",
                    "data_type",
                    "numeric_precision",
                    "numeric_precision_radix",
                    "numeric_scale",
                    "start_value",
                    "minimum_value",
                    "maximum_value",
                    "increment",
                    "cycle_option",
                ],
                vec![
                    TEXT, TEXT, TEXT, TEXT, INT4, INT4, INT4, TEXT, TEXT, TEXT, TEXT, TEXT,
                ],
                rows,
            )
        }

        ("information_schema", "key_column_usage") => {
            let mut rows = Vec::new();
            for (tname, tab) in catalog.tables_iter() {
                let names = tab.schema.names();
                let col = |i: usize| names.get(i).cloned().unwrap_or_else(|| format!("col{i}"));
                for uk in &tab.constraints.uniques {
                    let cname = uk.name.clone().unwrap_or_else(|| {
                        format!("{tname}_{}", if uk.is_primary { "pkey" } else { "key" })
                    });
                    for (ord, &ci) in uk.cols.iter().enumerate() {
                        rows.push(vec![
                            txt("postcrust"),
                            txt("public"),
                            txt(&cname),
                            txt("postcrust"),
                            txt("public"),
                            txt(tname),
                            txt(&col(ci)),
                            SqlValue::Int((ord + 1) as i64),
                            SqlValue::Null,
                        ]);
                    }
                }
                for fk in &tab.constraints.foreign_keys {
                    let cname = fk.name.clone().unwrap_or_else(|| format!("{tname}_fkey"));
                    for (ord, &ci) in fk.cols.iter().enumerate() {
                        rows.push(vec![
                            txt("postcrust"),
                            txt("public"),
                            txt(&cname),
                            txt("postcrust"),
                            txt("public"),
                            txt(tname),
                            txt(&col(ci)),
                            SqlValue::Int((ord + 1) as i64),
                            SqlValue::Int((ord + 1) as i64),
                        ]);
                    }
                }
            }
            (
                vec![
                    "constraint_catalog",
                    "constraint_schema",
                    "constraint_name",
                    "table_catalog",
                    "table_schema",
                    "table_name",
                    "column_name",
                    "ordinal_position",
                    "position_in_unique_constraint",
                ],
                vec![TEXT, TEXT, TEXT, TEXT, TEXT, TEXT, TEXT, INT4, INT4],
                rows,
            )
        }

        ("information_schema", "referential_constraints") => {
            let rule = |a: crate::catalog::RefAction| match a {
                crate::catalog::RefAction::NoAction => "NO ACTION",
                crate::catalog::RefAction::Restrict => "RESTRICT",
                crate::catalog::RefAction::Cascade => "CASCADE",
                crate::catalog::RefAction::SetNull => "SET NULL",
            };
            let mut rows = Vec::new();
            for (tname, tab) in catalog.tables_iter() {
                for fk in &tab.constraints.foreign_keys {
                    let cname = fk.name.clone().unwrap_or_else(|| format!("{tname}_fkey"));

                    let uniq_name = catalog.get(&fk.parent).and_then(|pt| {
                        let want: std::collections::BTreeSet<usize> =
                            fk.parent_cols.iter().copied().collect();
                        pt.constraints
                            .uniques
                            .iter()
                            .find(|uk| {
                                uk.cols
                                    .iter()
                                    .copied()
                                    .collect::<std::collections::BTreeSet<_>>()
                                    == want
                            })
                            .map(|uk| {
                                uk.name.clone().unwrap_or_else(|| {
                                    format!(
                                        "{}_{}",
                                        fk.parent,
                                        if uk.is_primary { "pkey" } else { "key" }
                                    )
                                })
                            })
                    });
                    let uniq_cell = uniq_name.map(|n| txt(&n)).unwrap_or(SqlValue::Null);
                    rows.push(vec![
                        txt("postcrust"),
                        txt("public"),
                        txt(&cname),
                        txt("postcrust"),
                        txt("public"),
                        uniq_cell,
                        txt("NONE"),
                        txt(rule(fk.on_update)),
                        txt(rule(fk.on_delete)),
                    ]);
                }
            }
            (
                vec![
                    "constraint_catalog",
                    "constraint_schema",
                    "constraint_name",
                    "unique_constraint_catalog",
                    "unique_constraint_schema",
                    "unique_constraint_name",
                    "match_option",
                    "update_rule",
                    "delete_rule",
                ],
                vec![TEXT, TEXT, TEXT, TEXT, TEXT, TEXT, TEXT, TEXT, TEXT],
                rows,
            )
        }

        ("pg_catalog", "pg_tables") => {
            let mut rows = Vec::new();
            for (name, _) in catalog.tables_iter() {
                rows.push(vec![txt("public"), txt(name)]);
            }
            (vec!["schemaname", "tablename"], vec![TEXT, TEXT], rows)
        }

        ("pg_catalog", "pg_class") => {
            use crate::types::oid::FLOAT4;

            let mut rows = Vec::new();
            for (name, t) in catalog.tables_iter() {
                rows.push(vec![
                    txt(name),
                    txt("r"),
                    SqlValue::Real(t.reltuples),
                    SqlValue::Int(0),
                ]);
            }
            for (name, v) in catalog.views_iter() {
                let relkind = if v.materialized { "m" } else { "v" };
                rows.push(vec![
                    txt(name),
                    txt(relkind),
                    SqlValue::Real(-1.0),
                    SqlValue::Int(0),
                ]);
            }
            for idx in catalog.indexes_iter() {
                rows.push(vec![
                    txt(&idx.name),
                    txt("i"),
                    SqlValue::Real(-1.0),
                    SqlValue::Int(0),
                ]);
            }
            (
                vec!["relname", "relkind", "reltuples", "relpages"],
                vec![TEXT, TEXT, FLOAT4, INT4],
                rows,
            )
        }

        ("pg_catalog", "pg_collation") => {
            use crate::types::oid::BOOL;
            let rows = crate::collation::BUILTIN
                .iter()
                .map(|(name, _, _)| vec![txt(name), SqlValue::Int(1)])
                .collect();
            (
                vec!["collname", "collisdeterministic"],
                vec![TEXT, BOOL],
                rows,
            )
        }

        ("pg_catalog", "pg_indexes") => {
            let mut rows = Vec::new();
            for idx in catalog.indexes_iter() {
                let col_names: Vec<String> = catalog
                    .get(&idx.table)
                    .map(|t| {
                        let names = t.schema.names();
                        idx.cols
                            .iter()
                            .map(|&c| names.get(c).cloned().unwrap_or_else(|| format!("col{c}")))
                            .collect()
                    })
                    .unwrap_or_default();
                let indexdef = format!(
                    "CREATE {}INDEX {} ON public.{} USING btree ({})",
                    if idx.unique { "UNIQUE " } else { "" },
                    idx.name,
                    idx.table,
                    col_names.join(", "),
                );
                rows.push(vec![
                    txt("public"),
                    txt(&idx.table),
                    txt(&idx.name),
                    SqlValue::Null,
                    txt(&indexdef),
                ]);
            }
            (
                vec![
                    "schemaname",
                    "tablename",
                    "indexname",
                    "tablespace",
                    "indexdef",
                ],
                vec![TEXT, TEXT, TEXT, TEXT, TEXT],
                rows,
            )
        }

        ("pg_catalog", "pg_stats") => {
            use crate::types::oid::{BOOL, FLOAT4};

            let arr_vals = |col_oid: u32, vs: &[SqlValue]| -> SqlValue {
                if vs.is_empty() {
                    return SqlValue::Null;
                }
                let parts: Vec<String> = vs
                    .iter()
                    .map(|v| crate::types::output(col_oid, v))
                    .collect();
                SqlValue::Text(format!("{{{}}}", parts.join(",")))
            };
            let arr_freqs = |fs: &[f64]| -> SqlValue {
                if fs.is_empty() {
                    return SqlValue::Null;
                }
                let parts: Vec<String> = fs
                    .iter()
                    .map(|f| crate::types::output(FLOAT4, &SqlValue::Real(*f)))
                    .collect();
                SqlValue::Text(format!("{{{}}}", parts.join(",")))
            };
            let mut rows = Vec::new();
            for (name, t) in catalog.tables_iter() {
                let names = t.schema.names();
                for (&ci, cs) in &t.stats {
                    let col_oid = t.col_types.get(ci).copied().unwrap_or(0);
                    let attname = names.get(ci).cloned().unwrap_or_else(|| format!("col{ci}"));
                    rows.push(vec![
                        txt("public"),
                        txt(name),
                        txt(&attname),
                        SqlValue::Int(0),
                        SqlValue::Real(cs.null_frac),
                        SqlValue::Int(cs.avg_width as i64),
                        SqlValue::Real(cs.n_distinct),
                        arr_vals(col_oid, &cs.most_common_vals),
                        arr_freqs(&cs.most_common_freqs),
                        arr_vals(col_oid, &cs.histogram_bounds),
                        cs.correlation.map(SqlValue::Real).unwrap_or(SqlValue::Null),
                    ]);
                }
            }
            (
                vec![
                    "schemaname",
                    "tablename",
                    "attname",
                    "inherited",
                    "null_frac",
                    "avg_width",
                    "n_distinct",
                    "most_common_vals",
                    "most_common_freqs",
                    "histogram_bounds",
                    "correlation",
                ],
                vec![
                    TEXT, TEXT, TEXT, BOOL, FLOAT4, INT4, FLOAT4, TEXT, TEXT, TEXT, FLOAT4,
                ],
                rows,
            )
        }
        _ => {
            return Err(PgError::InvalidInputSyntax {
                typ: "query",
                input: format!("relation \"{schema}.{table}\" does not exist"),
            })
        }
    };

    let qual = alias.clone().unwrap_or_else(|| table.to_string());
    let out_schema = Schema::new(cols.iter().map(|s| s.to_string())).qualified(&qual);
    Ok((out_schema, oids, Plan::Scan(rows)))
}

fn eval_srf(
    name: &str,
    args: &[Expr],
    regs: &Arc<TypeRegistries>,
) -> Result<(Vec<String>, Vec<u32>, Vec<Row>), PgError> {
    let srf_err = |msg: String| PgError::InvalidInputSyntax {
        typ: "query",
        input: msg,
    };
    match name {
        "generate_series" => {
            if args.len() < 2 || args.len() > 3 {
                return Err(srf_err(
                    "generate_series(start, stop [, step]) expects 2 or 3 arguments".into(),
                ));
            }

            let ival = |e: &Expr| -> Result<i64, PgError> {
                match crate::expr::eval::eval_row(e, &[], EvalCtx::new(regs))? {
                    SqlValue::Int(n) => Ok(n),
                    SqlValue::Null => Err(srf_err("generate_series: NULL argument".into())),
                    other => Err(srf_err(format!(
                        "generate_series: non-integer argument {other:?}"
                    ))),
                }
            };
            let start = ival(&args[0])?;
            let stop = ival(&args[1])?;
            let step = if args.len() == 3 { ival(&args[2])? } else { 1 };
            if step == 0 {
                return Err(srf_err("step size cannot equal zero".into()));
            }
            let mut rows = Vec::new();
            let mut v = start;

            while (step > 0 && v <= stop) || (step < 0 && v >= stop) {
                rows.push(vec![SqlValue::Int(v)]);
                match v.checked_add(step) {
                    Some(n) => v = n,
                    None => break,
                }
            }
            Ok((
                vec!["generate_series".to_string()],
                vec![crate::types::oid::INT8],
                rows,
            ))
        }
        "json_each" | "jsonb_each" | "json_each_text" | "jsonb_each_text" => {
            eval_json_each(name, args, regs)
        }
        _ => Err(srf_err(format!(
            "set-returning function \"{name}\" does not exist"
        ))),
    }
}

fn eval_json_each(
    name: &str,
    args: &[Expr],
    regs: &Arc<TypeRegistries>,
) -> Result<(Vec<String>, Vec<u32>, Vec<Row>), PgError> {
    let srf_err = |msg: String| PgError::InvalidInputSyntax {
        typ: "query",
        input: msg,
    };
    if args.len() != 1 {
        return Err(srf_err(format!("{name}(json) expects 1 argument")));
    }
    let arg = crate::expr::eval::eval_row(&args[0], &[], EvalCtx::new(regs))?;
    let jsonb = name.starts_with("jsonb");
    let as_text = name.ends_with("_text");
    let pairs = crate::expr::functions::json_each_rows(name, &arg, jsonb, as_text)?;
    let value_oid = if as_text {
        crate::types::oid::TEXT
    } else if jsonb {
        crate::types::oid::JSONB
    } else {
        crate::types::oid::JSON
    };
    let rows = pairs
        .into_iter()
        .map(|(k, v)| vec![SqlValue::Text(k), v])
        .collect();
    Ok((
        vec!["key".to_string(), "value".to_string()],
        vec![crate::types::oid::TEXT, value_oid],
        rows,
    ))
}

fn srf_alias_cols(cols: Vec<String>, alias: &Option<String>) -> Vec<String> {
    if cols.len() == 1 {
        if let Some(a) = alias {
            return vec![a.clone()];
        }
    }
    cols
}

fn from_is_lateral(item: &FromItem) -> bool {
    matches!(
        item,
        FromItem::Subquery { lateral: true, .. } | FromItem::Function { lateral: true, .. }
    )
}

fn lateral_qual(item: &FromItem) -> String {
    match item {
        FromItem::Subquery { alias, .. } => alias.clone(),
        FromItem::Function { name, alias, .. } => alias.clone().unwrap_or_else(|| name.clone()),
        _ => String::new(),
    }
}

fn lateral_shape(
    right: &FromItem,
    ls: &Schema,
    catalog: &Catalog,
) -> Result<(Vec<String>, Vec<u32>), PgError> {
    match right {
        FromItem::Subquery {
            query,
            alias: _,
            lateral: _,
        } => {
            let null_row: Row = vec![SqlValue::Null; ls.width()];
            let sq = subst_outer_select(query, ls, &null_row, catalog)?;
            let r = run_select(&sq, catalog)?;
            Ok((r.columns, r.col_types))
        }
        FromItem::Function { name, alias, .. } => {

            match name.as_str() {
                "json_each" | "jsonb_each" | "json_each_text" | "jsonb_each_text" => {
                    let jsonb = name.starts_with("jsonb");
                    let as_text = name.ends_with("_text");
                    let value_oid = if as_text {
                        crate::types::oid::TEXT
                    } else if jsonb {
                        crate::types::oid::JSONB
                    } else {
                        crate::types::oid::JSON
                    };
                    Ok((
                        vec!["key".to_string(), "value".to_string()],
                        vec![crate::types::oid::TEXT, value_oid],
                    ))
                }
                _ => {
                    let col = alias.clone().unwrap_or_else(|| name.clone());
                    Ok((vec![col], vec![crate::types::oid::INT8]))
                }
            }
        }
        _ => Err(exec_err(
            "LATERAL source must be a subquery or set-returning function".into(),
        )),
    }
}

fn eval_lateral_right(
    right: &FromItem,
    ls: &Schema,
    orow: &Row,
    catalog: &Catalog,
) -> Result<(Vec<String>, Vec<u32>, Vec<Row>), PgError> {
    let regs = Arc::new(catalog.type_registries());
    let regs = &regs;
    match right {
        FromItem::Subquery {
            query,
            alias: _,
            lateral: _,
        } => {
            let sq = subst_outer_select(query, ls, orow, catalog)?;
            let r = run_select(&sq, catalog)?;
            Ok((r.columns, r.col_types, r.rows))
        }
        FromItem::Function {
            name,
            args,
            alias,
            lateral: _,
        } => {
            let sargs: Vec<Expr> = args
                .iter()
                .map(|a| subst_outer_expr(a, &Schema::default(), ls, orow, catalog))
                .collect::<Result<_, PgError>>()?;
            let (cols, oids, rows) = eval_srf(name, &sargs, regs)?;
            let cols = srf_alias_cols(cols, alias);
            Ok((cols, oids, rows))
        }

        _ => Err(exec_err(
            "LATERAL source must be a subquery or set-returning function".into(),
        )),
    }
}

fn plan_lateral_join(
    left: &FromItem,
    right: &FromItem,
    kind: super::ast::JoinKind,
    on: Option<&Expr>,
    catalog: &Catalog,
    regs: &Arc<TypeRegistries>,
) -> Result<(Schema, Vec<u32>, Plan), PgError> {
    let (ls, lt, lp) = plan_from(left, catalog, regs)?;
    let left_rows = lp.execute().map_err(exec_err)?;

    let (rcols, rtypes) = lateral_shape(right, &ls, catalog)?;
    let right_width = rcols.len();
    let right_schema = Schema::new(rcols).qualified(&lateral_qual(right));

    let out_schema = ls.clone().concat(right_schema);
    let mut out_types = lt.clone();
    out_types.extend(rtypes);

    let pred = match on {
        Some(e) => Some(lower_pred(e, &out_schema, regs.clone())?),
        None => None,
    };

    let keep_unmatched = matches!(kind, super::ast::JoinKind::Left);

    let mut out_rows: Vec<Row> = Vec::new();
    for lrow in &left_rows {
        let (_c, _t, rrows) = eval_lateral_right(right, &ls, lrow, catalog)?;
        let mut matched = false;
        for rrow in &rrows {
            let mut joined = lrow.clone();
            joined.extend(rrow.iter().cloned());
            if let Some(p) = &pred {
                if !p(&joined).map_err(exec_err)? {
                    continue;
                }
            }
            out_rows.push(joined);
            matched = true;
        }
        if !matched && keep_unmatched {

            let mut joined = lrow.clone();
            joined.extend(std::iter::repeat(SqlValue::Null).take(right_width));
            out_rows.push(joined);
        }
    }

    Ok((out_schema, out_types, Plan::Scan(out_rows)))
}

fn rewrite_row_to_json(
    expr: &Expr,
    schema: &Schema,
    col_oids: &[u32],
    regs: &Arc<TypeRegistries>,
) -> Expr {
    let Expr::Func { name, args, .. } = expr else {
        return expr.clone();
    };
    if name != "row_to_json" || args.len() != 1 {
        return expr.clone();
    }
    let Some(fields) = row_to_json_fields(&args[0], schema, col_oids, regs) else {
        return expr.clone();
    };
    let mut new_args = Vec::with_capacity(1 + fields.len() * 2);
    new_args.push(args[0].clone());
    for (fname, foid) in fields {
        new_args.push(Expr::Str(fname));
        new_args.push(Expr::Int(foid as i64));
    }
    Expr::Func {
        name: "%row_to_json".to_string(),
        args: new_args,
        distinct: false,
        filter: None,
        order_by: Vec::new(),
    }
}

fn row_to_json_fields(
    arg: &Expr,
    schema: &Schema,
    col_oids: &[u32],
    regs: &Arc<TypeRegistries>,
) -> Option<Vec<(String, u32)>> {

    if let Expr::Row(elems) = arg {
        return Some(
            elems
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    (
                        format!("f{}", i + 1),
                        crate::expr::infer::infer(e, schema, col_oids).unwrap_or(0),
                    )
                })
                .collect(),
        );
    }

    let oid = crate::expr::infer::infer(arg, schema, col_oids)?;
    let info = regs.composite(oid)?;
    Some(
        info.fields
            .iter()
            .map(|(n, o, _)| (n.clone(), *o))
            .collect(),
    )
}

fn is_select_srf_name(name: &str) -> bool {
    matches!(
        name,
        "generate_series"
            | "unnest"
            | "regexp_matches"
            | "regexp_split_to_table"
            | "jsonb_path_query"
            | "jsonb_object_keys"
            | "json_object_keys"
    )
}

fn select_srf_index(projection: &[SelectItem]) -> Option<usize> {
    projection.iter().position(|it| {
        matches!(it,
            SelectItem::Expr { expr: Expr::Func { name, .. }, .. } if is_select_srf_name(name))
    })
}

fn srf_col_oid(name: &str) -> u32 {
    match name {
        "generate_series" => crate::types::oid::INT8,

        "jsonb_path_query" => crate::types::oid::JSONB,

        _ => crate::types::oid::TEXT,
    }
}

enum SrfItem {

    Scalar(Expr),

    Srf { name: String, args: Vec<Expr> },
}

fn run_srf_select(
    s: &SelectStmt,
    schema: &Schema,
    col_oids: &[u32],
    plan: Plan,
    regs: &Arc<TypeRegistries>,
) -> Result<QueryResult, PgError> {

    let input_rows = plan.execute().map_err(exec_err)?;

    let mut items: Vec<SrfItem> = Vec::new();
    let mut columns: Vec<String> = Vec::new();
    let mut col_types: Vec<u32> = Vec::new();
    for it in s.projection.iter() {
        match it {
            SelectItem::Star => {
                for (i, name) in schema.names().iter().enumerate() {
                    items.push(SrfItem::Scalar(Expr::ColumnRef(i)));
                    columns.push(name.clone());
                    col_types.push(col_oids.get(i).copied().unwrap_or(0));
                }
            }
            SelectItem::Expr {
                expr: Expr::Func { name, args, .. },
                alias,
            } if is_select_srf_name(name) => {
                let rargs = args
                    .iter()
                    .map(|a| crate::expr::bind::resolve(a, schema))
                    .collect::<Result<Vec<_>, _>>()?;
                columns.push(alias.clone().unwrap_or_else(|| name.clone()));
                col_types.push(srf_col_oid(name));
                items.push(SrfItem::Srf {
                    name: name.clone(),
                    args: rargs,
                });
            }
            SelectItem::Expr { expr, alias } => {
                columns.push(alias.clone().unwrap_or_else(|| inferred_name(expr)));
                col_types.push(crate::expr::infer::infer(expr, schema, col_oids).unwrap_or(0));
                items.push(SrfItem::Scalar(crate::expr::bind::resolve(expr, schema)?));
            }
        }
    }

    let mut out_rows: Vec<Row> = Vec::new();
    for row in &input_rows {

        let mut scalars: Vec<SqlValue> = Vec::with_capacity(items.len());
        let mut generated: Vec<Vec<SqlValue>> = Vec::with_capacity(items.len());
        let mut maxlen = 0usize;
        for item in &items {
            match item {
                SrfItem::Scalar(e) => {
                    scalars.push(crate::expr::eval::eval_row(e, row, EvalCtx::new(regs))?);
                    generated.push(Vec::new());
                }
                SrfItem::Srf { name, args } => {
                    let argvals = args
                        .iter()
                        .map(|a| crate::expr::eval::eval_row(a, row, EvalCtx::new(regs)))
                        .collect::<Result<Vec<_>, _>>()?;
                    let vals = eval_select_srf(name, &argvals)?;
                    maxlen = maxlen.max(vals.len());
                    generated.push(vals);

                    scalars.push(SqlValue::Null);
                }
            }
        }
        for i in 0..maxlen {
            let mut out = Vec::with_capacity(items.len());
            for (col, item) in items.iter().enumerate() {
                match item {
                    SrfItem::Scalar(_) => out.push(scalars[col].clone()),

                    SrfItem::Srf { .. } => {
                        out.push(generated[col].get(i).cloned().unwrap_or(SqlValue::Null))
                    }
                }
            }
            out_rows.push(out);
        }
    }

    let out_schema = Schema::new(columns.clone());
    let mut plan2 = Plan::Scan(out_rows);
    if !s.order_by.is_empty() {
        let mut keys = Vec::with_capacity(s.order_by.len());
        for k in &s.order_by {
            let expr = match &k.expr {
                Expr::Int(n) if *n >= 1 && (*n as usize) <= out_schema.width() => {
                    Expr::ColumnRef(*n as usize - 1)
                }
                other => other.clone(),
            };
            let mut key = lower(&expr, &out_schema, regs.clone())?;

            let out_idx = match &expr {
                Expr::ColumnRef(i) => Some(*i),
                Expr::Column(name) => out_schema.index_of(name),
                _ => None,
            };
            if let Some(i) = out_idx {
                match col_types.get(i).copied() {
                    Some(crate::types::oid::NUMERIC) => key = numeric_sort_key(key),
                    Some(crate::types::oid::JSONB) => key = jsonb_sort_key(key),
                    Some(crate::types::oid::BPCHAR) => key = bpchar_sort_key(key),
                    Some(oid) if regs.composite(oid).is_some() => {
                        let fields = regs.composite(oid).unwrap().fields.clone();
                        key = composite_sort_key(key, fields, regs.clone());
                    }
                    _ => {}
                }
            }
            keys.push((
                key,
                SortOptions::with_default(
                    k.descending,
                    k.nulls_first,
                    NullsDefault::Postgres,
                    TextCollation::Binary,
                ),
            ));
        }
        plan2 = Plan::Sort {
            input: Box::new(plan2),
            keys,
        };
    }

    let mut rows = plan2.execute().map_err(exec_err)?;
    if s.distinct {
        rows = dedup_preserving_order_typed(rows, &col_types, regs);
    }
    let off = s.offset.max(0) as usize;
    rows = rows.into_iter().skip(off).collect();
    if let Some(n) = s.limit {
        rows.truncate(n.max(0) as usize);
    }
    Ok(QueryResult {
        columns,
        col_types,
        rows,
    })
}

fn eval_select_srf(name: &str, args: &[SqlValue]) -> Result<Vec<SqlValue>, PgError> {
    match name {
        "generate_series" => gen_series_values(args),
        "unnest" => unnest_values(args),
        "jsonb_path_query" => jsonb_path_query_values(args),
        _ => crate::expr::functions::call_srf(name, args).unwrap_or_else(|| {
            Err(srf_err(format!(
                "set-returning function \"{name}\" does not exist"
            )))
        }),
    }
}

fn srf_err(msg: String) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "query",
        input: msg,
    }
}

fn gen_series_values(args: &[SqlValue]) -> Result<Vec<SqlValue>, PgError> {
    if args.len() < 2 || args.len() > 3 {
        return Err(srf_err(
            "generate_series(start, stop [, step]) expects 2 or 3 arguments".into(),
        ));
    }
    let ival = |v: &SqlValue| -> Result<Option<i64>, PgError> {
        match v {
            SqlValue::Int(n) => Ok(Some(*n)),
            SqlValue::Null => Ok(None),
            other => Err(srf_err(format!(
                "generate_series: non-integer argument {other:?}"
            ))),
        }
    };
    let start = ival(&args[0])?;
    let stop = ival(&args[1])?;
    let step = if args.len() == 3 {
        ival(&args[2])?
    } else {
        Some(1)
    };
    let (Some(start), Some(stop), Some(step)) = (start, stop, step) else {
        return Ok(Vec::new());
    };
    if step == 0 {
        return Err(srf_err("step size cannot equal zero".into()));
    }
    let mut rows = Vec::new();
    let mut v = start;
    while (step > 0 && v <= stop) || (step < 0 && v >= stop) {
        rows.push(SqlValue::Int(v));
        match v.checked_add(step) {
            Some(n) => v = n,
            None => break,
        }
    }
    Ok(rows)
}

fn unnest_values(args: &[SqlValue]) -> Result<Vec<SqlValue>, PgError> {
    if args.len() != 1 {
        return Err(srf_err("unnest expects 1 argument".into()));
    }
    match &args[0] {
        SqlValue::Null => Ok(Vec::new()),
        SqlValue::Text(s) => parse_array_elements(s),
        other => Err(srf_err(format!(
            "unnest: argument is not an array: {other:?}"
        ))),
    }
}

fn jsonb_path_query_values(args: &[SqlValue]) -> Result<Vec<SqlValue>, PgError> {
    if args.len() != 2 {
        return Err(srf_err(
            "jsonb_path_query(target, path) expects 2 arguments".into(),
        ));
    }
    let (target, path) = match (&args[0], &args[1]) {
        (SqlValue::Text(t), SqlValue::Text(p)) => (t.as_str(), p.as_str()),
        (SqlValue::Null, _) | (_, SqlValue::Null) => return Ok(Vec::new()),
        _ => {
            return Err(srf_err(
                "jsonb_path_query: arguments must be jsonb and jsonpath text".into(),
            ))
        }
    };
    Ok(crate::expr::functions::jsonpath::query_all(target, path)?
        .into_iter()
        .map(SqlValue::Text)
        .collect())
}

fn parse_array_elements(text: &str) -> Result<Vec<SqlValue>, PgError> {
    let malformed = || srf_err(format!("malformed array literal: \"{text}\""));
    let cs: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    while i < cs.len() && cs[i].is_whitespace() {
        i += 1;
    }
    if cs.get(i) != Some(&'{') {
        return Err(malformed());
    }
    i += 1;
    let mut out = Vec::new();
    loop {
        while i < cs.len() && cs[i].is_whitespace() {
            i += 1;
        }
        match cs.get(i) {
            Some('}') => break,
            None => return Err(malformed()),
            _ => {}
        }

        if cs[i] == '"' {
            i += 1;
            let mut buf = String::new();
            loop {
                match cs.get(i) {
                    None => return Err(malformed()),
                    Some('\\') => {
                        i += 1;
                        match cs.get(i) {
                            Some(c) => {
                                buf.push(*c);
                                i += 1;
                            }
                            None => return Err(malformed()),
                        }
                    }
                    Some('"') => {
                        i += 1;
                        break;
                    }
                    Some(c) => {
                        buf.push(*c);
                        i += 1;
                    }
                }
            }
            out.push(SqlValue::Text(buf));
        } else {
            let start = i;
            let mut buf = String::new();
            let mut escaped = false;
            while let Some(c) = cs.get(i) {
                match c {
                    ',' | '}' | '{' => break,
                    '"' => return Err(malformed()),
                    '\\' => {
                        i += 1;
                        match cs.get(i) {
                            Some(c) => {
                                buf.push(*c);
                                escaped = true;
                                i += 1;
                            }
                            None => return Err(malformed()),
                        }
                    }
                    _ => {
                        buf.push(*c);
                        i += 1;
                    }
                }
            }
            if i == start {
                return Err(malformed());
            }
            let trimmed = buf.trim_end().to_string();
            if !escaped && trimmed.eq_ignore_ascii_case("NULL") {
                out.push(SqlValue::Null);
            } else {
                out.push(SqlValue::Text(trimmed));
            }
        }

        while i < cs.len() && cs[i].is_whitespace() {
            i += 1;
        }
        match cs.get(i) {
            Some(',') => i += 1,
            Some('}') => break,
            _ => return Err(malformed()),
        }
    }
    Ok(out)
}

fn resolve_named_windows(s: &SelectStmt) -> Result<SelectStmt, PgError> {
    if s.windows.is_empty() {

        for it in &s.projection {
            if let SelectItem::Expr { expr, .. } = it {
                check_no_window_ref(expr)?;
            }
        }
        for k in &s.order_by {
            check_no_window_ref(&k.expr)?;
        }
        return Ok(s.clone());
    }
    let defs = &s.windows;
    let mut out = s.clone();
    out.windows = Vec::new();
    out.projection = s
        .projection
        .iter()
        .map(|it| match it {
            SelectItem::Star => Ok(SelectItem::Star),
            SelectItem::Expr { expr, alias } => Ok(SelectItem::Expr {
                expr: resolve_win_expr(expr, defs)?,
                alias: alias.clone(),
            }),
        })
        .collect::<Result<Vec<_>, PgError>>()?;
    out.order_by = s
        .order_by
        .iter()
        .map(|k| {
            Ok(OrderKey {
                expr: resolve_win_expr(&k.expr, defs)?,
                descending: k.descending,
                nulls_first: k.nulls_first,
                comp_oid: k.comp_oid,
            })
        })
        .collect::<Result<Vec<_>, PgError>>()?;
    Ok(out)
}

fn check_no_window_ref(e: &Expr) -> Result<(), PgError> {
    if let Expr::Window {
        window_ref: Some(name),
        ..
    } = e
    {
        return Err(undefined_window(name));
    }
    for_each_child(e, &mut |c| check_no_window_ref(c))
}

fn undefined_window(name: &str) -> PgError {
    exec_err(format!("window \"{name}\" does not exist"))
}

fn resolve_win_expr(e: &Expr, defs: &[super::ast::NamedWindow]) -> Result<Expr, PgError> {
    if let Expr::Window {
        func,
        args,
        partition_by,
        order_by,
        frame,
        window_ref: Some(name),
    } = e
    {
        let def = defs
            .iter()
            .find(|d| &d.name == name)
            .ok_or_else(|| undefined_window(name))?;

        let mut merged_order = def.order_by.clone();
        merged_order.extend(order_by.iter().cloned());
        debug_assert!(partition_by.is_empty());
        return Ok(Expr::Window {
            func: func.clone(),
            args: args
                .iter()
                .map(|a| resolve_win_expr(a, defs))
                .collect::<Result<_, _>>()?,
            partition_by: def.partition_by.clone(),
            order_by: merged_order,
            frame: frame.clone(),
            window_ref: None,
        });
    }
    map_children(e, &mut |c| resolve_win_expr(c, defs))
}

fn for_each_child(
    e: &Expr,
    f: &mut dyn FnMut(&Expr) -> Result<(), PgError>,
) -> Result<(), PgError> {
    match e {
        Expr::Unary { expr, .. }
        | Expr::GenUnary { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::IsNull { expr, .. } => f(expr)?,
        Expr::Binary { left, right, .. } | Expr::GenBinary { left, right, .. } => {
            f(left)?;
            f(right)?;
        }
        Expr::Func { args, .. } | Expr::Window { args, .. } => {
            for a in args {
                f(a)?;
            }
        }
        Expr::Case {
            operand,
            whens,
            else_,
        } => {
            if let Some(o) = operand {
                f(o)?;
            }
            for (c, r) in whens {
                f(c)?;
                f(r)?;
            }
            if let Some(x) = else_ {
                f(x)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn map_children(
    e: &Expr,
    f: &mut dyn FnMut(&Expr) -> Result<Expr, PgError>,
) -> Result<Expr, PgError> {
    Ok(match e {
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: Box::new(f(expr)?),
        },
        Expr::Binary { op, left, right } => Expr::Binary {
            op: *op,
            left: Box::new(f(left)?),
            right: Box::new(f(right)?),
        },
        Expr::Func {
            name,
            args,
            distinct,
            filter,
            order_by,
        } => Expr::Func {
            name: name.clone(),
            args: args.iter().map(|a| f(a)).collect::<Result<_, _>>()?,
            distinct: *distinct,
            filter: match filter {
                Some(x) => Some(Box::new(f(x)?)),
                None => None,
            },
            order_by: order_by
                .iter()
                .map(|k| {
                    Ok(OrderKey {
                        expr: f(&k.expr)?,
                        descending: k.descending,
                        nulls_first: k.nulls_first,
                        comp_oid: k.comp_oid,
                    })
                })
                .collect::<Result<_, PgError>>()?,
        },
        Expr::GenBinary { op, left, right } => Expr::GenBinary {
            op: op.clone(),
            left: Box::new(f(left)?),
            right: Box::new(f(right)?),
        },
        Expr::GenUnary { op, expr } => Expr::GenUnary {
            op: op.clone(),
            expr: Box::new(f(expr)?),
        },
        Expr::Cast { expr, type_name } => Expr::Cast {
            expr: Box::new(f(expr)?),
            type_name: type_name.clone(),
        },
        Expr::IsNull { expr, negated } => Expr::IsNull {
            expr: Box::new(f(expr)?),
            negated: *negated,
        },
        Expr::Case {
            operand,
            whens,
            else_,
        } => Expr::Case {
            operand: operand.as_ref().map(|o| f(o).map(Box::new)).transpose()?,
            whens: whens
                .iter()
                .map(|(c, r)| Ok((f(c)?, f(r)?)))
                .collect::<Result<_, PgError>>()?,
            else_: else_.as_ref().map(|x| f(x).map(Box::new)).transpose()?,
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
            args: args.iter().map(|a| f(a)).collect::<Result<_, _>>()?,
            partition_by: partition_by.clone(),
            order_by: order_by.clone(),
            frame: frame.clone(),
            window_ref: window_ref.clone(),
        },
        _ => e.clone(),
    })
}

fn stmt_has_window(s: &SelectStmt) -> bool {
    let mut found = false;
    for it in &s.projection {
        if let SelectItem::Expr { expr, .. } = it {
            expr_has_window(expr, &mut found);
        }
    }
    for k in &s.order_by {
        expr_has_window(&k.expr, &mut found);
    }
    found
}

fn expr_has_window(e: &Expr, found: &mut bool) {
    match e {
        Expr::Window { .. } => *found = true,
        Expr::Unary { expr, .. }
        | Expr::GenUnary { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::IsNull { expr, .. } => expr_has_window(expr, found),
        Expr::Binary { left, right, .. } | Expr::GenBinary { left, right, .. } => {
            expr_has_window(left, found);
            expr_has_window(right, found);
        }
        Expr::Func { args, .. } => args.iter().for_each(|a| expr_has_window(a, found)),
        Expr::Case {
            operand,
            whens,
            else_,
        } => {
            if let Some(o) = operand {
                expr_has_window(o, found);
            }
            for (c, r) in whens {
                expr_has_window(c, found);
                expr_has_window(r, found);
            }
            if let Some(x) = else_ {
                expr_has_window(x, found);
            }
        }
        _ => {}
    }
}

fn collect_windows(e: &Expr, out: &mut Vec<Expr>) {
    match e {
        Expr::Window { .. } => {
            if !out.contains(e) {
                out.push(e.clone());
            }
        }
        Expr::Unary { expr, .. }
        | Expr::GenUnary { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::IsNull { expr, .. } => collect_windows(expr, out),
        Expr::Binary { left, right, .. } | Expr::GenBinary { left, right, .. } => {
            collect_windows(left, out);
            collect_windows(right, out);
        }
        Expr::Func { args, .. } => args.iter().for_each(|a| collect_windows(a, out)),
        Expr::Case {
            operand,
            whens,
            else_,
        } => {
            if let Some(o) = operand {
                collect_windows(o, out);
            }
            for (c, r) in whens {
                collect_windows(c, out);
                collect_windows(r, out);
            }
            if let Some(x) = else_ {
                collect_windows(x, out);
            }
        }
        _ => {}
    }
}

fn rewrite_windows(e: &Expr, windows: &[Expr], base: usize) -> Expr {
    if let Some(i) = windows.iter().position(|w| w == e) {
        return Expr::ColumnRef(base + i);
    }
    match e {
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: Box::new(rewrite_windows(expr, windows, base)),
        },
        Expr::Binary { op, left, right } => Expr::Binary {
            op: *op,
            left: Box::new(rewrite_windows(left, windows, base)),
            right: Box::new(rewrite_windows(right, windows, base)),
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
                .map(|a| rewrite_windows(a, windows, base))
                .collect(),
            distinct: *distinct,
            filter: filter
                .as_ref()
                .map(|x| Box::new(rewrite_windows(x, windows, base))),
            order_by: order_by
                .iter()
                .map(|k| OrderKey {
                    expr: rewrite_windows(&k.expr, windows, base),
                    descending: k.descending,
                    nulls_first: k.nulls_first,
                    comp_oid: k.comp_oid,
                })
                .collect(),
        },
        Expr::GenBinary { op, left, right } => Expr::GenBinary {
            op: op.clone(),
            left: Box::new(rewrite_windows(left, windows, base)),
            right: Box::new(rewrite_windows(right, windows, base)),
        },
        Expr::GenUnary { op, expr } => Expr::GenUnary {
            op: op.clone(),
            expr: Box::new(rewrite_windows(expr, windows, base)),
        },
        Expr::Cast { expr, type_name } => Expr::Cast {
            expr: Box::new(rewrite_windows(expr, windows, base)),
            type_name: type_name.clone(),
        },
        Expr::IsNull { expr, negated } => Expr::IsNull {
            expr: Box::new(rewrite_windows(expr, windows, base)),
            negated: *negated,
        },
        Expr::Case {
            operand,
            whens,
            else_,
        } => Expr::Case {
            operand: operand
                .as_ref()
                .map(|o| Box::new(rewrite_windows(o, windows, base))),
            whens: whens
                .iter()
                .map(|(c, r)| {
                    (
                        rewrite_windows(c, windows, base),
                        rewrite_windows(r, windows, base),
                    )
                })
                .collect(),
            else_: else_
                .as_ref()
                .map(|x| Box::new(rewrite_windows(x, windows, base))),
        },
        _ => e.clone(),
    }
}

fn apply_windows(
    s: &SelectStmt,
    schema: &Schema,
    col_oids: &[u32],
    input: Plan,
    catalog: &Catalog,
    regs: &Arc<TypeRegistries>,
) -> Result<(SelectStmt, Schema, Vec<u32>, Plan), PgError> {
    let _ = catalog;
    let mut windows = Vec::new();
    for it in &s.projection {
        if let SelectItem::Expr { expr, .. } = it {
            collect_windows(expr, &mut windows);
        }
    }
    for k in &s.order_by {
        collect_windows(&k.expr, &mut windows);
    }

    let mut rows = input.execute().map_err(exec_err)?;
    let base = schema.width();

    let mut ext_schema = schema.clone();
    let mut ext_oids = col_oids.to_vec();
    for (i, w) in windows.iter().enumerate() {
        let vals = compute_window(w, &rows, schema, regs)?;
        for (j, row) in rows.iter_mut().enumerate() {
            row.push(vals[j].clone());
        }
        ext_schema = ext_schema.concat(Schema::new([format!("__win{i}")]));
        ext_oids.push(window_result_oid(w, schema, col_oids));
    }

    let projection = s
        .projection
        .iter()
        .map(|it| match it {
            SelectItem::Star => SelectItem::Star,
            SelectItem::Expr { expr, alias } => SelectItem::Expr {
                expr: rewrite_windows(expr, &windows, base),
                alias: alias.clone(),
            },
        })
        .collect();
    let order_by = s
        .order_by
        .iter()
        .map(|k| OrderKey {
            expr: rewrite_windows(&k.expr, &windows, base),
            descending: k.descending,
            nulls_first: k.nulls_first,
            comp_oid: k.comp_oid,
        })
        .collect();

    let rewritten = SelectStmt {
        distinct: s.distinct,
        distinct_on: s.distinct_on.clone(),
        projection,
        from: None,
        filter: None,
        group_by: Vec::new(),
        grouping_sets: Vec::new(),
        having: None,
        order_by,
        limit: s.limit,
        offset: s.offset,
        windows: Vec::new(),
        tail: Vec::new(),
        locking: Vec::new(),
    };
    Ok((rewritten, ext_schema, ext_oids, Plan::Scan(rows)))
}

fn window_result_oid(w: &Expr, schema: &Schema, col_oids: &[u32]) -> u32 {
    if let Expr::Window { func, args, .. } = w {
        return match func.as_str() {
            "row_number" | "rank" | "dense_rank" | "count" | "ntile" => sql_core_oid_int8(),
            "avg" | "percent_rank" | "cume_dist" => crate::types::oid::FLOAT8,
            _ => args
                .first()
                .and_then(|a| crate::expr::infer::infer(a, schema, col_oids))
                .unwrap_or(0),
        };
    }
    0
}

fn sql_core_oid_int8() -> u32 {
    crate::types::oid::INT8
}

fn compute_window(
    w: &Expr,
    rows: &[Row],
    schema: &Schema,
    regs: &Arc<TypeRegistries>,
) -> Result<Vec<SqlValue>, PgError> {
    use crate::expr::ast::{FrameBound, FrameExclude, FrameMode};
    use crate::expr::{bind, eval};

    let Expr::Window {
        func,
        args,
        partition_by,
        order_by,
        frame,
        ..
    } = w
    else {
        return Ok(vec![SqlValue::Null; rows.len()]);
    };

    let part: Vec<Expr> = partition_by
        .iter()
        .map(|e| bind::resolve(e, schema))
        .collect::<Result<_, _>>()?;
    let ord: Vec<(Expr, bool, Option<bool>)> = order_by
        .iter()
        .map(|k| {
            Ok((
                bind::resolve(&k.expr, schema)?,
                k.descending,
                Some(k.nulls_first.unwrap_or(k.descending)),
            ))
        })
        .collect::<Result<Vec<_>, PgError>>()?;
    let arg: Vec<Expr> = args
        .iter()
        .map(|e| bind::resolve(e, schema))
        .collect::<Result<_, _>>()?;

    if let Some(f) = frame {
        let has_offset = matches!(f.start, FrameBound::Preceding(_) | FrameBound::Following(_))
            || matches!(f.end, FrameBound::Preceding(_) | FrameBound::Following(_));
        if matches!(f.mode, FrameMode::Range) && has_offset && ord.len() != 1 {
            return Err(exec_err(
                "RANGE with offset PRECEDING/FOLLOWING requires exactly one ORDER BY column".into(),
            ));
        }
        if matches!(f.mode, FrameMode::Groups) && ord.is_empty() {
            return Err(exec_err("GROUPS mode requires an ORDER BY clause".into()));
        }
    }

    let key_of = |i: usize| -> Result<Vec<SqlValue>, PgError> {
        part.iter()
            .map(|e| eval::eval_row(e, &rows[i], EvalCtx::new(regs)))
            .collect()
    };
    let mut partitions: Vec<(Vec<SqlValue>, Vec<usize>)> = Vec::new();
    for i in 0..rows.len() {
        let k = key_of(i)?;
        match partitions.iter_mut().find(|(pk, _)| keys_eq(pk, &k)) {
            Some((_, v)) => v.push(i),
            None => partitions.push((k, vec![i])),
        }
    }

    let mut out = vec![SqlValue::Null; rows.len()];
    for (_, idxs) in &mut partitions {

        let ord_key = |i: usize| -> Result<Vec<SqlValue>, PgError> {
            ord.iter()
                .map(|(e, _, _)| eval::eval_row(e, &rows[i], EvalCtx::new(regs)))
                .collect()
        };
        if !ord.is_empty() {
            let mut keyed: Vec<(usize, Vec<SqlValue>)> = idxs
                .iter()
                .map(|&i| Ok((i, ord_key(i)?)))
                .collect::<Result<_, PgError>>()?;
            keyed.sort_by(|a, b| {
                for (col, (_, desc, nf)) in ord.iter().enumerate() {
                    let o = sql_core::sort_cmp_nulls(&a.1[col], &b.1[col], *desc, *nf);
                    if o != std::cmp::Ordering::Equal {
                        return o;
                    }
                }
                std::cmp::Ordering::Equal
            });
            *idxs = keyed.into_iter().map(|(i, _)| i).collect();
        }

        let n = idxs.len();

        let peer_end = |p: usize| -> Result<usize, PgError> {
            if ord.is_empty() {
                return Ok(n - 1);
            }
            let kp = ord_key(idxs[p])?;
            let mut e = p;
            while e + 1 < n && keys_eq(&ord_key(idxs[e + 1])?, &kp) {
                e += 1;
            }
            Ok(e)
        };
        let peer_start = |p: usize| -> Result<usize, PgError> {
            if ord.is_empty() {
                return Ok(0);
            }
            let kp = ord_key(idxs[p])?;
            let mut s = p;
            while s > 0 && keys_eq(&ord_key(idxs[s - 1])?, &kp) {
                s -= 1;
            }
            Ok(s)
        };

        let mut group_ids = vec![0usize; n];
        if !ord.is_empty() {
            for q in 1..n {
                group_ids[q] = group_ids[q - 1]
                    + if keys_eq(&ord_key(idxs[q])?, &ord_key(idxs[q - 1])?) {
                        0
                    } else {
                        1
                    };
            }
        }

        let ord_f64 = |p: usize| -> Result<Option<f64>, PgError> {
            Ok(
                match eval::eval_row(&ord[0].0, &rows[idxs[p]], EvalCtx::new(regs))? {
                    SqlValue::Int(k) => Some(k as f64),
                    SqlValue::Real(f) => Some(f),
                    SqlValue::Null => None,
                    _ => {
                        return Err(exec_err(
                            "RANGE offset requires a numeric ORDER BY column".into(),
                        ))
                    }
                },
            )
        };

        let range_limit = |cv: f64, k: i64, following: bool| -> f64 {
            let desc = ord[0].1;
            match (desc, following) {
                (false, false) => cv - k as f64,
                (false, true) => cv + k as f64,
                (true, false) => cv + k as f64,
                (true, true) => cv - k as f64,
            }
        };

        let range_start = |p: usize, k: i64, following: bool| -> Result<i64, PgError> {
            let cv = match ord_f64(p)? {
                Some(v) => v,
                None => return Ok(peer_start(p)? as i64),
            };
            let limit = range_limit(cv, k, following);
            let desc = ord[0].1;
            for i in 0..n {
                if let Some(vi) = ord_f64(i)? {
                    if if desc { vi <= limit } else { vi >= limit } {
                        return Ok(i as i64);
                    }
                }
            }
            Ok(n as i64)
        };

        let range_end = |p: usize, k: i64, following: bool| -> Result<i64, PgError> {
            let cv = match ord_f64(p)? {
                Some(v) => v,
                None => return Ok(peer_end(p)? as i64),
            };
            let limit = range_limit(cv, k, following);
            let desc = ord[0].1;
            let mut hi = -1i64;
            for i in 0..n {
                if let Some(vi) = ord_f64(i)? {
                    if if desc { vi >= limit } else { vi <= limit } {
                        hi = i as i64;
                    }
                }
            }
            Ok(hi)
        };

        let group_start = |p: usize, delta: i64| -> i64 {
            let target = group_ids[p] as i64 + delta;
            for i in 0..n {
                if group_ids[i] as i64 >= target {
                    return i as i64;
                }
            }
            n as i64
        };
        let group_end = |p: usize, delta: i64| -> i64 {
            let target = group_ids[p] as i64 + delta;
            let mut hi = -1i64;
            for i in 0..n {
                if group_ids[i] as i64 <= target {
                    hi = i as i64;
                }
            }
            hi
        };

        let base_bounds = |p: usize| -> Result<Option<(usize, usize)>, PgError> {
            let (mode, start_b, end_b) = match frame {
                Some(f) => (f.mode, f.start, f.end),
                None => (
                    FrameMode::Range,
                    FrameBound::UnboundedPreceding,
                    FrameBound::CurrentRow,
                ),
            };
            let start_s: i64 = match start_b {
                FrameBound::UnboundedPreceding => 0,
                FrameBound::UnboundedFollowing => n as i64,
                FrameBound::CurrentRow => match mode {
                    FrameMode::Rows => p as i64,
                    FrameMode::Range | FrameMode::Groups => peer_start(p)? as i64,
                },
                FrameBound::Preceding(k) => match mode {
                    FrameMode::Rows => p as i64 - k,
                    FrameMode::Range => range_start(p, k, false)?,
                    FrameMode::Groups => group_start(p, -k),
                },
                FrameBound::Following(k) => match mode {
                    FrameMode::Rows => p as i64 + k,
                    FrameMode::Range => range_start(p, k, true)?,
                    FrameMode::Groups => group_start(p, k),
                },
            };
            let end_s: i64 = match end_b {
                FrameBound::UnboundedFollowing => n as i64 - 1,
                FrameBound::UnboundedPreceding => -1,
                FrameBound::CurrentRow => match mode {
                    FrameMode::Rows => p as i64,
                    FrameMode::Range | FrameMode::Groups => peer_end(p)? as i64,
                },
                FrameBound::Preceding(k) => match mode {
                    FrameMode::Rows => p as i64 - k,
                    FrameMode::Range => range_end(p, k, false)?,
                    FrameMode::Groups => group_end(p, -k),
                },
                FrameBound::Following(k) => match mode {
                    FrameMode::Rows => p as i64 + k,
                    FrameMode::Range => range_end(p, k, true)?,
                    FrameMode::Groups => group_end(p, k),
                },
            };
            let lo = start_s.max(0);
            let hi = end_s.min(n as i64 - 1);
            if lo > hi || lo >= n as i64 || hi < 0 {
                Ok(None)
            } else {
                Ok(Some((lo as usize, hi as usize)))
            }
        };

        let frame_positions = |p: usize| -> Result<Vec<usize>, PgError> {
            let (lo, hi) = match base_bounds(p)? {
                Some(x) => x,
                None => return Ok(Vec::new()),
            };
            let exclude = frame
                .as_ref()
                .map(|f| f.exclude)
                .unwrap_or(FrameExclude::NoOthers);
            let (ps, pe) = (peer_start(p)?, peer_end(p)?);
            let mut v = Vec::new();
            for i in lo..=hi {
                let in_peers = i >= ps && i <= pe;
                let keep = match exclude {
                    FrameExclude::NoOthers => true,
                    FrameExclude::CurrentRow => i != p,
                    FrameExclude::Group => !in_peers,
                    FrameExclude::Ties => !(in_peers && i != p),
                };
                if keep {
                    v.push(i);
                }
            }
            Ok(v)
        };

        for p in 0..n {
            let ri = idxs[p];
            let val = match func.as_str() {
                "row_number" => SqlValue::Int(p as i64 + 1),
                "rank" => {

                    let mut start = p;
                    while start > 0 && keys_eq(&ord_key(idxs[start - 1])?, &ord_key(ri)?) {
                        start -= 1;
                    }
                    SqlValue::Int(start as i64 + 1)
                }
                "dense_rank" => {

                    let mut groups = 1i64;
                    for q in 1..=p {
                        if !keys_eq(&ord_key(idxs[q])?, &ord_key(idxs[q - 1])?) {
                            groups += 1;
                        }
                    }
                    SqlValue::Int(groups)
                }
                "lag" | "lead" => {
                    let offset = match arg.get(1) {
                        Some(e) => match eval::eval_row(e, &rows[ri], EvalCtx::new(regs))? {
                            SqlValue::Int(k) => k,
                            _ => 1,
                        },
                        None => 1,
                    };
                    let target = if func == "lag" {
                        p as i64 - offset
                    } else {
                        p as i64 + offset
                    };
                    if target >= 0 && (target as usize) < n {
                        eval::eval_row(&arg[0], &rows[idxs[target as usize]], EvalCtx::new(regs))?
                    } else {

                        match arg.get(2) {
                            Some(e) => eval::eval_row(e, &rows[ri], EvalCtx::new(regs))?,
                            None => SqlValue::Null,
                        }
                    }
                }

                "first_value" => match frame_positions(p)?.first() {
                    Some(&pos) => eval::eval_row(&arg[0], &rows[idxs[pos]], EvalCtx::new(regs))?,
                    None => SqlValue::Null,
                },
                "last_value" => match frame_positions(p)?.last() {
                    Some(&pos) => eval::eval_row(&arg[0], &rows[idxs[pos]], EvalCtx::new(regs))?,
                    None => SqlValue::Null,
                },

                "percent_rank" => {

                    if n <= 1 {
                        SqlValue::Real(0.0)
                    } else {
                        let mut start = p;
                        while start > 0 && keys_eq(&ord_key(idxs[start - 1])?, &ord_key(ri)?) {
                            start -= 1;
                        }
                        SqlValue::Real(start as f64 / (n as f64 - 1.0))
                    }
                }
                "cume_dist" => {

                    let end = peer_end(p)?;
                    SqlValue::Real((end as f64 + 1.0) / n as f64)
                }
                "ntile" => {

                    let buckets = match eval::eval_row(&arg[0], &rows[ri], EvalCtx::new(regs))? {
                        SqlValue::Int(k) => k,
                        _ => 1,
                    };
                    if buckets <= 0 {
                        SqlValue::Null
                    } else {
                        let b = buckets as usize;
                        let base = n / b;
                        let rem = n % b;
                        let big = rem * (base + 1);
                        let bucket = if p < big {
                            p / (base + 1) + 1
                        } else {
                            rem + (p - big) / base + 1
                        };
                        SqlValue::Int(bucket as i64)
                    }
                }
                "nth_value" => {
                    let nth = match arg.get(1) {
                        Some(e) => match eval::eval_row(e, &rows[ri], EvalCtx::new(regs))? {
                            SqlValue::Int(k) => k,
                            _ => 1,
                        },
                        None => 1,
                    };
                    let fp = frame_positions(p)?;
                    match fp.get((nth as usize).wrapping_sub(1)) {
                        Some(&pos) if nth >= 1 => {
                            eval::eval_row(&arg[0], &rows[idxs[pos]], EvalCtx::new(regs))?
                        }
                        _ => SqlValue::Null,
                    }
                }

                _ => {
                    let frame_rows: Vec<Row> = frame_positions(p)?
                        .iter()
                        .map(|&pos| rows[idxs[pos]].clone())
                        .collect();
                    eval::eval_aggregate(
                        func,
                        &arg,
                        &frame_rows,
                        false,
                        None,
                        &[],
                        EvalCtx::new(regs),
                    )?
                }
            };
            out[ri] = val;
        }
    }
    Ok(out)
}

fn keys_eq(a: &[SqlValue], b: &[SqlValue]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| x.cmp(y) == std::cmp::Ordering::Equal)
}

fn resolve_stmt(s: &SelectStmt, catalog: &Catalog) -> Result<SelectStmt, PgError> {
    Ok(SelectStmt {
        distinct: s.distinct,
        distinct_on: s
            .distinct_on
            .iter()
            .map(|e| resolve_sub(e, catalog))
            .collect::<Result<_, _>>()?,
        projection: s
            .projection
            .iter()
            .map(|it| match it {
                SelectItem::Star => Ok(SelectItem::Star),
                SelectItem::Expr { expr, alias } => Ok(SelectItem::Expr {
                    expr: resolve_sub(expr, catalog)?,
                    alias: alias.clone(),
                }),
            })
            .collect::<Result<Vec<_>, PgError>>()?,
        from: match &s.from {
            Some(f) => Some(resolve_from(f, catalog)?),
            None => None,
        },
        filter: match &s.filter {
            Some(f) => Some(resolve_sub(f, catalog)?),
            None => None,
        },
        group_by: s
            .group_by
            .iter()
            .map(|e| resolve_sub(e, catalog))
            .collect::<Result<_, _>>()?,
        grouping_sets: s
            .grouping_sets
            .iter()
            .map(|set| {
                set.iter()
                    .map(|e| resolve_sub(e, catalog))
                    .collect::<Result<_, _>>()
            })
            .collect::<Result<_, _>>()?,
        having: match &s.having {
            Some(h) => Some(resolve_sub(h, catalog)?),
            None => None,
        },
        order_by: s
            .order_by
            .iter()
            .map(|k| {
                Ok(OrderKey {
                    expr: resolve_sub(&k.expr, catalog)?,
                    descending: k.descending,
                    nulls_first: k.nulls_first,
                    comp_oid: k.comp_oid,
                })
            })
            .collect::<Result<Vec<_>, PgError>>()?,
        limit: s.limit,
        offset: s.offset,
        windows: s.windows.clone(),
        tail: s.tail.clone(),
        locking: s.locking.clone(),
    })
}

fn resolve_from(f: &FromItem, catalog: &Catalog) -> Result<FromItem, PgError> {
    Ok(match f {
        FromItem::Table { .. } | FromItem::Subquery { .. } | FromItem::Function { .. } => f.clone(),
        FromItem::Join {
            left,
            right,
            kind,
            on,
            using,
            natural,
        } => FromItem::Join {
            left: Box::new(resolve_from(left, catalog)?),
            right: Box::new(resolve_from(right, catalog)?),
            kind: *kind,
            on: match on {
                Some(e) => Some(resolve_sub(e, catalog)?),
                None => None,
            },
            using: using.clone(),
            natural: *natural,
        },
    })
}

pub(crate) fn resolve_sub(e: &Expr, catalog: &Catalog) -> Result<Expr, PgError> {
    let rec = |x: &Expr| resolve_sub(x, catalog);
    Ok(match e {
        Expr::ScalarSubquery(q) => {
            let r = run_select(q, catalog)?;
            if r.columns.len() != 1 {
                return Err(PgError::InvalidInputSyntax {
                    typ: "query",
                    input: "subquery must return only one column".into(),
                });
            }
            match r.rows.len() {
                0 => Expr::Lit(SqlValue::Null),
                1 => Expr::Lit(r.rows[0][0].clone()),
                _ => {
                    return Err(PgError::InvalidInputSyntax {
                        typ: "query",
                        input: "more than one row returned by a subquery used as an expression"
                            .into(),
                    })
                }
            }
        }
        Expr::Exists { query, negated } => {
            let r = run_select(query, catalog)?;
            let exists = !r.rows.is_empty();
            Expr::Lit(SqlValue::Int(if exists != *negated { 1 } else { 0 }))
        }
        Expr::InSubquery {
            expr,
            query,
            negated,
        } => {
            let inner = rec(expr)?;
            let r = run_select(query, catalog)?;
            if r.columns.len() != 1 {
                return Err(PgError::InvalidInputSyntax {
                    typ: "query",
                    input: "subquery must return only one column".into(),
                });
            }
            if r.rows.is_empty() {
                Expr::Lit(SqlValue::Int(if *negated { 1 } else { 0 }))
            } else {
                let (cmp, join) = if *negated {
                    (BinOp::NotEq, BinOp::And)
                } else {
                    (BinOp::Eq, BinOp::Or)
                };
                let mk = |v: SqlValue| Expr::Binary {
                    op: cmp,
                    left: Box::new(inner.clone()),
                    right: Box::new(Expr::Lit(v)),
                };
                let mut it = r.rows.iter().map(|row| row[0].clone());
                let mut acc = mk(it.next().unwrap());
                for v in it {
                    acc = Expr::Binary {
                        op: join,
                        left: Box::new(acc),
                        right: Box::new(mk(v)),
                    };
                }
                acc
            }
        }
        Expr::Quantified {
            expr,
            op,
            quantifier,
            query,
        } => {
            let inner = rec(expr)?;
            let r = run_select(query, catalog)?;
            if r.columns.len() != 1 {
                return Err(PgError::InvalidInputSyntax {
                    typ: "query",
                    input: "subquery must return only one column".into(),
                });
            }

            let (empty, join) = match quantifier {
                crate::expr::ast::Quantifier::Any => (0, BinOp::Or),
                crate::expr::ast::Quantifier::All => (1, BinOp::And),
            };
            if r.rows.is_empty() {
                Expr::Lit(SqlValue::Int(empty))
            } else {
                let mk = |v: SqlValue| Expr::Binary {
                    op: *op,
                    left: Box::new(inner.clone()),
                    right: Box::new(Expr::Lit(v)),
                };
                let mut it = r.rows.iter().map(|row| row[0].clone());
                let mut acc = mk(it.next().unwrap());
                for v in it {
                    acc = Expr::Binary {
                        op: join,
                        left: Box::new(acc),
                        right: Box::new(mk(v)),
                    };
                }
                acc
            }
        }
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: Box::new(rec(expr)?),
        },
        Expr::Binary { op, left, right } => Expr::Binary {
            op: *op,
            left: Box::new(rec(left)?),
            right: Box::new(rec(right)?),
        },
        Expr::Func {
            name,
            args,
            distinct,
            filter,
            order_by,
        } => Expr::Func {
            name: name.clone(),
            args: args.iter().map(rec).collect::<Result<_, _>>()?,
            distinct: *distinct,
            filter: match filter {
                Some(x) => Some(Box::new(rec(x)?)),
                None => None,
            },
            order_by: order_by
                .iter()
                .map(|k| {
                    Ok(OrderKey {
                        expr: rec(&k.expr)?,
                        descending: k.descending,
                        nulls_first: k.nulls_first,
                        comp_oid: k.comp_oid,
                    })
                })
                .collect::<Result<_, PgError>>()?,
        },
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
        Expr::Case {
            operand,
            whens,
            else_,
        } => Expr::Case {
            operand: match operand {
                Some(o) => Some(Box::new(rec(o)?)),
                None => None,
            },
            whens: whens
                .iter()
                .map(|(c, r)| Ok((rec(c)?, rec(r)?)))
                .collect::<Result<_, PgError>>()?,
            else_: match else_ {
                Some(x) => Some(Box::new(rec(x)?)),
                None => None,
            },
        },
        _ => e.clone(),
    })
}

pub(crate) fn from_schema(item: &FromItem, catalog: &Catalog) -> Result<Schema, PgError> {
    let regs = Arc::new(catalog.type_registries());
    let regs = &regs;
    match item {
        FromItem::Table { name, alias } => {

            let (schema_q, bare) = match name.split_once('.') {
                Some((s, t)) => (Some(s), t),
                None => (None, name.as_str()),
            };
            if matches!(schema_q, Some("information_schema") | Some("pg_catalog")) {
                let (schema, _oids, _plan) =
                    synth_system_view(schema_q.unwrap(), bare, alias, catalog)?;
                return Ok(schema);
            }
            if let Some(t) = catalog.get(bare) {
                let qual = alias.clone().unwrap_or_else(|| bare.to_string());
                return Ok(t.schema.clone().qualified(&qual));
            }
            if let Some(v) = catalog.get_view(bare) {
                let qual = alias.clone().unwrap_or_else(|| bare.to_string());
                if v.materialized {
                    return Ok(Schema::new(v.mat_columns.clone()).qualified(&qual));
                }
                let r = run_select(&v.query, catalog)?;
                let (cols, _) = view_columns(v, &r);
                return Ok(Schema::new(cols).qualified(&qual));
            }
            Err(PgError::InvalidInputSyntax {
                typ: "query",
                input: format!("relation \"{name}\" does not exist"),
            })
        }
        FromItem::Join { left, right, .. } => {
            Ok(from_schema(left, catalog)?.concat(from_schema(right, catalog)?))
        }
        FromItem::Subquery {
            query,
            alias,
            lateral: _,
        } => {
            let r = run_select(query, catalog)?;
            Ok(Schema::new(r.columns).qualified(alias))
        }
        FromItem::Function {
            name,
            args,
            alias,
            lateral: _,
        } => {
            let (cols, _oids, _rows) = match regs.functions.get(name) {
                Some(fdef) if fdef.returns.is_setof() => {
                    eval_setof_func(fdef, args, alias, name, catalog)?
                }
                _ => eval_srf(name, args, regs)?,
            };
            let qual = alias.clone().unwrap_or_else(|| name.clone());
            let cols = srf_alias_cols(cols, alias);
            Ok(Schema::new(cols).qualified(&qual))
        }
    }
}

pub(crate) fn hashable_kind(kind: super::ast::JoinKind) -> Option<sql_core::JoinKind> {
    match kind {
        super::ast::JoinKind::Inner => Some(sql_core::JoinKind::Inner),
        super::ast::JoinKind::Left => Some(sql_core::JoinKind::Left),
        super::ast::JoinKind::Right | super::ast::JoinKind::Full | super::ast::JoinKind::Cross => {
            None
        }
    }
}

fn collect_conjuncts<'a>(e: &'a Expr, out: &mut Vec<&'a Expr>) {
    if let Expr::Binary {
        op: BinOp::And,
        left,
        right,
    } = e
    {
        collect_conjuncts(left, out);
        collect_conjuncts(right, out);
    } else {
        out.push(e);
    }
}

#[allow(clippy::type_complexity)]
fn split_equi_on(on: &Expr, left_width: usize) -> (Vec<(usize, usize)>, Option<Expr>) {
    let mut conjuncts = Vec::new();
    collect_conjuncts(on, &mut conjuncts);
    let mut keys: Vec<(usize, usize)> = Vec::new();
    let mut residual: Vec<&Expr> = Vec::new();
    for c in conjuncts {
        if let Expr::Binary {
            op: BinOp::Eq,
            left,
            right,
        } = c
        {
            if let (Expr::ColumnRef(a), Expr::ColumnRef(b)) = (left.as_ref(), right.as_ref()) {
                let (a, b) = (*a, *b);
                if a < left_width && b >= left_width {
                    keys.push((a, b));
                    continue;
                } else if b < left_width && a >= left_width {
                    keys.push((b, a));
                    continue;
                }
            }
        }
        residual.push(c);
    }
    let residual = residual.into_iter().cloned().reduce(|acc, e| Expr::Binary {
        op: BinOp::And,
        left: Box::new(acc),
        right: Box::new(e),
    });
    (keys, residual)
}

pub(crate) fn join_hashes(
    kind: super::ast::JoinKind,
    natural: bool,
    using: &[String],
    on: Option<&Expr>,
    ls: &Schema,
    rs: &Schema,
) -> Option<sql_core::JoinKind> {
    let core_kind = hashable_kind(kind)?;

    let join_cols_nonempty = if natural {
        let rnames = rs.names();
        ls.names().iter().any(|n| rnames.contains(n))
    } else {
        !using.is_empty()
    };
    if join_cols_nonempty {
        return Some(core_kind);
    }

    let on = on?;
    let raw = ls.clone().concat(rs.clone());
    let resolved = crate::expr::bind::resolve(on, &raw).ok()?;
    let (keys, _residual) = split_equi_on(&resolved, ls.width());
    if keys.is_empty() {
        None
    } else {
        Some(core_kind)
    }
}

fn stmt_is_correlated(s: &SelectStmt, outer: &Schema, catalog: &Catalog) -> Result<bool, PgError> {
    for it in &s.projection {
        if let SelectItem::Expr { expr, .. } = it {
            if expr_is_correlated(expr, outer, catalog)? {
                return Ok(true);
            }
        }
    }
    if let Some(f) = &s.filter {
        if expr_is_correlated(f, outer, catalog)? {
            return Ok(true);
        }
    }
    if let Some(h) = &s.having {
        if expr_is_correlated(h, outer, catalog)? {
            return Ok(true);
        }
    }
    for k in &s.order_by {
        if expr_is_correlated(&k.expr, outer, catalog)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn stmt_has_subquery(s: &SelectStmt) -> bool {
    let mut found = false;
    let mut visit = |e: &Expr| expr_has_subquery(e, &mut found);
    for it in &s.projection {
        if let SelectItem::Expr { expr, .. } = it {
            visit(expr);
        }
    }
    if let Some(f) = &s.filter {
        visit(f);
    }
    if let Some(h) = &s.having {
        visit(h);
    }
    for k in &s.order_by {
        visit(&k.expr);
    }
    found
}

fn expr_has_subquery(e: &Expr, found: &mut bool) {
    if *found {
        return;
    }
    match e {
        Expr::ScalarSubquery(_)
        | Expr::Exists { .. }
        | Expr::InSubquery { .. }
        | Expr::Quantified { .. } => *found = true,
        _ => {
            let _ = for_each_child(e, &mut |c| {
                expr_has_subquery(c, found);
                Ok(())
            });
        }
    }
}

pub(crate) fn expr_is_correlated(
    e: &Expr,
    outer: &Schema,
    catalog: &Catalog,
) -> Result<bool, PgError> {
    let rec = |x: &Expr| expr_is_correlated(x, outer, catalog);
    Ok(match e {
        Expr::ScalarSubquery(q) => subquery_is_correlated(q, outer, catalog)?,
        Expr::Exists { query, .. } => subquery_is_correlated(query, outer, catalog)?,
        Expr::InSubquery { expr, query, .. } => {
            rec(expr)? || subquery_is_correlated(query, outer, catalog)?
        }
        Expr::Quantified { expr, query, .. } => {
            rec(expr)? || subquery_is_correlated(query, outer, catalog)?
        }
        Expr::Unary { expr, .. }
        | Expr::GenUnary { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::IsNull { expr, .. } => rec(expr)?,
        Expr::Binary { left, right, .. } | Expr::GenBinary { left, right, .. } => {
            rec(left)? || rec(right)?
        }
        Expr::Func { args, .. } => {
            for a in args {
                if rec(a)? {
                    return Ok(true);
                }
            }
            false
        }
        Expr::Case {
            operand,
            whens,
            else_,
        } => {
            if let Some(o) = operand {
                if rec(o)? {
                    return Ok(true);
                }
            }
            for (c, r) in whens {
                if rec(c)? || rec(r)? {
                    return Ok(true);
                }
            }
            match else_ {
                Some(x) => rec(x)?,
                None => false,
            }
        }
        _ => false,
    })
}

fn subquery_is_correlated(
    q: &SelectStmt,
    outer: &Schema,
    catalog: &Catalog,
) -> Result<bool, PgError> {
    let inner = match &q.from {
        Some(f) => from_schema(f, catalog)?,
        None => Schema::default(),
    };
    let mut names = Vec::new();
    collect_stmt_columns(q, &mut names);
    Ok(names
        .iter()
        .any(|n| inner.index_of(n).is_none() && outer.index_of(n).is_some()))
}

fn collect_stmt_columns(q: &SelectStmt, out: &mut Vec<String>) {
    let mut visit = |e: &Expr| collect_expr_columns(e, out);
    for it in &q.projection {
        if let SelectItem::Expr { expr, .. } = it {
            visit(expr);
        }
    }
    if let Some(f) = &q.filter {
        visit(f);
    }
    for e in &q.group_by {
        visit(e);
    }
    if let Some(h) = &q.having {
        visit(h);
    }
    for k in &q.order_by {
        visit(&k.expr);
    }
}

fn collect_expr_columns(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Column(name) => out.push(name.clone()),

        Expr::ScalarSubquery(_) | Expr::Exists { .. } => {}
        Expr::InSubquery { expr, .. } => collect_expr_columns(expr, out),

        Expr::Quantified { expr, .. } => collect_expr_columns(expr, out),
        Expr::Unary { expr, .. }
        | Expr::GenUnary { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::IsNull { expr, .. } => collect_expr_columns(expr, out),
        Expr::Binary { left, right, .. } | Expr::GenBinary { left, right, .. } => {
            collect_expr_columns(left, out);
            collect_expr_columns(right, out);
        }
        Expr::Func { args, order_by, .. } => {
            args.iter().for_each(|a| collect_expr_columns(a, out));
            order_by
                .iter()
                .for_each(|k| collect_expr_columns(&k.expr, out));
        }
        Expr::Case {
            operand,
            whens,
            else_,
        } => {
            if let Some(o) = operand {
                collect_expr_columns(o, out);
            }
            for (c, r) in whens {
                collect_expr_columns(c, out);
                collect_expr_columns(r, out);
            }
            if let Some(x) = else_ {
                collect_expr_columns(x, out);
            }
        }
        _ => {}
    }
}

fn subst_outer_select(
    q: &SelectStmt,
    outer: &Schema,
    orow: &Row,
    catalog: &Catalog,
) -> Result<SelectStmt, PgError> {
    subst_outer_select_scoped(q, &Schema::default(), outer, orow, catalog)
}

fn subst_outer_select_scoped(
    q: &SelectStmt,
    keep: &Schema,
    outer: &Schema,
    orow: &Row,
    catalog: &Catalog,
) -> Result<SelectStmt, PgError> {
    let q_inner = match &q.from {
        Some(f) => from_schema(f, catalog)?,
        None => Schema::default(),
    };

    let inner = keep.clone().concat(q_inner);
    let sub = |e: &Expr| subst_outer_expr(e, &inner, outer, orow, catalog);
    Ok(SelectStmt {
        distinct: q.distinct,
        distinct_on: q
            .distinct_on
            .iter()
            .map(|e| sub(e))
            .collect::<Result<_, _>>()?,
        projection: q
            .projection
            .iter()
            .map(|it| match it {
                SelectItem::Star => Ok(SelectItem::Star),
                SelectItem::Expr { expr, alias } => Ok(SelectItem::Expr {
                    expr: sub(expr)?,
                    alias: alias.clone(),
                }),
            })
            .collect::<Result<_, PgError>>()?,
        from: q.from.clone(),
        filter: q.filter.as_ref().map(|f| sub(f)).transpose()?,
        group_by: q
            .group_by
            .iter()
            .map(|e| sub(e))
            .collect::<Result<_, _>>()?,
        grouping_sets: q
            .grouping_sets
            .iter()
            .map(|set| set.iter().map(|e| sub(e)).collect::<Result<_, _>>())
            .collect::<Result<_, _>>()?,
        having: q.having.as_ref().map(|h| sub(h)).transpose()?,
        order_by: q
            .order_by
            .iter()
            .map(|k| {
                Ok(OrderKey {
                    expr: sub(&k.expr)?,
                    descending: k.descending,
                    nulls_first: k.nulls_first,
                    comp_oid: k.comp_oid,
                })
            })
            .collect::<Result<_, PgError>>()?,
        limit: q.limit,
        offset: q.offset,
        windows: q.windows.clone(),

        tail: q
            .tail
            .iter()
            .map(|a| {
                Ok(super::ast::SetOpArm {
                    op: a.op,
                    all: a.all,
                    arm: subst_outer_select_scoped(&a.arm, keep, outer, orow, catalog)?,
                })
            })
            .collect::<Result<_, PgError>>()?,
        locking: Vec::new(),
    })
}

fn subst_outer_expr(
    e: &Expr,
    inner: &Schema,
    outer: &Schema,
    orow: &Row,
    catalog: &Catalog,
) -> Result<Expr, PgError> {
    let rec = |x: &Expr| subst_outer_expr(x, inner, outer, orow, catalog);

    let sub_nested = |q: &SelectStmt| subst_outer_select_scoped(q, inner, outer, orow, catalog);
    Ok(match e {
        Expr::Column(name) => {
            if inner.index_of(name).is_some() {
                e.clone()
            } else if let Some(i) = outer.index_of(name) {
                Expr::Lit(orow.get(i).cloned().unwrap_or(SqlValue::Null))
            } else {
                e.clone()
            }
        }
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: Box::new(rec(expr)?),
        },
        Expr::Binary { op, left, right } => Expr::Binary {
            op: *op,
            left: Box::new(rec(left)?),
            right: Box::new(rec(right)?),
        },
        Expr::Func {
            name,
            args,
            distinct,
            filter,
            order_by,
        } => Expr::Func {
            name: name.clone(),
            args: args.iter().map(|a| rec(a)).collect::<Result<_, _>>()?,
            distinct: *distinct,
            filter: match filter {
                Some(x) => Some(Box::new(rec(x)?)),
                None => None,
            },
            order_by: order_by
                .iter()
                .map(|k| {
                    Ok(OrderKey {
                        expr: rec(&k.expr)?,
                        descending: k.descending,
                        nulls_first: k.nulls_first,
                        comp_oid: k.comp_oid,
                    })
                })
                .collect::<Result<_, PgError>>()?,
        },
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
        Expr::Case {
            operand,
            whens,
            else_,
        } => Expr::Case {
            operand: operand.as_ref().map(|o| rec(o).map(Box::new)).transpose()?,
            whens: whens
                .iter()
                .map(|(c, r)| Ok((rec(c)?, rec(r)?)))
                .collect::<Result<_, PgError>>()?,
            else_: else_.as_ref().map(|x| rec(x).map(Box::new)).transpose()?,
        },

        Expr::ScalarSubquery(q) => Expr::ScalarSubquery(Box::new(sub_nested(q)?)),
        Expr::Exists { query, negated } => Expr::Exists {
            query: Box::new(sub_nested(query)?),
            negated: *negated,
        },
        Expr::InSubquery {
            expr,
            query,
            negated,
        } => Expr::InSubquery {
            expr: Box::new(rec(expr)?),
            query: Box::new(sub_nested(query)?),
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
            query: Box::new(sub_nested(query)?),
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
            args: args.iter().map(|a| rec(a)).collect::<Result<_, _>>()?,
            partition_by: partition_by
                .iter()
                .map(|a| rec(a))
                .collect::<Result<_, _>>()?,
            order_by: order_by
                .iter()
                .map(|k| {
                    Ok(OrderKey {
                        expr: rec(&k.expr)?,
                        descending: k.descending,
                        nulls_first: k.nulls_first,
                        comp_oid: k.comp_oid,
                    })
                })
                .collect::<Result<_, PgError>>()?,
            frame: frame.clone(),
            window_ref: window_ref.clone(),
        },

        _ => e.clone(),
    })
}

pub(crate) fn fold_correlated(
    e: &Expr,
    outer: &Schema,
    orow: &Row,
    catalog: &Catalog,
) -> Result<Expr, PgError> {
    let rec = |x: &Expr| fold_correlated(x, outer, orow, catalog);
    Ok(match e {
        Expr::ScalarSubquery(q) => {
            let q2 = subst_outer_select(q, outer, orow, catalog)?;
            resolve_sub(&Expr::ScalarSubquery(Box::new(q2)), catalog)?
        }
        Expr::Exists { query, negated } => {
            let q2 = subst_outer_select(query, outer, orow, catalog)?;
            resolve_sub(
                &Expr::Exists {
                    query: Box::new(q2),
                    negated: *negated,
                },
                catalog,
            )?
        }
        Expr::InSubquery {
            expr,
            query,
            negated,
        } => {
            let inner_expr = rec(expr)?;
            let q2 = subst_outer_select(query, outer, orow, catalog)?;
            resolve_sub(
                &Expr::InSubquery {
                    expr: Box::new(inner_expr),
                    query: Box::new(q2),
                    negated: *negated,
                },
                catalog,
            )?
        }
        Expr::Quantified {
            expr,
            op,
            quantifier,
            query,
        } => {

            let inner_expr = rec(expr)?;
            let q2 = subst_outer_select(query, outer, orow, catalog)?;
            resolve_sub(
                &Expr::Quantified {
                    expr: Box::new(inner_expr),
                    op: *op,
                    quantifier: *quantifier,
                    query: Box::new(q2),
                },
                catalog,
            )?
        }
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: Box::new(rec(expr)?),
        },
        Expr::Binary { op, left, right } => Expr::Binary {
            op: *op,
            left: Box::new(rec(left)?),
            right: Box::new(rec(right)?),
        },
        Expr::Func {
            name,
            args,
            distinct,
            filter,
            order_by,
        } => Expr::Func {
            name: name.clone(),
            args: args.iter().map(|a| rec(a)).collect::<Result<_, _>>()?,
            distinct: *distinct,
            filter: match filter {
                Some(x) => Some(Box::new(rec(x)?)),
                None => None,
            },
            order_by: order_by
                .iter()
                .map(|k| {
                    Ok(OrderKey {
                        expr: rec(&k.expr)?,
                        descending: k.descending,
                        nulls_first: k.nulls_first,
                        comp_oid: k.comp_oid,
                    })
                })
                .collect::<Result<_, PgError>>()?,
        },
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
        Expr::Case {
            operand,
            whens,
            else_,
        } => Expr::Case {
            operand: match operand {
                Some(o) => Some(Box::new(rec(o)?)),
                None => None,
            },
            whens: whens
                .iter()
                .map(|(c, r)| Ok((rec(c)?, rec(r)?)))
                .collect::<Result<_, PgError>>()?,
            else_: match else_ {
                Some(x) => Some(Box::new(rec(x)?)),
                None => None,
            },
        },
        _ => e.clone(),
    })
}

fn run_correlated(
    s: &SelectStmt,
    schema: &Schema,
    col_oids: &[u32],
    plan: Plan,
    catalog: &Catalog,
    regs: &Arc<TypeRegistries>,
) -> Result<QueryResult, PgError> {
    let outer_rows = plan.execute().map_err(exec_err)?;

    let mut kept: Vec<Row> = Vec::new();
    for orow in &outer_rows {
        if let Some(f) = &s.filter {
            let folded = fold_correlated(f, schema, orow, catalog)?;
            let folded = rewrite_bpchar_cmp(&folded, schema, col_oids);
            let pred = lower_pred(&folded, schema, regs.clone())?;
            if !pred(orow).map_err(exec_err)? {
                continue;
            }
        }
        kept.push(orow.clone());
    }

    if stmt_has_window(s) {
        return finish_correlated_windowed(s, schema, col_oids, kept, catalog, regs);
    }

    if !s.order_by.is_empty() {
        let mut keyed: Vec<(Vec<SqlValue>, Row)> = Vec::with_capacity(kept.len());
        for orow in kept.drain(..) {
            let mut key = Vec::with_capacity(s.order_by.len());
            for k in &s.order_by {
                let expr = order_key_expr(k, s, schema);
                let folded = fold_correlated(&expr, schema, &orow, catalog)?;
                key.push(lower(&folded, schema, regs.clone())?(&orow).map_err(exec_err)?);
            }
            keyed.push((key, orow));
        }
        keyed.sort_by(|a, b| {
            for (i, k) in s.order_by.iter().enumerate() {
                let o = sql_core::sort_cmp_nulls(
                    &a.0[i],
                    &b.0[i],
                    k.descending,
                    Some(k.nulls_first.unwrap_or(k.descending)),
                );
                if o != std::cmp::Ordering::Equal {
                    return o;
                }
            }
            std::cmp::Ordering::Equal
        });
        kept = keyed.into_iter().map(|(_, r)| r).collect();
    }

    let mut columns = Vec::new();
    let mut col_types = Vec::new();
    for item in &s.projection {
        match item {
            SelectItem::Star => {
                for (i, name) in schema.names().iter().enumerate() {
                    columns.push(name.clone());
                    col_types.push(col_oids.get(i).copied().unwrap_or(0));
                }
            }
            SelectItem::Expr { expr, alias } => {
                columns.push(alias.clone().unwrap_or_else(|| inferred_name(expr)));
                col_types.push(crate::expr::infer::infer(expr, schema, col_oids).unwrap_or(0));
            }
        }
    }

    let mut rows: Vec<Row> = Vec::with_capacity(kept.len());
    for orow in &kept {
        let mut row = Vec::with_capacity(columns.len());
        for item in &s.projection {
            match item {
                SelectItem::Star => {
                    for i in 0..schema.width() {
                        row.push(orow.get(i).cloned().unwrap_or(SqlValue::Null));
                    }
                }
                SelectItem::Expr { expr, .. } => {
                    let folded = fold_correlated(expr, schema, orow, catalog)?;
                    row.push(lower(&folded, schema, regs.clone())?(orow).map_err(exec_err)?);
                }
            }
        }
        rows.push(row);
    }

    if s.distinct {
        rows = dedup_preserving_order(rows);
    }
    let off = s.offset.max(0) as usize;
    rows = rows.into_iter().skip(off).collect();
    if let Some(n) = s.limit {
        rows.truncate(n.max(0) as usize);
    }
    Ok(QueryResult {
        columns,
        col_types,
        rows,
    })
}

fn finish_correlated_windowed(
    s: &SelectStmt,
    schema: &Schema,
    col_oids: &[u32],
    kept: Vec<Row>,
    catalog: &Catalog,
    regs: &Arc<TypeRegistries>,
) -> Result<QueryResult, PgError> {
    let base = schema.width();

    let mut subs: Vec<Expr> = Vec::new();
    let projection = s
        .projection
        .iter()
        .map(|it| match it {
            SelectItem::Star => SelectItem::Star,
            SelectItem::Expr { expr, alias } => SelectItem::Expr {
                expr: lift_subqueries(expr, &mut subs, base),
                alias: alias.clone(),
            },
        })
        .collect();
    let order_by = s
        .order_by
        .iter()
        .map(|k| OrderKey {
            expr: lift_subqueries(&k.expr, &mut subs, base),
            descending: k.descending,
            nulls_first: k.nulls_first,
            comp_oid: k.comp_oid,
        })
        .collect();

    let mut ext_oids = col_oids.to_vec();
    ext_oids.extend(std::iter::repeat(0).take(subs.len()));
    let mut rows: Vec<Row> = Vec::with_capacity(kept.len());
    for orow in &kept {
        let mut row = orow.clone();
        for (i, sub) in subs.iter().enumerate() {
            let folded = fold_correlated(sub, schema, orow, catalog)?;
            let oi = base + i;
            if ext_oids[oi] == 0 && !matches!(folded, Expr::Null | Expr::Lit(SqlValue::Null)) {
                if let Some(t) = crate::expr::infer::infer(&folded, schema, col_oids) {
                    ext_oids[oi] = t;
                }
            }
            row.push(lower(&folded, schema, regs.clone())?(orow).map_err(exec_err)?);
        }
        rows.push(row);
    }

    let mut ext_schema = schema.clone();
    for i in 0..subs.len() {
        ext_schema = ext_schema.concat(Schema::new([format!("__corr{i}")]));
    }
    let rewritten = SelectStmt {
        distinct: s.distinct,
        distinct_on: s.distinct_on.clone(),
        projection,
        from: None,
        filter: None,
        group_by: Vec::new(),
        having: None,
        order_by,
        limit: s.limit,
        offset: s.offset,
        windows: s.windows.clone(),
        tail: Vec::new(),
        locking: Vec::new(),
        grouping_sets: Vec::new(),
    };
    let rewritten = resolve_named_windows(&rewritten)?;
    finish_projection(
        &rewritten,
        &ext_schema,
        &ext_oids,
        Plan::Scan(rows),
        catalog,
        regs,
    )
}

fn lift_subqueries(e: &Expr, subs: &mut Vec<Expr>, base: usize) -> Expr {
    match e {
        Expr::ScalarSubquery(_)
        | Expr::Exists { .. }
        | Expr::InSubquery { .. }
        | Expr::Quantified { .. } => {
            let idx = subs.len();
            subs.push(e.clone());
            Expr::ColumnRef(base + idx)
        }
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: Box::new(lift_subqueries(expr, subs, base)),
        },
        Expr::Binary { op, left, right } => Expr::Binary {
            op: *op,
            left: Box::new(lift_subqueries(left, subs, base)),
            right: Box::new(lift_subqueries(right, subs, base)),
        },
        Expr::GenBinary { op, left, right } => Expr::GenBinary {
            op: op.clone(),
            left: Box::new(lift_subqueries(left, subs, base)),
            right: Box::new(lift_subqueries(right, subs, base)),
        },
        Expr::GenUnary { op, expr } => Expr::GenUnary {
            op: op.clone(),
            expr: Box::new(lift_subqueries(expr, subs, base)),
        },
        Expr::Cast { expr, type_name } => Expr::Cast {
            expr: Box::new(lift_subqueries(expr, subs, base)),
            type_name: type_name.clone(),
        },
        Expr::IsNull { expr, negated } => Expr::IsNull {
            expr: Box::new(lift_subqueries(expr, subs, base)),
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
            args: args
                .iter()
                .map(|a| lift_subqueries(a, subs, base))
                .collect(),
            distinct: *distinct,
            filter: filter
                .as_ref()
                .map(|x| Box::new(lift_subqueries(x, subs, base))),
            order_by: order_by.clone(),
        },
        Expr::Case {
            operand,
            whens,
            else_,
        } => Expr::Case {
            operand: operand
                .as_ref()
                .map(|o| Box::new(lift_subqueries(o, subs, base))),
            whens: whens
                .iter()
                .map(|(c, r)| {
                    (
                        lift_subqueries(c, subs, base),
                        lift_subqueries(r, subs, base),
                    )
                })
                .collect(),
            else_: else_
                .as_ref()
                .map(|x| Box::new(lift_subqueries(x, subs, base))),
        },

        _ => e.clone(),
    }
}

fn run_aggregate_correlated(
    s: &SelectStmt,
    schema: &Schema,
    col_oids: &[u32],
    input: Plan,
    catalog: &Catalog,
    regs: &Arc<TypeRegistries>,
) -> Result<QueryResult, PgError> {
    use crate::expr::{bind, eval, infer};

    let mut rows = input.execute().map_err(exec_err)?;

    if let Some(f) = &s.filter {
        let mut kept = Vec::with_capacity(rows.len());
        for row in rows.drain(..) {
            let folded = fold_correlated(f, schema, &row, catalog)?;
            let folded = rewrite_bpchar_cmp(&folded, schema, col_oids);
            let pred = lower_pred(&folded, schema, regs.clone())?;
            if pred(&row).map_err(exec_err)? {
                kept.push(row);
            }
        }
        rows = kept;
    }

    let key_scalars: Vec<Scalar> = s
        .group_by
        .iter()
        .map(|e| {
            let k = bind::lower(e, schema, regs.clone())?;
            Ok(group_key_wrap(k, infer::infer(e, schema, col_oids)))
        })
        .collect::<Result<_, PgError>>()?;
    let keyless = key_scalars.is_empty();

    let mut groups: Vec<(Vec<SqlValue>, Vec<Row>)> = Vec::new();
    if keyless {
        groups.push((Vec::new(), rows.clone()));
    } else {
        for row in &rows {
            let key: Vec<SqlValue> = key_scalars
                .iter()
                .map(|k| k(row).map_err(exec_err))
                .collect::<Result<_, _>>()?;
            match groups.iter_mut().find(|(gk, _)| keys_eq(gk, &key)) {
                Some((_, v)) => v.push(row.clone()),
                None => groups.push((key, vec![row.clone()])),
            }
        }
    }

    let mut columns: Vec<String> = Vec::new();
    let mut col_types: Vec<u32> = Vec::new();
    let mut orig: Vec<Expr> = Vec::new();
    for item in &s.projection {
        match item {
            SelectItem::Star => {
                return Err(PgError::InvalidInputSyntax {
                    typ: "query",
                    input: "SELECT * is not supported with GROUP BY / aggregates".into(),
                });
            }
            SelectItem::Expr { expr, alias } => {
                orig.push(expr.clone());
                columns.push(alias.clone().unwrap_or_else(|| inferred_name(expr)));
                col_types.push(infer::infer(expr, schema, col_oids).unwrap_or(0));
            }
        }
    }

    let null_row: Row = vec![SqlValue::Null; schema.width()];
    let mut out_rows: Vec<Row> = Vec::new();
    for (_, grp) in &groups {
        let rep: &Row = grp.first().unwrap_or(&null_row);
        if let Some(h) = &s.having {
            let folded = fold_correlated(h, schema, rep, catalog)?;
            let bound = bind::resolve(&folded, schema)?;
            match eval::eval_group(&bound, grp, EvalCtx::new(regs))? {
                SqlValue::Int(0) | SqlValue::Null => continue,
                _ => {}
            }
        }
        let mut row = Vec::with_capacity(orig.len());
        for e in &orig {
            let folded = fold_correlated(e, schema, rep, catalog)?;
            let bound = bind::resolve(&folded, schema)?;
            row.push(eval::eval_group(&bound, grp, EvalCtx::new(regs))?);
        }
        out_rows.push(row);
    }

    if !s.order_by.is_empty() {
        out_rows = order_output(out_rows, s, &columns, &orig, &col_types)?;
    }
    if s.distinct {
        out_rows = dedup_preserving_order(out_rows);
    }
    let off = s.offset.max(0) as usize;
    out_rows = out_rows.into_iter().skip(off).collect();
    if let Some(n) = s.limit {
        out_rows.truncate(n.max(0) as usize);
    }
    Ok(QueryResult {
        columns,
        col_types,
        rows: out_rows,
    })
}

fn run_aggregate_correlated_windowed(
    s: &SelectStmt,
    schema: &Schema,
    col_oids: &[u32],
    input: Plan,
    catalog: &Catalog,
    regs: &Arc<TypeRegistries>,
) -> Result<QueryResult, PgError> {
    use crate::expr::{bind, eval, infer};

    if !s.grouping_sets.is_empty() {
        return Err(PgError::InvalidInputSyntax {
            typ: "query",
            input: "window functions over GROUPING SETS / ROLLUP / CUBE are not supported".into(),
        });
    }
    if s.projection.iter().any(|it| matches!(it, SelectItem::Star)) {
        return Err(PgError::InvalidInputSyntax {
            typ: "query",
            input: "SELECT * is not supported with GROUP BY / aggregates".into(),
        });
    }

    let mut rows = input.execute().map_err(exec_err)?;

    if let Some(f) = &s.filter {
        let mut kept = Vec::with_capacity(rows.len());
        for row in rows.drain(..) {
            let folded = fold_correlated(f, schema, &row, catalog)?;
            let pred = lower_pred(&folded, schema, regs.clone())?;
            if pred(&row).map_err(exec_err)? {
                kept.push(row);
            }
        }
        rows = kept;
    }

    let mut bases: Vec<Expr> = s.group_by.clone();
    let mut aggs: Vec<Expr> = Vec::new();
    for it in &s.projection {
        if let SelectItem::Expr { expr, .. } = it {
            collect_aggregates(expr, &mut aggs);
        }
    }
    for k in &s.order_by {
        collect_aggregates(&k.expr, &mut aggs);
    }
    for e in &s.distinct_on {
        collect_aggregates(e, &mut aggs);
    }
    for a in aggs {
        if !bases.contains(&a) {
            bases.push(a);
        }
    }
    let nbases = bases.len();

    let mut int_names: Vec<String> = Vec::new();
    let mut int_oids: Vec<u32> = Vec::new();
    let mut resolved_bases: Vec<Expr> = Vec::new();
    for (i, b) in bases.iter().enumerate() {
        let mut rb = bind::resolve(b, schema)?;

        stamp_agg_order_keys(&mut rb, schema, col_oids, regs);
        resolved_bases.push(rb);
        int_oids.push(infer::infer(b, schema, col_oids).unwrap_or(0));
        int_names.push(match b {
            Expr::Column(n) => n.rsplit('.').next().unwrap_or(n).to_string(),
            _ => format!("__base{i}"),
        });
    }

    let mut subs: Vec<Expr> = Vec::new();
    let projection = s
        .projection
        .iter()
        .map(|it| match it {
            SelectItem::Star => SelectItem::Star,
            SelectItem::Expr { expr, alias } => {
                let name = alias.clone().unwrap_or_else(|| inferred_name(expr));
                let e = lift_subqueries(&rewrite_bases(expr, &bases), &mut subs, nbases);
                SelectItem::Expr {
                    expr: e,
                    alias: Some(name),
                }
            }
        })
        .collect();
    let order_by = s
        .order_by
        .iter()
        .map(|k| OrderKey {
            expr: lift_subqueries(&rewrite_bases(&k.expr, &bases), &mut subs, nbases),
            descending: k.descending,
            nulls_first: k.nulls_first,
            comp_oid: k.comp_oid,
        })
        .collect();
    let distinct_on = s
        .distinct_on
        .iter()
        .map(|e| lift_subqueries(&rewrite_bases(e, &bases), &mut subs, nbases))
        .collect();

    let key_scalars: Vec<Scalar> = s
        .group_by
        .iter()
        .map(|e| {
            let k = bind::lower(e, schema, regs.clone())?;
            Ok(
                if infer::infer(e, schema, col_oids) == Some(crate::types::oid::NUMERIC) {
                    numeric_sort_key(k)
                } else {
                    k
                },
            )
        })
        .collect::<Result<_, PgError>>()?;
    let keyless = key_scalars.is_empty();
    let mut groups: Vec<(Vec<SqlValue>, Vec<Row>)> = Vec::new();
    if keyless {
        groups.push((Vec::new(), rows.clone()));
    } else {
        for row in &rows {
            let key: Vec<SqlValue> = key_scalars
                .iter()
                .map(|k| k(row).map_err(exec_err))
                .collect::<Result<_, _>>()?;
            match groups.iter_mut().find(|(gk, _)| keys_eq(gk, &key)) {
                Some((_, v)) => v.push(row.clone()),
                None => groups.push((key, vec![row.clone()])),
            }
        }
    }

    let mut ext_oids = int_oids.clone();
    ext_oids.extend(std::iter::repeat(0).take(subs.len()));
    let null_row: Row = vec![SqlValue::Null; schema.width()];
    let mut int_rows: Vec<Row> = Vec::new();
    for (_, grp) in &groups {
        let rep: &Row = grp.first().unwrap_or(&null_row);
        if let Some(h) = &s.having {
            let folded = fold_correlated(h, schema, rep, catalog)?;
            let bound = bind::resolve(&folded, schema)?;
            match eval::eval_group(&bound, grp, EvalCtx::new(regs))? {
                SqlValue::Int(0) | SqlValue::Null => continue,
                _ => {}
            }
        }
        let mut row = Vec::with_capacity(nbases + subs.len());
        for e in &resolved_bases {
            row.push(eval::eval_group(e, grp, EvalCtx::new(regs))?);
        }
        for (i, sub) in subs.iter().enumerate() {
            let folded = fold_correlated(sub, schema, rep, catalog)?;
            let oi = nbases + i;
            if ext_oids[oi] == 0 && !matches!(folded, Expr::Null | Expr::Lit(SqlValue::Null)) {
                if let Some(t) = infer::infer(&folded, schema, col_oids) {
                    ext_oids[oi] = t;
                }
            }
            row.push(lower(&folded, schema, regs.clone())?(rep).map_err(exec_err)?);
        }
        int_rows.push(row);
    }

    let mut ext_names = int_names;
    for i in 0..subs.len() {
        ext_names.push(format!("__corr{i}"));
    }
    let ext_schema = Schema::new(ext_names);

    let rewritten = SelectStmt {
        distinct: s.distinct,
        distinct_on,
        projection,
        from: None,
        filter: None,
        group_by: Vec::new(),
        grouping_sets: Vec::new(),
        having: None,
        order_by,
        limit: s.limit,
        offset: s.offset,
        windows: s.windows.clone(),
        tail: Vec::new(),
        locking: Vec::new(),
    };
    let rewritten = resolve_named_windows(&rewritten)?;
    finish_projection(
        &rewritten,
        &ext_schema,
        &ext_oids,
        Plan::Scan(int_rows),
        catalog,
        regs,
    )
}

fn order_key_expr(k: &OrderKey, s: &SelectStmt, schema: &Schema) -> Expr {
    match &k.expr {
        Expr::Int(n) if *n >= 1 => match s.projection.get(*n as usize - 1) {
            Some(SelectItem::Expr { expr, .. }) => expr.clone(),
            _ => k.expr.clone(),
        },
        Expr::Column(name) if schema.index_of(name).is_none() => {
            let want = name.rsplit('.').next().unwrap_or(name);
            s.projection
                .iter()
                .find_map(|it| match it {
                    SelectItem::Expr {
                        expr,
                        alias: Some(a),
                    } if a == want => Some(expr.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| k.expr.clone())
        }
        _ => k.expr.clone(),
    }
}

fn finish_aggregate(
    s: &SelectStmt,
    schema: &Schema,
    col_oids: &[u32],
    input: Plan,
    regs: &Arc<TypeRegistries>,
) -> Result<QueryResult, PgError> {
    use crate::expr::{bind, eval, infer};

    if !s.grouping_sets.is_empty() {
        return finish_grouping_sets(s, schema, col_oids, input, regs);
    }

    let mut key: Vec<Scalar> = Vec::new();
    for e in &s.group_by {
        let mut k = bind::lower(e, schema, regs.clone())?;

        k = group_key_wrap(k, infer::infer(e, schema, col_oids));
        key.push(k);
    }

    let mut proj: Vec<Expr> = Vec::new();
    let mut proj_hints: Vec<Option<u32>> = Vec::new();
    let mut orig: Vec<Expr> = Vec::new();
    let mut columns: Vec<String> = Vec::new();
    let mut col_types: Vec<u32> = Vec::new();
    for item in &s.projection {
        match item {
            SelectItem::Star => {
                return Err(PgError::InvalidInputSyntax {
                    typ: "query",
                    input: "SELECT * is not supported with GROUP BY / aggregates".into(),
                });
            }
            SelectItem::Expr { expr, alias } => {

                let rexpr = rewrite_composite_cmp(expr, schema, col_oids, regs)?;
                let rexpr = rewrite_enum(&rexpr, schema, col_oids, regs);
                let mut bound = bind::resolve(&rexpr, schema)?;

                stamp_agg_order_keys(&mut bound, schema, col_oids, regs);

                proj_hints.push(composite_min_max_oid(&bound, schema, col_oids, regs));
                proj.push(bound);
                orig.push(expr.clone());
                columns.push(alias.clone().unwrap_or_else(|| inferred_name(expr)));
                col_types.push(infer::infer(&rexpr, schema, col_oids).unwrap_or(0));
            }
        }
    }
    let having = match &s.having {
        Some(h) => {
            let mut hb = bind::resolve(h, schema)?;
            stamp_agg_order_keys(&mut hb, schema, col_oids, regs);
            Some(hb)
        }
        None => None,
    };
    let having_hint = having
        .as_ref()
        .and_then(|h| composite_min_max_oid(h, schema, col_oids, regs));

    let cregs = regs.clone();
    let output: Box<dyn Fn(&[Row]) -> Result<Option<Row>, String>> =
        Box::new(move |grp: &[Row]| {
            if let Some(h) = &having {
                let ctx = EvalCtx::new(&cregs).with_comp_mm(having_hint);
                match eval::eval_group(h, grp, ctx).map_err(|e| e.message())? {
                    SqlValue::Int(0) | SqlValue::Null => return Ok(None),
                    _ => {}
                }
            }
            let mut row = Vec::with_capacity(proj.len());
            for (e, hint) in proj.iter().zip(proj_hints.iter()) {
                let ctx = EvalCtx::new(&cregs).with_comp_mm(*hint);
                row.push(eval::eval_group(e, grp, ctx).map_err(|er| er.message())?);
            }
            Ok(Some(row))
        });

    let plan = Plan::Aggregate {
        input: Box::new(input),
        key,
        output,
    };
    let mut rows = plan.execute().map_err(exec_err)?;

    if !s.order_by.is_empty() {
        rows = order_output(rows, s, &columns, &orig, &col_types)?;
    }
    if s.distinct {
        rows = dedup_preserving_order(rows);
    }
    let off = s.offset.max(0) as usize;
    rows = rows.into_iter().skip(off).collect();
    if let Some(n) = s.limit {
        rows.truncate(n.max(0) as usize);
    }
    Ok(QueryResult {
        columns,
        col_types,
        rows,
    })
}

fn finish_grouping_sets(
    s: &SelectStmt,
    schema: &Schema,
    col_oids: &[u32],
    input: Plan,
    regs: &Arc<TypeRegistries>,
) -> Result<QueryResult, PgError> {
    use crate::expr::{bind, eval, infer};

    let mut columns: Vec<String> = Vec::new();
    let mut col_types: Vec<u32> = Vec::new();
    let mut orig: Vec<Expr> = Vec::new();
    for item in &s.projection {
        match item {
            SelectItem::Star => {
                return Err(PgError::InvalidInputSyntax {
                    typ: "query",
                    input: "SELECT * is not supported with GROUP BY / aggregates".into(),
                });
            }
            SelectItem::Expr { expr, alias } => {
                columns.push(alias.clone().unwrap_or_else(|| inferred_name(expr)));
                col_types.push(infer::infer(expr, schema, col_oids).unwrap_or(0));
                orig.push(expr.clone());
            }
        }
    }

    let mut universe: Vec<Expr> = Vec::new();
    for set in &s.grouping_sets {
        for e in set {
            if !universe.contains(e) {
                universe.push(e.clone());
            }
        }
    }

    let base_rows = input.execute().map_err(exec_err)?;

    let mut all_rows: Vec<Row> = Vec::new();
    for set in &s.grouping_sets {

        let mut key: Vec<Scalar> = Vec::new();
        for e in set {
            let k = bind::lower(e, schema, regs.clone())?;
            let k = group_key_wrap(k, infer::infer(e, schema, col_oids));
            key.push(k);
        }

        let mut proj: Vec<Expr> = Vec::with_capacity(orig.len());
        for e in &orig {
            let mut b = bind::resolve(&rewrite_for_set(e, set, &universe), schema)?;
            stamp_agg_order_keys(&mut b, schema, col_oids, regs);
            proj.push(b);
        }
        let having = match &s.having {
            Some(h) => {
                let mut hb = bind::resolve(&rewrite_for_set(h, set, &universe), schema)?;
                stamp_agg_order_keys(&mut hb, schema, col_oids, regs);
                Some(hb)
            }
            None => None,
        };

        let cregs = regs.clone();
        let output: Box<dyn Fn(&[Row]) -> Result<Option<Row>, String>> =
            Box::new(move |grp: &[Row]| {
                if let Some(h) = &having {
                    match eval::eval_group(h, grp, EvalCtx::new(&cregs)).map_err(|e| e.message())? {
                        SqlValue::Int(0) | SqlValue::Null => return Ok(None),
                        _ => {}
                    }
                }
                let mut row = Vec::with_capacity(proj.len());
                for e in &proj {
                    row.push(
                        eval::eval_group(e, grp, EvalCtx::new(&cregs))
                            .map_err(|er| er.message())?,
                    );
                }
                Ok(Some(row))
            });

        let plan = Plan::Aggregate {
            input: Box::new(Plan::Scan(base_rows.clone())),
            key,
            output,
        };
        all_rows.extend(plan.execute().map_err(exec_err)?);
    }

    let mut rows = all_rows;
    if !s.order_by.is_empty() {
        rows = order_output(rows, s, &columns, &orig, &col_types)?;
    }
    if s.distinct {
        rows = dedup_preserving_order(rows);
    }
    let off = s.offset.max(0) as usize;
    rows = rows.into_iter().skip(off).collect();
    if let Some(n) = s.limit {
        rows.truncate(n.max(0) as usize);
    }
    Ok(QueryResult {
        columns,
        col_types,
        rows,
    })
}

fn finish_grouping_sets_windowed(
    s: &SelectStmt,
    schema: &Schema,
    col_oids: &[u32],
    input: Plan,
    catalog: &Catalog,
    regs: &Arc<TypeRegistries>,
) -> Result<QueryResult, PgError> {
    use crate::expr::{bind, eval, infer};

    if s.projection.iter().any(|it| matches!(it, SelectItem::Star)) {
        return Err(PgError::InvalidInputSyntax {
            typ: "query",
            input: "SELECT * is not supported with GROUP BY / aggregates".into(),
        });
    }

    let mut universe: Vec<Expr> = Vec::new();
    for set in &s.grouping_sets {
        for e in set {
            if !universe.contains(e) {
                universe.push(e.clone());
            }
        }
    }

    let mut bases: Vec<Expr> = universe.clone();
    let mut aggs: Vec<Expr> = Vec::new();
    let mut groupings: Vec<Expr> = Vec::new();
    for it in &s.projection {
        if let SelectItem::Expr { expr, .. } = it {
            collect_aggregates(expr, &mut aggs);
            collect_grouping_calls(expr, &mut groupings);
        }
    }
    for k in &s.order_by {
        collect_aggregates(&k.expr, &mut aggs);
        collect_grouping_calls(&k.expr, &mut groupings);
    }
    for e in &s.distinct_on {
        collect_aggregates(e, &mut aggs);
        collect_grouping_calls(e, &mut groupings);
    }
    for a in aggs.into_iter().chain(groupings) {
        if !bases.contains(&a) {
            bases.push(a);
        }
    }

    let mut int_names: Vec<String> = Vec::new();
    let mut int_oids: Vec<u32> = Vec::new();
    for (i, b) in bases.iter().enumerate() {
        int_names.push(match b {
            Expr::Column(n) => n.rsplit('.').next().unwrap_or(n).to_string(),
            _ => format!("__base{i}"),
        });
        let oid = if matches!(b, Expr::Func { name, .. } if name.eq_ignore_ascii_case("grouping")) {
            crate::types::oid::INT4
        } else {
            infer::infer(b, schema, col_oids).unwrap_or(0)
        };
        int_oids.push(oid);
    }

    let base_rows = input.execute().map_err(exec_err)?;
    let mut rows: Vec<Row> = Vec::new();
    for set in &s.grouping_sets {
        let mut key: Vec<Scalar> = Vec::new();
        for e in set {
            let mut k = bind::lower(e, schema, regs.clone())?;
            if infer::infer(e, schema, col_oids) == Some(crate::types::oid::NUMERIC) {
                k = numeric_sort_key(k);
            }
            key.push(k);
        }

        let mut resolved: Vec<Expr> = Vec::with_capacity(bases.len());
        for b in &bases {
            resolved.push(bind::resolve(&rewrite_for_set(b, set, &universe), schema)?);
        }
        let having = match &s.having {
            Some(h) => Some(bind::resolve(&rewrite_for_set(h, set, &universe), schema)?),
            None => None,
        };
        let cregs = regs.clone();
        let output: Box<dyn Fn(&[Row]) -> Result<Option<Row>, String>> =
            Box::new(move |grp: &[Row]| {
                if let Some(h) = &having {
                    match eval::eval_group(h, grp, EvalCtx::new(&cregs)).map_err(|e| e.message())? {
                        SqlValue::Int(0) | SqlValue::Null => return Ok(None),
                        _ => {}
                    }
                }
                let mut row = Vec::with_capacity(resolved.len());
                for e in &resolved {
                    row.push(
                        eval::eval_group(e, grp, EvalCtx::new(&cregs))
                            .map_err(|er| er.message())?,
                    );
                }
                Ok(Some(row))
            });
        let plan = Plan::Aggregate {
            input: Box::new(Plan::Scan(base_rows.clone())),
            key,
            output,
        };
        rows.extend(plan.execute().map_err(exec_err)?);
    }
    let int_schema = Schema::new(int_names);

    let projection = s
        .projection
        .iter()
        .map(|it| match it {
            SelectItem::Star => SelectItem::Star,
            SelectItem::Expr { expr, alias } => {
                let name = alias.clone().unwrap_or_else(|| inferred_name(expr));
                SelectItem::Expr {
                    expr: rewrite_bases(expr, &bases),
                    alias: Some(name),
                }
            }
        })
        .collect();
    let order_by = s
        .order_by
        .iter()
        .map(|k| OrderKey {
            expr: rewrite_bases(&k.expr, &bases),
            descending: k.descending,
            nulls_first: k.nulls_first,
            comp_oid: k.comp_oid,
        })
        .collect();
    let distinct_on = s
        .distinct_on
        .iter()
        .map(|e| rewrite_bases(e, &bases))
        .collect();

    let rewritten = SelectStmt {
        distinct: s.distinct,
        distinct_on,
        projection,
        from: None,
        filter: None,
        group_by: Vec::new(),
        grouping_sets: Vec::new(),
        having: None,
        order_by,
        limit: s.limit,
        offset: s.offset,
        windows: Vec::new(),
        tail: Vec::new(),
        locking: Vec::new(),
    };

    finish_projection(
        &rewritten,
        &int_schema,
        &int_oids,
        Plan::Scan(rows),
        catalog,
        regs,
    )
}

fn collect_grouping_calls(e: &Expr, out: &mut Vec<Expr>) {
    match e {
        Expr::Func { name, .. } if name.eq_ignore_ascii_case("grouping") => {
            if !out.contains(e) {
                out.push(e.clone());
            }
        }
        Expr::Func { args, .. } => args.iter().for_each(|a| collect_grouping_calls(a, out)),
        Expr::Unary { expr, .. }
        | Expr::GenUnary { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::IsNull { expr, .. } => collect_grouping_calls(expr, out),
        Expr::Binary { left, right, .. } | Expr::GenBinary { left, right, .. } => {
            collect_grouping_calls(left, out);
            collect_grouping_calls(right, out);
        }
        Expr::Case {
            operand,
            whens,
            else_,
        } => {
            if let Some(o) = operand {
                collect_grouping_calls(o, out);
            }
            for (c, r) in whens {
                collect_grouping_calls(c, out);
                collect_grouping_calls(r, out);
            }
            if let Some(x) = else_ {
                collect_grouping_calls(x, out);
            }
        }
        Expr::Window {
            args,
            partition_by,
            order_by,
            ..
        } => {
            args.iter().for_each(|a| collect_grouping_calls(a, out));
            partition_by
                .iter()
                .for_each(|a| collect_grouping_calls(a, out));
            order_by
                .iter()
                .for_each(|k| collect_grouping_calls(&k.expr, out));
        }
        _ => {}
    }
}

fn rewrite_for_set(e: &Expr, set: &[Expr], universe: &[Expr]) -> Expr {
    use crate::expr::eval::is_aggregate_name;
    if let Expr::Func { name, args, .. } = e {
        if name.eq_ignore_ascii_case("grouping") {
            let mut mask: i64 = 0;
            for a in args {
                mask <<= 1;
                if !set.iter().any(|k| k == a) {
                    mask |= 1;
                }
            }
            return Expr::Int(mask);
        }
        if is_aggregate_name(name) {
            return e.clone();
        }
    }
    if universe.iter().any(|u| u == e) && !set.iter().any(|k| k == e) {
        return Expr::Null;
    }
    match e {
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: Box::new(rewrite_for_set(expr, set, universe)),
        },
        Expr::Binary { op, left, right } => Expr::Binary {
            op: *op,
            left: Box::new(rewrite_for_set(left, set, universe)),
            right: Box::new(rewrite_for_set(right, set, universe)),
        },
        Expr::GenBinary { op, left, right } => Expr::GenBinary {
            op: op.clone(),
            left: Box::new(rewrite_for_set(left, set, universe)),
            right: Box::new(rewrite_for_set(right, set, universe)),
        },
        Expr::GenUnary { op, expr } => Expr::GenUnary {
            op: op.clone(),
            expr: Box::new(rewrite_for_set(expr, set, universe)),
        },
        Expr::Cast { expr, type_name } => Expr::Cast {
            expr: Box::new(rewrite_for_set(expr, set, universe)),
            type_name: type_name.clone(),
        },
        Expr::IsNull { expr, negated } => Expr::IsNull {
            expr: Box::new(rewrite_for_set(expr, set, universe)),
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
            args: args
                .iter()
                .map(|a| rewrite_for_set(a, set, universe))
                .collect(),
            distinct: *distinct,
            filter: filter.clone(),
            order_by: order_by.clone(),
        },
        Expr::Case {
            operand,
            whens,
            else_,
        } => Expr::Case {
            operand: operand
                .as_ref()
                .map(|o| Box::new(rewrite_for_set(o, set, universe))),
            whens: whens
                .iter()
                .map(|(c, r)| {
                    (
                        rewrite_for_set(c, set, universe),
                        rewrite_for_set(r, set, universe),
                    )
                })
                .collect(),
            else_: else_
                .as_ref()
                .map(|x| Box::new(rewrite_for_set(x, set, universe))),
        },
        _ => e.clone(),
    }
}

fn collect_aggregates(e: &Expr, out: &mut Vec<Expr>) {
    use crate::expr::eval::is_aggregate_name;
    match e {
        Expr::Func { name, .. } if is_aggregate_name(name) => {
            if !out.contains(e) {
                out.push(e.clone());
            }
        }
        Expr::Func { args, .. } => args.iter().for_each(|a| collect_aggregates(a, out)),
        Expr::Unary { expr, .. }
        | Expr::GenUnary { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::IsNull { expr, .. } => collect_aggregates(expr, out),
        Expr::Binary { left, right, .. } | Expr::GenBinary { left, right, .. } => {
            collect_aggregates(left, out);
            collect_aggregates(right, out);
        }
        Expr::Case {
            operand,
            whens,
            else_,
        } => {
            if let Some(o) = operand {
                collect_aggregates(o, out);
            }
            for (c, r) in whens {
                collect_aggregates(c, out);
                collect_aggregates(r, out);
            }
            if let Some(x) = else_ {
                collect_aggregates(x, out);
            }
        }
        Expr::Window {
            args,
            partition_by,
            order_by,
            ..
        } => {
            args.iter().for_each(|a| collect_aggregates(a, out));
            partition_by.iter().for_each(|a| collect_aggregates(a, out));
            order_by
                .iter()
                .for_each(|k| collect_aggregates(&k.expr, out));
        }
        _ => {}
    }
}

fn rewrite_bases(e: &Expr, bases: &[Expr]) -> Expr {
    if let Some(i) = bases.iter().position(|b| b == e) {
        return Expr::ColumnRef(i);
    }
    match e {
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: Box::new(rewrite_bases(expr, bases)),
        },
        Expr::Binary { op, left, right } => Expr::Binary {
            op: *op,
            left: Box::new(rewrite_bases(left, bases)),
            right: Box::new(rewrite_bases(right, bases)),
        },
        Expr::Func {
            name,
            args,
            distinct,
            filter,
            order_by,
        } => Expr::Func {
            name: name.clone(),
            args: args.iter().map(|a| rewrite_bases(a, bases)).collect(),
            distinct: *distinct,
            filter: filter.as_ref().map(|x| Box::new(rewrite_bases(x, bases))),
            order_by: order_by
                .iter()
                .map(|k| OrderKey {
                    expr: rewrite_bases(&k.expr, bases),
                    descending: k.descending,
                    nulls_first: k.nulls_first,
                    comp_oid: k.comp_oid,
                })
                .collect(),
        },
        Expr::GenBinary { op, left, right } => Expr::GenBinary {
            op: op.clone(),
            left: Box::new(rewrite_bases(left, bases)),
            right: Box::new(rewrite_bases(right, bases)),
        },
        Expr::GenUnary { op, expr } => Expr::GenUnary {
            op: op.clone(),
            expr: Box::new(rewrite_bases(expr, bases)),
        },
        Expr::Cast { expr, type_name } => Expr::Cast {
            expr: Box::new(rewrite_bases(expr, bases)),
            type_name: type_name.clone(),
        },
        Expr::IsNull { expr, negated } => Expr::IsNull {
            expr: Box::new(rewrite_bases(expr, bases)),
            negated: *negated,
        },
        Expr::Case {
            operand,
            whens,
            else_,
        } => Expr::Case {
            operand: operand.as_ref().map(|o| Box::new(rewrite_bases(o, bases))),
            whens: whens
                .iter()
                .map(|(c, r)| (rewrite_bases(c, bases), rewrite_bases(r, bases)))
                .collect(),
            else_: else_.as_ref().map(|x| Box::new(rewrite_bases(x, bases))),
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
            args: args.iter().map(|a| rewrite_bases(a, bases)).collect(),
            partition_by: partition_by
                .iter()
                .map(|a| rewrite_bases(a, bases))
                .collect(),
            order_by: order_by
                .iter()
                .map(|k| OrderKey {
                    expr: rewrite_bases(&k.expr, bases),
                    descending: k.descending,
                    nulls_first: k.nulls_first,
                    comp_oid: k.comp_oid,
                })
                .collect(),
            frame: frame.clone(),
            window_ref: window_ref.clone(),
        },
        _ => e.clone(),
    }
}

fn finish_aggregate_windowed(
    s: &SelectStmt,
    schema: &Schema,
    col_oids: &[u32],
    input: Plan,
    catalog: &Catalog,
    regs: &Arc<TypeRegistries>,
) -> Result<QueryResult, PgError> {
    use crate::expr::{bind, eval, infer};

    if !s.grouping_sets.is_empty() {
        return finish_grouping_sets_windowed(s, schema, col_oids, input, catalog, regs);
    }

    if s.projection.iter().any(|it| matches!(it, SelectItem::Star)) {
        return Err(PgError::InvalidInputSyntax {
            typ: "query",
            input: "SELECT * is not supported with GROUP BY / aggregates".into(),
        });
    }

    let mut bases: Vec<Expr> = s.group_by.clone();
    let mut aggs: Vec<Expr> = Vec::new();
    for it in &s.projection {
        if let SelectItem::Expr { expr, .. } = it {
            collect_aggregates(expr, &mut aggs);
        }
    }
    for k in &s.order_by {
        collect_aggregates(&k.expr, &mut aggs);
    }
    for e in &s.distinct_on {
        collect_aggregates(e, &mut aggs);
    }
    for a in aggs {
        if !bases.contains(&a) {
            bases.push(a);
        }
    }

    let mut int_names: Vec<String> = Vec::new();
    let mut int_oids: Vec<u32> = Vec::new();
    let mut resolved_bases: Vec<Expr> = Vec::new();
    for (i, b) in bases.iter().enumerate() {
        let mut rb = bind::resolve(b, schema)?;

        stamp_agg_order_keys(&mut rb, schema, col_oids, regs);
        resolved_bases.push(rb);
        int_oids.push(infer::infer(b, schema, col_oids).unwrap_or(0));
        int_names.push(match b {
            Expr::Column(n) => n.rsplit('.').next().unwrap_or(n).to_string(),
            _ => format!("__base{i}"),
        });
    }

    let mut key: Vec<Scalar> = Vec::new();
    for e in &s.group_by {
        key.push(bind::lower(e, schema, regs.clone())?);
    }
    let having = match &s.having {
        Some(h) => Some(bind::resolve(h, schema)?),
        None => None,
    };
    let cregs = regs.clone();
    let output: Box<dyn Fn(&[Row]) -> Result<Option<Row>, String>> =
        Box::new(move |grp: &[Row]| {
            if let Some(h) = &having {
                match eval::eval_group(h, grp, EvalCtx::new(&cregs)).map_err(|e| e.message())? {
                    SqlValue::Int(0) | SqlValue::Null => return Ok(None),
                    _ => {}
                }
            }
            let mut row = Vec::with_capacity(resolved_bases.len());
            for e in &resolved_bases {
                row.push(
                    eval::eval_group(e, grp, EvalCtx::new(&cregs)).map_err(|er| er.message())?,
                );
            }
            Ok(Some(row))
        });
    let grouped = Plan::Aggregate {
        input: Box::new(input),
        key,
        output,
    };
    let rows = grouped.execute().map_err(exec_err)?;
    let int_schema = Schema::new(int_names);

    let projection = s
        .projection
        .iter()
        .map(|it| match it {
            SelectItem::Star => SelectItem::Star,
            SelectItem::Expr { expr, alias } => {

                let name = alias.clone().unwrap_or_else(|| inferred_name(expr));
                SelectItem::Expr {
                    expr: rewrite_bases(expr, &bases),
                    alias: Some(name),
                }
            }
        })
        .collect();
    let order_by = s
        .order_by
        .iter()
        .map(|k| OrderKey {
            expr: rewrite_bases(&k.expr, &bases),
            descending: k.descending,
            nulls_first: k.nulls_first,
            comp_oid: k.comp_oid,
        })
        .collect();
    let distinct_on = s
        .distinct_on
        .iter()
        .map(|e| rewrite_bases(e, &bases))
        .collect();

    let rewritten = SelectStmt {
        distinct: s.distinct,
        distinct_on,
        projection,
        from: None,
        filter: None,
        group_by: Vec::new(),
        grouping_sets: Vec::new(),
        having: None,
        order_by,
        limit: s.limit,
        offset: s.offset,
        windows: Vec::new(),
        tail: Vec::new(),
        locking: Vec::new(),
    };

    finish_projection(
        &rewritten,
        &int_schema,
        &int_oids,
        Plan::Scan(rows),
        catalog,
        regs,
    )
}

fn order_output(
    mut rows: Vec<Row>,
    s: &SelectStmt,
    columns: &[String],
    proj_exprs: &[Expr],
    col_types: &[u32],
) -> Result<Vec<Row>, PgError> {
    let mut keys: Vec<(usize, bool, Option<bool>)> = Vec::new();
    for k in &s.order_by {

        let by_name = match &k.expr {
            Expr::Column(name) => {
                let want = name.rsplit('.').next().unwrap_or(name);
                columns.iter().position(|c| c == want)
            }
            _ => None,
        };
        let idx = by_name
            .or_else(|| proj_exprs.iter().position(|pe| pe == &k.expr))
            .ok_or_else(|| PgError::InvalidInputSyntax {
                typ: "query",
                input: "ORDER BY on a grouped query must name an output column".into(),
            })?;
        keys.push((
            idx,
            k.descending,
            Some(k.nulls_first.unwrap_or(k.descending)),
        ));
    }
    rows.sort_by(|a, b| {
        for (i, desc, nf) in &keys {
            let (av, bv) = (&a[*i], &b[*i]);

            let ord = if !matches!(av, SqlValue::Null)
                && !matches!(bv, SqlValue::Null)
                && col_types.get(*i).copied() == Some(crate::types::oid::NUMERIC)
            {
                let base = crate::types::numeric::value_cmp(av, bv)
                    .unwrap_or_else(|| sql_core::sort_cmp(av, bv));
                if *desc {
                    base.reverse()
                } else {
                    base
                }
            } else {
                sql_core::sort_cmp_nulls(av, bv, *desc, Some(nf.unwrap_or(*desc)))
            };
            if ord != std::cmp::Ordering::Equal {
                return ord;
            }
        }
        std::cmp::Ordering::Equal
    });
    Ok(rows)
}

pub(crate) fn order_by_names(
    mut rows: Vec<Row>,
    order_by: &[super::ast::OrderKey],
    columns: &[String],
) -> Result<Vec<Row>, PgError> {
    let mut keys: Vec<(usize, bool, Option<bool>)> = Vec::new();
    for k in order_by {
        let idx = match &k.expr {
            Expr::Column(name) => {
                let want = name.rsplit('.').next().unwrap_or(name);
                columns.iter().position(|c| c == want)
            }
            Expr::Int(n) if *n >= 1 && (*n as usize) <= columns.len() => Some(*n as usize - 1),
            _ => None,
        }
        .ok_or_else(|| PgError::InvalidInputSyntax {
            typ: "query",
            input: "ORDER BY must reference an output column".into(),
        })?;
        keys.push((
            idx,
            k.descending,
            Some(k.nulls_first.unwrap_or(k.descending)),
        ));
    }
    rows.sort_by(|a, b| {
        for (i, desc, nf) in &keys {
            let ord = sql_core::sort_cmp_nulls(&a[*i], &b[*i], *desc, Some(nf.unwrap_or(*desc)));
            if ord != std::cmp::Ordering::Equal {
                return ord;
            }
        }
        std::cmp::Ordering::Equal
    });
    Ok(rows)
}

pub(crate) fn inferred_name(e: &Expr) -> String {
    match e {

        Expr::Column(name) => name.rsplit('.').next().unwrap_or(name).to_string(),
        Expr::Func { name, .. } => name.clone(),
        Expr::Cast { type_name, .. } => type_name.clone(),

        Expr::Row(_) => "row".to_string(),
        Expr::FieldAccess { field, .. } => field.clone(),
        _ => "?column?".to_string(),
    }
}

fn dedup_on_keys(rows: Vec<Row>, key_from: usize) -> Vec<Row> {
    let mut seen: Vec<Vec<SqlValue>> = Vec::new();
    let mut out: Vec<Row> = Vec::new();
    for r in rows {
        let key = r[key_from..].to_vec();
        let dup = seen.iter().any(|k| {
            k.len() == key.len()
                && k.iter().zip(&key).all(|(a, b)| {
                    crate::types::numeric::value_cmp(a, b).unwrap_or_else(|| a.cmp(b))
                        == std::cmp::Ordering::Equal
                })
        });
        if !dup {
            seen.push(key);
            out.push(r);
        }
    }
    out
}

fn numeric_sort_key(inner: Scalar) -> Scalar {
    Box::new(move |row: &Row| inner(row).map(|v| crate::types::numeric::sort_key(&v)))
}

fn bpchar_sort_key(inner: Scalar) -> Scalar {
    Box::new(move |row: &Row| inner(row).map(|v| crate::types::text::rtrim_blanks(&v)))
}

fn group_key_wrap(k: Scalar, inferred: Option<u32>) -> Scalar {
    match inferred {
        Some(crate::types::oid::NUMERIC) => numeric_sort_key(k),
        Some(crate::types::oid::BPCHAR) => bpchar_sort_key(k),
        _ => k,
    }
}

fn rewrite_bpchar_cmp(e: &Expr, schema: &Schema, col_oids: &[u32]) -> Expr {
    use crate::expr::ast::BinOp;
    let is_bpchar = |x: &Expr| {
        crate::expr::infer::infer(x, schema, col_oids) == Some(crate::types::oid::BPCHAR)
    };
    let wrap = |x: Expr| Expr::Func {
        name: "rtrim".to_string(),
        args: vec![x],
        distinct: false,
        filter: None,
        order_by: Vec::new(),
    };
    match e {
        Expr::Binary { op, left, right } => {
            let l = rewrite_bpchar_cmp(left, schema, col_oids);
            let r = rewrite_bpchar_cmp(right, schema, col_oids);
            let is_cmp = matches!(
                op,
                BinOp::Lt | BinOp::Gt | BinOp::Eq | BinOp::LtEq | BinOp::GtEq | BinOp::NotEq
            );
            if is_cmp && (is_bpchar(left) || is_bpchar(right)) {
                Expr::Binary {
                    op: *op,
                    left: Box::new(wrap(l)),
                    right: Box::new(wrap(r)),
                }
            } else {
                Expr::Binary {
                    op: *op,
                    left: Box::new(l),
                    right: Box::new(r),
                }
            }
        }
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: Box::new(rewrite_bpchar_cmp(expr, schema, col_oids)),
        },
        Expr::GenBinary { op, left, right } => Expr::GenBinary {
            op: op.clone(),
            left: Box::new(rewrite_bpchar_cmp(left, schema, col_oids)),
            right: Box::new(rewrite_bpchar_cmp(right, schema, col_oids)),
        },
        Expr::GenUnary { op, expr } => Expr::GenUnary {
            op: op.clone(),
            expr: Box::new(rewrite_bpchar_cmp(expr, schema, col_oids)),
        },
        Expr::IsNull { expr, negated } => Expr::IsNull {
            expr: Box::new(rewrite_bpchar_cmp(expr, schema, col_oids)),
            negated: *negated,
        },
        Expr::Case {
            operand,
            whens,
            else_,
        } => Expr::Case {
            operand: operand
                .as_deref()
                .map(|o| Box::new(rewrite_bpchar_cmp(o, schema, col_oids))),
            whens: whens
                .iter()
                .map(|(c, r)| {
                    (
                        rewrite_bpchar_cmp(c, schema, col_oids),
                        rewrite_bpchar_cmp(r, schema, col_oids),
                    )
                })
                .collect(),
            else_: else_
                .as_deref()
                .map(|x| Box::new(rewrite_bpchar_cmp(x, schema, col_oids))),
        },
        other => other.clone(),
    }
}

fn enum_sort_key(inner: Scalar, labels: Vec<String>) -> Scalar {
    Box::new(move |row: &Row| {
        inner(row).map(|v| match &v {
            SqlValue::Text(s) => match labels.iter().position(|l| l == s) {
                Some(i) => SqlValue::Text(format!("{i:08}")),
                None => v,
            },
            _ => v,
        })
    })
}

fn enum_oid_of(
    e: &Expr,
    schema: &Schema,
    col_oids: &[u32],
    regs: &Arc<TypeRegistries>,
) -> Option<u32> {
    crate::expr::infer::infer(e, schema, col_oids).filter(|&o| regs.is_enum(o))
}

fn enum_label_to_ord(inner: Expr, labels: &[String]) -> Expr {
    Expr::Case {
        operand: Some(Box::new(inner)),
        whens: labels
            .iter()
            .enumerate()
            .map(|(i, l)| (Expr::Str(l.clone()), Expr::Int(i as i64)))
            .collect(),
        else_: None,
    }
}

fn enum_ord_to_label(inner: Expr, labels: &[String]) -> Expr {
    Expr::Case {
        operand: Some(Box::new(inner)),
        whens: labels
            .iter()
            .enumerate()
            .map(|(i, l)| (Expr::Int(i as i64), Expr::Str(l.clone())))
            .collect(),
        else_: None,
    }
}

fn rewrite_enum(e: &Expr, schema: &Schema, col_oids: &[u32], regs: &Arc<TypeRegistries>) -> Expr {
    use crate::expr::ast::BinOp;
    match e {
        Expr::Binary { op, left, right } => {
            let l = rewrite_enum(left, schema, col_oids, regs);
            let r = rewrite_enum(right, schema, col_oids, regs);
            let is_cmp = matches!(
                op,
                BinOp::Lt | BinOp::Gt | BinOp::Eq | BinOp::LtEq | BinOp::GtEq | BinOp::NotEq
            );
            let enum_oid = enum_oid_of(left, schema, col_oids, regs)
                .or_else(|| enum_oid_of(right, schema, col_oids, regs));
            if is_cmp {
                if let Some(oid) = enum_oid {
                    if let Some(labels) = regs.labels(oid) {
                        return Expr::Binary {
                            op: *op,
                            left: Box::new(enum_label_to_ord(l, labels)),
                            right: Box::new(enum_label_to_ord(r, labels)),
                        };
                    }
                }
            }
            Expr::Binary {
                op: *op,
                left: Box::new(l),
                right: Box::new(r),
            }
        }
        Expr::Func {
            name,
            args,
            distinct,
            filter,
            order_by,
        } if (name == "min" || name == "max")
            && args.len() == 1
            && enum_oid_of(&args[0], schema, col_oids, regs).is_some() =>
        {
            let oid = enum_oid_of(&args[0], schema, col_oids, regs).unwrap();
            match regs.labels(oid) {
                Some(labels) => {
                    let inner_arg = enum_label_to_ord(args[0].clone(), labels);
                    let agg = Expr::Func {
                        name: name.clone(),
                        args: vec![inner_arg],
                        distinct: *distinct,
                        filter: filter.clone(),
                        order_by: order_by.clone(),
                    };
                    enum_ord_to_label(agg, labels)
                }
                None => e.clone(),
            }
        }

        Expr::Func { name, args, .. } if name == "pg_typeof" && args.len() == 1 => {
            match pg_typeof_name(&args[0], schema, col_oids, regs) {
                Some(tn) => Expr::Str(tn),
                None => e.clone(),
            }
        }
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
                .map(|a| rewrite_enum(a, schema, col_oids, regs))
                .collect(),
            distinct: *distinct,
            filter: filter.clone(),
            order_by: order_by.clone(),
        },
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: Box::new(rewrite_enum(expr, schema, col_oids, regs)),
        },

        Expr::FieldAccess {
            base,
            field,
            comp_oid: pc,
            field_oid: pf,
        } => {
            let base_r = rewrite_enum(base, schema, col_oids, regs);

            let comp_oid = if *pc != 0 {
                *pc
            } else {
                crate::expr::infer::infer(&base_r, schema, col_oids).unwrap_or(0)
            };
            let field_oid = if *pf != 0 {
                *pf
            } else {
                regs.composite(comp_oid)
                    .and_then(|ci| ci.field_oid(field))
                    .unwrap_or(0)
            };
            Expr::FieldAccess {
                base: Box::new(base_r),
                field: field.clone(),
                comp_oid,
                field_oid,
            }
        }
        Expr::Row(elems) => Expr::Row(
            elems
                .iter()
                .map(|e| rewrite_enum(e, schema, col_oids, regs))
                .collect(),
        ),
        Expr::IsNull { expr, negated } => Expr::IsNull {
            expr: Box::new(rewrite_enum(expr, schema, col_oids, regs)),
            negated: *negated,
        },
        Expr::Case {
            operand,
            whens,
            else_,
        } => Expr::Case {
            operand: operand
                .as_deref()
                .map(|o| Box::new(rewrite_enum(o, schema, col_oids, regs))),
            whens: whens
                .iter()
                .map(|(cnd, res)| {
                    (
                        rewrite_enum(cnd, schema, col_oids, regs),
                        rewrite_enum(res, schema, col_oids, regs),
                    )
                })
                .collect(),
            else_: else_
                .as_deref()
                .map(|x| Box::new(rewrite_enum(x, schema, col_oids, regs))),
        },
        other => other.clone(),
    }
}

fn pg_typeof_name(
    e: &Expr,
    schema: &Schema,
    col_oids: &[u32],
    regs: &Arc<TypeRegistries>,
) -> Option<String> {

    if matches!(e, Expr::Float(_)) {
        return Some("numeric".to_string());
    }
    let oid = crate::expr::infer::infer(e, schema, col_oids)?;
    if let Some(n) = regs.enum_name(oid) {
        return Some(n.to_string());
    }
    if let Some(ci) = regs.composite(oid) {
        return Some(ci.name.clone());
    }
    Some(crate::types::type_name(oid).to_string())
}

fn composite_min_max_oid(
    e: &Expr,
    schema: &Schema,
    col_oids: &[u32],
    regs: &Arc<TypeRegistries>,
) -> Option<u32> {
    match e {
        Expr::Func { name, args, .. } if (name == "min" || name == "max") && args.len() == 1 => {
            composite_oid_of(&args[0], schema, col_oids, regs)
                .or_else(|| composite_min_max_oid(&args[0], schema, col_oids, regs))
        }
        Expr::Func { args, .. } => args
            .iter()
            .find_map(|a| composite_min_max_oid(a, schema, col_oids, regs)),
        Expr::Unary { expr, .. } | Expr::GenUnary { expr, .. } | Expr::IsNull { expr, .. } => {
            composite_min_max_oid(expr, schema, col_oids, regs)
        }
        Expr::Binary { left, right, .. } | Expr::GenBinary { left, right, .. } => {
            composite_min_max_oid(left, schema, col_oids, regs)
                .or_else(|| composite_min_max_oid(right, schema, col_oids, regs))
        }
        Expr::Cast { expr, .. } => composite_min_max_oid(expr, schema, col_oids, regs),
        Expr::Case {
            operand,
            whens,
            else_,
        } => operand
            .as_deref()
            .and_then(|o| composite_min_max_oid(o, schema, col_oids, regs))
            .or_else(|| {
                whens.iter().find_map(|(c, r)| {
                    composite_min_max_oid(c, schema, col_oids, regs)
                        .or_else(|| composite_min_max_oid(r, schema, col_oids, regs))
                })
            })
            .or_else(|| {
                else_
                    .as_deref()
                    .and_then(|x| composite_min_max_oid(x, schema, col_oids, regs))
            }),
        _ => None,
    }
}

fn stamp_agg_order_keys(
    e: &mut Expr,
    schema: &Schema,
    col_oids: &[u32],
    regs: &Arc<TypeRegistries>,
) {
    match e {
        Expr::Func {
            args,
            order_by,
            filter,
            ..
        } => {
            for k in order_by.iter_mut() {
                k.comp_oid = composite_oid_of(&k.expr, schema, col_oids, regs);
            }
            for a in args.iter_mut() {
                stamp_agg_order_keys(a, schema, col_oids, regs);
            }
            if let Some(f) = filter {
                stamp_agg_order_keys(f, schema, col_oids, regs);
            }
        }
        Expr::Unary { expr, .. }
        | Expr::GenUnary { expr, .. }
        | Expr::IsNull { expr, .. }
        | Expr::Cast { expr, .. } => stamp_agg_order_keys(expr, schema, col_oids, regs),
        Expr::Binary { left, right, .. } | Expr::GenBinary { left, right, .. } => {
            stamp_agg_order_keys(left, schema, col_oids, regs);
            stamp_agg_order_keys(right, schema, col_oids, regs);
        }
        Expr::Case {
            operand,
            whens,
            else_,
        } => {
            if let Some(o) = operand {
                stamp_agg_order_keys(o, schema, col_oids, regs);
            }
            for (c, r) in whens.iter_mut() {
                stamp_agg_order_keys(c, schema, col_oids, regs);
                stamp_agg_order_keys(r, schema, col_oids, regs);
            }
            if let Some(x) = else_ {
                stamp_agg_order_keys(x, schema, col_oids, regs);
            }
        }
        _ => {}
    }
}

fn composite_oid_of(
    e: &Expr,
    schema: &Schema,
    col_oids: &[u32],
    regs: &Arc<TypeRegistries>,
) -> Option<u32> {

    if let Some(o) =
        crate::expr::infer::infer(e, schema, col_oids).filter(|&o| regs.composite(o).is_some())
    {
        return Some(o);
    }

    if let Expr::Cast { type_name, .. } = e {
        return regs.composite_oid_by_name(type_name);
    }
    None
}

fn field_access(base: &Expr, comp_oid: u32, field: &str, field_oid: u32) -> Expr {
    Expr::FieldAccess {
        base: Box::new(base.clone()),
        field: field.to_string(),
        comp_oid,
        field_oid,
    }
}

fn field_cmp(
    op: crate::expr::ast::BinOp,
    l: &Expr,
    r: &Expr,
    comp_oid: u32,
    field: &(String, u32, i32),
) -> Expr {
    Expr::Binary {
        op,
        left: Box::new(field_access(l, comp_oid, &field.0, field.1)),
        right: Box::new(field_access(r, comp_oid, &field.0, field.1)),
    }
}

fn composite_cmp_expansion(
    op: crate::expr::ast::BinOp,
    l: &Expr,
    r: &Expr,
    comp_oid: u32,
    fields: &[(String, u32, i32)],
) -> Expr {
    use crate::expr::ast::{BinOp, UnOp};

    let eq_all = || -> Expr {
        let mut it = fields.iter();
        let first = field_cmp(BinOp::Eq, l, r, comp_oid, it.next().unwrap());
        it.fold(first, |acc, f| Expr::Binary {
            op: BinOp::And,
            left: Box::new(acc),
            right: Box::new(field_cmp(BinOp::Eq, l, r, comp_oid, f)),
        })
    };
    match op {
        BinOp::Eq => eq_all(),
        BinOp::NotEq => Expr::Unary {
            op: UnOp::Not,
            expr: Box::new(eq_all()),
        },

        BinOp::Lt | BinOp::LtEq | BinOp::Gt | BinOp::GtEq => {
            let (strict, tail) = match op {
                BinOp::Lt => (BinOp::Lt, BinOp::Lt),
                BinOp::LtEq => (BinOp::Lt, BinOp::LtEq),
                BinOp::Gt => (BinOp::Gt, BinOp::Gt),
                BinOp::GtEq => (BinOp::Gt, BinOp::GtEq),
                _ => unreachable!(),
            };
            let last = fields.len() - 1;
            let mut acc = field_cmp(tail, l, r, comp_oid, &fields[last]);
            for i in (0..last).rev() {
                let lt_i = field_cmp(strict, l, r, comp_oid, &fields[i]);
                let eq_i = field_cmp(BinOp::Eq, l, r, comp_oid, &fields[i]);
                let and = Expr::Binary {
                    op: BinOp::And,
                    left: Box::new(eq_i),
                    right: Box::new(acc),
                };
                acc = Expr::Binary {
                    op: BinOp::Or,
                    left: Box::new(lt_i),
                    right: Box::new(and),
                };
            }
            acc
        }
        _ => unreachable!("composite_cmp_expansion called with a non-comparison op"),
    }
}

fn rewrite_composite_cmp(
    e: &Expr,
    schema: &Schema,
    col_oids: &[u32],
    regs: &Arc<TypeRegistries>,
) -> Result<Expr, PgError> {
    use crate::expr::ast::BinOp;
    let recur = |x: &Expr| rewrite_composite_cmp(x, schema, col_oids, regs);
    match e {
        Expr::Binary { op, left, right } => {
            let is_cmp = matches!(
                op,
                BinOp::Lt | BinOp::Gt | BinOp::Eq | BinOp::LtEq | BinOp::GtEq | BinOp::NotEq
            );
            if is_cmp {
                let lc = composite_oid_of(left, schema, col_oids, regs);
                let rc = composite_oid_of(right, schema, col_oids, regs);
                match (lc, rc) {
                    (Some(a), Some(b)) if a == b => {
                        let fields = regs.composite(a).unwrap().fields.clone();
                        return Ok(composite_cmp_expansion(*op, left, right, a, &fields));
                    }
                    (Some(a), Some(b)) => {

                        let (na, nb) = (
                            regs.composite(a).unwrap().name.clone(),
                            regs.composite(b).unwrap().name.clone(),
                        );
                        let sym = match op {
                            BinOp::Lt => "<",
                            BinOp::Gt => ">",
                            BinOp::Eq => "=",
                            BinOp::LtEq => "<=",
                            BinOp::GtEq => ">=",
                            BinOp::NotEq => "<>",
                            _ => "?",
                        };
                        return Err(PgError::InvalidInputSyntax {
                            typ: "expression",
                            input: format!("operator does not exist: {na} {sym} {nb}"),
                        });
                    }
                    _ => {}
                }
            }
            Ok(Expr::Binary {
                op: *op,
                left: Box::new(recur(left)?),
                right: Box::new(recur(right)?),
            })
        }
        Expr::Unary { op, expr } => Ok(Expr::Unary {
            op: *op,
            expr: Box::new(recur(expr)?),
        }),
        Expr::Func {
            name,
            args,
            distinct,
            filter,
            order_by,
        } => {
            let mut new_args = Vec::with_capacity(args.len());
            for a in args {
                new_args.push(recur(a)?);
            }
            Ok(Expr::Func {
                name: name.clone(),
                args: new_args,
                distinct: *distinct,
                filter: filter.clone(),
                order_by: order_by.clone(),
            })
        }
        Expr::IsNull { expr, negated } => Ok(Expr::IsNull {
            expr: Box::new(recur(expr)?),
            negated: *negated,
        }),
        Expr::Case {
            operand,
            whens,
            else_,
        } => {
            let operand = match operand {
                Some(o) => Some(Box::new(recur(o)?)),
                None => None,
            };
            let mut new_whens = Vec::with_capacity(whens.len());
            for (c, r) in whens {
                new_whens.push((recur(c)?, recur(r)?));
            }
            let else_ = match else_ {
                Some(x) => Some(Box::new(recur(x)?)),
                None => None,
            };
            Ok(Expr::Case {
                operand,
                whens: new_whens,
                else_,
            })
        }
        other => Ok(other.clone()),
    }
}

fn jsonb_sort_key(inner: Scalar) -> Scalar {
    Box::new(move |row: &Row| inner(row).map(|v| jsonb_order_key(&v)))
}

fn jsonb_order_key(v: &SqlValue) -> SqlValue {
    match v {
        SqlValue::Text(s) => crate::types::jsonb::order_key(s.trim()),
        _ => v.clone(),
    }
}

fn field_sort_key(oid: u32, t: &str, regs: &TypeRegistries) -> String {
    use crate::types::oid;
    if regs.is_enum(oid) {
        return match regs.ordinal(oid, t) {
            Some(i) => format!("{i:08}"),
            None => t.to_string(),
        };
    }
    match oid {
        oid::NUMERIC | oid::INT2 | oid::INT4 | oid::INT8 | oid::OID => {
            match crate::types::numeric::sort_key(&SqlValue::Text(t.to_string())) {
                SqlValue::Text(k) => k,
                _ => t.to_string(),
            }
        }
        _ => t.to_string(),
    }
}

pub(crate) fn composite_order_key(
    v: &SqlValue,
    fields: &[(String, u32, i32)],
    regs: &TypeRegistries,
) -> SqlValue {
    let lit = match v {
        SqlValue::Text(s) => s,
        _ => return v.clone(),
    };
    let parts = match crate::types::composite::decode(lit) {
        Ok(p) => p,
        Err(_) => return v.clone(),
    };
    let mut key = String::new();
    for (i, f) in fields.iter().enumerate() {
        key.push('\u{00}');
        match parts.get(i) {
            Some(Some(t)) => {
                key.push('\u{01}');
                key.push_str(&field_sort_key(f.1, t, regs));
            }
            _ => key.push('\u{02}'),
        }
    }
    SqlValue::Text(key)
}

fn composite_sort_key(
    inner: Scalar,
    fields: Vec<(String, u32, i32)>,
    regs: Arc<TypeRegistries>,
) -> Scalar {
    Box::new(move |row: &Row| inner(row).map(|v| composite_order_key(&v, &fields, &regs)))
}

pub(crate) fn dedup_preserving_order_typed(
    rows: Vec<Row>,
    col_types: &[u32],
    regs: &TypeRegistries,
) -> Vec<Row> {
    let key_of = |r: &Row| -> Row {
        r.iter()
            .enumerate()
            .map(|(i, c)| {
                match col_types.get(i).copied() {
                    Some(crate::types::oid::NUMERIC) => crate::types::numeric::sort_key(c),

                    Some(crate::types::oid::BPCHAR) => crate::types::text::rtrim_blanks(c),

                    Some(oid) if regs.composite(oid).is_some() => {
                        let fields = regs.composite(oid).unwrap().fields.clone();
                        composite_order_key(c, &fields, regs)
                    }
                    _ => c.clone(),
                }
            })
            .collect()
    };
    let mut seen: Vec<Row> = Vec::new();
    let mut out = Vec::new();
    for r in rows {
        let k = key_of(&r);
        if !seen.iter().any(|s| rows_eq(s, &k)) {
            seen.push(k);
            out.push(r);
        }
    }
    out
}

pub(crate) fn dedup_preserving_order(rows: Vec<Row>) -> Vec<Row> {
    let mut seen: Vec<Row> = Vec::new();
    let mut out = Vec::new();
    for r in rows {
        if !seen.iter().any(|s| rows_eq(s, &r)) {
            seen.push(r.clone());
            out.push(r);
        }
    }
    out
}

pub(crate) fn rows_eq(a: &Row, b: &Row) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| x.cmp(y) == std::cmp::Ordering::Equal)
}

pub(crate) fn rows_eq_typed(a: &Row, b: &Row, col_types: &[u32]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .enumerate()
            .all(|(i, (x, y))| match col_types.get(i).copied() {
                Some(crate::types::oid::NUMERIC) => {
                    crate::types::numeric::value_cmp(x, y).unwrap_or_else(|| x.cmp(y))
                        == std::cmp::Ordering::Equal
                }
                Some(crate::types::oid::BPCHAR) => {
                    crate::types::text::rtrim_blanks(x) == crate::types::text::rtrim_blanks(y)
                }
                _ => x.cmp(y) == std::cmp::Ordering::Equal,
            })
}

#[cfg(test)]
mod tests {
    use super::*;
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
            "people",
            ["id", "name", "age"],
            vec![
                vec![i(1), t("alice"), i(30)],
                vec![i(2), t("bob"), i(25)],
                vec![i(3), t("carol"), i(30)],
                vec![i(4), t("dave"), i(40)],
            ],
        );
        c
    }

    fn run_ok(sql: &str) -> QueryResult {
        run(sql, &catalog()).expect("query ok")
    }

    #[test]
    fn select_star() {
        let r = run_ok("SELECT * FROM people WHERE age = 30 ORDER BY id");
        assert_eq!(r.columns, vec!["id", "name", "age"]);
        assert_eq!(
            r.rows,
            vec![vec![i(1), t("alice"), i(30)], vec![i(3), t("carol"), i(30)]]
        );
    }

    #[test]
    fn projection_expr_alias_and_function() {
        let r = run_ok("SELECT upper(name) AS n, age * 2 AS a2 FROM people ORDER BY id LIMIT 2");
        assert_eq!(r.columns, vec!["n", "a2"]);
        assert_eq!(r.rows, vec![vec![t("ALICE"), i(60)], vec![t("BOB"), i(50)]]);
    }

    #[test]
    fn where_order_limit_offset() {

        let r = run_ok("SELECT age FROM people WHERE age >= 30 ORDER BY age DESC OFFSET 1 LIMIT 1");
        assert_eq!(r.rows, vec![vec![i(30)]]);
    }

    #[test]
    fn distinct() {
        let r = run_ok("SELECT DISTINCT age FROM people ORDER BY age");
        assert_eq!(r.rows, vec![vec![i(25)], vec![i(30)], vec![i(40)]]);
    }

    #[test]
    fn no_from_constant() {
        let r = run_ok("SELECT 1 + 2 AS three, upper('hi')");
        assert_eq!(r.columns, vec!["three", "upper"]);
        assert_eq!(r.rows, vec![vec![i(3), t("HI")]]);
    }

    #[test]
    fn unknown_relation_and_column_error() {
        assert!(run("SELECT * FROM nope", &catalog()).is_err());
        assert!(run("SELECT bogus FROM people", &catalog()).is_err());
    }

    #[test]
    fn full_ddl_dml_select_roundtrip() {

        let mut c = Catalog::new();
        let run = |sql: &str, c: &mut Catalog| super::super::run_mut(sql, c).expect(sql);
        run(
            "CREATE TABLE t (id integer, name text, age integer)",
            &mut c,
        );
        run(
            "INSERT INTO t VALUES (1, 'alice', 30), (2, 'bob', 25)",
            &mut c,
        );
        run(
            "INSERT INTO t (id, name, age) VALUES (3, 'carol', 40)",
            &mut c,
        );
        let r = run(
            "SELECT upper(name), age FROM t WHERE age >= 30 ORDER BY age DESC",
            &mut c,
        );
        assert_eq!(r.columns, vec!["upper", "age"]);
        assert_eq!(
            r.rows,
            vec![vec![t("CAROL"), i(40)], vec![t("ALICE"), i(30)]]
        );

        run("DELETE FROM t WHERE age < 30", &mut c);
        let r3 = run("SELECT id FROM t ORDER BY id", &mut c);
        assert_eq!(r3.rows, vec![vec![i(1)], vec![i(3)]]);

        let u = run(
            "SELECT id FROM t WHERE id = 1 UNION SELECT id FROM t WHERE id = 3",
            &mut c,
        );
        assert_eq!(dedup_preserving_order(u.rows.clone()).len(), 2);
    }

    fn join_catalog() -> Catalog {
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

    #[test]
    fn window_functions() {
        let mut c = Catalog::new();
        c.create(
            "s",
            ["id", "dept", "sal"],
            vec![
                vec![i(1), t("a"), i(100)],
                vec![i(2), t("a"), i(200)],
                vec![i(3), t("b"), i(150)],
                vec![i(4), t("b"), i(150)],
                vec![i(5), t("b"), i(300)],
            ],
        );

        let r = run(
            "SELECT id, row_number() OVER (ORDER BY sal DESC) AS rn FROM s ORDER BY id",
            &c,
        )
        .unwrap();

        assert_eq!(r.rows[0], vec![i(1), i(5)]);
        assert_eq!(r.rows[4], vec![i(5), i(1)]);

        let rk = run(
            "SELECT id, rank() OVER (ORDER BY sal) AS rk FROM s ORDER BY id",
            &c,
        )
        .unwrap();

        assert_eq!(rk.rows[2], vec![i(3), i(2)]);
        assert_eq!(rk.rows[3], vec![i(4), i(2)]);
        assert_eq!(rk.rows[1], vec![i(2), i(4)]);

        let dr = run(
            "SELECT id, dense_rank() OVER (ORDER BY sal) AS d FROM s ORDER BY id",
            &c,
        )
        .unwrap();
        assert_eq!(dr.rows[1], vec![i(2), i(3)]);

        let ps = run(
            "SELECT id, sum(sal) OVER (PARTITION BY dept) AS ds FROM s ORDER BY id",
            &c,
        )
        .unwrap();
        assert_eq!(ps.rows[0], vec![i(1), i(300)]);
        assert_eq!(ps.rows[2], vec![i(3), i(600)]);

        let pc = run(
            "SELECT id, count(*) OVER (PARTITION BY dept) AS n FROM s ORDER BY id",
            &c,
        )
        .unwrap();
        assert_eq!(pc.rows[0], vec![i(1), i(2)]);
        assert_eq!(pc.rows[2], vec![i(3), i(3)]);

        let lg = run(
            "SELECT id, lag(sal) OVER (ORDER BY id) AS prev FROM s ORDER BY id",
            &c,
        )
        .unwrap();
        assert_eq!(lg.rows[0], vec![i(1), SqlValue::Null]);
        assert_eq!(lg.rows[1], vec![i(2), i(100)]);
    }

    #[test]
    fn subqueries() {
        let c = join_catalog();

        let r = run(
            "SELECT name FROM emp WHERE id = (SELECT max(id) FROM emp) ORDER BY id",
            &c,
        )
        .unwrap();
        assert_eq!(r.rows, vec![vec![t("dave")]]);
        let r2 = run("SELECT (SELECT count(*) FROM emp) AS total", &c).unwrap();
        assert_eq!(r2.rows, vec![vec![i(4)]]);

        let r3 = run(
            "SELECT name FROM emp WHERE dept_id IN (SELECT did FROM dept) ORDER BY id",
            &c,
        )
        .unwrap();
        assert_eq!(
            r3.rows,
            vec![vec![t("alice")], vec![t("bob")], vec![t("carol")]]
        );

        let r4 = run(
            "SELECT name FROM emp WHERE dept_id NOT IN (SELECT did FROM dept) ORDER BY id",
            &c,
        )
        .unwrap();
        assert_eq!(r4.rows, vec![vec![t("dave")]]);

        assert_eq!(
            run("SELECT 1 AS x WHERE EXISTS (SELECT 1 FROM emp)", &c)
                .unwrap()
                .rows,
            vec![vec![i(1)]]
        );
        assert!(run(
            "SELECT 1 AS x WHERE EXISTS (SELECT 1 FROM emp WHERE id > 100)",
            &c
        )
        .unwrap()
        .rows
        .is_empty());
        assert_eq!(
            run(
                "SELECT 1 AS x WHERE NOT EXISTS (SELECT 1 FROM emp WHERE id > 100)",
                &c
            )
            .unwrap()
            .rows,
            vec![vec![i(1)]]
        );

        let r5 = run(
            "SELECT d.name FROM (SELECT name, dept_id FROM emp WHERE dept_id = 10) AS d ORDER BY d.name",
            &c,
        )
        .unwrap();
        assert_eq!(r5.rows, vec![vec![t("alice")], vec![t("carol")]]);
    }

    #[test]
    fn correlated_subqueries() {
        let mut c = Catalog::new();
        c.create(
            "a",
            ["id", "threshold"],
            vec![vec![i(1), i(150)], vec![i(2), i(250)], vec![i(3), i(50)]],
        );
        c.create(
            "b",
            ["gid", "x"],
            vec![
                vec![i(1), i(100)],
                vec![i(1), i(200)],
                vec![i(2), i(300)],
                vec![i(2), i(50)],
            ],
        );

        let r = run(
            "SELECT id, (SELECT max(x) FROM b WHERE b.gid = a.id) AS mx FROM a ORDER BY id",
            &c,
        )
        .unwrap();
        assert_eq!(r.columns, vec!["id", "mx"]);
        assert_eq!(
            r.rows,
            vec![
                vec![i(1), i(200)],
                vec![i(2), i(300)],
                vec![i(3), SqlValue::Null]
            ]
        );

        assert_eq!(
            run(
                "SELECT id FROM a WHERE EXISTS (SELECT 1 FROM b WHERE b.gid = a.id) ORDER BY id",
                &c
            )
            .unwrap()
            .rows,
            vec![vec![i(1)], vec![i(2)]]
        );
        assert_eq!(
            run("SELECT id FROM a WHERE NOT EXISTS (SELECT 1 FROM b WHERE b.gid = a.id) ORDER BY id", &c)
                .unwrap()
                .rows,
            vec![vec![i(3)]]
        );

        assert_eq!(
            run("SELECT id FROM a WHERE id IN (SELECT gid FROM b WHERE x > a.threshold) ORDER BY id", &c)
                .unwrap()
                .rows,
            vec![vec![i(1)], vec![i(2)]]
        );

        assert!(run("SELECT id, (SELECT x FROM b WHERE b.gid = a.id) FROM a", &c).is_err());
    }

    #[test]
    fn aggregate_filter_and_distinct() {
        let mut c = Catalog::new();

        c.create(
            "s",
            ["g", "v"],
            vec![
                vec![t("a"), i(10)],
                vec![t("a"), i(10)],
                vec![t("a"), i(20)],
                vec![t("a"), SqlValue::Null],
                vec![t("b"), i(30)],
                vec![t("b"), i(-5)],
            ],
        );

        let r = run(
            "SELECT count(*) FILTER (WHERE v > 0) AS p, sum(v) FILTER (WHERE v > 0) AS s FROM s",
            &c,
        )
        .unwrap();
        assert_eq!(r.rows, vec![vec![i(4), i(70)]]);

        let r = run(
            "SELECT g, count(DISTINCT v) AS d FROM s GROUP BY g ORDER BY g",
            &c,
        )
        .unwrap();
        assert_eq!(r.rows, vec![vec![t("a"), i(2)], vec![t("b"), i(2)]]);

        let r = run(
            "SELECT g, sum(DISTINCT v) AS s FROM s GROUP BY g ORDER BY g",
            &c,
        )
        .unwrap();
        assert_eq!(r.rows, vec![vec![t("a"), i(30)], vec![t("b"), i(25)]]);

        let r = run(
            "SELECT count(DISTINCT v) FILTER (WHERE v > 0) AS d FROM s",
            &c,
        )
        .unwrap();
        assert_eq!(r.rows, vec![vec![i(3)]]);
    }

    #[test]
    fn distinct_on_keeps_first_per_group() {
        let mut c = Catalog::new();
        c.create(
            "s",
            ["id", "g", "v"],
            vec![
                vec![i(1), t("a"), i(10)],
                vec![i(2), t("a"), i(20)],
                vec![i(3), t("b"), i(30)],
                vec![i(4), t("b"), i(5)],
            ],
        );

        let r = run("SELECT DISTINCT ON (g) g, v FROM s ORDER BY g, v", &c).unwrap();
        assert_eq!(r.rows, vec![vec![t("a"), i(10)], vec![t("b"), i(5)]]);

        let r = run(
            "SELECT DISTINCT ON (g) g, id FROM s ORDER BY g, id DESC",
            &c,
        )
        .unwrap();
        assert_eq!(r.rows, vec![vec![t("a"), i(2)], vec![t("b"), i(4)]]);
    }

    #[test]
    fn correlated_name_shadowing() {

        let mut c = Catalog::new();
        c.create(
            "p",
            ["id", "lim"],
            vec![vec![i(1), i(1000)], vec![i(2), i(1000)]],
        );
        c.create(
            "q",
            ["pid", "lim", "v"],
            vec![
                vec![i(1), i(5), i(100)],
                vec![i(1), i(5), i(3)],
                vec![i(2), i(5), i(100)],
            ],
        );
        let r = run(
            "SELECT id, (SELECT count(*) FROM q WHERE q.pid = p.id AND v > lim) AS c FROM p ORDER BY id",
            &c,
        )
        .unwrap();

        assert_eq!(r.rows, vec![vec![i(1), i(1)], vec![i(2), i(1)]]);
    }

    #[test]
    fn whole_table_aggregates() {
        let c = join_catalog();

        assert_eq!(
            run("SELECT count(*) FROM emp", &c).unwrap().rows,
            vec![vec![i(4)]]
        );
        let r = run("SELECT sum(id), min(id), max(id), avg(id) FROM emp", &c).unwrap();
        assert_eq!(r.rows, vec![vec![i(10), i(1), i(4), SqlValue::Real(2.5)]]);

        assert_eq!(
            run("SELECT count(*) FROM emp WHERE id > 100", &c)
                .unwrap()
                .rows,
            vec![vec![i(0)]]
        );
    }

    #[test]
    fn group_by_and_having() {
        let c = join_catalog();

        let r = run(
            "SELECT dept_id, count(*) AS n FROM emp GROUP BY dept_id ORDER BY dept_id",
            &c,
        )
        .unwrap();
        assert_eq!(r.columns, vec!["dept_id", "n"]);
        assert_eq!(
            r.rows,
            vec![vec![i(10), i(2)], vec![i(20), i(1)], vec![i(99), i(1)]]
        );

        let h = run(
            "SELECT dept_id, count(*) AS n FROM emp GROUP BY dept_id HAVING count(*) > 1 ORDER BY dept_id",
            &c,
        )
        .unwrap();
        assert_eq!(h.rows, vec![vec![i(10), i(2)]]);
    }

    #[test]
    fn group_by_over_join() {

        let r = run(
            "SELECT dept.dname, count(*) AS n FROM emp JOIN dept ON emp.dept_id = dept.did GROUP BY dept.dname ORDER BY dept.dname",
            &join_catalog(),
        )
        .unwrap();
        assert_eq!(r.rows, vec![vec![t("eng"), i(2)], vec![t("sales"), i(1)]]);
    }

    #[test]
    fn inner_join_on_qualified() {
        let r = run(
            "SELECT emp.name, dept.dname FROM emp JOIN dept ON emp.dept_id = dept.did ORDER BY emp.id",
            &join_catalog(),
        )
        .unwrap();
        assert_eq!(r.columns, vec!["name", "dname"]);
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
    fn left_join_pads_unmatched() {

        let r = run(
            "SELECT emp.name, dept.dname FROM emp LEFT JOIN dept ON emp.dept_id = dept.did ORDER BY emp.id",
            &join_catalog(),
        )
        .unwrap();
        assert_eq!(r.rows.last().unwrap(), &vec![t("dave"), SqlValue::Null]);
        assert_eq!(r.rows.len(), 4);
    }

    #[test]
    fn cross_join_and_comma() {
        let r = run(
            "SELECT emp.id, dept.did FROM emp CROSS JOIN dept",
            &join_catalog(),
        )
        .unwrap();
        assert_eq!(r.rows.len(), 8);
        let r2 = run("SELECT emp.id FROM emp, dept", &join_catalog()).unwrap();
        assert_eq!(r2.rows.len(), 8);
    }

    #[test]
    fn join_with_alias_and_where() {
        let r = run(
            "SELECT e.name FROM emp e JOIN dept d ON e.dept_id = d.did WHERE d.dname = 'eng' ORDER BY e.id",
            &join_catalog(),
        )
        .unwrap();
        assert_eq!(r.rows, vec![vec![t("alice")], vec![t("carol")]]);
    }

    #[test]
    fn multi_key_order() {

        let r = run_ok("SELECT id, age FROM people ORDER BY age DESC, id ASC");
        assert_eq!(
            r.rows,
            vec![
                vec![i(4), i(40)],
                vec![i(1), i(30)],
                vec![i(3), i(30)],
                vec![i(2), i(25)],
            ]
        );
    }
}
