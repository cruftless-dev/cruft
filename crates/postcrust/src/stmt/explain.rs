
use super::ast::{FromItem, JoinKind, SelectItem, SelectStmt, Stmt};
use super::lower::QueryResult;
use crate::catalog::Catalog;
use crate::expr::lexer::Tok;
use crate::types::PgError;
use sql_core::SqlValue;

fn err(msg: impl Into<String>) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "query",
        input: msg.into(),
    }
}

struct Node {
    label: String,
    children: Vec<Node>,
}

impl Node {
    fn leaf(label: impl Into<String>) -> Node {
        Node {
            label: label.into(),
            children: Vec::new(),
        }
    }
    fn wrap(label: impl Into<String>, child: Node) -> Node {
        Node {
            label: label.into(),
            children: vec![child],
        }
    }
}

pub fn run(sql: &str, catalog: &Catalog) -> Result<QueryResult, PgError> {

    let trimmed = sql.trim_start();
    let rest = trimmed.get("explain".len()..).unwrap_or("").trim_start();
    let low = rest.to_ascii_lowercase();

    if low.starts_with("analyze") || low.starts_with("analyse") {
        return Err(err(
            "EXPLAIN ANALYZE is not supported (it would execute the statement)",
        ));
    }
    if low.starts_with("verbose") {
        return Err(err("EXPLAIN VERBOSE is not supported"));
    }
    if rest.starts_with('(') {
        return Err(err(
            "EXPLAIN options (e.g. ANALYZE, VERBOSE, FORMAT) are not supported",
        ));
    }
    if rest.is_empty() {
        return Err(err("EXPLAIN requires a statement"));
    }

    let inner = rest;
    let toks = crate::expr::lexer::lex(inner)?;

    if super::cte::has_with(&toks) {
        return Err(err("EXPLAIN of a WITH query is not yet supported"));
    }
    if super::setops::has_top_level_setop(&toks) {
        return Err(err("EXPLAIN of a set operation is not yet supported"));
    }

    if let Some(Tok::Ident(kw)) = toks.first() {
        match kw.as_str() {
            "insert" | "update" | "delete" => {
                return Ok(to_result(vec![dml_label(kw, &toks)]));
            }
            "create" | "drop" | "alter" | "truncate" => {
                return Err(err("EXPLAIN of DDL statements is not supported"));
            }
            _ => {}
        }
    }

    let Stmt::Select(s) = super::parser::parse(inner)?;
    let node = explain_select(&s, catalog)?;
    let mut lines = Vec::new();
    render(&node, 0, &mut lines);
    Ok(to_result(lines))
}

fn explain_select(s: &SelectStmt, catalog: &Catalog) -> Result<Node, PgError> {
    let mut node = match &s.from {

        Some(f) => from_node(f, catalog, s.filter.as_ref())?,

        None => Node::leaf("Result"),
    };

    if s.filter.is_some() {
        node = Node::wrap("Filter", node);
    }

    let is_aggregate = !s.group_by.is_empty()
        || !s.grouping_sets.is_empty()
        || s.having.is_some()
        || s.projection.iter().any(|it| {
            matches!(it, SelectItem::Expr { expr, .. }
                if crate::expr::eval::contains_aggregate(expr))
        });
    if is_aggregate {
        let grouped = !s.group_by.is_empty() || !s.grouping_sets.is_empty();
        let label = if grouped {
            "Aggregate (grouped)"
        } else {
            "Aggregate"
        };
        node = Node::wrap(label, node);
    }

    if !s.order_by.is_empty() {
        node = Node::wrap("Sort", node);
    }

    node = Node::wrap("Project", node);

    if s.distinct || !s.distinct_on.is_empty() {
        node = Node::wrap("Distinct", node);
    }

    if s.limit.is_some() || s.offset != 0 {
        node = Node::wrap("Limit", node);
    }

    Ok(node)
}

fn from_node(
    item: &FromItem,
    catalog: &Catalog,
    where_: Option<&crate::expr::ast::Expr>,
) -> Result<Node, PgError> {
    match item {
        FromItem::Table { name, alias } => {
            let Some(t) = catalog.get(name) else {
                return Err(err(format!("relation \"{name}\" does not exist")));
            };

            let qual = alias.clone().unwrap_or_else(|| name.clone());
            let schema = t.schema.clone().qualified(&qual);
            let regs = std::sync::Arc::new(catalog.type_registries());
            let access = super::lower::index_access_path(t, &schema, where_, &regs);

            let range = if access.is_none() {
                super::lower::range_access_path(t, &schema, where_, &regs)
            } else {
                None
            };
            let rel = match alias {
                Some(a) => format!("{name} {a}"),
                None => name.clone(),
            };
            let label = if let Some((col, key)) = &access {
                format!("Index Scan({rel}) ({col} = {})", render_const(key))
            } else if let Some((col, lo, hi)) = &range {
                format!("Index Scan({rel}) ({})", render_range(col, lo, hi))
            } else {
                format!("Scan({rel})")
            };
            Ok(Node::leaf(label))
        }
        FromItem::Join {
            left,
            right,
            kind,
            on,
            using,
            natural,
        } => {
            let l = from_node(left, catalog, None)?;
            let r = from_node(right, catalog, None)?;

            let hashed = match (
                super::lower::from_schema(left, catalog),
                super::lower::from_schema(right, catalog),
            ) {
                (Ok(ls), Ok(rs)) => {
                    super::lower::join_hashes(*kind, *natural, using, on.as_ref(), &ls, &rs)
                }
                _ => None,
            };
            let label = match hashed {
                Some(hk) => format!("Hash Join ({})", core_kind_name(hk)),
                None => format!("NestedLoopJoin ({})", join_kind_name(*kind)),
            };
            Ok(Node {
                label,
                children: vec![l, r],
            })
        }
        FromItem::Subquery { query, alias, .. } => {
            let inner = explain_select(query, catalog)?;
            Ok(Node {
                label: format!("Subquery Scan({alias})"),
                children: vec![inner],
            })
        }
        FromItem::Function { name, alias, .. } => {
            let disp = alias.clone().unwrap_or_else(|| name.clone());
            Ok(Node::leaf(format!("Function Scan({disp})")))
        }
    }
}

fn render_const(v: &SqlValue) -> String {
    match v {
        SqlValue::Int(i) => i.to_string(),
        SqlValue::Real(r) => r.to_string(),
        SqlValue::Text(s) => format!("'{s}'"),
        SqlValue::Null => "NULL".to_string(),
        SqlValue::Blob(_) => "?".to_string(),
    }
}

fn render_range(
    col: &str,
    lo: &std::ops::Bound<SqlValue>,
    hi: &std::ops::Bound<SqlValue>,
) -> String {
    use std::ops::Bound;
    let mut parts: Vec<String> = Vec::new();
    match lo {
        Bound::Included(v) => parts.push(format!("{col} >= {}", render_const(v))),
        Bound::Excluded(v) => parts.push(format!("{col} > {}", render_const(v))),
        Bound::Unbounded => {}
    }
    match hi {
        Bound::Included(v) => parts.push(format!("{col} <= {}", render_const(v))),
        Bound::Excluded(v) => parts.push(format!("{col} < {}", render_const(v))),
        Bound::Unbounded => {}
    }
    parts.join(" AND ")
}

fn core_kind_name(kind: sql_core::JoinKind) -> &'static str {
    match kind {
        sql_core::JoinKind::Inner => "Inner",
        sql_core::JoinKind::Left => "Left",
        sql_core::JoinKind::Right => "Right",
        sql_core::JoinKind::Full => "Full",
        sql_core::JoinKind::Cross => "Cross",
    }
}

fn join_kind_name(kind: JoinKind) -> &'static str {
    match kind {
        JoinKind::Inner => "Inner",
        JoinKind::Left => "Left",
        JoinKind::Right => "Right",
        JoinKind::Full => "Full",
        JoinKind::Cross => "Cross",
    }
}

fn dml_label(kw: &str, toks: &[Tok]) -> String {
    let op = match kw {
        "insert" => "Insert",
        "update" => "Update",
        "delete" => "Delete",
        _ => "Modify",
    };

    let ident_after = |anchor: &str| -> Option<String> {
        let pos = toks
            .iter()
            .position(|t| matches!(t, Tok::Ident(k) if k == anchor))?;
        match toks.get(pos + 1) {
            Some(Tok::Ident(name)) => Some(name.clone()),
            _ => None,
        }
    };
    let target = match kw {
        "insert" => ident_after("into"),
        "update" => ident_after("update"),
        "delete" => ident_after("from"),
        _ => None,
    };
    match target {
        Some(t) => format!("{op} on {t}"),
        None => op.to_string(),
    }
}

fn render(n: &Node, depth: usize, out: &mut Vec<String>) {
    out.push(format!("{}{}", "..".repeat(depth), n.label));
    for c in &n.children {
        render(c, depth + 1, out);
    }
}

fn to_result(lines: Vec<String>) -> QueryResult {
    QueryResult {
        columns: vec!["QUERY PLAN".to_string()],
        col_types: vec![0],
        rows: lines.into_iter().map(|l| vec![SqlValue::Text(l)]).collect(),
    }
}
