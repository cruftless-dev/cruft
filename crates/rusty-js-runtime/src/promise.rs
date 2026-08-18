
use crate::interp::{Runtime, RuntimeError};
use crate::value::{
    CapturedBinding, FunctionInternals, InternalKind, NativeFn, Object, ObjectRef, PromiseReaction,
    PromiseReactionHandler, PromiseState, PromiseStatus, Value,
};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

static PROMISE_RESOLVE_CALLS: AtomicU64 = AtomicU64::new(0);
static PROMISE_RESOLVE_TOTAL_NS: AtomicU64 = AtomicU64::new(0);
static PROMISE_RESOLVE_REACTIONS: AtomicU64 = AtomicU64::new(0);
static PROMISE_RESOLVE_OBJECT_VALUES: AtomicU64 = AtomicU64::new(0);
static PROMISE_RESOLVE_PRIMITIVE_VALUES: AtomicU64 = AtomicU64::new(0);
static PROMISE_RESOLVE_NATIVE_ADOPTS: AtomicU64 = AtomicU64::new(0);
static PROMISE_RESOLVE_THENABLE_JOBS: AtomicU64 = AtomicU64::new(0);
static PROMISE_ALLOC_CALLS: AtomicU64 = AtomicU64::new(0);
static PROMISE_RESOLVING_FUNCTION_PAIRS: AtomicU64 = AtomicU64::new(0);
static PROMISE_RESOLVING_FUNCTION_OBJECTS: AtomicU64 = AtomicU64::new(0);
static PROMISE_REACTION_RESULT_CALLS: AtomicU64 = AtomicU64::new(0);
static PROMISE_REACTION_RESULT_HANDLER_NS: AtomicU64 = AtomicU64::new(0);
static PROMISE_REACTION_RESULT_CAPABILITY_NS: AtomicU64 = AtomicU64::new(0);
static PROMISE_REACTION_RESULT_CAPABILITY_CALLS: AtomicU64 = AtomicU64::new(0);
static PROMISE_REACTION_RESULT_REJECTED: AtomicU64 = AtomicU64::new(0);
static PROMISE_REACTION_ENQUEUE_CALLS: AtomicU64 = AtomicU64::new(0);
static PROMISE_REACTION_ENQUEUE_ROOT_NS: AtomicU64 = AtomicU64::new(0);
static PROMISE_REACTION_ENQUEUE_TOTAL_NS: AtomicU64 = AtomicU64::new(0);
static PROMISE_REACTION_ENQUEUE_ROOTS: AtomicU64 = AtomicU64::new(0);
static PROMISE_REACTION_TOTAL_CALLS: AtomicU64 = AtomicU64::new(0);
static PROMISE_REACTION_TOTAL_NS: AtomicU64 = AtomicU64::new(0);
static PROMISE_REACTION_TOTAL_ERRORS: AtomicU64 = AtomicU64::new(0);
static PROMISE_REACTION_TAIL_DROP_CALLS: AtomicU64 = AtomicU64::new(0);
static PROMISE_REACTION_TAIL_DROP_NS: AtomicU64 = AtomicU64::new(0);
static PROMISE_REACTION_DROP_SPLIT_CALLS: AtomicU64 = AtomicU64::new(0);
static PROMISE_REACTION_DROP_SPLIT_HANDLER_NS: AtomicU64 = AtomicU64::new(0);
static PROMISE_REACTION_DROP_SPLIT_CAP_RESOLVE_NS: AtomicU64 = AtomicU64::new(0);
static PROMISE_REACTION_DROP_SPLIT_CAP_REJECT_NS: AtomicU64 = AtomicU64::new(0);
static PROMISE_REACTION_DROP_SPLIT_SHAPE_NS: AtomicU64 = AtomicU64::new(0);
static PROMISE_ONE_CELL_DROP_FIELD_CALLS: AtomicU64 = AtomicU64::new(0);
static PROMISE_ONE_CELL_DROP_FIELD_PROTO_NS: AtomicU64 = AtomicU64::new(0);
static PROMISE_ONE_CELL_DROP_FIELD_UPVALUE_NS: AtomicU64 = AtomicU64::new(0);
static PROMISE_ONE_CELL_DROP_FIELD_GLOBAL_NS: AtomicU64 = AtomicU64::new(0);
static PROMISE_ONE_CELL_DROP_FIELD_THIS_NS: AtomicU64 = AtomicU64::new(0);
static PROMISE_ONE_CELL_DROP_FIELD_EXEC_FN_NS: AtomicU64 = AtomicU64::new(0);
static PROMISE_ONE_CELL_DROP_FIELD_BOOL_NS: AtomicU64 = AtomicU64::new(0);
static PROMISE_LAZY_PAYLOAD_SHAPE_CALLS: AtomicU64 = AtomicU64::new(0);
static PROMISE_LAZY_PAYLOAD_SHAPE_UPVALUES: AtomicU64 = AtomicU64::new(0);
static PROMISE_LAZY_PAYLOAD_SHAPE_BINDINGS: AtomicU64 = AtomicU64::new(0);
static PROMISE_LAZY_PAYLOAD_SHAPE_WITH_ENVS: AtomicU64 = AtomicU64::new(0);
static PROMISE_LAZY_PAYLOAD_SHAPE_IMPORT_META: AtomicU64 = AtomicU64::new(0);
static PROMISE_LAZY_PAYLOAD_SHAPE_BOUND_THIS: AtomicU64 = AtomicU64::new(0);
static PROMISE_LAZY_PAYLOAD_SHAPE_BOUND_THIS_CELL: AtomicU64 = AtomicU64::new(0);
static PROMISE_LAZY_PAYLOAD_SHAPE_DERIVED_THIS: AtomicU64 = AtomicU64::new(0);
static PROMISE_LAZY_PAYLOAD_SHAPE_EXEC_FN: AtomicU64 = AtomicU64::new(0);
static PROMISE_LAZY_PAYLOAD_SHAPE_NEW_TARGET: AtomicU64 = AtomicU64::new(0);
static PROMISE_LAZY_PAYLOAD_SHAPE_NEW_TARGET_ALLOWED: AtomicU64 = AtomicU64::new(0);
static PROMISE_LAZY_PAYLOAD_SHAPE_COMPACT1: AtomicU64 = AtomicU64::new(0);
static PROMISE_LAZY_PAYLOAD_SHAPE_BINDING_CELL: AtomicU64 = AtomicU64::new(0);
static PROMISE_LAZY_PAYLOAD_SHAPE_BINDING_GLOBAL: AtomicU64 = AtomicU64::new(0);
static PROMISE_LAZY_PAYLOAD_SHAPE_BINDING_SCRIPT_GLOBAL: AtomicU64 = AtomicU64::new(0);
static PROMISE_LAZY_PAYLOAD_SHAPE_BINDING_EVAL_SHADOW: AtomicU64 = AtomicU64::new(0);
static PROMISE_LAZY_PAYLOAD_SHAPE_BINDING_IMMUTABLE_SELF: AtomicU64 = AtomicU64::new(0);
static PROMISE_ASYNC_AWAIT_CONTINUATION_CALLS: AtomicU64 = AtomicU64::new(0);
static PROMISE_ASYNC_AWAIT_CONTINUATION_CLONE_NS: AtomicU64 = AtomicU64::new(0);
static PROMISE_ASYNC_AWAIT_CONTINUATION_RESUME_NS: AtomicU64 = AtomicU64::new(0);
static PROMISE_ASYNC_AWAIT_CONTINUATION_SETTLE_NS: AtomicU64 = AtomicU64::new(0);

fn promise_resolve_counters_enabled() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("CRUFT_PROMISE_RESOLVE_COUNTERS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn promise_alloc_counters_enabled() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("CRUFT_PROMISE_ALLOC_COUNTERS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn record_promise_alloc(kind: &'static str) {
    if !promise_alloc_counters_enabled() {
        return;
    }
    let n = match kind {
        "promise" => PROMISE_ALLOC_CALLS.fetch_add(1, Ordering::Relaxed) + 1,
        "resolving-pair" => PROMISE_RESOLVING_FUNCTION_PAIRS.fetch_add(1, Ordering::Relaxed) + 1,
        "resolving-function" => {
            PROMISE_RESOLVING_FUNCTION_OBJECTS.fetch_add(1, Ordering::Relaxed) + 1
        }
        _ => return,
    };
    if n <= 16 || n % 1024 == 0 {
        eprintln!(
            "[promise-alloc-counters] promise={} resolving_pairs={} resolving_functions={} last={kind}",
            PROMISE_ALLOC_CALLS.load(Ordering::Relaxed),
            PROMISE_RESOLVING_FUNCTION_PAIRS.load(Ordering::Relaxed),
            PROMISE_RESOLVING_FUNCTION_OBJECTS.load(Ordering::Relaxed)
        );
    }
}

fn record_promise_resolve(
    start: Option<std::time::Instant>,
    value_is_object: bool,
    reaction_count: usize,
    native_adopt: bool,
    thenable_job: bool,
) {
    let Some(start) = start else {
        return;
    };
    let n = PROMISE_RESOLVE_CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    let elapsed = start.elapsed().as_nanos() as u64;
    let total_ns = PROMISE_RESOLVE_TOTAL_NS.fetch_add(elapsed, Ordering::Relaxed) + elapsed;
    let total_reactions = PROMISE_RESOLVE_REACTIONS
        .fetch_add(reaction_count as u64, Ordering::Relaxed)
        + reaction_count as u64;
    if value_is_object {
        PROMISE_RESOLVE_OBJECT_VALUES.fetch_add(1, Ordering::Relaxed);
    } else {
        PROMISE_RESOLVE_PRIMITIVE_VALUES.fetch_add(1, Ordering::Relaxed);
    }
    let native_adopts = if native_adopt {
        PROMISE_RESOLVE_NATIVE_ADOPTS.fetch_add(1, Ordering::Relaxed) + 1
    } else {
        PROMISE_RESOLVE_NATIVE_ADOPTS.load(Ordering::Relaxed)
    };
    let thenable_jobs = if thenable_job {
        PROMISE_RESOLVE_THENABLE_JOBS.fetch_add(1, Ordering::Relaxed) + 1
    } else {
        PROMISE_RESOLVE_THENABLE_JOBS.load(Ordering::Relaxed)
    };
    if n <= 16 || n % 1024 == 0 {
        eprintln!(
            "[promise-resolve-counters] calls={n} avg_ns={} avg_reactions={} native_adopts={native_adopts} thenable_jobs={thenable_jobs}",
            total_ns / n,
            total_reactions / n
        );
    }
}

fn promise_reaction_result_counters_enabled() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("CRUFT_PROMISE_REACTION_RESULT_COUNTERS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn record_promise_reaction_result(
    handler_kind: &'static str,
    handler_ns: u64,
    capability_start: Option<std::time::Instant>,
    capability_function_call: bool,
    rejected: bool,
) {
    let Some(capability_start) = capability_start else {
        return;
    };
    let n = PROMISE_REACTION_RESULT_CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    let cap_ns = capability_start.elapsed().as_nanos() as u64;
    let total_handler =
        PROMISE_REACTION_RESULT_HANDLER_NS.fetch_add(handler_ns, Ordering::Relaxed) + handler_ns;
    let total_cap =
        PROMISE_REACTION_RESULT_CAPABILITY_NS.fetch_add(cap_ns, Ordering::Relaxed) + cap_ns;
    let cap_calls = if capability_function_call {
        PROMISE_REACTION_RESULT_CAPABILITY_CALLS.fetch_add(1, Ordering::Relaxed) + 1
    } else {
        PROMISE_REACTION_RESULT_CAPABILITY_CALLS.load(Ordering::Relaxed)
    };
    let rejected_count = if rejected {
        PROMISE_REACTION_RESULT_REJECTED.fetch_add(1, Ordering::Relaxed) + 1
    } else {
        PROMISE_REACTION_RESULT_REJECTED.load(Ordering::Relaxed)
    };
    if n <= 16 || n % 1024 == 0 {
        eprintln!(
            "[promise-reaction-result-counters] calls={n} handler={handler_kind} avg_handler_ns={} avg_capability_ns={} capability_function_calls={cap_calls} rejected={rejected_count}",
            total_handler / n,
            total_cap / n
        );
    }
}

fn promise_reaction_enqueue_counters_enabled() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("CRUFT_PROMISE_REACTION_ENQUEUE_COUNTERS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn record_promise_reaction_enqueue(
    handler_kind: &'static str,
    root_ns: u64,
    total_start: Option<std::time::Instant>,
    root_count: usize,
) {
    let Some(total_start) = total_start else {
        return;
    };
    let n = PROMISE_REACTION_ENQUEUE_CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    let total_ns = total_start.elapsed().as_nanos() as u64;
    let root_total =
        PROMISE_REACTION_ENQUEUE_ROOT_NS.fetch_add(root_ns, Ordering::Relaxed) + root_ns;
    let total_total =
        PROMISE_REACTION_ENQUEUE_TOTAL_NS.fetch_add(total_ns, Ordering::Relaxed) + total_ns;
    let roots_total = PROMISE_REACTION_ENQUEUE_ROOTS
        .fetch_add(root_count as u64, Ordering::Relaxed)
        + root_count as u64;
    if n <= 16 || n % 1024 == 0 {
        eprintln!(
            "[promise-reaction-enqueue-counters] calls={n} handler={handler_kind} avg_root_ns={} avg_total_ns={} avg_roots={}",
            root_total / n,
            total_total / n,
            roots_total / n
        );
    }
}

fn promise_reaction_total_counters_enabled() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("CRUFT_PROMISE_REACTION_TOTAL_COUNTERS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn record_promise_reaction_total(
    handler_kind: &'static str,
    start: Option<std::time::Instant>,
    is_err: bool,
) {
    let Some(start) = start else {
        return;
    };
    let n = PROMISE_REACTION_TOTAL_CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    let elapsed = start.elapsed().as_nanos() as u64;
    let total = PROMISE_REACTION_TOTAL_NS.fetch_add(elapsed, Ordering::Relaxed) + elapsed;
    let errors = if is_err {
        PROMISE_REACTION_TOTAL_ERRORS.fetch_add(1, Ordering::Relaxed) + 1
    } else {
        PROMISE_REACTION_TOTAL_ERRORS.load(Ordering::Relaxed)
    };
    if n <= 16 || n % 1024 == 0 {
        eprintln!(
            "[promise-reaction-total-counters] calls={n} handler={handler_kind} avg_total_ns={} errors={errors}",
            total / n
        );
    }
}

fn async_hooks_namespace(rt: &Runtime) -> Option<ObjectRef> {
    match rt.global_get("async_hooks") {
        Value::Object(id) => Some(id),
        _ => None,
    }
}

fn async_hooks_enabled_count(rt: &Runtime, ns: ObjectRef) -> f64 {
    match rt.object_get(ns, "__enabled_hook_count__") {
        Value::Number(n) if n.is_finite() && n > 0.0 => n,
        _ => 0.0,
    }
}

fn async_hooks_next_id(rt: &mut Runtime, ns: ObjectRef) -> f64 {
    let next = match rt.object_get(ns, "__next_async_id__") {
        Value::Number(n) if n.is_finite() && n >= 1.0 => n,
        _ => 2.0,
    };
    rt.object_set(ns, "__next_async_id__".into(), Value::Number(next + 1.0));
    next
}

fn async_hooks_current_trigger(rt: &Runtime, ns: ObjectRef) -> Value {
    match rt.object_get(ns, "__execution_async_id__") {
        Value::Number(n) if n.is_finite() && n > 0.0 => Value::Number(n),
        _ => Value::Number(1.0),
    }
}

fn promise_async_id(rt: &Runtime, promise: ObjectRef) -> Option<f64> {
    match rt.object_get(promise, "__async_id__") {
        Value::Number(n) if n.is_finite() && n >= 1.0 => Some(n),
        _ => None,
    }
}

fn promise_trigger_async_id(rt: &Runtime, promise: ObjectRef) -> Value {
    match rt.object_get(promise, "__trigger_async_id__") {
        Value::Number(n) if n.is_finite() && n >= 0.0 => Value::Number(n),
        _ => Value::Number(0.0),
    }
}

fn async_hooks_emit_promise_init(rt: &mut Runtime, promise: ObjectRef, trigger: Option<Value>) {
    let Some(ns) = async_hooks_namespace(rt) else {
        return;
    };
    let async_id = async_hooks_next_id(rt, ns);
    let trigger = trigger.unwrap_or_else(|| async_hooks_current_trigger(rt, ns));

    rt.set_engine_sentinel(promise, "__async_id__", Value::Number(async_id));
    rt.set_engine_sentinel(promise, "__trigger_async_id__", trigger.clone());
    if async_hooks_enabled_count(rt, ns) <= 0.0 {
        return;
    }
    let hooks = match rt.object_get(ns, "__async_hooks_hooks__") {
        Value::Object(id) => id,
        _ => return,
    };
    let len = match rt.object_get(hooks, "length") {
        Value::Number(n) if n.is_finite() && n >= 0.0 => n as usize,
        _ => 0,
    };
    for idx in 0..len {
        let hook = match rt.object_get(hooks, idx.to_string().as_str()) {
            Value::Object(id) => id,
            _ => continue,
        };
        if !matches!(
            rt.object_get(hook, "__async_hooks_enabled__"),
            Value::Boolean(true)
        ) {
            continue;
        }
        let cb = rt.object_get(hook, "__async_hook_init__");
        if !rt.is_callable(&cb) {
            continue;
        }
        if let Err(error) = rt.call_function(
            cb,
            Value::Undefined,
            vec![
                Value::Number(async_id),
                Value::String(Rc::new(crate::value::JsString::from("PROMISE"))),
                trigger.clone(),
                Value::Object(promise),
            ],
        ) {
            rt.record_async_hook_fatal_exception(error);
            return;
        }
    }
}

fn async_hooks_emit_promise_resolve(rt: &mut Runtime, promise: ObjectRef) {
    let Some(ns) = async_hooks_namespace(rt) else {
        return;
    };
    if async_hooks_enabled_count(rt, ns) <= 0.0 {
        return;
    }
    let Some(async_id) = promise_async_id(rt, promise) else {
        return;
    };
    let hooks = match rt.object_get(ns, "__async_hooks_hooks__") {
        Value::Object(id) => id,
        _ => return,
    };
    let len = match rt.object_get(hooks, "length") {
        Value::Number(n) if n.is_finite() && n >= 0.0 => n as usize,
        _ => 0,
    };
    for idx in 0..len {
        let hook = match rt.object_get(hooks, idx.to_string().as_str()) {
            Value::Object(id) => id,
            _ => continue,
        };
        if !matches!(
            rt.object_get(hook, "__async_hooks_enabled__"),
            Value::Boolean(true)
        ) {
            continue;
        }
        let cb = rt.object_get(hook, "__async_hook_promise_resolve__");
        if !rt.is_callable(&cb) {
            continue;
        }
        if let Err(error) = rt.call_function(cb, Value::Undefined, vec![Value::Number(async_id)]) {
            rt.record_async_hook_fatal_exception(error);
            return;
        }
    }
}

fn async_hooks_emit_promise_destroy(rt: &mut Runtime, promise: ObjectRef) {
    let Some(ns) = async_hooks_namespace(rt) else {
        return;
    };
    if async_hooks_enabled_count(rt, ns) <= 0.0 {
        return;
    }
    let Some(async_id) = promise_async_id(rt, promise) else {
        return;
    };
    let hooks = match rt.object_get(ns, "__async_hooks_hooks__") {
        Value::Object(id) => id,
        _ => return,
    };
    let len = match rt.object_get(hooks, "length") {
        Value::Number(n) if n.is_finite() && n >= 0.0 => n as usize,
        _ => 0,
    };
    for idx in 0..len {
        let hook = match rt.object_get(hooks, idx.to_string().as_str()) {
            Value::Object(id) => id,
            _ => continue,
        };
        if !matches!(
            rt.object_get(hook, "__async_hooks_enabled__"),
            Value::Boolean(true)
        ) {
            continue;
        }
        let cb = rt.object_get(hook, "__async_hook_destroy__");
        if !rt.is_callable(&cb) {
            continue;
        }
        if let Err(error) = rt.call_function(cb, Value::Undefined, vec![Value::Number(async_id)]) {
            rt.record_async_hook_fatal_exception(error);
            return;
        }
    }
}

fn async_hooks_emit_promise_lifecycle(rt: &mut Runtime, ns: ObjectRef, slot: &str, async_id: f64) {
    if async_hooks_enabled_count(rt, ns) <= 0.0 {
        return;
    }
    let hooks = match rt.object_get(ns, "__async_hooks_hooks__") {
        Value::Object(id) => id,
        _ => return,
    };
    let len = match rt.object_get(hooks, "length") {
        Value::Number(n) if n.is_finite() && n >= 0.0 => n as usize,
        _ => 0,
    };
    for idx in 0..len {
        let hook = match rt.object_get(hooks, idx.to_string().as_str()) {
            Value::Object(id) => id,
            _ => continue,
        };
        if !matches!(
            rt.object_get(hook, "__async_hooks_enabled__"),
            Value::Boolean(true)
        ) {
            continue;
        }
        let cb = rt.object_get(hook, slot);
        if !rt.is_callable(&cb) {
            continue;
        }
        if let Err(error) = rt.call_function(cb, Value::Undefined, vec![Value::Number(async_id)]) {
            rt.record_async_hook_fatal_exception(error);
            return;
        }
    }
}

fn async_hooks_enter_promise_reaction(
    rt: &mut Runtime,
    chain: ObjectRef,
) -> Option<(ObjectRef, Value, Value)> {
    let ns = async_hooks_namespace(rt)?;
    let async_id = promise_async_id(rt, chain)?;
    let trigger = promise_trigger_async_id(rt, chain);
    let prev_id = rt.object_get(ns, "__execution_async_id__");
    let prev_trigger = rt.object_get(ns, "__trigger_async_id__");
    rt.object_set(ns, "__execution_async_id__".into(), Value::Number(async_id));
    rt.object_set(ns, "__trigger_async_id__".into(), trigger);
    async_hooks_emit_promise_lifecycle(rt, ns, "__async_hook_before__", async_id);
    Some((ns, prev_id, prev_trigger))
}

fn async_hooks_exit_promise_reaction(
    rt: &mut Runtime,
    state: Option<(ObjectRef, Value, Value)>,
    chain: ObjectRef,
) {
    let Some((ns, prev_id, prev_trigger)) = state else {
        return;
    };
    if let Some(async_id) = promise_async_id(rt, chain) {
        async_hooks_emit_promise_lifecycle(rt, ns, "__async_hook_after__", async_id);
    }
    rt.object_set(ns, "__trigger_async_id__".into(), prev_trigger);
    rt.object_set(ns, "__execution_async_id__".into(), prev_id);
}

fn promise_reaction_tail_drop_counters_enabled() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("CRUFT_PROMISE_REACTION_TAIL_DROP_COUNTERS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn record_promise_reaction_tail_drop(
    handler_kind: &'static str,
    drop_start: Option<std::time::Instant>,
) {
    let Some(drop_start) = drop_start else {
        return;
    };
    let n = PROMISE_REACTION_TAIL_DROP_CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    let elapsed = drop_start.elapsed().as_nanos() as u64;
    let total = PROMISE_REACTION_TAIL_DROP_NS.fetch_add(elapsed, Ordering::Relaxed) + elapsed;
    if n <= 16 || n % 1024 == 0 {
        eprintln!(
            "[promise-reaction-tail-drop-counters] calls={n} handler={handler_kind} avg_drop_ns={}",
            total / n
        );
    }
}

fn promise_reaction_drop_split_counters_enabled() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("CRUFT_PROMISE_REACTION_DROP_SPLIT_COUNTERS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn record_promise_reaction_drop_split(
    handler_kind: &'static str,
    shape_ns: u64,
    handler_ns: u64,
    cap_resolve_ns: u64,
    cap_reject_ns: u64,
) {
    if !promise_reaction_drop_split_counters_enabled() {
        return;
    }
    let n = PROMISE_REACTION_DROP_SPLIT_CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    let shape_total =
        PROMISE_REACTION_DROP_SPLIT_SHAPE_NS.fetch_add(shape_ns, Ordering::Relaxed) + shape_ns;
    let handler_total = PROMISE_REACTION_DROP_SPLIT_HANDLER_NS
        .fetch_add(handler_ns, Ordering::Relaxed)
        + handler_ns;
    let cap_resolve_total = PROMISE_REACTION_DROP_SPLIT_CAP_RESOLVE_NS
        .fetch_add(cap_resolve_ns, Ordering::Relaxed)
        + cap_resolve_ns;
    let cap_reject_total = PROMISE_REACTION_DROP_SPLIT_CAP_REJECT_NS
        .fetch_add(cap_reject_ns, Ordering::Relaxed)
        + cap_reject_ns;
    if n <= 16 || n % 1024 == 0 {
        eprintln!(
            "[promise-reaction-drop-split-counters] calls={n} handler={handler_kind} avg_shape_ns={} avg_handler_ns={} avg_cap_resolve_ns={} avg_cap_reject_ns={}",
            shape_total / n,
            handler_total / n,
            cap_resolve_total / n,
            cap_reject_total / n
        );
    }
}

fn promise_one_cell_drop_field_counters_enabled() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("CRUFT_PROMISE_ONE_CELL_DROP_FIELD_COUNTERS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn record_promise_one_cell_drop_fields(
    proto_ns: u64,
    upvalue_ns: u64,
    global_ns: u64,
    this_ns: u64,
    exec_fn_ns: u64,
    bool_ns: u64,
) {
    if !promise_one_cell_drop_field_counters_enabled() {
        return;
    }
    let n = PROMISE_ONE_CELL_DROP_FIELD_CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    let proto_total =
        PROMISE_ONE_CELL_DROP_FIELD_PROTO_NS.fetch_add(proto_ns, Ordering::Relaxed) + proto_ns;
    let upvalue_total = PROMISE_ONE_CELL_DROP_FIELD_UPVALUE_NS
        .fetch_add(upvalue_ns, Ordering::Relaxed)
        + upvalue_ns;
    let global_total =
        PROMISE_ONE_CELL_DROP_FIELD_GLOBAL_NS.fetch_add(global_ns, Ordering::Relaxed) + global_ns;
    let this_total =
        PROMISE_ONE_CELL_DROP_FIELD_THIS_NS.fetch_add(this_ns, Ordering::Relaxed) + this_ns;
    let exec_fn_total = PROMISE_ONE_CELL_DROP_FIELD_EXEC_FN_NS
        .fetch_add(exec_fn_ns, Ordering::Relaxed)
        + exec_fn_ns;
    let bool_total =
        PROMISE_ONE_CELL_DROP_FIELD_BOOL_NS.fetch_add(bool_ns, Ordering::Relaxed) + bool_ns;
    if n <= 16 || n % 1024 == 0 {
        eprintln!(
            "[promise-one-cell-drop-field-counters] calls={n} avg_proto_ns={} avg_upvalue_ns={} avg_global_ns={} avg_this_ns={} avg_exec_fn_ns={} avg_bool_ns={}",
            proto_total / n,
            upvalue_total / n,
            global_total / n,
            this_total / n,
            exec_fn_total / n,
            bool_total / n
        );
    }
}

fn promise_async_await_continuation_counters_enabled() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("CRUFT_PROMISE_ASYNC_AWAIT_CONTINUATION_COUNTERS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn record_promise_async_await_continuation(clone_ns: u64, resume_ns: u64, settle_ns: u64) {
    if !promise_async_await_continuation_counters_enabled() {
        return;
    }
    let n = PROMISE_ASYNC_AWAIT_CONTINUATION_CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    let clone_total =
        PROMISE_ASYNC_AWAIT_CONTINUATION_CLONE_NS.fetch_add(clone_ns, Ordering::Relaxed) + clone_ns;
    let resume_total = PROMISE_ASYNC_AWAIT_CONTINUATION_RESUME_NS
        .fetch_add(resume_ns, Ordering::Relaxed)
        + resume_ns;
    let settle_total = PROMISE_ASYNC_AWAIT_CONTINUATION_SETTLE_NS
        .fetch_add(settle_ns, Ordering::Relaxed)
        + settle_ns;
    if n <= 16 || n % 1024 == 0 {
        eprintln!(
            "[promise-async-await-continuation-counters] calls={n} avg_clone_ns={} avg_resume_ns={} avg_settle_ns={}",
            clone_total / n,
            resume_total / n,
            settle_total / n
        );
    }
}

fn promise_lazy_payload_shape_counters_enabled() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("CRUFT_PROMISE_LAZY_PAYLOAD_SHAPE_COUNTERS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn record_promise_lazy_payload_shape(handler: &Option<PromiseReactionHandler>) {
    if !promise_lazy_payload_shape_counters_enabled() {
        return;
    }
    let Some(PromiseReactionHandler::LazyArrow(lazy)) = handler else {
        return;
    };
    let n = PROMISE_LAZY_PAYLOAD_SHAPE_CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    let upvalues = lazy.upvalues.len() as u64;
    let bindings = lazy.captured_bindings.len() as u64;
    let with_envs = lazy.captured_with_env_stack.len() as u64;
    let total_upvalues =
        PROMISE_LAZY_PAYLOAD_SHAPE_UPVALUES.fetch_add(upvalues, Ordering::Relaxed) + upvalues;
    let total_bindings =
        PROMISE_LAZY_PAYLOAD_SHAPE_BINDINGS.fetch_add(bindings, Ordering::Relaxed) + bindings;
    let total_with_envs =
        PROMISE_LAZY_PAYLOAD_SHAPE_WITH_ENVS.fetch_add(with_envs, Ordering::Relaxed) + with_envs;
    let import_meta = if lazy.import_meta.is_some() {
        PROMISE_LAZY_PAYLOAD_SHAPE_IMPORT_META.fetch_add(1, Ordering::Relaxed) + 1
    } else {
        PROMISE_LAZY_PAYLOAD_SHAPE_IMPORT_META.load(Ordering::Relaxed)
    };
    let bound_this = if lazy.bound_this.is_some() {
        PROMISE_LAZY_PAYLOAD_SHAPE_BOUND_THIS.fetch_add(1, Ordering::Relaxed) + 1
    } else {
        PROMISE_LAZY_PAYLOAD_SHAPE_BOUND_THIS.load(Ordering::Relaxed)
    };
    let bound_this_cell = if lazy.bound_this_cell.is_some() {
        PROMISE_LAZY_PAYLOAD_SHAPE_BOUND_THIS_CELL.fetch_add(1, Ordering::Relaxed) + 1
    } else {
        PROMISE_LAZY_PAYLOAD_SHAPE_BOUND_THIS_CELL.load(Ordering::Relaxed)
    };
    let derived_this = if lazy.bound_derived_initial_this.is_some() {
        PROMISE_LAZY_PAYLOAD_SHAPE_DERIVED_THIS.fetch_add(1, Ordering::Relaxed) + 1
    } else {
        PROMISE_LAZY_PAYLOAD_SHAPE_DERIVED_THIS.load(Ordering::Relaxed)
    };
    let exec_fn = if lazy.bound_executing_function.is_some() {
        PROMISE_LAZY_PAYLOAD_SHAPE_EXEC_FN.fetch_add(1, Ordering::Relaxed) + 1
    } else {
        PROMISE_LAZY_PAYLOAD_SHAPE_EXEC_FN.load(Ordering::Relaxed)
    };
    let new_target = if lazy.bound_new_target.is_some() {
        PROMISE_LAZY_PAYLOAD_SHAPE_NEW_TARGET.fetch_add(1, Ordering::Relaxed) + 1
    } else {
        PROMISE_LAZY_PAYLOAD_SHAPE_NEW_TARGET.load(Ordering::Relaxed)
    };
    let new_target_allowed = if lazy.bound_new_target_allowed {
        PROMISE_LAZY_PAYLOAD_SHAPE_NEW_TARGET_ALLOWED.fetch_add(1, Ordering::Relaxed) + 1
    } else {
        PROMISE_LAZY_PAYLOAD_SHAPE_NEW_TARGET_ALLOWED.load(Ordering::Relaxed)
    };
    let compact1 = upvalues == 1
        && bindings == 0
        && with_envs == 0
        && lazy.import_meta.is_none()
        && lazy.bound_this_cell.is_none()
        && lazy.bound_derived_initial_this.is_none()
        && lazy.bound_executing_function.is_none()
        && lazy.bound_new_target.is_none()
        && !lazy.bound_new_target_allowed;
    let compact1_count = if compact1 {
        PROMISE_LAZY_PAYLOAD_SHAPE_COMPACT1.fetch_add(1, Ordering::Relaxed) + 1
    } else {
        PROMISE_LAZY_PAYLOAD_SHAPE_COMPACT1.load(Ordering::Relaxed)
    };
    for binding in &lazy.captured_bindings {
        match binding {
            CapturedBinding::Cell(_) => {
                PROMISE_LAZY_PAYLOAD_SHAPE_BINDING_CELL.fetch_add(1, Ordering::Relaxed);
            }
            CapturedBinding::GlobalObject { .. } => {
                PROMISE_LAZY_PAYLOAD_SHAPE_BINDING_GLOBAL.fetch_add(1, Ordering::Relaxed);
            }
            CapturedBinding::ScriptGlobalVar { .. } => {
                PROMISE_LAZY_PAYLOAD_SHAPE_BINDING_SCRIPT_GLOBAL.fetch_add(1, Ordering::Relaxed);
            }
            CapturedBinding::EvalVarShadow { .. } => {
                PROMISE_LAZY_PAYLOAD_SHAPE_BINDING_EVAL_SHADOW.fetch_add(1, Ordering::Relaxed);
            }
            CapturedBinding::ImmutableSelfName { .. } => {
                PROMISE_LAZY_PAYLOAD_SHAPE_BINDING_IMMUTABLE_SELF.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    if n <= 16 || n % 1024 == 0 {
        let binding_cell = PROMISE_LAZY_PAYLOAD_SHAPE_BINDING_CELL.load(Ordering::Relaxed);
        let binding_global = PROMISE_LAZY_PAYLOAD_SHAPE_BINDING_GLOBAL.load(Ordering::Relaxed);
        let binding_script_global =
            PROMISE_LAZY_PAYLOAD_SHAPE_BINDING_SCRIPT_GLOBAL.load(Ordering::Relaxed);
        let binding_eval_shadow =
            PROMISE_LAZY_PAYLOAD_SHAPE_BINDING_EVAL_SHADOW.load(Ordering::Relaxed);
        let binding_immutable_self =
            PROMISE_LAZY_PAYLOAD_SHAPE_BINDING_IMMUTABLE_SELF.load(Ordering::Relaxed);
        eprintln!(
            "[promise-lazy-payload-shape-counters] calls={n} avg_upvalues={} avg_bindings={} avg_with_envs={} import_meta={import_meta} bound_this={bound_this} bound_this_cell={bound_this_cell} derived_this={derived_this} exec_fn={exec_fn} new_target={new_target} new_target_allowed={new_target_allowed} compact1={compact1_count} binding_cell={binding_cell} binding_global={binding_global} binding_script_global={binding_script_global} binding_eval_shadow={binding_eval_shadow} binding_immutable_self={binding_immutable_self}",
            total_upvalues / n,
            total_bindings / n,
            total_with_envs / n
        );
    }
}

impl Runtime {
    pub fn install_promise(&mut self) {

        let promise_ctor = crate::intrinsics::make_native("Promise", |rt, args| {

            if rt.current_new_target.is_none() {
                return Err(RuntimeError::TypeError(
                    "Promise constructor must be called with new".into(),
                ));
            }
            let executor = args.first().cloned().unwrap_or(Value::Undefined);
            if !rt.is_callable(&executor) {
                return Err(RuntimeError::TypeError(
                    "Promise constructor: executor must be callable".into(),
                ));
            }
            let p = new_promise(rt);

            if let Some(Value::Object(nt_id)) = rt.current_new_target.clone() {

                if let Some(fallback) = rt.promise_prototype {
                    let proto = rt.get_prototype_from_constructor(
                        nt_id,
                        |rr| rr.promise_prototype,
                        fallback,
                    )?;
                    rt.obj_mut(p).proto = Some(proto);
                } else if let Value::Object(pid) = rt.object_get(nt_id, "prototype") {
                    rt.obj_mut(p).proto = Some(pid);
                }
            }
            let (resolve_id, reject_id) = create_resolving_functions(rt, p);

            let _executor_roots = rt.push_temporary_value_roots(&[
                Value::Object(p),
                Value::Object(resolve_id),
                Value::Object(reject_id),
                executor.clone(),
            ]);
            if let Err(e) = rt.call_function(
                executor,
                Value::Undefined,
                vec![Value::Object(resolve_id), Value::Object(reject_id)],
            ) {
                let reason = match e {
                    crate::interp::RuntimeError::Thrown(v) => v,
                    other => match crate::intrinsics::make_error_instance(
                        rt,
                        "TypeError",
                        &format!("{:?}", other),
                    ) {
                        Some(id) => Value::Object(id),
                        None => Value::String(std::rc::Rc::new(crate::value::JsString::from(
                            format!("{:?}", other),
                        ))),
                    },
                };
                rt.call_function(Value::Object(reject_id), Value::Undefined, vec![reason])?;
            }
            Ok(Value::Object(p))
        });
        let promise_obj = self.alloc_object(promise_ctor);

        crate::intrinsics::register_intrinsic_method(
            self,
            promise_obj,
            "resolve",
            1,
            move |rt, args| {
                let c = rt.current_this();
                if !matches!(c, Value::Object(_)) {
                    return Err(RuntimeError::TypeError(
                        "Promise.resolve: this is not an Object".into(),
                    ));
                }
                let x = args.first().cloned().unwrap_or(Value::Undefined);

                if let Value::Object(xid) = &x {
                    if matches!(rt.obj(*xid).internal_kind, InternalKind::Promise(_)) {
                        let xc = rt.spec_get(&x, "constructor")?;
                        if matches!((&xc, &c), (Value::Object(a), Value::Object(b)) if a == b) {
                            return Ok(x);
                        }
                    }
                }
                let (cap_promise, cap_resolve, _cap_reject) = rt.new_promise_capability(&c)?;
                rt.call_function(cap_resolve, Value::Undefined, vec![x])?;
                Ok(cap_promise)
            },
        );
        crate::intrinsics::register_intrinsic_method(
            self,
            promise_obj,
            "reject",
            1,
            move |rt, args| {
                let c = rt.current_this();
                if !matches!(c, Value::Object(_)) {
                    return Err(RuntimeError::TypeError(
                        "Promise.reject: this is not an Object".into(),
                    ));
                }
                let r = args.first().cloned().unwrap_or(Value::Undefined);

                let (cap_promise, _cap_resolve, cap_reject) = rt.new_promise_capability(&c)?;
                rt.call_function(cap_reject, Value::Undefined, vec![r])?;
                Ok(cap_promise)
            },
        );

        crate::intrinsics::register_intrinsic_method(self, promise_obj, "then", 3, |rt, args| {
            crate::generated::promise_prototype_then(rt, rt.current_this(), args)
        });
        crate::intrinsics::register_intrinsic_method(self, promise_obj, "catch_", 1, |rt, args| {
            crate::generated::promise_prototype_catch(rt, rt.current_this(), args)
        });

        crate::intrinsics::register_intrinsic_method(self, promise_obj, "all", 1, |rt, args| {
            let c = rt.current_this();
            if !matches!(c, Value::Object(_)) {
                return Err(RuntimeError::TypeError(
                    "Promise.all: this is not an Object".into(),
                ));
            }
            crate::generated::promise_all(rt, c, args)
        });
        crate::intrinsics::register_intrinsic_method(
            self,
            promise_obj,
            "allSettled",
            1,
            |rt, args| {
                let c = rt.current_this();
                if !matches!(c, Value::Object(_)) {
                    return Err(RuntimeError::TypeError(
                        "Promise.allSettled: this is not an Object".into(),
                    ));
                }
                crate::generated::promise_all_settled(rt, c, args)
            },
        );
        crate::intrinsics::register_intrinsic_method(self, promise_obj, "any", 1, |rt, args| {
            let c = rt.current_this();
            if !matches!(c, Value::Object(_)) {
                return Err(RuntimeError::TypeError(
                    "Promise.any: this is not an Object".into(),
                ));
            }
            crate::generated::promise_any(rt, c, args)
        });
        crate::intrinsics::register_intrinsic_method(self, promise_obj, "race", 1, |rt, args| {
            let c = rt.current_this();
            if !matches!(c, Value::Object(_)) {
                return Err(RuntimeError::TypeError(
                    "Promise.race: this is not an Object".into(),
                ));
            }
            crate::generated::promise_race(rt, c, args)
        });

        crate::intrinsics::register_intrinsic_method(
            self,
            promise_obj,
            "allKeyed",
            1,
            |rt, args| {
                let c = rt.current_this();
                if !matches!(c, Value::Object(_)) {
                    return Err(RuntimeError::TypeError(
                        "Promise.allKeyed: this is not an Object".into(),
                    ));
                }
                rt.promise_all_keyed_via(args)
            },
        );
        crate::intrinsics::register_intrinsic_method(
            self,
            promise_obj,
            "allSettledKeyed",
            1,
            |rt, args| {
                let c = rt.current_this();
                if !matches!(c, Value::Object(_)) {
                    return Err(RuntimeError::TypeError(
                        "Promise.allSettledKeyed: this is not an Object".into(),
                    ));
                }
                rt.promise_all_settled_keyed_via(args)
            },
        );

        crate::intrinsics::register_intrinsic_method(
            self,
            promise_obj,
            "withResolvers",
            0,
            |rt, _args| {

                let c = rt.current_this();
                let (promise, resolve, reject) = rt.new_promise_capability(&c)?;
                rt.promise_with_resolvers_assemble_via(&promise, &resolve, &reject)
            },
        );
        if let Some(proto) = self.promise_prototype {
            self.obj_mut(promise_obj)
                .set_own_frozen("prototype".into(), Value::Object(proto));

            self.obj_mut(proto).dict_mut().insert(
                "@@toStringTag".into(),
                crate::value::PropertyDescriptor {
                    value: Value::String(Rc::new(crate::value::JsString::from("Promise"))),
                    writable: false,
                    enumerable: false,
                    configurable: true,
                    getter: None,
                    setter: None,
                },
            );

            self.obj_mut(proto)
                .set_own_internal("constructor".into(), Value::Object(promise_obj));

            crate::intrinsics::register_intrinsic_method(self, proto, "finally", 1, |rt, args| {
                let mut a: Vec<Value> = Vec::with_capacity(args.len() + 1);
                a.push(rt.current_this());
                a.extend(args.iter().cloned());
                crate::generated::promise_prototype_finally(rt, rt.current_this(), &a)
            });
        }
        self.promise_prototype_then = self
            .promise_prototype
            .and_then(|proto| match self.object_get(proto, "then") {
                Value::Object(id) => Some(id),
                _ => None,
            })
            .or_else(|| match self.object_get(promise_obj, "then") {
                Value::Object(id) => Some(id),
                _ => None,
            });
        if let Some(realm) = self.realms.get_mut(self.current_realm) {
            realm.promise_prototype_then = self.promise_prototype_then;
        }

        let species_getter =
            crate::intrinsics::make_native_with_length("get [Symbol.species]", 0, |rt, _args| {
                Ok(rt.current_this())
            });
        let species_getter_id = self.alloc_object(species_getter);
        let species_desc = crate::value::PropertyDescriptor {
            value: Value::Undefined,
            writable: false,
            enumerable: false,
            configurable: true,
            getter: Some(Value::Object(species_getter_id)),
            setter: None,
        };
        self.obj_mut(promise_obj).dict_mut().insert(
            crate::value::PropertyKey::String("@@species".into()),
            species_desc,
        );
        self.define_global_property("Promise", Value::Object(promise_obj));
    }
}

pub fn new_promise(rt: &mut Runtime) -> ObjectRef {
    new_promise_with_async_trigger(rt, None)
}

pub fn new_promise_with_async_trigger(rt: &mut Runtime, trigger: Option<Value>) -> ObjectRef {
    record_promise_alloc("promise");

    let proto = rt.promise_prototype;
    let promise = rt.alloc_object(Object {
        proto,
        extensible: true,
        properties: indexmap::IndexMap::new(),
        internal_kind: InternalKind::Promise(Box::new(PromiseState {
            status: PromiseStatus::Pending,
            value: Value::Undefined,
            fulfill_reactions: Vec::new(),
            reject_reactions: Vec::new(),
        })),

        ..Default::default()
    });
    async_hooks_emit_promise_init(rt, promise, trigger);
    promise
}

pub(crate) fn async_hooks_promise_id_value(rt: &Runtime, promise: ObjectRef) -> Option<Value> {
    promise_async_id(rt, promise).map(Value::Number)
}

pub(crate) fn create_resolving_functions(
    rt: &mut Runtime,
    promise: ObjectRef,
) -> (ObjectRef, ObjectRef) {
    record_promise_alloc("resolving-pair");
    let already_resolved = Rc::new(Cell::new(false));

    let p_for_resolve = promise;
    let resolve_flag = already_resolved.clone();
    let resolve_fn = crate::intrinsics::make_native_non_ctor("", 1, move |rt, args| {
        if resolve_flag.get() {
            return Ok(Value::Undefined);
        }
        resolve_flag.set(true);
        let v = args.first().cloned().unwrap_or(Value::Undefined);
        resolve_promise(rt, p_for_resolve, v);
        Ok(Value::Undefined)
    });

    let p_for_reject = promise;
    let reject_flag = already_resolved;
    let reject_fn = crate::intrinsics::make_native_non_ctor("", 1, move |rt, args| {
        if reject_flag.get() {
            return Ok(Value::Undefined);
        }
        reject_flag.set(true);
        let v = args.first().cloned().unwrap_or(Value::Undefined);
        reject_promise(rt, p_for_reject, v);
        Ok(Value::Undefined)
    });

    let resolve_id = rt.alloc_object(resolve_fn);
    record_promise_alloc("resolving-function");
    let reject_id = rt.alloc_object(reject_fn);
    record_promise_alloc("resolving-function");
    for id in [resolve_id, reject_id] {
        if let InternalKind::Function(f) = &mut rt.obj_mut(id).internal_kind {
            f.roots.push(promise);
        }
    }
    (resolve_id, reject_id)
}

pub fn resolve_promise(rt: &mut Runtime, promise: ObjectRef, value: Value) {
    let timing = promise_resolve_counters_enabled().then(std::time::Instant::now);
    let value_is_object = matches!(value, Value::Object(_));
    if let Value::Object(value_id) = value.clone() {
        if value_id == promise {

            let reason = crate::intrinsics::make_error_instance(
                rt,
                "TypeError",
                "Chaining cycle detected for promise",
            )
            .map(Value::Object)
            .unwrap_or_else(|| {
                Value::String(std::rc::Rc::new(crate::value::JsString::from(
                    "Chaining cycle detected for promise".to_string(),
                )))
            });
            reject_promise(rt, promise, reason);
            record_promise_resolve(timing, value_is_object, 0, false, false);
            return;
        }

        let has_own_then = rt.obj(value_id).has_own_str("then");

        let proto_is_default_promise = rt.obj(value_id).proto == rt.promise_prototype;
        let maybe_promise = if has_own_then || !proto_is_default_promise {
            None
        } else {
            let o = rt.obj(value_id);
            match &o.internal_kind {
                InternalKind::Promise(ps) => Some((ps.status, ps.value.clone())),
                _ => None,
            }
        };
        if let Some((status, settled_value)) = maybe_promise {
            match status {
                PromiseStatus::Pending => {
                    let src = rt.obj_mut(value_id);
                    if let InternalKind::Promise(ps) = &mut src.internal_kind {
                        ps.fulfill_reactions.push(PromiseReaction {
                            handler: None,
                            chain: promise,
                            cap_resolve: None,
                            cap_reject: None,
                        });
                        ps.reject_reactions.push(PromiseReaction {
                            handler: None,
                            chain: promise,
                            cap_resolve: None,
                            cap_reject: None,
                        });
                    }
                    rt.pending_unhandled.remove(&value_id);
                    record_promise_resolve(timing, value_is_object, 0, true, false);
                    return;
                }
                PromiseStatus::Fulfilled => {
                    record_promise_resolve(timing, value_is_object, 0, true, false);
                    resolve_promise(rt, promise, settled_value);
                    return;
                }
                PromiseStatus::Rejected => {
                    rt.pending_unhandled.remove(&value_id);
                    record_promise_resolve(timing, value_is_object, 0, true, false);
                    reject_promise(rt, promise, settled_value);
                    return;
                }
            }
        }

        let then_v = match rt.spec_get(&value, "then") {
            Ok(v) => v,
            Err(e) => {
                let reason = match e {
                    crate::interp::RuntimeError::Thrown(v) => v,
                    other => {
                        let msg = format!("{:?}", other);
                        Value::String(std::rc::Rc::new(crate::value::JsString::from(msg)))
                    }
                };
                reject_promise(rt, promise, reason);
                record_promise_resolve(timing, value_is_object, 0, false, false);
                return;
            }
        };
        if rt.is_callable(&then_v) {
            let (resolve_id, reject_id) = create_resolving_functions(rt, promise);
            let roots = [
                &then_v,
                &value,
                &Value::Object(resolve_id),
                &Value::Object(reject_id),
            ]
            .into_iter()
            .filter_map(|v| match v {
                Value::Object(id) => Some(*id),
                _ => None,
            })
            .collect();
            rt.enqueue_microtask_rooted("PromiseResolveThenableJob", roots, move |rt| {
                if let Err(e) = rt.call_function(
                    then_v,
                    value,
                    vec![Value::Object(resolve_id), Value::Object(reject_id)],
                ) {
                    let reason = match e {
                        crate::interp::RuntimeError::Thrown(v) => v,
                        other => Value::String(std::rc::Rc::new(crate::value::JsString::from(
                            format!("{:?}", other),
                        ))),
                    };
                    rt.call_function(Value::Object(reject_id), Value::Undefined, vec![reason])?;
                }
                Ok(())
            });
            record_promise_resolve(timing, value_is_object, 0, false, true);
            return;
        }
    }
    let reactions = {
        let p = rt.obj_mut(promise);
        if let InternalKind::Promise(ps) = &mut p.internal_kind {
            if !matches!(ps.status, PromiseStatus::Pending) {
                record_promise_resolve(timing, value_is_object, 0, false, false);
                return;
            }
            ps.status = PromiseStatus::Fulfilled;
            ps.value = value;
            std::mem::take(&mut ps.fulfill_reactions)
        } else {
            record_promise_resolve(timing, value_is_object, 0, false, false);
            return;
        }
    };
    async_hooks_emit_promise_resolve(rt, promise);
    async_hooks_emit_promise_destroy(rt, promise);
    let reaction_count = reactions.len();
    let value = match &rt.obj(promise).internal_kind {
        InternalKind::Promise(ps) => ps.value.clone(),
        _ => Value::Undefined,
    };
    for reaction in reactions {
        enqueue_reaction(
            rt,
            reaction.handler,
            value.clone(),
            reaction.chain,
            reaction.cap_resolve,
            reaction.cap_reject,
            false,
        );
    }
    record_promise_resolve(timing, value_is_object, reaction_count, false, false);
}

pub(crate) fn fulfill_promise_non_object(rt: &mut Runtime, promise: ObjectRef, value: Value) {
    debug_assert!(
        !matches!(value, Value::Object(_)),
        "non-object fulfill fast path must not bypass thenable adoption"
    );
    let reactions = {
        let p = rt.obj_mut(promise);
        if let InternalKind::Promise(ps) = &mut p.internal_kind {
            if !matches!(ps.status, PromiseStatus::Pending) {
                return;
            }
            ps.status = PromiseStatus::Fulfilled;
            ps.value = value.clone();
            std::mem::take(&mut ps.fulfill_reactions)
        } else {
            return;
        }
    };
    async_hooks_emit_promise_resolve(rt, promise);
    async_hooks_emit_promise_destroy(rt, promise);
    let reaction_count = reactions.len();
    for reaction in reactions {
        enqueue_reaction(
            rt,
            reaction.handler,
            value.clone(),
            reaction.chain,
            reaction.cap_resolve,
            reaction.cap_reject,
            false,
        );
    }
    record_promise_resolve(None, false, reaction_count, false, false);
}

pub fn reject_promise(rt: &mut Runtime, promise: ObjectRef, reason: Value) {
    let reactions = {
        let p = rt.obj_mut(promise);
        if let InternalKind::Promise(ps) = &mut p.internal_kind {
            if !matches!(ps.status, PromiseStatus::Pending) {
                return;
            }
            ps.status = PromiseStatus::Rejected;
            ps.value = reason;
            std::mem::take(&mut ps.reject_reactions)
        } else {
            return;
        }
    };
    async_hooks_emit_promise_resolve(rt, promise);
    async_hooks_emit_promise_destroy(rt, promise);

    let host_report_suppressed = rt
        .obj(promise)
        .has_own_str("__cruft_suppress_unhandled_rejection");
    if reactions.is_empty() && !host_report_suppressed {
        rt.pending_unhandled.insert(promise);
    }
    let value = match &rt.obj(promise).internal_kind {
        InternalKind::Promise(ps) => ps.value.clone(),
        _ => Value::Undefined,
    };
    for reaction in reactions {
        enqueue_reaction(
            rt,
            reaction.handler,
            value.clone(),
            reaction.chain,
            reaction.cap_resolve,
            reaction.cap_reject,
            true,
        );
    }
}

pub(crate) fn enqueue_reaction(
    rt: &mut Runtime,
    handler: Option<PromiseReactionHandler>,
    value: Value,
    chain: ObjectRef,
    cap_resolve: Option<Value>,
    cap_reject: Option<Value>,
    is_rejected: bool,
) {
    let enqueue_timing = promise_reaction_enqueue_counters_enabled().then(std::time::Instant::now);
    let handler_kind = match &handler {
        Some(PromiseReactionHandler::Callable(_)) => "callable",
        Some(PromiseReactionHandler::LazyArrow(_)) => "lazy-arrow",
        Some(PromiseReactionHandler::LazyArrowOneCell(_)) => "lazy-arrow-one-cell",
        Some(PromiseReactionHandler::AsyncAwaitContinuation { .. }) => "async-await",
        None => "empty",
    };
    let root_start = promise_reaction_enqueue_counters_enabled().then(std::time::Instant::now);
    let root_capacity_hint = match &handler {
        Some(PromiseReactionHandler::LazyArrowOneCell(_)) => 4,
        Some(PromiseReactionHandler::LazyArrow(lazy)) => {
            4 + lazy.upvalues.len()
                + lazy.captured_bindings.len()
                + lazy.captured_with_env_stack.len()
        }
        Some(PromiseReactionHandler::Callable(_)) => 4,
        Some(PromiseReactionHandler::AsyncAwaitContinuation { .. }) => 8,
        None => 3,
    };
    let mut roots = Vec::with_capacity(root_capacity_hint);
    if let Some(PromiseReactionHandler::Callable(Value::Object(id))) = &handler {
        roots.push(*id);
    }
    if let Some(PromiseReactionHandler::LazyArrow(lazy)) = &handler {
        if let Some(id) = lazy.creation_global {
            roots.push(id);
        }
        if let Some(id) = lazy.import_meta {
            roots.push(id);
        }
        for cell in &lazy.upvalues {
            if let Value::Object(id) = &*cell.borrow() {
                roots.push(*id);
            }
        }
        for binding in &lazy.captured_bindings {
            match binding {
                crate::value::CapturedBinding::Cell(cell)
                | crate::value::CapturedBinding::EvalVarShadow { cell, .. }
                | crate::value::CapturedBinding::ImmutableSelfName { cell, .. } => {
                    if let Value::Object(id) = &*cell.borrow() {
                        roots.push(*id);
                    }
                }
                crate::value::CapturedBinding::GlobalObject { .. }
                | crate::value::CapturedBinding::ScriptGlobalVar { .. } => {}
            }
        }
        if let Some(Value::Object(id)) = &lazy.bound_this {
            roots.push(*id);
        }
        if let Some(cell) = &lazy.bound_this_cell {
            if let Value::Object(id) = &*cell.borrow() {
                roots.push(*id);
            }
        }
        if let Some(Value::Object(id)) = &lazy.bound_derived_initial_this {
            roots.push(*id);
        }
        if let Some(id) = lazy.bound_executing_function {
            roots.push(id);
        }
        if let Some(Value::Object(id)) = &lazy.bound_new_target {
            roots.push(*id);
        }
        roots.extend(lazy.captured_with_env_stack.iter().copied());
    }
    if let Some(PromiseReactionHandler::LazyArrowOneCell(lazy)) = &handler {
        if let Some(id) = lazy.creation_global {
            roots.push(id);
        }
        if let Value::Object(id) = &*lazy.upvalue.borrow() {
            roots.push(*id);
        }
        if let Some(Value::Object(id)) = &lazy.bound_this {
            roots.push(*id);
        }
        if let Some(id) = lazy.bound_executing_function {
            roots.push(id);
        }
    }
    if let Some(PromiseReactionHandler::AsyncAwaitContinuation { promise, snapshot }) = &handler {
        roots.push(*promise);
        snapshot.trace_object_refs(&mut roots);
    }
    if let Value::Object(id) = &value {
        roots.push(*id);
    }
    roots.push(chain);
    if let Some(Value::Object(id)) = &cap_resolve {
        roots.push(*id);
    }
    if let Some(Value::Object(id)) = &cap_reject {
        roots.push(*id);
    }
    let root_ns = root_start
        .as_ref()
        .map(|start| start.elapsed().as_nanos() as u64)
        .unwrap_or(0);
    let root_count = roots.len();
    if !structured_promise_reaction_job_enabled() {
        rt.enqueue_microtask_rooted("PromiseReactionJob", roots, move |rt| {
            run_reaction_job(
                rt,
                handler,
                value,
                chain,
                cap_resolve,
                cap_reject,
                is_rejected,
            )
        });
        record_promise_reaction_enqueue(handler_kind, root_ns, enqueue_timing, root_count);
        return;
    }
    if structured_async_await_continuation_job_enabled()
        && cap_resolve.is_none()
        && cap_reject.is_none()
    {
        if let Some(PromiseReactionHandler::AsyncAwaitContinuation { promise, snapshot }) = handler
        {
            rt.enqueue_async_await_continuation_job(
                roots,
                chain,
                promise,
                snapshot,
                value,
                is_rejected,
            );
            record_promise_reaction_enqueue(handler_kind, root_ns, enqueue_timing, root_count);
            return;
        }
    }
    rt.enqueue_promise_reaction_job(
        roots,
        handler,
        value,
        chain,
        cap_resolve,
        cap_reject,
        is_rejected,
    );
    record_promise_reaction_enqueue(handler_kind, root_ns, enqueue_timing, root_count);
}

fn structured_promise_reaction_job_enabled() -> bool {
    std::env::var("CRUFT_PROMISE_STRUCTURED_REACTION_JOB")
        .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
        .unwrap_or(true)
}

fn structured_async_await_continuation_job_enabled() -> bool {
    std::env::var("CRUFT_ASYNC_AWAIT_CONTINUATION_STRUCTURED_JOB")
        .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
        .unwrap_or(true)
}

pub(crate) fn run_reaction_job(
    rt: &mut Runtime,
    handler: Option<PromiseReactionHandler>,
    value: Value,
    chain: ObjectRef,
    cap_resolve: Option<Value>,
    cap_reject: Option<Value>,
    is_rejected: bool,
) -> Result<(), RuntimeError> {
    let handler_kind = match &handler {
        Some(PromiseReactionHandler::Callable(_)) => "callable",
        Some(PromiseReactionHandler::LazyArrow(_)) => "lazy-arrow",
        Some(PromiseReactionHandler::LazyArrowOneCell(_)) => "lazy-arrow-one-cell",
        Some(PromiseReactionHandler::AsyncAwaitContinuation { .. }) => "async-await",
        None => "empty",
    };
    let total_start = promise_reaction_total_counters_enabled().then(std::time::Instant::now);
    let reaction_result_counters = promise_reaction_result_counters_enabled();
    let async_hooks_state = async_hooks_enter_promise_reaction(rt, chain);
    if let Some(PromiseReactionHandler::AsyncAwaitContinuation { promise, snapshot }) = handler {
        let handler_start = reaction_result_counters.then(std::time::Instant::now);
        let resume = if is_rejected {
            crate::interp::AsyncResume::Throw(value)
        } else {
            crate::interp::AsyncResume::Value(value)
        };
        let split_enabled = promise_async_await_continuation_counters_enabled();
        let clone_start = split_enabled.then(std::time::Instant::now);
        let snapshot = *snapshot;
        let clone_ns = clone_start
            .map(|start| start.elapsed().as_nanos() as u64)
            .unwrap_or(0);
        let resume_start = split_enabled.then(std::time::Instant::now);
        let r = rt.resume_suspended_async(snapshot, resume);
        let resume_ns = resume_start
            .map(|start| start.elapsed().as_nanos() as u64)
            .unwrap_or(0);
        let handler_ns = handler_start
            .map(|start| start.elapsed().as_nanos() as u64)
            .unwrap_or(0);
        let capability_start = reaction_result_counters.then(std::time::Instant::now);
        let settle_start = split_enabled.then(std::time::Instant::now);
        rt.settle_async_result(promise, r);
        let settle_ns = settle_start
            .map(|start| start.elapsed().as_nanos() as u64)
            .unwrap_or(0);
        record_promise_async_await_continuation(clone_ns, resume_ns, settle_ns);
        record_promise_reaction_result(
            "async-await",
            handler_ns,
            capability_start,
            false,
            is_rejected,
        );
        let result = Ok(());
        async_hooks_exit_promise_reaction(rt, async_hooks_state, chain);
        record_promise_reaction_total(handler_kind, total_start, false);
        let drop_start =
            promise_reaction_tail_drop_counters_enabled().then(std::time::Instant::now);
        let split_enabled = promise_reaction_drop_split_counters_enabled();
        let cap_resolve_drop_start = split_enabled.then(std::time::Instant::now);
        drop(cap_resolve);
        let cap_resolve_drop_ns = cap_resolve_drop_start
            .map(|start| start.elapsed().as_nanos() as u64)
            .unwrap_or(0);
        let cap_reject_drop_start = split_enabled.then(std::time::Instant::now);
        drop(cap_reject);
        let cap_reject_drop_ns = cap_reject_drop_start
            .map(|start| start.elapsed().as_nanos() as u64)
            .unwrap_or(0);
        record_promise_reaction_drop_split(
            handler_kind,
            0,
            0,
            cap_resolve_drop_ns,
            cap_reject_drop_ns,
        );
        record_promise_reaction_tail_drop(handler_kind, drop_start);
        return result;
    }
    let resolve_capability = |rt: &mut Runtime, value: Value| -> Result<(), RuntimeError> {
        if let Some(resolve) = cap_resolve.clone() {
            rt.call_function(resolve, Value::Undefined, vec![value])?;
        } else {
            resolve_promise(rt, chain, value);
        }
        Ok(())
    };
    let reject_capability = |rt: &mut Runtime, reason: Value| -> Result<(), RuntimeError> {
        if let Some(reject) = cap_reject.clone() {
            rt.call_function(reject, Value::Undefined, vec![reason])?;
        } else {
            reject_promise(rt, chain, reason);
        }
        Ok(())
    };
    let result = match &handler {
        Some(PromiseReactionHandler::Callable(h)) => {
            let handler_start = reaction_result_counters.then(std::time::Instant::now);
            match rt.call_function(h.clone(), Value::Undefined, vec![value]) {
                Ok(result) => {
                    let handler_ns = handler_start
                        .map(|start| start.elapsed().as_nanos() as u64)
                        .unwrap_or(0);
                    let capability_start = reaction_result_counters.then(std::time::Instant::now);
                    resolve_capability(rt, result)?;
                    record_promise_reaction_result(
                        "callable",
                        handler_ns,
                        capability_start,
                        cap_resolve.is_some(),
                        false,
                    );
                    Ok(())
                }
                Err(e) => {
                    let handler_ns = handler_start
                        .map(|start| start.elapsed().as_nanos() as u64)
                        .unwrap_or(0);
                    let thrown = match e {
                        RuntimeError::Thrown(v) => v,
                        other => Value::String(std::rc::Rc::new(crate::value::JsString::from(
                            format!("{:?}", other),
                        ))),
                    };
                    let capability_start = reaction_result_counters.then(std::time::Instant::now);
                    reject_capability(rt, thrown)?;
                    record_promise_reaction_result(
                        "callable",
                        handler_ns,
                        capability_start,
                        cap_reject.is_some(),
                        true,
                    );
                    Ok(())
                }
            }
        }
        Some(PromiseReactionHandler::LazyArrow(lazy)) => {
            let handler_start = reaction_result_counters.then(std::time::Instant::now);
            match rt.call_lazy_promise_arrow(lazy, value) {
                Ok(result) => {
                    let handler_ns = handler_start
                        .map(|start| start.elapsed().as_nanos() as u64)
                        .unwrap_or(0);
                    let capability_start = reaction_result_counters.then(std::time::Instant::now);
                    resolve_capability(rt, result)?;
                    record_promise_reaction_result(
                        "lazy-arrow",
                        handler_ns,
                        capability_start,
                        cap_resolve.is_some(),
                        false,
                    );
                    Ok(())
                }
                Err(e) => {
                    let handler_ns = handler_start
                        .map(|start| start.elapsed().as_nanos() as u64)
                        .unwrap_or(0);
                    let thrown = match e {
                        RuntimeError::Thrown(v) => v,
                        other => Value::String(std::rc::Rc::new(crate::value::JsString::from(
                            format!("{:?}", other),
                        ))),
                    };
                    let capability_start = reaction_result_counters.then(std::time::Instant::now);
                    reject_capability(rt, thrown)?;
                    record_promise_reaction_result(
                        "lazy-arrow",
                        handler_ns,
                        capability_start,
                        cap_reject.is_some(),
                        true,
                    );
                    Ok(())
                }
            }
        }
        Some(PromiseReactionHandler::LazyArrowOneCell(lazy)) => {
            let handler_start = reaction_result_counters.then(std::time::Instant::now);
            match rt.call_lazy_promise_arrow_one_cell(lazy, value) {
                Ok(result) => {
                    let handler_ns = handler_start
                        .map(|start| start.elapsed().as_nanos() as u64)
                        .unwrap_or(0);
                    let capability_start = reaction_result_counters.then(std::time::Instant::now);
                    resolve_capability(rt, result)?;
                    record_promise_reaction_result(
                        "lazy-arrow-one-cell",
                        handler_ns,
                        capability_start,
                        cap_resolve.is_some(),
                        false,
                    );
                    Ok(())
                }
                Err(e) => {
                    let handler_ns = handler_start
                        .map(|start| start.elapsed().as_nanos() as u64)
                        .unwrap_or(0);
                    let thrown = match e {
                        RuntimeError::Thrown(v) => v,
                        other => Value::String(std::rc::Rc::new(crate::value::JsString::from(
                            format!("{:?}", other),
                        ))),
                    };
                    let capability_start = reaction_result_counters.then(std::time::Instant::now);
                    reject_capability(rt, thrown)?;
                    record_promise_reaction_result(
                        "lazy-arrow-one-cell",
                        handler_ns,
                        capability_start,
                        cap_reject.is_some(),
                        true,
                    );
                    Ok(())
                }
            }
        }
        Some(PromiseReactionHandler::AsyncAwaitContinuation { promise, snapshot }) => {
            let handler_start = reaction_result_counters.then(std::time::Instant::now);
            let resume = if is_rejected {
                crate::interp::AsyncResume::Throw(value)
            } else {
                crate::interp::AsyncResume::Value(value)
            };
            let split_enabled = promise_async_await_continuation_counters_enabled();
            let clone_start = split_enabled.then(std::time::Instant::now);
            let snapshot = (**snapshot).clone();
            let clone_ns = clone_start
                .map(|start| start.elapsed().as_nanos() as u64)
                .unwrap_or(0);
            let resume_start = split_enabled.then(std::time::Instant::now);
            let r = rt.resume_suspended_async(snapshot, resume);
            let resume_ns = resume_start
                .map(|start| start.elapsed().as_nanos() as u64)
                .unwrap_or(0);
            let handler_ns = handler_start
                .map(|start| start.elapsed().as_nanos() as u64)
                .unwrap_or(0);
            let capability_start = reaction_result_counters.then(std::time::Instant::now);
            let settle_start = split_enabled.then(std::time::Instant::now);
            rt.settle_async_result(*promise, r);
            let settle_ns = settle_start
                .map(|start| start.elapsed().as_nanos() as u64)
                .unwrap_or(0);
            record_promise_async_await_continuation(clone_ns, resume_ns, settle_ns);
            record_promise_reaction_result(
                "async-await",
                handler_ns,
                capability_start,
                false,
                is_rejected,
            );
            Ok(())
        }
        None => {
            if is_rejected {
                let capability_start = reaction_result_counters.then(std::time::Instant::now);
                reject_capability(rt, value)?;
                record_promise_reaction_result(
                    "empty",
                    0,
                    capability_start,
                    cap_reject.is_some(),
                    true,
                );
                Ok(())
            } else {
                let capability_start = reaction_result_counters.then(std::time::Instant::now);
                resolve_capability(rt, value)?;
                record_promise_reaction_result(
                    "empty",
                    0,
                    capability_start,
                    cap_resolve.is_some(),
                    false,
                );
                Ok(())
            }
        }
    };
    async_hooks_exit_promise_reaction(rt, async_hooks_state, chain);
    record_promise_reaction_total(handler_kind, total_start, result.is_err());
    let drop_start = promise_reaction_tail_drop_counters_enabled().then(std::time::Instant::now);
    let split_enabled = promise_reaction_drop_split_counters_enabled();
    let shape_start = split_enabled.then(std::time::Instant::now);
    record_promise_lazy_payload_shape(&handler);
    let shape_ns = shape_start
        .map(|start| start.elapsed().as_nanos() as u64)
        .unwrap_or(0);
    let handler_drop_start = split_enabled.then(std::time::Instant::now);
    if promise_one_cell_drop_field_counters_enabled() {
        match handler {
            Some(PromiseReactionHandler::LazyArrowOneCell(lazy)) => {
                let crate::value::PromiseLazyArrowOneCellHandler {
                    proto,
                    upvalue,
                    creation_realm,
                    creation_global,
                    bound_this,
                    bound_executing_function,
                    bound_new_target_allowed,
                } = lazy;
                let proto_start = std::time::Instant::now();
                drop(proto);
                let proto_ns = proto_start.elapsed().as_nanos() as u64;
                let upvalue_start = std::time::Instant::now();
                drop(upvalue);
                let upvalue_ns = upvalue_start.elapsed().as_nanos() as u64;
                let global_start = std::time::Instant::now();
                let _ = creation_global;
                let global_ns = global_start.elapsed().as_nanos() as u64;
                let this_start = std::time::Instant::now();
                drop(bound_this);
                let this_ns = this_start.elapsed().as_nanos() as u64;
                let exec_fn_start = std::time::Instant::now();
                let _ = bound_executing_function;
                let exec_fn_ns = exec_fn_start.elapsed().as_nanos() as u64;
                let bool_start = std::time::Instant::now();
                let _ = creation_realm;
                let _ = bound_new_target_allowed;
                let bool_ns = bool_start.elapsed().as_nanos() as u64;
                record_promise_one_cell_drop_fields(
                    proto_ns, upvalue_ns, global_ns, this_ns, exec_fn_ns, bool_ns,
                );
            }
            other => drop(other),
        }
    } else {
        drop(handler);
    }
    let handler_drop_ns = handler_drop_start
        .map(|start| start.elapsed().as_nanos() as u64)
        .unwrap_or(0);
    let cap_resolve_drop_start = split_enabled.then(std::time::Instant::now);
    drop(cap_resolve);
    let cap_resolve_drop_ns = cap_resolve_drop_start
        .map(|start| start.elapsed().as_nanos() as u64)
        .unwrap_or(0);
    let cap_reject_drop_start = split_enabled.then(std::time::Instant::now);
    drop(cap_reject);
    let cap_reject_drop_ns = cap_reject_drop_start
        .map(|start| start.elapsed().as_nanos() as u64)
        .unwrap_or(0);
    record_promise_reaction_drop_split(
        handler_kind,
        shape_ns,
        handler_drop_ns,
        cap_resolve_drop_ns,
        cap_reject_drop_ns,
    );
    record_promise_reaction_tail_drop(handler_kind, drop_start);
    result
}

pub(crate) fn run_async_await_continuation_job(
    rt: &mut Runtime,
    chain: ObjectRef,
    promise: ObjectRef,
    snapshot: Box<crate::interp::FrameSnapshot>,
    value: Value,
    is_rejected: bool,
) -> Result<(), RuntimeError> {
    let total_start = promise_reaction_total_counters_enabled().then(std::time::Instant::now);
    let reaction_result_counters = promise_reaction_result_counters_enabled();
    let handler_start = reaction_result_counters.then(std::time::Instant::now);
    let resume = if is_rejected {
        crate::interp::AsyncResume::Throw(value)
    } else {
        crate::interp::AsyncResume::Value(value)
    };
    let split_enabled = promise_async_await_continuation_counters_enabled();
    let resume_start = split_enabled.then(std::time::Instant::now);
    let async_hooks_state = async_hooks_enter_promise_reaction(rt, chain);
    let result = rt.resume_suspended_async(*snapshot, resume);
    let resume_ns = resume_start
        .map(|start| start.elapsed().as_nanos() as u64)
        .unwrap_or(0);
    let handler_ns = handler_start
        .map(|start| start.elapsed().as_nanos() as u64)
        .unwrap_or(0);
    let capability_start = reaction_result_counters.then(std::time::Instant::now);
    let settle_start = split_enabled.then(std::time::Instant::now);
    rt.settle_async_result(promise, result);
    let settle_ns = settle_start
        .map(|start| start.elapsed().as_nanos() as u64)
        .unwrap_or(0);
    async_hooks_exit_promise_reaction(rt, async_hooks_state, chain);
    record_promise_async_await_continuation(0, resume_ns, settle_ns);
    record_promise_reaction_result(
        "async-await",
        handler_ns,
        capability_start,
        false,
        is_rejected,
    );
    record_promise_reaction_total("async-await", total_start, false);
    Ok(())
}

fn register_method<F>(rt: &mut Runtime, host: ObjectRef, name: &str, f: F)
where
    F: Fn(&mut Runtime, &[Value]) -> Result<Value, RuntimeError> + 'static,
{
    let native: NativeFn = Rc::new(f);
    let mut properties = indexmap::IndexMap::new();
    crate::value::install_function_meta_props(&mut properties, name, 0.0);
    let fn_obj = Object {
        proto: None,
        extensible: true,
        properties,

        internal_kind: InternalKind::Function(Box::new(FunctionInternals {
            name: name.to_string(),
            length: 0,
            native,
            is_constructor: false,
            creation_realm: 0,
            roots: Vec::new(),
        })),

        ..Default::default()
    };
    let fn_id = rt.alloc_object(fn_obj);
    rt.object_set(host, name.into(), Value::Object(fn_id));
}
