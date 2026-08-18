
use crate::register::{make_callable, make_callable_rooted, new_object, register_method};
use rusty_js_runtime::abstract_ops;
use rusty_js_runtime::value::Object as RtObject;
use rusty_js_runtime::{HostEnqueuePhase, Runtime, RuntimeError, Value};
use std::borrow::Cow;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

fn event_emit_phase_counters_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_EVENT_EMIT_PHASE_COUNTERS")
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or(false)
    })
}

fn event_emit_listener_counters_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_EVENT_EMIT_LISTENER_COUNTERS")
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or(false)
    })
}

fn event_emit_direct_listener_counters_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_EVENT_EMIT_DIRECT_LISTENER_COUNTERS")
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or(false)
    })
}

fn event_emit_direct_phase_counters_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_EVENT_EMIT_DIRECT_PHASE_COUNTERS")
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or(false)
    })
}

fn event_emit_direct_call_bucket_counters_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_EVENT_EMIT_DIRECT_CALL_BUCKET_COUNTERS")
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or(false)
    })
}

fn event_emit_direct_overhead_counters_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_EVENT_EMIT_DIRECT_OVERHEAD_COUNTERS")
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or(false)
    })
}

fn event_emit_direct_forward_probe_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_EVENT_EMIT_DIRECT_FORWARD_PROBE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn js_string(s: &str) -> Value {
    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(s)))
}

fn emit_remove_listener(
    rt: &mut Runtime,
    em: rusty_js_runtime::ObjectRef,
    event: &Value,
    listener: &Value,
) {
    if matches!(listener, Value::Undefined) {
        return;
    }
    let emit_fn = rt.object_get(em, "emit");
    if rt.is_callable(&emit_fn) {
        let _ = rt.call_function(
            emit_fn,
            Value::Object(em),
            vec![js_string("removeListener"), event.clone(), listener.clone()],
        );
    }
}

fn emit_removed_for_key(
    rt: &mut Runtime,
    em: rusty_js_runtime::ObjectRef,
    bag: rusty_js_runtime::ObjectRef,
    key: &str,
    ev_val: &Value,
) {
    let stored = listener_bag_get(rt, bag, key);

    let mut listeners: Vec<Value> = match &stored {
        Value::Object(id)
            if matches!(
                rt.obj(*id).internal_kind,
                rusty_js_runtime::value::InternalKind::Array
            ) =>
        {
            let len = rt.array_length(*id);
            (0..len)
                .map(|i| rt.object_get(*id, &i.to_string()))
                .collect()
        }
        Value::Object(_) => vec![stored.clone()],
        _ => return,
    };
    listeners.reverse();
    for l in listeners {
        let orig = match &l {
            Value::Object(id) => {
                let inner = rt.object_get(*id, "__once");
                if matches!(inner, Value::Undefined) {
                    l.clone()
                } else {
                    inner
                }
            }
            _ => l.clone(),
        };
        emit_remove_listener(rt, em, ev_val, &orig);
    }
}

fn capture_listener_rejection(
    rt: &mut Runtime,
    em: rusty_js_runtime::ObjectRef,
    event_value: &Value,
    rest: &[Value],
    ret: Value,
) {
    if !matches!(
        rt.object_get(em, "__cruft_capture_rejections__"),
        Value::Boolean(true)
    ) {
        return;
    }
    let ret_id = match ret {
        Value::Object(id) => id,
        _ => return,
    };
    let then = rt.object_get(ret_id, "then");
    if !rt.is_callable(&then) {
        return;
    }
    let ev = event_value.clone();
    let args_vec: Vec<Value> = rest.to_vec();
    let handler = make_callable_rooted(rt, "", vec![em], move |rt, hargs| {
        let err = hargs.first().cloned().unwrap_or(Value::Undefined);

        let handler_fn = match rt.symbol_for_via(&[js_string("nodejs.rejection")]) {
            Ok(Value::Symbol(rc)) => rt
                .read_property_pk(em, &rusty_js_runtime::value::PropertyKey::Symbol(rc))
                .unwrap_or(Value::Undefined),
            _ => Value::Undefined,
        };
        if rt.is_callable(&handler_fn) {
            let mut call_args = vec![err, ev.clone()];
            call_args.extend(args_vec.iter().cloned());
            let _ = rt.call_function(handler_fn, Value::Object(em), call_args);
        } else {
            let emit_fn = rt.object_get(em, "emit");
            if rt.is_callable(&emit_fn) {
                let _ = rt.call_function(emit_fn, Value::Object(em), vec![js_string("error"), err]);
            }
        }
        Ok(Value::Undefined)
    });
    let _ = rt.call_function(
        then,
        Value::Object(ret_id),
        vec![Value::Undefined, Value::Object(handler)],
    );
}

fn record_event_emit_direct_forward_probe(outcome: &'static str, argc: usize) {
    if !event_emit_direct_forward_probe_enabled() {
        return;
    }
    static CALLS: AtomicU64 = AtomicU64::new(0);
    static HITS: AtomicU64 = AtomicU64::new(0);
    static MISSES: AtomicU64 = AtomicU64::new(0);
    static ERRORS: AtomicU64 = AtomicU64::new(0);
    static ARGC0: AtomicU64 = AtomicU64::new(0);
    static ARGC1: AtomicU64 = AtomicU64::new(0);
    static ARGC2: AtomicU64 = AtomicU64::new(0);
    static ARGC3PLUS: AtomicU64 = AtomicU64::new(0);
    let calls = CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    match outcome {
        "hit" => {
            HITS.fetch_add(1, Ordering::Relaxed);
        }
        "error" => {
            ERRORS.fetch_add(1, Ordering::Relaxed);
        }
        _ => {
            MISSES.fetch_add(1, Ordering::Relaxed);
        }
    }
    match argc {
        0 => {
            ARGC0.fetch_add(1, Ordering::Relaxed);
        }
        1 => {
            ARGC1.fetch_add(1, Ordering::Relaxed);
        }
        2 => {
            ARGC2.fetch_add(1, Ordering::Relaxed);
        }
        _ => {
            ARGC3PLUS.fetch_add(1, Ordering::Relaxed);
        }
    }
    if calls <= 16 || calls.is_power_of_two() {
        eprintln!(
            "[event-emit-direct-forward-probe] calls={} hits={} misses={} errors={} argc0={} argc1={} argc2={} argc3plus={} last_outcome={} last_argc={}",
            calls,
            HITS.load(Ordering::Relaxed),
            MISSES.load(Ordering::Relaxed),
            ERRORS.load(Ordering::Relaxed),
            ARGC0.load(Ordering::Relaxed),
            ARGC1.load(Ordering::Relaxed),
            ARGC2.load(Ordering::Relaxed),
            ARGC3PLUS.load(Ordering::Relaxed),
            outcome,
            argc
        );
    }
}

fn record_event_emit_direct_phase(
    callable_ns: u64,
    classify_ns: u64,
    call_ns: u64,
    total_ns: u64,
    argc: usize,
    called: bool,
) {
    if !event_emit_direct_phase_counters_enabled() {
        return;
    }
    static CALLS: AtomicU64 = AtomicU64::new(0);
    static CALLED: AtomicU64 = AtomicU64::new(0);
    static CALLABLE_NS: AtomicU64 = AtomicU64::new(0);
    static CLASSIFY_NS: AtomicU64 = AtomicU64::new(0);
    static CALL_NS: AtomicU64 = AtomicU64::new(0);
    static TOTAL_NS: AtomicU64 = AtomicU64::new(0);
    static ARG0: AtomicU64 = AtomicU64::new(0);
    static ARG1: AtomicU64 = AtomicU64::new(0);
    static ARG2: AtomicU64 = AtomicU64::new(0);
    static ARG3PLUS: AtomicU64 = AtomicU64::new(0);

    if called {
        CALLED.fetch_add(1, Ordering::Relaxed);
    }
    CALLABLE_NS.fetch_add(callable_ns, Ordering::Relaxed);
    CLASSIFY_NS.fetch_add(classify_ns, Ordering::Relaxed);
    CALL_NS.fetch_add(call_ns, Ordering::Relaxed);
    TOTAL_NS.fetch_add(total_ns, Ordering::Relaxed);
    match argc {
        0 => ARG0.fetch_add(1, Ordering::Relaxed),
        1 => ARG1.fetch_add(1, Ordering::Relaxed),
        2 => ARG2.fetch_add(1, Ordering::Relaxed),
        _ => ARG3PLUS.fetch_add(1, Ordering::Relaxed),
    };
    let calls = CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    if calls <= 16 || calls.is_power_of_two() {
        let avg = |ns: &AtomicU64| ns.load(Ordering::Relaxed) / calls.max(1);
        eprintln!(
            "[event-emit-direct-phase] calls={} called={} avg_callable_ns={} avg_classify_ns={} avg_call_ns={} avg_total_ns={} argc0={} argc1={} argc2={} argc3plus={}",
            calls,
            CALLED.load(Ordering::Relaxed),
            avg(&CALLABLE_NS),
            avg(&CLASSIFY_NS),
            avg(&CALL_NS),
            avg(&TOTAL_NS),
            ARG0.load(Ordering::Relaxed),
            ARG1.load(Ordering::Relaxed),
            ARG2.load(Ordering::Relaxed),
            ARG3PLUS.load(Ordering::Relaxed)
        );
    }
}

fn record_event_emit_direct_overhead(
    path_ns: u64,
    callable_ns: u64,
    classify_ns: u64,
    call_ns: u64,
    post_call_ns: u64,
    total_ns: u64,
    argc: usize,
) {
    if !event_emit_direct_overhead_counters_enabled() {
        return;
    }
    static CALLS: AtomicU64 = AtomicU64::new(0);
    static PATH_NS: AtomicU64 = AtomicU64::new(0);
    static CALLABLE_NS: AtomicU64 = AtomicU64::new(0);
    static CLASSIFY_NS: AtomicU64 = AtomicU64::new(0);
    static CALL_NS: AtomicU64 = AtomicU64::new(0);
    static POST_CALL_NS: AtomicU64 = AtomicU64::new(0);
    static TOTAL_NS: AtomicU64 = AtomicU64::new(0);
    static RESIDUAL_NS: AtomicU64 = AtomicU64::new(0);
    static ARG0: AtomicU64 = AtomicU64::new(0);
    static ARG1: AtomicU64 = AtomicU64::new(0);
    static ARG2: AtomicU64 = AtomicU64::new(0);
    static ARG3PLUS: AtomicU64 = AtomicU64::new(0);

    let measured = path_ns
        .saturating_add(callable_ns)
        .saturating_add(classify_ns)
        .saturating_add(call_ns)
        .saturating_add(post_call_ns);
    let residual_ns = total_ns.saturating_sub(measured);
    PATH_NS.fetch_add(path_ns, Ordering::Relaxed);
    CALLABLE_NS.fetch_add(callable_ns, Ordering::Relaxed);
    CLASSIFY_NS.fetch_add(classify_ns, Ordering::Relaxed);
    CALL_NS.fetch_add(call_ns, Ordering::Relaxed);
    POST_CALL_NS.fetch_add(post_call_ns, Ordering::Relaxed);
    TOTAL_NS.fetch_add(total_ns, Ordering::Relaxed);
    RESIDUAL_NS.fetch_add(residual_ns, Ordering::Relaxed);
    match argc {
        0 => ARG0.fetch_add(1, Ordering::Relaxed),
        1 => ARG1.fetch_add(1, Ordering::Relaxed),
        2 => ARG2.fetch_add(1, Ordering::Relaxed),
        _ => ARG3PLUS.fetch_add(1, Ordering::Relaxed),
    };
    let calls = CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    if calls <= 16 || calls.is_power_of_two() {
        let avg = |ns: &AtomicU64| ns.load(Ordering::Relaxed) / calls.max(1);
        eprintln!(
            "[event-emit-direct-overhead] calls={} avg_path_ns={} avg_callable_ns={} avg_classify_ns={} avg_call_ns={} avg_post_call_ns={} avg_total_ns={} avg_residual_ns={} argc0={} argc1={} argc2={} argc3plus={}",
            calls,
            avg(&PATH_NS),
            avg(&CALLABLE_NS),
            avg(&CLASSIFY_NS),
            avg(&CALL_NS),
            avg(&POST_CALL_NS),
            avg(&TOTAL_NS),
            avg(&RESIDUAL_NS),
            ARG0.load(Ordering::Relaxed),
            ARG1.load(Ordering::Relaxed),
            ARG2.load(Ordering::Relaxed),
            ARG3PLUS.load(Ordering::Relaxed)
        );
    }
}

fn record_event_emit_direct_call_bucket(reason: &'static str, argc: usize, call_ns: u64) {
    if !event_emit_direct_call_bucket_counters_enabled() {
        return;
    }
    static CALLS: AtomicU64 = AtomicU64::new(0);
    static TOTAL_NS: AtomicU64 = AtomicU64::new(0);
    static ARG0: AtomicU64 = AtomicU64::new(0);
    static ARG0_NS: AtomicU64 = AtomicU64::new(0);
    static ARG1: AtomicU64 = AtomicU64::new(0);
    static ARG1_NS: AtomicU64 = AtomicU64::new(0);
    static ARG2: AtomicU64 = AtomicU64::new(0);
    static ARG2_NS: AtomicU64 = AtomicU64::new(0);
    static ARG3PLUS: AtomicU64 = AtomicU64::new(0);
    static ARG3PLUS_NS: AtomicU64 = AtomicU64::new(0);
    static ELIGIBLE: AtomicU64 = AtomicU64::new(0);
    static ELIGIBLE_NS: AtomicU64 = AtomicU64::new(0);
    static NEW_TARGET_OR_ARGC: AtomicU64 = AtomicU64::new(0);
    static NEW_TARGET_OR_ARGC_NS: AtomicU64 = AtomicU64::new(0);
    static ARG0_NOT_NUMBER: AtomicU64 = AtomicU64::new(0);
    static ARG0_NOT_NUMBER_NS: AtomicU64 = AtomicU64::new(0);
    static CLOSURE_INELIGIBLE: AtomicU64 = AtomicU64::new(0);
    static CLOSURE_INELIGIBLE_NS: AtomicU64 = AtomicU64::new(0);
    static BODY_SHAPE: AtomicU64 = AtomicU64::new(0);
    static BODY_SHAPE_NS: AtomicU64 = AtomicU64::new(0);
    static OTHER: AtomicU64 = AtomicU64::new(0);
    static OTHER_NS: AtomicU64 = AtomicU64::new(0);

    let calls = CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    TOTAL_NS.fetch_add(call_ns, Ordering::Relaxed);
    match argc {
        0 => {
            ARG0.fetch_add(1, Ordering::Relaxed);
            ARG0_NS.fetch_add(call_ns, Ordering::Relaxed);
        }
        1 => {
            ARG1.fetch_add(1, Ordering::Relaxed);
            ARG1_NS.fetch_add(call_ns, Ordering::Relaxed);
        }
        2 => {
            ARG2.fetch_add(1, Ordering::Relaxed);
            ARG2_NS.fetch_add(call_ns, Ordering::Relaxed);
        }
        _ => {
            ARG3PLUS.fetch_add(1, Ordering::Relaxed);
            ARG3PLUS_NS.fetch_add(call_ns, Ordering::Relaxed);
        }
    }
    match reason {
        "eligible" => {
            ELIGIBLE.fetch_add(1, Ordering::Relaxed);
            ELIGIBLE_NS.fetch_add(call_ns, Ordering::Relaxed);
        }
        "shape:new-target-or-argc" => {
            NEW_TARGET_OR_ARGC.fetch_add(1, Ordering::Relaxed);
            NEW_TARGET_OR_ARGC_NS.fetch_add(call_ns, Ordering::Relaxed);
        }
        "arg0-not-number" => {
            ARG0_NOT_NUMBER.fetch_add(1, Ordering::Relaxed);
            ARG0_NOT_NUMBER_NS.fetch_add(call_ns, Ordering::Relaxed);
        }
        "closure-ineligible" => {
            CLOSURE_INELIGIBLE.fetch_add(1, Ordering::Relaxed);
            CLOSURE_INELIGIBLE_NS.fetch_add(call_ns, Ordering::Relaxed);
        }
        "shape:params-upvalues"
        | "shape:load-arg"
        | "shape:add"
        | "shape:store-or-return"
        | "shape:load-upvalue"
        | "shape:other" => {
            BODY_SHAPE.fetch_add(1, Ordering::Relaxed);
            BODY_SHAPE_NS.fetch_add(call_ns, Ordering::Relaxed);
        }
        _ => {
            OTHER.fetch_add(1, Ordering::Relaxed);
            OTHER_NS.fetch_add(call_ns, Ordering::Relaxed);
        }
    }
    if calls <= 16 || calls.is_power_of_two() {
        let avg = |ns: &AtomicU64, count: &AtomicU64| {
            let count = count.load(Ordering::Relaxed);
            if count == 0 {
                0
            } else {
                ns.load(Ordering::Relaxed) / count
            }
        };
        eprintln!(
            "[event-emit-direct-call-bucket] calls={} avg_call_ns={} argc0={} avg_argc0_ns={} argc1={} avg_argc1_ns={} argc2={} avg_argc2_ns={} argc3plus={} avg_argc3plus_ns={} eligible={} avg_eligible_ns={} new_target_or_argc={} avg_new_target_or_argc_ns={} arg0_not_number={} avg_arg0_not_number_ns={} closure_ineligible={} avg_closure_ineligible_ns={} body_shape={} avg_body_shape_ns={} other={} avg_other_ns={} last_reason={} last_argc={}",
            calls,
            TOTAL_NS.load(Ordering::Relaxed) / calls,
            ARG0.load(Ordering::Relaxed),
            avg(&ARG0_NS, &ARG0),
            ARG1.load(Ordering::Relaxed),
            avg(&ARG1_NS, &ARG1),
            ARG2.load(Ordering::Relaxed),
            avg(&ARG2_NS, &ARG2),
            ARG3PLUS.load(Ordering::Relaxed),
            avg(&ARG3PLUS_NS, &ARG3PLUS),
            ELIGIBLE.load(Ordering::Relaxed),
            avg(&ELIGIBLE_NS, &ELIGIBLE),
            NEW_TARGET_OR_ARGC.load(Ordering::Relaxed),
            avg(&NEW_TARGET_OR_ARGC_NS, &NEW_TARGET_OR_ARGC),
            ARG0_NOT_NUMBER.load(Ordering::Relaxed),
            avg(&ARG0_NOT_NUMBER_NS, &ARG0_NOT_NUMBER),
            CLOSURE_INELIGIBLE.load(Ordering::Relaxed),
            avg(&CLOSURE_INELIGIBLE_NS, &CLOSURE_INELIGIBLE),
            BODY_SHAPE.load(Ordering::Relaxed),
            avg(&BODY_SHAPE_NS, &BODY_SHAPE),
            OTHER.load(Ordering::Relaxed),
            avg(&OTHER_NS, &OTHER),
            reason,
            argc
        );
    }
}

fn record_event_emit_direct_listener_candidate(reason: &'static str, argc: usize) {
    if !event_emit_direct_listener_counters_enabled() {
        return;
    }
    static CALLS: AtomicU64 = AtomicU64::new(0);
    static ELIGIBLE: AtomicU64 = AtomicU64::new(0);
    static NEW_TARGET_OR_ARGC: AtomicU64 = AtomicU64::new(0);
    static ARG0_NOT_NUMBER: AtomicU64 = AtomicU64::new(0);
    static CLOSURE_INELIGIBLE: AtomicU64 = AtomicU64::new(0);
    static BODY_SHAPE: AtomicU64 = AtomicU64::new(0);
    static OTHER: AtomicU64 = AtomicU64::new(0);
    static ARG0: AtomicU64 = AtomicU64::new(0);
    static ARG1: AtomicU64 = AtomicU64::new(0);
    static ARG2: AtomicU64 = AtomicU64::new(0);
    static ARG3PLUS: AtomicU64 = AtomicU64::new(0);

    match reason {
        "eligible" => ELIGIBLE.fetch_add(1, Ordering::Relaxed),
        "shape:new-target-or-argc" => NEW_TARGET_OR_ARGC.fetch_add(1, Ordering::Relaxed),
        "arg0-not-number" => ARG0_NOT_NUMBER.fetch_add(1, Ordering::Relaxed),
        "closure-ineligible" => CLOSURE_INELIGIBLE.fetch_add(1, Ordering::Relaxed),
        "shape:params-upvalues"
        | "shape:load-arg"
        | "shape:add"
        | "shape:store-or-return"
        | "shape:load-upvalue"
        | "shape:other" => BODY_SHAPE.fetch_add(1, Ordering::Relaxed),
        _ => OTHER.fetch_add(1, Ordering::Relaxed),
    };
    match argc {
        0 => ARG0.fetch_add(1, Ordering::Relaxed),
        1 => ARG1.fetch_add(1, Ordering::Relaxed),
        2 => ARG2.fetch_add(1, Ordering::Relaxed),
        _ => ARG3PLUS.fetch_add(1, Ordering::Relaxed),
    };
    let calls = CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    if calls <= 16 || calls.is_power_of_two() {
        eprintln!(
            "[event-emit-direct-listener] calls={} eligible={} new_target_or_argc={} arg0_not_number={} closure_ineligible={} body_shape={} other={} argc0={} argc1={} argc2={} argc3plus={} last_reason={} last_argc={}",
            calls,
            ELIGIBLE.load(Ordering::Relaxed),
            NEW_TARGET_OR_ARGC.load(Ordering::Relaxed),
            ARG0_NOT_NUMBER.load(Ordering::Relaxed),
            CLOSURE_INELIGIBLE.load(Ordering::Relaxed),
            BODY_SHAPE.load(Ordering::Relaxed),
            OTHER.load(Ordering::Relaxed),
            ARG0.load(Ordering::Relaxed),
            ARG1.load(Ordering::Relaxed),
            ARG2.load(Ordering::Relaxed),
            ARG3PLUS.load(Ordering::Relaxed),
            reason,
            argc
        );
    }
}

fn record_event_emit_listener_path(path: &'static str, argc: usize, len: u64) {
    if !event_emit_listener_counters_enabled() {
        return;
    }
    static CALLS: AtomicU64 = AtomicU64::new(0);
    static NO_BAG: AtomicU64 = AtomicU64::new(0);
    static DIRECT_CALLABLE: AtomicU64 = AtomicU64::new(0);
    static DIRECT_ONCE: AtomicU64 = AtomicU64::new(0);
    static DIRECT_MISS: AtomicU64 = AtomicU64::new(0);
    static ARRAY_EMPTY: AtomicU64 = AtomicU64::new(0);
    static ARRAY_ONE_CALLABLE: AtomicU64 = AtomicU64::new(0);
    static ARRAY_MULTI_CALLS: AtomicU64 = AtomicU64::new(0);
    static ARRAY_ONCE: AtomicU64 = AtomicU64::new(0);
    static ARG0: AtomicU64 = AtomicU64::new(0);
    static ARG1: AtomicU64 = AtomicU64::new(0);
    static ARG2: AtomicU64 = AtomicU64::new(0);
    static ARG3PLUS: AtomicU64 = AtomicU64::new(0);
    static LAST_LEN: AtomicU64 = AtomicU64::new(0);

    match path {
        "no-bag" => NO_BAG.fetch_add(1, Ordering::Relaxed),
        "direct-callable" => DIRECT_CALLABLE.fetch_add(1, Ordering::Relaxed),
        "direct-once" => DIRECT_ONCE.fetch_add(1, Ordering::Relaxed),
        "direct-miss" => DIRECT_MISS.fetch_add(1, Ordering::Relaxed),
        "array-empty" => ARRAY_EMPTY.fetch_add(1, Ordering::Relaxed),
        "array-one-callable" => ARRAY_ONE_CALLABLE.fetch_add(1, Ordering::Relaxed),
        "array-multi-calls" => ARRAY_MULTI_CALLS.fetch_add(len, Ordering::Relaxed),
        "array-once" => ARRAY_ONCE.fetch_add(1, Ordering::Relaxed),
        _ => return,
    };
    match argc {
        0 => ARG0.fetch_add(1, Ordering::Relaxed),
        1 => ARG1.fetch_add(1, Ordering::Relaxed),
        2 => ARG2.fetch_add(1, Ordering::Relaxed),
        _ => ARG3PLUS.fetch_add(1, Ordering::Relaxed),
    };
    LAST_LEN.store(len, Ordering::Relaxed);
    let calls = CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    if calls <= 16 || calls.is_power_of_two() {
        eprintln!(
            "[event-emit-listener-path] calls={} no_bag={} direct_callable={} direct_once={} direct_miss={} array_empty={} array_one_callable={} array_multi_calls={} array_once={} argc0={} argc1={} argc2={} argc3plus={} last_path={} last_argc={} last_len={}",
            calls,
            NO_BAG.load(Ordering::Relaxed),
            DIRECT_CALLABLE.load(Ordering::Relaxed),
            DIRECT_ONCE.load(Ordering::Relaxed),
            DIRECT_MISS.load(Ordering::Relaxed),
            ARRAY_EMPTY.load(Ordering::Relaxed),
            ARRAY_ONE_CALLABLE.load(Ordering::Relaxed),
            ARRAY_MULTI_CALLS.load(Ordering::Relaxed),
            ARRAY_ONCE.load(Ordering::Relaxed),
            ARG0.load(Ordering::Relaxed),
            ARG1.load(Ordering::Relaxed),
            ARG2.load(Ordering::Relaxed),
            ARG3PLUS.load(Ordering::Relaxed),
            path,
            argc,
            LAST_LEN.load(Ordering::Relaxed)
        );
    }
}

fn record_event_emit_phase(phase: &'static str, elapsed_ns: u64) {
    if !event_emit_phase_counters_enabled() {
        return;
    }
    static CALLS: AtomicU64 = AtomicU64::new(0);
    static EVENT_NS: AtomicU64 = AtomicU64::new(0);
    static REST_NS: AtomicU64 = AtomicU64::new(0);
    static BAG_NS: AtomicU64 = AtomicU64::new(0);
    static LIST_NS: AtomicU64 = AtomicU64::new(0);
    static CALL_NS: AtomicU64 = AtomicU64::new(0);
    static ARRAY_NS: AtomicU64 = AtomicU64::new(0);
    static TOTAL_NS: AtomicU64 = AtomicU64::new(0);

    let bucket = match phase {
        "event" => &EVENT_NS,
        "rest" => &REST_NS,
        "bag" => &BAG_NS,
        "list" => &LIST_NS,
        "call" => &CALL_NS,
        "array" => &ARRAY_NS,
        "total" => &TOTAL_NS,
        _ => return,
    };
    bucket.fetch_add(elapsed_ns, Ordering::Relaxed);
    if phase != "total" {
        return;
    }
    let calls = CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    if !calls.is_power_of_two() {
        return;
    }
    let total = TOTAL_NS.load(Ordering::Relaxed);
    if total == 0 {
        return;
    }
    let avg = |ns: &AtomicU64| ns.load(Ordering::Relaxed) / calls.max(1);
    eprintln!(
        "[event-emit-phase] calls={} avg_total_ns={} avg_event_ns={} avg_rest_ns={} avg_bag_ns={} avg_list_ns={} avg_call_ns={} avg_array_ns={}",
        calls,
        avg(&TOTAL_NS),
        avg(&EVENT_NS),
        avg(&REST_NS),
        avg(&BAG_NS),
        avg(&LIST_NS),
        avg(&CALL_NS),
        avg(&ARRAY_NS)
    );
}

fn event_on_phase_counters_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_EVENT_ON_PHASE_COUNTERS")
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or(false)
    })
}

fn record_event_on_phase(phase: &'static str, elapsed_ns: u64) {
    if !event_on_phase_counters_enabled() {
        return;
    }
    static CALLS: AtomicU64 = AtomicU64::new(0);
    static EVENT_NS: AtomicU64 = AtomicU64::new(0);
    static BAG_NS: AtomicU64 = AtomicU64::new(0);
    static BAG_LOOKUP_NS: AtomicU64 = AtomicU64::new(0);
    static BAG_ALLOC_NS: AtomicU64 = AtomicU64::new(0);
    static BAG_PUBLISH_NS: AtomicU64 = AtomicU64::new(0);
    static NEW_LISTENER_NS: AtomicU64 = AtomicU64::new(0);
    static APPEND_NS: AtomicU64 = AtomicU64::new(0);
    static MARK_NS: AtomicU64 = AtomicU64::new(0);
    static TOTAL_NS: AtomicU64 = AtomicU64::new(0);

    let bucket = match phase {
        "event" => &EVENT_NS,
        "bag" => &BAG_NS,
        "bag_lookup" => &BAG_LOOKUP_NS,
        "bag_alloc" => &BAG_ALLOC_NS,
        "bag_publish" => &BAG_PUBLISH_NS,
        "new_listener" => &NEW_LISTENER_NS,
        "append" => &APPEND_NS,
        "mark" => &MARK_NS,
        "total" => &TOTAL_NS,
        _ => return,
    };
    bucket.fetch_add(elapsed_ns, Ordering::Relaxed);
    if phase != "total" {
        return;
    }
    let calls = CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    if !calls.is_power_of_two() {
        return;
    }
    let avg = |ns: &AtomicU64| ns.load(Ordering::Relaxed) / calls.max(1);
    eprintln!(
        "[event-on-phase] calls={} avg_total_ns={} avg_event_ns={} avg_bag_ns={} avg_bag_lookup_ns={} avg_bag_alloc_ns={} avg_bag_publish_ns={} avg_new_listener_ns={} avg_append_ns={} avg_mark_ns={}",
        calls,
        avg(&TOTAL_NS),
        avg(&EVENT_NS),
        avg(&BAG_NS),
        avg(&BAG_LOOKUP_NS),
        avg(&BAG_ALLOC_NS),
        avg(&BAG_PUBLISH_NS),
        avg(&NEW_LISTENER_NS),
        avg(&APPEND_NS),
        avg(&MARK_NS)
    );
}

fn event_first_listener_phase_counters_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_EVENT_FIRST_LISTENER_PHASE_COUNTERS")
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or(false)
    })
}

fn record_event_first_listener_phase(phase: &'static str, elapsed_ns: u64) {
    if !event_first_listener_phase_counters_enabled() {
        return;
    }
    static CALLS: AtomicU64 = AtomicU64::new(0);
    static HOOK_NS: AtomicU64 = AtomicU64::new(0);
    static EMPTY_NS: AtomicU64 = AtomicU64::new(0);
    static BAG_OBJ_NS: AtomicU64 = AtomicU64::new(0);
    static BAG_SET_NS: AtomicU64 = AtomicU64::new(0);
    static ALLOC_NS: AtomicU64 = AtomicU64::new(0);
    static PUBLISH_NS: AtomicU64 = AtomicU64::new(0);
    static TOTAL_NS: AtomicU64 = AtomicU64::new(0);

    let bucket = match phase {
        "hook" => &HOOK_NS,
        "empty" => &EMPTY_NS,
        "bag_obj" => &BAG_OBJ_NS,
        "bag_set" => &BAG_SET_NS,
        "alloc" => &ALLOC_NS,
        "publish" => &PUBLISH_NS,
        "total" => &TOTAL_NS,
        _ => return,
    };
    bucket.fetch_add(elapsed_ns, Ordering::Relaxed);
    if phase != "total" {
        return;
    }
    let calls = CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    if !calls.is_power_of_two() {
        return;
    }
    let avg = |ns: &AtomicU64| ns.load(Ordering::Relaxed) / calls.max(1);
    eprintln!(
        "[event-first-listener-phase] calls={} avg_total_ns={} avg_hook_ns={} avg_empty_ns={} avg_bag_obj_ns={} avg_bag_set_ns={} avg_alloc_ns={} avg_publish_ns={}",
        calls,
        avg(&TOTAL_NS),
        avg(&HOOK_NS),
        avg(&EMPTY_NS),
        avg(&BAG_OBJ_NS),
        avg(&BAG_SET_NS),
        avg(&ALLOC_NS),
        avg(&PUBLISH_NS)
    );
}

fn this_emitter(rt: &Runtime) -> Option<rusty_js_runtime::value::ObjectRef> {
    match rt.current_this() {
        Value::Object(id) => Some(id),
        _ => None,
    }
}

fn event_name(v: &Value) -> Cow<'_, str> {
    match v {
        Value::String(s) => Cow::Borrowed(s.as_str()),
        Value::Symbol(s) => Cow::Owned(format!("@@event-symbol:{}", s.as_str())),
        other => Cow::Owned(abstract_ops::to_string(other).as_str().to_string()),
    }
}

const ERROR_MONITOR_KEY: &str = "@@event-symbol:events.errorMonitor";

fn emit_error_monitor(rt: &mut Runtime, em: rusty_js_runtime::value::ObjectRef, extra: &[Value]) {
    let Some(bag) = existing_listeners(rt, em) else {
        return;
    };
    let list = listener_bag_get(rt, bag, ERROR_MONITOR_KEY);

    let listeners: Vec<Value> = match list {
        Value::Object(arr)
            if matches!(
                rt.obj(arr).internal_kind,
                rusty_js_runtime::value::InternalKind::Array
            ) =>
        {
            let n = rt.array_length(arr);
            (0..n).map(|i| rt.object_get(arr, &i.to_string())).collect()
        }
        Value::Undefined | Value::Null => return,
        other => vec![other],
    };
    if listeners.is_empty() {
        return;
    }
    let mut survivors: Vec<Value> = Vec::new();
    for l in listeners {

        let once = match &l {
            Value::Object(id) => rt.object_get(*id, "__once"),
            _ => Value::Undefined,
        };
        let is_once = rt.is_callable(&once);
        let target = if is_once { once } else { l.clone() };
        if rt.is_callable(&target) {
            let _ = rt.call_function(target, Value::Object(em), extra.to_vec());
        }
        if !is_once {
            survivors.push(l);
        }
    }

    let arr = rt.alloc_object(RtObject::new_array());
    for (i, l) in survivors.iter().enumerate() {
        rt.object_set(arr, i.to_string(), l.clone());
    }
    rt.object_set(arr, "length".into(), Value::Number(survivors.len() as f64));
    listener_bag_set(rt, bag, ERROR_MONITOR_KEY, Value::Object(arr));
}

const SYMBOL_EVENT_KEY_PREFIX: &str = "@@event-symbol:";
const SYMBOL_KEY_REGISTRY_SLOT: &str = "__cruft_symbol_event_keys";

fn record_symbol_event_key(rt: &mut Runtime, em: rusty_js_runtime::value::ObjectRef, raw: &Value) {
    if !matches!(raw, Value::Symbol(_)) {
        return;
    }
    let key = event_name(raw).into_owned();
    let reg = match rt.object_get(em, SYMBOL_KEY_REGISTRY_SLOT) {
        Value::Object(id) => id,
        _ => {
            let id = new_object(rt);
            rt.set_engine_sentinel(em, SYMBOL_KEY_REGISTRY_SLOT, Value::Object(id));
            id
        }
    };
    rt.object_set(reg, key.into(), raw.clone());
}

fn get_or_create_listeners(
    rt: &mut Runtime,
    emitter: rusty_js_runtime::value::ObjectRef,
) -> rusty_js_runtime::value::ObjectRef {
    get_or_create_listeners_with_created(rt, emitter).0
}

fn get_or_create_listeners_with_created(
    rt: &mut Runtime,
    emitter: rusty_js_runtime::value::ObjectRef,
) -> (rusty_js_runtime::value::ObjectRef, bool) {
    if event_on_phase_counters_enabled() {
        let lookup_start = std::time::Instant::now();
        if let Value::Object(id) = rt.object_get(emitter, "__listeners") {
            record_event_on_phase("bag_lookup", lookup_start.elapsed().as_nanos() as u64);
            return (id, false);
        }
        if let Value::Object(id) = rt.object_get(emitter, "_events") {
            record_event_on_phase("bag_lookup", lookup_start.elapsed().as_nanos() as u64);
            let publish_start = std::time::Instant::now();
            rt.object_set(emitter, "__listeners".into(), Value::Object(id));
            record_event_on_phase("bag_publish", publish_start.elapsed().as_nanos() as u64);
            return (id, false);
        }
        record_event_on_phase("bag_lookup", lookup_start.elapsed().as_nanos() as u64);
        let alloc_start = std::time::Instant::now();
        let bag = rt.alloc_object(RtObject::new_dictionary_with_property_capacity(1));
        record_event_on_phase("bag_alloc", alloc_start.elapsed().as_nanos() as u64);
        let publish_start = std::time::Instant::now();
        {
            let obj = rt.obj_mut(emitter);
            obj.set_own("_events".into(), Value::Object(bag));
            obj.set_own("_eventsCount".into(), Value::Number(0.0));
            obj.set_own_internal("__listeners".into(), Value::Object(bag));
        }
        record_event_on_phase("bag_publish", publish_start.elapsed().as_nanos() as u64);
        return (bag, true);
    }
    if let Value::Object(id) = rt.object_get(emitter, "__listeners") {
        return (id, false);
    }
    if let Value::Object(id) = rt.object_get(emitter, "_events") {
        rt.object_set(emitter, "__listeners".into(), Value::Object(id));
        return (id, false);
    }

    let bag = rt.alloc_object(RtObject::new_dictionary_with_property_capacity(1));
    {
        let obj = rt.obj_mut(emitter);
        obj.set_own("_events".into(), Value::Object(bag));
        obj.set_own("_eventsCount".into(), Value::Number(0.0));
        obj.set_own_internal("__listeners".into(), Value::Object(bag));
    }
    (bag, true)
}

fn sync_events_count(rt: &mut Runtime, emitter: rusty_js_runtime::value::ObjectRef) {
    let bag = match rt.object_get(emitter, "__listeners") {
        Value::Object(id) => id,
        _ => match rt.object_get(emitter, "_events") {
            Value::Object(id) => id,
            _ => return,
        },
    };
    let count = rt
        .ordinary_own_enumerable_string_keys(bag)
        .into_iter()
        .filter(|k| listener_count_for_value(rt, &listener_bag_get(rt, bag, k)) > 0)
        .count();
    rt.obj_mut(emitter)
        .set_own("_eventsCount".into(), Value::Number(count as f64));
}

fn has_new_listener_hook(rt: &Runtime, emitter: rusty_js_runtime::value::ObjectRef) -> bool {
    matches!(
        rt.obj(emitter)
            .get_own("__has_new_listener")
            .map(|d| &d.value),
        Some(Value::Boolean(true))
    )
}

fn try_install_first_listener_bag(
    rt: &mut Runtime,
    emitter: rusty_js_runtime::value::ObjectRef,
    event: &str,
    listener: Value,
) -> bool {
    if event_first_listener_phase_counters_enabled() {
        let total_start = std::time::Instant::now();
        let hook_start = std::time::Instant::now();
        if event == "newListener" || has_new_listener_hook(rt, emitter) {
            record_event_first_listener_phase("hook", hook_start.elapsed().as_nanos() as u64);
            return false;
        }
        record_event_first_listener_phase("hook", hook_start.elapsed().as_nanos() as u64);

        let empty_start = std::time::Instant::now();
        if !matches!(rt.object_get(emitter, "__listeners"), Value::Undefined)
            || !matches!(rt.object_get(emitter, "_events"), Value::Undefined)
        {
            record_event_first_listener_phase("empty", empty_start.elapsed().as_nanos() as u64);
            return false;
        }
        record_event_first_listener_phase("empty", empty_start.elapsed().as_nanos() as u64);

        let bag_obj_start = std::time::Instant::now();
        let mut bag_obj = RtObject::new_dictionary_with_property_capacity(1);
        record_event_first_listener_phase("bag_obj", bag_obj_start.elapsed().as_nanos() as u64);

        let bag_set_start = std::time::Instant::now();
        bag_obj.set_own(event.to_string(), listener);
        record_event_first_listener_phase("bag_set", bag_set_start.elapsed().as_nanos() as u64);

        let alloc_start = std::time::Instant::now();
        let bag = rt.alloc_object(bag_obj);
        record_event_first_listener_phase("alloc", alloc_start.elapsed().as_nanos() as u64);

        let publish_start = std::time::Instant::now();
        {
            let obj = rt.obj_mut(emitter);
            obj.set_own_literal_key("_events", Value::Object(bag));
            obj.set_own_literal_key("_eventsCount", Value::Number(1.0));
            obj.set_own_internal("__listeners".into(), Value::Object(bag));
        }
        record_event_first_listener_phase("publish", publish_start.elapsed().as_nanos() as u64);
        record_event_first_listener_phase("total", total_start.elapsed().as_nanos() as u64);
        return true;
    }

    if event == "newListener" || has_new_listener_hook(rt, emitter) {
        return false;
    }
    if !matches!(rt.object_get(emitter, "__listeners"), Value::Undefined)
        || !matches!(rt.object_get(emitter, "_events"), Value::Undefined)
    {
        return false;
    }

    let mut bag_obj = RtObject::new_dictionary_with_property_capacity(1);
    bag_obj.set_own(event.to_string(), listener);
    let bag = rt.alloc_object(bag_obj);
    {
        let obj = rt.obj_mut(emitter);
        obj.set_own_literal_key("_events", Value::Object(bag));
        obj.set_own_literal_key("_eventsCount", Value::Number(1.0));
        obj.set_own_internal("__listeners".into(), Value::Object(bag));
    }
    true
}

fn existing_listeners(
    rt: &Runtime,
    emitter: rusty_js_runtime::value::ObjectRef,
) -> Option<rusty_js_runtime::value::ObjectRef> {
    match rt.object_get(emitter, "__listeners") {
        Value::Object(id) => Some(id),
        _ => None,
    }
}

fn listener_bag_get(rt: &Runtime, bag: rusty_js_runtime::value::ObjectRef, event: &str) -> Value {
    rt.obj(bag)
        .get_own(event)
        .map(|d| d.value.clone())
        .unwrap_or(Value::Undefined)
}

fn listener_bag_set(
    rt: &mut Runtime,
    bag: rusty_js_runtime::value::ObjectRef,
    event: &str,
    value: Value,
) {
    rt.obj_mut(bag).set_own(event.to_string(), value);
}

fn listener_count_for_value(rt: &mut Runtime, v: &Value) -> usize {
    match v {
        Value::Object(id)
            if matches!(
                rt.obj(*id).internal_kind,
                rusty_js_runtime::value::InternalKind::Array
            ) =>
        {
            rt.array_length(*id)
        }
        Value::Object(_) => 1,
        _ => 0,
    }
}

fn listener_matches(rt: &Runtime, item: &Value, target: &Value) -> bool {
    match (item, target) {
        (Value::Object(a), Value::Object(b)) if a == b => true,
        (Value::Object(id), Value::Object(b)) => match once_inner(rt, item) {
            Some(Value::Object(inner)) => inner == *b || *id == *b,
            _ => false,
        },
        _ => false,
    }
}

fn listener_count_for_target(rt: &mut Runtime, v: &Value, target: &Value) -> usize {
    match v {
        Value::Object(id)
            if matches!(
                rt.obj(*id).internal_kind,
                rusty_js_runtime::value::InternalKind::Array
            ) =>
        {
            let len = rt.array_length(*id);
            (0..len)
                .filter(|i| {
                    let item = rt.object_get(*id, &i.to_string());
                    listener_matches(rt, &item, target)
                })
                .count()
        }
        _ if listener_matches(rt, v, target) => 1,
        _ => 0,
    }
}

fn event_target_listener_count(
    rt: &mut Runtime,
    target: rusty_js_runtime::value::ObjectRef,
    event: &str,
) -> usize {
    let Value::Object(bag) = rt.object_get(target, "__et_listeners") else {
        return 0;
    };
    match rt.object_get(bag, event) {
        Value::Object(arr)
            if matches!(
                rt.obj(arr).internal_kind,
                rusty_js_runtime::value::InternalKind::Array
            ) =>
        {
            rt.array_length(arr)
        }
        _ => 0,
    }
}

fn once_inner(rt: &Runtime, item: &Value) -> Option<Value> {
    if let Value::Object(id) = item {
        let inner = rt.object_get(*id, "__once");
        if !matches!(inner, Value::Undefined) {
            return Some(inner);
        }
    }
    None
}

fn listeners_array_for_value(
    rt: &mut Runtime,
    src_v: Value,
    unwrap_once: bool,
) -> rusty_js_runtime::value::ObjectRef {
    let out = rt.alloc_object(RtObject::new_array());
    if !matches!(
        src_v,
        Value::Object(a)
            if matches!(
                rt.obj(a).internal_kind,
                rusty_js_runtime::value::InternalKind::Array
            )
    ) {
        if let Some(unwrapped) = once_inner(rt, &src_v).filter(|_| unwrap_once) {
            rt.object_set(out, "0".into(), unwrapped);
            rt.object_set(out, "length".into(), Value::Number(1.0));
        } else if rt.is_callable(&src_v) || once_inner(rt, &src_v).is_some() {
            rt.object_set(out, "0".into(), src_v);
            rt.object_set(out, "length".into(), Value::Number(1.0));
        } else {
            rt.object_set(out, "length".into(), Value::Number(0.0));
        }
        return out;
    }
    let src = if let Value::Object(src) = src_v {
        src
    } else {
        unreachable!()
    };
    let len = rt.array_length(src);
    for i in 0..len {
        let item = rt.object_get(src, &i.to_string());
        let item = if unwrap_once {
            once_inner(rt, &item).unwrap_or(item)
        } else {
            item
        };
        rt.object_set(out, i.to_string(), item);
    }
    rt.object_set(out, "length".into(), Value::Number(len as f64));
    out
}

fn max_listeners(rt: &Runtime, em: rusty_js_runtime::value::ObjectRef) -> f64 {
    match rt.object_get(em, "__max_listeners") {
        Value::Number(n) => n,
        _ if is_abort_signal_like(rt, em) => 0.0,
        _ => 10.0,
    }
}

fn is_abort_signal_like(rt: &Runtime, id: rusty_js_runtime::value::ObjectRef) -> bool {
    matches!(rt.object_get(id, "aborted"), Value::Boolean(_))
        && matches!(rt.object_get(id, "__ac_listeners__"), Value::Object(_))
}

fn set_max_listeners(rt: &mut Runtime, em: rusty_js_runtime::value::ObjectRef, value: Value) {
    let n = match value {
        Value::Number(n) => n,
        _ => 10.0,
    };
    rt.object_set(em, "__max_listeners".into(), Value::Number(n));
}

fn empty_listener_array(rt: &mut Runtime) -> rusty_js_runtime::value::ObjectRef {
    let arr = rt.alloc_object(RtObject::new_array());
    rt.object_set(arr, "length".into(), Value::Number(0.0));
    arr
}

fn append_listener(rt: &mut Runtime, arr: rusty_js_runtime::value::ObjectRef, fn_v: Value) {
    let len = rt.array_length(arr);
    rt.object_set(arr, len.to_string(), fn_v);
    rt.object_set(arr, "length".into(), Value::Number((len + 1) as f64));
}

fn listener_pair_array(
    rt: &mut Runtime,
    first: Value,
    second: Value,
) -> rusty_js_runtime::value::ObjectRef {
    let mut arr = RtObject::new_array();
    arr.array_dense = true;
    arr.dense_elements.push(first);
    arr.dense_elements.push(second);
    rt.alloc_object(arr)
}

fn append_listener_for_event(
    rt: &mut Runtime,
    bag: rusty_js_runtime::value::ObjectRef,
    event: &str,
    fn_v: Value,
) {
    match listener_bag_get(rt, bag, event) {
        Value::Object(arr)
            if matches!(
                rt.obj(arr).internal_kind,
                rusty_js_runtime::value::InternalKind::Array
            ) =>
        {
            append_listener(rt, arr, fn_v);
        }
        Value::Undefined => {
            listener_bag_set(rt, bag, event, fn_v);
        }
        existing => {
            let arr = listener_pair_array(rt, existing, fn_v);
            listener_bag_set(rt, bag, event, Value::Object(arr));
        }
    }
}

fn check_max_listeners_warning(
    rt: &mut Runtime,
    em: rusty_js_runtime::value::ObjectRef,
    event: &str,
) {
    let max = max_listeners(rt, em);
    if max <= 0.0 || !max.is_finite() {
        return;
    }
    let Some(bag) = existing_listeners(rt, em) else {
        return;
    };
    let cur = listener_bag_get(rt, bag, event);
    let count = listener_count_for_value(rt, &cur);
    if (count as f64) <= max {
        return;
    }
    let warned_key = format!("__maxwarn:{event}");
    if matches!(rt.object_get(em, &warned_key), Value::Boolean(true)) {
        return;
    }
    rt.set_engine_sentinel(em, &warned_key, Value::Boolean(true));
    let ctor_name = match rt.object_get(em, "constructor") {
        Value::Object(c) => match rt.object_get(c, "name") {
            Value::String(s) if !s.as_str().is_empty() => s.as_str().to_string(),
            _ => "EventEmitter".to_string(),
        },
        _ => "EventEmitter".to_string(),
    };
    let msg = format!(
        "Possible EventEmitter memory leak detected. {count} {event} listeners added to [{ctor_name}]. MaxListeners is {max}. Use emitter.setMaxListeners() to increase limit"
    );
    let ctor = rt.global_get("Error");
    let warning = match rt.construct(ctor, vec![js_string(&msg)]) {
        Ok(Value::Object(id)) => id,
        _ => return,
    };
    rt.object_set(
        warning,
        "name".into(),
        js_string("MaxListenersExceededWarning"),
    );
    rt.object_set(warning, "emitter".into(), Value::Object(em));
    rt.object_set(warning, "type".into(), js_string(event));
    rt.object_set(warning, "count".into(), Value::Number(count as f64));

    if let Value::Object(pid) = rt.global_get("process") {
        rt.enqueue_host_phase_rooted(
            HostEnqueuePhase::EventSemanticMacrotask,
            "events.maxListenersWarning",
            vec![pid, warning],
            move |rt| {
                let emit = rt.object_get(pid, "emit");
                if rt.is_callable(&emit) {
                    let _ = rt.call_function(
                        emit,
                        Value::Object(pid),
                        vec![js_string("warning"), Value::Object(warning)],
                    );
                }
                Ok(())
            },
        );
    }
}

fn emit_new_listener(
    rt: &mut Runtime,
    emitter: rusty_js_runtime::value::ObjectRef,
    event: &str,
    listener: Value,
) -> Result<(), RuntimeError> {
    if event == "newListener"
        || !matches!(
            rt.obj(emitter)
                .get_own("__has_new_listener")
                .map(|d| &d.value),
            Some(Value::Boolean(true))
        )
    {
        return Ok(());
    }
    let Some(bag) = existing_listeners(rt, emitter) else {
        return Ok(());
    };
    let hooks = listener_bag_get(rt, bag, "newListener");
    if rt.is_callable(&hooks) {
        let event = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(event)));
        let _ = rt.call_function(hooks, Value::Object(emitter), vec![event, listener])?;
        return Ok(());
    }
    let Value::Object(arr) = hooks else {
        return Ok(());
    };
    if !matches!(
        rt.obj(arr).internal_kind,
        rusty_js_runtime::value::InternalKind::Array
    ) {
        return Ok(());
    }
    let len = rt.array_length(arr);
    let event_value = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(event)));
    let mut to_call = Vec::new();
    for i in 0..len {
        let hook = rt.object_get(arr, &i.to_string());
        if rt.is_callable(&hook) {
            to_call.push(hook);
        }
    }
    for hook in to_call {
        let _ = rt.call_function(
            hook,
            Value::Object(emitter),
            vec![event_value.clone(), listener.clone()],
        )?;
    }
    Ok(())
}

fn mark_new_listener_hook(
    rt: &mut Runtime,
    emitter: rusty_js_runtime::value::ObjectRef,
    event: &str,
) {
    if event == "newListener" {
        rt.obj_mut(emitter)
            .set_own_internal("__has_new_listener".into(), Value::Boolean(true));
    }
}

fn prepend_listener_for_event(
    rt: &mut Runtime,
    bag: rusty_js_runtime::value::ObjectRef,
    event: &str,
    fn_v: Value,
) {
    match listener_bag_get(rt, bag, event) {
        Value::Object(arr)
            if matches!(
                rt.obj(arr).internal_kind,
                rusty_js_runtime::value::InternalKind::Array
            ) =>
        {
            let len = rt.array_length(arr);
            for i in (0..len).rev() {
                let v = rt.object_get(arr, &i.to_string());
                rt.object_set(arr, (i + 1).to_string(), v);
            }
            rt.object_set(arr, "0".into(), fn_v);
            rt.object_set(arr, "length".into(), Value::Number((len + 1) as f64));
        }
        Value::Undefined => {
            listener_bag_set(rt, bag, event, fn_v);
        }
        existing => {
            let arr = listener_pair_array(rt, fn_v, existing);
            listener_bag_set(rt, bag, event, Value::Object(arr));
        }
    }
}

fn build_core_emitter(rt: &mut Runtime) -> rusty_js_runtime::value::ObjectRef {
    let proto = new_object(rt);

    register_method(rt, proto, "on", |rt, args| {
        let phase_enabled = event_on_phase_counters_enabled();
        let total_t0 = phase_enabled.then(std::time::Instant::now);
        let em = this_emitter(rt)
            .ok_or_else(|| RuntimeError::TypeError("on: this is not an EventEmitter".into()))?;
        let phase_t0 = phase_enabled.then(std::time::Instant::now);
        let event = event_name(args.first().unwrap_or(&Value::Undefined));
        record_symbol_event_key(rt, em, args.first().unwrap_or(&Value::Undefined));
        if let Some(t0) = phase_t0 {
            record_event_on_phase("event", t0.elapsed().as_nanos() as u64);
        }
        let fn_v = args.get(1).cloned().unwrap_or(Value::Undefined);
        let phase_t0 = phase_enabled.then(std::time::Instant::now);
        if try_install_first_listener_bag(rt, em, &event, fn_v.clone()) {
            if let Some(t0) = phase_t0 {
                record_event_on_phase("bag", t0.elapsed().as_nanos() as u64);
            }
            let phase_t0 = phase_enabled.then(std::time::Instant::now);
            mark_new_listener_hook(rt, em, &event);
            check_max_listeners_warning(rt, em, &event);
            if let Some(t0) = phase_t0 {
                record_event_on_phase("mark", t0.elapsed().as_nanos() as u64);
            }
            if let Some(t0) = total_t0 {
                record_event_on_phase("total", t0.elapsed().as_nanos() as u64);
            }
            sync_events_count(rt, em);
            return Ok(Value::Object(em));
        }
        let phase_t0 = phase_enabled.then(std::time::Instant::now);
        let (bag, created_bag) = get_or_create_listeners_with_created(rt, em);
        if let Some(t0) = phase_t0 {
            record_event_on_phase("bag", t0.elapsed().as_nanos() as u64);
        }
        let phase_t0 = phase_enabled.then(std::time::Instant::now);
        emit_new_listener(rt, em, &event, fn_v.clone())?;
        if let Some(t0) = phase_t0 {
            record_event_on_phase("new_listener", t0.elapsed().as_nanos() as u64);
        }
        let phase_t0 = phase_enabled.then(std::time::Instant::now);
        if created_bag {
            listener_bag_set(rt, bag, &event, fn_v);
        } else {
            append_listener_for_event(rt, bag, &event, fn_v);
        }
        if let Some(t0) = phase_t0 {
            record_event_on_phase("append", t0.elapsed().as_nanos() as u64);
        }
        let phase_t0 = phase_enabled.then(std::time::Instant::now);
        mark_new_listener_hook(rt, em, &event);
        check_max_listeners_warning(rt, em, &event);
        if let Some(t0) = phase_t0 {
            record_event_on_phase("mark", t0.elapsed().as_nanos() as u64);
        }
        if let Some(t0) = total_t0 {
            record_event_on_phase("total", t0.elapsed().as_nanos() as u64);
        }
        sync_events_count(rt, em);
        Ok(Value::Object(em))
    });
    register_method(rt, proto, "addListener", |rt, args| {
        let em = this_emitter(rt).ok_or_else(|| {
            RuntimeError::TypeError("addListener: this is not an EventEmitter".into())
        })?;
        let event = event_name(args.first().unwrap_or(&Value::Undefined));
        record_symbol_event_key(rt, em, args.first().unwrap_or(&Value::Undefined));
        let fn_v = args.get(1).cloned().unwrap_or(Value::Undefined);
        let bag = get_or_create_listeners(rt, em);
        emit_new_listener(rt, em, &event, fn_v.clone())?;
        append_listener_for_event(rt, bag, &event, fn_v);
        mark_new_listener_hook(rt, em, &event);
        check_max_listeners_warning(rt, em, &event);
        sync_events_count(rt, em);
        Ok(Value::Object(em))
    });
    register_method(rt, proto, "once", |rt, args| {
        let em = this_emitter(rt)
            .ok_or_else(|| RuntimeError::TypeError("once: this is not an EventEmitter".into()))?;
        let event_raw = args.first().cloned().unwrap_or(Value::Undefined);
        let event = event_name(&event_raw);
        record_symbol_event_key(rt, em, &event_raw);
        let fn_v = args.get(1).cloned().unwrap_or(Value::Undefined);

        let cell = std::rc::Rc::new(std::cell::Cell::new(None::<rusty_js_runtime::ObjectRef>));
        let cell2 = cell.clone();
        let event_for_remove = event_raw.clone();
        let fn_for_call = fn_v.clone();
        let wrapper = crate::register::make_callable_with_length_rooted(
            rt,
            "bound onceWrapper",
            0,
            vec![em],
            move |rt, cargs| {
                if let Some(w) = cell2.get() {
                    let rm = rt.object_get(em, "removeListener");
                    if rt.is_callable(&rm) {
                        let _ = rt.call_function(
                            rm,
                            Value::Object(em),
                            vec![event_for_remove.clone(), Value::Object(w)],
                        );
                    }
                }
                let this = rt.current_this();
                rt.call_function(fn_for_call.clone(), this, cargs.to_vec())
            },
        );
        cell.set(Some(wrapper));
        rt.object_set(wrapper, "__once".into(), fn_v.clone());
        rt.object_set(wrapper, "listener".into(), fn_v.clone());
        let bag = get_or_create_listeners(rt, em);
        emit_new_listener(rt, em, &event, fn_v)?;
        append_listener_for_event(rt, bag, &event, Value::Object(wrapper));
        mark_new_listener_hook(rt, em, &event);
        check_max_listeners_warning(rt, em, &event);
        sync_events_count(rt, em);
        Ok(Value::Object(em))
    });
    register_method(rt, proto, "off", |rt, args| {
        let em = this_emitter(rt)
            .ok_or_else(|| RuntimeError::TypeError("off: this is not an EventEmitter".into()))?;
        let event = event_name(args.first().unwrap_or(&Value::Undefined));
        let target = args.get(1).cloned().unwrap_or(Value::Undefined);
        let bag = get_or_create_listeners(rt, em);
        let cur = listener_bag_get(rt, bag, &event);
        if !matches!(
            cur,
            Value::Object(id)
                if matches!(
                    rt.obj(id).internal_kind,
                    rusty_js_runtime::value::InternalKind::Array
                )
        ) {
            if listener_matches(rt, &cur, &target) {
                let arr = empty_listener_array(rt);
                listener_bag_set(rt, bag, &event, Value::Object(arr));
                emit_remove_listener(rt, em, &js_string(&event), &target);
            }
            sync_events_count(rt, em);
            return Ok(Value::Object(em));
        }
        let arr = if let Value::Object(arr) = cur {
            arr
        } else {
            unreachable!()
        };
        let len = rt.array_length(arr);

        for i in 0..len {
            let item = rt.object_get(arr, &i.to_string());
            if listener_matches(rt, &item, &target) {

                for j in i..(len - 1) {
                    let next = rt.object_get(arr, &(j + 1).to_string());
                    rt.object_set(arr, j.to_string(), next);
                }
                rt.object_set(arr, (len - 1).to_string(), Value::Undefined);
                rt.object_set(arr, "length".into(), Value::Number((len - 1) as f64));
                emit_remove_listener(rt, em, &js_string(&event), &target);
                break;
            }
        }
        sync_events_count(rt, em);
        Ok(Value::Object(em))
    });
    register_method(rt, proto, "removeListener", |rt, args| {

        let em = this_emitter(rt).ok_or_else(|| {
            RuntimeError::TypeError("removeListener: this is not an EventEmitter".into())
        })?;
        let event = event_name(args.first().unwrap_or(&Value::Undefined));
        let target = args.get(1).cloned().unwrap_or(Value::Undefined);
        let bag = get_or_create_listeners(rt, em);
        let cur = listener_bag_get(rt, bag, &event);
        if !matches!(
            cur,
            Value::Object(id)
                if matches!(
                    rt.obj(id).internal_kind,
                    rusty_js_runtime::value::InternalKind::Array
                )
        ) {
            if listener_matches(rt, &cur, &target) {
                let arr = empty_listener_array(rt);
                listener_bag_set(rt, bag, &event, Value::Object(arr));
                emit_remove_listener(rt, em, &js_string(&event), &target);
            }
            sync_events_count(rt, em);
            return Ok(Value::Object(em));
        }
        let arr = if let Value::Object(arr) = cur {
            arr
        } else {
            unreachable!()
        };
        let len = rt.array_length(arr);
        for i in 0..len {
            let item = rt.object_get(arr, &i.to_string());
            if listener_matches(rt, &item, &target) {
                for j in i..(len - 1) {
                    let next = rt.object_get(arr, &(j + 1).to_string());
                    rt.object_set(arr, j.to_string(), next);
                }
                rt.object_set(arr, (len - 1).to_string(), Value::Undefined);
                rt.object_set(arr, "length".into(), Value::Number((len - 1) as f64));
                emit_remove_listener(rt, em, &js_string(&event), &target);
                break;
            }
        }
        sync_events_count(rt, em);
        Ok(Value::Object(em))
    });
    register_method(rt, proto, "removeAllListeners", |rt, args| {
        let em = this_emitter(rt).ok_or_else(|| {
            RuntimeError::TypeError("removeAllListeners: this is not an EventEmitter".into())
        })?;
        let bag = get_or_create_listeners(rt, em);

        let has_rl = listener_count_for_value(rt, &listener_bag_get(rt, bag, "removeListener")) > 0;
        if let Some(ev) = args.first() {
            let event = event_name(ev).into_owned();
            if has_rl && event != "removeListener" {
                emit_removed_for_key(rt, em, bag, &event, ev);
            }
            let arr = rt.alloc_object(RtObject::new_array());
            rt.object_set(arr, "length".into(), Value::Number(0.0));
            listener_bag_set(rt, bag, &event, Value::Object(arr));
        } else {
            if has_rl {

                let keys = rt.ordinary_own_enumerable_string_keys(bag);
                let registry = match rt.object_get(em, SYMBOL_KEY_REGISTRY_SLOT) {
                    Value::Object(id) => Some(id),
                    _ => None,
                };
                for k in keys {
                    if k == "removeListener" {
                        continue;
                    }
                    let ev_val = if k.starts_with(SYMBOL_EVENT_KEY_PREFIX) {
                        registry
                            .map(|r| rt.object_get(r, &k))
                            .filter(|v| matches!(v, Value::Symbol(_)))
                            .unwrap_or_else(|| js_string(&k))
                    } else {
                        js_string(&k)
                    };
                    emit_removed_for_key(rt, em, bag, &k, &ev_val);
                }
            }

            let new_bag = rt.alloc_object(RtObject::new_dictionary_with_property_capacity(1));
            let obj = rt.obj_mut(em);
            obj.set_own("_events".into(), Value::Object(new_bag));
            obj.set_own("_eventsCount".into(), Value::Number(0.0));
            obj.set_own_internal("__listeners".into(), Value::Object(new_bag));
        }
        sync_events_count(rt, em);
        Ok(Value::Object(em))
    });
    register_method(rt, proto, "emit", |rt, args| {
        let phase_enabled = event_emit_phase_counters_enabled();
        let total_t0 = phase_enabled.then(std::time::Instant::now);
        let em = this_emitter(rt)
            .ok_or_else(|| RuntimeError::TypeError("emit: this is not an EventEmitter".into()))?;
        let phase_t0 = phase_enabled.then(std::time::Instant::now);
        let event = event_name(args.first().unwrap_or(&Value::Undefined));
        let event_value = args.first().cloned().unwrap_or(Value::Undefined);
        if let Some(t0) = phase_t0 {
            record_event_emit_phase("event", t0.elapsed().as_nanos() as u64);
        }

        if event == "error" {
            emit_error_monitor(rt, em, args.get(1..).unwrap_or(&[]));
        }
        let argc = args.len().saturating_sub(1);
        let phase_t0 = phase_enabled.then(std::time::Instant::now);
        let Some(bag) = existing_listeners(rt, em) else {
            if let Some(t0) = phase_t0 {
                record_event_emit_phase("bag", t0.elapsed().as_nanos() as u64);
            }
            record_event_emit_listener_path("no-bag", argc, 0);
            if event == "error" {
                let errv = match args.get(1) {
                    Some(v @ Value::Object(_)) => v.clone(),
                    _ => {
                        let ctor = rt.global_get("Error");
                        let msg = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                            "Unhandled error.",
                        )));
                        rt.construct(ctor, vec![msg]).unwrap_or(Value::Undefined)
                    }
                };
                return Err(RuntimeError::Thrown(errv));
            }
            if let Some(t0) = total_t0 {
                record_event_emit_phase("total", t0.elapsed().as_nanos() as u64);
            }
            return Ok(Value::Boolean(false));
        };
        if let Some(t0) = phase_t0 {
            record_event_emit_phase("bag", t0.elapsed().as_nanos() as u64);
        }
        let phase_t0 = phase_enabled.then(std::time::Instant::now);
        let event_list = listener_bag_get(rt, bag, &event);
        if let Some(t0) = phase_t0 {
            record_event_emit_phase("list", t0.elapsed().as_nanos() as u64);
        }
        if !matches!(
            event_list,
            Value::Object(a)
                if matches!(
                    rt.obj(a).internal_kind,
                    rusty_js_runtime::value::InternalKind::Array
                )
        ) {
            let direct_phase_enabled = event_emit_direct_phase_counters_enabled();
            let direct_total_t0 = direct_phase_enabled.then(std::time::Instant::now);
            let callable_t0 = direct_phase_enabled.then(std::time::Instant::now);
            let direct_is_callable = rt.is_callable(&event_list);
            let callable_ns = callable_t0
                .map(|t0| t0.elapsed().as_nanos() as u64)
                .unwrap_or(0);
            if direct_is_callable {
                if event_emit_direct_forward_probe_enabled() {
                    match rt.try_function_call_captured_add_store_forward(&event_list, &args[1..]) {
                        Some(Ok(_)) => {
                            record_event_emit_listener_path("direct-forward-probe-hit", argc, 1);
                            record_event_emit_direct_forward_probe("hit", argc);
                            if let Some(t0) = total_t0 {
                                record_event_emit_phase("total", t0.elapsed().as_nanos() as u64);
                            }
                            return Ok(Value::Boolean(true));
                        }
                        Some(Err(err)) => {
                            record_event_emit_direct_forward_probe("error", argc);
                            return Err(err);
                        }
                        None => {
                            record_event_emit_direct_forward_probe("miss", argc);
                        }
                    }
                }
                let phase_t0 = phase_enabled.then(std::time::Instant::now);
                let rest: Vec<Value> = args.iter().skip(1).cloned().collect();
                if let Some(t0) = phase_t0 {
                    record_event_emit_phase("rest", t0.elapsed().as_nanos() as u64);
                }
                let direct_overhead_enabled = event_emit_direct_overhead_counters_enabled();
                let path_t0 = direct_overhead_enabled.then(std::time::Instant::now);
                record_event_emit_listener_path("direct-callable", argc, 1);
                let path_ns = path_t0
                    .map(|t0| t0.elapsed().as_nanos() as u64)
                    .unwrap_or(0);
                let mut classify_ns = 0;
                let mut direct_reason = "not-classified";
                let direct_listener_counters = event_emit_direct_listener_counters_enabled();
                let direct_call_bucket_counters = event_emit_direct_call_bucket_counters_enabled();
                if direct_listener_counters || direct_call_bucket_counters {
                    let classify_t0 = direct_phase_enabled.then(std::time::Instant::now);
                    direct_reason = rt.classify_function_call_captured_add_store_forward(
                        &event_list,
                        rest.as_slice(),
                    );
                    classify_ns = classify_t0
                        .map(|t0| t0.elapsed().as_nanos() as u64)
                        .unwrap_or(0);
                    if direct_listener_counters {
                        record_event_emit_direct_listener_candidate(direct_reason, argc);
                    }
                }
                let phase_t0 = phase_enabled.then(std::time::Instant::now);
                let direct_call_t0 = direct_phase_enabled.then(std::time::Instant::now);
                let ret = rt.call_function(event_list, Value::Object(em), rest.clone())?;
                capture_listener_rejection(rt, em, &event_value, &rest, ret);
                let direct_call_ns = direct_call_t0
                    .map(|t0| t0.elapsed().as_nanos() as u64)
                    .unwrap_or(0);
                record_event_emit_direct_call_bucket(direct_reason, argc, direct_call_ns);
                let post_call_t0 = direct_overhead_enabled.then(std::time::Instant::now);
                if let Some(t0) = direct_total_t0 {
                    let total_ns = t0.elapsed().as_nanos() as u64;
                    let post_call_ns = post_call_t0
                        .map(|t0| t0.elapsed().as_nanos() as u64)
                        .unwrap_or(0);
                    record_event_emit_direct_overhead(
                        path_ns,
                        callable_ns,
                        classify_ns,
                        direct_call_ns,
                        post_call_ns,
                        total_ns,
                        argc,
                    );
                    record_event_emit_direct_phase(
                        callable_ns,
                        classify_ns,
                        direct_call_ns,
                        total_ns,
                        argc,
                        true,
                    );
                }
                if let Some(t0) = phase_t0 {
                    record_event_emit_phase("call", t0.elapsed().as_nanos() as u64);
                }
                if let Some(t0) = total_t0 {
                    record_event_emit_phase("total", t0.elapsed().as_nanos() as u64);
                }
                return Ok(Value::Boolean(true));
            }
            if let Some(inner) = once_inner(rt, &event_list) {
                let phase_t0 = phase_enabled.then(std::time::Instant::now);
                let rest: Vec<Value> = args.iter().skip(1).cloned().collect();
                if let Some(t0) = phase_t0 {
                    record_event_emit_phase("rest", t0.elapsed().as_nanos() as u64);
                }
                record_event_emit_listener_path("direct-once", argc, 1);
                let phase_t0 = phase_enabled.then(std::time::Instant::now);
                let ret = rt.call_function(inner.clone(), Value::Object(em), rest.clone())?;
                capture_listener_rejection(rt, em, &event_value, &rest, ret);
                if let Some(t0) = phase_t0 {
                    record_event_emit_phase("call", t0.elapsed().as_nanos() as u64);
                }
                let arr = empty_listener_array(rt);
                listener_bag_set(rt, bag, &event, Value::Object(arr));

                emit_remove_listener(rt, em, &event_value, &inner);
                if let Some(t0) = total_t0 {
                    record_event_emit_phase("total", t0.elapsed().as_nanos() as u64);
                }
                return Ok(Value::Boolean(true));
            }
            if event == "error" {
                let errv = match args.get(1) {
                    Some(v @ Value::Object(_)) => v.clone(),
                    _ => {
                        let ctor = rt.global_get("Error");
                        let msg = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                            "Unhandled error.",
                        )));
                        rt.construct(ctor, vec![msg]).unwrap_or(Value::Undefined)
                    }
                };
                return Err(RuntimeError::Thrown(errv));
            }
            record_event_emit_listener_path("direct-miss", argc, 0);
            if let Some(t0) = total_t0 {
                record_event_emit_phase("total", t0.elapsed().as_nanos() as u64);
            }
            return Ok(Value::Boolean(false));
        }
        let phase_t0 = phase_enabled.then(std::time::Instant::now);
        let rest: Vec<Value> = args.iter().skip(1).cloned().collect();
        if let Some(t0) = phase_t0 {
            record_event_emit_phase("rest", t0.elapsed().as_nanos() as u64);
        }
        let arr = if let Value::Object(arr) = event_list {
            arr
        } else {
            unreachable!()
        };
        let len = rt.array_length(arr);
        if len == 0 {
            record_event_emit_listener_path("array-empty", argc, 0);

            if event == "error" {
                let errv = match rest.first() {
                    Some(v @ Value::Object(_)) => v.clone(),
                    _ => {
                        let ctor = rt.global_get("Error");
                        let msg = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                            "Unhandled error.",
                        )));
                        rt.construct(ctor, vec![msg]).unwrap_or(Value::Undefined)
                    }
                };
                return Err(RuntimeError::Thrown(errv));
            }
            if let Some(t0) = total_t0 {
                record_event_emit_phase("total", t0.elapsed().as_nanos() as u64);
            }
            return Ok(Value::Boolean(false));
        }
        if len == 1 {
            let phase_t0 = phase_enabled.then(std::time::Instant::now);
            let item = rt.object_get(arr, "0");
            if let Some(t0) = phase_t0 {
                record_event_emit_phase("array", t0.elapsed().as_nanos() as u64);
            }

            let is_once_wrapper = matches!(&item,
                Value::Object(id) if rt.is_callable(&rt.object_get(*id, "__once")));
            if rt.is_callable(&item) && !is_once_wrapper {
                record_event_emit_listener_path("array-one-callable", argc, 1);
                let phase_t0 = phase_enabled.then(std::time::Instant::now);
                let _ = rt.call_function(item, Value::Object(em), rest)?;
                if let Some(t0) = phase_t0 {
                    record_event_emit_phase("call", t0.elapsed().as_nanos() as u64);
                }
                if let Some(t0) = total_t0 {
                    record_event_emit_phase("total", t0.elapsed().as_nanos() as u64);
                }
                return Ok(Value::Boolean(true));
            }
        }
        let phase_t0 = phase_enabled.then(std::time::Instant::now);
        let mut to_call: Vec<(Value, bool)> = Vec::new();
        for i in 0..len {
            let item = rt.object_get(arr, &i.to_string());

            let (fn_v, once) = match &item {
                Value::Object(id) if rt.is_callable(&rt.object_get(*id, "__once")) => {
                    (rt.object_get(*id, "__once"), true)
                }
                _ if rt.is_callable(&item) => (item.clone(), false),
                Value::Object(_) => (item.clone(), false),
                _ => continue,
            };
            to_call.push((fn_v, once));
        }
        record_event_emit_listener_path("array-multi-calls", argc, to_call.len() as u64);
        if let Some(t0) = phase_t0 {
            record_event_emit_phase("array", t0.elapsed().as_nanos() as u64);
        }

        for (fn_v, _once) in &to_call {
            let phase_t0 = phase_enabled.then(std::time::Instant::now);
            let ret = rt.call_function(fn_v.clone(), Value::Object(em), rest.clone())?;
            capture_listener_rejection(rt, em, &event_value, &rest, ret);
            if let Some(t0) = phase_t0 {
                record_event_emit_phase("call", t0.elapsed().as_nanos() as u64);
            }
        }

        if to_call.iter().any(|(_, once)| *once) {
            record_event_emit_listener_path("array-once", argc, to_call.len() as u64);

            let mut dropped_once: Vec<Value> = Vec::new();
            let keep: Vec<Value> = (0..len)
                .filter_map(|i| {
                    let item = rt.object_get(arr, &i.to_string());
                    if let Value::Object(id) = &item {
                        let inner = rt.object_get(*id, "__once");
                        if !matches!(inner, Value::Undefined) {
                            dropped_once.push(inner);
                            return None;
                        }
                    }
                    Some(item)
                })
                .collect();
            for (i, v) in keep.iter().enumerate() {
                rt.object_set(arr, i.to_string(), v.clone());
            }
            for i in keep.len()..(len as usize) {
                rt.object_set(arr, i.to_string(), Value::Undefined);
            }
            rt.object_set(arr, "length".into(), Value::Number(keep.len() as f64));
            for inner in &dropped_once {
                emit_remove_listener(rt, em, &event_value, inner);
            }
        }
        if let Some(t0) = total_t0 {
            record_event_emit_phase("total", t0.elapsed().as_nanos() as u64);
        }
        Ok(Value::Boolean(true))
    });
    register_method(rt, proto, "listenerCount", |rt, args| {
        let em = this_emitter(rt).ok_or_else(|| {
            RuntimeError::TypeError("listenerCount: this is not an EventEmitter".into())
        })?;
        let event = event_name(args.first().unwrap_or(&Value::Undefined));
        let bag = get_or_create_listeners(rt, em);
        let cur = listener_bag_get(rt, bag, &event);
        let n = if let Some(target) = args.get(1) {
            listener_count_for_target(rt, &cur, target)
        } else {
            listener_count_for_value(rt, &cur)
        };
        Ok(Value::Number(n as f64))
    });
    register_method(rt, proto, "listeners", |rt, args| {
        let em = this_emitter(rt).ok_or_else(|| {
            RuntimeError::TypeError("listeners: this is not an EventEmitter".into())
        })?;
        let event = event_name(args.first().unwrap_or(&Value::Undefined));
        let bag = get_or_create_listeners(rt, em);
        let src_v = listener_bag_get(rt, bag, &event);
        let out = listeners_array_for_value(rt, src_v, true);
        Ok(Value::Object(out))
    });
    register_method(rt, proto, "eventNames", |rt, _args| {
        let em = this_emitter(rt).ok_or_else(|| {
            RuntimeError::TypeError("eventNames: this is not an EventEmitter".into())
        })?;
        let bag = get_or_create_listeners(rt, em);
        let keys = rt.ordinary_own_enumerable_string_keys(bag);
        let registry = match rt.object_get(em, SYMBOL_KEY_REGISTRY_SLOT) {
            Value::Object(id) => Some(id),
            _ => None,
        };
        let arr = rt.alloc_object(RtObject::new_array());
        let mut n = 0usize;

        let live: Vec<String> = keys
            .into_iter()
            .filter(|k| listener_count_for_value(rt, &listener_bag_get(rt, bag, k)) > 0)
            .collect();
        for symbol_pass in [false, true] {
            for k in &live {
                let is_symbol = k.starts_with(SYMBOL_EVENT_KEY_PREFIX);
                if is_symbol != symbol_pass {
                    continue;
                }

                let value = if is_symbol {
                    registry
                        .map(|r| rt.object_get(r, k))
                        .filter(|v| matches!(v, Value::Symbol(_)))
                        .unwrap_or_else(|| {
                            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                                k.clone(),
                            )))
                        })
                } else {
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(k.clone())))
                };
                rt.object_set(arr, n.to_string(), value);
                n += 1;
            }
        }
        rt.object_set(arr, "length".into(), Value::Number(n as f64));
        Ok(Value::Object(arr))
    });
    register_method(rt, proto, "setMaxListeners", |rt, args| {
        let em = this_emitter(rt).ok_or_else(|| {
            RuntimeError::TypeError("setMaxListeners: this is not an EventEmitter".into())
        })?;
        let n = args.first().cloned().unwrap_or(Value::Number(10.0));
        set_max_listeners(rt, em, n);
        Ok(Value::Object(em))
    });
    register_method(rt, proto, "getMaxListeners", |rt, _args| {
        let em = this_emitter(rt).ok_or_else(|| {
            RuntimeError::TypeError("getMaxListeners: this is not an EventEmitter".into())
        })?;
        Ok(Value::Number(max_listeners(rt, em)))
    });
    register_method(rt, proto, "prependListener", |rt, args| {
        let em = this_emitter(rt).ok_or_else(|| {
            RuntimeError::TypeError("prependListener: this is not an EventEmitter".into())
        })?;
        let event = event_name(args.first().unwrap_or(&Value::Undefined));
        record_symbol_event_key(rt, em, args.first().unwrap_or(&Value::Undefined));
        let fn_v = args.get(1).cloned().unwrap_or(Value::Undefined);
        let bag = get_or_create_listeners(rt, em);
        emit_new_listener(rt, em, &event, fn_v.clone())?;
        prepend_listener_for_event(rt, bag, &event, fn_v);
        mark_new_listener_hook(rt, em, &event);
        check_max_listeners_warning(rt, em, &event);
        sync_events_count(rt, em);
        Ok(Value::Object(em))
    });
    register_method(rt, proto, "prependOnceListener", |rt, args| {
        let em = this_emitter(rt).ok_or_else(|| {
            RuntimeError::TypeError("prependOnceListener: this is not an EventEmitter".into())
        })?;
        let event = event_name(args.first().unwrap_or(&Value::Undefined));
        record_symbol_event_key(rt, em, args.first().unwrap_or(&Value::Undefined));
        let fn_v = args.get(1).cloned().unwrap_or(Value::Undefined);
        let wrapper = rt.alloc_object(RtObject::new_ordinary());
        rt.object_set(wrapper, "__once".into(), fn_v);
        rt.object_set(
            wrapper,
            "listener".into(),
            args.get(1).cloned().unwrap_or(Value::Undefined),
        );
        let bag = get_or_create_listeners(rt, em);
        emit_new_listener(
            rt,
            em,
            &event,
            args.get(1).cloned().unwrap_or(Value::Undefined),
        )?;
        prepend_listener_for_event(rt, bag, &event, Value::Object(wrapper));
        mark_new_listener_hook(rt, em, &event);
        check_max_listeners_warning(rt, em, &event);
        sync_events_count(rt, em);
        Ok(Value::Object(em))
    });
    register_method(rt, proto, "rawListeners", |rt, args| {

        let em = this_emitter(rt).ok_or_else(|| {
            RuntimeError::TypeError("rawListeners: this is not an EventEmitter".into())
        })?;
        let event = event_name(args.first().unwrap_or(&Value::Undefined));
        let bag = get_or_create_listeners(rt, em);
        let cur = listener_bag_get(rt, bag, &event);
        let out = listeners_array_for_value(rt, cur, false);
        Ok(Value::Object(out))
    });

    let proto_for_ctor = proto;
    let ctor = make_callable(rt, "EventEmitter", move |rt, args| {

        let capture = matches!(args.first(), Some(Value::Object(opts))
            if matches!(rt.object_get(*opts, "captureRejections"), Value::Boolean(true)));

        match rt.current_this() {
            Value::Object(id) => {
                let obj = rt.obj_mut(id);
                obj.set_own_internal("__cruft_event_emitter__".into(), Value::Boolean(true));
                if capture {
                    obj.set_own_internal(
                        "__cruft_capture_rejections__".into(),
                        Value::Boolean(true),
                    );
                }
                Ok(Value::Undefined)
            }
            _ => {

                let mut o = RtObject::new_ordinary();
                o.proto = Some(proto_for_ctor);
                o.set_own_internal("__cruft_event_emitter__".into(), Value::Boolean(true));
                if capture {
                    o.set_own_internal("__cruft_capture_rejections__".into(), Value::Boolean(true));
                }
                let id = rt.alloc_object(o);
                Ok(Value::Object(id))
            }
        }
    });
    rt.set_own_frozen_property(ctor, "prototype".into(), Value::Object(proto));
    rt.obj_mut(proto)
        .set_own_internal("constructor".into(), Value::Object(ctor));

    ctor
}

pub fn install_canonical(rt: &mut Runtime) {
    let ctor = build_core_emitter(rt);

    rt.object_set(ctor, "EventEmitter".into(), Value::Object(ctor));
    rt.object_set(ctor, "default".into(), Value::Object(ctor));
    rt.define_global_property("__cruft_events", Value::Object(ctor));
}

fn on_arr_len(rt: &mut Runtime, arr: rusty_js_runtime::ObjectRef) -> usize {
    match rt.object_get(arr, "length") {
        Value::Number(n) if n > 0.0 => n as usize,
        _ => 0,
    }
}
fn on_arr_push(rt: &mut Runtime, arr: rusty_js_runtime::ObjectRef, v: Value) {
    let push = rt.object_get(arr, "push");
    if rt.is_callable(&push) {
        let _ = rt.call_function(push, Value::Object(arr), vec![v]);
    }
}
fn on_arr_shift(rt: &mut Runtime, arr: rusty_js_runtime::ObjectRef) -> Value {
    let shift = rt.object_get(arr, "shift");
    if rt.is_callable(&shift) {
        rt.call_function(shift, Value::Object(arr), vec![])
            .unwrap_or(Value::Undefined)
    } else {
        Value::Undefined
    }
}
fn on_iter_result(rt: &mut Runtime, value: Value, done: bool) -> Value {
    let r = new_object(rt);
    rt.object_set(r, "value".into(), value);
    rt.object_set(r, "done".into(), Value::Boolean(done));
    Value::Object(r)
}
fn on_sentinel_obj(
    rt: &mut Runtime,
    iter: rusty_js_runtime::ObjectRef,
    key: &str,
) -> Option<rusty_js_runtime::ObjectRef> {
    match rt.object_get(iter, key) {
        Value::Object(o) => Some(o),
        _ => None,
    }
}

fn cleanup_events_once_state(rt: &mut Runtime, state: rusty_js_runtime::ObjectRef) -> bool {
    if matches!(rt.object_get(state, "__once_settled"), Value::Boolean(true)) {
        return false;
    }
    rt.set_engine_sentinel(state, "__once_settled", Value::Boolean(true));
    let Value::Object(emitter) = rt.object_get(state, "__once_emitter") else {
        return true;
    };
    let name = rt.object_get(state, "__once_name");
    let event_listener = rt.object_get(state, "__once_event_listener");
    let remove = rt.object_get(emitter, "removeListener");
    if rt.is_callable(&remove) {
        let _ = rt.call_function(
            remove.clone(),
            Value::Object(emitter),
            vec![name.clone(), event_listener.clone()],
        );
        let error_listener = rt.object_get(state, "__once_error_listener");
        if rt.is_callable(&error_listener) {
            let err_name = js_string("error");
            let _ = rt.call_function(
                remove,
                Value::Object(emitter),
                vec![err_name, error_listener],
            );
        }
    } else {
        let remove_event = rt.object_get(emitter, "removeEventListener");
        if rt.is_callable(&remove_event) {
            let _ = rt.call_function(
                remove_event,
                Value::Object(emitter),
                vec![name, event_listener],
            );
        }
    }
    let signal = rt.object_get(state, "__once_signal");
    let abort_listener = rt.object_get(state, "__once_abort_listener");
    if let (Value::Object(sig), listener) = (signal, abort_listener) {
        if rt.is_callable(&listener) {
            let remove_abort = rt.object_get(sig, "removeEventListener");
            if rt.is_callable(&remove_abort) {
                let _ = rt.call_function(
                    remove_abort,
                    Value::Object(sig),
                    vec![js_string("abort"), listener],
                );
            }
        }
    }
    true
}

fn events_once_error_value(rt: &mut Runtime, name: &str, code: &str, msg: &str) -> Value {
    let ctor = rt.global_get("Error");
    match rt.construct(ctor, vec![js_string(msg)]) {
        Ok(Value::Object(e)) => {
            rt.object_set(e, "name".into(), js_string(name));
            rt.object_set(e, "code".into(), js_string(code));
            Value::Object(e)
        }
        _ => js_string(msg),
    }
}

fn events_once_error_promise(rt: &mut Runtime, name: &str, code: &str, msg: &str) -> Value {
    use rusty_js_runtime::promise::new_promise;
    let p = new_promise(rt);
    rt.object_set(
        p,
        "__cruft_suppress_unhandled_rejection".into(),
        Value::Boolean(true),
    );
    let err = events_once_error_value(rt, name, code, msg);
    rt.enqueue_host_phase_rooted(
        HostEnqueuePhase::EventSemanticMacrotask,
        "events.once.reject",
        vec![p],
        move |rt| {
            rusty_js_runtime::promise::reject_promise(rt, p, err);
            Ok(())
        },
    );
    Value::Object(p)
}

fn events_once_invalid_arg_promise(rt: &mut Runtime) -> Value {
    events_once_error_promise(
        rt,
        "TypeError",
        "ERR_INVALID_ARG_TYPE",
        "The \"options\" argument must be an object with a valid AbortSignal",
    )
}

fn events_once_abort_promise(rt: &mut Runtime) -> Value {
    events_once_error_promise(rt, "AbortError", "ABORT_ERR", "The operation was aborted")
}

pub fn install(rt: &mut Runtime) {
    let ctor = build_core_emitter(rt);
    let proto = match rt.object_get(ctor, "prototype") {
        Value::Object(p) => p,
        _ => unreachable!("build_core_emitter wires the prototype"),
    };

    rt.object_set(ctor, "EventEmitter".into(), Value::Object(ctor));

    rt.object_set(
        ctor,
        "EventEmitterAsyncResource".into(),
        Value::Object(ctor),
    );
    rt.define_data_property_attrs(ctor, "prototype", Value::Object(proto), true, true, true);
    rt.object_set(ctor, "captureRejections".into(), Value::Boolean(false));
    rt.object_set(ctor, "defaultMaxListeners".into(), Value::Number(10.0));
    rt.object_set(ctor, "usingDomains".into(), Value::Boolean(false));

    let capture_rejection_symbol = rt
        .symbol_for_via(&[js_string("nodejs.rejection")])
        .unwrap_or(Value::Undefined);
    rt.object_set(
        ctor,
        "captureRejectionSymbol".into(),
        capture_rejection_symbol,
    );
    rt.object_set(
        ctor,
        "errorMonitor".into(),
        Value::Symbol(std::rc::Rc::new("events.errorMonitor".to_string())),
    );
    register_method(rt, ctor, "addAbortListener", |rt, args| {
        let signal = args.first().cloned().unwrap_or(Value::Undefined);
        let listener = args.get(1).cloned().unwrap_or(Value::Undefined);
        let disposable = new_object(rt);
        let disposed_key = "__cruft_add_abort_listener_disposed";
        rt.set_engine_sentinel(disposable, disposed_key, Value::Boolean(false));

        let signal_for_dispose = signal.clone();
        let listener_for_dispose = listener.clone();
        let dispose = make_callable(rt, "addAbortListener.dispose", move |rt, _args| {
            let this = match rt.current_this() {
                Value::Object(id) => id,
                _ => return Ok(Value::Undefined),
            };
            if matches!(rt.object_get(this, disposed_key), Value::Boolean(true)) {
                return Ok(Value::Undefined);
            }
            rt.set_engine_sentinel(this, disposed_key, Value::Boolean(true));
            if let Value::Object(sig) = signal_for_dispose.clone() {
                let remove = rt.object_get(sig, "removeEventListener");
                if rt.is_callable(&remove) {
                    let _ = rt.call_function(
                        remove,
                        Value::Object(sig),
                        vec![js_string("abort"), listener_for_dispose.clone()],
                    );
                }
            }
            Ok(Value::Undefined)
        });
        rt.object_set(disposable, "@@dispose".into(), Value::Object(dispose));

        let Value::Object(sig) = signal else {
            return Ok(Value::Object(disposable));
        };

        if abstract_ops::to_boolean(&rt.object_get(sig, "aborted")) {
            if rt.is_callable(&listener) {
                let _ = rt.call_function(listener, Value::Undefined, vec![]);
            }
            rt.set_engine_sentinel(disposable, disposed_key, Value::Boolean(true));
            return Ok(Value::Object(disposable));
        }

        let add = rt.object_get(sig, "addEventListener");
        if rt.is_callable(&add) {
            let options = new_object(rt);
            rt.object_set(options, "once".into(), Value::Boolean(true));
            let _ = rt.call_function(
                add,
                Value::Object(sig),
                vec![js_string("abort"), listener, Value::Object(options)],
            );
        }

        Ok(Value::Object(disposable))
    });
    register_method(rt, ctor, "getEventListeners", |rt, args| {
        let Some(Value::Object(target)) = args.first() else {
            let arr = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
            return Ok(Value::Object(arr));
        };
        let event = event_name(args.get(1).unwrap_or(&Value::Undefined));
        let cur = match existing_listeners(rt, *target) {
            Some(bag) => listener_bag_get(rt, bag, &event),
            None => Value::Undefined,
        };
        let arr = listeners_array_for_value(rt, cur, true);
        Ok(Value::Object(arr))
    });
    register_method(rt, ctor, "getMaxListeners", |rt, args| match args.first() {
        Some(Value::Object(target)) => Ok(Value::Number(max_listeners(rt, *target))),
        _ => Ok(Value::Number(10.0)),
    });
    register_method(rt, ctor, "init", |_rt, _args| Ok(Value::Undefined));

    register_method(rt, ctor, "on", |rt, args| {
        use rusty_js_runtime::promise::{new_promise, reject_promise, resolve_promise};
        let emitter = match args.first() {
            Some(Value::Object(e)) => *e,
            _ => {
                return Err(RuntimeError::TypeError(
                    "events.on: the first argument must be an EventEmitter".into(),
                ))
            }
        };
        let name = args.get(1).cloned().unwrap_or(Value::Undefined);
        let awaiting_error = matches!(&name, Value::String(s) if s.as_str() == "error");
        let signal = args.get(2).and_then(|v| match v {
            Value::Object(o) => match rt.object_get(*o, "signal") {
                Value::Object(sig) => Some(sig),
                _ => None,
            },
            _ => None,
        });

        let close_events: Vec<Value> = match args.get(2) {
            Some(Value::Object(o)) => match rt.object_get(*o, "close") {
                Value::Object(arr)
                    if matches!(
                        rt.obj(arr).internal_kind,
                        rusty_js_runtime::value::InternalKind::Array
                    ) =>
                {
                    let n = rt.array_length(arr);
                    (0..n).map(|i| rt.object_get(arr, &i.to_string())).collect()
                }
                _ => Vec::new(),
            },
            _ => Vec::new(),
        };

        let iter = new_object(rt);

        static ON_SEQ: AtomicU64 = AtomicU64::new(0);
        let root_key = format!(
            "events.on.active.{}",
            ON_SEQ.fetch_add(1, Ordering::Relaxed)
        );
        rt.retain_host_roots(root_key.clone(), vec![Value::Object(iter)]);
        rt.set_engine_sentinel(
            iter,
            "__on_root_key",
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(root_key))),
        );
        let queue = rt.alloc_object(RtObject::new_array());
        let waiters = rt.alloc_object(RtObject::new_array());
        rt.set_engine_sentinel(iter, "__on_queue", Value::Object(queue));
        rt.set_engine_sentinel(iter, "__on_waiters", Value::Object(waiters));
        rt.set_engine_sentinel(iter, "__on_done", Value::Boolean(false));
        rt.set_engine_sentinel(iter, "__on_has_error", Value::Boolean(false));
        rt.set_engine_sentinel(iter, "__on_error", Value::Undefined);
        rt.set_engine_sentinel(iter, "__on_emitter", Value::Object(emitter));
        rt.set_engine_sentinel(iter, "__on_event", name.clone());
        if let Some(sig) = signal {
            rt.set_engine_sentinel(iter, "__on_signal", Value::Object(sig));
        }

        let ev_listener = make_callable(rt, "events.on.onEvent", move |rt, a| {
            let arr = rt.alloc_object(RtObject::new_array());
            for (i, v) in a.iter().enumerate() {
                rt.object_set(arr, i.to_string().into(), v.clone());
            }
            let waiters = match on_sentinel_obj(rt, iter, "__on_waiters") {
                Some(w) => w,
                None => return Ok(Value::Undefined),
            };
            if on_arr_len(rt, waiters) > 0 {
                if let Value::Object(p) = on_arr_shift(rt, waiters) {
                    let res = on_iter_result(rt, Value::Object(arr), false);
                    resolve_promise(rt, p, res);
                }
            } else if let Some(q) = on_sentinel_obj(rt, iter, "__on_queue") {
                on_arr_push(rt, q, Value::Object(arr));
            }
            Ok(Value::Undefined)
        });
        rt.set_engine_sentinel(iter, "__on_ev_listener", Value::Object(ev_listener));

        if !awaiting_error {
            let err_listener = make_callable(rt, "events.on.onError", move |rt, a| {
                let reason = a.first().cloned().unwrap_or(Value::Undefined);
                rt.set_engine_sentinel(iter, "__on_done", Value::Boolean(true));
                let waiters = match on_sentinel_obj(rt, iter, "__on_waiters") {
                    Some(w) => w,
                    None => return Ok(Value::Undefined),
                };
                if on_arr_len(rt, waiters) > 0 {
                    if let Value::Object(p) = on_arr_shift(rt, waiters) {
                        reject_promise(rt, p, reason);
                    }
                } else {
                    rt.set_engine_sentinel(iter, "__on_has_error", Value::Boolean(true));
                    rt.set_engine_sentinel(iter, "__on_error", reason);
                }
                Ok(Value::Undefined)
            });
            rt.set_engine_sentinel(iter, "__on_err_listener", Value::Object(err_listener));
        }

        let on_m = rt.object_get(emitter, "on");
        if rt.is_callable(&on_m) {
            let evl = rt.object_get(iter, "__on_ev_listener");
            let _ = rt.call_function(on_m.clone(), Value::Object(emitter), vec![name, evl]);
            if !awaiting_error {
                let errl = rt.object_get(iter, "__on_err_listener");
                let err_name =
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from("error")));
                let _ = rt.call_function(on_m, Value::Object(emitter), vec![err_name, errl]);
            }
        }

        if let Some(sig) = signal {
            let abort_listener = make_callable(rt, "events.on.abort", move |rt, _a| {
                let reason = match rt.object_get(sig, "reason") {
                    Value::Undefined => Value::String(Rc::new(
                        rusty_js_runtime::value::JsString::from("The operation was aborted"),
                    )),
                    v => v,
                };
                rt.set_engine_sentinel(iter, "__on_done", Value::Boolean(true));
                if let Some(w) = on_sentinel_obj(rt, iter, "__on_waiters") {
                    while on_arr_len(rt, w) > 0 {
                        if let Value::Object(p) = on_arr_shift(rt, w) {
                            reject_promise(rt, p, reason.clone());
                        } else {
                            break;
                        }
                    }
                }
                rt.set_engine_sentinel(iter, "__on_has_error", Value::Boolean(true));
                rt.set_engine_sentinel(iter, "__on_error", reason);
                if let Value::String(k) = rt.object_get(iter, "__on_root_key") {
                    rt.release_host_roots(&k.as_str().to_string());
                }
                Ok(Value::Undefined)
            });
            rt.set_engine_sentinel(iter, "__on_abort_listener", Value::Object(abort_listener));
            if matches!(rt.object_get(sig, "aborted"), Value::Boolean(true)) {
                rt.set_engine_sentinel(iter, "__on_done", Value::Boolean(true));
                rt.set_engine_sentinel(iter, "__on_has_error", Value::Boolean(true));
                rt.set_engine_sentinel(iter, "__on_error", rt.object_get(sig, "reason"));
                if let Value::String(k) = rt.object_get(iter, "__on_root_key") {
                    rt.release_host_roots(&k.as_str().to_string());
                }
            } else {
                let add = rt.object_get(sig, "addEventListener");
                if rt.is_callable(&add) {
                    let ev =
                        Value::String(Rc::new(rusty_js_runtime::value::JsString::from("abort")));
                    let _ = rt.call_function(
                        add,
                        Value::Object(sig),
                        vec![ev, Value::Object(abort_listener)],
                    );
                }
            }
        }

        for close_name in &close_events {
            if !matches!(close_name, Value::String(_)) {
                continue;
            }
            let close_listener = make_callable(rt, "events.on.onClose", move |rt, _a| {
                rt.set_engine_sentinel(iter, "__on_done", Value::Boolean(true));
                if let Some(w) = on_sentinel_obj(rt, iter, "__on_waiters") {
                    while on_arr_len(rt, w) > 0 {
                        if let Value::Object(p) = on_arr_shift(rt, w) {
                            let res = on_iter_result(rt, Value::Undefined, true);
                            resolve_promise(rt, p, res);
                        } else {
                            break;
                        }
                    }
                }
                if let Value::String(k) = rt.object_get(iter, "__on_root_key") {
                    rt.release_host_roots(&k.as_str().to_string());
                }
                Ok(Value::Undefined)
            });
            let on_m = rt.object_get(emitter, "on");
            if rt.is_callable(&on_m) {
                let _ = rt.call_function(
                    on_m,
                    Value::Object(emitter),
                    vec![close_name.clone(), Value::Object(close_listener)],
                );
            }
        }

        register_method(rt, iter, "next", move |rt, _a| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };

            if let Some(q) = on_sentinel_obj(rt, this, "__on_queue") {
                if on_arr_len(rt, q) > 0 {
                    let arr = on_arr_shift(rt, q);
                    let p = new_promise(rt);
                    let res = on_iter_result(rt, arr, false);
                    resolve_promise(rt, p, res);
                    return Ok(Value::Object(p));
                }
            }

            if matches!(rt.object_get(this, "__on_has_error"), Value::Boolean(true)) {
                let reason = rt.object_get(this, "__on_error");
                rt.set_engine_sentinel(this, "__on_has_error", Value::Boolean(false));
                let p = new_promise(rt);
                reject_promise(rt, p, reason);
                return Ok(Value::Object(p));
            }

            if matches!(rt.object_get(this, "__on_done"), Value::Boolean(true)) {
                let p = new_promise(rt);
                let res = on_iter_result(rt, Value::Undefined, true);
                resolve_promise(rt, p, res);
                return Ok(Value::Object(p));
            }

            let p = new_promise(rt);
            if let Some(w) = on_sentinel_obj(rt, this, "__on_waiters") {
                on_arr_push(rt, w, Value::Object(p));
            }
            Ok(Value::Object(p))
        });

        let ret_impl = move |rt: &mut Runtime, _a: &[Value]| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            rt.set_engine_sentinel(this, "__on_done", Value::Boolean(true));

            if let Some(emitter) = on_sentinel_obj(rt, this, "__on_emitter") {
                let remove = rt.object_get(emitter, "removeListener");
                if rt.is_callable(&remove) {
                    let name = rt.object_get(this, "__on_event");
                    let evl = rt.object_get(this, "__on_ev_listener");
                    let _ =
                        rt.call_function(remove.clone(), Value::Object(emitter), vec![name, evl]);
                    let errl = rt.object_get(this, "__on_err_listener");
                    if rt.is_callable(&errl) {
                        let err_name = Value::String(Rc::new(
                            rusty_js_runtime::value::JsString::from("error"),
                        ));
                        let _ =
                            rt.call_function(remove, Value::Object(emitter), vec![err_name, errl]);
                    }
                }
            }

            if let Some(w) = on_sentinel_obj(rt, this, "__on_waiters") {
                while on_arr_len(rt, w) > 0 {
                    if let Value::Object(p) = on_arr_shift(rt, w) {
                        let res = on_iter_result(rt, Value::Undefined, true);
                        resolve_promise(rt, p, res);
                    } else {
                        break;
                    }
                }
            }

            if let Value::String(k) = rt.object_get(this, "__on_root_key") {
                rt.release_host_roots(&k.as_str().to_string());
            }
            let p = new_promise(rt);
            let res = on_iter_result(rt, Value::Undefined, true);
            resolve_promise(rt, p, res);
            Ok(Value::Object(p))
        };
        register_method(rt, iter, "return", ret_impl);
        register_method(rt, iter, "@@asyncIterator", |rt, _a| Ok(rt.current_this()));

        Ok(Value::Object(iter))
    });

    register_method(rt, ctor, "once", |rt, args| {
        use rusty_js_runtime::promise::{new_promise, reject_promise, resolve_promise};
        let emitter = match args.first() {
            Some(Value::Object(e)) => *e,
            _ => {
                return Err(RuntimeError::TypeError(
                    "events.once: the first argument must be an EventEmitter".into(),
                ))
            }
        };
        let name = args.get(1).cloned().unwrap_or(Value::Undefined);
        let awaiting_error = matches!(&name, Value::String(s) if s.as_str() == "error");
        let once_m = rt.object_get(emitter, "once");
        let add_event_listener = rt.object_get(emitter, "addEventListener");
        let is_event_emitter = rt.is_callable(&once_m);
        let is_event_target = rt.is_callable(&add_event_listener);
        if !is_event_emitter && !is_event_target {
            return Ok(events_once_invalid_arg_promise(rt));
        }
        let signal = match args.get(2) {
            None | Some(Value::Undefined) => None,
            Some(Value::Object(options)) => match rt.object_get(*options, "signal") {
                Value::Undefined => None,
                Value::Object(sig) if is_abort_signal_like(rt, sig) => Some(sig),
                _ => return Ok(events_once_invalid_arg_promise(rt)),
            },
            _ => return Ok(events_once_invalid_arg_promise(rt)),
        };
        if let Some(sig) = signal {
            if matches!(rt.object_get(sig, "aborted"), Value::Boolean(true)) {
                return Ok(events_once_abort_promise(rt));
            }
        }
        let p = new_promise(rt);

        static ONCE_SEQ: AtomicU64 = AtomicU64::new(0);
        let root_key = format!(
            "events.once.pending.{}",
            ONCE_SEQ.fetch_add(1, Ordering::Relaxed)
        );
        let state = new_object(rt);
        rt.set_engine_sentinel(state, "__once_emitter", Value::Object(emitter));
        rt.set_engine_sentinel(state, "__once_name", name.clone());
        rt.set_engine_sentinel(state, "__once_settled", Value::Boolean(false));
        rt.set_engine_sentinel(state, "__once_signal", Value::Undefined);
        rt.set_engine_sentinel(state, "__once_abort_listener", Value::Undefined);
        rt.retain_host_roots(
            root_key.clone(),
            vec![Value::Object(p), Value::Object(state)],
        );

        let ev_key = root_key.clone();
        let on_event = make_callable(rt, "events.once.onEvent", move |rt, a| {
            if !cleanup_events_once_state(rt, state) {
                return Ok(Value::Undefined);
            }
            let arr = rt.alloc_object(RtObject::new_array());
            for (i, v) in a.iter().enumerate() {
                rt.object_set(arr, i.to_string().into(), v.clone());
            }
            resolve_promise(rt, p, Value::Object(arr));
            rt.release_host_roots(&ev_key);
            Ok(Value::Undefined)
        });
        rt.set_engine_sentinel(state, "__once_event_listener", Value::Object(on_event));
        if awaiting_error {
            rt.set_engine_sentinel(state, "__once_error_listener", Value::Undefined);
        }
        if is_event_emitter {
            let _ = rt.call_function(
                once_m.clone(),
                Value::Object(emitter),
                vec![name, Value::Object(on_event)],
            );
        } else {
            let options = new_object(rt);
            rt.object_set(options, "once".into(), Value::Boolean(true));
            let _ = rt.call_function(
                add_event_listener.clone(),
                Value::Object(emitter),
                vec![name, Value::Object(on_event), Value::Object(options)],
            );
        }
        if is_event_emitter && !awaiting_error {
            let err_key = root_key.clone();
            let on_error = make_callable(rt, "events.once.onError", move |rt, a| {
                if !cleanup_events_once_state(rt, state) {
                    return Ok(Value::Undefined);
                }
                let reason = a.first().cloned().unwrap_or(Value::Undefined);
                reject_promise(rt, p, reason);
                rt.release_host_roots(&err_key);
                Ok(Value::Undefined)
            });
            rt.set_engine_sentinel(state, "__once_error_listener", Value::Object(on_error));
            let err_name = Value::String(Rc::new(rusty_js_runtime::value::JsString::from("error")));
            let _ = rt.call_function(
                once_m,
                Value::Object(emitter),
                vec![err_name, Value::Object(on_error)],
            );
        }
        if let Some(sig) = signal {
            rt.set_engine_sentinel(state, "__once_signal", Value::Object(sig));
            let abort_key = root_key.clone();
            let on_abort = make_callable(rt, "events.once.onAbort", move |rt, _a| {
                if !cleanup_events_once_state(rt, state) {
                    return Ok(Value::Undefined);
                }
                let reason = events_once_error_value(
                    rt,
                    "AbortError",
                    "ABORT_ERR",
                    "The operation was aborted",
                );
                rt.object_set(
                    p,
                    "__cruft_suppress_unhandled_rejection".into(),
                    Value::Boolean(true),
                );
                reject_promise(rt, p, reason);
                rt.release_host_roots(&abort_key);
                Ok(Value::Undefined)
            });
            rt.set_engine_sentinel(state, "__once_abort_listener", Value::Object(on_abort));
            let add = rt.object_get(sig, "addEventListener");
            if rt.is_callable(&add) {
                let _ = rt.call_function(
                    add,
                    Value::Object(sig),
                    vec![js_string("abort"), Value::Object(on_abort)],
                );
            }
        }
        Ok(Value::Object(p))
    });
    register_method(rt, ctor, "setMaxListeners", |rt, args| {
        let n = args.first().cloned().unwrap_or(Value::Number(10.0));
        for target in args.iter().skip(1) {
            if let Value::Object(id) = target {
                set_max_listeners(rt, *id, n.clone());
            }
        }
        Ok(Value::Undefined)
    });
    register_method(rt, ctor, "listenerCount", |rt, args| {
        let Some(Value::Object(target)) = args.first() else {
            return Ok(Value::Number(0.0));
        };
        let event = event_name(args.get(1).unwrap_or(&Value::Undefined));
        let n = if let Some(bag) = existing_listeners(rt, *target) {
            let cur = listener_bag_get(rt, bag, &event);
            if let Some(listener) = args.get(2) {
                listener_count_for_target(rt, &cur, listener)
            } else {
                listener_count_for_value(rt, &cur)
            }
        } else {
            event_target_listener_count(rt, *target, &event)
        };
        Ok(Value::Number(n as f64))
    });
    if let Some(d) = rt.obj_mut(ctor).get_own_mut("prototype") {
        d.enumerable = false;
    }
    rt.define_global_property("events", Value::Object(ctor));
}
