
use crate::node_stubs::{async_hooks_emit_destroy_for_global, async_hooks_emit_init_for_global};
use crate::register::{
    make_callable, make_callable_rooted, make_callable_with_length, new_object, register_method,
};
use rusty_js_runtime::promise::{new_promise, reject_promise, resolve_promise};
use rusty_js_runtime::value::{Object, ObjectRef};
use rusty_js_runtime::{AgentId, Runtime, Value};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

const NODE_MAX_TIMER_DELAY_MS: f64 = 2_147_483_647.0;
const PROMISIFY_CUSTOM_KEY: &str = "@@sym:nodejs.util.promisify.custom";

struct TimerEntry {
    agent_id: AgentId,
    id: u64,
    callback: Value,
    args: Vec<Value>,
    due_at: Instant,

    refed: bool,

    repeat_ms: Option<u64>,

    async_context: HashMap<ObjectRef, Value>,

    async_resource: Option<ObjectRef>,
}

thread_local! {
    static TIMERS: RefCell<Vec<TimerEntry>> = RefCell::new(Vec::new());
    static NEXT_TIMER_ID: RefCell<u64> = RefCell::new(1);
}

fn timer_phase_counters_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_TIMER_PHASE_COUNTERS")
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or(false)
    })
}

fn record_timer_phase(phase: &'static str, elapsed_ns: u64) {
    if !timer_phase_counters_enabled() {
        return;
    }
    static SET_CALLS: AtomicU64 = AtomicU64::new(0);
    static CLEAR_CALLS: AtomicU64 = AtomicU64::new(0);
    static NORMALIZE_NS: AtomicU64 = AtomicU64::new(0);
    static EXTRA_ARGS_NS: AtomicU64 = AtomicU64::new(0);
    static REGISTER_NS: AtomicU64 = AtomicU64::new(0);
    static HANDLE_NS: AtomicU64 = AtomicU64::new(0);
    static SET_TOTAL_NS: AtomicU64 = AtomicU64::new(0);
    static ID_NS: AtomicU64 = AtomicU64::new(0);
    static CANCEL_NS: AtomicU64 = AtomicU64::new(0);
    static RELEASE_NS: AtomicU64 = AtomicU64::new(0);
    static CLEAR_TOTAL_NS: AtomicU64 = AtomicU64::new(0);

    let bucket = match phase {
        "normalize" => &NORMALIZE_NS,
        "extra_args" => &EXTRA_ARGS_NS,
        "register" => &REGISTER_NS,
        "handle" => &HANDLE_NS,
        "set_total" => &SET_TOTAL_NS,
        "id" => &ID_NS,
        "cancel" => &CANCEL_NS,
        "release" => &RELEASE_NS,
        "clear_total" => &CLEAR_TOTAL_NS,
        _ => return,
    };
    bucket.fetch_add(elapsed_ns, Ordering::Relaxed);
    match phase {
        "set_total" => {
            let calls = SET_CALLS.fetch_add(1, Ordering::Relaxed) + 1;
            if calls <= 8 || calls.is_power_of_two() {
                let avg = |counter: &AtomicU64| counter.load(Ordering::Relaxed) / calls;
                eprintln!(
                    "[timer-phase] kind=set calls={} avg_normalize_ns={} avg_extra_args_ns={} avg_register_ns={} avg_handle_ns={} avg_total_ns={}",
                    calls,
                    avg(&NORMALIZE_NS),
                    avg(&EXTRA_ARGS_NS),
                    avg(&REGISTER_NS),
                    avg(&HANDLE_NS),
                    avg(&SET_TOTAL_NS)
                );
            }
        }
        "clear_total" => {
            let calls = CLEAR_CALLS.fetch_add(1, Ordering::Relaxed) + 1;
            if calls <= 8 || calls.is_power_of_two() {
                let avg = |counter: &AtomicU64| counter.load(Ordering::Relaxed) / calls;
                eprintln!(
                    "[timer-phase] kind=clear calls={} avg_id_ns={} avg_cancel_ns={} avg_release_ns={} avg_total_ns={}",
                    calls,
                    avg(&ID_NS),
                    avg(&CANCEL_NS),
                    avg(&RELEASE_NS),
                    avg(&CLEAR_TOTAL_NS)
                );
            }
        }
        _ => {}
    }
}

fn next_id() -> u64 {
    NEXT_TIMER_ID.with(|c| {
        let mut c = c.borrow_mut();
        let id = *c;
        *c += 1;
        id
    })
}

fn timer_root_key(id: u64) -> String {
    format!("timer:{id}")
}

fn timeout_proto_root_key() -> &'static str {
    "timer:timeout_proto"
}

fn timer_abort_root_key(id: u64) -> String {
    format!("timer_abort:{id}")
}

fn set_timer_id(rt: &mut Runtime, object: ObjectRef, id: u64) {
    rt.obj_mut(object)
        .set_own("__timer_id".into(), Value::Number(id as f64));
}

fn get_timer_id(rt: &Runtime, object: ObjectRef) -> Option<u64> {
    match rt
        .obj(object)
        .get_own("__timer_id")
        .map(|d| d.value.clone())
    {
        Some(Value::Number(n)) if n.is_finite() && n >= 0.0 => Some(n as u64),
        _ => None,
    }
}

fn register_with_id(
    rt: &mut Runtime,
    id: u64,
    callback: Value,
    args: Vec<Value>,
    delay_ms: u64,
    repeat: bool,
    async_resource: Option<ObjectRef>,
) {
    let due_at = Instant::now() + Duration::from_millis(delay_ms);
    let repeat_ms = if repeat { Some(delay_ms.max(1)) } else { None };
    let async_context = rt.als_context.clone();
    let mut roots = Vec::with_capacity(args.len() + 1);
    roots.push(callback.clone());
    roots.extend(args.iter().cloned());
    roots.extend(async_context.keys().copied().map(Value::Object));
    roots.extend(async_context.values().cloned());
    if let Some(resource) = async_resource {
        roots.push(Value::Object(resource));
    }
    rt.retain_host_roots(timer_root_key(id), roots);
    let agent_id = rt.agent_id();
    TIMERS.with(|t| {
        t.borrow_mut().push(TimerEntry {
            agent_id,
            id,
            callback,
            args,
            due_at,
            refed: true,
            repeat_ms,
            async_context,
            async_resource,
        });
    });
}

fn set_refed(rt: &Runtime, id: u64, refed: bool) {
    let agent_id = rt.agent_id();
    TIMERS.with(|t| {
        if let Some(entry) = t
            .borrow_mut()
            .iter_mut()
            .find(|entry| entry.agent_id == agent_id && entry.id == id)
        {
            entry.refed = refed;
        }
    });
}

fn is_refed(rt: &Runtime, id: u64) -> bool {
    let agent_id = rt.agent_id();
    TIMERS.with(|t| {
        t.borrow()
            .iter()
            .find(|entry| entry.agent_id == agent_id && entry.id == id)
            .map(|entry| entry.refed)
            .unwrap_or(false)
    })
}

fn timer_extra_args(args: &[Value], start: usize) -> Vec<Value> {
    if args.len() <= start {
        Vec::new()
    } else {
        args[start..].to_vec()
    }
}

fn normalize_timer_delay(v: Option<&Value>) -> u64 {
    let Some(Value::Number(n)) = v else {
        return 0;
    };
    let warning = if n.is_nan() {
        Some(format!(
            "TimeoutNaNWarning: {} is not a number.",
            node_delay_label(*n)
        ))
    } else if *n < 0.0 {
        Some(format!(
            "TimeoutNegativeWarning: {} is a negative number.",
            node_delay_label(*n)
        ))
    } else if !n.is_finite() || *n > NODE_MAX_TIMER_DELAY_MS {
        Some(format!(
            "TimeoutOverflowWarning: {} does not fit into a 32-bit signed integer.",
            node_delay_label(*n)
        ))
    } else {
        None
    };
    if let Some(warning) = warning {
        rusty_js_runtime::interp::queue_node_warning(format!(
            "(node:{}) {}\nTimeout duration was set to 1.\n(Use `node --trace-warnings ...` to show where the warning was created)",
            std::process::id(),
            warning
        ));
        return 1;
    }

    (*n as u64).max(1)
}

fn node_delay_label(n: f64) -> String {
    if n.is_nan() {
        "NaN".to_string()
    } else if n == f64::INFINITY {
        "Infinity".to_string()
    } else if n == f64::NEG_INFINITY {
        "-Infinity".to_string()
    } else {
        n.to_string()
    }
}

fn cancel(rt: &Runtime, id: u64) -> bool {
    let agent_id = rt.agent_id();
    TIMERS.with(|t| {
        let mut timers = t.borrow_mut();
        if let Some(pos) = timers
            .iter()
            .position(|e| e.agent_id == agent_id && e.id == id)
        {
            timers.remove(pos);
            true
        } else {
            false
        }
    })
}

pub fn release_roots(rt: &mut Runtime, id: u64) {
    rt.release_host_roots(&timer_root_key(id));
}

pub fn has_pending(rt: &Runtime) -> bool {
    let agent_id = rt.agent_id();
    TIMERS.with(|t| {
        t.borrow()
            .iter()
            .any(|entry| entry.agent_id == agent_id && entry.refed)
    })
}

pub fn next_due_ms(rt: &Runtime) -> Option<u64> {
    let agent_id = rt.agent_id();
    let now = Instant::now();
    TIMERS.with(|t| {
        t.borrow()
            .iter()
            .filter(|entry| entry.agent_id == agent_id && entry.refed)
            .map(|e| {
                if e.due_at <= now {
                    0
                } else {
                    (e.due_at - now).as_millis() as u64
                }
            })
            .min()
    })
}

pub fn drain_due_pairs() -> Vec<(
    u64,
    Value,
    Vec<Value>,
    bool,
    HashMap<ObjectRef, Value>,
    Option<ObjectRef>,
)> {
    drain_due_pairs_for_agent(AgentId::DEFAULT)
}

pub fn drain_due_pairs_for_runtime(
    rt: &Runtime,
) -> Vec<(
    u64,
    Value,
    Vec<Value>,
    bool,
    HashMap<ObjectRef, Value>,
    Option<ObjectRef>,
)> {
    drain_due_pairs_for_agent(rt.agent_id())
}

fn drain_due_pairs_for_agent(
    agent_id: AgentId,
) -> Vec<(
    u64,
    Value,
    Vec<Value>,
    bool,
    HashMap<ObjectRef, Value>,
    Option<ObjectRef>,
)> {
    let now = Instant::now();
    let mut fired: Vec<(
        u64,
        Value,
        Vec<Value>,
        bool,
        HashMap<ObjectRef, Value>,
        Option<ObjectRef>,
    )> = Vec::new();
    TIMERS.with(|t| {
        let mut t = t.borrow_mut();
        let mut keep: Vec<TimerEntry> = Vec::with_capacity(t.len());
        for e in t.drain(..) {
            if e.agent_id == agent_id && e.due_at <= now {
                if let Some(ms) = e.repeat_ms {
                    fired.push((
                        e.id,
                        e.callback.clone(),
                        e.args.clone(),
                        true,
                        e.async_context.clone(),
                        e.async_resource,
                    ));
                    keep.push(TimerEntry {
                        agent_id: e.agent_id,
                        id: e.id,
                        callback: e.callback,
                        args: e.args,
                        due_at: now + Duration::from_millis(ms),
                        refed: e.refed,
                        repeat_ms: e.repeat_ms,
                        async_context: e.async_context,
                        async_resource: e.async_resource,
                    });
                } else {
                    fired.push((
                        e.id,
                        e.callback.clone(),
                        e.args.clone(),
                        false,
                        e.async_context,
                        e.async_resource,
                    ));
                }
            } else {
                keep.push(e);
            }
        }
        *t = keep;
    });
    fired
}

pub fn roots_for_callback(cb: &Value, args: &[Value]) -> Vec<ObjectRef> {
    let mut roots = Vec::new();
    if let Value::Object(id) = cb {
        roots.push(*id);
    }
    for arg in args {
        if let Value::Object(id) = arg {
            roots.push(*id);
        }
    }
    roots
}

pub fn roots_for_callback_with_resource(
    cb: &Value,
    args: &[Value],
    async_resource: Option<ObjectRef>,
) -> Vec<ObjectRef> {
    let mut roots = roots_for_callback(cb, args);
    if let Some(resource) = async_resource {
        roots.push(resource);
    }
    roots
}

pub fn install(rt: &mut Runtime) {
    fn make_timeout_obj(rt: &mut Runtime, id: u64, proto: ObjectRef) -> ObjectRef {
        let o = rt.alloc_object(Object::new_ordinary());
        rt.set_object_prototype_internal(o, Some(proto));
        set_timer_id(rt, o, id);
        o
    }

    let timeout_proto = rt.alloc_object(Object::new_ordinary());
    register_method(rt, timeout_proto, "ref", |rt, _args| {
        if let Value::Object(id) = rt.current_this() {
            if let Some(timer_id) = get_timer_id(rt, id) {
                set_refed(rt, timer_id, true);
            }
        }
        Ok(rt.current_this())
    });
    register_method(rt, timeout_proto, "unref", |rt, _args| {
        if let Value::Object(id) = rt.current_this() {
            if let Some(timer_id) = get_timer_id(rt, id) {
                set_refed(rt, timer_id, false);
            }
        }
        Ok(rt.current_this())
    });
    register_method(rt, timeout_proto, "hasRef", |rt, _args| {
        Ok(Value::Boolean(match rt.current_this() {
            Value::Object(id) => get_timer_id(rt, id)
                .map(|timer_id| is_refed(rt, timer_id))
                .unwrap_or(false),
            _ => false,
        }))
    });
    register_method(rt, timeout_proto, "refresh", |rt, _args| {
        Ok(rt.current_this())
    });
    register_method(rt, timeout_proto, "@@toPrimitive", |rt, _args| {
        Ok(match rt.current_this() {
            Value::Object(id) => get_timer_id(rt, id)
                .map(|id| Value::Number(id as f64))
                .unwrap_or(Value::Number(0.0)),
            _ => Value::Number(0.0),
        })
    });

    let to_prim = rt.object_get(timeout_proto, "@@toPrimitive");
    rt.obj_mut(timeout_proto)
        .set_own_internal("@@toPrimitive".into(), to_prim);
    register_method(rt, timeout_proto, "valueOf", |rt, _args| {
        Ok(match rt.current_this() {
            Value::Object(id) => get_timer_id(rt, id)
                .map(|id| Value::Number(id as f64))
                .unwrap_or(Value::Number(0.0)),
            _ => Value::Number(0.0),
        })
    });

    register_method(rt, timeout_proto, "close", |rt, _args| {
        if let Value::Object(id) = rt.current_this() {
            if let Some(timer_id) = get_timer_id(rt, id) {
                if cancel(rt, timer_id) {
                    release_roots(rt, timer_id);
                }
            }
        }
        Ok(rt.current_this())
    });

    let timeout_ctor = make_callable(rt, "Timeout", |rt, _a| Ok(rt.current_this()));
    rt.object_set(
        timeout_ctor,
        "prototype".into(),
        Value::Object(timeout_proto),
    );
    rt.obj_mut(timeout_proto)
        .set_own_internal("constructor".into(), Value::Object(timeout_ctor));
    let immediate_proto = rt.alloc_object(Object::new_ordinary());
    rt.set_object_prototype_internal(immediate_proto, Some(timeout_proto));
    let immediate_ctor = make_callable(rt, "Immediate", |rt, _a| Ok(rt.current_this()));
    rt.object_set(
        immediate_ctor,
        "prototype".into(),
        Value::Object(immediate_proto),
    );
    rt.obj_mut(immediate_proto)
        .set_own_internal("constructor".into(), Value::Object(immediate_ctor));
    rt.retain_host_roots(
        timeout_proto_root_key(),
        vec![Value::Object(timeout_proto), Value::Object(immediate_proto)],
    );

    let timeout_proto_for_set_timeout = timeout_proto;
    let set_timeout = make_callable_with_length(rt, "setTimeout", 2, move |rt, args| {
        let total_t0 = timer_phase_counters_enabled().then(Instant::now);
        let cb = args.first().cloned().unwrap_or(Value::Undefined);
        let normalize_t0 = timer_phase_counters_enabled().then(Instant::now);
        let delay = normalize_timer_delay(args.get(1));
        rt.require_timer_cap(delay)?;
        if let Some(t0) = normalize_t0 {
            record_timer_phase("normalize", t0.elapsed().as_nanos() as u64);
        }
        let extra_args_t0 = timer_phase_counters_enabled().then(Instant::now);
        let cb_args = timer_extra_args(args, 2);
        if let Some(t0) = extra_args_t0 {
            record_timer_phase("extra_args", t0.elapsed().as_nanos() as u64);
        }
        let id = next_id();
        let handle_t0 = timer_phase_counters_enabled().then(Instant::now);
        let handle = make_timeout_obj(rt, id, timeout_proto_for_set_timeout);
        async_hooks_emit_init_for_global(rt, "Timeout", Value::Object(handle))?;
        let register_t0 = timer_phase_counters_enabled().then(Instant::now);
        register_with_id(rt, id, cb, cb_args, delay, false, Some(handle));
        if let Some(t0) = register_t0 {
            record_timer_phase("register", t0.elapsed().as_nanos() as u64);
        }
        if let Some(t0) = handle_t0 {
            record_timer_phase("handle", t0.elapsed().as_nanos() as u64);
        }
        if let Some(t0) = total_t0 {
            record_timer_phase("set_total", t0.elapsed().as_nanos() as u64);
        }
        Ok(Value::Object(handle))
    });
    let set_timeout_promisified = make_callable(rt, "setTimeout", |rt, args| {
        let delay = delay_arg(args, 0);
        let value = args.get(1).cloned().unwrap_or(Value::Undefined);
        Ok(promise_timer_with_abort(rt, delay, value, args.get(2)))
    });
    rt.object_set(
        set_timeout,
        PROMISIFY_CUSTOM_KEY.into(),
        Value::Object(set_timeout_promisified),
    );
    rt.define_global_property("setTimeout", Value::Object(set_timeout));

    let timeout_proto_for_set_interval = timeout_proto;
    let set_interval = make_callable_with_length(rt, "setInterval", 2, move |rt, args| {
        let cb = args.first().cloned().unwrap_or(Value::Undefined);
        let delay = normalize_timer_delay(args.get(1));
        rt.require_timer_cap(delay)?;
        let cb_args = timer_extra_args(args, 2);
        let id = next_id();
        let handle = make_timeout_obj(rt, id, timeout_proto_for_set_interval);
        async_hooks_emit_init_for_global(rt, "Timeout", Value::Object(handle))?;
        register_with_id(rt, id, cb, cb_args, delay, true, Some(handle));
        Ok(Value::Object(handle))
    });
    rt.define_global_property("setInterval", Value::Object(set_interval));

    let clear_t = make_callable_with_length(rt, "clearTimeout", 1, |rt, args| {
        let total_t0 = timer_phase_counters_enabled().then(Instant::now);
        let id_t0 = timer_phase_counters_enabled().then(Instant::now);
        let id = timer_id_from(rt, args.first().cloned().unwrap_or(Value::Undefined));
        if let Some(t0) = id_t0 {
            record_timer_phase("id", t0.elapsed().as_nanos() as u64);
        }
        if let Some(id) = id {
            let cancel_t0 = timer_phase_counters_enabled().then(Instant::now);
            let cancelled = cancel(rt, id);
            if let Some(t0) = cancel_t0 {
                record_timer_phase("cancel", t0.elapsed().as_nanos() as u64);
            }
            if cancelled {
                let release_t0 = timer_phase_counters_enabled().then(Instant::now);
                release_roots(rt, id);
                if let Some(t0) = release_t0 {
                    record_timer_phase("release", t0.elapsed().as_nanos() as u64);
                }
            }
        }
        if let Some(t0) = total_t0 {
            record_timer_phase("clear_total", t0.elapsed().as_nanos() as u64);
        }
        Ok(Value::Undefined)
    });
    rt.define_global_property("clearTimeout", Value::Object(clear_t));
    let clear_i = make_callable_with_length(rt, "clearInterval", 1, |rt, args| {
        let id = timer_id_from(rt, args.first().cloned().unwrap_or(Value::Undefined));
        if let Some(id) = id {
            if cancel(rt, id) {
                release_roots(rt, id);
            }
        }
        Ok(Value::Undefined)
    });
    rt.define_global_property("clearInterval", Value::Object(clear_i));

    let qmt = make_callable_with_length(rt, "queueMicrotask", 1, |rt, args| {
        rt.require_microtask_cap()?;
        let cb = args.first().cloned().unwrap_or(Value::Undefined);
        let roots = roots_for_callback(&cb, &[]);
        rt.enqueue_queue_microtask_callback(cb, roots);
        Ok(Value::Undefined)
    });
    rt.define_global_property("queueMicrotask", Value::Object(qmt));

    let timeout_proto_for_set_immediate = immediate_proto;
    let set_immediate = make_callable_with_length(rt, "setImmediate", 1, move |rt, args| {
        rt.require_timer_cap(0)?;
        let cb = args.first().cloned().unwrap_or(Value::Undefined);
        let cb_args = timer_extra_args(args, 1);
        let id = next_id();
        let handle = make_timeout_obj(rt, id, timeout_proto_for_set_immediate);
        async_hooks_emit_init_for_global(rt, "Immediate", Value::Object(handle))?;
        register_with_id(rt, id, cb, cb_args, 0, false, Some(handle));
        Ok(Value::Object(handle))
    });
    let set_immediate_promisified = make_callable(rt, "setImmediate", |rt, args| {
        let value = args.first().cloned().unwrap_or(Value::Undefined);
        Ok(promise_timer_with_abort(rt, 0, value, args.get(1)))
    });
    rt.object_set(
        set_immediate,
        PROMISIFY_CUSTOM_KEY.into(),
        Value::Object(set_immediate_promisified),
    );
    rt.define_global_property("setImmediate", Value::Object(set_immediate));
    let clear_im = make_callable_with_length(rt, "clearImmediate", 1, |rt, args| {
        let handle = args.first().cloned().unwrap_or(Value::Undefined);
        let id = timer_id_from(rt, handle.clone());
        if let Some(id) = id {
            if cancel(rt, id) {
                async_hooks_emit_destroy_for_global(rt, handle)?;
                release_roots(rt, id);
            }
        }
        Ok(Value::Undefined)
    });
    rt.define_global_property("clearImmediate", Value::Object(clear_im));

    install_node_timer_namespaces(rt);
}

fn timer_id_from(rt: &Runtime, v: Value) -> Option<u64> {
    match v {
        Value::Number(n) => Some(n as u64),
        Value::Object(id) => get_timer_id(rt, id),
        _ => None,
    }
}

fn delay_arg(args: &[Value], idx: usize) -> u64 {
    args.get(idx)
        .and_then(|v| match v {
            Value::Number(n) if n.is_finite() && *n > 0.0 => Some(*n as u64),
            _ => None,
        })
        .unwrap_or(0)
}

fn js_string(s: &str) -> Value {
    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
        s.to_string(),
    )))
}

fn abort_error(rt: &mut Runtime) -> Value {
    let ctor = rt.global_get("Error");
    let msg = js_string("The operation was aborted");
    match rt.construct(ctor, vec![msg.clone()]) {
        Ok(Value::Object(e)) => {
            rt.object_set(e, "name".into(), js_string("AbortError"));
            rt.object_set(e, "code".into(), js_string("ABORT_ERR"));
            Value::Object(e)
        }
        _ => msg,
    }
}

fn rejected_abort_promise(rt: &mut Runtime) -> Value {
    let p = new_promise(rt);
    let err = abort_error(rt);
    reject_promise(rt, p, err);
    Value::Object(p)
}

fn signal_from_opts(rt: &mut Runtime, opts: Option<&Value>) -> Option<ObjectRef> {
    let opts = match opts {
        Some(Value::Object(o)) => *o,
        _ => return None,
    };
    match rt.object_get(opts, "signal") {
        Value::Object(s) => Some(s),
        _ => None,
    }
}

fn aborted_rejection(rt: &mut Runtime, opts: Option<&Value>) -> Option<Value> {
    let sig = signal_from_opts(rt, opts)?;
    if !matches!(rt.object_get(sig, "aborted"), Value::Boolean(true)) {
        return None;
    }
    Some(rejected_abort_promise(rt))
}

fn promise_timer(rt: &mut Runtime, delay: u64, value: Value) -> Value {
    let promise = new_promise(rt);
    let resolver = make_callable_rooted(
        rt,
        "__timers_promises_resolve",
        vec![promise],
        move |rt, _args| {
            resolve_promise(rt, promise, value.clone());
            Ok(Value::Undefined)
        },
    );
    register_promise_timer(rt, resolver, delay);
    Value::Object(promise)
}

fn register_promise_timer(rt: &mut Runtime, resolver: ObjectRef, delay: u64) -> u64 {
    let id = next_id();
    let resource = new_object(rt);
    async_hooks_emit_init_for_global(rt, "Timeout", Value::Object(resource)).ok();
    register_with_id(
        rt,
        id,
        Value::Object(resolver),
        Vec::new(),
        delay,
        false,
        Some(resource),
    );
    id
}

fn remove_abort_listener(rt: &mut Runtime, sig: ObjectRef, listener: ObjectRef) {
    let remove = rt.object_get(sig, "removeEventListener");
    if rt.is_callable(&remove) {
        let _ = rt.call_function(
            remove,
            Value::Object(sig),
            vec![js_string("abort"), Value::Object(listener)],
        );
    }
}

fn promise_timer_with_abort(
    rt: &mut Runtime,
    delay: u64,
    value: Value,
    opts: Option<&Value>,
) -> Value {
    let Some(sig) = signal_from_opts(rt, opts) else {
        return promise_timer(rt, delay, value);
    };
    if matches!(rt.object_get(sig, "aborted"), Value::Boolean(true)) {
        let p = new_promise(rt);
        let err = abort_error(rt);
        reject_promise(rt, p, err);
        return Value::Object(p);
    }

    let promise = new_promise(rt);
    let done = Rc::new(Cell::new(false));
    let timer_id = Rc::new(Cell::new(0_u64));
    let listener_ref = Rc::new(Cell::new(None::<ObjectRef>));

    let resolve_done = done.clone();
    let resolve_timer_id = timer_id.clone();
    let resolve_listener_ref = listener_ref.clone();
    let resolver = make_callable_rooted(
        rt,
        "__timers_promises_resolve",
        vec![promise, sig],
        move |rt, _args| {
            if resolve_done.replace(true) {
                return Ok(Value::Undefined);
            }
            if let Some(listener) = resolve_listener_ref.get() {
                remove_abort_listener(rt, sig, listener);
            }
            let id = resolve_timer_id.get();
            rt.release_host_roots(&timer_abort_root_key(id));
            resolve_promise(rt, promise, value.clone());
            Ok(Value::Undefined)
        },
    );

    let abort_done = done.clone();
    let abort_timer_id = timer_id.clone();
    let listener = make_callable(rt, "__timers_promises_abort", move |rt, _args| {
        if abort_done.replace(true) {
            return Ok(Value::Undefined);
        }
        let id = abort_timer_id.get();
        cancel(rt, id);
        release_roots(rt, id);
        rt.release_host_roots(&timer_abort_root_key(id));
        let err = abort_error(rt);
        reject_promise(rt, promise, err);
        Ok(Value::Undefined)
    });
    listener_ref.set(Some(listener));

    let id = register_promise_timer(rt, resolver, delay);
    timer_id.set(id);
    rt.retain_host_roots(
        timer_abort_root_key(id),
        vec![
            Value::Object(sig),
            Value::Object(listener),
            Value::Object(promise),
        ],
    );

    let add = rt.object_get(sig, "addEventListener");
    if rt.is_callable(&add) {
        let _ = rt.call_function(
            add,
            Value::Object(sig),
            vec![js_string("abort"), Value::Object(listener)],
        );
    }

    Value::Object(promise)
}

fn make_iterator_result(rt: &mut Runtime, value: Value, done: bool) -> Value {
    let result = rt.alloc_object(Object::new_ordinary());
    rt.object_set(result, "value".into(), value);
    rt.object_set(result, "done".into(), Value::Boolean(done));
    Value::Object(result)
}

fn install_node_timer_namespaces(rt: &mut Runtime) {
    let timers = new_object(rt);
    let set_timeout = rt.global_get("setTimeout");
    let clear_timeout = rt.global_get("clearTimeout");
    let set_interval = rt.global_get("setInterval");
    let clear_interval = rt.global_get("clearInterval");
    let set_immediate = rt.global_get("setImmediate");
    let clear_immediate = rt.global_get("clearImmediate");
    rt.object_set(timers, "setTimeout".into(), set_timeout);
    rt.object_set(timers, "clearTimeout".into(), clear_timeout);
    rt.object_set(timers, "setInterval".into(), set_interval);
    rt.object_set(timers, "clearInterval".into(), clear_interval);
    rt.object_set(timers, "setImmediate".into(), set_immediate);
    rt.object_set(timers, "clearImmediate".into(), clear_immediate);
    rt.define_global_property("timers", Value::Object(timers));

    let promises = new_object(rt);
    register_method(rt, promises, "setTimeout", |rt, args| {
        if let Some(p) = aborted_rejection(rt, args.get(2)) {
            return Ok(p);
        }
        let delay = delay_arg(args, 0);
        let value = args.get(1).cloned().unwrap_or(Value::Undefined);
        Ok(promise_timer_with_abort(rt, delay, value, args.get(2)))
    });
    register_method(rt, promises, "setImmediate", |rt, args| {
        if let Some(p) = aborted_rejection(rt, args.get(1)) {
            return Ok(p);
        }
        let value = args.first().cloned().unwrap_or(Value::Undefined);
        Ok(promise_timer_with_abort(rt, 0, value, args.get(1)))
    });
    register_method(rt, promises, "setInterval", |rt, args| {
        let delay = delay_arg(args, 0).max(1);
        let value = args.get(1).cloned().unwrap_or(Value::Undefined);
        let opts = args.get(2).cloned();
        let iter = rt.alloc_object(Object::new_ordinary());
        let next_delay = delay;
        let next_value = value.clone();
        register_method(rt, iter, "next", move |rt, _args| {
            let result = make_iterator_result(rt, next_value.clone(), false);
            Ok(promise_timer_with_abort(
                rt,
                next_delay,
                result,
                opts.as_ref(),
            ))
        });
        register_method(rt, iter, "return", |rt, _args| {
            let promise = new_promise(rt);
            let result = make_iterator_result(rt, Value::Undefined, true);
            resolve_promise(rt, promise, result);
            Ok(Value::Object(promise))
        });
        register_method(rt, iter, "@@asyncIterator", |rt, _args| {
            Ok(rt.current_this())
        });
        Ok(Value::Object(iter))
    });
    let scheduler = new_object(rt);
    register_method(rt, scheduler, "wait", |rt, args| {
        if let Some(p) = aborted_rejection(rt, args.get(1)) {
            return Ok(p);
        }
        let delay = delay_arg(args, 0);
        Ok(promise_timer_with_abort(
            rt,
            delay,
            Value::Undefined,
            args.get(1),
        ))
    });
    register_method(rt, scheduler, "yield", |rt, _args| {
        Ok(promise_timer(rt, 0, Value::Undefined))
    });
    rt.object_set(promises, "scheduler".into(), Value::Object(scheduler));
    rt.object_set(timers, "promises".into(), Value::Object(promises));
    rt.define_global_property("timers_promises", Value::Object(promises));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_timer_callback_is_runtime_rooted() {
        let mut rt = Runtime::new();
        let callback = rt.alloc_object(Object::new_ordinary());
        let arg = rt.alloc_object(Object::new_ordinary());
        let id = next_id();
        register_with_id(
            &mut rt,
            id,
            Value::Object(callback),
            vec![Value::Object(arg)],
            60_000,
            false,
            None,
        );

        let roots = rt.enumerate_roots();
        assert!(
            roots.contains(&callback),
            "pending timer callback must be visible to runtime roots"
        );
        assert!(
            roots.contains(&arg),
            "pending timer callback arguments must be visible to runtime roots"
        );

        cancel(&rt, id);
        release_roots(&mut rt, id);
    }

    #[test]
    fn timer_registry_is_scoped_by_runtime_agent_id() {
        let mut rt_a = Runtime::new_with_agent_id(AgentId::from_raw(101));
        let mut rt_b = Runtime::new_with_agent_id(AgentId::from_raw(202));
        let cb_a = rt_a.alloc_object(Object::new_ordinary());
        let cb_b = rt_b.alloc_object(Object::new_ordinary());
        let id_a = next_id();
        let id_b = next_id();

        register_with_id(
            &mut rt_a,
            id_a,
            Value::Object(cb_a),
            Vec::new(),
            0,
            false,
            None,
        );
        register_with_id(
            &mut rt_b,
            id_b,
            Value::Object(cb_b),
            Vec::new(),
            0,
            false,
            None,
        );

        assert!(has_pending(&rt_a));
        assert!(has_pending(&rt_b));

        let fired_a = drain_due_pairs_for_runtime(&rt_a);
        assert_eq!(fired_a.len(), 1);
        assert_eq!(fired_a[0].0, id_a);
        assert!(!has_pending(&rt_a));
        assert!(
            has_pending(&rt_b),
            "draining agent A must not harvest agent B timers"
        );

        let fired_b = drain_due_pairs_for_runtime(&rt_b);
        assert_eq!(fired_b.len(), 1);
        assert_eq!(fired_b[0].0, id_b);
        assert!(!has_pending(&rt_b));

        release_roots(&mut rt_a, id_a);
        release_roots(&mut rt_b, id_b);
    }
}
