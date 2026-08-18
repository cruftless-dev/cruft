
use crate::register::{
    arg_string, make_callable, make_callable_rooted, new_object, register_method,
    register_method_internal,
};

#[cfg(unix)]
#[allow(unused_imports)]
use libc::{gid_t, mode_t, uid_t};
#[cfg(not(unix))]
#[allow(non_camel_case_types)]
type mode_t = u32;
#[cfg(not(unix))]
#[allow(non_camel_case_types)]
type uid_t = u32;
#[cfg(not(unix))]
#[allow(non_camel_case_types)]
type gid_t = u32;

fn arg_path_or_url(rt: &mut rusty_js_runtime::Runtime, args: &[Value], i: usize) -> String {
    if let Some(Value::Object(id)) = args.get(i) {

        for slot in ["__url_href__", "href"] {
            if let Value::String(href) = rt.object_get(*id, slot) {
                if let Some(rest) = href.strip_prefix("file://") {

                    let rest = rest.split_once(['?', '#']).map(|(p, _)| p).unwrap_or(rest);
                    return percent_decode_path(rest);
                }
            }
        }
    }
    arg_string(args, i)
}

fn percent_decode_path(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = [bytes[i + 1], bytes[i + 2]];
            if hex.iter().all(u8::is_ascii_hexdigit) {
                let hv = |c: u8| (c as char).to_digit(16).unwrap() as u8;
                out.push(hv(hex[0]) * 16 + hv(hex[1]));
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
use rusty_js_runtime::caps;
use rusty_js_runtime::caps::{ModuleId, ModuleProvenance};
use rusty_js_runtime::promise::{new_promise, reject_promise, resolve_promise};
use rusty_js_runtime::value::{Object, ObjectRef};
use rusty_js_runtime::{AgentId, HostEnqueuePhase, HostHook, Runtime, RuntimeError, Value};
#[cfg(unix)]
use std::ffi::CString;

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
))]
unsafe extern "C" {
    fn lchmod(path: *const libc::c_char, mode: mode_t) -> libc::c_int;
}

#[cfg(unix)]
fn seconds_to_timespec(seconds: f64) -> libc::timespec {
    let clamped = seconds.max(0.0);
    let secs = clamped.trunc();
    let nanos = ((clamped - secs) * 1_000_000_000.0).round();
    libc::timespec {
        tv_sec: secs as libc::time_t,
        tv_nsec: nanos.min(999_999_999.0) as libc::c_long,
    }
}

fn check_fs(rt: &Runtime, op: caps::FsOp) -> Result<(), RuntimeError> {
    let url = rt.current_module_url.last().cloned().unwrap_or_default();
    let provenance = if url.contains("/node_modules/") {
        ModuleProvenance::Dependency
    } else if url.starts_with("node:") {
        ModuleProvenance::Builtin
    } else {
        ModuleProvenance::Application
    };
    let caller = ModuleId { url, provenance };
    rt.caps
        .require_fs(&caps::Fs::none(), op, &caller)
        .map_err(|e| RuntimeError::TypeError(e.to_string()))
}
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

fn fs_completion_trace_enabled() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("CRUFT_FS_COMPLETION_TRACE")
            .or_else(|_| std::env::var("CRUFT_PROFILE_FS_COMPLETION"))
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn fs_trace_path(path: &str) -> String {
    const MAX: usize = 180;
    if path.len() <= MAX {
        path.to_string()
    } else {
        format!("...{}", &path[path.len() - MAX..])
    }
}

fn trace_fs_completion(phase: &str, id: u64, path: &str, detail: impl AsRef<str>) {
    if !fs_completion_trace_enabled() {
        return;
    }
    eprintln!(
        "[fs-completion-trace] op=readFile phase={phase} id={id} path={} {}",
        fs_trace_path(path),
        detail.as_ref()
    );
}

fn poll_io_trace_enabled() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("CRUFT_POLL_IO_TRACE")
            .or_else(|_| std::env::var("CRUFT_PROFILE_POLL_IO"))
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn trace_poll_io(stage: &str, detail: impl AsRef<str>) {
    if !poll_io_trace_enabled() {
        return;
    }
    eprintln!("[poll-io-trace] stage={stage} {}", detail.as_ref());
}

enum FsOp {
    Read {
        path: String,
        encoding: Option<String>,
    },
    ReadFd {
        fd: i32,
        encoding: Option<String>,
    },
    Write {
        path: String,
        data: Vec<u8>,
    },
    ReadDir {
        path: String,
        with_file_types: bool,
        recursive: bool,
    },
    Exists {
        path: String,
    },
}

struct PendingFsOp {
    id: u64,
    promise: ObjectRef,
    op: FsOp,
    callback: Option<Value>,
}

thread_local! {
    static PENDING: RefCell<Vec<PendingFsOp>> = RefCell::new(Vec::new());
    static NEXT_PENDING_ID: RefCell<u64> = RefCell::new(1);

    static WATCHERS: RefCell<Vec<WatchEntry>> = RefCell::new(Vec::new());
    static NEXT_WATCH_ID: RefCell<u64> = RefCell::new(1);
    static NEXT_WATCH_EVENT_ID: RefCell<u64> = RefCell::new(1);
}

struct WatchEntry {
    agent_id: AgentId,
    id: u64,
    path: String,
    listener: Option<Value>,

    last_mtime: Option<f64>,

    last_size: Option<u64>,

    watcher_obj: ObjectRef,

    interval_ms: u64,
    last_polled: std::time::Instant,
}

fn watch_root_key(id: u64) -> String {
    format!("fs:watch:{id}")
}

fn watch_event_root_key(id: u64) -> String {
    format!("fs:watch:event:{id}")
}

fn register_watcher(
    rt: &mut Runtime,
    path: String,
    listener: Option<Value>,
    watcher_obj: ObjectRef,
    interval_ms: u64,
) -> u64 {
    let id = NEXT_WATCH_ID.with(|c| {
        let mut c = c.borrow_mut();
        let id = *c;
        *c += 1;
        id
    });
    let (last_mtime, last_size) = mtime_size(&path);
    let mut roots = vec![Value::Object(watcher_obj)];
    if let Some(cb) = listener.clone() {
        roots.push(cb);
    }
    rt.retain_host_roots(watch_root_key(id), roots);
    WATCHERS.with(|w| {
        w.borrow_mut().push(WatchEntry {
            agent_id: rt.agent_id(),
            id,
            path,
            listener,
            last_mtime,
            last_size,
            watcher_obj,
            interval_ms,
            last_polled: std::time::Instant::now(),
        });
    });
    id
}

fn unregister_watcher(rt: &mut Runtime, id: u64) {
    let agent_id = rt.agent_id();
    let removed = WATCHERS.with(|w| {
        let mut w = w.borrow_mut();
        let before = w.len();
        w.retain(|e| e.id != id || e.agent_id != agent_id);
        w.len() != before
    });
    if removed {
        rt.release_host_roots(&watch_root_key(id));
    }
}

fn unregister_watchers_by_path(rt: &mut Runtime, path: &str) {
    let agent_id = rt.agent_id();
    let removed: Vec<u64> = WATCHERS.with(|w| {
        let mut w = w.borrow_mut();
        let mut removed = Vec::new();
        w.retain(|e| {
            let keep = e.path != path || e.agent_id != agent_id;
            if !keep {
                removed.push(e.id);
            }
            keep
        });
        removed
    });
    for id in removed {
        rt.release_host_roots(&watch_root_key(id));
    }
}

fn mtime_size(path: &str) -> (Option<f64>, Option<u64>) {
    match std::fs::metadata(path) {
        Ok(md) => {
            let mt = md
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs_f64());
            (mt, Some(md.len()))
        }
        Err(_) => (None, None),
    }
}

fn pending_root_key(id: u64) -> String {
    format!("fs:pending:{id}")
}

fn push_pending(rt: &mut Runtime, promise: ObjectRef, op: FsOp, callback: Option<Value>) {
    let id = NEXT_PENDING_ID.with(|c| {
        let mut c = c.borrow_mut();
        let id = *c;
        *c += 1;
        id
    });
    let mut roots = vec![Value::Object(promise)];
    if let Some(cb) = callback.clone() {
        roots.push(cb);
    }
    rt.retain_host_roots(pending_root_key(id), roots);
    PENDING.with(|q| {
        q.borrow_mut().push(PendingFsOp {
            id,
            promise,
            op,
            callback,
        })
    });
}

fn drain_pending() -> Vec<PendingFsOp> {
    PENDING.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

fn trailing_callback(rt: &Runtime, args: &[Value]) -> Option<Value> {
    args.last().cloned().filter(|v| rt.is_callable(v))
}

fn fh_current_fd(rt: &mut Runtime) -> Value {
    match rt.current_this() {
        Value::Object(this) => rt.object_get(this, "__cruft_fd"),
        _ => Value::Undefined,
    }
}

fn fh_call_fs_sync(
    rt: &mut Runtime,
    name: &str,
    mut call_args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let fd = fh_current_fd(rt);
    let fs_global = match rt.global_get("fs") {
        Value::Object(id) => id,
        _ => return Ok(Value::Undefined),
    };
    let f = rt.object_get(fs_global, name);
    let mut a = vec![fd];
    a.append(&mut call_args);
    rt.call_function(f, Value::Object(fs_global), a)
}

fn fh_settle(rt: &mut Runtime, p: ObjectRef, r: Result<Value, RuntimeError>) {
    match r {
        Ok(v) => resolve_promise(rt, p, v),
        Err(RuntimeError::Thrown(v)) => reject_promise(rt, p, v),
        Err(e) => {
            let msg = match &e {
                RuntimeError::TypeError(m)
                | RuntimeError::RangeError(m)
                | RuntimeError::ReferenceError(m) => m.clone(),
                _ => format!("{e:?}"),
            };
            reject_promise(
                rt,
                p,
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(msg))),
            );
        }
    }
}

fn install_filehandle_methods(rt: &mut Runtime, handle: ObjectRef) {

    for (method, sync_fn) in [
        ("truncate", "ftruncateSync"),
        ("chmod", "fchmodSync"),
        ("chown", "fchownSync"),
        ("sync", "fsyncSync"),
        ("datasync", "fdatasyncSync"),
        ("utimes", "futimesSync"),
    ] {
        let sync_fn = sync_fn.to_string();
        register_method(rt, handle, method, move |rt, args| {
            let p = new_promise(rt);
            let r = fh_call_fs_sync(rt, &sync_fn, args.to_vec());
            fh_settle(rt, p, r);
            Ok(Value::Object(p))
        });
    }

    for (method, sync_fn, count_key) in [
        ("read", "readSync", "bytesRead"),
        ("write", "writeSync", "bytesWritten"),
    ] {
        let sync_fn = sync_fn.to_string();
        let count_key = count_key.to_string();
        register_method(rt, handle, method, move |rt, args| {
            let p = new_promise(rt);
            let buffer = args.first().cloned().unwrap_or(Value::Undefined);
            let r = fh_call_fs_sync(rt, &sync_fn, args.to_vec()).map(|count| {
                let out = new_object(rt);
                rt.object_set(out, count_key.clone().into(), count);
                rt.object_set(out, "buffer".into(), buffer.clone());
                Value::Object(out)
            });
            fh_settle(rt, p, r);
            Ok(Value::Object(p))
        });
    }

    register_method(rt, handle, "createReadStream", |rt, args| {
        let path = match rt.current_this() {
            Value::Object(this) => match rt.object_get(this, "__cruft_fh_path") {
                Value::String(s) => s.as_str().to_string(),
                _ => String::new(),
            },
            _ => String::new(),
        };
        Ok(Value::Object(make_read_stream(
            rt,
            path,
            args.first().cloned(),
        )))
    });
    register_method(rt, handle, "createWriteStream", |rt, args| {
        let path = match rt.current_this() {
            Value::Object(this) => match rt.object_get(this, "__cruft_fh_path") {
                Value::String(s) => s.as_str().to_string(),
                _ => String::new(),
            },
            _ => String::new(),
        };
        Ok(Value::Object(make_write_stream(
            rt,
            path,
            args.first().cloned(),
        )))
    });

    register_method(rt, handle, "readLines", |rt, args| {
        let path = match rt.current_this() {
            Value::Object(this) => match rt.object_get(this, "__cruft_fh_path") {
                Value::String(s) => s.as_str().to_string(),
                _ => String::new(),
            },
            _ => String::new(),
        };
        let stream = make_read_stream(rt, path, args.first().cloned());
        let opts = new_object(rt);
        rt.object_set(opts, "input".into(), Value::Object(stream));
        rt.object_set(opts, "crlfDelay".into(), Value::Number(f64::INFINITY));

        rt.materialize_lazy_host_module("readline");
        let readline = rt.global_get("readline");
        if let Value::Object(rl) = &readline {
            let create_interface = rt.object_get(*rl, "createInterface");
            if rt.is_callable(&create_interface) {
                return rt.call_function(create_interface, readline, vec![Value::Object(opts)]);
            }
        }
        Ok(Value::Undefined)
    });

    register_method(rt, handle, "readFile", |rt, args| {
        let p = new_promise(rt);
        let r = (|| -> Result<Value, RuntimeError> {
            let fd = fh_current_fd(rt);
            let fs_global = match rt.global_get("fs") {
                Value::Object(id) => id,
                _ => return Ok(Value::Undefined),
            };
            let fstat = rt.object_get(fs_global, "fstatSync");
            let stats = rt.call_function(fstat, Value::Object(fs_global), vec![fd.clone()])?;
            let size = match &stats {
                Value::Object(id) => match rt.object_get(*id, "size") {
                    Value::Number(n) => n,
                    _ => 0.0,
                },
                _ => 0.0,
            };
            let buffer_ctor = rt.global_get("Buffer");
            let alloc = match &buffer_ctor {
                Value::Object(id) => rt.object_get(*id, "alloc"),
                _ => Value::Undefined,
            };
            let buf = rt.call_function(alloc, buffer_ctor.clone(), vec![Value::Number(size)])?;
            let read_sync = rt.object_get(fs_global, "readSync");

            let bytes_read = match rt.call_function(
                read_sync,
                Value::Object(fs_global),
                vec![
                    fd,
                    buf.clone(),
                    Value::Number(0.0),
                    Value::Number(size),
                    Value::Null,
                ],
            )? {
                Value::Number(n) => n,
                _ => size,
            };

            let buf = if bytes_read < size {
                let subarray = match &buf {
                    Value::Object(id) => rt.object_get(*id, "subarray"),
                    _ => Value::Undefined,
                };
                rt.call_function(
                    subarray,
                    buf,
                    vec![Value::Number(0.0), Value::Number(bytes_read)],
                )?
            } else {
                buf
            };

            let enc = match args.first() {
                Some(Value::String(s)) => Some(s.to_string()),
                Some(Value::Object(o)) => match rt.object_get(*o, "encoding") {
                    Value::String(s) => Some(s.to_string()),
                    _ => None,
                },
                _ => None,
            };
            match enc {
                Some(e) if e != "buffer" => {
                    let to_string = match &buf {
                        Value::Object(id) => rt.object_get(*id, "toString"),
                        _ => Value::Undefined,
                    };
                    rt.call_function(
                        to_string,
                        buf,
                        vec![Value::String(Rc::new(
                            rusty_js_runtime::value::JsString::from(e),
                        ))],
                    )
                }
                _ => Ok(buf),
            }
        })();
        fh_settle(rt, p, r);
        Ok(Value::Object(p))
    });

    register_method(rt, handle, "readableWebStream", |rt, _args| {

        let fd = fh_current_fd(rt);
        let fs_global = match rt.global_get("fs") {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let fstat = rt.object_get(fs_global, "fstatSync");
        let stats = rt.call_function(fstat, Value::Object(fs_global), vec![fd.clone()])?;
        let size = match &stats {
            Value::Object(id) => match rt.object_get(*id, "size") {
                Value::Number(n) => n,
                _ => 0.0,
            },
            _ => 0.0,
        };
        let buffer_ctor = rt.global_get("Buffer");
        let alloc = match &buffer_ctor {
            Value::Object(id) => rt.object_get(*id, "alloc"),
            _ => Value::Undefined,
        };
        let buf = rt.call_function(alloc, buffer_ctor, vec![Value::Number(size)])?;
        let read_sync = rt.object_get(fs_global, "readSync");
        let bytes_read = match rt.call_function(
            read_sync,
            Value::Object(fs_global),
            vec![
                fd,
                buf.clone(),
                Value::Number(0.0),
                Value::Number(size),
                Value::Null,
            ],
        )? {
            Value::Number(n) => n,
            _ => size,
        };
        let buf = if bytes_read < size {
            let subarray = match &buf {
                Value::Object(id) => rt.object_get(*id, "subarray"),
                _ => Value::Undefined,
            };
            rt.call_function(
                subarray,
                buf,
                vec![Value::Number(0.0), Value::Number(bytes_read)],
            )?
        } else {
            buf
        };

        let source = new_object(rt);
        rt.object_set(source, "__chunk".into(), buf);
        register_method(rt, source, "start", |rt, args| {
            let controller = args.first().cloned().unwrap_or(Value::Undefined);
            let this = match rt.current_this() {
                Value::Object(id) => id,
                _ => return Ok(Value::Undefined),
            };
            let chunk = rt.object_get(this, "__chunk");
            if let Value::Object(cid) = &controller {

                let has_bytes = matches!(&chunk, Value::Object(bid)
                    if matches!(rt.object_get(*bid, "length"), Value::Number(n) if n > 0.0));
                if has_bytes {
                    let enqueue = rt.object_get(*cid, "enqueue");
                    rt.call_function(enqueue, controller.clone(), vec![chunk])?;
                }
                let close = rt.object_get(*cid, "close");
                rt.call_function(close, controller, Vec::new())?;
            }
            Ok(Value::Undefined)
        });
        let rs_ctor = rt.global_get("ReadableStream");
        rt.construct(rs_ctor, vec![Value::Object(source)])
    });
}

fn normalize_filehandle_fd_arg(rt: &Runtime, args: &mut [Value]) {
    let Some(first) = args.first_mut() else {
        return;
    };
    let Value::Object(id) = first else {
        return;
    };
    if !matches!(rt.object_get(*id, "__cruft_fd"), Value::Number(_)) {
        return;
    }
    let fd = match rt.object_get(*id, "fd") {
        Value::Number(fd) => fd,
        _ => return,
    };
    if !rt.is_callable(&rt.object_get(*id, "close")) || !rt.is_callable(&rt.object_get(*id, "stat"))
    {
        return;
    }
    *first = Value::Number(fd);
}

fn call_node_callback(rt: &mut Runtime, callback: Option<Value>, args: Vec<Value>) {
    if let Some(cb) = callback {
        let _ = rt.call_function(cb, Value::Undefined, args);
    }
}

fn dispatch_async_result(rt: &mut Runtime, args: &[Value], result: Result<Value, Value>) -> Value {
    if let Some(cb) = trailing_callback(rt, args) {
        let cb_args = match result {
            Ok(v) => vec![Value::Null, v],
            Err(e) => vec![e],
        };
        let roots = crate::timer::roots_for_callback(&cb, &cb_args);
        rt.enqueue_host_phase_rooted(
            HostEnqueuePhase::HostCompletionMacrotask,
            "fs async callback",
            roots,
            move |rt| {
                let _ = rt.call_function(cb, Value::Undefined, cb_args);
                Ok(())
            },
        );
        Value::Undefined
    } else {
        let p = new_promise(rt);
        match result {
            Ok(v) => resolve_promise(rt, p, v),
            Err(e) => reject_promise(rt, p, e),
        }
        Value::Object(p)
    }
}

fn fs_error_from_runtime(rt: &mut Runtime, e: RuntimeError) -> Value {
    let _ = rt;
    match e {
        RuntimeError::Thrown(v) => v,
        RuntimeError::TypeError(m)
        | RuntimeError::RangeError(m)
        | RuntimeError::ReferenceError(m) => {
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(m)))
        }
        other => Value::String(Rc::new(rusty_js_runtime::value::JsString::from(format!(
            "{:?}",
            other
        )))),
    }
}

fn poll_fire_watchers(rt: &mut Runtime) -> bool {
    let now = std::time::Instant::now();
    let agent_id = rt.agent_id();

    let due: Vec<(u64, String, Option<Value>, ObjectRef)> = WATCHERS.with(|w| {
        let mut w = w.borrow_mut();
        let mut out = Vec::new();
        for e in w.iter_mut() {
            if e.agent_id != agent_id {
                continue;
            }
            if now.duration_since(e.last_polled).as_millis() as u64 >= e.interval_ms {
                let (mt, sz) = mtime_size(&e.path);
                let changed = mt != e.last_mtime || sz != e.last_size;
                if changed {
                    e.last_mtime = mt;
                    e.last_size = sz;
                    out.push((e.id, e.path.clone(), e.listener.clone(), e.watcher_obj));
                }
                e.last_polled = now;
            }
        }
        out
    });
    if due.is_empty() {
        return false;
    }
    for (_id, path, listener, _watcher_obj) in due {
        if let Some(cb) = listener {
            let event_id = NEXT_WATCH_EVENT_ID.with(|c| {
                let mut c = c.borrow_mut();
                let id = *c;
                *c += 1;
                id
            });
            let event_root_key = watch_event_root_key(event_id);
            rt.retain_host_roots(event_root_key.clone(), vec![cb.clone()]);

            let path = path.clone();
            rt.enqueue_host_phase_rooted(
                HostEnqueuePhase::HostCompletionMacrotask,
                "fs.watch listener",
                Vec::new(),
                move |rt| {
                    let _ = rt.call_function(
                        cb,
                        Value::Undefined,
                        vec![
                            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                                "change",
                            ))),
                            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(path))),
                        ],
                    );
                    rt.release_host_roots(&event_root_key);
                    Ok(())
                },
            );
        }
    }
    true
}

fn has_watchers(rt: &Runtime) -> bool {
    let agent_id = rt.agent_id();
    WATCHERS.with(|w| w.borrow().iter().any(|e| e.agent_id == agent_id))
}

#[allow(dead_code)]
fn sleep_until_next_poll() {
    let now = std::time::Instant::now();
    let next = WATCHERS.with(|w| {
        let w = w.borrow();
        let mut min_wait_ms: u64 = 1000;
        for e in w.iter() {
            let elapsed = now.duration_since(e.last_polled).as_millis() as u64;
            let remaining = e.interval_ms.saturating_sub(elapsed);
            if remaining < min_wait_ms {
                min_wait_ms = remaining;
            }
        }
        min_wait_ms.max(10)
    });
    std::thread::sleep(std::time::Duration::from_millis(next));
}

fn sleep_until_next_event(rt: &Runtime) {
    let now = std::time::Instant::now();
    let agent_id = rt.agent_id();
    let watcher_ms = WATCHERS.with(|w| {
        let w = w.borrow();
        let mut min: u64 = u64::MAX;
        for e in w.iter() {
            if e.agent_id != agent_id {
                continue;
            }
            let elapsed = now.duration_since(e.last_polled).as_millis() as u64;
            let remaining = e.interval_ms.saturating_sub(elapsed);
            if remaining < min {
                min = remaining;
            }
        }
        min
    });
    let timer_ms = crate::timer::next_due_ms(rt).unwrap_or(u64::MAX);
    let wait = watcher_ms.min(timer_ms).min(1000).max(1);
    std::thread::sleep(std::time::Duration::from_millis(wait));
}

fn poll_due_timers(rt: &mut Runtime) -> Result<bool, RuntimeError> {
    let due_timers = crate::timer::drain_due_pairs_for_runtime(rt);
    if due_timers.is_empty() {
        return Ok(false);
    }
    for (id, cb, args, repeat, async_context, async_resource) in due_timers {
        let roots = crate::timer::roots_for_callback_with_resource(&cb, &args, async_resource);
        rt.enqueue_host_phase_rooted_with_async_context(
            HostEnqueuePhase::TimerCallbackMacrotask,
            "timer callback",
            roots,
            async_context,
            move |rt| {

                if let Some(resource) = async_resource {
                    crate::node_stubs::async_hooks_call_with_global_resource_and_microtasks(
                        rt,
                        resource,
                        cb,
                        Value::Undefined,
                        args,
                    )
                    .map(|_| ())?;
                } else {
                    rt.call_function(cb, Value::Undefined, args).map(|_| ())?;
                }
                if !repeat {
                    if let Some(resource) = async_resource {
                        crate::node_stubs::async_hooks_emit_destroy_for_global(
                            rt,
                            Value::Object(resource),
                        )?;
                    }
                }
                Ok(())
            },
        );
        if !repeat {
            crate::timer::release_roots(rt, id);
        }
    }
    Ok(true)
}

fn idle_chain() -> Vec<Box<dyn crate::host_surfaces::HostSurface>> {
    use crate::host_surfaces::named;
    vec![

        named("process-signals", |rt, _| Ok(crate::process::poll_signals(rt))),

        named("napi-inbox", |rt, _| {
            Ok(rusty_js_runtime::napi::drain_main_inbox(rt) > 0)
        }),

        named("timers", |rt, _| poll_due_timers(rt)),

        named("http-client", |rt, _| crate::http::client_poll_io(rt)),

        named("fetch", |rt, _| crate::fetch::fetch_poll_io(rt)),

        named("spawn-harvest", |rt, _| crate::spawn::harvest(rt)),

        named("dgram", |rt, _| crate::dgram::poll_io(rt)),

        named("ipc", |rt, _| crate::ipc::harvest(rt)),

        named("http2-client", |rt, _| crate::http2_client::server_poll(rt)),

        named("net-harvest", |rt, _| crate::net::harvest_socket_io(rt)),

        named("ws", |rt, pass| {
            pass.had_ws_sessions = crate::ws::has_active_sessions_for_runtime(rt);
            crate::ws::poll_io(rt)
        }),

        named("http-server", |rt, pass| {
            if pass.had_ws_sessions {
                crate::http::poll_io_nonsticky(rt)
            } else {
                crate::http::poll_io(rt)
            }
        }),

        named("net", |rt, _| crate::net::poll_io(rt)),

        named("tls", |rt, _| crate::tls::poll_io(rt)),

        named("spawn", |rt, _| crate::spawn::poll_io(rt)),

        named("ipc-liveness", |rt, _| crate::ipc::poll_io(rt)),

        named("stdin", |rt, _| crate::stdin::poll_io(rt)),
    ]
}

pub fn install_poll_io(rt: &mut Runtime) {
    rt.install_host_hook(HostHook::PollIo(Box::new(|rt: &mut Runtime| {
        trace_poll_io("enter", "");

        if crate::process::has_process_listener(rt, "unhandledRejection") {
            let mut unhandled = rt.drain_unhandled_rejections();
            if !unhandled.is_empty() {

                unhandled.sort_by_key(|(id, _)| id.0);
                for (id, reason) in &unhandled {

                    crate::process::emit_process_event(
                        rt,
                        "unhandledRejection",
                        vec![reason.clone(), Value::Object(*id)],
                    );
                }
                return Ok(true);
            }
        }
        let entries = drain_pending();
        trace_poll_io("pending-drained", format!("entries={}", entries.len()));
        if entries.is_empty() {

            let mut pass = crate::host_surfaces::PollPass::default();
            for surface in idle_chain().iter_mut() {
                trace_poll_io(&format!("{}:begin", surface.name()), "");
                let progressed = surface.poll(rt, &mut pass)?;
                trace_poll_io(
                    &format!("{}:end", surface.name()),
                    if progressed { "progressed=true" } else { "progressed=false" },
                );
                if progressed {
                    return Ok(true);
                }
            }
            let has_w = has_watchers(rt);
            let has_t = crate::timer::has_pending(rt);
            let has_napi = rusty_js_runtime::napi::has_pending(rt);
            let has_ipc = crate::ipc::has_open_inprocess_workers(rt);
            let has_ws = crate::ws::has_active_sessions_for_runtime(rt);

            let has_stdin = crate::stdin::has_pending(rt);
            if has_w {
                trace_poll_io("watchers:begin", "");
                if poll_fire_watchers(rt) {
                    trace_poll_io("watchers:end", "progressed=true");
                    return Ok(true);
                }
                trace_poll_io("watchers:end", "progressed=false");
            }
            if has_w || has_t || has_napi || has_ipc || has_ws || has_stdin {
                trace_poll_io(
                    "sleep:begin",
                    format!(
                        "has_w={has_w} has_t={has_t} has_napi={has_napi} has_ipc={has_ipc} has_ws={has_ws} has_stdin={has_stdin}"
                    ),
                );
                if has_ws && !has_w && !has_t && !has_napi && !has_ipc && !has_stdin {

                    std::thread::sleep(std::time::Duration::from_micros(250));
                } else if has_napi && !has_w && !has_t && !has_ipc && !has_stdin {

                    let observed = rt.agent_wake_generation();
                    let drained = rusty_js_runtime::napi::drain_main_inbox(rt);
                    if drained > 0 {
                        trace_poll_io("napi-inbox:post-sleep-race", format!("drained={drained}"));
                        return Ok(true);
                    }
                    let _ =
                        rt.wait_agent_wake_timeout(observed, std::time::Duration::from_secs(1));
                } else {
                    sleep_until_next_event(rt);
                }
                trace_poll_io("sleep:end", "progressed=true");
                return Ok(true);
            }
            if crate::http::has_pending_client(rt) || crate::fetch::has_pending_fetch(rt) {
                trace_poll_io("pending-client-fetch-sleep:begin", "");
                let observed = rt.agent_wake_generation();
                if crate::http::client_poll_io(rt)? {
                    trace_poll_io("pending-client-fetch-sleep:end", "http-progressed=true");
                    return Ok(true);
                }
                if crate::fetch::fetch_poll_io(rt)? {
                    trace_poll_io("pending-client-fetch-sleep:end", "fetch-progressed=true");
                    return Ok(true);
                }
                let _ =
                    rt.wait_agent_wake_timeout(observed, std::time::Duration::from_secs(1));
                trace_poll_io("pending-client-fetch-sleep:end", "progressed=true");
                return Ok(true);
            }
            trace_poll_io("exit", "progressed=false");
            return Ok(false);
        }
        for entry in entries {

            let root_key = pending_root_key(entry.id);
            let promise = entry.promise;
            let callback = entry.callback;
            match entry.op {
                FsOp::Read { path, encoding } => {
                    rt.enqueue_host_phase_rooted(
                        HostEnqueuePhase::HostCompletionMacrotask,
                        "fs.readFile completion",
                        Vec::new(),
                        move |rt| {
                        let trace_start =
                            fs_completion_trace_enabled().then(std::time::Instant::now);
                        trace_fs_completion(
                            "enter",
                            entry.id,
                            &path,
                            format!(
                                "encoding={} callback={}",
                                encoding.as_deref().unwrap_or("<buffer>"),
                                callback.is_some()
                            ),
                        );
                        match std::fs::read(&path) {
                            Ok(bytes) => {
                                trace_fs_completion(
                                    "read-ok",
                                    entry.id,
                                    &path,
                                    format!("bytes={}", bytes.len()),
                                );
                                let v = bytes_to_value(rt, &bytes, encoding.as_deref());
                                trace_fs_completion(
                                    "value-ok",
                                    entry.id,
                                    &path,
                                    format!("bytes={}", bytes.len()),
                                );
                                resolve_promise(rt, promise, v.clone());
                                trace_fs_completion("promise-resolved", entry.id, &path, "");
                                call_node_callback(rt, callback, vec![Value::Null, v]);
                                trace_fs_completion("callback-called", entry.id, &path, "");
                            }
                            Err(e) => {
                                trace_fs_completion(
                                    "read-err",
                                    entry.id,
                                    &path,
                                    format!("error={}", e),
                                );

                                let t = if io_err_code(&e) == "EISDIR" {
                                    fs_io_throw(rt, "read", e)
                                } else {
                                    fs_throw(rt, "open", &path, e)
                                };
                                let err = fs_error_from_runtime(rt, t);
                                if callback.is_some() {
                                    call_node_callback(rt, callback, vec![err]);
                                    trace_fs_completion("callback-called", entry.id, &path, "");
                                } else {
                                    reject_promise(rt, promise, err);
                                    trace_fs_completion("promise-rejected", entry.id, &path, "");
                                }
                            }
                        }
                        rt.release_host_roots(&root_key);
                        let detail = trace_start
                            .map(|t| {
                                format!("elapsed_ms={:.3}", t.elapsed().as_secs_f64() * 1000.0)
                            })
                            .unwrap_or_default();
                        trace_fs_completion("release", entry.id, &path, detail);
                            Ok(())
                        },
                    );
                }
                FsOp::ReadFd { fd, encoding } => {
                    rt.enqueue_host_phase_rooted(
                        HostEnqueuePhase::HostCompletionMacrotask,
                        "fs.readFile(fd) completion",
                        Vec::new(),
                        move |rt| {
                        use std::io::Read as _;

                        let mut buf = Vec::new();
                        let outcome: Option<std::io::Result<usize>> =
                            match rt.fd_table.get_mut(&fd) {
                                Some(file) => Some(file.read_to_end(&mut buf)),
                                None => None,
                            };
                        match outcome {
                            Some(Ok(_)) => {
                                let v = bytes_to_value(rt, &buf, encoding.as_deref());
                                resolve_promise(rt, promise, v.clone());
                                call_node_callback(rt, callback, vec![Value::Null, v]);
                            }
                            other => {
                                let t = match other {
                                    Some(Err(e)) => fs_io_throw(rt, "read", e),
                                    _ => RuntimeError::Thrown(fs_error_object_full(
                                        rt,
                                        "EBADF",
                                        "EBADF: bad file descriptor, fstat",
                                        Some("fstat"),
                                        None,
                                    )),
                                };
                                let err = fs_error_from_runtime(rt, t);
                                if callback.is_some() {
                                    call_node_callback(rt, callback, vec![err]);
                                } else {
                                    reject_promise(rt, promise, err);
                                }
                            }
                        }
                        rt.release_host_roots(&root_key);
                            Ok(())
                        },
                    );
                }
                FsOp::Write { path, data } => {
                    rt.enqueue_host_phase_rooted(
                        HostEnqueuePhase::HostCompletionMacrotask,
                        "fs.writeFile completion",
                        Vec::new(),
                        move |rt| {
                        match std::fs::write(&path, &data) {
                            Ok(()) => {
                                resolve_promise(rt, promise, Value::Undefined);
                                call_node_callback(rt, callback, vec![Value::Null]);
                            }
                            Err(e) => {

                                let t = fs_throw(rt, "open", &path, e);
                                let err = fs_error_from_runtime(rt, t);
                                if callback.is_some() {
                                    call_node_callback(rt, callback, vec![err]);
                                } else {
                                    reject_promise(rt, promise, err);
                                }
                            }
                        }
                        rt.release_host_roots(&root_key);
                            Ok(())
                        },
                    );
                }
                FsOp::ReadDir {
                    path,
                    with_file_types,
                    recursive,
                } => {
                    rt.enqueue_host_phase_rooted(
                        HostEnqueuePhase::HostCompletionMacrotask,
                        "fs.readdir completion",
                        Vec::new(),
                        move |rt| {
                        match collect_readdir(std::path::Path::new(&path), recursive) {
                            Ok(entries) => {
                                let arr = rt.alloc_object(Object::new_array());
                                for (i, (rel, parent, name, ft)) in entries.iter().enumerate() {
                                    let val = if with_file_types {
                                        Value::Object(dirent_object(rt, name, parent, Some(*ft)))
                                    } else {
                                        Value::String(Rc::new(
                                            rusty_js_runtime::value::JsString::from(rel.clone()),
                                        ))
                                    };
                                    rt.object_set(arr, i.to_string(), val);
                                }
                                rt.object_set(arr, "length".into(), Value::Number(entries.len() as f64));
                                let result = Value::Object(arr);
                                resolve_promise(rt, promise, result.clone());
                                call_node_callback(rt, callback, vec![Value::Null, result]);
                            }
                            Err(e) => {
                                let t = fs_io_throw(rt, "scandir", e);
                                let err = fs_error_from_runtime(rt, t);
                                if callback.is_some() {
                                    call_node_callback(rt, callback, vec![err]);
                                } else {
                                    reject_promise(rt, promise, err);
                                }
                            }
                        }
                        rt.release_host_roots(&root_key);
                            Ok(())
                        },
                    );
                }
                FsOp::Exists { path } => {
                    rt.enqueue_host_phase_rooted(
                        HostEnqueuePhase::HostCompletionMacrotask,
                        "fs.exists completion",
                        Vec::new(),
                        move |rt| {
                            let ok = std::path::Path::new(&path).exists();
                            resolve_promise(rt, promise, Value::Boolean(ok));
                            call_node_callback(rt, callback, vec![Value::Boolean(ok)]);
                            rt.release_host_roots(&root_key);
                            Ok(())
                        },
                    );
                }
            }
        }
        Ok(true)
    })));
}

fn bytes_to_value(rt: &mut Runtime, bytes: &[u8], encoding: Option<&str>) -> Value {
    match encoding {
        Some(e) if matches!(e, "utf-8" | "utf8") => Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(String::from_utf8_lossy(bytes).into_owned()),
        )),

        Some(enc) => {
            let u8a = rt.alloc_uint8_array_from_bytes(bytes);
            let buffer = rt.global_get("Buffer");
            if let Value::Object(b) = buffer {
                let from = rt.object_get(b, "from");
                if rt.is_callable(&from) {
                    if let Ok(buf @ Value::Object(bid)) =
                        rt.call_function(from, buffer.clone(), vec![Value::Object(u8a)])
                    {
                        let ts = rt.object_get(bid, "toString");
                        if rt.is_callable(&ts) {
                            let encv = Value::String(Rc::new(
                                rusty_js_runtime::value::JsString::from(enc),
                            ));
                            if let Ok(s @ Value::String(_)) = rt.call_function(ts, buf, vec![encv])
                            {
                                return s;
                            }
                        }
                    }
                }
            }
            Value::Object(u8a)
        }
        _ => {

            crate::node_stubs::intrinsic_buffer_from_bytes(rt, bytes)
        }
    }
}

fn decode_string_by_encoding(s: &str, enc: &str) -> Vec<u8> {
    match enc.to_ascii_lowercase().as_str() {
        "hex" => {
            let b = s.as_bytes();
            let mut out = Vec::with_capacity(b.len() / 2);
            let mut i = 0;
            while i + 1 < b.len() {
                match ((b[i] as char).to_digit(16), (b[i + 1] as char).to_digit(16)) {
                    (Some(h), Some(l)) => {
                        out.push(((h << 4) | l) as u8);
                        i += 2;
                    }
                    _ => break,
                }
            }
            out
        }
        "base64" | "base64url" => {

            let mut lut = [255u8; 128];
            for (i, c) in b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
                .iter()
                .enumerate()
            {
                lut[*c as usize] = i as u8;
            }
            lut[b'-' as usize] = 62;
            lut[b'_' as usize] = 63;
            let mut out = Vec::with_capacity(s.len() * 3 / 4);
            let (mut buf, mut bits) = (0u32, 0u32);
            for c in s.bytes() {
                if c == b'=' {
                    break;
                }
                if c >= 128 || lut[c as usize] == 255 {
                    continue;
                }
                buf = (buf << 6) | lut[c as usize] as u32;
                bits += 6;
                if bits >= 8 {
                    bits -= 8;
                    out.push(((buf >> bits) & 0xff) as u8);
                }
            }
            out
        }
        "latin1" | "binary" => s.chars().map(|c| c as u8).collect(),
        "ascii" => s.chars().map(|c| (c as u32 & 0x7f) as u8).collect(),
        "ucs2" | "ucs-2" | "utf16le" | "utf-16le" => {
            s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect()
        }

        _ => s.as_bytes().to_vec(),
    }
}

fn value_to_bytes(rt: &Runtime, v: &Value, encoding: Option<&str>) -> Vec<u8> {

    if let Some(enc) = encoding {
        if !matches!(v, Value::Object(_)) {
            return decode_string_by_encoding(
                rusty_js_runtime::abstract_ops::to_string(v).as_str(),
                enc,
            );
        }
    }
    match v {
        Value::String(s) => s.as_bytes().to_vec(),
        Value::Object(id) => {

            let len = match rt.object_get(*id, "length") {
                Value::Number(n) => n as usize,
                _ => 0,
            };
            let mut out = Vec::with_capacity(len);
            for i in 0..len {
                let b = match rt.object_get(*id, &i.to_string()) {
                    Value::Number(n) => n as u8,
                    _ => 0,
                };
                out.push(b);
            }
            out
        }
        other => rusty_js_runtime::abstract_ops::to_string(other)
            .as_str()
            .as_bytes()
            .to_vec(),
    }
}

fn arg_encoding(rt: &Runtime, args: &[Value], i: usize) -> Option<String> {
    match args.get(i) {
        Some(Value::String(s)) => Some(s.as_str().to_string()),

        Some(Value::Object(id)) => match rt.object_get(*id, "encoding") {
            Value::String(s) => Some(s.as_str().to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn stat_throw_if_no_entry(rt: &mut Runtime, args: &[Value], i: usize) -> bool {
    match args.get(i) {
        Some(Value::Object(id)) => {
            !matches!(rt.object_get(*id, "throwIfNoEntry"), Value::Boolean(false))
        }
        _ => true,
    }
}

fn stat_bigint(rt: &mut Runtime, args: &[Value], i: usize) -> bool {
    matches!(args.get(i), Some(Value::Object(id))
        if matches!(rt.object_get(*id, "bigint"), Value::Boolean(true)))
}

const FS_LISTENERS_SLOT: &str = "__fs_listeners";

fn fs_buffer_from_bytes(rt: &mut Runtime, bytes: &[u8]) -> Value {

    crate::node_stubs::intrinsic_buffer_from_bytes(rt, bytes)
}

fn install_fs_emitter(rt: &mut Runtime, obj: ObjectRef) {
    let registry = new_object(rt);
    rt.set_engine_sentinel(obj, FS_LISTENERS_SLOT, Value::Object(registry));
    let on_impl = |rt: &mut Runtime, args: &[Value]| -> Result<Value, RuntimeError> {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let event = match args.first() {
            Some(v) => value_to_string_lossy(rt, v),
            None => String::new(),
        };
        let listener = args.get(1).cloned().unwrap_or(Value::Undefined);
        if !rt.is_callable(&listener) {
            return Ok(Value::Object(this));
        }
        let registry = match rt.object_get(this, FS_LISTENERS_SLOT) {
            Value::Object(id) => id,
            _ => return Ok(Value::Object(this)),
        };
        let arr = match rt.object_get(registry, &event) {
            Value::Object(a) => a,
            _ => {
                let a = rt.alloc_object(Object::new_array());
                rt.object_set(a, "length".into(), Value::Number(0.0));
                rt.object_set(registry, event.clone(), Value::Object(a));
                a
            }
        };
        let len = rt.array_length(arr);
        rt.object_set(arr, len.to_string(), listener);
        rt.object_set(arr, "length".into(), Value::Number((len + 1) as f64));
        Ok(Value::Object(this))
    };
    register_method(rt, obj, "on", on_impl);
    register_method(rt, obj, "once", on_impl);
    register_method(rt, obj, "addListener", on_impl);
    register_method(rt, obj, "removeListener", |rt, _args| Ok(rt.current_this()));
}

fn value_to_string_lossy(rt: &Runtime, v: &Value) -> String {
    match v {
        Value::String(s) => s.as_str().to_string(),
        _ => rusty_js_runtime::abstract_ops::to_string(v)
            .as_str()
            .to_string(),
    }
}

fn fs_emit(rt: &mut Runtime, obj: ObjectRef, event: &str, args: Vec<Value>) {
    let registry = match rt.object_get(obj, FS_LISTENERS_SLOT) {
        Value::Object(id) => id,
        _ => return,
    };
    let arr = match rt.object_get(registry, event) {
        Value::Object(a) => a,
        _ => return,
    };
    let len = rt.array_length(arr);
    for i in 0..len {
        let cb = rt.object_get(arr, &i.to_string());
        if rt.is_callable(&cb) {
            let _ = rt.call_function(cb, Value::Object(obj), args.clone());
        }
    }
}

fn fs_error_object(rt: &mut Runtime, code: &str, message: &str) -> Value {
    fs_error_object_full(rt, code, message, None, None)
}

fn fs_error_object_full(
    rt: &mut Runtime,
    code: &str,
    message: &str,
    syscall: Option<&str>,
    path: Option<&str>,
) -> Value {
    let js = |s: &str| {
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            s.to_string(),
        )))
    };
    let msg_v = js(message);
    let ctor = rt.global_get("Error");
    let err = match rt.construct(ctor, vec![msg_v.clone()]) {
        Ok(Value::Object(id)) => id,
        _ => {
            let o = new_object(rt);
            rt.object_set(o, "message".into(), msg_v);
            o
        }
    };
    if let Some(errno) = node_errno(code) {
        rt.object_set(err, "errno".into(), Value::Number(f64::from(errno)));
    }
    rt.object_set(err, "code".into(), js(code));
    if let Some(sc) = syscall {
        rt.object_set(err, "syscall".into(), js(sc));
    }
    if let Some(p) = path {
        rt.object_set(err, "path".into(), js(p));
    }
    Value::Object(err)
}

fn node_errno(code: &str) -> Option<i32> {
    Some(match code {
        "ENOENT" => -2,
        "EACCES" => -13,
        "EEXIST" => -17,
        "ENOTDIR" => -20,
        "EISDIR" => -21,
        "ENOTEMPTY" => -39,
        "EPERM" => -1,
        "EIO" => -5,
        "EBADF" => -9,
        _ => return None,
    })
}

fn node_strerror(code: &str) -> Option<&'static str> {
    Some(match code {
        "ENOENT" => "no such file or directory",
        "EACCES" => "permission denied",
        "EEXIST" => "file already exists",
        "ENOTDIR" => "not a directory",
        "EISDIR" => "illegal operation on a directory",
        "ENOTEMPTY" => "directory not empty",
        "EPERM" => "operation not permitted",
        _ => return None,
    })
}

fn io_err_code(e: &std::io::Error) -> &'static str {
    use std::io::ErrorKind::*;

    if let Some(errno) = e.raw_os_error() {
        match errno {
            2 => return "ENOENT",
            13 => return "EACCES",
            17 => return "EEXIST",
            20 => return "ENOTDIR",
            21 => return "EISDIR",
            39 => return "ENOTEMPTY",
            _ => {}
        }
    }
    match e.kind() {
        NotFound => "ENOENT",
        PermissionDenied => "EACCES",
        AlreadyExists => "EEXIST",
        _ => "EIO",
    }
}

fn realpath_first_missing(path: &str) -> String {
    let mut acc = std::path::PathBuf::new();
    for comp in std::path::Path::new(path).components() {
        acc.push(comp);
        if std::fs::symlink_metadata(&acc).is_err() {
            return acc.to_string_lossy().into_owned();
        }
    }
    path.to_string()
}

fn fs_io_throw(rt: &mut Runtime, syscall: &str, e: std::io::Error) -> RuntimeError {
    let code = io_err_code(&e);
    let desc = node_strerror(code)
        .map(str::to_string)
        .unwrap_or_else(|| e.to_string());
    RuntimeError::Thrown(fs_error_object_full(
        rt,
        code,
        &format!("{code}: {desc}, {syscall}"),
        Some(syscall),
        None,
    ))
}

fn fs_throw(rt: &mut Runtime, syscall: &str, path: &str, e: std::io::Error) -> RuntimeError {
    let code = io_err_code(&e);
    let desc = node_strerror(code)
        .map(str::to_string)
        .unwrap_or_else(|| e.to_string());
    let msg = format!("{code}: {desc}, {syscall} '{path}'");
    RuntimeError::Thrown(fs_error_object_full(
        rt,
        code,
        &msg,
        Some(syscall),
        Some(path),
    ))
}

fn opt_usize(rt: &Runtime, opts: Option<ObjectRef>, key: &str) -> Option<usize> {
    let o = opts?;
    match rt.object_get(o, key) {
        Value::Number(n) if n >= 0.0 => Some(n as usize),
        _ => None,
    }
}

fn opt_string(rt: &Runtime, opts: Option<ObjectRef>, key: &str) -> Option<String> {
    let o = opts?;
    match rt.object_get(o, key) {
        Value::String(s) => Some(s.as_str().to_string()),
        _ => None,
    }
}

fn install_fs_stream_shape(
    rt: &mut Runtime,
    obj: ObjectRef,
    opts_arg: Option<&Value>,
    default_flags: &str,
    proto_global_key: &str,
) {
    let opts_obj = match opts_arg {
        Some(Value::Object(o)) => Some(*o),
        _ => None,
    };
    let flags = opt_string(rt, opts_obj, "flags").unwrap_or_else(|| default_flags.to_string());
    let mode = opt_usize(rt, opts_obj, "mode").unwrap_or(0o666);

    let auto_close = match opts_obj {
        Some(o) => !matches!(rt.object_get(o, "autoClose"), Value::Boolean(false)),
        None => true,
    };

    let fd = match opts_obj {
        Some(o) => match rt.object_get(o, "fd") {
            n @ Value::Number(_) => n,
            _ => Value::Null,
        },
        None => Value::Null,
    };
    rt.object_set(
        obj,
        "flags".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(flags))),
    );
    rt.object_set(obj, "mode".into(), Value::Number(mode as f64));
    rt.object_set(obj, "autoClose".into(), Value::Boolean(auto_close));
    rt.object_set(obj, "fd".into(), fd);
    rt.object_set(obj, "pending".into(), Value::Boolean(true));
    if let Value::Object(proto) = rt.global_get(proto_global_key) {

        let class_name = if default_flags == "r" {
            "Readable"
        } else {
            "Writable"
        };
        if let Value::Object(se) = rt.global_get("stream") {
            if let Value::Object(class_ctor) = rt.object_get(se, class_name) {
                if let Value::Object(class_proto) = rt.object_get(class_ctor, "prototype") {
                    if !matches!(rt.obj(proto).proto, Some(p) if p == class_proto) {
                        rt.set_object_prototype_internal(proto, Some(class_proto));
                    }
                }
            }
        }
        rt.set_object_prototype_internal(obj, Some(proto));
    }
}

fn make_read_stream(rt: &mut Runtime, path: String, opts_arg: Option<Value>) -> ObjectRef {
    crate::stream::ensure_installed(rt);
    let obj = new_object(rt);
    install_fs_emitter(rt, obj);
    rt.object_set(
        obj,
        "path".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            path.clone(),
        ))),
    );
    rt.object_set(obj, "bytesRead".into(), Value::Number(0.0));

    rt.object_set(obj, "readableFlowing".into(), Value::Null);
    rt.object_set(obj, "destroyed".into(), Value::Boolean(false));
    install_fs_stream_shape(
        rt,
        obj,
        opts_arg.as_ref(),
        "r",
        "__cruft_fs_readstream_proto",
    );
    register_method(rt, obj, "close", |rt, _args| {
        if let Value::Object(t) = rt.current_this() {
            rt.object_set(t, "destroyed".into(), Value::Boolean(true));
        }
        Ok(rt.current_this())
    });
    register_method(rt, obj, "destroy", |rt, _args| {

        if let Value::Object(t) = rt.current_this() {
            rt.object_set(t, "destroyed".into(), Value::Boolean(true));
        }
        Ok(rt.current_this())
    });
    register_method(rt, obj, "pause", |rt, _args| Ok(rt.current_this()));
    register_method(rt, obj, "resume", |rt, _args| Ok(rt.current_this()));

    register_method(rt, obj, "pipe", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(t) => t,
            _ => return Ok(Value::Undefined),
        };
        let dest = match args.first() {
            Some(Value::Object(d)) => *d,
            _ => return Ok(rt.current_this()),
        };
        let sval = |rt: &mut Runtime, s: &str| {
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(s)))
        };
        let on = rt.object_get(this, "on");
        if !rt.is_callable(&on) {
            return Ok(Value::Object(dest));
        }
        let on_data = make_callable_rooted(rt, "pipe.onData", vec![dest], move |rt, a| {
            let chunk = a.first().cloned().unwrap_or(Value::Undefined);
            let w = rt.object_get(dest, "write");
            if rt.is_callable(&w) {
                let _ = rt.call_function(w, Value::Object(dest), vec![chunk]);
            }
            Ok(Value::Undefined)
        });
        let dv = sval(rt, "data");
        let _ = rt.call_function(
            on.clone(),
            Value::Object(this),
            vec![dv, Value::Object(on_data)],
        );
        let on_end = make_callable_rooted(rt, "pipe.onEnd", vec![dest], move |rt, _a| {
            let e = rt.object_get(dest, "end");
            if rt.is_callable(&e) {
                let _ = rt.call_function(e, Value::Object(dest), Vec::new());
            }
            Ok(Value::Undefined)
        });
        let ev = sval(rt, "end");
        let _ = rt.call_function(
            on.clone(),
            Value::Object(this),
            vec![ev, Value::Object(on_end)],
        );
        let on_err = make_callable_rooted(rt, "pipe.onError", vec![dest], move |rt, a| {
            let err = a.first().cloned().unwrap_or(Value::Undefined);
            let em = rt.object_get(dest, "emit");
            if rt.is_callable(&em) {
                let etag = Value::String(Rc::new(rusty_js_runtime::value::JsString::from("error")));
                let _ = rt.call_function(em, Value::Object(dest), vec![etag, err]);
            }
            Ok(Value::Undefined)
        });
        let erv = sval(rt, "error");
        let _ = rt.call_function(on, Value::Object(this), vec![erv, Value::Object(on_err)]);
        Ok(Value::Object(dest))
    });

    let (encoding, start, end, hwm, emit_close) = match &opts_arg {
        Some(Value::String(s)) => (Some(s.as_str().to_string()), None, None, 65536usize, true),
        Some(Value::Object(o)) => (
            opt_string(rt, Some(*o), "encoding"),
            opt_usize(rt, Some(*o), "start"),
            opt_usize(rt, Some(*o), "end"),
            opt_usize(rt, Some(*o), "highWaterMark").unwrap_or(65536),
            !matches!(rt.object_get(*o, "emitClose"), Value::Boolean(false)),
        ),
        _ => (None, None, None, 65536usize, true),
    };
    let hwm = hwm.max(1);

    rt.enqueue_host_phase_rooted(
        HostEnqueuePhase::HostCompletionMacrotask,
        "fs.createReadStream delivery",
        vec![obj],
        move |rt| {

            rt.object_set(obj, "pending".into(), Value::Boolean(false));
            match std::fs::read(&path) {
                Err(e) => {
                    let code = io_err_code(&e);
                    let desc = node_strerror(code)
                        .map(str::to_string)
                        .unwrap_or_else(|| e.to_string());
                    let err = fs_error_object_full(
                        rt,
                        code,
                        &format!("{code}: {desc}, open '{path}'"),
                        Some("open"),
                        Some(&path),
                    );
                    fs_emit(rt, obj, "error", vec![err]);
                    if emit_close {
                        fs_emit(rt, obj, "close", Vec::new());
                    }
                }
                Ok(all) => {

                    let s = start.unwrap_or(0).min(all.len());
                    let e = end
                        .map(|e| (e + 1).min(all.len()))
                        .unwrap_or(all.len())
                        .max(s);
                    let bytes = &all[s..e];
                    fs_emit(rt, obj, "open", vec![Value::Number(0.0)]);
                    rt.object_set(obj, "bytesRead".into(), Value::Number(bytes.len() as f64));
                    let mut off = 0usize;
                    while off < bytes.len() {
                        let chunk_end = (off + hwm).min(bytes.len());
                        let slice = &bytes[off..chunk_end];
                        let chunk = match &encoding {
                            Some(enc) if matches!(enc.as_str(), "utf8" | "utf-8") => {
                                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                                    String::from_utf8_lossy(slice).into_owned(),
                                )))
                            }
                            _ => fs_buffer_from_bytes(rt, slice),
                        };
                        fs_emit(rt, obj, "data", vec![chunk]);
                        off = chunk_end;
                    }
                    fs_emit(rt, obj, "end", Vec::new());
                    if emit_close {
                        fs_emit(rt, obj, "close", Vec::new());
                    }
                }
            }
            Ok(())
        },
    );

    crate::stream::install_async_iterator(rt, obj);
    obj
}

fn make_write_stream(rt: &mut Runtime, path: String, opts_arg: Option<Value>) -> ObjectRef {
    crate::stream::ensure_installed(rt);
    let obj = new_object(rt);
    install_fs_emitter(rt, obj);
    rt.object_set(
        obj,
        "path".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            path.clone(),
        ))),
    );
    rt.object_set(obj, "bytesWritten".into(), Value::Number(0.0));
    install_fs_stream_shape(
        rt,
        obj,
        opts_arg.as_ref(),
        "w",
        "__cruft_fs_writestream_proto",
    );

    let (encoding, flags, emit_close) = match &opts_arg {
        Some(Value::String(s)) => (Some(s.as_str().to_string()), "w".to_string(), true),
        Some(Value::Object(o)) => (
            opt_string(rt, Some(*o), "encoding"),
            opt_string(rt, Some(*o), "flags").unwrap_or_else(|| "w".to_string()),
            !matches!(rt.object_get(*o, "emitClose"), Value::Boolean(false)),
        ),
        _ => (None, "w".to_string(), true),
    };

    if !flags.starts_with('a') {
        let _ = std::fs::write(&path, b"");
    } else {
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path);
    }
    let open_path = path.clone();

    rt.enqueue_host_phase_rooted(
        HostEnqueuePhase::HostCompletionMacrotask,
        "fs.createWriteStream open",
        vec![obj],
        move |rt| {
            let _ = open_path;

            rt.object_set(obj, "pending".into(), Value::Boolean(false));
            fs_emit(rt, obj, "open", vec![Value::Number(0.0)]);
            Ok(())
        },
    );

    let write_path = path.clone();
    let write_enc = encoding.clone();
    register_method(rt, obj, "write", move |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Boolean(false)),
        };

        let cb = args.iter().rev().find(|a| rt.is_callable(a)).cloned();
        let bytes = match args.first() {
            Some(v) if !rt.is_callable(v) => value_to_bytes(rt, v, write_enc.as_deref()),
            _ => Vec::new(),
        };
        let appended = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&write_path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, &bytes).map(|_| bytes.len()));
        if let Ok(n) = appended {
            let prior = match rt.object_get(this, "bytesWritten") {
                Value::Number(n) => n,
                _ => 0.0,
            };
            rt.object_set(this, "bytesWritten".into(), Value::Number(prior + n as f64));
        }

        if cb.is_some() {
            let mut roots = vec![this];
            if let Some(Value::Object(c)) = &cb {
                roots.push(*c);
            }
            rt.enqueue_host_phase_rooted(
                HostEnqueuePhase::HostCompletionMacrotask,
                "fs.WriteStream write cb",
                roots,
                move |rt| {
                    if let Some(c) = &cb {
                        let _ = rt.call_function(c.clone(), Value::Object(this), Vec::new());
                    }
                    Ok(())
                },
            );
        }
        Ok(Value::Boolean(true))
    });

    let end_enc = encoding.clone();
    let end_path = path.clone();
    register_method(rt, obj, "end", move |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };

        let cb = args.iter().rev().find(|a| rt.is_callable(a)).cloned();
        if let Some(v) = args.first() {
            if !rt.is_callable(v) {
                let bytes = value_to_bytes(rt, v, end_enc.as_deref());
                let n = bytes.len();
                let wrote = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&end_path)
                    .and_then(|mut f| std::io::Write::write_all(&mut f, &bytes));

                if wrote.is_ok() {
                    let prior = match rt.object_get(this, "bytesWritten") {
                        Value::Number(p) => p,
                        _ => 0.0,
                    };
                    rt.object_set(this, "bytesWritten".into(), Value::Number(prior + n as f64));
                }
            }
        }
        let mut roots = vec![this];
        if let Some(Value::Object(c)) = &cb {
            roots.push(*c);
        }
        rt.enqueue_host_phase_rooted(
            HostEnqueuePhase::HostCompletionMacrotask,
            "fs.WriteStream finish",
            roots,
            move |rt| {
                rt.object_set(this, "writableFinished".into(), Value::Boolean(true));
                fs_emit(rt, this, "finish", Vec::new());

                if let Some(c) = &cb {
                    let _ = rt.call_function(c.clone(), Value::Object(this), Vec::new());
                }
                if emit_close {
                    fs_emit(rt, this, "close", Vec::new());
                }
                Ok(())
            },
        );
        Ok(Value::Undefined)
    });
    register_method(rt, obj, "destroy", |rt, _args| Ok(rt.current_this()));
    obj
}

fn stat_current_mode(rt: &mut Runtime) -> u32 {
    match rt.current_this() {
        Value::Object(this) => match rt.object_get(this, "mode") {
            Value::Number(n) => n as u32,

            Value::BigInt(b) => b.to_decimal().parse::<u64>().unwrap_or(0) as u32,
            _ => 0,
        },
        _ => 0,
    }
}

fn dirent_current_type(rt: &mut Runtime) -> u32 {
    match rt.current_this() {
        Value::Object(this) => match rt.object_get(this, "__dirent_type") {
            Value::Number(n) => n as u32,
            _ => 0,
        },
        _ => 0,
    }
}

fn link_fs_class_prototype(rt: &mut Runtime, o: ObjectRef, class: &str) {
    if let Value::Object(fs_global) = rt.global_get("fs") {
        if let Value::Object(ctor) = rt.object_get(fs_global, class) {
            if let Value::Object(proto) = rt.object_get(ctor, "prototype") {
                rt.set_object_prototype_internal(o, Some(proto));
            }
        }
    }
}

fn install_lazy_date_field(rt: &mut Runtime, o: ObjectRef, key: &str, ms_key: &str) {
    use rusty_js_runtime::value::{PropertyDescriptor, PropertyKey};
    let key_owned = key.to_string();
    let ms_key_owned = ms_key.to_string();
    let getter = crate::register::make_callable(rt, &format!("get {key}"), move |rt, _args| {
        let this = match rt.current_this() {
            Value::Object(t) => t,
            other => return Ok(other),
        };
        let ms = match rt.object_get(this, &ms_key_owned) {
            Value::Number(n) => n,
            Value::BigInt(b) => b.to_f64(),
            _ => 0.0,
        };
        let date = match rt.global_get("Date") {
            Value::Object(ctor) => rt
                .construct(Value::Object(ctor), vec![Value::Number(ms)])
                .unwrap_or(Value::Undefined),
            _ => Value::Undefined,
        };

        rt.obj_mut(this).dict_mut().insert(
            PropertyKey::String(key_owned.clone()),
            PropertyDescriptor {
                value: date.clone(),
                writable: true,
                enumerable: true,
                configurable: true,
                getter: None,
                setter: None,
            },
        );
        Ok(date)
    });
    rt.obj_mut(o).dict_mut().insert(
        PropertyKey::String(key.to_string()),
        PropertyDescriptor {
            value: Value::Undefined,
            writable: false,
            enumerable: false,
            configurable: true,
            getter: Some(Value::Object(getter)),
            setter: None,
        },
    );
}

fn link_stats_prototype(rt: &mut Runtime, o: ObjectRef) {
    if let Value::Object(fs_global) = rt.global_get("fs") {
        if let Value::Object(stats_ctor) = rt.object_get(fs_global, "Stats") {
            if let Value::Object(proto) = rt.object_get(stats_ctor, "prototype") {
                rt.set_object_prototype_internal(o, Some(proto));
            }
        }
    }
}

fn stat_object(rt: &mut Runtime, md: &std::fs::Metadata) -> ObjectRef {
    let o = new_object(rt);
    rt.object_set(o, "size".into(), Value::Number(md.len() as f64));
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        rt.object_set(o, "mode".into(), Value::Number(md.mode() as f64));
        rt.object_set(o, "uid".into(), Value::Number(md.uid() as f64));
        rt.object_set(o, "gid".into(), Value::Number(md.gid() as f64));

        rt.object_set(o, "dev".into(), Value::Number(md.dev() as f64));
        rt.object_set(o, "ino".into(), Value::Number(md.ino() as f64));
        rt.object_set(o, "nlink".into(), Value::Number(md.nlink() as f64));
        rt.object_set(o, "rdev".into(), Value::Number(md.rdev() as f64));
        rt.object_set(o, "blksize".into(), Value::Number(md.blksize() as f64));
        rt.object_set(o, "blocks".into(), Value::Number(md.blocks() as f64));
    }
    #[cfg(windows)]
    {

        let ft = md.file_type();
        let mut mode: u32 = if md.permissions().readonly() {
            0o444
        } else {
            0o666
        };
        if ft.is_dir() {
            mode |= 0o111 | 0o040000;
        } else if ft.is_symlink() {
            mode |= 0o120000;
        } else {
            mode |= 0o100000;
        }
        rt.object_set(o, "mode".into(), Value::Number(mode as f64));
        rt.object_set(o, "uid".into(), Value::Number(0.0));
        rt.object_set(o, "gid".into(), Value::Number(0.0));
        for (k, v) in [
            ("dev", 0.0),
            ("ino", 0.0),
            ("nlink", 1.0),
            ("rdev", 0.0),
            ("blksize", 4096.0),
            ("blocks", (md.len() as f64 / 512.0).ceil()),
        ] {
            rt.object_set(o, k.into(), Value::Number(v));
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        rt.object_set(o, "mode".into(), Value::Number(0.0));
        rt.object_set(o, "uid".into(), Value::Number(0.0));
        rt.object_set(o, "gid".into(), Value::Number(0.0));
        for (k, v) in [
            ("dev", 0.0),
            ("ino", 0.0),
            ("nlink", 1.0),
            ("rdev", 0.0),
            ("blksize", 4096.0),
            ("blocks", (md.len() as f64 / 512.0).ceil()),
        ] {
            rt.object_set(o, k.into(), Value::Number(v));
        }
    }
    let is_file = md.is_file();
    let is_dir = md.is_dir();
    let mtime_ms = md
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0);
    rt.object_set(o, "mtimeMs".into(), Value::Number(mtime_ms));

    let atime_ms = md
        .accessed()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as f64)
        .unwrap_or(mtime_ms);
    let birthtime_ms = md
        .created()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as f64)
        .unwrap_or(mtime_ms);
    rt.object_set(o, "atimeMs".into(), Value::Number(atime_ms));
    rt.object_set(o, "ctimeMs".into(), Value::Number(mtime_ms));
    rt.object_set(o, "birthtimeMs".into(), Value::Number(birthtime_ms));

    let _ = (mtime_ms, atime_ms, birthtime_ms);
    for (key, ms_key) in [
        ("mtime", "mtimeMs"),
        ("ctime", "ctimeMs"),
        ("atime", "atimeMs"),
        ("birthtime", "birthtimeMs"),
    ] {
        install_lazy_date_field(rt, o, key, ms_key);
    }
    let _ = (is_file, is_dir);

    link_stats_prototype(rt, o);
    o
}

fn stat_object_opt(rt: &mut Runtime, md: &std::fs::Metadata, bigint: bool) -> ObjectRef {
    let o = stat_object(rt, md);
    if !bigint {
        return o;
    }
    let bi = |v: u64| -> Value {
        Value::BigInt(std::rc::Rc::new(
            rusty_js_runtime::bigint::JsBigInt::from_u64(v),
        ))
    };

    let ms_of = |rt: &mut Runtime, k: &str| -> f64 {
        match rt.object_get(o, k) {
            Value::Number(n) => n,
            _ => 0.0,
        }
    };
    let atime_ms = ms_of(rt, "atimeMs");
    let mtime_ms = ms_of(rt, "mtimeMs");
    let ctime_ms = ms_of(rt, "ctimeMs");
    let birthtime_ms = ms_of(rt, "birthtimeMs");
    for key in [
        "dev",
        "mode",
        "nlink",
        "uid",
        "gid",
        "rdev",
        "blksize",
        "ino",
        "size",
        "blocks",
        "atimeMs",
        "mtimeMs",
        "ctimeMs",
        "birthtimeMs",
    ] {
        if let Value::Number(n) = rt.object_get(o, key) {
            let v = if n < 0.0 { 0 } else { n as u64 };
            rt.object_set(o, key.into(), bi(v));
        }
    }

    for (ns_key, ms) in [
        ("atimeNs", atime_ms),
        ("mtimeNs", mtime_ms),
        ("ctimeNs", ctime_ms),
        ("birthtimeNs", birthtime_ms),
    ] {
        let ns = if ms < 0.0 {
            0
        } else {
            (ms * 1_000_000.0) as u64
        };
        rt.object_set(o, ns_key.into(), bi(ns));
    }

    if let Value::Object(fs_global) = rt.global_get("fs") {
        if let Value::Object(proto) = rt.object_get(fs_global, "__bigintstats_proto") {
            rt.set_object_prototype_internal(o, Some(proto));
        }
    }
    o
}

fn dirent_object(
    rt: &mut Runtime,
    name: &str,
    parent_path: &str,
    ftype: Option<std::fs::FileType>,
) -> ObjectRef {
    let o = new_object(rt);
    rt.object_set(
        o,
        "name".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            name.to_string(),
        ))),
    );
    rt.object_set(
        o,
        "parentPath".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            parent_path.to_string(),
        ))),
    );

    let ifmt: u32 = match ftype {
        Some(f) if f.is_dir() => 0o040000,
        Some(f) if f.is_symlink() => 0o120000,
        Some(f) if f.is_file() => 0o100000,
        _ => 0,
    };
    rt.obj_mut(o)
        .set_own_internal("__dirent_type".into(), Value::Number(ifmt as f64));
    link_fs_class_prototype(rt, o, "Dirent");
    o
}

fn fs_iterator_result(rt: &mut Runtime, value: Value, done: bool) -> Value {
    let result = new_object(rt);
    rt.object_set(result, "value".into(), value);
    rt.object_set(result, "done".into(), Value::Boolean(done));
    Value::Object(result)
}

fn install_dir_async_iterator(rt: &mut Runtime, dir: ObjectRef) {
    register_method_internal(rt, dir, "@@asyncIterator", |rt, _args| {
        let dir = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let iter = new_object(rt);
        rt.set_engine_sentinel(iter, "__cruft_dir", Value::Object(dir));
        register_method(rt, iter, "next", |rt, _args| {
            let iter = match rt.current_this() {
                Value::Object(id) => id,
                _ => return Ok(Value::Undefined),
            };
            let dir = match rt.object_get(iter, "__cruft_dir") {
                Value::Object(id) => id,
                _ => return Ok(Value::Undefined),
            };
            let p = new_promise(rt);
            let read = rt.object_get(dir, "read");
            let entry = if rt.is_callable(&read) {
                rt.call_function(read, Value::Object(dir), Vec::new())?
            } else {
                Value::Null
            };
            let done = matches!(entry, Value::Null | Value::Undefined);
            let value = if done { Value::Undefined } else { entry };
            let result = fs_iterator_result(rt, value, done);
            resolve_promise(rt, p, result);
            Ok(Value::Object(p))
        });
        register_method(rt, iter, "return", |rt, _args| {
            if let Value::Object(iter) = rt.current_this() {
                if let Value::Object(dir) = rt.object_get(iter, "__cruft_dir") {
                    let close = rt.object_get(dir, "close");
                    if rt.is_callable(&close) {
                        let _ = rt.call_function(close, Value::Object(dir), Vec::new());
                    }
                }
            }
            let p = new_promise(rt);
            let result = fs_iterator_result(rt, Value::Undefined, true);
            resolve_promise(rt, p, result);
            Ok(Value::Object(p))
        });
        register_method_internal(rt, iter, "@@asyncIterator", |rt, _args| {
            Ok(rt.current_this())
        });
        Ok(Value::Object(iter))
    });
}

fn collect_readdir(
    base: &std::path::Path,
    recursive: bool,
) -> std::io::Result<Vec<(String, String, String, std::fs::FileType)>> {
    fn walk(
        cur: &std::path::Path,
        prefix: &str,
        recursive: bool,
        out: &mut Vec<(String, String, String, std::fs::FileType)>,
    ) -> std::io::Result<()> {
        for entry in std::fs::read_dir(cur)?.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let Ok(ft) = entry.file_type() else { continue };
            out.push((rel.clone(), cur.to_string_lossy().into_owned(), name, ft));
            if recursive && ft.is_dir() {
                walk(&entry.path(), &rel, recursive, out)?;
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(base, "", recursive, &mut out)?;
    Ok(out)
}

fn mkdtemp_suffix(attempt: u64) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);

    let mut z = nanos
        .wrapping_add(attempt.wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    let mut out = String::with_capacity(6);
    for _ in 0..6 {
        out.push(CHARS[(z % 62) as usize] as char);
        z /= 62;
    }
    out
}

fn utimes_arg_secs(rt: &Runtime, v: Option<&Value>) -> f64 {
    match v {
        Some(Value::Number(n)) => *n,
        Some(Value::Object(id)) => match rt.object_get(*id, "__date_ms") {
            Value::Number(ms) => ms / 1000.0,
            _ => 0.0,
        },
        Some(Value::String(s)) => s.as_str().parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    }
}

fn dir_object(rt: &mut Runtime, path: String) -> Result<ObjectRef, RuntimeError> {
    let entries: Vec<(String, Option<std::fs::FileType>)> = std::fs::read_dir(&path)
        .map_err(|e| fs_io_throw(rt, "opendir", e))?
        .flatten()
        .map(|e| {
            (
                e.file_name().to_string_lossy().into_owned(),
                e.file_type().ok(),
            )
        })
        .collect();
    let dir = new_object(rt);
    rt.obj_mut(dir).set_own_internal(
        "path".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            path.clone(),
        ))),
    );
    let entries_arr = rt.alloc_object(Object::new_array());
    for (i, (name, ftype)) in entries.iter().enumerate() {
        let de = dirent_object(rt, name, &path, *ftype);
        rt.object_set(entries_arr, i.to_string(), Value::Object(de));
    }
    rt.object_set(
        entries_arr,
        "length".into(),
        Value::Number(entries.len() as f64),
    );
    rt.obj_mut(dir)
        .set_own_internal("__entries".into(), Value::Object(entries_arr));
    rt.obj_mut(dir)
        .set_own_internal("__cursor".into(), Value::Number(0.0));
    register_method_internal(rt, dir, "read", |rt, _args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Null),
        };
        let entries = match rt.object_get(this, "__entries") {
            Value::Object(id) => id,
            _ => return Ok(Value::Null),
        };
        let cur = match rt.object_get(this, "__cursor") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        let len = match rt.object_get(entries, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        if cur >= len {
            return Ok(Value::Null);
        }
        let entry = rt.object_get(entries, &cur.to_string());
        rt.object_set(this, "__cursor".into(), Value::Number((cur + 1) as f64));
        Ok(entry)
    });
    register_method_internal(rt, dir, "close", |_rt, _args| Ok(Value::Undefined));

    register_method_internal(rt, dir, "readSync", |rt, _args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Null),
        };
        let entries = match rt.object_get(this, "__entries") {
            Value::Object(id) => id,
            _ => return Ok(Value::Null),
        };
        let cur = match rt.object_get(this, "__cursor") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        let len = match rt.object_get(entries, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        if cur >= len {
            return Ok(Value::Null);
        }
        let entry = rt.object_get(entries, &cur.to_string());
        rt.object_set(this, "__cursor".into(), Value::Number((cur + 1) as f64));
        Ok(entry)
    });
    register_method_internal(rt, dir, "closeSync", |_rt, _args| Ok(Value::Undefined));
    install_dir_async_iterator(rt, dir);

    link_fs_class_prototype(rt, dir, "Dir");
    Ok(dir)
}

fn reject_io(rt: &mut Runtime, p: ObjectRef, syscall: &str, path: &str, e: std::io::Error) {
    let code = io_err_code(&e);
    let desc = node_strerror(code)
        .map(str::to_string)
        .unwrap_or_else(|| e.to_string());
    let msg = format!("{code}: {desc}, {syscall} '{path}'");
    let err = fs_error_object_full(rt, code, &msg, Some(syscall), Some(path));
    reject_promise(rt, p, err);
}

pub fn install_canonical(rt: &mut Runtime) {
    let ns = new_object(rt);

    register_method(rt, ns, "read", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Read(path.clone().into()))?;
        let encoding = arg_encoding(rt, args, 1);
        let p = new_promise(rt);
        match std::fs::read(&path) {
            Ok(bytes) => {
                let v = bytes_to_value(rt, &bytes, encoding.as_deref());
                resolve_promise(rt, p, v);
            }
            Err(e) => reject_io(rt, p, "open", &path, e),
        }
        Ok(Value::Object(p))
    });

    register_method(rt, ns, "write", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Write(path.clone().into()))?;
        let encoding = arg_encoding(rt, args, 2);
        let data = args.get(1).cloned().unwrap_or(Value::Undefined);
        let bytes = value_to_bytes(rt, &data, encoding.as_deref());
        let p = new_promise(rt);
        match std::fs::write(&path, &bytes) {
            Ok(()) => resolve_promise(rt, p, Value::Undefined),
            Err(e) => reject_io(rt, p, "open", &path, e),
        }
        Ok(Value::Object(p))
    });

    register_method(rt, ns, "stat", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Stat(path.clone().into()))?;
        let p = new_promise(rt);
        match std::fs::metadata(&path) {
            Ok(md) => {
                let s = stat_object(rt, &md);
                resolve_promise(rt, p, Value::Object(s));
            }
            Err(e) => reject_io(rt, p, "stat", &path, e),
        }
        Ok(Value::Object(p))
    });

    register_method(rt, ns, "readdir", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::List(path.clone().into()))?;
        let p = new_promise(rt);
        match std::fs::read_dir(&path) {
            Ok(iter) => {
                let arr = rt.alloc_object(Object::new_array());
                let mut i = 0usize;
                for entry in iter.flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    rt.object_set(
                        arr,
                        i.to_string(),
                        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(name))),
                    );
                    i += 1;
                }
                rt.object_set(arr, "length".into(), Value::Number(i as f64));
                resolve_promise(rt, p, Value::Object(arr));
            }
            Err(e) => reject_io(rt, p, "scandir", &path, e),
        }
        Ok(Value::Object(p))
    });

    register_method(rt, ns, "mkdir", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Mkdir(path.clone().into()))?;
        let recursive = matches!(args.get(1), Some(Value::Boolean(true)));
        let p = new_promise(rt);
        let res = if recursive {
            std::fs::create_dir_all(&path)
        } else {
            std::fs::create_dir(&path)
        };
        match res {
            Ok(()) => resolve_promise(rt, p, Value::Undefined),
            Err(e) => reject_io(rt, p, "mkdir", &path, e),
        }
        Ok(Value::Object(p))
    });

    register_method(rt, ns, "remove", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Remove(path.clone().into()))?;
        let p = new_promise(rt);
        let is_dir = std::fs::metadata(&path)
            .map(|m| m.is_dir())
            .unwrap_or(false);
        let res = if is_dir {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        match res {
            Ok(()) => resolve_promise(rt, p, Value::Undefined),
            Err(e) => reject_io(rt, p, "unlink", &path, e),
        }
        Ok(Value::Object(p))
    });

    register_method(rt, ns, "exists", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Stat(path.clone().into()))?;
        let p = new_promise(rt);
        let exists = std::fs::metadata(&path).is_ok();
        resolve_promise(rt, p, Value::Boolean(exists));
        Ok(Value::Object(p))
    });

    rt.define_global_property("__cruft_fs", Value::Object(ns));
}

pub fn install(rt: &mut Runtime) {
    let fs = new_object(rt);
    let _fs_root = rt.push_temporary_value_roots(&[Value::Object(fs)]);

    register_method(rt, fs, "readFileSync", |rt, args| {

        if let Some(Value::Number(n)) = args.first() {
            use std::io::Read;
            let fd = *n as i32;
            let encoding = arg_encoding(rt, args, 1);
            let mut buf = Vec::new();
            let res = match rt.fd_table.get_mut(&fd) {
                Some(file) => file.read_to_end(&mut buf),
                None => {

                    return Err(RuntimeError::Thrown(fs_error_object_full(
                        rt,
                        "EBADF",
                        "EBADF: bad file descriptor, fstat",
                        Some("fstat"),
                        None,
                    )));
                }
            };
            return match res {
                Ok(_) => Ok(bytes_to_value(rt, &buf, encoding.as_deref())),
                Err(e) => Err(fs_io_throw(rt, "read", e)),
            };
        }
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Read(path.clone().into()))?;
        let encoding = arg_encoding(rt, args, 1);
        match std::fs::read(&path) {
            Ok(bytes) => Ok(bytes_to_value(rt, &bytes, encoding.as_deref())),

            Err(e) if io_err_code(&e) == "EISDIR" => Err(fs_io_throw(rt, "read", e)),
            Err(e) => Err(fs_throw(rt, "open", &path, e)),
        }
    });

    register_method(rt, fs, "writeFileSync", |rt, args| {

        fn write_file_open_options(flag: &str) -> std::fs::OpenOptions {
            let mut o = std::fs::OpenOptions::new();
            match flag {
                "a" => {
                    o.append(true).create(true);
                }
                "a+" => {
                    o.read(true).append(true).create(true);
                }
                "ax" | "xa" => {
                    o.append(true).create_new(true);
                }
                "ax+" | "xa+" => {
                    o.read(true).append(true).create_new(true);
                }
                "wx" | "xw" => {
                    o.write(true).create_new(true);
                }
                "wx+" | "xw+" => {
                    o.read(true).write(true).create_new(true);
                }
                "r+" => {
                    o.read(true).write(true);
                }
                "w+" => {
                    o.read(true).write(true).create(true).truncate(true);
                }
                _ => {

                    o.write(true).create(true).truncate(true);
                }
            }
            o
        }
        let encoding = arg_encoding(rt, args, 2);

        if let Some(Value::Number(n)) = args.first() {
            use std::io::Write as _;
            let fd = *n as i32;
            let data = match args.get(1) {
                Some(v) => value_to_bytes(rt, v, encoding.as_deref()),
                None => Vec::new(),
            };
            let res = match rt.fd_table.get_mut(&fd) {
                Some(file) => file.write_all(&data),
                None => {
                    return Err(RuntimeError::Thrown(fs_error_object_full(
                        rt,
                        "EBADF",
                        "EBADF: bad file descriptor, write",
                        Some("write"),
                        None,
                    )));
                }
            };
            return match res {
                Ok(()) => Ok(Value::Undefined),
                Err(e) => Err(fs_io_throw(rt, "write", e)),
            };
        }
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Write(path.clone().into()))?;
        let data = match args.get(1) {
            Some(v) => value_to_bytes(rt, v, encoding.as_deref()),
            None => Vec::new(),
        };

        let (flag, mode) = match args.get(2) {
            Some(Value::Object(id)) => {
                let flag = match rt.object_get(*id, "flag") {
                    Value::String(s) => s.as_str().to_string(),
                    _ => "w".to_string(),
                };
                let mode = match rt.object_get(*id, "mode") {
                    Value::Number(n) if n.is_finite() && n >= 0.0 => Some(n as u32),
                    _ => None,
                };
                (flag, mode)
            }
            _ => ("w".to_string(), None),
        };
        let mut opts = write_file_open_options(&flag);
        #[cfg(unix)]
        if let Some(m) = mode {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(m);
        }
        #[cfg(not(unix))]
        let _ = mode;
        use std::io::Write;
        match opts.open(&path) {
            Ok(mut f) => f
                .write_all(&data)
                .map(|_| Value::Undefined)
                .map_err(|e| fs_io_throw(rt, "write", e)),
            Err(e) => Err(fs_io_throw(rt, "open", e)),
        }
    });

    register_method(rt, fs, "existsSync", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Stat(path.clone().into()))?;
        Ok(Value::Boolean(std::path::Path::new(&path).exists()))
    });

    register_method(rt, fs, "statSync", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        let throw_if_no_entry = stat_throw_if_no_entry(rt, args, 1);
        let bigint = stat_bigint(rt, args, 1);
        check_fs(rt, caps::FsOp::Stat(path.clone().into()))?;
        match std::fs::metadata(&path) {
            Ok(md) => Ok(Value::Object(stat_object_opt(rt, &md, bigint))),
            Err(e) if !throw_if_no_entry && e.kind() == std::io::ErrorKind::NotFound => {
                Ok(Value::Undefined)
            }
            Err(e) => Err(fs_throw(rt, "stat", &path, e)),
        }
    });

    register_method(rt, fs, "lstatSync", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        let throw_if_no_entry = stat_throw_if_no_entry(rt, args, 1);
        let bigint = stat_bigint(rt, args, 1);
        check_fs(rt, caps::FsOp::Stat(path.clone().into()))?;
        match std::fs::symlink_metadata(&path) {
            Ok(md) => Ok(Value::Object(stat_object_opt(rt, &md, bigint))),
            Err(e) if !throw_if_no_entry && e.kind() == std::io::ErrorKind::NotFound => {
                Ok(Value::Undefined)
            }
            Err(e) => Err(fs_throw(rt, "lstat", &path, e)),
        }
    });

    register_method(rt, fs, "readdirSync", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::List(path.clone().into()))?;

        let with_file_types = matches!(args.get(1),
            Some(Value::Object(id)) if matches!(rt.object_get(*id, "withFileTypes"), Value::Boolean(true)));
        let recursive = matches!(args.get(1),
            Some(Value::Object(id)) if matches!(rt.object_get(*id, "recursive"), Value::Boolean(true)));
        match collect_readdir(std::path::Path::new(&path), recursive) {
            Ok(entries) => {
                let arr = rt.alloc_object(Object::new_array());
                for (i, (rel, parent, name, ft)) in entries.iter().enumerate() {
                    let val = if with_file_types {
                        Value::Object(dirent_object(rt, name, parent, Some(*ft)))
                    } else {
                        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                            rel.clone(),
                        )))
                    };
                    rt.object_set(arr, i.to_string(), val);
                }
                rt.object_set(arr, "length".into(), Value::Number(entries.len() as f64));
                Ok(Value::Object(arr))
            }
            Err(e) => Err(fs_throw(rt, "scandir", &path, e)),
        }
    });

    register_method(rt, fs, "readdir", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::List(path.clone().into()))?;
        let with_file_types = matches!(args.get(1),
            Some(Value::Object(id)) if matches!(rt.object_get(*id, "withFileTypes"), Value::Boolean(true)));
        let recursive = matches!(args.get(1),
            Some(Value::Object(id)) if matches!(rt.object_get(*id, "recursive"), Value::Boolean(true)));
        let p = new_promise(rt);
        let callback = trailing_callback(rt, args);
        let has_callback = callback.is_some();
        push_pending(
            rt,
            p,
            FsOp::ReadDir {
                path,
                with_file_types,
                recursive,
            },
            callback,
        );
        if has_callback {
            Ok(Value::Undefined)
        } else {
            Ok(Value::Object(p))
        }
    });

    register_method(rt, fs, "mkdirSync", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Mkdir(path.clone().into()))?;
        let recursive = match args.get(1) {
            Some(Value::Object(id)) => {
                matches!(rt.object_get(*id, "recursive"), Value::Boolean(true))
            }
            _ => false,
        };

        let mode: Option<u32> = match args.get(1) {
            Some(Value::Number(n)) if n.is_finite() && *n >= 0.0 => Some(*n as u32),
            Some(Value::Object(id)) => match rt.object_get(*id, "mode") {
                Value::Number(n) if n.is_finite() && n >= 0.0 => Some(n as u32),
                _ => None,
            },
            _ => None,
        };

        let first_created = if recursive {
            let mut cur = std::path::Path::new(&path);
            let mut shallowest: Option<String> = None;
            while !cur.exists() {
                shallowest = Some(cur.to_string_lossy().to_string());
                match cur.parent() {
                    Some(par) if !par.as_os_str().is_empty() => cur = par,
                    _ => break,
                }
            }
            shallowest
        } else {
            None
        };
        let mut db = std::fs::DirBuilder::new();
        db.recursive(recursive);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            if let Some(m) = mode {
                db.mode(m);
            }
        }
        let _ = mode;
        let r = db.create(&path);
        match r {
            Ok(()) => Ok(match first_created {
                Some(p) => Value::String(Rc::new(rusty_js_runtime::value::JsString::from(p))),
                None => Value::Undefined,
            }),
            Err(e) => Err(fs_throw(rt, "mkdir", &path, e)),
        }
    });

    register_method(rt, fs, "unlinkSync", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Remove(path.clone().into()))?;
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(Value::Undefined),
            Err(e) => Err(fs_throw(rt, "unlink", &path, e)),
        }
    });

    register_method(rt, fs, "readFile", |rt, args| {
        let encoding = arg_encoding(rt, args, 1);
        let p = new_promise(rt);
        let callback = trailing_callback(rt, args);
        let has_callback = callback.is_some();

        if let Some(Value::Number(n)) = args.first() {
            push_pending(
                rt,
                p,
                FsOp::ReadFd {
                    fd: *n as i32,
                    encoding,
                },
                callback,
            );
        } else {
            let path = arg_path_or_url(rt, args, 0);
            check_fs(rt, caps::FsOp::Read(path.clone().into()))?;
            push_pending(rt, p, FsOp::Read { path, encoding }, callback);
        }
        if has_callback {
            Ok(Value::Undefined)
        } else {
            Ok(Value::Object(p))
        }
    });

    register_method(rt, fs, "writeFile", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Write(path.clone().into()))?;
        let encoding = arg_encoding(rt, args, 2);
        let data = match args.get(1) {
            Some(v) => value_to_bytes(rt, v, encoding.as_deref()),
            None => Vec::new(),
        };
        let p = new_promise(rt);
        let callback = trailing_callback(rt, args);
        let has_callback = callback.is_some();
        push_pending(rt, p, FsOp::Write { path, data }, callback);
        if has_callback {
            Ok(Value::Undefined)
        } else {
            Ok(Value::Object(p))
        }
    });

    register_method(rt, fs, "exists", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Stat(path.clone().into()))?;
        let p = new_promise(rt);
        let callback = trailing_callback(rt, args);
        let has_callback = callback.is_some();
        push_pending(rt, p, FsOp::Exists { path }, callback);
        if has_callback {
            Ok(Value::Undefined)
        } else {
            Ok(Value::Object(p))
        }
    });

    register_method(rt, fs, "stat", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Stat(path.clone().into()))?;
        let result = match std::fs::metadata(&path) {
            Ok(md) => Ok(Value::Object(stat_object(rt, &md))),
            Err(e) => {
                let t = fs_io_throw(rt, "stat", e);
                Err(fs_error_from_runtime(rt, t))
            }
        };
        Ok(dispatch_async_result(rt, args, result))
    });

    register_method(rt, fs, "lstat", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Stat(path.clone().into()))?;
        let result = match std::fs::symlink_metadata(&path) {
            Ok(md) => Ok(Value::Object(stat_object(rt, &md))),
            Err(e) => {
                let t = fs_io_throw(rt, "lstat", e);
                Err(fs_error_from_runtime(rt, t))
            }
        };
        Ok(dispatch_async_result(rt, args, result))
    });

    let promises = new_object(rt);
    let _promises_root = rt.push_temporary_value_roots(&[Value::Object(promises)]);
    register_method(rt, promises, "stat", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Stat(path.clone().into()))?;
        let p = new_promise(rt);
        match std::fs::metadata(&path) {
            Ok(md) => {
                let stat = stat_object(rt, &md);
                resolve_promise(rt, p, Value::Object(stat));
            }
            Err(e) => {
                let reason = match fs_throw(rt, "stat", &path, e) {
                    RuntimeError::Thrown(value) => value,
                    other => Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                        format!("{other:?}"),
                    ))),
                };
                reject_promise(rt, p, reason);
            }
        }
        Ok(Value::Object(p))
    });
    register_method(rt, promises, "readFile", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Read(path.clone().into()))?;
        let encoding = arg_encoding(rt, args, 1);
        match std::fs::read(&path) {
            Ok(bytes) => Ok(bytes_to_value(rt, &bytes, encoding.as_deref())),

            Err(e) if io_err_code(&e) == "EISDIR" => Err(fs_io_throw(rt, "read", e)),
            Err(e) => Err(fs_throw(rt, "open", &path, e)),
        }
    });

    register_method(rt, promises, "readdir", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::List(path.clone().into()))?;
        let p = new_promise(rt);
        let with_file_types = matches!(args.get(1),
            Some(Value::Object(id)) if matches!(rt.object_get(*id, "withFileTypes"), Value::Boolean(true)));
        let recursive = matches!(args.get(1),
            Some(Value::Object(id)) if matches!(rt.object_get(*id, "recursive"), Value::Boolean(true)));
        match collect_readdir(std::path::Path::new(&path), recursive) {
            Ok(entries) => {
                let arr = rt.alloc_object(Object::new_array());
                for (i, (rel, parent, name, ft)) in entries.iter().enumerate() {
                    let val = if with_file_types {
                        Value::Object(dirent_object(rt, name, parent, Some(*ft)))
                    } else {
                        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                            rel.clone(),
                        )))
                    };
                    rt.object_set(arr, i.to_string(), val);
                }
                rt.object_set(arr, "length".into(), Value::Number(entries.len() as f64));
                resolve_promise(rt, p, Value::Object(arr));
            }
            Err(e) => {
                let reason = match fs_throw(rt, "scandir", &path, e) {
                    RuntimeError::Thrown(value) => value,
                    other => Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                        format!("{other:?}"),
                    ))),
                };
                reject_promise(rt, p, reason);
            }
        }
        Ok(Value::Object(p))
    });
    register_method(rt, promises, "writeFile", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Write(path.clone().into()))?;
        let encoding = arg_encoding(rt, args, 2);
        let data = match args.get(1) {
            Some(v) => value_to_bytes(rt, v, encoding.as_deref()),
            None => Vec::new(),
        };
        match std::fs::write(&path, &data) {
            Ok(()) => Ok(Value::Undefined),
            Err(e) => Err(fs_io_throw(rt, "open", e)),
        }
    });
    register_method(rt, promises, "access", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Stat(path.clone().into()))?;
        if std::path::Path::new(&path).exists() {
            Ok(Value::Undefined)
        } else {
            Err(RuntimeError::Thrown(fs_error_object(
                rt,
                "ENOENT",
                &format!("ENOENT: no such file or directory, access '{path}'"),
            )))
        }
    });
    register_method(rt, promises, "mkdir", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Mkdir(path.clone().into()))?;
        match std::fs::create_dir_all(&path) {
            Ok(()) => Ok(Value::Undefined),
            Err(e) => Err(fs_io_throw(rt, "mkdir", e)),
        }
    });
    register_method(rt, promises, "unlink", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Remove(path.clone().into()))?;
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(Value::Undefined),
            Err(e) => Err(fs_io_throw(rt, "unlink", e)),
        }
    });
    register_method(rt, promises, "appendFile", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Write(path.clone().into()))?;
        let enc = arg_encoding(rt, args, 2);
        let data = match args.get(1) {
            Some(v) => value_to_bytes(rt, v, enc.as_deref()),
            None => Vec::new(),
        };
        let mut f = match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            Ok(f) => f,
            Err(e) => return Err(fs_io_throw(rt, "open", e)),
        };
        if let Err(e) = std::io::Write::write_all(&mut f, &data) {
            return Err(fs_io_throw(rt, "open", e));
        }
        Ok(Value::Undefined)
    });
    register_method(rt, promises, "copyFile", |rt, args| {
        let src = arg_string(args, 0);
        let dst = arg_string(args, 1);
        check_fs(rt, caps::FsOp::Read(src.clone().into()))?;
        check_fs(rt, caps::FsOp::Write(dst.clone().into()))?;
        if std::fs::metadata(&src).map(|m| m.is_dir()).unwrap_or(false) {
            let dst_parent_missing = std::path::Path::new(&dst)
                .parent()
                .map(|par| !par.as_os_str().is_empty() && !par.exists())
                .unwrap_or(false);
            if dst_parent_missing {
                return Err(RuntimeError::Thrown(fs_error_object(
                    rt,
                    "ENOENT",
                    &format!("ENOENT: no such file or directory, copyfile '{src}' -> '{dst}'"),
                )));
            }
            return Err(RuntimeError::Thrown(fs_error_object(
                rt,
                "EISDIR",
                &format!("EISDIR: illegal operation on a directory, copyfile '{src}' -> '{dst}'"),
            )));
        }
        match std::fs::copy(&src, &dst) {
            Ok(_) => Ok(Value::Undefined),
            Err(e) => Err(fs_io_throw(rt, "copyfile", e)),
        }
    });
    register_method(rt, promises, "rename", |rt, args| {
        let src = arg_string(args, 0);
        let dst = arg_string(args, 1);
        check_fs(rt, caps::FsOp::Write(src.clone().into()))?;
        check_fs(rt, caps::FsOp::Write(dst.clone().into()))?;
        match std::fs::rename(&src, &dst) {
            Ok(()) => Ok(Value::Undefined),
            Err(e) => Err(fs_io_throw(rt, "rename", e)),
        }
    });
    register_method(rt, promises, "rm", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Remove(path.clone().into()))?;
        let (recursive, force) = match args.get(1) {
            Some(Value::Object(id)) => (
                matches!(rt.object_get(*id, "recursive"), Value::Boolean(true)),
                matches!(rt.object_get(*id, "force"), Value::Boolean(true)),
            ),
            _ => (false, false),
        };
        let md = std::fs::symlink_metadata(&path);
        let res = match md {
            Ok(m) if m.is_dir() => {
                if recursive {
                    std::fs::remove_dir_all(&path)
                } else {
                    std::fs::remove_dir(&path)
                }
            }
            Ok(_) => std::fs::remove_file(&path),
            Err(e) => {
                if force {
                    return Ok(Value::Undefined);
                } else {
                    return Err(fs_io_throw(rt, "rm", e));
                }
            }
        };
        match res {
            Ok(()) => Ok(Value::Undefined),
            Err(_) if force => Ok(Value::Undefined),
            Err(e) => Err(fs_io_throw(rt, "rm", e)),
        }
    });
    register_method(rt, promises, "readlink", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Read(path.clone().into()))?;
        match std::fs::read_link(&path) {
            Ok(t) => Ok(Value::String(Rc::new(
                rusty_js_runtime::value::JsString::from(t.to_string_lossy().into_owned()),
            ))),
            Err(e) => Err(fs_io_throw(rt, "readlink", e)),
        }
    });
    register_method(rt, promises, "realpath", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Read(path.clone().into()))?;
        let p = new_promise(rt);
        match std::fs::canonicalize(&path) {
            Ok(t) => {
                resolve_promise(
                    rt,
                    p,
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                        t.to_string_lossy().into_owned(),
                    ))),
                );
            }
            Err(e) => {
                let reason = match fs_throw(rt, "realpath", &path, e) {
                    RuntimeError::Thrown(value) => value,
                    other => Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                        format!("{other:?}"),
                    ))),
                };
                reject_promise(rt, p, reason);
            }
        }
        Ok(Value::Object(p))
    });
    register_method(rt, promises, "lstat", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Stat(path.clone().into()))?;
        let p = new_promise(rt);
        match std::fs::symlink_metadata(&path) {
            Ok(md) => {
                let stat = stat_object(rt, &md);
                resolve_promise(rt, p, Value::Object(stat));
            }
            Err(e) => {
                let reason = match fs_throw(rt, "lstat", &path, e) {
                    RuntimeError::Thrown(value) => value,
                    other => Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                        format!("{other:?}"),
                    ))),
                };
                reject_promise(rt, p, reason);
            }
        }
        Ok(Value::Object(p))
    });
    register_method(rt, promises, "mkdtemp", |rt, args| {
        let prefix = arg_string(args, 0);
        check_fs(rt, caps::FsOp::Mkdir(prefix.clone().into()))?;
        use std::sync::atomic::{AtomicU64, Ordering};
        static CTR: AtomicU64 = AtomicU64::new(0);
        let mut last: Option<std::io::Error> = None;
        for _ in 0..256 {
            let n = CTR.fetch_add(1, Ordering::Relaxed);

            let cand = format!("{prefix}{}", mkdtemp_suffix(n));
            match std::fs::create_dir(&cand) {
                Ok(()) => {
                    return Ok(Value::String(Rc::new(
                        rusty_js_runtime::value::JsString::from(cand),
                    )))
                }
                Err(e) => last = Some(e),
            }
        }
        Err(fs_io_throw(
            rt,
            "mkdtemp",
            last.unwrap_or_else(|| std::io::Error::other("mkdtemp")),
        ))
    });
    rt.object_set(fs, "promises".into(), Value::Object(promises));

    let create_read_stream = make_callable(rt, "createReadStream", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Read(path.clone().into()))?;
        Ok(Value::Object(make_read_stream(
            rt,
            path,
            args.get(1).cloned(),
        )))
    });
    rt.object_set(
        fs,
        "createReadStream".into(),
        Value::Object(create_read_stream),
    );
    let create_write_stream = make_callable(rt, "createWriteStream", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Write(path.clone().into()))?;
        Ok(Value::Object(make_write_stream(
            rt,
            path,
            args.get(1).cloned(),
        )))
    });
    rt.object_set(
        fs,
        "createWriteStream".into(),
        Value::Object(create_write_stream),
    );

    for (name, global_key) in [
        ("ReadStream", "__cruft_fs_readstream_proto"),
        ("WriteStream", "__cruft_fs_writestream_proto"),
    ] {
        let proto = new_object(rt);
        let ctor = make_callable(rt, name, |_rt, _a| {
            Err(RuntimeError::TypeError(
                "fs stream constructors are created via createReadStream/createWriteStream".into(),
            ))
        });
        rt.set_own_frozen_property(ctor, "prototype".into(), Value::Object(proto));
        rt.obj_mut(proto)
            .set_own_internal("constructor".into(), Value::Object(ctor));
        rt.object_set(fs, name.into(), Value::Object(ctor));
        rt.define_global_property(global_key, Value::Object(proto));
    }

    for stub in &[
        "chown", "lchmod", "lchown",

        "readv", "writev",
    ] {
        let nm: &'static str = stub;
        let cb = make_callable(rt, nm, move |_rt, _args| {
            Err(RuntimeError::TypeError(format!(
                "fs.{} is not yet implemented in cruft host-v2",
                nm
            )))
        });
        rt.object_set(fs, (*stub).into(), Value::Object(cb));
    }

    let realpath = make_callable(rt, "realpath", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Read(path.clone().into()))?;
        let Some(callback) = trailing_callback(rt, args) else {
            return Err(RuntimeError::TypeError(
                "The \"cb\" argument must be of type function".to_string(),
            ));
        };
        let result = match std::fs::canonicalize(&path) {
            Ok(t) => Ok(Value::String(std::rc::Rc::new(
                rusty_js_runtime::value::JsString::from(t.to_string_lossy().into_owned()),
            ))),
            Err(e) => {
                let thrown = fs_io_throw(rt, "realpath", e);
                Err(fs_error_from_runtime(rt, thrown))
            }
        };
        let cb_args = match result {
            Ok(v) => vec![Value::Null, v],
            Err(e) => vec![e],
        };
        let roots = crate::timer::roots_for_callback(&callback, &cb_args);
        rt.enqueue_host_phase_rooted(
            HostEnqueuePhase::HostCompletionMacrotask,
            "fs.realpath callback",
            roots,
            move |rt| {
                let _ = rt.call_function(callback, Value::Undefined, cb_args);
                Ok(())
            },
        );
        Ok(Value::Undefined)
    });
    let realpath_native = make_callable(rt, "realpath", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Read(path.clone().into()))?;
        let Some(callback) = trailing_callback(rt, args) else {
            return Err(RuntimeError::TypeError(
                "The \"cb\" argument must be of type function".to_string(),
            ));
        };
        let result = match std::fs::canonicalize(&path) {
            Ok(t) => Ok(Value::String(std::rc::Rc::new(
                rusty_js_runtime::value::JsString::from(t.to_string_lossy().into_owned()),
            ))),
            Err(e) => {
                let thrown = fs_io_throw(rt, "realpath", e);
                Err(fs_error_from_runtime(rt, thrown))
            }
        };
        let cb_args = match result {
            Ok(v) => vec![Value::Null, v],
            Err(e) => vec![e],
        };
        let roots = crate::timer::roots_for_callback(&callback, &cb_args);
        rt.enqueue_host_phase_rooted(
            HostEnqueuePhase::HostCompletionMacrotask,
            "fs.realpath.native callback",
            roots,
            move |rt| {
                let _ = rt.call_function(callback, Value::Undefined, cb_args);
                Ok(())
            },
        );
        Ok(Value::Undefined)
    });
    rt.object_set(realpath, "native".into(), Value::Object(realpath_native));
    rt.object_set(fs, "realpath".into(), Value::Object(realpath));

    let realpath_sync = make_callable(rt, "realpathSync", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Read(path.clone().into()))?;
        std::fs::canonicalize(&path)
            .map(|t| {
                Value::String(std::rc::Rc::new(rusty_js_runtime::value::JsString::from(
                    t.to_string_lossy().into_owned(),
                )))
            })
            .map_err(|e| {
                if io_err_code(&e) == "ENOENT" {
                    let missing = realpath_first_missing(&path);
                    fs_throw(rt, "lstat", &missing, e)
                } else {
                    fs_throw(rt, "lstat", &path, e)
                }
            })
    });
    let realpath_sync_native = make_callable(rt, "realpathSync", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Read(path.clone().into()))?;
        std::fs::canonicalize(&path)
            .map(|t| {
                Value::String(std::rc::Rc::new(rusty_js_runtime::value::JsString::from(
                    t.to_string_lossy().into_owned(),
                )))
            })
            .map_err(|e| fs_throw(rt, "realpath", &path, e))
    });
    rt.object_set(
        realpath_sync,
        "native".into(),
        Value::Object(realpath_sync_native),
    );
    rt.object_set(fs, "realpathSync".into(), Value::Object(realpath_sync));

    let constants = new_object(rt);
    let consts: &[(&str, f64)] = &[
        ("F_OK", 0.0),
        ("R_OK", 4.0),
        ("W_OK", 2.0),
        ("X_OK", 1.0),
        ("O_RDONLY", 0.0),
        ("O_WRONLY", 1.0),
        ("O_RDWR", 2.0),
        ("O_CREAT", 64.0),
        ("O_EXCL", 128.0),
        ("O_NOCTTY", 256.0),
        ("O_TRUNC", 512.0),
        ("O_APPEND", 1024.0),
        ("O_DIRECTORY", 65536.0),
        ("O_NOFOLLOW", 131072.0),
        ("O_SYNC", 1052672.0),
        ("O_DSYNC", 4096.0),

        ("O_NONBLOCK", 2048.0),
        ("O_DIRECT", 16384.0),
        ("O_NOATIME", 262144.0),
        ("S_IFMT", 61440.0),
        ("S_IFREG", 32768.0),
        ("S_IFDIR", 16384.0),
        ("S_IFCHR", 8192.0),
        ("S_IFBLK", 24576.0),
        ("S_IFIFO", 4096.0),
        ("S_IFLNK", 40960.0),
        ("S_IFSOCK", 49152.0),
        ("S_IRWXU", 448.0),
        ("S_IRUSR", 256.0),
        ("S_IWUSR", 128.0),
        ("S_IXUSR", 64.0),
        ("S_IRWXG", 56.0),
        ("S_IRGRP", 32.0),
        ("S_IWGRP", 16.0),
        ("S_IXGRP", 8.0),
        ("S_IRWXO", 7.0),
        ("S_IROTH", 4.0),
        ("S_IWOTH", 2.0),
        ("S_IXOTH", 1.0),
        ("COPYFILE_EXCL", 1.0),
        ("COPYFILE_FICLONE", 2.0),
        ("COPYFILE_FICLONE_FORCE", 4.0),

        ("UV_FS_COPYFILE_EXCL", 1.0),
        ("UV_FS_COPYFILE_FICLONE", 2.0),
        ("UV_FS_COPYFILE_FICLONE_FORCE", 4.0),
        ("UV_FS_SYMLINK_DIR", 1.0),
        ("UV_FS_SYMLINK_JUNCTION", 2.0),
        ("UV_FS_O_FILEMAP", 0.0),
        ("UV_DIRENT_UNKNOWN", 0.0),
        ("UV_DIRENT_FILE", 1.0),
        ("UV_DIRENT_DIR", 2.0),
        ("UV_DIRENT_LINK", 3.0),

        ("UV_DIRENT_FIFO", 4.0),
        ("UV_DIRENT_SOCKET", 5.0),
        ("UV_DIRENT_CHAR", 6.0),
        ("UV_DIRENT_BLOCK", 7.0),
    ];
    for (name, val) in consts {
        rt.object_set(constants, (*name).into(), Value::Number(*val));
    }
    rt.object_set(fs, "constants".into(), Value::Object(constants));

    register_method(rt, fs, "accessSync", |rt, args| {
        let _ = check_fs(rt, caps::FsOp::Stat(arg_string(args, 0).into()))?;
        let path = arg_path_or_url(rt, args, 0);
        let mode = match args.get(1) {
            Some(Value::Number(n)) => *n as u32,
            _ => 0,
        };
        match std::fs::metadata(&path) {
            Ok(md) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let perms = md.permissions().mode();

                    let need_r = mode & 4 != 0;
                    let need_w = mode & 2 != 0;
                    let need_x = mode & 1 != 0;
                    if need_r && perms & 0o400 == 0 {
                        return Err(RuntimeError::TypeError(format!(
                            "accessSync: EACCES on '{}'",
                            path
                        )));
                    }
                    if need_w && perms & 0o200 == 0 {
                        return Err(RuntimeError::TypeError(format!(
                            "accessSync: EACCES on '{}'",
                            path
                        )));
                    }
                    if need_x && perms & 0o100 == 0 {
                        return Err(RuntimeError::TypeError(format!(
                            "accessSync: EACCES on '{}'",
                            path
                        )));
                    }
                }
                let _ = rt;
                Ok(Value::Undefined)
            }
            Err(e) => Err(fs_throw(rt, "access", &path, e)),
        }
    });

    register_method(rt, fs, "appendFileSync", |rt, args| {
        use std::io::Write;
        let encoding = arg_encoding(rt, args, 2);

        if let Some(Value::Number(n)) = args.first() {
            let fd = *n as i32;
            let data = match args.get(1) {
                Some(v) => value_to_bytes(rt, v, encoding.as_deref()),
                None => Vec::new(),
            };
            let res = match rt.fd_table.get_mut(&fd) {
                Some(file) => file.write_all(&data),
                None => {
                    return Err(RuntimeError::Thrown(fs_error_object_full(
                        rt,
                        "EBADF",
                        "EBADF: bad file descriptor, write",
                        Some("write"),
                        None,
                    )));
                }
            };
            return match res {
                Ok(()) => Ok(Value::Undefined),
                Err(e) => Err(fs_io_throw(rt, "write", e)),
            };
        }
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Write(path.clone().into()))?;
        let data = match args.get(1) {
            Some(v) => value_to_bytes(rt, v, encoding.as_deref()),
            None => Vec::new(),
        };

        let mut oo = std::fs::OpenOptions::new();
        oo.create(true).append(true);
        #[cfg(unix)]
        if let Some(Value::Object(id)) = args.get(2) {
            if let Value::Number(n) = rt.object_get(*id, "mode") {
                if n.is_finite() && n >= 0.0 {
                    use std::os::unix::fs::OpenOptionsExt;
                    oo.mode(n as u32);
                }
            }
        }
        let mut file = match oo.open(&path) {
            Ok(f) => f,
            Err(e) => return Err(fs_throw(rt, "open", &path, e)),
        };
        if let Err(e) = file.write_all(&data) {
            return Err(fs_throw(rt, "open", &path, e));
        }
        Ok(Value::Undefined)
    });

    register_method(rt, fs, "copyFileSync", |rt, args| {
        let src = arg_string(args, 0);
        let dst = arg_string(args, 1);
        check_fs(rt, caps::FsOp::Read(src.clone().into()))?;
        check_fs(rt, caps::FsOp::Write(dst.clone().into()))?;

        if std::fs::metadata(&src).map(|m| m.is_dir()).unwrap_or(false) {
            let dst_parent_missing = std::path::Path::new(&dst)
                .parent()
                .map(|par| !par.as_os_str().is_empty() && !par.exists())
                .unwrap_or(false);
            if dst_parent_missing {
                return Err(RuntimeError::Thrown(fs_error_object(
                    rt,
                    "ENOENT",
                    &format!("ENOENT: no such file or directory, copyfile '{src}' -> '{dst}'"),
                )));
            }
            return Err(RuntimeError::Thrown(fs_error_object(
                rt,
                "EISDIR",
                &format!("EISDIR: illegal operation on a directory, copyfile '{src}' -> '{dst}'"),
            )));
        }
        match std::fs::copy(&src, &dst) {
            Ok(_) => Ok(Value::Undefined),
            Err(e) => Err(fs_throw(rt, "copyfile", &src, e)),
        }
    });

    register_method(rt, fs, "cpSync", |rt, args| {
        let src = arg_string(args, 0);
        let dst = arg_string(args, 1);
        check_fs(rt, caps::FsOp::Read(src.clone().into()))?;
        check_fs(rt, caps::FsOp::Write(dst.clone().into()))?;
        let recursive = match args.get(2) {
            Some(Value::Object(id)) => {
                matches!(rt.object_get(*id, "recursive"), Value::Boolean(true))
            }
            _ => false,
        };

        let (filter, force, error_on_exist) = match args.get(2) {
            Some(Value::Object(id)) => {
                let f = rt.object_get(*id, "filter");
                let filter = if rt.is_callable(&f) { Some(f) } else { None };
                let force = !matches!(rt.object_get(*id, "force"), Value::Boolean(false));
                let eoe = matches!(rt.object_get(*id, "errorOnExist"), Value::Boolean(true));
                (filter, force, eoe)
            }
            _ => (None, true, false),
        };
        if filter.is_some() || !force || error_on_exist {
            cp_walk(
                rt,
                std::path::Path::new(&src),
                std::path::Path::new(&dst),
                recursive,
                filter.as_ref(),
                force,
                error_on_exist,
            )?;
        } else {
            cp_recursive(
                std::path::Path::new(&src),
                std::path::Path::new(&dst),
                recursive,
            )
            .map_err(|e| RuntimeError::TypeError(format!("cpSync: {}", e)))?;
        }
        Ok(Value::Undefined)
    });

    register_method(rt, fs, "linkSync", |rt, args| {
        let src = arg_string(args, 0);
        let dst = arg_string(args, 1);
        check_fs(rt, caps::FsOp::Read(src.clone().into()))?;
        check_fs(rt, caps::FsOp::Write(dst.clone().into()))?;
        std::fs::hard_link(&src, &dst)
            .map(|_| Value::Undefined)
            .map_err(|e| RuntimeError::TypeError(format!("linkSync: {}", e)))
    });

    register_method(rt, fs, "readlinkSync", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Read(path.clone().into()))?;
        match std::fs::read_link(&path) {
            Ok(p) => Ok(Value::String(Rc::new(
                rusty_js_runtime::value::JsString::from(p.to_string_lossy().into_owned()),
            ))),
            Err(e) => Err(fs_throw(rt, "readlink", &path, e)),
        }
    });

    register_method(rt, fs, "renameSync", |rt, args| {
        let src = arg_string(args, 0);
        let dst = arg_string(args, 1);
        check_fs(rt, caps::FsOp::Read(src.clone().into()))?;
        check_fs(rt, caps::FsOp::Write(src.clone().into()))?;
        check_fs(rt, caps::FsOp::Write(dst.clone().into()))?;
        match std::fs::rename(&src, &dst) {
            Ok(_) => Ok(Value::Undefined),
            Err(e) => Err(fs_throw(rt, "rename", &src, e)),
        }
    });

    register_method(rt, fs, "rmSync", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Remove(path.clone().into()))?;
        let (recursive, force) = match args.get(1) {
            Some(Value::Object(id)) => (
                matches!(rt.object_get(*id, "recursive"), Value::Boolean(true)),
                matches!(rt.object_get(*id, "force"), Value::Boolean(true)),
            ),
            _ => (false, false),
        };
        let p = std::path::Path::new(&path);
        let r = if p.is_dir() {
            if recursive {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_dir(&path)
            }
        } else {
            std::fs::remove_file(&path)
        };
        match r {
            Ok(()) => Ok(Value::Undefined),
            Err(e) if force && e.kind() == std::io::ErrorKind::NotFound => Ok(Value::Undefined),
            Err(e) => Err(fs_throw(rt, "lstat", &path, e)),
        }
    });

    register_method(rt, fs, "rmdirSync", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Remove(path.clone().into()))?;
        let recursive = match args.get(1) {
            Some(Value::Object(id)) => {
                matches!(rt.object_get(*id, "recursive"), Value::Boolean(true))
            }
            _ => false,
        };
        let r = if recursive {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_dir(&path)
        };
        r.map(|_| Value::Undefined)
            .map_err(|e| fs_throw(rt, "rmdir", &path, e))
    });

    register_method(rt, fs, "symlinkSync", |rt, args| {
        let target = arg_string(args, 0);
        let link = arg_string(args, 1);
        check_fs(rt, caps::FsOp::Read(target.clone().into()))?;
        check_fs(rt, caps::FsOp::Write(link.clone().into()))?;
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&target, &link)
                .map(|_| Value::Undefined)
                .map_err(|e| RuntimeError::TypeError(format!("symlinkSync: {}", e)))
        }
        #[cfg(windows)]
        {

            use std::os::windows::fs::{symlink_dir, symlink_file};
            let link_type = arg_string(args, 2);
            let want_dir = match link_type.as_str() {
                "dir" | "junction" => true,
                "file" => false,
                _ => std::path::Path::new(&target).is_dir(),
            };
            let res = if want_dir {
                symlink_dir(&target, &link)
            } else {
                symlink_file(&target, &link)
            };
            res.map(|_| Value::Undefined)
                .map_err(|e| RuntimeError::TypeError(format!("symlinkSync: {}", e)))
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = (target, link);
            Err(RuntimeError::TypeError(
                "symlinkSync: unsupported on this platform".into(),
            ))
        }
    });

    register_method(rt, fs, "truncateSync", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Write(path.clone().into()))?;
        let len = match args.get(1) {
            Some(Value::Number(n)) => *n as u64,
            _ => 0,
        };
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .map_err(|e| fs_throw(rt, "open", &path, e))?;
        file.set_len(len)
            .map(|_| Value::Undefined)
            .map_err(|e| fs_throw(rt, "ftruncate", &path, e))
    });

    register_method(rt, fs, "chmodSync", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Write(path.clone().into()))?;
        let mode = match args.get(1) {
            Some(Value::Number(n)) => *n as u32,
            _ => 0o666,
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::Permissions::from_mode(mode);
            std::fs::set_permissions(&path, permissions)
                .map(|_| Value::Undefined)
                .map_err(|e| fs_throw(rt, "chmod", &path, e))
        }
        #[cfg(windows)]
        {

            let mut perms = std::fs::metadata(&path)
                .map_err(|e| RuntimeError::TypeError(format!("chmodSync: {}", e)))?
                .permissions();
            perms.set_readonly(mode & 0o200 == 0);
            std::fs::set_permissions(&path, perms)
                .map(|_| Value::Undefined)
                .map_err(|e| RuntimeError::TypeError(format!("chmodSync: {}", e)))
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = (path, mode);
            Err(RuntimeError::TypeError(
                "chmodSync: unsupported on this platform".into(),
            ))
        }
    });
    register_method(rt, fs, "lchmodSync", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Write(path.clone().into()))?;
        let mode = match args.get(1) {
            Some(Value::Number(n)) => *n as mode_t,
            _ => 0o666 as mode_t,
        };
        #[cfg(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd"
        ))]
        {
            let c_path = std::ffi::CString::new(path.as_bytes())
                .map_err(|_| RuntimeError::TypeError("lchmodSync: path contains NUL".into()))?;
            let rc = unsafe { lchmod(c_path.as_ptr(), mode) };
            if rc == 0 {
                Ok(Value::Undefined)
            } else {
                Err(RuntimeError::TypeError(format!(
                    "lchmodSync: {}",
                    std::io::Error::last_os_error()
                )))
            }
        }
        #[cfg(not(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd"
        )))]
        {
            let _ = (path, mode);
            Err(RuntimeError::TypeError(
                "lchmodSync: unsupported on this platform".into(),
            ))
        }
    });
    register_method(rt, fs, "chownSync", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Write(path.clone().into()))?;
        let uid = match args.get(1) {
            Some(Value::Number(n)) => *n as uid_t,
            _ => u32::MAX as uid_t,
        };
        let gid = match args.get(2) {
            Some(Value::Number(n)) => *n as gid_t,
            _ => u32::MAX as gid_t,
        };
        #[cfg(unix)]
        {
            let c_path = std::ffi::CString::new(path.as_bytes())
                .map_err(|_| RuntimeError::TypeError("chownSync: path contains NUL".into()))?;
            let rc = unsafe { libc::chown(c_path.as_ptr(), uid, gid) };
            if rc == 0 {
                Ok(Value::Undefined)
            } else {
                Err(fs_throw(
                    rt,
                    "chown",
                    &path,
                    std::io::Error::last_os_error(),
                ))
            }
        }
        #[cfg(windows)]
        {

            let _ = (path, uid, gid);
            Ok(Value::Undefined)
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = (path, uid, gid);
            Err(RuntimeError::TypeError(
                "chownSync: unsupported on this platform".into(),
            ))
        }
    });
    register_method(rt, fs, "lchownSync", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Write(path.clone().into()))?;
        let uid = match args.get(1) {
            Some(Value::Number(n)) => *n as uid_t,
            _ => u32::MAX as uid_t,
        };
        let gid = match args.get(2) {
            Some(Value::Number(n)) => *n as gid_t,
            _ => u32::MAX as gid_t,
        };
        #[cfg(unix)]
        {
            let c_path = std::ffi::CString::new(path.as_bytes())
                .map_err(|_| RuntimeError::TypeError("lchownSync: path contains NUL".into()))?;
            let rc = unsafe { libc::lchown(c_path.as_ptr(), uid, gid) };
            if rc == 0 {
                Ok(Value::Undefined)
            } else {
                Err(fs_throw(
                    rt,
                    "lchown",
                    &path,
                    std::io::Error::last_os_error(),
                ))
            }
        }
        #[cfg(not(unix))]
        {
            let _ = (path, uid, gid);
            Err(RuntimeError::TypeError(
                "lchownSync: unsupported on this platform".into(),
            ))
        }
    });

    register_method(rt, fs, "mkdtempSync", |_rt, args| {
        let prefix = arg_string(args, 0);
        let mut attempts = 0u64;
        loop {
            attempts += 1;
            if attempts > 64 {
                return Err(RuntimeError::TypeError(
                    "mkdtempSync: too many collisions".into(),
                ));
            }

            let path = format!("{}{}", prefix, mkdtemp_suffix(attempts));
            match std::fs::create_dir(&path) {
                Ok(()) => {
                    return Ok(Value::String(Rc::new(
                        rusty_js_runtime::value::JsString::from(path),
                    )))
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(RuntimeError::TypeError(format!("mkdtempSync: {}", e))),
            }
        }
    });

    register_method(rt, fs, "statfsSync", |rt, args| {
        let path = arg_string(args, 0);
        check_fs(rt, caps::FsOp::Stat(path.clone().into()))?;
        let bigint = stat_bigint(rt, args, 1);
        let o = new_object(rt);
        let put = |rt: &mut Runtime, k: &str, v: u64| {
            let val = if bigint {
                Value::BigInt(std::rc::Rc::new(
                    rusty_js_runtime::bigint::JsBigInt::from_u64(v),
                ))
            } else {
                Value::Number(v as f64)
            };
            rt.object_set(o, k.into(), val);
        };
        #[cfg(unix)]
        {
            let cpath = std::ffi::CString::new(path.as_bytes()).map_err(|_| {
                fs_io_throw(
                    rt,
                    "statfs",
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "path contains NUL"),
                )
            })?;
            let mut buf: libc::statfs = unsafe { std::mem::zeroed() };
            if unsafe { libc::statfs(cpath.as_ptr(), &mut buf) } != 0 {
                return Err(fs_throw(
                    rt,
                    "statfs",
                    &path,
                    std::io::Error::last_os_error(),
                ));
            }
            put(rt, "type", buf.f_type as u64);
            put(rt, "bsize", buf.f_bsize as u64);
            #[cfg(any(target_os = "linux", target_os = "android"))]
            let frsize = buf.f_frsize as u64;
            #[cfg(not(any(target_os = "linux", target_os = "android")))]
            let frsize = buf.f_bsize as u64;
            put(rt, "frsize", frsize);
            put(rt, "blocks", buf.f_blocks);
            put(rt, "bfree", buf.f_bfree);
            put(rt, "bavail", buf.f_bavail);
            put(rt, "files", buf.f_files);
            put(rt, "ffree", buf.f_ffree);
        }
        #[cfg(windows)]
        {
            use std::ffi::OsStr;
            use std::os::windows::ffi::OsStrExt;

            extern "system" {
                fn GetVolumePathNameW(
                    lpszFileName: *const u16,
                    lpszVolumePathName: *mut u16,
                    cchBufferLength: u32,
                ) -> i32;
                fn GetDiskFreeSpaceExW(
                    lpDirectoryName: *const u16,
                    lpFreeBytesAvailableToCaller: *mut u64,
                    lpTotalNumberOfBytes: *mut u64,
                    lpTotalNumberOfFreeBytes: *mut u64,
                ) -> i32;
            }

            let wide: Vec<u16> = OsStr::new(&path).encode_wide().chain(Some(0)).collect();
            let mut root = vec![0u16; 32768];
            if unsafe { GetVolumePathNameW(wide.as_ptr(), root.as_mut_ptr(), root.len() as u32) }
                == 0
            {
                return Err(fs_throw(
                    rt,
                    "statfs",
                    &path,
                    std::io::Error::last_os_error(),
                ));
            }
            let mut free_avail = 0u64;
            let mut total = 0u64;
            let mut total_free = 0u64;
            if unsafe {
                GetDiskFreeSpaceExW(root.as_ptr(), &mut free_avail, &mut total, &mut total_free)
            } == 0
            {
                return Err(fs_throw(
                    rt,
                    "statfs",
                    &path,
                    std::io::Error::last_os_error(),
                ));
            }
            let block_size = 4096u64;
            put(rt, "type", 0);
            put(rt, "bsize", block_size);
            put(rt, "frsize", block_size);
            put(rt, "blocks", total / block_size);
            put(rt, "bfree", total_free / block_size);
            put(rt, "bavail", free_avail / block_size);
            put(rt, "files", 0);
            put(rt, "ffree", 0);
        }
        Ok(Value::Object(o))
    });

    for (async_name, sync_name) in [
        ("access", "accessSync"),
        ("appendFile", "appendFileSync"),
        ("chmod", "chmodSync"),
        ("chown", "chownSync"),
        ("lchmod", "lchmodSync"),
        ("lchown", "lchownSync"),
        ("copyFile", "copyFileSync"),
        ("cp", "cpSync"),
        ("link", "linkSync"),
        ("readlink", "readlinkSync"),
        ("rename", "renameSync"),
        ("rm", "rmSync"),
        ("rmdir", "rmdirSync"),
        ("symlink", "symlinkSync"),
        ("truncate", "truncateSync"),
        ("mkdtemp", "mkdtempSync"),
        ("statfs", "statfsSync"),
        ("unlink", "unlinkSync"),
        ("mkdir", "mkdirSync"),
        ("utimes", "utimesSync"),
        ("lutimes", "lutimesSync"),
        ("glob", "globSync"),
        ("opendir", "opendirSync"),

        ("open", "openSync"),
        ("close", "closeSync"),
        ("fsync", "fsyncSync"),
        ("fdatasync", "fdatasyncSync"),
        ("fchmod", "fchmodSync"),
        ("fchown", "fchownSync"),
        ("fstat", "fstatSync"),
        ("ftruncate", "ftruncateSync"),
        ("futimes", "futimesSync"),
        ("write", "writeSync"),
        ("read", "readSync"),
        ("readv", "readvSync"),
        ("writev", "writevSync"),
    ] {
        let key = sync_name.to_string();
        let takes_fd = matches!(
            async_name,
            "close"
                | "fdatasync"
                | "fchmod"
                | "fchown"
                | "fsync"
                | "fstat"
                | "ftruncate"
                | "futimes"
                | "read"
                | "readv"
                | "write"
                | "writev"
        );
        register_method(rt, fs, async_name, move |rt, args| {
            let fs_global = match rt.global_get("fs") {
                Value::Object(id) => id,
                _ => return Ok(Value::Object(new_promise(rt))),
            };
            let sync_fn = rt.object_get(fs_global, &key);

            let callback = trailing_callback(rt, args);
            let mut argv: Vec<Value> = args.to_vec();
            if callback.is_some() && argv.last().map(|v| rt.is_callable(v)).unwrap_or(false) {
                argv.pop();
            }
            if takes_fd {
                normalize_filehandle_fd_arg(rt, &mut argv);
            }
            let read_callback_buffer = if async_name == "read" {
                argv.get(1).cloned().unwrap_or(Value::Undefined)
            } else {
                Value::Undefined
            };
            let result = rt.call_function(sync_fn, Value::Object(fs_global), argv);
            if let Some(cb) = callback {

                let cb_args = match result {
                    Ok(v) if async_name == "read" => vec![Value::Null, v, read_callback_buffer],
                    Ok(v) => vec![Value::Null, v],
                    Err(e) => vec![fs_error_from_runtime(rt, e)],
                };
                let async_resource = new_object(rt);
                let _ = crate::node_stubs::async_hooks_emit_init_for_global(
                    rt,
                    "FSREQCALLBACK",
                    Value::Object(async_resource),
                )?;
                let roots = crate::timer::roots_for_callback_with_resource(
                    &cb,
                    &cb_args,
                    Some(async_resource),
                );
                rt.enqueue_host_phase_rooted(
                    HostEnqueuePhase::HostCompletionMacrotask,
                    "fs async callback",
                    roots,
                    move |rt| {
                        let _ = crate::node_stubs::async_hooks_call_with_global_resource(
                            rt,
                            async_resource,
                            cb,
                            Value::Undefined,
                            cb_args,
                        );
                        Ok(())
                    },
                );
                return Ok(Value::Undefined);
            }
            let p = new_promise(rt);
            match result {
                Ok(v) => resolve_promise(rt, p, v),
                Err(e) => {
                    let err = fs_error_from_runtime(rt, e);
                    reject_promise(rt, p, err);
                }
            }
            Ok(Value::Object(p))
        });
    }

    register_method(rt, fs, "openSync", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        let flags = match args.get(1) {
            Some(Value::String(s)) => s.as_str().to_string(),
            Some(Value::Number(n)) => format!("{}", *n as i32),
            _ => "r".into(),
        };
        let mut opts = std::fs::OpenOptions::new();

        match flags.as_str() {
            "r" => {
                opts.read(true);
            }
            "r+" => {
                opts.read(true).write(true);
            }
            "w" => {
                opts.write(true).create(true).truncate(true);
            }
            "w+" => {
                opts.read(true).write(true).create(true).truncate(true);
            }
            "a" => {
                opts.append(true).create(true);
            }
            "a+" => {
                opts.read(true).append(true).create(true);
            }
            "wx" => {
                opts.write(true).create_new(true);
            }
            "wx+" => {
                opts.read(true).write(true).create_new(true);
            }
            "ax" => {
                opts.append(true).create_new(true);
            }
            "ax+" => {
                opts.read(true).append(true).create_new(true);
            }
            other => {

                let n = other.parse::<i32>().unwrap_or(0);
                match n & 0x3 {
                    1 => {
                        opts.write(true).create(true);
                    }
                    2 => {
                        opts.read(true).write(true).create(true);
                    }
                    _ => {
                        opts.read(true);
                    }
                }
            }
        }

        #[cfg(unix)]
        if let Some(Value::Number(n)) = args.get(2) {
            if n.is_finite() && *n >= 0.0 {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(*n as u32);
            }
        }
        let file = opts
            .open(&path)
            .map_err(|e| fs_throw(rt, "open", &path, e))?;
        let fd = rt.next_fd;
        rt.next_fd += 1;
        rt.fd_table.insert(fd, file);
        Ok(Value::Number(fd as f64))
    });
    register_method(rt, fs, "closeSync", |rt, args| {
        let fd = match args.first() {
            Some(Value::Number(n)) => *n as i32,
            _ => -1,
        };
        match rt.fd_table.remove(&fd) {
            Some(_) => Ok(Value::Undefined),
            None => Err(RuntimeError::TypeError(format!(
                "closeSync: EBADF (fd={})",
                fd
            ))),
        }
    });
    register_method(rt, fs, "fsyncSync", |rt, args| {
        let fd = match args.first() {
            Some(Value::Number(n)) => *n as i32,
            _ => -1,
        };
        let file = rt
            .fd_table
            .get(&fd)
            .ok_or_else(|| RuntimeError::TypeError(format!("fsyncSync: EBADF (fd={})", fd)))?;
        file.sync_all()
            .map(|_| Value::Undefined)
            .map_err(|e| RuntimeError::TypeError(format!("fsyncSync: {}", e)))
    });
    register_method(rt, fs, "fdatasyncSync", |rt, args| {
        let fd = match args.first() {
            Some(Value::Number(n)) => *n as i32,
            _ => -1,
        };
        let file = rt
            .fd_table
            .get(&fd)
            .ok_or_else(|| RuntimeError::TypeError(format!("fdatasyncSync: EBADF (fd={})", fd)))?;
        file.sync_data()
            .map(|_| Value::Undefined)
            .map_err(|e| RuntimeError::TypeError(format!("fdatasyncSync: {}", e)))
    });
    register_method(rt, fs, "ftruncateSync", |rt, args| {
        let fd = match args.first() {
            Some(Value::Number(n)) => *n as i32,
            _ => -1,
        };
        let len = match args.get(1) {
            Some(Value::Number(n)) => *n as u64,
            _ => 0,
        };
        let file = rt
            .fd_table
            .get(&fd)
            .ok_or_else(|| RuntimeError::TypeError(format!("ftruncateSync: EBADF (fd={})", fd)))?;
        file.set_len(len)
            .map(|_| Value::Undefined)
            .map_err(|e| RuntimeError::TypeError(format!("ftruncateSync: {}", e)))
    });
    register_method(rt, fs, "futimesSync", |rt, args| {
        let fd = match args.first() {
            Some(Value::Number(n)) => *n as i32,
            _ => -1,
        };
        let atime_s = utimes_arg_secs(rt, args.get(1));
        let mtime_s = utimes_arg_secs(rt, args.get(2));
        let file = rt
            .fd_table
            .get(&fd)
            .ok_or_else(|| RuntimeError::TypeError(format!("futimesSync: EBADF (fd={})", fd)))?;
        let to_st = |s: f64| -> std::time::SystemTime {
            let dur = std::time::Duration::from_secs_f64(s.max(0.0));
            std::time::UNIX_EPOCH + dur
        };
        let times = std::fs::FileTimes::new()
            .set_accessed(to_st(atime_s))
            .set_modified(to_st(mtime_s));
        file.set_times(times)
            .map(|_| Value::Undefined)
            .map_err(|e| RuntimeError::TypeError(format!("futimesSync: {}", e)))
    });
    register_method(rt, fs, "writeSync", |rt, args| {
        use std::io::Write;
        let fd = match args.first() {
            Some(Value::Number(n)) => *n as i32,
            _ => -1,
        };
        let data: Vec<u8> = match args.get(1) {
            Some(v) => value_to_bytes(rt, v, None),
            None => Vec::new(),
        };

        if fd == 1 || fd == 2 {
            let n = data.len();
            if fd == 1 {
                let mut out = std::io::stdout();
                let _ = out.write_all(&data);
                let _ = out.flush();
            } else {
                let mut err = std::io::stderr();
                let _ = err.write_all(&data);
                let _ = err.flush();
            }
            return Ok(Value::Number(n as f64));
        }

        let position = if matches!(args.get(1), Some(Value::String(_))) {
            args.get(2)
        } else {
            args.get(4)
        };
        let position = match position {
            Some(Value::Number(n)) if n.is_finite() && *n >= 0.0 => Some(*n as u64),
            _ => None,
        };
        let file = rt
            .fd_table
            .get_mut(&fd)
            .ok_or_else(|| RuntimeError::TypeError(format!("writeSync: EBADF (fd={})", fd)))?;
        use std::io::{Seek, SeekFrom};
        let saved = match position {
            Some(pos) => {
                let cur = file.stream_position().ok();
                file.seek(SeekFrom::Start(pos))
                    .map_err(|e| RuntimeError::TypeError(format!("writeSync: {}", e)))?;
                cur
            }
            None => None,
        };
        let n = file
            .write(&data)
            .map_err(|e| RuntimeError::TypeError(format!("writeSync: {}", e)))?;
        if let Some(cur) = saved {
            let _ = file.seek(SeekFrom::Start(cur));
        }
        Ok(Value::Number(n as f64))
    });
    register_method(rt, fs, "readSync", |rt, args| {
        use std::io::Read;

        let fd = match args.first() {
            Some(Value::Number(n)) => *n as i32,
            _ => -1,
        };
        let length = match args.get(3) {
            Some(Value::Number(n)) => *n as usize,
            _ => 0,
        };
        let mut buf = vec![0u8; length];

        let position = match args.get(4) {
            Some(Value::Number(n)) if n.is_finite() && *n >= 0.0 => Some(*n as u64),
            _ => None,
        };
        let file = rt
            .fd_table
            .get_mut(&fd)
            .ok_or_else(|| RuntimeError::TypeError(format!("readSync: EBADF (fd={})", fd)))?;
        use std::io::{Seek, SeekFrom};
        let saved = match position {
            Some(pos) => {
                let cur = file.stream_position().ok();
                file.seek(SeekFrom::Start(pos))
                    .map_err(|e| RuntimeError::TypeError(format!("readSync: {}", e)))?;
                cur
            }
            None => None,
        };
        let n = file
            .read(&mut buf)
            .map_err(|e| RuntimeError::TypeError(format!("readSync: {}", e)))?;
        if let Some(cur) = saved {
            let _ = file.seek(SeekFrom::Start(cur));
        }

        let offset = match args.get(2) {
            Some(Value::Number(n)) => *n as usize,
            _ => 0,
        };
        if let Some(Value::Object(bid)) = args.get(1).cloned() {
            for (i, b) in buf[..n].iter().enumerate() {
                rt.object_set(bid, (offset + i).to_string(), Value::Number(*b as f64));
            }
        }
        Ok(Value::Number(n as f64))
    });
    register_method(rt, fs, "fstatSync", |rt, args| {
        let fd = match args.first() {
            Some(Value::Number(n)) => *n as i32,
            _ => -1,
        };
        let md = {
            let file = rt
                .fd_table
                .get(&fd)
                .ok_or_else(|| RuntimeError::TypeError(format!("fstatSync: EBADF (fd={})", fd)))?;
            file.metadata()
                .map_err(|e| RuntimeError::TypeError(format!("fstatSync: {}", e)))?
        };
        Ok(Value::Object(stat_object(rt, &md)))
    });
    register_method(rt, fs, "fchmodSync", |rt, args| {
        let fd = match args.first() {
            Some(Value::Number(n)) => *n as i32,
            _ => -1,
        };
        let mode = match args.get(1) {
            Some(Value::Number(n)) => *n as u32,
            _ => 0o666,
        };
        let file = rt
            .fd_table
            .get(&fd)
            .ok_or_else(|| RuntimeError::TypeError(format!("fchmodSync: EBADF (fd={})", fd)))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::Permissions::from_mode(mode);
            file.set_permissions(permissions)
                .map(|_| Value::Undefined)
                .map_err(|e| RuntimeError::TypeError(format!("fchmodSync: {}", e)))
        }
        #[cfg(not(unix))]
        {
            let _ = (file, mode);
            Err(RuntimeError::TypeError(
                "fchmodSync: unsupported on this platform".into(),
            ))
        }
    });
    register_method(rt, fs, "fchownSync", |rt, args| {
        let fd = match args.first() {
            Some(Value::Number(n)) => *n as i32,
            _ => -1,
        };
        let uid = match args.get(1) {
            Some(Value::Number(n)) => *n as uid_t,
            _ => u32::MAX as uid_t,
        };
        let gid = match args.get(2) {
            Some(Value::Number(n)) => *n as gid_t,
            _ => u32::MAX as gid_t,
        };
        let _file = rt
            .fd_table
            .get(&fd)
            .ok_or_else(|| RuntimeError::TypeError(format!("fchownSync: EBADF (fd={})", fd)))?;
        #[cfg(unix)]
        {
            let rc = unsafe { libc::fchown(fd, uid, gid) };
            if rc == 0 {
                Ok(Value::Undefined)
            } else {
                Err(RuntimeError::TypeError(format!(
                    "fchownSync: {}",
                    std::io::Error::last_os_error()
                )))
            }
        }
        #[cfg(not(unix))]
        {
            let _ = (fd, uid, gid);
            Err(RuntimeError::TypeError(
                "fchownSync: unsupported on this platform".into(),
            ))
        }
    });

    register_method(rt, fs, "utimesSync", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Write(path.clone().into()))?;
        let atime_s = utimes_arg_secs(rt, args.get(1));
        let mtime_s = utimes_arg_secs(rt, args.get(2));
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .or_else(|_| std::fs::OpenOptions::new().read(true).open(&path))
            .map_err(|e| fs_throw(rt, "utime", &path, e))?;
        let to_st = |s: f64| -> std::time::SystemTime {
            let dur = std::time::Duration::from_secs_f64(s.max(0.0));
            std::time::UNIX_EPOCH + dur
        };
        let times = std::fs::FileTimes::new()
            .set_accessed(to_st(atime_s))
            .set_modified(to_st(mtime_s));
        file.set_times(times)
            .map(|_| Value::Undefined)
            .map_err(|e| fs_throw(rt, "utime", &path, e))
    });

    register_method(rt, fs, "lutimesSync", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::Write(path.clone().into()))?;
        let atime_s = utimes_arg_secs(rt, args.get(1));
        let mtime_s = utimes_arg_secs(rt, args.get(2));
        #[cfg(unix)]
        {
            let c_path = CString::new(path.as_str())
                .map_err(|_| RuntimeError::TypeError("lutimesSync: path contains NUL".into()))?;
            let times = [seconds_to_timespec(atime_s), seconds_to_timespec(mtime_s)];
            let rc = unsafe {
                libc::utimensat(
                    libc::AT_FDCWD,
                    c_path.as_ptr(),
                    times.as_ptr(),
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            };
            if rc == 0 {
                Ok(Value::Undefined)
            } else {
                Err(RuntimeError::TypeError(format!(
                    "lutimesSync: {}",
                    std::io::Error::last_os_error()
                )))
            }
        }
        #[cfg(not(unix))]
        {
            let _ = (path, atime_s, mtime_s);
            Err(RuntimeError::TypeError(
                "lutimesSync: unsupported on this platform".into(),
            ))
        }
    });

    register_method(rt, fs, "globSync", |rt, args| {

        let patterns: Vec<String> = match args.first() {
            Some(Value::Object(id))
                if matches!(
                    rt.obj(*id).internal_kind,
                    rusty_js_runtime::value::InternalKind::Array
                ) =>
            {
                let len = rt.array_length(*id);
                (0..len)
                    .filter_map(|i| match rt.object_get(*id, &i.to_string()) {
                        Value::String(s) => Some(s.as_str().to_string()),
                        _ => None,
                    })
                    .collect()
            }
            _ => vec![arg_string(args, 0)],
        };

        let (cwd, exclude) = match args.get(1) {
            Some(Value::Object(id)) => {
                let cwd = match rt.object_get(*id, "cwd") {
                    Value::String(s) => s.as_str().to_string(),
                    _ => ".".to_string(),
                };
                let exclude = match rt.object_get(*id, "exclude") {
                    Value::Object(f) if rt.is_callable(&Value::Object(f)) => Some(f),
                    _ => None,
                };
                (cwd, exclude)
            }
            _ => (".".to_string(), None),
        };
        check_fs(rt, caps::FsOp::List(cwd.clone().into()))?;
        let mut results: Vec<String> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for pattern in &patterns {
            let is_abs = pattern.starts_with('/');
            let (walk_start, walk_pat) = if is_abs {
                ("/".to_string(), pattern.trim_start_matches('/').to_string())
            } else {
                (cwd.clone(), pattern.clone())
            };
            let mut r: Vec<String> = Vec::new();
            glob_walk(rt, &walk_start, &walk_pat, exclude, &mut r)?;
            for mut p in r {
                if is_abs {
                    p = format!("/{p}");
                }
                if seen.insert(p.clone()) {
                    results.push(p);
                }
            }
        }
        let arr = rt.alloc_object(Object::new_array());
        for (i, p) in results.iter().enumerate() {
            rt.object_set(
                arr,
                i.to_string(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(p.clone()))),
            );
        }
        rt.object_set(arr, "length".into(), Value::Number(results.len() as f64));
        Ok(Value::Object(arr))
    });

    register_method(rt, fs, "opendirSync", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        check_fs(rt, caps::FsOp::List(path.clone().into()))?;
        let dir = dir_object(rt, path)?;
        Ok(Value::Object(dir))
    });

    register_method(rt, fs, "readvSync", |rt, args| {
        use std::io::Read;
        let fd = match args.first() {
            Some(Value::Number(n)) => *n as i32,
            _ => -1,
        };
        let buffers_id = match args.get(1) {
            Some(Value::Object(id)) => *id,
            _ => {
                return Err(RuntimeError::TypeError(
                    "readvSync: buffers must be an array".into(),
                ))
            }
        };
        let len = match rt.object_get(buffers_id, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        };

        let mut targets: Vec<(ObjectRef, usize)> = Vec::with_capacity(len);
        for i in 0..len {
            if let Value::Object(buf_id) = rt.object_get(buffers_id, &i.to_string()) {
                let blen = match rt.object_get(buf_id, "length") {
                    Value::Number(n) => n as usize,
                    _ => 0,
                };
                targets.push((buf_id, blen));
            }
        }

        let mut chunks: Vec<(ObjectRef, Vec<u8>)> = Vec::with_capacity(targets.len());
        let mut total = 0usize;
        {
            let file = rt
                .fd_table
                .get_mut(&fd)
                .ok_or_else(|| RuntimeError::TypeError(format!("readvSync: EBADF (fd={})", fd)))?;
            for (id, blen) in &targets {
                let mut b = vec![0u8; *blen];
                let n = file
                    .read(&mut b)
                    .map_err(|e| RuntimeError::TypeError(format!("readvSync: {}", e)))?;
                total += n;
                b.truncate(n);
                let short = n < *blen;
                chunks.push((*id, b));
                if short {
                    break;
                }
            }
        }
        for (id, bytes) in chunks {
            for (i, byte) in bytes.iter().enumerate() {
                rt.object_set(id, i.to_string(), Value::Number(*byte as f64));
            }
        }
        Ok(Value::Number(total as f64))
    });

    register_method(rt, fs, "writevSync", |rt, args| {
        use std::io::Write;
        let fd = match args.first() {
            Some(Value::Number(n)) => *n as i32,
            _ => -1,
        };
        let buffers_id = match args.get(1) {
            Some(Value::Object(id)) => *id,
            _ => {
                return Err(RuntimeError::TypeError(
                    "writevSync: buffers must be an array".into(),
                ))
            }
        };
        let len = match rt.object_get(buffers_id, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        };

        let mut all_bytes: Vec<Vec<u8>> = Vec::with_capacity(len);
        for i in 0..len {
            let buf_v = rt.object_get(buffers_id, &i.to_string());
            let buf_id = match buf_v {
                Value::Object(id) => id,
                _ => continue,
            };
            let blen = match rt.object_get(buf_id, "length") {
                Value::Number(n) => n as usize,
                _ => 0,
            };
            let mut b = Vec::with_capacity(blen);
            for j in 0..blen {
                let v = rt.object_get(buf_id, &j.to_string());
                b.push(match v {
                    Value::Number(n) => n as u8,
                    _ => 0,
                });
            }
            all_bytes.push(b);
        }
        let file = rt
            .fd_table
            .get_mut(&fd)
            .ok_or_else(|| RuntimeError::TypeError(format!("writevSync: EBADF (fd={})", fd)))?;
        let mut total = 0usize;
        for chunk in all_bytes {
            let n = file
                .write(&chunk)
                .map_err(|e| RuntimeError::TypeError(format!("writevSync: {}", e)))?;
            total += n;
        }
        Ok(Value::Number(total as f64))
    });

    register_method(rt, fs, "openAsBlob", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        let mime = match args.get(1) {
            Some(Value::Object(id)) => match rt.object_get(*id, "type") {
                Value::String(s) => s.as_str().to_string(),
                _ => "".into(),
            },
            _ => "".into(),
        };
        let p = new_promise(rt);
        match std::fs::metadata(&path) {
            Ok(meta) if meta.is_file() => {
                let blob = build_file_backed_blob(rt, path, &meta, mime);
                resolve_promise(rt, p, Value::Object(blob));
            }
            Ok(_) => reject_promise(
                rt,
                p,
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    "openAsBlob: path is not a file",
                ))),
            ),
            Err(e) => {
                reject_promise(
                    rt,
                    p,
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(format!(
                        "openAsBlob: {}",
                        e
                    )))),
                );
            }
        }
        Ok(Value::Object(p))
    });

    register_method(rt, fs, "watch", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        let watcher = rt.alloc_object(rusty_js_runtime::value::Object::new_ordinary());
        let listener = args
            .iter()
            .rev()
            .find(|v| matches!(v, Value::Object(_)))
            .cloned();
        if let Some(v) = listener.clone() {
            rt.object_set(watcher, "__listener".into(), v);
        }
        let id = register_watcher(rt, path, listener, watcher,   200);
        rt.object_set(watcher, "__watch_id".into(), Value::Number(id as f64));
        register_method(rt, watcher, "close", |rt, _args| {
            let this = match rt.current_this() {
                Value::Object(id) => id,
                _ => return Ok(Value::Undefined),
            };
            if let Value::Number(wid) = rt.object_get(this, "__watch_id") {
                unregister_watcher(rt, wid as u64);
            }
            Ok(Value::Undefined)
        });
        register_method(rt, watcher, "on", |rt, _args| Ok(rt.current_this()));
        register_method(rt, watcher, "off", |rt, _args| Ok(rt.current_this()));
        register_method(rt, watcher, "ref", |rt, _args| Ok(rt.current_this()));
        register_method(rt, watcher, "unref", |rt, _args| Ok(rt.current_this()));
        Ok(Value::Object(watcher))
    });
    register_method(rt, fs, "watchFile", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);

        let mut interval_ms = 5007u64;
        for a in args {
            if let Value::Object(id) = a {
                if let Value::Number(n) = rt.object_get(*id, "interval") {
                    interval_ms = (n as u64).max(50);
                    break;
                }
            }
        }
        let watcher = rt.alloc_object(rusty_js_runtime::value::Object::new_ordinary());
        let listener = args
            .iter()
            .rev()
            .find(|v| matches!(v, Value::Object(_)))
            .cloned();
        if let Some(v) = listener.clone() {
            rt.object_set(watcher, "__listener".into(), v);
        }
        let id = register_watcher(rt, path, listener, watcher, interval_ms);
        rt.object_set(watcher, "__watch_id".into(), Value::Number(id as f64));
        register_method(rt, watcher, "ref", |rt, _args| Ok(rt.current_this()));
        register_method(rt, watcher, "unref", |rt, _args| Ok(rt.current_this()));
        Ok(Value::Object(watcher))
    });
    register_method(rt, fs, "unwatchFile", |rt, args| {
        let path = arg_path_or_url(rt, args, 0);
        unregister_watchers_by_path(rt, &path);
        Ok(Value::Undefined)
    });

    for cls in ["Stats", "Dirent", "Dir"] {
        let n = cls.to_string();
        let stub = make_callable(rt, cls, move |_rt, _args| {
            Err(RuntimeError::TypeError(format!(
                "fs.{}: class not constructable (Tier-Ω.5.P32.E1 stub)",
                n
            )))
        });

        let proto = new_object(rt);
        rt.object_set(proto, "constructor".into(), Value::Object(stub));
        if cls == "Stats" || cls == "Dirent" {

            let is_dirent = cls == "Dirent";
            for (method, ifmt) in [
                ("isFile", 0o100000u32),
                ("isDirectory", 0o040000),
                ("isSymbolicLink", 0o120000),
                ("isBlockDevice", 0o060000),
                ("isCharacterDevice", 0o020000),
                ("isFIFO", 0o010000),
                ("isSocket", 0o140000),
            ] {
                register_method(rt, proto, method, move |rt, _a| {
                    let bits = if is_dirent {
                        dirent_current_type(rt)
                    } else {
                        stat_current_mode(rt)
                    };
                    Ok(Value::Boolean(bits & 0o170000 == ifmt))
                });
            }
        }
        rt.object_set(stub, "prototype".into(), Value::Object(proto));
        rt.object_set(fs, cls.into(), Value::Object(stub));
    }

    {
        let ctor = make_callable(rt, "BigIntStats", |_rt, _a| {
            Err(RuntimeError::TypeError(
                "fs.BigIntStats: class not constructable".into(),
            ))
        });
        let proto = new_object(rt);
        rt.object_set(proto, "constructor".into(), Value::Object(ctor));
        for (method, ifmt) in [
            ("isFile", 0o100000u32),
            ("isDirectory", 0o040000),
            ("isSymbolicLink", 0o120000),
            ("isBlockDevice", 0o060000),
            ("isCharacterDevice", 0o020000),
            ("isFIFO", 0o010000),
            ("isSocket", 0o140000),
        ] {
            register_method(rt, proto, method, move |rt, _a| {
                Ok(Value::Boolean(stat_current_mode(rt) & 0o170000 == ifmt))
            });
        }
        rt.object_set(ctor, "prototype".into(), Value::Object(proto));
        rt.obj_mut(fs)
            .set_own_internal("__bigintstats_proto".into(), Value::Object(proto));
    }

    let to_unix = make_callable(rt, "_toUnixTimestamp", |_rt, args| {
        let v = args.first().cloned().unwrap_or(Value::Number(0.0));
        Ok(v)
    });
    rt.object_set(fs, "_toUnixTimestamp".into(), Value::Object(to_unix));

    for cls in [
        "ReadStream",
        "WriteStream",
        "FileReadStream",
        "FileWriteStream",
        "Utf8Stream",
    ] {
        if !rt.is_callable(&rt.object_get(fs, cls)) {
            let c = make_callable(rt, cls, |rt, _a| Ok(rt.current_this()));
            let proto = new_object(rt);
            rt.object_set(proto, "constructor".into(), Value::Object(c));
            rt.object_set(c, "prototype".into(), Value::Object(proto));
            rt.object_set(fs, cls.to_string(), Value::Object(c));
        }
    }
    if !rt.is_callable(&rt.object_get(fs, "mkdtempDisposableSync")) {
        let mk = rt.object_get(fs, "mkdtempSync");
        rt.object_set(fs, "mkdtempDisposableSync".into(), mk);
    }

    if let Value::Object(promises) = rt.object_get(fs, "promises") {
        for name in [
            "access",
            "appendFile",
            "chmod",
            "chown",
            "close",
            "copyFile",
            "cp",
            "fdatasync",
            "fchmod",
            "fchown",
            "fsync",
            "fstat",
            "ftruncate",
            "futimes",
            "lchmod",
            "lchown",
            "link",
            "lstat",
            "lutimes",
            "mkdir",
            "mkdtemp",
            "open",
            "opendir",
            "read",
            "readFile",
            "readlink",
            "realpath",
            "rename",
            "rm",
            "rmdir",
            "statfs",
            "symlink",
            "truncate",
            "unlink",
            "utimes",
            "watch",
            "write",
            "writeFile",
            "glob",
        ] {
            let v = rt.object_get(fs, name);
            if rt.is_callable(&v)
                && (!matches!(name, "lstat" | "realpath")
                    || !rt.is_callable(&rt.object_get(promises, name)))
            {
                rt.object_set(promises, name.to_string(), v);
            }
        }

        {
            const FSP_GLOB_JS: &str = r#"
(function () {
  const target = globalThis.__cruft_fsp_glob_target;
  const gsync = globalThis.__cruft_globSync;
  Object.defineProperty(target, 'glob', {
    value: function (pattern, options) {
      const results = gsync(pattern, options);
      return (async function* () {
        for (const p of results) yield p;
      })();
    },
    writable: true, enumerable: true, configurable: true,
  });
})();
"#;
            let gsync = rt.object_get(fs, "globSync");
            rt.define_global_property("__cruft_globSync", gsync);
            rt.define_global_property("__cruft_fsp_glob_target", Value::Object(promises));
            let _ = rt.run_script(FSP_GLOB_JS, "cruft:internal/fsp-glob.js");
            rt.define_global_property("__cruft_globSync", Value::Undefined);
            rt.define_global_property("__cruft_fsp_glob_target", Value::Undefined);
        }

        register_method(rt, promises, "open", |rt, args| {
            let p = new_promise(rt);
            let fs_global = match rt.global_get("fs") {
                Value::Object(id) => id,
                _ => return Ok(Value::Object(p)),
            };
            let open_sync = rt.object_get(fs_global, "openSync");
            let fd_result = rt.call_function(open_sync, Value::Object(fs_global), args.to_vec());
            match fd_result {
                Ok(Value::Number(fd)) => {
                    let handle = new_object(rt);
                    rt.object_set(handle, "fd".into(), Value::Number(fd));
                    rt.object_set(handle, "__cruft_fd".into(), Value::Number(fd));

                    let fh_path = arg_path_or_url(rt, args, 0);
                    rt.object_set(
                        handle,
                        "__cruft_fh_path".into(),
                        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(fh_path))),
                    );
                    register_method(rt, handle, "stat", |rt, _args| {
                        let p = new_promise(rt);
                        let fd = match rt.current_this() {
                            Value::Object(this) => rt.object_get(this, "__cruft_fd"),
                            _ => Value::Undefined,
                        };
                        let fs_global = match rt.global_get("fs") {
                            Value::Object(id) => id,
                            _ => return Ok(Value::Object(p)),
                        };
                        let fstat_sync = rt.object_get(fs_global, "fstatSync");
                        match rt.call_function(fstat_sync, Value::Object(fs_global), vec![fd]) {
                            Ok(v) => resolve_promise(rt, p, v),
                            Err(e) => {
                                let msg = match &e {
                                    RuntimeError::TypeError(m) => m.clone(),
                                    RuntimeError::RangeError(m) => m.clone(),
                                    RuntimeError::ReferenceError(m) => m.clone(),
                                    RuntimeError::Thrown(v) => format!("{:?}", v),
                                    _ => format!("{:?}", e),
                                };
                                reject_promise(
                                    rt,
                                    p,
                                    Value::String(Rc::new(
                                        rusty_js_runtime::value::JsString::from(msg),
                                    )),
                                );
                            }
                        }
                        Ok(Value::Object(p))
                    });
                    register_method(rt, handle, "close", |rt, _args| {
                        let p = new_promise(rt);
                        let fd = match rt.current_this() {
                            Value::Object(this) => rt.object_get(this, "__cruft_fd"),
                            _ => Value::Undefined,
                        };
                        let fs_global = match rt.global_get("fs") {
                            Value::Object(id) => id,
                            _ => return Ok(Value::Object(p)),
                        };
                        let close_sync = rt.object_get(fs_global, "closeSync");
                        match rt.call_function(close_sync, Value::Object(fs_global), vec![fd]) {
                            Ok(v) => resolve_promise(rt, p, v),
                            Err(e) => {
                                let msg = match &e {
                                    RuntimeError::TypeError(m) => m.clone(),
                                    RuntimeError::RangeError(m) => m.clone(),
                                    RuntimeError::ReferenceError(m) => m.clone(),
                                    RuntimeError::Thrown(v) => format!("{:?}", v),
                                    _ => format!("{:?}", e),
                                };
                                reject_promise(
                                    rt,
                                    p,
                                    Value::String(Rc::new(
                                        rusty_js_runtime::value::JsString::from(msg),
                                    )),
                                );
                            }
                        }
                        Ok(Value::Object(p))
                    });
                    install_filehandle_methods(rt, handle);
                    resolve_promise(rt, p, Value::Object(handle));
                }
                Ok(v) => resolve_promise(rt, p, v),

                Err(RuntimeError::Thrown(v)) => {
                    reject_promise(rt, p, v);
                }
                Err(e) => {
                    let msg = match &e {
                        RuntimeError::TypeError(m) => m.clone(),
                        RuntimeError::RangeError(m) => m.clone(),
                        RuntimeError::ReferenceError(m) => m.clone(),
                        _ => format!("{:?}", e),
                    };
                    reject_promise(
                        rt,
                        p,
                        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(msg))),
                    );
                }
            }
            Ok(Value::Object(p))
        });

        register_method(rt, promises, "opendir", |rt, args| {
            let p = new_promise(rt);
            let path = arg_path_or_url(rt, args, 0);
            if let Err(e) = check_fs(rt, caps::FsOp::List(path.clone().into())) {
                reject_promise(
                    rt,
                    p,
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(match &e {
                        RuntimeError::TypeError(m) => m.clone(),
                        RuntimeError::RangeError(m) => m.clone(),
                        RuntimeError::ReferenceError(m) => m.clone(),
                        RuntimeError::Thrown(v) => format!("{:?}", v),
                        _ => format!("{:?}", e),
                    }))),
                );
                return Ok(Value::Object(p));
            }
            match dir_object(rt, path) {
                Ok(dir) => resolve_promise(rt, p, Value::Object(dir)),
                Err(e) => {
                    let msg = match &e {
                        RuntimeError::TypeError(m) => m.clone(),
                        RuntimeError::RangeError(m) => m.clone(),
                        RuntimeError::ReferenceError(m) => m.clone(),
                        RuntimeError::Thrown(v) => format!("{:?}", v),
                        _ => format!("{:?}", e),
                    };
                    reject_promise(
                        rt,
                        p,
                        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(msg))),
                    );
                }
            }
            Ok(Value::Object(p))
        });

        let c = rt.object_get(fs, "constants");
        rt.object_set(promises, "constants".into(), c);
        for (async_name, sync_name) in [("readv", "readvSync"), ("writev", "writevSync")] {
            let key = sync_name.to_string();
            register_method(rt, promises, async_name, move |rt, args| {
                let p = new_promise(rt);
                let fs_global = match rt.global_get("fs") {
                    Value::Object(id) => id,
                    _ => return Ok(Value::Object(p)),
                };
                let sync_fn = rt.object_get(fs_global, &key);
                let mut argv: Vec<Value> = args.to_vec();
                normalize_filehandle_fd_arg(rt, &mut argv);
                match rt.call_function(sync_fn, Value::Object(fs_global), argv) {
                    Ok(v) => resolve_promise(rt, p, v),
                    Err(e) => {
                        let msg = match &e {
                            RuntimeError::TypeError(m) => m.clone(),
                            RuntimeError::RangeError(m) => m.clone(),
                            RuntimeError::ReferenceError(m) => m.clone(),
                            RuntimeError::Thrown(v) => format!("{:?}", v),
                            _ => format!("{:?}", e),
                        };
                        reject_promise(
                            rt,
                            p,
                            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(msg))),
                        );
                    }
                }
                Ok(Value::Object(p))
            });
        }

        register_method(rt, promises, "mkdtempDisposable", |rt, args| {
            let p = new_promise(rt);
            let fsg = match rt.global_get("fs") {
                Value::Object(i) => i,
                _ => return Ok(Value::Object(p)),
            };
            let mk = rt.object_get(fsg, "mkdtempSync");
            match rt.call_function(mk, Value::Undefined, args.to_vec()) {
                Ok(v) => resolve_promise(rt, p, v),
                Err(e) => {
                    let msg = match &e {
                        RuntimeError::TypeError(m) => m.clone(),
                        RuntimeError::RangeError(m) => m.clone(),
                        RuntimeError::ReferenceError(m) => m.clone(),
                        RuntimeError::Thrown(v) => format!("{:?}", v),
                        _ => format!("{:?}", e),
                    };
                    reject_promise(
                        rt,
                        p,
                        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(msg))),
                    );
                }
            }
            Ok(Value::Object(p))
        });
        if !rt.is_callable(&rt.object_get(promises, "watch")) {
            register_method(rt, promises, "watch", |_rt, _a| Ok(Value::Undefined));
        }
        rt.define_global_property("fs_promises", Value::Object(promises));
    }
    rt.define_global_property("fs", Value::Object(fs));
}

fn build_file_backed_blob(
    rt: &mut rusty_js_runtime::Runtime,
    path: String,
    meta: &std::fs::Metadata,
    mime: String,
) -> rusty_js_runtime::value::ObjectRef {
    use rusty_js_runtime::value::Object as RObj;
    rt.materialize_lazy_global("Blob");
    let proto = match rt.global_get("Blob") {
        Value::Object(ctor) => match rt.object_get(ctor, "prototype") {
            Value::Object(proto) => Some(proto),
            _ => None,
        },
        _ => None,
    };
    let mut obj = RObj::new_ordinary();
    obj.proto = proto;
    let blob = rt.alloc_object(obj);
    let len = meta.len();
    rt.object_set(
        blob,
        "__blob_bytes".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(""))),
    );
    rt.object_set(
        blob,
        "__blob_type".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(mime))),
    );
    let chunks = rt.alloc_object(RObj::new_array());
    rt.object_set(chunks, "length".into(), Value::Number(0.0));
    rt.object_set(blob, "__blob_chunks".into(), Value::Object(chunks));
    rt.object_set(
        blob,
        "__blob_file_path".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(path))),
    );
    rt.object_set(blob, "__blob_file_len".into(), Value::Number(len as f64));
    rt.object_set(
        blob,
        "__blob_file_mtime_ns".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            file_mtime_ns(meta),
        ))),
    );
    rt.object_set(blob, "__blob_file_start".into(), Value::Number(0.0));
    rt.object_set(blob, "__blob_file_end".into(), Value::Number(len as f64));
    blob
}

fn file_mtime_ns(meta: &std::fs::Metadata) -> String {
    meta.modified()
        .ok()
        .and_then(|mtime| mtime.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos().to_string())
        .unwrap_or_default()
}

fn glob_excluded(
    rt: &mut Runtime,
    exclude: Option<ObjectRef>,
    rel: &str,
) -> Result<bool, RuntimeError> {
    let Some(func) = exclude else {
        return Ok(false);
    };
    let v = rt.call_function(
        Value::Object(func),
        Value::Undefined,
        vec![Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(rel.to_string()),
        ))],
    )?;
    Ok(matches!(v, Value::Boolean(true)))
}

fn glob_walk(
    rt: &mut Runtime,
    start_dir: &str,
    pattern: &str,
    exclude: Option<ObjectRef>,
    out: &mut Vec<String>,
) -> Result<(), RuntimeError> {
    let segs: Vec<&str> = pattern.split('/').collect();
    fn walk(
        rt: &mut Runtime,
        cur: &std::path::Path,
        rel: &str,
        segs: &[&str],
        exclude: Option<ObjectRef>,
        out: &mut Vec<String>,
    ) -> Result<(), RuntimeError> {
        if segs.is_empty() {
            out.push(rel.to_string());
            return Ok(());
        }
        let seg = segs[0];
        let rest = &segs[1..];
        if seg == "**" {

            walk(rt, cur, rel, rest, exclude, out)?;
            if let Ok(read) = std::fs::read_dir(cur) {
                for entry in read.flatten() {
                    if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        let name = entry.file_name().to_string_lossy().into_owned();
                        let new_rel = if rel.is_empty() {
                            name.clone()
                        } else {
                            format!("{}/{}", rel, name)
                        };
                        if glob_excluded(rt, exclude, &new_rel)? {
                            continue;
                        }
                        walk(rt, &entry.path(), &new_rel, segs, exclude, out)?;
                    }
                }
            }
            return Ok(());
        }
        if let Ok(read) = std::fs::read_dir(cur) {
            for entry in read.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if glob_match(seg, &name) {
                    let new_rel = if rel.is_empty() {
                        name.clone()
                    } else {
                        format!("{}/{}", rel, name)
                    };
                    if glob_excluded(rt, exclude, &new_rel)? {
                        continue;
                    }
                    if rest.is_empty() {
                        out.push(new_rel);
                    } else if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        walk(rt, &entry.path(), &new_rel, rest, exclude, out)?;
                    }
                }
            }
        }
        Ok(())
    }
    walk(rt, std::path::Path::new(start_dir), "", &segs, exclude, out)
}

fn glob_match(pattern: &str, name: &str) -> bool {
    let pat: Vec<char> = pattern.chars().collect();
    let txt: Vec<char> = name.chars().collect();
    fn rec(p: &[char], t: &[char]) -> bool {
        if p.is_empty() {
            return t.is_empty();
        }
        match p[0] {
            '*' => {

                if rec(&p[1..], t) {
                    return true;
                }
                if !t.is_empty() && rec(p, &t[1..]) {
                    return true;
                }
                false
            }
            '?' => !t.is_empty() && rec(&p[1..], &t[1..]),
            c => !t.is_empty() && t[0] == c && rec(&p[1..], &t[1..]),
        }
    }
    rec(&pat, &txt)
}

fn cp_walk(
    rt: &mut Runtime,
    src: &std::path::Path,
    dst: &std::path::Path,
    recursive: bool,
    filter: Option<&Value>,
    force: bool,
    error_on_exist: bool,
) -> Result<(), RuntimeError> {
    if let Some(filter) = filter {
        let src_v = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            src.to_string_lossy().to_string(),
        )));
        let dst_v = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            dst.to_string_lossy().to_string(),
        )));
        let accept = rusty_js_runtime::abstract_ops::to_boolean(&rt.call_function(
            filter.clone(),
            Value::Undefined,
            vec![src_v, dst_v],
        )?);
        if !accept {
            return Ok(());
        }
    }
    let md = std::fs::symlink_metadata(src)
        .map_err(|e| RuntimeError::TypeError(format!("cpSync: {e}")))?;
    if md.is_dir() {
        if !recursive {
            return Err(RuntimeError::TypeError(
                "cpSync: source is a directory and recursive is not set".into(),
            ));
        }
        let _ = std::fs::create_dir_all(dst);
        let entries = std::fs::read_dir(src)
            .map_err(|e| RuntimeError::TypeError(format!("cpSync: {e}")))?
            .filter_map(|e| e.ok())
            .map(|e| (e.path(), dst.join(e.file_name())))
            .collect::<Vec<_>>();
        for (from, to) in entries {
            cp_walk(rt, &from, &to, recursive, filter, force, error_on_exist)?;
        }
        Ok(())
    } else {
        if dst.exists() && !force {
            if error_on_exist {
                let d = dst.display();
                let msg =
                    format!("Target already exists: cp returned EEXIST ({d} already exists) {d}");
                return Err(
                    match rusty_js_runtime::intrinsics::make_error_instance(rt, "Error", &msg) {
                        Some(id) => {
                            rt.object_set(
                                id,
                                "code".into(),
                                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                                    "ERR_FS_CP_EEXIST",
                                ))),
                            );
                            RuntimeError::Thrown(Value::Object(id))
                        }
                        None => RuntimeError::TypeError(msg),
                    },
                );
            }
            return Ok(());
        }
        if let Some(parent) = dst.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        std::fs::copy(src, dst)
            .map(|_| ())
            .map_err(|e| RuntimeError::TypeError(format!("cpSync: {e}")))
    }
}

fn cp_recursive(
    src: &std::path::Path,
    dst: &std::path::Path,
    recursive: bool,
) -> std::io::Result<()> {
    let md = std::fs::metadata(src)?;
    if md.is_dir() {
        if !recursive {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "cp: source is a directory and recursive is not set",
            ));
        }
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let from = entry.path();
            let to = dst.join(entry.file_name());
            cp_recursive(&from, &to, true)?;
        }
        Ok(())
    } else {
        if let Some(parent) = dst.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        std::fs::copy(src, dst).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusty_js_runtime::Runtime;

    fn fresh() -> Runtime {
        let mut rt = Runtime::new();
        rt.install_intrinsics();
        install(&mut rt);
        install_poll_io(&mut rt);
        rt
    }

    #[test]
    fn watcher_registry_is_scoped_by_runtime_agent_id() {
        WATCHERS.with(|w| w.borrow_mut().clear());
        let mut rt_a = Runtime::new_with_agent_id(AgentId::from_raw(1201));
        let mut rt_b = Runtime::new_with_agent_id(AgentId::from_raw(1202));
        let watcher_obj_b = rt_b.alloc_object(rusty_js_runtime::value::Object::new_ordinary());
        let id_b = register_watcher(
            &mut rt_b,
            "__cruft_missing_watch_target_b".to_string(),
            None,
            watcher_obj_b,
            1,
        );

        assert!(!has_watchers(&rt_a));
        assert!(has_watchers(&rt_b));
        unregister_watcher(&mut rt_a, id_b);
        assert!(has_watchers(&rt_b));
        unregister_watcher(&mut rt_b, id_b);
        assert!(!has_watchers(&rt_b));
        WATCHERS.with(|w| w.borrow_mut().clear());
    }

    fn tmpdir(label: &str) -> std::path::PathBuf {
        let pid = std::process::id();
        let p = std::env::temp_dir().join(format!("cruft-fs-unit-{}-{}", pid, label));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("mkdir tmp");
        p
    }

    fn compile(src: &str) -> rusty_js_bytecode::CompiledModule {
        rusty_js_bytecode::compile_module(src).expect("compile")
    }

    fn run_with(rt: &mut Runtime, src: &str) {
        let m = compile(src);
        rt.run_module(&m).expect("run");
        rt.run_to_completion().expect("loop");
    }

    fn recorded(rt: &Runtime) -> Option<Value> {

        let v = rt.global_get("__last_recorded");
        if matches!(v, Value::Undefined) {
            None
        } else {
            Some(v)
        }
    }

    #[test]
    fn write_then_read_sync_utf8() {
        let dir = tmpdir("rw-utf8");
        let path = dir.join("a.txt");
        let mut rt = fresh();
        rt.define_global_property(
            "PATH",
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                path.to_string_lossy().into_owned(),
            ))),
        );
        run_with(
            &mut rt,
            r#"fs.writeFileSync(PATH, "hello, world");
               __record(fs.readFileSync(PATH, "utf-8"));"#,
        );
        match recorded(&rt) {
            Some(Value::String(s)) => assert_eq!(s.as_str(), "hello, world"),
            other => panic!("unexpected: {:?}", other),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_sync_bytes_default_returns_array() {
        let dir = tmpdir("bytes");
        let path = dir.join("b.bin");
        std::fs::write(&path, [0x68u8, 0x69]).unwrap();
        let mut rt = fresh();
        rt.define_global_property(
            "PATH",
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                path.to_string_lossy().into_owned(),
            ))),
        );
        run_with(
            &mut rt,
            r#"let b = fs.readFileSync(PATH); __record(b.length + ":" + b[0] + "," + b[1]);"#,
        );
        match recorded(&rt) {
            Some(Value::String(s)) => assert_eq!(s.as_str(), "2:104,105"),
            other => panic!("{:?}", other),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn exists_sync_true_and_false() {
        let dir = tmpdir("exists");
        let present = dir.join("p");
        std::fs::write(&present, "x").unwrap();
        let missing = dir.join("missing");
        let mut rt = fresh();
        rt.define_global_property(
            "P",
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                present.to_string_lossy().into_owned(),
            ))),
        );
        rt.define_global_property(
            "M",
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                missing.to_string_lossy().into_owned(),
            ))),
        );
        run_with(
            &mut rt,
            "__record(fs.existsSync(P) + ',' + fs.existsSync(M));",
        );
        match recorded(&rt) {
            Some(Value::String(s)) => assert_eq!(s.as_str(), "true,false"),
            other => panic!("{:?}", other),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stat_sync_reports_file_and_size() {
        let dir = tmpdir("stat");
        let path = dir.join("s.txt");
        std::fs::write(&path, "abcd").unwrap();
        let mut rt = fresh();
        rt.define_global_property(
            "PATH",
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                path.to_string_lossy().into_owned(),
            ))),
        );
        run_with(&mut rt, "let s = fs.statSync(PATH); __record(s.size + ',' + s.isFile() + ',' + s.isDirectory());");
        match recorded(&rt) {
            Some(Value::String(s)) => assert_eq!(s.as_str(), "4,true,false"),
            other => panic!("{:?}", other),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn readdir_sync_lists_entries() {
        let dir = tmpdir("dir");
        std::fs::write(dir.join("a"), "").unwrap();
        std::fs::write(dir.join("b"), "").unwrap();
        let mut rt = fresh();
        rt.define_global_property(
            "D",
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                dir.to_string_lossy().into_owned(),
            ))),
        );
        run_with(&mut rt, "let e = fs.readdirSync(D); __record(e.length);");
        assert!(matches!(recorded(&rt), Some(Value::Number(n)) if n == 2.0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn mkdir_sync_recursive() {
        let dir = tmpdir("mkdir");
        let nested = dir.join("a/b/c");
        let mut rt = fresh();
        rt.define_global_property(
            "D",
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                nested.to_string_lossy().into_owned(),
            ))),
        );
        run_with(
            &mut rt,
            "fs.mkdirSync(D, {recursive: true}); __record(fs.existsSync(D));",
        );
        assert!(matches!(recorded(&rt), Some(Value::Boolean(true))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unlink_sync_removes() {
        let dir = tmpdir("unlink");
        let path = dir.join("u");
        std::fs::write(&path, "x").unwrap();
        let mut rt = fresh();
        rt.define_global_property(
            "P",
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                path.to_string_lossy().into_owned(),
            ))),
        );
        run_with(&mut rt, "fs.unlinkSync(P); __record(fs.existsSync(P));");
        assert!(matches!(recorded(&rt), Some(Value::Boolean(false))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn writefile_sync_then_readfilesync_bytes_roundtrip() {
        let dir = tmpdir("byte-rt");
        let path = dir.join("r.bin");
        let mut rt = fresh();
        rt.define_global_property(
            "PATH",
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                path.to_string_lossy().into_owned(),
            ))),
        );

        run_with(
            &mut rt,
            r#"let arr = [72, 73]; arr.length = 2;
               fs.writeFileSync(PATH, arr);
               __record(fs.readFileSync(PATH, "utf8"));"#,
        );
        match recorded(&rt) {
            Some(Value::String(s)) => assert_eq!(s.as_str(), "HI"),
            other => panic!("{:?}", other),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn async_read_resolves_through_poll_io() {
        let dir = tmpdir("async-read");
        let path = dir.join("a.txt");
        std::fs::write(&path, "async-payload").unwrap();
        let mut rt = fresh();
        rt.define_global_property(
            "PATH",
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                path.to_string_lossy().into_owned(),
            ))),
        );

        run_with(
            &mut rt,
            r#"Promise.then(fs.readFile(PATH, "utf-8"), function(s) {
                  __record(s);
               });"#,
        );
        match recorded(&rt) {
            Some(Value::String(s)) => assert_eq!(s.as_str(), "async-payload"),
            other => panic!("{:?}", other),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn async_exists_resolves_through_poll_io() {
        let dir = tmpdir("async-exists");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p");
        std::fs::write(&path, "x").unwrap();
        let mut rt = fresh();
        rt.define_global_property(
            "P",
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                path.to_string_lossy().into_owned(),
            ))),
        );
        run_with(
            &mut rt,
            r#"Promise.then(fs.exists(P), function(b) { __record(b ? "yes" : "no"); });"#,
        );
        assert!(matches!(recorded(&rt), Some(Value::String(ref s)) if s.as_str() == "yes"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn async_write_then_read_chain() {
        let dir = tmpdir("async-chain");
        let path = dir.join("c.txt");
        let mut rt = fresh();
        rt.define_global_property(
            "PATH",
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                path.to_string_lossy().into_owned(),
            ))),
        );
        run_with(
            &mut rt,
            r#"Promise.then(fs.writeFile(PATH, "chained"), function() {
                  Promise.then(fs.readFile(PATH, "utf-8"), function(s) { __record(s); });
               });"#,
        );
        match recorded(&rt) {
            Some(Value::String(s)) => assert_eq!(s.as_str(), "chained"),
            other => panic!("{:?}", other),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn callback_readfile_invokes_err_first_callback() {
        let dir = tmpdir("callback-read");
        let path = dir.join("a.txt");
        std::fs::write(&path, "callback-payload").unwrap();
        let mut rt = fresh();
        rt.define_global_property(
            "PATH",
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                path.to_string_lossy().into_owned(),
            ))),
        );
        run_with(
            &mut rt,
            r#"let ret = fs.readFile(PATH, "utf8", function(err, data) {
                  __record(typeof ret + ":" + (err === null) + ":" + data);
               });"#,
        );
        match recorded(&rt) {
            Some(Value::String(s)) => assert_eq!(s.as_str(), "undefined:true:callback-payload"),
            other => panic!("{:?}", other),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn callback_writefile_invokes_after_write() {
        let dir = tmpdir("callback-write");
        let path = dir.join("w.txt");
        let mut rt = fresh();
        rt.define_global_property(
            "PATH",
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                path.to_string_lossy().into_owned(),
            ))),
        );
        run_with(
            &mut rt,
            r#"let ret = fs.writeFile(PATH, "callback-write", function(err) {
                  __record(typeof ret + ":" + (err === null) + ":" + fs.readFileSync(PATH, "utf8"));
               });"#,
        );
        match recorded(&rt) {
            Some(Value::String(s)) => assert_eq!(s.as_str(), "undefined:true:callback-write"),
            other => panic!("{:?}", other),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn callback_exists_invokes_with_boolean() {
        let dir = tmpdir("callback-exists");
        let path = dir.join("p");
        std::fs::write(&path, "x").unwrap();
        let mut rt = fresh();
        rt.define_global_property(
            "P",
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                path.to_string_lossy().into_owned(),
            ))),
        );
        run_with(
            &mut rt,
            r#"let ret = fs.exists(P, function(ok) {
                  __record(typeof ret + ":" + ok);
               });"#,
        );
        match recorded(&rt) {
            Some(Value::String(s)) => assert_eq!(s.as_str(), "undefined:true"),
            other => panic!("{:?}", other),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
