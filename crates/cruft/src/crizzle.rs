
use std::cell::RefCell;
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use rusty_js_runtime::value::{JsString, ObjectRef};
use rusty_js_runtime::{Object, Runtime, RuntimeError, Value};

use crate::register::{make_callable, native_function, new_object, register_method};
use postcrust::catalog::Catalog;
use postcrust::crizzle::dml::{
    execute_delete, execute_insert, execute_update, Delete, Insert, Update, WriteResult,
};
use postcrust::crizzle::query::{execute_with, plan, JoinKind, Query, RelResult, TypedResult};
use postcrust::crizzle::{
    derived_columns, propagate_result, sanitize_result, validate_result, CsType, PropagationRecord,
    RelKind, SanitizationRecord, SanitizeDefaults, ViolationKind,
};
use postcrust::stmt::{run_mut, QueryResult};
use postcrust::types::oid;
use sql_core::SqlValue;

thread_local! {

    static CATALOGS: RefCell<Vec<Option<PgCatalogHandle>>> = const { RefCell::new(Vec::new()) };
    static PG_PATH_LOCKS: RefCell<HashSet<PathBuf>> = RefCell::new(HashSet::new());

    static QUERIES: RefCell<Vec<Option<Query>>> = const { RefCell::new(Vec::new()) };

    static DMLS: RefCell<Vec<Option<Dml>>> = const { RefCell::new(Vec::new()) };
}

struct PgCatalogHandle {
    catalog: Catalog,
    path: Option<PathBuf>,
    _lock: Option<PgCatalogLock>,
}

struct PgCatalogLock {
    path: PathBuf,
    file: File,
}

impl Drop for PgCatalogLock {
    fn drop(&mut self) {
        let _ = unlock_catalog_file(&self.file);
        PG_PATH_LOCKS.with(|locks| {
            locks.borrow_mut().remove(&self.path);
        });
    }
}

enum Dml {
    Ins(Insert),
    Upd(Update),
    Del(Delete),
}

const DB_ID_SLOT: &str = "__crizzle_catalog";
const Q_ID_SLOT: &str = "__crizzle_query";
const Q_DB_SLOT: &str = "__crizzle_query_db";
const D_ID_SLOT: &str = "__crizzle_dml";
const D_DB_SLOT: &str = "__crizzle_dml_db";

const Q_ENG_SLOT: &str = "__crizzle_query_eng";
const D_ENG_SLOT: &str = "__crizzle_dml_eng";

fn builder_is_sqlite(rt: &mut Runtime) -> bool {
    matches!(rt.current_this(), Value::Object(id) if matches!(rt.object_get(id, Q_ENG_SLOT), Value::Number(n) if n == 1.0))
}
fn write_is_sqlite(rt: &mut Runtime) -> bool {
    matches!(rt.current_this(), Value::Object(id) if matches!(rt.object_get(id, D_ENG_SLOT), Value::Number(n) if n == 1.0))
}

fn rt_err(e: String) -> RuntimeError {
    RuntimeError::TypeError(e)
}

fn js_string(s: impl Into<String>) -> Value {
    Value::String(std::rc::Rc::new(JsString::from(s.into())))
}

fn sql_to_js(rt: &mut Runtime, v: &SqlValue, oid_val: u32) -> Value {
    match v {
        SqlValue::Null => Value::Null,
        SqlValue::Int(i) => {
            let force_bigint = oid_val == oid::INT8 || i.unsigned_abs() > (1u64 << 53);
            if force_bigint {
                Value::BigInt(std::rc::Rc::new(
                    rusty_js_runtime::bigint::JsBigInt::from_i64(*i),
                ))
            } else {
                Value::Number(*i as f64)
            }
        }
        SqlValue::Real(r) => Value::Number(*r),
        SqlValue::Text(s) => js_string(s.clone()),
        SqlValue::Blob(b) => Value::Object(rt.alloc_uint8_array_from_bytes(b)),
    }
}

fn row_object(rt: &mut Runtime, result: &QueryResult, row: &[SqlValue]) -> Value {
    let obj = rt.alloc_object(Object::new_ordinary());
    for (i, name) in result.columns.iter().enumerate() {
        let oid_val = result.col_types.get(i).copied().unwrap_or(0);
        let val = row
            .get(i)
            .map(|v| sql_to_js(rt, v, oid_val))
            .unwrap_or(Value::Null);
        rt.object_set(obj, name.clone().into(), val);
    }
    Value::Object(obj)
}

fn rows_as_objects(rt: &mut Runtime, result: &QueryResult) -> Value {
    let arr = rt.alloc_object(Object::new_array());
    {
        let o = rt.obj_mut(arr);
        o.array_dense = true;
        o.dense_elements.reserve(result.rows.len());
    }
    for row in &result.rows {
        let o = row_object(rt, result, row);
        rt.obj_mut(arr).dense_elements.push(o);
    }
    Value::Object(arr)
}

fn register_catalog(handle: PgCatalogHandle) -> usize {
    CATALOGS.with(|c| {
        let mut c = c.borrow_mut();
        c.push(Some(handle));
        c.len() - 1
    })
}

fn load_catalog_path(path: PathBuf) -> Result<PgCatalogHandle, RuntimeError> {
    let lock = lock_catalog_path(&path)?;
    cleanup_stale_catalog_temps(&path).map_err(|e| rt_err(format!("openPostgres: {e}")))?;
    let catalog = match fs::read(&path) {
        Ok(bytes) => Catalog::from_persisted_bytes(&bytes)
            .map_err(|e| rt_err(format!("openPostgres: {}", e.message())))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Catalog::new(),
        Err(e) => return Err(rt_err(format!("openPostgres: {e}"))),
    };
    Ok(PgCatalogHandle {
        catalog,
        path: Some(path),
        _lock: Some(lock),
    })
}

fn lock_identity(path: &Path) -> Result<PathBuf, RuntimeError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let parent_abs = if parent.exists() {
        fs::canonicalize(parent).map_err(|e| rt_err(format!("openPostgres: {e}")))?
    } else {
        let cwd = std::env::current_dir().map_err(|e| rt_err(format!("openPostgres: {e}")))?;
        cwd.join(parent)
    };
    Ok(parent_abs.join(path.file_name().unwrap_or_default()))
}

fn lock_catalog_path(path: &Path) -> Result<PgCatalogLock, RuntimeError> {
    let identity = lock_identity(path)?;
    let already_open = PG_PATH_LOCKS.with(|locks| locks.borrow().contains(&identity));
    if already_open {
        return Err(rt_err(format!(
            "openPostgres: database is already open for writing: {}",
            path.display()
        )));
    }

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|e| rt_err(format!("openPostgres: {e}")))?;
    let lock_path = parent.join(format!(
        ".{}.lock",
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("postcrust.db")
    ));
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&lock_path)
        .map_err(|e| rt_err(format!("openPostgres: {e}")))?;
    try_lock_catalog_file(&file).map_err(|e| {
        rt_err(format!(
            "openPostgres: database is already open for writing: {} ({e})",
            path.display()
        ))
    })?;
    PG_PATH_LOCKS.with(|locks| {
        locks.borrow_mut().insert(identity.clone());
    });
    Ok(PgCatalogLock {
        path: identity,
        file,
    })
}

#[cfg(unix)]
fn try_lock_catalog_file(file: &File) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(unix)]
fn unlock_catalog_file(file: &File) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) };
    if rc == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(not(unix))]
fn try_lock_catalog_file(_file: &File) -> std::io::Result<()> {
    Ok(())
}

#[cfg(not(unix))]
fn unlock_catalog_file(_file: &File) -> std::io::Result<()> {
    Ok(())
}

fn save_catalog_handle(handle: &PgCatalogHandle) -> Result<(), RuntimeError> {
    let Some(path) = &handle.path else {
        return Ok(());
    };
    let bytes = handle
        .catalog
        .to_persisted_bytes()
        .map_err(|e| rt_err(format!("openPostgres: {}", e.message())))?;
    atomic_write_catalog(path, &bytes).map_err(|e| rt_err(format!("openPostgres: {e}")))
}

fn catalog_temp_prefix(path: &Path) -> String {
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("postcrust.db");
    format!(".{file_name}.tmp-")
}

fn cleanup_stale_catalog_temps(path: &Path) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let Ok(entries) = fs::read_dir(parent) else {
        return Ok(());
    };
    let prefix = catalog_temp_prefix(path);
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.starts_with(&prefix) && entry.metadata()?.is_file() {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

fn atomic_write_catalog(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(
        "{}{}-{}",
        catalog_temp_prefix(path),
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));

    {
        let mut f = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    if let Ok(dir) = fs::File::open(parent) {
        let _ = dir.sync_all();
    }
    Ok(())
}

fn db_id(rt: &mut Runtime, this: ObjectRef) -> Result<usize, RuntimeError> {
    match rt.object_get(this, DB_ID_SLOT) {
        Value::Number(n) => Ok(n as usize),
        _ => Err(rt_err("not an ORM db (missing catalog handle)".into())),
    }
}

fn run_sql(rt: &mut Runtime, this: ObjectRef, sql: &str) -> Result<QueryResult, RuntimeError> {
    let id = db_id(rt, this)?;
    CATALOGS.with(|c| {
        let mut c = c.borrow_mut();
        let handle = c
            .get_mut(id)
            .and_then(|slot| slot.as_mut())
            .ok_or_else(|| rt_err("ORM db is closed".into()))?;
        let result = run_mut(sql, &mut handle.catalog).map_err(|e| rt_err(e.message()))?;
        if !handle.catalog.in_transaction() {
            save_catalog_handle(handle)?;
        }
        Ok(result)
    })
}

fn arg_sql(args: &[Value]) -> Result<String, RuntimeError> {
    match args.first() {
        Some(Value::String(s)) => Ok(s.as_str().to_string()),
        _ => Err(rt_err("expected a SQL string argument".into())),
    }
}

fn this_db(rt: &mut Runtime) -> Result<ObjectRef, RuntimeError> {
    match rt.current_this() {
        Value::Object(id) => Ok(id),
        _ => Err(rt_err("ORM db method called without a db receiver".into())),
    }
}

fn build_db(rt: &mut Runtime, id: usize) -> Value {
    let db = new_object(rt);
    rt.object_set(db, DB_ID_SLOT.into(), Value::Number(id as f64));

    register_method(rt, db, "exec", |rt, args| {
        let this = this_db(rt)?;
        let sql = arg_sql(args)?;
        run_sql(rt, this, &sql)?;
        Ok(Value::Undefined)
    });

    register_method(rt, db, "query", |rt, args| {
        let this = this_db(rt)?;
        let sql = arg_sql(args)?;
        let result = run_sql(rt, this, &sql)?;
        match contract_expected(rt, this, args.get(1))? {

            Some(expected) => boundary_cross(rt, expected, result, Mode::Halt),
            None => Err(rt_err(
                "query needs a contract (table name or descriptor)".into(),
            )),
        }
    });

    register_method(rt, db, "querySanitize", |rt, args| {
        let this = this_db(rt)?;
        let sql = arg_sql(args)?;
        let result = run_sql(rt, this, &sql)?;
        let expected = contract_expected(rt, this, args.get(1))?.ok_or_else(|| {
            rt_err("querySanitize needs a contract (table name or descriptor)".into())
        })?;
        let defaults = match args.get(2) {
            Some(Value::Object(id)) => parse_defaults(rt, *id),
            _ => SanitizeDefaults::new(),
        };
        boundary_cross(rt, expected, result, Mode::Sanitize(defaults))
    });

    register_method(rt, db, "queryPropagate", |rt, args| {
        let this = this_db(rt)?;
        let sql = arg_sql(args)?;
        let result = run_sql(rt, this, &sql)?;
        let expected = contract_expected(rt, this, args.get(1))?.ok_or_else(|| {
            rt_err("queryPropagate needs a contract (table name or descriptor)".into())
        })?;
        boundary_cross(rt, expected, result, Mode::Propagate)
    });

    register_method(rt, db, "from", |rt, args| {
        let this = this_db(rt)?;
        let dbid = db_id(rt, this)?;
        let table = arg_sql(args)?;
        let qid = QUERIES.with(|q| {
            let mut q = q.borrow_mut();
            q.push(Some(Query::from(&table)));
            q.len() - 1
        });
        Ok(build_builder(rt, dbid, qid, false))
    });

    register_method(rt, db, "insertInto", |rt, args| {
        let this = this_db(rt)?;
        let dbid = db_id(rt, this)?;
        let t = arg_sql(args)?;
        Ok(build_write_builder(
            rt,
            dbid,
            Dml::Ins(Insert::into(&t)),
            false,
        ))
    });
    register_method(rt, db, "update", |rt, args| {
        let this = this_db(rt)?;
        let dbid = db_id(rt, this)?;
        let t = arg_sql(args)?;
        Ok(build_write_builder(
            rt,
            dbid,
            Dml::Upd(Update::table(&t)),
            false,
        ))
    });
    register_method(rt, db, "deleteFrom", |rt, args| {
        let this = this_db(rt)?;
        let dbid = db_id(rt, this)?;
        let t = arg_sql(args)?;
        Ok(build_write_builder(
            rt,
            dbid,
            Dml::Del(Delete::table(&t)),
            false,
        ))
    });

    Value::Object(db)
}

fn build_write_builder(rt: &mut Runtime, dbid: usize, dml: Dml, sqlite: bool) -> Value {
    let did = DMLS.with(|d| {
        let mut d = d.borrow_mut();
        d.push(Some(dml));
        d.len() - 1
    });
    let b = new_object(rt);
    rt.object_set(b, D_DB_SLOT.into(), Value::Number(dbid as f64));
    rt.object_set(b, D_ID_SLOT.into(), Value::Number(did as f64));
    rt.object_set(
        b,
        D_ENG_SLOT.into(),
        Value::Number(if sqlite { 1.0 } else { 0.0 }),
    );

    register_method(rt, b, "values", |rt, args| {
        let (this, did) = this_write(rt)?;
        let rows = read_rows(rt, args.first().unwrap_or(&Value::Undefined));
        mutate_dml(did, move |dml| match dml {
            Dml::Ins(mut ins) => {
                if let Some((cols, _)) = rows.first() {
                    let refs: Vec<&str> = cols.iter().map(|s| s.as_str()).collect();
                    ins = ins.columns(&refs);
                }
                for (_, vals) in &rows {
                    ins = ins.row(vals);
                }
                Dml::Ins(ins)
            }
            other => other,
        });
        Ok(Value::Object(this))
    });

    register_method(rt, b, "set", |rt, args| {
        let (this, did) = this_write(rt)?;
        let assigns = read_assigns(rt, args.first().unwrap_or(&Value::Undefined));
        mutate_dml(did, move |dml| match dml {
            Dml::Upd(mut up) => {
                for (col, val) in &assigns {
                    up = up.set(col, val.clone());
                }
                Dml::Upd(up)
            }
            other => other,
        });
        Ok(Value::Object(this))
    });

    register_method(rt, b, "where", |rt, args| {
        let (this, did) = this_write(rt)?;
        let col = str_arg(args, 0)?;
        let op = str_arg(args, 1)?;
        let values = if op == "in" {
            js_array_to_sql(rt, args.get(2))
        } else {
            Vec::new()
        };
        let v = js_to_sql(args.get(2).unwrap_or(&Value::Null));
        mutate_dml(did, move |dml| apply_dml_filter(dml, &col, &op, v, values));
        Ok(Value::Object(this))
    });

    register_method(rt, b, "returning", |rt, args| {
        let (this, did) = this_write(rt)?;
        let all = matches!(args.first(), Some(Value::Boolean(true)) | None);
        let cols = if all {
            Vec::new()
        } else {
            collect_keys(rt, &args[0])
        };
        mutate_dml(did, move |dml| apply_returning(dml, all, &cols));
        Ok(Value::Object(this))
    });

    register_method(rt, b, "onConflictDoNothing", |rt, args| {
        let (this, did) = this_write(rt)?;
        let target = collect_keys(rt, args.first().unwrap_or(&Value::Undefined));
        mutate_dml(did, move |dml| match dml {
            Dml::Ins(ins) => {
                let refs: Vec<&str> = target.iter().map(|s| s.as_str()).collect();
                Dml::Ins(ins.on_conflict_do_nothing(&refs))
            }
            other => other,
        });
        Ok(Value::Object(this))
    });
    register_method(rt, b, "onConflictDoUpdate", |rt, args| {
        let (this, did) = this_write(rt)?;
        let target = collect_keys(rt, args.first().unwrap_or(&Value::Undefined));
        let set = collect_keys(rt, args.get(1).unwrap_or(&Value::Undefined));
        mutate_dml(did, move |dml| match dml {
            Dml::Ins(ins) => {
                let trefs: Vec<&str> = target.iter().map(|s| s.as_str()).collect();
                let srefs: Vec<&str> = set.iter().map(|s| s.as_str()).collect();
                Dml::Ins(ins.on_conflict_do_update(&trefs, &srefs))
            }
            other => other,
        });
        Ok(Value::Object(this))
    });

    register_method(rt, b, "run", |rt, _args| {
        let (_this, did) = this_write(rt)?;
        let dbid = write_db_id(rt)?;
        if write_is_sqlite(rt) {
            run_write_sqlite(rt, dbid, did)
        } else {
            run_write(rt, dbid, did)
        }
    });

    Value::Object(b)
}

fn this_write(rt: &mut Runtime) -> Result<(ObjectRef, usize), RuntimeError> {
    let this = match rt.current_this() {
        Value::Object(id) => id,
        _ => {
            return Err(rt_err(
                "crizzle: write method called without a builder".into(),
            ))
        }
    };
    match rt.object_get(this, D_ID_SLOT) {
        Value::Number(n) => Ok((this, n as usize)),
        _ => Err(rt_err("crizzle: not a write builder".into())),
    }
}
fn write_db_id(rt: &mut Runtime) -> Result<usize, RuntimeError> {
    match rt.current_this() {
        Value::Object(id) => match rt.object_get(id, D_DB_SLOT) {
            Value::Number(n) => Ok(n as usize),
            _ => Err(rt_err("crizzle: write builder has no db".into())),
        },
        _ => Err(rt_err(
            "crizzle: write method called without a builder".into(),
        )),
    }
}

fn mutate_dml<F: FnOnce(Dml) -> Dml>(did: usize, f: F) {
    DMLS.with(|d| {
        let mut d = d.borrow_mut();
        if let Some(slot) = d.get_mut(did) {
            if let Some(dml) = slot.take() {
                *slot = Some(f(dml));
            }
        }
    });
}

fn read_rows(rt: &mut Runtime, v: &Value) -> Vec<(Vec<String>, Vec<SqlValue>)> {

    if let Value::Object(id) = v {
        if rt.array_length(*id) == 0 && !is_array(rt, *id) {
            return vec![read_assigns_split(rt, *id)];
        }

        let len = rt.array_length(*id);
        let mut out = Vec::new();
        let mut cols: Option<Vec<String>> = None;
        for i in 0..len {
            if let Value::Object(row_id) = rt.object_get(*id, &i.to_string()) {
                match &cols {
                    Some(cols) => {
                        out.push((Vec::new(), read_values_for_cols(rt, row_id, cols)));
                    }
                    None => {
                        let (first_cols, vals) = read_assigns_split(rt, row_id);
                        cols = Some(first_cols.clone());
                        out.push((first_cols, vals));
                    }
                }
            }
        }
        return out;
    }
    Vec::new()
}

fn is_array(rt: &mut Runtime, id: ObjectRef) -> bool {

    rt.array_length(id) > 0
}

fn read_assigns_split(rt: &mut Runtime, id: ObjectRef) -> (Vec<String>, Vec<SqlValue>) {
    let keys_v = rt
        .own_enumerable_string_keys_via(&Value::Object(id))
        .unwrap_or(Value::Undefined);
    let keys = collect_keys(rt, &keys_v);
    let mut cols = Vec::with_capacity(keys.len());
    let mut vals = Vec::with_capacity(keys.len());
    for k in keys {
        let v = rt.object_get(id, &k);
        vals.push(js_to_sql(&v));
        cols.push(k);
    }
    (cols, vals)
}

fn read_values_for_cols(rt: &mut Runtime, id: ObjectRef, cols: &[String]) -> Vec<SqlValue> {
    cols.iter()
        .map(|k| js_to_sql(&rt.object_get(id, k)))
        .collect()
}

fn read_assigns(rt: &mut Runtime, v: &Value) -> Vec<(String, SqlValue)> {
    if let Value::Object(id) = v {
        let (cols, vals) = read_assigns_split(rt, *id);
        return cols.into_iter().zip(vals).collect();
    }
    Vec::new()
}

fn apply_dml_filter(dml: Dml, col: &str, op: &str, v: SqlValue, values: Vec<SqlValue>) -> Dml {
    match dml {
        Dml::Upd(up) => Dml::Upd(match op {
            "=" | "==" => up.filter_eq(col, v),
            "!=" | "<>" => up.filter_ne(col, v),
            "<" => up.filter_lt(col, v),
            "<=" => up.filter_le(col, v),
            ">" => up.filter_gt(col, v),
            ">=" => up.filter_ge(col, v),
            "like" => up.filter_like(col, v),
            "in" => up.filter_in(col, values),
            "isNull" | "is null" => up.filter_is_null(col),
            "isNotNull" | "is not null" => up.filter_is_not_null(col),
            _ => up.filter_eq(col, v),
        }),
        Dml::Del(del) => Dml::Del(match op {
            "=" | "==" => del.filter_eq(col, v),
            "!=" | "<>" => del.filter_ne(col, v),
            "<" => del.filter_lt(col, v),
            "<=" => del.filter_le(col, v),
            ">" => del.filter_gt(col, v),
            ">=" => del.filter_ge(col, v),
            "like" => del.filter_like(col, v),
            "in" => del.filter_in(col, values),
            "isNull" | "is null" => del.filter_is_null(col),
            "isNotNull" | "is not null" => del.filter_is_not_null(col),
            _ => del.filter_eq(col, v),
        }),
        other => other,
    }
}

fn apply_returning(dml: Dml, all: bool, cols: &[String]) -> Dml {
    let refs: Vec<&str> = cols.iter().map(|s| s.as_str()).collect();
    match dml {
        Dml::Ins(i) => Dml::Ins(if all {
            i.returning_all()
        } else {
            i.returning(&refs)
        }),
        Dml::Upd(u) => Dml::Upd(if all {
            u.returning_all()
        } else {
            u.returning(&refs)
        }),
        Dml::Del(d) => Dml::Del(if all {
            d.returning_all()
        } else {
            d.returning(&refs)
        }),
    }
}

fn run_write(rt: &mut Runtime, dbid: usize, did: usize) -> Result<Value, RuntimeError> {
    let dml = DMLS.with(|d| d.borrow_mut().get_mut(did).and_then(|s| s.take()));
    let dml = dml.ok_or_else(|| rt_err("crizzle: write already run".into()))?;
    let wr = CATALOGS.with(|cs| {
        let mut cs = cs.borrow_mut();
        let handle = cs
            .get_mut(dbid)
            .and_then(|s| s.as_mut())
            .ok_or_else(|| rt_err("crizzle db is closed".into()))?;
        let wr = match dml {
            Dml::Ins(i) => execute_insert(&i, &mut handle.catalog),
            Dml::Upd(u) => execute_update(&u, &mut handle.catalog),
            Dml::Del(d) => execute_delete(&d, &mut handle.catalog),
        }
        .map_err(|e| rt_err(e.message()))?;
        if !handle.catalog.in_transaction() {
            save_catalog_handle(handle)?;
        }
        Ok(wr)
    })?;
    Ok(write_result_to_js(rt, wr))
}

fn write_result_to_js(rt: &mut Runtime, wr: WriteResult) -> Value {
    let o = rt.alloc_object(Object::new_ordinary());
    rt.object_set(o, "affected".into(), Value::Number(wr.affected as f64));
    if let Some(tr) = &wr.returned {
        let rows = typed_result_to_js(rt, tr);
        rt.object_set(o, "rows".into(), rows);
    }
    Value::Object(o)
}

fn typed_result_to_js(rt: &mut Runtime, tr: &TypedResult) -> Value {
    let arr = rt.alloc_object(Object::new_array());
    for (i, row) in tr.rows.iter().enumerate() {
        let obj = rt.alloc_object(Object::new_ordinary());
        for (j, (name, _ty)) in tr.columns.iter().enumerate() {
            let v = row
                .get(j)
                .map(|v| sql_to_js(rt, v, 0))
                .unwrap_or(Value::Null);
            rt.object_set(obj, name.clone().into(), v);
        }
        rt.object_set(arr, i.to_string().into(), Value::Object(obj));
    }
    rt.object_set(arr, "length".into(), Value::Number(tr.rows.len() as f64));
    Value::Object(arr)
}

fn build_builder(rt: &mut Runtime, dbid: usize, qid: usize, sqlite: bool) -> Value {
    let b = new_object(rt);
    rt.object_set(b, Q_DB_SLOT.into(), Value::Number(dbid as f64));
    rt.object_set(b, Q_ID_SLOT.into(), Value::Number(qid as f64));
    rt.object_set(
        b,
        Q_ENG_SLOT.into(),
        Value::Number(if sqlite { 1.0 } else { 0.0 }),
    );

    register_method(rt, b, "where", |rt, args| {
        let (this, qid) = this_builder(rt)?;
        let col = str_arg(args, 0)?;
        let op = str_arg(args, 1)?;
        apply_filter(rt, qid, &col, &op, args.get(2))?;
        Ok(Value::Object(this))
    });

    register_method(rt, b, "join", |rt, args| {
        add_join(rt, JoinKind::Inner, args)
    });
    register_method(rt, b, "joinInner", |rt, args| {
        add_join(rt, JoinKind::Inner, args)
    });
    register_method(rt, b, "joinLeft", |rt, args| {
        add_join(rt, JoinKind::Left, args)
    });
    register_method(rt, b, "joinRight", |rt, args| {
        add_join(rt, JoinKind::Right, args)
    });
    register_method(rt, b, "joinFull", |rt, args| {
        add_join(rt, JoinKind::Full, args)
    });

    register_method(rt, b, "with", |rt, args| {
        let (this, qid) = this_builder(rt)?;
        let rel = str_arg(args, 0)?;
        mutate_query(qid, move |q| q.with(&rel));
        Ok(Value::Object(this))
    });

    register_method(rt, b, "groupBy", |rt, args| {
        let (this, qid) = this_builder(rt)?;
        let cols = collect_keys(rt, args.first().unwrap_or(&Value::Undefined));
        mutate_query(qid, move |q| {
            let refs: Vec<&str> = cols.iter().map(|s| s.as_str()).collect();
            q.group_by(&refs)
        });
        Ok(Value::Object(this))
    });
    register_method(rt, b, "count", |rt, args| {
        let (this, qid) = this_builder(rt)?;
        let alias = str_arg(args, 0)?;
        mutate_query(qid, move |q| q.count(&alias));
        Ok(Value::Object(this))
    });
    register_method(rt, b, "countCol", |rt, args| {
        let (this, qid) = this_builder(rt)?;
        let col = str_arg(args, 0)?;
        let alias = str_arg(args, 1)?;
        mutate_query(qid, move |q| q.count_col(&col, &alias));
        Ok(Value::Object(this))
    });
    register_method(rt, b, "sum", |rt, args| {
        let (this, qid) = this_builder(rt)?;
        let col = str_arg(args, 0)?;
        let alias = str_arg(args, 1)?;
        mutate_query(qid, move |q| q.sum(&col, &alias));
        Ok(Value::Object(this))
    });
    register_method(rt, b, "avg", |rt, args| {
        let (this, qid) = this_builder(rt)?;
        let col = str_arg(args, 0)?;
        let alias = str_arg(args, 1)?;
        mutate_query(qid, move |q| q.avg(&col, &alias));
        Ok(Value::Object(this))
    });
    register_method(rt, b, "min", |rt, args| {
        let (this, qid) = this_builder(rt)?;
        let col = str_arg(args, 0)?;
        let alias = str_arg(args, 1)?;
        mutate_query(qid, move |q| q.min(&col, &alias));
        Ok(Value::Object(this))
    });
    register_method(rt, b, "max", |rt, args| {
        let (this, qid) = this_builder(rt)?;
        let col = str_arg(args, 0)?;
        let alias = str_arg(args, 1)?;
        mutate_query(qid, move |q| q.max(&col, &alias));
        Ok(Value::Object(this))
    });
    register_method(rt, b, "select", |rt, args| {
        let (this, qid) = this_builder(rt)?;
        let cols = collect_keys(rt, args.first().unwrap_or(&Value::Undefined));
        mutate_query(qid, move |q| {
            let refs: Vec<&str> = cols.iter().map(|s| s.as_str()).collect();
            q.select(&refs)
        });
        Ok(Value::Object(this))
    });
    register_method(rt, b, "orderBy", |rt, args| {
        let (this, qid) = this_builder(rt)?;
        let col = str_arg(args, 0)?;
        let desc = matches!(args.get(1), Some(Value::String(s)) if s.as_str().eq_ignore_ascii_case("desc"));
        mutate_query(qid, move |q| q.order_by(&col, desc));
        Ok(Value::Object(this))
    });
    register_method(rt, b, "limit", |rt, args| {
        let (this, qid) = this_builder(rt)?;
        let n = num_arg(args, 0);
        mutate_query(qid, move |q| q.limit(n));
        Ok(Value::Object(this))
    });
    register_method(rt, b, "offset", |rt, args| {
        let (this, qid) = this_builder(rt)?;
        let n = num_arg(args, 0);
        mutate_query(qid, move |q| q.offset(n));
        Ok(Value::Object(this))
    });
    register_method(rt, b, "all", |rt, _args| {
        let (_this, qid) = this_builder(rt)?;
        let dbid = builder_db_id(rt)?;
        let sqlite = builder_is_sqlite(rt);
        run_builder(rt, dbid, qid, sqlite)
    });
    register_method(rt, b, "iter", |rt, _args| {
        let (_this, qid) = this_builder(rt)?;
        let dbid = builder_db_id(rt)?;
        if !builder_is_sqlite(rt) {
            return Err(rt_err(
                "crizzle iter() is currently implemented for SQLite only".into(),
            ));
        }
        run_builder_sqlite_iter(rt, dbid, qid)
    });
    register_method(rt, b, "get", |rt, _args| {
        let (_this, qid) = this_builder(rt)?;
        let dbid = builder_db_id(rt)?;
        let sqlite = builder_is_sqlite(rt);
        let rows = run_builder(rt, dbid, qid, sqlite)?;

        if let Value::Object(arr) = &rows {
            if rt.array_length(*arr) > 0 {
                return Ok(rt.object_get(*arr, "0"));
            }
        }
        Ok(Value::Null)
    });

    Value::Object(b)
}

fn this_builder(rt: &mut Runtime) -> Result<(ObjectRef, usize), RuntimeError> {
    let this = match rt.current_this() {
        Value::Object(id) => id,
        _ => {
            return Err(rt_err(
                "crizzle: builder method called without a builder receiver".into(),
            ))
        }
    };
    match rt.object_get(this, Q_ID_SLOT) {
        Value::Number(n) => Ok((this, n as usize)),
        _ => Err(rt_err("crizzle: not a query builder".into())),
    }
}

fn builder_db_id(rt: &mut Runtime) -> Result<usize, RuntimeError> {
    let this = match rt.current_this() {
        Value::Object(id) => id,
        _ => {
            return Err(rt_err(
                "crizzle: builder method called without a builder receiver".into(),
            ))
        }
    };
    match rt.object_get(this, Q_DB_SLOT) {
        Value::Number(n) => Ok(n as usize),
        _ => Err(rt_err("crizzle: builder has no db".into())),
    }
}

fn str_arg(args: &[Value], i: usize) -> Result<String, RuntimeError> {
    match args.get(i) {
        Some(Value::String(s)) => Ok(s.as_str().to_string()),
        _ => Err(rt_err(format!(
            "crizzle: expected a string argument at position {i}"
        ))),
    }
}

fn num_arg(args: &[Value], i: usize) -> u64 {
    match args.get(i) {
        Some(Value::Number(n)) if *n >= 0.0 => *n as u64,
        _ => 0,
    }
}

fn add_join(rt: &mut Runtime, kind: JoinKind, args: &[Value]) -> Result<Value, RuntimeError> {
    let (this, qid) = this_builder(rt)?;
    let table = str_arg(args, 0)?;
    let lcol = str_arg(args, 1)?;
    let rcol = str_arg(args, 2)?;
    mutate_query(qid, move |q| {
        q.join(kind, &table, &[(lcol.as_str(), rcol.as_str())])
    });
    Ok(Value::Object(this))
}

fn mutate_query<F: FnOnce(Query) -> Query>(qid: usize, f: F) {
    QUERIES.with(|q| {
        let mut q = q.borrow_mut();
        if let Some(slot) = q.get_mut(qid) {
            if let Some(query) = slot.take() {
                *slot = Some(f(query));
            }
        }
    });
}

fn apply_filter(
    rt: &mut Runtime,
    qid: usize,
    col: &str,
    op: &str,
    val: Option<&Value>,
) -> Result<(), RuntimeError> {
    let col = col.to_string();
    match op {
        "=" | "==" => {
            let v = js_to_sql(val.unwrap_or(&Value::Null));
            mutate_query(qid, move |q| q.filter_eq(&col, v));
        }
        "!=" | "<>" => {
            let v = js_to_sql(val.unwrap_or(&Value::Null));
            mutate_query(qid, move |q| q.filter_ne(&col, v));
        }
        "<" => {
            let v = js_to_sql(val.unwrap_or(&Value::Null));
            mutate_query(qid, move |q| q.filter_lt(&col, v));
        }
        "<=" => {
            let v = js_to_sql(val.unwrap_or(&Value::Null));
            mutate_query(qid, move |q| q.filter_le(&col, v));
        }
        ">" => {
            let v = js_to_sql(val.unwrap_or(&Value::Null));
            mutate_query(qid, move |q| q.filter_gt(&col, v));
        }
        ">=" => {
            let v = js_to_sql(val.unwrap_or(&Value::Null));
            mutate_query(qid, move |q| q.filter_ge(&col, v));
        }
        "like" => {
            let v = js_to_sql(val.unwrap_or(&Value::Null));
            mutate_query(qid, move |q| q.filter_like(&col, v));
        }
        "in" => {
            let vs = js_array_to_sql(rt, val);
            mutate_query(qid, move |q| q.filter_in(&col, vs));
        }
        "isNull" | "is null" => mutate_query(qid, move |q| q.filter_is_null(&col)),
        "isNotNull" | "is not null" => mutate_query(qid, move |q| q.filter_is_not_null(&col)),
        other => return Err(rt_err(format!("crizzle: unknown filter op '{other}'"))),
    }
    Ok(())
}

fn js_array_to_sql(rt: &mut Runtime, v: Option<&Value>) -> Vec<SqlValue> {
    let mut out = Vec::new();
    if let Some(Value::Object(id)) = v {
        let len = rt.array_length(*id);
        for i in 0..len {
            let e = rt.object_get(*id, &i.to_string());
            out.push(js_to_sql(&e));
        }
    }
    out
}

fn run_builder(
    rt: &mut Runtime,
    dbid: usize,
    qid: usize,
    sqlite: bool,
) -> Result<Value, RuntimeError> {
    if sqlite {
        return run_builder_sqlite(rt, dbid, qid);
    }

    let has_relations = QUERIES
        .with(|qs| {
            qs.borrow()
                .get(qid)
                .and_then(|s| s.as_ref())
                .map(|q| !q.with.is_empty())
        })
        .unwrap_or(false);
    if has_relations {
        return run_with_relations(rt, dbid, qid);
    }

    let (final_sql, result_type) = QUERIES.with(|qs| {
        let qs = qs.borrow();
        let query = qs
            .get(qid)
            .and_then(|s| s.as_ref())
            .ok_or_else(|| rt_err("crizzle: query already consumed".into()))?;
        CATALOGS.with(|cs| {
            let cs = cs.borrow();
            let handle = cs
                .get(dbid)
                .and_then(|s| s.as_ref())
                .ok_or_else(|| rt_err("crizzle db is closed".into()))?;
            plan(query, &handle.catalog).map_err(|e| rt_err(e.message()))
        })
    })?;

    let result = CATALOGS.with(|cs| {
        let mut cs = cs.borrow_mut();
        let handle = cs
            .get_mut(dbid)
            .and_then(|s| s.as_mut())
            .ok_or_else(|| rt_err("crizzle db is closed".into()))?;
        run_mut(&final_sql, &mut handle.catalog).map_err(|e| rt_err(e.message()))
    })?;

    boundary_cross(rt, result_type, result, Mode::Halt)
}

fn run_with_relations(rt: &mut Runtime, dbid: usize, qid: usize) -> Result<Value, RuntimeError> {
    let query = QUERIES
        .with(|qs| qs.borrow().get(qid).and_then(|s| s.as_ref()).cloned())
        .ok_or_else(|| rt_err("crizzle: query already consumed".into()))?;
    let rr = CATALOGS.with(|cs| {
        let mut cs = cs.borrow_mut();
        let handle = cs
            .get_mut(dbid)
            .and_then(|s| s.as_mut())
            .ok_or_else(|| rt_err("crizzle db is closed".into()))?;
        execute_with(&query, &mut handle.catalog).map_err(|e| rt_err(e.message()))
    })?;
    Ok(rel_result_to_js(rt, &rr))
}

fn typed_row_obj(rt: &mut Runtime, cols: &[(String, CsType)], row: &[SqlValue]) -> Value {
    let obj = rt.alloc_object(Object::new_ordinary());
    for (j, (name, _)) in cols.iter().enumerate() {
        let v = row
            .get(j)
            .map(|v| sql_to_js(rt, v, 0))
            .unwrap_or(Value::Null);
        rt.object_set(obj, name.clone().into(), v);
    }
    Value::Object(obj)
}

fn rel_result_to_js(rt: &mut Runtime, rr: &RelResult) -> Value {
    let arr = rt.alloc_object(Object::new_array());
    for (i, prow) in rr.parent.rows.iter().enumerate() {
        let parent = typed_row_obj(rt, &rr.parent.columns, prow);
        let parent_id = match parent {
            Value::Object(id) => id,
            _ => continue,
        };
        for rel in &rr.relations {
            let children = &rel.children[i];
            let field = match rel.kind {
                RelKind::HasMany => {
                    let carr = rt.alloc_object(Object::new_array());
                    for (k, crow) in children.iter().enumerate() {
                        let cobj = typed_row_obj(rt, &rel.child_columns, crow);
                        rt.object_set(carr, k.to_string().into(), cobj);
                    }
                    rt.object_set(carr, "length".into(), Value::Number(children.len() as f64));
                    Value::Object(carr)
                }
                RelKind::BelongsTo => match children.first() {
                    Some(crow) => typed_row_obj(rt, &rel.child_columns, crow),
                    None => Value::Null,
                },
            };
            rt.object_set(parent_id, rel.name.clone().into(), field);
        }
        rt.object_set(arr, i.to_string().into(), Value::Object(parent_id));
    }
    rt.object_set(
        arr,
        "length".into(),
        Value::Number(rr.parent.rows.len() as f64),
    );
    Value::Object(arr)
}

fn contract_expected(
    rt: &mut Runtime,
    this: ObjectRef,
    arg: Option<&Value>,
) -> Result<Option<Vec<(String, CsType)>>, RuntimeError> {
    match arg {
        Some(Value::String(table)) => Ok(Some(derive_expected(rt, this, table.as_str())?)),
        Some(Value::Object(id)) => Ok(Some(parse_declared(rt, *id)?)),
        _ => Ok(None),
    }
}

fn derive_expected(
    rt: &mut Runtime,
    this: ObjectRef,
    table: &str,
) -> Result<Vec<(String, CsType)>, RuntimeError> {
    let id = db_id(rt, this)?;
    CATALOGS.with(|c| {
        let c = c.borrow();
        let handle = c
            .get(id)
            .and_then(|slot| slot.as_ref())
            .ok_or_else(|| rt_err("crizzle db is closed".into()))?;
        derived_columns(&handle.catalog, table)
            .ok_or_else(|| rt_err(format!("no such table: {table}")))
    })
}

fn parse_declared(rt: &mut Runtime, obj: ObjectRef) -> Result<Vec<(String, CsType)>, RuntimeError> {
    let keys_v = rt.own_enumerable_string_keys_via(&Value::Object(obj))?;
    let keys = collect_keys(rt, &keys_v);
    let mut cols = Vec::with_capacity(keys.len());
    for key in keys {
        let tag = rt.object_get(obj, &key);
        cols.push((key, cstype_from_tag(rt, &tag)?));
    }
    Ok(cols)
}

fn cstype_from_tag(rt: &mut Runtime, tag: &Value) -> Result<CsType, RuntimeError> {
    match tag {
        Value::String(s) => {
            let mut t = s.as_str().to_string();
            let nullable = t.ends_with('?');
            if nullable {
                t.pop();
            }
            let base = match t.as_str() {
                "number" => CsType::Number,
                "bigint" => CsType::BigInt,
                "boolean" => CsType::Boolean,
                "string" => CsType::Str,
                "Date" => CsType::Date,
                "bytes" | "Uint8Array" => CsType::Bytes,
                "unknown" => CsType::Unknown("declared"),
                other => return Err(rt_err(format!("crizzle: unknown type tag '{other}'"))),
            };
            Ok(if nullable {
                CsType::Nullable(Box::new(base))
            } else {
                base
            })
        }

        Value::Object(_) => {
            let labels = collect_keys(rt, tag);
            if labels.is_empty() {
                return Err(rt_err(
                    "crizzle: type descriptor must be a tag or non-empty label array".into(),
                ));
            }
            Ok(CsType::Enum(labels))
        }
        _ => Err(rt_err(
            "crizzle: type descriptor must be a string or array".into(),
        )),
    }
}

fn collect_keys(rt: &mut Runtime, arr: &Value) -> Vec<String> {
    let mut out = Vec::new();
    if let Value::Object(id) = arr {
        let len = rt.array_length(*id);
        for i in 0..len {
            if let Value::String(s) = rt.object_get(*id, &i.to_string()) {
                out.push(s.as_str().to_string());
            }
        }
    }
    out
}

enum Mode {
    Halt,
    Sanitize(SanitizeDefaults),
    Propagate,
}

fn boundary_cross(
    rt: &mut Runtime,
    expected: Vec<(String, CsType)>,
    result: QueryResult,
    mode: Mode,
) -> Result<Value, RuntimeError> {
    let qr = Rc::new(result);
    let exp = Rc::new(expected);
    let halt = matches!(mode, Mode::Halt);

    let qr_v = qr.clone();
    let exp_v = exp.clone();
    let validator = native_function(rt, "crizzleValidate", move |_rt, _args| {
        if halt {
            match validate_result(qr_v.as_ref(), exp_v.as_ref()) {
                Ok(()) => Ok(Value::Boolean(true)),
                Err(e) => Err(rt_err(e.message().to_string())),
            }
        } else {
            Ok(Value::Boolean(true))
        }
    });

    let qr_t = qr.clone();
    let exp_t = exp.clone();
    let target = native_function(rt, "crizzleTarget", move |rt, _args| match &mode {
        Mode::Halt => Ok(rows_as_objects(rt, qr_t.as_ref())),
        Mode::Sanitize(defaults) => {
            let s = sanitize_result(qr_t.as_ref(), exp_t.as_ref(), defaults)
                .map_err(|e| rt_err(e.message().to_string()))?;
            let rows = rows_as_objects(rt, &s.result);
            let recs = sanitize_records_js(rt, &s.records);
            attach_provenance(rt, &rows, "sanitizations", recs);
            Ok(rows)
        }
        Mode::Propagate => {
            let p = propagate_result(qr_t.as_ref(), exp_t.as_ref())
                .map_err(|e| rt_err(e.message().to_string()))?;
            let rows = rows_as_objects(rt, &p.result);
            let recs = propagate_records_js(rt, &p.records);
            attach_provenance(rt, &rows, "propagations", recs);
            Ok(rows)
        }
    });

    let wrapper = rt.install_boundary_wrapper(target, 1, validator);
    rt.invoke_boundary_wrapper(wrapper, Vec::new())
}

fn contract_expected_sqlite(
    rt: &mut Runtime,
    id: usize,
    arg: Option<&Value>,
) -> Result<Option<Vec<(String, CsType)>>, RuntimeError> {
    match arg {
        Some(Value::String(table)) => {
            let t = table.as_str().to_string();
            let cols = SQLITE_DBS.with(|d| -> Result<Vec<(String, CsType)>, RuntimeError> {
                let mut d = d.borrow_mut();
                let db = d
                    .get_mut(id)
                    .and_then(|o| o.as_mut())
                    .ok_or_else(|| rt_err("SQLite db is closed".into()))?;
                Ok(rusty_sqlite::crizzle::derive_columns(db, &t))
            })?;
            Ok(Some(cols))
        }
        Some(Value::Object(oid)) => Ok(Some(parse_declared(rt, *oid)?)),
        _ => Ok(None),
    }
}

fn boundary_cross_sqlite(
    rt: &mut Runtime,
    expected: Vec<(String, CsType)>,
    rowset: crizzle_core::RowSet,
    mode: Mode,
) -> Result<Value, RuntimeError> {
    let rs = Rc::new(rowset);
    let exp = Rc::new(expected);
    let halt = matches!(mode, Mode::Halt);

    let rs_v = rs.clone();
    let exp_v = exp.clone();
    let validator = native_function(rt, "crizzleValidate", move |_rt, _args| {
        if halt {
            match crizzle_core::validate_result(rs_v.as_ref(), exp_v.as_ref()) {
                Ok(()) => Ok(Value::Boolean(true)),
                Err(e) => Err(rt_err(e.message().to_string())),
            }
        } else {
            Ok(Value::Boolean(true))
        }
    });

    let rs_t = rs.clone();
    let exp_t = exp.clone();
    let target = native_function(rt, "crizzleTarget", move |rt, _args| match &mode {
        Mode::Halt => Ok(rowset_as_objects(rt, rs_t.as_ref())),
        Mode::Sanitize(defaults) => {
            let s = crizzle_core::sanitize_result(rs_t.as_ref(), exp_t.as_ref(), defaults)
                .map_err(|e| rt_err(e.message().to_string()))?;
            let rows = rowset_as_objects(rt, &s.result);
            let recs = sanitize_records_js(rt, &s.records);
            attach_provenance(rt, &rows, "sanitizations", recs);
            Ok(rows)
        }
        Mode::Propagate => {
            let p = crizzle_core::propagate_result(rs_t.as_ref(), exp_t.as_ref())
                .map_err(|e| rt_err(e.message().to_string()))?;
            let rows = rowset_as_objects(rt, &p.result);
            let recs = propagate_records_js(rt, &p.records);
            attach_provenance(rt, &rows, "propagations", recs);
            Ok(rows)
        }
    });

    let wrapper = rt.install_boundary_wrapper(target, 1, validator);
    rt.invoke_boundary_wrapper(wrapper, Vec::new())
}

fn attach_provenance(rt: &mut Runtime, rows: &Value, name: &str, records: Value) {
    if let Value::Object(arr) = rows {
        rt.object_set(*arr, name.into(), records);
    }
}

fn kind_label(kind: ViolationKind) -> &'static str {
    match kind {
        ViolationKind::NullInNonNull => "null-in-non-null",
        ViolationKind::NotInUnion => "not-in-union",
        ViolationKind::TypeMismatch => "type-mismatch",
        ViolationKind::MissingColumn => "missing-column",
        ViolationKind::UnexpectedColumn => "unexpected-column",
        ViolationKind::NoSanitizerDefault => "no-sanitizer-default",
    }
}

fn sanitize_records_js(rt: &mut Runtime, records: &[SanitizationRecord]) -> Value {
    let arr = rt.alloc_object(Object::new_array());
    for (i, r) in records.iter().enumerate() {
        let o = rt.alloc_object(Object::new_ordinary());
        rt.object_set(o, "column".into(), js_string(r.column.clone()));
        rt.object_set(o, "row".into(), Value::Number(r.row_index as f64));
        rt.object_set(o, "expected".into(), js_string(r.expected.render()));
        rt.object_set(o, "received".into(), js_string(r.received.clone()));
        rt.object_set(o, "kind".into(), js_string(kind_label(r.kind)));
        let rep = sql_to_js(rt, &r.replaced_with, 0);
        rt.object_set(o, "replacedWith".into(), rep);
        rt.object_set(arr, i.to_string().into(), Value::Object(o));
    }
    rt.object_set(arr, "length".into(), Value::Number(records.len() as f64));
    Value::Object(arr)
}

fn propagate_records_js(rt: &mut Runtime, records: &[PropagationRecord]) -> Value {
    let arr = rt.alloc_object(Object::new_array());
    for (i, r) in records.iter().enumerate() {
        let o = rt.alloc_object(Object::new_ordinary());
        rt.object_set(o, "column".into(), js_string(r.column.clone()));
        rt.object_set(o, "row".into(), Value::Number(r.row_index as f64));
        rt.object_set(o, "expected".into(), js_string(r.expected.render()));
        rt.object_set(o, "received".into(), js_string(r.received.clone()));
        rt.object_set(o, "kind".into(), js_string(kind_label(r.kind)));
        rt.object_set(arr, i.to_string().into(), Value::Object(o));
    }
    rt.object_set(arr, "length".into(), Value::Number(records.len() as f64));
    Value::Object(arr)
}

fn parse_defaults(rt: &mut Runtime, obj: ObjectRef) -> SanitizeDefaults {
    let mut d = SanitizeDefaults::new();
    if let Some(v) = present(rt, obj, "number") {
        d = d.number(v);
    }
    if let Some(v) = present(rt, obj, "string") {
        d = d.string(v);
    }
    if let Some(v) = present(rt, obj, "boolean") {
        d = d.boolean(v);
    }
    if let Some(v) = present(rt, obj, "bigint") {
        d = d.bigint(v);
    }
    if let Some(v) = present(rt, obj, "Date") {
        d = d.date(v);
    }
    if let Some(v) = present(rt, obj, "bytes") {
        d = d.bytes(v);
    }
    d
}

fn present(rt: &mut Runtime, obj: ObjectRef, key: &str) -> Option<SqlValue> {
    match rt.object_get(obj, key) {
        Value::Undefined => None,
        v => Some(js_to_sql(&v)),
    }
}

fn js_to_sql(v: &Value) -> SqlValue {
    match v {
        Value::Null | Value::Undefined => SqlValue::Null,
        Value::Boolean(b) => SqlValue::Int(*b as i64),
        Value::Number(n) if n.fract() == 0.0 => SqlValue::Int(*n as i64),
        Value::Number(n) => SqlValue::Real(*n),
        Value::String(s) => SqlValue::Text(s.as_str().to_string()),
        Value::BigInt(b) => SqlValue::Int(b.to_u64_wrapping() as i64),
        _ => SqlValue::Null,
    }
}

thread_local! {

    static SQLITE_DBS: std::cell::RefCell<Vec<Option<rusty_sqlite::Database>>> =
        const { std::cell::RefCell::new(Vec::new()) };

    static SQLITE_CURSORS: std::cell::RefCell<Vec<Option<SqliteCursorRecord>>> =
        const { std::cell::RefCell::new(Vec::new()) };
}
const SQLITE_ID_SLOT: &str = "__crizzle_sqlite";
const SQLITE_CURSOR_SLOT: &str = "__crizzle_sqlite_cursor";

struct SqliteCursorRecord {
    dbid: usize,
    expected: Vec<(String, CsType)>,
    session: rusty_sqlite::SelectCursorSession,
}

fn register_sqlite(db: rusty_sqlite::Database) -> usize {
    SQLITE_DBS.with(|d| {
        let mut d = d.borrow_mut();
        d.push(Some(db));
        d.len() - 1
    })
}

fn register_sqlite_cursor(record: SqliteCursorRecord) -> usize {
    SQLITE_CURSORS.with(|c| {
        let mut c = c.borrow_mut();
        c.push(Some(record));
        c.len() - 1
    })
}

fn sqlite_db_id(rt: &mut Runtime, this: ObjectRef) -> Result<usize, RuntimeError> {
    match rt.object_get(this, SQLITE_ID_SLOT) {
        Value::Number(n) => Ok(n as usize),
        _ => Err(rt_err("not a crizzle SQLite db".into())),
    }
}

fn rowset_as_objects(rt: &mut Runtime, rs: &crizzle_core::RowSet) -> Value {
    let arr = rt.alloc_object(Object::new_array());
    {
        let o = rt.obj_mut(arr);
        o.array_dense = true;
        o.dense_elements.reserve(rs.rows.len());
    }
    for row in &rs.rows {
        let obj = rt.alloc_object(Object::new_ordinary());
        for (j, name) in rs.columns.iter().enumerate() {
            let val = row
                .get(j)
                .map(|v| sql_to_js(rt, v, 0))
                .unwrap_or(Value::Null);
            rt.object_set(obj, name.clone(), val);
        }
        rt.obj_mut(arr).dense_elements.push(Value::Object(obj));
    }
    Value::Object(arr)
}

fn sqlite_value_to_sql(v: &rusty_sqlite::Value) -> SqlValue {
    match v {
        rusty_sqlite::Value::Null => SqlValue::Null,
        rusty_sqlite::Value::Int(i) => SqlValue::Int(*i),
        rusty_sqlite::Value::Real(r) => SqlValue::Real(*r),
        rusty_sqlite::Value::Text(s) => SqlValue::Text(s.clone()),
        rusty_sqlite::Value::Blob(b) => SqlValue::Blob(b.clone()),
    }
}

fn sqlite_row_object(rt: &mut Runtime, columns: &[String], row: &[SqlValue]) -> Value {
    let obj = rt.alloc_object(Object::new_ordinary());
    for (j, name) in columns.iter().enumerate() {
        let val = row
            .get(j)
            .map(|v| sql_to_js(rt, v, 0))
            .unwrap_or(Value::Null);
        rt.object_set(obj, name.clone(), val);
    }
    Value::Object(obj)
}

fn iterator_result(rt: &mut Runtime, value: Value, done: bool) -> Value {
    let obj = rt.alloc_object(Object::new_ordinary());
    rt.object_set(obj, "value".into(), value);
    rt.object_set(obj, "done".into(), Value::Boolean(done));
    Value::Object(obj)
}

fn sqlite_cursor_id(rt: &mut Runtime) -> Result<usize, RuntimeError> {
    match rt.current_this() {
        Value::Object(id) => match rt.object_get(id, SQLITE_CURSOR_SLOT) {
            Value::Number(n) => Ok(n as usize),
            _ => Err(rt_err(
                "crizzle SQLite iterator receiver lacks cursor state".into(),
            )),
        },
        _ => Err(rt_err(
            "crizzle SQLite iterator method called without receiver".into(),
        )),
    }
}

fn close_sqlite_cursor(id: usize) {
    SQLITE_CURSORS.with(|c| {
        if let Some(Some(cur)) = c.borrow_mut().get_mut(id) {
            cur.session.close();
        }
    });
}

fn make_sqlite_cursor_iterator(rt: &mut Runtime, id: usize) -> Value {
    let iter = new_object(rt);
    rt.object_set(iter, SQLITE_CURSOR_SLOT.into(), Value::Number(id as f64));
    register_method(rt, iter, "next", |rt, _args| {
        let id = sqlite_cursor_id(rt)?;
        let stepped = SQLITE_CURSORS.with(
            |c| -> Result<
                Option<(Vec<String>, Vec<SqlValue>, Vec<(String, CsType)>)>,
                RuntimeError,
            > {
                let mut cursors = c.borrow_mut();
                let cur = cursors
                    .get_mut(id)
                    .and_then(|o| o.as_mut())
                    .ok_or_else(|| rt_err("crizzle SQLite iterator is closed".into()))?;
                if cur.session.is_closed() {
                    return Ok(None);
                }
                SQLITE_DBS.with(
                    |d| -> Result<
                        Option<(Vec<String>, Vec<SqlValue>, Vec<(String, CsType)>)>,
                        RuntimeError,
                    > {
                        let d = d.borrow();
                        let db = d
                            .get(cur.dbid)
                            .and_then(|o| o.as_ref())
                            .ok_or_else(|| rt_err("SQLite db is closed".into()))?;
                        let row = db.step_select_cursor(&mut cur.session).map_err(rt_err)?;
                        Ok(row.map(|row| {
                            let columns = cur.session.columns().to_vec();
                            let sql_row = row.iter().map(sqlite_value_to_sql).collect();
                            (columns, sql_row, cur.expected.clone())
                        }))
                    },
                )
            },
        )?;
        let Some((columns, row, expected)) = stepped else {
            return Ok(iterator_result(rt, Value::Undefined, true));
        };
        let rs = crizzle_core::RowSet {
            columns: columns.clone(),
            rows: vec![row.clone()],
            col_bool: vec![false; columns.len()],
            col_type_name: vec![String::new(); columns.len()],
        };
        crizzle_core::validate_result(&rs, &expected)
            .map_err(|be| rt_err(be.message().to_string()))?;
        let value = sqlite_row_object(rt, &columns, &row);
        Ok(iterator_result(rt, value, false))
    });
    register_method(rt, iter, "close", |rt, _args| {
        let id = sqlite_cursor_id(rt)?;
        close_sqlite_cursor(id);
        Ok(Value::Undefined)
    });
    register_method(rt, iter, "@@iterator", |rt, _args| Ok(rt.current_this()));
    Value::Object(iter)
}

fn run_builder_sqlite_iter(
    rt: &mut Runtime,
    dbid: usize,
    qid: usize,
) -> Result<Value, RuntimeError> {
    let query = QUERIES
        .with(|qs| qs.borrow().get(qid).and_then(|s| s.as_ref()).cloned())
        .ok_or_else(|| rt_err("crizzle: query already consumed".into()))?;
    if !query.with.is_empty() || !query.joins.is_empty() {
        return Err(rt_err(
            "crizzle SQLite iter() only supports simple base-table SELECTs".into(),
        ));
    }
    let (expected, session) = SQLITE_DBS.with(
        |d| -> Result<(Vec<(String, CsType)>, rusty_sqlite::SelectCursorSession), RuntimeError> {
            let mut d = d.borrow_mut();
            let db = d
                .get_mut(dbid)
                .and_then(|o| o.as_mut())
                .ok_or_else(|| rt_err("SQLite db is closed".into()))?;
            let expected = rusty_sqlite::crizzle::derive_query_type(db, &query);
            let sql = rusty_sqlite::crizzle::lower(&query);
            let st = db.prepare(&sql).map_err(rt_err)?;
            let session = db
                .select_cursor_session(&st, &rusty_sqlite::Bindings::new())
                .map_err(rt_err)?;
            Ok((expected, session))
        },
    )?;
    let id = register_sqlite_cursor(SqliteCursorRecord {
        dbid,
        expected,
        session,
    });
    Ok(make_sqlite_cursor_iterator(rt, id))
}

fn sqlite_dml_err(e: &rusty_sqlite::crizzle::DmlError) -> String {
    match e {
        rusty_sqlite::crizzle::DmlError::Query(s) => s.clone(),
        rusty_sqlite::crizzle::DmlError::Boundary(be) => be.message().to_string(),
    }
}

fn run_builder_sqlite(rt: &mut Runtime, dbid: usize, qid: usize) -> Result<Value, RuntimeError> {
    let query = QUERIES
        .with(|qs| qs.borrow().get(qid).and_then(|s| s.as_ref()).cloned())
        .ok_or_else(|| rt_err("crizzle: query already consumed".into()))?;

    if !query.with.is_empty() {
        return run_with_relations_sqlite(rt, dbid, &query);
    }
    let rs = SQLITE_DBS.with(|d| -> Result<crizzle_core::RowSet, RuntimeError> {
        let mut d = d.borrow_mut();
        let db = d
            .get_mut(dbid)
            .and_then(|o| o.as_mut())
            .ok_or_else(|| rt_err("SQLite db is closed".into()))?;
        let expected = rusty_sqlite::crizzle::derive_query_type(db, &query);
        let rs = rusty_sqlite::crizzle::run_query(db, &query).map_err(rt_err)?;
        crizzle_core::validate_result(&rs, &expected)
            .map_err(|be| rt_err(be.message().to_string()))?;
        Ok(rs)
    })?;
    Ok(rowset_as_objects(rt, &rs))
}

struct SqliteLoadedRel {
    name: String,
    has_many: bool,
    parent_key_idx: Option<usize>,
    child_key_idx: usize,
    child_rs: crizzle_core::RowSet,
}

fn rowset_row_obj(rt: &mut Runtime, rs: &crizzle_core::RowSet, i: usize) -> ObjectRef {
    let obj = rt.alloc_object(Object::new_ordinary());
    if let Some(row) = rs.rows.get(i) {
        for (j, name) in rs.columns.iter().enumerate() {
            let val = row
                .get(j)
                .map(|v| sql_to_js(rt, v, 0))
                .unwrap_or(Value::Null);
            rt.object_set(obj, name.clone(), val);
        }
    }
    obj
}

fn run_with_relations_sqlite(
    rt: &mut Runtime,
    dbid: usize,
    query: &crizzle_core::query::Query,
) -> Result<Value, RuntimeError> {

    let (parent_rs, loaded) = SQLITE_DBS.with(
        |d| -> Result<(crizzle_core::RowSet, Vec<SqliteLoadedRel>), RuntimeError> {
            let mut d = d.borrow_mut();
            let db = d
                .get_mut(dbid)
                .and_then(|o| o.as_mut())
                .ok_or_else(|| rt_err("SQLite db is closed".into()))?;

            let expected = rusty_sqlite::crizzle::derive_query_type(db, query);
            let parent_rs = rusty_sqlite::crizzle::run_query(db, query).map_err(rt_err)?;
            crizzle_core::validate_result(&parent_rs, &expected)
                .map_err(|be| rt_err(be.message().to_string()))?;

            let derived = rusty_sqlite::crizzle::derive_relations(db, &query.from);
            let mut loaded = Vec::new();
            for name in &query.with {
                let Some(rel) = derived.iter().find(|r| &r.name == name) else {
                    continue;
                };
                let local_col = &rel.local_cols[0];
                let parent_key_idx = parent_rs.columns.iter().position(|n| n == local_col);

                let mut keys: Vec<sql_core::SqlValue> = Vec::new();
                if let Some(ki) = parent_key_idx {
                    for row in &parent_rs.rows {
                        if let Some(v) = row.get(ki) {
                            if *v != sql_core::SqlValue::Null && !keys.iter().any(|k| k == v) {
                                keys.push(v.clone());
                            }
                        }
                    }
                }
                let child_q = crizzle_core::query::Query::from(&rel.target)
                    .filter_in(&rel.target_cols[0], keys);
                let child_expected = rusty_sqlite::crizzle::derive_query_type(db, &child_q);
                let child_rs = rusty_sqlite::crizzle::run_query(db, &child_q).map_err(rt_err)?;
                crizzle_core::validate_result(&child_rs, &child_expected)
                    .map_err(|be| rt_err(be.message().to_string()))?;
                let child_key_idx = child_rs
                    .columns
                    .iter()
                    .position(|n| n == &rel.target_cols[0])
                    .unwrap_or(0);
                loaded.push(SqliteLoadedRel {
                    name: rel.name.clone(),
                    has_many: rel.kind == RelKind::HasMany,
                    parent_key_idx,
                    child_key_idx,
                    child_rs,
                });
            }
            Ok((parent_rs, loaded))
        },
    )?;

    let arr = rt.alloc_object(Object::new_array());
    for i in 0..parent_rs.rows.len() {
        let parent = rowset_row_obj(rt, &parent_rs, i);
        for rel in &loaded {

            let key = rel
                .parent_key_idx
                .and_then(|ki| parent_rs.rows[i].get(ki))
                .filter(|v| **v != sql_core::SqlValue::Null)
                .cloned();
            let matches: Vec<usize> = match &key {
                Some(k) => rel
                    .child_rs
                    .rows
                    .iter()
                    .enumerate()
                    .filter(|(_, crow)| crow.get(rel.child_key_idx) == Some(k))
                    .map(|(ci, _)| ci)
                    .collect(),
                None => Vec::new(),
            };
            let field = if rel.has_many {
                let carr = rt.alloc_object(Object::new_array());
                for (k, ci) in matches.iter().enumerate() {
                    let cobj = rowset_row_obj(rt, &rel.child_rs, *ci);
                    rt.object_set(carr, k.to_string(), Value::Object(cobj));
                }
                rt.object_set(carr, "length".into(), Value::Number(matches.len() as f64));
                Value::Object(carr)
            } else {
                match matches.first() {
                    Some(ci) => Value::Object(rowset_row_obj(rt, &rel.child_rs, *ci)),
                    None => Value::Null,
                }
            };
            rt.object_set(parent, rel.name.clone(), field);
        }
        rt.object_set(arr, i.to_string(), Value::Object(parent));
    }
    rt.object_set(
        arr,
        "length".into(),
        Value::Number(parent_rs.rows.len() as f64),
    );
    Ok(Value::Object(arr))
}

fn run_write_sqlite(rt: &mut Runtime, dbid: usize, did: usize) -> Result<Value, RuntimeError> {
    let dml = DMLS
        .with(|d| d.borrow_mut().get_mut(did).and_then(|s| s.take()))
        .ok_or_else(|| rt_err("crizzle: write already run".into()))?;
    let wr = SQLITE_DBS.with(
        |d| -> Result<rusty_sqlite::crizzle::WriteResult, RuntimeError> {
            let mut d = d.borrow_mut();
            let db = d
                .get_mut(dbid)
                .and_then(|o| o.as_mut())
                .ok_or_else(|| rt_err("SQLite db is closed".into()))?;
            let wr = match dml {
                Dml::Ins(i) => rusty_sqlite::crizzle::execute_insert_owned(db, i),
                Dml::Upd(u) => rusty_sqlite::crizzle::execute_update(db, &u),
                Dml::Del(dl) => rusty_sqlite::crizzle::execute_delete(db, &dl),
            }
            .map_err(|e| rt_err(sqlite_dml_err(&e)))?;

            if db.is_file_backed() {
                db.persist().map_err(rt_err)?;
            }
            Ok(wr)
        },
    )?;
    let o = rt.alloc_object(Object::new_ordinary());
    rt.object_set(o, "affected".into(), Value::Number(wr.affected as f64));
    if let Some(rs) = &wr.returned {
        let rows = rowset_as_objects(rt, rs);
        rt.object_set(o, "rows".into(), rows);
    }
    Ok(Value::Object(o))
}

fn build_sqlite_db(rt: &mut Runtime, id: usize) -> Value {
    let db = new_object(rt);
    rt.object_set(db, SQLITE_ID_SLOT.into(), Value::Number(id as f64));

    register_method(rt, db, "exec", |rt, args| {
        let this = this_db(rt)?;
        let id = sqlite_db_id(rt, this)?;
        let sql = arg_sql(args)?;
        SQLITE_DBS.with(|d| {
            let mut d = d.borrow_mut();
            let db = d
                .get_mut(id)
                .and_then(|o| o.as_mut())
                .ok_or_else(|| rt_err("SQLite db is closed".into()))?;
            db.exec_script(&sql).map_err(|e| rt_err(format!("{e:?}")))?;

            if db.is_file_backed() {
                db.persist().map_err(rt_err)?;
            }
            Ok(())
        })?;
        Ok(Value::Undefined)
    });

    register_method(rt, db, "query", |rt, args| {
        let this = this_db(rt)?;
        let id = sqlite_db_id(rt, this)?;
        let sql = arg_sql(args)?;
        let rs = SQLITE_DBS.with(|d| -> Result<crizzle_core::RowSet, RuntimeError> {
            let mut d = d.borrow_mut();
            let db = d
                .get_mut(id)
                .and_then(|o| o.as_mut())
                .ok_or_else(|| rt_err("SQLite db is closed".into()))?;
            rusty_sqlite::crizzle::query_rowset(db, &sql).map_err(rt_err)
        })?;

        match contract_expected_sqlite(rt, id, args.get(1))? {
            Some(expected) => boundary_cross_sqlite(rt, expected, rs, Mode::Halt),
            None => Err(rt_err(
                "query needs a contract (table name or descriptor)".into(),
            )),
        }
    });

    register_method(rt, db, "querySanitize", |rt, args| {
        let this = this_db(rt)?;
        let id = sqlite_db_id(rt, this)?;
        let sql = arg_sql(args)?;

        let expected = contract_expected_sqlite(rt, id, args.get(1))?.ok_or_else(|| {
            rt_err("querySanitize needs a contract (table name or descriptor)".into())
        })?;
        let defaults = match args.get(2) {
            Some(Value::Object(oid)) => parse_defaults(rt, *oid),
            _ => SanitizeDefaults::new(),
        };
        let rs = SQLITE_DBS.with(|d| -> Result<crizzle_core::RowSet, RuntimeError> {
            let mut d = d.borrow_mut();
            let db = d
                .get_mut(id)
                .and_then(|o| o.as_mut())
                .ok_or_else(|| rt_err("SQLite db is closed".into()))?;
            rusty_sqlite::crizzle::query_rowset(db, &sql).map_err(rt_err)
        })?;
        boundary_cross_sqlite(rt, expected, rs, Mode::Sanitize(defaults))
    });

    register_method(rt, db, "queryPropagate", |rt, args| {
        let this = this_db(rt)?;
        let id = sqlite_db_id(rt, this)?;
        let sql = arg_sql(args)?;

        let expected = contract_expected_sqlite(rt, id, args.get(1))?.ok_or_else(|| {
            rt_err("queryPropagate needs a contract (table name or descriptor)".into())
        })?;
        let rs = SQLITE_DBS.with(|d| -> Result<crizzle_core::RowSet, RuntimeError> {
            let mut d = d.borrow_mut();
            let db = d
                .get_mut(id)
                .and_then(|o| o.as_mut())
                .ok_or_else(|| rt_err("SQLite db is closed".into()))?;
            rusty_sqlite::crizzle::query_rowset(db, &sql).map_err(rt_err)
        })?;
        boundary_cross_sqlite(rt, expected, rs, Mode::Propagate)
    });

    register_method(rt, db, "from", |rt, args| {
        let this = this_db(rt)?;
        let dbid = sqlite_db_id(rt, this)?;
        let table = arg_sql(args)?;
        let qid = QUERIES.with(|q| {
            let mut q = q.borrow_mut();
            q.push(Some(Query::from(&table)));
            q.len() - 1
        });
        Ok(build_builder(rt, dbid, qid, true))
    });
    register_method(rt, db, "insertInto", |rt, args| {
        let this = this_db(rt)?;
        let dbid = sqlite_db_id(rt, this)?;
        let t = arg_sql(args)?;
        Ok(build_write_builder(
            rt,
            dbid,
            Dml::Ins(Insert::into(&t)),
            true,
        ))
    });
    register_method(rt, db, "update", |rt, args| {
        let this = this_db(rt)?;
        let dbid = sqlite_db_id(rt, this)?;
        let t = arg_sql(args)?;
        Ok(build_write_builder(
            rt,
            dbid,
            Dml::Upd(Update::table(&t)),
            true,
        ))
    });
    register_method(rt, db, "deleteFrom", |rt, args| {
        let this = this_db(rt)?;
        let dbid = sqlite_db_id(rt, this)?;
        let t = arg_sql(args)?;
        Ok(build_write_builder(
            rt,
            dbid,
            Dml::Del(Delete::table(&t)),
            true,
        ))
    });

    Value::Object(db)
}

pub fn install(rt: &mut Runtime) {
    let ns = new_object(rt);

    let open_pg = make_callable(rt, "openPostgres", |rt, args| {
        let handle = match args.first() {
            Some(Value::String(path))
                if !path.as_str().is_empty() && path.as_str() != ":memory:" =>
            {
                load_catalog_path(PathBuf::from(path.as_str()))?
            }
            _ => PgCatalogHandle {
                catalog: Catalog::new(),
                path: None,
                _lock: None,
            },
        };
        let id = register_catalog(handle);
        Ok(build_db(rt, id))
    });
    rt.object_set(ns, "openPostgres".into(), Value::Object(open_pg));

    let open_sqlite = make_callable(rt, "openSqlite", |rt, args| {
        let db = match args.first() {
            Some(Value::String(path))
                if !path.as_str().is_empty() && path.as_str() != ":memory:" =>
            {
                rusty_sqlite::Database::open_file(path.as_str())
                    .map_err(|e| rt_err(format!("openSqlite: {e}")))?
            }
            _ => rusty_sqlite::Database::open_memory(),
        };
        let id = register_sqlite(db);
        Ok(build_sqlite_db(rt, id))
    });
    rt.object_set(ns, "openSqlite".into(), Value::Object(open_sqlite));
    rt.define_global_property("__crizzle", Value::Object(ns));
}
