
use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::catalog::{Catalog, ColumnStats};
use crate::expr::lexer::Tok;
use crate::stmt::lower::QueryResult;
use crate::types::oid;
use crate::types::PgError;
use sql_core::SqlValue;

const STATISTICS_TARGET: usize = 100;

pub fn run(toks: &[Tok], catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    let (targets, cols) = parse(toks)?;

    let tables: Vec<String> = match targets {
        Some(t) => vec![t],
        None => catalog.tables_iter().map(|(n, _)| n.clone()).collect(),
    };

    for table in &tables {
        analyze_table(catalog, table, cols.as_deref())?;
    }
    Ok(QueryResult {
        columns: Vec::new(),
        col_types: Vec::new(),
        rows: Vec::new(),
    })
}

fn parse(toks: &[Tok]) -> Result<(Option<String>, Option<Vec<String>>), PgError> {
    let syntax = || PgError::InvalidInputSyntax {
        typ: "query",
        input: "syntax error in ANALYZE".to_string(),
    };

    let mut i = 1;
    let ident = |t: &Tok| -> Option<String> {
        match t {
            Tok::Ident(s) => Some(s.clone()),
            _ => None,
        }
    };

    if matches!(toks.get(i), Some(Tok::Ident(s)) if s == "verbose") {
        i += 1;
    }

    if matches!(toks.get(i), None | Some(Tok::Eof)) {
        return Ok((None, None));
    }
    let table = ident(&toks[i]).ok_or_else(syntax)?;
    i += 1;

    let mut cols: Option<Vec<String>> = None;
    if matches!(toks.get(i), Some(Tok::LParen)) {
        i += 1;
        let mut list = Vec::new();
        loop {
            let c = toks.get(i).and_then(ident).ok_or_else(syntax)?;
            list.push(c);
            i += 1;
            match toks.get(i) {
                Some(Tok::Comma) => {
                    i += 1;
                    continue;
                }
                Some(Tok::RParen) => {
                    i += 1;
                    break;
                }
                _ => return Err(syntax()),
            }
        }
        cols = Some(list);
    }
    if !matches!(toks.get(i), None | Some(Tok::Eof)) {
        return Err(syntax());
    }
    Ok((Some(table), cols))
}

fn no_such_table(name: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "query",
        input: format!("relation \"{name}\" does not exist"),
    }
}

fn analyze_table(
    catalog: &mut Catalog,
    table: &str,
    cols: Option<&[String]>,
) -> Result<(), PgError> {
    let tab = catalog.get(table).ok_or_else(|| no_such_table(table))?;
    let names = tab.schema.names().to_vec();
    let col_types = tab.col_types.clone();

    let positions: Vec<usize> = match cols {
        Some(cs) => {
            let mut ps = Vec::new();
            for c in cs {
                let p = names.iter().position(|n| n == c).ok_or_else(|| {
                    PgError::InvalidInputSyntax {
                        typ: "query",
                        input: format!("column \"{c}\" of relation \"{table}\" does not exist"),
                    }
                })?;
                ps.push(p);
            }
            ps
        }
        None => (0..names.len()).collect(),
    };

    let rows = catalog
        .visible_rows(table)
        .ok_or_else(|| no_such_table(table))?;
    let total = rows.len();

    let mut computed: BTreeMap<usize, ColumnStats> = BTreeMap::new();
    for &ci in &positions {
        let oid = col_types.get(ci).copied().unwrap_or(0);
        let col_vals: Vec<&SqlValue> = rows.iter().map(|r| &r[ci]).collect();
        if let Some(cs) = compute_column_stats(&col_vals, oid, total) {
            computed.insert(ci, cs);
        }
    }

    let tab = catalog
        .get_table_mut(table)
        .ok_or_else(|| no_such_table(table))?;
    for (ci, cs) in computed {
        tab.stats.insert(ci, cs);
    }

    tab.reltuples = total as f64;
    Ok(())
}

fn compute_column_stats(vals: &[&SqlValue], oid: u32, total: usize) -> Option<ColumnStats> {
    if total == 0 {
        return None;
    }
    let totalf = total as f64;

    let mut nonnull: Vec<(&SqlValue, usize)> = Vec::new();
    let mut null_count = 0usize;
    let mut total_width: usize = 0;
    for v in vals {
        if matches!(v, SqlValue::Null) {
            null_count += 1;
        } else {
            let tupno = nonnull.len();
            total_width += value_width(oid, v);
            nonnull.push((v, tupno));
        }
    }
    let nonnull_cnt = nonnull.len();
    let null_frac = null_count as f64 / totalf;

    if nonnull_cnt == 0 {

        return Some(ColumnStats {
            null_frac,
            avg_width: 0,
            n_distinct: 0.0,
            most_common_vals: Vec::new(),
            most_common_freqs: Vec::new(),
            histogram_bounds: Vec::new(),
            correlation: None,
        });
    }

    let avg_width = (total_width / nonnull_cnt) as i32;

    let mut sorted = nonnull.clone();
    sorted.sort_by(|a, b| value_order(a.0, b.0).then(a.1.cmp(&b.1)));

    let mut groups: Vec<(&SqlValue, usize)> = Vec::new();
    for (v, _) in &sorted {
        match groups.last_mut() {
            Some((prev, cnt)) if value_order(prev, v) == Ordering::Equal => *cnt += 1,
            _ => groups.push((v, 1)),
        }
    }
    let ndistinct = groups.len();
    let nmultiple = groups.iter().filter(|(_, c)| *c > 1).count();

    let mut stadistinct = if nmultiple == 0 {
        -1.0 * (1.0 - null_frac)
    } else {
        ndistinct as f64
    };
    if stadistinct > 0.1 * totalf {
        stadistinct = -(stadistinct / totalf);
    }

    let mut by_count: Vec<(&SqlValue, usize)> = groups.clone();
    by_count.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| value_order(a.0, b.0)));

    let (mut most_common_vals, mut most_common_freqs) = (Vec::new(), Vec::new());
    if nmultiple != 0 {
        if stadistinct > 0.0 && ndistinct <= STATISTICS_TARGET {

            for (v, c) in &by_count {
                most_common_vals.push((*v).clone());
                most_common_freqs.push(*c as f64 / totalf);
            }
        } else {

            let ndistinct_table = if stadistinct < 0.0 {
                -stadistinct * totalf
            } else {
                stadistinct
            };
            let avgcount = nonnull_cnt as f64 / ndistinct_table;
            let mincount = (avgcount * 1.25).max(2.0);
            for (v, c) in &by_count {
                if most_common_vals.len() >= STATISTICS_TARGET {
                    break;
                }
                if (*c as f64) < mincount {
                    break;
                }
                most_common_vals.push((*v).clone());
                most_common_freqs.push(*c as f64 / totalf);
            }
        }
    }

    let mcv_set: Vec<&SqlValue> = most_common_vals.iter().collect();
    let is_mcv = |v: &SqlValue| mcv_set.iter().any(|m| value_order(m, v) == Ordering::Equal);
    let hist_vals: Vec<&SqlValue> = sorted
        .iter()
        .map(|(v, _)| *v)
        .filter(|v| !is_mcv(v))
        .collect();
    let hist_distinct = ndistinct - most_common_vals.len();
    let histogram_bounds = build_histogram(&hist_vals, hist_distinct);

    let correlation = compute_correlation(&sorted);

    Some(ColumnStats {
        null_frac,
        avg_width,
        n_distinct: stadistinct,
        most_common_vals,
        most_common_freqs,
        histogram_bounds,
        correlation,
    })
}

fn build_histogram(vals: &[&SqlValue], distinct: usize) -> Vec<SqlValue> {
    let nvals = vals.len();
    if nvals == 0 {
        return Vec::new();
    }
    let mut num_hist = distinct;
    if num_hist > STATISTICS_TARGET {
        num_hist = STATISTICS_TARGET + 1;
    }
    if num_hist < 2 {
        return Vec::new();
    }
    let mut bounds = Vec::with_capacity(num_hist);
    for i in 0..num_hist {
        let pos = i * (nvals - 1) / (num_hist - 1);
        bounds.push(vals[pos].clone());
    }
    bounds
}

fn compute_correlation(sorted: &[(&SqlValue, usize)]) -> Option<f64> {
    let n = sorted.len();
    if n < 2 {
        return None;
    }
    let nf = n as f64;
    let sum_i: f64 = (0..n).map(|i| i as f64).sum();
    let sum_i2: f64 = (0..n).map(|i| (i * i) as f64).sum();
    let corr_xysum: f64 = sorted
        .iter()
        .enumerate()
        .map(|(i, (_, tupno))| i as f64 * *tupno as f64)
        .sum();
    let num = nf * corr_xysum - sum_i * sum_i;
    let den = nf * sum_i2 - sum_i * sum_i;
    if den == 0.0 {
        return None;
    }
    Some(num / den)
}

fn value_width(oid: u32, v: &SqlValue) -> usize {
    match v {
        SqlValue::Int(_) => match oid {
            oid::INT2 => 2,
            oid::INT8 => 8,
            _ => 4,
        },
        SqlValue::Real(_) => {
            if oid == oid::FLOAT4 {
                4
            } else {
                8
            }
        }
        SqlValue::Text(s) => varlena_width(s.len()),
        SqlValue::Blob(b) => varlena_width(b.len()),
        SqlValue::Null => 0,
    }
}

fn varlena_width(len: usize) -> usize {
    len + if len < 127 { 1 } else { 4 }
}

pub(crate) fn value_order(a: &SqlValue, b: &SqlValue) -> Ordering {
    if let Some(o) = crate::types::numeric::value_cmp(a, b) {
        return o;
    }
    match (a, b) {
        (SqlValue::Real(x), SqlValue::Real(y)) => x.partial_cmp(y).unwrap_or(Ordering::Equal),
        (SqlValue::Text(x), SqlValue::Text(y)) => x.cmp(y),
        (SqlValue::Blob(x), SqlValue::Blob(y)) => x.cmp(y),
        _ => Ordering::Equal,
    }
}
