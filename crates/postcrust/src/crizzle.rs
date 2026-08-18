
use crate::catalog::{Catalog, Table};
use crate::types::oid;

pub use crizzle_core::{value_satisfies, CruftBoundaryError, CsType, Divergence, ViolationKind};

pub use crizzle_core::{PropagationRecord, SanitizationRecord, SanitizeDefaults};

fn cs_type_of_oid(catalog: &Catalog, type_oid: u32) -> CsType {
    match type_oid {
        oid::INT2 | oid::INT4 | oid::OID => CsType::Number,
        oid::INT8 => CsType::BigInt,
        oid::FLOAT4 | oid::FLOAT8 => CsType::Number,

        oid::NUMERIC => CsType::Number,
        oid::BOOL => CsType::Boolean,
        oid::TEXT | oid::VARCHAR | oid::BPCHAR | oid::UUID => CsType::Str,
        oid::BIT | oid::VARBIT => CsType::Str,
        oid::MONEY => CsType::Str,
        oid::DATE | oid::TIMESTAMP | oid::TIMESTAMPTZ | oid::TIME | oid::TIMETZ => CsType::Date,
        oid::BYTEA => CsType::Bytes,
        oid::JSON | oid::JSONB => CsType::Unknown("json"),
        oid::INTERVAL => CsType::Unknown("interval"),

        oid::BOOL_ARRAY => CsType::Array(Box::new(CsType::Boolean)),
        oid::INT2_ARRAY | oid::INT4_ARRAY => CsType::Array(Box::new(CsType::Number)),
        oid::INT8_ARRAY => CsType::Array(Box::new(CsType::BigInt)),
        oid::FLOAT8_ARRAY | oid::NUMERIC_ARRAY => CsType::Array(Box::new(CsType::Number)),
        oid::TEXT_ARRAY => CsType::Array(Box::new(CsType::Str)),

        oid::INET | oid::CIDR | oid::MACADDR | oid::MACADDR8 => CsType::Unknown("network"),
        oid::POINT | oid::LSEG | oid::PATH | oid::BOX | oid::POLYGON | oid::LINE | oid::CIRCLE => {
            CsType::Unknown("geometric")
        }
        oid::INT4RANGE | oid::NUMRANGE | oid::TSRANGE | oid::DATERANGE | oid::INT8RANGE => {
            CsType::Unknown("range")
        }
        oid::INT4MULTIRANGE
        | oid::NUMMULTIRANGE
        | oid::TSMULTIRANGE
        | oid::DATEMULTIRANGE
        | oid::INT8MULTIRANGE => CsType::Unknown("multirange"),
        oid::TSVECTOR | oid::TSQUERY => CsType::Unknown("fulltext"),
        oid::TID | oid::PG_LSN => CsType::Unknown("systemid"),
        other => {

            if let Some(e) = catalog.enum_by_oid(other) {
                CsType::Enum(e.labels.clone())
            } else if let Some(c) = catalog.composite_by_oid(other) {
                let fields = c
                    .fields
                    .iter()
                    .map(|(name, foid, _typmod)| (name.clone(), cs_type_of_oid(catalog, *foid)))
                    .collect();
                CsType::Record(fields)
            } else {
                CsType::Unknown("unmapped")
            }
        }
    }
}

fn column_cs_type(catalog: &Catalog, table: &Table, i: usize) -> (CsType, bool) {

    let inner = cs_type_of_oid(catalog, table.col_types[i]);

    let col_not_null = table.constraints.not_null.get(i).copied().unwrap_or(false);
    let domain_not_null = table
        .col_domains
        .get(i)
        .and_then(|d| d.as_ref())
        .and_then(|name| catalog.get_domain(name))
        .map(|d| d.not_null)
        .unwrap_or(false);
    let nullable = !(col_not_null || domain_not_null);
    (inner, nullable)
}

fn render_column(catalog: &Catalog, table: &Table, i: usize) -> String {
    let (inner, nullable) = column_cs_type(catalog, table, i);
    if nullable {
        format!("{} | null", inner.render())
    } else {
        inner.render()
    }
}

pub fn derive_row_type(catalog: &Catalog, table_name: &str) -> Option<String> {
    let table = catalog.get(table_name)?;
    let names = table.schema.names();
    let mut out = format!("type {table_name} = {{\n");
    for (i, col_name) in names.iter().enumerate() {
        out.push_str(&format!(
            "  {col_name}: {},\n",
            render_column(catalog, table, i)
        ));
    }
    out.push_str("};");
    Some(out)
}

pub fn derive_schema(catalog: &Catalog) -> String {
    let mut names: Vec<&String> = catalog
        .tables_iter()
        .map(|(n, _)| n)
        .filter(|n| !n.starts_with("pg_") && !n.starts_with("information_schema"))
        .collect();
    names.sort();
    names
        .iter()
        .filter_map(|n| derive_row_type(catalog, n))
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn check_row_type(
    catalog: &Catalog,
    table_name: &str,
    declared_columns: &[(&str, &str, bool)],
) -> Vec<Divergence> {
    let mut divergences = Vec::new();

    let table = match catalog.get(table_name) {
        Some(t) => t,
        None => {
            for (name, _, _) in declared_columns {
                divergences.push(Divergence::MissingInDb {
                    column: (*name).to_string(),
                });
            }
            return divergences;
        }
    };

    let live_names = table.schema.names();

    for (decl_name, decl_type, decl_nullable) in declared_columns {
        match live_names.iter().position(|n| n == decl_name) {
            None => divergences.push(Divergence::MissingInDb {
                column: (*decl_name).to_string(),
            }),
            Some(i) => {
                let (inner, actual_nullable) = column_cs_type(catalog, table, i);
                let actual_type = inner.render();
                if actual_type != *decl_type {
                    divergences.push(Divergence::TypeMismatch {
                        column: (*decl_name).to_string(),
                        declared: (*decl_type).to_string(),
                        actual: actual_type,
                    });
                }
                if actual_nullable != *decl_nullable {
                    divergences.push(Divergence::NullabilityMismatch {
                        column: (*decl_name).to_string(),
                        declared_nullable: *decl_nullable,
                        actual_nullable,
                    });
                }
            }
        }
    }

    for live_name in &live_names {
        if !declared_columns.iter().any(|(n, _, _)| n == live_name) {
            divergences.push(Divergence::MissingInDeclaration {
                column: live_name.clone(),
            });
        }
    }

    divergences
}

use crate::stmt::QueryResult;
use sql_core::SqlValue;

fn sql_type_name(oid_val: u32) -> String {
    use oid::*;
    match oid_val {
        BOOL => "boolean".into(),
        INT2 => "smallint".into(),
        INT4 => "integer".into(),
        INT8 => "bigint".into(),
        OID => "oid".into(),
        FLOAT4 => "real".into(),
        FLOAT8 => "double precision".into(),
        NUMERIC => "numeric".into(),
        TEXT => "text".into(),
        VARCHAR => "varchar".into(),
        DATE => "date".into(),
        TIMESTAMP => "timestamp".into(),
        TIMESTAMPTZ => "timestamptz".into(),
        UUID => "uuid".into(),
        BYTEA => "bytea".into(),
        JSON => "json".into(),
        JSONB => "jsonb".into(),
        0 => "unknown".into(),
        other => format!("oid {other}"),
    }
}

fn render_received(value: &SqlValue, oid_val: u32) -> String {
    match value {
        SqlValue::Null => "NULL".to_string(),
        SqlValue::Int(i) => format!("{i} ({})", sql_type_name(oid_val)),
        SqlValue::Real(r) => format!("{r} ({})", sql_type_name(oid_val)),
        SqlValue::Text(s) => format!("\"{s}\" ({})", sql_type_name(oid_val)),
        SqlValue::Blob(b) => format!("<{} bytes> ({})", b.len(), sql_type_name(oid_val)),
    }
}

fn row_set(qr: &QueryResult) -> crizzle_core::RowSet {
    let n = qr.columns.len();
    crizzle_core::RowSet {
        columns: qr.columns.clone(),
        rows: qr.rows.clone(),
        col_bool: (0..n)
            .map(|i| qr.col_types.get(i).copied() == Some(oid::BOOL))
            .collect(),
        col_type_name: (0..n)
            .map(|i| sql_type_name(qr.col_types.get(i).copied().unwrap_or(0)))
            .collect(),
    }
}

pub fn validate_result(
    result: &QueryResult,
    expected: &[(String, CsType)],
) -> Result<(), CruftBoundaryError> {
    crizzle_core::validate_result(&row_set(result), expected)
}

pub fn validate_against_declared(
    result: &QueryResult,
    declared: &[(String, CsType)],
) -> Result<(), CruftBoundaryError> {
    validate_result(result, declared)
}

pub fn validate_against_derived(
    catalog: &Catalog,
    table_name: &str,
    result: &QueryResult,
) -> Result<(), CruftBoundaryError> {
    let expected = derived_columns(catalog, table_name).ok_or_else(|| {
        CruftBoundaryError::new(
            table_name,
            0,
            CsType::Unknown("unmapped"),
            "<table not in catalog>".to_string(),
            ViolationKind::MissingColumn,
        )
    })?;
    validate_result(result, &expected)
}

pub fn derived_columns(catalog: &Catalog, table_name: &str) -> Option<Vec<(String, CsType)>> {
    let table = catalog.get(table_name)?;
    let names = table.schema.names();
    let mut expected: Vec<(String, CsType)> = Vec::with_capacity(names.len());
    for (i, name) in names.iter().enumerate() {
        let (inner, nullable) = column_cs_type(catalog, table, i);
        let ty = if nullable {
            CsType::Nullable(Box::new(inner))
        } else {
            inner
        };
        expected.push((name.clone(), ty));
    }
    Some(expected)
}

#[derive(Debug, Clone)]
pub struct SanitizedResult {
    pub result: QueryResult,
    pub records: Vec<SanitizationRecord>,
}

pub fn sanitize_result(
    result: &QueryResult,
    expected: &[(String, CsType)],
    defaults: &SanitizeDefaults,
) -> Result<SanitizedResult, CruftBoundaryError> {

    for (name, ty) in expected {
        if !result.columns.iter().any(|c| c == name) {
            return Err(CruftBoundaryError::new(
                name,
                0,
                ty.clone(),
                "<absent from result>".to_string(),
                ViolationKind::MissingColumn,
            ));
        }
    }
    for name in &result.columns {
        if !expected.iter().any(|(n, _)| n == name) {
            return Err(CruftBoundaryError::new(
                name,
                0,
                CsType::Unknown("unexpected"),
                "<absent from expected type>".to_string(),
                ViolationKind::UnexpectedColumn,
            ));
        }
    }

    let mut out = result.clone();
    let mut records = Vec::new();
    for (row_index, row) in out.rows.iter_mut().enumerate() {
        for (name, ty) in expected {
            let idx = result
                .columns
                .iter()
                .position(|c| c == name)
                .expect("presence checked above");
            let oid_val = result.col_types.get(idx).copied().unwrap_or(0);
            if let Some(kind) = value_satisfies(&row[idx], oid_val == oid::BOOL, ty) {
                let received = render_received(&row[idx], oid_val);
                match defaults.default_for(ty) {
                    Some(default) => {
                        records.push(SanitizationRecord {
                            column: name.clone(),
                            row_index,
                            expected: ty.clone(),
                            received,
                            kind,
                            replaced_with: default.clone(),
                        });
                        row[idx] = default;
                    }

                    None => {
                        return Err(CruftBoundaryError::new(
                            name,
                            row_index,
                            ty.clone(),
                            received,
                            ViolationKind::NoSanitizerDefault,
                        ));
                    }
                }
            }
        }
    }
    Ok(SanitizedResult {
        result: out,
        records,
    })
}

pub fn sanitize_against_declared(
    result: &QueryResult,
    declared: &[(String, CsType)],
    defaults: &SanitizeDefaults,
) -> Result<SanitizedResult, CruftBoundaryError> {
    sanitize_result(result, declared, defaults)
}

pub fn sanitize_against_derived(
    catalog: &Catalog,
    table_name: &str,
    result: &QueryResult,
    defaults: &SanitizeDefaults,
) -> Result<SanitizedResult, CruftBoundaryError> {
    let table = match catalog.get(table_name) {
        Some(t) => t,
        None => {
            return Err(CruftBoundaryError::new(
                table_name,
                0,
                CsType::Unknown("unmapped"),
                "<table not in catalog>".to_string(),
                ViolationKind::MissingColumn,
            ))
        }
    };
    let names = table.schema.names();
    let mut expected: Vec<(String, CsType)> = Vec::with_capacity(names.len());
    for (i, name) in names.iter().enumerate() {
        let (inner, nullable) = column_cs_type(catalog, table, i);
        let ty = if nullable {
            CsType::Nullable(Box::new(inner))
        } else {
            inner
        };
        expected.push((name.clone(), ty));
    }
    sanitize_result(result, &expected, defaults)
}

#[derive(Debug, Clone)]
pub struct PropagatedResult {
    pub result: QueryResult,
    pub result_type: Vec<(String, CsType)>,
    pub records: Vec<PropagationRecord>,
}

pub fn propagate_result(
    result: &QueryResult,
    expected: &[(String, CsType)],
) -> Result<PropagatedResult, CruftBoundaryError> {

    for (name, ty) in expected {
        if !result.columns.iter().any(|c| c == name) {
            return Err(CruftBoundaryError::new(
                name,
                0,
                ty.clone(),
                "<absent from result>".to_string(),
                ViolationKind::MissingColumn,
            ));
        }
    }
    for name in &result.columns {
        if !expected.iter().any(|(n, _)| n == name) {
            return Err(CruftBoundaryError::new(
                name,
                0,
                CsType::Unknown("unexpected"),
                "<absent from expected type>".to_string(),
                ViolationKind::UnexpectedColumn,
            ));
        }
    }

    let mut records = Vec::new();
    let mut propagated: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (row_index, row) in result.rows.iter().enumerate() {
        for (name, ty) in expected {
            let idx = result
                .columns
                .iter()
                .position(|c| c == name)
                .expect("presence checked above");
            let oid_val = result.col_types.get(idx).copied().unwrap_or(0);
            if let Some(kind) = value_satisfies(&row[idx], oid_val == oid::BOOL, ty) {
                records.push(PropagationRecord {
                    column: name.clone(),
                    row_index,
                    expected: ty.clone(),
                    received: render_received(&row[idx], oid_val),
                    kind,
                });
                propagated.insert(name.clone());
            }
        }
    }

    let result_type = expected
        .iter()
        .map(|(n, t)| {
            if propagated.contains(n) {
                (n.clone(), CsType::Unknown("propagated"))
            } else {
                (n.clone(), t.clone())
            }
        })
        .collect();
    Ok(PropagatedResult {
        result: result.clone(),
        result_type,
        records,
    })
}

pub fn propagate_against_declared(
    result: &QueryResult,
    declared: &[(String, CsType)],
) -> Result<PropagatedResult, CruftBoundaryError> {
    propagate_result(result, declared)
}

pub fn propagate_against_derived(
    catalog: &Catalog,
    table_name: &str,
    result: &QueryResult,
) -> Result<PropagatedResult, CruftBoundaryError> {
    let table = match catalog.get(table_name) {
        Some(t) => t,
        None => {
            return Err(CruftBoundaryError::new(
                table_name,
                0,
                CsType::Unknown("unmapped"),
                "<table not in catalog>".to_string(),
                ViolationKind::MissingColumn,
            ))
        }
    };
    let names = table.schema.names();
    let mut expected: Vec<(String, CsType)> = Vec::with_capacity(names.len());
    for (i, name) in names.iter().enumerate() {
        let (inner, nullable) = column_cs_type(catalog, table, i);
        let ty = if nullable {
            CsType::Nullable(Box::new(inner))
        } else {
            inner
        };
        expected.push((name.clone(), ty));
    }
    propagate_result(result, &expected)
}

pub use crizzle_core::{RelKind, Relation};

pub fn derive_relations(catalog: &Catalog, table_name: &str) -> Vec<Relation> {
    let mut relations = Vec::new();
    let table = match catalog.get(table_name) {
        Some(t) => t,
        None => return relations,
    };
    let local_names = table.schema.names();

    for fk in &table.constraints.foreign_keys {
        if fk.cols.len() != 1 || fk.parent_cols.len() != 1 {
            continue;
        }
        let parent = match catalog.get(&fk.parent) {
            Some(p) => p,
            None => continue,
        };
        let parent_names = parent.schema.names();
        let local = match local_names.get(fk.cols[0]) {
            Some(n) => n.clone(),
            None => continue,
        };
        let target_col = match parent_names.get(fk.parent_cols[0]) {
            Some(n) => n.clone(),
            None => continue,
        };
        relations.push(Relation {
            name: fk.parent.clone(),
            kind: RelKind::BelongsTo,
            local_cols: vec![local],
            target: fk.parent.clone(),
            target_cols: vec![target_col],
        });
    }

    for (child_name, child) in catalog.tables_iter() {
        if child_name == table_name {
            continue;
        }
        let child_names = child.schema.names();
        for fk in &child.constraints.foreign_keys {
            if fk.parent != table_name {
                continue;
            }
            if fk.cols.len() != 1 || fk.parent_cols.len() != 1 {
                continue;
            }
            let local = match local_names.get(fk.parent_cols[0]) {
                Some(n) => n.clone(),
                None => continue,
            };
            let target_col = match child_names.get(fk.cols[0]) {
                Some(n) => n.clone(),
                None => continue,
            };
            relations.push(Relation {
                name: child_name.clone(),
                kind: RelKind::HasMany,
                local_cols: vec![local],
                target: child_name.clone(),
                target_cols: vec![target_col],
            });
        }
    }

    relations
}

pub mod query {
    use super::{
        column_cs_type, derive_relations, propagate_result, sanitize_result, validate_result,
        CruftBoundaryError, CsType, PropagationRecord, RelKind, Relation, SanitizationRecord,
        SanitizeDefaults,
    };
    use crate::catalog::Catalog;
    use crate::stmt::{run_mut, QueryResult};
    use crate::types::PgError;
    use crizzle_core::ident::is_safe_identifier;
    use sql_core::{Row, SqlValue};

    pub use crizzle_core::query::{
        Agg, AggFunc, Filter, FilterOp, FilterVal, Join, JoinKind, Query, Select,
    };

    #[derive(Debug, Clone, PartialEq)]
    pub struct TypedResult {
        pub columns: Vec<(String, CsType)>,
        pub rows: Vec<Row>,
    }

    #[derive(Debug, Clone)]
    pub enum OrmError {
        Query(PgError),
        Boundary(CruftBoundaryError),
    }

    impl OrmError {

        pub fn message(&self) -> String {
            match self {
                OrmError::Query(e) => e.message(),
                OrmError::Boundary(e) => e.message().to_string(),
            }
        }
    }

    pub(super) fn relation_error(table: &str) -> PgError {
        PgError::InvalidInputSyntax {
            typ: "query",
            input: format!("relation \"{table}\" does not exist"),
        }
    }

    pub(super) fn column_error(col: &str) -> PgError {
        PgError::InvalidInputSyntax {
            typ: "query",
            input: format!("column \"{col}\" does not exist"),
        }
    }

    pub(super) fn unsafe_identifier_error(name: &str) -> PgError {
        PgError::InvalidInputSyntax {
            typ: "identifier",
            input: format!("unsafe SQL identifier \"{name}\""),
        }
    }

    pub(super) fn require_safe_identifier(name: &str) -> Result<(), PgError> {
        if is_safe_identifier(name) {
            Ok(())
        } else {
            Err(unsafe_identifier_error(name))
        }
    }

    pub(super) fn ambiguous_error(col: &str) -> PgError {
        PgError::InvalidInputSyntax {
            typ: "query",
            input: format!("column reference \"{col}\" is ambiguous"),
        }
    }

    pub(super) fn missing_from_error(table: &str) -> PgError {
        PgError::InvalidInputSyntax {
            typ: "query",
            input: format!("missing FROM-clause entry for table \"{table}\""),
        }
    }

    pub(super) fn quote_ident(name: &str) -> String {
        debug_assert!(is_safe_identifier(name));
        name.to_string()
    }

    pub(super) fn derived_col_type(
        catalog: &Catalog,
        table: &crate::catalog::Table,
        i: usize,
    ) -> CsType {
        let (inner, nullable) = column_cs_type(catalog, table, i);
        if nullable {
            CsType::Nullable(Box::new(inner))
        } else {
            inner
        }
    }

    pub(super) fn lower_projection(
        catalog: &Catalog,
        table: &crate::catalog::Table,
        select: &Select,
    ) -> Result<(String, Vec<(String, CsType)>), PgError> {
        let names = table.schema.names();
        match select {
            Select::All => {
                let mut parts = Vec::with_capacity(names.len());
                let mut rt = Vec::with_capacity(names.len());
                for (i, name) in names.iter().enumerate() {
                    require_safe_identifier(name)?;
                    parts.push(quote_ident(name));
                    rt.push((name.clone(), derived_col_type(catalog, table, i)));
                }
                Ok((parts.join(", "), rt))
            }
            Select::Cols(cols) => {
                let mut parts = Vec::with_capacity(cols.len());
                let mut rt = Vec::with_capacity(cols.len());
                for c in cols {
                    require_safe_identifier(c)?;
                    let i = names
                        .iter()
                        .position(|n| n == c)
                        .ok_or_else(|| column_error(c))?;
                    parts.push(quote_ident(c));
                    rt.push((c.clone(), derived_col_type(catalog, table, i)));
                }
                Ok((parts.join(", "), rt))
            }
        }
    }

    pub(super) fn lower_filters(
        names: &[String],
        filters: &[Filter],
        params: &mut Vec<SqlValue>,
    ) -> Result<Vec<String>, PgError> {
        let index_of = |c: &str| names.iter().position(|n| n == c);
        let mut where_parts: Vec<String> = Vec::new();
        for f in filters {
            require_safe_identifier(&f.col)?;
            index_of(&f.col).ok_or_else(|| column_error(&f.col))?;
            let col = quote_ident(&f.col);
            match &f.op {
                FilterOp::IsNull => where_parts.push(format!("{col} IS NULL")),
                FilterOp::IsNotNull => where_parts.push(format!("{col} IS NOT NULL")),
                FilterOp::In => {
                    let vals = match &f.value {
                        FilterVal::Many(v) => v,
                        _ => {
                            return Err(PgError::InvalidInputSyntax {
                                typ: "query",
                                input: format!("IN filter on \"{}\" requires a value list", f.col),
                            })
                        }
                    };
                    if vals.is_empty() {

                        where_parts.push("FALSE".to_string());
                    } else {
                        let mut phs = Vec::with_capacity(vals.len());
                        for v in vals {
                            params.push(v.clone());
                            phs.push(format!("${}", params.len()));
                        }
                        where_parts.push(format!("{col} IN ({})", phs.join(", ")));
                    }
                }
                op => {
                    let v = match &f.value {
                        FilterVal::One(v) => v,
                        _ => {
                            return Err(PgError::InvalidInputSyntax {
                                typ: "query",
                                input: format!("filter on \"{}\" requires a single value", f.col),
                            })
                        }
                    };
                    params.push(v.clone());
                    where_parts.push(format!("{col} {} ${}", op.sql(), params.len()));
                }
            }
        }
        Ok(where_parts)
    }

    pub fn lower(
        q: &Query,
        catalog: &Catalog,
    ) -> Result<(String, Vec<SqlValue>, Vec<(String, CsType)>), PgError> {

        if !q.aggregates.is_empty() {
            return lower_aggregate(q, catalog);
        }

        if !q.joins.is_empty() {
            return lower_joined(q, catalog);
        }
        require_safe_identifier(&q.from)?;
        let table = catalog
            .get(&q.from)
            .ok_or_else(|| relation_error(&q.from))?;
        let names = table.schema.names();
        let index_of = |c: &str| names.iter().position(|n| n == c);

        let (col_sql, result_type) = lower_projection(catalog, table, &q.select)?;

        let mut params: Vec<SqlValue> = Vec::new();
        let where_parts = lower_filters(&names, &q.filters, &mut params)?;

        let mut sql = format!("SELECT {col_sql} FROM {}", quote_ident(&q.from));
        if !where_parts.is_empty() {
            sql.push_str(&format!(" WHERE {}", where_parts.join(" AND ")));
        }
        if !q.order.is_empty() {
            let mut keys = Vec::with_capacity(q.order.len());
            for (c, desc) in &q.order {
                require_safe_identifier(c)?;
                index_of(c).ok_or_else(|| column_error(c))?;
                keys.push(format!(
                    "{} {}",
                    quote_ident(c),
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

        Ok((sql, params, result_type))
    }

    fn lower_aggregate(
        q: &Query,
        catalog: &Catalog,
    ) -> Result<(String, Vec<SqlValue>, Vec<(String, CsType)>), PgError> {
        let table = catalog
            .get(&q.from)
            .ok_or_else(|| relation_error(&q.from))?;
        let names = table.schema.names();
        let index_of = |c: &str| names.iter().position(|n| n == c);

        let mut select_parts: Vec<String> = Vec::new();
        let mut result_type: Vec<(String, CsType)> = Vec::new();

        for gc in &q.group_by {
            let i = index_of(gc).ok_or_else(|| column_error(gc))?;
            select_parts.push(quote_ident(gc));
            let (inner, nullable) = column_cs_type(catalog, table, i);
            let ty = if nullable {
                CsType::Nullable(Box::new(inner))
            } else {
                inner
            };
            result_type.push((gc.clone(), ty));
        }

        for a in &q.aggregates {
            let (expr, ty) = match (a.func, &a.col) {
                (AggFunc::Count, None) => ("count(*)".to_string(), CsType::BigInt),
                (AggFunc::Count, Some(c)) => {
                    index_of(c).ok_or_else(|| column_error(c))?;
                    (format!("count({})", quote_ident(c)), CsType::BigInt)
                }
                (AggFunc::Sum, Some(c)) => {
                    index_of(c).ok_or_else(|| column_error(c))?;
                    (format!("sum({})", quote_ident(c)), CsType::Number)
                }
                (AggFunc::Avg, Some(c)) => {
                    index_of(c).ok_or_else(|| column_error(c))?;
                    (format!("avg({})", quote_ident(c)), CsType::Number)
                }
                (AggFunc::Min, Some(c)) | (AggFunc::Max, Some(c)) => {
                    let i = index_of(c).ok_or_else(|| column_error(c))?;
                    let f = if a.func == AggFunc::Min { "min" } else { "max" };
                    (
                        format!("{}({})", f, quote_ident(c)),
                        column_cs_type(catalog, table, i).0,
                    )
                }
                _ => {
                    return Err(PgError::InvalidInputSyntax {
                        typ: "query",
                        input: "aggregate function requires a column".to_string(),
                    })
                }
            };
            select_parts.push(format!("{} AS {}", expr, quote_ident(&a.alias)));
            result_type.push((a.alias.clone(), ty));
        }

        let mut params: Vec<SqlValue> = Vec::new();
        let where_parts = lower_filters(&names, &q.filters, &mut params)?;

        let mut sql = format!(
            "SELECT {} FROM {}",
            select_parts.join(", "),
            quote_ident(&q.from)
        );
        if !where_parts.is_empty() {
            sql.push_str(&format!(" WHERE {}", where_parts.join(" AND ")));
        }
        if !q.group_by.is_empty() {
            let g: Vec<String> = q.group_by.iter().map(|c| quote_ident(c)).collect();
            sql.push_str(&format!(" GROUP BY {}", g.join(", ")));
        }
        if !q.order.is_empty() {

            let keys: Vec<String> = q
                .order
                .iter()
                .map(|(c, desc)| {
                    format!("{} {}", quote_ident(c), if *desc { "DESC" } else { "ASC" })
                })
                .collect();
            sql.push_str(&format!(" ORDER BY {}", keys.join(", ")));
        }
        if let Some(n) = q.limit {
            sql.push_str(&format!(" LIMIT {n}"));
        }
        if let Some(n) = q.offset {
            sql.push_str(&format!(" OFFSET {n}"));
        }

        Ok((sql, params, result_type))
    }

    struct JoinCtx<'a> {
        tables: Vec<(String, &'a crate::catalog::Table)>,
        widened: Vec<String>,
    }

    impl<'a> JoinCtx<'a> {

        fn resolve(&self, col: &str) -> Result<(usize, usize), PgError> {
            if let Some(dot) = col.find('.') {
                let (t, c) = (&col[..dot], &col[dot + 1..]);
                let ti = self
                    .tables
                    .iter()
                    .position(|(n, _)| n == t)
                    .ok_or_else(|| missing_from_error(t))?;
                let ci = self.tables[ti]
                    .1
                    .schema
                    .names()
                    .iter()
                    .position(|n| n == c)
                    .ok_or_else(|| column_error(col))?;
                Ok((ti, ci))
            } else {
                let mut found: Option<(usize, usize)> = None;
                for (ti, (_, tbl)) in self.tables.iter().enumerate() {
                    if let Some(ci) = tbl.schema.names().iter().position(|n| n == col) {
                        if found.is_some() {
                            return Err(ambiguous_error(col));
                        }
                        found = Some((ti, ci));
                    }
                }
                found.ok_or_else(|| column_error(col))
            }
        }

        fn qualified(&self, ti: usize, ci: usize) -> String {
            let names = self.tables[ti].1.schema.names();
            format!("{}.{}", self.tables[ti].0, names[ci])
        }

        fn alias(&self, ti: usize, ci: usize) -> String {
            let names = self.tables[ti].1.schema.names();
            format!("{}_{}", self.tables[ti].0, names[ci])
        }

        fn result_type(&self, catalog: &Catalog, ti: usize, ci: usize) -> CsType {
            let base = derived_col_type(catalog, self.tables[ti].1, ci);
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

    fn build_join_ctx<'a>(q: &Query, catalog: &'a Catalog) -> Result<JoinCtx<'a>, PgError> {
        let from_tbl = catalog
            .get(&q.from)
            .ok_or_else(|| relation_error(&q.from))?;
        let mut tables: Vec<(String, &crate::catalog::Table)> = vec![(q.from.clone(), from_tbl)];
        for j in &q.joins {
            let t = catalog
                .get(&j.table)
                .ok_or_else(|| relation_error(&j.table))?;
            tables.push((j.table.clone(), t));
        }

        let mut widened: Vec<String> = Vec::new();
        let mut left_tables: Vec<String> = vec![q.from.clone()];
        let widen = |name: &str, set: &mut Vec<String>| {
            if !set.iter().any(|n| n == name) {
                set.push(name.to_string());
            }
        };
        for j in &q.joins {
            match j.kind {
                JoinKind::Inner => {}
                JoinKind::Left => widen(&j.table, &mut widened),
                JoinKind::Right => {
                    for lt in &left_tables {
                        widen(lt, &mut widened);
                    }
                }
                JoinKind::Full => {
                    for lt in &left_tables {
                        widen(lt, &mut widened);
                    }
                    widen(&j.table, &mut widened);
                }
            }
            left_tables.push(j.table.clone());
        }

        Ok(JoinCtx { tables, widened })
    }

    fn lower_joined(
        q: &Query,
        catalog: &Catalog,
    ) -> Result<(String, Vec<SqlValue>, Vec<(String, CsType)>), PgError> {
        let ctx = build_join_ctx(q, catalog)?;

        let (col_sql, result_type): (String, Vec<(String, CsType)>) = match &q.select {
            Select::Cols(cols) => {
                let mut parts = Vec::with_capacity(cols.len());
                let mut rt = Vec::with_capacity(cols.len());
                for c in cols {
                    let (ti, ci) = ctx.resolve(c)?;
                    let alias = ctx.alias(ti, ci);
                    parts.push(format!("{} AS {}", ctx.qualified(ti, ci), alias));
                    rt.push((alias, ctx.result_type(catalog, ti, ci)));
                }
                (parts.join(", "), rt)
            }
            Select::All => {

                let mut parts = Vec::new();
                let mut rt = Vec::new();
                for ti in 0..ctx.tables.len() {
                    let names = ctx.tables[ti].1.schema.names();
                    for ci in 0..names.len() {
                        let alias = ctx.alias(ti, ci);
                        parts.push(format!("{} AS {}", ctx.qualified(ti, ci), alias));
                        rt.push((alias, ctx.result_type(catalog, ti, ci)));
                    }
                }
                (parts.join(", "), rt)
            }
        };

        let mut params: Vec<SqlValue> = Vec::new();
        let mut where_parts: Vec<String> = Vec::new();
        for f in &q.filters {
            let (ti, ci) = ctx.resolve(&f.col)?;
            let col = ctx.qualified(ti, ci);
            match &f.op {
                FilterOp::IsNull => where_parts.push(format!("{col} IS NULL")),
                FilterOp::IsNotNull => where_parts.push(format!("{col} IS NOT NULL")),
                FilterOp::In => {
                    let vals = match &f.value {
                        FilterVal::Many(v) => v,
                        _ => {
                            return Err(PgError::InvalidInputSyntax {
                                typ: "query",
                                input: format!("IN filter on \"{}\" requires a value list", f.col),
                            })
                        }
                    };
                    if vals.is_empty() {
                        where_parts.push("FALSE".to_string());
                    } else {
                        let mut phs = Vec::with_capacity(vals.len());
                        for v in vals {
                            params.push(v.clone());
                            phs.push(format!("${}", params.len()));
                        }
                        where_parts.push(format!("{col} IN ({})", phs.join(", ")));
                    }
                }
                op => {
                    let v = match &f.value {
                        FilterVal::One(v) => v,
                        _ => {
                            return Err(PgError::InvalidInputSyntax {
                                typ: "query",
                                input: format!("filter on \"{}\" requires a single value", f.col),
                            })
                        }
                    };
                    params.push(v.clone());
                    where_parts.push(format!("{col} {} ${}", op.sql(), params.len()));
                }
            }
        }

        let mut sql = format!("SELECT {col_sql} FROM {}", quote_ident(&q.from));
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
                quote_ident(&j.table),
                on_parts.join(" AND ")
            ));
        }
        if !where_parts.is_empty() {
            sql.push_str(&format!(" WHERE {}", where_parts.join(" AND ")));
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

        Ok((sql, params, result_type))
    }

    pub(super) fn render_literal(v: &SqlValue) -> String {
        match v {
            SqlValue::Null => "NULL".to_string(),
            SqlValue::Int(i) => format!("(({i})::int8)"),
            SqlValue::Real(r) => format!("(({r})::float8)"),
            SqlValue::Text(s) => format!("(('{}')::text)", s.replace('\'', "''")),
            SqlValue::Blob(b) => {
                let hex: String = b.iter().map(|byte| format!("{byte:02x}")).collect();
                format!("(('\\x{hex}')::bytea)")
            }
        }
    }

    pub(super) fn substitute_params(sql: &str, params: &[SqlValue]) -> String {
        let mut out = sql.to_string();
        for (i, v) in params.iter().enumerate().rev() {
            out = out.replace(&format!("${}", i + 1), &render_literal(v));
        }
        out
    }

    pub fn execute(q: &Query, catalog: &mut Catalog) -> Result<TypedResult, OrmError> {
        let (sql, params, result_type) = lower(q, catalog).map_err(OrmError::Query)?;
        let final_sql = substitute_params(&sql, &params);
        let result: QueryResult = run_mut(&final_sql, catalog).map_err(OrmError::Query)?;
        validate_result(&result, &result_type).map_err(OrmError::Boundary)?;
        Ok(TypedResult {
            columns: result_type,
            rows: result.rows,
        })
    }

    pub fn plan(q: &Query, catalog: &Catalog) -> Result<(String, Vec<(String, CsType)>), OrmError> {
        let (sql, params, result_type) = lower(q, catalog).map_err(OrmError::Query)?;
        Ok((substitute_params(&sql, &params), result_type))
    }

    #[derive(Debug, Clone)]
    pub struct SanitizedTypedResult {
        pub columns: Vec<(String, CsType)>,
        pub rows: Vec<Row>,
        pub records: Vec<SanitizationRecord>,
    }

    pub fn execute_sanitized(
        q: &Query,
        catalog: &mut Catalog,
        defaults: &SanitizeDefaults,
    ) -> Result<SanitizedTypedResult, OrmError> {
        let (sql, params, result_type) = lower(q, catalog).map_err(OrmError::Query)?;
        let final_sql = substitute_params(&sql, &params);
        let result: QueryResult = run_mut(&final_sql, catalog).map_err(OrmError::Query)?;
        let sanitized =
            sanitize_result(&result, &result_type, defaults).map_err(OrmError::Boundary)?;
        Ok(SanitizedTypedResult {
            columns: result_type,
            rows: sanitized.result.rows,
            records: sanitized.records,
        })
    }

    #[derive(Debug, Clone)]
    pub struct PropagatedTypedResult {
        pub columns: Vec<(String, CsType)>,
        pub rows: Vec<Row>,
        pub records: Vec<PropagationRecord>,
    }

    pub fn execute_propagated(
        q: &Query,
        catalog: &mut Catalog,
    ) -> Result<PropagatedTypedResult, OrmError> {
        let (sql, params, result_type) = lower(q, catalog).map_err(OrmError::Query)?;
        let final_sql = substitute_params(&sql, &params);
        let result: QueryResult = run_mut(&final_sql, catalog).map_err(OrmError::Query)?;
        let propagated = propagate_result(&result, &result_type).map_err(OrmError::Boundary)?;
        Ok(PropagatedTypedResult {
            columns: propagated.result_type,
            rows: propagated.result.rows,
            records: propagated.records,
        })
    }

    #[derive(Debug, Clone)]
    pub struct LoadedRelation {
        pub name: String,
        pub kind: RelKind,
        pub child_columns: Vec<(String, CsType)>,
        pub children: Vec<Vec<Row>>,
    }

    impl LoadedRelation {

        fn result_field(&self, nullable: bool) -> (String, CsType) {
            let record = CsType::Record(self.child_columns.clone());
            let ty = match self.kind {
                RelKind::HasMany => CsType::Array(Box::new(record)),
                RelKind::BelongsTo => {
                    if nullable {
                        CsType::Nullable(Box::new(record))
                    } else {
                        record
                    }
                }
            };
            (self.name.clone(), ty)
        }
    }

    #[derive(Debug, Clone)]
    pub struct RelResult {
        pub parent: TypedResult,
        pub relations: Vec<LoadedRelation>,

        nullable: Vec<bool>,
    }

    impl RelResult {

        pub fn result_type(&self) -> Vec<(String, CsType)> {
            let mut ty = self.parent.columns.clone();
            for (rel, &nn) in self.relations.iter().zip(self.nullable.iter()) {
                ty.push(rel.result_field(nn));
            }
            ty
        }

        pub fn children_of(&self, i: usize, name: &str) -> Option<&[Row]> {
            self.relations
                .iter()
                .find(|r| r.name == name)
                .and_then(|r| r.children.get(i))
                .map(|v| v.as_slice())
        }
    }

    pub fn child_query(relation: &Relation, keys: Vec<SqlValue>) -> Query {
        Query::from(&relation.target).filter_in(&relation.target_cols[0], keys)
    }

    pub fn execute_with(q: &Query, catalog: &mut Catalog) -> Result<RelResult, OrmError> {

        let parent = execute(q, catalog)?;

        let derived = derive_relations(catalog, &q.from);
        let mut relations = Vec::new();
        let mut nullable = Vec::new();

        for name in &q.with {
            let relation = match derived.iter().find(|r| &r.name == name) {
                Some(r) => r.clone(),
                None => continue,
            };

            let local_col = &relation.local_cols[0];
            let local_idx = parent.columns.iter().position(|(n, _)| n == local_col);
            let local_idx = match local_idx {
                Some(i) => i,
                None => {

                    relations.push(LoadedRelation {
                        name: relation.name.clone(),
                        kind: relation.kind,
                        child_columns: Vec::new(),
                        children: vec![Vec::new(); parent.rows.len()],
                    });
                    nullable.push(false);
                    continue;
                }
            };

            let local_nullable = matches!(&parent.columns[local_idx].1, CsType::Nullable(_));

            let mut keys: Vec<SqlValue> = Vec::new();
            for row in &parent.rows {
                let v = &row[local_idx];
                if *v == SqlValue::Null {
                    continue;
                }
                if !keys.iter().any(|k| k == v) {
                    keys.push(v.clone());
                }
            }

            let child_q = child_query(&relation, keys);
            let child = execute(&child_q, catalog)?;

            let target_col = &relation.target_cols[0];
            let child_key_idx = child
                .columns
                .iter()
                .position(|(n, _)| n == target_col)
                .expect("child query projects *; target key column present");

            let mut children: Vec<Vec<Row>> = Vec::with_capacity(parent.rows.len());
            for prow in &parent.rows {
                let key = &prow[local_idx];
                let mut matched: Vec<Row> = Vec::new();
                if *key != SqlValue::Null {
                    for crow in &child.rows {
                        if &crow[child_key_idx] == key {
                            matched.push(crow.clone());

                            if relation.kind == RelKind::BelongsTo {
                                break;
                            }
                        }
                    }
                }
                children.push(matched);
            }

            relations.push(LoadedRelation {
                name: relation.name.clone(),
                kind: relation.kind,
                child_columns: child.columns,
                children,
            });
            nullable.push(relation.kind == RelKind::BelongsTo && local_nullable);
        }

        Ok(RelResult {
            parent,
            relations,
            nullable,
        })
    }
}

pub mod dml {
    use super::query::{
        column_error, lower_filters, lower_projection, quote_ident, relation_error,
        require_safe_identifier, substitute_params, Filter, OrmError, Select, TypedResult,
    };
    use super::{validate_result, CsType};
    use crate::catalog::Catalog;
    use crate::stmt::{run_mut, QueryResult};
    use crate::types::PgError;
    use sql_core::SqlValue;

    #[derive(Debug, Clone)]
    pub struct WriteResult {
        pub affected: usize,
        pub returned: Option<TypedResult>,
    }

    pub use crizzle_core::dml::{Delete, Insert, OnConflict, Update};

    pub fn lower_insert(
        ins: &Insert,
        catalog: &Catalog,
    ) -> Result<(String, Vec<SqlValue>, Option<Vec<(String, CsType)>>), PgError> {
        require_safe_identifier(&ins.into)?;
        let table = catalog
            .get(&ins.into)
            .ok_or_else(|| relation_error(&ins.into))?;
        let names = table.schema.names();
        if ins.columns.is_empty() {
            return Err(PgError::InvalidInputSyntax {
                typ: "query",
                input: format!("INSERT into \"{}\" requires a column list", ins.into),
            });
        }

        for c in &ins.columns {
            require_safe_identifier(c)?;
            names
                .iter()
                .position(|n| n == c)
                .ok_or_else(|| column_error(c))?;
        }
        if ins.rows.is_empty() {
            return Err(PgError::InvalidInputSyntax {
                typ: "query",
                input: format!(
                    "INSERT into \"{}\" requires at least one VALUES row",
                    ins.into
                ),
            });
        }

        let col_sql = ins
            .columns
            .iter()
            .map(|c| quote_ident(c))
            .collect::<Vec<_>>()
            .join(", ");

        let mut params: Vec<SqlValue> = Vec::new();
        let mut row_sqls: Vec<String> = Vec::with_capacity(ins.rows.len());
        for row in &ins.rows {
            if row.len() != ins.columns.len() {
                return Err(PgError::InvalidInputSyntax {
                    typ: "query",
                    input: format!(
                        "INSERT into \"{}\": VALUES row has {} values, expected {}",
                        ins.into,
                        row.len(),
                        ins.columns.len()
                    ),
                });
            }
            let mut phs = Vec::with_capacity(row.len());
            for v in row {
                params.push(v.clone());
                phs.push(format!("${}", params.len()));
            }
            row_sqls.push(format!("({})", phs.join(", ")));
        }

        let mut sql = format!(
            "INSERT INTO {} ({}) VALUES {}",
            quote_ident(&ins.into),
            col_sql,
            row_sqls.join(", ")
        );

        if let Some(oc) = &ins.on_conflict {
            for c in &oc.target {
                require_safe_identifier(c)?;
                names
                    .iter()
                    .position(|n| n == c)
                    .ok_or_else(|| column_error(c))?;
            }
            let target = oc
                .target
                .iter()
                .map(|c| quote_ident(c))
                .collect::<Vec<_>>()
                .join(", ");
            match &oc.update {
                None if oc.target.is_empty() => sql.push_str(" ON CONFLICT DO NOTHING"),
                None => sql.push_str(&format!(" ON CONFLICT ({target}) DO NOTHING")),
                Some(set) => {
                    for c in set {
                        require_safe_identifier(c)?;
                        names
                            .iter()
                            .position(|n| n == c)
                            .ok_or_else(|| column_error(c))?;
                    }
                    let assigns = set
                        .iter()
                        .map(|c| format!("{} = EXCLUDED.{}", quote_ident(c), quote_ident(c)))
                        .collect::<Vec<_>>()
                        .join(", ");
                    sql.push_str(&format!(" ON CONFLICT ({target}) DO UPDATE SET {assigns}"));
                }
            }
        }
        let result_type = lower_returning(&ins.returning, catalog, &ins.into, &mut sql)?;
        Ok((sql, params, result_type))
    }

    pub fn execute_insert(ins: &Insert, catalog: &mut Catalog) -> Result<WriteResult, OrmError> {
        let (sql, params, result_type) = lower_insert(ins, catalog).map_err(OrmError::Query)?;

        let affected = ins.rows.len();
        run_write(sql, params, result_type, affected, catalog)
    }

    pub fn lower_update(
        up: &Update,
        catalog: &Catalog,
    ) -> Result<(String, Vec<SqlValue>, Option<Vec<(String, CsType)>>), PgError> {
        require_safe_identifier(&up.table)?;
        let table = catalog
            .get(&up.table)
            .ok_or_else(|| relation_error(&up.table))?;
        let names = table.schema.names();
        if up.set.is_empty() {
            return Err(PgError::InvalidInputSyntax {
                typ: "query",
                input: format!(
                    "UPDATE \"{}\" requires at least one SET assignment",
                    up.table
                ),
            });
        }

        let mut params: Vec<SqlValue> = Vec::new();

        let mut set_parts = Vec::with_capacity(up.set.len());
        for (col, value) in &up.set {
            require_safe_identifier(col)?;
            names
                .iter()
                .position(|n| n == col)
                .ok_or_else(|| column_error(col))?;
            params.push(value.clone());
            set_parts.push(format!("{} = ${}", quote_ident(col), params.len()));
        }

        let where_parts = lower_filters(&names, &up.filters, &mut params)?;

        let mut sql = format!(
            "UPDATE {} SET {}",
            quote_ident(&up.table),
            set_parts.join(", ")
        );
        if !where_parts.is_empty() {
            sql.push_str(&format!(" WHERE {}", where_parts.join(" AND ")));
        }
        let result_type = lower_returning(&up.returning, catalog, &up.table, &mut sql)?;
        Ok((sql, params, result_type))
    }

    pub fn execute_update(up: &Update, catalog: &mut Catalog) -> Result<WriteResult, OrmError> {

        let affected = count_matching(&up.table, &up.filters, catalog)?;
        let (sql, params, result_type) = lower_update(up, catalog).map_err(OrmError::Query)?;
        run_write(sql, params, result_type, affected, catalog)
    }

    pub fn lower_delete(
        del: &Delete,
        catalog: &Catalog,
    ) -> Result<(String, Vec<SqlValue>, Option<Vec<(String, CsType)>>), PgError> {
        require_safe_identifier(&del.table)?;
        let table = catalog
            .get(&del.table)
            .ok_or_else(|| relation_error(&del.table))?;
        let names = table.schema.names();

        let mut params: Vec<SqlValue> = Vec::new();
        let where_parts = lower_filters(&names, &del.filters, &mut params)?;

        let mut sql = format!("DELETE FROM {}", quote_ident(&del.table));
        if !where_parts.is_empty() {
            sql.push_str(&format!(" WHERE {}", where_parts.join(" AND ")));
        }
        let result_type = lower_returning(&del.returning, catalog, &del.table, &mut sql)?;
        Ok((sql, params, result_type))
    }

    pub fn execute_delete(del: &Delete, catalog: &mut Catalog) -> Result<WriteResult, OrmError> {

        let affected = count_matching(&del.table, &del.filters, catalog)?;
        let (sql, params, result_type) = lower_delete(del, catalog).map_err(OrmError::Query)?;
        run_write(sql, params, result_type, affected, catalog)
    }

    fn lower_returning(
        returning: &Option<Select>,
        catalog: &Catalog,
        table_name: &str,
        sql: &mut String,
    ) -> Result<Option<Vec<(String, CsType)>>, PgError> {
        match returning {
            None => Ok(None),
            Some(select) => {
                let table = catalog
                    .get(table_name)
                    .ok_or_else(|| relation_error(table_name))?;
                let (col_sql, result_type) = lower_projection(catalog, table, select)?;
                sql.push_str(&format!(" RETURNING {col_sql}"));
                Ok(Some(result_type))
            }
        }
    }

    fn count_matching(
        table_name: &str,
        filters: &[Filter],
        catalog: &mut Catalog,
    ) -> Result<usize, OrmError> {
        let names = {
            let table = catalog
                .get(table_name)
                .ok_or_else(|| relation_error(table_name))
                .map_err(OrmError::Query)?;
            table.schema.names()
        };
        let mut params: Vec<SqlValue> = Vec::new();
        let where_parts = lower_filters(&names, filters, &mut params).map_err(OrmError::Query)?;
        let mut sql = format!("SELECT count(*) FROM {}", quote_ident(table_name));
        if !where_parts.is_empty() {
            sql.push_str(&format!(" WHERE {}", where_parts.join(" AND ")));
        }
        let final_sql = substitute_params(&sql, &params);
        let result: QueryResult = run_mut(&final_sql, catalog).map_err(OrmError::Query)?;
        match result.rows.first().and_then(|r| r.first()) {
            Some(SqlValue::Int(n)) => Ok((*n).max(0) as usize),
            _ => Ok(0),
        }
    }

    fn run_write(
        sql: String,
        params: Vec<SqlValue>,
        result_type: Option<Vec<(String, CsType)>>,
        affected: usize,
        catalog: &mut Catalog,
    ) -> Result<WriteResult, OrmError> {
        let final_sql = substitute_params(&sql, &params);
        let result: QueryResult = run_mut(&final_sql, catalog).map_err(OrmError::Query)?;
        let returned = match result_type {
            Some(rt) => {
                validate_result(&result, &rt).map_err(OrmError::Boundary)?;
                Some(TypedResult {
                    columns: rt,
                    rows: result.rows,
                })
            }
            None => None,
        };
        Ok(WriteResult { affected, returned })
    }
}
