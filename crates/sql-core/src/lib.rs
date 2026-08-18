
use std::cmp::Ordering;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),

}

impl SqlValue {
    fn type_rank(&self) -> u8 {
        match self {
            SqlValue::Null => 0,
            SqlValue::Int(_) => 1,
            SqlValue::Real(_) => 2,
            SqlValue::Text(_) => 3,
            SqlValue::Blob(_) => 4,
        }
    }

    pub fn cmp(&self, other: &SqlValue) -> Ordering {
        let (ra, rb) = (self.type_rank(), other.type_rank());
        if ra != rb {
            return ra.cmp(&rb);
        }
        match (self, other) {
            (SqlValue::Null, SqlValue::Null) => Ordering::Equal,
            (SqlValue::Int(a), SqlValue::Int(b)) => a.cmp(b),
            (SqlValue::Real(a), SqlValue::Real(b)) => a.total_cmp(b),
            (SqlValue::Text(a), SqlValue::Text(b)) => a.cmp(b),
            (SqlValue::Blob(a), SqlValue::Blob(b)) => a.cmp(b),
            _ => Ordering::Equal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullsPlacement {
    First,
    Last,
}

impl NullsPlacement {
    pub const fn from_bool(nulls_first: bool) -> Self {
        if nulls_first {
            Self::First
        } else {
            Self::Last
        }
    }

    pub const fn is_first(self) -> bool {
        matches!(self, Self::First)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullsDefault {
    Sqlite,
    Postgres,
}

impl NullsDefault {
    pub const fn placement(self, descending: bool) -> NullsPlacement {
        match self {

            Self::Sqlite => NullsPlacement::from_bool(!descending),

            Self::Postgres => NullsPlacement::from_bool(descending),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextCollation {
    Binary,
    AsciiNoCase,
    RTrim,
}

impl TextCollation {
    pub fn compare(self, a: &str, b: &str) -> Ordering {
        match self {
            Self::Binary => a.cmp(b),
            Self::AsciiNoCase => a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()),
            Self::RTrim => a.trim_end_matches(' ').cmp(b.trim_end_matches(' ')),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortOptions {
    pub descending: bool,
    pub nulls: NullsPlacement,
    pub collation: TextCollation,
}

impl SortOptions {
    pub const fn new(descending: bool, nulls: NullsPlacement, collation: TextCollation) -> Self {
        Self {
            descending,
            nulls,
            collation,
        }
    }

    pub const fn with_default(
        descending: bool,
        nulls_first: Option<bool>,
        default: NullsDefault,
        collation: TextCollation,
    ) -> Self {
        Self::new(
            descending,
            match nulls_first {
                Some(nf) => NullsPlacement::from_bool(nf),
                None => default.placement(descending),
            },
            collation,
        )
    }
}

pub fn sort_cmp(a: &SqlValue, b: &SqlValue) -> Ordering {
    match (matches!(a, SqlValue::Null), matches!(b, SqlValue::Null)) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        (false, false) => a.cmp(b),
    }
}

pub fn sort_cmp_nulls(
    a: &SqlValue,
    b: &SqlValue,
    desc: bool,
    nulls_first: Option<bool>,
) -> Ordering {
    let an = matches!(a, SqlValue::Null);
    let bn = matches!(b, SqlValue::Null);
    match (an, bn) {
        (true, true) => Ordering::Equal,
        (false, false) => {
            let o = a.cmp(b);
            if desc {
                o.reverse()
            } else {
                o
            }
        }

        _ => {

            let nf = nulls_first.unwrap_or(!desc);

            if an == nf {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        }
    }
}

pub fn sort_cmp_with_options(a: &SqlValue, b: &SqlValue, options: SortOptions) -> Ordering {
    let an = matches!(a, SqlValue::Null);
    let bn = matches!(b, SqlValue::Null);
    match (an, bn) {
        (true, true) => Ordering::Equal,
        (false, false) => {
            let o = match (a, b) {
                (SqlValue::Text(x), SqlValue::Text(y)) => options.collation.compare(x, y),
                _ => a.cmp(b),
            };
            if options.descending {
                o.reverse()
            } else {
                o
            }
        }
        _ => {
            if an == options.nulls.is_first() {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        }
    }
}

pub type Row = Vec<SqlValue>;

#[derive(Clone, PartialEq, Debug)]
pub struct IndexKey(pub SqlValue);
impl Eq for IndexKey {}
impl PartialOrd for IndexKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for IndexKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

#[derive(Clone, Debug)]
pub struct EqIndex {
    map: BTreeMap<IndexKey, Vec<usize>>,
}
impl EqIndex {

    pub fn build<I: IntoIterator<Item = SqlValue>>(keys: I) -> EqIndex {
        let mut map: BTreeMap<IndexKey, Vec<usize>> = BTreeMap::new();
        for (i, k) in keys.into_iter().enumerate() {
            map.entry(IndexKey(k)).or_default().push(i);
        }
        EqIndex { map }
    }

    pub fn probe(&self, key: &SqlValue) -> &[usize] {
        self.map
            .get(&IndexKey(key.clone()))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn range(
        &self,
        lo: std::ops::Bound<&SqlValue>,
        hi: std::ops::Bound<&SqlValue>,
    ) -> Vec<usize> {
        use std::ops::Bound;
        fn to_key(b: Bound<&SqlValue>) -> Bound<IndexKey> {
            match b {
                Bound::Included(v) => Bound::Included(IndexKey(v.clone())),
                Bound::Excluded(v) => Bound::Excluded(IndexKey(v.clone())),
                Bound::Unbounded => Bound::Unbounded,
            }
        }

        let empty = match (lo, hi) {
            (Bound::Included(a), Bound::Included(b)) => a.cmp(b) == Ordering::Greater,
            (Bound::Included(a), Bound::Excluded(b))
            | (Bound::Excluded(a), Bound::Included(b))
            | (Bound::Excluded(a), Bound::Excluded(b)) => a.cmp(b) != Ordering::Less,
            _ => false,
        };
        if empty {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (_k, positions) in self.map.range((to_key(lo), to_key(hi))) {
            out.extend_from_slice(positions);
        }
        out
    }

    pub fn insert(&mut self, key: SqlValue, pos: usize) {
        self.map.entry(IndexKey(key)).or_default().push(pos);
    }
}

#[derive(Clone, Debug)]
pub struct EqIndexN {
    map: BTreeMap<Vec<IndexKey>, Vec<usize>>,
}
impl EqIndexN {

    pub fn build<I: IntoIterator<Item = Vec<SqlValue>>>(keys: I) -> EqIndexN {
        let mut map: BTreeMap<Vec<IndexKey>, Vec<usize>> = BTreeMap::new();
        for (i, tup) in keys.into_iter().enumerate() {
            map.entry(tup.into_iter().map(IndexKey).collect())
                .or_default()
                .push(i);
        }
        EqIndexN { map }
    }

    pub fn probe(&self, key: &[SqlValue]) -> &[usize] {
        let k: Vec<IndexKey> = key.iter().cloned().map(IndexKey).collect();
        self.map.get(&k).map(Vec::as_slice).unwrap_or(&[])
    }
}

pub type Scalar = Box<dyn Fn(&Row) -> Result<SqlValue, String>>;

pub type Pred = Box<dyn Fn(&Row) -> Result<bool, String>>;

const DEFAULT_MAX_ROWS: usize = 1_000_000;
const DEFAULT_MAX_CELLS: usize = 16_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionLimits {
    pub max_rows: usize,
    pub max_cells: usize,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            max_rows: DEFAULT_MAX_ROWS,
            max_cells: DEFAULT_MAX_CELLS,
        }
    }
}

impl ExecutionLimits {
    fn check_rows(self, op: &str, rows: &[Row]) -> Result<(), String> {
        if rows.len() > self.max_rows {
            return Err(format!(
                "sql-core resource limit exceeded in {op}: {} rows > {}",
                rows.len(),
                self.max_rows
            ));
        }
        let cells = rows
            .iter()
            .try_fold(0usize, |acc, row| acc.checked_add(row.len()))
            .ok_or_else(|| {
                format!("sql-core resource limit exceeded in {op}: cell count overflow")
            })?;
        if cells > self.max_cells {
            return Err(format!(
                "sql-core resource limit exceeded in {op}: {cells} cells > {}",
                self.max_cells
            ));
        }
        Ok(())
    }

    fn check_len_width(self, op: &str, len: usize, width: usize) -> Result<(), String> {
        if len > self.max_rows {
            return Err(format!(
                "sql-core resource limit exceeded in {op}: {len} rows > {}",
                self.max_rows
            ));
        }
        let cells = len.checked_mul(width).ok_or_else(|| {
            format!("sql-core resource limit exceeded in {op}: cell count overflow")
        })?;
        if cells > self.max_cells {
            return Err(format!(
                "sql-core resource limit exceeded in {op}: {cells} cells > {}",
                self.max_cells
            ));
        }
        Ok(())
    }

    fn check_push_cells(
        self,
        op: &str,
        len_before: usize,
        cells_before: usize,
        next_width: usize,
    ) -> Result<usize, String> {
        let len = len_before.checked_add(1).ok_or_else(|| {
            format!("sql-core resource limit exceeded in {op}: row count overflow")
        })?;
        let cells = cells_before.checked_add(next_width).ok_or_else(|| {
            format!("sql-core resource limit exceeded in {op}: cell count overflow")
        })?;
        if len > self.max_rows {
            return Err(format!(
                "sql-core resource limit exceeded in {op}: {len} rows > {}",
                self.max_rows
            ));
        }
        if cells > self.max_cells {
            return Err(format!(
                "sql-core resource limit exceeded in {op}: {cells} cells > {}",
                self.max_cells
            ));
        }
        Ok(cells)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JoinKind {
    Inner,
    Left,
    Cross,

    Right,

    Full,
}

pub enum Plan {

    Scan(Vec<Row>),

    Values(Vec<Row>),

    Filter { input: Box<Plan>, pred: Pred },

    Project { input: Box<Plan>, cols: Vec<Scalar> },

    Sort {
        input: Box<Plan>,
        keys: Vec<(Scalar, SortOptions)>,
    },

    Limit {
        input: Box<Plan>,
        limit: Option<usize>,
        offset: usize,
    },

    NestedLoopJoin {
        left: Box<Plan>,
        right: Box<Plan>,
        left_width: usize,
        right_width: usize,
        kind: JoinKind,
        pred: Option<Pred>,
    },

    HashJoin {
        left: Box<Plan>,
        right: Box<Plan>,
        left_width: usize,
        right_width: usize,
        kind: JoinKind,
        left_keys: Vec<Scalar>,
        right_keys: Vec<Scalar>,
        extra: Option<Pred>,
    },

    Aggregate {
        input: Box<Plan>,
        key: Vec<Scalar>,
        #[allow(clippy::type_complexity)]
        output: Box<dyn Fn(&[Row]) -> Result<Option<Row>, String>>,
    },
}

impl Plan {

    pub fn execute(&self) -> Result<Vec<Row>, String> {
        self.execute_with_limits(ExecutionLimits::default())
    }

    pub fn execute_with_limits(&self, limits: ExecutionLimits) -> Result<Vec<Row>, String> {
        match self {
            Plan::Scan(rows) | Plan::Values(rows) => {
                limits.check_rows("scan", rows)?;
                Ok(rows.clone())
            }
            Plan::Filter { input, pred } => {
                let mut out = Vec::new();
                let mut cells = 0usize;
                for r in input.execute_with_limits(limits)? {
                    if pred(&r)? {
                        cells = limits.check_push_cells("filter", out.len(), cells, r.len())?;
                        out.push(r);
                    }
                }
                Ok(out)
            }
            Plan::Project { input, cols } => {
                let mut out = Vec::new();
                let mut cells = 0usize;
                for r in input.execute_with_limits(limits)? {
                    let mut row = Vec::with_capacity(cols.len());
                    for c in cols {
                        row.push(c(&r)?);
                    }
                    cells = limits.check_push_cells("project", out.len(), cells, row.len())?;
                    out.push(row);
                }
                Ok(out)
            }
            Plan::Sort { input, keys } => {
                let mut rows = input.execute_with_limits(limits)?;

                let mut keyed: Vec<(Vec<SqlValue>, Row)> = Vec::with_capacity(rows.len());
                for r in rows.drain(..) {
                    let mut k = Vec::with_capacity(keys.len());
                    for (kf, _) in keys {
                        k.push(kf(&r)?);
                    }
                    limits.check_len_width("sort keys", keyed.len() + 1, k.len())?;
                    keyed.push((k, r));
                }
                keyed.sort_by(|a, b| {
                    for (i, (_, options)) in keys.iter().enumerate() {
                        let ord = sort_cmp_with_options(&a.0[i], &b.0[i], *options);
                        if ord != Ordering::Equal {
                            return ord;
                        }
                    }
                    Ordering::Equal
                });
                Ok(keyed.into_iter().map(|(_, r)| r).collect())
            }
            Plan::Limit {
                input,
                limit,
                offset,
            } => {
                let rows = input.execute_with_limits(limits)?;
                let it = rows.into_iter().skip(*offset);
                let out: Vec<Row> = match limit {
                    Some(n) => it.take(*n).collect(),
                    None => it.collect(),
                };
                limits.check_rows("limit", &out)?;
                Ok(out)
            }
            Plan::NestedLoopJoin {
                left,
                right,
                left_width,
                right_width,
                kind,
                pred,
            } => {
                let lrows = left.execute_with_limits(limits)?;
                let rrows = right.execute_with_limits(limits)?;
                let keeps_left_unmatched = matches!(kind, JoinKind::Left | JoinKind::Full);
                let keeps_right_unmatched = matches!(kind, JoinKind::Right | JoinKind::Full);

                let mut right_matched = vec![false; rrows.len()];
                let mut out = Vec::new();
                let mut cells = 0usize;
                for l in &lrows {
                    let mut matched = false;
                    for (ri, r) in rrows.iter().enumerate() {
                        let mut combined = l.clone();
                        combined.extend_from_slice(r);
                        let keep = match pred {
                            Some(p) => p(&combined)?,
                            None => true,
                        };
                        if keep {
                            matched = true;
                            right_matched[ri] = true;
                            cells = limits.check_push_cells(
                                "nested-loop join",
                                out.len(),
                                cells,
                                combined.len(),
                            )?;
                            out.push(combined);
                        }
                    }
                    if !matched && keeps_left_unmatched {
                        let mut combined = l.clone();
                        combined.extend(std::iter::repeat(SqlValue::Null).take(*right_width));
                        cells = limits.check_push_cells(
                            "nested-loop join",
                            out.len(),
                            cells,
                            combined.len(),
                        )?;
                        out.push(combined);
                    }
                }
                if keeps_right_unmatched {
                    for (ri, r) in rrows.iter().enumerate() {
                        if !right_matched[ri] {
                            let mut combined = vec![SqlValue::Null; *left_width];
                            combined.extend_from_slice(r);
                            cells = limits.check_push_cells(
                                "nested-loop join",
                                out.len(),
                                cells,
                                combined.len(),
                            )?;
                            out.push(combined);
                        }
                    }
                }
                Ok(out)
            }
            Plan::HashJoin {
                left,
                right,
                left_width,
                right_width,
                kind,
                left_keys,
                right_keys,
                extra,
            } => {
                let lrows = left.execute_with_limits(limits)?;
                let rrows = right.execute_with_limits(limits)?;
                let keeps_left_unmatched = matches!(kind, JoinKind::Left | JoinKind::Full);
                let keeps_right_unmatched = matches!(kind, JoinKind::Right | JoinKind::Full);

                let mut table: BTreeMap<Vec<IndexKey>, Vec<usize>> = BTreeMap::new();
                for (ri, r) in rrows.iter().enumerate() {
                    let mut key = Vec::with_capacity(right_keys.len());
                    let mut has_null = false;
                    for k in right_keys {
                        let v = k(r)?;
                        if matches!(v, SqlValue::Null) {
                            has_null = true;
                            break;
                        }
                        key.push(IndexKey(v));
                    }
                    if has_null {
                        continue;
                    }
                    table.entry(key).or_default().push(ri);
                }
                let mut right_matched = vec![false; rrows.len()];
                let mut out = Vec::new();
                let mut cells = 0usize;
                for l in &lrows {
                    let mut matched = false;

                    let mut key = Vec::with_capacity(left_keys.len());
                    let mut has_null = false;
                    for k in left_keys {
                        let v = k(l)?;
                        if matches!(v, SqlValue::Null) {
                            has_null = true;
                            break;
                        }
                        key.push(IndexKey(v));
                    }
                    if !has_null {
                        if let Some(bucket) = table.get(&key) {
                            for &ri in bucket {
                                let mut combined = l.clone();
                                combined.extend_from_slice(&rrows[ri]);
                                let keep = match extra {
                                    Some(p) => p(&combined)?,
                                    None => true,
                                };
                                if keep {
                                    matched = true;
                                    right_matched[ri] = true;
                                    cells = limits.check_push_cells(
                                        "hash join",
                                        out.len(),
                                        cells,
                                        combined.len(),
                                    )?;
                                    out.push(combined);
                                }
                            }
                        }
                    }
                    if !matched && keeps_left_unmatched {
                        let mut combined = l.clone();
                        combined.extend(std::iter::repeat(SqlValue::Null).take(*right_width));
                        cells = limits.check_push_cells(
                            "hash join",
                            out.len(),
                            cells,
                            combined.len(),
                        )?;
                        out.push(combined);
                    }
                }
                if keeps_right_unmatched {
                    for (ri, r) in rrows.iter().enumerate() {
                        if !right_matched[ri] {
                            let mut combined = vec![SqlValue::Null; *left_width];
                            combined.extend_from_slice(r);
                            cells = limits.check_push_cells(
                                "hash join",
                                out.len(),
                                cells,
                                combined.len(),
                            )?;
                            out.push(combined);
                        }
                    }
                }
                Ok(out)
            }
            Plan::Aggregate { input, key, output } => {
                let rows = input.execute_with_limits(limits)?;

                let mut groups: Vec<(Vec<SqlValue>, Vec<Row>)> = Vec::new();
                for r in rows {
                    let k: Vec<SqlValue> = key.iter().map(|kf| kf(&r)).collect::<Result<_, _>>()?;
                    match groups.iter_mut().find(|(gk, _)| {
                        gk.len() == k.len()
                            && gk.iter().zip(&k).all(|(a, b)| a.cmp(b) == Ordering::Equal)
                    }) {
                        Some((_, v)) => v.push(r),
                        None => groups.push((k, vec![r])),
                    }
                }

                if key.is_empty() && groups.is_empty() {
                    groups.push((Vec::new(), Vec::new()));
                }
                let mut out = Vec::new();
                let mut cells = 0usize;
                for (_, group_rows) in &groups {
                    if let Some(row) = output(group_rows)? {
                        cells =
                            limits.check_push_cells("aggregate", out.len(), cells, row.len())?;
                        out.push(row);
                    }
                }
                Ok(out)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(rows: Vec<Vec<SqlValue>>) -> Box<Plan> {
        Box::new(Plan::Scan(rows))
    }
    fn i(n: i64) -> SqlValue {
        SqlValue::Int(n)
    }

    #[test]
    fn filter_project_sort_limit() {

        let rows: Vec<Row> = [5, 1, 3, 2, 4]
            .iter()
            .enumerate()
            .map(|(id, v)| vec![i(id as i64), i(*v)])
            .collect();

        let plan = Plan::Limit {
            input: Box::new(Plan::Project {
                input: Box::new(Plan::Sort {
                    input: Box::new(Plan::Filter {
                        input: scan(rows),
                        pred: Box::new(|r| Ok(matches!(&r[1], SqlValue::Int(n) if *n >= 2))),
                    }),
                    keys: vec![(
                        Box::new(|r: &Row| Ok(r[1].clone())),
                        SortOptions::with_default(
                            true,
                            None,
                            NullsDefault::Sqlite,
                            TextCollation::Binary,
                        ),
                    )],
                }),
                cols: vec![Box::new(|r: &Row| Ok(r[1].clone()))],
            }),
            limit: Some(2),
            offset: 0,
        };
        let out = plan.execute().unwrap();
        assert_eq!(out, vec![vec![i(5)], vec![i(4)]]);
    }

    #[test]
    fn execution_limits_reject_oversized_scan() {
        let plan = Plan::Scan(vec![vec![i(1)], vec![i(2)], vec![i(3)]]);
        let err = plan
            .execute_with_limits(ExecutionLimits {
                max_rows: 2,
                max_cells: 10,
            })
            .unwrap_err();
        assert!(err.contains("resource limit exceeded"));
        assert!(err.contains("scan"));
    }

    #[test]
    fn execution_limits_reject_join_blowup_before_unbounded_materialization() {
        let plan = Plan::NestedLoopJoin {
            left: scan(vec![vec![i(1)], vec![i(2)]]),
            right: scan(vec![vec![i(10)], vec![i(20)]]),
            left_width: 1,
            right_width: 1,
            kind: JoinKind::Cross,
            pred: None,
        };
        let err = plan
            .execute_with_limits(ExecutionLimits {
                max_rows: 3,
                max_cells: 100,
            })
            .unwrap_err();
        assert!(err.contains("nested-loop join"));
    }

    #[test]
    fn execution_limits_reject_cell_blowup() {
        let plan = Plan::Project {
            input: scan(vec![vec![i(1)], vec![i(2)]]),
            cols: vec![
                Box::new(|r: &Row| Ok(r[0].clone())),
                Box::new(|r: &Row| Ok(r[0].clone())),
                Box::new(|r: &Row| Ok(r[0].clone())),
            ],
        };
        let err = plan
            .execute_with_limits(ExecutionLimits {
                max_rows: 10,
                max_cells: 5,
            })
            .unwrap_err();
        assert!(err.contains("project"));
        assert!(err.contains("cells"));
    }

    #[test]
    fn eq_index_probes_in_scan_order() {

        let idx = EqIndex::build([10, 20, 10, 30, 20].iter().map(|n| i(*n)));
        assert_eq!(idx.probe(&i(10)), &[0, 2]);
        assert_eq!(idx.probe(&i(20)), &[1, 4]);
        assert_eq!(idx.probe(&i(30)), &[3]);
        assert_eq!(idx.probe(&i(99)), &[] as &[usize]);
    }

    #[test]
    fn eq_index_range_scans_in_key_then_position_order() {
        use std::ops::Bound::{Excluded, Included, Unbounded};

        let idx = EqIndex::build([10, 20, 10, 30, 20].iter().map(|n| i(*n)));

        assert_eq!(idx.range(Excluded(&i(10)), Unbounded), &[1, 4, 3]);

        assert_eq!(idx.range(Included(&i(10)), Excluded(&i(20))), &[0, 2]);

        assert_eq!(idx.range(Included(&i(20)), Included(&i(30))), &[1, 4, 3]);

        assert_eq!(idx.range(Unbounded, Included(&i(10))), &[0, 2]);

        assert_eq!(idx.range(Excluded(&i(30)), Unbounded), &[] as &[usize]);
        assert_eq!(
            idx.range(Included(&i(11)), Excluded(&i(20))),
            &[] as &[usize]
        );
    }

    #[test]
    fn eq_index_n_probes_tuple_in_scan_order() {
        fn t(s: &str) -> SqlValue {
            SqlValue::Text(s.into())
        }

        let rows = vec![
            vec![i(1), t("x")],
            vec![i(2), t("y")],
            vec![i(1), t("x")],
            vec![i(1), t("z")],
            vec![i(2), t("y")],
        ];
        let idx = EqIndexN::build(rows);
        assert_eq!(idx.probe(&[i(1), t("x")]), &[0, 2]);
        assert_eq!(idx.probe(&[i(2), t("y")]), &[1, 4]);
        assert_eq!(idx.probe(&[i(1), t("z")]), &[3]);
        assert_eq!(idx.probe(&[i(1), t("y")]), &[] as &[usize]);
        assert_eq!(idx.probe(&[i(9), t("x")]), &[] as &[usize]);
    }

    #[test]
    fn left_join_pads_nulls() {
        let left = Plan::Scan(vec![vec![i(1)], vec![i(2)]]);
        let right = Plan::Scan(vec![vec![i(1), SqlValue::Text("a".into())]]);
        let plan = Plan::NestedLoopJoin {
            left: Box::new(left),
            right: Box::new(right),
            left_width: 1,
            right_width: 2,
            kind: JoinKind::Left,
            pred: Some(Box::new(|r: &Row| Ok(r[0].cmp(&r[1]) == Ordering::Equal))),
        };
        let out = plan.execute().unwrap();

        assert_eq!(out[0], vec![i(1), i(1), SqlValue::Text("a".into())]);
        assert_eq!(out[1], vec![i(2), SqlValue::Null, SqlValue::Null]);
    }

    #[test]
    fn right_join_pads_left_nulls() {

        let left = Plan::Scan(vec![vec![i(1)], vec![i(2)]]);
        let right = Plan::Scan(vec![
            vec![i(1), SqlValue::Text("a".into())],
            vec![i(3), SqlValue::Text("c".into())],
        ]);
        let plan = Plan::NestedLoopJoin {
            left: Box::new(left),
            right: Box::new(right),
            left_width: 1,
            right_width: 2,
            kind: JoinKind::Right,
            pred: Some(Box::new(|r: &Row| Ok(r[0].cmp(&r[1]) == Ordering::Equal))),
        };
        let out = plan.execute().unwrap();
        assert_eq!(out[0], vec![i(1), i(1), SqlValue::Text("a".into())]);
        assert_eq!(
            out[1],
            vec![SqlValue::Null, i(3), SqlValue::Text("c".into())]
        );
    }

    #[test]
    fn full_join_keeps_both_unmatched() {
        let left = Plan::Scan(vec![vec![i(1)], vec![i(2)]]);
        let right = Plan::Scan(vec![
            vec![i(1), SqlValue::Text("a".into())],
            vec![i(3), SqlValue::Text("c".into())],
        ]);
        let plan = Plan::NestedLoopJoin {
            left: Box::new(left),
            right: Box::new(right),
            left_width: 1,
            right_width: 2,
            kind: JoinKind::Full,
            pred: Some(Box::new(|r: &Row| Ok(r[0].cmp(&r[1]) == Ordering::Equal))),
        };
        let out = plan.execute().unwrap();

        assert_eq!(out[0], vec![i(1), i(1), SqlValue::Text("a".into())]);
        assert_eq!(out[1], vec![i(2), SqlValue::Null, SqlValue::Null]);
        assert_eq!(
            out[2],
            vec![SqlValue::Null, i(3), SqlValue::Text("c".into())]
        );
    }

    #[test]
    fn hash_join_matches_nested_loop_order() {
        fn t(s: &str) -> SqlValue {
            SqlValue::Text(s.into())
        }

        let left_rows: Vec<Row> = vec![vec![i(1)], vec![i(2)], vec![i(1)], vec![SqlValue::Null]];

        let right_rows: Vec<Row> = vec![
            vec![i(1), t("a")],
            vec![i(3), t("c")],
            vec![i(1), t("d")],
            vec![SqlValue::Null, t("e")],
        ];

        for kind in [JoinKind::Inner, JoinKind::Left] {
            let nlj = Plan::NestedLoopJoin {
                left: Box::new(Plan::Scan(left_rows.clone())),
                right: Box::new(Plan::Scan(right_rows.clone())),
                left_width: 1,
                right_width: 2,
                kind,
                pred: Some(Box::new(|r: &Row| {
                    let (a, b) = (&r[0], &r[1]);
                    Ok(!matches!(a, SqlValue::Null)
                        && !matches!(b, SqlValue::Null)
                        && a.cmp(b) == Ordering::Equal)
                })),
            };
            let hj = Plan::HashJoin {
                left: Box::new(Plan::Scan(left_rows.clone())),
                right: Box::new(Plan::Scan(right_rows.clone())),
                left_width: 1,
                right_width: 2,
                kind,
                left_keys: vec![Box::new(|r: &Row| Ok(r[0].clone()))],
                right_keys: vec![Box::new(|r: &Row| Ok(r[0].clone()))],
                extra: None,
            };
            assert_eq!(
                nlj.execute().unwrap(),
                hj.execute().unwrap(),
                "kind {kind:?}"
            );
        }
    }

    #[test]
    fn hash_join_multikey_with_residual() {

        let left_rows: Vec<Row> = vec![vec![i(1), i(9)], vec![i(1), i(8)], vec![i(2), i(9)]];
        let right_rows: Vec<Row> = vec![
            vec![i(1), i(9), i(5)],
            vec![i(1), i(9), i(-1)],
            vec![i(2), i(9), i(3)],
        ];

        let nlj = Plan::NestedLoopJoin {
            left: Box::new(Plan::Scan(left_rows.clone())),
            right: Box::new(Plan::Scan(right_rows.clone())),
            left_width: 2,
            right_width: 3,
            kind: JoinKind::Left,
            pred: Some(Box::new(|r: &Row| {
                Ok(r[0].cmp(&r[2]) == Ordering::Equal
                    && r[1].cmp(&r[3]) == Ordering::Equal
                    && matches!(&r[4], SqlValue::Int(n) if *n > 0))
            })),
        };
        let hj = Plan::HashJoin {
            left: Box::new(Plan::Scan(left_rows)),
            right: Box::new(Plan::Scan(right_rows)),
            left_width: 2,
            right_width: 3,
            kind: JoinKind::Left,
            left_keys: vec![
                Box::new(|r: &Row| Ok(r[0].clone())),
                Box::new(|r: &Row| Ok(r[1].clone())),
            ],
            right_keys: vec![
                Box::new(|r: &Row| Ok(r[0].clone())),
                Box::new(|r: &Row| Ok(r[1].clone())),
            ],
            extra: Some(Box::new(|r: &Row| {
                Ok(matches!(&r[4], SqlValue::Int(n) if *n > 0))
            })),
        };
        assert_eq!(nlj.execute().unwrap(), hj.execute().unwrap());
    }

    #[test]
    fn aggregate_keyless_empty_input_emits_one_group() {
        let plan = Plan::Aggregate {
            input: Box::new(Plan::Scan(Vec::new())),
            key: Vec::new(),
            output: Box::new(|rows: &[Row]| Ok(Some(vec![i(rows.len() as i64)]))),
        };
        assert_eq!(plan.execute().unwrap(), vec![vec![i(0)]]);
    }

    #[test]
    fn aggregate_output_none_drops_group_like_having() {
        let plan = Plan::Aggregate {
            input: Box::new(Plan::Scan(vec![vec![i(1)], vec![i(2)]])),
            key: vec![Box::new(|r: &Row| Ok(r[0].clone()))],
            output: Box::new(|rows: &[Row]| {
                if rows.len() > 1 {
                    Ok(Some(vec![i(rows.len() as i64)]))
                } else {
                    Ok(None)
                }
            }),
        };
        assert!(plan.execute().unwrap().is_empty());
    }

    #[test]
    fn aggregate_groups_nulls_but_not_nan_or_mixed_storage_classes() {
        let nan = f64::NAN;
        let rows = vec![
            vec![SqlValue::Null],
            vec![SqlValue::Null],
            vec![SqlValue::Real(nan)],
            vec![SqlValue::Real(1.0)],
            vec![SqlValue::Int(1)],
        ];
        let plan = Plan::Aggregate {
            input: Box::new(Plan::Scan(rows)),
            key: vec![Box::new(|r: &Row| Ok(r[0].clone()))],
            output: Box::new(|rows: &[Row]| {
                Ok(Some(vec![rows[0][0].clone(), i(rows.len() as i64)]))
            }),
        };
        let out = plan.execute().unwrap();
        assert_eq!(out.len(), 4);
        assert_eq!(out[0], vec![SqlValue::Null, i(2)]);
        assert!(matches!(out[1][0], SqlValue::Real(v) if v.is_nan()));
        assert_eq!(out[1][1], i(1));
        assert_eq!(out[2], vec![SqlValue::Real(1.0), i(1)]);
        assert_eq!(out[3], vec![SqlValue::Int(1), i(1)]);
    }

    #[test]
    fn sort_options_encode_dialect_defaults_and_explicit_settings() {
        let rows = vec![vec![i(2)], vec![SqlValue::Null], vec![i(1)]];
        let run = |options| {
            Plan::Sort {
                input: Box::new(Plan::Scan(rows.clone())),
                keys: vec![(Box::new(|r: &Row| Ok(r[0].clone())), options)],
            }
            .execute()
            .unwrap()
            .into_iter()
            .map(|mut row| row.remove(0))
            .collect::<Vec<_>>()
        };
        assert_eq!(
            run(SortOptions::with_default(
                false,
                None,
                NullsDefault::Sqlite,
                TextCollation::Binary
            )),
            vec![SqlValue::Null, i(1), i(2)]
        );
        assert_eq!(
            run(SortOptions::with_default(
                true,
                None,
                NullsDefault::Sqlite,
                TextCollation::Binary
            )),
            vec![i(2), i(1), SqlValue::Null]
        );
        assert_eq!(
            run(SortOptions::with_default(
                false,
                None,
                NullsDefault::Postgres,
                TextCollation::Binary
            )),
            vec![i(1), i(2), SqlValue::Null]
        );
        assert_eq!(
            run(SortOptions::with_default(
                true,
                None,
                NullsDefault::Postgres,
                TextCollation::Binary
            )),
            vec![SqlValue::Null, i(2), i(1)]
        );
        assert_eq!(
            run(SortOptions::with_default(
                false,
                Some(false),
                NullsDefault::Sqlite,
                TextCollation::Binary
            )),
            vec![i(1), i(2), SqlValue::Null]
        );
    }

    #[test]
    fn sort_text_collation_hook_is_part_of_core_sort_options() {
        let rows = vec![
            vec![SqlValue::Text("b".into())],
            vec![SqlValue::Text("A ".into())],
            vec![SqlValue::Text("a".into())],
        ];
        let run = |collation| {
            Plan::Sort {
                input: Box::new(Plan::Scan(rows.clone())),
                keys: vec![(
                    Box::new(|r: &Row| Ok(r[0].clone())),
                    SortOptions::new(false, NullsPlacement::Last, collation),
                )],
            }
            .execute()
            .unwrap()
            .into_iter()
            .map(|mut row| row.remove(0))
            .collect::<Vec<_>>()
        };
        assert_eq!(
            run(TextCollation::Binary),
            vec![
                SqlValue::Text("A ".into()),
                SqlValue::Text("a".into()),
                SqlValue::Text("b".into())
            ]
        );
        assert_eq!(
            run(TextCollation::RTrim),
            vec![
                SqlValue::Text("A ".into()),
                SqlValue::Text("a".into()),
                SqlValue::Text("b".into())
            ]
        );
        assert_eq!(
            run(TextCollation::AsciiNoCase),
            vec![
                SqlValue::Text("a".into()),
                SqlValue::Text("A ".into()),
                SqlValue::Text("b".into())
            ]
        );
    }

    #[test]
    fn filter_false_predicate_drops_unknown_rows_at_core_boundary() {
        let plan = Plan::Filter {
            input: Box::new(Plan::Scan(vec![
                vec![i(1)],
                vec![SqlValue::Null],
                vec![i(2)],
            ])),
            pred: Box::new(|r: &Row| Ok(matches!(&r[0], SqlValue::Int(n) if *n > 1))),
        };
        assert_eq!(plan.execute().unwrap(), vec![vec![i(2)]]);
    }
}
