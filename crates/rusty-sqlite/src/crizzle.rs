
use crate::{parse_statement, val_to_sql, Bindings, Database, Outcome, Value};
use crizzle_core::{
    ident::is_safe_identifier, validate_result, CruftBoundaryError, CsType, RowSet,
};

pub fn affinity_cs_type(declared: &str, not_null: bool) -> CsType {
    let d = declared.to_ascii_uppercase();
    let base = if d.contains("BOOL") {
        CsType::Boolean
    } else if d.contains("INT") {
        CsType::Number
    } else if d.contains("CHAR") || d.contains("CLOB") || d.contains("TEXT") {
        CsType::Str
    } else if d.contains("BLOB") || d.is_empty() {
        CsType::Bytes
    } else if d.contains("REAL") || d.contains("FLOA") || d.contains("DOUB") {
        CsType::Number
    } else {

        CsType::Number
    };
    if not_null {
        base
    } else {
        CsType::Nullable(Box::new(base))
    }
}

pub fn derive_columns(db: &mut Database, table: &str) -> Vec<(String, CsType)> {
    if !is_safe_identifier(table) {
        return Vec::new();
    }
    let sql = format!("PRAGMA table_info({})", qident(table));
    let st = match parse_statement(&sql) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    match db.run(&st, &Bindings::new()) {
        Ok(Outcome::Rows { columns, rows }) => {
            let ci = |n: &str| columns.iter().position(|c| c == n);
            let (n_i, t_i, nn_i) = (ci("name"), ci("type"), ci("notnull"));
            rows.iter()
                .filter_map(|r| {
                    let name = match n_i.and_then(|i| r.get(i)) {
                        Some(Value::Text(s)) => s.clone(),
                        _ => return None,
                    };
                    let ty = match t_i.and_then(|i| r.get(i)) {
                        Some(Value::Text(s)) => s.clone(),
                        _ => String::new(),
                    };
                    let not_null = matches!(nn_i.and_then(|i| r.get(i)), Some(Value::Int(1)));
                    Some((name, affinity_cs_type(&ty, not_null)))
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

pub fn derive_relations(db: &mut Database, table: &str) -> Vec<crizzle_core::Relation> {
    use crizzle_core::{RelKind, Relation};

    fn fks(db: &mut Database, t: &str) -> Vec<(String, String, String)> {
        let rs = match query_rowset(db, &format!("PRAGMA foreign_key_list({t})")) {
            Ok(rs) => rs,
            Err(_) => return Vec::new(),
        };
        let idx = |n: &str| rs.columns.iter().position(|c| c == n);
        let (ti, fi, oi) = (idx("table"), idx("from"), idx("to"));
        rs.rows
            .iter()
            .filter_map(|r| {
                let text = |i: Option<usize>| match i.and_then(|i| r.get(i)) {
                    Some(sql_core::SqlValue::Text(s)) => Some(s.clone()),
                    _ => None,
                };
                Some((text(ti)?, text(fi)?, text(oi)?))
            })
            .collect()
    }

    let mut rels = Vec::new();

    for (parent, from, to) in fks(db, table) {
        rels.push(Relation {
            name: parent.clone(),
            kind: RelKind::BelongsTo,
            local_cols: vec![from],
            target: parent,
            target_cols: vec![to],
        });
    }

    let others: Vec<String> =
        match query_rowset(db, "SELECT name FROM sqlite_master WHERE type='table'") {
            Ok(rs) => rs
                .rows
                .iter()
                .filter_map(|r| match r.first() {
                    Some(sql_core::SqlValue::Text(s)) => Some(s.clone()),
                    _ => None,
                })
                .collect(),
            Err(_) => Vec::new(),
        };
    for other in others {
        if other == table {
            continue;
        }
        for (parent, from, to) in fks(db, &other) {
            if parent == table {
                rels.push(Relation {
                    name: other.clone(),
                    kind: RelKind::HasMany,
                    local_cols: vec![to],
                    target: other.clone(),
                    target_cols: vec![from],
                });
            }
        }
    }
    rels
}

pub fn query_rowset(db: &mut Database, sql: &str) -> Result<RowSet, String> {
    let st = parse_statement(sql).map_err(|e| format!("{e:?}"))?;
    match db
        .run(&st, &Bindings::new())
        .map_err(|e| format!("{e:?}"))?
    {
        Outcome::Rows { columns, rows } => {
            let n = columns.len();
            Ok(RowSet {
                columns,
                rows: rows
                    .iter()
                    .map(|r| r.iter().map(val_to_sql).collect())
                    .collect(),
                col_bool: vec![false; n],
                col_type_name: vec![String::new(); n],
            })
        }
        Outcome::Mutation { .. } => Ok(RowSet {
            columns: Vec::new(),
            rows: Vec::new(),
            col_bool: Vec::new(),
            col_type_name: Vec::new(),
        }),
    }
}

pub fn validate(
    db: &mut Database,
    sql: &str,
    expected: &[(String, CsType)],
) -> Result<Result<(), CruftBoundaryError>, String> {
    let rs = query_rowset(db, sql)?;
    Ok(validate_result(&rs, expected))
}

use crizzle_core::query::{Filter, FilterOp, FilterVal, Query, Select};
use sql_core::SqlValue;

fn sql_literal(v: &SqlValue) -> String {
    match v {
        SqlValue::Null => "NULL".to_string(),
        SqlValue::Int(i) => i.to_string(),
        SqlValue::Real(r) => r.to_string(),
        SqlValue::Text(s) => format!("'{}'", s.replace('\'', "''")),
        SqlValue::Blob(b) => {
            format!(
                "x'{}'",
                b.iter().map(|x| format!("{x:02x}")).collect::<String>()
            )
        }
    }
}

fn qident(c: &str) -> String {
    debug_assert!(is_safe_identifier(c));
    format!("\"{}\"", c.replace('"', "\"\""))
}

fn validate_ident(name: &str) -> Result<(), String> {
    if is_safe_identifier(name) {
        Ok(())
    } else {
        Err(format!("unsafe SQL identifier \"{name}\""))
    }
}

fn validate_single_table_query(
    db: &mut Database,
    q: &Query,
) -> Result<Vec<(String, CsType)>, String> {
    validate_ident(&q.from)?;
    let cols = derive_columns(db, &q.from);
    if cols.is_empty() {
        return Err(format!("relation \"{}\" does not exist", q.from));
    }
    let has_col = |name: &str| cols.iter().any(|(n, _)| n == name);
    let require_col = |name: &str| -> Result<(), String> {
        validate_ident(name)?;
        if has_col(name) {
            Ok(())
        } else {
            Err(format!("column \"{name}\" does not exist"))
        }
    };
    if let Select::Cols(selected) = &q.select {
        for c in selected {
            require_col(c)?;
        }
    }
    for f in &q.filters {
        require_col(&f.col)?;
    }
    for (c, _) in &q.order {
        require_col(c)?;
    }
    for c in &q.group_by {
        require_col(c)?;
    }
    for a in &q.aggregates {
        validate_ident(&a.alias)?;
        if let Some(c) = &a.col {
            require_col(c)?;
        }
    }
    Ok(cols)
}

pub fn lower(q: &Query) -> String {
    if !q.aggregates.is_empty() {
        return lower_aggregate(q);
    }
    let cols = match &q.select {
        Select::All => "*".to_string(),
        Select::Cols(cs) => cs.iter().map(|c| qident(c)).collect::<Vec<_>>().join(", "),
    };
    let mut sql = format!("SELECT {cols} FROM {}", qident(&q.from));
    let mut wheres: Vec<String> = Vec::new();
    for f in &q.filters {
        let col = qident(&f.col);
        match &f.op {
            FilterOp::IsNull => wheres.push(format!("{col} IS NULL")),
            FilterOp::IsNotNull => wheres.push(format!("{col} IS NOT NULL")),
            FilterOp::In => {
                if let FilterVal::Many(vs) = &f.value {
                    if vs.is_empty() {
                        wheres.push("0".to_string());
                    } else {
                        let items = vs.iter().map(sql_literal).collect::<Vec<_>>().join(", ");
                        wheres.push(format!("{col} IN ({items})"));
                    }
                }
            }
            op => {
                if let FilterVal::One(v) = &f.value {
                    wheres.push(format!("{col} {} {}", op.sql(), sql_literal(v)));
                }
            }
        }
    }
    if !wheres.is_empty() {
        sql.push_str(&format!(" WHERE {}", wheres.join(" AND ")));
    }
    if !q.order.is_empty() {
        let keys: Vec<String> = q
            .order
            .iter()
            .map(|(c, desc)| format!("{} {}", qident(c), if *desc { "DESC" } else { "ASC" }))
            .collect();
        sql.push_str(&format!(" ORDER BY {}", keys.join(", ")));
    }
    if let Some(n) = q.limit {
        sql.push_str(&format!(" LIMIT {n}"));
    }
    if let Some(n) = q.offset {
        sql.push_str(&format!(" OFFSET {n}"));
    }
    sql
}

use crizzle_core::query::{AggFunc, Join, JoinKind};

fn lower_aggregate(q: &Query) -> String {
    let mut parts: Vec<String> = q.group_by.iter().map(|c| qident(c)).collect();
    for a in &q.aggregates {
        let expr = match (a.func, &a.col) {
            (AggFunc::Count, None) => "count(*)".to_string(),
            (AggFunc::Count, Some(c)) => format!("count({})", qident(c)),
            (AggFunc::Sum, Some(c)) => format!("sum({})", qident(c)),
            (AggFunc::Avg, Some(c)) => format!("avg({})", qident(c)),
            (AggFunc::Min, Some(c)) => format!("min({})", qident(c)),
            (AggFunc::Max, Some(c)) => format!("max({})", qident(c)),

            _ => "count(*)".to_string(),
        };
        parts.push(format!("{} AS {}", expr, qident(&a.alias)));
    }
    let mut sql = format!("SELECT {} FROM {}", parts.join(", "), qident(&q.from));
    let mut wheres: Vec<String> = Vec::new();
    for f in &q.filters {
        let col = qident(&f.col);
        match &f.op {
            FilterOp::IsNull => wheres.push(format!("{col} IS NULL")),
            FilterOp::IsNotNull => wheres.push(format!("{col} IS NOT NULL")),
            FilterOp::In => {
                if let FilterVal::Many(vs) = &f.value {
                    if vs.is_empty() {
                        wheres.push("0".to_string());
                    } else {
                        let items = vs.iter().map(sql_literal).collect::<Vec<_>>().join(", ");
                        wheres.push(format!("{col} IN ({items})"));
                    }
                }
            }
            op => {
                if let FilterVal::One(v) = &f.value {
                    wheres.push(format!("{col} {} {}", op.sql(), sql_literal(v)));
                }
            }
        }
    }
    if !wheres.is_empty() {
        sql.push_str(&format!(" WHERE {}", wheres.join(" AND ")));
    }
    if !q.group_by.is_empty() {
        let g: Vec<String> = q.group_by.iter().map(|c| qident(c)).collect();
        sql.push_str(&format!(" GROUP BY {}", g.join(", ")));
    }
    if !q.order.is_empty() {
        let keys: Vec<String> = q
            .order
            .iter()
            .map(|(c, desc)| format!("{} {}", qident(c), if *desc { "DESC" } else { "ASC" }))
            .collect();
        sql.push_str(&format!(" ORDER BY {}", keys.join(", ")));
    }
    if let Some(n) = q.limit {
        sql.push_str(&format!(" LIMIT {n}"));
    }
    if let Some(n) = q.offset {
        sql.push_str(&format!(" OFFSET {n}"));
    }
    sql
}

fn aggregate_type(db: &mut Database, q: &Query) -> Vec<(String, CsType)> {
    let cols = derive_columns(db, &q.from);
    let base_of = |c: &str| -> CsType {
        match cols.iter().find(|(n, _)| n == c).map(|(_, t)| t.clone()) {
            Some(CsType::Nullable(inner)) => *inner,
            Some(t) => t,
            None => CsType::Number,
        }
    };
    let mut out: Vec<(String, CsType)> = Vec::new();
    for gc in &q.group_by {
        let ty = cols
            .iter()
            .find(|(n, _)| n == gc)
            .map(|(_, t)| t.clone())
            .unwrap_or(CsType::Number);
        out.push((gc.clone(), ty));
    }
    for a in &q.aggregates {
        let ty = match a.func {
            AggFunc::Count | AggFunc::Sum | AggFunc::Avg => CsType::Number,
            AggFunc::Min | AggFunc::Max => a.col.as_deref().map(base_of).unwrap_or(CsType::Number),
        };
        out.push((a.alias.clone(), ty));
    }
    out
}

struct JoinCtx {

    tables: Vec<(String, Vec<(String, CsType)>)>,

    widened: Vec<String>,
}

impl JoinCtx {
    fn resolve(&self, col: &str) -> Result<(usize, usize), String> {
        if let Some(dot) = col.find('.') {
            let (t, c) = (&col[..dot], &col[dot + 1..]);
            let ti = self
                .tables
                .iter()
                .position(|(n, _)| n == t)
                .ok_or_else(|| format!("missing FROM-clause entry for table \"{t}\""))?;
            let ci = self.tables[ti]
                .1
                .iter()
                .position(|(n, _)| n == c)
                .ok_or_else(|| format!("column \"{col}\" does not exist"))?;
            Ok((ti, ci))
        } else {
            let mut found: Option<(usize, usize)> = None;
            for (ti, (_, cols)) in self.tables.iter().enumerate() {
                if let Some(ci) = cols.iter().position(|(n, _)| n == col) {
                    if found.is_some() {
                        return Err(format!("column reference \"{col}\" is ambiguous"));
                    }
                    found = Some((ti, ci));
                }
            }
            found.ok_or_else(|| format!("column \"{col}\" does not exist"))
        }
    }
    fn qualified(&self, ti: usize, ci: usize) -> String {
        format!(
            "{}.{}",
            qident(&self.tables[ti].0),
            qident(&self.tables[ti].1[ci].0)
        )
    }
    fn alias(&self, ti: usize, ci: usize) -> String {
        format!("{}_{}", self.tables[ti].0, self.tables[ti].1[ci].0)
    }
    fn result_type(&self, ti: usize, ci: usize) -> CsType {
        let base = self.tables[ti].1[ci].1.clone();
        if self.widened.contains(&self.tables[ti].0) {
            match base {
                CsType::Nullable(_) => base,
                other => CsType::Nullable(Box::new(other)),
            }
        } else {
            base
        }
    }
}

fn explicit_projection(db: &mut Database, q: &Query) -> Vec<String> {
    match &q.select {
        Select::Cols(cs) => cs.clone(),
        Select::All => {
            let mut v = Vec::new();
            let mut tabs = vec![q.from.clone()];
            tabs.extend(q.joins.iter().map(|j| j.table.clone()));
            for t in tabs {
                for (c, _) in derive_columns(db, &t) {
                    v.push(format!("{t}.{c}"));
                }
            }
            v
        }
    }
}

fn swap_single_join(q: &Query, new_kind: JoinKind) -> Query {
    let j = &q.joins[0];
    let mut nq = q.clone();
    nq.from = j.table.clone();
    nq.joins = vec![Join {
        kind: new_kind,
        table: q.from.clone(),
        on: j.on.clone(),
    }];
    nq
}

fn force_nullable(t: CsType) -> CsType {
    match t {
        CsType::Nullable(_) => t,
        other => CsType::Nullable(Box::new(other)),
    }
}

fn lower_query_joins(
    db: &mut Database,
    q: &Query,
) -> Result<(String, Vec<(String, CsType)>), String> {
    let has_outer = q
        .joins
        .iter()
        .any(|j| matches!(j.kind, JoinKind::Right | JoinKind::Full));
    if !has_outer {
        return lower_joined(db, q);
    }
    if q.joins.len() != 1 {
        return Err(
            "RIGHT/FULL JOIN combined with other joins is not supported by the rusty-sqlite engine"
                .into(),
        );
    }

    let cols = explicit_projection(db, q);
    let mut base = q.clone();
    base.select = Select::Cols(cols);
    match q.joins[0].kind {
        JoinKind::Right => lower_joined(db, &swap_single_join(&base, JoinKind::Left)),
        JoinKind::Full => {

            let mut left = base.clone();
            left.joins[0].kind = JoinKind::Left;
            let order = std::mem::take(&mut left.order);
            let limit = left.limit.take();
            let offset = left.offset.take();
            let (sql1, ty1) = lower_joined(db, &left)?;

            let mut anti = swap_single_join(&base, JoinKind::Left);
            anti.order.clear();
            anti.limit = None;
            anti.offset = None;
            anti.filters.push(Filter {
                col: q.joins[0].on[0].0.clone(),
                op: FilterOp::IsNull,
                value: FilterVal::One(sql_core::SqlValue::Null),
            });
            let (sql2, _) = lower_joined(db, &anti)?;

            let ty: Vec<(String, CsType)> = ty1
                .into_iter()
                .map(|(n, t)| (n, force_nullable(t)))
                .collect();
            let mut sql = format!("SELECT * FROM ({sql1} UNION ALL {sql2}) AS __full");
            if !order.is_empty() {
                let ctx = build_join_ctx(db, &left)?;
                let mut keys = Vec::with_capacity(order.len());
                for (c, desc) in &order {
                    let (ti, ci) = ctx.resolve(c)?;
                    keys.push(format!(
                        "{} {}",
                        qident(&ctx.alias(ti, ci)),
                        if *desc { "DESC" } else { "ASC" }
                    ));
                }
                sql.push_str(&format!(" ORDER BY {}", keys.join(", ")));
            }
            if let Some(n) = limit {
                sql.push_str(&format!(" LIMIT {n}"));
            }
            if let Some(n) = offset {
                sql.push_str(&format!(" OFFSET {n}"));
            }
            Ok((sql, ty))
        }
        _ => unreachable!("has_outer implies a single Right or Full join"),
    }
}

fn build_join_ctx(db: &mut Database, q: &Query) -> Result<JoinCtx, String> {
    let mut tables: Vec<(String, Vec<(String, CsType)>)> =
        vec![(q.from.clone(), derive_columns(db, &q.from))];
    for j in &q.joins {
        if matches!(j.kind, JoinKind::Right | JoinKind::Full) {
            return Err(format!(
                "{} is not supported by the rusty-sqlite engine",
                j.kind.sql()
            ));
        }
        tables.push((j.table.clone(), derive_columns(db, &j.table)));
    }

    let mut widened: Vec<String> = Vec::new();
    for j in &q.joins {
        if matches!(j.kind, JoinKind::Left) && !widened.iter().any(|n| n == &j.table) {
            widened.push(j.table.clone());
        }
    }
    Ok(JoinCtx { tables, widened })
}

fn lower_joined(db: &mut Database, q: &Query) -> Result<(String, Vec<(String, CsType)>), String> {
    let ctx = build_join_ctx(db, q)?;
    let (col_sql, result_type): (String, Vec<(String, CsType)>) = match &q.select {
        Select::Cols(cols) => {
            let mut parts = Vec::with_capacity(cols.len());
            let mut rt = Vec::with_capacity(cols.len());
            for c in cols {
                let (ti, ci) = ctx.resolve(c)?;
                let alias = ctx.alias(ti, ci);
                parts.push(format!("{} AS {}", ctx.qualified(ti, ci), qident(&alias)));
                rt.push((alias, ctx.result_type(ti, ci)));
            }
            (parts.join(", "), rt)
        }
        Select::All => {
            let mut parts = Vec::new();
            let mut rt = Vec::new();
            for ti in 0..ctx.tables.len() {
                for ci in 0..ctx.tables[ti].1.len() {
                    let alias = ctx.alias(ti, ci);
                    parts.push(format!("{} AS {}", ctx.qualified(ti, ci), qident(&alias)));
                    rt.push((alias, ctx.result_type(ti, ci)));
                }
            }
            (parts.join(", "), rt)
        }
    };
    let mut wheres: Vec<String> = Vec::new();
    for f in &q.filters {
        let (ti, ci) = ctx.resolve(&f.col)?;
        let col = ctx.qualified(ti, ci);
        match &f.op {
            FilterOp::IsNull => wheres.push(format!("{col} IS NULL")),
            FilterOp::IsNotNull => wheres.push(format!("{col} IS NOT NULL")),
            FilterOp::In => {
                if let FilterVal::Many(vs) = &f.value {
                    if vs.is_empty() {
                        wheres.push("0".to_string());
                    } else {
                        let items = vs.iter().map(sql_literal).collect::<Vec<_>>().join(", ");
                        wheres.push(format!("{col} IN ({items})"));
                    }
                }
            }
            op => {
                if let FilterVal::One(v) = &f.value {
                    wheres.push(format!("{col} {} {}", op.sql(), sql_literal(v)));
                }
            }
        }
    }
    let mut sql = format!("SELECT {col_sql} FROM {}", qident(&q.from));
    for j in &q.joins {
        let mut on_parts = Vec::with_capacity(j.on.len());
        for (l, r) in &j.on {
            let (lti, lci) = ctx.resolve(l)?;
            let (rti, rci) = ctx.resolve(r)?;
            on_parts.push(format!(
                "{} = {}",
                ctx.qualified(lti, lci),
                ctx.qualified(rti, rci)
            ));
        }
        sql.push_str(&format!(
            " {} {} ON {}",
            j.kind.sql(),
            qident(&j.table),
            on_parts.join(" AND ")
        ));
    }
    if !wheres.is_empty() {
        sql.push_str(&format!(" WHERE {}", wheres.join(" AND ")));
    }
    if !q.order.is_empty() {
        let mut keys = Vec::with_capacity(q.order.len());
        for (c, desc) in &q.order {
            let (ti, ci) = ctx.resolve(c)?;
            keys.push(format!(
                "{} {}",
                ctx.qualified(ti, ci),
                if *desc { "DESC" } else { "ASC" }
            ));
        }
        sql.push_str(&format!(" ORDER BY {}", keys.join(", ")));
    }
    if let Some(n) = q.limit {
        sql.push_str(&format!(" LIMIT {n}"));
    }
    if let Some(n) = q.offset {
        sql.push_str(&format!(" OFFSET {n}"));
    }
    Ok((sql, result_type))
}

pub fn derive_query_type(db: &mut Database, q: &Query) -> Vec<(String, CsType)> {
    if !q.joins.is_empty() {
        return lower_query_joins(db, q).map(|(_, t)| t).unwrap_or_default();
    }
    if !q.aggregates.is_empty() {
        return aggregate_type(db, q);
    }
    let all = derive_columns(db, &q.from);
    match &q.select {
        Select::All => all,
        Select::Cols(cs) => cs
            .iter()
            .filter_map(|c| all.iter().find(|(n, _)| n == c).cloned())
            .collect(),
    }
}

pub fn run_query(db: &mut Database, q: &Query) -> Result<RowSet, String> {
    let sql = if q.joins.is_empty() {
        validate_single_table_query(db, q)?;
        lower(q)
    } else {
        lower_query_joins(db, q)?.0
    };
    query_rowset(db, &sql)
}

pub fn validate_query(
    db: &mut Database,
    q: &Query,
) -> Result<Result<(), CruftBoundaryError>, String> {
    let (sql, expected) = if q.joins.is_empty() {
        let expected = if !q.aggregates.is_empty() {
            validate_single_table_query(db, q)?;
            aggregate_type(db, q)
        } else {
            validate_single_table_query(db, q)?
        };
        (
            lower(q),
            match &q.select {
                Select::All => expected,
                Select::Cols(cs) => cs
                    .iter()
                    .filter_map(|c| expected.iter().find(|(n, _)| n == c).cloned())
                    .collect(),
            },
        )
    } else {
        lower_query_joins(db, q)?
    };
    let rs = query_rowset(db, &sql)?;
    Ok(validate_result(&rs, &expected))
}

use crizzle_core::dml::{Delete, Insert, Update};

#[derive(Debug)]
pub enum DmlError {
    Query(String),
    Boundary(CruftBoundaryError),
}

#[derive(Debug)]
pub struct WriteResult {
    pub affected: usize,
    pub returned: Option<RowSet>,
}

fn render_filters(filters: &[Filter]) -> Vec<String> {
    let mut out = Vec::new();
    for f in filters {
        let col = qident(&f.col);
        match &f.op {
            FilterOp::IsNull => out.push(format!("{col} IS NULL")),
            FilterOp::IsNotNull => out.push(format!("{col} IS NOT NULL")),
            FilterOp::In => {
                if let FilterVal::Many(vs) = &f.value {
                    if vs.is_empty() {
                        out.push("0".to_string());
                    } else {
                        let items = vs.iter().map(sql_literal).collect::<Vec<_>>().join(", ");
                        out.push(format!("{col} IN ({items})"));
                    }
                }
            }
            op => {
                if let FilterVal::One(v) = &f.value {
                    out.push(format!("{col} {} {}", op.sql(), sql_literal(v)));
                }
            }
        }
    }
    out
}

fn returning_type(
    db: &mut Database,
    table: &str,
    returning: &Option<Select>,
) -> Option<Vec<(String, CsType)>> {
    let sel = returning.as_ref()?;
    let all = derive_columns(db, table);
    Some(match sel {
        Select::All => all,
        Select::Cols(cs) => cs
            .iter()
            .filter_map(|c| all.iter().find(|(n, _)| n == c).cloned())
            .collect(),
    })
}

fn validate_returning(cols: &[(String, CsType)], returning: &Option<Select>) -> Result<(), String> {
    if let Some(Select::Cols(cs)) = returning {
        for c in cs {
            validate_ident(c)?;
            if !cols.iter().any(|(n, _)| n == c) {
                return Err(format!("column \"{c}\" does not exist"));
            }
        }
    }
    Ok(())
}

fn validate_dml_table(db: &mut Database, table: &str) -> Result<Vec<(String, CsType)>, String> {
    validate_ident(table)?;
    let cols = derive_columns(db, table);
    if cols.is_empty() {
        Err(format!("relation \"{table}\" does not exist"))
    } else {
        Ok(cols)
    }
}

fn validate_dml_col(cols: &[(String, CsType)], col: &str) -> Result<(), String> {
    validate_ident(col)?;
    if cols.iter().any(|(n, _)| n == col) {
        Ok(())
    } else {
        Err(format!("column \"{col}\" does not exist"))
    }
}

fn validate_dml_filters(cols: &[(String, CsType)], filters: &[Filter]) -> Result<(), String> {
    for f in filters {
        validate_dml_col(cols, &f.col)?;
    }
    Ok(())
}

fn validate_insert(db: &mut Database, ins: &Insert) -> Result<(), String> {
    let cols = validate_dml_table(db, &ins.into)?;
    for c in &ins.columns {
        validate_dml_col(&cols, c)?;
    }
    if let Some(oc) = &ins.on_conflict {
        for c in &oc.target {
            validate_dml_col(&cols, c)?;
        }
        if let Some(set) = &oc.update {
            for c in set {
                validate_dml_col(&cols, c)?;
            }
        }
    }
    validate_returning(&cols, &ins.returning)
}

fn validate_update(db: &mut Database, up: &Update) -> Result<(), String> {
    let cols = validate_dml_table(db, &up.table)?;
    for (c, _) in &up.set {
        validate_dml_col(&cols, c)?;
    }
    validate_dml_filters(&cols, &up.filters)?;
    validate_returning(&cols, &up.returning)
}

fn validate_delete(db: &mut Database, del: &Delete) -> Result<(), String> {
    let cols = validate_dml_table(db, &del.table)?;
    validate_dml_filters(&cols, &del.filters)?;
    validate_returning(&cols, &del.returning)
}

fn render_returning(returning: &Option<Select>) -> String {
    match returning {
        None => String::new(),
        Some(Select::All) => " RETURNING *".to_string(),
        Some(Select::Cols(cs)) => {
            let cols = cs.iter().map(|c| qident(c)).collect::<Vec<_>>().join(", ");
            format!(" RETURNING {cols}")
        }
    }
}

pub fn lower_insert(ins: &Insert) -> Result<String, String> {
    if ins.columns.is_empty() {
        return Err(format!(
            "INSERT into \"{}\" requires a column list",
            ins.into
        ));
    }
    if ins.rows.is_empty() {
        return Err(format!(
            "INSERT into \"{}\" requires at least one VALUES row",
            ins.into
        ));
    }
    let col_sql = ins
        .columns
        .iter()
        .map(|c| qident(c))
        .collect::<Vec<_>>()
        .join(", ");
    let mut row_sqls = Vec::with_capacity(ins.rows.len());
    for row in &ins.rows {
        if row.len() != ins.columns.len() {
            return Err(format!(
                "INSERT into \"{}\": VALUES row has {} values, expected {}",
                ins.into,
                row.len(),
                ins.columns.len()
            ));
        }
        row_sqls.push(format!(
            "({})",
            row.iter().map(sql_literal).collect::<Vec<_>>().join(", ")
        ));
    }
    let mut sql = format!(
        "INSERT INTO {} ({}) VALUES {}",
        qident(&ins.into),
        col_sql,
        row_sqls.join(", ")
    );
    if let Some(oc) = &ins.on_conflict {
        let target = oc
            .target
            .iter()
            .map(|c| qident(c))
            .collect::<Vec<_>>()
            .join(", ");
        match &oc.update {
            None if oc.target.is_empty() => sql.push_str(" ON CONFLICT DO NOTHING"),
            None => sql.push_str(&format!(" ON CONFLICT ({target}) DO NOTHING")),
            Some(set) => {
                let assigns = set
                    .iter()
                    .map(|c| format!("{} = excluded.{}", qident(c), qident(c)))
                    .collect::<Vec<_>>()
                    .join(", ");
                sql.push_str(&format!(" ON CONFLICT ({target}) DO UPDATE SET {assigns}"));
            }
        }
    }
    sql.push_str(&render_returning(&ins.returning));
    Ok(sql)
}

pub fn lower_update(up: &Update) -> Result<String, String> {
    if up.set.is_empty() {
        return Err(format!(
            "UPDATE \"{}\" requires at least one SET assignment",
            up.table
        ));
    }
    let set_parts: Vec<String> = up
        .set
        .iter()
        .map(|(c, v)| format!("{} = {}", qident(c), sql_literal(v)))
        .collect();
    let mut sql = format!("UPDATE {} SET {}", qident(&up.table), set_parts.join(", "));
    let wheres = render_filters(&up.filters);
    if !wheres.is_empty() {
        sql.push_str(&format!(" WHERE {}", wheres.join(" AND ")));
    }
    sql.push_str(&render_returning(&up.returning));
    Ok(sql)
}

pub fn lower_delete(del: &Delete) -> Result<String, String> {
    let mut sql = format!("DELETE FROM {}", qident(&del.table));
    let wheres = render_filters(&del.filters);
    if !wheres.is_empty() {
        sql.push_str(&format!(" WHERE {}", wheres.join(" AND ")));
    }
    sql.push_str(&render_returning(&del.returning));
    Ok(sql)
}

fn run_write(
    db: &mut Database,
    sql: &str,
    result_type: Option<Vec<(String, CsType)>>,
) -> Result<WriteResult, DmlError> {
    let st = parse_statement(sql).map_err(|e| DmlError::Query(format!("{e:?}")))?;
    match db
        .run(&st, &Bindings::new())
        .map_err(|e| DmlError::Query(format!("{e:?}")))?
    {
        Outcome::Rows { columns, rows } => {
            let n = columns.len();
            let rs = RowSet {
                columns,
                rows: rows
                    .iter()
                    .map(|r| r.iter().map(val_to_sql).collect())
                    .collect(),
                col_bool: vec![false; n],
                col_type_name: vec![String::new(); n],
            };
            if let Some(rt) = &result_type {
                validate_result(&rs, rt).map_err(DmlError::Boundary)?;
            }
            let affected = rs.rows.len();
            Ok(WriteResult {
                affected,
                returned: Some(rs),
            })
        }
        Outcome::Mutation { changes, .. } => Ok(WriteResult {
            affected: changes.max(0) as usize,
            returned: None,
        }),
    }
}

pub fn execute_insert(db: &mut Database, ins: &Insert) -> Result<WriteResult, DmlError> {
    validate_insert(db, ins).map_err(DmlError::Query)?;
    if ins.returning.is_none() && ins.on_conflict.is_none() {
        return match db
            .insert_values_no_returning(&ins.into, &ins.columns, &ins.rows)
            .map_err(DmlError::Query)?
        {
            Outcome::Mutation { changes, .. } => Ok(WriteResult {
                affected: changes.max(0) as usize,
                returned: None,
            }),
            Outcome::Rows { .. } => {
                unreachable!("plain INSERT without RETURNING cannot yield rows")
            }
        };
    }
    let sql = lower_insert(ins).map_err(DmlError::Query)?;
    let rt = returning_type(db, &ins.into, &ins.returning);
    run_write(db, &sql, rt)
}

pub fn execute_insert_owned(db: &mut Database, mut ins: Insert) -> Result<WriteResult, DmlError> {
    validate_insert(db, &ins).map_err(DmlError::Query)?;
    if ins.returning.is_none() && ins.on_conflict.is_none() {
        let rows = std::mem::take(&mut ins.rows);
        return match db
            .insert_values_no_returning_owned(&ins.into, &ins.columns, rows)
            .map_err(DmlError::Query)?
        {
            Outcome::Mutation { changes, .. } => Ok(WriteResult {
                affected: changes.max(0) as usize,
                returned: None,
            }),
            Outcome::Rows { .. } => {
                unreachable!("plain INSERT without RETURNING cannot yield rows")
            }
        };
    }
    execute_insert(db, &ins)
}

pub fn execute_update(db: &mut Database, up: &Update) -> Result<WriteResult, DmlError> {
    validate_update(db, up).map_err(DmlError::Query)?;
    if up.returning.is_none() && up.filters.len() == 1 {
        let f = &up.filters[0];
        if matches!(f.op, FilterOp::In) {
            if let FilterVal::Many(values) = &f.value {
                let affected = db
                    .update_where_in_values_no_returning(&up.table, &up.set, &f.col, values)
                    .map_err(DmlError::Query)?;
                return Ok(WriteResult {
                    affected,
                    returned: None,
                });
            }
        }
    }
    let sql = lower_update(up).map_err(DmlError::Query)?;
    let rt = returning_type(db, &up.table, &up.returning);
    run_write(db, &sql, rt)
}

pub fn execute_delete(db: &mut Database, del: &Delete) -> Result<WriteResult, DmlError> {
    validate_delete(db, del).map_err(DmlError::Query)?;
    if del.returning.is_none() && del.filters.len() == 1 {
        let f = &del.filters[0];
        if matches!(f.op, FilterOp::In) {
            if let FilterVal::Many(values) = &f.value {
                let affected = db
                    .delete_where_in_values_no_returning(&del.table, &f.col, values)
                    .map_err(DmlError::Query)?;
                return Ok(WriteResult {
                    affected,
                    returned: None,
                });
            }
        }
    }
    let sql = lower_delete(del).map_err(DmlError::Query)?;
    let rt = returning_type(db, &del.table, &del.returning);
    run_write(db, &sql, rt)
}

use crizzle_core::{
    propagate_result, sanitize_result, PropagatedRowSet, SanitizeDefaults, SanitizedRowSet,
};

pub fn sanitize(
    db: &mut Database,
    sql: &str,
    expected: &[(String, CsType)],
    defaults: &SanitizeDefaults,
) -> Result<Result<SanitizedRowSet, CruftBoundaryError>, String> {
    let rs = query_rowset(db, sql)?;
    Ok(sanitize_result(&rs, expected, defaults))
}

pub fn propagate(
    db: &mut Database,
    sql: &str,
    expected: &[(String, CsType)],
) -> Result<Result<PropagatedRowSet, CruftBoundaryError>, String> {
    let rs = query_rowset(db, sql)?;
    Ok(propagate_result(&rs, expected))
}
