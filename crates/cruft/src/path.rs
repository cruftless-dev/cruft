
use crate::register::{new_object, register_method, set_constant};
use rusty_js_runtime::value::JsString;
use rusty_js_runtime::value::ObjectRef;
use rusty_js_runtime::{Runtime, RuntimeError, Value};
use std::rc::Rc;

pub fn install(rt: &mut Runtime) {
    let path = new_object(rt);

    register_method(rt, path, "basename", |rt, args| {
        let ext = optional_path_string_arg(rt, args, 1, "suffix")?;
        Ok(js_string_from(with_path_arg(rt, args, 0, "path", |p| {
            posix_basename(p, ext)
        })?))
    });

    register_method(rt, path, "dirname", |rt, args| {
        Ok(js_string_from(with_path_arg(
            rt,
            args,
            0,
            "path",
            posix_dirname,
        )?))
    });

    register_method(rt, path, "extname", |rt, args| {
        Ok(js_string_from(with_path_arg(
            rt,
            args,
            0,
            "path",
            posix_extname,
        )?))
    });

    register_method(rt, path, "join", |rt, args| {
        Ok(js_string_from(posix_join_args(rt, args)?))
    });

    register_method(rt, path, "normalize", |rt, args| {
        Ok(js_string_from(with_path_arg(
            rt,
            args,
            0,
            "path",
            posix_normalize,
        )?))
    });

    register_method(rt, path, "isAbsolute", |rt, args| {
        Ok(Value::Boolean(with_path_arg(rt, args, 0, "path", |p| {
            p.starts_with('/')
        })?))
    });

    register_method(rt, path, "resolve", |rt, args| {
        Ok(js_string_from(posix_resolve_args(rt, args)?))
    });

    set_constant(
        rt,
        path,
        "sep",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from("/"))),
    );
    set_constant(
        rt,
        path,
        "delimiter",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(":"))),
    );

    register_method(rt, path, "parse", |rt, args| path_parse_via(rt, args));
    register_method(rt, path, "format", |rt, args| {
        let o = match args.first() {
            Some(Value::Object(id)) => *id,
            Some(v) => return Err(path_invalid_arg_type(rt, "pathObject", "object", Some(v))),
            None => return Err(path_invalid_arg_type(rt, "pathObject", "object", None)),
        };
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(posix_format(rt, o)),
        )))
    });
    register_method(rt, path, "relative", |rt, args| {
        let from = path_string_arg(rt, args, 0, "from")?;
        let to = path_string_arg(rt, args, 1, "to")?;
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(posix_relative(from, to)),
        )))
    });
    register_method(rt, path, "toNamespacedPath", |_rt, args| {
        Ok(args.first().cloned().unwrap_or(Value::Undefined))
    });

    register_method(rt, path, "matchesGlob", |_rt, args| {
        let p = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => return Ok(Value::Boolean(false)),
        };
        let pat = match args.get(1) {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => return Ok(Value::Boolean(false)),
        };
        Ok(Value::Boolean(glob_match(pat.as_bytes(), p.as_bytes())))
    });

    let posix = new_object(rt);
    let win32 = new_object(rt);
    for &(name, _) in &[
        ("basename", 0u8),
        ("dirname", 0),
        ("extname", 0),
        ("join", 0),
        ("normalize", 0),
        ("isAbsolute", 0),
        ("resolve", 0),
    ] {

        let v = rt.object_get(path, &name.to_string());
        rt.object_set(posix, name.into(), v);
    }
    for nm in &["parse", "format", "relative"] {
        let v = rt.object_get(path, &nm.to_string());
        rt.object_set(posix, (*nm).into(), v);
    }

    register_win32(rt, win32);

    for nm in &["matchesGlob", "toNamespacedPath"] {
        let v = rt.object_get(path, &nm.to_string());
        rt.object_set(posix, (*nm).into(), v.clone());
        rt.object_set(win32, (*nm).into(), v);
    }

    register_method(rt, win32, "toNamespacedPath", |_rt, args| {
        let s = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            other => return Ok(other.cloned().unwrap_or(Value::Undefined)),
        };
        let b = s.as_bytes();
        let out = if b.len() < 3 {
            s.clone()
        } else if b[0] == b'\\' && b[1] == b'\\' {

            if b[2] == b'?' || b[2] == b'.' {
                s.clone()
            } else {
                format!("\\\\?\\UNC\\{}", &s[2..])
            }
        } else if b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/') {

            format!("\\\\?\\{}", s.replace('/', "\\"))
        } else {
            s.clone()
        };
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(out),
        )))
    });
    rt.object_set(
        posix,
        "sep".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from("/"))),
    );
    rt.object_set(
        posix,
        "delimiter".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(":"))),
    );
    rt.object_set(
        win32,
        "sep".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from("\\"))),
    );
    rt.object_set(
        win32,
        "delimiter".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(";"))),
    );

    #[cfg(windows)]
    {
        for nm in &[
            "basename",
            "dirname",
            "extname",
            "join",
            "normalize",
            "isAbsolute",
            "resolve",
            "parse",
            "format",
            "relative",
            "sep",
            "delimiter",
        ] {
            let v = rt.object_get(win32, &nm.to_string());
            rt.object_set(path, (*nm).into(), v);
        }
    }

    rt.object_set(path, "posix".into(), Value::Object(posix));
    rt.object_set(path, "win32".into(), Value::Object(win32));

    for v in [path, posix, win32] {
        register_method(rt, v, "_makeLong", |_rt, args| {
            Ok(args.first().cloned().unwrap_or(Value::Undefined))
        });
    }
    rt.object_set(posix, "posix".into(), Value::Object(posix));
    rt.object_set(posix, "win32".into(), Value::Object(win32));
    rt.object_set(win32, "posix".into(), Value::Object(posix));
    rt.object_set(win32, "win32".into(), Value::Object(win32));
    rt.define_global_property("path", Value::Object(path));
}

fn register_core(rt: &mut Runtime, obj: ObjectRef) {
    register_method(rt, obj, "basename", |rt, args| {
        let ext = optional_path_string_arg(rt, args, 1, "suffix")?;
        Ok(js_string_from(with_path_arg(rt, args, 0, "path", |p| {
            posix_basename(p, ext)
        })?))
    });
    register_method(rt, obj, "dirname", |rt, args| {
        Ok(js_string_from(with_path_arg(
            rt,
            args,
            0,
            "path",
            posix_dirname,
        )?))
    });
    register_method(rt, obj, "extname", |rt, args| {
        Ok(js_string_from(with_path_arg(
            rt,
            args,
            0,
            "path",
            posix_extname,
        )?))
    });
    register_method(rt, obj, "join", |rt, args| {
        Ok(js_string_from(posix_join_args(rt, args)?))
    });
    register_method(rt, obj, "normalize", |rt, args| {
        Ok(js_string_from(with_path_arg(
            rt,
            args,
            0,
            "path",
            posix_normalize,
        )?))
    });
    register_method(rt, obj, "isAbsolute", |rt, args| {
        Ok(Value::Boolean(with_path_arg(rt, args, 0, "path", |p| {
            p.starts_with('/')
        })?))
    });
    register_method(rt, obj, "resolve", |rt, args| {
        Ok(js_string_from(posix_resolve_args(rt, args)?))
    });
    register_method(rt, obj, "parse", |rt, args| path_parse_via(rt, args));
    register_method(rt, obj, "format", |rt, args| {
        let o = match args.first() {
            Some(Value::Object(id)) => *id,
            Some(v) => return Err(path_invalid_arg_type(rt, "pathObject", "object", Some(v))),
            None => return Err(path_invalid_arg_type(rt, "pathObject", "object", None)),
        };
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(posix_format(rt, o)),
        )))
    });
    register_method(rt, obj, "relative", |rt, args| {
        let from = path_string_arg(rt, args, 0, "from")?;
        let to = path_string_arg(rt, args, 1, "to")?;
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(posix_relative(from, to)),
        )))
    });
    set_constant(
        rt,
        obj,
        "sep",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from("/"))),
    );
    set_constant(
        rt,
        obj,
        "delimiter",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(":"))),
    );
}

pub fn install_canonical(rt: &mut Runtime) {
    let ns = new_object(rt);
    register_core(rt, ns);
    rt.define_global_property("__cruft_path", Value::Object(ns));
}

#[derive(Default)]
struct PosixParsed {
    root: String,
    dir: String,
    base: String,
    ext: String,
    name: String,
}

type ParsedPathStringSlots = [Rc<JsString>; 5];

fn alloc_posix_parsed_path(rt: &mut Runtime, parsed: PosixParsed) -> ObjectRef {
    alloc_posix_parsed_path_strings(
        rt,
        [
            Rc::new(JsString::from(parsed.root)),
            Rc::new(JsString::from(parsed.dir)),
            Rc::new(JsString::from(parsed.base)),
            Rc::new(JsString::from(parsed.ext)),
            Rc::new(JsString::from(parsed.name)),
        ],
    )
}

fn alloc_posix_parsed_path_strings(rt: &mut Runtime, slots: ParsedPathStringSlots) -> ObjectRef {
    record_path_parse_materialize_direct();
    if path_parse_static_shape_enabled() {
        return alloc_posix_parsed_path_static_shape(rt, slots);
    }
    let mut out = rusty_js_runtime::value::Object::new_ordinary_with_shape_capacity(5);
    let subphase_counters = path_parse_materialize_subphase_counters_enabled();
    let root_start = subphase_counters.then(std::time::Instant::now);
    out.set_own_literal_key("root", Value::String(slots[0].clone()));
    record_path_parse_materialize_subphase(PathParseMaterializeSubphase::Root, root_start);
    let dir_start = subphase_counters.then(std::time::Instant::now);
    out.set_own_literal_key("dir", Value::String(slots[1].clone()));
    record_path_parse_materialize_subphase(PathParseMaterializeSubphase::Dir, dir_start);
    let base_start = subphase_counters.then(std::time::Instant::now);
    out.set_own_literal_key("base", Value::String(slots[2].clone()));
    record_path_parse_materialize_subphase(PathParseMaterializeSubphase::Base, base_start);

    let ext_start = subphase_counters.then(std::time::Instant::now);
    out.set_own_literal_key("ext", Value::String(slots[3].clone()));
    record_path_parse_materialize_subphase(PathParseMaterializeSubphase::Ext, ext_start);
    let name_start = subphase_counters.then(std::time::Instant::now);
    out.set_own_literal_key("name", Value::String(slots[4].clone()));
    record_path_parse_materialize_subphase(PathParseMaterializeSubphase::Name, name_start);
    let alloc_start = subphase_counters.then(std::time::Instant::now);
    let id = rt.alloc_object(out);
    record_path_parse_materialize_subphase(PathParseMaterializeSubphase::Alloc, alloc_start);
    id
}

fn alloc_posix_parsed_path_static_shape(
    rt: &mut Runtime,
    slots: ParsedPathStringSlots,
) -> ObjectRef {
    let subphase_counters = path_parse_materialize_subphase_counters_enabled();
    let slots_start = subphase_counters.then(std::time::Instant::now);
    let mut out = rusty_js_runtime::value::Object::new_ordinary_with_shape_template(
        posix_parsed_path_shape(),
        5,
    );
    out.shape_values[0] = Value::String(slots[0].clone());
    out.shape_values[1] = Value::String(slots[1].clone());
    out.shape_values[2] = Value::String(slots[2].clone());
    out.shape_values[3] = Value::String(slots[3].clone());
    out.shape_values[4] = Value::String(slots[4].clone());
    record_path_parse_materialize_subphase(PathParseMaterializeSubphase::Slots, slots_start);
    let alloc_start = subphase_counters.then(std::time::Instant::now);
    let id = rt.alloc_object(out);
    record_path_parse_materialize_subphase(PathParseMaterializeSubphase::Alloc, alloc_start);
    id
}

fn posix_parsed_path_shape() -> Rc<rusty_js_shapes::Shape> {
    thread_local! {
        static SHAPE: Rc<rusty_js_shapes::Shape> = rusty_js_shapes::Shape::root()
            .transition_to("root")
            .transition_to("dir")
            .transition_to("base")
            .transition_to("ext")
            .transition_to("name");
    }
    SHAPE.with(Rc::clone)
}

fn path_parse_static_shape_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_PATH_PARSE_STATIC_SHAPE")
            .map(|v| v != "0")
            .unwrap_or(true)
    })
}

fn path_parse_via(rt: &mut Runtime, args: &[Value]) -> Result<Value, RuntimeError> {
    let phase_counters = path_parse_phase_counters_enabled();
    let total_start = phase_counters.then(std::time::Instant::now);
    let parse_start = phase_counters.then(std::time::Instant::now);
    path_string_arg(rt, args, 0, "path")?;
    let cached_slots = path_parse_cached_string_slots(args);
    let parsed = if cached_slots.is_none() {
        Some(posix_parse(path_string_arg(rt, args, 0, "path")?))
    } else {
        None
    };
    let parse_ns = parse_start
        .map(|start| start.elapsed().as_nanos() as u64)
        .unwrap_or(0);
    let materialize_start = phase_counters.then(std::time::Instant::now);
    let out = match cached_slots {
        Some(slots) => alloc_posix_parsed_path_strings(rt, slots),
        None => alloc_posix_parsed_path(rt, parsed.expect("parsed path computed on cache miss")),
    };
    let materialize_ns = materialize_start
        .map(|start| start.elapsed().as_nanos() as u64)
        .unwrap_or(0);
    if let Some(start) = total_start {
        record_path_parse_phase(parse_ns, materialize_ns, start.elapsed().as_nanos() as u64);
    }
    Ok(Value::Object(out))
}

fn path_parse_last_cache_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_PATH_PARSE_LAST_CACHE")
            .map(|v| v != "0" && !v.eq_ignore_ascii_case("false"))
            .unwrap_or(true)
    })
}

fn path_parse_cached_string_slots(args: &[Value]) -> Option<ParsedPathStringSlots> {
    if !path_parse_last_cache_enabled() {
        return None;
    }
    let Some(Value::String(path)) = args.first() else {
        return None;
    };
    let path_str = path.as_str();
    thread_local! {
        static LAST: std::cell::RefCell<Option<(String, ParsedPathStringSlots)>> =
            const { std::cell::RefCell::new(None) };
    }
    LAST.with(|last| {
        let mut last = last.borrow_mut();
        if let Some((cached_path, slots)) = last.as_ref() {
            if cached_path == path_str {
                return Some([
                    slots[0].clone(),
                    slots[1].clone(),
                    slots[2].clone(),
                    slots[3].clone(),
                    slots[4].clone(),
                ]);
            }
        }
        let parsed = posix_parse(path_str);
        let slots = [
            Rc::new(JsString::from(parsed.root)),
            Rc::new(JsString::from(parsed.dir)),
            Rc::new(JsString::from(parsed.base)),
            Rc::new(JsString::from(parsed.ext)),
            Rc::new(JsString::from(parsed.name)),
        ];
        *last = Some((path_str.to_string(), slots.clone()));
        Some(slots)
    })
}

fn path_parse_phase_counters_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_PATH_PARSE_PHASE_COUNTERS")
            .map(|v| v != "0")
            .unwrap_or(false)
    })
}

fn record_path_parse_phase(parse_ns: u64, materialize_ns: u64, total_ns: u64) {
    static CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    static PARSE_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    static MATERIALIZE_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    static TOTAL_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let calls = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    PARSE_NS.fetch_add(parse_ns, std::sync::atomic::Ordering::Relaxed);
    MATERIALIZE_NS.fetch_add(materialize_ns, std::sync::atomic::Ordering::Relaxed);
    TOTAL_NS.fetch_add(total_ns, std::sync::atomic::Ordering::Relaxed);
    if calls <= 8 || calls.is_power_of_two() {
        let avg = |counter: &std::sync::atomic::AtomicU64| {
            counter.load(std::sync::atomic::Ordering::Relaxed) / calls
        };
        eprintln!(
            "[path-parse-phase] calls={} avg_parse_ns={} avg_materialize_ns={} avg_total_ns={}",
            calls,
            avg(&PARSE_NS),
            avg(&MATERIALIZE_NS),
            avg(&TOTAL_NS)
        );
    }
}

fn path_parse_materialize_counters_enabled() -> bool {
    std::env::var("CRUFT_PATH_PARSE_MATERIALIZE_COUNTERS")
        .map(|v| v != "0")
        .unwrap_or(false)
}

fn record_path_parse_materialize_direct() {
    if !path_parse_materialize_counters_enabled() {
        return;
    }
    static CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let calls = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    if calls <= 3 || calls % 100_000 == 0 {
        eprintln!("[path-parse-materialize-direct] calls={calls}");
    }
}

#[derive(Clone, Copy)]
enum PathParseMaterializeSubphase {
    Root,
    Dir,
    Base,
    Ext,
    Name,
    Slots,
    Alloc,
}

fn path_parse_materialize_subphase_counters_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_PATH_PARSE_MATERIALIZE_SUBPHASE_COUNTERS")
            .map(|v| v != "0")
            .unwrap_or(false)
    })
}

fn record_path_parse_materialize_subphase(
    phase: PathParseMaterializeSubphase,
    start: Option<std::time::Instant>,
) {
    let Some(start) = start else {
        return;
    };
    static CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    static ROOT_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    static DIR_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    static BASE_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    static EXT_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    static NAME_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    static SLOTS_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    static ALLOC_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let elapsed = start.elapsed().as_nanos() as u64;
    match phase {
        PathParseMaterializeSubphase::Root => {
            ROOT_NS.fetch_add(elapsed, std::sync::atomic::Ordering::Relaxed);
        }
        PathParseMaterializeSubphase::Dir => {
            DIR_NS.fetch_add(elapsed, std::sync::atomic::Ordering::Relaxed);
        }
        PathParseMaterializeSubphase::Base => {
            BASE_NS.fetch_add(elapsed, std::sync::atomic::Ordering::Relaxed);
        }
        PathParseMaterializeSubphase::Ext => {
            EXT_NS.fetch_add(elapsed, std::sync::atomic::Ordering::Relaxed);
        }
        PathParseMaterializeSubphase::Name => {
            NAME_NS.fetch_add(elapsed, std::sync::atomic::Ordering::Relaxed);
        }
        PathParseMaterializeSubphase::Slots => {
            SLOTS_NS.fetch_add(elapsed, std::sync::atomic::Ordering::Relaxed);
        }
        PathParseMaterializeSubphase::Alloc => {
            ALLOC_NS.fetch_add(elapsed, std::sync::atomic::Ordering::Relaxed);
            let calls = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            if calls <= 8 || calls.is_power_of_two() {
                let avg = |counter: &std::sync::atomic::AtomicU64| {
                    counter.load(std::sync::atomic::Ordering::Relaxed) / calls
                };
                eprintln!(
                    "[path-parse-materialize-subphase] calls={} avg_root_ns={} avg_dir_ns={} avg_base_ns={} avg_ext_ns={} avg_name_ns={} avg_slots_ns={} avg_alloc_ns={}",
                    calls,
                    avg(&ROOT_NS),
                    avg(&DIR_NS),
                    avg(&BASE_NS),
                    avg(&EXT_NS),
                    avg(&NAME_NS),
                    avg(&SLOTS_NS),
                    avg(&ALLOC_NS)
                );
            }
        }
    }
}

fn js_string_from(s: String) -> Value {
    Value::String(Rc::new(JsString::from(s)))
}

fn path_invalid_arg_suffix(rt: &Runtime, value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Undefined) => " Received undefined".to_string(),
        Some(Value::Null) => " Received null".to_string(),
        Some(Value::String(s)) => format!(" Received type string ('{}')", s.as_str()),
        Some(Value::Number(n)) if n.is_nan() => " Received type number (NaN)".to_string(),
        Some(Value::Number(n)) => format!(" Received type number ({})", n),
        Some(Value::Boolean(b)) => format!(" Received type boolean ({})", b),
        Some(Value::Object(id)) => {
            let ctor = match rt.object_get(*id, "constructor") {
                Value::Object(c) => match rt.object_get(c, "name") {
                    Value::String(name) if !name.as_str().is_empty() => name.as_str().to_string(),
                    _ => "Object".to_string(),
                },
                _ => "Object".to_string(),
            };
            format!(" Received an instance of {ctor}")
        }
        _ => " Received an invalid value".to_string(),
    }
}

fn path_invalid_arg_type(
    rt: &mut Runtime,
    name: &str,
    expected: &str,
    value: Option<&Value>,
) -> RuntimeError {
    let msg = format!(
        "The \"{}\" argument must be of type {}.{}",
        name,
        expected,
        path_invalid_arg_suffix(rt, value)
    );
    match rusty_js_runtime::intrinsics::make_error_instance(rt, "TypeError", &msg) {
        Some(id) => {
            rt.object_set(
                id,
                "code".into(),
                Value::String(Rc::new(JsString::from("ERR_INVALID_ARG_TYPE"))),
            );
            RuntimeError::Thrown(Value::Object(id))
        }
        None => RuntimeError::TypeError(msg),
    }
}

fn path_string_arg<'a>(
    rt: &mut Runtime,
    args: &'a [Value],
    i: usize,
    name: &str,
) -> Result<&'a str, RuntimeError> {
    match args.get(i) {
        Some(Value::String(s)) => Ok(s.as_str()),
        other => Err(path_invalid_arg_type(rt, name, "string", other)),
    }
}

fn optional_path_string_arg<'a>(
    rt: &mut Runtime,
    args: &'a [Value],
    i: usize,
    name: &str,
) -> Result<Option<&'a str>, RuntimeError> {
    match args.get(i) {
        None | Some(Value::Undefined) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.as_str())),
        other => Err(path_invalid_arg_type(rt, name, "string", other)),
    }
}

fn with_path_arg<R>(
    rt: &mut Runtime,
    args: &[Value],
    i: usize,
    name: &str,
    f: impl FnOnce(&str) -> R,
) -> Result<R, RuntimeError> {
    Ok(f(path_string_arg(rt, args, i, name)?))
}

fn path_string_parts(
    rt: &mut Runtime,
    args: &[Value],
    skip_empty: bool,
    fixed_arg_name: Option<&str>,
) -> Result<Vec<String>, RuntimeError> {
    let mut parts = Vec::new();
    for (idx, v) in args.iter().enumerate() {
        let Value::String(s) = v else {
            if let Some(name) = fixed_arg_name {
                return Err(path_invalid_arg_type(rt, name, "string", Some(v)));
            }
            let name = format!("paths[{idx}]");
            return Err(path_invalid_arg_type(rt, &name, "string", Some(v)));
        };
        if skip_empty && s.as_str().is_empty() {
            continue;
        }
        parts.push(s.as_str().to_string());
    }
    Ok(parts)
}

fn strip_trailing_seps(path: &str) -> &str {
    path.trim_end_matches('/')
}

fn posix_basename(path: &str, ext: Option<&str>) -> String {
    if path.is_empty() {
        return String::new();
    }
    let trimmed = strip_trailing_seps(path);
    if trimmed.is_empty() {
        return String::new();
    }
    let last = trimmed.rsplit('/').next().unwrap_or(trimmed);
    if let Some(suffix) = ext {
        if !suffix.is_empty() && last.ends_with(suffix) {
            if last == suffix && trimmed[..trimmed.len() - last.len()].contains('/') {
                return last.to_string();
            }
            return last[..last.len() - suffix.len()].to_string();
        }
    }
    last.to_string()
}

fn posix_dirname(path: &str) -> String {
    if path.is_empty() {
        return ".".to_string();
    }
    let trimmed = strip_trailing_seps(path);
    if trimmed.is_empty() {
        return "/".to_string();
    }
    match trimmed.rfind('/') {
        Some(0) => "/".to_string(),
        Some(1) if trimmed.starts_with("//") => "//".to_string(),
        Some(i) => trimmed[..i].to_string(),
        None => ".".to_string(),
    }
}

fn posix_extname(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    let trimmed = strip_trailing_seps(path);
    if trimmed.is_empty() {
        return String::new();
    }
    let base = trimmed.rsplit('/').next().unwrap_or(trimmed);
    posix_extname_from_base(base).to_string()
}

fn posix_extname_from_base(base: &str) -> &str {
    match base.rfind('.') {
        Some(0) => "",
        Some(1) if base.as_bytes().first() == Some(&b'.') && base.len() == 2 => "",
        Some(i) => &base[i..],
        None => "",
    }
}

fn posix_join(parts: &[String]) -> String {
    let mut joined = String::new();
    for part in parts {
        if part.is_empty() {
            continue;
        }
        if !joined.is_empty() && !joined.ends_with('/') {
            joined.push('/');
        }
        joined.push_str(part);
    }
    if joined.is_empty() {
        return ".".to_string();
    }
    posix_normalize(&joined)
}

fn posix_join_strs(parts: &[&str]) -> String {
    let mut joined = String::new();
    for part in parts {
        if part.is_empty() {
            continue;
        }
        if !joined.is_empty() && !joined.ends_with('/') {
            joined.push('/');
        }
        joined.push_str(part);
    }
    if joined.is_empty() {
        return ".".to_string();
    }
    posix_normalize(&joined)
}

fn posix_join_args(rt: &mut Runtime, args: &[Value]) -> Result<String, RuntimeError> {
    if args.iter().all(|v| matches!(v, Value::String(_))) {
        let parts: Vec<&str> = args
            .iter()
            .filter_map(|v| match v {
                Value::String(s) if !s.as_str().is_empty() => Some(s.as_str()),
                _ => None,
            })
            .collect();
        return Ok(posix_join_strs(&parts));
    }

    let mut parts = Vec::new();
    for v in args {
        let Value::String(s) = v else {
            return Err(path_invalid_arg_type(rt, "path", "string", Some(v)));
        };
        let part = s.as_str();
        if part.is_empty() {
            continue;
        }
        parts.push(part.to_string());
    }
    Ok(posix_join(&parts))
}

fn posix_resolve_strs(parts: &[&str]) -> String {
    let mut selected: Vec<&str> = Vec::new();
    let mut hit_absolute = false;
    for part in parts.iter().rev() {
        if part.is_empty() {
            continue;
        }
        selected.push(*part);
        if part.starts_with('/') {
            hit_absolute = true;
            break;
        }
    }
    selected.reverse();

    let cwd;
    if !hit_absolute {
        cwd = std::env::current_dir()
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "/".to_string());
        selected.insert(0, cwd.as_str());
    }

    let mut joined = String::new();
    for part in selected {
        if part.is_empty() {
            continue;
        }
        if !joined.is_empty() && !joined.ends_with('/') {
            joined.push('/');
        }
        joined.push_str(part);
    }
    posix_resolve_joined(&joined)
}

fn posix_resolve_args(rt: &mut Runtime, args: &[Value]) -> Result<String, RuntimeError> {
    if args.iter().all(|v| matches!(v, Value::String(_))) {
        let parts: Vec<&str> = args
            .iter()
            .filter_map(|v| match v {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        return Ok(posix_resolve_strs(&parts));
    }

    let mut parts: Vec<String> = Vec::new();
    let mut hit_absolute = false;
    for (idx, v) in args.iter().enumerate().rev() {
        let Value::String(s) = v else {
            let name = format!("paths[{idx}]");
            return Err(path_invalid_arg_type(rt, &name, "string", Some(v)));
        };
        let part = s.as_str().to_string();
        if part.is_empty() {
            continue;
        }
        parts.insert(0, part.clone());
        if part.starts_with('/') {
            hit_absolute = true;
            break;
        }
    }
    if !hit_absolute {
        let cwd = std::env::current_dir()
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "/".to_string());
        parts.insert(0, cwd);
    }
    Ok(posix_resolve_joined(&parts.join("/")))
}

fn posix_resolve_joined(joined: &str) -> String {
    let absolute = joined.starts_with('/');
    let segs: Vec<&str> = joined
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .collect();
    let mut out: Vec<&str> = Vec::new();
    for s in segs {
        if s == ".." {
            if !out.is_empty() {
                out.pop();
            }
        } else {
            out.push(s);
        }
    }
    if absolute {
        format!("/{}", out.join("/"))
    } else {
        out.join("/")
    }
}

fn posix_normalize(path: &str) -> String {
    if path.is_empty() {
        return ".".to_string();
    }
    let absolute = path.starts_with('/');
    let trailing_sep = path.len() > 1 && path.ends_with('/');
    let mut out: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if !out.is_empty() && out.last() != Some(&"..") {
                    out.pop();
                } else if !absolute {
                    out.push("..");
                }
            }
            _ => out.push(part),
        }
    }
    let mut result = match (absolute, out.is_empty()) {
        (true, true) => "/".to_string(),
        (true, false) => format!("/{}", out.join("/")),
        (false, true) => ".".to_string(),
        (false, false) => out.join("/"),
    };
    if trailing_sep && result != "/" && !result.ends_with('/') {
        result.push('/');
    }
    result
}

fn posix_parse(path: &str) -> PosixParsed {
    if path.is_empty() {
        return PosixParsed::default();
    }
    let has_root = path.starts_with('/');

    let path = {
        let t = path.trim_end_matches('/');
        if t.is_empty() {
            "/"
        } else {
            t
        }
    };
    let split = path.rfind('/');
    let dir = match split {
        Some(0) => "/",
        Some(i) => &path[..i],
        None => "",
    };
    let base = match split {
        Some(i) => &path[i + 1..],
        None => path,
    };
    let ext = posix_extname_from_base(base);
    let name = if !ext.is_empty() && base.ends_with(ext) {
        &base[..base.len() - ext.len()]
    } else {
        base
    };
    PosixParsed {
        root: if has_root { "/" } else { "" }.to_string(),
        dir: dir.to_string(),
        base: base.to_string(),
        ext: ext.to_string(),
        name: name.to_string(),
    }
}

fn object_string(rt: &Runtime, id: ObjectRef, name: &str) -> String {
    match rt.object_get(id, &name.to_string()) {
        Value::String(s) => s.as_str().to_string(),
        Value::Undefined => String::new(),
        v => rusty_js_runtime::abstract_ops::to_string(&v)
            .as_str()
            .to_string(),
    }
}

fn posix_format(rt: &Runtime, id: ObjectRef) -> String {
    let dir = object_string(rt, id, "dir");
    let root = object_string(rt, id, "root");
    let base = object_string(rt, id, "base");
    let name = object_string(rt, id, "name");
    let ext = object_string(rt, id, "ext");

    let base = if !base.is_empty() {
        base
    } else if !ext.is_empty() && !ext.starts_with('.') {
        format!("{}.{}", name, ext)
    } else {
        format!("{}{}", name, ext)
    };

    if dir.is_empty() && root.is_empty() {
        return base;
    }
    if base.is_empty() {
        let dir_or_root = if !dir.is_empty() { dir } else { root };
        return dir_or_root;
    }
    if !dir.is_empty() {
        if dir == "/" {
            format!("/{}", base)
        } else {
            format!("{}/{}", dir, base)
        }
    } else {
        format!("{}{}", root, base)
    }
}

fn posix_relative(from: &str, to: &str) -> String {
    let from_abs = posix_resolve_for_relative(from);
    let to_abs = posix_resolve_for_relative(to);
    if from_abs == to_abs {
        return String::new();
    }
    let from_segs: Vec<&str> = from_abs
        .trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    let to_segs: Vec<&str> = to_abs
        .trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    let common = from_segs
        .iter()
        .zip(to_segs.iter())
        .take_while(|(a, b)| a == b)
        .count();
    let mut parts: Vec<&str> = vec![".."; from_segs.len() - common];
    parts.extend_from_slice(&to_segs[common..]);
    parts.join("/")
}

fn posix_resolve_for_relative(path: &str) -> String {
    if path.starts_with('/') {
        posix_normalize(path)
    } else {
        let cwd = std::env::current_dir()
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "/".to_string());
        posix_normalize(&format!("{}/{}", cwd.trim_end_matches('/'), path))
    }
}

fn glob_match(pat: &[u8], s: &[u8]) -> bool {
    fn rec(p: &[u8], pi: usize, s: &[u8], si: usize) -> bool {
        if pi == p.len() {
            return si == s.len();
        }
        let c = p[pi];
        if c == b'*' {
            if pi + 1 < p.len() && p[pi + 1] == b'*' {

                let mut j = si;
                loop {
                    if rec(p, pi + 2, s, j) {
                        return true;
                    }
                    if j == s.len() {
                        return false;
                    }
                    j += 1;
                }
            } else {

                let mut j = si;
                loop {
                    if rec(p, pi + 1, s, j) {
                        return true;
                    }
                    if j == s.len() {
                        return false;
                    }
                    if s[j] == b'/' {
                        return false;
                    }
                    j += 1;
                }
            }
        } else if c == b'?' {
            if si < s.len() && s[si] != b'/' {
                rec(p, pi + 1, s, si + 1)
            } else {
                false
            }
        } else {
            if si < s.len() && s[si] == c {
                rec(p, pi + 1, s, si + 1)
            } else {
                false
            }
        }
    }
    rec(pat, 0, s, 0)
}

fn w_is_sep(c: char) -> bool {
    c == '\\' || c == '/'
}

fn win32_split(path: &str) -> (String, bool, String) {
    let ch: Vec<char> = path.chars().collect();
    let n = ch.len();
    if n >= 2 && w_is_sep(ch[0]) && w_is_sep(ch[1]) && (n == 2 || !w_is_sep(ch[2])) {
        let mut i = 2;
        while i < n && !w_is_sep(ch[i]) {
            i += 1;
        }
        let mut j = i;
        if j < n {
            j += 1;
            while j < n && !w_is_sep(ch[j]) {
                j += 1;
            }
        }
        if i > 2 && j > i + 1 {
            let device: String = ch[..j].iter().collect();
            let rest: String = ch[j..].iter().collect();
            let rest = rest.trim_start_matches(w_is_sep).to_string();
            return (device, true, rest);
        }
    }
    if n >= 2 && ch[1] == ':' && ch[0].is_ascii_alphabetic() {
        let device: String = ch[..2].iter().collect();
        let absolute = n >= 3 && w_is_sep(ch[2]);
        let mut s = 2;
        if absolute {
            while s < n && w_is_sep(ch[s]) {
                s += 1;
            }
        }
        let rest: String = ch[s..].iter().collect();
        return (device, absolute, rest);
    }
    if n >= 1 && w_is_sep(ch[0]) {
        let mut i = 0;
        while i < n && w_is_sep(ch[i]) {
            i += 1;
        }
        let rest: String = ch[i..].iter().collect();
        return (String::new(), true, rest);
    }
    (String::new(), false, path.to_string())
}

fn win32_absolute_root(path: &str, device: &str, absolute: bool) -> String {
    if !absolute {
        return device.to_string();
    }
    let sep = path[device.len()..]
        .chars()
        .find(|c| w_is_sep(*c))
        .unwrap_or('\\');
    format!("{}{}", device, sep)
}

fn win32_is_unc_device(device: &str) -> bool {
    let mut chars = device.chars();
    matches!(
        (chars.next(), chars.next()),
        (Some(a), Some(b)) if w_is_sep(a) && w_is_sep(b)
    )
}

fn win32_normalize(path: &str) -> String {
    if path.is_empty() {
        return ".".to_string();
    }
    let (device, absolute, rest) = win32_split(path);
    let trailing = rest.chars().last().map(w_is_sep).unwrap_or(false);
    let mut out: Vec<&str> = Vec::new();
    for part in rest.split(w_is_sep) {
        match part {
            "" | "." => {}
            ".." => {
                if !out.is_empty() && out.last() != Some(&"..") {
                    out.pop();
                } else if !absolute {
                    out.push("..");
                }
            }
            _ => out.push(part),
        }
    }
    let mut body = out.join("\\");
    if trailing && !body.is_empty() {
        body.push('\\');
    }
    if device.is_empty() && !absolute && body.is_empty() {
        return ".".to_string();
    }
    let mut result = if win32_is_unc_device(&device) {
        device
            .chars()
            .map(|c| if w_is_sep(c) { '\\' } else { c })
            .collect()
    } else {
        device.clone()
    };
    if absolute {
        result.push('\\');
    } else if !device.is_empty() && body.is_empty() {
        result.push('.');
    }
    result.push_str(&body);
    if result.is_empty() {
        ".".to_string()
    } else {
        result
    }
}

fn win32_join(parts: &[String]) -> String {
    let mut joined = String::new();
    for part in parts {
        if part.is_empty() {
            continue;
        }
        if joined.is_empty() {
            joined = part.clone();
        } else {
            if !joined.chars().last().map(w_is_sep).unwrap_or(false) {
                joined.push('\\');
            }
            joined.push_str(part);
        }
    }
    if joined.is_empty() {
        return ".".to_string();
    }
    win32_normalize(&joined)
}

fn win32_is_absolute(path: &str) -> bool {
    win32_split(path).1
}

fn win32_basename(path: &str, ext: Option<&str>) -> String {
    let s = path.trim_end_matches(w_is_sep);
    if s.is_empty() {
        return String::new();
    }
    let mut last = s.rsplit(w_is_sep).next().unwrap_or(s);
    if last.len() >= 2 && last.as_bytes()[1] == b':' && last.as_bytes()[0].is_ascii_alphabetic() {
        last = &last[2..];
    }
    if let Some(suf) = ext {
        if !suf.is_empty() && last.ends_with(suf) {
            if last == suf && s[..s.len() - last.len()].chars().any(w_is_sep) {
                return last.to_string();
            }
            return last[..last.len() - suf.len()].to_string();
        }
    }
    last.to_string()
}

fn win32_dirname(path: &str) -> String {
    if path.is_empty() {
        return ".".to_string();
    }
    let (device, absolute, rest) = win32_split(path);
    if absolute && device.is_empty() {
        let root = win32_absolute_root(path, &device, absolute);
        let rest_trim = path.trim_end_matches(w_is_sep);
        if rest_trim.is_empty() {
            return root;
        }
        return match rest_trim.rfind(w_is_sep) {
            Some(0) => root,
            Some(i) => rest_trim[..i].to_string(),
            None => root,
        };
    }
    if absolute && win32_is_unc_device(&device) && rest.is_empty() && path.ends_with(w_is_sep) {
        return win32_absolute_root(path, &device, absolute);
    }
    let rest_trim = rest.trim_end_matches(w_is_sep);
    match rest_trim.rfind(w_is_sep) {
        Some(i) => {
            let root = if absolute {
                win32_absolute_root(path, &device, absolute)
            } else {
                device.clone()
            };
            format!("{}{}", root, &rest_trim[..i])
        }
        None => {
            if absolute {
                if rest_trim.is_empty() && win32_is_unc_device(&device) {
                    device
                } else {
                    win32_absolute_root(path, &device, absolute)
                }
            } else if !device.is_empty() {
                device
            } else {
                ".".to_string()
            }
        }
    }
}

fn win32_extname(path: &str) -> String {
    let base = win32_basename(path, None);
    posix_extname_from_base(&base).to_string()
}

fn win32_parse(path: &str) -> PosixParsed {
    if path.is_empty() {
        return PosixParsed::default();
    }
    let (device, absolute, rest) = win32_split(path);
    if absolute && win32_is_unc_device(&device) && rest.is_empty() {
        let rooted = if path.ends_with(w_is_sep) {
            win32_absolute_root(path, &device, absolute)
        } else {
            device.clone()
        };
        return PosixParsed {
            root: rooted.clone(),
            dir: rooted,
            base: String::new(),
            ext: String::new(),
            name: String::new(),
        };
    }
    let root = if absolute {
        win32_absolute_root(path, &device, absolute)
    } else if !device.is_empty() {
        device.clone()
    } else {
        String::new()
    };
    let base = win32_basename(path, None);
    let ext = win32_extname(path);
    let name = if !ext.is_empty() && base.ends_with(&ext) {
        base[..base.len() - ext.len()].to_string()
    } else {
        base.clone()
    };
    let dir = {
        let d = win32_dirname(path);
        if d == "." {
            String::new()
        } else {
            d
        }
    };
    PosixParsed {
        root,
        dir,
        base,
        ext,
        name,
    }
}

fn win32_format(rt: &Runtime, id: ObjectRef) -> String {
    let dir = object_string(rt, id, "dir");
    let root = object_string(rt, id, "root");
    let base = object_string(rt, id, "base");
    let name = object_string(rt, id, "name");
    let ext = object_string(rt, id, "ext");
    let base = if !base.is_empty() {
        base
    } else {
        format!("{}{}", name, ext)
    };
    if dir.is_empty() && root.is_empty() {
        return base;
    }
    if base.is_empty() {
        let dir_or_root = if !dir.is_empty() { dir } else { root };
        return dir_or_root;
    }
    if !dir.is_empty() {
        if win32_format_dir_is_root(&dir) {
            format!("{}{}", dir, base)
        } else {
            format!("{}\\{}", dir, base)
        }
    } else {
        format!("{}{}", root, base)
    }
}

fn win32_format_dir_is_root(dir: &str) -> bool {
    let (device, absolute, rest) = win32_split(dir);
    absolute && rest.is_empty() && dir.ends_with('\\') && !device.is_empty()
}

fn win32_relative(from: &str, to: &str) -> String {
    let f = win32_normalize(from);
    let t = win32_normalize(to);
    if f.eq_ignore_ascii_case(&t) {
        return String::new();
    }
    let (fd, _fa, fr) = win32_split(&f);
    let (td, _ta, tr) = win32_split(&t);
    if !fd.eq_ignore_ascii_case(&td) {
        return t;
    }
    let fseg: Vec<&str> = fr.split(w_is_sep).filter(|s| !s.is_empty()).collect();
    let tseg: Vec<&str> = tr.split(w_is_sep).filter(|s| !s.is_empty()).collect();
    let common = fseg
        .iter()
        .zip(tseg.iter())
        .take_while(|(a, b)| a.eq_ignore_ascii_case(b))
        .count();
    let mut parts: Vec<&str> = vec![".."; fseg.len() - common];
    parts.extend_from_slice(&tseg[common..]);
    parts.join("\\")
}

fn win32_resolve(parts: &[String]) -> String {
    let mut resolved = String::new();
    let mut absolute = false;
    for part in parts.iter().rev() {
        if part.is_empty() {
            continue;
        }
        resolved = if resolved.is_empty() {
            part.clone()
        } else {
            format!("{}\\{}", part, resolved)
        };
        if win32_is_absolute(part) {
            absolute = true;
            break;
        }
    }
    if !absolute {
        let cwd = std::env::current_dir()
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "C:\\".to_string());
        resolved = if resolved.is_empty() {
            cwd
        } else {
            format!("{}\\{}", cwd, resolved)
        };
    }
    win32_normalize(&resolved)
}

fn register_win32(rt: &mut Runtime, win32: ObjectRef) {
    register_method(rt, win32, "basename", |rt, args| {
        let p = path_string_arg(rt, args, 0, "path")?;
        let ext = optional_path_string_arg(rt, args, 1, "suffix")?;
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(win32_basename(p, ext)),
        )))
    });
    register_method(rt, win32, "dirname", |rt, args| {
        let p = path_string_arg(rt, args, 0, "path")?;
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(win32_dirname(p)),
        )))
    });
    register_method(rt, win32, "extname", |rt, args| {
        let p = path_string_arg(rt, args, 0, "path")?;
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(win32_extname(p)),
        )))
    });
    register_method(rt, win32, "join", |rt, args| {
        let parts = path_string_parts(rt, args, true, Some("path"))?;
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(win32_join(&parts)),
        )))
    });
    register_method(rt, win32, "normalize", |rt, args| {
        let p = path_string_arg(rt, args, 0, "path")?;
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(win32_normalize(p)),
        )))
    });
    register_method(rt, win32, "isAbsolute", |rt, args| {
        let p = path_string_arg(rt, args, 0, "path")?;
        Ok(Value::Boolean(win32_is_absolute(p)))
    });
    register_method(rt, win32, "resolve", |rt, args| {
        let parts = path_string_parts(rt, args, false, None)?;
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(win32_resolve(&parts)),
        )))
    });
    register_method(rt, win32, "parse", |rt, args| {
        let parsed = win32_parse(path_string_arg(rt, args, 0, "path")?);
        let out = rt.alloc_object(rusty_js_runtime::value::Object::new_ordinary());
        rt.object_set(
            out,
            "root".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                parsed.root,
            ))),
        );
        rt.object_set(
            out,
            "dir".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(parsed.dir))),
        );
        rt.object_set(
            out,
            "base".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                parsed.base,
            ))),
        );
        rt.object_set(
            out,
            "ext".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(parsed.ext))),
        );
        rt.object_set(
            out,
            "name".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                parsed.name,
            ))),
        );
        Ok(Value::Object(out))
    });
    register_method(rt, win32, "format", |rt, args| {
        let o = match args.first() {
            Some(Value::Object(id)) => *id,
            Some(v) => return Err(path_invalid_arg_type(rt, "pathObject", "object", Some(v))),
            None => return Err(path_invalid_arg_type(rt, "pathObject", "object", None)),
        };
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(win32_format(rt, o)),
        )))
    });
    register_method(rt, win32, "relative", |rt, args| {
        let from = path_string_arg(rt, args, 0, "from")?;
        let to = path_string_arg(rt, args, 1, "to")?;
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(win32_relative(from, to)),
        )))
    });
}
