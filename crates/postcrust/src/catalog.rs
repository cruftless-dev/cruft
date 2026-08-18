
use crate::expr::ast::{BinOp, Expr, UnOp};
use crate::expr::bind::{resolve, Schema};
use crate::stmt::ast::SelectStmt;
use crate::types::PgError;
use sql_core::Row;
use sql_core::SqlValue;
use std::collections::{HashMap, HashSet};

pub const INVALID_XID: u64 = 0;

pub const FROZEN_XID: u64 = 1;

const FIRST_REAL_XID: u64 = 2;

#[derive(Debug, Clone, Copy)]
pub struct TupleHeader {
    pub xmin: u64,
    pub xmax: u64,
}

impl TupleHeader {

    fn live(xid: u64) -> TupleHeader {
        TupleHeader { xmin: xid, xmax: 0 }
    }

    fn frozen() -> TupleHeader {
        TupleHeader {
            xmin: FROZEN_XID,
            xmax: 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct LockInfo {
    exclusive: Option<u64>,
    shared: HashSet<u64>,
}

impl LockInfo {

    fn conflicts_exclusive(&self, xid: u64) -> bool {
        self.exclusive.is_some_and(|x| x != xid) || self.shared.iter().any(|&x| x != xid)
    }

    fn conflicts_shared(&self, xid: u64) -> bool {
        self.exclusive.is_some_and(|x| x != xid)
    }

    fn is_empty(&self) -> bool {
        self.exclusive.is_none() && self.shared.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockMode {
    Exclusive,
    Shared,
}

#[derive(Debug, Clone)]
pub struct UniqueKey {
    pub cols: Vec<usize>,
    pub is_primary: bool,

    pub name: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PartitionInfo {
    pub key_col: usize,

    pub children: Vec<String>,

    pub default_child: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AggregateDef {
    pub name: String,
    pub sfunc: String,
    pub stype_oid: u32,
    pub initcond: String,
    pub finalfunc: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastDef {
    pub source_oid: u32,
    pub target_oid: u32,
    pub func: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RefAction {
    #[default]
    NoAction,
    Restrict,
    Cascade,
    SetNull,
}

#[derive(Debug, Clone)]
pub struct ForeignKey {
    pub cols: Vec<usize>,
    pub parent: String,
    pub parent_cols: Vec<usize>,
    pub on_delete: RefAction,
    pub on_update: RefAction,

    pub name: Option<String>,

    pub deferrable: bool,

    pub initially_deferred: bool,
}

#[derive(Debug, Clone)]
pub struct CheckConstraint {
    pub name: Option<String>,
    pub expr: Expr,
}

#[derive(Debug, Clone)]
pub struct DomainDef {
    pub name: String,

    pub base_oid: u32,

    pub base_typmod: i32,

    pub not_null: bool,

    pub default: Option<Expr>,

    pub checks: Vec<CheckConstraint>,
}

#[derive(Debug, Clone, Default)]
pub struct TableConstraints {

    pub not_null: Vec<bool>,

    pub uniques: Vec<UniqueKey>,

    pub checks: Vec<CheckConstraint>,

    pub foreign_keys: Vec<ForeignKey>,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    tables: HashMap<String, Table>,
    views: HashMap<String, View>,
}

#[derive(Debug, Clone)]
struct TxnFrame {
    name: Option<String>,
    snapshot: Snapshot,
}

#[derive(Debug, Clone)]
pub struct ColumnStats {

    pub null_frac: f64,

    pub avg_width: i32,

    pub n_distinct: f64,

    pub most_common_vals: Vec<SqlValue>,

    pub most_common_freqs: Vec<f64>,

    pub histogram_bounds: Vec<SqlValue>,

    pub correlation: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct Table {
    pub schema: Schema,
    pub col_types: Vec<u32>,

    pub col_typmods: Vec<i32>,
    pub rows: Vec<Row>,

    pub versions: Vec<TupleHeader>,

    pub rids: Vec<u64>,

    pub next_rid: u64,

    pub constraints: TableConstraints,

    pub defaults: Vec<Option<Expr>>,

    pub generated: Vec<Option<Expr>>,

    pub eq_indexes: std::cell::RefCell<std::collections::HashMap<usize, sql_core::EqIndex>>,

    pub eq_indexes_multi:
        std::cell::RefCell<std::collections::HashMap<Vec<usize>, sql_core::EqIndexN>>,

    pub col_domains: Vec<Option<String>>,

    pub identity: Vec<IdentityKind>,

    pub stats: std::collections::BTreeMap<usize, ColumnStats>,

    pub reltuples: f64,
}

#[derive(Debug, Clone)]
pub struct View {
    pub query: SelectStmt,

    pub query_sql: String,
    pub columns: Option<Vec<String>>,

    pub materialized: bool,

    pub mat_columns: Vec<String>,

    pub mat_col_types: Vec<u32>,

    pub mat_rows: Vec<Row>,

    pub check_option: bool,
}

#[derive(Debug, Clone)]
pub struct IndexDef {
    pub name: String,
    pub table: String,
    pub cols: Vec<usize>,
    pub unique: bool,
}

#[derive(Debug, Clone)]
pub struct SequenceDef {
    pub name: String,
    pub increment: i64,
    pub min: i64,
    pub max: i64,
    pub start: i64,
    pub cache: i64,
    pub cycle: bool,

    pub current: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Volatility {
    Immutable,
    Stable,
    #[default]
    Volatile,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FuncBody {
    Expr(crate::expr::ast::Expr),
    Query(Box<crate::stmt::ast::SelectStmt>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RetShape {
    Scalar,
    SetofScalar { oid: u32 },
    SetofTable(Vec<(String, u32, i32)>),
    SetofRel,
}

impl RetShape {

    pub fn is_setof(&self) -> bool {
        !matches!(self, RetShape::Scalar)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDef {
    pub name: String,

    pub args: Vec<(Option<String>, u32, i32)>,
    pub ret_oid: u32,
    pub ret_typmod: i32,

    pub returns: RetShape,
    pub body: FuncBody,

    pub strict: bool,
    pub volatility: Volatility,

    pub language: Lang,

    pub pl_body: Option<crate::stmt::plpgsql::PlBlock>,

    pub source_body: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Sql,
    PlPgSql,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrigTiming {
    Before,
    After,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrigEvent {
    Insert,
    Update,
    Delete,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TriggerDef {
    pub name: String,
    pub timing: TrigTiming,
    pub events: Vec<TrigEvent>,
    pub table: String,

    pub when: Option<crate::expr::ast::Expr>,

    pub func: String,
}

#[derive(Debug, Clone)]
pub struct PreparedStmt {

    pub param_types: Vec<String>,

    pub param_count: usize,

    pub inner_sql: String,
}

#[derive(Debug, Clone)]
pub struct Cursor {

    pub cols: Vec<String>,

    pub oids: Vec<u32>,

    pub rows: Vec<Row>,

    pub pos: usize,

    pub scroll: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IdentityKind {
    #[default]
    None,
    Always,
    ByDefault,
}

#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name: String,
    pub oid: u32,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CompositeDef {
    pub name: String,
    pub oid: u32,
    pub fields: Vec<(String, u32, i32)>,
}

const FIRST_ENUM_OID: u32 = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IsolationLevel {
    #[default]
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

#[derive(Debug, Clone)]
pub struct TxnSnapshot {
    ceiling: u64,
    in_progress: HashSet<u64>,
}

#[derive(Debug, Clone)]
pub struct Session {
    txn_stack: Vec<TxnFrame>,
    aborted: bool,
    constraints_deferred: Option<bool>,
    cur_xid: Option<u64>,
    autocommit_stmt: bool,
    isolation: IsolationLevel,

    snapshot: Option<TxnSnapshot>,

    default_isolation: IsolationLevel,

    txn_query_seen: bool,
}

impl Default for Session {
    fn default() -> Session {
        Session {
            txn_stack: Vec::new(),
            aborted: false,
            constraints_deferred: None,
            cur_xid: None,
            autocommit_stmt: false,
            isolation: IsolationLevel::ReadCommitted,
            snapshot: None,
            default_isolation: IsolationLevel::ReadCommitted,
            txn_query_seen: false,
        }
    }
}

impl Session {

    pub fn new() -> Session {
        Session::default()
    }

    pub fn current_xid(&self) -> Option<u64> {
        self.cur_xid
    }

    pub fn in_transaction(&self) -> bool {
        !self.txn_stack.is_empty()
    }

    pub fn isolation(&self) -> IsolationLevel {
        self.isolation
    }
}

#[derive(Debug, Clone)]
pub struct Catalog {
    tables: HashMap<String, Table>,
    views: HashMap<String, View>,

    domains: HashMap<String, DomainDef>,

    indexes: Vec<IndexDef>,

    sequences: HashMap<String, SequenceDef>,

    comments: HashMap<(String, i32), String>,

    operators: HashMap<String, String>,

    aggregates: HashMap<String, AggregateDef>,

    casts: HashMap<(u32, u32), CastDef>,

    partitioned: HashMap<String, PartitionInfo>,

    functions: HashMap<String, FunctionDef>,

    triggers: Vec<TriggerDef>,

    prepared: HashMap<String, PreparedStmt>,

    cursors: HashMap<String, Cursor>,

    enum_types: HashMap<String, EnumDef>,

    composites: HashMap<String, CompositeDef>,

    next_enum_oid: u32,

    txn_stack: Vec<TxnFrame>,

    aborted: bool,

    constraints_deferred: Option<bool>,

    next_xid: u64,

    committed: HashSet<u64>,

    aborted_xids: HashSet<u64>,

    cur_xid: Option<u64>,

    autocommit_stmt: bool,

    isolation: IsolationLevel,

    snapshot: Option<TxnSnapshot>,

    default_isolation: IsolationLevel,

    txn_query_seen: bool,

    open_txn_count: usize,

    open_xids: HashSet<u64>,

    row_locks: HashMap<(String, usize), LockInfo>,

    lock_skip: Option<(String, HashSet<usize>)>,

    ser_reads: std::cell::RefCell<HashMap<u64, HashSet<(String, usize)>>>,

    ser_writes: HashMap<u64, HashSet<(String, usize)>>,

    ser_open: HashSet<u64>,

    ser_retain: HashMap<u64, HashSet<u64>>,
}

impl Default for Catalog {
    fn default() -> Catalog {
        Catalog {
            tables: HashMap::new(),
            views: HashMap::new(),
            domains: HashMap::new(),
            indexes: Vec::new(),
            sequences: HashMap::new(),
            comments: HashMap::new(),
            operators: HashMap::new(),
            aggregates: HashMap::new(),
            casts: HashMap::new(),
            partitioned: HashMap::new(),
            functions: HashMap::new(),
            triggers: Vec::new(),
            prepared: HashMap::new(),
            cursors: HashMap::new(),
            enum_types: HashMap::new(),
            composites: HashMap::new(),
            next_enum_oid: FIRST_ENUM_OID,
            txn_stack: Vec::new(),
            aborted: false,
            constraints_deferred: None,
            next_xid: FIRST_REAL_XID,
            committed: HashSet::new(),
            aborted_xids: HashSet::new(),
            cur_xid: None,
            autocommit_stmt: false,
            isolation: IsolationLevel::ReadCommitted,
            snapshot: None,
            default_isolation: IsolationLevel::ReadCommitted,
            txn_query_seen: false,
            open_txn_count: 0,
            open_xids: HashSet::new(),
            row_locks: HashMap::new(),
            lock_skip: None,
            ser_reads: std::cell::RefCell::new(HashMap::new()),
            ser_writes: HashMap::new(),
            ser_open: HashSet::new(),
            ser_retain: HashMap::new(),
        }
    }
}

fn persist_err(input: String) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "postcrust persistence",
        input,
    }
}

const PERSIST_MAGIC: &[u8] = b"CRUFTPG\0";
const PERSIST_VERSION_V1_NO_FEATURES: u32 = 1;
const PERSIST_VERSION_CURRENT: u32 = 2;
const PERSIST_FEATURES_NONE: u32 = 0;
const PERSIST_FEATURE_TABLE_NOT_NULL: u32 = 1 << 0;
const PERSIST_FEATURE_TABLE_UNIQUE_KEYS: u32 = 1 << 1;
const PERSIST_FEATURE_INDEXES: u32 = 1 << 2;
const PERSIST_FEATURE_SEQUENCES: u32 = 1 << 3;
const PERSIST_FEATURE_TABLE_CHECKS: u32 = 1 << 4;
const PERSIST_FEATURE_TABLE_DEFAULTS: u32 = 1 << 5;
const PERSIST_FEATURE_TABLE_FOREIGN_KEYS: u32 = 1 << 6;
const PERSIST_FEATURE_TABLE_IDENTITY: u32 = 1 << 7;
const PERSIST_FEATURE_TABLE_GENERATED: u32 = 1 << 8;
const PERSIST_FEATURE_DOMAINS: u32 = 1 << 9;
const PERSIST_FEATURE_VIEWS: u32 = 1 << 10;
const PERSIST_FEATURE_USER_TYPES: u32 = 1 << 11;
const PERSIST_FEATURE_COMMENTS: u32 = 1 << 12;
const PERSIST_FEATURE_FUNCTIONS: u32 = 1 << 13;
const PERSIST_FEATURE_FUNCTION_REFS: u32 = 1 << 14;
const PERSIST_FEATURE_PARTITIONS: u32 = 1 << 15;
const PERSIST_FEATURE_STATS: u32 = 1 << 16;
const PERSIST_FEATURE_CASTS: u32 = 1 << 17;
const PERSIST_FEATURE_FUNCTION_SOURCE_BODIES: u32 = 1 << 18;
const PERSIST_FEATURE_TRIGGERS: u32 = 1 << 19;
const PERSIST_FEATURES_KNOWN: u32 = PERSIST_FEATURE_TABLE_NOT_NULL
    | PERSIST_FEATURE_TABLE_UNIQUE_KEYS
    | PERSIST_FEATURE_INDEXES
    | PERSIST_FEATURE_SEQUENCES
    | PERSIST_FEATURE_TABLE_CHECKS
    | PERSIST_FEATURE_TABLE_DEFAULTS
    | PERSIST_FEATURE_TABLE_FOREIGN_KEYS
    | PERSIST_FEATURE_TABLE_IDENTITY
    | PERSIST_FEATURE_TABLE_GENERATED
    | PERSIST_FEATURE_DOMAINS
    | PERSIST_FEATURE_VIEWS
    | PERSIST_FEATURE_USER_TYPES
    | PERSIST_FEATURE_COMMENTS
    | PERSIST_FEATURE_FUNCTIONS
    | PERSIST_FEATURE_FUNCTION_REFS
    | PERSIST_FEATURE_PARTITIONS
    | PERSIST_FEATURE_STATS
    | PERSIST_FEATURE_CASTS
    | PERSIST_FEATURE_FUNCTION_SOURCE_BODIES
    | PERSIST_FEATURE_TRIGGERS;

fn write_u32(out: &mut Vec<u8>, n: u32) {
    out.extend_from_slice(&n.to_le_bytes());
}

fn write_i32(out: &mut Vec<u8>, n: i32) {
    out.extend_from_slice(&n.to_le_bytes());
}

fn write_u64(out: &mut Vec<u8>, n: u64) {
    out.extend_from_slice(&n.to_le_bytes());
}

fn write_i64(out: &mut Vec<u8>, n: i64) {
    out.extend_from_slice(&n.to_le_bytes());
}

fn write_bool_vec(out: &mut Vec<u8>, values: &[bool]) {
    write_u32(out, values.len() as u32);
    for value in values {
        out.push(u8::from(*value));
    }
}

fn write_usize_vec(out: &mut Vec<u8>, values: &[usize]) {
    write_u32(out, values.len() as u32);
    for value in values {
        write_u32(out, *value as u32);
    }
}

fn write_string_option(out: &mut Vec<u8>, value: Option<&String>) {
    match value {
        Some(value) => {
            out.push(1);
            write_string(out, value);
        }
        None => out.push(0),
    }
}

fn write_string_options(out: &mut Vec<u8>, values: &[Option<String>], expected_len: usize) {
    write_u32(out, expected_len as u32);
    for idx in 0..expected_len {
        write_string_option(out, values.get(idx).and_then(|v| v.as_ref()));
    }
}

fn write_string_vec_option(out: &mut Vec<u8>, values: Option<&Vec<String>>) {
    match values {
        Some(values) => {
            out.push(1);
            write_u32(out, values.len() as u32);
            for value in values {
                write_string(out, value);
            }
        }
        None => out.push(0),
    }
}

fn write_unique_keys(out: &mut Vec<u8>, keys: &[UniqueKey]) {
    write_u32(out, keys.len() as u32);
    for key in keys {
        out.push(u8::from(key.is_primary));
        write_string_option(out, key.name.as_ref());
        write_usize_vec(out, &key.cols);
    }
}

fn write_ref_action(out: &mut Vec<u8>, action: RefAction) {
    out.push(match action {
        RefAction::NoAction => 0,
        RefAction::Restrict => 1,
        RefAction::Cascade => 2,
        RefAction::SetNull => 3,
    });
}

fn write_foreign_keys(out: &mut Vec<u8>, keys: &[ForeignKey]) {
    write_u32(out, keys.len() as u32);
    for key in keys {
        write_string_option(out, key.name.as_ref());
        write_usize_vec(out, &key.cols);
        write_string(out, &key.parent);
        write_usize_vec(out, &key.parent_cols);
        write_ref_action(out, key.on_delete);
        write_ref_action(out, key.on_update);
        out.push(u8::from(key.deferrable));
        out.push(u8::from(key.initially_deferred));
    }
}

fn write_identity_kinds(out: &mut Vec<u8>, values: &[IdentityKind], expected_len: usize) {
    write_u32(out, expected_len as u32);
    for idx in 0..expected_len {
        out.push(match values.get(idx).copied().unwrap_or_default() {
            IdentityKind::None => 0,
            IdentityKind::Always => 1,
            IdentityKind::ByDefault => 2,
        });
    }
}

fn write_un_op(out: &mut Vec<u8>, op: UnOp) {
    out.push(match op {
        UnOp::Neg => 0,
        UnOp::Plus => 1,
        UnOp::Not => 2,
    });
}

fn write_bin_op(out: &mut Vec<u8>, op: BinOp) {
    out.push(match op {
        BinOp::Or => 0,
        BinOp::And => 1,
        BinOp::Lt => 2,
        BinOp::Gt => 3,
        BinOp::Eq => 4,
        BinOp::LtEq => 5,
        BinOp::GtEq => 6,
        BinOp::NotEq => 7,
        BinOp::Add => 8,
        BinOp::Sub => 9,
        BinOp::Mul => 10,
        BinOp::Div => 11,
        BinOp::Mod => 12,
        BinOp::Pow => 13,
    });
}

fn check_expr_supported(expr: &Expr, allow_sequence_funcs: bool) -> Result<(), PgError> {
    match expr {
        Expr::Null
        | Expr::Lit(_)
        | Expr::Bool(_)
        | Expr::Int(_)
        | Expr::Float(_)
        | Expr::Str(_)
        | Expr::Column(_)
        | Expr::ColumnRef(_) => Ok(()),
        Expr::Unary { expr, .. } => check_expr_supported(expr, allow_sequence_funcs),
        Expr::Binary { left, right, .. } => {
            check_expr_supported(left, allow_sequence_funcs)?;
            check_expr_supported(right, allow_sequence_funcs)
        }
        Expr::IsNull { expr, .. } | Expr::Cast { expr, .. } | Expr::Collate { expr, .. } => {
            check_expr_supported(expr, allow_sequence_funcs)
        }
        Expr::Func {
            name,
            args,
            distinct,
            filter,
            order_by,
        } if allow_sequence_funcs
            && name == "nextval"
            && !*distinct
            && filter.is_none()
            && order_by.is_empty()
            && args.len() == 1 =>
        {
            check_expr_supported(&args[0], false)
        }
        other => Err(persist_err(format!(
            "unsupported persisted expression {other:?}"
        ))),
    }
}

fn validate_foreign_key_metadata(
    child_name: &str,
    child: &Table,
    fk: &ForeignKey,
    tables: &HashMap<String, Table>,
) -> Result<(), PgError> {
    if fk.cols.is_empty() {
        return Err(persist_err(format!(
            "table {child_name}: empty foreign key column list"
        )));
    }
    if fk.cols.len() != fk.parent_cols.len() {
        return Err(persist_err(format!(
            "table {child_name}: foreign key column width mismatch"
        )));
    }
    let child_col_count = child.schema.names().len();
    for &idx in &fk.cols {
        if idx >= child_col_count {
            return Err(persist_err(format!(
                "table {child_name}: foreign key child column out of range"
            )));
        }
    }
    let Some(parent) = tables.get(&fk.parent) else {
        return Err(persist_err(format!(
            "table {child_name}: unknown foreign key parent {}",
            fk.parent
        )));
    };
    let parent_col_count = parent.schema.names().len();
    for &idx in &fk.parent_cols {
        if idx >= parent_col_count {
            return Err(persist_err(format!(
                "table {child_name}: foreign key parent column out of range"
            )));
        }
    }
    Ok(())
}

fn is_user_type_oid(oid: u32) -> bool {
    oid >= FIRST_ENUM_OID
}

fn validate_user_type_refs(
    context: &str,
    oid: u32,
    enum_types: &HashMap<String, EnumDef>,
    composites: &HashMap<String, CompositeDef>,
) -> Result<(), PgError> {
    if !is_user_type_oid(oid) {
        return Ok(());
    }
    if enum_types.values().any(|def| def.oid == oid)
        || composites.values().any(|def| def.oid == oid)
    {
        return Ok(());
    }
    Err(persist_err(format!(
        "{context}: unknown user type oid {oid}"
    )))
}

fn validate_comment_metadata(
    rel: &str,
    subid: i32,
    tables: &HashMap<String, Table>,
) -> Result<(), PgError> {
    let Some(table) = tables.get(rel) else {
        return Err(persist_err(format!(
            "comment {rel}.{subid}: unknown relation"
        )));
    };
    if subid < 0 {
        return Err(persist_err(format!(
            "comment {rel}.{subid}: negative subid"
        )));
    }
    if subid > table.schema.names().len() as i32 {
        return Err(persist_err(format!(
            "comment {rel}.{subid}: column subid out of range"
        )));
    }
    Ok(())
}

fn validate_persisted_function(def: &FunctionDef) -> Result<(), PgError> {
    match def.language {
        Lang::Sql => match &def.body {
            FuncBody::Expr(expr) => check_expr_supported(expr, false),
            FuncBody::Query(_) => {
                if def.source_body.as_deref().unwrap_or("").trim().is_empty() {
                    return Err(persist_err(format!(
                        "function {}: query body source is required",
                        def.name
                    )));
                }
                Ok(())
            }
        },
        Lang::PlPgSql => {
            if def.pl_body.is_none() {
                return Err(persist_err(format!(
                    "function {}: plpgsql parsed body is required",
                    def.name
                )));
            }
            if def.source_body.as_deref().unwrap_or("").trim().is_empty() {
                return Err(persist_err(format!(
                    "function {}: plpgsql source body is required",
                    def.name
                )));
            }
            Ok(())
        }
    }
}

fn function_needs_source_body_format(def: &FunctionDef) -> bool {
    def.language == Lang::PlPgSql || !matches!(def.body, FuncBody::Expr(_))
}

fn validate_persisted_scalar_sql_expr_function(def: &FunctionDef) -> Result<(), PgError> {
    if def.language != Lang::Sql || def.returns != RetShape::Scalar {
        return Err(persist_err(format!(
            "function {}: function references require scalar LANGUAGE sql",
            def.name
        )));
    }
    let FuncBody::Expr(expr) = &def.body else {
        return Err(persist_err(format!(
            "function {}: function references require expression bodies",
            def.name
        )));
    };
    check_expr_supported(expr, false)
}

fn validate_function_ref(
    context: &str,
    func: &str,
    functions: &HashMap<String, FunctionDef>,
) -> Result<(), PgError> {
    let Some(def) = functions.get(func) else {
        return Err(persist_err(format!("{context}: unknown function {func}")));
    };
    validate_persisted_scalar_sql_expr_function(def)
}

fn validate_aggregate_metadata(
    def: &AggregateDef,
    functions: &HashMap<String, FunctionDef>,
) -> Result<(), PgError> {
    validate_function_ref(
        &format!("aggregate {} sfunc", def.name),
        &def.sfunc,
        functions,
    )?;
    if let Some(finalfunc) = &def.finalfunc {
        validate_function_ref(
            &format!("aggregate {} finalfunc", def.name),
            finalfunc,
            functions,
        )?;
    }
    Ok(())
}

fn validate_trigger_metadata(
    trigger: &TriggerDef,
    tables: &HashMap<String, Table>,
    functions: &HashMap<String, FunctionDef>,
) -> Result<(), PgError> {
    let Some(table) = tables.get(&trigger.table) else {
        return Err(persist_err(format!(
            "trigger {}: unknown table {}",
            trigger.name, trigger.table
        )));
    };
    if trigger.events.is_empty() {
        return Err(persist_err(format!(
            "trigger {}: empty event list",
            trigger.name
        )));
    }
    let mut seen = HashSet::new();
    for event in &trigger.events {
        if !seen.insert(*event as u8) {
            return Err(persist_err(format!(
                "trigger {}: duplicate event",
                trigger.name
            )));
        }
    }
    if let Some(when) = &trigger.when {
        check_expr_supported(when, false)?;
    }
    let Some(func) = functions.get(&trigger.func) else {
        return Err(persist_err(format!(
            "trigger {}: unknown function {}",
            trigger.name, trigger.func
        )));
    };
    if func.language != Lang::PlPgSql || func.ret_oid != 2279 || func.pl_body.is_none() {
        return Err(persist_err(format!(
            "trigger {}: function {} is not a persisted trigger function",
            trigger.name, trigger.func
        )));
    }
    if table.schema.names().is_empty() {
        return Err(persist_err(format!(
            "trigger {}: target table has no columns",
            trigger.name
        )));
    }
    Ok(())
}

fn validate_partition_metadata(
    parent: &str,
    info: &PartitionInfo,
    tables: &HashMap<String, Table>,
) -> Result<(), PgError> {
    let Some(parent_table) = tables.get(parent) else {
        return Err(persist_err(format!(
            "partition parent {parent}: unknown table"
        )));
    };
    if info.key_col >= parent_table.schema.names().len() {
        return Err(persist_err(format!(
            "partition parent {parent}: key column out of range"
        )));
    }
    let mut seen = HashSet::new();
    for child in &info.children {
        if !seen.insert(child.clone()) {
            return Err(persist_err(format!(
                "partition parent {parent}: duplicate child {child}"
            )));
        }
        let Some(child_table) = tables.get(child) else {
            return Err(persist_err(format!(
                "partition parent {parent}: unknown child {child}"
            )));
        };
        if child_table.schema.names().len() != parent_table.schema.names().len() {
            return Err(persist_err(format!(
                "partition parent {parent}: child {child} width mismatch"
            )));
        }
    }
    if let Some(default_child) = &info.default_child {
        if !seen.contains(default_child) {
            return Err(persist_err(format!(
                "partition parent {parent}: default child {default_child} not registered"
            )));
        }
    }
    Ok(())
}

fn validate_table_stats(name: &str, table: &Table) -> Result<(), PgError> {
    let width = table.schema.names().len();
    if table.reltuples < -1.0 {
        return Err(persist_err(format!("table {name}: invalid reltuples")));
    }
    for (&idx, stats) in &table.stats {
        if idx >= width {
            return Err(persist_err(format!(
                "table {name}: stats column out of range"
            )));
        }
        if stats.most_common_vals.len() != stats.most_common_freqs.len() {
            return Err(persist_err(format!(
                "table {name}: stats MCV width mismatch"
            )));
        }
    }
    Ok(())
}

fn parse_persisted_view_query(sql: &str) -> Result<SelectStmt, PgError> {
    let toks = crate::expr::lexer::lex(sql)?;
    let (query, next) = crate::stmt::parser::parse_query_at(&toks, 0, sql)?;
    if !matches!(toks.get(next), Some(crate::expr::lexer::Tok::Eof)) {
        return Err(persist_err(format!(
            "view query did not consume input: {sql}"
        )));
    }
    Ok(query)
}

fn parse_persisted_function_query(name: &str, source: &str) -> Result<SelectStmt, PgError> {
    let trimmed = source.trim();
    let sql = if trimmed.to_ascii_lowercase().starts_with("select") {
        trimmed.to_string()
    } else {
        format!("SELECT {trimmed}")
    };
    let stmt = crate::stmt::parser::parse(&sql).map_err(|_| {
        persist_err(format!(
            "function {name}: could not parse persisted SQL body"
        ))
    })?;
    let crate::stmt::ast::Stmt::Select(sel) = stmt;
    Ok(sel)
}

fn write_expr(out: &mut Vec<u8>, expr: &Expr, allow_sequence_funcs: bool) -> Result<(), PgError> {
    check_expr_supported(expr, allow_sequence_funcs)?;
    match expr {
        Expr::Null => out.push(0),
        Expr::Lit(value) => {
            out.push(1);
            write_sql_value(out, value);
        }
        Expr::Bool(value) => {
            out.push(2);
            out.push(u8::from(*value));
        }
        Expr::Int(value) => {
            out.push(3);
            write_i64(out, *value);
        }
        Expr::Float(value) => {
            out.push(4);
            write_f64(out, *value);
        }
        Expr::Str(value) => {
            out.push(5);
            write_string(out, value);
        }
        Expr::Column(value) => {
            out.push(6);
            write_string(out, value);
        }
        Expr::ColumnRef(value) => {
            out.push(7);
            write_u32(out, *value as u32);
        }
        Expr::Unary { op, expr } => {
            out.push(8);
            write_un_op(out, *op);
            write_expr(out, expr, allow_sequence_funcs)?;
        }
        Expr::Binary { op, left, right } => {
            out.push(9);
            write_bin_op(out, *op);
            write_expr(out, left, allow_sequence_funcs)?;
            write_expr(out, right, allow_sequence_funcs)?;
        }
        Expr::IsNull { expr, negated } => {
            out.push(10);
            out.push(u8::from(*negated));
            write_expr(out, expr, allow_sequence_funcs)?;
        }
        Expr::Cast { expr, type_name } => {
            out.push(11);
            write_string(out, type_name);
            write_expr(out, expr, allow_sequence_funcs)?;
        }
        Expr::Collate { expr, collation } => {
            out.push(12);
            write_string(out, collation);
            write_expr(out, expr, allow_sequence_funcs)?;
        }
        Expr::Func { name, args, .. } => {
            out.push(13);
            write_string(out, name);
            write_u32(out, args.len() as u32);
            for arg in args {
                write_expr(out, arg, false)?;
            }
        }
        _ => unreachable!("check_expr_supported gates expression alphabet"),
    }
    Ok(())
}

fn write_checks(out: &mut Vec<u8>, checks: &[CheckConstraint]) -> Result<(), PgError> {
    write_u32(out, checks.len() as u32);
    for check in checks {
        write_string_option(out, check.name.as_ref());
        write_expr(out, &check.expr, false)?;
    }
    Ok(())
}

fn write_expr_options(
    out: &mut Vec<u8>,
    values: &[Option<Expr>],
    expected_len: usize,
) -> Result<(), PgError> {
    write_u32(out, expected_len as u32);
    for idx in 0..expected_len {
        match values.get(idx).and_then(|value| value.as_ref()) {
            Some(expr) => {
                out.push(1);
                write_expr(out, expr, true)?;
            }
            None => out.push(0),
        }
    }
    Ok(())
}

fn write_expr_option(
    out: &mut Vec<u8>,
    value: Option<&Expr>,
    allow_sequence_funcs: bool,
) -> Result<(), PgError> {
    match value {
        Some(expr) => {
            out.push(1);
            write_expr(out, expr, allow_sequence_funcs)?;
        }
        None => out.push(0),
    }
    Ok(())
}

fn write_domains(out: &mut Vec<u8>, domains: &HashMap<String, DomainDef>) -> Result<(), PgError> {
    let mut domains: Vec<(&String, &DomainDef)> = domains.iter().collect();
    domains.sort_by(|(a, _), (b, _)| a.cmp(b));
    write_u32(out, domains.len() as u32);
    for (name, domain) in domains {
        if name != &domain.name {
            return Err(persist_err(format!("domain {name}: name mismatch")));
        }
        write_string(out, name);
        write_u32(out, domain.base_oid);
        write_i32(out, domain.base_typmod);
        out.push(u8::from(domain.not_null));
        write_expr_option(out, domain.default.as_ref(), false)?;
        write_checks(out, &domain.checks)?;
    }
    Ok(())
}

fn write_indexes(out: &mut Vec<u8>, indexes: &[IndexDef]) {
    write_u32(out, indexes.len() as u32);
    for index in indexes {
        write_string(out, &index.name);
        write_string(out, &index.table);
        out.push(u8::from(index.unique));
        write_usize_vec(out, &index.cols);
    }
}

fn write_i64_option(out: &mut Vec<u8>, value: Option<i64>) {
    match value {
        Some(value) => {
            out.push(1);
            write_i64(out, value);
        }
        None => out.push(0),
    }
}

fn write_sequences(out: &mut Vec<u8>, sequences: &HashMap<String, SequenceDef>) {
    let mut sequences: Vec<(&String, &SequenceDef)> = sequences.iter().collect();
    sequences.sort_by(|(a, _), (b, _)| a.cmp(b));
    write_u32(out, sequences.len() as u32);
    for (name, seq) in sequences {
        write_string(out, name);
        write_i64(out, seq.increment);
        write_i64(out, seq.min);
        write_i64(out, seq.max);
        write_i64(out, seq.start);
        write_i64(out, seq.cache);
        out.push(u8::from(seq.cycle));
        write_i64_option(out, seq.current);
    }
}

fn write_views(out: &mut Vec<u8>, views: &HashMap<String, View>) {
    let mut views: Vec<(&String, &View)> = views.iter().collect();
    views.sort_by(|(a, _), (b, _)| a.cmp(b));
    write_u32(out, views.len() as u32);
    for (name, view) in views {
        write_string(out, name);
        write_string(out, &view.query_sql);
        write_string_vec_option(out, view.columns.as_ref());
        out.push(u8::from(view.materialized));
        out.push(u8::from(view.check_option));
        if view.materialized {
            write_u32(out, view.mat_columns.len() as u32);
            for col in &view.mat_columns {
                write_string(out, col);
            }
            write_u32(out, view.mat_col_types.len() as u32);
            for oid in &view.mat_col_types {
                write_u32(out, *oid);
            }
            write_u32(out, view.mat_rows.len() as u32);
            for row in &view.mat_rows {
                write_u32(out, row.len() as u32);
                for value in row {
                    write_sql_value(out, value);
                }
            }
        }
    }
}

fn write_user_types(
    out: &mut Vec<u8>,
    enum_types: &HashMap<String, EnumDef>,
    composites: &HashMap<String, CompositeDef>,
) {
    let mut enums: Vec<(&String, &EnumDef)> = enum_types.iter().collect();
    enums.sort_by(|(a, _), (b, _)| a.cmp(b));
    write_u32(out, enums.len() as u32);
    for (name, def) in enums {
        write_string(out, name);
        write_u32(out, def.oid);
        write_u32(out, def.labels.len() as u32);
        for label in &def.labels {
            write_string(out, label);
        }
    }

    let mut composite_defs: Vec<(&String, &CompositeDef)> = composites.iter().collect();
    composite_defs.sort_by(|(a, _), (b, _)| a.cmp(b));
    write_u32(out, composite_defs.len() as u32);
    for (name, def) in composite_defs {
        write_string(out, name);
        write_u32(out, def.oid);
        write_u32(out, def.fields.len() as u32);
        for (field_name, field_oid, field_typmod) in &def.fields {
            write_string(out, field_name);
            write_u32(out, *field_oid);
            write_i32(out, *field_typmod);
        }
    }
}

fn write_comments(out: &mut Vec<u8>, comments: &HashMap<(String, i32), String>) {
    let mut comments: Vec<(&(String, i32), &String)> = comments.iter().collect();
    comments.sort_by(|((rel_a, sub_a), _), ((rel_b, sub_b), _)| {
        rel_a.cmp(rel_b).then_with(|| sub_a.cmp(sub_b))
    });
    write_u32(out, comments.len() as u32);
    for ((rel, subid), text) in comments {
        write_string(out, rel);
        write_i32(out, *subid);
        write_string(out, text);
    }
}

fn write_volatility(out: &mut Vec<u8>, volatility: Volatility) {
    out.push(match volatility {
        Volatility::Immutable => 0,
        Volatility::Stable => 1,
        Volatility::Volatile => 2,
    });
}

fn write_lang(out: &mut Vec<u8>, language: Lang) {
    out.push(match language {
        Lang::Sql => 0,
        Lang::PlPgSql => 1,
    });
}

fn write_ret_shape(out: &mut Vec<u8>, returns: &RetShape) {
    match returns {
        RetShape::Scalar => out.push(0),
        RetShape::SetofScalar { oid } => {
            out.push(1);
            write_u32(out, *oid);
        }
        RetShape::SetofTable(cols) => {
            out.push(2);
            write_u32(out, cols.len() as u32);
            for (name, oid, typmod) in cols {
                write_string(out, name);
                write_u32(out, *oid);
                write_i32(out, *typmod);
            }
        }
        RetShape::SetofRel => out.push(3),
    }
}

fn write_functions(
    out: &mut Vec<u8>,
    functions: &HashMap<String, FunctionDef>,
    source_body_format: bool,
) -> Result<(), PgError> {
    let mut functions: Vec<(&String, &FunctionDef)> = functions.iter().collect();
    functions.sort_by(|(name_a, _), (name_b, _)| name_a.cmp(name_b));
    write_u32(out, functions.len() as u32);
    for (name, def) in functions {
        validate_persisted_function(def)?;
        write_string(out, name);
        write_u32(out, def.args.len() as u32);
        for (arg_name, oid, typmod) in &def.args {
            write_string_option(out, arg_name.as_ref());
            write_u32(out, *oid);
            write_i32(out, *typmod);
        }
        write_u32(out, def.ret_oid);
        write_i32(out, def.ret_typmod);
        out.push(u8::from(def.strict));
        write_volatility(out, def.volatility);
        if source_body_format {
            write_lang(out, def.language);
            write_ret_shape(out, &def.returns);
            match (&def.language, &def.body) {
                (Lang::Sql, FuncBody::Expr(expr)) if def.source_body.is_none() => {
                    out.push(0);
                    write_expr(out, expr, false)?;
                }
                (Lang::Sql, FuncBody::Query(_)) => {
                    out.push(1);
                    write_string(
                        out,
                        def.source_body
                            .as_ref()
                            .expect("validate_persisted_function gates source body"),
                    );
                }
                (Lang::PlPgSql, _) => {
                    out.push(2);
                    write_string(
                        out,
                        def.source_body
                            .as_ref()
                            .expect("validate_persisted_function gates pl source body"),
                    );
                }
                (Lang::Sql, FuncBody::Expr(expr)) => {
                    out.push(0);
                    write_expr(out, expr, false)?;
                }
            }
        } else {
            let FuncBody::Expr(expr) = &def.body else {
                unreachable!("source_body_format gates body shape")
            };
            write_expr(out, expr, false)?;
        }
    }
    Ok(())
}

fn write_function_refs(
    out: &mut Vec<u8>,
    operators: &HashMap<String, String>,
    aggregates: &HashMap<String, AggregateDef>,
    functions: &HashMap<String, FunctionDef>,
) -> Result<(), PgError> {
    let mut operators: Vec<(&String, &String)> = operators.iter().collect();
    operators.sort_by(|(symbol_a, _), (symbol_b, _)| symbol_a.cmp(symbol_b));
    write_u32(out, operators.len() as u32);
    for (symbol, func) in operators {
        validate_function_ref(&format!("operator {symbol}"), func, functions)?;
        write_string(out, symbol);
        write_string(out, func);
    }

    let mut aggregates: Vec<(&String, &AggregateDef)> = aggregates.iter().collect();
    aggregates.sort_by(|(name_a, _), (name_b, _)| name_a.cmp(name_b));
    write_u32(out, aggregates.len() as u32);
    for (name, def) in aggregates {
        validate_aggregate_metadata(def, functions)?;
        write_string(out, name);
        write_string(out, &def.sfunc);
        write_u32(out, def.stype_oid);
        write_string(out, &def.initcond);
        write_string_option(out, def.finalfunc.as_ref());
    }
    Ok(())
}

fn write_casts(
    out: &mut Vec<u8>,
    casts: &HashMap<(u32, u32), CastDef>,
    functions: &HashMap<String, FunctionDef>,
) -> Result<(), PgError> {
    let mut casts: Vec<(&(u32, u32), &CastDef)> = casts.iter().collect();
    casts.sort_by(|((source_a, target_a), _), ((source_b, target_b), _)| {
        source_a.cmp(source_b).then_with(|| target_a.cmp(target_b))
    });
    write_u32(out, casts.len() as u32);
    for ((source_oid, target_oid), def) in casts {
        if def.source_oid != *source_oid || def.target_oid != *target_oid {
            return Err(persist_err(format!(
                "cast {} -> {}: key mismatch",
                source_oid, target_oid
            )));
        }
        validate_function_ref(
            &format!("cast {} -> {}", source_oid, target_oid),
            &def.func,
            functions,
        )?;
        write_u32(out, *source_oid);
        write_u32(out, *target_oid);
        write_string(out, &def.func);
    }
    Ok(())
}

fn write_trig_timing(out: &mut Vec<u8>, timing: TrigTiming) {
    out.push(match timing {
        TrigTiming::Before => 0,
        TrigTiming::After => 1,
    });
}

fn write_trig_event(out: &mut Vec<u8>, event: TrigEvent) {
    out.push(match event {
        TrigEvent::Insert => 0,
        TrigEvent::Update => 1,
        TrigEvent::Delete => 2,
    });
}

fn write_triggers(
    out: &mut Vec<u8>,
    triggers: &[TriggerDef],
    tables: &HashMap<String, Table>,
    functions: &HashMap<String, FunctionDef>,
) -> Result<(), PgError> {
    let mut triggers: Vec<&TriggerDef> = triggers.iter().collect();
    triggers.sort_by(|a, b| a.table.cmp(&b.table).then_with(|| a.name.cmp(&b.name)));
    write_u32(out, triggers.len() as u32);
    let mut seen = HashSet::new();
    for trigger in triggers {
        validate_trigger_metadata(trigger, tables, functions)?;
        if !seen.insert((trigger.table.clone(), trigger.name.clone())) {
            return Err(persist_err(format!(
                "duplicate trigger {} on {}",
                trigger.name, trigger.table
            )));
        }
        write_string(out, &trigger.name);
        write_trig_timing(out, trigger.timing);
        write_u32(out, trigger.events.len() as u32);
        for event in &trigger.events {
            write_trig_event(out, *event);
        }
        write_string(out, &trigger.table);
        write_expr_option(out, trigger.when.as_ref(), false)?;
        write_string(out, &trigger.func);
    }
    Ok(())
}

fn write_partitions(
    out: &mut Vec<u8>,
    partitioned: &HashMap<String, PartitionInfo>,
    tables: &HashMap<String, Table>,
) -> Result<(), PgError> {
    let mut partitioned: Vec<(&String, &PartitionInfo)> = partitioned.iter().collect();
    partitioned.sort_by(|(parent_a, _), (parent_b, _)| parent_a.cmp(parent_b));
    write_u32(out, partitioned.len() as u32);
    for (parent, info) in partitioned {
        validate_partition_metadata(parent, info, tables)?;
        write_string(out, parent);
        write_u32(out, info.key_col as u32);
        write_u32(out, info.children.len() as u32);
        for child in &info.children {
            write_string(out, child);
        }
        write_string_option(out, info.default_child.as_ref());
    }
    Ok(())
}

fn write_sql_values(out: &mut Vec<u8>, values: &[SqlValue]) {
    write_u32(out, values.len() as u32);
    for value in values {
        write_sql_value(out, value);
    }
}

fn write_stats(out: &mut Vec<u8>, table: &Table) {
    write_f64(out, table.reltuples);
    write_u32(out, table.stats.len() as u32);
    for (&idx, stats) in &table.stats {
        write_u32(out, idx as u32);
        write_f64(out, stats.null_frac);
        write_i32(out, stats.avg_width);
        write_f64(out, stats.n_distinct);
        write_sql_values(out, &stats.most_common_vals);
        write_u32(out, stats.most_common_freqs.len() as u32);
        for freq in &stats.most_common_freqs {
            write_f64(out, *freq);
        }
        write_sql_values(out, &stats.histogram_bounds);
        match stats.correlation {
            Some(value) => {
                out.push(1);
                write_f64(out, value);
            }
            None => out.push(0),
        }
    }
}

fn write_f64(out: &mut Vec<u8>, n: f64) {
    out.extend_from_slice(&n.to_le_bytes());
}

fn write_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    write_u32(out, bytes.len() as u32);
    out.extend_from_slice(bytes);
}

fn write_string(out: &mut Vec<u8>, s: &str) {
    write_bytes(out, s.as_bytes());
}

fn write_sql_value(out: &mut Vec<u8>, value: &SqlValue) {
    match value {
        SqlValue::Null => out.push(0),
        SqlValue::Int(n) => {
            out.push(1);
            out.extend_from_slice(&n.to_le_bytes());
        }
        SqlValue::Real(n) => {
            out.push(2);
            write_f64(out, *n);
        }
        SqlValue::Text(s) => {
            out.push(3);
            write_string(out, s);
        }
        SqlValue::Blob(bytes) => {
            out.push(4);
            write_bytes(out, bytes);
        }
    }
}

struct PersistReader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> PersistReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn expect_magic(&mut self, magic: &[u8]) -> Result<(), PgError> {
        let got = self.read_exact(magic.len())?;
        if got != magic {
            return Err(persist_err("bad magic".into()));
        }
        Ok(())
    }

    fn finish(&self) -> Result<(), PgError> {
        if self.pos == self.bytes.len() {
            Ok(())
        } else {
            Err(persist_err("trailing bytes".into()))
        }
    }

    fn read_exact(&mut self, n: usize) -> Result<&'a [u8], PgError> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| persist_err("offset overflow".into()))?;
        if end > self.bytes.len() {
            return Err(persist_err("truncated payload".into()));
        }
        let out = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(out)
    }

    fn read_u32(&mut self) -> Result<u32, PgError> {
        let b = self.read_exact(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn read_i32(&mut self) -> Result<i32, PgError> {
        let b = self.read_exact(4)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn read_u64(&mut self) -> Result<u64, PgError> {
        let b = self.read_exact(8)?;
        Ok(u64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    fn read_i64(&mut self) -> Result<i64, PgError> {
        let b = self.read_exact(8)?;
        Ok(i64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    fn read_f64(&mut self) -> Result<f64, PgError> {
        let b = self.read_exact(8)?;
        Ok(f64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    fn read_bytes(&mut self) -> Result<Vec<u8>, PgError> {
        let len = self.read_u32()? as usize;
        Ok(self.read_exact(len)?.to_vec())
    }

    fn read_string(&mut self) -> Result<String, PgError> {
        String::from_utf8(self.read_bytes()?).map_err(|_| persist_err("invalid utf8 string".into()))
    }

    fn read_bool_vec(&mut self, expected_len: usize) -> Result<Vec<bool>, PgError> {
        let len = self.read_u32()? as usize;
        if len != expected_len {
            return Err(persist_err("bool vector width mismatch".into()));
        }
        let mut out = Vec::with_capacity(len);
        for _ in 0..len {
            match self.read_exact(1)?[0] {
                0 => out.push(false),
                1 => out.push(true),
                other => return Err(persist_err(format!("invalid bool tag {other}"))),
            }
        }
        Ok(out)
    }

    fn read_bool_tag(&mut self, context: &str) -> Result<bool, PgError> {
        match self.read_exact(1)?[0] {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(persist_err(format!("invalid {context} bool tag {other}"))),
        }
    }

    fn read_string_option(&mut self) -> Result<Option<String>, PgError> {
        match self.read_exact(1)?[0] {
            0 => Ok(None),
            1 => Ok(Some(self.read_string()?)),
            other => Err(persist_err(format!("invalid option tag {other}"))),
        }
    }

    fn read_usize_vec(
        &mut self,
        max_exclusive: usize,
        context: &str,
    ) -> Result<Vec<usize>, PgError> {
        let len = self.read_u32()? as usize;
        let mut out = Vec::with_capacity(len);
        for _ in 0..len {
            let idx = self.read_u32()? as usize;
            if idx >= max_exclusive {
                return Err(persist_err(format!("{context} column out of range")));
            }
            out.push(idx);
        }
        Ok(out)
    }

    fn read_string_options(&mut self, expected_len: usize) -> Result<Vec<Option<String>>, PgError> {
        let len = self.read_u32()? as usize;
        if len != expected_len {
            return Err(persist_err("string option vector width mismatch".into()));
        }
        let mut out = Vec::with_capacity(len);
        for _ in 0..len {
            out.push(self.read_string_option()?);
        }
        Ok(out)
    }

    fn read_string_vec_option(&mut self) -> Result<Option<Vec<String>>, PgError> {
        match self.read_exact(1)?[0] {
            0 => Ok(None),
            1 => {
                let len = self.read_u32()? as usize;
                let mut out = Vec::with_capacity(len);
                for _ in 0..len {
                    out.push(self.read_string()?);
                }
                Ok(Some(out))
            }
            other => Err(persist_err(format!(
                "invalid string vec option tag {other}"
            ))),
        }
    }

    fn read_unique_keys(&mut self, col_count: usize) -> Result<Vec<UniqueKey>, PgError> {
        let key_count = self.read_u32()? as usize;
        let mut out = Vec::with_capacity(key_count);
        for _ in 0..key_count {
            let is_primary = self.read_bool_tag("unique key primary")?;
            let name = self.read_string_option()?;
            let cols = self.read_usize_vec(col_count, "unique key")?;
            if cols.is_empty() {
                return Err(persist_err("empty unique key".into()));
            }
            out.push(UniqueKey {
                cols,
                is_primary,
                name,
            });
        }
        Ok(out)
    }

    fn read_ref_action(&mut self) -> Result<RefAction, PgError> {
        match self.read_exact(1)?[0] {
            0 => Ok(RefAction::NoAction),
            1 => Ok(RefAction::Restrict),
            2 => Ok(RefAction::Cascade),
            3 => Ok(RefAction::SetNull),
            other => Err(persist_err(format!(
                "unknown foreign key ref-action tag {other}"
            ))),
        }
    }

    fn read_foreign_keys(&mut self, child_col_count: usize) -> Result<Vec<ForeignKey>, PgError> {
        let key_count = self.read_u32()? as usize;
        let mut out = Vec::with_capacity(key_count);
        for _ in 0..key_count {
            let name = self.read_string_option()?;
            let cols = self.read_usize_vec(child_col_count, "foreign key child")?;
            if cols.is_empty() {
                return Err(persist_err("empty foreign key column list".into()));
            }
            let parent = self.read_string()?;
            let parent_cols = self.read_usize_vec(usize::MAX, "foreign key parent")?;
            if cols.len() != parent_cols.len() {
                return Err(persist_err("foreign key column width mismatch".into()));
            }
            out.push(ForeignKey {
                cols,
                parent,
                parent_cols,
                on_delete: self.read_ref_action()?,
                on_update: self.read_ref_action()?,
                name,
                deferrable: self.read_bool_tag("foreign key deferrable")?,
                initially_deferred: self.read_bool_tag("foreign key initially deferred")?,
            });
        }
        Ok(out)
    }

    fn read_identity_kinds(&mut self, expected_len: usize) -> Result<Vec<IdentityKind>, PgError> {
        let len = self.read_u32()? as usize;
        if len != expected_len {
            return Err(persist_err("identity vector width mismatch".into()));
        }
        let mut out = Vec::with_capacity(len);
        for _ in 0..len {
            out.push(match self.read_exact(1)?[0] {
                0 => IdentityKind::None,
                1 => IdentityKind::Always,
                2 => IdentityKind::ByDefault,
                other => return Err(persist_err(format!("unknown identity kind tag {other}"))),
            });
        }
        Ok(out)
    }

    fn read_un_op(&mut self) -> Result<UnOp, PgError> {
        match self.read_exact(1)?[0] {
            0 => Ok(UnOp::Neg),
            1 => Ok(UnOp::Plus),
            2 => Ok(UnOp::Not),
            other => Err(persist_err(format!("unknown unary operator tag {other}"))),
        }
    }

    fn read_bin_op(&mut self) -> Result<BinOp, PgError> {
        match self.read_exact(1)?[0] {
            0 => Ok(BinOp::Or),
            1 => Ok(BinOp::And),
            2 => Ok(BinOp::Lt),
            3 => Ok(BinOp::Gt),
            4 => Ok(BinOp::Eq),
            5 => Ok(BinOp::LtEq),
            6 => Ok(BinOp::GtEq),
            7 => Ok(BinOp::NotEq),
            8 => Ok(BinOp::Add),
            9 => Ok(BinOp::Sub),
            10 => Ok(BinOp::Mul),
            11 => Ok(BinOp::Div),
            12 => Ok(BinOp::Mod),
            13 => Ok(BinOp::Pow),
            other => Err(persist_err(format!("unknown binary operator tag {other}"))),
        }
    }

    fn read_expr(&mut self, col_count: usize, allow_sequence_funcs: bool) -> Result<Expr, PgError> {
        let expr = match self.read_exact(1)?[0] {
            0 => Expr::Null,
            1 => Expr::Lit(self.read_sql_value()?),
            2 => Expr::Bool(self.read_bool_tag("expr bool")?),
            3 => Expr::Int(self.read_i64()?),
            4 => Expr::Float(self.read_f64()?),
            5 => Expr::Str(self.read_string()?),
            6 => Expr::Column(self.read_string()?),
            7 => {
                let idx = self.read_u32()? as usize;
                if idx >= col_count {
                    return Err(persist_err("check expression column out of range".into()));
                }
                Expr::ColumnRef(idx)
            }
            8 => Expr::Unary {
                op: self.read_un_op()?,
                expr: Box::new(self.read_expr(col_count, allow_sequence_funcs)?),
            },
            9 => Expr::Binary {
                op: self.read_bin_op()?,
                left: Box::new(self.read_expr(col_count, allow_sequence_funcs)?),
                right: Box::new(self.read_expr(col_count, allow_sequence_funcs)?),
            },
            10 => {
                let negated = self.read_bool_tag("is null negated")?;
                Expr::IsNull {
                    expr: Box::new(self.read_expr(col_count, allow_sequence_funcs)?),
                    negated,
                }
            }
            11 => {
                let type_name = self.read_string()?;
                Expr::Cast {
                    expr: Box::new(self.read_expr(col_count, allow_sequence_funcs)?),
                    type_name,
                }
            }
            12 => {
                let collation = self.read_string()?;
                Expr::Collate {
                    expr: Box::new(self.read_expr(col_count, allow_sequence_funcs)?),
                    collation,
                }
            }
            13 => {
                let name = self.read_string()?;
                let argc = self.read_u32()? as usize;
                let mut args = Vec::with_capacity(argc);
                for _ in 0..argc {
                    args.push(self.read_expr(col_count, false)?);
                }
                Expr::Func {
                    name,
                    args,
                    distinct: false,
                    filter: None,
                    order_by: Vec::new(),
                }
            }
            other => return Err(persist_err(format!("unknown expression tag {other}"))),
        };
        check_expr_supported(&expr, allow_sequence_funcs)?;
        Ok(expr)
    }

    fn read_checks(&mut self, col_count: usize) -> Result<Vec<CheckConstraint>, PgError> {
        let check_count = self.read_u32()? as usize;
        let mut out = Vec::with_capacity(check_count);
        for _ in 0..check_count {
            out.push(CheckConstraint {
                name: self.read_string_option()?,
                expr: self.read_expr(col_count, false)?,
            });
        }
        Ok(out)
    }

    fn read_expr_options(&mut self, expected_len: usize) -> Result<Vec<Option<Expr>>, PgError> {
        let len = self.read_u32()? as usize;
        if len != expected_len {
            return Err(persist_err("expression vector width mismatch".into()));
        }
        let mut out = Vec::with_capacity(len);
        for _ in 0..len {
            match self.read_exact(1)?[0] {
                0 => out.push(None),
                1 => out.push(Some(self.read_expr(expected_len, true)?)),
                other => {
                    return Err(persist_err(format!(
                        "invalid expression option tag {other}"
                    )))
                }
            }
        }
        Ok(out)
    }

    fn read_expr_option(
        &mut self,
        col_count: usize,
        allow_sequence_funcs: bool,
    ) -> Result<Option<Expr>, PgError> {
        match self.read_exact(1)?[0] {
            0 => Ok(None),
            1 => Ok(Some(self.read_expr(col_count, allow_sequence_funcs)?)),
            other => Err(persist_err(format!(
                "invalid expression option tag {other}"
            ))),
        }
    }

    fn read_domains(&mut self) -> Result<HashMap<String, DomainDef>, PgError> {
        let domain_count = self.read_u32()? as usize;
        let mut out = HashMap::new();
        for _ in 0..domain_count {
            let name = self.read_string()?;
            let domain = DomainDef {
                name: name.clone(),
                base_oid: self.read_u32()?,
                base_typmod: self.read_i32()?,
                not_null: self.read_bool_tag("domain not-null")?,
                default: self.read_expr_option(1, false)?,
                checks: self.read_checks(1)?,
            };
            if let Some(default) = &domain.default {
                check_expr_supported(default, false)?;
            }
            for check in &domain.checks {
                check_expr_supported(&check.expr, false)?;
            }
            if out.insert(name.clone(), domain).is_some() {
                return Err(persist_err(format!("duplicate domain {name}")));
            }
        }
        Ok(out)
    }

    fn read_indexes(&mut self, tables: &HashMap<String, Table>) -> Result<Vec<IndexDef>, PgError> {
        let index_count = self.read_u32()? as usize;
        let mut out = Vec::with_capacity(index_count);
        let mut names = HashSet::new();
        for _ in 0..index_count {
            let name = self.read_string()?;
            if !names.insert(name.clone()) {
                return Err(persist_err(format!("duplicate index name {name}")));
            }
            let table = self.read_string()?;
            let Some(table_def) = tables.get(&table) else {
                return Err(persist_err(format!("index {name}: unknown table {table}")));
            };
            let unique = self.read_bool_tag("index unique")?;
            let cols = self.read_usize_vec(table_def.schema.names().len(), "index")?;
            if cols.is_empty() {
                return Err(persist_err(format!("index {name}: empty column list")));
            }
            out.push(IndexDef {
                name,
                table,
                cols,
                unique,
            });
        }
        Ok(out)
    }

    fn read_i64_option(&mut self) -> Result<Option<i64>, PgError> {
        match self.read_exact(1)?[0] {
            0 => Ok(None),
            1 => Ok(Some(self.read_i64()?)),
            other => Err(persist_err(format!("invalid i64 option tag {other}"))),
        }
    }

    fn read_sequences(&mut self) -> Result<HashMap<String, SequenceDef>, PgError> {
        let sequence_count = self.read_u32()? as usize;
        let mut out = HashMap::new();
        for _ in 0..sequence_count {
            let name = self.read_string()?;
            let sequence = SequenceDef {
                name: name.clone(),
                increment: self.read_i64()?,
                min: self.read_i64()?,
                max: self.read_i64()?,
                start: self.read_i64()?,
                cache: self.read_i64()?,
                cycle: self.read_bool_tag("sequence cycle")?,
                current: self.read_i64_option()?,
            };
            if sequence.increment == 0 {
                return Err(persist_err(format!("sequence {name}: zero increment")));
            }
            if sequence.min > sequence.max {
                return Err(persist_err(format!("sequence {name}: invalid bounds")));
            }
            if out.insert(name.clone(), sequence).is_some() {
                return Err(persist_err(format!("duplicate sequence {name}")));
            }
        }
        Ok(out)
    }

    fn read_views(
        &mut self,
        tables: &HashMap<String, Table>,
    ) -> Result<HashMap<String, View>, PgError> {
        let view_count = self.read_u32()? as usize;
        let mut out = HashMap::new();
        for _ in 0..view_count {
            let name = self.read_string()?;
            if tables.contains_key(&name) {
                return Err(persist_err(format!("view {name}: collides with table")));
            }
            let query_sql = self.read_string()?;
            if query_sql.trim().is_empty() {
                return Err(persist_err(format!("view {name}: empty query")));
            }
            let query = parse_persisted_view_query(&query_sql)?;
            let columns = self.read_string_vec_option()?;
            let materialized = self.read_bool_tag("view materialized")?;
            let check_option = self.read_bool_tag("view check option")?;
            let (mat_columns, mat_col_types, mat_rows) = if materialized {
                let col_count = self.read_u32()? as usize;
                let mut mat_columns = Vec::with_capacity(col_count);
                for _ in 0..col_count {
                    mat_columns.push(self.read_string()?);
                }
                let type_count = self.read_u32()? as usize;
                if type_count != col_count {
                    return Err(persist_err(format!(
                        "view {name}: materialized type width mismatch"
                    )));
                }
                let mut mat_col_types = Vec::with_capacity(type_count);
                for _ in 0..type_count {
                    mat_col_types.push(self.read_u32()?);
                }
                let row_count = self.read_u32()? as usize;
                let mut mat_rows = Vec::with_capacity(row_count);
                for _ in 0..row_count {
                    let width = self.read_u32()? as usize;
                    if width != col_count {
                        return Err(persist_err(format!(
                            "view {name}: materialized row width mismatch"
                        )));
                    }
                    let mut row = Vec::with_capacity(width);
                    for _ in 0..width {
                        row.push(self.read_sql_value()?);
                    }
                    mat_rows.push(row);
                }
                (mat_columns, mat_col_types, mat_rows)
            } else {
                (Vec::new(), Vec::new(), Vec::new())
            };
            if out
                .insert(
                    name.clone(),
                    View {
                        query,
                        query_sql,
                        columns,
                        materialized,
                        mat_columns,
                        mat_col_types,
                        mat_rows,
                        check_option,
                    },
                )
                .is_some()
            {
                return Err(persist_err(format!("duplicate view {name}")));
            }
        }
        Ok(out)
    }

    fn read_user_types(
        &mut self,
    ) -> Result<(HashMap<String, EnumDef>, HashMap<String, CompositeDef>, u32), PgError> {
        let mut enum_types = HashMap::new();
        let mut composites = HashMap::new();
        let mut oids = HashSet::new();
        let mut max_oid = FIRST_ENUM_OID - 1;

        let enum_count = self.read_u32()? as usize;
        for _ in 0..enum_count {
            let name = self.read_string()?;
            let oid = self.read_u32()?;
            if oid < FIRST_ENUM_OID || !oids.insert(oid) {
                return Err(persist_err(format!("enum {name}: invalid oid {oid}")));
            }
            max_oid = max_oid.max(oid);
            let label_count = self.read_u32()? as usize;
            let mut labels = Vec::with_capacity(label_count);
            let mut seen = HashSet::new();
            for _ in 0..label_count {
                let label = self.read_string()?;
                if !seen.insert(label.clone()) {
                    return Err(persist_err(format!("enum {name}: duplicate label {label}")));
                }
                labels.push(label);
            }
            if enum_types
                .insert(
                    name.clone(),
                    EnumDef {
                        name: name.clone(),
                        oid,
                        labels,
                    },
                )
                .is_some()
            {
                return Err(persist_err(format!("duplicate enum type {name}")));
            }
        }

        let composite_count = self.read_u32()? as usize;
        for _ in 0..composite_count {
            let name = self.read_string()?;
            let oid = self.read_u32()?;
            if oid < FIRST_ENUM_OID || !oids.insert(oid) {
                return Err(persist_err(format!("composite {name}: invalid oid {oid}")));
            }
            max_oid = max_oid.max(oid);
            let field_count = self.read_u32()? as usize;
            let mut fields = Vec::with_capacity(field_count);
            let mut seen = HashSet::new();
            for _ in 0..field_count {
                let field_name = self.read_string()?;
                if !seen.insert(field_name.clone()) {
                    return Err(persist_err(format!(
                        "composite {name}: duplicate field {field_name}"
                    )));
                }
                fields.push((field_name, self.read_u32()?, self.read_i32()?));
            }
            if composites
                .insert(
                    name.clone(),
                    CompositeDef {
                        name: name.clone(),
                        oid,
                        fields,
                    },
                )
                .is_some()
            {
                return Err(persist_err(format!("duplicate composite type {name}")));
            }
        }

        for (name, def) in &composites {
            for (field_name, oid, _) in &def.fields {
                validate_user_type_refs(
                    &format!("composite {name}.{field_name}"),
                    *oid,
                    &enum_types,
                    &composites,
                )?;
            }
        }

        Ok((enum_types, composites, max_oid.saturating_add(1)))
    }

    fn read_comments(
        &mut self,
        tables: &HashMap<String, Table>,
    ) -> Result<HashMap<(String, i32), String>, PgError> {
        let comment_count = self.read_u32()? as usize;
        let mut out = HashMap::new();
        for _ in 0..comment_count {
            let rel = self.read_string()?;
            let subid = self.read_i32()?;
            validate_comment_metadata(&rel, subid, tables)?;
            let text = self.read_string()?;
            if out.insert((rel.clone(), subid), text).is_some() {
                return Err(persist_err(format!("duplicate comment {rel}.{subid}")));
            }
        }
        Ok(out)
    }

    fn read_volatility(&mut self) -> Result<Volatility, PgError> {
        match self.read_exact(1)?[0] {
            0 => Ok(Volatility::Immutable),
            1 => Ok(Volatility::Stable),
            2 => Ok(Volatility::Volatile),
            other => Err(persist_err(format!(
                "unknown function volatility tag {other}"
            ))),
        }
    }

    fn read_lang(&mut self) -> Result<Lang, PgError> {
        match self.read_exact(1)?[0] {
            0 => Ok(Lang::Sql),
            1 => Ok(Lang::PlPgSql),
            other => Err(persist_err(format!(
                "unknown function language tag {other}"
            ))),
        }
    }

    fn read_ret_shape(&mut self) -> Result<RetShape, PgError> {
        match self.read_exact(1)?[0] {
            0 => Ok(RetShape::Scalar),
            1 => Ok(RetShape::SetofScalar {
                oid: self.read_u32()?,
            }),
            2 => {
                let count = self.read_u32()? as usize;
                let mut cols = Vec::with_capacity(count);
                for _ in 0..count {
                    cols.push((self.read_string()?, self.read_u32()?, self.read_i32()?));
                }
                Ok(RetShape::SetofTable(cols))
            }
            3 => Ok(RetShape::SetofRel),
            other => Err(persist_err(format!(
                "unknown function return-shape tag {other}"
            ))),
        }
    }

    fn read_functions(
        &mut self,
        source_body_format: bool,
    ) -> Result<HashMap<String, FunctionDef>, PgError> {
        let function_count = self.read_u32()? as usize;
        let mut out = HashMap::new();
        for _ in 0..function_count {
            let name = self.read_string()?;
            let arg_count = self.read_u32()? as usize;
            let mut args = Vec::with_capacity(arg_count);
            for _ in 0..arg_count {
                args.push((
                    self.read_string_option()?,
                    self.read_u32()?,
                    self.read_i32()?,
                ));
            }
            let ret_oid = self.read_u32()?;
            let ret_typmod = self.read_i32()?;
            let strict = self.read_bool_tag("function strict")?;
            let volatility = self.read_volatility()?;
            let def = if source_body_format {
                let language = self.read_lang()?;
                let returns = self.read_ret_shape()?;
                match self.read_exact(1)?[0] {
                    0 => FunctionDef {
                        name: name.clone(),
                        args,
                        ret_oid,
                        ret_typmod,
                        returns,
                        body: FuncBody::Expr(self.read_expr(usize::MAX, false)?),
                        strict,
                        volatility,
                        language,
                        pl_body: None,
                        source_body: None,
                    },
                    1 => {
                        let source = self.read_string()?;
                        let body = FuncBody::Query(Box::new(parse_persisted_function_query(
                            &name, &source,
                        )?));
                        FunctionDef {
                            name: name.clone(),
                            args,
                            ret_oid,
                            ret_typmod,
                            returns,
                            body,
                            strict,
                            volatility,
                            language,
                            pl_body: None,
                            source_body: Some(source),
                        }
                    }
                    2 => {
                        let source = self.read_string()?;
                        let pl_body = crate::stmt::plpgsql::parse_block(&source).map_err(|e| {
                            persist_err(format!(
                                "function {name}: could not parse persisted plpgsql body: {e:?}"
                            ))
                        })?;
                        FunctionDef {
                            name: name.clone(),
                            args,
                            ret_oid,
                            ret_typmod,
                            returns,
                            body: FuncBody::Expr(Expr::Null),
                            strict,
                            volatility,
                            language,
                            pl_body: Some(pl_body),
                            source_body: Some(source),
                        }
                    }
                    other => {
                        return Err(persist_err(format!("unknown function body tag {other}")));
                    }
                }
            } else {
                let body = FuncBody::Expr(self.read_expr(usize::MAX, false)?);
                FunctionDef {
                    name: name.clone(),
                    args,
                    ret_oid,
                    ret_typmod,
                    returns: RetShape::Scalar,
                    body,
                    strict,
                    volatility,
                    language: Lang::Sql,
                    pl_body: None,
                    source_body: None,
                }
            };
            validate_persisted_function(&def)?;
            if out.insert(name.clone(), def).is_some() {
                return Err(persist_err(format!("duplicate function {name}")));
            }
        }
        Ok(out)
    }

    fn read_trig_timing(&mut self) -> Result<TrigTiming, PgError> {
        match self.read_exact(1)?[0] {
            0 => Ok(TrigTiming::Before),
            1 => Ok(TrigTiming::After),
            other => Err(persist_err(format!("unknown trigger timing tag {other}"))),
        }
    }

    fn read_trig_event(&mut self) -> Result<TrigEvent, PgError> {
        match self.read_exact(1)?[0] {
            0 => Ok(TrigEvent::Insert),
            1 => Ok(TrigEvent::Update),
            2 => Ok(TrigEvent::Delete),
            other => Err(persist_err(format!("unknown trigger event tag {other}"))),
        }
    }

    fn read_triggers(
        &mut self,
        tables: &HashMap<String, Table>,
        functions: &HashMap<String, FunctionDef>,
    ) -> Result<Vec<TriggerDef>, PgError> {
        let trigger_count = self.read_u32()? as usize;
        let mut out = Vec::with_capacity(trigger_count);
        let mut seen = HashSet::new();
        for _ in 0..trigger_count {
            let name = self.read_string()?;
            let timing = self.read_trig_timing()?;
            let event_count = self.read_u32()? as usize;
            let mut events = Vec::with_capacity(event_count);
            for _ in 0..event_count {
                events.push(self.read_trig_event()?);
            }
            let table = self.read_string()?;
            let when = self.read_expr_option(usize::MAX, false)?;
            let func = self.read_string()?;
            if !seen.insert((table.clone(), name.clone())) {
                return Err(persist_err(format!("duplicate trigger {name} on {table}")));
            }
            let trigger = TriggerDef {
                name,
                timing,
                events,
                table,
                when,
                func,
            };
            validate_trigger_metadata(&trigger, tables, functions)?;
            out.push(trigger);
        }
        Ok(out)
    }

    fn read_function_refs(
        &mut self,
        functions: &HashMap<String, FunctionDef>,
    ) -> Result<(HashMap<String, String>, HashMap<String, AggregateDef>), PgError> {
        let operator_count = self.read_u32()? as usize;
        let mut operators = HashMap::new();
        for _ in 0..operator_count {
            let symbol = self.read_string()?;
            let func = self.read_string()?;
            validate_function_ref(&format!("operator {symbol}"), &func, functions)?;
            if operators.insert(symbol.clone(), func).is_some() {
                return Err(persist_err(format!("duplicate operator {symbol}")));
            }
        }

        let aggregate_count = self.read_u32()? as usize;
        let mut aggregates = HashMap::new();
        for _ in 0..aggregate_count {
            let name = self.read_string()?;
            let def = AggregateDef {
                name: name.clone(),
                sfunc: self.read_string()?,
                stype_oid: self.read_u32()?,
                initcond: self.read_string()?,
                finalfunc: self.read_string_option()?,
            };
            validate_aggregate_metadata(&def, functions)?;
            if aggregates.insert(name.clone(), def).is_some() {
                return Err(persist_err(format!("duplicate aggregate {name}")));
            }
        }
        Ok((operators, aggregates))
    }

    fn read_casts(
        &mut self,
        functions: &HashMap<String, FunctionDef>,
    ) -> Result<HashMap<(u32, u32), CastDef>, PgError> {
        let cast_count = self.read_u32()? as usize;
        let mut casts = HashMap::new();
        for _ in 0..cast_count {
            let source_oid = self.read_u32()?;
            let target_oid = self.read_u32()?;
            let func = self.read_string()?;
            validate_function_ref(
                &format!("cast {source_oid} -> {target_oid}"),
                &func,
                functions,
            )?;
            let def = CastDef {
                source_oid,
                target_oid,
                func,
            };
            if casts.insert((source_oid, target_oid), def).is_some() {
                return Err(persist_err(format!(
                    "duplicate cast {source_oid} -> {target_oid}"
                )));
            }
        }
        Ok(casts)
    }

    fn read_partitions(
        &mut self,
        tables: &HashMap<String, Table>,
    ) -> Result<HashMap<String, PartitionInfo>, PgError> {
        let partition_count = self.read_u32()? as usize;
        let mut out = HashMap::new();
        for _ in 0..partition_count {
            let parent = self.read_string()?;
            let key_col = self.read_u32()? as usize;
            let child_count = self.read_u32()? as usize;
            let mut children = Vec::with_capacity(child_count);
            for _ in 0..child_count {
                children.push(self.read_string()?);
            }
            let default_child = self.read_string_option()?;
            let info = PartitionInfo {
                key_col,
                children,
                default_child,
            };
            validate_partition_metadata(&parent, &info, tables)?;
            if out.insert(parent.clone(), info).is_some() {
                return Err(persist_err(format!("duplicate partition parent {parent}")));
            }
        }
        Ok(out)
    }

    fn read_sql_values(&mut self) -> Result<Vec<SqlValue>, PgError> {
        let len = self.read_u32()? as usize;
        let mut out = Vec::with_capacity(len);
        for _ in 0..len {
            out.push(self.read_sql_value()?);
        }
        Ok(out)
    }

    fn read_stats(
        &mut self,
        table_name: &str,
        width: usize,
    ) -> Result<(f64, std::collections::BTreeMap<usize, ColumnStats>), PgError> {
        let reltuples = self.read_f64()?;
        let stats_count = self.read_u32()? as usize;
        let mut stats = std::collections::BTreeMap::new();
        for _ in 0..stats_count {
            let idx = self.read_u32()? as usize;
            if idx >= width {
                return Err(persist_err(format!(
                    "table {table_name}: stats column out of range"
                )));
            }
            let null_frac = self.read_f64()?;
            let avg_width = self.read_i32()?;
            let n_distinct = self.read_f64()?;
            let most_common_vals = self.read_sql_values()?;
            let freq_count = self.read_u32()? as usize;
            let mut most_common_freqs = Vec::with_capacity(freq_count);
            for _ in 0..freq_count {
                most_common_freqs.push(self.read_f64()?);
            }
            let histogram_bounds = self.read_sql_values()?;
            let correlation = match self.read_exact(1)?[0] {
                0 => None,
                1 => Some(self.read_f64()?),
                other => {
                    return Err(persist_err(format!(
                        "table {table_name}: invalid stats correlation tag {other}"
                    )))
                }
            };
            let column_stats = ColumnStats {
                null_frac,
                avg_width,
                n_distinct,
                most_common_vals,
                most_common_freqs,
                histogram_bounds,
                correlation,
            };
            if stats.insert(idx, column_stats).is_some() {
                return Err(persist_err(format!(
                    "table {table_name}: duplicate stats column {idx}"
                )));
            }
        }
        Ok((reltuples, stats))
    }

    fn read_sql_value(&mut self) -> Result<SqlValue, PgError> {
        let tag = self.read_exact(1)?[0];
        match tag {
            0 => Ok(SqlValue::Null),
            1 => Ok(SqlValue::Int(self.read_i64()?)),
            2 => Ok(SqlValue::Real(self.read_f64()?)),
            3 => Ok(SqlValue::Text(self.read_string()?)),
            4 => Ok(SqlValue::Blob(self.read_bytes()?)),
            other => Err(persist_err(format!("unknown sql value tag {other}"))),
        }
    }
}

impl Catalog {
    pub fn new() -> Catalog {
        Catalog::default()
    }

    pub fn to_persisted_bytes(&self) -> Result<Vec<u8>, PgError> {
        self.ensure_minimal_persistence_supported()?;
        let features = self.persistence_feature_bits();
        let mut out = Vec::new();
        out.extend_from_slice(PERSIST_MAGIC);
        write_u32(&mut out, PERSIST_VERSION_CURRENT);
        write_u32(&mut out, features);

        let mut tables: Vec<(&String, &Table)> = self.tables.iter().collect();
        tables.sort_by(|(a, _), (b, _)| a.cmp(b));
        write_u32(&mut out, tables.len() as u32);
        for (name, table) in tables {
            write_string(&mut out, name);
            let cols = table.schema.names();
            write_u32(&mut out, cols.len() as u32);
            for col in &cols {
                write_string(&mut out, col);
            }
            write_u32(&mut out, table.col_types.len() as u32);
            for oid in &table.col_types {
                write_u32(&mut out, *oid);
            }
            write_u32(&mut out, table.col_typmods.len() as u32);
            for typmod in &table.col_typmods {
                write_i32(&mut out, *typmod);
            }
            if features & PERSIST_FEATURE_TABLE_NOT_NULL != 0 {
                let mut not_null = table.constraints.not_null.clone();
                not_null.resize(cols.len(), false);
                write_bool_vec(&mut out, &not_null);
            }
            if features & PERSIST_FEATURE_TABLE_UNIQUE_KEYS != 0 {
                write_unique_keys(&mut out, &table.constraints.uniques);
            }
            if features & PERSIST_FEATURE_TABLE_CHECKS != 0 {
                write_checks(&mut out, &table.constraints.checks)?;
            }
            if features & PERSIST_FEATURE_TABLE_DEFAULTS != 0 {
                write_expr_options(&mut out, &table.defaults, cols.len())?;
            }
            if features & PERSIST_FEATURE_TABLE_FOREIGN_KEYS != 0 {
                write_foreign_keys(&mut out, &table.constraints.foreign_keys);
            }
            if features & PERSIST_FEATURE_TABLE_IDENTITY != 0 {
                write_identity_kinds(&mut out, &table.identity, cols.len());
            }
            if features & PERSIST_FEATURE_TABLE_GENERATED != 0 {
                write_expr_options(&mut out, &table.generated, cols.len())?;
            }
            if features & PERSIST_FEATURE_DOMAINS != 0 {
                write_string_options(&mut out, &table.col_domains, cols.len());
            }
            if features & PERSIST_FEATURE_STATS != 0 {
                write_stats(&mut out, table);
            }
            write_u64(&mut out, table.next_rid);
            write_u32(&mut out, table.rows.len() as u32);
            for idx in 0..table.rows.len() {
                let h = table
                    .versions
                    .get(idx)
                    .copied()
                    .unwrap_or_else(TupleHeader::frozen);
                write_u64(&mut out, h.xmin);
                write_u64(&mut out, h.xmax);
                write_u64(&mut out, table.rids.get(idx).copied().unwrap_or(idx as u64));
                let row = &table.rows[idx];
                write_u32(&mut out, row.len() as u32);
                for value in row {
                    write_sql_value(&mut out, value);
                }
            }
        }
        if features & PERSIST_FEATURE_DOMAINS != 0 {
            write_domains(&mut out, &self.domains)?;
        }
        if features & PERSIST_FEATURE_INDEXES != 0 {
            write_indexes(&mut out, &self.indexes);
        }
        if features & PERSIST_FEATURE_SEQUENCES != 0 {
            write_sequences(&mut out, &self.sequences);
        }
        if features & PERSIST_FEATURE_VIEWS != 0 {
            write_views(&mut out, &self.views);
        }
        if features & PERSIST_FEATURE_USER_TYPES != 0 {
            write_user_types(&mut out, &self.enum_types, &self.composites);
        }
        if features & PERSIST_FEATURE_COMMENTS != 0 {
            write_comments(&mut out, &self.comments);
        }
        if features & PERSIST_FEATURE_FUNCTIONS != 0 {
            write_functions(
                &mut out,
                &self.functions,
                features & PERSIST_FEATURE_FUNCTION_SOURCE_BODIES != 0,
            )?;
        }
        if features & PERSIST_FEATURE_FUNCTION_REFS != 0 {
            write_function_refs(&mut out, &self.operators, &self.aggregates, &self.functions)?;
        }
        if features & PERSIST_FEATURE_CASTS != 0 {
            write_casts(&mut out, &self.casts, &self.functions)?;
        }
        if features & PERSIST_FEATURE_TRIGGERS != 0 {
            write_triggers(&mut out, &self.triggers, &self.tables, &self.functions)?;
        }
        if features & PERSIST_FEATURE_PARTITIONS != 0 {
            write_partitions(&mut out, &self.partitioned, &self.tables)?;
        }
        Ok(out)
    }

    fn persistence_feature_bits(&self) -> u32 {
        let mut features = PERSIST_FEATURES_NONE;
        if self
            .tables
            .values()
            .any(|table| table.constraints.not_null.iter().any(|&v| v))
        {
            features |= PERSIST_FEATURE_TABLE_NOT_NULL;
        }
        if self
            .tables
            .values()
            .any(|table| !table.constraints.uniques.is_empty())
        {
            features |= PERSIST_FEATURE_TABLE_UNIQUE_KEYS;
        }
        if self
            .tables
            .values()
            .any(|table| !table.constraints.checks.is_empty())
        {
            features |= PERSIST_FEATURE_TABLE_CHECKS;
        }
        if self
            .tables
            .values()
            .any(|table| table.defaults.iter().any(|v| v.is_some()))
        {
            features |= PERSIST_FEATURE_TABLE_DEFAULTS;
        }
        if self
            .tables
            .values()
            .any(|table| !table.constraints.foreign_keys.is_empty())
        {
            features |= PERSIST_FEATURE_TABLE_FOREIGN_KEYS;
        }
        if self
            .tables
            .values()
            .any(|table| table.identity.iter().any(|v| *v != IdentityKind::None))
        {
            features |= PERSIST_FEATURE_TABLE_IDENTITY;
        }
        if self
            .tables
            .values()
            .any(|table| table.generated.iter().any(|v| v.is_some()))
        {
            features |= PERSIST_FEATURE_TABLE_GENERATED;
        }
        if !self.domains.is_empty()
            || self
                .tables
                .values()
                .any(|table| table.col_domains.iter().any(|v| v.is_some()))
        {
            features |= PERSIST_FEATURE_DOMAINS;
        }
        if !self.indexes.is_empty() {
            features |= PERSIST_FEATURE_INDEXES;
        }
        if !self.sequences.is_empty() {
            features |= PERSIST_FEATURE_SEQUENCES;
        }
        if !self.views.is_empty() {
            features |= PERSIST_FEATURE_VIEWS;
        }
        if !self.enum_types.is_empty() || !self.composites.is_empty() {
            features |= PERSIST_FEATURE_USER_TYPES;
        }
        if !self.comments.is_empty() {
            features |= PERSIST_FEATURE_COMMENTS;
        }
        if !self.functions.is_empty() {
            features |= PERSIST_FEATURE_FUNCTIONS;
        }
        if self
            .functions
            .values()
            .any(function_needs_source_body_format)
        {
            features |= PERSIST_FEATURE_FUNCTION_SOURCE_BODIES;
        }
        if !self.operators.is_empty() || !self.aggregates.is_empty() {
            features |= PERSIST_FEATURE_FUNCTION_REFS;
        }
        if !self.casts.is_empty() {
            features |= PERSIST_FEATURE_CASTS;
        }
        if !self.triggers.is_empty() {
            features |= PERSIST_FEATURE_TRIGGERS;
        }
        if !self.partitioned.is_empty() {
            features |= PERSIST_FEATURE_PARTITIONS;
        }
        if self
            .tables
            .values()
            .any(|table| !table.stats.is_empty() || table.reltuples != -1.0)
        {
            features |= PERSIST_FEATURE_STATS;
        }
        features
    }

    fn ensure_minimal_persistence_supported(&self) -> Result<(), PgError> {
        for ((rel, subid), _) in &self.comments {
            validate_comment_metadata(rel, *subid, &self.tables)?;
        }
        for def in self.functions.values() {
            validate_persisted_function(def)?;
        }
        for (symbol, func) in &self.operators {
            validate_function_ref(&format!("operator {symbol}"), func, &self.functions)?;
        }
        for def in self.aggregates.values() {
            validate_aggregate_metadata(def, &self.functions)?;
        }
        for ((source_oid, target_oid), def) in &self.casts {
            if def.source_oid != *source_oid || def.target_oid != *target_oid {
                return Err(persist_err(format!(
                    "cast {} -> {}: key mismatch",
                    source_oid, target_oid
                )));
            }
            validate_function_ref(
                &format!("cast {} -> {}", source_oid, target_oid),
                &def.func,
                &self.functions,
            )?;
        }
        for trigger in &self.triggers {
            validate_trigger_metadata(trigger, &self.tables, &self.functions)?;
        }
        for (parent, info) in &self.partitioned {
            validate_partition_metadata(parent, info, &self.tables)?;
        }
        let mut user_type_oids = HashSet::new();
        for (name, def) in &self.enum_types {
            if name != &def.name {
                return Err(persist_err(format!("enum {name}: name mismatch")));
            }
            if def.oid < FIRST_ENUM_OID || !user_type_oids.insert(def.oid) {
                return Err(persist_err(format!("enum {name}: invalid oid")));
            }
            let mut labels = HashSet::new();
            for label in &def.labels {
                if !labels.insert(label) {
                    return Err(persist_err(format!("enum {name}: duplicate label {label}")));
                }
            }
        }
        for (name, def) in &self.composites {
            if name != &def.name {
                return Err(persist_err(format!("composite {name}: name mismatch")));
            }
            if def.oid < FIRST_ENUM_OID || !user_type_oids.insert(def.oid) {
                return Err(persist_err(format!("composite {name}: invalid oid")));
            }
            let mut fields = HashSet::new();
            for (field_name, field_oid, _) in &def.fields {
                if !fields.insert(field_name) {
                    return Err(persist_err(format!(
                        "composite {name}: duplicate field {field_name}"
                    )));
                }
                validate_user_type_refs(
                    &format!("composite {name}.{field_name}"),
                    *field_oid,
                    &self.enum_types,
                    &self.composites,
                )?;
            }
        }
        for (name, table) in &self.tables {
            validate_table_stats(name, table)?;
            for check in &table.constraints.checks {
                check_expr_supported(&check.expr, false)?;
            }
            if !table.defaults.is_empty() && table.defaults.len() != table.schema.names().len() {
                return Err(persist_err(format!(
                    "table {name}: defaults width mismatch"
                )));
            }
            if !table.identity.is_empty() && table.identity.len() != table.schema.names().len() {
                return Err(persist_err(format!(
                    "table {name}: identity width mismatch"
                )));
            }
            if !table.generated.is_empty() && table.generated.len() != table.schema.names().len() {
                return Err(persist_err(format!(
                    "table {name}: generated width mismatch"
                )));
            }
            if !table.col_domains.is_empty()
                && table.col_domains.len() != table.schema.names().len()
            {
                return Err(persist_err(format!(
                    "table {name}: domain binding width mismatch"
                )));
            }
            for default in table.defaults.iter().flatten() {
                check_expr_supported(default, true)?;
            }
            for generated in table.generated.iter().flatten() {
                check_expr_supported(generated, false)?;
            }
            for domain in table.col_domains.iter().flatten() {
                if !self.domains.contains_key(domain) {
                    return Err(persist_err(format!(
                        "table {name}: unknown domain {domain}"
                    )));
                }
            }
            for (idx, oid) in table.col_types.iter().enumerate() {
                validate_user_type_refs(
                    &format!("table {name}: column {idx}"),
                    *oid,
                    &self.enum_types,
                    &self.composites,
                )?;
            }
            if !table.constraints.not_null.is_empty()
                && table.constraints.not_null.len() != table.schema.names().len()
            {
                return Err(persist_err(format!(
                    "table {name}: not-null width mismatch"
                )));
            }
            for key in &table.constraints.uniques {
                if key.cols.is_empty() {
                    return Err(persist_err(format!("table {name}: empty unique key")));
                }
                for &idx in &key.cols {
                    if idx >= table.schema.names().len() {
                        return Err(persist_err(format!(
                            "table {name}: unique key column out of range"
                        )));
                    }
                }
            }
            for fk in &table.constraints.foreign_keys {
                validate_foreign_key_metadata(name, table, fk, &self.tables)?;
            }
        }
        for (name, view) in &self.views {
            if self.tables.contains_key(name) {
                return Err(persist_err(format!("view {name}: collides with table")));
            }
            if view.query_sql.trim().is_empty() {
                return Err(persist_err(format!("view {name}: empty query")));
            }
            parse_persisted_view_query(&view.query_sql)?;
            if !view.materialized
                && (!view.mat_columns.is_empty()
                    || !view.mat_col_types.is_empty()
                    || !view.mat_rows.is_empty())
            {
                return Err(persist_err(format!(
                    "view {name}: plain view carries materialized snapshot"
                )));
            }
            if view.materialized {
                if view.mat_columns.len() != view.mat_col_types.len() {
                    return Err(persist_err(format!(
                        "view {name}: materialized type width mismatch"
                    )));
                }
                for row in &view.mat_rows {
                    if row.len() != view.mat_columns.len() {
                        return Err(persist_err(format!(
                            "view {name}: materialized row width mismatch"
                        )));
                    }
                }
            }
        }
        for index in &self.indexes {
            let Some(table) = self.tables.get(&index.table) else {
                return Err(persist_err(format!(
                    "index {}: unknown table {}",
                    index.name, index.table
                )));
            };
            if index.cols.is_empty() {
                return Err(persist_err(format!(
                    "index {}: empty column list",
                    index.name
                )));
            }
            for &idx in &index.cols {
                if idx >= table.schema.names().len() {
                    return Err(persist_err(format!(
                        "index {}: column out of range",
                        index.name
                    )));
                }
            }
        }
        for (name, domain) in &self.domains {
            if name != &domain.name {
                return Err(persist_err(format!("domain {name}: name mismatch")));
            }
            if let Some(default) = &domain.default {
                check_expr_supported(default, false)?;
            }
            for check in &domain.checks {
                check_expr_supported(&check.expr, false)?;
            }
        }
        for (name, seq) in &self.sequences {
            if name != &seq.name {
                return Err(persist_err(format!("sequence {name}: name mismatch")));
            }
            if seq.increment == 0 {
                return Err(persist_err(format!("sequence {name}: zero increment")));
            }
            if seq.min > seq.max {
                return Err(persist_err(format!("sequence {name}: invalid bounds")));
            }
        }
        Ok(())
    }

    pub fn from_persisted_bytes(bytes: &[u8]) -> Result<Catalog, PgError> {
        let mut r = PersistReader::new(bytes);
        r.expect_magic(PERSIST_MAGIC)?;
        let version = r.read_u32()?;
        let features = match version {
            PERSIST_VERSION_V1_NO_FEATURES => PERSIST_FEATURES_NONE,
            PERSIST_VERSION_CURRENT => {
                let features = r.read_u32()?;
                if features & !PERSIST_FEATURES_KNOWN != 0 {
                    return Err(persist_err(format!(
                        "unsupported feature bits 0x{features:08x}"
                    )));
                }
                features
            }
            _ => return Err(persist_err(format!("unsupported version {version}"))),
        };

        let mut catalog = Catalog::new();
        let table_count = r.read_u32()? as usize;
        for _ in 0..table_count {
            let name = r.read_string()?;
            let col_count = r.read_u32()? as usize;
            let mut cols = Vec::with_capacity(col_count);
            for _ in 0..col_count {
                cols.push(r.read_string()?);
            }
            let type_count = r.read_u32()? as usize;
            let mut col_types = Vec::with_capacity(type_count);
            for _ in 0..type_count {
                col_types.push(r.read_u32()?);
            }
            let typmod_count = r.read_u32()? as usize;
            let mut col_typmods = Vec::with_capacity(typmod_count);
            for _ in 0..typmod_count {
                col_typmods.push(r.read_i32()?);
            }
            if !col_types.is_empty() && col_types.len() != col_count {
                return Err(persist_err(format!(
                    "table {name}: col_types width mismatch"
                )));
            }
            if !col_typmods.is_empty() && col_typmods.len() != col_types.len() {
                return Err(persist_err(format!("table {name}: typmod width mismatch")));
            }
            let not_null = if features & PERSIST_FEATURE_TABLE_NOT_NULL != 0 {
                r.read_bool_vec(col_count)?
            } else {
                Vec::new()
            };
            let uniques = if features & PERSIST_FEATURE_TABLE_UNIQUE_KEYS != 0 {
                r.read_unique_keys(col_count)?
            } else {
                Vec::new()
            };
            let checks = if features & PERSIST_FEATURE_TABLE_CHECKS != 0 {
                r.read_checks(col_count)?
            } else {
                Vec::new()
            };
            let defaults = if features & PERSIST_FEATURE_TABLE_DEFAULTS != 0 {
                r.read_expr_options(col_count)?
            } else {
                Vec::new()
            };
            let foreign_keys = if features & PERSIST_FEATURE_TABLE_FOREIGN_KEYS != 0 {
                r.read_foreign_keys(col_count)?
            } else {
                Vec::new()
            };
            let identity = if features & PERSIST_FEATURE_TABLE_IDENTITY != 0 {
                r.read_identity_kinds(col_count)?
            } else {
                Vec::new()
            };
            let generated = if features & PERSIST_FEATURE_TABLE_GENERATED != 0 {
                r.read_expr_options(col_count)?
            } else {
                Vec::new()
            };
            let col_domains = if features & PERSIST_FEATURE_DOMAINS != 0 {
                r.read_string_options(col_count)?
            } else {
                Vec::new()
            };
            let (reltuples, stats) = if features & PERSIST_FEATURE_STATS != 0 {
                r.read_stats(&name, col_count)?
            } else {
                (-1.0, std::collections::BTreeMap::new())
            };

            let next_rid = r.read_u64()?;
            let row_count = r.read_u32()? as usize;
            let mut rows = Vec::with_capacity(row_count);
            let mut versions = Vec::with_capacity(row_count);
            let mut rids = Vec::with_capacity(row_count);
            for _ in 0..row_count {
                versions.push(TupleHeader {
                    xmin: r.read_u64()?,
                    xmax: r.read_u64()?,
                });
                rids.push(r.read_u64()?);
                let width = r.read_u32()? as usize;
                if width != col_count {
                    return Err(persist_err(format!("table {name}: row width mismatch")));
                }
                let mut row = Vec::with_capacity(width);
                for _ in 0..width {
                    row.push(r.read_sql_value()?);
                }
                rows.push(row);
            }
            catalog.tables.insert(
                name,
                Table {
                    schema: Schema::new(cols),
                    col_types,
                    col_typmods,
                    rows,
                    versions,
                    rids,
                    next_rid,
                    constraints: TableConstraints {
                        not_null,
                        uniques,
                        checks,
                        foreign_keys,
                    },
                    defaults,
                    generated,
                    eq_indexes: std::cell::RefCell::new(HashMap::new()),
                    eq_indexes_multi: std::cell::RefCell::new(HashMap::new()),
                    col_domains,
                    identity,
                    stats,
                    reltuples,
                },
            );
        }
        for (name, table) in &catalog.tables {
            validate_table_stats(name, table)?;
        }
        for (name, table) in &catalog.tables {
            for fk in &table.constraints.foreign_keys {
                validate_foreign_key_metadata(name, table, fk, &catalog.tables)?;
            }
        }
        if features & PERSIST_FEATURE_DOMAINS != 0 {
            catalog.domains = r.read_domains()?;
            for (name, table) in &catalog.tables {
                for domain in table.col_domains.iter().flatten() {
                    if !catalog.domains.contains_key(domain) {
                        return Err(persist_err(format!(
                            "table {name}: unknown domain {domain}"
                        )));
                    }
                }
            }
        }
        if features & PERSIST_FEATURE_INDEXES != 0 {
            catalog.indexes = r.read_indexes(&catalog.tables)?;
            for index in catalog.indexes.clone() {
                if index.cols.len() == 1 {
                    if let Some(table) = catalog.tables.get_mut(&index.table) {
                        let ci = index.cols[0];
                        let eq =
                            sql_core::EqIndex::build(table.rows.iter().map(|row| row[ci].clone()));
                        table.eq_indexes.get_mut().insert(ci, eq);
                    }
                }
            }
        }
        if features & PERSIST_FEATURE_SEQUENCES != 0 {
            catalog.sequences = r.read_sequences()?;
        }
        if features & PERSIST_FEATURE_VIEWS != 0 {
            catalog.views = r.read_views(&catalog.tables)?;
        }
        if features & PERSIST_FEATURE_USER_TYPES != 0 {
            let (enum_types, composites, next_enum_oid) = r.read_user_types()?;
            catalog.enum_types = enum_types;
            catalog.composites = composites;
            catalog.next_enum_oid = next_enum_oid;
        }
        if features & PERSIST_FEATURE_COMMENTS != 0 {
            catalog.comments = r.read_comments(&catalog.tables)?;
        }
        if features & PERSIST_FEATURE_FUNCTIONS != 0 {
            catalog.functions =
                r.read_functions(features & PERSIST_FEATURE_FUNCTION_SOURCE_BODIES != 0)?;
        }
        if features & PERSIST_FEATURE_FUNCTION_REFS != 0 {
            let (operators, aggregates) = r.read_function_refs(&catalog.functions)?;
            catalog.operators = operators;
            catalog.aggregates = aggregates;
        }
        if features & PERSIST_FEATURE_CASTS != 0 {
            catalog.casts = r.read_casts(&catalog.functions)?;
        }
        if features & PERSIST_FEATURE_TRIGGERS != 0 {
            catalog.triggers = r.read_triggers(&catalog.tables, &catalog.functions)?;
        }
        if features & PERSIST_FEATURE_PARTITIONS != 0 {
            catalog.partitioned = r.read_partitions(&catalog.tables)?;
        }
        for (name, table) in &catalog.tables {
            for (idx, oid) in table.col_types.iter().enumerate() {
                validate_user_type_refs(
                    &format!("table {name}: column {idx}"),
                    *oid,
                    &catalog.enum_types,
                    &catalog.composites,
                )?;
            }
        }
        r.finish()?;
        Ok(catalog)
    }

    fn swap_session(&mut self, s: &mut Session) {
        std::mem::swap(&mut self.txn_stack, &mut s.txn_stack);
        std::mem::swap(&mut self.aborted, &mut s.aborted);
        std::mem::swap(&mut self.constraints_deferred, &mut s.constraints_deferred);
        std::mem::swap(&mut self.cur_xid, &mut s.cur_xid);
        std::mem::swap(&mut self.autocommit_stmt, &mut s.autocommit_stmt);
        std::mem::swap(&mut self.isolation, &mut s.isolation);
        std::mem::swap(&mut self.snapshot, &mut s.snapshot);
        std::mem::swap(&mut self.default_isolation, &mut s.default_isolation);
        std::mem::swap(&mut self.txn_query_seen, &mut s.txn_query_seen);
    }

    pub fn run_session(
        &mut self,
        session: &mut Session,
        sql: &str,
    ) -> Result<crate::stmt::lower::QueryResult, PgError> {
        self.swap_session(session);
        let r = crate::stmt::lower::run_mut(sql, self);
        self.swap_session(session);
        r
    }

    pub fn open_txn_count(&self) -> usize {
        self.open_txn_count
    }

    pub fn isolation(&self) -> IsolationLevel {
        self.isolation
    }

    pub fn set_txn_isolation(&mut self, level: IsolationLevel) {
        self.isolation = level;
    }

    pub fn set_transaction_isolation(&mut self, level: IsolationLevel) -> Result<(), PgError> {
        if !self.in_transaction() || self.txn_query_seen {
            return Err(PgError::SetTransactionTooLate);
        }
        self.isolation = level;
        Ok(())
    }

    pub fn set_session_isolation(&mut self, level: IsolationLevel) {
        self.default_isolation = level;
        if !self.in_transaction() {
            self.isolation = level;
        }
    }

    fn xid_committed(&self, xid: u64) -> bool {
        xid == FROZEN_XID || self.committed.contains(&xid)
    }

    fn snapshot_sees(&self, xid: u64) -> bool {
        match &self.snapshot {
            None => self.xid_committed(xid),
            Some(s) => {
                xid == FROZEN_XID
                    || (xid < s.ceiling
                        && self.committed.contains(&xid)
                        && !s.in_progress.contains(&xid))
            }
        }
    }

    fn capture_snapshot(&self) -> TxnSnapshot {
        TxnSnapshot {
            ceiling: self.next_xid,
            in_progress: self.open_xids.clone(),
        }
    }

    pub fn stmt_snapshot_hook(&mut self) {
        if self.in_transaction() {
            if self.isolation != IsolationLevel::ReadCommitted && self.snapshot.is_none() {
                self.snapshot = Some(self.capture_snapshot());

                if self.isolation == IsolationLevel::Serializable {
                    let x = self.current_xid_for_write();
                    self.ser_open.insert(x);
                    self.ser_reads.borrow_mut().entry(x).or_default();
                    self.ser_writes.entry(x).or_default();
                }
            }
            self.txn_query_seen = true;
        }
    }

    pub fn note_ser_read(&self, name: &str, pos: usize) {
        if self.isolation == IsolationLevel::Serializable && !self.txn_stack.is_empty() {
            if let Some(x) = self.cur_xid {
                self.ser_reads
                    .borrow_mut()
                    .entry(x)
                    .or_default()
                    .insert((name.to_string(), pos));
            }
        }
    }

    fn read_xid(&self) -> u64 {
        self.cur_xid.unwrap_or(INVALID_XID)
    }

    pub fn tuple_visible(&self, h: &TupleHeader, my_xid: u64) -> bool {
        if h.xmin != my_xid && !self.snapshot_sees(h.xmin) {
            return false;
        }
        if h.xmax == 0 {
            return true;
        }
        if h.xmax == my_xid {
            return false;
        }
        if self.snapshot_sees(h.xmax) {
            return false;
        }
        true
    }

    pub fn read_visibility_xid(&self) -> u64 {
        self.read_xid()
    }

    pub fn visible_rows(&self, name: &str) -> Option<Vec<Row>> {
        Some(
            self.visible_rows_with_pos(name)?
                .into_iter()
                .map(|(_, r)| r)
                .collect(),
        )
    }

    pub fn visible_rows_with_pos(&self, name: &str) -> Option<Vec<(usize, Row)>> {
        let t = self.tables.get(name)?;
        let my = self.read_xid();
        let mut out: Vec<(u64, usize)> = t
            .versions
            .iter()
            .enumerate()
            .filter(|(_, h)| self.tuple_visible(h, my))
            .map(|(pos, _)| (t.rids[pos], pos))
            .collect();
        out.sort_by_key(|&(rid, _)| rid);
        Some(
            out.into_iter()
                .map(|(_, pos)| (pos, t.rows[pos].clone()))
                .collect(),
        )
    }

    fn current_xid_for_write(&mut self) -> u64 {
        if let Some(x) = self.cur_xid {
            return x;
        }
        let x = self.next_xid;
        self.next_xid += 1;
        self.cur_xid = Some(x);

        if !self.txn_stack.is_empty() {
            self.open_xids.insert(x);
        }
        x
    }

    pub fn stmt_write_begin(&mut self) {
        self.autocommit_stmt = true;
        self.cur_xid = None;
    }

    pub fn stmt_write_end(&mut self, ok: bool) {
        if self.autocommit_stmt {
            if let Some(x) = self.cur_xid {
                if ok {
                    self.committed.insert(x);
                } else {
                    self.aborted_xids.insert(x);
                }
            }
        }
        self.autocommit_stmt = false;
        self.cur_xid = None;
        self.compact();
    }

    fn compact(&mut self) {
        if !self.txn_stack.is_empty() {
            return;
        }

        if self.open_txn_count > 0 {
            return;
        }
        let committed = &self.committed;
        for t in self.tables.values_mut() {

            let mut survivors: Vec<(u64, Row)> = Vec::with_capacity(t.rows.len());
            for ((r, h), rid) in t
                .rows
                .drain(..)
                .zip(t.versions.drain(..))
                .zip(t.rids.drain(..))
            {
                let xmin_committed = h.xmin == FROZEN_XID || committed.contains(&h.xmin);
                let xmax_committed =
                    h.xmax != 0 && (h.xmax == FROZEN_XID || committed.contains(&h.xmax));
                if xmin_committed && !xmax_committed {
                    survivors.push((rid, r));
                }
            }
            survivors.sort_by_key(|&(rid, _)| rid);
            let k = survivors.len();
            t.rows = survivors.into_iter().map(|(_, r)| r).collect();
            t.versions = vec![TupleHeader::frozen(); k];
            t.rids = (0..k as u64).collect();
            t.next_rid = k as u64;
            t.eq_indexes.get_mut().clear();
            t.eq_indexes_multi.get_mut().clear();
        }
    }

    pub fn mvcc_insert(&mut self, name: &str, new_rows: Vec<Row>) -> Option<usize> {
        let xid = self.current_xid_for_write();
        let t = self.tables.get_mut(name)?;
        let n = new_rows.len();
        for r in new_rows {
            t.rows.push(r);
            t.versions.push(TupleHeader::live(xid));

            t.rids.push(t.next_rid);
            t.next_rid += 1;
        }
        t.eq_indexes.get_mut().clear();
        t.eq_indexes_multi.get_mut().clear();
        Some(n)
    }

    fn precise_write_precheck(
        &self,
        name: &str,
        positions: &[usize],
        xid: u64,
    ) -> Result<Option<()>, PgError> {
        if !self.tables.contains_key(name) {
            return Ok(None);
        }
        let t = &self.tables[name];
        for &pos in positions {
            let h = &t.versions[pos];

            if h.xmax != 0 && h.xmax != xid {
                return Err(PgError::SerializationFailure);
            }
            if !self.row_locks.is_empty() {
                if let Some(li) = self.row_locks.get(&(name.to_string(), pos)) {
                    if li.conflicts_exclusive(xid) {
                        return Err(PgError::SerializationFailure);
                    }
                }
            }
        }
        Ok(Some(()))
    }

    fn record_ser_writes(&mut self, name: &str, positions: &[usize], xid: u64) {
        if self.isolation != IsolationLevel::Serializable || self.txn_stack.is_empty() {
            return;
        }
        if positions.is_empty() {
            return;
        }
        let set = self.ser_writes.entry(xid).or_default();
        for &pos in positions {
            set.insert((name.to_string(), pos));
        }
    }

    pub fn mvcc_update(
        &mut self,
        name: &str,
        changes: Vec<(usize, Row)>,
    ) -> Result<Option<()>, PgError> {
        let xid = self.current_xid_for_write();
        let positions: Vec<usize> = changes.iter().map(|(p, _)| *p).collect();
        if self
            .precise_write_precheck(name, &positions, xid)?
            .is_none()
        {
            return Ok(None);
        }
        let t = self.tables.get_mut(name).expect("table checked present");
        for (pos, new_row) in changes {
            t.versions[pos].xmax = xid;
            let rid = t.rids[pos];
            t.rows.push(new_row);
            t.versions.push(TupleHeader::live(xid));
            t.rids.push(rid);
        }
        t.eq_indexes.get_mut().clear();
        t.eq_indexes_multi.get_mut().clear();
        self.record_ser_writes(name, &positions, xid);
        Ok(Some(()))
    }

    pub fn mvcc_delete(
        &mut self,
        name: &str,
        positions: Vec<usize>,
    ) -> Result<Option<()>, PgError> {
        let xid = self.current_xid_for_write();
        if self
            .precise_write_precheck(name, &positions, xid)?
            .is_none()
        {
            return Ok(None);
        }
        let t = self.tables.get_mut(name).expect("table checked present");
        for &pos in &positions {
            t.versions[pos].xmax = xid;
        }
        t.eq_indexes.get_mut().clear();
        t.eq_indexes_multi.get_mut().clear();
        self.record_ser_writes(name, &positions, xid);
        Ok(Some(()))
    }

    pub fn lock_xid(&mut self) -> u64 {
        self.current_xid_for_write()
    }

    pub fn row_lock_conflict(&self, table: &str, pos: usize, xid: u64, mode: LockMode) -> bool {
        match self.row_locks.get(&(table.to_string(), pos)) {
            None => false,
            Some(li) => match mode {
                LockMode::Exclusive => li.conflicts_exclusive(xid),
                LockMode::Shared => li.conflicts_shared(xid),
            },
        }
    }

    pub fn acquire_row_lock(&mut self, table: &str, pos: usize, xid: u64, mode: LockMode) {
        let li = self.row_locks.entry((table.to_string(), pos)).or_default();
        match mode {
            LockMode::Exclusive => li.exclusive = Some(xid),
            LockMode::Shared => {
                li.shared.insert(xid);
            }
        }
    }

    fn release_locks_of(&mut self, xid: u64) {
        self.row_locks.retain(|_, li| {
            if li.exclusive == Some(xid) {
                li.exclusive = None;
            }
            li.shared.remove(&xid);
            !li.is_empty()
        });
    }

    pub fn set_lock_skip(&mut self, skip: Option<(String, HashSet<usize>)>) {
        self.lock_skip = skip;
    }

    pub fn lock_skip_table(&self, table: &str) -> bool {
        matches!(&self.lock_skip, Some((t, _)) if t == table)
    }

    pub fn scan_skips(&self, table: &str, pos: usize) -> bool {
        match &self.lock_skip {
            Some((t, set)) => t == table && set.contains(&pos),
            None => false,
        }
    }

    pub fn create<I, S>(&mut self, name: &str, columns: I, rows: Vec<Row>)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tables.insert(
            name.to_string(),
            Table {
                schema: Schema::new(columns),
                col_types: Vec::new(),
                col_typmods: Vec::new(),
                versions: vec![TupleHeader::frozen(); rows.len()],
                rids: (0..rows.len() as u64).collect(),
                next_rid: rows.len() as u64,
                rows,
                constraints: TableConstraints::default(),
                defaults: Vec::new(),
                generated: Vec::new(),
                eq_indexes: std::cell::RefCell::new(std::collections::HashMap::new()),
                eq_indexes_multi: std::cell::RefCell::new(std::collections::HashMap::new()),
                col_domains: Vec::new(),
                identity: Vec::new(),
                stats: std::collections::BTreeMap::new(),
                reltuples: -1.0,
            },
        );
    }

    pub fn create_typed(&mut self, name: &str, cols: Vec<(String, u32, i32)>, rows: Vec<Row>) {
        self.create_typed_with_constraints(name, cols, TableConstraints::default(), rows);
    }

    pub fn create_typed_with_constraints(
        &mut self,
        name: &str,
        cols: Vec<(String, u32, i32)>,
        constraints: TableConstraints,
        rows: Vec<Row>,
    ) {
        let names: Vec<String> = cols.iter().map(|(n, _, _)| n.clone()).collect();
        let col_types: Vec<u32> = cols.iter().map(|(_, t, _)| *t).collect();
        let col_typmods: Vec<i32> = cols.iter().map(|(_, _, m)| *m).collect();
        self.tables.insert(
            name.to_string(),
            Table {
                schema: Schema::new(names),
                col_types,
                col_typmods,
                versions: vec![TupleHeader::frozen(); rows.len()],
                rids: (0..rows.len() as u64).collect(),
                next_rid: rows.len() as u64,
                rows,
                constraints,
                defaults: Vec::new(),
                generated: Vec::new(),
                eq_indexes: std::cell::RefCell::new(std::collections::HashMap::new()),
                eq_indexes_multi: std::cell::RefCell::new(std::collections::HashMap::new()),
                col_domains: Vec::new(),
                identity: Vec::new(),
                stats: std::collections::BTreeMap::new(),
                reltuples: -1.0,
            },
        );
    }

    pub fn create_query_typed(&mut self, name: &str, cols: Vec<(String, u32)>, rows: Vec<Row>) {
        let triples = cols
            .into_iter()
            .map(|(n, t)| (n, t, crate::types::typmod::NONE))
            .collect();
        self.create_typed(name, triples, rows);
    }

    pub fn append_rows(&mut self, name: &str, mut new_rows: Vec<Row>) -> Option<usize> {
        let t = self.tables.get_mut(name)?;
        let n = new_rows.len();
        t.versions
            .extend(std::iter::repeat(TupleHeader::frozen()).take(new_rows.len()));
        for _ in 0..n {
            t.rids.push(t.next_rid);
            t.next_rid += 1;
        }
        t.rows.append(&mut new_rows);

        t.eq_indexes.get_mut().clear();
        t.eq_indexes_multi.get_mut().clear();
        Some(n)
    }

    pub fn set_rows(&mut self, name: &str, rows: Vec<Row>) -> Option<()> {
        let t = self.tables.get_mut(name)?;
        t.versions = vec![TupleHeader::frozen(); rows.len()];
        t.rids = (0..rows.len() as u64).collect();
        t.next_rid = rows.len() as u64;
        t.rows = rows;

        t.eq_indexes.get_mut().clear();
        t.eq_indexes_multi.get_mut().clear();
        Some(())
    }

    pub fn drop_table(&mut self, name: &str) -> bool {
        self.tables.remove(name).is_some()
    }

    pub fn create_enum(&mut self, name: &str, labels: Vec<String>) -> Result<u32, ()> {
        if self.enum_types.contains_key(name) || self.composites.contains_key(name) {
            return Err(());
        }
        let oid = self.next_enum_oid;
        self.next_enum_oid += 1;
        self.enum_types.insert(
            name.to_string(),
            EnumDef {
                name: name.to_string(),
                oid,
                labels,
            },
        );
        Ok(oid)
    }

    pub fn create_composite(
        &mut self,
        name: &str,
        fields: Vec<(String, u32, i32)>,
    ) -> Result<u32, ()> {
        if self.enum_types.contains_key(name) || self.composites.contains_key(name) {
            return Err(());
        }
        let oid = self.next_enum_oid;
        self.next_enum_oid += 1;
        self.composites.insert(
            name.to_string(),
            CompositeDef {
                name: name.to_string(),
                oid,
                fields,
            },
        );
        Ok(oid)
    }

    pub fn composite_by_name(&self, name: &str) -> Option<&CompositeDef> {
        self.composites.get(name)
    }

    pub fn composite_by_oid(&self, oid: u32) -> Option<&CompositeDef> {
        self.composites.values().find(|d| d.oid == oid)
    }

    pub fn enum_by_oid(&self, oid: u32) -> Option<&EnumDef> {
        self.enum_types.values().find(|d| d.oid == oid)
    }

    pub fn drop_enum(&mut self, name: &str) -> bool {
        self.enum_types.remove(name).is_some() || self.composites.remove(name).is_some()
    }

    pub fn enum_by_name(&self, name: &str) -> Option<&EnumDef> {
        self.enum_types.get(name)
    }

    pub fn enum_add_value(
        &mut self,
        name: &str,
        label: String,
        position: Option<(bool, String)>,
    ) -> Result<(), ()> {
        let def = self.enum_types.get_mut(name).ok_or(())?;
        if def.labels.iter().any(|l| l == &label) {
            return Err(());
        }
        match position {
            None => def.labels.push(label),
            Some((before, anchor)) => {
                let at = def.labels.iter().position(|l| l == &anchor).ok_or(())?;
                let idx = if before { at } else { at + 1 };
                def.labels.insert(idx, label);
            }
        }
        Ok(())
    }

    pub fn type_registries(&self) -> crate::types::registry::TypeRegistries {
        use crate::types::registry::TypeRegistries;
        let mut regs = TypeRegistries::default();
        for d in self.enum_types.values() {
            let info = crate::types::enums::EnumInfo {
                name: d.name.clone(),
                labels: d.labels.clone(),
            };
            regs.enums.insert(d.oid, info.clone());
            regs.enums_by_name.insert(d.name.clone(), info);
        }
        for d in self.composites.values() {
            let info = crate::types::composite::CompositeInfo {
                name: d.name.clone(),
                fields: d.fields.clone(),
            };
            regs.composites.insert(d.oid, info.clone());
            regs.composites_by_name.insert(d.name.clone(), info);
        }
        let schema = Schema::new(["value"]);
        for d in self.domains.values() {
            let checks = d
                .checks
                .iter()
                .filter_map(|c| {
                    resolve(&c.expr, &schema)
                        .ok()
                        .map(|bound| (c.name.clone().unwrap_or_default(), bound))
                })
                .collect();
            regs.domains.insert(
                d.name.clone(),
                crate::types::domains::DomainInfo {
                    base_oid: d.base_oid,
                    base_typmod: d.base_typmod,
                    not_null: d.not_null,
                    checks,
                },
            );
        }

        for f in self.functions.values() {
            regs.functions.insert(f.name.clone(), f.clone());
        }

        regs.operators = self.operators.clone();

        regs.aggregates = self.aggregates.clone();

        regs.casts = self.casts.clone();
        regs
    }

    pub fn get(&self, name: &str) -> Option<&Table> {
        self.tables.get(name)
    }

    pub fn get_table_mut(&mut self, name: &str) -> Option<&mut Table> {
        let t = self.tables.get_mut(name)?;

        t.eq_indexes.get_mut().clear();
        t.eq_indexes_multi.get_mut().clear();
        Some(t)
    }

    pub fn rename_table(&mut self, old: &str, new: &str) -> bool {
        if !self.tables.contains_key(old)
            || self.tables.contains_key(new)
            || self.views.contains_key(new)
        {
            return false;
        }
        if let Some(t) = self.tables.remove(old) {
            self.tables.insert(new.to_string(), t);
            true
        } else {
            false
        }
    }

    pub fn in_transaction(&self) -> bool {
        !self.txn_stack.is_empty()
    }

    pub fn in_autocommit_stmt(&self) -> bool {
        self.autocommit_stmt
    }

    pub fn peek_next_xid(&self) -> u64 {
        self.next_xid
    }

    pub fn is_xid_committed(&self, xid: u64) -> bool {
        self.committed.contains(&xid)
    }

    pub fn is_xid_aborted(&self, xid: u64) -> bool {
        self.aborted_xids.contains(&xid)
    }

    pub fn current_xid_if_assigned(&self) -> Option<u64> {
        self.cur_xid
    }

    pub fn assign_current_xid(&mut self) -> u64 {
        self.current_xid_for_write()
    }

    pub fn txid_status(&self, xid: u64) -> Option<&'static str> {
        if xid < FIRST_REAL_XID || xid >= self.next_xid {
            return None;
        }
        if self.committed.contains(&xid) {
            Some("committed")
        } else if self.aborted_xids.contains(&xid) {
            Some("aborted")
        } else {
            Some("in progress")
        }
    }

    pub fn txn_begin(&mut self) {
        if self.txn_stack.is_empty() {

            self.open_txn_count += 1;

            self.txn_stack.push(TxnFrame {
                name: None,
                snapshot: self.snapshot(),
            });
            self.constraints_deferred = None;

            self.cur_xid = None;
            self.autocommit_stmt = false;

            self.isolation = self.default_isolation;
            self.snapshot = None;
            self.txn_query_seen = false;
        }
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            tables: self.tables.clone(),
            views: self.views.clone(),
        }
    }

    fn restore(&mut self, snap: &Snapshot) {
        self.tables = snap.tables.clone();
        self.views = snap.views.clone();
    }

    fn structural_restore(&mut self, snap: &Snapshot) {
        self.views = snap.views.clone();
        let mut new_tables: HashMap<String, Table> = HashMap::new();
        for (nm, snap_t) in &snap.tables {
            match self.tables.get(nm) {
                Some(live) if live.schema.width() == snap_t.schema.width() => {

                    let mut t = snap_t.clone();
                    t.rows = live.rows.clone();
                    t.versions = live.versions.clone();

                    t.rids = live.rids.clone();
                    t.next_rid = live.next_rid;
                    t.eq_indexes = std::cell::RefCell::new(std::collections::HashMap::new());
                    t.eq_indexes_multi = std::cell::RefCell::new(std::collections::HashMap::new());
                    new_tables.insert(nm.clone(), t);
                }
                _ => {
                    new_tables.insert(nm.clone(), snap_t.clone());
                }
            }
        }
        self.tables = new_tables;
    }

    pub fn stmt_snapshot(&self) -> Snapshot {
        self.snapshot()
    }

    pub fn stmt_restore(&mut self, snap: &Snapshot) {
        self.restore(snap);
    }

    pub fn is_aborted(&self) -> bool {
        self.aborted
    }

    pub fn set_aborted(&mut self) {
        self.aborted = true;
    }

    pub fn constraints_deferred_mode(&self) -> Option<bool> {
        self.constraints_deferred
    }

    pub fn set_constraints_deferred(&mut self, deferred: bool) {
        self.constraints_deferred = Some(deferred);
    }

    pub fn ser_write_skew_conflict(&self) -> bool {
        let t = match self.cur_xid {
            Some(x) => x,
            None => return false,
        };
        if self.isolation != IsolationLevel::Serializable {
            return false;
        }
        let reads = self.ser_reads.borrow();
        let (t_reads, t_writes) = match (reads.get(&t), self.ser_writes.get(&t)) {
            (Some(r), Some(w)) if !r.is_empty() && !w.is_empty() => (r, w),
            _ => return false,
        };

        let mut candidates: HashSet<u64> =
            self.ser_open.iter().copied().filter(|&c| c != t).collect();
        for (c, waiters) in &self.ser_retain {
            if waiters.contains(&t) {
                candidates.insert(*c);
            }
        }
        for c in candidates {
            let c_reads = match reads.get(&c) {
                Some(r) => r,
                None => continue,
            };
            let c_writes = match self.ser_writes.get(&c) {
                Some(w) => w,
                None => continue,
            };
            let t_reads_c_wrote = t_reads.iter().any(|k| c_writes.contains(k));
            let c_reads_t_wrote = c_reads.iter().any(|k| t_writes.contains(k));
            if t_reads_c_wrote && c_reads_t_wrote {
                return true;
            }
        }
        false
    }

    fn ser_txn_end(&mut self, x: u64, committed: bool) {
        if !self.ser_open.remove(&x) {
            return;
        }

        let mut reclaim: Vec<u64> = Vec::new();
        self.ser_retain.retain(|c, waiters| {
            waiters.remove(&x);
            if waiters.is_empty() {
                reclaim.push(*c);
                false
            } else {
                true
            }
        });
        for c in reclaim {
            self.ser_reads.borrow_mut().remove(&c);
            self.ser_writes.remove(&c);
        }

        let still_open: HashSet<u64> = self.ser_open.iter().copied().collect();
        if committed && !still_open.is_empty() {

            self.ser_retain.insert(x, still_open);
        } else {
            self.ser_reads.borrow_mut().remove(&x);
            self.ser_writes.remove(&x);
        }
    }

    pub fn txn_commit(&mut self) {
        if let Some(x) = self.cur_xid {
            self.ser_txn_end(x, true);
        }

        if let Some(x) = self.cur_xid {
            self.committed.insert(x);
            self.open_xids.remove(&x);
            self.release_locks_of(x);
        }
        self.cur_xid = None;

        if !self.txn_stack.is_empty() {
            self.open_txn_count -= 1;
        }
        self.txn_stack.clear();
        self.aborted = false;
        self.constraints_deferred = None;
        self.snapshot = None;
        self.txn_query_seen = false;
        self.compact();
    }

    pub fn txn_rollback(&mut self) {
        if let Some(x) = self.cur_xid {
            self.ser_txn_end(x, false);
        }

        if let Some(x) = self.cur_xid {
            self.aborted_xids.insert(x);
            self.open_xids.remove(&x);
            self.release_locks_of(x);
        }
        self.cur_xid = None;

        if !self.txn_stack.is_empty() {
            self.open_txn_count -= 1;
        }

        if let Some(base) = self.txn_stack.first() {
            let snap = base.snapshot.clone();
            self.structural_restore(&snap);
        }
        self.txn_stack.clear();
        self.aborted = false;
        self.constraints_deferred = None;
        self.snapshot = None;
        self.txn_query_seen = false;
        self.compact();
    }

    pub fn txn_savepoint(&mut self, name: &str) {
        let snapshot = self.snapshot();
        self.txn_stack.push(TxnFrame {
            name: Some(name.to_string()),
            snapshot,
        });
    }

    pub fn txn_rollback_to(&mut self, name: &str) -> Result<(), PgError> {
        let idx = self
            .txn_stack
            .iter()
            .rposition(|f| f.name.as_deref() == Some(name))
            .ok_or_else(|| no_such_savepoint(name))?;
        let snap = self.txn_stack[idx].snapshot.clone();
        self.restore(&snap);
        self.txn_stack.truncate(idx + 1);

        self.aborted = false;
        Ok(())
    }

    pub fn txn_release(&mut self, name: &str) -> Result<(), PgError> {
        let idx = self
            .txn_stack
            .iter()
            .rposition(|f| f.name.as_deref() == Some(name))
            .ok_or_else(|| no_such_savepoint(name))?;
        self.txn_stack.truncate(idx);
        Ok(())
    }

    pub fn create_view(
        &mut self,
        name: &str,
        query: SelectStmt,
        query_sql: String,
        columns: Option<Vec<String>>,
        check_option: bool,
    ) {
        self.views.insert(
            name.to_string(),
            View {
                query,
                query_sql,
                columns,
                materialized: false,
                mat_columns: Vec::new(),
                mat_col_types: Vec::new(),
                mat_rows: Vec::new(),
                check_option,
            },
        );
    }

    pub fn create_matview(
        &mut self,
        name: &str,
        query: SelectStmt,
        query_sql: String,
        columns: Option<Vec<String>>,
        col_names: Vec<String>,
        col_types: Vec<u32>,
        rows: Vec<Row>,
    ) {
        self.views.insert(
            name.to_string(),
            View {
                query,
                query_sql,
                columns,
                materialized: true,
                mat_columns: col_names,
                mat_col_types: col_types,
                mat_rows: rows,
                check_option: false,
            },
        );
    }

    pub fn refresh_matview(
        &mut self,
        name: &str,
        col_names: Vec<String>,
        col_types: Vec<u32>,
        rows: Vec<Row>,
    ) -> Option<()> {
        let v = self.views.get_mut(name)?;
        if !v.materialized {
            return None;
        }
        v.mat_columns = col_names;
        v.mat_col_types = col_types;
        v.mat_rows = rows;
        Some(())
    }

    pub fn drop_view(&mut self, name: &str) -> bool {
        self.views.remove(name).is_some()
    }

    pub fn get_view(&self, name: &str) -> Option<&View> {
        self.views.get(name)
    }

    pub fn create_domain(&mut self, def: DomainDef) {
        self.domains.insert(def.name.clone(), def);
    }

    pub fn get_domain(&self, name: &str) -> Option<&DomainDef> {
        self.domains.get(name)
    }

    pub fn drop_domain(&mut self, name: &str) -> bool {
        self.domains.remove(name).is_some()
    }

    pub fn create_function(&mut self, def: FunctionDef) {
        self.functions.insert(def.name.clone(), def);
    }

    pub fn has_function(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }

    pub fn get_function(&self, name: &str) -> Option<&FunctionDef> {
        self.functions.get(name)
    }

    pub fn prepare_stmt(&mut self, name: &str, def: PreparedStmt) -> Result<(), PgError> {
        if self.prepared.contains_key(name) {
            return Err(PgError::InvalidInputSyntax {
                typ: "query",
                input: format!("prepared statement \"{name}\" already exists"),
            });
        }
        self.prepared.insert(name.to_string(), def);
        Ok(())
    }

    pub fn get_prepared(&self, name: &str) -> Option<PreparedStmt> {
        self.prepared.get(name).cloned()
    }

    pub fn deallocate(&mut self, name: &str) -> Result<(), PgError> {
        if self.prepared.remove(name).is_none() {
            return Err(PgError::InvalidInputSyntax {
                typ: "query",
                input: format!("prepared statement \"{name}\" does not exist"),
            });
        }
        Ok(())
    }

    pub fn deallocate_all(&mut self) {
        self.prepared.clear();
    }

    pub fn declare_cursor(&mut self, name: &str, cur: Cursor) -> Result<(), PgError> {
        if self.cursors.contains_key(name) {
            return Err(PgError::InvalidInputSyntax {
                typ: "query",
                input: format!("cursor \"{name}\" already exists"),
            });
        }
        self.cursors.insert(name.to_string(), cur);
        Ok(())
    }

    pub fn get_cursor_mut(&mut self, name: &str) -> Option<&mut Cursor> {
        self.cursors.get_mut(name)
    }

    pub fn close_cursor(&mut self, name: &str) -> Result<(), PgError> {
        if self.cursors.remove(name).is_none() {
            return Err(PgError::InvalidInputSyntax {
                typ: "query",
                input: format!("cursor \"{name}\" does not exist"),
            });
        }
        Ok(())
    }

    pub fn close_all_cursors(&mut self) {
        self.cursors.clear();
    }

    pub fn any_plpgsql_function(&self) -> bool {
        self.functions.values().any(|f| f.language == Lang::PlPgSql)
    }

    pub fn is_plpgsql_function(&self, name: &str) -> bool {
        self.functions
            .get(name)
            .map(|f| f.language == Lang::PlPgSql)
            .unwrap_or(false)
    }

    pub fn drop_function(&mut self, name: &str) -> bool {
        self.functions.remove(name).is_some()
    }

    pub fn create_trigger(&mut self, def: TriggerDef) {
        self.triggers.push(def);
    }

    pub fn has_trigger(&self, name: &str, table: &str) -> bool {
        self.triggers
            .iter()
            .any(|t| t.name == name && t.table == table)
    }

    pub fn drop_trigger(&mut self, name: &str, table: &str) -> bool {
        let before = self.triggers.len();
        self.triggers
            .retain(|t| !(t.name == name && t.table == table));
        self.triggers.len() != before
    }

    pub fn has_triggers_for(&self, table: &str) -> bool {
        self.triggers.iter().any(|t| t.table == table)
    }

    pub fn matching_triggers(
        &self,
        table: &str,
        timing: TrigTiming,
        event: TrigEvent,
    ) -> Vec<TriggerDef> {
        let mut v: Vec<TriggerDef> = self
            .triggers
            .iter()
            .filter(|t| t.table == table && t.timing == timing && t.events.contains(&event))
            .cloned()
            .collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }

    pub fn get_index(&self, name: &str) -> Option<&IndexDef> {
        self.indexes.iter().find(|i| i.name == name)
    }

    pub fn unique_indexes_for<'a>(&'a self, table: &str) -> impl Iterator<Item = &'a IndexDef> {
        let table = table.to_string();
        self.indexes
            .iter()
            .filter(move |i| i.unique && i.table == table)
    }

    pub fn add_index(&mut self, def: IndexDef) {
        if def.cols.len() == 1 {
            let ci = def.cols[0];
            if let Some(t) = self.tables.get_mut(&def.table) {
                if ci < t.schema.names().len() {
                    let eq = sql_core::EqIndex::build(t.rows.iter().map(|r| r[ci].clone()));
                    t.eq_indexes.get_mut().insert(ci, eq);
                }
            }
        }
        self.indexes.push(def);
    }

    pub fn drop_index(&mut self, name: &str) -> bool {
        if let Some(pos) = self.indexes.iter().position(|i| i.name == name) {
            let def = self.indexes.remove(pos);
            if def.cols.len() == 1 {
                if let Some(t) = self.tables.get_mut(&def.table) {
                    t.eq_indexes.get_mut().remove(&def.cols[0]);
                }
            }
            true
        } else {
            false
        }
    }

    pub fn tables_iter(&self) -> impl Iterator<Item = (&String, &Table)> {
        self.tables.iter()
    }

    pub fn views_iter(&self) -> impl Iterator<Item = (&String, &View)> {
        self.views.iter()
    }

    pub fn indexes_iter(&self) -> impl Iterator<Item = &IndexDef> {
        self.indexes.iter()
    }

    pub fn create_sequence(
        &mut self,
        def: SequenceDef,
        if_not_exists: bool,
    ) -> Result<(), PgError> {
        if self.sequences.contains_key(&def.name) {
            if if_not_exists {
                return Ok(());
            }
            return Err(PgError::InvalidInputSyntax {
                typ: "query",
                input: format!("relation \"{}\" already exists", def.name),
            });
        }
        self.sequences.insert(def.name.clone(), def);
        Ok(())
    }

    pub fn create_sequence_implicit(&mut self, def: SequenceDef) {
        self.sequences.insert(def.name.clone(), def);
    }

    pub fn get_sequence(&self, name: &str) -> Option<&SequenceDef> {
        self.sequences.get(name)
    }

    pub fn get_sequence_mut(&mut self, name: &str) -> Option<&mut SequenceDef> {
        self.sequences.get_mut(name)
    }

    pub fn sequences_iter(&self) -> impl Iterator<Item = (&String, &SequenceDef)> {
        self.sequences.iter()
    }

    pub fn set_comment(&mut self, rel: &str, subid: i32, text: Option<&str>) {
        match text {
            Some(t) => {
                self.comments
                    .insert((rel.to_string(), subid), t.to_string());
            }
            None => {
                self.comments.remove(&(rel.to_string(), subid));
            }
        }
    }

    pub fn get_comment(&self, rel: &str, subid: i32) -> Option<&String> {
        self.comments.get(&(rel.to_string(), subid))
    }

    pub fn create_operator(&mut self, symbol: &str, func: &str) -> Result<(), PgError> {
        if self.operators.contains_key(symbol) {
            return Err(PgError::InvalidInputSyntax {
                typ: "query",
                input: format!("operator already exists: {symbol}"),
            });
        }
        self.operators.insert(symbol.to_string(), func.to_string());
        Ok(())
    }

    pub fn mark_partitioned(&mut self, parent: &str, key_col: usize) {
        self.partitioned.insert(
            parent.to_string(),
            PartitionInfo {
                key_col,
                ..Default::default()
            },
        );
    }

    pub fn add_partition_child(&mut self, parent: &str, child: &str) -> Result<(), PgError> {
        match self.partitioned.get_mut(parent) {
            Some(p) => {
                p.children.push(child.to_string());
                Ok(())
            }
            None => Err(PgError::InvalidInputSyntax {
                typ: "query",
                input: format!("\"{parent}\" is not partitioned"),
            }),
        }
    }

    pub fn set_default_partition(&mut self, parent: &str, child: &str) -> Result<(), PgError> {
        let p = self
            .partitioned
            .get_mut(parent)
            .ok_or_else(|| PgError::InvalidInputSyntax {
                typ: "query",
                input: format!("\"{parent}\" is not partitioned"),
            })?;
        if p.default_child.is_some() {
            return Err(PgError::InvalidInputSyntax {
                typ: "query",
                input: format!("partition \"{parent}\" already has a default partition"),
            });
        }
        p.children.push(child.to_string());
        p.default_child = Some(child.to_string());
        Ok(())
    }

    pub fn partition_info(&self, name: &str) -> Option<&PartitionInfo> {
        self.partitioned.get(name)
    }

    pub fn create_cast(
        &mut self,
        source_oid: u32,
        target_oid: u32,
        func: &str,
    ) -> Result<(), PgError> {
        if self.casts.contains_key(&(source_oid, target_oid)) {
            return Err(PgError::InvalidInputSyntax {
                typ: "query",
                input: "cast already exists".to_string(),
            });
        }
        self.casts.insert(
            (source_oid, target_oid),
            CastDef {
                source_oid,
                target_oid,
                func: func.to_string(),
            },
        );
        Ok(())
    }

    pub fn create_aggregate(&mut self, def: AggregateDef) -> Result<(), PgError> {
        if self.aggregates.contains_key(&def.name) {
            return Err(PgError::InvalidInputSyntax {
                typ: "query",
                input: format!(
                    "function {} already exists with same argument types",
                    def.name
                ),
            });
        }
        self.aggregates.insert(def.name.clone(), def);
        Ok(())
    }

    pub fn drop_sequence(&mut self, name: &str) -> bool {
        self.sequences.remove(name).is_some()
    }

    fn no_such_sequence(name: &str) -> PgError {
        PgError::InvalidInputSyntax {
            typ: "query",
            input: format!("relation \"{name}\" does not exist"),
        }
    }

    pub fn seq_nextval(&mut self, name: &str) -> Result<i64, PgError> {
        let s = self
            .sequences
            .get_mut(name)
            .ok_or_else(|| Self::no_such_sequence(name))?;
        let next = match s.current {
            None => s.start,
            Some(cur) => {
                let stepped = cur.checked_add(s.increment);
                if s.increment >= 0 {
                    match stepped {
                        Some(n) if n <= s.max => n,
                        _ if s.cycle => s.min,
                        _ => {
                            return Err(PgError::SequenceReachedBound {
                                name: name.to_string(),
                                max: true,
                                bound: s.max,
                            })
                        }
                    }
                } else {
                    match stepped {
                        Some(n) if n >= s.min => n,
                        _ if s.cycle => s.max,
                        _ => {
                            return Err(PgError::SequenceReachedBound {
                                name: name.to_string(),
                                max: false,
                                bound: s.min,
                            })
                        }
                    }
                }
            }
        };
        s.current = Some(next);
        Ok(next)
    }

    pub fn seq_currval(&self, name: &str) -> Result<i64, PgError> {
        let s = self
            .sequences
            .get(name)
            .ok_or_else(|| Self::no_such_sequence(name))?;
        s.current.ok_or_else(|| PgError::CurrvalNotYetDefined {
            name: name.to_string(),
        })
    }

    pub fn seq_setval(&mut self, name: &str, n: i64, is_called: bool) -> Result<i64, PgError> {
        let s = self
            .sequences
            .get_mut(name)
            .ok_or_else(|| Self::no_such_sequence(name))?;
        if is_called {
            s.current = Some(n);
        } else {
            s.current = None;
            s.start = n;
        }
        Ok(n)
    }

    pub fn seq_restart(&mut self, name: &str, with: Option<i64>) -> Result<(), PgError> {
        let s = self
            .sequences
            .get_mut(name)
            .ok_or_else(|| Self::no_such_sequence(name))?;
        if let Some(n) = with {
            s.start = n;
        }
        s.current = None;
        Ok(())
    }
}

fn no_such_savepoint(name: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "query",
        input: format!("savepoint \"{name}\" does not exist"),
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    use crate::stmt::lower::run_mut;

    fn i(n: i64) -> SqlValue {
        SqlValue::Int(n)
    }

    fn t(s: &str) -> SqlValue {
        SqlValue::Text(s.into())
    }

    fn sample_catalog_bytes() -> Vec<u8> {
        let mut c = Catalog::new();
        run_mut("CREATE TABLE people (id int4, name text)", &mut c).expect("create");
        run_mut("INSERT INTO people VALUES (1, 'alice')", &mut c).expect("insert");
        c.to_persisted_bytes().expect("persist")
    }

    #[test]
    fn minimal_catalog_slice_round_trips_queries() {
        let mut c = Catalog::new();
        run_mut("CREATE TABLE people (id int4, name text)", &mut c).expect("create");
        run_mut("INSERT INTO people VALUES (2, 'bob'), (1, 'alice')", &mut c).expect("insert");

        let before = run_mut("SELECT id, name FROM people ORDER BY id", &mut c).expect("before");
        let bytes = c.to_persisted_bytes().expect("persist");
        assert!(bytes.starts_with(b"CRUFTPG\0"));
        assert_eq!(
            bytes,
            c.to_persisted_bytes().expect("persist again"),
            "persistence bytes are deterministic"
        );

        let mut reopened = Catalog::from_persisted_bytes(&bytes).expect("reopen");
        let after =
            run_mut("SELECT id, name FROM people ORDER BY id", &mut reopened).expect("after");
        assert_eq!(after, before);
        assert_eq!(
            after.rows,
            vec![vec![i(1), t("alice")], vec![i(2), t("bob")]]
        );
    }

    #[test]
    fn persisted_catalog_rejects_bad_magic_and_truncation() {
        assert!(Catalog::from_persisted_bytes(b"not a catalog").is_err());

        let mut c = Catalog::new();
        run_mut("CREATE TABLE people (id int4)", &mut c).expect("create");
        let mut bytes = c.to_persisted_bytes().expect("persist");
        bytes.pop();
        assert!(Catalog::from_persisted_bytes(&bytes).is_err());
    }

    #[test]
    fn persisted_catalog_round_trips_row_trigger_metadata() {
        let mut c = Catalog::new();
        run_mut("CREATE TABLE people (id int4)", &mut c).expect("create");
        run_mut(
            "CREATE FUNCTION force_id() RETURNS trigger AS $$ BEGIN NEW.id := NEW.id + 10; RETURN NEW; END $$ LANGUAGE plpgsql",
            &mut c,
        )
        .expect("create trigger function");
        run_mut(
            "CREATE TRIGGER people_bi BEFORE INSERT ON people FOR EACH ROW EXECUTE FUNCTION force_id()",
            &mut c,
        )
        .expect("create trigger");
        let bytes = c.to_persisted_bytes().expect("persist");
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap())
                & (PERSIST_FEATURE_FUNCTIONS
                    | PERSIST_FEATURE_FUNCTION_SOURCE_BODIES
                    | PERSIST_FEATURE_TRIGGERS),
            PERSIST_FEATURE_FUNCTIONS
                | PERSIST_FEATURE_FUNCTION_SOURCE_BODIES
                | PERSIST_FEATURE_TRIGGERS
        );

        let mut reopened = Catalog::from_persisted_bytes(&bytes).expect("reopen");
        run_mut("INSERT INTO people VALUES (1)", &mut reopened).expect("insert");
        let rows = run_mut("SELECT id FROM people ORDER BY id", &mut reopened).expect("select");
        assert_eq!(rows.rows, vec![vec![i(11)]]);
    }

    #[test]
    fn persisted_catalog_round_trips_comments() {
        let mut c = Catalog::new();
        run_mut("CREATE TABLE people (id int4, name text)", &mut c).expect("create");
        run_mut("COMMENT ON TABLE people IS 'people table'", &mut c).expect("table comment");
        run_mut("COMMENT ON COLUMN people.name IS 'display name'", &mut c).expect("column comment");
        let bytes = c.to_persisted_bytes().expect("persist");
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            PERSIST_FEATURE_COMMENTS
        );

        let mut reopened = Catalog::from_persisted_bytes(&bytes).expect("reopen");
        let comments = run_mut(
            "SELECT obj_description('people'::regclass), col_description('people'::regclass, 2)",
            &mut reopened,
        )
        .expect("comment select");
        assert_eq!(
            comments.rows,
            vec![vec![
                SqlValue::Text("people table".into()),
                SqlValue::Text("display name".into()),
            ]]
        );
        run_mut("COMMENT ON COLUMN people.name IS NULL", &mut reopened).expect("remove");
        let removed = run_mut(
            "SELECT col_description('people'::regclass, 2)",
            &mut reopened,
        )
        .expect("removed select");
        assert_eq!(removed.rows, vec![vec![SqlValue::Null]]);
    }

    #[test]
    fn persisted_catalog_round_trips_scalar_sql_functions() {
        let mut c = Catalog::new();
        run_mut(
            "CREATE FUNCTION add_one(x int4) RETURNS int4 AS $$ x + 1 $$ LANGUAGE sql IMMUTABLE STRICT",
            &mut c,
        )
        .expect("create function");
        let bytes = c.to_persisted_bytes().expect("persist");
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            PERSIST_FEATURE_FUNCTIONS
        );

        let mut reopened = Catalog::from_persisted_bytes(&bytes).expect("reopen");
        let rows =
            run_mut("SELECT add_one(41), add_one(NULL)", &mut reopened).expect("function select");
        assert_eq!(rows.rows, vec![vec![i(42), SqlValue::Null]]);
    }

    #[test]
    fn persisted_catalog_round_trips_query_body_sql_functions() {
        let mut c = Catalog::new();
        run_mut("CREATE TABLE nums (n int4)", &mut c).expect("create");
        run_mut("INSERT INTO nums VALUES (7)", &mut c).expect("insert");
        run_mut(
            "CREATE FUNCTION first_num() RETURNS int4 AS $$ SELECT n FROM nums $$ LANGUAGE sql",
            &mut c,
        )
        .expect("create query-body function");
        let bytes = c.to_persisted_bytes().expect("persist");
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            PERSIST_FEATURE_FUNCTIONS | PERSIST_FEATURE_FUNCTION_SOURCE_BODIES
        );

        let mut reopened = Catalog::from_persisted_bytes(&bytes).expect("reopen");
        let rows = run_mut("SELECT first_num()", &mut reopened).expect("function select");
        assert_eq!(rows.rows, vec![vec![i(7)]]);
    }

    #[test]
    fn persisted_catalog_round_trips_set_returning_sql_functions() {
        let mut c = Catalog::new();
        run_mut("CREATE TABLE nums (n int4)", &mut c).expect("create");
        run_mut("INSERT INTO nums VALUES (1), (2), (3)", &mut c).expect("insert");
        run_mut(
            "CREATE FUNCTION all_nums() RETURNS SETOF int4 AS $$ SELECT n FROM nums ORDER BY n $$ LANGUAGE sql STABLE",
            &mut c,
        )
        .expect("create set-returning function");
        let bytes = c.to_persisted_bytes().expect("persist");
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            PERSIST_FEATURE_FUNCTIONS | PERSIST_FEATURE_FUNCTION_SOURCE_BODIES
        );

        let mut reopened = Catalog::from_persisted_bytes(&bytes).expect("reopen");
        let rows = run_mut("SELECT all_nums FROM all_nums()", &mut reopened)
            .expect("set-returning function select");
        assert_eq!(rows.rows, vec![vec![i(1)], vec![i(2)], vec![i(3)]]);
    }

    #[test]
    fn persisted_catalog_round_trips_plpgsql_function_bodies() {
        let mut c = Catalog::new();
        run_mut(
            "CREATE FUNCTION plus_ten(x int4) RETURNS int4 AS $$ BEGIN RETURN x + 10; END $$ LANGUAGE plpgsql",
            &mut c,
        )
        .expect("create plpgsql function");
        let bytes = c.to_persisted_bytes().expect("persist");
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            PERSIST_FEATURE_FUNCTIONS | PERSIST_FEATURE_FUNCTION_SOURCE_BODIES
        );

        let mut reopened = Catalog::from_persisted_bytes(&bytes).expect("reopen");
        let rows = run_mut("SELECT plus_ten(5)", &mut reopened).expect("function select");
        assert_eq!(rows.rows, vec![vec![i(15)]]);
    }

    #[test]
    fn persisted_catalog_round_trips_function_ref_namespaces() {
        let mut c = Catalog::new();
        run_mut("CREATE TABLE nums (n int4)", &mut c).expect("create");
        run_mut("INSERT INTO nums VALUES (1), (2), (3)", &mut c).expect("insert");
        run_mut(
            "CREATE FUNCTION add2(a int4, b int4) RETURNS int4 AS $$ a + b $$ LANGUAGE sql IMMUTABLE STRICT",
            &mut c,
        )
        .expect("create add2");
        run_mut(
            "CREATE FUNCTION inc1(a int4) RETURNS int4 AS $$ a + 1 $$ LANGUAGE sql IMMUTABLE STRICT",
            &mut c,
        )
        .expect("create inc1");
        run_mut("CREATE OPERATOR ## (FUNCTION = add2)", &mut c).expect("create operator");
        run_mut(
            "CREATE AGGREGATE mysum (int4) (SFUNC = add2, STYPE = int4, INITCOND = '0', FINALFUNC = inc1)",
            &mut c,
        )
        .expect("create aggregate");
        let bytes = c.to_persisted_bytes().expect("persist");
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            PERSIST_FEATURE_FUNCTIONS | PERSIST_FEATURE_FUNCTION_REFS
        );

        let mut reopened = Catalog::from_persisted_bytes(&bytes).expect("reopen");
        let rows = run_mut("SELECT 2 ## 3, mysum(n) FROM nums", &mut reopened)
            .expect("operator aggregate select");
        assert_eq!(rows.rows, vec![vec![i(5), i(7)]]);
    }

    #[test]
    fn persisted_catalog_round_trips_cast_metadata() {
        let mut c = Catalog::new();
        run_mut(
            "CREATE FUNCTION bigint_to_bool(a bigint) RETURNS boolean AS $$ a = 1 $$ LANGUAGE sql IMMUTABLE STRICT",
            &mut c,
        )
        .expect("create cast function");
        run_mut(
            "CREATE CAST (bigint AS boolean) WITH FUNCTION bigint_to_bool",
            &mut c,
        )
        .expect("create cast");
        let bytes = c.to_persisted_bytes().expect("persist");
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            PERSIST_FEATURE_FUNCTIONS | PERSIST_FEATURE_CASTS
        );

        let mut reopened = Catalog::from_persisted_bytes(&bytes).expect("reopen");
        let rows = run_mut("SELECT 1::boolean, 2::boolean", &mut reopened).expect("cast select");
        assert_eq!(rows.rows, vec![vec![i(1), i(0)]]);
    }

    #[test]
    fn persisted_catalog_drops_prepared_and_cursor_session_state() {
        let mut c = Catalog::new();
        run_mut("CREATE TABLE t (id int4)", &mut c).expect("create table");
        run_mut("PREPARE p(int4) AS INSERT INTO t VALUES ($1)", &mut c).expect("prepare");
        run_mut("EXECUTE p(1)", &mut c).expect("execute prepared");
        run_mut(
            "DECLARE cur CURSOR FOR SELECT id FROM t ORDER BY id",
            &mut c,
        )
        .expect("declare cursor");
        let fetched = run_mut("FETCH NEXT cur", &mut c).expect("fetch cursor");
        assert_eq!(fetched.rows, vec![vec![i(1)]]);

        let bytes = c.to_persisted_bytes().expect("persist");
        let mut reopened = Catalog::from_persisted_bytes(&bytes).expect("reopen");
        let rows = run_mut("SELECT id FROM t ORDER BY id", &mut reopened).expect("select");
        assert_eq!(rows.rows, vec![vec![i(1)]]);
        let prepared_err = run_mut("EXECUTE p(2)", &mut reopened).expect_err("prepared is session");
        assert!(
            prepared_err
                .message()
                .contains("prepared statement \"p\" does not exist"),
            "{}",
            prepared_err.message()
        );
        let cursor_err = run_mut("FETCH NEXT cur", &mut reopened).expect_err("cursor is session");
        assert!(
            cursor_err
                .message()
                .contains("cursor \"cur\" does not exist"),
            "{}",
            cursor_err.message()
        );
    }

    #[test]
    fn persisted_catalog_round_trips_partition_metadata() {
        let mut c = Catalog::new();
        run_mut(
            "CREATE TABLE measurement (id int4, bucket int4) PARTITION BY RANGE (bucket)",
            &mut c,
        )
        .expect("create parent");
        run_mut(
            "CREATE TABLE measurement_low PARTITION OF measurement FOR VALUES FROM (0) TO (10)",
            &mut c,
        )
        .expect("create low");
        run_mut(
            "CREATE TABLE measurement_default PARTITION OF measurement DEFAULT",
            &mut c,
        )
        .expect("create default");
        run_mut("INSERT INTO measurement VALUES (1, 3), (2, 30)", &mut c).expect("insert");
        let before =
            run_mut("SELECT id, bucket FROM measurement ORDER BY id", &mut c).expect("before");
        let bytes = c.to_persisted_bytes().expect("persist");
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            PERSIST_FEATURE_TABLE_CHECKS | PERSIST_FEATURE_PARTITIONS
        );

        let mut reopened = Catalog::from_persisted_bytes(&bytes).expect("reopen");
        let after = run_mut(
            "SELECT id, bucket FROM measurement ORDER BY id",
            &mut reopened,
        )
        .expect("after");
        assert_eq!(after, before);
        run_mut(
            "INSERT INTO measurement VALUES (3, 8), (4, 80)",
            &mut reopened,
        )
        .expect("insert after reopen");
        let rows = run_mut(
            "SELECT id, bucket FROM measurement ORDER BY id",
            &mut reopened,
        )
        .expect("select after insert");
        assert_eq!(
            rows.rows,
            vec![
                vec![i(1), i(3)],
                vec![i(2), i(30)],
                vec![i(3), i(8)],
                vec![i(4), i(80)],
            ]
        );
    }

    #[test]
    fn persisted_catalog_round_trips_analyze_stats() {
        let mut c = Catalog::new();
        run_mut("CREATE TABLE nums (id int4, label text)", &mut c).expect("create");
        run_mut(
            "INSERT INTO nums VALUES (1, 'a'), (2, 'a'), (3, 'b'), (4, NULL)",
            &mut c,
        )
        .expect("insert");
        run_mut("ANALYZE nums", &mut c).expect("analyze");
        let before_stats = run_mut(
            "SELECT attname, null_frac, avg_width FROM pg_catalog.pg_stats WHERE tablename = 'nums' ORDER BY attname",
            &mut c,
        )
        .expect("before stats");
        let before_class = run_mut(
            "SELECT reltuples FROM pg_catalog.pg_class WHERE relname = 'nums'",
            &mut c,
        )
        .expect("before class");
        let bytes = c.to_persisted_bytes().expect("persist");
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            PERSIST_FEATURE_STATS
        );

        let mut reopened = Catalog::from_persisted_bytes(&bytes).expect("reopen");
        let after_stats = run_mut(
            "SELECT attname, null_frac, avg_width FROM pg_catalog.pg_stats WHERE tablename = 'nums' ORDER BY attname",
            &mut reopened,
        )
        .expect("after stats");
        let after_class = run_mut(
            "SELECT reltuples FROM pg_catalog.pg_class WHERE relname = 'nums'",
            &mut reopened,
        )
        .expect("after class");
        assert_eq!(after_stats, before_stats);
        assert_eq!(after_class, before_class);
    }

    #[test]
    fn persisted_catalog_round_trips_user_types() {
        let mut c = Catalog::new();
        run_mut("CREATE TYPE mood AS ENUM ('sad', 'ok', 'happy')", &mut c).expect("enum");
        run_mut("CREATE TYPE profile AS (name text, age int4)", &mut c).expect("composite");
        run_mut(
            "CREATE TABLE people (id int4, current_mood mood, attrs profile)",
            &mut c,
        )
        .expect("table");
        run_mut("INSERT INTO people VALUES (1, 'ok', '(ada,41)')", &mut c).expect("insert");
        let bytes = c.to_persisted_bytes().expect("persist");
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            PERSIST_FEATURE_USER_TYPES
        );

        let mut reopened = Catalog::from_persisted_bytes(&bytes).expect("reopen");
        let rows = run_mut(
            "SELECT id, current_mood, attrs FROM people ORDER BY id",
            &mut reopened,
        )
        .expect("select");
        assert_eq!(
            rows.rows,
            vec![vec![
                SqlValue::Int(1),
                SqlValue::Text("ok".into()),
                SqlValue::Text("(ada,41)".into()),
            ]]
        );
        assert!(run_mut(
            "INSERT INTO people VALUES (2, 'angry', '(ok,12)')",
            &mut reopened,
        )
        .is_err());
        run_mut("CREATE TABLE later (attrs profile)", &mut reopened).expect("composite after");
        run_mut("INSERT INTO later VALUES ('(bob,7)')", &mut reopened).expect("insert later");
    }

    #[test]
    fn persisted_catalog_round_trips_views() {
        let mut c = Catalog::new();
        run_mut("CREATE TABLE people (id int4, name text, age int4)", &mut c).expect("create");
        run_mut(
            "INSERT INTO people VALUES (1, 'ada', 41), (2, 'bob', 17)",
            &mut c,
        )
        .expect("insert");
        run_mut(
            "CREATE VIEW adults AS SELECT id, name FROM people WHERE age >= 18",
            &mut c,
        )
        .expect("create view");
        run_mut(
            "CREATE MATERIALIZED VIEW adult_snapshot AS SELECT id, name FROM people WHERE age >= 18",
            &mut c,
        )
        .expect("create matview");
        let bytes = c.to_persisted_bytes().expect("persist");
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            PERSIST_FEATURE_VIEWS
        );

        let mut reopened = Catalog::from_persisted_bytes(&bytes).expect("reopen");
        run_mut("INSERT INTO people VALUES (3, 'cy', 32)", &mut reopened).expect("insert after");
        let live =
            run_mut("SELECT id, name FROM adults ORDER BY id", &mut reopened).expect("select view");
        assert_eq!(
            live.rows,
            vec![
                vec![SqlValue::Int(1), SqlValue::Text("ada".into())],
                vec![SqlValue::Int(3), SqlValue::Text("cy".into())],
            ]
        );

        let stale = run_mut(
            "SELECT id, name FROM adult_snapshot ORDER BY id",
            &mut reopened,
        )
        .expect("select matview");
        assert_eq!(
            stale.rows,
            vec![vec![SqlValue::Int(1), SqlValue::Text("ada".into())]]
        );
        run_mut("REFRESH MATERIALIZED VIEW adult_snapshot", &mut reopened).expect("refresh");
        let refreshed = run_mut(
            "SELECT id, name FROM adult_snapshot ORDER BY id",
            &mut reopened,
        )
        .expect("select refreshed");
        assert_eq!(
            refreshed.rows,
            vec![
                vec![SqlValue::Int(1), SqlValue::Text("ada".into())],
                vec![SqlValue::Int(3), SqlValue::Text("cy".into())],
            ]
        );
    }

    #[test]
    fn persisted_catalog_round_trips_domains() {
        let mut c = Catalog::new();
        run_mut(
            "CREATE DOMAIN positive_int AS int4 DEFAULT 5 NOT NULL CHECK (VALUE > 0)",
            &mut c,
        )
        .expect("create domain");
        run_mut("CREATE TABLE scores (id int4, score positive_int)", &mut c).expect("create table");
        run_mut("INSERT INTO scores (id, score) VALUES (1, 7)", &mut c).expect("insert");
        let bytes = c.to_persisted_bytes().expect("persist");
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            PERSIST_FEATURE_DOMAINS
        );

        let mut reopened = Catalog::from_persisted_bytes(&bytes).expect("reopen");
        run_mut("INSERT INTO scores (id) VALUES (2)", &mut reopened)
            .expect("domain default after reopen");
        let bad_check = run_mut(
            "INSERT INTO scores (id, score) VALUES (3, -1)",
            &mut reopened,
        )
        .expect_err("domain check after reopen");
        assert!(
            bad_check.message().contains("violates check constraint"),
            "got: {}",
            bad_check.message()
        );
        let bad_null = run_mut(
            "INSERT INTO scores (id, score) VALUES (4, NULL)",
            &mut reopened,
        )
        .expect_err("domain not-null after reopen");
        assert!(
            bad_null.message().contains("does not allow null values"),
            "got: {}",
            bad_null.message()
        );
        let bad_update = run_mut("UPDATE scores SET score = -2 WHERE id = 1", &mut reopened)
            .expect_err("domain check on update after reopen");
        assert!(
            bad_update.message().contains("violates check constraint"),
            "got: {}",
            bad_update.message()
        );
        run_mut(
            "CREATE TABLE more_scores (score positive_int)",
            &mut reopened,
        )
        .expect("domain type available after reopen");
        run_mut("INSERT INTO more_scores DEFAULT VALUES", &mut reopened)
            .expect("domain default in new table");
        let scores =
            run_mut("SELECT id, score FROM scores ORDER BY id", &mut reopened).expect("select");
        assert_eq!(scores.rows, vec![vec![i(1), i(7)], vec![i(2), i(5)]]);
        let more = run_mut("SELECT score FROM more_scores", &mut reopened).expect("select more");
        assert_eq!(more.rows, vec![vec![i(5)]]);
    }

    #[test]
    fn persisted_catalog_round_trips_generated_columns() {
        let mut c = Catalog::new();
        run_mut(
            "CREATE TABLE people (id int4 PRIMARY KEY, base int4, doubled int4 GENERATED ALWAYS AS (base * 2) STORED)",
            &mut c,
        )
        .expect("create");
        run_mut(
            "INSERT INTO people (id, base) VALUES (1, 4), (2, 7)",
            &mut c,
        )
        .expect("insert");
        let bytes = c.to_persisted_bytes().expect("persist");
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            PERSIST_FEATURE_TABLE_NOT_NULL
                | PERSIST_FEATURE_TABLE_UNIQUE_KEYS
                | PERSIST_FEATURE_TABLE_GENERATED
        );

        let mut reopened = Catalog::from_persisted_bytes(&bytes).expect("reopen");
        let rows = run_mut(
            "SELECT id, base, doubled FROM people ORDER BY id",
            &mut reopened,
        )
        .expect("select");
        assert_eq!(
            rows.rows,
            vec![vec![i(1), i(4), i(8)], vec![i(2), i(7), i(14)]]
        );
        let err = run_mut(
            "INSERT INTO people (id, base, doubled) VALUES (3, 9, 18)",
            &mut reopened,
        )
        .expect_err("generated column rejects explicit insert");
        assert!(
            err.message()
                .contains("cannot insert a non-DEFAULT value into column"),
            "got: {}",
            err.message()
        );
        run_mut("UPDATE people SET base = 5 WHERE id = 1", &mut reopened)
            .expect("update recomputes generated");
        run_mut("INSERT INTO people (id, base) VALUES (3, 9)", &mut reopened)
            .expect("insert generated");
        let rows = run_mut(
            "SELECT id, base, doubled FROM people ORDER BY id",
            &mut reopened,
        )
        .expect("select after writes");
        assert_eq!(
            rows.rows,
            vec![
                vec![i(1), i(5), i(10)],
                vec![i(2), i(7), i(14)],
                vec![i(3), i(9), i(18)]
            ]
        );
    }

    #[test]
    fn persisted_catalog_round_trips_identity_metadata() {
        let mut c = Catalog::new();
        run_mut(
            "CREATE TABLE always_people (id int4 GENERATED ALWAYS AS IDENTITY, name text)",
            &mut c,
        )
        .expect("create always identity");
        run_mut(
            "CREATE TABLE default_people (id int4 GENERATED BY DEFAULT AS IDENTITY, name text)",
            &mut c,
        )
        .expect("create default identity");
        run_mut("INSERT INTO always_people (name) VALUES ('ada')", &mut c).expect("insert always");
        run_mut(
            "INSERT INTO default_people (id, name) VALUES (10, 'manual')",
            &mut c,
        )
        .expect("insert default explicit");
        let bytes = c.to_persisted_bytes().expect("persist");
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            PERSIST_FEATURE_TABLE_NOT_NULL
                | PERSIST_FEATURE_SEQUENCES
                | PERSIST_FEATURE_TABLE_DEFAULTS
                | PERSIST_FEATURE_TABLE_IDENTITY
        );

        let mut reopened = Catalog::from_persisted_bytes(&bytes).expect("reopen");
        let err = run_mut(
            "INSERT INTO always_people (id, name) VALUES (99, 'bad')",
            &mut reopened,
        )
        .expect_err("always identity rejects explicit value after reopen");
        assert!(
            err.message()
                .contains("cannot insert a non-DEFAULT value into column"),
            "got: {}",
            err.message()
        );
        run_mut(
            "INSERT INTO always_people (id, name) OVERRIDING SYSTEM VALUE VALUES (99, 'system')",
            &mut reopened,
        )
        .expect("overriding system");
        run_mut(
            "INSERT INTO always_people (name) VALUES ('next')",
            &mut reopened,
        )
        .expect("always default after reopen");
        run_mut(
            "INSERT INTO default_people (id, name) VALUES (11, 'manual2')",
            &mut reopened,
        )
        .expect("by default explicit after reopen");
        run_mut(
            "INSERT INTO default_people (id, name) OVERRIDING USER VALUE VALUES (50, 'forced')",
            &mut reopened,
        )
        .expect("by default overriding user");
        let always = run_mut(
            "SELECT id, name FROM always_people ORDER BY id",
            &mut reopened,
        )
        .expect("select always");
        assert_eq!(
            always.rows,
            vec![
                vec![i(1), t("ada")],
                vec![i(2), t("next")],
                vec![i(99), t("system")]
            ]
        );
        let defaults = run_mut(
            "SELECT id, name FROM default_people ORDER BY id",
            &mut reopened,
        )
        .expect("select default");
        assert_eq!(
            defaults.rows,
            vec![
                vec![i(1), t("forced")],
                vec![i(10), t("manual")],
                vec![i(11), t("manual2")]
            ]
        );
    }

    #[test]
    fn persisted_catalog_round_trips_foreign_keys() {
        let mut c = Catalog::new();
        run_mut(
            "CREATE TABLE parent (id int4 PRIMARY KEY, name text)",
            &mut c,
        )
        .expect("create parent");
        run_mut(
            "CREATE TABLE child (id int4 PRIMARY KEY, parent_id int4 REFERENCES parent(id) ON DELETE CASCADE)",
            &mut c,
        )
        .expect("create child");
        run_mut("INSERT INTO parent VALUES (1, 'p1'), (2, 'p2')", &mut c).expect("insert parent");
        run_mut("INSERT INTO child VALUES (10, 1), (20, 2)", &mut c).expect("insert child");
        let bytes = c.to_persisted_bytes().expect("persist");
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            PERSIST_FEATURE_TABLE_NOT_NULL
                | PERSIST_FEATURE_TABLE_UNIQUE_KEYS
                | PERSIST_FEATURE_TABLE_FOREIGN_KEYS
        );

        let mut reopened = Catalog::from_persisted_bytes(&bytes).expect("reopen");
        let err = run_mut("INSERT INTO child VALUES (30, 99)", &mut reopened)
            .expect_err("foreign key still enforced");
        assert!(
            err.message().contains("violates foreign key constraint"),
            "got: {}",
            err.message()
        );
        run_mut("DELETE FROM parent WHERE id = 1", &mut reopened).expect("cascade delete");
        let rows =
            run_mut("SELECT id, parent_id FROM child ORDER BY id", &mut reopened).expect("select");
        assert_eq!(rows.rows, vec![vec![i(20), i(2)]]);
    }

    #[test]
    fn persisted_catalog_round_trips_check_constraints() {
        let mut c = Catalog::new();
        run_mut(
            "CREATE TABLE people (id int4 PRIMARY KEY, age int4 CHECK (age >= 0))",
            &mut c,
        )
        .expect("create");
        run_mut("INSERT INTO people VALUES (1, NULL), (2, 5)", &mut c).expect("insert");
        let bytes = c.to_persisted_bytes().expect("persist");
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            PERSIST_FEATURE_TABLE_NOT_NULL
                | PERSIST_FEATURE_TABLE_UNIQUE_KEYS
                | PERSIST_FEATURE_TABLE_CHECKS
        );

        let mut reopened = Catalog::from_persisted_bytes(&bytes).expect("reopen");
        let rows =
            run_mut("SELECT id, age FROM people ORDER BY id", &mut reopened).expect("select");
        assert_eq!(
            rows.rows,
            vec![vec![i(1), SqlValue::Null], vec![i(2), i(5)]]
        );
        run_mut("INSERT INTO people VALUES (3, 0)", &mut reopened).expect("valid insert");
        let err = run_mut("INSERT INTO people VALUES (4, -1)", &mut reopened)
            .expect_err("check still enforced");
        assert!(
            err.message().contains("violates check constraint"),
            "got: {}",
            err.message()
        );
        let update_err = run_mut("UPDATE people SET age = -2 WHERE id = 2", &mut reopened)
            .expect_err("check still enforced on update");
        assert!(
            update_err.message().contains("violates check constraint"),
            "got: {}",
            update_err.message()
        );
    }

    #[test]
    fn persisted_catalog_round_trips_column_defaults() {
        let mut c = Catalog::new();
        run_mut(
            "CREATE TABLE people (id int4 PRIMARY KEY, name text DEFAULT 'anon', visits int4 DEFAULT 3)",
            &mut c,
        )
        .expect("create");
        run_mut("INSERT INTO people (id) VALUES (1)", &mut c).expect("insert omitted");
        let bytes = c.to_persisted_bytes().expect("persist");
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            PERSIST_FEATURE_TABLE_NOT_NULL
                | PERSIST_FEATURE_TABLE_UNIQUE_KEYS
                | PERSIST_FEATURE_TABLE_DEFAULTS
        );

        let mut reopened = Catalog::from_persisted_bytes(&bytes).expect("reopen");
        run_mut(
            "INSERT INTO people (id, name, visits) VALUES (2, DEFAULT, DEFAULT)",
            &mut reopened,
        )
        .expect("explicit defaults");
        run_mut(
            "INSERT INTO people (id, name, visits) VALUES (3, 'ada', 7)",
            &mut reopened,
        )
        .expect("explicit values");
        let rows = run_mut(
            "SELECT id, name, visits FROM people ORDER BY id",
            &mut reopened,
        )
        .expect("select");
        assert_eq!(
            rows.rows,
            vec![
                vec![i(1), t("anon"), i(3)],
                vec![i(2), t("anon"), i(3)],
                vec![i(3), t("ada"), i(7)]
            ]
        );
    }

    #[test]
    fn persisted_catalog_round_trips_serial_defaults() {
        let mut c = Catalog::new();
        run_mut(
            "CREATE TABLE people (id serial PRIMARY KEY, name text)",
            &mut c,
        )
        .expect("create");
        run_mut("INSERT INTO people (name) VALUES ('ada'), ('bob')", &mut c).expect("insert");
        let bytes = c.to_persisted_bytes().expect("persist");
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            PERSIST_FEATURE_TABLE_NOT_NULL
                | PERSIST_FEATURE_TABLE_UNIQUE_KEYS
                | PERSIST_FEATURE_SEQUENCES
                | PERSIST_FEATURE_TABLE_DEFAULTS
        );

        let mut reopened = Catalog::from_persisted_bytes(&bytes).expect("reopen");
        run_mut("INSERT INTO people (name) VALUES ('cy')", &mut reopened)
            .expect("serial default after reopen");
        run_mut(
            "INSERT INTO people (id, name) VALUES (10, 'manual')",
            &mut reopened,
        )
        .expect("manual serial value");
        run_mut("INSERT INTO people (name) VALUES ('dee')", &mut reopened)
            .expect("serial continues after manual value");
        let rows =
            run_mut("SELECT id, name FROM people ORDER BY id", &mut reopened).expect("select");
        assert_eq!(
            rows.rows,
            vec![
                vec![i(1), t("ada")],
                vec![i(2), t("bob")],
                vec![i(3), t("cy")],
                vec![i(4), t("dee")],
                vec![i(10), t("manual")]
            ]
        );
    }

    #[test]
    fn persisted_catalog_round_trips_unique_and_primary_constraints() {
        let mut c = Catalog::new();
        run_mut(
            "CREATE TABLE people (id int4 PRIMARY KEY, email text UNIQUE)",
            &mut c,
        )
        .expect("create");
        run_mut(
            "INSERT INTO people VALUES (1, 'a@example.test'), (2, 'b@example.test')",
            &mut c,
        )
        .expect("insert");
        let bytes = c.to_persisted_bytes().expect("persist");
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            PERSIST_FEATURE_TABLE_NOT_NULL | PERSIST_FEATURE_TABLE_UNIQUE_KEYS
        );

        let mut reopened = Catalog::from_persisted_bytes(&bytes).expect("reopen");
        let rows =
            run_mut("SELECT id, email FROM people ORDER BY id", &mut reopened).expect("select");
        assert_eq!(
            rows.rows,
            vec![
                vec![i(1), t("a@example.test")],
                vec![i(2), t("b@example.test")]
            ]
        );
        let duplicate_pk = run_mut(
            "INSERT INTO people VALUES (1, 'c@example.test')",
            &mut reopened,
        )
        .expect_err("primary key still enforced");
        assert!(
            duplicate_pk
                .message()
                .contains("duplicate key value violates unique constraint"),
            "got: {}",
            duplicate_pk.message()
        );
        let duplicate_unique = run_mut(
            "INSERT INTO people VALUES (3, 'b@example.test')",
            &mut reopened,
        )
        .expect_err("unique still enforced");
        assert!(
            duplicate_unique
                .message()
                .contains("duplicate key value violates unique constraint"),
            "got: {}",
            duplicate_unique.message()
        );
        let null_pk = run_mut(
            "INSERT INTO people VALUES (NULL, 'n@example.test')",
            &mut reopened,
        )
        .expect_err("primary-key not-null still enforced");
        assert!(
            null_pk.message().contains("violates not-null constraint"),
            "got: {}",
            null_pk.message()
        );
    }

    #[test]
    fn persisted_catalog_round_trips_explicit_indexes() {
        let mut c = Catalog::new();
        run_mut("CREATE TABLE people (id int4, email text)", &mut c).expect("create");
        run_mut(
            "INSERT INTO people VALUES (1, 'a@example.test'), (2, 'b@example.test')",
            &mut c,
        )
        .expect("insert");
        run_mut("CREATE INDEX people_id_idx ON people (id)", &mut c).expect("index");
        run_mut(
            "CREATE UNIQUE INDEX people_email_uidx ON people (email)",
            &mut c,
        )
        .expect("unique index");
        let bytes = c.to_persisted_bytes().expect("persist");
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            PERSIST_FEATURE_INDEXES
        );

        let mut reopened = Catalog::from_persisted_bytes(&bytes).expect("reopen");
        let rows =
            run_mut("SELECT id, email FROM people ORDER BY id", &mut reopened).expect("select");
        assert_eq!(
            rows.rows,
            vec![
                vec![i(1), t("a@example.test")],
                vec![i(2), t("b@example.test")]
            ]
        );
        let duplicate = run_mut(
            "INSERT INTO people VALUES (3, 'b@example.test')",
            &mut reopened,
        )
        .expect_err("unique index still enforced");
        assert!(
            duplicate
                .message()
                .contains("duplicate key value violates unique constraint"),
            "got: {}",
            duplicate.message()
        );
        run_mut("DROP INDEX people_id_idx", &mut reopened).expect("drop ordinary index");
        let after_drop =
            run_mut("SELECT id FROM people ORDER BY id", &mut reopened).expect("select after drop");
        assert_eq!(after_drop.rows, vec![vec![i(1)], vec![i(2)]]);
    }

    #[test]
    fn persisted_catalog_round_trips_sequences() {
        let mut c = Catalog::new();
        run_mut(
            "CREATE SEQUENCE s START WITH 10 INCREMENT BY 5 MINVALUE 10 MAXVALUE 30 CACHE 2 CYCLE",
            &mut c,
        )
        .expect("create sequence");
        let first = run_mut("SELECT nextval('s') AS v", &mut c).expect("first");
        assert_eq!(first.rows, vec![vec![i(10)]]);
        run_mut("BEGIN", &mut c).expect("begin");
        let second = run_mut("SELECT nextval('s') AS v", &mut c).expect("second");
        assert_eq!(second.rows, vec![vec![i(15)]]);
        run_mut("ROLLBACK", &mut c).expect("rollback");

        let bytes = c.to_persisted_bytes().expect("persist");
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            PERSIST_FEATURE_SEQUENCES
        );

        let mut reopened = Catalog::from_persisted_bytes(&bytes).expect("reopen");
        let curr = run_mut("SELECT currval('s') AS v", &mut reopened).expect("currval");
        assert_eq!(curr.rows, vec![vec![i(15)]]);
        let next = run_mut("SELECT nextval('s') AS v", &mut reopened).expect("next");
        assert_eq!(next.rows, vec![vec![i(20)]]);
        run_mut("ALTER SEQUENCE s RESTART WITH 25", &mut reopened).expect("restart");
        let restarted = run_mut("SELECT nextval('s') AS v", &mut reopened).expect("restarted");
        assert_eq!(restarted.rows, vec![vec![i(25)]]);
    }

    #[test]
    fn persisted_catalog_round_trips_not_null_constraints() {
        let mut c = Catalog::new();
        run_mut("CREATE TABLE people (id int4 NOT NULL, name text)", &mut c).expect("create");
        run_mut("INSERT INTO people VALUES (1, 'alice')", &mut c).expect("insert");
        let bytes = c.to_persisted_bytes().expect("persist");
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            PERSIST_FEATURE_TABLE_NOT_NULL
        );

        let mut reopened = Catalog::from_persisted_bytes(&bytes).expect("reopen");
        let rows =
            run_mut("SELECT id, name FROM people ORDER BY id", &mut reopened).expect("select");
        assert_eq!(rows.rows, vec![vec![i(1), t("alice")]]);
        let err = run_mut("INSERT INTO people VALUES (NULL, 'bad')", &mut reopened)
            .expect_err("not-null still enforced");
        assert!(
            err.message().contains("violates not-null constraint"),
            "got: {}",
            err.message()
        );
    }

    #[test]
    fn persisted_catalog_current_header_carries_version_and_feature_bits() {
        let bytes = sample_catalog_bytes();
        assert_eq!(&bytes[..8], b"CRUFTPG\0");
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 2);
        assert_eq!(u32::from_le_bytes(bytes[12..16].try_into().unwrap()), 0);

        let mut reopened = Catalog::from_persisted_bytes(&bytes).expect("current reopen");
        let rows =
            run_mut("SELECT id, name FROM people ORDER BY id", &mut reopened).expect("select");
        assert_eq!(rows.rows, vec![vec![i(1), t("alice")]]);
    }

    #[test]
    fn persisted_catalog_reads_legacy_v1_without_feature_bits() {
        let current = sample_catalog_bytes();
        let mut legacy = Vec::new();
        legacy.extend_from_slice(&current[..8]);
        legacy.extend_from_slice(&1u32.to_le_bytes());
        legacy.extend_from_slice(&current[16..]);

        let mut reopened = Catalog::from_persisted_bytes(&legacy).expect("legacy reopen");
        let rows =
            run_mut("SELECT id, name FROM people ORDER BY id", &mut reopened).expect("select");
        assert_eq!(rows.rows, vec![vec![i(1), t("alice")]]);
    }

    #[test]
    fn persisted_catalog_rejects_newer_versions_and_unknown_features() {
        let mut newer = sample_catalog_bytes();
        newer[8..12].copy_from_slice(&999u32.to_le_bytes());
        let newer_err = Catalog::from_persisted_bytes(&newer).expect_err("newer version");
        assert!(matches!(
            newer_err,
            PgError::InvalidInputSyntax {
                typ: "postcrust persistence",
                ..
            }
        ));

        let mut feature = sample_catalog_bytes();
        feature[12..16].copy_from_slice(&(1u32 << 31).to_le_bytes());
        let feature_err = Catalog::from_persisted_bytes(&feature).expect_err("feature bit");
        assert!(matches!(
            feature_err,
            PgError::InvalidInputSyntax {
                typ: "postcrust persistence",
                ..
            }
        ));
    }
}
