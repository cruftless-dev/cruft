
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use rusty_js_runtime::value::{JsString, ObjectRef};
use rusty_js_runtime::{Object, Runtime, RuntimeError, Value};

use crate::register::{make_callable, make_callable_rooted, new_object, register_method};
use rusty_sqlite::{Bindings, Database, Outcome, Value as SqlValue};

thread_local! {

    static DBS: RefCell<Vec<Option<Database>>> = const { RefCell::new(Vec::new()) };

    static RO: RefCell<Vec<bool>> = const { RefCell::new(Vec::new()) };
}

const DB_ID_SLOT: &str = "__cruft_sqlite_db";
const SQL_SLOT: &str = "__cruft_sqlite_sql";
const SAFE_SLOT: &str = "__cruft_sqlite_safe";
const CLASS_SLOT: &str = "__cruft_sqlite_as_class";
const TXN_CB_SLOT: &str = "__cruft_sqlite_txn_cb";
const TXN_DBOBJ_SLOT: &str = "__cruft_sqlite_txn_dbobj";

const STMT_SLOT: &str = "__cruft_sqlite_stmt";

fn rt_err(e: String) -> RuntimeError {

    RuntimeError::TypeError(e)
}

fn enrich_err(rt: &mut Runtime, e: RuntimeError) -> RuntimeError {
    let RuntimeError::TypeError(msg) = &e else {
        return e;
    };
    let Some((tag, real)) = msg.split_once('\x1f') else {
        return e;
    };
    let errno: i64 = match tag {
        "PRIMARYKEY" => 1555,
        "UNIQUE" => 2067,
        "NOTNULL" => 1299,
        "CHECK" => 275,
        "FOREIGNKEY" => 787,
        _ => 19,
    };
    match rusty_js_runtime::intrinsics::make_error_instance(rt, "Error", real) {
        Some(id) => {
            rt.object_set(
                id,
                "code".into(),
                js_string(format!("SQLITE_CONSTRAINT_{tag}")),
            );
            rt.object_set(id, "errno".into(), Value::Number(errno as f64));
            RuntimeError::Thrown(Value::Object(id))
        }
        None => RuntimeError::TypeError(real.to_string()),
    }
}

fn js_string(s: impl Into<String>) -> Value {
    Value::String(Rc::new(JsString::from(s.into())))
}

fn array_like(rt: &mut Runtime, id: ObjectRef) -> Option<usize> {
    match rt.object_get(id, "length") {
        Value::Number(n) if n >= 0.0 => Some(n as usize),
        _ => None,
    }
}

fn js_to_sql(rt: &mut Runtime, v: &Value) -> SqlValue {
    match v {
        Value::Null | Value::Undefined => SqlValue::Null,
        Value::Boolean(b) => SqlValue::Int(*b as i64),
        Value::Number(n) => {
            if n.is_finite() && n.fract() == 0.0 && n.abs() < 9.007_199_254_740_992e15 {
                SqlValue::Int(*n as i64)
            } else {
                SqlValue::Real(*n)
            }
        }
        Value::String(s) => SqlValue::Text(s.as_str().to_string()),

        Value::BigInt(b) => SqlValue::Int(b.to_u64_wrapping() as i64),
        Value::Object(id) => match array_like(rt, *id) {
            Some(len) => {
                let mut bytes = Vec::with_capacity(len);
                for i in 0..len {
                    match rt.object_get(*id, &i.to_string()) {
                        Value::Number(n) => bytes.push(n as u8),
                        _ => bytes.push(0),
                    }
                }
                SqlValue::Blob(bytes)
            }
            None => SqlValue::Null,
        },
        _ => SqlValue::Null,
    }
}

fn sql_to_js(rt: &mut Runtime, v: &SqlValue, safe: bool) -> Value {
    match v {
        SqlValue::Null => Value::Null,
        SqlValue::Int(i) => {
            if safe {
                Value::BigInt(Rc::new(rusty_js_runtime::bigint::JsBigInt::from_i64(*i)))
            } else {
                Value::Number(*i as f64)
            }
        }
        SqlValue::Real(r) => Value::Number(*r),
        SqlValue::Text(s) => js_string(s.clone()),
        SqlValue::Blob(b) => Value::Object(rt.alloc_uint8_array_from_bytes(b)),
    }
}

fn bindings_from_args(rt: &mut Runtime, args: &[Value]) -> Bindings {
    let mut b = Bindings::new();

    if args.len() == 1 {
        if let Value::Object(id) = &args[0] {
            let is_typed = matches!(rt.object_get(*id, "BYTES_PER_ELEMENT"), Value::Number(_));
            match array_like(rt, *id) {
                Some(_) if is_typed => {
                    let sv = js_to_sql(rt, &args[0]);
                    b.positional.push(sv);
                    return b;
                }
                Some(len) => {
                    for i in 0..len {
                        let el = rt.object_get(*id, &i.to_string());
                        let sv = js_to_sql(rt, &el);
                        b.positional.push(sv);
                    }
                    return b;
                }
                None => {
                    let keys = rt
                        .own_enumerable_string_keys_via(&args[0])
                        .ok()
                        .map(|v| collect_keys(rt, &v))
                        .unwrap_or_default();
                    for k in keys {
                        let val = rt.object_get(*id, &k);
                        let name = k.trim_start_matches(['$', ':', '@']).to_string();
                        let sv = js_to_sql(rt, &val);
                        b.named.insert(name, sv);
                    }
                    return b;
                }
            }
        }
    }
    for a in args {
        let sv = js_to_sql(rt, a);
        b.positional.push(sv);
    }
    b
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

fn db_id(rt: &mut Runtime, this: ObjectRef) -> Result<usize, RuntimeError> {
    match rt.object_get(this, DB_ID_SLOT) {
        Value::Number(n) => Ok(n as usize),
        _ => Err(rt_err("not a Database (missing handle)".into())),
    }
}

fn first_keyword(sql: &str) -> String {
    sql.trim_start()
        .split(|c: char| c.is_whitespace() || c == '(' || c == ';')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase()
}

fn is_write_sql(sql: &str) -> bool {
    matches!(
        first_keyword(sql).as_str(),
        "INSERT" | "UPDATE" | "DELETE" | "CREATE" | "DROP" | "ALTER" | "REPLACE" | "TRUNCATE"
    )
}

fn readonly_guard(id: usize, sql: &str) -> Result<(), RuntimeError> {
    let ro = RO.with(|v| v.borrow().get(id).copied().unwrap_or(false));
    if ro && is_write_sql(sql) {
        return Err(rt_err("attempt to write a readonly database".into()));
    }
    Ok(())
}

thread_local! {

    static STMT_CACHE: RefCell<std::collections::HashMap<String, Rc<rusty_sqlite::Statement>>> =
        RefCell::new(std::collections::HashMap::new());
}

fn cached_statement(sql: &str) -> Result<Rc<rusty_sqlite::Statement>, RuntimeError> {
    if let Some(st) = STMT_CACHE.with(|c| c.borrow().get(sql).cloned()) {
        return Ok(st);
    }
    if std::env::var_os("CRUFT_SQL_PLAN").is_some() {
        eprintln!("[sql-plan] parse {sql}");
    }
    let st = Rc::new(rusty_sqlite::parse_statement(sql).map_err(rt_err)?);
    STMT_CACHE.with(|c| c.borrow_mut().insert(sql.to_string(), st.clone()));
    Ok(st)
}

struct StmtEntry {
    st: Rc<rusty_sqlite::Statement>,
    is_write: bool,
    db: usize,
}

thread_local! {
    static STMTS: RefCell<Vec<StmtEntry>> = const { RefCell::new(Vec::new()) };
}

fn register_stmt(st: Rc<rusty_sqlite::Statement>, is_write: bool, db: usize) -> usize {
    STMTS.with(|v| {
        let mut vs = v.borrow_mut();
        vs.push(StmtEntry { st, is_write, db });
        vs.len() - 1
    })
}

fn stmt_entry(handle: usize) -> Option<(Rc<rusty_sqlite::Statement>, bool, usize)> {
    STMTS.with(|v| {
        v.borrow()
            .get(handle)
            .map(|e| (e.st.clone(), e.is_write, e.db))
    })
}

fn run_on_db(
    rt: &mut Runtime,
    id: usize,
    sql: &str,
    b: &Bindings,
) -> Result<Outcome, RuntimeError> {
    let st = cached_statement(sql)?;
    run_stmt(rt, id, &st, is_write_sql(sql), b)
}

fn run_stmt(
    rt: &mut Runtime,
    id: usize,
    st: &Rc<rusty_sqlite::Statement>,
    is_write: bool,
    b: &Bindings,
) -> Result<Outcome, RuntimeError> {
    if is_write && RO.with(|v| v.borrow().get(id).copied().unwrap_or(false)) {
        return Err(rt_err("attempt to write a readonly database".into()));
    }
    DBS.with(|v| {
        let mut dbs = v.borrow_mut();
        let db = dbs
            .get_mut(id)
            .and_then(|s| s.as_mut())
            .ok_or_else(|| rt_err("database is closed".into()))?;
        db.run(st, b).map_err(rt_err)
    })
    .map_err(|e| enrich_err(rt, e))
}

fn exec_on_db(rt: &mut Runtime, id: usize, sql: &str) -> Result<(), RuntimeError> {
    readonly_guard(id, sql)?;
    DBS.with(|v| {
        let mut dbs = v.borrow_mut();
        let db = dbs
            .get_mut(id)
            .and_then(|s| s.as_mut())
            .ok_or_else(|| rt_err("database is closed".into()))?;
        db.exec_script(sql).map_err(rt_err).map(|_| ())
    })
    .map_err(|e| enrich_err(rt, e))
}

fn row_object(
    rt: &mut Runtime,
    columns: &[String],
    row: &[SqlValue],
    safe: bool,
    proto: Option<ObjectRef>,
) -> Value {
    let obj = new_object(rt);
    for (c, val) in columns.iter().zip(row) {
        let jv = sql_to_js(rt, val, safe);
        rt.object_set(obj, c.clone(), jv);
    }
    if let Some(p) = proto {

        let _ = rt.reflect_set_prototype_of_via(&Value::Object(obj), &Value::Object(p));
    }
    Value::Object(obj)
}

fn rows_as_objects(
    rt: &mut Runtime,
    columns: &[String],
    rows: &[Vec<SqlValue>],
    safe: bool,
    proto: Option<ObjectRef>,
) -> Value {
    let arr = rt.alloc_object(Object::new_array());
    for (i, row) in rows.iter().enumerate() {
        let o = row_object(rt, columns, row, safe, proto);
        rt.object_set(arr, i.to_string(), o);
    }
    rt.object_set(arr, "length".into(), Value::Number(rows.len() as f64));
    Value::Object(arr)
}

fn rows_as_arrays(rt: &mut Runtime, rows: &[Vec<SqlValue>], safe: bool) -> Value {
    let arr = rt.alloc_object(Object::new_array());
    for (i, row) in rows.iter().enumerate() {
        let inner = rt.alloc_object(Object::new_array());
        for (j, val) in row.iter().enumerate() {
            let jv = sql_to_js(rt, val, safe);
            rt.object_set(inner, j.to_string(), jv);
        }
        rt.object_set(inner, "length".into(), Value::Number(row.len() as f64));
        rt.object_set(arr, i.to_string(), Value::Object(inner));
    }
    rt.object_set(arr, "length".into(), Value::Number(rows.len() as f64));
    Value::Object(arr)
}

fn string_array(rt: &mut Runtime, items: &[String]) -> Value {
    let arr = rt.alloc_object(Object::new_array());
    for (i, s) in items.iter().enumerate() {
        rt.object_set(arr, i.to_string(), js_string(s.clone()));
    }
    rt.object_set(arr, "length".into(), Value::Number(items.len() as f64));
    Value::Object(arr)
}

fn run_result(rt: &mut Runtime, changes: i64, last: i64) -> Value {
    let o = new_object(rt);
    rt.object_set(o, "changes".into(), Value::Number(changes as f64));
    rt.object_set(o, "lastInsertRowid".into(), Value::Number(last as f64));
    Value::Object(o)
}

fn params_count(sql: &str) -> i64 {
    let bytes = sql.as_bytes();
    let mut i = 0usize;
    let mut anon = 0i64;
    let mut maxn = 0i64;
    let mut named: BTreeSet<String> = BTreeSet::new();
    let mut in_str: u8 = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if in_str != 0 {
            if c == in_str {
                in_str = 0;
            }
            i += 1;
            continue;
        }
        match c {
            b'\'' | b'"' => in_str = c,
            b'?' => {
                let mut j = i + 1;
                let mut num = String::new();
                while j < bytes.len() && bytes[j].is_ascii_digit() {
                    num.push(bytes[j] as char);
                    j += 1;
                }
                if num.is_empty() {
                    anon += 1;
                } else {
                    let n: i64 = num.parse().unwrap_or(0);
                    if n > maxn {
                        maxn = n;
                    }
                    i = j;
                    continue;
                }
            }
            b':' | b'@' | b'$' => {
                let mut j = i + 1;
                let mut name = String::new();
                while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                    name.push(bytes[j] as char);
                    j += 1;
                }
                if !name.is_empty() {
                    named.insert(name);
                    i = j;
                    continue;
                }
            }
            _ => {}
        }
        i += 1;
    }
    std::cmp::max(anon + maxn, maxn) + named.len() as i64
}

fn column_names(rt: &mut Runtime, id: usize, sql: &str) -> Option<Vec<String>> {
    let kw = first_keyword(sql);
    if kw != "SELECT" && kw != "WITH" {
        return None;
    }
    let mut b = Bindings::new();
    for _ in 0..64 {
        b.positional.push(SqlValue::Null);
    }
    match run_on_db(rt, id, sql, &b) {
        Ok(Outcome::Rows { columns, .. }) => Some(columns),
        _ => None,
    }
}

fn make_statement(rt: &mut Runtime, id: usize, sql: &str, safe_default: bool) -> Value {
    let st = new_object(rt);
    rt.set_engine_sentinel(st, DB_ID_SLOT, Value::Number(id as f64));
    rt.set_engine_sentinel(st, SQL_SLOT, js_string(sql));
    rt.set_engine_sentinel(st, SAFE_SLOT, Value::Boolean(safe_default));

    if let Ok(parsed) = cached_statement(sql) {
        let handle = register_stmt(parsed, is_write_sql(sql), id);
        rt.set_engine_sentinel(st, STMT_SLOT, Value::Number(handle as f64));
    }

    rt.object_set(
        st,
        "paramsCount".into(),
        Value::Number(params_count(sql) as f64),
    );
    let cols = column_names(rt, id, sql);
    let cols_val = match &cols {
        Some(c) => string_array(rt, c),
        None => Value::Null,
    };
    rt.object_set(st, "columnNames".into(), cols_val);

    fn ctx(
        rt: &mut Runtime,
    ) -> Result<(usize, Rc<rusty_sqlite::Statement>, bool, ObjectRef), RuntimeError> {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Err(rt_err("Statement method called without a statement".into())),
        };

        if let Value::Number(h) = rt.object_get(this, STMT_SLOT) {
            if let Some((st, is_write, id)) = stmt_entry(h as usize) {
                return Ok((id, st, is_write, this));
            }
        }

        let id = db_id(rt, this)?;
        let sql = match rt.object_get(this, SQL_SLOT) {
            Value::String(s) => s.as_str().to_string(),
            _ => return Err(rt_err("statement missing sql".into())),
        };
        let st = cached_statement(&sql)?;
        Ok((id, st, is_write_sql(&sql), this))
    }

    fn flags(rt: &mut Runtime, this: ObjectRef) -> (bool, Option<ObjectRef>) {
        let safe = matches!(rt.object_get(this, SAFE_SLOT), Value::Boolean(true));
        let proto = match rt.object_get(this, CLASS_SLOT) {
            Value::Object(cls) => match rt.object_get(cls, "prototype") {
                Value::Object(p) => Some(p),
                _ => None,
            },
            _ => None,
        };
        (safe, proto)
    }

    register_method(rt, st, "all", |rt, args| {
        let (id, st, is_write, this) = ctx(rt)?;
        let (safe, proto) = flags(rt, this);
        let b = bindings_from_args(rt, args);
        match run_stmt(rt, id, &st, is_write, &b)? {
            Outcome::Rows { columns, rows } => {
                Ok(rows_as_objects(rt, &columns, &rows, safe, proto))
            }
            Outcome::Mutation { .. } => Ok(rows_as_objects(rt, &[], &[], safe, proto)),
        }
    });
    register_method(rt, st, "get", |rt, args| {
        let (id, st, is_write, this) = ctx(rt)?;
        let (safe, proto) = flags(rt, this);
        let b = bindings_from_args(rt, args);
        match run_stmt(rt, id, &st, is_write, &b)? {
            Outcome::Rows { columns, rows } => Ok(rows
                .first()
                .map(|r| row_object(rt, &columns, r, safe, proto))
                .unwrap_or(Value::Undefined)),
            Outcome::Mutation { .. } => Ok(Value::Undefined),
        }
    });
    register_method(rt, st, "values", |rt, args| {
        let (id, st, is_write, this) = ctx(rt)?;
        let (safe, _proto) = flags(rt, this);
        let b = bindings_from_args(rt, args);
        match run_stmt(rt, id, &st, is_write, &b)? {
            Outcome::Rows { rows, .. } => Ok(rows_as_arrays(rt, &rows, safe)),
            Outcome::Mutation { .. } => Ok(rows_as_arrays(rt, &[], safe)),
        }
    });
    register_method(rt, st, "run", |rt, args| {
        let (id, st, is_write, _this) = ctx(rt)?;
        let b = bindings_from_args(rt, args);
        match run_stmt(rt, id, &st, is_write, &b)? {
            Outcome::Mutation {
                changes,
                last_insert_rowid,
            } => Ok(run_result(rt, changes, last_insert_rowid)),
            Outcome::Rows { .. } => Ok(run_result(rt, 0, 0)),
        }
    });

    register_method(rt, st, "iterate", |rt, args| {
        let (id, st, is_write, this) = ctx(rt)?;
        let (safe, proto) = flags(rt, this);
        let b = bindings_from_args(rt, args);
        match run_stmt(rt, id, &st, is_write, &b)? {
            Outcome::Rows { columns, rows } => {
                Ok(rows_as_objects(rt, &columns, &rows, safe, proto))
            }
            Outcome::Mutation { .. } => Ok(rows_as_objects(rt, &[], &[], safe, proto)),
        }
    });

    register_method(rt, st, "safeIntegers", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Err(rt_err("safeIntegers called without a statement".into())),
        };
        let on = match args.first() {
            Some(Value::Boolean(b)) => *b,
            Some(Value::Undefined) | None => true,
            Some(v) => rusty_js_runtime::abstract_ops::to_boolean(v),
        };
        rt.set_engine_sentinel(this, SAFE_SLOT, Value::Boolean(on));
        Ok(Value::Object(this))
    });

    register_method(rt, st, "setReadBigInts", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Err(rt_err("setReadBigInts called without a statement".into())),
        };
        let on = match args.first() {
            Some(Value::Boolean(b)) => *b,
            _ => {
                let msg = "The \"readBigInts\" argument must be a boolean.";
                let err =
                    match rusty_js_runtime::intrinsics::make_error_instance(rt, "TypeError", msg) {
                        Some(id) => {
                            rt.object_set(id, "code".into(), js_string("ERR_INVALID_ARG_TYPE"));
                            Value::Object(id)
                        }
                        None => return Err(RuntimeError::TypeError(msg.to_string())),
                    };
                return Err(RuntimeError::Thrown(err));
            }
        };
        rt.set_engine_sentinel(this, SAFE_SLOT, Value::Boolean(on));
        Ok(Value::Undefined)
    });

    register_method(rt, st, "as", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Err(rt_err("as() called without a statement".into())),
        };
        match args.first() {
            Some(Value::Object(cls)) => {
                rt.set_engine_sentinel(this, CLASS_SLOT, Value::Object(*cls));
            }
            _ => {
                rt.set_engine_sentinel(this, CLASS_SLOT, Value::Null);
            }
        }
        Ok(Value::Object(this))
    });
    register_method(rt, st, "finalize", |rt, _a| Ok(rt.current_this()));

    Value::Object(st)
}

fn execute_txn(
    rt: &mut Runtime,
    id: usize,
    db_obj: ObjectRef,
    cb: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    exec_on_db(rt, id, "BEGIN")?;
    rt.object_set(db_obj, "inTransaction".into(), Value::Boolean(true));
    let result = rt.call_function(cb, Value::Undefined, args.to_vec());
    match result {
        Ok(v) => {
            let commit = exec_on_db(rt, id, "COMMIT");
            rt.object_set(db_obj, "inTransaction".into(), Value::Boolean(false));
            commit?;
            Ok(v)
        }
        Err(e) => {
            let _ = exec_on_db(rt, id, "ROLLBACK");
            rt.object_set(db_obj, "inTransaction".into(), Value::Boolean(false));
            Err(e)
        }
    }
}

fn txn_ctx(rt: &mut Runtime, this: ObjectRef) -> Result<(usize, ObjectRef, Value), RuntimeError> {
    let id = db_id(rt, this)?;
    let db_obj = match rt.object_get(this, TXN_DBOBJ_SLOT) {
        Value::Object(o) => o,
        _ => return Err(rt_err("transaction missing database handle".into())),
    };
    let cb = rt.object_get(this, TXN_CB_SLOT);
    Ok((id, db_obj, cb))
}

fn build_bun_db(rt: &mut Runtime, id: usize) -> Value {
    let db = new_object(rt);
    rt.set_engine_sentinel(db, DB_ID_SLOT, Value::Number(id as f64));
    rt.set_engine_sentinel(db, SAFE_SLOT, Value::Boolean(false));

    rt.object_set(db, "inTransaction".into(), Value::Boolean(false));

    fn db_safe(rt: &mut Runtime, this: ObjectRef) -> bool {
        matches!(rt.object_get(this, SAFE_SLOT), Value::Boolean(true))
    }

    register_method(rt, db, "query", |rt, args| {
        let this = self_obj(rt)?;
        let id = db_id(rt, this)?;
        let sql = arg_sql(args)?;
        let safe = db_safe(rt, this);
        Ok(make_statement(rt, id, &sql, safe))
    });
    register_method(rt, db, "prepare", |rt, args| {
        let this = self_obj(rt)?;
        let id = db_id(rt, this)?;
        let sql = arg_sql(args)?;
        let safe = db_safe(rt, this);
        Ok(make_statement(rt, id, &sql, safe))
    });
    register_method(rt, db, "run", |rt, args| {
        let this = self_id(rt)?;
        let sql = arg_sql(args)?;
        let b = bindings_from_args(rt, &args[1..]);
        match run_on_db(rt, this, &sql, &b)? {
            Outcome::Mutation {
                changes,
                last_insert_rowid,
            } => Ok(run_result(rt, changes, last_insert_rowid)),
            Outcome::Rows { .. } => Ok(run_result(rt, 0, 0)),
        }
    });
    register_method(rt, db, "exec", |rt, args| {
        let this = self_id(rt)?;
        let sql = arg_sql(args)?;
        exec_on_db(rt, this, &sql)?;
        Ok(Value::Undefined)
    });

    register_method(rt, db, "safeIntegers", |rt, args| {
        let this = self_obj(rt)?;
        let on = match args.first() {
            Some(Value::Boolean(b)) => *b,
            Some(Value::Undefined) | None => true,
            Some(v) => rusty_js_runtime::abstract_ops::to_boolean(v),
        };
        rt.set_engine_sentinel(this, SAFE_SLOT, Value::Boolean(on));
        Ok(Value::Object(this))
    });

    register_method(rt, db, "transaction", |rt, args| {
        let this = self_obj(rt)?;
        let id = db_id(rt, this)?;
        let cb = args.first().cloned().unwrap_or(Value::Undefined);
        let mut roots = vec![this];
        if let Value::Object(o) = &cb {
            roots.push(*o);
        }
        let cb_for_base = cb.clone();
        let wrapped = make_callable_rooted(rt, "transaction", roots, move |rt, call_args| {
            execute_txn(rt, id, this, cb_for_base.clone(), call_args)
        });
        rt.set_engine_sentinel(wrapped, DB_ID_SLOT, Value::Number(id as f64));
        rt.set_engine_sentinel(wrapped, TXN_DBOBJ_SLOT, Value::Object(this));
        rt.set_engine_sentinel(wrapped, TXN_CB_SLOT, cb);
        for variant in ["deferred", "immediate", "exclusive"] {
            register_method(rt, wrapped, variant, |rt, call_args| {
                let this = match rt.current_this() {
                    Value::Object(o) => o,
                    _ => {
                        return Err(rt_err(
                            "transaction variant called without a function".into(),
                        ))
                    }
                };
                let (id, db_obj, cb) = txn_ctx(rt, this)?;
                execute_txn(rt, id, db_obj, cb, call_args)
            });
        }
        Ok(Value::Object(wrapped))
    });

    register_method(rt, db, "serialize", |rt, _a| {
        let this = self_obj(rt)?;
        let id = db_id(rt, this)?;
        let bytes = DBS
            .with(|v| {
                v.borrow()
                    .get(id)
                    .and_then(|s| s.as_ref())
                    .map(|d| d.serialize_bytes())
            })
            .ok_or_else(|| rt_err("database is closed".into()))?;
        Ok(Value::Object(rt.alloc_uint8_array_from_bytes(&bytes)))
    });

    register_method(rt, db, "close", |rt, _a| {
        let this = self_id(rt)?;
        DBS.with(|v| {
            if let Some(slot) = v.borrow_mut().get_mut(this) {
                if let Some(db) = slot.as_ref() {
                    let _ = db.persist();
                }
                *slot = None;
            }
        });
        Ok(Value::Undefined)
    });

    Value::Object(db)
}

fn register_db(db: Database) -> usize {
    DBS.with(|v| {
        let mut dbs = v.borrow_mut();
        dbs.push(Some(db));
        let id = dbs.len() - 1;
        RO.with(|r| {
            let mut ro = r.borrow_mut();
            while ro.len() <= id {
                ro.push(false);
            }
        });
        id
    })
}

fn self_obj(rt: &mut Runtime) -> Result<ObjectRef, RuntimeError> {
    match rt.current_this() {
        Value::Object(id) => Ok(id),
        _ => Err(rt_err("Database method called without a database".into())),
    }
}

fn self_id(rt: &mut Runtime) -> Result<usize, RuntimeError> {
    match rt.current_this() {
        Value::Object(id) => db_id(rt, id),
        _ => Err(rt_err("Database method called without a database".into())),
    }
}

fn arg_sql(args: &[Value]) -> Result<String, RuntimeError> {
    match args.first() {
        Some(Value::String(s)) => Ok(s.as_str().to_string()),
        _ => Err(rt_err("expected a SQL string".into())),
    }
}

fn open_new(rt: &mut Runtime, args: &[Value]) -> Result<usize, RuntimeError> {
    let filename = match args.first() {
        Some(Value::String(s)) => s.as_str().to_string(),
        _ => ":memory:".to_string(),
    };
    let readonly = match args.get(1) {
        Some(Value::Object(o)) => matches!(rt.object_get(*o, "readonly"), Value::Boolean(true)),
        _ => false,
    };
    let db = if filename.is_empty() || filename == ":memory:" {
        Database::open_memory()
    } else {
        Database::open_file(&filename).map_err(rt_err)?
    };
    Ok(DBS.with(|v| {
        let mut dbs = v.borrow_mut();
        dbs.push(Some(db));
        let id = dbs.len() - 1;
        RO.with(|r| {
            let mut ro = r.borrow_mut();
            while ro.len() <= id {
                ro.push(false);
            }
            ro[id] = readonly;
        });
        id
    }))
}

fn build_cruft_db(rt: &mut Runtime, id: usize) -> Value {
    let db = new_object(rt);
    rt.set_engine_sentinel(db, DB_ID_SLOT, Value::Number(id as f64));

    register_method(rt, db, "query", |rt, args| {
        let this = self_id(rt)?;
        let sql = arg_sql(args)?;
        let b = bindings_from_args(rt, if args.len() > 1 { &args[1..] } else { &[] });
        match run_on_db(rt, this, &sql, &b)? {
            Outcome::Rows { columns, rows } => {
                Ok(rows_as_objects(rt, &columns, &rows, false, None))
            }
            Outcome::Mutation { .. } => Ok(rows_as_objects(rt, &[], &[], false, None)),
        }
    });

    register_method(rt, db, "run", |rt, args| {
        let this = self_id(rt)?;
        let sql = arg_sql(args)?;
        let b = bindings_from_args(rt, if args.len() > 1 { &args[1..] } else { &[] });
        match run_on_db(rt, this, &sql, &b)? {
            Outcome::Mutation {
                changes,
                last_insert_rowid,
            } => Ok(run_result(rt, changes, last_insert_rowid)),
            Outcome::Rows { columns, rows } => {
                Ok(rows_as_objects(rt, &columns, &rows, false, None))
            }
        }
    });
    register_method(rt, db, "close", |rt, _a| {
        let this = self_id(rt)?;
        DBS.with(|v| {
            if let Some(slot) = v.borrow_mut().get_mut(this) {
                if let Some(db) = slot.as_ref() {
                    let _ = db.persist();
                }
                *slot = None;
            }
        });
        Ok(Value::Undefined)
    });
    Value::Object(db)
}

pub fn install(rt: &mut Runtime) {

    let bun_ns = new_object(rt);
    let ctor = make_callable(rt, "Database", |rt, args| {
        let id = open_new(rt, args)?;
        Ok(build_bun_db(rt, id))
    });

    let deser = make_callable(rt, "deserialize", |rt, args| {
        let bytes = match args.first() {
            Some(v @ Value::Object(_)) => match js_to_sql(rt, v) {
                SqlValue::Blob(b) => b,
                _ => return Err(rt_err("deserialize expects a Uint8Array".into())),
            },
            _ => return Err(rt_err("deserialize expects a Uint8Array".into())),
        };
        let db = Database::deserialize_bytes(&bytes).map_err(rt_err)?;
        let id = register_db(db);
        Ok(build_bun_db(rt, id))
    });
    rt.object_set(ctor, "deserialize".into(), Value::Object(deser));
    rt.object_set(bun_ns, "Database".into(), Value::Object(ctor));
    rt.object_set(bun_ns, "default".into(), Value::Object(ctor));
    rt.define_global_property("__bun_sqlite", Value::Object(bun_ns));

    let node_ns = new_object(rt);
    let db_sync = make_callable(rt, "DatabaseSync", |rt, args| {
        let id = open_new(rt, args)?;
        Ok(build_bun_db(rt, id))
    });
    rt.object_set(node_ns, "DatabaseSync".into(), Value::Object(db_sync));

    let stmt_sync = make_callable(rt, "StatementSync", |_rt, _a| {
        Err(rt_err(
            "StatementSync is not directly constructible; use DatabaseSync.prepare()".into(),
        ))
    });
    rt.object_set(node_ns, "StatementSync".into(), Value::Object(stmt_sync));
    rt.define_global_property("__node_sqlite", Value::Object(node_ns));

    let cruft_ns = new_object(rt);
    let open = make_callable(rt, "open", |rt, args| {
        let id = open_new(rt, args)?;
        Ok(build_cruft_db(rt, id))
    });
    rt.object_set(cruft_ns, "open".into(), Value::Object(open));
    rt.define_global_property("__cruft_sqlite", Value::Object(cruft_ns));
}
