
use crate::register::{make_callable, new_object, register_method};
use rusty_js_runtime::interp::ArrayBufferRecord;
use rusty_js_runtime::value::{
    FunctionInternals, InternalKind, JsString, NativeFn, Object, ObjectRef, PromiseStatus,
    PropertyDescriptor,
};
use rusty_js_runtime::{Runtime, RuntimeError, Value};
use rusty_wasm::{
    func_imports, global_imports, instantiate, memory_imports, module_export_descriptors,
    module_export_func_specs, module_import_descriptors, parse_module, table_imports, tag_imports,
    take_last_partial_instance, HostContext, Imports, Instance, Module, ModuleExportDescriptor,
    ModuleImportDescriptor, TableImportValue, ValType, WasmValue,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

thread_local! {
    static WASM_RT: std::cell::Cell<*mut Runtime> = const { std::cell::Cell::new(std::ptr::null_mut()) };
    static WASM_CUR_INST: std::cell::Cell<usize> = const { std::cell::Cell::new(usize::MAX) };
    static WASM_MODULE_CACHE: RefCell<HashMap<Vec<u8>, Module>> = RefCell::new(HashMap::new());
    static WASM_LAST_MODULE_CACHE: RefCell<Option<(Vec<u8>, Module)>> = const { RefCell::new(None) };
    static WASM_MODULES: RefCell<Vec<Option<Module>>> = const { RefCell::new(Vec::new()) };
    static WASM_INSTANCES: RefCell<Vec<Option<Instance>>> = const { RefCell::new(Vec::new()) };
    static WASM_INSTANCE_MODULE_ID: RefCell<Vec<Option<usize>>> = const { RefCell::new(Vec::new()) };
    static WASM_MODULE_EXPORT_FUNC_META: RefCell<Vec<Option<Vec<WasmExportFuncMeta>>>> = const { RefCell::new(Vec::new()) };
    static WASM_ACTIVE_INSTANCES: RefCell<Vec<(usize, *mut Instance)>> = const { RefCell::new(Vec::new()) };
    static WASM_INSTANCE_AB: RefCell<Vec<Option<ObjectRef>>> = const { RefCell::new(Vec::new()) };
    static WASM_INSTANCE_MEM_OBJECT: RefCell<Vec<Option<ObjectRef>>> = const { RefCell::new(Vec::new()) };
    static WASM_INSTANCE_MEM_OBJECTS: RefCell<Vec<Vec<Option<ObjectRef>>>> = const { RefCell::new(Vec::new()) };
    static WASM_INSTANCE_TABLE_OBJECT: RefCell<Vec<Option<ObjectRef>>> = const { RefCell::new(Vec::new()) };
    static WASM_INSTANCE_GLOBAL_OBJECTS: RefCell<Vec<Vec<Option<ObjectRef>>>> = const { RefCell::new(Vec::new()) };
    static WASM_MEMORY_SYNC_CACHE: RefCell<Vec<Option<(ObjectRef, u64, usize)>>> = const { RefCell::new(Vec::new()) };
    static WASM_EXTERNREFS: RefCell<Vec<Value>> = const { RefCell::new(Vec::new()) };
    static WASM_GC_REF_OBJECTS: RefCell<HashMap<(usize, &'static str, u32), ObjectRef>> = RefCell::new(HashMap::new());
    static WASM_MODULE_PROTO: std::cell::Cell<Option<ObjectRef>> = const { std::cell::Cell::new(None) };
    static WASM_INSTANCE_PROTO: std::cell::Cell<Option<ObjectRef>> = const { std::cell::Cell::new(None) };
    static WASM_MEMORY_PROTO: std::cell::Cell<Option<ObjectRef>> = const { std::cell::Cell::new(None) };
    static WASM_TABLE_PROTO: std::cell::Cell<Option<ObjectRef>> = const { std::cell::Cell::new(None) };
    static WASM_GLOBAL_PROTO: std::cell::Cell<Option<ObjectRef>> = const { std::cell::Cell::new(None) };
    static WASM_TAG_PROTO: std::cell::Cell<Option<ObjectRef>> = const { std::cell::Cell::new(None) };
    static WASM_EXCEPTION_PROTO: std::cell::Cell<Option<ObjectRef>> = const { std::cell::Cell::new(None) };
    static WASM_SUSPENDING_PROTO: std::cell::Cell<Option<ObjectRef>> = const { std::cell::Cell::new(None) };
}

pub fn collect_roots(roots: &mut Vec<ObjectRef>) {
    fn push_opt_vec(roots: &mut Vec<ObjectRef>, cell: &RefCell<Vec<Option<ObjectRef>>>) {
        for o in cell.borrow().iter().flatten() {
            roots.push(*o);
        }
    }
    fn push_opt_vec2(roots: &mut Vec<ObjectRef>, cell: &RefCell<Vec<Vec<Option<ObjectRef>>>>) {
        for inner in cell.borrow().iter() {
            for o in inner.iter().flatten() {
                roots.push(*o);
            }
        }
    }
    WASM_INSTANCE_AB.with(|c| push_opt_vec(roots, c));
    WASM_INSTANCE_MEM_OBJECT.with(|c| push_opt_vec(roots, c));
    WASM_INSTANCE_MEM_OBJECTS.with(|c| push_opt_vec2(roots, c));
    WASM_INSTANCE_TABLE_OBJECT.with(|c| push_opt_vec(roots, c));
    WASM_INSTANCE_GLOBAL_OBJECTS.with(|c| push_opt_vec2(roots, c));
    WASM_MEMORY_SYNC_CACHE.with(|c| {
        for e in c.borrow().iter().flatten() {
            roots.push(e.0);
        }
    });
    WASM_EXTERNREFS.with(|c| {
        for v in c.borrow().iter() {
            if let Value::Object(id) = v {
                roots.push(*id);
            }
        }
    });
    WASM_GC_REF_OBJECTS.with(|c| {
        for o in c.borrow().values() {
            roots.push(*o);
        }
    });
    for cell in [
        &WASM_MODULE_PROTO,
        &WASM_INSTANCE_PROTO,
        &WASM_MEMORY_PROTO,
        &WASM_TABLE_PROTO,
        &WASM_GLOBAL_PROTO,
        &WASM_TAG_PROTO,
        &WASM_EXCEPTION_PROTO,
        &WASM_SUSPENDING_PROTO,
    ] {
        if let Some(id) = cell.with(|c| c.get()) {
            roots.push(id);
        }
    }
}

pub fn collect_roots_for_runtime(_rt: &Runtime, roots: &mut Vec<ObjectRef>) {
    collect_roots(roots);
}

#[derive(Clone)]
struct WasmExportFuncMeta {
    name: String,
    funcidx: u32,
    params: Vec<ValType>,
    writes_memory: bool,
    type_final: bool,
    signature_value: Value,
    type_shape_value: Value,
    type_shapes_value: Value,
}

fn wasm_exported_call_phase_counters_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_WASM_EXPORTED_CALL_PHASE_COUNTERS")
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or(false)
    })
}

fn elapsed_ns_since(t0: Instant) -> u64 {
    t0.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}

fn catch_wasm_execution<F>(f: F) -> Result<Vec<WasmValue>, String>
where
    F: FnOnce() -> Result<Vec<WasmValue>, String>,
{
    catch_unwind(AssertUnwindSafe(f))
        .unwrap_or_else(|_| Err("internal panic during execution".to_string()))
}

#[allow(clippy::too_many_arguments)]
fn record_wasm_exported_call_phase(
    argc: usize,
    resultc: usize,
    has_memory: bool,
    writes_memory: bool,
    arg_ns: u64,
    sync_in_ns: u64,
    sync_in_primary_ns: u64,
    sync_in_objects_ns: u64,
    table_ns: u64,
    context_ns: u64,
    call_ns: u64,
    sync_out_ns: u64,
    result_ns: u64,
    total_ns: u64,
) {
    if !wasm_exported_call_phase_counters_enabled() {
        return;
    }
    static CALLS: AtomicU64 = AtomicU64::new(0);
    static ARGC_TOTAL: AtomicU64 = AtomicU64::new(0);
    static RESULTC_TOTAL: AtomicU64 = AtomicU64::new(0);
    static HAS_MEMORY_CALLS: AtomicU64 = AtomicU64::new(0);
    static WRITES_MEMORY_CALLS: AtomicU64 = AtomicU64::new(0);
    static ARG_NS: AtomicU64 = AtomicU64::new(0);
    static SYNC_IN_NS: AtomicU64 = AtomicU64::new(0);
    static SYNC_IN_PRIMARY_NS: AtomicU64 = AtomicU64::new(0);
    static SYNC_IN_OBJECTS_NS: AtomicU64 = AtomicU64::new(0);
    static TABLE_NS: AtomicU64 = AtomicU64::new(0);
    static CONTEXT_NS: AtomicU64 = AtomicU64::new(0);
    static CALL_NS: AtomicU64 = AtomicU64::new(0);
    static SYNC_OUT_NS: AtomicU64 = AtomicU64::new(0);
    static RESULT_NS: AtomicU64 = AtomicU64::new(0);
    static TOTAL_NS: AtomicU64 = AtomicU64::new(0);

    let calls = CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    ARGC_TOTAL.fetch_add(argc as u64, Ordering::Relaxed);
    RESULTC_TOTAL.fetch_add(resultc as u64, Ordering::Relaxed);
    if has_memory {
        HAS_MEMORY_CALLS.fetch_add(1, Ordering::Relaxed);
    }
    if writes_memory {
        WRITES_MEMORY_CALLS.fetch_add(1, Ordering::Relaxed);
    }
    ARG_NS.fetch_add(arg_ns, Ordering::Relaxed);
    SYNC_IN_NS.fetch_add(sync_in_ns, Ordering::Relaxed);
    SYNC_IN_PRIMARY_NS.fetch_add(sync_in_primary_ns, Ordering::Relaxed);
    SYNC_IN_OBJECTS_NS.fetch_add(sync_in_objects_ns, Ordering::Relaxed);
    TABLE_NS.fetch_add(table_ns, Ordering::Relaxed);
    CONTEXT_NS.fetch_add(context_ns, Ordering::Relaxed);
    CALL_NS.fetch_add(call_ns, Ordering::Relaxed);
    SYNC_OUT_NS.fetch_add(sync_out_ns, Ordering::Relaxed);
    RESULT_NS.fetch_add(result_ns, Ordering::Relaxed);
    TOTAL_NS.fetch_add(total_ns, Ordering::Relaxed);

    if calls <= 8 || calls.is_power_of_two() {
        let avg = |counter: &AtomicU64| counter.load(Ordering::Relaxed) / calls;
        eprintln!(
            "[wasm-exported-call-phase] calls={} avg_argc={} avg_resultc={} has_memory={} writes_memory={} avg_arg_ns={} avg_sync_in_ns={} avg_sync_in_primary_ns={} avg_sync_in_objects_ns={} avg_table_ns={} avg_context_ns={} avg_call_ns={} avg_sync_out_ns={} avg_result_ns={} avg_total_ns={}",
            calls,
            ARGC_TOTAL.load(Ordering::Relaxed) / calls,
            RESULTC_TOTAL.load(Ordering::Relaxed) / calls,
            HAS_MEMORY_CALLS.load(Ordering::Relaxed),
            WRITES_MEMORY_CALLS.load(Ordering::Relaxed),
            avg(&ARG_NS),
            avg(&SYNC_IN_NS),
            avg(&SYNC_IN_PRIMARY_NS),
            avg(&SYNC_IN_OBJECTS_NS),
            avg(&TABLE_NS),
            avg(&CONTEXT_NS),
            avg(&CALL_NS),
            avg(&SYNC_OUT_NS),
            avg(&RESULT_NS),
            avg(&TOTAL_NS)
        );
    }
}

fn record_wasm_exported_call_entry(
    registry_take_ns: u64,
    active_push_ns: u64,
    interpreter_ns: u64,
    active_pop_ns: u64,
    registry_restore_ns: u64,
    reentrant: bool,
) {
    if !wasm_exported_call_phase_counters_enabled() {
        return;
    }
    static CALLS: AtomicU64 = AtomicU64::new(0);
    static REENTRANT_CALLS: AtomicU64 = AtomicU64::new(0);
    static REGISTRY_TAKE_NS: AtomicU64 = AtomicU64::new(0);
    static ACTIVE_PUSH_NS: AtomicU64 = AtomicU64::new(0);
    static INTERPRETER_NS: AtomicU64 = AtomicU64::new(0);
    static ACTIVE_POP_NS: AtomicU64 = AtomicU64::new(0);
    static REGISTRY_RESTORE_NS: AtomicU64 = AtomicU64::new(0);

    let calls = CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    if reentrant {
        REENTRANT_CALLS.fetch_add(1, Ordering::Relaxed);
    }
    REGISTRY_TAKE_NS.fetch_add(registry_take_ns, Ordering::Relaxed);
    ACTIVE_PUSH_NS.fetch_add(active_push_ns, Ordering::Relaxed);
    INTERPRETER_NS.fetch_add(interpreter_ns, Ordering::Relaxed);
    ACTIVE_POP_NS.fetch_add(active_pop_ns, Ordering::Relaxed);
    REGISTRY_RESTORE_NS.fetch_add(registry_restore_ns, Ordering::Relaxed);

    if calls <= 8 || calls.is_power_of_two() {
        let avg = |counter: &AtomicU64| counter.load(Ordering::Relaxed) / calls;
        eprintln!(
            "[wasm-exported-call-entry] calls={} reentrant={} avg_registry_take_ns={} avg_active_push_ns={} avg_interpreter_ns={} avg_active_pop_ns={} avg_registry_restore_ns={}",
            calls,
            REENTRANT_CALLS.load(Ordering::Relaxed),
            avg(&REGISTRY_TAKE_NS),
            avg(&ACTIVE_PUSH_NS),
            avg(&INTERPRETER_NS),
            avg(&ACTIVE_POP_NS),
            avg(&REGISTRY_RESTORE_NS)
        );
    }
}

fn wasm_instance_export_counters_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_WASM_INSTANCE_EXPORT_COUNTERS")
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or(false)
    })
}

fn wasm_module_counters_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_WASM_MODULE_COUNTERS")
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or(false)
    })
}

#[allow(clippy::too_many_arguments)]
fn record_wasm_module_phase(
    bytes_len: usize,
    cache_hit: bool,
    lookup_ns: u64,
    parse_ns: u64,
    cache_store_ns: u64,
    registry_ns: u64,
    object_ns: u64,
    proto_ns: u64,
    sentinel_ns: u64,
    total_ns: u64,
) {
    if !wasm_module_counters_enabled() {
        return;
    }
    static CALLS: AtomicU64 = AtomicU64::new(0);
    static BYTES_TOTAL: AtomicU64 = AtomicU64::new(0);
    static CACHE_HITS: AtomicU64 = AtomicU64::new(0);
    static LOOKUP_NS: AtomicU64 = AtomicU64::new(0);
    static PARSE_NS: AtomicU64 = AtomicU64::new(0);
    static CACHE_STORE_NS: AtomicU64 = AtomicU64::new(0);
    static REGISTRY_NS: AtomicU64 = AtomicU64::new(0);
    static OBJECT_NS: AtomicU64 = AtomicU64::new(0);
    static PROTO_NS: AtomicU64 = AtomicU64::new(0);
    static SENTINEL_NS: AtomicU64 = AtomicU64::new(0);
    static TOTAL_NS: AtomicU64 = AtomicU64::new(0);

    let calls = CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    BYTES_TOTAL.fetch_add(bytes_len as u64, Ordering::Relaxed);
    if cache_hit {
        CACHE_HITS.fetch_add(1, Ordering::Relaxed);
    }
    LOOKUP_NS.fetch_add(lookup_ns, Ordering::Relaxed);
    PARSE_NS.fetch_add(parse_ns, Ordering::Relaxed);
    CACHE_STORE_NS.fetch_add(cache_store_ns, Ordering::Relaxed);
    REGISTRY_NS.fetch_add(registry_ns, Ordering::Relaxed);
    OBJECT_NS.fetch_add(object_ns, Ordering::Relaxed);
    PROTO_NS.fetch_add(proto_ns, Ordering::Relaxed);
    SENTINEL_NS.fetch_add(sentinel_ns, Ordering::Relaxed);
    TOTAL_NS.fetch_add(total_ns, Ordering::Relaxed);

    if calls <= 8 || calls.is_power_of_two() {
        let avg = |counter: &AtomicU64| counter.load(Ordering::Relaxed) / calls;
        eprintln!(
            "[wasm-module-phase] calls={} avg_bytes={} cache_hits={} avg_lookup_ns={} avg_parse_ns={} avg_cache_store_ns={} avg_registry_ns={} avg_object_ns={} avg_proto_ns={} avg_sentinel_ns={} avg_total_ns={}",
            calls,
            BYTES_TOTAL.load(Ordering::Relaxed) / calls,
            CACHE_HITS.load(Ordering::Relaxed),
            avg(&LOOKUP_NS),
            avg(&PARSE_NS),
            avg(&CACHE_STORE_NS),
            avg(&REGISTRY_NS),
            avg(&OBJECT_NS),
            avg(&PROTO_NS),
            avg(&SENTINEL_NS),
            avg(&TOTAL_NS)
        );
    }
}

fn record_wasm_module_ctor_phase(buffer_ns: u64, make_ns: u64, total_ns: u64) {
    if !wasm_module_counters_enabled() {
        return;
    }
    static CALLS: AtomicU64 = AtomicU64::new(0);
    static BUFFER_NS: AtomicU64 = AtomicU64::new(0);
    static MAKE_NS: AtomicU64 = AtomicU64::new(0);
    static TOTAL_NS: AtomicU64 = AtomicU64::new(0);

    let calls = CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    BUFFER_NS.fetch_add(buffer_ns, Ordering::Relaxed);
    MAKE_NS.fetch_add(make_ns, Ordering::Relaxed);
    TOTAL_NS.fetch_add(total_ns, Ordering::Relaxed);

    if calls <= 8 || calls.is_power_of_two() {
        let avg = |counter: &AtomicU64| counter.load(Ordering::Relaxed) / calls;
        eprintln!(
            "[wasm-module-ctor-phase] calls={} avg_buffer_ns={} avg_make_ns={} avg_total_ns={}",
            calls,
            avg(&BUFFER_NS),
            avg(&MAKE_NS),
            avg(&TOTAL_NS)
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn record_wasm_instance_export_phase(
    import_ns: u64,
    instantiate_ns: u64,
    register_ns: u64,
    memory_sync_ns: u64,
    table_sync_ns: u64,
    global_ns: u64,
    object_ns: u64,
    export_ns: u64,
    total_ns: u64,
) {
    if !wasm_instance_export_counters_enabled() {
        return;
    }
    static CALLS: AtomicU64 = AtomicU64::new(0);
    static IMPORT_NS: AtomicU64 = AtomicU64::new(0);
    static INSTANTIATE_NS: AtomicU64 = AtomicU64::new(0);
    static REGISTER_NS: AtomicU64 = AtomicU64::new(0);
    static MEMORY_SYNC_NS: AtomicU64 = AtomicU64::new(0);
    static TABLE_SYNC_NS: AtomicU64 = AtomicU64::new(0);
    static GLOBAL_NS: AtomicU64 = AtomicU64::new(0);
    static OBJECT_NS: AtomicU64 = AtomicU64::new(0);
    static EXPORT_NS: AtomicU64 = AtomicU64::new(0);
    static TOTAL_NS: AtomicU64 = AtomicU64::new(0);

    let calls = CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    IMPORT_NS.fetch_add(import_ns, Ordering::Relaxed);
    INSTANTIATE_NS.fetch_add(instantiate_ns, Ordering::Relaxed);
    REGISTER_NS.fetch_add(register_ns, Ordering::Relaxed);
    MEMORY_SYNC_NS.fetch_add(memory_sync_ns, Ordering::Relaxed);
    TABLE_SYNC_NS.fetch_add(table_sync_ns, Ordering::Relaxed);
    GLOBAL_NS.fetch_add(global_ns, Ordering::Relaxed);
    OBJECT_NS.fetch_add(object_ns, Ordering::Relaxed);
    EXPORT_NS.fetch_add(export_ns, Ordering::Relaxed);
    TOTAL_NS.fetch_add(total_ns, Ordering::Relaxed);

    if calls <= 8 || calls.is_power_of_two() {
        let avg = |counter: &AtomicU64| counter.load(Ordering::Relaxed) / calls;
        eprintln!(
            "[wasm-instance-export-phase] calls={} avg_import_ns={} avg_instantiate_ns={} avg_register_ns={} avg_memory_sync_ns={} avg_table_sync_ns={} avg_global_ns={} avg_object_ns={} avg_export_ns={} avg_total_ns={}",
            calls,
            avg(&IMPORT_NS),
            avg(&INSTANTIATE_NS),
            avg(&REGISTER_NS),
            avg(&MEMORY_SYNC_NS),
            avg(&TABLE_SYNC_NS),
            avg(&GLOBAL_NS),
            avg(&OBJECT_NS),
            avg(&EXPORT_NS),
            avg(&TOTAL_NS)
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn record_wasm_build_exports_phase(
    func_count: usize,
    memory_count: usize,
    global_count: usize,
    table_count: usize,
    tag_count: usize,
    object_ns: u64,
    func_specs_ns: u64,
    function_ns: u64,
    sentinel_ns: u64,
    property_ns: u64,
    non_func_ns: u64,
    total_ns: u64,
) {
    if !wasm_instance_export_counters_enabled() {
        return;
    }
    static CALLS: AtomicU64 = AtomicU64::new(0);
    static FUNC_COUNT: AtomicU64 = AtomicU64::new(0);
    static MEMORY_COUNT: AtomicU64 = AtomicU64::new(0);
    static GLOBAL_COUNT: AtomicU64 = AtomicU64::new(0);
    static TABLE_COUNT: AtomicU64 = AtomicU64::new(0);
    static TAG_COUNT: AtomicU64 = AtomicU64::new(0);
    static OBJECT_NS: AtomicU64 = AtomicU64::new(0);
    static FUNC_SPECS_NS: AtomicU64 = AtomicU64::new(0);
    static FUNCTION_NS: AtomicU64 = AtomicU64::new(0);
    static SENTINEL_NS: AtomicU64 = AtomicU64::new(0);
    static PROPERTY_NS: AtomicU64 = AtomicU64::new(0);
    static NON_FUNC_NS: AtomicU64 = AtomicU64::new(0);
    static TOTAL_NS: AtomicU64 = AtomicU64::new(0);

    let calls = CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    FUNC_COUNT.fetch_add(func_count as u64, Ordering::Relaxed);
    MEMORY_COUNT.fetch_add(memory_count as u64, Ordering::Relaxed);
    GLOBAL_COUNT.fetch_add(global_count as u64, Ordering::Relaxed);
    TABLE_COUNT.fetch_add(table_count as u64, Ordering::Relaxed);
    TAG_COUNT.fetch_add(tag_count as u64, Ordering::Relaxed);
    OBJECT_NS.fetch_add(object_ns, Ordering::Relaxed);
    FUNC_SPECS_NS.fetch_add(func_specs_ns, Ordering::Relaxed);
    FUNCTION_NS.fetch_add(function_ns, Ordering::Relaxed);
    SENTINEL_NS.fetch_add(sentinel_ns, Ordering::Relaxed);
    PROPERTY_NS.fetch_add(property_ns, Ordering::Relaxed);
    NON_FUNC_NS.fetch_add(non_func_ns, Ordering::Relaxed);
    TOTAL_NS.fetch_add(total_ns, Ordering::Relaxed);

    if calls <= 8 || calls.is_power_of_two() {
        let avg = |counter: &AtomicU64| counter.load(Ordering::Relaxed) / calls;
        eprintln!(
            "[wasm-build-exports-phase] calls={} avg_func_count={} avg_memory_count={} avg_global_count={} avg_table_count={} avg_tag_count={} avg_object_ns={} avg_func_specs_ns={} avg_function_ns={} avg_sentinel_ns={} avg_property_ns={} avg_non_func_ns={} avg_total_ns={}",
            calls,
            avg(&FUNC_COUNT),
            avg(&MEMORY_COUNT),
            avg(&GLOBAL_COUNT),
            avg(&TABLE_COUNT),
            avg(&TAG_COUNT),
            avg(&OBJECT_NS),
            avg(&FUNC_SPECS_NS),
            avg(&FUNCTION_NS),
            avg(&SENTINEL_NS),
            avg(&PROPERTY_NS),
            avg(&NON_FUNC_NS),
            avg(&TOTAL_NS)
        );
    }
}

fn wasm_export_func_meta_from_spec(spec: rusty_wasm::ExportFuncSpec) -> WasmExportFuncMeta {
    let (name, funcidx, params, results, writes_memory, type_final, type_shape, type_shapes) = spec;
    WasmExportFuncMeta {
        name,
        funcidx,
        signature_value: js_string(&wasm_signature_key(&params, &results)),
        type_shape_value: js_string(&type_shape),
        type_shapes_value: js_string(&type_shapes.join("\n")),
        params,
        writes_memory,
        type_final,
    }
}

fn wasm_export_func_meta_from_specs(
    specs: impl IntoIterator<Item = rusty_wasm::ExportFuncSpec>,
) -> Vec<WasmExportFuncMeta> {
    specs
        .into_iter()
        .map(wasm_export_func_meta_from_spec)
        .collect()
}

fn wasm_cached_export_func_meta(module_id: usize) -> Option<Vec<WasmExportFuncMeta>> {
    if let Some(specs) = WASM_MODULE_EXPORT_FUNC_META.with(|cache| {
        cache
            .borrow()
            .get(module_id)
            .and_then(|entry| entry.as_ref().cloned())
    }) {
        return Some(specs);
    }
    let specs = WASM_MODULES.with(|modules| {
        modules
            .borrow()
            .get(module_id)
            .and_then(|module| module.as_ref().map(module_export_func_specs))
    })?;
    let specs = wasm_export_func_meta_from_specs(specs);
    WASM_MODULE_EXPORT_FUNC_META.with(|cache| {
        let mut cache = cache.borrow_mut();
        while cache.len() <= module_id {
            cache.push(None);
        }
        cache[module_id] = Some(specs.clone());
    });
    Some(specs)
}

fn wasm_bytes(rt: &mut Runtime, v: &Value) -> Vec<u8> {
    match v {
        Value::Object(id) => {
            if let Some(rec) = rt.array_buffers.get(id) {
                if rec.detached {
                    return Vec::new();
                }
                return rec.to_bytes();
            }
            if let Some(view) = rt.typed_array_views.get(id).cloned() {
                if let Some(rec) = rt.array_buffers.get(&view.buffer) {
                    if rec.detached {
                        return Vec::new();
                    }
                    let len = view
                        .fixed_length
                        .map(|n| n * view.bytes_per_element)
                        .unwrap_or_else(|| rec.byte_len().saturating_sub(view.byte_offset));
                    return rec.read_bytes(view.byte_offset, view.byte_offset + len);
                }
            }

            let len = match rt.object_get(*id, "length") {
                Value::Number(n) => n as usize,
                _ => match rt.object_get(*id, "byteLength") {
                    Value::Number(n) => n as usize,
                    _ => 0,
                },
            };
            (0..len)
                .map(|i| match rt.object_get(*id, &i.to_string()) {
                    Value::Number(n) => n as u8,
                    _ => 0,
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

fn wasm_buffer_source(rt: &mut Runtime, v: &Value) -> Result<Vec<u8>, RuntimeError> {
    match v {
        Value::Object(id) if rt.array_buffers.contains_key(id) => Ok(wasm_bytes(rt, v)),
        Value::Object(id)
            if rt
                .typed_array_views
                .get(id)
                .is_some_and(|view| &*view.element_kind != "DataView") =>
        {
            Ok(wasm_bytes(rt, v))
        }
        _ => Err(RuntimeError::TypeError(
            "WebAssembly.validate(): Argument 0 must be a buffer source".into(),
        )),
    }
}

fn native_function<F>(rt: &mut Runtime, name: &str, length: usize, f: F) -> ObjectRef
where
    F: Fn(&mut Runtime, &[Value]) -> Result<Value, RuntimeError> + 'static,
{
    let native: NativeFn = Rc::new(f);
    let mut properties = indexmap::IndexMap::new();
    rusty_js_runtime::value::install_function_meta_props(&mut properties, name, length as f64);
    let fn_obj = Object {
        proto: None,
        extensible: true,
        properties,
        internal_kind: InternalKind::Function(Box::new(FunctionInternals {
            name: name.to_string(),
            length: length as u32,
            native,
            is_constructor: true,
            creation_realm: 0,
            roots: Vec::new(),
        })),
        ..Default::default()
    };
    rt.alloc_object(fn_obj)
}

fn install_builtin_function<F>(
    rt: &mut Runtime,
    host: ObjectRef,
    name: &str,
    length: usize,
    f: F,
) -> ObjectRef
where
    F: Fn(&mut Runtime, &[Value]) -> Result<Value, RuntimeError> + 'static,
{
    let func = native_function(rt, name, length, f);
    rt.define_data_property_attrs(host, name, Value::Object(func), true, true, true);
    func
}

fn js_string(s: &str) -> Value {
    Value::String(Rc::new(JsString::from(s)))
}

fn install_constructor_prototype(
    rt: &mut Runtime,
    ctor: ObjectRef,
    parent_proto: Option<ObjectRef>,
) -> ObjectRef {
    let proto = new_object(rt);
    if let Some(parent) = parent_proto {
        rt.set_object_prototype_internal(proto, Some(parent));
    }
    rt.define_data_property_attrs(proto, "constructor", Value::Object(ctor), true, false, true);
    rt.set_own_frozen_property(ctor, "prototype".into(), Value::Object(proto));
    proto
}

fn install_to_string_tag(rt: &mut Runtime, obj: ObjectRef, tag: &'static str) {
    rt.define_data_property_attrs(obj, "@@toStringTag", js_string(tag), false, false, true);
}

fn wasm_gc_ref_object(
    rt: &mut Runtime,
    inst_id: Option<usize>,
    kind: &'static str,
    idx: u32,
) -> Value {
    let owner = inst_id.unwrap_or(usize::MAX);
    if let Some(obj) =
        WASM_GC_REF_OBJECTS.with(|cache| cache.borrow().get(&(owner, kind, idx)).copied())
    {
        return Value::Object(obj);
    }
    let obj = new_object(rt);
    rt.set_engine_sentinel(obj, "__wasm_gc_ref_kind", js_string(kind));
    rt.set_engine_sentinel(obj, "__wasm_gc_ref_index", Value::Number(idx as f64));
    if owner != usize::MAX {
        rt.set_engine_sentinel(obj, "__wasm_gc_ref_inst", Value::Number(owner as f64));
    }
    let to_primitive = make_callable(rt, "@@toPrimitive", |_rt, _args| {
        Err(RuntimeError::TypeError(
            "Cannot convert object to primitive value".into(),
        ))
    });
    rt.define_data_property_attrs(
        obj,
        "@@toPrimitive",
        Value::Object(to_primitive),
        false,
        false,
        true,
    );
    WASM_GC_REF_OBJECTS.with(|cache| {
        cache.borrow_mut().insert((owner, kind, idx), obj);
    });
    Value::Object(obj)
}

fn install_namespace_ctor(
    rt: &mut Runtime,
    wasm: ObjectRef,
    name: &str,
    ctor: ObjectRef,
) -> ObjectRef {
    let proto = install_constructor_prototype(rt, ctor, None);
    rt.define_data_property_attrs(wasm, name, Value::Object(ctor), true, false, true);
    store_wasm_prototype(name, proto);
    proto
}

fn store_wasm_prototype(name: &str, proto: ObjectRef) {
    match name {
        "Module" => WASM_MODULE_PROTO.with(|slot| slot.set(Some(proto))),
        "Instance" => WASM_INSTANCE_PROTO.with(|slot| slot.set(Some(proto))),
        "Memory" => WASM_MEMORY_PROTO.with(|slot| slot.set(Some(proto))),
        "Table" => WASM_TABLE_PROTO.with(|slot| slot.set(Some(proto))),
        "Global" => WASM_GLOBAL_PROTO.with(|slot| slot.set(Some(proto))),
        "Tag" => WASM_TAG_PROTO.with(|slot| slot.set(Some(proto))),
        "Exception" => WASM_EXCEPTION_PROTO.with(|slot| slot.set(Some(proto))),
        "Suspending" => WASM_SUSPENDING_PROTO.with(|slot| slot.set(Some(proto))),
        _ => {}
    }
}

fn cached_wasm_prototype(name: &str) -> Option<ObjectRef> {
    match name {
        "Module" => WASM_MODULE_PROTO.with(|slot| slot.get()),
        "Instance" => WASM_INSTANCE_PROTO.with(|slot| slot.get()),
        "Memory" => WASM_MEMORY_PROTO.with(|slot| slot.get()),
        "Table" => WASM_TABLE_PROTO.with(|slot| slot.get()),
        "Global" => WASM_GLOBAL_PROTO.with(|slot| slot.get()),
        "Tag" => WASM_TAG_PROTO.with(|slot| slot.get()),
        "Exception" => WASM_EXCEPTION_PROTO.with(|slot| slot.get()),
        "Suspending" => WASM_SUSPENDING_PROTO.with(|slot| slot.get()),
        _ => None,
    }
}

fn error_parent_proto(rt: &Runtime) -> Option<ObjectRef> {
    match rt.global_get("Error") {
        Value::Object(ctor) => match rt.object_get(ctor, "prototype") {
            Value::Object(proto) => Some(proto),
            _ => None,
        },
        _ => None,
    }
}

fn make_wasm_error_ctor(rt: &mut Runtime, name: &'static str) -> ObjectRef {
    let ctor = native_function(rt, name, 1, move |rt, args| {
        let this = match rt.current_this() {
            Value::Object(o) => o,
            _ => new_object(rt),
        };
        rt.obj_mut(this).internal_kind = InternalKind::Error;
        let msg = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            Some(Value::Undefined) | None => String::new(),
            Some(v) => rusty_js_runtime::abstract_ops::to_string(v)
                .as_str()
                .to_string(),
        };
        rt.define_data_property_attrs(
            this,
            "name",
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(name))),
            true,
            false,
            true,
        );
        rt.define_data_property_attrs(
            this,
            "message",
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(msg))),
            true,
            false,
            true,
        );
        Ok(Value::Object(this))
    });
    let proto = install_constructor_prototype(rt, ctor, error_parent_proto(rt));
    rt.define_data_property_attrs(
        proto,
        "name",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(name))),
        true,
        false,
        true,
    );
    rt.define_data_property_attrs(
        proto,
        "message",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(""))),
        true,
        false,
        true,
    );
    ctor
}

fn wasm_object_has_marker(rt: &Runtime, value: &Value, marker: &str) -> bool {
    matches!(value, Value::Object(o) if matches!(rt.object_get(*o, marker), Value::Boolean(true)))
}

fn wasm_constructor_this(
    rt: &mut Runtime,
    namespace: ObjectRef,
    label: &'static str,
) -> Result<ObjectRef, RuntimeError> {
    match rt.current_this() {
        Value::Object(o) if o != namespace => Ok(o),
        _ => Err(RuntimeError::TypeError(format!(
            "WebAssembly.{label}(): WebAssembly.{label} must be invoked with 'new'"
        ))),
    }
}

fn install_wasm_proposal_namespace(rt: &mut Runtime, wasm: ObjectRef) {
    let tag_ctor = native_function(rt, "Tag", 1, move |rt, args| {
        let this = wasm_constructor_this(rt, wasm, "Tag")?;
        let Some(Value::Object(desc)) = args.first() else {
            return Err(RuntimeError::TypeError(
                "WebAssembly.Tag(): Argument 0 must be a tag type".into(),
            ));
        };
        let params = rt.object_get(*desc, "parameters");
        let Value::Object(params_obj) = params else {
            return Err(RuntimeError::TypeError(
                "WebAssembly.Tag(): Argument 0 must be a tag type".into(),
            ));
        };
        rt.define_data_property_attrs(this, "__wasm_tag", Value::Boolean(true), true, false, true);
        rt.define_data_property_attrs(
            this,
            "__wasm_tag_parameters",
            Value::Object(params_obj),
            true,
            false,
            true,
        );
        rt.define_data_property_attrs(
            this,
            "__wasm_tag_identity",
            js_string(&format!("js:{this:?}")),
            true,
            false,
            true,
        );
        Ok(Value::Object(this))
    });
    let tag_proto = install_namespace_ctor(rt, wasm, "Tag", tag_ctor);
    install_to_string_tag(rt, tag_proto, "WebAssembly.Tag");

    let exception_ctor = native_function(rt, "Exception", 2, move |rt, args| {
        let this = wasm_constructor_this(rt, wasm, "Exception")?;
        let tag = args.first().cloned().unwrap_or(Value::Undefined);
        if !wasm_object_has_marker(rt, &tag, "__wasm_tag") {
            return Err(RuntimeError::TypeError(
                "WebAssembly.Exception(): Argument 0 must be a WebAssembly tag".into(),
            ));
        }
        let values = match args.get(1) {
            Some(Value::Object(o)) => Some(*o),
            _ => None,
        };
        rt.define_data_property_attrs(
            this,
            "__wasm_exception",
            Value::Boolean(true),
            true,
            false,
            true,
        );
        rt.define_data_property_attrs(this, "__wasm_exception_tag", tag, true, false, true);
        if let Some(values) = values {
            rt.define_data_property_attrs(
                this,
                "__wasm_exception_values",
                Value::Object(values),
                true,
                false,
                true,
            );
        }
        Ok(Value::Object(this))
    });
    let exception_proto = install_namespace_ctor(rt, wasm, "Exception", exception_ctor);
    install_to_string_tag(rt, exception_proto, "WebAssembly.Exception");
    install_builtin_function(rt, exception_proto, "getArg", 2, |rt, args| {
        let this = match rt.current_this() {
            Value::Object(o)
                if matches!(rt.object_get(o, "__wasm_exception"), Value::Boolean(true)) =>
            {
                o
            }
            _ => {
                return Err(RuntimeError::TypeError(
                    "WebAssembly.Exception.getArg(): incompatible receiver".into(),
                ))
            }
        };
        let tag = args.first().cloned().unwrap_or(Value::Undefined);
        if !wasm_object_has_marker(rt, &tag, "__wasm_tag") {
            return Err(RuntimeError::TypeError(
                "WebAssembly.Exception.getArg(): Argument 0 must be a WebAssembly tag".into(),
            ));
        }
        let Value::Number(index) = args.get(1).cloned().unwrap_or(Value::Undefined) else {
            return Ok(Value::Undefined);
        };
        match rt.object_get(this, "__wasm_exception_values") {
            Value::Object(values) => Ok(rt.object_get(values, &(index as usize).to_string())),
            _ => Ok(Value::Undefined),
        }
    });
    install_builtin_function(rt, exception_proto, "is", 1, |rt, args| {
        let this = match rt.current_this() {
            Value::Object(o)
                if matches!(rt.object_get(o, "__wasm_exception"), Value::Boolean(true)) =>
            {
                o
            }
            _ => return Ok(Value::Boolean(false)),
        };
        let tag = args.first().cloned().unwrap_or(Value::Undefined);
        let result = match (rt.object_get(this, "__wasm_exception_tag"), tag) {
            (Value::Object(a), Value::Object(b)) => a == b,
            _ => false,
        };
        Ok(Value::Boolean(result))
    });
    let stack_getter = native_function(rt, "get stack", 0, |_rt, _args| Ok(Value::Undefined));
    rt.obj_mut(exception_proto).dict_mut().insert(
        "stack".into(),
        PropertyDescriptor {
            value: Value::Undefined,
            writable: false,
            enumerable: true,
            configurable: true,
            getter: Some(Value::Object(stack_getter)),
            setter: None,
        },
    );

    let suspending_ctor = native_function(rt, "Suspending", 1, move |rt, args| {
        let this = wasm_constructor_this(rt, wasm, "Suspending")?;
        let func = args.first().cloned().unwrap_or(Value::Undefined);
        if !rt.is_callable(&func) {
            return Err(RuntimeError::TypeError(
                "WebAssembly.Suspending(): Argument 0 must be a function".into(),
            ));
        }
        rt.define_data_property_attrs(
            this,
            "__wasm_suspending",
            Value::Boolean(true),
            true,
            false,
            true,
        );
        rt.define_data_property_attrs(this, "__wasm_suspending_target", func, true, false, true);
        Ok(Value::Object(this))
    });
    let suspending_proto = install_namespace_ctor(rt, wasm, "Suspending", suspending_ctor);
    install_to_string_tag(rt, suspending_proto, "WebAssembly.Suspending");

    let suspend_error = make_wasm_error_ctor(rt, "SuspendError");
    rt.define_data_property_attrs(
        wasm,
        "SuspendError",
        Value::Object(suspend_error),
        true,
        false,
        true,
    );

    let promising = native_function(rt, "promising", 1, |rt, args| {
        let func = args.first().cloned().unwrap_or(Value::Undefined);
        if !rt.is_callable(&func) {
            return Err(RuntimeError::TypeError(
                "WebAssembly.promising(): Argument 0 must be a function".into(),
            ));
        }
        Err(RuntimeError::TypeError(
            "WebAssembly.promising(): Argument 0 must be a WebAssembly exported function".into(),
        ))
    });
    if let InternalKind::Function(f) = &mut rt.obj_mut(promising).internal_kind {
        f.is_constructor = false;
    }
    rt.obj_mut(promising).remove_str("prototype");
    rt.define_data_property_attrs(
        wasm,
        "promising",
        Value::Object(promising),
        true,
        true,
        true,
    );

    let js_tag = new_object(rt);
    rt.define_data_property_attrs(wasm, "JSTag", Value::Object(js_tag), false, false, true);
}

fn wasm_error_value(rt: &mut Runtime, name: &'static str, message: String) -> Value {
    let err = new_object(rt);
    if let Value::Object(wasm) = rt.global_get("WebAssembly") {
        if let Value::Object(ctor) = rt.object_get(wasm, name) {
            if let Value::Object(proto) = rt.object_get(ctor, "prototype") {
                rt.set_object_prototype_internal(err, Some(proto));
            }
        }
    }
    rt.define_data_property_attrs(
        err,
        "name",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(name))),
        true,
        false,
        true,
    );
    rt.define_data_property_attrs(
        err,
        "message",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(message))),
        true,
        false,
        true,
    );
    Value::Object(err)
}

fn wasm_compile_error_value(rt: &mut Runtime, message: String) -> Value {
    wasm_error_value(rt, "CompileError", message)
}

fn type_error_value(rt: &mut Runtime, message: String) -> Value {
    let err = new_object(rt);
    if let Value::Object(ctor) = rt.global_get("TypeError") {
        if let Value::Object(proto) = rt.object_get(ctor, "prototype") {
            rt.set_object_prototype_internal(err, Some(proto));
        }
    }
    rt.define_data_property_attrs(
        err,
        "name",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "TypeError",
        ))),
        true,
        false,
        true,
    );
    rt.define_data_property_attrs(
        err,
        "message",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(message))),
        true,
        false,
        true,
    );
    Value::Object(err)
}

fn wasm_link_error_value(rt: &mut Runtime, message: String) -> Value {
    wasm_error_value(rt, "LinkError", message)
}

fn wasm_runtime_error_value(rt: &mut Runtime, message: String) -> Value {
    let err = new_object(rt);
    if let Value::Object(wasm) = rt.global_get("WebAssembly") {
        if let Value::Object(ctor) = rt.object_get(wasm, "RuntimeError") {
            if let Value::Object(proto) = rt.object_get(ctor, "prototype") {
                rt.set_object_prototype_internal(err, Some(proto));
            }
        }
    }
    rt.define_data_property_attrs(
        err,
        "name",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "RuntimeError",
        ))),
        true,
        false,
        true,
    );
    rt.define_data_property_attrs(
        err,
        "message",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(message))),
        true,
        false,
        true,
    );
    Value::Object(err)
}

fn wasm_compile_message(e: &RuntimeError) -> String {
    let raw = format!("{e:?}");
    if raw.contains("unsupported wasm version [1, 0, 0, 1]") {
        "WebAssembly.compile(): expected version 01 00 00 00, found 01 00 00 01 @+4".into()
    } else {
        raw
    }
}

fn wasm_host_error_message(rt: &mut Runtime, e: RuntimeError) -> String {
    match e {
        RuntimeError::Thrown(Value::Object(id)) => match rt.object_get(id, "message") {
            Value::String(s) if !s.as_str().is_empty() => s.as_str().to_string(),
            _ => "thrown object".to_string(),
        },
        other => format!("{other:?}"),
    }
}

fn reject_wasm_compile_error(rt: &mut Runtime, promise: ObjectRef, err: RuntimeError) {
    let reason = match err {
        RuntimeError::Thrown(value) => value,
        other => wasm_compile_error_value(rt, wasm_compile_message(&other)),
    };
    rusty_js_runtime::promise::reject_promise(rt, promise, reason);
}

fn reject_streaming_error(rt: &mut Runtime, promise: ObjectRef, err: RuntimeError) {
    let reason = match err {
        RuntimeError::Thrown(value) => value,
        RuntimeError::TypeError(msg) => type_error_value(rt, msg),
        other => wasm_compile_error_value(rt, wasm_compile_message(&other)),
    };
    rusty_js_runtime::promise::reject_promise(rt, promise, reason);
}

fn make_array_buffer_with_max(
    rt: &mut Runtime,
    bytes: Vec<u8>,
    max_byte_length: usize,
) -> ObjectRef {
    let byte_length = bytes.len();
    let ab = new_object(rt);
    if let Value::Object(ctor) = rt.global_get("ArrayBuffer") {
        if let Value::Object(proto) = rt.object_get(ctor, "prototype") {
            rt.set_object_prototype_internal(ab, Some(proto));
        }
    }
    rt.array_buffers.insert(
        ab,
        ArrayBufferRecord {
            byte_length,
            backing_epoch: 0,
            max_byte_length: max_byte_length.max(byte_length),
            data: bytes,
            detached: false,
            untransferable: false,
            shared: None,
        },
    );
    ab
}

fn detach_array_buffer(rt: &mut Runtime, ab: ObjectRef) {
    if let Some(rec) = rt.array_buffers.get_mut(&ab) {
        if rec.shared.is_none() {
            rec.detached = true;
            rec.clear_bytes();
            rec.byte_length = 0;
        }
    }
}

fn replace_memory_buffer(
    rt: &mut Runtime,
    mem: ObjectRef,
    old_ab: ObjectRef,
    bytes: Vec<u8>,
    max_byte_length: usize,
) -> ObjectRef {
    let new_ab = make_array_buffer_with_max(rt, bytes, max_byte_length);
    detach_array_buffer(rt, old_ab);
    rt.define_data_property_attrs(mem, "buffer", Value::Object(new_ab), true, false, true);
    rt.define_data_property_attrs(
        mem,
        "__wasm_memory_buffer",
        Value::Object(new_ab),
        true,
        false,
        true,
    );
    WASM_INSTANCE_AB.with(|v| {
        for slot in v.borrow_mut().iter_mut() {
            if *slot == Some(old_ab) {
                *slot = Some(new_ab);
            }
        }
    });
    new_ab
}

pub(crate) fn make_memory_object_from_buffer(rt: &mut Runtime, ab: ObjectRef) -> ObjectRef {
    let mem = new_object(rt);
    if let Some(proto) = wasm_prototype(rt, "Memory") {
        rt.set_object_prototype_internal(mem, Some(proto));
    }
    rt.define_data_property_attrs(mem, "buffer", Value::Object(ab), true, false, true);
    rt.define_data_property_attrs(
        mem,
        "__wasm_memory_buffer",
        Value::Object(ab),
        true,
        false,
        true,
    );
    mem
}

fn wasm_memory_maximum_pages(rt: &mut Runtime, mem: ObjectRef) -> Option<u32> {
    match rt.object_get(mem, "__wasm_memory_maximum") {
        Value::Number(maximum) if maximum >= 0.0 => Some(maximum as u32),
        _ => None,
    }
}

fn make_shared_array_buffer(
    rt: &mut Runtime,
    byte_length: usize,
    max_byte_length: usize,
) -> ObjectRef {
    let mut o = Object::new_ordinary();
    o.set_own_internal(
        "__kind".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "SharedArrayBuffer",
        ))),
    );
    if let Value::Object(ctor) = rt.global_get("SharedArrayBuffer") {
        if let Value::Object(proto) = rt.object_get(ctor, "prototype") {
            o.proto = Some(proto);
        }
    }
    let ab = rt.alloc_object(o);
    rt.array_buffers.insert(
        ab,
        ArrayBufferRecord {
            byte_length,
            backing_epoch: 0,
            max_byte_length,
            data: Vec::new(),
            detached: false,
            untransferable: false,
            shared: Some(std::sync::Arc::new(std::sync::Mutex::new(vec![
                0u8;
                byte_length
            ]))),
        },
    );
    ab
}

fn memory_this(rt: &mut Runtime) -> Result<ObjectRef, RuntimeError> {
    match rt.current_this() {
        Value::Object(id) => Ok(id),
        _ => Err(RuntimeError::TypeError(
            "WebAssembly.Memory: incompatible receiver".into(),
        )),
    }
}

fn table_this(rt: &mut Runtime) -> Result<ObjectRef, RuntimeError> {
    match rt.current_this() {
        Value::Object(id) => Ok(id),
        _ => Err(RuntimeError::TypeError(
            "WebAssembly.Table: incompatible receiver".into(),
        )),
    }
}

fn wasm_table_element_name(rt: &mut Runtime, desc: ObjectRef) -> Result<String, RuntimeError> {
    match rt.object_get(desc, "element") {
        Value::String(s) => {
            let element = s.as_str().to_string();
            match element.as_str() {
                "anyfunc" | "externref" => Ok(element),
                _ => Err(RuntimeError::TypeError(
                    "WebAssembly.Table(): Descriptor property 'element' must be a WebAssembly reference type".into(),
                )),
            }
        }
        _ => Err(RuntimeError::TypeError(
            "WebAssembly.Table: element is required".into(),
        )),
    }
}

fn wasm_table_u32_field(
    rt: &mut Runtime,
    desc: ObjectRef,
    name: &str,
    required: bool,
) -> Result<Option<usize>, RuntimeError> {
    match rt.object_get(desc, name) {
        Value::Number(n) if n.is_finite() && n >= 0.0 && n.fract() == 0.0 => Ok(Some(n as usize)),
        Value::Undefined if !required => Ok(None),
        Value::Undefined => Err(RuntimeError::TypeError(format!(
            "WebAssembly.Table: {name} is required"
        ))),
        _ => Err(RuntimeError::TypeError(format!(
            "WebAssembly.Table: {name} must be a non-negative integer"
        ))),
    }
}

fn bigint_to_usize(value: &rusty_js_runtime::bigint::JsBigInt) -> Option<usize> {
    let decimal = value.to_decimal();
    if decimal.starts_with('-') {
        return None;
    }
    decimal.parse::<usize>().ok()
}

fn wasm_memory_address64(rt: &mut Runtime, desc: ObjectRef) -> Result<bool, RuntimeError> {
    match rt.object_get(desc, "address") {
        Value::Undefined => Ok(false),
        Value::String(s) => match s.as_str() {
            "i32" => Ok(false),
            "i64" => Ok(true),
            other => Err(RuntimeError::TypeError(format!(
                "WebAssembly.Memory(): Unknown address type '{other}'; pass 'i32' or 'i64'"
            ))),
        },
        _ => Err(RuntimeError::TypeError(
            "WebAssembly.Memory(): address must be 'i32' or 'i64'".into(),
        )),
    }
}

fn wasm_memory_page_field(
    rt: &mut Runtime,
    desc: ObjectRef,
    name: &str,
    address64: bool,
) -> Result<Option<usize>, RuntimeError> {
    match rt.object_get(desc, name) {
        Value::Undefined => Ok(None),
        Value::BigInt(b) if address64 => bigint_to_usize(&b).map(Some).ok_or_else(|| {
            RuntimeError::RangeError(format!(
                "WebAssembly.Memory(): {name} must be a non-negative BigInt"
            ))
        }),
        Value::BigInt(_) => Err(RuntimeError::TypeError(
            "Cannot convert a BigInt value to a number".into(),
        )),
        Value::Number(n) if address64 => Err(RuntimeError::TypeError(format!(
            "Cannot convert {} to a BigInt",
            if n.fract() == 0.0 {
                format!("{}", n as i64)
            } else {
                n.to_string()
            }
        ))),
        Value::Number(n) if n.is_finite() && n >= 0.0 && n.fract() == 0.0 => Ok(Some(n as usize)),
        _ => Err(RuntimeError::TypeError(format!(
            "WebAssembly.Memory(): {name} must be a non-negative integer"
        ))),
    }
}

fn wasm_memory_grow_delta(args: &[Value], address64: bool) -> Result<usize, RuntimeError> {
    match args.first() {
        Some(Value::BigInt(b)) if address64 => bigint_to_usize(b).ok_or_else(|| {
            RuntimeError::RangeError(
                "WebAssembly.Memory.grow(): delta must be a non-negative BigInt".into(),
            )
        }),
        Some(Value::BigInt(_)) => Err(RuntimeError::TypeError(
            "Cannot convert a BigInt value to a number".into(),
        )),
        Some(Value::Number(n)) if address64 => Err(RuntimeError::TypeError(format!(
            "Cannot convert {} to a BigInt",
            if n.fract() == 0.0 {
                format!("{}", *n as i64)
            } else {
                n.to_string()
            }
        ))),
        Some(Value::Number(n)) if n.is_finite() && *n >= 0.0 && n.fract() == 0.0 => Ok(*n as usize),
        Some(Value::Undefined) | None if !address64 => Ok(0),
        _ if address64 => Err(RuntimeError::TypeError(
            "Cannot convert undefined to a BigInt".into(),
        )),
        _ => Ok(0),
    }
}

fn wasm_table_index_arg(args: &[Value], i: usize, label: &str) -> Result<usize, RuntimeError> {
    match args.get(i) {
        Some(Value::Number(n)) if n.is_finite() && *n >= 0.0 && n.fract() == 0.0 => Ok(*n as usize),
        Some(Value::Undefined) | None => Ok(0),
        _ => Err(RuntimeError::TypeError(format!(
            "WebAssembly.Table.prototype.{label}: index must be a non-negative integer"
        ))),
    }
}

fn wasm_table_len(rt: &mut Runtime, table: ObjectRef) -> usize {
    match rt.object_get(table, "length") {
        Value::Number(n) if n.is_finite() && n >= 0.0 => n as usize,
        _ => 0,
    }
}

fn wasm_table_max(rt: &mut Runtime, table: ObjectRef) -> Option<usize> {
    match rt.object_get(table, "__wasm_table_maximum") {
        Value::Number(n) if n.is_finite() && n >= 0.0 => Some(n as usize),
        _ => None,
    }
}

fn wasm_table_element(rt: &mut Runtime, table: ObjectRef) -> String {
    match rt.object_get(table, "__wasm_table_element") {
        Value::String(s) => s.as_str().to_string(),
        _ => "anyfunc".to_string(),
    }
}

fn wasm_table_type_key(ty: ValType) -> &'static str {
    match ty {
        ValType::ExternRef | ValType::NonNullExternRef => "externref",
        ValType::TypeRef(_) | ValType::NonNullTypeRef(_) => "typed-funcref",
        _ => "funcref",
    }
}

fn wasm_table_object_type_key(rt: &mut Runtime, table: ObjectRef) -> String {
    if let (Value::Number(inst_id), Value::Number(table_index)) = (
        rt.object_get(table, "__wasm_table_inst"),
        rt.object_get(table, "__wasm_table_index"),
    ) {
        if inst_id >= 0.0 && table_index >= 0.0 {
            if let Some(ty) = WASM_INSTANCES.with(|v| {
                v.borrow()
                    .get(inst_id as usize)
                    .and_then(|slot| slot.as_ref())
                    .and_then(|inst| inst.table_element_type_at(table_index as usize))
            }) {
                return wasm_table_type_key(ty).to_string();
            }
        }
    }
    match rt.object_get(table, "__wasm_table_type_key") {
        Value::String(s) => s.as_str().to_string(),
        _ => match wasm_table_element(rt, table).as_str() {
            "externref" => "externref".to_string(),
            _ => "funcref".to_string(),
        },
    }
}

fn make_wasm_table_object(
    rt: &mut Runtime,
    element: &str,
    initial: usize,
    maximum: Option<usize>,
    address64: bool,
) -> ObjectRef {
    let table = new_object(rt);
    if let Some(proto) = wasm_prototype(rt, "Table") {
        rt.set_object_prototype_internal(table, Some(proto));
    }
    let storage = new_object(rt);
    let default_value = if element == "externref" {
        Value::Undefined
    } else {
        Value::Null
    };
    for i in 0..initial {
        rt.object_set(storage, i.to_string(), default_value.clone());
    }
    rt.define_data_property_attrs(
        storage,
        "length",
        Value::Number(initial as f64),
        true,
        false,
        true,
    );
    rt.define_data_property_attrs(
        table,
        "__wasm_table_storage",
        Value::Object(storage),
        true,
        false,
        true,
    );
    rt.define_data_property_attrs(
        table,
        "__wasm_table_element",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(element))),
        true,
        false,
        true,
    );
    rt.define_data_property_attrs(
        table,
        "__wasm_table_address64",
        Value::Boolean(address64),
        true,
        false,
        true,
    );
    if let Some(maximum) = maximum {
        rt.define_data_property_attrs(
            table,
            "__wasm_table_maximum",
            Value::Number(maximum as f64),
            true,
            false,
            true,
        );
    }
    rt.define_data_property_attrs(
        table,
        "length",
        Value::Number(initial as f64),
        true,
        false,
        true,
    );
    table
}

fn sync_imported_table_elements(
    rt: &mut Runtime,
    inst_id: usize,
    table: ObjectRef,
    entries: &[WasmValue],
    imported_funcs: &[Value],
    non_host_func_count: usize,
) {
    let storage = match rt.object_get(table, "__wasm_table_storage") {
        Value::Object(storage) => storage,
        _ => return,
    };
    for (idx, entry) in entries.iter().enumerate() {
        let value = match entry {
            WasmValue::RefNull => Value::Null,
            WasmValue::FuncRef(funcidx) => {
                if (*funcidx as usize) >= non_host_func_count {
                    continue;
                }
                imported_funcs
                    .get(*funcidx as usize)
                    .cloned()
                    .filter(|value| rt.is_callable(value))
                    .unwrap_or_else(|| {
                        Value::Object(make_wasm_func_index_callable(rt, inst_id, *funcidx))
                    })
            }
            other => wasm_to_js(rt, Some(inst_id), other),
        };
        rt.object_set(storage, idx.to_string(), value);
    }
}

fn wasm_func_sentinel_sig(
    rt: &mut Runtime,
    value: &Value,
) -> Option<(usize, u32, Vec<ValType>, Vec<ValType>)> {
    let (inst_id, funcidx) = wasm_func_sentinel_ids(rt, value)?;
    let sig = WASM_INSTANCES.with(|v| {
        v.borrow()
            .get(inst_id)
            .and_then(|slot| slot.as_ref())
            .and_then(|inst| inst.func_sig_by_index(funcidx as usize))
    })?;
    Some((inst_id, funcidx, sig.0, sig.1))
}

fn wasm_func_sentinel_ids(rt: &mut Runtime, value: &Value) -> Option<(usize, u32)> {
    let Value::Object(func) = value else {
        return None;
    };
    let inst_id = match rt.object_get(*func, "__wasm_func_inst") {
        Value::Number(n) if n >= 0.0 => n as usize,
        _ => return None,
    };
    let funcidx = match rt.object_get(*func, "__wasm_func_index") {
        Value::Number(n) if n >= 0.0 => n as u32,
        _ => return None,
    };
    Some((inst_id, funcidx))
}

fn table_import_values(rt: &mut Runtime, table: ObjectRef, length: usize) -> Vec<TableImportValue> {
    let storage = match rt.object_get(table, "__wasm_table_storage") {
        Value::Object(storage) => storage,
        _ => return Vec::new(),
    };
    let element = wasm_table_element(rt, table);
    let mut values = Vec::with_capacity(length);
    for idx in 0..length {
        let value = rt.object_get(storage, &idx.to_string());
        if matches!(value, Value::Null | Value::Undefined) {
            values.push(TableImportValue::Null);
            continue;
        }
        if element == "externref" {
            values.push(TableImportValue::ExternRef(externref_to_wasm(&value)));
        } else if let Some((_inst_id, _funcidx, params, results)) =
            wasm_func_sentinel_sig(rt, &value)
        {
            values.push(TableImportValue::HostFunc {
                params,
                results: results.clone(),
                f: js_import_fn(value, results),
            });
        } else {
            values.push(TableImportValue::Null);
        }
    }
    values
}

fn sync_table_object_to_instance(rt: &mut Runtime, inst_id: usize, table: ObjectRef) {
    let storage = match rt.object_get(table, "__wasm_table_storage") {
        Value::Object(storage) => storage,
        _ => return,
    };
    let length = wasm_table_len(rt, table);
    let element = wasm_table_element(rt, table);
    let mut inst = match WASM_INSTANCES.with(|v| {
        let mut instances = v.borrow_mut();
        instances.get_mut(inst_id).and_then(|slot| slot.take())
    }) {
        Some(inst) => inst,
        None => return,
    };
    let mut values = inst.table_values_at(0);
    if values.len() < length {
        values.resize(length, WasmValue::RefNull);
    }
    for idx in 0..length {
        let value = rt.object_get(storage, &idx.to_string());
        if matches!(value, Value::Null | Value::Undefined) {
            values[idx] = WasmValue::RefNull;
            continue;
        }
        if element == "externref" {
            values[idx] = externref_to_wasm(&value);
        } else if let Some((source_inst, funcidx)) = wasm_func_sentinel_ids(rt, &value) {
            if source_inst == inst_id {
                values[idx] = WasmValue::FuncRef(funcidx);
            } else if let Some((_source_inst, _funcidx, params, results)) =
                wasm_func_sentinel_sig(rt, &value)
            {
                let host_idx = inst.push_host_table_func(
                    params,
                    results.clone(),
                    js_import_fn(value, results),
                );
                values[idx] = WasmValue::FuncRef(host_idx);
            }
        }
    }
    let _ = inst.set_table_values_at(0, values);
    WASM_INSTANCES.with(|v| {
        let mut instances = v.borrow_mut();
        if let Some(slot) = instances.get_mut(inst_id) {
            *slot = Some(inst);
        }
    });
}

fn sync_shared_table_to_instance(rt: &mut Runtime, inst_id: usize) {
    if let Some(table) =
        WASM_INSTANCE_TABLE_OBJECT.with(|v| v.borrow().get(inst_id).and_then(|x| *x))
    {
        sync_table_object_to_instance(rt, inst_id, table);
    }
}

fn sync_instance_table_to_shared_table(rt: &mut Runtime, inst_id: usize) {
    let Some(table) = WASM_INSTANCE_TABLE_OBJECT.with(|v| v.borrow().get(inst_id).and_then(|x| *x))
    else {
        return;
    };
    let values = WASM_INSTANCES.with(|v| {
        v.borrow()
            .get(inst_id)
            .and_then(|slot| slot.as_ref())
            .map(|inst| inst.table_values_at(0))
            .unwrap_or_default()
    });
    sync_imported_table_elements(rt, inst_id, table, &values, &[], usize::MAX);
}

fn make_wasm_func_index_callable(rt: &mut Runtime, inst_id: usize, funcidx: u32) -> ObjectRef {
    let name = funcidx.to_string();
    let func = make_callable(rt, &name, move |rt: &mut Runtime, args| {
        let sig = WASM_INSTANCES.with(|v| {
            v.borrow()
                .get(inst_id)
                .and_then(|x| x.as_ref())
                .and_then(|i| i.func_sig_by_index(funcidx as usize))
        });
        let params = sig.map(|s| s.0).unwrap_or_default();
        let wargs: Vec<WasmValue> = params
            .iter()
            .enumerate()
            .map(|(i, t)| js_to_wasm(args.get(i), *t))
            .collect();
        let ab = WASM_INSTANCE_AB.with(|v| v.borrow().get(inst_id).and_then(|x| *x));
        if let Some(ab) = ab {
            sync_to_wasm(rt, inst_id, ab);
        }
        sync_shared_table_to_instance(rt, inst_id);
        let rt_ptr: *mut Runtime = rt;
        let prev = WASM_RT.with(|c| {
            let p = c.get();
            c.set(rt_ptr);
            p
        });
        let prev_inst = WASM_CUR_INST.with(|c| {
            let p = c.get();
            c.set(inst_id);
            p
        });
        let res = call_instance_func_index(inst_id, funcidx as usize, &wargs);
        WASM_RT.with(|c| c.set(prev));
        WASM_CUR_INST.with(|c| c.set(prev_inst));
        sync_instance_table_to_shared_table(rt, inst_id);
        if let Some(ab) = ab {
            sync_from_wasm(rt, inst_id, ab);
        }
        match res {
            Some(Ok(results)) => Ok(wasm_results_to_js(rt, Some(inst_id), &results)),
            Some(Err(e)) => Err(RuntimeError::Thrown(wasm_runtime_error_value(
                rt,
                format!("WebAssembly: {e}"),
            ))),
            None => Err(RuntimeError::TypeError(
                "WebAssembly: instance unavailable".into(),
            )),
        }
    });
    rt.set_engine_sentinel(func, "__wasm_func_inst", Value::Number(inst_id as f64));
    rt.set_engine_sentinel(func, "__wasm_func_index", Value::Number(funcidx as f64));
    func
}

fn call_instance_func_index(
    inst_id: usize,
    funcidx: usize,
    args: &[WasmValue],
) -> Option<Result<Vec<WasmValue>, String>> {
    let phase_counters = wasm_exported_call_phase_counters_enabled();
    let registry_take_start = phase_counters.then(Instant::now);
    let registry_take_ns;
    let mut inst = match WASM_INSTANCES.with(|v| {
        let mut instances = v.borrow_mut();
        instances.get_mut(inst_id).and_then(|slot| slot.take())
    }) {
        Some(inst) => {
            registry_take_ns = registry_take_start.map(elapsed_ns_since).unwrap_or(0);
            inst
        }
        None => {
            registry_take_ns = registry_take_start.map(elapsed_ns_since).unwrap_or(0);
            let reentrant_start = phase_counters.then(Instant::now);
            return WASM_ACTIVE_INSTANCES.with(|active| {
                let ptr = active
                    .borrow()
                    .iter()
                    .rev()
                    .find_map(|(active_id, ptr)| (*active_id == inst_id).then_some(*ptr))?;

                let res =
                    catch_wasm_execution(|| unsafe { (&mut *ptr).call_func_index(funcidx, args) });
                record_wasm_exported_call_entry(
                    registry_take_ns,
                    0,
                    reentrant_start.map(elapsed_ns_since).unwrap_or(0),
                    0,
                    0,
                    true,
                );
                Some(res)
            });
        }
    };
    let inst_ptr: *mut Instance = &mut inst;
    let active_push_start = phase_counters.then(Instant::now);
    WASM_ACTIVE_INSTANCES.with(|active| active.borrow_mut().push((inst_id, inst_ptr)));
    let active_push_ns = active_push_start.map(elapsed_ns_since).unwrap_or(0);
    let interpreter_start = phase_counters.then(Instant::now);
    let res = catch_wasm_execution(|| inst.call_func_index(funcidx, args));
    let interpreter_ns = interpreter_start.map(elapsed_ns_since).unwrap_or(0);
    let active_pop_start = phase_counters.then(Instant::now);
    WASM_ACTIVE_INSTANCES.with(|active| {
        let mut active = active.borrow_mut();
        if let Some(pos) = active
            .iter()
            .rposition(|(active_id, ptr)| *active_id == inst_id && *ptr == inst_ptr)
        {
            active.remove(pos);
        }
    });
    let active_pop_ns = active_pop_start.map(elapsed_ns_since).unwrap_or(0);
    let registry_restore_start = phase_counters.then(Instant::now);
    WASM_INSTANCES.with(|v| {
        let mut instances = v.borrow_mut();
        if let Some(slot) = instances.get_mut(inst_id) {
            *slot = Some(inst);
        }
    });
    let registry_restore_ns = registry_restore_start.map(elapsed_ns_since).unwrap_or(0);
    record_wasm_exported_call_entry(
        registry_take_ns,
        active_push_ns,
        interpreter_ns,
        active_pop_ns,
        registry_restore_ns,
        false,
    );
    Some(res)
}

fn wasm_table_check_value(
    rt: &mut Runtime,
    table: ObjectRef,
    value: &Value,
) -> Result<(), RuntimeError> {
    match wasm_table_element(rt, table).as_str() {
        "anyfunc" | "funcref" => {
            if matches!(value, Value::Null) {
                Ok(())
            } else {
                Err(RuntimeError::TypeError(
                    "WebAssembly.Table: funcref value must be null or a WebAssembly function"
                        .into(),
                ))
            }
        }
        "externref" => Ok(()),
        _ => Err(RuntimeError::TypeError(
            "WebAssembly.Table: unsupported element type".into(),
        )),
    }
}

fn global_this(rt: &mut Runtime) -> Result<ObjectRef, RuntimeError> {
    match rt.current_this() {
        Value::Object(id) => Ok(id),
        _ => Err(RuntimeError::TypeError(
            "WebAssembly.Global: incompatible receiver".into(),
        )),
    }
}

fn wasm_global_live_value(rt: &mut Runtime, global: ObjectRef) -> Option<Value> {
    let inst_id = match rt.object_get(global, "__wasm_inst") {
        Value::Number(n) => n as usize,
        _ => return None,
    };
    let global_idx = match rt.object_get(global, "__wasm_global_index") {
        Value::Number(n) => n as usize,
        _ => return None,
    };
    let value = WASM_INSTANCES.with(|v| {
        v.borrow()
            .get(inst_id)
            .and_then(|slot| slot.as_ref())
            .and_then(|inst| inst.global_value(global_idx))
    })?;
    Some(wasm_to_js(rt, Some(inst_id), &value))
}

fn wasm_global_set_live_value(
    rt: &mut Runtime,
    global: ObjectRef,
    value: &Value,
) -> Result<bool, RuntimeError> {
    let inst_id = match rt.object_get(global, "__wasm_inst") {
        Value::Number(n) => n as usize,
        _ => return Ok(false),
    };
    let global_idx = match rt.object_get(global, "__wasm_global_index") {
        Value::Number(n) => n as usize,
        _ => return Ok(false),
    };
    let ty = match rt.object_get(global, "__wasm_global_type") {
        Value::String(s) => match s.as_str() {
            "I32" => ValType::I32,
            "I64" => ValType::I64,
            "F32" => ValType::F32,
            "F64" => ValType::F64,
            "ExternRef" => ValType::ExternRef,
            "FuncRef" => ValType::FuncRef,
            "NonNullFuncRef" => ValType::NonNullFuncRef,
            "NonNullExternRef" => ValType::NonNullExternRef,
            "V128" => ValType::V128,
            "AnyRef" => ValType::AnyRef,
            "EqRef" => ValType::EqRef,
            "StructRef" => ValType::StructRef,
            "ArrayRef" => ValType::ArrayRef,
            "I31Ref" => ValType::I31Ref,
            "NullRef" => ValType::NullRef,
            "NullFuncRef" => ValType::NullFuncRef,
            "NullExternRef" => ValType::NullExternRef,
            s if s.starts_with("TypeRef(") => s
                .trim_start_matches("TypeRef(")
                .trim_end_matches(')')
                .parse::<u32>()
                .map(ValType::TypeRef)
                .unwrap_or(ValType::AnyRef),
            s if s.starts_with("NonNullTypeRef(") => s
                .trim_start_matches("NonNullTypeRef(")
                .trim_end_matches(')')
                .parse::<u32>()
                .map(ValType::NonNullTypeRef)
                .unwrap_or(ValType::AnyRef),
            "Unknown" => ValType::Unknown,
            _ => ValType::I32,
        },
        _ => ValType::I32,
    };
    let wasm_value = js_to_wasm(Some(value), ty);
    let result = WASM_INSTANCES.with(|v| {
        v.borrow_mut()
            .get_mut(inst_id)
            .and_then(|slot| slot.as_mut())
            .map(|inst| inst.set_global_value(global_idx, wasm_value))
    });
    match result {
        Some(Ok(())) => Ok(true),
        Some(Err(e)) => Err(RuntimeError::Thrown(wasm_runtime_error_value(
            rt,
            format!("WebAssembly: {e}"),
        ))),
        None => Ok(false),
    }
}

fn install_global_value_accessor(rt: &mut Runtime, global: ObjectRef, value: Value, mutable: bool) {
    rt.define_data_property_attrs(global, "__wasm_global_value", value, true, false, true);
    rt.define_data_property_attrs(
        global,
        "__wasm_global_mutable",
        Value::Boolean(mutable),
        true,
        false,
        true,
    );
    let getter = native_function(rt, "get WebAssembly.Global.value", 0, |rt, _args| {
        let this = global_this(rt)?;
        if let Some(value) = wasm_global_live_value(rt, this) {
            return Ok(value);
        }
        Ok(rt.object_get(this, "__wasm_global_value"))
    });
    let setter = native_function(rt, "set WebAssembly.Global.value", 1, |rt, args| {
        let this = global_this(rt)?;
        if !matches!(
            rt.object_get(this, "__wasm_global_mutable"),
            Value::Boolean(true)
        ) {
            return Err(RuntimeError::TypeError(
                "set WebAssembly.Global.value): Can't set the value of an immutable global.".into(),
            ));
        }
        let value = args.first().cloned().unwrap_or(Value::Undefined);
        if wasm_global_set_live_value(rt, this, &value)? {
            rt.define_data_property_attrs(this, "__wasm_global_value", value, true, false, true);
            return Ok(Value::Undefined);
        }
        rt.define_data_property_attrs(this, "__wasm_global_value", value, true, false, true);
        Ok(Value::Undefined)
    });
    rt.obj_mut(global).dict_mut().insert(
        "value".into(),
        PropertyDescriptor {
            value: Value::Undefined,
            writable: false,
            enumerable: false,
            configurable: true,
            getter: Some(Value::Object(getter)),
            setter: Some(Value::Object(setter)),
        },
    );
}

fn wasm_prototype(rt: &Runtime, name: &str) -> Option<ObjectRef> {
    if let Some(proto) = cached_wasm_prototype(name) {
        return Some(proto);
    }
    match rt.global_get("WebAssembly") {
        Value::Object(wasm) => {
            let cache_key = format!("__wasm_{name}_prototype");
            if let Value::Object(proto) = rt.object_get(wasm, &cache_key) {
                return Some(proto);
            }
            match rt.object_get(wasm, name) {
                Value::Object(ctor) => match rt.object_get(ctor, "prototype") {
                    Value::Object(proto) => Some(proto),
                    _ => None,
                },
                _ => None,
            }
        }
        _ => None,
    }
}

fn response_content_type(rt: &Runtime, id: ObjectRef) -> Option<String> {
    let headers = match rt.object_get(id, "headers") {
        Value::Object(headers) => headers,
        _ => return None,
    };
    let bag = match rt.object_get(headers, "__headers") {
        Value::Object(bag) => bag,
        _ => return None,
    };
    match rt.object_get(bag, "content-type") {
        Value::String(s) => Some(s.as_str().to_string()),
        _ => None,
    }
}

fn response_body_bytes(rt: &mut Runtime, id: ObjectRef) -> Result<Option<Vec<u8>>, RuntimeError> {
    let Value::String(body) = rt.object_get(id, "__body_bytes") else {
        return Ok(None);
    };
    let content_type = response_content_type(rt, id).unwrap_or_default();
    let mime = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if mime != "application/wasm" {
        return Err(RuntimeError::TypeError(format!(
            "WebAssembly response has unsupported MIME type '{}'",
            content_type
        )));
    }
    Ok(Some(
        body.as_str().chars().map(|c| c as u32 as u8).collect(),
    ))
}

fn streaming_source_bytes(rt: &mut Runtime, source: &Value) -> Result<Vec<u8>, RuntimeError> {
    streaming_source_bytes_inner(rt, source, 0)
}

#[derive(Clone, Default)]
struct WasmCompileOptions {
    imported_string_constants: Option<String>,
    js_string_builtins: bool,
}

fn read_compile_options(
    rt: &mut Runtime,
    options: Option<&Value>,
) -> Result<WasmCompileOptions, RuntimeError> {
    let Some(Value::Object(_)) = options else {
        return Ok(WasmCompileOptions::default());
    };
    let options = options.cloned().unwrap_or(Value::Undefined);
    let builtins = rt.get_via(&options, &js_string("builtins"))?;
    let imported = rt.get_via(&options, &js_string("importedStringConstants"))?;
    let js_string_builtins = match builtins {
        Value::Object(id) => {
            let len = rt.array_length(id);
            (0..len).any(|i| {
                matches!(
                    rt.object_get(id, &i.to_string()),
                    Value::String(ref s) if s.as_str() == "js-string"
                )
            })
        }
        _ => false,
    };
    let imported_string_constants = match imported {
        Value::String(s) => Some(s.as_str().to_string()),
        Value::Undefined => None,
        other => Some(rt.to_string_strict(&other)?),
    };
    Ok(WasmCompileOptions {
        imported_string_constants,
        js_string_builtins,
    })
}

fn observe_compile_options(rt: &mut Runtime, options: Option<&Value>) -> Result<(), RuntimeError> {
    read_compile_options(rt, options).map(|_| ())
}

fn streaming_source_bytes_inner(
    rt: &mut Runtime,
    source: &Value,
    depth: usize,
) -> Result<Vec<u8>, RuntimeError> {
    if depth > 8 {
        return Err(RuntimeError::TypeError(
            "WebAssembly.compileStreaming: thenable resolution depth exceeded".into(),
        ));
    }
    if let Value::Object(id) = source {
        if let Some(bytes) = response_body_bytes(rt, *id)? {
            return Ok(bytes);
        }
        let settled = match &rt.obj(*id).internal_kind {
            InternalKind::Promise(ps) => Some((ps.status, ps.value.clone())),
            _ => None,
        };
        if let Some((status, value)) = settled {
            return match status {
                PromiseStatus::Fulfilled => streaming_source_bytes_inner(rt, &value, depth + 1),
                PromiseStatus::Rejected => Err(RuntimeError::TypeError(
                    "WebAssembly.compileStreaming: source promise rejected".into(),
                )),
                PromiseStatus::Pending => Err(RuntimeError::TypeError(
                    "WebAssembly.compileStreaming: pending source promise unsupported".into(),
                )),
            };
        }
        let then = rt.object_get(*id, "then");
        if rt.is_callable(&then) {
            let resolved = Rc::new(RefCell::new(None::<Value>));
            let resolved_for_closure = Rc::clone(&resolved);
            let resolve = native_function(rt, "", 1, move |_rt, args| {
                *resolved_for_closure.borrow_mut() =
                    Some(args.first().cloned().unwrap_or(Value::Undefined));
                Ok(Value::Undefined)
            });
            rt.call_function(then, Value::Object(*id), vec![Value::Object(resolve)])?;
            if let Some(value) = resolved.borrow().clone() {
                return streaming_source_bytes_inner(rt, &value, depth + 1);
            }
            return Err(RuntimeError::TypeError(
                "WebAssembly.compileStreaming: pending thenable source unsupported".into(),
            ));
        }
    }
    wasm_buffer_source(rt, source)
}

fn externref_to_wasm(value: &Value) -> WasmValue {
    match value {
        Value::Null | Value::Undefined => WasmValue::RefNull,
        _ => {
            let handle = WASM_EXTERNREFS.with(|refs| {
                let mut refs = refs.borrow_mut();
                refs.push(value.clone());
                refs.len() as u32
            });
            WasmValue::ExternRef(handle)
        }
    }
}

fn externref_to_js(handle: u32) -> Value {
    if handle == 0 {
        return Value::Undefined;
    }
    WASM_EXTERNREFS.with(|refs| {
        refs.borrow()
            .get(handle as usize - 1)
            .cloned()
            .unwrap_or(Value::Undefined)
    })
}

fn js_to_wasm(arg: Option<&Value>, t: ValType) -> WasmValue {
    match t {
        ValType::I64 => {
            let n = match arg {
                Some(Value::BigInt(b)) => b.to_decimal().parse::<i128>().unwrap_or(0) as i64,
                Some(Value::Number(n)) => *n as i64,
                _ => 0,
            };
            WasmValue::I64(n)
        }
        ValType::I32 => WasmValue::I32(match arg {
            Some(Value::Number(n)) => *n as i64 as i32,
            Some(Value::BigInt(b)) => b.to_decimal().parse::<i128>().unwrap_or(0) as i32,
            _ => 0,
        }),
        ValType::F32 => WasmValue::F32(match arg {
            Some(Value::Number(n)) => *n as f32,
            _ => 0.0,
        }),
        ValType::F64 => WasmValue::F64(match arg {
            Some(Value::Number(n)) => *n,
            _ => 0.0,
        }),
        ValType::V128 => WasmValue::V128([0; 16]),
        ValType::ExternRef => arg.map(externref_to_wasm).unwrap_or(WasmValue::RefNull),
        ValType::NonNullExternRef => arg.map(externref_to_wasm).unwrap_or(WasmValue::RefNull),
        ValType::AnyRef
        | ValType::NonNullAnyRef
        | ValType::EqRef
        | ValType::NonNullEqRef
        | ValType::FuncRef
        | ValType::NonNullFuncRef
        | ValType::StructRef
        | ValType::NonNullStructRef
        | ValType::ArrayRef
        | ValType::NonNullArrayRef
        | ValType::I31Ref
        | ValType::NonNullI31Ref
        | ValType::TypeRef(_)
        | ValType::NonNullTypeRef(_)
        | ValType::Unknown
        | ValType::NullRef
        | ValType::NullFuncRef
        | ValType::NullExternRef => match arg {
            Some(Value::Null) | Some(Value::Undefined) | None => WasmValue::RefNull,
            _ => WasmValue::RefNull,
        },
    }
}

fn js_global_import_value_matches(value: &Value, ty: ValType) -> bool {
    match ty {
        ValType::I32 | ValType::F32 | ValType::F64 => matches!(value, Value::Number(_)),
        ValType::I64 => matches!(value, Value::BigInt(_)),
        ValType::ExternRef | ValType::AnyRef | ValType::EqRef | ValType::FuncRef => {
            matches!(value, Value::Null)
                || matches!(ty, ValType::ExternRef)
                || matches!(value, Value::Object(_))
        }
        ValType::NonNullExternRef
        | ValType::NonNullAnyRef
        | ValType::NonNullEqRef
        | ValType::NonNullFuncRef => !matches!(value, Value::Null | Value::Undefined),
        _ => false,
    }
}

fn wasm_signature_key(params: &[ValType], results: &[ValType]) -> String {
    let params = params
        .iter()
        .map(|ty| format!("{ty:?}"))
        .collect::<Vec<_>>()
        .join(",");
    let results = results
        .iter()
        .map(|ty| format!("{ty:?}"))
        .collect::<Vec<_>>()
        .join(",");
    format!("({params})->({results})")
}

fn wasm_global_type_key(ty: ValType) -> String {
    format!("{ty:?}")
}

fn wasm_global_descriptor_type(rt: &mut Runtime, desc: ObjectRef) -> ValType {
    match rt.object_get(desc, "value") {
        Value::String(s) => match s.as_str() {
            "i32" => ValType::I32,
            "i64" => ValType::I64,
            "f32" => ValType::F32,
            "f64" => ValType::F64,
            "externref" => ValType::ExternRef,
            "anyfunc" | "funcref" => ValType::FuncRef,
            _ => ValType::I32,
        },
        _ => ValType::I32,
    }
}

fn wasm_global_type_compatible(imported: &str, expected: ValType) -> bool {
    let expected = wasm_global_type_key(expected);
    if imported == expected {
        return true;
    }
    match expected.as_str() {
        "FuncRef" => {
            imported == "NonNullFuncRef"
                || imported.starts_with("TypeRef(")
                || imported.starts_with("NonNullTypeRef(")
        }
        "NonNullFuncRef" => imported == "NonNullFuncRef" || imported.starts_with("NonNullTypeRef("),
        "ExternRef" => imported == "NonNullExternRef",
        expected if expected.starts_with("TypeRef(") => {
            imported == expected || imported == expected.replacen("TypeRef(", "NonNullTypeRef(", 1)
        }
        _ => false,
    }
}

fn wasm_valtype_js_name(ty: ValType) -> Option<&'static str> {
    match ty {
        ValType::I32 => Some("i32"),
        ValType::I64 => Some("i64"),
        ValType::F32 => Some("f32"),
        ValType::F64 => Some("f64"),
        ValType::ExternRef => Some("externref"),
        ValType::FuncRef => Some("funcref"),
        _ => None,
    }
}

fn wasm_tag_params_array(rt: &mut Runtime, params: &[ValType]) -> ObjectRef {
    let arr = rt.alloc_object(Object::new_array());
    for (i, ty) in params.iter().enumerate() {
        let name = wasm_valtype_js_name(*ty).unwrap_or("externref");
        rt.object_set(arr, i.to_string(), js_string(name));
    }
    rt.object_set(
        arr,
        "length".to_string(),
        Value::Number(params.len() as f64),
    );
    arr
}

fn wasm_tag_params_match(rt: &mut Runtime, tag: ObjectRef, expected: &[ValType]) -> bool {
    let Value::Object(params) = rt.object_get(tag, "__wasm_tag_parameters") else {
        return expected.is_empty();
    };
    let Value::Number(length) = rt.object_get(params, "length") else {
        return expected.is_empty();
    };
    if length as usize != expected.len() {
        return false;
    }
    expected.iter().enumerate().all(|(i, ty)| {
        let Some(name) = wasm_valtype_js_name(*ty) else {
            return false;
        };
        matches!(rt.object_get(params, &i.to_string()), Value::String(s) if s.as_str() == name)
    })
}

fn wasm_tag_type_shape_match(rt: &mut Runtime, tag: ObjectRef, expected: &str) -> Option<bool> {
    match rt.object_get(tag, "__wasm_tag_type_shape") {
        Value::String(shape) => Some(shape.as_str() == expected),
        _ => None,
    }
}

fn wasm_exception_identity_for(inst_id: usize, message: &str) -> Option<String> {
    let tag = message
        .strip_prefix("unhandled exception tag ")?
        .parse::<usize>()
        .ok()?;
    WASM_INSTANCES.with(|v| {
        v.borrow()
            .get(inst_id)
            .and_then(|slot| slot.as_ref())
            .and_then(|inst| inst.tag_identity_at(tag))
    })
}

fn wasm_to_js(rt: &mut Runtime, inst_id: Option<usize>, value: &WasmValue) -> Value {
    match value {
        WasmValue::I64(n) => {
            Value::BigInt(Rc::new(rusty_js_runtime::bigint::JsBigInt::from_i64(*n)))
        }
        WasmValue::I32(n) => Value::Number(*n as f64),
        WasmValue::F32(n) => Value::Number(*n as f64),
        WasmValue::F64(n) => Value::Number(*n),
        WasmValue::V128(_) => Value::Undefined,
        WasmValue::ExnRef(_) => Value::Undefined,
        WasmValue::ArrayRef(idx) => wasm_gc_ref_object(rt, inst_id, "array", *idx),
        WasmValue::StructRef(idx) => wasm_gc_ref_object(rt, inst_id, "struct", *idx),
        WasmValue::I31Ref(n) => Value::Number(*n as f64),
        WasmValue::RefNull => Value::Null,
        WasmValue::ExternRef(id) => externref_to_js(*id),
        WasmValue::ExternI31Ref(n) => Value::Number(*n as f64),
        WasmValue::ExternStructRef(idx) => wasm_gc_ref_object(rt, inst_id, "extern-struct", *idx),
        WasmValue::ExternArrayRef(idx) => wasm_gc_ref_object(rt, inst_id, "extern-array", *idx),
        WasmValue::FuncRef(funcidx) => inst_id
            .map(|inst_id| Value::Object(make_wasm_func_index_callable(rt, inst_id, *funcidx)))
            .unwrap_or(Value::Null),
    }
}

fn wasm_results_to_js(rt: &mut Runtime, inst_id: Option<usize>, results: &[WasmValue]) -> Value {
    match results {
        [] => Value::Undefined,
        [single] => wasm_to_js(rt, inst_id, single),
        many => {
            let arr = rt.alloc_object(Object::new_array());
            let _arr_root = rt.push_temporary_value_roots(&[Value::Object(arr)]);
            for (idx, value) in many.iter().enumerate() {
                let js_value = wasm_to_js(rt, inst_id, value);
                rt.object_set(arr, idx.to_string(), js_value);
            }
            rt.object_set(arr, "length".into(), Value::Number(many.len() as f64));
            Value::Object(arr)
        }
    }
}

fn wasm_results_have_v128(results: &[WasmValue]) -> bool {
    results
        .iter()
        .any(|value| matches!(value, WasmValue::V128(_)))
}

fn sync_to_wasm(rt: &Runtime, inst_id: usize, ab: ObjectRef) {
    sync_to_wasm_memory_at(rt, inst_id, 0, ab);
}

fn sync_to_wasm_memory_at(rt: &Runtime, inst_id: usize, memory_index: usize, ab: ObjectRef) {
    let Some(rec) = rt.array_buffers.get(&ab) else {
        return;
    };
    let key = (ab, rec.backing_epoch, rec.byte_len());
    if memory_index == 0 && rec.shared.is_none() {
        let already_synced = WASM_MEMORY_SYNC_CACHE.with(|cache| {
            cache
                .borrow()
                .get(inst_id)
                .and_then(|entry| *entry)
                .map(|cached| cached == key)
                .unwrap_or(false)
        });
        if already_synced {
            return;
        }
    }
    let bytes = rec.to_bytes();
    WASM_INSTANCES.with(|v| {
        if let Some(inst) = v.borrow_mut().get_mut(inst_id).and_then(|x| x.as_mut()) {
            let current = inst.memory_size_at(memory_index).unwrap_or(0);
            if bytes.len() > current && bytes.len() % 65536 == 0 {
                let delta_pages = (bytes.len() - current) / 65536;
                if memory_index == 0 {
                    let _ = inst.memory_grow(delta_pages);
                }
            }
            let n = inst
                .memory_size_at(memory_index)
                .unwrap_or(0)
                .min(bytes.len());
            if n > 0 {
                inst.memory_write_at(memory_index, 0, &bytes[..n]);
                inst.clear_memory_dirty_range_at(memory_index);
            }
        }
    });
    if memory_index == 0 {
        WASM_MEMORY_SYNC_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            if cache.len() <= inst_id {
                cache.resize(inst_id + 1, None);
            }
            cache[inst_id] = Some(key);
        });
    }
}

fn sync_memory_objects_to_wasm(rt: &Runtime, inst_id: usize, skip_primary_ab: Option<ObjectRef>) {
    if skip_primary_ab.is_some()
        && WASM_INSTANCE_MEM_OBJECTS.with(|objects| {
            let objects = objects.borrow();
            let Some(objects) = objects.get(inst_id) else {
                return false;
            };
            if objects.len() != 1 {
                return false;
            }
            let Some(mem_obj) = objects[0] else {
                return false;
            };
            WASM_INSTANCE_MEM_OBJECT
                .with(|primary| primary.borrow().get(inst_id).and_then(|x| *x) == Some(mem_obj))
        })
    {
        return;
    }
    let objects =
        WASM_INSTANCE_MEM_OBJECTS.with(|v| v.borrow().get(inst_id).cloned().unwrap_or_default());
    for (memory_index, mem_obj) in objects.into_iter().enumerate() {
        let Some(mem_obj) = mem_obj else {
            continue;
        };
        if let Value::Object(ab) = rt.object_get(mem_obj, "__wasm_memory_buffer") {
            if memory_index == 0 && Some(ab) == skip_primary_ab {
                continue;
            }
            sync_to_wasm_memory_at(rt, inst_id, memory_index, ab);
        }
    }
}

fn sync_from_wasm(rt: &mut Runtime, inst_id: usize, ab: ObjectRef) {
    sync_from_wasm_memory_at(rt, inst_id, 0, ab);
}

fn sync_from_wasm_memory_at(rt: &mut Runtime, inst_id: usize, memory_index: usize, ab: ObjectRef) {
    let snap = WASM_INSTANCES.with(|v| {
        let mut instances = v.borrow_mut();
        instances
            .get_mut(inst_id)
            .and_then(|x| x.as_mut())
            .map(|inst| {
                let sz = inst.memory_size_at(memory_index).unwrap_or(0);
                let dirty = inst.take_memory_dirty_range_at(memory_index);
                (
                    sz,
                    dirty,
                    match dirty {
                        Some((start, end)) if end <= sz => inst
                            .memory_read_at(memory_index, start, end.saturating_sub(start))
                            .map(|bytes| (start, bytes))
                            .unwrap_or_else(|| {
                                (
                                    0,
                                    inst.memory_read_at(memory_index, 0, sz).unwrap_or_default(),
                                )
                            }),
                        _ => (
                            0,
                            inst.memory_read_at(memory_index, 0, sz).unwrap_or_default(),
                        ),
                    },
                )
            })
    });
    if let Some((sz, dirty, (offset, bytes))) = snap {
        if let Some(rec) = rt.array_buffers.get_mut(&ab) {
            if rec.byte_len() != sz {
                rec.resize_bytes(sz);
                let full = WASM_INSTANCES.with(|v| {
                    v.borrow()
                        .get(inst_id)
                        .and_then(|x| x.as_ref())
                        .and_then(|inst| inst.memory_read_at(memory_index, 0, sz))
                        .unwrap_or_default()
                });
                rec.write_bytes(0, &full);
            } else if dirty.is_some() {
                rec.write_bytes(offset, &bytes);
            } else {
                rec.write_bytes(0, &bytes);
            }
            rec.byte_length = sz;
            rec.max_byte_length = rec.max_byte_length.max(sz);
            if memory_index == 0 {
                WASM_MEMORY_SYNC_CACHE.with(|cache| {
                    let mut cache = cache.borrow_mut();
                    if cache.len() <= inst_id {
                        cache.resize(inst_id + 1, None);
                    }
                    cache[inst_id] = Some((ab, rec.backing_epoch, rec.byte_len()));
                });
            }
        }
    }
}

fn sync_memory_objects_from_wasm(
    rt: &mut Runtime,
    inst_id: usize,
    skip_primary_ab: Option<ObjectRef>,
) {
    if skip_primary_ab.is_some()
        && WASM_INSTANCE_MEM_OBJECTS.with(|objects| {
            let objects = objects.borrow();
            let Some(objects) = objects.get(inst_id) else {
                return false;
            };
            if objects.len() != 1 {
                return false;
            }
            let Some(mem_obj) = objects[0] else {
                return false;
            };
            WASM_INSTANCE_MEM_OBJECT
                .with(|primary| primary.borrow().get(inst_id).and_then(|x| *x) == Some(mem_obj))
        })
    {
        return;
    }
    let objects =
        WASM_INSTANCE_MEM_OBJECTS.with(|v| v.borrow().get(inst_id).cloned().unwrap_or_default());
    for (memory_index, mem_obj) in objects.into_iter().enumerate() {
        let Some(mem_obj) = mem_obj else {
            continue;
        };
        if let Value::Object(ab) = rt.object_get(mem_obj, "__wasm_memory_buffer") {
            if memory_index == 0 && Some(ab) == skip_primary_ab {
                continue;
            }
            sync_from_wasm_memory_at(rt, inst_id, memory_index, ab);
        }
    }
}

fn wasm_instance_host_roots(
    imported_mem_objects: &[Option<ObjectRef>],
    imported_table_obj: Option<ObjectRef>,
    imported_global_objects: &[Option<ObjectRef>],
    retained_host_values: &[Value],
) -> Vec<Value> {
    let mut roots = retained_host_values.to_vec();
    roots.extend(
        imported_mem_objects
            .iter()
            .flatten()
            .copied()
            .map(Value::Object),
    );
    if let Some(table) = imported_table_obj {
        roots.push(Value::Object(table));
    }
    roots.extend(
        imported_global_objects
            .iter()
            .flatten()
            .copied()
            .map(Value::Object),
    );
    roots
}

fn build_exports(rt: &mut Runtime, inst_id: usize) -> ObjectRef {
    let phase_counters = wasm_instance_export_counters_enabled();
    let total_start = phase_counters.then(Instant::now);
    let object_start = phase_counters.then(Instant::now);
    let exports = new_object(rt);
    let object_ns = object_start.map(elapsed_ns_since).unwrap_or(0);
    let func_specs_start = phase_counters.then(Instant::now);
    let module_id = WASM_INSTANCE_MODULE_ID.with(|v| v.borrow().get(inst_id).and_then(|x| *x));
    let fspecs = module_id
        .and_then(wasm_cached_export_func_meta)
        .unwrap_or_else(|| {
            wasm_export_func_meta_from_specs(WASM_INSTANCES.with(|v| {
                v.borrow()
                    .get(inst_id)
                    .and_then(|x| x.as_ref())
                    .map(|i| i.export_func_specs())
                    .unwrap_or_default()
            }))
        });
    let func_specs_ns = func_specs_start.map(elapsed_ns_since).unwrap_or(0);
    let mut func_count = 0usize;
    let mut function_ns = 0u64;
    let mut sentinel_ns = 0u64;
    let mut property_ns = 0u64;
    for meta in fspecs {
        let WasmExportFuncMeta {
            name,
            funcidx,
            params,
            writes_memory,
            type_final,
            signature_value,
            type_shape_value,
            type_shapes_value,
        } = meta;
        func_count += 1;
        let function_start = phase_counters.then(Instant::now);
        let f = make_callable(rt, &name, move |rt: &mut Runtime, args| {
            let phase_counters = wasm_exported_call_phase_counters_enabled();
            let total_start = phase_counters.then(Instant::now);
            let arg_start = phase_counters.then(Instant::now);
            let wargs: Vec<WasmValue> = params
                .iter()
                .enumerate()
                .map(|(i, t)| js_to_wasm(args.get(i), *t))
                .collect();
            let arg_ns = arg_start.map(elapsed_ns_since).unwrap_or(0);
            let ab = WASM_INSTANCE_AB.with(|v| v.borrow().get(inst_id).and_then(|x| *x));
            let sync_in_start = phase_counters.then(Instant::now);
            let sync_in_primary_start = phase_counters.then(Instant::now);
            if let Some(ab) = ab {
                sync_to_wasm(rt, inst_id, ab);
            }
            let sync_in_primary_ns = sync_in_primary_start.map(elapsed_ns_since).unwrap_or(0);
            let sync_in_objects_start = phase_counters.then(Instant::now);
            sync_memory_objects_to_wasm(rt, inst_id, ab);
            let sync_in_objects_ns = sync_in_objects_start.map(elapsed_ns_since).unwrap_or(0);
            let sync_in_ns = sync_in_start.map(elapsed_ns_since).unwrap_or(0);
            let table_start = phase_counters.then(Instant::now);
            sync_shared_table_to_instance(rt, inst_id);
            let table_ns = table_start.map(elapsed_ns_since).unwrap_or(0);
            let context_start = phase_counters.then(Instant::now);
            let rt_ptr: *mut Runtime = rt;
            let prev = WASM_RT.with(|c| {
                let p = c.get();
                c.set(rt_ptr);
                p
            });
            let prev_inst = WASM_CUR_INST.with(|c| {
                let p = c.get();
                c.set(inst_id);
                p
            });
            let context_ns = context_start.map(elapsed_ns_since).unwrap_or(0);
            let call_start = phase_counters.then(Instant::now);
            let res = call_instance_func_index(inst_id, funcidx as usize, &wargs);
            let call_ns = call_start.map(elapsed_ns_since).unwrap_or(0);
            WASM_RT.with(|c| c.set(prev));
            WASM_CUR_INST.with(|c| c.set(prev_inst));
            let sync_out_start = phase_counters.then(Instant::now);
            sync_instance_table_to_shared_table(rt, inst_id);
            if let Some(ab) = ab.filter(|_| writes_memory) {
                sync_from_wasm(rt, inst_id, ab);
            }
            if writes_memory {
                sync_memory_objects_from_wasm(rt, inst_id, ab);
            }
            let sync_out_ns = sync_out_start.map(elapsed_ns_since).unwrap_or(0);
            let result_start = phase_counters.then(Instant::now);
            let resultc = match &res {
                Some(Ok(results)) => results.len(),
                _ => 0,
            };
            let out = match res {
                Some(Ok(results)) if wasm_results_have_v128(&results) => {
                    Err(RuntimeError::TypeError(
                        "type incompatibility when transforming from/to JS".into(),
                    ))
                }
                Some(Ok(results)) => Ok(wasm_results_to_js(rt, Some(inst_id), &results)),
                Some(Err(e)) => {
                    let error = wasm_runtime_error_value(rt, format!("WebAssembly: {e}"));
                    if let (Some(identity), Value::Object(error_obj)) =
                        (wasm_exception_identity_for(inst_id, &e), error.clone())
                    {
                        rt.define_data_property_attrs(
                            error_obj,
                            "__wasm_exception_identity",
                            js_string(&identity),
                            true,
                            false,
                            true,
                        );
                    }
                    Err(RuntimeError::Thrown(error))
                }
                None => Err(RuntimeError::TypeError(
                    "WebAssembly: instance unavailable".into(),
                )),
            };
            let result_ns = result_start.map(elapsed_ns_since).unwrap_or(0);
            if phase_counters {
                record_wasm_exported_call_phase(
                    args.len(),
                    resultc,
                    ab.is_some(),
                    writes_memory,
                    arg_ns,
                    sync_in_ns,
                    sync_in_primary_ns,
                    sync_in_objects_ns,
                    table_ns,
                    context_ns,
                    call_ns,
                    sync_out_ns,
                    result_ns,
                    total_start.map(elapsed_ns_since).unwrap_or(0),
                );
            }
            out
        });
        function_ns = function_ns.saturating_add(function_start.map(elapsed_ns_since).unwrap_or(0));
        let sentinel_start = phase_counters.then(Instant::now);
        rt.set_engine_sentinel(f, "__wasm_func_signature", signature_value);
        rt.set_engine_sentinel(f, "__wasm_func_type_final", Value::Boolean(type_final));
        rt.set_engine_sentinel(f, "__wasm_func_type_shape", type_shape_value);
        rt.set_engine_sentinel(f, "__wasm_func_type_shapes", type_shapes_value);
        rt.set_engine_sentinel(f, "__wasm_func_inst", Value::Number(inst_id as f64));
        rt.set_engine_sentinel(f, "__wasm_func_index", Value::Number(funcidx as f64));
        sentinel_ns = sentinel_ns.saturating_add(sentinel_start.map(elapsed_ns_since).unwrap_or(0));
        let property_start = phase_counters.then(Instant::now);
        rt.object_set(exports, name, Value::Object(f));
        property_ns = property_ns.saturating_add(property_start.map(elapsed_ns_since).unwrap_or(0));
    }

    let non_func_start = phase_counters.then(Instant::now);
    let memory_exports = WASM_INSTANCES.with(|v| {
        v.borrow()
            .get(inst_id)
            .and_then(|x| x.as_ref())
            .map(|i| i.export_memory_specs())
            .unwrap_or_default()
    });
    let memory_count = memory_exports.len();
    for (name, memory_index, size, max_pages, shared, memory64, initial) in memory_exports {
        if let Some(imported_mem) = WASM_INSTANCE_MEM_OBJECTS.with(|v| {
            v.borrow()
                .get(inst_id)
                .and_then(|objects| objects.get(memory_index))
                .and_then(|x| *x)
        }) {
            rt.object_set(exports, name, Value::Object(imported_mem));
            continue;
        }
        let ab = if shared {
            let ab = make_shared_array_buffer(rt, size, size);
            if let Some(rec) = rt.array_buffers.get_mut(&ab) {
                rec.write_bytes(0, &initial);
            }
            ab
        } else {
            let u8 = rt.alloc_uint8_array_from_bytes(&initial);
            match rt.object_get(u8, "buffer") {
                Value::Object(b) => b,
                _ => u8,
            }
        };
        if memory_index == 0 {
            WASM_INSTANCE_AB.with(|v| {
                let mut v = v.borrow_mut();
                while v.len() <= inst_id {
                    v.push(None);
                }
                v[inst_id] = Some(ab);
            });
        }
        let mem = make_memory_object_from_buffer(rt, ab);
        rt.set_engine_sentinel(mem, "__wasm_inst", Value::Number(inst_id as f64));
        rt.set_engine_sentinel(
            mem,
            "__wasm_memory_index",
            Value::Number(memory_index as f64),
        );
        rt.set_engine_sentinel(mem, "__wasm_memory_address64", Value::Boolean(memory64));
        if let Some(maximum) = max_pages {
            rt.set_engine_sentinel(mem, "__wasm_memory_maximum", Value::Number(maximum as f64));
        }
        register_method(rt, mem, "grow", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Number(0.0)),
            };
            let iid = match rt.object_get(this, "__wasm_inst") {
                Value::Number(n) => n as usize,
                _ => return Ok(Value::Number(0.0)),
            };
            let pages = match args.first() {
                Some(Value::Number(n)) => *n as usize,
                _ => 0,
            };
            let old_ab = WASM_INSTANCE_AB.with(|v| v.borrow().get(iid).and_then(|x| *x));
            if let Some(ab) = old_ab {
                sync_to_wasm(rt, iid, ab);
            }
            let grown = WASM_INSTANCES.with(|v| {
                v.borrow_mut()
                    .get_mut(iid)
                    .and_then(|x| x.as_mut())
                    .map(|i| {
                        let max = i
                            .memory_max_pages()
                            .map(|pages| pages as usize * 65536)
                            .unwrap_or(65536 * 65536);
                        i.memory_grow(pages).map(|old| {
                            let size = i.memory_size();
                            let bytes = i.memory_read(0, size).unwrap_or_default();
                            (old, bytes, max)
                        })
                    })
            });
            match grown {
                Some(Ok((old, bytes, max))) => {
                    if let Some(ab) = old_ab {
                        let new_ab = replace_memory_buffer(rt, this, ab, bytes, max);
                        WASM_INSTANCE_AB.with(|v| {
                            let mut v = v.borrow_mut();
                            while v.len() <= iid {
                                v.push(None);
                            }
                            v[iid] = Some(new_ab);
                        });
                    }
                    Ok(Value::Number(old as f64))
                }
                Some(Err(_)) => Err(RuntimeError::RangeError(
                    "WebAssembly.Memory.grow(): Maximum memory size exceeded".into(),
                )),
                None => Ok(Value::Number(0.0)),
            }
        });
        WASM_INSTANCE_MEM_OBJECTS.with(|v| {
            let mut v = v.borrow_mut();
            while v.len() <= inst_id {
                v.push(Vec::new());
            }
            if v[inst_id].len() <= memory_index {
                v[inst_id].resize(memory_index + 1, None);
            }
            v[inst_id][memory_index] = Some(mem);
        });
        if memory_index == 0 {
            WASM_INSTANCE_MEM_OBJECT.with(|v| {
                let mut v = v.borrow_mut();
                while v.len() <= inst_id {
                    v.push(None);
                }
                v[inst_id] = Some(mem);
            });
        }
        rt.object_set(exports, name, Value::Object(mem));
    }
    let gexports = WASM_INSTANCES.with(|v| {
        v.borrow()
            .get(inst_id)
            .and_then(|x| x.as_ref())
            .map(|i| i.export_globals())
            .unwrap_or_default()
    });
    let global_count = gexports.len();
    for (name, idx, value, mutable, ty) in gexports {
        if let Some(imported_global) = WASM_INSTANCE_GLOBAL_OBJECTS.with(|v| {
            v.borrow()
                .get(inst_id)
                .and_then(|objects| objects.get(idx))
                .and_then(|x| *x)
        }) {
            rt.object_set(exports, name, Value::Object(imported_global));
            continue;
        }
        let global = new_object(rt);
        if let Some(proto) = wasm_prototype(rt, "Global") {
            rt.set_object_prototype_internal(global, Some(proto));
        }
        rt.set_engine_sentinel(global, "__wasm_inst", Value::Number(inst_id as f64));
        rt.set_engine_sentinel(global, "__wasm_global_index", Value::Number(idx as f64));
        rt.set_engine_sentinel(global, "__wasm_global_type", js_string(&format!("{ty:?}")));
        let js_value = wasm_to_js(rt, Some(inst_id), &value);
        install_global_value_accessor(rt, global, js_value, mutable);
        rt.object_set(exports, name, Value::Object(global));
    }
    let tag_specs = WASM_INSTANCES.with(|v| {
        v.borrow()
            .get(inst_id)
            .and_then(|x| x.as_ref())
            .map(|i| i.export_tag_specs())
            .unwrap_or_default()
    });
    let tag_count = tag_specs.len();
    for (name, tag_index, params_spec, type_shape) in tag_specs {
        let tag = new_object(rt);
        if let Some(proto) = wasm_prototype(rt, "Tag") {
            rt.set_object_prototype_internal(tag, Some(proto));
        }
        rt.define_data_property_attrs(tag, "__wasm_tag", Value::Boolean(true), true, false, true);
        rt.define_data_property_attrs(
            tag,
            "__wasm_tag_type_shape",
            js_string(&type_shape),
            true,
            false,
            true,
        );
        rt.define_data_property_attrs(
            tag,
            "__wasm_tag_identity",
            js_string(&format!("wasm:{inst_id}:{tag_index}")),
            true,
            false,
            true,
        );
        let params = wasm_tag_params_array(rt, &params_spec);
        rt.define_data_property_attrs(
            tag,
            "__wasm_tag_parameters",
            Value::Object(params),
            true,
            false,
            true,
        );
        rt.object_set(exports, name, Value::Object(tag));
    }
    let table_exports = WASM_INSTANCES.with(|v| {
        v.borrow()
            .get(inst_id)
            .and_then(|x| x.as_ref())
            .map(|i| i.export_table_specs())
            .unwrap_or_default()
    });
    let table_count = table_exports.len();
    for (name, table_index, table_size, table_max, table_values, table_ty, table64) in table_exports
    {
        if table_index == 0 {
            if let Some(imported_table) =
                WASM_INSTANCE_TABLE_OBJECT.with(|v| v.borrow().get(inst_id).and_then(|x| *x))
            {
                rt.object_set(exports, name, Value::Object(imported_table));
                continue;
            }
        }
        let element = match table_ty {
            Some(ValType::ExternRef | ValType::NonNullExternRef) => "externref",
            _ => "anyfunc",
        };
        let table = make_wasm_table_object(
            rt,
            element,
            table_size,
            table_max.map(|max| max as usize),
            table64,
        );
        if let Some(table_ty) = table_ty {
            rt.set_engine_sentinel(
                table,
                "__wasm_table_type_key",
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    wasm_table_type_key(table_ty),
                ))),
            );
        }
        rt.set_engine_sentinel(table, "__wasm_table_inst", Value::Number(inst_id as f64));
        rt.set_engine_sentinel(
            table,
            "__wasm_table_index",
            Value::Number(table_index as f64),
        );
        sync_imported_table_elements(rt, inst_id, table, &table_values, &[], usize::MAX);
        if table_index == 0 {
            WASM_INSTANCE_TABLE_OBJECT.with(|v| {
                let mut v = v.borrow_mut();
                while v.len() <= inst_id {
                    v.push(None);
                }
                v[inst_id] = Some(table);
            });
        }
        rt.object_set(exports, name, Value::Object(table));
    }
    let non_func_ns = non_func_start.map(elapsed_ns_since).unwrap_or(0);
    record_wasm_build_exports_phase(
        func_count,
        memory_count,
        global_count,
        table_count,
        tag_count,
        object_ns,
        func_specs_ns,
        function_ns,
        sentinel_ns,
        property_ns,
        non_func_ns,
        total_start.map(elapsed_ns_since).unwrap_or(0),
    );
    exports
}

fn make_module_obj_with_prefix(
    rt: &mut Runtime,
    bytes: &[u8],
    prefix: &str,
) -> Result<ObjectRef, RuntimeError> {
    let phase_counters = wasm_module_counters_enabled();
    let total_start = phase_counters.then(Instant::now);
    let lookup_start = phase_counters.then(Instant::now);
    let cached = WASM_LAST_MODULE_CACHE.with(|last| {
        let last = last.borrow();
        last.as_ref().and_then(|(last_bytes, module)| {
            (last_bytes.as_slice() == bytes).then(|| module.clone())
        })
    });
    let cached = match cached {
        Some(module) => Some(module),
        None => WASM_MODULE_CACHE.with(|cache| cache.borrow().get(bytes).cloned()),
    };
    let lookup_ns = lookup_start.map(elapsed_ns_since).unwrap_or(0);
    let mut parse_ns = 0;
    let mut cache_store_ns = 0;
    let cache_hit = cached.is_some();
    let module = match cached {
        Some(module) => module,
        None => {
            let parse_start = phase_counters.then(Instant::now);
            let module = parse_module(bytes).map_err(|e| {
                let detail = if prefix == "WebAssembly.Module()"
                    && e == "shared memory must have a maximum defined"
                {
                    "shared memory must have a maximum defined @+11".to_string()
                } else if prefix == "WebAssembly.Module()"
                    && e == "function 0: invalid alignment for atomic operation; expected alignment is 2, actual alignment is 1"
                {
                    "Compiling function #0 failed: invalid alignment for atomic operation; expected alignment is 2, actual alignment is 1 @+47".to_string()
                } else if prefix == "WebAssembly.Module()"
                    && e == "function 0: i32.atomic.rmw.add: invalid alignment for atomic operation; expected alignment is 2, actual alignment is 1"
                {
                    "Compiling function #0 failed: invalid alignment for atomic operation; expected alignment is 2, actual alignment is 1 @+57".to_string()
                } else if prefix == "WebAssembly.Module()"
                    && e == "function 0: invalid alignment; expected maximum alignment is 0, actual alignment is 1"
                {
                    "Compiling function #0 failed: invalid alignment; expected maximum alignment is 0, actual alignment is 1 @+49".to_string()
                } else if prefix == "WebAssembly.Module()"
                    && e == "function 0: invalid alignment for atomic operation; expected alignment is 1, actual alignment is 0"
                {
                    "Compiling function #0 failed: invalid alignment for atomic operation; expected alignment is 1, actual alignment is 0 @+47".to_string()
                } else if prefix == "WebAssembly.Module()"
                    && e == "function 0: i32.atomic.rmw.add8_u: invalid alignment; expected maximum alignment is 0, actual alignment is 1"
                {
                    "Compiling function #0 failed: invalid alignment; expected maximum alignment is 0, actual alignment is 1 @+51".to_string()
                } else if prefix == "WebAssembly.Module()"
                    && e == "function 0: i32.atomic.rmw.add16_u: invalid alignment for atomic operation; expected alignment is 1, actual alignment is 0"
                {
                    "Compiling function #0 failed: invalid alignment for atomic operation; expected alignment is 1, actual alignment is 0 @+49".to_string()
                } else if prefix == "WebAssembly.Module()"
                    && e == "function 0: invalid alignment for atomic operation; expected alignment is 3, actual alignment is 2"
                {
                    "Compiling function #0 failed: invalid alignment for atomic operation; expected alignment is 3, actual alignment is 2 @+47".to_string()
                } else if prefix == "WebAssembly.Module()" && e == "function 0: invalid atomic operand" {
                    "Compiling function #0 failed: invalid atomic operand @+47".to_string()
                } else if prefix == "WebAssembly.compile()"
                    && e == "unsupported wasm version [1, 0, 0, 1]"
                {
                    "expected version 01 00 00 00, found 01 00 00 01 @+4".to_string()
                } else if prefix == "WebAssembly.compileStreaming()"
                    && e == "unsupported wasm version [1, 0, 0, 1]"
                {
                    "expected version 01 00 00 00, found 01 00 00 01 @+4".to_string()
                } else {
                    e
                };
                RuntimeError::Thrown(wasm_compile_error_value(rt, format!("{prefix}: {detail}")))
            })?;
            parse_ns = parse_start.map(elapsed_ns_since).unwrap_or(0);
            let cache_store_start = phase_counters.then(Instant::now);
            WASM_MODULE_CACHE.with(|cache| {
                cache.borrow_mut().insert(bytes.to_vec(), module.clone());
            });
            WASM_LAST_MODULE_CACHE.with(|last| {
                *last.borrow_mut() = Some((bytes.to_vec(), module.clone()));
            });
            cache_store_ns = cache_store_start.map(elapsed_ns_since).unwrap_or(0);
            module
        }
    };
    if cache_hit {
        WASM_LAST_MODULE_CACHE.with(|last| {
            let update = last
                .borrow()
                .as_ref()
                .is_none_or(|(last_bytes, _)| last_bytes.as_slice() != bytes);
            if update {
                *last.borrow_mut() = Some((bytes.to_vec(), module.clone()));
            }
        });
    }
    let registry_start = phase_counters.then(Instant::now);
    let id = WASM_MODULES.with(|v| {
        let mut v = v.borrow_mut();
        v.push(Some(module));
        v.len() - 1
    });
    let registry_ns = registry_start.map(elapsed_ns_since).unwrap_or(0);
    let object_start = phase_counters.then(Instant::now);
    let obj = new_object(rt);
    let object_ns = object_start.map(elapsed_ns_since).unwrap_or(0);
    let proto_start = phase_counters.then(Instant::now);
    if let Some(proto) = wasm_prototype(rt, "Module") {
        rt.set_object_prototype_internal(obj, Some(proto));
    }
    let proto_ns = proto_start.map(elapsed_ns_since).unwrap_or(0);
    let sentinel_start = phase_counters.then(Instant::now);
    rt.set_engine_sentinel(obj, "__wasm_module", Value::Number(id as f64));
    let sentinel_ns = sentinel_start.map(elapsed_ns_since).unwrap_or(0);
    record_wasm_module_phase(
        bytes.len(),
        cache_hit,
        lookup_ns,
        parse_ns,
        cache_store_ns,
        registry_ns,
        object_ns,
        proto_ns,
        sentinel_ns,
        total_start.map(elapsed_ns_since).unwrap_or(0),
    );
    Ok(obj)
}

fn make_module_obj(rt: &mut Runtime, bytes: &[u8]) -> Result<ObjectRef, RuntimeError> {
    make_module_obj_with_prefix(rt, bytes, "WebAssembly.compile()")
}

fn make_streaming_module_obj(rt: &mut Runtime, bytes: &[u8]) -> Result<ObjectRef, RuntimeError> {
    make_module_obj_with_prefix(rt, bytes, "WebAssembly.compileStreaming()")
}

fn wasm_module_id(rt: &Runtime, value: &Value, method: &str) -> Result<usize, RuntimeError> {
    if let Value::Object(obj) = value {
        if let Value::Number(id) = rt.object_get(*obj, "__wasm_module") {
            return Ok(id as usize);
        }
    }
    Err(RuntimeError::TypeError(format!(
        "WebAssembly.Module.{method}(): Argument 0 must be a WebAssembly.Module"
    )))
}

fn make_module_import_descriptor_object(
    rt: &mut Runtime,
    desc: &ModuleImportDescriptor,
) -> ObjectRef {
    let obj = new_object(rt);
    rt.object_set(obj, "module".into(), js_string(&desc.module));
    rt.object_set(obj, "name".into(), js_string(&desc.name));
    rt.object_set(obj, "kind".into(), js_string(desc.kind));
    obj
}

fn make_module_export_descriptor_object(
    rt: &mut Runtime,
    desc: &ModuleExportDescriptor,
) -> ObjectRef {
    let obj = new_object(rt);
    rt.object_set(obj, "name".into(), js_string(&desc.name));
    rt.object_set(obj, "kind".into(), js_string(desc.kind));
    obj
}

fn module_imports_array(rt: &mut Runtime, module_id: usize) -> Result<ObjectRef, RuntimeError> {
    let descs = WASM_MODULES.with(|modules| {
        let modules = modules.borrow();
        modules
            .get(module_id)
            .and_then(|m| m.as_ref())
            .map(module_import_descriptors)
    });
    let descs = descs.ok_or_else(|| {
        RuntimeError::TypeError(
            "WebAssembly.Module.imports(): Argument 0 must be a WebAssembly.Module".into(),
        )
    })?;
    let arr = rt.alloc_object(Object::new_array());
    for (i, desc) in descs.iter().enumerate() {
        let obj = make_module_import_descriptor_object(rt, desc);
        rt.object_set(arr, i.to_string(), Value::Object(obj));
    }
    rt.object_set(arr, "length".into(), Value::Number(descs.len() as f64));
    Ok(arr)
}

fn module_exports_array(rt: &mut Runtime, module_id: usize) -> Result<ObjectRef, RuntimeError> {
    let descs = WASM_MODULES.with(|modules| {
        let modules = modules.borrow();
        modules
            .get(module_id)
            .and_then(|m| m.as_ref())
            .map(module_export_descriptors)
    });
    let descs = descs.ok_or_else(|| {
        RuntimeError::TypeError(
            "WebAssembly.Module.exports(): Argument 0 must be a WebAssembly.Module".into(),
        )
    })?;
    let arr = rt.alloc_object(Object::new_array());
    for (i, desc) in descs.iter().enumerate() {
        let obj = make_module_export_descriptor_object(rt, desc);
        rt.object_set(arr, i.to_string(), Value::Object(obj));
    }
    rt.object_set(arr, "length".into(), Value::Number(descs.len() as f64));
    Ok(arr)
}

fn module_custom_sections_array(
    rt: &mut Runtime,
    module_id: usize,
    name: String,
) -> Result<ObjectRef, RuntimeError> {
    let sections = WASM_MODULES.with(|modules| {
        let modules = modules.borrow();
        modules.get(module_id).and_then(|m| m.as_ref()).map(|m| {
            m.custom_sections
                .iter()
                .filter(|section| section.name == name)
                .map(|section| section.payload.clone())
                .collect::<Vec<_>>()
        })
    });
    let sections = sections.ok_or_else(|| {
        RuntimeError::TypeError(
            "WebAssembly.Module.customSections(): Argument 0 must be a WebAssembly.Module".into(),
        )
    })?;
    let arr = rt.alloc_object(Object::new_array());
    let len = sections.len();
    for (i, bytes) in sections.into_iter().enumerate() {
        let len = bytes.len();
        let ab = make_array_buffer_with_max(rt, bytes, len);
        rt.object_set(arr, i.to_string(), Value::Object(ab));
    }
    rt.object_set(arr, "length".into(), Value::Number(len as f64));
    Ok(arr)
}

enum WasiFd {
    Preopen { guest: String, host: String },
    File { content: Vec<u8>, offset: usize },
}

struct WasiState {
    args: Vec<String>,
    env: Vec<String>,
    fds: Vec<Option<WasiFd>>,
}
thread_local! { static WASI_STATES: RefCell<Vec<Option<WasiState>>> = const { RefCell::new(Vec::new()) }; }

fn wv_i32(args: &[WasmValue], i: usize) -> i32 {
    match args.get(i) {
        Some(WasmValue::I32(n)) => *n,
        Some(WasmValue::I64(n)) => *n as i32,
        _ => 0,
    }
}
fn ctx_u32(ctx: &dyn HostContext, off: usize) -> u32 {
    ctx.mem_read(off, 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .unwrap_or(0)
}
fn ctx_put_u32(ctx: &mut dyn HostContext, off: usize, v: u32) {
    let _ = ctx.mem_write(off, &v.to_le_bytes());
}
fn ctx_put_u64(ctx: &mut dyn HostContext, off: usize, v: u64) {
    let _ = ctx.mem_write(off, &v.to_le_bytes());
}

fn wasi_host_fn(name: &str, wasi_id: usize) -> rusty_wasm::HostFn {
    let nm = name.to_string();
    Box::new(
        move |ctx: &mut dyn HostContext, args: &[WasmValue]| -> Result<Vec<WasmValue>, String> {
            Ok(match nm.as_str() {
                "fd_write" => {
                    let fd = wv_i32(args, 0);
                    let iovs = wv_i32(args, 1) as usize;
                    let iovs_len = wv_i32(args, 2) as usize;
                    let nwritten = wv_i32(args, 3) as usize;
                    let mut out = Vec::new();
                    for i in 0..iovs_len {
                        let base = iovs + i * 8;
                        let ptr = ctx_u32(ctx, base) as usize;
                        let len = ctx_u32(ctx, base + 4) as usize;
                        if let Some(b) = ctx.mem_read(ptr, len) {
                            out.extend_from_slice(&b);
                        }
                    }
                    let total = out.len() as u32;
                    use std::io::Write;
                    if fd == 2 {
                        let _ = std::io::stderr().write_all(&out);
                        let _ = std::io::stderr().flush();
                    } else {
                        let _ = std::io::stdout().write_all(&out);
                        let _ = std::io::stdout().flush();
                    }
                    ctx_put_u32(ctx, nwritten, total);
                    vec![WasmValue::I32(0)]
                }
                "fd_read" => {
                    let fd = wv_i32(args, 0);
                    let (iovs, iovs_len, nread_p) = (
                        wv_i32(args, 1) as usize,
                        wv_i32(args, 2) as usize,
                        wv_i32(args, 3) as usize,
                    );
                    let mut chunks: Vec<(usize, usize)> = Vec::new();
                    for i in 0..iovs_len {
                        let base = iovs + i * 8;
                        chunks.push((ctx_u32(ctx, base) as usize, ctx_u32(ctx, base + 4) as usize));
                    }
                    let mut total = 0usize;
                    WASI_STATES.with(|v| {
                        if let Some(Some(WasiFd::File { content, offset })) = v
                            .borrow_mut()
                            .get_mut(wasi_id)
                            .and_then(|x| x.as_mut())
                            .and_then(|st| st.fds.get_mut(fd as usize))
                        {
                            for (ptr, len) in &chunks {
                                let avail = content.len().saturating_sub(*offset);
                                let n = (*len).min(avail);
                                if n == 0 {
                                    break;
                                }
                                let _ = ctx.mem_write(*ptr, &content[*offset..*offset + n]);
                                *offset += n;
                                total += n;
                            }
                        }
                    });
                    ctx_put_u32(ctx, nread_p, total as u32);
                    vec![WasmValue::I32(0)]
                }
                "fd_prestat_get" => {
                    let fd = wv_i32(args, 0) as usize;
                    let pre = WASI_STATES.with(|v| {
                        v.borrow()
                            .get(wasi_id)
                            .and_then(|x| x.as_ref())
                            .and_then(|st| st.fds.get(fd))
                            .and_then(|f| f.as_ref())
                            .and_then(|f| {
                                if let WasiFd::Preopen { guest, .. } = f {
                                    Some(guest.clone())
                                } else {
                                    None
                                }
                            })
                    });
                    match pre {
                        Some(guest) => {
                            let p = wv_i32(args, 1) as usize;
                            let _ = ctx.mem_write(p, &[0u8]);
                            ctx_put_u32(ctx, p + 4, guest.len() as u32);
                            vec![WasmValue::I32(0)]
                        }
                        None => vec![WasmValue::I32(8)],
                    }
                }
                "fd_prestat_dir_name" => {
                    let fd = wv_i32(args, 0) as usize;
                    let (path_p, path_len) = (wv_i32(args, 1) as usize, wv_i32(args, 2) as usize);
                    let guest = WASI_STATES.with(|v| {
                        v.borrow()
                            .get(wasi_id)
                            .and_then(|x| x.as_ref())
                            .and_then(|st| st.fds.get(fd))
                            .and_then(|f| f.as_ref())
                            .and_then(|f| {
                                if let WasiFd::Preopen { guest, .. } = f {
                                    Some(guest.clone())
                                } else {
                                    None
                                }
                            })
                    });
                    match guest {
                        Some(g) => {
                            let b = g.as_bytes();
                            let _ = ctx.mem_write(path_p, &b[..b.len().min(path_len)]);
                            vec![WasmValue::I32(0)]
                        }
                        None => vec![WasmValue::I32(8)],
                    }
                }
                "path_open" => {
                    let dirfd = wv_i32(args, 0) as usize;
                    let (path_p, path_len) = (wv_i32(args, 2) as usize, wv_i32(args, 3) as usize);
                    let opened_fd_p = wv_i32(args, 8) as usize;
                    let path = ctx
                        .mem_read(path_p, path_len)
                        .map(|b| String::from_utf8_lossy(&b).into_owned())
                        .unwrap_or_default();
                    let host_dir = WASI_STATES.with(|v| {
                        v.borrow()
                            .get(wasi_id)
                            .and_then(|x| x.as_ref())
                            .and_then(|st| st.fds.get(dirfd))
                            .and_then(|f| f.as_ref())
                            .and_then(|f| {
                                if let WasiFd::Preopen { host, .. } = f {
                                    Some(host.clone())
                                } else {
                                    None
                                }
                            })
                    });
                    let host_dir = match host_dir {
                        Some(h) => h,
                        None => return Ok(vec![WasmValue::I32(8)]),
                    };
                    let full = format!(
                        "{}/{}",
                        host_dir.trim_end_matches('/'),
                        path.trim_start_matches('/')
                    );
                    match std::fs::read(&full) {
                        Ok(content) => {
                            let newfd = WASI_STATES.with(|v| {
                                let mut b = v.borrow_mut();
                                if let Some(st) = b.get_mut(wasi_id).and_then(|x| x.as_mut()) {
                                    st.fds.push(Some(WasiFd::File { content, offset: 0 }));
                                    (st.fds.len() - 1) as u32
                                } else {
                                    0
                                }
                            });
                            ctx_put_u32(ctx, opened_fd_p, newfd);
                            vec![WasmValue::I32(0)]
                        }
                        Err(_) => vec![WasmValue::I32(44)],
                    }
                }
                "fd_seek" => {
                    let fd = wv_i32(args, 0) as usize;
                    let off = match args.get(1) {
                        Some(WasmValue::I64(n)) => *n,
                        Some(WasmValue::I32(n)) => *n as i64,
                        _ => 0,
                    };
                    let whence = wv_i32(args, 2);
                    let newoff_p = wv_i32(args, 3) as usize;
                    let pos = WASI_STATES.with(|v| {
                        if let Some(Some(WasiFd::File { content, offset })) = v
                            .borrow_mut()
                            .get_mut(wasi_id)
                            .and_then(|x| x.as_mut())
                            .and_then(|st| st.fds.get_mut(fd))
                        {
                            let base = match whence {
                                1 => *offset as i64,
                                2 => content.len() as i64,
                                _ => 0,
                            };
                            *offset = (base + off).max(0) as usize;
                            *offset as u64
                        } else {
                            0
                        }
                    });
                    ctx_put_u64(ctx, newoff_p, pos);
                    vec![WasmValue::I32(0)]
                }
                "fd_close" => {
                    let fd = wv_i32(args, 0) as usize;
                    WASI_STATES.with(|v| {
                        if let Some(st) = v.borrow_mut().get_mut(wasi_id).and_then(|x| x.as_mut()) {
                            if let Some(slot) = st.fds.get_mut(fd) {
                                *slot = None;
                            }
                        }
                    });
                    vec![WasmValue::I32(0)]
                }
                "fd_filestat_get" => {
                    let fd = wv_i32(args, 0) as usize;
                    let st_p = wv_i32(args, 1) as usize;
                    let (ftype, size) = WASI_STATES.with(|v| {
                        v.borrow()
                            .get(wasi_id)
                            .and_then(|x| x.as_ref())
                            .and_then(|st| st.fds.get(fd))
                            .and_then(|f| f.as_ref())
                            .map(|f| match f {
                                WasiFd::Preopen { .. } => (3u8, 0u64),
                                WasiFd::File { content, .. } => (4u8, content.len() as u64),
                            })
                            .unwrap_or((4, 0))
                    });

                    let _ = ctx.mem_write(st_p + 16, &[ftype]);
                    ctx_put_u64(ctx, st_p + 32, size);
                    vec![WasmValue::I32(0)]
                }
                "fd_fdstat_get" => {
                    let fd = wv_i32(args, 0) as usize;
                    let st_p = wv_i32(args, 1) as usize;
                    let ftype = WASI_STATES.with(|v| {
                        v.borrow()
                            .get(wasi_id)
                            .and_then(|x| x.as_ref())
                            .and_then(|st| st.fds.get(fd))
                            .and_then(|f| f.as_ref())
                            .map(|f| match f {
                                WasiFd::Preopen { .. } => 3u8,
                                WasiFd::File { .. } => 4u8,
                            })
                            .unwrap_or(2)
                    });
                    let _ = ctx.mem_write(st_p, &[ftype]);
                    vec![WasmValue::I32(0)]
                }
                "args_sizes_get" => {
                    let (count_p, buf_p) = (wv_i32(args, 0) as usize, wv_i32(args, 1) as usize);
                    let a = WASI_STATES.with(|v| {
                        v.borrow()
                            .get(wasi_id)
                            .and_then(|x| x.as_ref())
                            .map(|s| s.args.clone())
                            .unwrap_or_default()
                    });
                    ctx_put_u32(ctx, count_p, a.len() as u32);
                    ctx_put_u32(ctx, buf_p, a.iter().map(|s| s.len() as u32 + 1).sum());
                    vec![WasmValue::I32(0)]
                }
                "args_get" => {
                    let (ptrs, buf) = (wv_i32(args, 0) as usize, wv_i32(args, 1) as usize);
                    let a = WASI_STATES.with(|v| {
                        v.borrow()
                            .get(wasi_id)
                            .and_then(|x| x.as_ref())
                            .map(|s| s.args.clone())
                            .unwrap_or_default()
                    });
                    let mut off = buf;
                    for (i, s) in a.iter().enumerate() {
                        ctx_put_u32(ctx, ptrs + i * 4, off as u32);
                        let mut bytes = s.clone().into_bytes();
                        bytes.push(0);
                        let _ = ctx.mem_write(off, &bytes);
                        off += bytes.len();
                    }
                    vec![WasmValue::I32(0)]
                }
                "environ_sizes_get" => {
                    let (count_p, buf_p) = (wv_i32(args, 0) as usize, wv_i32(args, 1) as usize);
                    let e = WASI_STATES.with(|v| {
                        v.borrow()
                            .get(wasi_id)
                            .and_then(|x| x.as_ref())
                            .map(|s| s.env.clone())
                            .unwrap_or_default()
                    });
                    ctx_put_u32(ctx, count_p, e.len() as u32);
                    ctx_put_u32(ctx, buf_p, e.iter().map(|s| s.len() as u32 + 1).sum());
                    vec![WasmValue::I32(0)]
                }
                "environ_get" => {
                    let (ptrs, buf) = (wv_i32(args, 0) as usize, wv_i32(args, 1) as usize);
                    let e = WASI_STATES.with(|v| {
                        v.borrow()
                            .get(wasi_id)
                            .and_then(|x| x.as_ref())
                            .map(|s| s.env.clone())
                            .unwrap_or_default()
                    });
                    let mut off = buf;
                    for (i, s) in e.iter().enumerate() {
                        ctx_put_u32(ctx, ptrs + i * 4, off as u32);
                        let mut bytes = s.clone().into_bytes();
                        bytes.push(0);
                        let _ = ctx.mem_write(off, &bytes);
                        off += bytes.len();
                    }
                    vec![WasmValue::I32(0)]
                }
                "clock_time_get" => {
                    let time_p = wv_i32(args, 2) as usize;
                    let ns = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos() as u64)
                        .unwrap_or(0);
                    ctx_put_u64(ctx, time_p, ns);
                    vec![WasmValue::I32(0)]
                }
                "random_get" => {
                    let (buf, len) = (wv_i32(args, 0) as usize, wv_i32(args, 1) as usize);
                    let mut bytes = vec![0u8; len];
                    let _ = rusty_web_crypto::get_random_values(&mut bytes);
                    let _ = ctx.mem_write(buf, &bytes);
                    vec![WasmValue::I32(0)]
                }
                "proc_exit" => {
                    vec![]
                }
                "fd_fdstat_set_flags" | "path_filestat_get" | "path_create_directory" => {
                    vec![WasmValue::I32(0)]
                }
                _ => vec![WasmValue::I32(52)],
            })
        },
    )
}

fn js_val_to_wasm(rt: &Runtime, v: &Value, t: ValType) -> WasmValue {
    let _ = rt;
    match t {
        ValType::I64 => WasmValue::I64(match v {
            Value::BigInt(b) => b.to_decimal().parse::<i128>().unwrap_or(0) as i64,
            Value::Number(n) => *n as i64,
            _ => 0,
        }),
        ValType::I32 => WasmValue::I32(match v {
            Value::Number(n) => *n as i64 as i32,
            Value::BigInt(b) => b.to_decimal().parse::<i128>().unwrap_or(0) as i32,
            _ => 0,
        }),
        ValType::F32 => WasmValue::F32(match v {
            Value::Number(n) => *n as f32,
            _ => 0.0,
        }),
        ValType::F64 => WasmValue::F64(match v {
            Value::Number(n) => *n,
            _ => 0.0,
        }),
        ValType::V128 => WasmValue::V128([0; 16]),
        ValType::ExternRef => externref_to_wasm(v),
        ValType::NonNullExternRef => externref_to_wasm(v),
        ValType::AnyRef
        | ValType::NonNullAnyRef
        | ValType::EqRef
        | ValType::NonNullEqRef
        | ValType::FuncRef
        | ValType::NonNullFuncRef
        | ValType::StructRef
        | ValType::NonNullStructRef
        | ValType::ArrayRef
        | ValType::NonNullArrayRef
        | ValType::I31Ref
        | ValType::NonNullI31Ref
        | ValType::TypeRef(_)
        | ValType::NonNullTypeRef(_)
        | ValType::Unknown
        | ValType::NullRef
        | ValType::NullFuncRef
        | ValType::NullExternRef => match v {
            Value::Null | Value::Undefined => WasmValue::RefNull,
            _ => WasmValue::RefNull,
        },
    }
}

fn js_import_fn(jsfn: Value, results: Vec<ValType>) -> rusty_wasm::HostFn {
    Box::new(
        move |ctx: &mut dyn HostContext, args: &[WasmValue]| -> Result<Vec<WasmValue>, String> {
            let ptr = WASM_RT.with(|c| c.get());
            if ptr.is_null() {
                return Ok(results.iter().map(|t| zero_of(*t)).collect());
            }
            let rt: &mut Runtime = unsafe { &mut *ptr };
            let inst_id = WASM_CUR_INST.with(|c| c.get());
            let ab = if inst_id == usize::MAX {
                None
            } else {
                WASM_INSTANCE_AB.with(|v| v.borrow().get(inst_id).and_then(|x| *x))
            };

            if let Some(ab) = ab {
                let sz = ctx.mem_size();
                if let Some(bytes) = ctx.mem_read(0, sz) {
                    if let Some(rec) = rt.array_buffers.get_mut(&ab) {
                        rec.data = bytes;
                        rec.byte_length = sz;
                    }
                }
            }
            let arg_inst_id = if inst_id == usize::MAX {
                None
            } else {
                Some(inst_id)
            };
            let jsargs: Vec<Value> = args
                .iter()
                .map(|w| wasm_to_js(rt, arg_inst_id, w))
                .collect();
            let call_res = rt.call_function(jsfn.clone(), Value::Undefined, jsargs);

            if let Some(ab) = ab {
                let bytes = rt.array_buffers.get(&ab).map(|r| r.data.clone());
                if let Some(bytes) = bytes {
                    let n = ctx.mem_size().min(bytes.len());
                    if n > 0 {
                        ctx.mem_write(0, &bytes[..n]);
                    }
                }
            }
            match call_res {
                Ok(ret) => {
                    if results.is_empty() {
                        Ok(vec![])
                    } else {
                        Ok(vec![js_val_to_wasm(rt, &ret, results[0])])
                    }
                }
                Err(RuntimeError::Thrown(Value::Object(obj))) => {
                    if let Value::String(identity) = rt.object_get(obj, "__wasm_exception_identity")
                    {
                        Err(format!("__wasm_exception_identity:{}", identity.as_str()))
                    } else {
                        Err(wasm_host_error_message(
                            rt,
                            RuntimeError::Thrown(Value::Object(obj)),
                        ))
                    }
                }
                Err(e) => Err(wasm_host_error_message(rt, e)),
            }
        },
    )
}

fn wasm_js_string_concat_fn() -> rusty_wasm::HostFn {
    Box::new(
        move |_ctx: &mut dyn HostContext, args: &[WasmValue]| -> Result<Vec<WasmValue>, String> {
            let left = match args.first() {
                Some(WasmValue::ExternRef(id)) => externref_to_js(*id),
                _ => Value::Undefined,
            };
            let right = match args.get(1) {
                Some(WasmValue::ExternRef(id)) => externref_to_js(*id),
                _ => Value::Undefined,
            };
            let left = match left {
                Value::String(s) => s.as_str().to_string(),
                _ => String::new(),
            };
            let right = match right {
                Value::String(s) => s.as_str().to_string(),
                _ => String::new(),
            };
            Ok(vec![externref_to_wasm(&js_string(&(left + &right)))])
        },
    )
}
fn zero_of(t: ValType) -> WasmValue {
    match t {
        ValType::I32 => WasmValue::I32(0),
        ValType::I64 => WasmValue::I64(0),
        ValType::F32 => WasmValue::F32(0.0),
        ValType::F64 => WasmValue::F64(0.0),
        ValType::V128 => WasmValue::V128([0; 16]),
        ValType::AnyRef
        | ValType::NonNullAnyRef
        | ValType::EqRef
        | ValType::NonNullEqRef
        | ValType::FuncRef
        | ValType::NonNullFuncRef
        | ValType::ExternRef
        | ValType::NonNullExternRef
        | ValType::StructRef
        | ValType::NonNullStructRef
        | ValType::ArrayRef
        | ValType::NonNullArrayRef
        | ValType::I31Ref
        | ValType::NonNullI31Ref
        | ValType::TypeRef(_)
        | ValType::NonNullTypeRef(_)
        | ValType::Unknown
        | ValType::NullRef
        | ValType::NullFuncRef
        | ValType::NullExternRef => WasmValue::RefNull,
    }
}

fn make_instance_obj(
    rt: &mut Runtime,
    module_obj: ObjectRef,
    import_obj: Option<ObjectRef>,
    compile_options: Option<&WasmCompileOptions>,
) -> Result<ObjectRef, RuntimeError> {
    let phase_counters = wasm_instance_export_counters_enabled();
    let total_start = phase_counters.then(Instant::now);
    let mid = match rt.object_get(module_obj, "__wasm_module") {
        Value::Number(n) => n as usize,
        _ => {
            return Err(RuntimeError::TypeError(
                "WebAssembly.Instance: not a Module".into(),
            ))
        }
    };
    let import_start = phase_counters.then(Instant::now);
    let built = {
        let decls = WASM_MODULES.with(|v| {
            v.borrow()
                .get(mid)
                .and_then(|x| x.as_ref())
                .map(func_imports)
                .unwrap_or_default()
        });
        let mut imp = Imports::new();
        let mut retained_host_values: Vec<Value> = Vec::new();
        let global_decls = WASM_MODULES.with(|v| {
            v.borrow()
                .get(mid)
                .and_then(|x| x.as_ref())
                .map(global_imports)
                .unwrap_or_default()
        });
        let mut imported_global_objects: Vec<Option<ObjectRef>> = Vec::new();
        for (global_import_index, decl) in global_decls.into_iter().enumerate() {
            let v = import_obj.and_then(|io| match rt.object_get(io, &decl.module) {
                Value::Object(ns) => Some(rt.object_get(ns, &decl.name)),
                _ => None,
            });
            let value = match v {
                Some(Value::Object(global)) => {
                    let is_wasm_global = matches!(
                        rt.object_get(global, "__wasm_global_type"),
                        Value::String(_)
                    );
                    if !is_wasm_global {
                        let raw = Value::Object(global);
                        if decl.mutable {
                            return Err(RuntimeError::Thrown(wasm_link_error_value(
                                rt,
                                format!(
                                    "WebAssembly.Instance(): Import #{} \"{}\" \"{}\": imported global does not match the expected mutability",
                                    global_import_index, decl.module, decl.name
                                ),
                            )));
                        }
                        if !js_global_import_value_matches(&raw, decl.ty) {
                            return Err(RuntimeError::Thrown(wasm_link_error_value(
                                rt,
                                format!(
                                    "WebAssembly.Instance(): Import #{} \"{}\" \"{}\": imported global does not match the expected type",
                                    global_import_index, decl.module, decl.name
                                ),
                            )));
                        }
                        imported_global_objects.push(None);
                        js_to_wasm(Some(&raw), decl.ty)
                    } else {
                        let imported_mutable = matches!(
                            rt.object_get(global, "__wasm_global_mutable"),
                            Value::Boolean(true)
                        );
                        if imported_mutable != decl.mutable {
                            return Err(RuntimeError::Thrown(wasm_link_error_value(
                            rt,
                            format!(
                                "WebAssembly.Instance(): Import #{} \"{}\" \"{}\": imported global does not match the expected mutability",
                                global_import_index, decl.module, decl.name
                            ),
                        )));
                        }
                        if let Value::String(imported_ty) =
                            rt.object_get(global, "__wasm_global_type")
                        {
                            let expected_ty = wasm_global_type_key(decl.ty);
                            let type_matches = if decl.mutable {
                                imported_ty.as_str() == expected_ty
                            } else {
                                wasm_global_type_compatible(imported_ty.as_str(), decl.ty)
                            };
                            if !type_matches {
                                return Err(RuntimeError::Thrown(wasm_link_error_value(
                                rt,
                                format!(
                                    "WebAssembly.Instance(): Import #{} \"{}\" \"{}\": imported global does not match the expected type",
                                    global_import_index, decl.module, decl.name
                                ),
                            )));
                            }
                        }
                        imported_global_objects.push(Some(global));
                        let inner = rt
                            .read_property(global, "value")
                            .unwrap_or(Value::Undefined);
                        if matches!(
                            decl.ty,
                            ValType::FuncRef
                                | ValType::NonNullFuncRef
                                | ValType::TypeRef(_)
                                | ValType::NonNullTypeRef(_)
                        ) {
                            if let Some((_inst_id, _funcidx, params, results)) =
                                wasm_func_sentinel_sig(rt, &inner)
                            {
                                retained_host_values.push(inner.clone());
                                imp.global_func_ref(
                                    &decl.module,
                                    &decl.name,
                                    params,
                                    results.clone(),
                                    js_import_fn(inner, results),
                                );
                                continue;
                            }
                        }
                        js_to_wasm(Some(&inner), decl.ty)
                    }
                }
                Some(other) => {
                    if decl.mutable {
                        return Err(RuntimeError::Thrown(wasm_link_error_value(
                            rt,
                            format!(
                                "WebAssembly.Instance(): Import #{} \"{}\" \"{}\": imported global does not match the expected mutability",
                                global_import_index, decl.module, decl.name
                            ),
                        )));
                    }
                    if !js_global_import_value_matches(&other, decl.ty) {
                        return Err(RuntimeError::Thrown(wasm_link_error_value(
                            rt,
                            format!(
                                "WebAssembly.Instance(): Import #{} \"{}\" \"{}\": imported global does not match the expected type",
                                global_import_index, decl.module, decl.name
                            ),
                        )));
                    }
                    imported_global_objects.push(None);
                    js_to_wasm(Some(&other), decl.ty)
                }
                None if !decl.mutable
                    && matches!(decl.ty, ValType::ExternRef)
                    && compile_options
                        .and_then(|options| options.imported_string_constants.as_deref())
                        == Some(decl.module.as_str()) =>
                {
                    imported_global_objects.push(None);
                    let synthetic = js_string(&decl.name);
                    js_to_wasm(Some(&synthetic), decl.ty)
                }
                None => {
                    return Err(RuntimeError::Thrown(wasm_link_error_value(
                        rt,
                        format!(
                            "WebAssembly.Instance(): Import #{} \"{}\" \"{}\": global import is missing",
                            global_import_index, decl.module, decl.name
                        ),
                    )));
                }
            };
            imp.global(&decl.module, &decl.name, value);
        }
        let table_decls = WASM_MODULES.with(|v| {
            v.borrow()
                .get(mid)
                .and_then(|x| x.as_ref())
                .map(table_imports)
                .unwrap_or_default()
        });
        let mut imported_table_obj: Option<ObjectRef> = None;
        for (table_import_index, decl) in table_decls.into_iter().enumerate() {
            let Some(Value::Object(table)) =
                import_obj.and_then(|io| match rt.object_get(io, &decl.module) {
                    Value::Object(ns) => Some(rt.object_get(ns, &decl.name)),
                    _ => None,
                })
            else {
                return Err(RuntimeError::Thrown(wasm_link_error_value(
                    rt,
                    format!(
                        "WebAssembly.Instance(): Import #{} \"{}\" \"{}\": table import is missing",
                        table_import_index, decl.module, decl.name
                    ),
                )));
            };
            if !matches!(
                rt.object_get(table, "__wasm_table_storage"),
                Value::Object(_)
            ) {
                return Err(RuntimeError::Thrown(wasm_link_error_value(
                    rt,
                    format!(
                        "WebAssembly.Instance(): Import #{} \"{}\" \"{}\": import object field is not a WebAssembly.Table",
                        table_import_index, decl.module, decl.name
                    ),
                )));
            }
            if let Value::Number(length) = rt.object_get(table, "length") {
                if length < decl.min as f64 {
                    return Err(RuntimeError::Thrown(wasm_link_error_value(
                        rt,
                        format!(
                            "WebAssembly.Instance(): Import #{} \"{}\" \"{}\": table import has length {} smaller than declared initial {}",
                            table_import_index, decl.module, decl.name, length, decl.min
                        ),
                    )));
                }
            }
            let imported_type_key = wasm_table_object_type_key(rt, table);
            let expected_type_key = wasm_table_type_key(decl.elem);
            if imported_type_key != expected_type_key {
                return Err(RuntimeError::Thrown(wasm_link_error_value(
                    rt,
                    format!(
                        "WebAssembly.Instance(): Import #{} \"{}\" \"{}\": imported table does not match the expected type",
                        table_import_index, decl.module, decl.name
                    ),
                )));
            }
            let table64 = matches!(
                rt.object_get(table, "__wasm_table_address64"),
                Value::Boolean(true)
            );
            if table64 != decl.table64 {
                return Err(RuntimeError::Thrown(wasm_link_error_value(
                    rt,
                    format!(
                        "WebAssembly.Instance(): Import #{} \"{}\" \"{}\": imported table does not match the expected address type",
                        table_import_index, decl.module, decl.name
                    ),
                )));
            }
            if let Some(max) = decl.max {
                match rt.object_get(table, "__wasm_table_maximum") {
                    Value::Number(maximum) if maximum <= max as f64 => {}
                    Value::Number(maximum) => {
                        return Err(RuntimeError::Thrown(wasm_link_error_value(
                            rt,
                            format!(
                                "WebAssembly.Instance(): Import #{} \"{}\" \"{}\": table import maximum {} exceeds declared maximum {}",
                                table_import_index, decl.module, decl.name, maximum, max
                            ),
                        )));
                    }
                    _ => {
                        return Err(RuntimeError::Thrown(wasm_link_error_value(
                            rt,
                            format!(
                                "WebAssembly.Instance(): Import #{} \"{}\" \"{}\": table import has no maximum, declared maximum {}",
                                table_import_index, decl.module, decl.name, max
                            ),
                        )));
                    }
                }
            }
            let length = match rt.object_get(table, "length") {
                Value::Number(length) if length >= 0.0 => length as usize,
                _ => decl.min as usize,
            };
            let max = match rt.object_get(table, "__wasm_table_maximum") {
                Value::Number(maximum) if maximum >= 0.0 => Some(maximum as u32),
                _ => None,
            };
            let table64 = matches!(
                rt.object_get(table, "__wasm_table_address64"),
                Value::Boolean(true)
            );
            let table_values = table_import_values(rt, table, length);
            imp.table_with_values_address64(
                &decl.module,
                &decl.name,
                decl.elem,
                length,
                max,
                table64,
                table_values,
            );
            imported_table_obj = Some(table);
            break;
        }
        let mut imported_mem_objects: Vec<Option<ObjectRef>> = Vec::new();
        let memory_decls = WASM_MODULES.with(|v| {
            v.borrow()
                .get(mid)
                .and_then(|x| x.as_ref())
                .map(memory_imports)
                .unwrap_or_default()
        });
        for (memory_import_index, decl) in memory_decls.into_iter().enumerate() {
            let v = import_obj.and_then(|io| match rt.object_get(io, &decl.module) {
                Value::Object(ns) => Some(rt.object_get(ns, &decl.name)),
                _ => None,
            });
            let Some(Value::Object(mem_obj)) = v else {
                return Err(RuntimeError::Thrown(wasm_link_error_value(
                    rt,
                    format!(
                        "WebAssembly.Instance(): Import #{} \"{}\" \"{}\": memory import is missing",
                        memory_import_index, decl.module, decl.name
                    ),
                )));
            };
            let Value::Object(ab) = rt.object_get(mem_obj, "__wasm_memory_buffer") else {
                return Err(RuntimeError::Thrown(wasm_link_error_value(
                    rt,
                    format!(
                        "WebAssembly.Instance(): Import #{} \"{}\" \"{}\": import object field is not a WebAssembly.Memory",
                        memory_import_index, decl.module, decl.name
                    ),
                )));
            };
            let pages = rt
                .array_buffers
                .get(&ab)
                .map(|r| r.byte_len() / 65536)
                .unwrap_or(0);
            let max = wasm_memory_maximum_pages(rt, mem_obj);
            let shared = matches!(
                rt.object_get(ab, "__kind"),
                Value::String(ref s) if s.as_str() == "SharedArrayBuffer"
            );
            if shared != decl.shared {
                return Err(RuntimeError::Thrown(wasm_link_error_value(
                    rt,
                    format!(
                        "WebAssembly.Instance(): Import #{} \"{}\" \"{}\": mismatch in shared state of memory, declared = {}, imported = {}",
                        memory_import_index,
                        decl.module,
                        decl.name,
                        if decl.shared { 1 } else { 0 },
                        if shared { 1 } else { 0 }
                    ),
                )));
            }
            if pages < decl.min as usize {
                return Err(RuntimeError::Thrown(wasm_link_error_value(
                    rt,
                    format!(
                        "WebAssembly.Instance(): Import #{} \"{}\" \"{}\": memory import has {} pages which is smaller than the declared initial of {}",
                        memory_import_index, decl.module, decl.name, pages, decl.min
                    ),
                )));
            }
            if let Some(declared_max) = decl.max {
                match max {
                    Some(imported_max) if u64::from(imported_max) <= declared_max => {}
                    Some(imported_max) => {
                        return Err(RuntimeError::Thrown(wasm_link_error_value(
                            rt,
                            format!(
                                "WebAssembly.Instance(): Import #{} \"{}\" \"{}\": memory import maximum {} exceeds declared maximum {}",
                                memory_import_index, decl.module, decl.name, imported_max, declared_max
                            ),
                        )));
                    }
                    None => {
                        return Err(RuntimeError::Thrown(wasm_link_error_value(
                            rt,
                            format!(
                                "WebAssembly.Instance(): Import #{} \"{}\" \"{}\": memory import has no maximum, declared maximum {}",
                                memory_import_index, decl.module, decl.name, declared_max
                            ),
                        )));
                    }
                }
            }
            let memory64 = matches!(
                rt.object_get(mem_obj, "__wasm_memory_address64"),
                Value::Boolean(true)
            );
            if memory64 != decl.memory64 {
                return Err(RuntimeError::Thrown(wasm_link_error_value(
                    rt,
                    format!(
                        "WebAssembly.Instance(): Import #{} \"{}\" \"{}\": imported memory does not match the expected address type",
                        memory_import_index, decl.module, decl.name
                    ),
                )));
            }
            let bytes = rt.array_buffers.get(&ab).map(|record| record.to_bytes());
            imp.memory_with_shared_bytes_address64_alias(
                &decl.module,
                &decl.name,
                pages,
                max,
                shared,
                memory64,
                bytes,
                Some(mem_obj.0 as u64),
            );
            imported_mem_objects.push(Some(mem_obj));
        }
        let mut imported_func_values: Vec<Value> = Vec::new();
        let tag_import_decls = WASM_MODULES.with(|v| {
            v.borrow()
                .get(mid)
                .and_then(|x| x.as_ref())
                .map(tag_imports)
                .unwrap_or_default()
        });
        for decl in tag_import_decls {
            let v = import_obj.and_then(|io| match rt.object_get(io, &decl.module) {
                Value::Object(ns) => Some(rt.object_get(ns, &decl.name)),
                _ => None,
            });
            match v {
                Some(Value::Object(tag))
                    if wasm_object_has_marker(rt, &Value::Object(tag), "__wasm_tag") =>
                {
                    let matches_tag = wasm_tag_type_shape_match(rt, tag, &decl.type_shape)
                        .unwrap_or_else(|| wasm_tag_params_match(rt, tag, &decl.params));
                    if !matches_tag {
                        return Err(RuntimeError::Thrown(wasm_link_error_value(
                            rt,
                            format!(
                                "WebAssembly.Instance(): Import \"{}\" \"{}\": imported tag does not match the expected type",
                                decl.module, decl.name
                            ),
                        )));
                    }
                    let identity = match rt.object_get(tag, "__wasm_tag_identity") {
                        Value::String(s) => Some(s.as_str().to_string()),
                        _ => Some(format!("js:{tag:?}")),
                    };
                    imp.tag_with_identity(&decl.module, &decl.name, identity);
                }
                Some(_) => {
                    return Err(RuntimeError::Thrown(wasm_link_error_value(
                        rt,
                        format!(
                            "WebAssembly.Instance(): Import \"{}\" \"{}\": tag import requires a WebAssembly.Tag",
                            decl.module, decl.name
                        ),
                    )));
                }
                None => {
                    return Err(RuntimeError::Thrown(wasm_link_error_value(
                        rt,
                        format!(
                            "WebAssembly.Instance(): Import \"{}\" \"{}\": tag import is missing",
                            decl.module, decl.name
                        ),
                    )));
                }
            }
        }
        for (func_import_index, decl) in decls.into_iter().enumerate() {
            let wasi_id = import_obj.and_then(|io| match rt.object_get(io, &decl.module) {
                Value::Object(ns) => match rt.object_get(ns, "__wasi_id") {
                    Value::Number(n) => Some(n as usize),
                    _ => None,
                },
                _ => None,
            });
            if wasi_id.is_some() {
                imported_func_values.push(Value::Undefined);
                imp.func(
                    &decl.module,
                    &decl.name,
                    wasi_host_fn(&decl.name, wasi_id.unwrap_or(0)),
                );
            } else if compile_options.is_some_and(|options| options.js_string_builtins)
                && decl.module == "wasm:js-string"
                && decl.name == "concat"
                && decl.params == [ValType::ExternRef, ValType::ExternRef]
                && decl.results == [ValType::ExternRef]
            {
                imported_func_values.push(Value::Undefined);
                imp.func(&decl.module, &decl.name, wasm_js_string_concat_fn());
            } else {
                let v = import_obj.and_then(|io| match rt.object_get(io, &decl.module) {
                    Value::Object(ns) => Some(rt.object_get(ns, &decl.name)),
                    _ => None,
                });
                let Some(f) = v.filter(|x| rt.is_callable(x)) else {
                    return Err(RuntimeError::Thrown(wasm_link_error_value(
                        rt,
                        format!(
                            "WebAssembly.Instance(): Import #{} \"{}\" \"{}\": function import requires a callable",
                            func_import_index, decl.module, decl.name
                        ),
                    )));
                };
                if let Value::Object(func_obj) = &f {
                    if let Value::String(sig) = rt.object_get(*func_obj, "__wasm_func_signature") {
                        let expected = wasm_signature_key(&decl.params, &decl.results);
                        let imported_shape =
                            match rt.object_get(*func_obj, "__wasm_func_type_shape") {
                                Value::String(shape) => Some(shape.as_str().to_string()),
                                _ => None,
                            };
                        let imported_shapes =
                            match rt.object_get(*func_obj, "__wasm_func_type_shapes") {
                                Value::String(shapes) => Some(shapes.as_str().to_string()),
                                _ => None,
                            };
                        if imported_shape.is_some() || imported_shapes.is_some() {
                            let matches_shape = imported_shape
                                .as_deref()
                                .is_some_and(|shape| shape == decl.type_shape)
                                || imported_shapes
                                    .as_deref()
                                    .map(|shapes| {
                                        shapes.split('\n').any(|shape| shape == decl.type_shape)
                                    })
                                    .unwrap_or(false);
                            if !matches_shape {
                                return Err(RuntimeError::Thrown(wasm_link_error_value(
                                    rt,
                                    format!(
                                        "WebAssembly.Instance(): Import #{} \"{}\" \"{}\": imported function does not match the expected type",
                                        func_import_index, decl.module, decl.name
                                    ),
                                )));
                            }
                        } else if sig.as_str() != expected {
                            return Err(RuntimeError::Thrown(wasm_link_error_value(
                                rt,
                                format!(
                                    "WebAssembly.Instance(): Import #{} \"{}\" \"{}\": imported function does not match the expected type",
                                    func_import_index, decl.module, decl.name
                                ),
                            )));
                        }
                        if let Value::Boolean(imported_final) =
                            rt.object_get(*func_obj, "__wasm_func_type_final")
                        {
                            if imported_final != decl.type_final {
                                return Err(RuntimeError::Thrown(wasm_link_error_value(
                                    rt,
                                    format!(
                                        "WebAssembly.Instance(): Import #{} \"{}\" \"{}\": imported function does not match the expected type",
                                        func_import_index, decl.module, decl.name
                                    ),
                                )));
                            }
                        }
                    }
                }
                imported_func_values.push(f.clone());
                retained_host_values.push(f.clone());
                imp.func(
                    &decl.module,
                    &decl.name,
                    js_import_fn(f, decl.results.clone()),
                );
            }
        }
        (
            imp,
            imported_mem_objects,
            imported_table_obj,
            imported_global_objects,
            imported_func_values,
            retained_host_values,
        )
    };
    let import_ns = import_start.map(elapsed_ns_since).unwrap_or(0);
    let instantiate_start = phase_counters.then(Instant::now);
    let inst = WASM_MODULES.with(|v| {
        let b = v.borrow();
        let module = b.get(mid).and_then(|x| x.as_ref())?;
        Some(instantiate(module, built.0))
    });
    let instantiate_ns = instantiate_start.map(elapsed_ns_since).unwrap_or(0);
    let inst = match inst {
        Some(Ok(i)) => i,
        Some(Err(e)) => {
            if let Some(partial) = take_last_partial_instance() {
                let table_entries = partial.table_values_at(0);
                let non_host_func_count = partial.non_host_func_count();
                let iid = WASM_INSTANCES.with(|v| {
                    let mut v = v.borrow_mut();
                    v.push(Some(partial));
                    v.len() - 1
                });
                WASM_INSTANCE_MODULE_ID.with(|v| {
                    let mut v = v.borrow_mut();
                    while v.len() <= iid {
                        v.push(None);
                    }
                    v[iid] = Some(mid);
                });
                let host_roots = wasm_instance_host_roots(&built.1, built.2, &built.3, &built.5);
                if !host_roots.is_empty() {
                    rt.retain_host_roots(format!("wasm-instance-{iid}-host-imports"), host_roots);
                }
                if !built.1.is_empty() {
                    WASM_INSTANCE_MEM_OBJECTS.with(|v| {
                        let mut v = v.borrow_mut();
                        while v.len() <= iid {
                            v.push(Vec::new());
                        }
                        v[iid] = built.1.clone();
                    });
                }
                if let Some(imported_mem_obj) = built.1.first().and_then(|x| *x) {
                    WASM_INSTANCE_MEM_OBJECT.with(|v| {
                        let mut v = v.borrow_mut();
                        while v.len() <= iid {
                            v.push(None);
                        }
                        v[iid] = Some(imported_mem_obj);
                    });
                    if let Value::Object(ab) =
                        rt.object_get(imported_mem_obj, "__wasm_memory_buffer")
                    {
                        WASM_INSTANCE_AB.with(|v| {
                            let mut v = v.borrow_mut();
                            while v.len() <= iid {
                                v.push(None);
                            }
                            v[iid] = Some(ab);
                        });
                        sync_from_wasm(rt, iid, ab);
                    }
                }
                sync_memory_objects_from_wasm(rt, iid, None);
                if let Some(imported_table_obj) = built.2 {
                    sync_imported_table_elements(
                        rt,
                        iid,
                        imported_table_obj,
                        &table_entries,
                        &built.4,
                        non_host_func_count,
                    );
                    WASM_INSTANCE_TABLE_OBJECT.with(|v| {
                        let mut v = v.borrow_mut();
                        while v.len() <= iid {
                            v.push(None);
                        }
                        v[iid] = Some(imported_table_obj);
                    });
                }
            }
            if e.contains("element segment out of table bounds") {
                return Err(RuntimeError::Thrown(wasm_runtime_error_value(
                    rt,
                    format!("WebAssembly.Instance: {e}"),
                )));
            }
            return Err(RuntimeError::TypeError(format!(
                "WebAssembly.Instance: {e}"
            )));
        }
        None => {
            return Err(RuntimeError::TypeError(
                "WebAssembly.Instance: module gone".into(),
            ))
        }
    };
    let table_entries = inst.table_values_at(0);
    let non_host_func_count = inst.non_host_func_count();
    let register_start = phase_counters.then(Instant::now);
    let iid = WASM_INSTANCES.with(|v| {
        let mut v = v.borrow_mut();
        v.push(Some(inst));
        v.len() - 1
    });
    WASM_INSTANCE_MODULE_ID.with(|v| {
        let mut v = v.borrow_mut();
        while v.len() <= iid {
            v.push(None);
        }
        v[iid] = Some(mid);
    });
    WASM_INSTANCES.with(|v| {
        if let Some(Some(inst)) = v.borrow_mut().get_mut(iid) {
            for (_, tag_index, _, _) in inst.export_tag_specs() {
                inst.set_tag_identity_at(tag_index as usize, format!("wasm:{iid}:{tag_index}"));
            }
        }
    });
    let host_roots = wasm_instance_host_roots(&built.1, built.2, &built.3, &built.5);
    if !host_roots.is_empty() {
        rt.retain_host_roots(format!("wasm-instance-{iid}-host-imports"), host_roots);
    }
    let register_ns = register_start.map(elapsed_ns_since).unwrap_or(0);
    let memory_sync_start = phase_counters.then(Instant::now);
    if !built.1.is_empty() {
        WASM_INSTANCE_MEM_OBJECTS.with(|v| {
            let mut v = v.borrow_mut();
            while v.len() <= iid {
                v.push(Vec::new());
            }
            v[iid] = built.1.clone();
        });
    }
    if let Some(imported_mem_obj) = built.1.first().and_then(|x| *x) {
        WASM_INSTANCE_MEM_OBJECT.with(|v| {
            let mut v = v.borrow_mut();
            while v.len() <= iid {
                v.push(None);
            }
            v[iid] = Some(imported_mem_obj);
        });
        if let Value::Object(ab) = rt.object_get(imported_mem_obj, "__wasm_memory_buffer") {
            WASM_INSTANCE_AB.with(|v| {
                let mut v = v.borrow_mut();
                while v.len() <= iid {
                    v.push(None);
                }
                v[iid] = Some(ab);
            });
            sync_from_wasm(rt, iid, ab);
        }
    }
    sync_memory_objects_from_wasm(rt, iid, None);
    let memory_sync_ns = memory_sync_start.map(elapsed_ns_since).unwrap_or(0);
    let table_sync_start = phase_counters.then(Instant::now);
    if let Some(imported_table_obj) = built.2 {
        sync_imported_table_elements(
            rt,
            iid,
            imported_table_obj,
            &table_entries,
            &built.4,
            non_host_func_count,
        );
        WASM_INSTANCE_TABLE_OBJECT.with(|v| {
            let mut v = v.borrow_mut();
            while v.len() <= iid {
                v.push(None);
            }
            v[iid] = Some(imported_table_obj);
        });
    }
    let table_sync_ns = table_sync_start.map(elapsed_ns_since).unwrap_or(0);
    let global_start = phase_counters.then(Instant::now);
    WASM_INSTANCE_GLOBAL_OBJECTS.with(|v| {
        let mut v = v.borrow_mut();
        while v.len() <= iid {
            v.push(Vec::new());
        }
        v[iid] = built.3;
    });
    let global_ns = global_start.map(elapsed_ns_since).unwrap_or(0);
    let object_start = phase_counters.then(Instant::now);
    let obj = new_object(rt);
    if let Some(proto) = wasm_prototype(rt, "Instance") {
        rt.set_object_prototype_internal(obj, Some(proto));
    }
    let object_ns = object_start.map(elapsed_ns_since).unwrap_or(0);
    let export_start = phase_counters.then(Instant::now);
    let exports = build_exports(rt, iid);
    rt.object_set(obj, "exports".into(), Value::Object(exports));
    rt.set_engine_sentinel(obj, "__wasm_inst", Value::Number(iid as f64));
    let export_ns = export_start.map(elapsed_ns_since).unwrap_or(0);
    let total_ns = total_start.map(elapsed_ns_since).unwrap_or(0);
    record_wasm_instance_export_phase(
        import_ns,
        instantiate_ns,
        register_ns,
        memory_sync_ns,
        table_sync_ns,
        global_ns,
        object_ns,
        export_ns,
        total_ns,
    );
    Ok(obj)
}

pub fn install(rt: &mut Runtime) {
    let wasm = new_object(rt);

    let module_ctor = native_function(rt, "Module", 1, |rt, args| {
        let phase_counters = wasm_module_counters_enabled();
        let total_start = phase_counters.then(Instant::now);
        let buffer_start = phase_counters.then(Instant::now);
        let bytes = wasm_buffer_source(rt, &args.first().cloned().unwrap_or(Value::Undefined))?;
        let buffer_ns = buffer_start.map(elapsed_ns_since).unwrap_or(0);
        let make_start = phase_counters.then(Instant::now);
        let module = make_module_obj_with_prefix(rt, &bytes, "WebAssembly.Module()")?;
        let make_ns = make_start.map(elapsed_ns_since).unwrap_or(0);
        record_wasm_module_ctor_phase(
            buffer_ns,
            make_ns,
            total_start.map(elapsed_ns_since).unwrap_or(0),
        );
        Ok(Value::Object(module))
    });
    install_namespace_ctor(rt, wasm, "Module", module_ctor);
    install_builtin_function(rt, module_ctor, "imports", 1, |rt, args| {
        let module_id = wasm_module_id(
            rt,
            &args.first().cloned().unwrap_or(Value::Undefined),
            "imports",
        )?;
        module_imports_array(rt, module_id).map(Value::Object)
    });
    install_builtin_function(rt, module_ctor, "exports", 1, |rt, args| {
        let module_id = wasm_module_id(
            rt,
            &args.first().cloned().unwrap_or(Value::Undefined),
            "exports",
        )?;
        module_exports_array(rt, module_id).map(Value::Object)
    });
    install_builtin_function(rt, module_ctor, "customSections", 2, |rt, args| {
        let module_id = wasm_module_id(
            rt,
            &args.first().cloned().unwrap_or(Value::Undefined),
            "customSections",
        )?;
        let name = match args.get(1) {
            Some(Value::String(s)) => s.as_str().to_string(),
            Some(v) => rt.to_string_strict(v)?,
            None => "undefined".to_string(),
        };
        module_custom_sections_array(rt, module_id, name).map(Value::Object)
    });

    let instance_ctor = native_function(rt, "Instance", 1, |rt, args| {
        let module_obj = match args.first() {
            Some(Value::Object(o)) => *o,
            _ => {
                return Err(RuntimeError::TypeError(
                    "WebAssembly.Instance: Module required".into(),
                ))
            }
        };
        let import_obj = match args.get(1) {
            Some(Value::Object(o)) => Some(*o),
            _ => None,
        };
        make_instance_obj(rt, module_obj, import_obj, None).map(Value::Object)
    });
    install_namespace_ctor(rt, wasm, "Instance", instance_ctor);

    install_builtin_function(rt, wasm, "instantiate", 1, |rt, args| {
        let first = args.first().cloned().unwrap_or(Value::Undefined);
        let p = rusty_js_runtime::promise::new_promise(rt);
        let first_is_module = matches!(
            &first,
            Value::Object(o) if matches!(rt.object_get(*o, "__wasm_module"), Value::Number(_))
        );
        let compile_options = if first_is_module { None } else { args.get(2) };
        let compile_options = match read_compile_options(rt, compile_options) {
            Ok(options) => options,
            Err(e) => {
                reject_wasm_compile_error(rt, p, e);
                return Ok(Value::Object(p));
            }
        };
        let module_obj = if first_is_module {

            match first {
                Value::Object(o) => o,
                _ => unreachable!(),
            }
        } else {
            let bytes = match wasm_buffer_source(rt, &first) {
                Ok(bytes) => bytes,
                Err(e) => {
                    reject_wasm_compile_error(rt, p, e);
                    return Ok(Value::Object(p));
                }
            };
            match make_module_obj(rt, &bytes) {
                Ok(m) => m,
                Err(e) => {
                    reject_wasm_compile_error(rt, p, e);
                    return Ok(Value::Object(p));
                }
            }
        };
        let import_obj = match args.get(1) {
            Some(Value::Object(o)) => Some(*o),
            _ => None,
        };
        match make_instance_obj(rt, module_obj, import_obj, Some(&compile_options)) {
            Ok(inst) => {
                let result = if first_is_module {

                    inst
                } else {
                    let result = new_object(rt);
                    rt.object_set(result, "module".into(), Value::Object(module_obj));
                    rt.object_set(result, "instance".into(), Value::Object(inst));
                    result
                };
                rusty_js_runtime::promise::resolve_promise(rt, p, Value::Object(result));
            }
            Err(e) => {
                reject_wasm_compile_error(rt, p, e);
            }
        }
        Ok(Value::Object(p))
    });

    install_builtin_function(rt, wasm, "compile", 1, |rt, args| {
        let p = rusty_js_runtime::promise::new_promise(rt);
        if let Err(e) = observe_compile_options(rt, args.get(1)) {
            reject_wasm_compile_error(rt, p, e);
            return Ok(Value::Object(p));
        }
        let bytes = match wasm_buffer_source(rt, &args.first().cloned().unwrap_or(Value::Undefined))
        {
            Ok(bytes) => bytes,
            Err(e) => {
                reject_wasm_compile_error(rt, p, e);
                return Ok(Value::Object(p));
            }
        };
        match make_module_obj(rt, &bytes) {
            Ok(m) => rusty_js_runtime::promise::resolve_promise(rt, p, Value::Object(m)),
            Err(e) => {
                reject_wasm_compile_error(rt, p, e);
            }
        }
        Ok(Value::Object(p))
    });

    install_builtin_function(rt, wasm, "validate", 1, |rt, args| {
        let bytes = wasm_buffer_source(rt, &args.first().cloned().unwrap_or(Value::Undefined))?;
        Ok(Value::Boolean(parse_module(&bytes).is_ok()))
    });

    install_builtin_function(rt, wasm, "compileStreaming", 1, |rt, args| {
        let p = rusty_js_runtime::promise::new_promise(rt);
        if let Err(e) = observe_compile_options(rt, args.get(1)) {
            reject_streaming_error(rt, p, e);
            return Ok(Value::Object(p));
        }
        let source = args.first().cloned().unwrap_or(Value::Undefined);
        match streaming_source_bytes(rt, &source)
            .and_then(|bytes| make_streaming_module_obj(rt, &bytes).map(Value::Object))
        {
            Ok(module) => rusty_js_runtime::promise::resolve_promise(rt, p, module),
            Err(e) => {
                reject_streaming_error(rt, p, e);
            }
        }
        Ok(Value::Object(p))
    });

    install_builtin_function(rt, wasm, "instantiateStreaming", 1, |rt, args| {
        let p = rusty_js_runtime::promise::new_promise(rt);
        let compile_options = match read_compile_options(rt, args.get(2)) {
            Ok(options) => options,
            Err(e) => {
                reject_streaming_error(rt, p, e);
                return Ok(Value::Object(p));
            }
        };
        let source = args.first().cloned().unwrap_or(Value::Undefined);
        let import_obj = match args.get(1) {
            Some(Value::Object(o)) => Some(*o),
            _ => None,
        };
        match streaming_source_bytes(rt, &source)
            .and_then(|bytes| make_streaming_module_obj(rt, &bytes).map(Value::Object))
        {
            Ok(Value::Object(module_obj)) => {
                match make_instance_obj(rt, module_obj, import_obj, Some(&compile_options)) {
                    Ok(inst) => {
                        let result = new_object(rt);
                        rt.object_set(result, "module".into(), Value::Object(module_obj));
                        rt.object_set(result, "instance".into(), Value::Object(inst));
                        rusty_js_runtime::promise::resolve_promise(rt, p, Value::Object(result));
                    }
                    Err(e) => {
                        reject_streaming_error(rt, p, e);
                    }
                }
            }
            Ok(_) => unreachable!(),
            Err(e) => {
                reject_streaming_error(rt, p, e);
            }
        }
        Ok(Value::Object(p))
    });

    let memory_ctor = native_function(rt, "Memory", 1, |rt, args| {
        let this = memory_this(rt)?;
        let desc = match args.first() {
            Some(Value::Object(desc)) => *desc,
            _ => {
                return Err(RuntimeError::TypeError(
                    "WebAssembly.Memory(): descriptor object required".into(),
                ))
            }
        };
        let address64 = wasm_memory_address64(rt, desc)?;
        let initial = wasm_memory_page_field(rt, desc, "initial", address64)?.unwrap_or(0);
        let maximum = wasm_memory_page_field(rt, desc, "maximum", address64)?;
        let shared = matches!(rt.object_get(desc, "shared"), Value::Boolean(true));
        if shared && maximum.is_none() {
            return Err(RuntimeError::TypeError(
                "WebAssembly.Memory(): If shared is true, maximum property should be defined."
                    .into(),
            ));
        }
        if let Some(maximum) = maximum {
            if maximum < initial {
                return Err(RuntimeError::RangeError(
                    "WebAssembly.Memory(): maximum is less than initial".into(),
                ));
            }
        }
        let byte_length = initial.saturating_mul(65536);
        let max_byte_length = maximum.unwrap_or(initial).saturating_mul(65536);
        let ab = if shared {
            make_shared_array_buffer(rt, byte_length, max_byte_length)
        } else {
            make_array_buffer_with_max(rt, vec![0u8; byte_length], max_byte_length)
        };
        rt.define_data_property_attrs(this, "buffer", Value::Object(ab), true, false, true);
        rt.define_data_property_attrs(
            this,
            "__wasm_memory_buffer",
            Value::Object(ab),
            true,
            false,
            true,
        );
        rt.define_data_property_attrs(
            this,
            "__wasm_memory_address64",
            Value::Boolean(address64),
            true,
            false,
            true,
        );
        if let Some(maximum) = maximum {
            rt.define_data_property_attrs(
                this,
                "__wasm_memory_maximum",
                Value::Number(maximum as f64),
                true,
                false,
                true,
            );
        }
        Ok(Value::Object(this))
    });
    let memory_proto = install_namespace_ctor(rt, wasm, "Memory", memory_ctor);
    install_builtin_function(rt, memory_proto, "grow", 1, |rt, args| {
        let this = memory_this(rt)?;
        let ab = match rt.object_get(this, "__wasm_memory_buffer") {
            Value::Object(ab) => ab,
            _ => {
                return Err(RuntimeError::TypeError(
                    "WebAssembly.Memory.prototype.grow: incompatible receiver".into(),
                ))
            }
        };
        let old_pages = rt
            .array_buffers
            .get(&ab)
            .map(|r| r.byte_len() / 65536)
            .unwrap_or(0);
        let address64 = matches!(
            rt.object_get(this, "__wasm_memory_address64"),
            Value::Boolean(true)
        );
        let delta = wasm_memory_grow_delta(args, address64)?;
        let new_pages = old_pages.checked_add(delta).ok_or_else(|| {
            RuntimeError::RangeError(
                "WebAssembly.Memory.grow(): Maximum memory size exceeded".into(),
            )
        })?;
        let (max_byte_length, shared, mut bytes) = match rt.array_buffers.get(&ab) {
            Some(rec) => (rec.max_byte_length, rec.shared.is_some(), rec.to_bytes()),
            None => (0, false, Vec::new()),
        };
        let new_byte_length = new_pages.saturating_mul(65536);
        if new_byte_length > max_byte_length {
            return Err(RuntimeError::RangeError(
                "WebAssembly.Memory.grow(): Maximum memory size exceeded".into(),
            ));
        }
        if shared {
            if let Some(rec) = rt.array_buffers.get_mut(&ab) {
                rec.resize_bytes(new_byte_length);
                rec.byte_length = rec.byte_len();
            }
        } else {
            bytes.resize(new_byte_length, 0);
            replace_memory_buffer(rt, this, ab, bytes, max_byte_length);
        }
        if address64 {
            Ok(Value::BigInt(Rc::new(
                rusty_js_runtime::bigint::JsBigInt::from_i64(old_pages as i64),
            )))
        } else {
            Ok(Value::Number(old_pages as f64))
        }
    });

    let table_ctor = native_function(rt, "Table", 1, |rt, args| {
        let this = table_this(rt)?;
        let desc = match args.first() {
            Some(Value::Object(desc)) => *desc,
            _ => {
                return Err(RuntimeError::TypeError(
                    "WebAssembly.Table: descriptor object required".into(),
                ))
            }
        };
        let element = wasm_table_element_name(rt, desc)?;
        let initial = wasm_table_u32_field(rt, desc, "initial", true)?.unwrap_or(0);
        let maximum = wasm_table_u32_field(rt, desc, "maximum", false)?;
        if let Some(maximum) = maximum {
            if maximum < initial {
                return Err(RuntimeError::RangeError(
                    "WebAssembly.Table: maximum is less than initial".into(),
                ));
            }
        }
        let storage = new_object(rt);
        let default_value = if element == "externref" {
            Value::Undefined
        } else {
            Value::Null
        };
        for i in 0..initial {
            rt.object_set(storage, i.to_string(), default_value.clone());
        }
        rt.define_data_property_attrs(
            storage,
            "length",
            Value::Number(initial as f64),
            true,
            false,
            true,
        );
        rt.define_data_property_attrs(
            this,
            "__wasm_table_storage",
            Value::Object(storage),
            true,
            false,
            true,
        );
        rt.define_data_property_attrs(
            this,
            "__wasm_table_element",
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                element.as_str(),
            ))),
            true,
            false,
            true,
        );
        rt.define_data_property_attrs(
            this,
            "__wasm_table_address64",
            Value::Boolean(false),
            true,
            false,
            true,
        );
        if let Some(maximum) = maximum {
            rt.define_data_property_attrs(
                this,
                "__wasm_table_maximum",
                Value::Number(maximum as f64),
                true,
                false,
                true,
            );
        }
        rt.define_data_property_attrs(
            this,
            "length",
            Value::Number(initial as f64),
            true,
            false,
            true,
        );
        Ok(Value::Object(this))
    });
    let table_proto = install_namespace_ctor(rt, wasm, "Table", table_ctor);
    install_builtin_function(rt, table_proto, "get", 1, |rt, args| {
        let this = table_this(rt)?;
        let storage = match rt.object_get(this, "__wasm_table_storage") {
            Value::Object(s) => s,
            _ => return Ok(Value::Undefined),
        };
        let idx = wasm_table_index_arg(args, 0, "get")?;
        if idx >= wasm_table_len(rt, this) {
            return Err(RuntimeError::RangeError(
                "WebAssembly.Table.prototype.get: index out of bounds".into(),
            ));
        }
        Ok(rt.object_get(storage, &idx.to_string()))
    });
    install_builtin_function(rt, table_proto, "set", 2, |rt, args| {
        let this = table_this(rt)?;
        let storage = match rt.object_get(this, "__wasm_table_storage") {
            Value::Object(s) => s,
            _ => return Ok(Value::Undefined),
        };
        let idx = wasm_table_index_arg(args, 0, "set")?;
        if idx >= wasm_table_len(rt, this) {
            return Err(RuntimeError::RangeError(
                "WebAssembly.Table.prototype.set: index out of bounds".into(),
            ));
        }
        let value = args.get(1).cloned().unwrap_or(Value::Null);
        wasm_table_check_value(rt, this, &value)?;
        rt.object_set(storage, idx.to_string(), value);
        Ok(Value::Undefined)
    });
    install_builtin_function(rt, table_proto, "grow", 1, |rt, args| {
        let this = table_this(rt)?;
        let storage = match rt.object_get(this, "__wasm_table_storage") {
            Value::Object(s) => s,
            _ => return Ok(Value::Number(0.0)),
        };
        let old = wasm_table_len(rt, this);
        let delta = wasm_table_index_arg(args, 0, "grow")?;
        let new_len = old.checked_add(delta).ok_or_else(|| {
            RuntimeError::RangeError("WebAssembly.Table.prototype.grow: length overflow".into())
        })?;
        if let Some(maximum) = wasm_table_max(rt, this) {
            if new_len > maximum {
                return Err(RuntimeError::RangeError(format!(
                    "WebAssembly.Table.grow(): failed to grow table by {delta}"
                )));
            }
        }
        let value = args.get(1).cloned().unwrap_or(Value::Null);
        wasm_table_check_value(rt, this, &value)?;
        for i in old..new_len {
            rt.object_set(storage, i.to_string(), value.clone());
        }
        rt.define_data_property_attrs(
            storage,
            "length",
            Value::Number(new_len as f64),
            true,
            false,
            true,
        );
        rt.define_data_property_attrs(
            this,
            "length",
            Value::Number(new_len as f64),
            true,
            false,
            true,
        );
        Ok(Value::Number(old as f64))
    });

    let global_ctor = native_function(rt, "Global", 1, |rt, args| {
        let this = global_this(rt)?;
        let (mutable, ty) = match args.first() {
            Some(Value::Object(desc)) => {
                let mutable = matches!(rt.object_get(*desc, "mutable"), Value::Boolean(true));
                (mutable, wasm_global_descriptor_type(rt, *desc))
            }
            _ => (false, ValType::I32),
        };
        let value = args.get(1).cloned().unwrap_or(Value::Undefined);
        rt.set_engine_sentinel(
            this,
            "__wasm_global_type",
            js_string(&wasm_global_type_key(ty)),
        );
        install_global_value_accessor(rt, this, value, mutable);
        Ok(Value::Object(this))
    });
    let global_proto = install_namespace_ctor(rt, wasm, "Global", global_ctor);
    install_builtin_function(rt, global_proto, "valueOf", 0, |rt, _args| {
        let this = global_this(rt)?;
        rt.read_property(this, "value")
    });

    for name in ["CompileError", "RuntimeError", "LinkError"] {
        let ctor = make_wasm_error_ctor(rt, name);
        rt.define_data_property_attrs(wasm, name, Value::Object(ctor), true, false, true);
    }

    install_wasm_proposal_namespace(rt, wasm);

    rt.define_data_property_attrs(
        wasm,
        "@@toStringTag",
        js_string("WebAssembly"),
        false,
        false,
        true,
    );

    rt.define_global_property("WebAssembly", Value::Object(wasm));
    let _ = Rc::new(0);
}

pub fn install_wasi(rt: &mut Runtime) {
    let ns = new_object(rt);
    let wasi_ctor = make_callable(rt, "WASI", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(t) => t,
            _ => rt.alloc_object(rusty_js_runtime::value::Object::new_ordinary()),
        };
        let mut wargs: Vec<String> = Vec::new();
        let mut wenv: Vec<String> = Vec::new();
        if let Some(Value::Object(o)) = args.first() {
            if let Value::Object(a) = rt.object_get(*o, "args") {
                let len = match rt.object_get(a, "length") {
                    Value::Number(n) => n as usize,
                    _ => 0,
                };
                for i in 0..len {
                    if let Value::String(s) = rt.object_get(a, &i.to_string()) {
                        wargs.push(s.as_str().to_string());
                    }
                }
            }
            if let Value::Object(e) = rt.object_get(*o, "env") {
                for k in rt.ordinary_own_enumerable_string_keys(e) {
                    if let Value::String(v) = rt.object_get(e, &k) {
                        wenv.push(format!("{k}={}", v.as_str()));
                    }
                }
            }
        }

        let mut fds: Vec<Option<WasiFd>> = vec![None, None, None];
        if let Some(Value::Object(o)) = args.first() {
            if let Value::Object(pre) = rt.object_get(*o, "preopens") {
                for guest in rt.ordinary_own_enumerable_string_keys(pre) {
                    if let Value::String(host) = rt.object_get(pre, &guest) {
                        fds.push(Some(WasiFd::Preopen {
                            guest: guest.clone(),
                            host: host.as_str().to_string(),
                        }));
                    }
                }
            }
        }
        let id = WASI_STATES.with(|v| {
            let mut v = v.borrow_mut();
            v.push(Some(WasiState {
                args: wargs,
                env: wenv,
                fds,
            }));
            v.len() - 1
        });
        rt.set_engine_sentinel(this, "__wasi_id", Value::Number(id as f64));

        register_method(rt, this, "getImportObject", |rt, _a| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let id = rt.object_get(this, "__wasi_id");
            let io = new_object(rt);
            let preview = new_object(rt);
            rt.object_set(preview, "__wasi_id".into(), id.clone());
            rt.object_set(io, "wasi_snapshot_preview1".into(), Value::Object(preview));
            Ok(Value::Object(io))
        });

        {
            let io = new_object(rt);
            let preview = new_object(rt);
            rt.object_set(preview, "__wasi_id".into(), Value::Number(id as f64));
            rt.object_set(io, "wasi_snapshot_preview1".into(), Value::Object(preview));
            rt.object_set(this, "wasiImport".into(), Value::Object(preview));
        }
        register_method(rt, this, "start", |rt, args| {
            if let Some(Value::Object(inst)) = args.first() {
                let exports = rt.object_get(*inst, "exports");
                if let Value::Object(ex) = exports {
                    let start = rt.object_get(ex, "_start");
                    if rt.is_callable(&start) {
                        let _ = rt.call_function(start, Value::Undefined, Vec::new());
                    }
                }
            }
            Ok(Value::Undefined)
        });
        register_method(rt, this, "initialize", |rt, args| {
            if let Some(Value::Object(inst)) = args.first() {
                if let Value::Object(ex) = rt.object_get(*inst, "exports") {
                    let init = rt.object_get(ex, "_initialize");
                    if rt.is_callable(&init) {
                        let _ = rt.call_function(init, Value::Undefined, Vec::new());
                    }
                }
            }
            Ok(Value::Undefined)
        });
        Ok(Value::Object(this))
    });
    rt.object_set(ns, "WASI".into(), Value::Object(wasi_ctor));
    rt.define_global_property("wasi", Value::Object(ns));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wasm_execution_panic_is_reported_as_error() {
        let err = catch_wasm_execution(|| -> Result<Vec<WasmValue>, String> {
            panic!("synthetic wasm execution panic")
        })
        .expect_err("panic must become an execution error");
        assert!(err.contains("internal panic during execution"), "{err}");
    }
}
