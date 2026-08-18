
use sql_core::SqlValue;

pub mod ident {

    pub fn is_safe_identifier(s: &str) -> bool {
        let mut chars = s.chars();
        match chars.next() {
            Some(c) if c == '_' || c.is_ascii_alphabetic() => {}
            _ => return false,
        }
        chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
    }

    #[cfg(test)]
    mod tests {
        use super::is_safe_identifier;

        #[test]
        fn identifier_predicate_rejects_sql_structure() {
            assert!(is_safe_identifier("users"));
            assert!(is_safe_identifier("_id2"));
            assert!(!is_safe_identifier(""));
            assert!(!is_safe_identifier("2bad"));
            assert!(!is_safe_identifier("users; DROP TABLE users; --"));
            assert!(!is_safe_identifier("user\"name"));
            assert!(!is_safe_identifier("public.users"));
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CsType {

    Number,

    BigInt,

    Boolean,

    Str,

    Date,

    Bytes,

    Enum(Vec<String>),

    Record(Vec<(String, CsType)>),

    Array(Box<CsType>),

    Unknown(&'static str),

    Nullable(Box<CsType>),
}

impl CsType {

    pub fn render(&self) -> String {
        match self {
            CsType::Number => "number".to_string(),
            CsType::BigInt => "bigint".to_string(),
            CsType::Boolean => "boolean".to_string(),
            CsType::Str => "string".to_string(),
            CsType::Date => "Date".to_string(),
            CsType::Bytes => "Uint8Array".to_string(),
            CsType::Enum(labels) => {
                if labels.is_empty() {
                    "never".to_string()
                } else {
                    labels
                        .iter()
                        .map(|l| format!("'{l}'"))
                        .collect::<Vec<_>>()
                        .join(" | ")
                }
            }
            CsType::Record(fields) => {
                let body = fields
                    .iter()
                    .map(|(n, t)| format!("{n}: {}", t.render()))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{ {body} }}")
            }
            CsType::Array(elem) => {

                match elem.as_ref() {
                    CsType::Enum(_) | CsType::Record(_) => format!("({})[]", elem.render()),
                    _ => format!("{}[]", elem.render()),
                }
            }
            CsType::Unknown(_) => "unknown".to_string(),

            CsType::Nullable(inner) => format!("{} | null", inner.render()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Divergence {

    MissingInDb { column: String },

    MissingInDeclaration { column: String },

    TypeMismatch {
        column: String,
        declared: String,
        actual: String,
    },

    NullabilityMismatch {
        column: String,
        declared_nullable: bool,
        actual_nullable: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolationKind {

    NullInNonNull,

    NotInUnion,

    TypeMismatch,

    MissingColumn,

    UnexpectedColumn,

    NoSanitizerDefault,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CruftBoundaryError {
    pub column: String,
    pub row_index: usize,
    pub expected: CsType,
    pub received: String,
    pub kind: ViolationKind,
    pub message: String,
}

impl CruftBoundaryError {
    pub fn new(
        column: &str,
        row_index: usize,
        expected: CsType,
        received: String,
        kind: ViolationKind,
    ) -> Self {
        let message = format!(
            "CruftBoundaryError: soundness violation ({}) — column \"{}\" expected {}, received {} (row {})",
            kind.label(),
            column,
            expected.render(),
            received,
            row_index
        );
        CruftBoundaryError {
            column: column.to_string(),
            row_index,
            expected,
            received,
            kind,
            message,
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

fn value_text(value: &SqlValue) -> Option<String> {
    match value {
        SqlValue::Text(s) => Some(s.clone()),
        SqlValue::Int(i) => Some(i.to_string()),
        SqlValue::Real(r) => Some(r.to_string()),
        _ => None,
    }
}

pub fn value_satisfies(value: &SqlValue, is_bool: bool, ty: &CsType) -> Option<ViolationKind> {
    match ty {

        CsType::Unknown(_) => None,

        CsType::Nullable(inner) => {
            if matches!(value, SqlValue::Null) {
                None
            } else {
                value_satisfies(value, is_bool, inner)
            }
        }

        _ if matches!(value, SqlValue::Null) => Some(ViolationKind::NullInNonNull),
        CsType::Number => match value {

            SqlValue::Int(_) if is_bool => Some(ViolationKind::TypeMismatch),
            SqlValue::Int(_) | SqlValue::Real(_) => None,
            SqlValue::Text(s) if s.trim().parse::<f64>().is_ok() => None,
            _ => Some(ViolationKind::TypeMismatch),
        },
        CsType::BigInt => match value {
            SqlValue::Int(_) if is_bool => Some(ViolationKind::TypeMismatch),
            SqlValue::Int(_) => None,
            SqlValue::Text(s) if s.trim().parse::<i64>().is_ok() => None,
            _ => Some(ViolationKind::TypeMismatch),
        },
        CsType::Boolean => match value {
            SqlValue::Int(0) | SqlValue::Int(1) => None,
            SqlValue::Text(s) => {
                let t = s.trim().to_ascii_lowercase();
                if matches!(t.as_str(), "t" | "f" | "true" | "false") {
                    None
                } else {
                    Some(ViolationKind::TypeMismatch)
                }
            }
            _ => Some(ViolationKind::TypeMismatch),
        },
        CsType::Str => match value {
            SqlValue::Text(_) => None,
            _ => Some(ViolationKind::TypeMismatch),
        },
        CsType::Date => match value {
            SqlValue::Text(_) | SqlValue::Int(_) | SqlValue::Real(_) => None,
            _ => Some(ViolationKind::TypeMismatch),
        },
        CsType::Bytes => match value {
            SqlValue::Blob(_) | SqlValue::Text(_) => None,
            _ => Some(ViolationKind::TypeMismatch),
        },
        CsType::Enum(labels) => match value_text(value) {
            Some(s) if labels.iter().any(|l| *l == s) => None,
            Some(_) => Some(ViolationKind::NotInUnion),
            None => Some(ViolationKind::TypeMismatch),
        },
        CsType::Array(elem) => {
            let s = match value {
                SqlValue::Text(s) => s.trim(),
                _ => return Some(ViolationKind::TypeMismatch),
            };
            if !(s.starts_with('{') && s.ends_with('}')) {
                return Some(ViolationKind::TypeMismatch);
            }
            let inner = &s[1..s.len() - 1];
            if inner.is_empty() {
                return None;
            }

            for part in inner.split(',') {
                let cell = part.trim().trim_matches('"');

                if cell.eq_ignore_ascii_case("null") {
                    continue;
                }
                let ev = SqlValue::Text(cell.to_string());
                if let Some(k) = value_satisfies(&ev, false, elem) {
                    return Some(k);
                }
            }
            None
        }
        CsType::Record(fields) => {
            let s = match value {
                SqlValue::Text(s) => s.trim(),
                _ => return Some(ViolationKind::TypeMismatch),
            };
            if !(s.starts_with('(') && s.ends_with(')')) {
                return Some(ViolationKind::TypeMismatch);
            }
            let inner = &s[1..s.len() - 1];
            let parts: Vec<&str> = if inner.is_empty() {
                Vec::new()
            } else {
                inner.split(',').collect()
            };

            if parts.len() != fields.len() {
                return Some(ViolationKind::TypeMismatch);
            }
            for ((_, fty), part) in fields.iter().zip(parts) {
                let cell = part.trim().trim_matches('"');

                if cell.is_empty() {
                    continue;
                }
                let fv = SqlValue::Text(cell.to_string());
                if let Some(k) = value_satisfies(&fv, false, fty) {
                    return Some(k);
                }
            }
            None
        }
    }
}

impl ViolationKind {
    fn label(self) -> &'static str {
        match self {
            ViolationKind::NullInNonNull => "null in non-null type",
            ViolationKind::NotInUnion => "value outside literal union",
            ViolationKind::TypeMismatch => "type-shape mismatch",
            ViolationKind::MissingColumn => "column missing from result",
            ViolationKind::UnexpectedColumn => "unexpected column in result",
            ViolationKind::NoSanitizerDefault => "no sanitizer default for expected type",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelKind {

    BelongsTo,

    HasMany,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relation {
    pub name: String,
    pub kind: RelKind,
    pub local_cols: Vec<String>,
    pub target: String,
    pub target_cols: Vec<String>,
}

pub mod query {
    use sql_core::SqlValue;

    #[derive(Debug, Clone, PartialEq)]
    pub enum FilterOp {
        Eq,
        Ne,
        Lt,
        Le,
        Gt,
        Ge,
        Like,
        In,
        IsNull,
        IsNotNull,
    }

    impl FilterOp {

        pub fn sql(&self) -> &'static str {
            match self {
                FilterOp::Eq => "=",
                FilterOp::Ne => "<>",
                FilterOp::Lt => "<",
                FilterOp::Le => "<=",
                FilterOp::Gt => ">",
                FilterOp::Ge => ">=",
                FilterOp::Like => "LIKE",
                FilterOp::In | FilterOp::IsNull | FilterOp::IsNotNull => "",
            }
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum FilterVal {
        One(SqlValue),
        Many(Vec<SqlValue>),
        None,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Filter {
        pub col: String,
        pub op: FilterOp,
        pub value: FilterVal,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum Select {
        All,
        Cols(Vec<String>),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AggFunc {
        Count,
        Sum,
        Avg,
        Min,
        Max,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Agg {
        pub func: AggFunc,
        pub col: Option<String>,
        pub alias: String,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum JoinKind {
        Inner,
        Left,
        Right,
        Full,
    }

    impl JoinKind {

        pub fn sql(self) -> &'static str {
            match self {
                JoinKind::Inner => "INNER JOIN",
                JoinKind::Left => "LEFT JOIN",
                JoinKind::Right => "RIGHT JOIN",
                JoinKind::Full => "FULL JOIN",
            }
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Join {
        pub kind: JoinKind,
        pub table: String,
        pub on: Vec<(String, String)>,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Query {
        pub from: String,
        pub joins: Vec<Join>,
        pub filters: Vec<Filter>,
        pub select: Select,

        pub order: Vec<(String, bool)>,
        pub limit: Option<u64>,
        pub offset: Option<u64>,

        pub with: Vec<String>,

        pub group_by: Vec<String>,

        pub aggregates: Vec<Agg>,
    }

    impl Query {

        pub fn from(table: &str) -> Query {
            Query {
                from: table.to_string(),
                joins: Vec::new(),
                filters: Vec::new(),
                select: Select::All,
                order: Vec::new(),
                limit: None,
                offset: None,
                with: Vec::new(),
                group_by: Vec::new(),
                aggregates: Vec::new(),
            }
        }

        pub fn group_by(mut self, cols: &[&str]) -> Query {
            self.group_by = cols.iter().map(|c| c.to_string()).collect();
            self
        }
        fn agg(mut self, func: AggFunc, col: Option<&str>, alias: &str) -> Query {
            self.aggregates.push(Agg {
                func,
                col: col.map(|c| c.to_string()),
                alias: alias.to_string(),
            });
            self
        }

        pub fn count(self, alias: &str) -> Query {
            self.agg(AggFunc::Count, None, alias)
        }

        pub fn count_col(self, col: &str, alias: &str) -> Query {
            self.agg(AggFunc::Count, Some(col), alias)
        }
        pub fn sum(self, col: &str, alias: &str) -> Query {
            self.agg(AggFunc::Sum, Some(col), alias)
        }
        pub fn avg(self, col: &str, alias: &str) -> Query {
            self.agg(AggFunc::Avg, Some(col), alias)
        }
        pub fn min(self, col: &str, alias: &str) -> Query {
            self.agg(AggFunc::Min, Some(col), alias)
        }
        pub fn max(self, col: &str, alias: &str) -> Query {
            self.agg(AggFunc::Max, Some(col), alias)
        }

        pub fn with(mut self, relation: &str) -> Query {
            self.with.push(relation.to_string());
            self
        }

        pub fn join(mut self, kind: JoinKind, table: &str, on: &[(&str, &str)]) -> Query {
            self.joins.push(Join {
                kind,
                table: table.to_string(),
                on: on
                    .iter()
                    .map(|(l, r)| (l.to_string(), r.to_string()))
                    .collect(),
            });
            self
        }

        pub fn join_inner(self, table: &str, on: &[(&str, &str)]) -> Query {
            self.join(JoinKind::Inner, table, on)
        }

        pub fn join_left(self, table: &str, on: &[(&str, &str)]) -> Query {
            self.join(JoinKind::Left, table, on)
        }

        pub fn join_right(self, table: &str, on: &[(&str, &str)]) -> Query {
            self.join(JoinKind::Right, table, on)
        }

        pub fn join_full(self, table: &str, on: &[(&str, &str)]) -> Query {
            self.join(JoinKind::Full, table, on)
        }

        pub fn filter(mut self, f: Filter) -> Query {
            self.filters.push(f);
            self
        }

        fn binary(self, col: &str, op: FilterOp, value: SqlValue) -> Query {
            self.filter(Filter {
                col: col.to_string(),
                op,
                value: FilterVal::One(value),
            })
        }

        pub fn filter_eq(self, col: &str, value: SqlValue) -> Query {
            self.binary(col, FilterOp::Eq, value)
        }

        pub fn filter_ne(self, col: &str, value: SqlValue) -> Query {
            self.binary(col, FilterOp::Ne, value)
        }

        pub fn filter_lt(self, col: &str, value: SqlValue) -> Query {
            self.binary(col, FilterOp::Lt, value)
        }

        pub fn filter_le(self, col: &str, value: SqlValue) -> Query {
            self.binary(col, FilterOp::Le, value)
        }

        pub fn filter_gt(self, col: &str, value: SqlValue) -> Query {
            self.binary(col, FilterOp::Gt, value)
        }

        pub fn filter_ge(self, col: &str, value: SqlValue) -> Query {
            self.binary(col, FilterOp::Ge, value)
        }

        pub fn filter_like(self, col: &str, value: SqlValue) -> Query {
            self.binary(col, FilterOp::Like, value)
        }

        pub fn filter_in(self, col: &str, values: Vec<SqlValue>) -> Query {
            self.filter(Filter {
                col: col.to_string(),
                op: FilterOp::In,
                value: FilterVal::Many(values),
            })
        }

        pub fn filter_is_null(self, col: &str) -> Query {
            self.filter(Filter {
                col: col.to_string(),
                op: FilterOp::IsNull,
                value: FilterVal::None,
            })
        }

        pub fn filter_is_not_null(self, col: &str) -> Query {
            self.filter(Filter {
                col: col.to_string(),
                op: FilterOp::IsNotNull,
                value: FilterVal::None,
            })
        }

        pub fn select(mut self, cols: &[&str]) -> Query {
            self.select = Select::Cols(cols.iter().map(|c| c.to_string()).collect());
            self
        }

        pub fn order_by(mut self, col: &str, desc: bool) -> Query {
            self.order.push((col.to_string(), desc));
            self
        }

        pub fn limit(mut self, n: u64) -> Query {
            self.limit = Some(n);
            self
        }

        pub fn offset(mut self, n: u64) -> Query {
            self.offset = Some(n);
            self
        }
    }
}

pub mod dml {
    use super::query::{Filter, FilterOp, FilterVal, Select};
    use sql_core::SqlValue;

    #[derive(Debug, Clone, PartialEq)]
    pub struct OnConflict {
        pub target: Vec<String>,
        pub update: Option<Vec<String>>,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Insert {
        pub into: String,
        pub columns: Vec<String>,
        pub rows: Vec<Vec<SqlValue>>,
        pub returning: Option<Select>,
        pub on_conflict: Option<OnConflict>,
    }

    impl Insert {

        pub fn into(table: &str) -> Insert {
            Insert {
                into: table.to_string(),
                columns: Vec::new(),
                rows: Vec::new(),
                returning: None,
                on_conflict: None,
            }
        }

        pub fn on_conflict_do_nothing(mut self, target: &[&str]) -> Insert {
            self.on_conflict = Some(OnConflict {
                target: target.iter().map(|c| c.to_string()).collect(),
                update: None,
            });
            self
        }

        pub fn on_conflict_do_update(mut self, target: &[&str], set: &[&str]) -> Insert {
            self.on_conflict = Some(OnConflict {
                target: target.iter().map(|c| c.to_string()).collect(),
                update: Some(set.iter().map(|c| c.to_string()).collect()),
            });
            self
        }

        pub fn columns(mut self, cols: &[&str]) -> Insert {
            self.columns = cols.iter().map(|c| c.to_string()).collect();
            self
        }

        pub fn row(mut self, values: &[SqlValue]) -> Insert {
            self.rows.push(values.to_vec());
            self
        }

        pub fn returning_all(mut self) -> Insert {
            self.returning = Some(Select::All);
            self
        }

        pub fn returning(mut self, cols: &[&str]) -> Insert {
            self.returning = Some(Select::Cols(cols.iter().map(|c| c.to_string()).collect()));
            self
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Update {
        pub table: String,
        pub set: Vec<(String, SqlValue)>,
        pub filters: Vec<Filter>,
        pub returning: Option<Select>,
    }

    impl Update {

        pub fn table(table: &str) -> Update {
            Update {
                table: table.to_string(),
                set: Vec::new(),
                filters: Vec::new(),
                returning: None,
            }
        }

        pub fn set(mut self, col: &str, value: SqlValue) -> Update {
            self.set.push((col.to_string(), value));
            self
        }

        pub fn filter(mut self, f: Filter) -> Update {
            self.filters.push(f);
            self
        }

        fn binary(self, col: &str, op: FilterOp, value: SqlValue) -> Update {
            self.filter(Filter {
                col: col.to_string(),
                op,
                value: FilterVal::One(value),
            })
        }

        pub fn filter_eq(self, col: &str, value: SqlValue) -> Update {
            self.binary(col, FilterOp::Eq, value)
        }

        pub fn filter_ne(self, col: &str, value: SqlValue) -> Update {
            self.binary(col, FilterOp::Ne, value)
        }

        pub fn filter_lt(self, col: &str, value: SqlValue) -> Update {
            self.binary(col, FilterOp::Lt, value)
        }

        pub fn filter_le(self, col: &str, value: SqlValue) -> Update {
            self.binary(col, FilterOp::Le, value)
        }

        pub fn filter_gt(self, col: &str, value: SqlValue) -> Update {
            self.binary(col, FilterOp::Gt, value)
        }

        pub fn filter_ge(self, col: &str, value: SqlValue) -> Update {
            self.binary(col, FilterOp::Ge, value)
        }

        pub fn filter_like(self, col: &str, value: SqlValue) -> Update {
            self.binary(col, FilterOp::Like, value)
        }

        pub fn filter_in(self, col: &str, values: Vec<SqlValue>) -> Update {
            self.filter(Filter {
                col: col.to_string(),
                op: FilterOp::In,
                value: FilterVal::Many(values),
            })
        }

        pub fn filter_is_null(self, col: &str) -> Update {
            self.filter(Filter {
                col: col.to_string(),
                op: FilterOp::IsNull,
                value: FilterVal::None,
            })
        }

        pub fn filter_is_not_null(self, col: &str) -> Update {
            self.filter(Filter {
                col: col.to_string(),
                op: FilterOp::IsNotNull,
                value: FilterVal::None,
            })
        }

        pub fn returning_all(mut self) -> Update {
            self.returning = Some(Select::All);
            self
        }

        pub fn returning(mut self, cols: &[&str]) -> Update {
            self.returning = Some(Select::Cols(cols.iter().map(|c| c.to_string()).collect()));
            self
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Delete {
        pub table: String,
        pub filters: Vec<Filter>,
        pub returning: Option<Select>,
    }

    impl Delete {

        pub fn table(table: &str) -> Delete {
            Delete {
                table: table.to_string(),
                filters: Vec::new(),
                returning: None,
            }
        }

        pub fn filter(mut self, f: Filter) -> Delete {
            self.filters.push(f);
            self
        }

        fn binary(self, col: &str, op: FilterOp, value: SqlValue) -> Delete {
            self.filter(Filter {
                col: col.to_string(),
                op,
                value: FilterVal::One(value),
            })
        }

        pub fn filter_eq(self, col: &str, value: SqlValue) -> Delete {
            self.binary(col, FilterOp::Eq, value)
        }

        pub fn filter_ne(self, col: &str, value: SqlValue) -> Delete {
            self.binary(col, FilterOp::Ne, value)
        }

        pub fn filter_lt(self, col: &str, value: SqlValue) -> Delete {
            self.binary(col, FilterOp::Lt, value)
        }

        pub fn filter_le(self, col: &str, value: SqlValue) -> Delete {
            self.binary(col, FilterOp::Le, value)
        }

        pub fn filter_gt(self, col: &str, value: SqlValue) -> Delete {
            self.binary(col, FilterOp::Gt, value)
        }

        pub fn filter_ge(self, col: &str, value: SqlValue) -> Delete {
            self.binary(col, FilterOp::Ge, value)
        }

        pub fn filter_like(self, col: &str, value: SqlValue) -> Delete {
            self.binary(col, FilterOp::Like, value)
        }

        pub fn filter_in(self, col: &str, values: Vec<SqlValue>) -> Delete {
            self.filter(Filter {
                col: col.to_string(),
                op: FilterOp::In,
                value: FilterVal::Many(values),
            })
        }

        pub fn filter_is_null(self, col: &str) -> Delete {
            self.filter(Filter {
                col: col.to_string(),
                op: FilterOp::IsNull,
                value: FilterVal::None,
            })
        }

        pub fn filter_is_not_null(self, col: &str) -> Delete {
            self.filter(Filter {
                col: col.to_string(),
                op: FilterOp::IsNotNull,
                value: FilterVal::None,
            })
        }

        pub fn returning_all(mut self) -> Delete {
            self.returning = Some(Select::All);
            self
        }

        pub fn returning(mut self, cols: &[&str]) -> Delete {
            self.returning = Some(Select::Cols(cols.iter().map(|c| c.to_string()).collect()));
            self
        }
    }
}

#[derive(Debug, Clone)]
pub struct RowSet {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<SqlValue>>,

    pub col_bool: Vec<bool>,

    pub col_type_name: Vec<String>,
}

fn render_received(value: &SqlValue, type_name: &str) -> String {
    match value {
        SqlValue::Null => "NULL".to_string(),
        SqlValue::Int(i) => format!("{i} ({type_name})"),
        SqlValue::Real(r) => format!("{r} ({type_name})"),
        SqlValue::Text(s) => format!("\"{s}\" ({type_name})"),
        SqlValue::Blob(b) => format!("<{} bytes> ({type_name})", b.len()),
    }
}

pub fn validate_result(
    result: &RowSet,
    expected: &[(String, CsType)],
) -> Result<(), CruftBoundaryError> {

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

    for (row_index, row) in result.rows.iter().enumerate() {
        for (name, ty) in expected {
            let idx = result
                .columns
                .iter()
                .position(|c| c == name)
                .expect("presence checked above");
            let value = &row[idx];
            let is_bool = result.col_bool.get(idx).copied().unwrap_or(false);
            if let Some(kind) = value_satisfies(value, is_bool, ty) {
                let tn = result
                    .col_type_name
                    .get(idx)
                    .map(|s| s.as_str())
                    .unwrap_or("");
                return Err(CruftBoundaryError::new(
                    name,
                    row_index,
                    ty.clone(),
                    render_received(value, tn),
                    kind,
                ));
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Default)]
pub struct SanitizeDefaults {
    number: Option<SqlValue>,
    bigint: Option<SqlValue>,
    boolean: Option<SqlValue>,
    string: Option<SqlValue>,
    date: Option<SqlValue>,
    bytes: Option<SqlValue>,
    named: Vec<(String, SqlValue)>,
}

impl SanitizeDefaults {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn number(mut self, v: SqlValue) -> Self {
        self.number = Some(v);
        self
    }
    pub fn bigint(mut self, v: SqlValue) -> Self {
        self.bigint = Some(v);
        self
    }
    pub fn boolean(mut self, v: SqlValue) -> Self {
        self.boolean = Some(v);
        self
    }
    pub fn string(mut self, v: SqlValue) -> Self {
        self.string = Some(v);
        self
    }
    pub fn date(mut self, v: SqlValue) -> Self {
        self.date = Some(v);
        self
    }
    pub fn bytes(mut self, v: SqlValue) -> Self {
        self.bytes = Some(v);
        self
    }

    pub fn for_type(mut self, ty: &CsType, v: SqlValue) -> Self {
        self.named.push((ty.render(), v));
        self
    }

    pub fn default_for(&self, ty: &CsType) -> Option<SqlValue> {
        match ty {
            CsType::Number => self.number.clone(),
            CsType::BigInt => self.bigint.clone(),
            CsType::Boolean => self.boolean.clone(),
            CsType::Str => self.string.clone(),
            CsType::Date => self.date.clone(),
            CsType::Bytes => self.bytes.clone(),
            CsType::Nullable(inner) => self.default_for(inner),
            CsType::Unknown(_) => None,
            CsType::Enum(_) | CsType::Array(_) | CsType::Record(_) => self
                .named
                .iter()
                .find(|(k, _)| *k == ty.render())
                .map(|(_, v)| v.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SanitizationRecord {
    pub column: String,
    pub row_index: usize,
    pub expected: CsType,
    pub received: String,
    pub kind: ViolationKind,
    pub replaced_with: SqlValue,
}

#[derive(Debug, Clone)]
pub struct SanitizedRowSet {
    pub result: RowSet,
    pub records: Vec<SanitizationRecord>,
}

pub fn sanitize_result(
    result: &RowSet,
    expected: &[(String, CsType)],
    defaults: &SanitizeDefaults,
) -> Result<SanitizedRowSet, CruftBoundaryError> {
    shape_check(result, expected)?;
    let mut out = result.clone();
    let mut records = Vec::new();
    for (row_index, row) in out.rows.iter_mut().enumerate() {
        for (name, ty) in expected {
            let idx = result
                .columns
                .iter()
                .position(|c| c == name)
                .expect("presence checked");
            let is_bool = result.col_bool.get(idx).copied().unwrap_or(false);
            if let Some(kind) = value_satisfies(&row[idx], is_bool, ty) {
                let tn = result
                    .col_type_name
                    .get(idx)
                    .map(|s| s.as_str())
                    .unwrap_or("");
                let received = render_received(&row[idx], tn);
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
    Ok(SanitizedRowSet {
        result: out,
        records,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct PropagationRecord {
    pub column: String,
    pub row_index: usize,
    pub expected: CsType,
    pub received: String,
    pub kind: ViolationKind,
}

#[derive(Debug, Clone)]
pub struct PropagatedRowSet {
    pub result: RowSet,
    pub result_type: Vec<(String, CsType)>,
    pub records: Vec<PropagationRecord>,
}

pub fn propagate_result(
    result: &RowSet,
    expected: &[(String, CsType)],
) -> Result<PropagatedRowSet, CruftBoundaryError> {
    shape_check(result, expected)?;
    let mut records = Vec::new();
    let mut propagated: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (row_index, row) in result.rows.iter().enumerate() {
        for (name, ty) in expected {
            let idx = result
                .columns
                .iter()
                .position(|c| c == name)
                .expect("presence checked");
            let is_bool = result.col_bool.get(idx).copied().unwrap_or(false);
            if let Some(kind) = value_satisfies(&row[idx], is_bool, ty) {
                let tn = result
                    .col_type_name
                    .get(idx)
                    .map(|s| s.as_str())
                    .unwrap_or("");
                records.push(PropagationRecord {
                    column: name.clone(),
                    row_index,
                    expected: ty.clone(),
                    received: render_received(&row[idx], tn),
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
    Ok(PropagatedRowSet {
        result: result.clone(),
        result_type,
        records,
    })
}

fn shape_check(result: &RowSet, expected: &[(String, CsType)]) -> Result<(), CruftBoundaryError> {
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
    Ok(())
}
