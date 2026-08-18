
use crate::register::{make_callable, native_function, new_object, register_method, set_constant};
use rusty_js_runtime::caps;
use rusty_js_runtime::caps::{ModuleId, ModuleProvenance};
use rusty_js_runtime::value::{InternalKind, JsString, Object, ObjectRef, PropertyDescriptor};
use rusty_js_runtime::{HostEnqueuePhase, Runtime, RuntimeError, Value};
use std::cell::RefCell;
use std::rc::Rc;

fn current_caller(rt: &Runtime) -> ModuleId {
    let url = rt.current_module_url.last().cloned().unwrap_or_default();
    let provenance = if url.contains("/node_modules/") {
        ModuleProvenance::Dependency
    } else if url.starts_with("node:") {
        ModuleProvenance::Builtin
    } else {
        ModuleProvenance::Application
    };
    ModuleId { url, provenance }
}

fn process_memory_usage_value_bytes(value: &Value) -> usize {
    std::mem::size_of::<Value>()
        + match value {
            Value::String(s) => s.len(),
            Value::BigInt(b) => b.to_string().len(),
            Value::Symbol(s) => s.len(),
            _ => 0,
        }
}

fn process_memory_usage_promise_reaction_bytes(
    reaction: &rusty_js_runtime::value::PromiseReaction,
) -> usize {
    let mut bytes = std::mem::size_of::<rusty_js_runtime::value::PromiseReaction>();
    if let Some(handler) = &reaction.handler {
        bytes = bytes.saturating_add(std::mem::size_of_val(handler));
        match handler {
            rusty_js_runtime::value::PromiseReactionHandler::Callable(value) => {
                bytes = bytes.saturating_add(process_memory_usage_value_bytes(value));
            }
            rusty_js_runtime::value::PromiseReactionHandler::LazyArrow(handler) => {
                bytes = bytes.saturating_add(std::mem::size_of_val(handler));
                bytes = bytes.saturating_add(
                    handler.upvalues.len()
                        * std::mem::size_of::<rusty_js_runtime::value::UpvalueCell>(),
                );
                bytes = bytes.saturating_add(
                    handler.captured_bindings.len()
                        * std::mem::size_of::<rusty_js_runtime::value::CapturedBinding>(),
                );
                if let Some(value) = &handler.bound_this {
                    bytes = bytes.saturating_add(process_memory_usage_value_bytes(value));
                }
                if let Some(value) = &handler.bound_derived_initial_this {
                    bytes = bytes.saturating_add(process_memory_usage_value_bytes(value));
                }
                if let Some(value) = &handler.bound_new_target {
                    bytes = bytes.saturating_add(process_memory_usage_value_bytes(value));
                }
                bytes = bytes.saturating_add(
                    handler.captured_with_env_stack.len() * std::mem::size_of::<ObjectRef>(),
                );
            }
            rusty_js_runtime::value::PromiseReactionHandler::LazyArrowOneCell(handler) => {
                bytes = bytes.saturating_add(std::mem::size_of_val(handler));
                if let Some(value) = &handler.bound_this {
                    bytes = bytes.saturating_add(process_memory_usage_value_bytes(value));
                }
            }
            rusty_js_runtime::value::PromiseReactionHandler::AsyncAwaitContinuation {
                snapshot,
                ..
            } => {
                bytes = bytes.saturating_add(std::mem::size_of_val(snapshot.as_ref()));
            }
        }
    }
    if let Some(value) = &reaction.cap_resolve {
        bytes = bytes.saturating_add(process_memory_usage_value_bytes(value));
    }
    if let Some(value) = &reaction.cap_reject {
        bytes = bytes.saturating_add(process_memory_usage_value_bytes(value));
    }
    bytes
}

fn process_memory_usage_object_bytes(object: &Object, capacity_view: bool) -> usize {
    let mut bytes = std::mem::size_of::<Object>();

    bytes = bytes.saturating_add(
        object.properties.len()
            * std::mem::size_of::<(rusty_js_runtime::value::PropertyKey, PropertyDescriptor)>(),
    );
    for (key, descriptor) in &object.properties {
        bytes = bytes.saturating_add(key.as_str().len());
        bytes = bytes.saturating_add(process_memory_usage_value_bytes(&descriptor.value));
        if let Some(getter) = &descriptor.getter {
            bytes = bytes.saturating_add(process_memory_usage_value_bytes(getter));
        }
        if let Some(setter) = &descriptor.setter {
            bytes = bytes.saturating_add(process_memory_usage_value_bytes(setter));
        }
    }

    let shape_len = if capacity_view {
        object.shape_values.capacity()
    } else {
        object.shape_values.len()
    };
    bytes = bytes.saturating_add(shape_len * std::mem::size_of::<Value>());
    for value in &object.shape_values {
        bytes = bytes.saturating_add(process_memory_usage_value_bytes(value));
    }

    let dense_len = if capacity_view {
        object.dense_elements.capacity()
    } else {
        object.dense_elements.len()
    };
    bytes = bytes.saturating_add(dense_len * std::mem::size_of::<Value>());
    for value in &object.dense_elements {
        bytes = bytes.saturating_add(process_memory_usage_value_bytes(value));
    }

    let doubles_len = if capacity_view {
        object.dense_doubles.capacity()
    } else {
        object.dense_doubles.len()
    };
    bytes = bytes.saturating_add(doubles_len * std::mem::size_of::<f64>());
    if let Some(sidecar) = &object.dense_i64_sidecar {
        let sidecar_len = if capacity_view {
            sidecar.capacity()
        } else {
            sidecar.len()
        };
        bytes = bytes.saturating_add(sidecar_len * std::mem::size_of::<i64>());
    }

    if let Some(slots) = &object.regexp_result_slots {
        bytes = bytes.saturating_add(slots.input.len());
        let position_len = if capacity_view {
            slots.positions.capacity()
        } else {
            slots.positions.len()
        };
        bytes = bytes.saturating_add(position_len * std::mem::size_of::<Option<(usize, usize)>>());
    }

    if let Some(private) = &object.private_members {
        bytes = bytes.saturating_add(private.fields.len() * std::mem::size_of::<(String, Value)>());
        for (key, value) in &private.fields {
            bytes = bytes.saturating_add(key.len());
            bytes = bytes.saturating_add(process_memory_usage_value_bytes(value));
        }
        bytes = bytes.saturating_add(private.names.len() * std::mem::size_of::<(String, ())>());
        for key in private.names.keys() {
            bytes = bytes.saturating_add(key.len());
        }
        bytes = bytes.saturating_add(private.methods.len() * std::mem::size_of::<(String, ())>());
        for key in private.methods.keys() {
            bytes = bytes.saturating_add(key.len());
        }
    }

    match &object.internal_kind {
        InternalKind::Function(function) => {
            bytes = bytes.saturating_add(std::mem::size_of_val(function.as_ref()));
            bytes = bytes.saturating_add(function.name.len());
            bytes = bytes.saturating_add(function.roots.len() * std::mem::size_of::<ObjectRef>());
        }
        InternalKind::Closure(closure) => {
            bytes = bytes.saturating_add(std::mem::size_of_val(closure.as_ref()));
            bytes = bytes.saturating_add(
                closure.upvalues.len()
                    * std::mem::size_of::<rusty_js_runtime::value::UpvalueCell>(),
            );
            bytes = bytes.saturating_add(
                closure.captured_bindings.len()
                    * std::mem::size_of::<rusty_js_runtime::value::CapturedBinding>(),
            );
            if let Some(value) = &closure.bound_this {
                bytes = bytes.saturating_add(process_memory_usage_value_bytes(value));
            }
            if let Some(value) = &closure.bound_derived_initial_this {
                bytes = bytes.saturating_add(process_memory_usage_value_bytes(value));
            }
            if let Some(value) = &closure.bound_new_target {
                bytes = bytes.saturating_add(process_memory_usage_value_bytes(value));
            }
            bytes = bytes.saturating_add(
                closure.captured_with_env_stack.len() * std::mem::size_of::<ObjectRef>(),
            );
        }
        InternalKind::BoundFunction(bound) => {
            bytes = bytes.saturating_add(std::mem::size_of_val(bound.as_ref()));
            bytes = bytes.saturating_add(process_memory_usage_value_bytes(&bound.this));
            bytes = bytes.saturating_add(bound.args.len() * std::mem::size_of::<Value>());
            for value in &bound.args {
                bytes = bytes.saturating_add(process_memory_usage_value_bytes(value));
            }
        }
        InternalKind::RegExp(regexp) => {
            bytes = bytes.saturating_add(std::mem::size_of_val(regexp.as_ref()));
            bytes = bytes.saturating_add(regexp.source.len());
            bytes = bytes.saturating_add(regexp.flags.len());
        }
        InternalKind::Promise(promise) => {
            bytes = bytes.saturating_add(std::mem::size_of_val(promise.as_ref()));
            bytes = bytes.saturating_add(process_memory_usage_value_bytes(&promise.value));
            bytes = bytes.saturating_add(
                promise.fulfill_reactions.len()
                    * std::mem::size_of::<rusty_js_runtime::value::PromiseReaction>(),
            );
            for reaction in &promise.fulfill_reactions {
                bytes = bytes.saturating_add(process_memory_usage_promise_reaction_bytes(reaction));
            }
            bytes = bytes.saturating_add(
                promise.reject_reactions.len()
                    * std::mem::size_of::<rusty_js_runtime::value::PromiseReaction>(),
            );
            for reaction in &promise.reject_reactions {
                bytes = bytes.saturating_add(process_memory_usage_promise_reaction_bytes(reaction));
            }
        }
        InternalKind::BoundaryWrapper(wrapper) => {
            bytes = bytes.saturating_add(std::mem::size_of_val(wrapper.as_ref()));
            bytes = bytes.saturating_add(process_memory_usage_value_bytes(&wrapper.target));
            bytes = bytes.saturating_add(process_memory_usage_value_bytes(&wrapper.validator));
            bytes = bytes.saturating_add(
                wrapper.sanitize_arg_defaults.len() * std::mem::size_of::<Option<Value>>(),
            );
            for value in wrapper.sanitize_arg_defaults.iter().flatten() {
                bytes = bytes.saturating_add(process_memory_usage_value_bytes(value));
            }
            if let Some(value) = &wrapper.sanitize_return_default {
                bytes = bytes.saturating_add(process_memory_usage_value_bytes(value));
            }
        }
        InternalKind::NumberWrapper(value)
        | InternalKind::StringWrapper(value)
        | InternalKind::BooleanWrapper(value)
        | InternalKind::BigIntWrapper(value) => {
            bytes = bytes.saturating_add(process_memory_usage_value_bytes(value));
        }
        InternalKind::Generator(generator) => {
            bytes = bytes.saturating_add(std::mem::size_of_val(generator.as_ref()));
            if let Some(value) = &generator.yielded_value {
                bytes = bytes.saturating_add(process_memory_usage_value_bytes(value));
            }
            if let Some(value) = &generator.pending_return {
                bytes = bytes.saturating_add(process_memory_usage_value_bytes(value));
            }
            if let Some(delegate) = &generator.delegate {
                bytes = bytes.saturating_add(std::mem::size_of_val(delegate));
                bytes =
                    bytes.saturating_add(process_memory_usage_value_bytes(&delegate.next_method));
            }
            bytes = bytes.saturating_add(
                generator.request_queue.len()
                    * std::mem::size_of::<rusty_js_runtime::value::AsyncGenRequest>(),
            );
            for request in &generator.request_queue {
                bytes = bytes.saturating_add(process_memory_usage_value_bytes(&request.value));
            }
        }
        InternalKind::MappedArguments { parameter_map } => {
            bytes = bytes.saturating_add(
                parameter_map.len()
                    * std::mem::size_of::<(String, rusty_js_runtime::value::UpvalueCell)>(),
            );
            for key in parameter_map.keys() {
                bytes = bytes.saturating_add(key.len());
            }
        }
        _ => {}
    }

    bytes
}

fn process_memory_usage_heap_estimate_bytes(rt: &Runtime, capacity_view: bool) -> usize {
    let slot_count = if capacity_view {
        rt.heap.len()
    } else {
        rt.heap.live_len()
    };
    let slot_floor = slot_count.max(1) * std::mem::size_of::<Object>();
    let object_bytes = rt
        .heap
        .live_object_ids()
        .filter_map(|id| rt.heap.get(id))
        .map(|object| process_memory_usage_object_bytes(object, capacity_view))
        .fold(0usize, usize::saturating_add);
    slot_floor.max(object_bytes).max(4096)
}

static PENDING_SIGNALS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

extern "C" fn cruft_signal_flag(sig: libc::c_int) {
    if (0..64).contains(&sig) {
        PENDING_SIGNALS.fetch_or(1u64 << sig, std::sync::atomic::Ordering::SeqCst);
    }
}

thread_local! {

    static SIGNAL_LISTENERS: std::cell::RefCell<Vec<(i32, String, Value)>> =
        std::cell::RefCell::new(Vec::new());
}

fn install_bootstrap_module_load_list(rt: &mut Runtime, list: ObjectRef) {
    let before_pre_exec = [
        "Internal Binding builtins",
        "Internal Binding encoding_binding",
        "Internal Binding modules",
        "Internal Binding errors",
        "Internal Binding util",
        "NativeModule internal/errors",
        "Internal Binding config",
        "Internal Binding timers",
        "Internal Binding async_context_frame",
        "NativeModule internal/async_context_frame",
        "Internal Binding async_wrap",
        "Internal Binding task_queue",
        "Internal Binding symbols",
        "NativeModule internal/async_hooks",
        "Internal Binding constants",
        "Internal Binding types",
        "NativeModule internal/util",
        "NativeModule internal/util/types",
        "NativeModule internal/validators",
        "NativeModule internal/linkedlist",
        "NativeModule internal/priority_queue",
        "NativeModule internal/assert",
        "NativeModule internal/util/inspect",
        "NativeModule internal/util/debuglog",
        "NativeModule internal/streams/utils",
        "NativeModule internal/timers",
        "NativeModule events",
        "Internal Binding buffer",
        "Internal Binding string_decoder",
        "NativeModule util/types",
        "NativeModule internal/buffer",
        "NativeModule buffer",
        "Internal Binding messaging",
        "NativeModule internal/worker/js_transferable",
        "Internal Binding process_methods",
        "NativeModule internal/process/per_thread",
        "Internal Binding credentials",
        "NativeModule internal/process/promises",
        "NativeModule internal/fixed_queue",
        "NativeModule async_hooks",
        "NativeModule internal/process/task_queues",
        "NativeModule timers",
        "Internal Binding trace_events",
        "NativeModule internal/constants",
        "NativeModule path",
        "NativeModule internal/process/execution",
        "NativeModule internal/process/permission",
        "NativeModule internal/process/warning",
        "NativeModule internal/console/constructor",
        "NativeModule internal/console/global",
        "NativeModule internal/querystring",
        "NativeModule querystring",
        "Internal Binding url",
        "Internal Binding url_pattern",
        "Internal Binding blob",
        "NativeModule internal/url",
        "NativeModule util",
        "NativeModule internal/webidl",
        "Internal Binding performance",
        "Internal Binding permission",
        "NativeModule internal/perf/utils",
        "NativeModule internal/event_target",
        "Internal Binding mksnapshot",
        "NativeModule internal/v8/startup_snapshot",
        "NativeModule internal/process/signal",
        "Internal Binding fs",
        "NativeModule internal/encoding",
        "NativeModule internal/encoding/single-byte",
        "NativeModule internal/encoding/util",
        "NativeModule internal/blob",
        "NativeModule internal/fs/utils",
        "NativeModule fs",
        "Internal Binding options",
        "NativeModule internal/options",
        "NativeModule internal/source_map/source_map_cache",
        "Internal Binding contextify",
        "NativeModule internal/vm",
        "NativeModule internal/modules/helpers",
        "NativeModule internal/modules/customization_hooks",
        "NativeModule internal/modules/package_json_reader",
        "Internal Binding module_wrap",
        "NativeModule internal/modules/cjs/loader",
        "NativeModule diagnostics_channel",
        "Internal Binding diagnostics_channel",
        "Internal Binding wasm_web_api",
        "NativeModule internal/events/abort_listener",
        "NativeModule internal/modules/typescript",
        "NativeModule internal/data_url",
        "NativeModule internal/mime",
        "NativeModule internal/modules/esm/utils",
        "Internal Binding worker",
        "NativeModule internal/modules/run_main",
        "NativeModule internal/net",
        "NativeModule internal/dns/utils",
        "NativeModule internal/modules/esm/get_format",
        "Internal Binding cjs_lexer",
        "NativeModule internal/modules/esm/assert",
        "NativeModule internal/modules/esm/loader",
        "NativeModule internal/modules/esm/load",
        "NativeModule internal/modules/esm/resolve",
        "NativeModule internal/modules/esm/translators",
        "NativeModule internal/modules/esm/module_job",
        "NativeModule internal/modules/esm/module_map",
        "NativeModule url",
        "Internal Binding icu",
    ];
    let mut index = 0usize;
    for name in before_pre_exec {
        rt.object_set(
            list,
            index.to_string(),
            Value::String(Rc::new(JsString::from(name))),
        );
        index += 1;
    }
    rt.object_set(
        list,
        index.to_string(),
        Value::String(Rc::new(JsString::from(
            "NativeModule internal/process/pre_execution",
        ))),
    );
    rt.object_set(list, "length".into(), Value::Number((index + 1) as f64));
}

#[cfg(unix)]
fn signal_number(name: &str) -> Option<i32> {
    Some(match name {
        "SIGHUP" => libc::SIGHUP,
        "SIGINT" => libc::SIGINT,
        "SIGQUIT" => libc::SIGQUIT,
        "SIGILL" => libc::SIGILL,
        "SIGTRAP" => libc::SIGTRAP,
        "SIGABRT" => libc::SIGABRT,
        "SIGFPE" => libc::SIGFPE,
        "SIGKILL" => libc::SIGKILL,
        "SIGBUS" => libc::SIGBUS,
        "SIGSEGV" => libc::SIGSEGV,
        "SIGSYS" => libc::SIGSYS,
        "SIGPIPE" => libc::SIGPIPE,
        "SIGALRM" => libc::SIGALRM,
        "SIGTERM" => libc::SIGTERM,
        "SIGURG" => libc::SIGURG,
        "SIGSTOP" => libc::SIGSTOP,
        "SIGTSTP" => libc::SIGTSTP,
        "SIGCONT" => libc::SIGCONT,
        "SIGCHLD" => libc::SIGCHLD,
        "SIGTTIN" => libc::SIGTTIN,
        "SIGTTOU" => libc::SIGTTOU,
        "SIGIO" => libc::SIGIO,
        "SIGXCPU" => libc::SIGXCPU,
        "SIGXFSZ" => libc::SIGXFSZ,
        "SIGVTALRM" => libc::SIGVTALRM,
        "SIGPROF" => libc::SIGPROF,
        "SIGWINCH" => libc::SIGWINCH,
        "SIGUSR1" => libc::SIGUSR1,
        "SIGUSR2" => libc::SIGUSR2,
        _ => return None,
    })
}

#[cfg(not(unix))]
fn signal_number(name: &str) -> Option<i32> {
    Some(match name {
        "SIGHUP" => 1,
        "SIGINT" => 2,

        "SIGBREAK" => 21,
        "SIGQUIT" => 3,
        "SIGILL" => 4,
        "SIGTRAP" => 5,
        "SIGABRT" => 6,
        "SIGBUS" => 7,
        "SIGFPE" => 8,
        "SIGKILL" => 9,
        "SIGUSR1" => 10,
        "SIGSEGV" => 11,
        "SIGUSR2" => 12,
        "SIGPIPE" => 13,
        "SIGALRM" => 14,
        "SIGTERM" => 15,
        "SIGCHLD" => 17,
        "SIGCONT" => 18,
        "SIGSTOP" => 19,
        "SIGTSTP" => 20,
        "SIGTTIN" => 21,
        "SIGTTOU" => 22,
        "SIGURG" => 23,
        "SIGXCPU" => 24,
        "SIGXFSZ" => 25,
        "SIGVTALRM" => 26,
        "SIGPROF" => 27,
        "SIGWINCH" => 28,
        "SIGIO" => 29,
        "SIGSYS" => 31,
        _ => return None,
    })
}

#[cfg(unix)]
fn install_signal_flag_handler(sig: i32) {
    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = cruft_signal_flag as usize;
        sa.sa_flags = libc::SA_RESTART;
        libc::sigemptyset(&mut sa.sa_mask);
        libc::sigaction(sig, &sa, std::ptr::null_mut());
    }
}

#[cfg(windows)]
static LISTENED_SIGNALS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[cfg(windows)]
unsafe extern "system" fn cruft_console_ctrl_handler(ctrl_type: u32) -> i32 {
    use std::sync::atomic::Ordering;
    const CTRL_C_EVENT: u32 = 0;
    const CTRL_BREAK_EVENT: u32 = 1;

    let sig: u64 = match ctrl_type {
        CTRL_C_EVENT => 2,
        CTRL_BREAK_EVENT => 21,
        _ => return 0,
    };
    if LISTENED_SIGNALS.load(Ordering::SeqCst) & (1u64 << sig) == 0 {
        return 0;
    }
    PENDING_SIGNALS.fetch_or(1u64 << sig, Ordering::SeqCst);
    1
}

#[cfg(windows)]
fn install_signal_flag_handler(sig: i32) {
    use std::sync::atomic::{AtomicBool, Ordering};
    if (0..64).contains(&sig) {
        LISTENED_SIGNALS.fetch_or(1u64 << sig, Ordering::SeqCst);
    }

    static INSTALLED: AtomicBool = AtomicBool::new(false);
    if INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetConsoleCtrlHandler(
            handler: Option<unsafe extern "system" fn(u32) -> i32>,
            add: i32,
        ) -> i32;
    }
    unsafe {
        SetConsoleCtrlHandler(Some(cruft_console_ctrl_handler), 1);
    }
}

#[cfg(not(any(unix, windows)))]
fn install_signal_flag_handler(_sig: i32) {}

#[cfg(windows)]
mod win_kill {
    use std::os::raw::c_void;

    const PROCESS_TERMINATE: u32 = 0x0001;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
        fn TerminateProcess(handle: *mut c_void, exit_code: u32) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }

    pub fn kill(pid: i32, sig: i32) -> i32 {

        if pid < 0 {
            return -1;
        }
        unsafe {
            if sig == 0 {
                let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid as u32);
                if h.is_null() {
                    return -1;
                }
                CloseHandle(h);
                0
            } else {
                let h = OpenProcess(PROCESS_TERMINATE, 0, pid as u32);
                if h.is_null() {
                    return -1;
                }
                let ok = TerminateProcess(h, 1);
                CloseHandle(h);
                if ok != 0 {
                    0
                } else {
                    -1
                }
            }
        }
    }
}

fn proc_events(rt: &mut Runtime, process: ObjectRef, sentinel: &str) -> ObjectRef {
    match rt.object_get(process, sentinel) {
        Value::Object(o) => o,
        _ => {
            let o = new_object(rt);
            rt.set_engine_sentinel(process, sentinel, Value::Object(o));
            o
        }
    }
}

fn proc_event_array(
    rt: &mut Runtime,
    process: ObjectRef,
    sentinel: &str,
    name: &str,
    create: bool,
) -> Option<ObjectRef> {
    let reg = proc_events(rt, process, sentinel);
    match rt.object_get(reg, name) {
        Value::Object(a) => Some(a),
        _ if create => {
            let a = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
            rt.object_set(reg, name.to_string(), Value::Object(a));
            Some(a)
        }
        _ => None,
    }
}

fn proc_add_listener(
    rt: &mut Runtime,
    process: ObjectRef,
    name: &str,
    cb: Value,
    once: bool,
    prepend: bool,
) {
    let sentinel = if once { "__proc_once" } else { "__proc_events" };
    if let Some(arr) = proc_event_array(rt, process, sentinel, name, true) {
        let len = rt.array_length(arr);
        if prepend {
            for i in (0..len).rev() {
                let v = rt.object_get(arr, &i.to_string());
                rt.object_set(arr, (i + 1).to_string(), v);
            }
            rt.object_set(arr, "0".into(), cb);
        } else {
            rt.object_set(arr, len.to_string(), cb);
        }
        rt.object_set(arr, "length".into(), Value::Number((len + 1) as f64));
    }
}

fn proc_listener_count(rt: &mut Runtime, process: ObjectRef, name: &str) -> usize {
    let mut n = 0;
    if let Some(a) = proc_event_array(rt, process, "__proc_events", name, false) {
        n += rt.array_length(a);
    }
    if let Some(a) = proc_event_array(rt, process, "__proc_once", name, false) {
        n += rt.array_length(a);
    }
    n
}

fn proc_snapshot(rt: &mut Runtime, process: ObjectRef, sentinel: &str, name: &str) -> Vec<Value> {
    let mut out = Vec::new();
    if let Some(a) = proc_event_array(rt, process, sentinel, name, false) {
        let len = rt.array_length(a);
        for i in 0..len {
            out.push(rt.object_get(a, &i.to_string()));
        }
    }
    out
}

fn proc_collect_event_names(rt: &mut Runtime, process: ObjectRef) -> Vec<String> {
    let mut names = Vec::new();
    for builtin in ["newListener", "removeListener", "warning"] {
        names.push(builtin.to_string());
    }
    for sentinel in ["__proc_events", "__proc_once"] {
        let reg = match rt.object_get(process, sentinel) {
            Value::Object(o) => o,
            _ => continue,
        };
        let registry_keys = rt.ordinary_own_enumerable_string_keys(reg);
        for name in registry_keys {
            if proc_listener_count(rt, process, &name) > 0
                && !names.iter().any(|existing| existing == &name)
            {
                names.push(name);
            }
        }
    }
    names
}

pub fn emit_process_event(rt: &mut Runtime, name: &str, args: Vec<Value>) -> bool {
    let process = match rt.global_get("process") {
        Value::Object(p) => p,
        _ => match rt.engine_helpers.get("__cruft_host_process") {
            Some(Value::Object(p)) => *p,
            _ => return false,
        },
    };
    let regular = proc_snapshot(rt, process, "__proc_events", name);
    let once = proc_snapshot(rt, process, "__proc_once", name);

    if !once.is_empty() {
        if let Some(a) = proc_event_array(rt, process, "__proc_once", name, false) {
            rt.object_set(a, "length".into(), Value::Number(0.0));
        }
    }
    let mut any = false;
    for cb in regular.into_iter().chain(once.into_iter()) {
        if rt.is_callable(&cb) {
            any = true;
            let _ = rt.call_function(cb, Value::Object(process), args.clone());
        }
    }
    any
}

pub fn has_process_listener(rt: &mut Runtime, name: &str) -> bool {
    match rt.global_get("process") {
        Value::Object(p) => proc_listener_count(rt, p, name) > 0,
        _ => false,
    }
}

fn process_on_impl(rt: &mut Runtime, args: &[Value], once: bool, prepend: bool) {
    let (name, cb) = match (args.first(), args.get(1)) {
        (Some(Value::String(name)), Some(cb)) if rt.is_callable(cb) => {
            (name.as_str().to_string(), cb.clone())
        }
        _ => return,
    };
    let process = match rt.current_this() {
        Value::Object(p) => p,
        _ => match rt.global_get("process") {
            Value::Object(p) => p,
            _ => return,
        },
    };
    if let Some(sig) = signal_number(&name) {
        install_signal_flag_handler(sig);
        let idx = SIGNAL_LISTENERS.with(|l| l.borrow().len());
        rt.retain_host_roots(format!("signal:{sig}:{idx}"), vec![cb.clone()]);
        SIGNAL_LISTENERS.with(|l| l.borrow_mut().push((sig, name.clone(), cb.clone())));
        proc_add_listener(rt, process, &name, cb, once, prepend);
        return;
    }
    proc_add_listener(rt, process, &name, cb, once, prepend);
}

fn process_binding_constants(rt: &mut Runtime) -> Value {
    let ns = new_object(rt);
    let fs = new_object(rt);
    let os = new_object(rt);
    let errno = new_object(rt);
    for object_id in [ns, fs, os, errno] {
        rt.obj_mut(object_id)
            .set_own_internal("__process_binding_constants__".into(), Value::Boolean(true));
    }

    let fs_consts: &[(&str, i32)] = &[
        ("O_RDONLY", libc::O_RDONLY),
        ("O_WRONLY", libc::O_WRONLY),
        ("O_RDWR", libc::O_RDWR),
        ("O_CREAT", libc::O_CREAT),
        ("O_EXCL", libc::O_EXCL),
        ("O_TRUNC", libc::O_TRUNC),
        ("O_APPEND", libc::O_APPEND),
    ];
    for (name, value) in fs_consts {
        let v = Value::Number(*value as f64);
        rt.object_set(ns, (*name).into(), v.clone());
        rt.object_set(fs, (*name).into(), v);
    }

    let errno_consts: &[(&str, i32)] = &[
        ("EBADF", libc::EBADF),
        ("ENOENT", libc::ENOENT),
        ("EEXIST", libc::EEXIST),
        ("EACCES", libc::EACCES),
        ("EISDIR", libc::EISDIR),
        ("ENOTDIR", libc::ENOTDIR),
        ("EINVAL", libc::EINVAL),
        ("EPERM", libc::EPERM),
    ];
    for (name, value) in errno_consts {
        let v = Value::Number(*value as f64);
        rt.object_set(ns, (*name).into(), v.clone());
        rt.object_set(errno, (*name).into(), v);
    }

    rt.object_set(os, "errno".into(), Value::Object(errno));
    rt.object_set(ns, "fs".into(), Value::Object(fs));
    rt.object_set(ns, "os".into(), Value::Object(os));
    Value::Object(ns)
}

fn process_binding_fs(rt: &mut Runtime) -> Value {
    let ns = new_object(rt);
    rt.obj_mut(ns)
        .set_own_internal("__process_binding_fs__".into(), Value::Boolean(true));
    register_method(rt, ns, "internalModuleStat", |_rt, args| {
        let path = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            Some(other) => rusty_js_runtime::abstract_ops::to_string(other)
                .as_str()
                .to_string(),
            None => String::new(),
        };
        let code = match std::fs::metadata(path) {
            Ok(md) if md.is_file() => 0.0,
            Ok(md) if md.is_dir() => 1.0,
            Ok(_) => -1.0,
            Err(_) => -2.0,
        };
        Ok(Value::Number(code))
    });
    Value::Object(ns)
}

fn process_resident_set_size_bytes() -> f64 {
    #[cfg(target_os = "linux")]
    {
        return std::fs::read_to_string("/proc/self/statm")
            .ok()
            .and_then(|s| s.split_whitespace().nth(1).map(|p| p.to_string()))
            .and_then(|pages| pages.parse::<f64>().ok())
            .map(|pages| {
                let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
                let page_size = if page_size > 0 {
                    page_size as f64
                } else {
                    4096.0
                };
                pages * page_size
            })
            .unwrap_or(0.0);
    }

    #[cfg(target_os = "macos")]
    {
        type KernReturn = libc::c_int;
        type MachPort = libc::c_uint;
        type TaskFlavor = libc::c_int;
        type MachMsgTypeNumber = libc::c_uint;
        type Integer = libc::c_int;

        #[repr(C)]
        #[derive(Clone, Copy)]
        struct TimeValue {
            seconds: libc::c_int,
            microseconds: libc::c_int,
        }

        #[repr(C)]
        #[derive(Clone, Copy)]
        struct MachTaskBasicInfo {
            virtual_size: u64,
            resident_size: u64,
            resident_size_max: u64,
            user_time: TimeValue,
            system_time: TimeValue,
            policy: libc::c_int,
            suspend_count: libc::c_int,
        }

        extern "C" {
            fn mach_task_self() -> MachPort;
            fn task_info(
                target_task: MachPort,
                flavor: TaskFlavor,
                task_info_out: *mut Integer,
                task_info_out_count: *mut MachMsgTypeNumber,
            ) -> KernReturn;
        }

        const KERN_SUCCESS: KernReturn = 0;
        const MACH_TASK_BASIC_INFO: TaskFlavor = 20;
        let mut info: MachTaskBasicInfo = unsafe { std::mem::zeroed() };
        let mut count = (std::mem::size_of::<MachTaskBasicInfo>() / std::mem::size_of::<Integer>())
            as MachMsgTypeNumber;
        let rc = unsafe {
            task_info(
                mach_task_self(),
                MACH_TASK_BASIC_INFO,
                &mut info as *mut MachTaskBasicInfo as *mut Integer,
                &mut count,
            )
        };
        return if rc == KERN_SUCCESS {
            info.resident_size as f64
        } else {
            0.0
        };
    }

    #[cfg(windows)]
    {
        type Bool = i32;
        type Dword = u32;
        type Handle = *mut core::ffi::c_void;
        type SizeT = usize;

        #[repr(C)]
        struct ProcessMemoryCounters {
            cb: Dword,
            page_fault_count: Dword,
            peak_working_set_size: SizeT,
            working_set_size: SizeT,
            quota_peak_paged_pool_usage: SizeT,
            quota_paged_pool_usage: SizeT,
            quota_peak_non_paged_pool_usage: SizeT,
            quota_non_paged_pool_usage: SizeT,
            pagefile_usage: SizeT,
            peak_pagefile_usage: SizeT,
        }

        #[link(name = "kernel32")]
        extern "system" {
            fn GetCurrentProcess() -> Handle;
        }
        #[link(name = "psapi")]
        extern "system" {
            fn GetProcessMemoryInfo(
                process: Handle,
                counters: *mut ProcessMemoryCounters,
                cb: Dword,
            ) -> Bool;
        }

        let mut counters: ProcessMemoryCounters = unsafe { std::mem::zeroed() };
        counters.cb = std::mem::size_of::<ProcessMemoryCounters>() as Dword;
        let ok = unsafe {
            GetProcessMemoryInfo(
                GetCurrentProcess(),
                &mut counters,
                std::mem::size_of::<ProcessMemoryCounters>() as Dword,
            )
        };
        return if ok != 0 {
            counters.working_set_size as f64
        } else {
            0.0
        };
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        0.0
    }
}

#[cfg(unix)]
fn timeval_to_microseconds(time: libc::timeval) -> f64 {
    (time.tv_sec as f64 * 1_000_000.0) + time.tv_usec as f64
}

fn process_cpu_usage_microseconds() -> (f64, f64) {
    #[cfg(unix)]
    {
        let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
        if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) } == 0 {
            return (
                timeval_to_microseconds(usage.ru_utime),
                timeval_to_microseconds(usage.ru_stime),
            );
        }
    }
    (0.0, 0.0)
}

fn thread_cpu_usage_microseconds() -> (f64, f64) {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
        if unsafe { libc::getrusage(libc::RUSAGE_THREAD, &mut usage) } == 0 {
            return (
                timeval_to_microseconds(usage.ru_utime),
                timeval_to_microseconds(usage.ru_stime),
            );
        }
    }
    process_cpu_usage_microseconds()
}

#[cfg(unix)]
fn resource_usage_max_rss_kib(usage: &libc::rusage) -> f64 {
    #[cfg(target_os = "macos")]
    {

        usage.ru_maxrss as f64 / 1024.0
    }
    #[cfg(not(target_os = "macos"))]
    {

        usage.ru_maxrss as f64
    }
}

fn previous_cpu_usage_arg(rt: &mut Runtime, args: &[Value]) -> Option<(f64, f64)> {
    let obj = match args.first() {
        Some(Value::Object(o)) => *o,
        _ => return None,
    };
    let user = match rt.object_get(obj, "user") {
        Value::Number(n) => n,
        other => rusty_js_runtime::abstract_ops::to_number(&other),
    };
    let system = match rt.object_get(obj, "system") {
        Value::Number(n) => n,
        other => rusty_js_runtime::abstract_ops::to_number(&other),
    };
    Some((user, system))
}

pub fn poll_signals(rt: &mut Runtime) -> bool {
    let bits = PENDING_SIGNALS.swap(0, std::sync::atomic::Ordering::SeqCst);
    if bits == 0 {
        return false;
    }
    let fired: Vec<String> = SIGNAL_LISTENERS.with(|l| {
        l.borrow()
            .iter()
            .filter(|(sig, _, _)| bits & (1u64 << *sig) != 0)
            .map(|(_, name, _)| name.clone())
            .collect()
    });
    let mut any = false;
    let mut seen = Vec::new();
    for name in fired {
        if seen.iter().any(|s| s == &name) || !has_process_listener(rt, &name) {
            continue;
        }
        seen.push(name.clone());
        any = true;
        rt.enqueue_host_phase_rooted(
            HostEnqueuePhase::HostCompletionMacrotask,
            "signal callback",
            Vec::new(),
            move |rt| {

                emit_process_event(
                    rt,
                    &name,
                    vec![Value::String(Rc::new(
                        rusty_js_runtime::value::JsString::from(name.clone()),
                    ))],
                );
                Ok(())
            },
        );
    }
    any
}

pub fn current_exit_code(rt: &mut Runtime) -> Option<i32> {
    if let Value::Object(p) = rt.global_get("process") {
        match rt.object_get(p, "exitCode") {
            Value::Undefined | Value::Null => None,
            v => Some(rusty_js_runtime::abstract_ops::to_number(&v) as i32),
        }
    } else {
        None
    }
}

fn check_clock(rt: &Runtime, op: caps::ClockOp) -> Result<(), RuntimeError> {
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
        .require_clock(&caps::Clock::disabled(), op, &caller)
        .map_err(|e| RuntimeError::TypeError(e.to_string()))
}

fn check_process(rt: &Runtime, op: caps::ProcessOp) -> Result<(), RuntimeError> {
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
        .require_process(&caps::Process::none(), op, &caller)
        .map_err(|e| RuntimeError::TypeError(e.to_string()))
}

pub fn install(rt: &mut Runtime, argv: Vec<String>) {
    let process = new_object(rt);
    rt.set_engine_helper_with_satb("__cruft_host_process".to_string(), Value::Object(process));
    let process_proto = new_object(rt);
    let process_ctor = make_callable(rt, "process", |rt, _args| Ok(rt.current_this()));
    rt.obj_mut(process_proto)
        .set_own_internal("constructor".into(), Value::Object(process_ctor));
    rt.set_own_frozen_property(
        process_ctor,
        "prototype".into(),
        Value::Object(process_proto),
    );
    rt.set_object_prototype_internal(process, Some(process_proto));

    rt.define_data_property_attrs(
        process,
        "@@toStringTag",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from("process"))),
        false,
        false,
        true,
    );
    let start = std::time::Instant::now();

    let argv_array = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
    rt.obj_mut(argv_array)
        .set_own_internal("__process_argv__".into(), Value::Boolean(true));
    for (i, s) in argv.iter().enumerate() {
        rt.object_set(
            argv_array,
            i.to_string(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(s.clone()))),
        );
    }
    set_constant(rt, process, "argv", Value::Object(argv_array));

    let exec_argv = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
    rt.obj_mut(exec_argv)
        .set_own_internal("__process_exec_argv__".into(), Value::Boolean(true));
    rt.object_set(exec_argv, "length".into(), Value::Number(0.0));
    set_constant(rt, process, "execArgv", Value::Object(exec_argv));

    let env_obj = new_object(rt);
    rt.obj_mut(env_obj)
        .set_own_internal("__process_env__".into(), Value::Boolean(true));
    let mode = rt.caps.mode;
    let sealed = matches!(
        mode,
        rusty_js_runtime::caps::CapMode::Sealed | rusty_js_runtime::caps::CapMode::SealedDeps
    );
    if !sealed {

        let vars: Vec<(String, String)> = std::env::vars().collect();
        #[cfg(windows)]
        let mut saw_path_upper = false;
        #[cfg(windows)]
        let mut path_value: Option<String> = None;
        for (k, v) in vars {
            #[cfg(windows)]
            {
                if k == "PATH" {
                    saw_path_upper = true;
                }
                if k.eq_ignore_ascii_case("PATH") && path_value.is_none() {
                    path_value = Some(v.clone());
                }
            }
            rt.object_set(env_obj, k, Value::String(Rc::new(JsString::from(v))));
        }
        #[cfg(windows)]
        if !saw_path_upper {
            if let Some(v) = path_value {
                rt.object_set(
                    env_obj,
                    "PATH".into(),
                    Value::String(Rc::new(JsString::from(v))),
                );
            }
        }
    } else {

        let allow: Vec<String> = std::env::var("CRUFT_ENV_ALLOW")
            .ok()
            .map(|s| {
                s.split(',')
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        let env_cap = caps::Env {
            vars: if allow.is_empty() {
                caps::EnvVarPolicy::None
            } else {
                caps::EnvVarPolicy::Whitelist(allow)
            },
            system_info: false,
        };
        let vars: Vec<(String, String)> = std::env::vars().collect();
        for (k, v) in vars {

            let cell = Rc::new(RefCell::new(v));
            let getter = {
                let name = k.clone();
                let cap = env_cap.clone();
                let cell = Rc::clone(&cell);
                native_function(rt, &format!("get {k}"), move |rt, _args| {
                    let caller = current_caller(rt);
                    rt.caps
                        .require_env(&cap, caps::EnvOp::ReadVar(name.clone()), &caller)
                        .map_err(|e| RuntimeError::TypeError(e.to_string()))?;
                    Ok(Value::String(Rc::new(JsString::from(
                        cell.borrow().clone(),
                    ))))
                })
            };
            let setter = {
                let cell = Rc::clone(&cell);
                native_function(rt, &format!("set {k}"), move |_rt, args| {
                    if let Some(Value::String(s)) = args.first() {
                        *cell.borrow_mut() = s.as_str().to_string();
                    }
                    Ok(Value::Undefined)
                })
            };
            rt.obj_mut(env_obj).insert_str(
                k,
                PropertyDescriptor {
                    value: Value::Undefined,
                    writable: false,
                    enumerable: true,
                    configurable: true,
                    getter: Some(getter),
                    setter: Some(setter),
                },
            );
        }
    }
    set_constant(rt, process, "env", Value::Object(env_obj));

    set_constant(
        rt,
        process,
        "platform",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            if cfg!(target_os = "linux") {
                "linux"
            } else if cfg!(target_os = "macos") {
                "darwin"
            } else if cfg!(windows) {
                "win32"
            } else {
                "unknown"
            }
            .to_string(),
        ))),
    );
    set_constant(
        rt,
        process,
        "arch",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            if cfg!(target_arch = "x86_64") {
                "x64"
            } else if cfg!(target_arch = "aarch64") {
                "arm64"
            } else {
                "unknown"
            }
            .to_string(),
        ))),
    );

    const DEFAULT_NODE_VERSION: &str = "26.3.0";
    const DEFAULT_NODE_MODULE_VERSION: f64 = 147.0;
    const DEFAULT_NAPI_VERSION: &str = "10";
    let node_version = std::env::var("CRUFT_NODE_VERSION")
        .ok()
        .map(|v| v.trim().trim_start_matches('v').to_string())
        .filter(|v| {
            let parts: Vec<&str> = v.split('.').collect();
            parts.len() == 3
                && parts
                    .iter()
                    .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        })
        .unwrap_or_else(|| DEFAULT_NODE_VERSION.to_string());
    set_constant(
        rt,
        process,
        "version",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(format!(
            "v{node_version}"
        )))),
    );

    let versions = new_object(rt);
    rt.obj_mut(versions)
        .set_own_internal("__process_versions__".into(), Value::Boolean(true));
    rt.object_set(
        versions,
        "node".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            node_version.clone(),
        ))),
    );
    rt.object_set(
        versions,
        "v8".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "14.6.202.34-node.20",
        ))),
    );
    rt.object_set(
        versions,
        "uv".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from("1.52.1"))),
    );
    rt.object_set(
        versions,
        "modules".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from("147"))),
    );

    rt.object_set(
        versions,
        "napi".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            DEFAULT_NAPI_VERSION,
        ))),
    );

    for (name, ver) in [
        ("acorn", "8.16.0"),
        ("ada", "3.4.4"),
        ("amaro", "1.1.9"),
        ("ares", "1.34.6"),
        ("brotli", "1.2.0"),
        ("cldr", "48.0"),
        ("icu", "78.3"),
        ("libffi", "3.5.2"),
        ("lief", "0.17.0"),
        ("llhttp", "9.4.1"),
        ("merve", "1.2.2"),
        ("nbytes", "0.1.4"),
        ("ncrypto", "0.0.1"),
        ("nghttp2", "1.69.0"),
        ("nghttp3", ""),
        ("ngtcp2", ""),
        ("openssl", "3.6.2"),
        ("simdjson", "4.6.4"),
        ("simdutf", "7.7.0"),
        ("sqlite", "3.53.2"),
        ("tz", "2026a"),
        ("undici", "8.3.0"),
        ("unicode", "17.0"),
        ("uvwasi", "0.0.23"),
        ("zlib", "1.2.12"),
        ("zstd", "1.5.7"),
    ] {
        rt.object_set(
            versions,
            name.into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(ver))),
        );
    }
    set_constant(rt, process, "versions", Value::Object(versions));
    set_constant(rt, process, "pid", Value::Number(std::process::id() as f64));
    #[cfg(unix)]
    let ppid = unsafe { libc::getppid() as f64 };
    #[cfg(not(unix))]
    let ppid = 1.0;
    set_constant(rt, process, "ppid", Value::Number(ppid));
    register_method(rt, process, "uptime", move |_rt, _args| {
        Ok(Value::Number(start.elapsed().as_secs_f64()))
    });

    register_method(rt, process, "umask", |_rt, _args| {
        Ok(Value::Number(0o022 as f64))
    });
    let features = new_object(rt);
    rt.obj_mut(features)
        .set_own_internal("__process_features__".into(), Value::Boolean(true));
    rt.object_set(features, "require_module".into(), Value::Boolean(true));

    rt.object_set(features, "inspector".into(), Value::Boolean(false));
    rt.object_set(features, "uv".into(), Value::Boolean(false));
    rt.object_set(features, "tls_ocsp".into(), Value::Boolean(false));
    rt.object_set(features, "cached_builtins".into(), Value::Boolean(false));
    rt.object_set(features, "tls".into(), Value::Boolean(true));

    rt.object_set(features, "tls_sni".into(), Value::Boolean(true));
    rt.object_set(features, "tls_alpn".into(), Value::Boolean(true));
    rt.object_set(features, "ipv6".into(), Value::Boolean(true));
    rt.object_set(features, "debug".into(), Value::Boolean(false));
    rt.object_set(
        features,
        "openssl_is_boringssl".into(),
        Value::Boolean(false),
    );
    rt.object_set(features, "quic".into(), Value::Boolean(false));

    rt.object_set(
        features,
        "typescript".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from("strip"))),
    );
    set_constant(rt, process, "features", Value::Object(features));

    for (name, fd_num) in [("stdout", 1.0), ("stderr", 2.0)] {
        let s = new_object(rt);

        use std::io::IsTerminal;
        let is_tty = if fd_num == 1.0 {
            std::io::stdout().is_terminal()
        } else {
            std::io::stderr().is_terminal()
        };
        rt.object_set(s, "fd".into(), Value::Number(fd_num));
        if is_tty {
            rt.object_set(s, "isTTY".into(), Value::Boolean(true));
            let cols = std::env::var("COLUMNS")
                .ok()
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(80.0);
            let rows = std::env::var("LINES")
                .ok()
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(24.0);
            rt.object_set(s, "columns".into(), Value::Number(cols));
            rt.object_set(s, "rows".into(), Value::Number(rows));
        }
        let fd = fd_num as u32;
        register_method(rt, s, "write", move |rt, args| {
            if let Some(Value::String(s)) = args.first() {

                let bytes = s.as_bytes().to_vec();
                let caller = current_caller(rt);
                let allow = std::env::var("CRUFT_STDIO_ALLOW").unwrap_or_default();
                let grant = caps::Stdio {
                    stdout: allow.split(',').any(|x| x.trim() == "stdout"),
                    stderr: allow.split(',').any(|x| x.trim() == "stderr"),
                };
                let op = if fd == 2 {
                    caps::StdioOp::Stderr(bytes)
                } else {
                    caps::StdioOp::Stdout(bytes)
                };
                rt.caps
                    .require_stdio(&grant, op, &caller)
                    .map_err(|e| RuntimeError::TypeError(e.to_string()))?;
                if fd == 2 {
                    eprint!("{}", s);
                } else {
                    print!("{}", s);
                }
            }
            Ok(Value::Boolean(true))
        });
        register_method(rt, s, "on", |rt, _args| Ok(rt.current_this()));

        rt.object_set(s, "writable".into(), Value::Boolean(true));
        rt.object_set(s, "writableEnded".into(), Value::Boolean(false));
        rt.object_set(s, "writableFinished".into(), Value::Boolean(false));
        rt.object_set(s, "writableCorked".into(), Value::Number(0.0));
        rt.object_set(s, "writableLength".into(), Value::Number(0.0));
        rt.object_set(s, "writableObjectMode".into(), Value::Boolean(false));
        rt.object_set(s, "writableHighWaterMark".into(), Value::Number(65536.0));
        rt.object_set(s, "destroyed".into(), Value::Boolean(false));
        register_method(rt, s, "cork", |_rt, _a| Ok(Value::Undefined));
        register_method(rt, s, "uncork", |_rt, _a| Ok(Value::Undefined));
        register_method(rt, s, "setDefaultEncoding", |rt, _a| Ok(rt.current_this()));
        register_method(rt, s, "destroy", |rt, _a| {
            if let Value::Object(id) = rt.current_this() {
                rt.object_set(id, "destroyed".into(), Value::Boolean(true));
            }
            Ok(rt.current_this())
        });

        register_method(rt, s, "end", |rt, args| {
            let this = rt.current_this();
            if let Some(chunk @ Value::String(_)) = args.first() {
                let write = match &this {
                    Value::Object(id) => rt.object_get(*id, "write"),
                    _ => Value::Undefined,
                };
                if rt.is_callable(&write) {
                    let _ = rt.call_function(write, this.clone(), vec![chunk.clone()]);
                }
            }
            if let Some(cb) = args.iter().rev().find(|v| rt.is_callable(v)).cloned() {
                let _ = rt.call_function(cb, Value::Undefined, Vec::new());
            }
            Ok(this)
        });
        set_constant(rt, process, name, Value::Object(s));
    }

    let argv0 = argv.first().cloned().unwrap_or_else(|| "cruft".to_string());
    set_constant(
        rt,
        process,
        "argv0",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(argv0))),
    );
    let exec_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "cruft".to_string());
    set_constant(
        rt,
        process,
        "execPath",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(exec_path))),
    );
    set_constant(
        rt,
        process,
        "title",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "cruft".to_string(),
        ))),
    );

    let release = new_object(rt);
    rt.obj_mut(release)
        .set_own_internal("__process_release__".into(), Value::Boolean(true));
    rt.object_set(
        release,
        "name".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "node".to_string(),
        ))),
    );

    rt.object_set(
        release,
        "sourceUrl".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "https://nodejs.org/download/release/v26.3.0/node-v26.3.0.tar.gz".to_string(),
        ))),
    );
    rt.object_set(
        release,
        "headersUrl".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "https://nodejs.org/download/release/v26.3.0/node-v26.3.0-headers.tar.gz".to_string(),
        ))),
    );
    set_constant(rt, process, "release", Value::Object(release));

    let flags_set = match rt.global_get("Set") {
        ctor if rt.is_callable(&ctor) => rt.construct(ctor, Vec::new()).unwrap_or(Value::Undefined),
        _ => Value::Undefined,
    };
    if let Value::Object(flags_id) = flags_set {
        rt.obj_mut(flags_id).set_own_internal(
            "__process_allowed_node_environment_flags__".into(),
            Value::Boolean(true),
        );
    }
    set_constant(rt, process, "allowedNodeEnvironmentFlags", flags_set);

    register_method(rt, process, "cpuUsage", |rt, args| {
        let (mut user, mut system) = process_cpu_usage_microseconds();
        if let Some((prev_user, prev_system)) = previous_cpu_usage_arg(rt, args) {
            user -= prev_user;
            system -= prev_system;
        }
        let o = new_object(rt);
        rt.obj_mut(o)
            .set_own_internal("__process_cpu_usage__".into(), Value::Boolean(true));
        rt.object_set(o, "user".into(), Value::Number(user));
        rt.object_set(o, "system".into(), Value::Number(system));
        Ok(Value::Object(o))
    });

    register_method(rt, process, "emitWarning", move |rt, args| {
        let message = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            Some(Value::Object(o)) => match rt.object_get(*o, "message") {
                Value::String(s) => s.as_str().to_string(),
                other => rusty_js_runtime::abstract_ops::to_string(&other)
                    .as_str()
                    .to_string(),
            },
            Some(other) => rusty_js_runtime::abstract_ops::to_string(other)
                .as_str()
                .to_string(),
            None => String::new(),
        };
        let mut name = "Warning".to_string();
        let mut code = Value::Undefined;
        let mut detail: Option<String> = None;
        if let Some(Value::String(s)) = args.get(1) {
            name = s.as_str().to_string();
        } else if let Some(Value::Object(o)) = args.get(1) {
            match rt.object_get(*o, "type") {
                Value::String(s) => name = s.as_str().to_string(),
                _ => {
                    if let Value::String(s) = rt.object_get(*o, "name") {
                        name = s.as_str().to_string();
                    }
                }
            }
            code = rt.object_get(*o, "code");
            if let Value::String(s) = rt.object_get(*o, "detail") {
                detail = Some(s.as_str().to_string());
            }
        }

        if let Some(Value::String(s)) = args.get(2) {
            code = Value::String(s.clone());
        }

        if name == "DeprecationWarning"
            && matches!(
                rt.object_get(process, "noDeprecation"),
                Value::Boolean(true)
            )
        {
            return Ok(Value::Undefined);
        }

        let warning = match rt.construct(
            rt.global_get("Error"),
            vec![Value::String(Rc::new(JsString::from(message.clone())))],
        ) {
            Ok(Value::Object(o)) => o,
            _ => new_object(rt),
        };
        rt.object_set(
            warning,
            "name".into(),
            Value::String(Rc::new(JsString::from(name.clone()))),
        );
        rt.object_set(
            warning,
            "message".into(),
            Value::String(Rc::new(JsString::from(message.clone()))),
        );
        let code_prefix = match &code {
            Value::String(s) if !s.as_str().is_empty() => format!("[{}] ", s.as_str()),
            _ => String::new(),
        };
        if !matches!(code, Value::Undefined) {
            rt.object_set(warning, "code".into(), code);
        }
        if let Some(source_url) = rt.current_module_url.last() {
            let stack = format!("{name}: {message}\n    at {source_url}");
            rt.object_set(
                warning,
                "stack".into(),
                Value::String(Rc::new(JsString::from(stack))),
            );
        }

        let stderr_line = if std::env::var_os("CRUFT_NO_WARNINGS").is_none() {

            let trace_flag = if name == "DeprecationWarning" {
                "trace-deprecation"
            } else {
                "trace-warnings"
            };

            static WARNING_HINT_SHOWN: std::sync::atomic::AtomicBool =
                std::sync::atomic::AtomicBool::new(false);
            let first = !WARNING_HINT_SHOWN.swap(true, std::sync::atomic::Ordering::Relaxed);
            let mut head = format!(
                "(node:{}) {}{}: {}",
                std::process::id(),
                code_prefix,
                name,
                message,
            );

            if let Some(d) = &detail {
                head.push('\n');
                head.push_str(d);
            }
            Some(if first {
                format!(
                    "{head}\n(Use `node --{trace_flag} ...` to show where the warning was created)"
                )
            } else {
                head
            })
        } else {
            None
        };
        rt.enqueue_nexttick_rooted("process.emitWarning", vec![warning], move |rt| {
            if let Some(line) = &stderr_line {
                rusty_js_runtime::interp::queue_node_warning(line.clone());
            }
            emit_process_event(rt, "warning", vec![Value::Object(warning)]);
            Ok(())
        });
        Ok(Value::Undefined)
    });
    register_method(
        rt,
        process,
        "hasUncaughtExceptionCaptureCallback",
        |_rt, _args| Ok(Value::Boolean(false)),
    );
    register_method(
        rt,
        process,
        "setUncaughtExceptionCaptureCallback",
        |_rt, _args| Ok(Value::Undefined),
    );

    register_method(rt, process, "on", |rt, args| {
        process_on_impl(rt, args, false, false);
        Ok(rt.current_this())
    });

    register_method(rt, process, "kill", |rt, args| {
        let pid = args
            .first()
            .map(|v| rusty_js_runtime::abstract_ops::to_number(v) as i32)
            .unwrap_or(0);
        let sig = match args.get(1) {
            None | Some(Value::Undefined) => libc::SIGTERM,
            Some(Value::String(s)) => match signal_number(s.as_str()) {
                Some(n) => n,
                None => {
                    return Err(RuntimeError::TypeError(format!(
                        "Unknown signal: {}",
                        s.as_str()
                    )))
                }
            },
            Some(v) => rusty_js_runtime::abstract_ops::to_number(v) as i32,
        };
        #[cfg(unix)]
        let rc = unsafe { libc::kill(pid, sig) };

        #[cfg(windows)]
        let rc = win_kill::kill(pid, sig);
        #[cfg(not(any(unix, windows)))]
        let rc: i32 = {
            let _ = (pid, sig);
            return Err(RuntimeError::TypeError(
                "process.kill is not supported on this platform".into(),
            ));
        };
        if rc != 0 {
            let errno = std::io::Error::last_os_error();
            let message = format!("kill ESRCH: no such process (pid {}): {}", pid, errno);
            return Err(
                match rusty_js_runtime::intrinsics::make_error_instance(rt, "Error", &message) {
                    Some(id) => RuntimeError::Thrown(Value::Object(id)),
                    None => RuntimeError::TypeError(message),
                },
            );
        }
        Ok(Value::Boolean(true))
    });
    register_method(rt, process, "abort", |_rt, _args| Ok(Value::Undefined));
    register_method(rt, process, "chdir", |rt, args| {
        if let Some(Value::String(s)) = args.first() {
            let _ = std::env::set_current_dir(s.as_str());
        }
        Ok(Value::Undefined)
    });

    fn proc_uid(effective: bool) -> f64 {
        std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|s| {
                s.lines().find(|l| l.starts_with("Uid:")).and_then(|l| {
                    let f: Vec<&str> = l.split_whitespace().collect();
                    f.get(if effective { 2 } else { 1 })
                        .and_then(|x| x.parse::<f64>().ok())
                })
            })
            .unwrap_or(0.0)
    }
    register_method(rt, process, "getuid", |_rt, _args| {
        Ok(Value::Number(proc_uid(false)))
    });
    register_method(rt, process, "geteuid", |_rt, _args| {
        Ok(Value::Number(proc_uid(true)))
    });
    register_method(rt, process, "availableMemory", |_rt, _args| {
        Ok(Value::Number(0.0))
    });
    register_method(rt, process, "constrainedMemory", |_rt, _args| {
        Ok(Value::Number(0.0))
    });
    register_method(rt, process, "loadEnvFile", |_rt, _args| {
        Ok(Value::Undefined)
    });

    register_method(rt, process, "cwd", |rt, _args| {
        check_process(rt, caps::ProcessOp::ReadCwd)?;
        let cwd = std::env::current_dir()
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "/".to_string());
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(cwd),
        )))
    });

    register_method(rt, process, "memoryUsage", |rt, _args| {
        let rss_bytes = process_resident_set_size_bytes();
        let obj = rt.alloc_object(rusty_js_runtime::value::Object::new_ordinary());
        rt.obj_mut(obj)
            .set_own_internal("__process_memory_usage__".into(), Value::Boolean(true));
        let array_buffer_bytes = rt
            .array_buffers
            .values()
            .filter(|buf| !buf.detached)
            .map(|buf| buf.byte_len())
            .sum::<usize>() as f64;
        let heap_used = process_memory_usage_heap_estimate_bytes(rt, false) as f64;
        let heap_total = (process_memory_usage_heap_estimate_bytes(rt, true) as f64).max(heap_used);
        rt.object_set(obj, "rss".into(), Value::Number(rss_bytes));
        rt.object_set(obj, "heapTotal".into(), Value::Number(heap_total));
        rt.object_set(obj, "heapUsed".into(), Value::Number(heap_used));
        rt.object_set(obj, "external".into(), Value::Number(array_buffer_bytes));
        rt.object_set(
            obj,
            "arrayBuffers".into(),
            Value::Number(array_buffer_bytes),
        );
        Ok(Value::Object(obj))
    });
    if let Value::Object(memory_usage) = rt.object_get(process, "memoryUsage") {
        let rss_fn = native_function(rt, "rss", |_rt, _args| {
            Ok(Value::Number(process_resident_set_size_bytes()))
        });
        rt.object_set(memory_usage, "rss".into(), rss_fn);
    }

    register_method(rt, process, "exit", |rt, args| {

        let code = match args.first() {
            None | Some(Value::Undefined) => current_exit_code(rt).unwrap_or(0),
            Some(v) => rusty_js_runtime::abstract_ops::to_number(v) as i32,
        };
        check_process(rt, caps::ProcessOp::Exit(code))?;

        if let Value::Object(p) = rt.current_this() {
            rt.object_set(p, "exitCode".into(), Value::Number(code as f64));
        }
        emit_process_event(rt, "exit", vec![Value::Number(code as f64)]);
        let final_code = current_exit_code(rt).unwrap_or(code);
        std::process::exit(final_code & 0xff);
    });

    register_method(rt, process, "hrtime", move |rt, args| {
        check_clock(rt, caps::ClockOp::HighResolution)?;
        let elapsed = start.elapsed();
        let mut secs = elapsed.as_secs() as i64;
        let mut nanos = elapsed.subsec_nanos() as i64;
        if let Some(Value::Object(previous)) = args.first() {
            let prev_secs = match rt.object_get(*previous, "0") {
                Value::Number(n) if n.is_finite() => n.trunc() as i64,
                _ => 0,
            };
            let prev_nanos = match rt.object_get(*previous, "1") {
                Value::Number(n) if n.is_finite() => n.trunc() as i64,
                _ => 0,
            };
            secs -= prev_secs;
            nanos -= prev_nanos;
            if nanos < 0 {
                secs -= 1;
                nanos += 1_000_000_000;
            }
            if secs < 0 {
                secs = 0;
                nanos = 0;
            }
        }
        let arr = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
        rt.obj_mut(arr)
            .set_own_internal("__process_hrtime__".into(), Value::Boolean(true));
        rt.object_set(arr, "0".into(), Value::Number(secs as f64));
        rt.object_set(arr, "1".into(), Value::Number(nanos as f64));
        Ok(Value::Object(arr))
    });

    if let rusty_js_runtime::Value::Object(hrtime_id) = rt.object_get(process, "hrtime") {
        let bigint_start = start;
        let bigint_fn: rusty_js_runtime::value::NativeFn = std::rc::Rc::new(move |rt, _args| {
            check_clock(rt, caps::ClockOp::HighResolution)?;
            let ns = bigint_start.elapsed().as_nanos() as i64;
            Ok(Value::BigInt(std::rc::Rc::new(
                rusty_js_runtime::bigint::JsBigInt::from_i64(ns),
            )))
        });
        let mut bigint_props = indexmap::IndexMap::new();
        rusty_js_runtime::value::install_function_meta_props(&mut bigint_props, "bigint", 0.0);
        let bigint_obj = rusty_js_runtime::value::Object {
            proto: None,
            extensible: true,
            properties: bigint_props,
            internal_kind: rusty_js_runtime::value::InternalKind::Function(Box::new(
                rusty_js_runtime::value::FunctionInternals {
                    name: "bigint".into(),
                    length: 0,
                    native: bigint_fn,
                    is_constructor: true,
                    creation_realm: 0,
                    roots: Vec::new(),
                },
            )),
            ..Default::default()
        };
        let bigint_id = rt.alloc_object(bigint_obj);
        rt.object_set(hrtime_id, "bigint".into(), Value::Object(bigint_id));
    }

    register_method(rt, process, "binding", |rt, args| {

        let name = match args.first() {
            Some(v) => rusty_js_runtime::abstract_ops::to_string(v)
                .as_str()
                .to_string(),
            None => "undefined".to_string(),
        };
        if name == "constants" {
            return Ok(process_binding_constants(rt));
        }
        if name == "fs" {
            return Ok(process_binding_fs(rt));
        }

        const KNOWN_BINDINGS: &[&str] = &[
            "natives",
            "buffer",
            "util",
            "config",
            "uv",
            "os",
            "tcp_wrap",
            "udp_wrap",
            "pipe_wrap",
            "process_wrap",
            "fs_event_wrap",
            "cares_wrap",
            "contextify",
            "icu",
            "spawn_sync",
            "stream_wrap",
            "tty_wrap",
            "zlib",
        ];
        if KNOWN_BINDINGS.contains(&name.as_str()) {
            let o = rt.alloc_object(rusty_js_runtime::value::Object::new_ordinary());
            rt.obj_mut(o)
                .set_own_internal("__process_binding_unknown__".into(), Value::Boolean(true));
            return Ok(Value::Object(o));
        }
        let message = format!("No such module: {name}");
        let ctor = rt.global_get("Error");
        let err = rt
            .construct(
                ctor,
                vec![Value::String(Rc::new(
                    rusty_js_runtime::value::JsString::from(message.clone()),
                ))],
            )
            .unwrap_or_else(|_| {
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(message)))
            });
        Err(RuntimeError::Thrown(err))
    });

    let report = rt.alloc_object(rusty_js_runtime::value::Object::new_ordinary());
    rt.object_set(report, "reportOnFatalError".into(), Value::Boolean(false));
    rt.object_set(report, "reportOnSignal".into(), Value::Boolean(false));
    rt.object_set(
        report,
        "reportOnUncaughtException".into(),
        Value::Boolean(false),
    );
    rt.object_set(
        report,
        "directory".into(),
        Value::String(std::rc::Rc::new(rusty_js_runtime::value::JsString::from(
            String::new(),
        ))),
    );

    rt.object_set(report, "compact".into(), Value::Boolean(false));
    rt.object_set(
        report,
        "filename".into(),
        Value::String(std::rc::Rc::new(rusty_js_runtime::value::JsString::from(
            String::new(),
        ))),
    );
    rt.object_set(
        report,
        "signal".into(),
        Value::String(std::rc::Rc::new(rusty_js_runtime::value::JsString::from(
            "SIGUSR2",
        ))),
    );
    register_method(rt, report, "writeReport", |_rt, _a| {
        Ok(Value::String(std::rc::Rc::new(
            rusty_js_runtime::value::JsString::from(String::new()),
        )))
    });
    let report_node_version = node_version.clone();
    register_method(rt, report, "getReport", move |rt, _a| {
        let snapshot = rt.alloc_object(rusty_js_runtime::value::Object::new_ordinary());
        rt.obj_mut(snapshot)
            .set_own_internal("__process_report_snapshot__".into(), Value::Boolean(true));

        let mkstr = |s: String| Value::String(Rc::new(rusty_js_runtime::value::JsString::from(s)));
        let header = rt.alloc_object(rusty_js_runtime::value::Object::new_ordinary());
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            let ptr = unsafe { libc::gnu_get_libc_version() };
            if !ptr.is_null() {
                let ver = unsafe { std::ffi::CStr::from_ptr(ptr) }
                    .to_string_lossy()
                    .into_owned();
                rt.object_set(header, "glibcVersionRuntime".into(), mkstr(ver));
            }
        }
        rt.object_set(header, "wordSize".into(), Value::Number(64.0));
        rt.object_set(
            header,
            "arch".into(),
            mkstr(
                if cfg!(target_arch = "x86_64") {
                    "x64"
                } else if cfg!(target_arch = "aarch64") {
                    "arm64"
                } else {
                    "unknown"
                }
                .to_string(),
            ),
        );
        rt.object_set(
            header,
            "platform".into(),
            mkstr(
                if cfg!(target_os = "linux") {
                    "linux"
                } else if cfg!(target_os = "macos") {
                    "darwin"
                } else if cfg!(windows) {
                    "win32"
                } else {
                    "unknown"
                }
                .to_string(),
            ),
        );
        rt.object_set(
            header,
            "nodejsVersion".into(),
            mkstr(format!("v{report_node_version}")),
        );
        rt.object_set(snapshot, "header".into(), Value::Object(header));
        Ok(Value::Object(snapshot))
    });
    rt.object_set(process, "report".into(), Value::Object(report));

    register_method(rt, process, "nextTick", |rt, args| {
        rt.require_microtask_cap()?;
        if let Some(cb) = args.first().cloned() {
            let rest: Vec<Value> = args.iter().skip(1).cloned().collect();
            let resource = new_object(rt);
            let _ = crate::node_stubs::async_hooks_emit_init_for_global(
                rt,
                "TickObject",
                Value::Object(resource),
            )?;
            let mut roots = crate::timer::roots_for_callback(&cb, &rest);
            roots.push(resource);
            rt.enqueue_nexttick_rooted("process.nextTick", roots, move |rt| {
                crate::node_stubs::async_hooks_call_with_global_resource(
                    rt,
                    resource,
                    cb,
                    Value::Undefined,
                    rest,
                )
                .map(|_| ())
            });
        }
        Ok(Value::Undefined)
    });
    register_method(rt, process, "emit", |rt, args| {
        let name = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => return Ok(Value::Boolean(false)),
        };
        let rest: Vec<Value> = args.iter().skip(1).cloned().collect();
        Ok(Value::Boolean(emit_process_event(rt, &name, rest)))
    });
    register_method(rt, process, "once", |rt, args| {
        process_on_impl(rt, args, true, false);
        Ok(rt.current_this())
    });
    register_method(rt, process, "addListener", |rt, args| {
        process_on_impl(rt, args, false, false);
        Ok(rt.current_this())
    });
    register_method(rt, process, "prependListener", |rt, args| {
        process_on_impl(rt, args, false, true);
        Ok(rt.current_this())
    });
    register_method(rt, process, "prependOnceListener", |rt, args| {
        process_on_impl(rt, args, true, true);
        Ok(rt.current_this())
    });
    let remove_listener = |rt: &mut Runtime, args: &[Value]| {
        if let (Some(Value::String(name)), Some(cb)) = (args.first(), args.get(1)) {
            let process = match rt.current_this() {
                Value::Object(p) => p,
                _ => return,
            };
            for sentinel in ["__proc_events", "__proc_once"] {
                if let Some(arr) = proc_event_array(rt, process, sentinel, name.as_str(), false) {
                    let len = rt.array_length(arr);
                    let mut kept = Vec::new();
                    for i in 0..len {
                        let v = rt.object_get(arr, &i.to_string());
                        if !rusty_js_runtime::abstract_ops::same_value(&v, cb) {
                            kept.push(v);
                        }
                    }
                    for (i, v) in kept.iter().enumerate() {
                        rt.object_set(arr, i.to_string(), v.clone());
                    }
                    rt.object_set(arr, "length".into(), Value::Number(kept.len() as f64));
                }
            }
        }
    };
    register_method(rt, process, "off", move |rt, args| {
        remove_listener(rt, args);
        Ok(rt.current_this())
    });
    register_method(rt, process, "removeListener", move |rt, args| {
        remove_listener(rt, args);
        Ok(rt.current_this())
    });
    register_method(rt, process, "removeAllListeners", |rt, args| {
        let process = match rt.current_this() {
            Value::Object(p) => p,
            _ => return Ok(rt.current_this()),
        };
        for sentinel in ["__proc_events", "__proc_once"] {
            match args.first() {
                Some(Value::String(name)) => {
                    if let Some(arr) = proc_event_array(rt, process, sentinel, name.as_str(), false)
                    {
                        rt.object_set(arr, "length".into(), Value::Number(0.0));
                    }
                }
                _ => {
                    let empty = new_object(rt);
                    rt.set_engine_sentinel(process, sentinel, Value::Object(empty));
                }
            }
        }
        Ok(rt.current_this())
    });
    register_method(rt, process, "listeners", |rt, args| {
        let out = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
        if let (Value::Object(process), Some(Value::String(name))) =
            (rt.current_this(), args.first())
        {
            let mut all = proc_snapshot(rt, process, "__proc_events", name.as_str());
            all.extend(proc_snapshot(rt, process, "__proc_once", name.as_str()));
            for (i, v) in all.iter().enumerate() {
                rt.object_set(out, i.to_string(), v.clone());
            }
            rt.object_set(out, "length".into(), Value::Number(all.len() as f64));
        }
        Ok(Value::Object(out))
    });
    register_method(rt, process, "rawListeners", |rt, args| {
        let out = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
        if let (Value::Object(process), Some(Value::String(name))) =
            (rt.current_this(), args.first())
        {
            let mut all = proc_snapshot(rt, process, "__proc_events", name.as_str());
            all.extend(proc_snapshot(rt, process, "__proc_once", name.as_str()));
            for (i, v) in all.iter().enumerate() {
                rt.object_set(out, i.to_string(), v.clone());
            }
            rt.object_set(out, "length".into(), Value::Number(all.len() as f64));
        }
        Ok(Value::Object(out))
    });
    register_method(rt, process, "listenerCount", |rt, args| {
        if let (Value::Object(process), Some(Value::String(name))) =
            (rt.current_this(), args.first())
        {
            return Ok(Value::Number(
                proc_listener_count(rt, process, name.as_str()) as f64,
            ));
        }
        Ok(Value::Number(0.0))
    });
    register_method(rt, process, "eventNames", |rt, _args| {
        let events = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
        rt.obj_mut(events)
            .set_own_internal("__process_event_names__".into(), Value::Boolean(true));
        if let Value::Object(process) = rt.current_this() {
            let names = proc_collect_event_names(rt, process);
            for (i, name) in names.iter().enumerate() {
                rt.object_set(
                    events,
                    i.to_string(),
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                        name.clone(),
                    ))),
                );
            }
            rt.object_set(events, "length".into(), Value::Number(names.len() as f64));
        }
        Ok(Value::Object(events))
    });
    register_method(rt, process, "setMaxListeners", |rt, args| {
        let n = args
            .first()
            .map(rusty_js_runtime::abstract_ops::to_number)
            .unwrap_or(10.0);
        if !n.is_finite() || n < 0.0 {
            return Err(RuntimeError::RangeError(format!(
                "The value of \"setMaxListeners\" is out of range. It must be >= 0. Received {n}"
            )));
        }
        if let Value::Object(process) = rt.current_this() {
            rt.set_engine_sentinel(process, "__proc_max_listeners", Value::Number(n));
        }
        Ok(rt.current_this())
    });
    register_method(rt, process, "getMaxListeners", |rt, _args| {
        if let Value::Object(process) = rt.current_this() {
            if let Value::Number(n) = rt.object_get(process, "__proc_max_listeners") {
                return Ok(Value::Number(n));
            }
        }
        Ok(Value::Number(10.0))
    });

    register_method(rt, process, "getBuiltinModule", |rt, args| {
        let name = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => return Ok(Value::Undefined),
        };
        let stripped = name.strip_prefix("node:").unwrap_or(&name);
        let global_name = match stripped {
            "dns/promises" => "dns_promises",
            "fs/promises" => "fs_promises",
            "module" => "__node_module",
            _ => stripped,
        };

        rt.materialize_lazy_host_module(global_name);
        Ok(rt.global_get(global_name))
    });

    for f in [
        "_debugEnd",
        "_debugProcess",
        "_fatalException",
        "_getActiveHandles",
        "_getActiveRequests",
        "_kill",
        "_linkedBinding",
        "_rawDebug",
        "_startProfilerIdleNotifier",
        "_stopProfilerIdleNotifier",
        "_tickCallback",
        "addUncaughtExceptionCaptureCallback",
        "dlopen",
        "execve",
        "initgroups",
        "openStdin",
        "reallyExit",
        "setSourceMapsEnabled",
        "setegid",
        "seteuid",
        "setgid",
        "setgroups",
        "setuid",
    ] {
        register_method(rt, process, f, |_rt, _a| Ok(Value::Undefined));
    }

    for g in ["getegid", "getgid"] {
        register_method(rt, process, g, |_rt, _a| Ok(Value::Number(0.0)));
    }
    register_method(rt, process, "getgroups", |rt, _a| {
        let groups = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
        rt.obj_mut(groups)
            .set_own_internal("__process_groups__".into(), Value::Boolean(true));
        #[cfg(unix)]
        {
            let count = unsafe { libc::getgroups(0, std::ptr::null_mut()) };
            if count > 0 {
                let mut gids = vec![0 as libc::gid_t; count as usize];
                let actual = unsafe { libc::getgroups(count, gids.as_mut_ptr()) };
                if actual > 0 {
                    for (index, gid) in gids.into_iter().take(actual as usize).enumerate() {
                        rt.object_set(groups, index.to_string(), Value::Number(gid as f64));
                    }
                }
            }
        }
        Ok(Value::Object(groups))
    });
    register_method(rt, process, "getActiveResourcesInfo", |rt, _a| {
        let resources = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
        rt.obj_mut(resources)
            .set_own_internal("__process_active_resources__".into(), Value::Boolean(true));
        Ok(Value::Object(resources))
    });
    register_method(rt, process, "resourceUsage", |rt, _a| {
        let obj = new_object(rt);
        rt.obj_mut(obj)
            .set_own_internal("__process_resource_usage__".into(), Value::Boolean(true));
        for field in [
            "userCPUTime",
            "systemCPUTime",
            "maxRSS",
            "sharedMemorySize",
            "unsharedDataSize",
            "unsharedStackSize",
            "minorPageFault",
            "majorPageFault",
            "swappedOut",
            "fsRead",
            "fsWrite",
            "ipcSent",
            "ipcReceived",
            "signalsCount",
            "voluntaryContextSwitches",
            "involuntaryContextSwitches",
        ] {
            rt.object_set(obj, field.into(), Value::Number(0.0));
        }
        #[cfg(unix)]
        {
            let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
            if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) } == 0 {
                rt.object_set(
                    obj,
                    "userCPUTime".into(),
                    Value::Number(timeval_to_microseconds(usage.ru_utime)),
                );
                rt.object_set(
                    obj,
                    "systemCPUTime".into(),
                    Value::Number(timeval_to_microseconds(usage.ru_stime)),
                );
                rt.object_set(
                    obj,
                    "maxRSS".into(),
                    Value::Number(resource_usage_max_rss_kib(&usage)),
                );
                rt.object_set(
                    obj,
                    "sharedMemorySize".into(),
                    Value::Number(usage.ru_ixrss as f64),
                );
                rt.object_set(
                    obj,
                    "unsharedDataSize".into(),
                    Value::Number(usage.ru_idrss as f64),
                );
                rt.object_set(
                    obj,
                    "unsharedStackSize".into(),
                    Value::Number(usage.ru_isrss as f64),
                );
                rt.object_set(
                    obj,
                    "minorPageFault".into(),
                    Value::Number(usage.ru_minflt as f64),
                );
                rt.object_set(
                    obj,
                    "majorPageFault".into(),
                    Value::Number(usage.ru_majflt as f64),
                );
                rt.object_set(
                    obj,
                    "swappedOut".into(),
                    Value::Number(usage.ru_nswap as f64),
                );
                rt.object_set(obj, "fsRead".into(), Value::Number(usage.ru_inblock as f64));
                rt.object_set(
                    obj,
                    "fsWrite".into(),
                    Value::Number(usage.ru_oublock as f64),
                );
                rt.object_set(obj, "ipcSent".into(), Value::Number(usage.ru_msgsnd as f64));
                rt.object_set(
                    obj,
                    "ipcReceived".into(),
                    Value::Number(usage.ru_msgrcv as f64),
                );
                rt.object_set(
                    obj,
                    "signalsCount".into(),
                    Value::Number(usage.ru_nsignals as f64),
                );
                rt.object_set(
                    obj,
                    "voluntaryContextSwitches".into(),
                    Value::Number(usage.ru_nvcsw as f64),
                );
                rt.object_set(
                    obj,
                    "involuntaryContextSwitches".into(),
                    Value::Number(usage.ru_nivcsw as f64),
                );
            }
        }
        Ok(Value::Object(obj))
    });
    register_method(rt, process, "threadCpuUsage", |rt, args| {
        let obj = new_object(rt);
        rt.obj_mut(obj)
            .set_own_internal("__process_thread_cpu_usage__".into(), Value::Boolean(true));
        let (mut user, mut system) = thread_cpu_usage_microseconds();
        if let Some((prev_user, prev_system)) = previous_cpu_usage_arg(rt, args) {
            user -= prev_user;
            system -= prev_system;
        }
        rt.object_set(obj, "user".into(), Value::Number(user));
        rt.object_set(obj, "system".into(), Value::Number(system));
        Ok(Value::Object(obj))
    });
    register_method(rt, process, "ref", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, process, "unref", |_rt, _a| Ok(Value::Undefined));

    rt.object_set(process, "_eventsCount".into(), Value::Number(0.0));
    rt.object_set(process, "debugPort".into(), Value::Number(9229.0));
    rt.object_set(process, "_exiting".into(), Value::Boolean(false));
    rt.object_set(process, "sourceMapsEnabled".into(), Value::Boolean(false));
    rt.object_set(process, "_maxListeners".into(), Value::Undefined);
    rt.object_set(process, "exitCode".into(), Value::Undefined);
    rt.object_set(process, "domain".into(), Value::Null);
    let events = new_object(rt);
    rt.obj_mut(events)
        .set_own_internal("__process_events__".into(), Value::Boolean(true));
    rt.object_set(process, "_events".into(), Value::Object(events));
    let finalization = new_object(rt);
    rt.obj_mut(finalization)
        .set_own_internal("__process_finalization__".into(), Value::Boolean(true));
    rt.object_set(process, "finalization".into(), Value::Object(finalization));

    let config = new_object(rt);
    rt.obj_mut(config)
        .set_own_internal("__process_config__".into(), Value::Boolean(true));

    let target_defaults = new_object(rt);
    for arr_field in ["cflags", "defines", "include_dirs", "libraries"] {
        let arr = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
        rt.object_set(target_defaults, arr_field.into(), Value::Object(arr));
    }
    rt.object_set(
        target_defaults,
        "default_configuration".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "Release".to_string(),
        ))),
    );
    rt.object_set(
        config,
        "target_defaults".into(),
        Value::Object(target_defaults),
    );

    let variables = new_object(rt);
    let vstr = |rt: &mut Runtime, obj, key: &str, val: &str| {
        rt.object_set(
            obj,
            key.into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                val.to_string(),
            ))),
        );
    };
    let node_arch = if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "unknown"
    };
    vstr(rt, variables, "host_arch", node_arch);
    vstr(rt, variables, "target_arch", node_arch);
    vstr(rt, variables, "node_byteorder", "little");
    vstr(rt, variables, "napi_build_version", DEFAULT_NAPI_VERSION);
    vstr(rt, variables, "arm_version", "");
    vstr(
        rt,
        variables,
        "arm_fpu",
        if cfg!(target_arch = "aarch64") {
            "neon"
        } else {
            ""
        },
    );
    rt.object_set(
        variables,
        "v8_enable_i18n_support".into(),
        Value::Number(1.0),
    );
    rt.object_set(
        variables,
        "node_module_version".into(),
        Value::Number(DEFAULT_NODE_MODULE_VERSION),
    );
    rt.object_set(variables, "node_use_openssl".into(), Value::Boolean(true));
    rt.object_set(variables, "node_shared".into(), Value::Boolean(false));
    rt.object_set(config, "variables".into(), Value::Object(variables));
    rt.object_set(process, "config".into(), Value::Object(config));
    let preload_modules = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
    rt.obj_mut(preload_modules)
        .set_own_internal("__process_preload_modules__".into(), Value::Boolean(true));
    rt.object_set(
        process,
        "_preload_modules".into(),
        Value::Object(preload_modules),
    );
    let module_load_list = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
    rt.obj_mut(module_load_list)
        .set_own_internal("__process_module_load_list__".into(), Value::Boolean(true));
    install_bootstrap_module_load_list(rt, module_load_list);
    rt.object_set(
        process,
        "moduleLoadList".into(),
        Value::Object(module_load_list),
    );

    for m in [
        "addListener",
        "emit",
        "eventNames",
        "getMaxListeners",
        "listenerCount",
        "listeners",
        "off",
        "on",
        "once",
        "prependListener",
        "prependOnceListener",
        "rawListeners",
        "removeAllListeners",
        "removeListener",
        "setMaxListeners",
    ] {
        let v = rt.object_get(process, m);
        if rt.is_callable(&v) {
            rt.set_engine_sentinel(process, m, v);
        }
    }

    rt.define_global_property("process", Value::Object(process));
}

pub fn install_stdio_event_emitters(rt: &mut Runtime, process: ObjectRef) {
    for name in ["stdout", "stderr"] {
        if let Value::Object(stream) = rt.object_get(process, name) {
            crate::net::install_emitter_methods_own(rt, stream);
        }
    }
}

pub fn wire_event_emitter_prototype(rt: &mut Runtime, process: ObjectRef) {
    let process_proto = match rt.object_get(process, "constructor") {
        Value::Object(ctor) => match rt.object_get(ctor, "prototype") {
            Value::Object(proto) => proto,
            _ => return,
        },
        _ => return,
    };
    let event_emitter_proto = match rt.global_get("events") {
        Value::Object(ctor) => match rt.object_get(ctor, "prototype") {
            Value::Object(proto) => proto,
            _ => return,
        },
        _ => return,
    };
    rt.set_object_prototype_internal(process_proto, Some(event_emitter_proto));
}

pub fn install_canonical(rt: &mut Runtime) {
    let ns = new_object(rt);
    let src = match rt.global_get("process") {
        Value::Object(id) => id,
        _ => {
            rt.define_global_property("__cruft_process", Value::Object(ns));
            return;
        }
    };

    for name in [
        "argv",
        "argv0",
        "env",
        "cwd",
        "exit",
        "pid",
        "platform",
        "arch",
        "version",
        "versions",
        "nextTick",
        "hrtime",
        "memoryUsage",
        "uptime",
        "title",
    ] {
        let v = rt.object_get(src, name);
        if !matches!(v, Value::Undefined) {
            rt.object_set(ns, name.into(), v);
        }
    }
    rt.object_set(ns, "default".into(), Value::Object(ns));
    rt.define_global_property("__cruft_process", Value::Object(ns));
}
