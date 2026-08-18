
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

use rusty_js_gc::{Scheduler, WorkerPool};

use crate::interp::Runtime;
use crate::value::{InternalKind, PromiseStatus, Value};

#[derive(Debug)]
pub struct HostCallRequest {
    pub compartment_id: u64,
    pub tool: String,
    pub args: SendIr,
    pub response: mpsc::Sender<Result<SendIr, String>>,
}

pub type HostCallSender = mpsc::Sender<HostCallRequest>;
pub type HostCallHandler = Rc<dyn Fn(&mut Runtime, &SendIr) -> Result<SendIr, String>>;
pub const DEFAULT_HOST_CALL_MAX_PAYLOAD_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCallAuditRecord {
    pub compartment_id: u64,
    pub tool: String,
    pub outcome: &'static str,
    pub arg_bytes: usize,
    pub result_bytes: Option<usize>,
    pub elapsed_ms: Option<u64>,
    pub error_kind: Option<&'static str>,
}

fn send_ir_approx_bytes(ir: &SendIr) -> usize {
    match ir {
        SendIr::Undefined | SendIr::Null | SendIr::Boolean(_) => 1,
        SendIr::Number(_) => 8,
        SendIr::BigInt(s) => s.len(),
        SendIr::Str(crate::send_ir::SendStr::Owned(s)) => s.len(),
        SendIr::Str(crate::send_ir::SendStr::Shared(_)) => 16,
        SendIr::ArrayBuffer(bytes) => bytes.len(),
        SendIr::TypedArray { bytes, .. } => bytes.len(),
        SendIr::SharedArrayBuffer { byte_length, .. } => *byte_length,
        SendIr::Composite { props, .. } => props
            .iter()
            .map(|(k, v)| k.len() + send_ir_approx_bytes(v))
            .sum(),
        SendIr::MapData {
            entries, orig_keys, ..
        } => entries
            .iter()
            .chain(orig_keys.iter())
            .map(|(k, v)| k.len() + send_ir_approx_bytes(v))
            .sum(),
        SendIr::SetData { values, .. } => values
            .iter()
            .map(|(k, v)| k.len() + send_ir_approx_bytes(v))
            .sum(),
        SendIr::RegExp { source, flags } => source.len() + flags.len(),
        SendIr::BoxedPrimitive { kind, value } => kind.len() + send_ir_approx_bytes(value),
        SendIr::ErrorObj {
            name,
            message,
            stack,
            cause,
        } => {
            name.len()
                + message.len()
                + stack.len()
                + cause.as_ref().map(|c| send_ir_approx_bytes(c)).unwrap_or(0)
        }
        SendIr::Ref(_) => 4,
        SendIr::Callable { name, .. } => name.len() + 24,
    }
}

fn host_call_payload_limit_error(
    direction: &str,
    tool: &str,
    bytes: usize,
    limit: usize,
) -> String {
    format!(
        "worker host-call {direction} payload exceeded limit for {tool}: {bytes} > {limit} bytes"
    )
}

#[derive(Debug, Clone)]
pub enum DescriptorValue {
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),
    Str(String),
}

#[derive(Debug, Clone)]
pub struct CompartmentDescriptor {
    pub initial_globals: Vec<(String, DescriptorValue)>,

    pub boundary_policy: Option<f64>,

    pub onmessage_source: Option<String>,

    pub timeout_ms: Option<f64>,

    pub module_loader_config: Option<ModuleLoaderConfig>,

    pub host_call_tx: Option<HostCallSender>,
}

#[derive(Debug, Clone)]
pub struct ModuleLoaderConfig {

    pub base_url: String,
}

const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<ModuleLoaderConfig>();
};

const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<CompartmentDescriptor>();
    assert_send::<HostCallSender>();
};

impl Runtime {
    pub fn ensure_worker_host_call_channel(&mut self) -> HostCallSender {
        if let Some(tx) = &self.worker_host_call_tx {
            return tx.clone();
        }
        let (tx, rx) = mpsc::channel();
        self.worker_host_call_tx = Some(tx.clone());
        self.worker_host_call_rx = Some(rx);
        tx
    }

    pub fn register_worker_host_call_handler<F>(&mut self, name: &str, f: F)
    where
        F: Fn(&mut Runtime, &SendIr) -> Result<SendIr, String> + 'static,
    {
        self.ensure_worker_host_call_channel();
        self.worker_host_call_handlers
            .insert(name.to_string(), Rc::new(f));
    }

    pub fn set_worker_host_call_max_payload_bytes(&mut self, limit: usize) {
        self.worker_host_call_max_payload_bytes = limit;
    }

    pub fn set_worker_host_call_timeout_ms(&mut self, timeout_ms: Option<u64>) {
        self.worker_host_call_timeout_ms = timeout_ms;
    }

    pub fn revoke_worker_host_calls_for_compartment(&mut self, compartment_id: u64) {
        self.worker_host_call_revoked_compartments
            .insert(compartment_id);
    }

    pub fn worker_host_call_audit_records(&self) -> &[HostCallAuditRecord] {
        &self.worker_host_call_audit
    }

    fn audit_worker_host_call(&mut self, record: HostCallAuditRecord) {
        self.worker_host_call_audit.push(record);
    }

    pub fn drain_worker_host_calls(&mut self) -> Result<bool, crate::RuntimeError> {
        let Some(rx) = self.worker_host_call_rx.take() else {
            return Ok(false);
        };
        let mut progressed = false;
        loop {
            match rx.try_recv() {
                Ok(req) => {
                    progressed = true;
                    if self
                        .worker_host_call_revoked_compartments
                        .contains(&req.compartment_id)
                    {
                        self.audit_worker_host_call(HostCallAuditRecord {
                            compartment_id: req.compartment_id,
                            tool: req.tool.clone(),
                            outcome: "error",
                            arg_bytes: 0,
                            result_bytes: None,
                            elapsed_ms: None,
                            error_kind: Some("revoked"),
                        });
                        let _ = req.response.send(Err(format!(
                            "worker host-call revoked for {}: compartment {} is closed",
                            req.tool, req.compartment_id
                        )));
                        continue;
                    }
                    let arg_bytes = send_ir_approx_bytes(&req.args);
                    if arg_bytes > self.worker_host_call_max_payload_bytes {
                        self.audit_worker_host_call(HostCallAuditRecord {
                            compartment_id: req.compartment_id,
                            tool: req.tool.clone(),
                            outcome: "error",
                            arg_bytes,
                            result_bytes: None,
                            elapsed_ms: None,
                            error_kind: Some("arg_payload_limit"),
                        });
                        let _ = req.response.send(Err(host_call_payload_limit_error(
                            "arg",
                            &req.tool,
                            arg_bytes,
                            self.worker_host_call_max_payload_bytes,
                        )));
                        continue;
                    }
                    let started = std::time::Instant::now();
                    let mut registry_error_kind = None;
                    let result = match self.worker_host_call_handlers.get(&req.tool).cloned() {
                        Some(handler) => handler(self, &req.args),
                        None => {
                            registry_error_kind = Some("unknown_tool");
                            Err(format!(
                                "worker host-call denied: unknown tool {}",
                                req.tool
                            ))
                        }
                    };
                    if let Some(timeout_ms) = self.worker_host_call_timeout_ms {
                        let elapsed_ms = started.elapsed().as_millis() as u64;
                        if elapsed_ms > timeout_ms {
                            self.audit_worker_host_call(HostCallAuditRecord {
                                compartment_id: req.compartment_id,
                                tool: req.tool.clone(),
                                outcome: "error",
                                arg_bytes,
                                result_bytes: None,
                                elapsed_ms: Some(elapsed_ms),
                                error_kind: Some("timeout"),
                            });
                            let _ = req.response.send(Err(format!(
                                "worker host-call timed out for {}: elapsed_ms={} timeout_ms={}",
                                req.tool, elapsed_ms, timeout_ms
                            )));
                            continue;
                        }
                    }
                    let result = match result {
                        Ok(ir) => {
                            let bytes = send_ir_approx_bytes(&ir);
                            if bytes > self.worker_host_call_max_payload_bytes {
                                self.audit_worker_host_call(HostCallAuditRecord {
                                    compartment_id: req.compartment_id,
                                    tool: req.tool.clone(),
                                    outcome: "error",
                                    arg_bytes,
                                    result_bytes: Some(bytes),
                                    elapsed_ms: Some(started.elapsed().as_millis() as u64),
                                    error_kind: Some("result_payload_limit"),
                                });
                                Err(host_call_payload_limit_error(
                                    "result",
                                    &req.tool,
                                    bytes,
                                    self.worker_host_call_max_payload_bytes,
                                ))
                            } else {
                                self.audit_worker_host_call(HostCallAuditRecord {
                                    compartment_id: req.compartment_id,
                                    tool: req.tool.clone(),
                                    outcome: "ok",
                                    arg_bytes,
                                    result_bytes: Some(bytes),
                                    elapsed_ms: Some(started.elapsed().as_millis() as u64),
                                    error_kind: None,
                                });
                                Ok(ir)
                            }
                        }
                        Err(e) => {
                            self.audit_worker_host_call(HostCallAuditRecord {
                                compartment_id: req.compartment_id,
                                tool: req.tool.clone(),
                                outcome: "error",
                                arg_bytes,
                                result_bytes: None,
                                elapsed_ms: Some(started.elapsed().as_millis() as u64),
                                error_kind: Some(registry_error_kind.unwrap_or("handler_error")),
                            });
                            Err(e)
                        }
                    };
                    let _ = req.response.send(result);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
        }
        self.worker_host_call_rx = Some(rx);
        Ok(progressed)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RealmHandle {
    pub global_this: rusty_js_gc::ObjectId,

    pub boundary_policy: Option<f64>,
}

thread_local! {

    static WORKER_RT: RefCell<Option<Runtime>> = const { RefCell::new(None) };
}

fn descriptor_value_to_value(d: &DescriptorValue) -> Value {
    match d {
        DescriptorValue::Undefined => Value::Undefined,
        DescriptorValue::Null => Value::Null,
        DescriptorValue::Boolean(b) => Value::Boolean(*b),
        DescriptorValue::Number(n) => Value::Number(*n),
        DescriptorValue::Str(s) => {
            Value::String(std::rc::Rc::new(crate::value::JsString::from(s.clone())))
        }
    }
}

pub fn build_realm_from_descriptor(desc: &CompartmentDescriptor) -> RealmHandle {
    WORKER_RT.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            let mut rt = Runtime::new();
            rt.install_intrinsics();
            *slot = Some(rt);
        }
        let rt = slot.as_mut().expect("worker Runtime present");
        let gt = rt
            .global_object
            .expect("globalThis present after install_intrinsics");
        for (k, v) in &desc.initial_globals {
            let val = descriptor_value_to_value(v);
            rt.define_global_property(k, val);
        }
        RealmHandle {
            global_this: gt,
            boundary_policy: desc.boundary_policy,
        }
    })
}

pub fn with_worker_runtime<R>(f: impl FnOnce(&mut Runtime) -> R) -> Option<R> {
    WORKER_RT.with(|cell| cell.borrow_mut().as_mut().map(f))
}

#[derive(Debug, Clone, Copy)]
pub struct RealmBuilt {
    pub compartment_id: u64,
    pub worker: usize,
}

pub fn dispatch_create_realm(
    scheduler: &mut Scheduler,
    pool: &WorkerPool,
    compartment_id: u64,
    descriptor: CompartmentDescriptor,
) -> std::sync::mpsc::Receiver<RealmBuilt> {
    let worker = scheduler.assign(compartment_id);
    let (tx, rx) = std::sync::mpsc::channel();
    pool.submit(worker, move || {
        let _handle = build_realm_from_descriptor(&descriptor);
        let _ = tx.send(RealmBuilt {
            compartment_id,
            worker,
        });
    })
    .expect("create-realm job submitted");
    rx
}

use crate::interp::RealmRecord;
use crate::send_ir::{rematerialize_send_ir, SendIr, SendIrDisposition};
use rusty_js_gc::Tier2Arena;

struct CompartmentEntry {
    realm_idx: usize,
    global_this: rusty_js_gc::ObjectId,
    onmessage: Value,
    timeout_ms: Option<f64>,

    loader_base: Option<String>,
}

thread_local! {

    static WORKER_COMPARTMENTS: RefCell<std::collections::HashMap<u64, CompartmentEntry>> =
        RefCell::new(std::collections::HashMap::new());
}

thread_local! {

    static WORKER_CALLABLES: RefCell<std::collections::HashMap<u64, crate::send_ir::CallableRegistry>> =
        RefCell::new(std::collections::HashMap::new());
}

fn ensure_worker_rt() {
    WORKER_RT.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            let mut rt = Runtime::new();
            rt.install_intrinsics();
            rt.register_root_source(collect_worker_roots);
            *slot = Some(rt);
        }
    });
}

fn allocate_lightweight_worker_compartment_realm(rt: &mut Runtime) -> usize {
    let idx = rt.realms.len();
    rt.realms.push(RealmRecord {
        object_prototype: rt.object_prototype,
        array_prototype: rt.array_prototype,
        function_prototype: rt.function_prototype,
        async_function_prototype: rt.async_function_prototype,
        promise_prototype: rt.promise_prototype,
        string_prototype: rt.string_prototype,
        number_prototype: rt.number_prototype,
        boolean_prototype: rt.boolean_prototype,
        bigint_prototype: rt.bigint_prototype,
        symbol_prototype: rt.symbol_prototype,
        regexp_prototype: rt.regexp_prototype,
        iterator_prototype: rt.iterator_prototype,
        array_iterator_prototype: rt.array_iterator_prototype,
        string_iterator_prototype: rt.string_iterator_prototype,
        regexp_string_iterator_prototype: rt.regexp_string_iterator_prototype,
        generator_prototype: rt.generator_prototype,
        generator_function_prototype: rt.generator_function_prototype,
        async_iterator_prototype: rt.async_iterator_prototype,
        async_generator_prototype: rt.async_generator_prototype,
        async_generator_function_prototype: rt.async_generator_function_prototype,
        ambient_denied: true,
        independent_gc_allowed: true,
        capability_mode: crate::caps::CapMode::Compat,
        ..RealmRecord::default()
    });
    idx
}

pub fn build_compartment_on_worker(compartment_id: u64, desc: &CompartmentDescriptor) {
    ensure_worker_rt();
    WORKER_RT.with(|rtcell| {
        let mut slot = rtcell.borrow_mut();
        let rt = slot.as_mut().expect("worker Runtime");

        {
            let mut core: Vec<Value> = Vec::new();
            for id in [
                rt.global_object,
                rt.object_prototype,
                rt.array_prototype,
                rt.function_prototype,
                rt.async_function_prototype,
                rt.promise_prototype,
                rt.string_prototype,
                rt.number_prototype,
                rt.boolean_prototype,
                rt.bigint_prototype,
                rt.symbol_prototype,
                rt.regexp_prototype,
                rt.iterator_prototype,
                rt.array_iterator_prototype,
                rt.string_iterator_prototype,
                rt.regexp_string_iterator_prototype,
                rt.generator_prototype,
                rt.generator_function_prototype,
                rt.async_iterator_prototype,
                rt.async_generator_prototype,
                rt.async_generator_function_prototype,
            ]
            .into_iter()
            .flatten()
            {
                core.push(Value::Object(id));
            }
            rt.retain_host_roots("__cruft_worker_primordials", core);
        }
        let realm_idx = allocate_lightweight_worker_compartment_realm(rt);
        let prior_realm_for_global = rt.current_realm;
        rt.current_realm = realm_idx;
        let gt = rt.alloc_object(crate::value::Object::new_ordinary());
        rt.current_realm = prior_realm_for_global;
        rt.realms[realm_idx].global = Some(gt);

        for name in Runtime::intrinsic_name_allowlist() {
            let v = rt.global_get(name);
            if !matches!(v, Value::Undefined) {
                rt.obj_mut(gt).dict_mut().insert(
                    crate::value::PropertyKey::String((*name).to_string()),
                    crate::value::PropertyDescriptor {
                        value: v,
                        writable: true,
                        enumerable: false,
                        configurable: true,
                        getter: None,
                        setter: None,
                    },
                );
            }
        }

        rt.object_set(gt, "globalThis".to_string(), Value::Object(gt));
        rt.object_set(gt, "self".to_string(), Value::Object(gt));
        if let Some(host_call_tx) = desc.host_call_tx.clone() {
            let host_call_compartment_id = compartment_id;
            let host_call = crate::intrinsics::make_native("__cruft_hostCall", move |rt, args| {
                let tool = match args.first() {
                    Some(Value::String(s)) => s.as_str().to_string(),
                    _ => {
                        return Err(crate::RuntimeError::TypeError(
                            "__cruft_hostCall: tool name must be a string".into(),
                        ))
                    }
                };
                let tool_for_error = tool.clone();
                let payload = args.get(1).cloned().unwrap_or(Value::Undefined);
                let mut ctx = crate::send_ir::LowerCtx::new(None);
                let args_ir =
                    crate::send_ir::lower_to_send_ir(rt, &payload, &mut ctx).map_err(|e| {
                        crate::RuntimeError::TypeError(format!(
                            "__cruft_hostCall: args not transferable: {:?}",
                            e
                        ))
                    })?;
                let (tx, rx) = mpsc::channel();
                host_call_tx
                    .send(HostCallRequest {
                        compartment_id: host_call_compartment_id,
                        tool,
                        args: args_ir,
                        response: tx,
                    })
                    .map_err(|_| {
                        crate::RuntimeError::TypeError(
                            "__cruft_hostCall: parent channel closed".into(),
                        )
                    })?;
                match rx.recv() {
                    Ok(Ok(ret_ir)) => {
                        let mut table = std::collections::HashMap::new();
                        crate::send_ir::rematerialize_send_ir(rt, &ret_ir, None, &mut table)
                    }
                    Ok(Err(msg)) => {
                        let err = rt.alloc_object(crate::value::Object::new_ordinary());
                        rt.object_set(err, "ok".to_string(), Value::Boolean(false));
                        rt.object_set(
                            err,
                            "tool".to_string(),
                            Value::String(std::rc::Rc::new(crate::value::JsString::from(
                                tool_for_error,
                            ))),
                        );
                        rt.object_set(
                            err,
                            "error".to_string(),
                            Value::String(std::rc::Rc::new(crate::value::JsString::from(msg))),
                        );
                        Ok(Value::Object(err))
                    }
                    Err(_) => Err(crate::RuntimeError::TypeError(
                        "__cruft_hostCall: parent response channel closed".into(),
                    )),
                }
            });
            let host_call_id = rt.alloc_object(host_call);
            rt.object_set(
                gt,
                "__cruft_hostCall".to_string(),
                Value::Object(host_call_id),
            );
        }
        for (k, v) in &desc.initial_globals {
            let val = descriptor_value_to_value(v);
            rt.object_set(gt, k.clone(), val);
        }

        let onmessage = match &desc.onmessage_source {
            Some(src) => {

                let wrapped = format!("globalThis.__om = ({});", src);
                let prior_gt = rt.global_object;
                rt.global_object = Some(gt);
                let prior_realm = rt.current_realm;
                rt.current_realm = realm_idx;
                let intrinsics_snapshot = rt.swap_realm_intrinsics_only(realm_idx);
                rt.evaluate_script(&wrapped, "file://compartment-onmessage")
                    .ok();
                rt.restore_realm_intrinsics_only(intrinsics_snapshot);
                rt.current_realm = prior_realm;
                rt.global_object = prior_gt;
                rt.object_get(gt, "__om")
            }
            None => Value::Undefined,
        };
        WORKER_COMPARTMENTS.with(|m| {
            m.borrow_mut().insert(
                compartment_id,
                CompartmentEntry {
                    realm_idx,
                    global_this: gt,
                    onmessage,
                    timeout_ms: desc.timeout_ms,
                    loader_base: desc
                        .module_loader_config
                        .as_ref()
                        .map(|c| c.base_url.clone()),
                },
            );
        });
    });
}

pub fn dispatch_create_compartment(
    scheduler: &mut Scheduler,
    pool: &WorkerPool,
    compartment_id: u64,
    descriptor: CompartmentDescriptor,
) -> std::sync::mpsc::Receiver<usize> {
    let worker = scheduler.assign(compartment_id);
    let (tx, rx) = std::sync::mpsc::channel();
    pool.submit(worker, move || {
        build_compartment_on_worker(compartment_id, &descriptor);
        let _ = tx.send(worker);
    })
    .expect("create-compartment job submitted");
    rx
}

pub fn set_worker_onmessage(compartment_id: u64, handler: Value) {
    WORKER_COMPARTMENTS.with(|m| {
        if let Some(entry) = m.borrow_mut().get_mut(&compartment_id) {
            entry.onmessage = handler;
        }
    });
}

pub fn teardown_worker_compartment(compartment_id: u64) -> bool {
    let removed_entry = WORKER_COMPARTMENTS.with(|m| m.borrow_mut().remove(&compartment_id));
    WORKER_CALLABLES.with(|m| {
        m.borrow_mut().remove(&compartment_id);
    });
    if let Some(entry) = removed_entry {
        WORKER_RT.with(|rtcell| {
            if let Some(rt) = rtcell.borrow_mut().as_mut() {
                let _ = rt.close_compartment_realm(entry.realm_idx);
            }
        });
        true
    } else {
        false
    }
}

pub(crate) fn collect_worker_roots(_rt: &Runtime, roots: &mut Vec<rusty_js_gc::ObjectId>) {
    WORKER_COMPARTMENTS.with(|m| {
        for entry in m.borrow().values() {
            roots.push(entry.global_this);
            if let Value::Object(handler) = &entry.onmessage {
                roots.push(*handler);
            }
        }
    });
    WORKER_CALLABLES.with(|m| {
        for registry in m.borrow().values() {
            roots.extend(registry.roots());
        }
    });
}

pub fn deliver_to_compartment(
    compartment_id: u64,
    payload: &SendIr,
    string_arena: Option<&Tier2Arena<String>>,
) -> bool {
    let (realm_idx, gt, handler) = match WORKER_COMPARTMENTS.with(|m| {
        m.borrow()
            .get(&compartment_id)
            .map(|e| (e.realm_idx, e.global_this, e.onmessage.clone()))
    }) {
        Some(pair) => pair,
        None => return false,
    };
    WORKER_RT.with(|rtcell| {
        let mut slot = rtcell.borrow_mut();
        let rt = slot.as_mut().expect("worker Runtime");
        let mut table = std::collections::HashMap::new();
        let data = match rematerialize_send_ir(rt, payload, string_arena, &mut table) {
            Ok(v) => v,
            Err(_) => return,
        };

        rt.object_set(gt, "message".to_string(), data.clone());

        if rt.is_callable(&handler) {
            let prior_realm = rt.current_realm;
            rt.current_realm = realm_idx;
            let intrinsics_snapshot = rt.swap_realm_intrinsics_only(realm_idx);
            let ev = rt.alloc_object(crate::value::Object::new_ordinary());
            rt.object_set(ev, "data".to_string(), data);
            let prior_gt = rt.global_object;
            rt.global_object = Some(gt);
            let _ = rt.call_function(handler, Value::Undefined, vec![Value::Object(ev)]);
            rt.global_object = prior_gt;
            rt.restore_realm_intrinsics_only(intrinsics_snapshot);
            rt.current_realm = prior_realm;
        }
    });
    true
}

pub fn worker_read_compartment_global(compartment_id: u64, name: &str) -> Value {
    let gt = WORKER_COMPARTMENTS.with(|m| m.borrow().get(&compartment_id).map(|e| e.global_this));
    match gt {
        Some(gt) => WORKER_RT.with(|rtcell| {
            rtcell
                .borrow_mut()
                .as_mut()
                .map(|rt| rt.object_get(gt, name))
                .unwrap_or(Value::Undefined)
        }),
        None => Value::Undefined,
    }
}

pub fn worker_compartment_loader_base(compartment_id: u64) -> Option<String> {
    WORKER_COMPARTMENTS.with(|m| {
        m.borrow()
            .get(&compartment_id)
            .and_then(|e| e.loader_base.clone())
    })
}

pub fn cross_thread_send(
    scheduler: &mut Scheduler,
    pool: &WorkerPool,
    target_compartment_id: u64,
    payload: SendIr,
) -> std::sync::mpsc::Receiver<bool> {
    if !worker_message_payload_allowed(&payload) {
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(false);
        return rx;
    }
    let worker = scheduler.assign(target_compartment_id);
    let (tx, rx) = std::sync::mpsc::channel();
    pool.submit(worker, move || {
        let delivered = deliver_to_compartment(target_compartment_id, &payload, None);
        let _ = tx.send(delivered);
    })
    .expect("cross-thread delivery job submitted");
    rx
}

pub fn cross_thread_teardown(
    scheduler: &mut Scheduler,
    pool: &WorkerPool,
    compartment_id: u64,
) -> std::sync::mpsc::Receiver<bool> {
    let worker = scheduler.assign(compartment_id);
    let (tx, rx) = std::sync::mpsc::channel();
    pool.submit(worker, move || {
        let removed = teardown_worker_compartment(compartment_id);
        let _ = tx.send(removed);
    })
    .expect("cross-thread teardown job submitted");
    rx
}

pub fn cross_thread_request(
    scheduler: &mut Scheduler,
    pool: &WorkerPool,
    target_compartment_id: u64,
    payload: SendIr,
) -> std::sync::mpsc::Receiver<Result<SendIr, String>> {
    if !worker_message_payload_allowed(&payload) {
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(Err(
            "request payload: callable proxy is not a generic message payload".to_string(),
        ));
        return rx;
    }
    let worker = scheduler.assign(target_compartment_id);
    let (tx, rx) = std::sync::mpsc::channel();
    pool.submit(worker, move || {
        let result: Result<SendIr, String> =
            request_from_compartment(target_compartment_id, &payload);
        let _ = tx.send(result);
    })
    .expect("cross-thread request job submitted");
    rx
}

fn request_from_compartment(compartment_id: u64, payload: &SendIr) -> Result<SendIr, String> {
    let (realm_idx, gt, handler, timeout_ms) = WORKER_COMPARTMENTS
        .with(|m| {
            m.borrow().get(&compartment_id).map(|e| {
                (
                    e.realm_idx,
                    e.global_this,
                    e.onmessage.clone(),
                    e.timeout_ms,
                )
            })
        })
        .ok_or_else(|| "request: target compartment not found".to_string())?;
    WORKER_RT.with(|rtcell| {
        let mut slot = rtcell.borrow_mut();
        let rt = slot
            .as_mut()
            .ok_or_else(|| "request: worker runtime missing".to_string())?;
        let mut table = std::collections::HashMap::new();
        let data = rematerialize_send_ir(rt, payload, None, &mut table)
            .map_err(|e| format!("request payload: {:?}", e))?;
        rt.object_set(gt, "message".to_string(), data.clone());
        if !rt.is_callable(&handler) {
            return Ok(SendIr::Undefined);
        }
        let prior_realm = rt.current_realm;
        rt.current_realm = realm_idx;
        let intrinsics_snapshot = rt.swap_realm_intrinsics_only(realm_idx);
        let ev = rt.alloc_object(crate::value::Object::new_ordinary());
        rt.object_set(ev, "data".to_string(), data);
        let prior_gt = rt.global_object;
        rt.global_object = Some(gt);
        let call_handler =
            |rt: &mut Runtime| rt.call_function(handler, Value::Undefined, vec![Value::Object(ev)]);
        let ret = match timeout_ms {
            Some(t) if t.is_finite() && t > 0.0 => rt
                .run_with_compartment_timeout(compartment_id, t, call_handler)
                .map_err(|e| match e {
                    crate::RuntimeError::Interrupted => {
                        format!("request handler: Compartment worker exceeded its {t} ms timeout")
                    }
                    other => format!("request handler: {:?}", other),
                }),
            _ => call_handler(rt).map_err(|e| format!("request handler: {:?}", e)),
        };
        rt.global_object = prior_gt;
        rt.restore_realm_intrinsics_only(intrinsics_snapshot);
        rt.current_realm = prior_realm;
        let ret = await_worker_request_value(rt, ret?)?;
        WORKER_CALLABLES.with(|m| {
            let mut map = m.borrow_mut();
            let reg = map
                .entry(compartment_id)
                .or_insert_with(|| crate::send_ir::CallableRegistry::new(compartment_id));
            let mut ctx = crate::send_ir::LowerCtx::with_callables(None, reg);
            crate::send_ir::lower_to_send_ir(rt, &ret, &mut ctx).map_err(|e| format!("{:?}", e))
        })
    })
}

fn await_worker_request_value(rt: &mut Runtime, value: Value) -> Result<Value, String> {
    let id = match value {
        Value::Object(id) => id,
        other => return Ok(other),
    };
    let wrapped = rt
        .promise_resolve_via(&Value::Object(id))
        .map_err(|e| format!("request promise resolve: {:?}", e))?;
    let id = match wrapped {
        Value::Object(id) => id,
        other => return Ok(other),
    };
    let is_promise = matches!(rt.obj(id).internal_kind, InternalKind::Promise(_));
    if !is_promise {
        return Ok(Value::Object(id));
    }
    let max_pumps = 100_000usize;
    for _ in 0..=max_pumps {
        let (status, settled) = match &rt.obj(id).internal_kind {
            InternalKind::Promise(ps) => (ps.status, ps.value.clone()),
            _ => return Err("request promise: lost promise during pump".to_string()),
        };
        match status {
            PromiseStatus::Fulfilled => {
                rt.pending_unhandled.remove(&id);
                return Ok(settled);
            }
            PromiseStatus::Rejected => {
                rt.pending_unhandled.remove(&id);
                return Err(format!("request promise rejected: {:?}", settled));
            }
            PromiseStatus::Pending => {}
        }
        let did_work = crate::job_queue::pump_one_tick(rt)
            .map_err(|e| format!("request promise pump: {:?}", e))?;
        if !did_work {
            let progressed = if let Some(poll) = rt.host_hooks.poll_io.take() {
                let progressed = poll(rt).map_err(|e| format!("request promise poll: {:?}", e))?;
                rt.host_hooks.poll_io = Some(poll);
                progressed
            } else {
                false
            };
            if !progressed {
                return Err("request promise never settled (worker event loop idle)".to_string());
            }
        }
    }
    Err("request promise max-pump bound exceeded".to_string())
}

pub fn cross_thread_import_value(
    scheduler: &mut Scheduler,
    pool: &WorkerPool,
    compartment_id: u64,
    specifier: String,
    binding: String,
) -> std::sync::mpsc::Receiver<Result<SendIr, String>> {
    let worker = scheduler.assign(compartment_id);
    let (tx, rx) = std::sync::mpsc::channel();
    pool.submit(worker, move || {
        let base = worker_compartment_loader_base(compartment_id)
            .unwrap_or_else(|| "file:///".to_string());
        let result: Result<SendIr, String> = with_worker_runtime(|rt| {

            let parent = format!("{}__entry__.js", base);
            let resolved = Runtime::resolve_module(&parent, &specifier)
                .map_err(|e| format!("resolve(base={}): {:?}", base, e))?;
            let ns = rt.load_module(&resolved).map_err(|e| format!("{:?}", e))?;
            let val = rt.object_get(ns, &binding);

            WORKER_CALLABLES.with(|m| {
                let mut map = m.borrow_mut();
                let reg = map
                    .entry(compartment_id)
                    .or_insert_with(|| crate::send_ir::CallableRegistry::new(compartment_id));
                let mut ctx = crate::send_ir::LowerCtx::with_callables(None, reg);
                crate::send_ir::lower_to_send_ir(rt, &val, &mut ctx).map_err(|e| format!("{:?}", e))
            })
        })
        .unwrap_or_else(|| Err("worker runtime missing".to_string()));
        let _ = tx.send(result);
    })
    .expect("import-value job submitted");
    rx
}

pub fn cross_thread_invoke_callable(
    scheduler: &mut Scheduler,
    pool: &WorkerPool,
    compartment_id: u64,
    binding_id: u64,
    args_ir: Vec<SendIr>,
) -> std::sync::mpsc::Receiver<Result<SendIr, String>> {
    let worker = scheduler.assign(compartment_id);
    let (tx, rx) = std::sync::mpsc::channel();
    pool.submit(worker, move || {
        let result: Result<SendIr, String> = with_worker_runtime(|rt| {
            let timeout_ms = WORKER_COMPARTMENTS.with(|m| {
                m.borrow()
                    .get(&compartment_id)
                    .and_then(|entry| entry.timeout_ms)
            });
            let callable = WORKER_CALLABLES.with(|m| {
                m.borrow()
                    .get(&compartment_id)
                    .and_then(|reg| reg.get(binding_id))
            });
            let callable = match callable {
                Some(id) => Value::Object(id),
                None => return Err(format!("invoke: unknown callable binding {}", binding_id)),
            };
            let mut argv = Vec::with_capacity(args_ir.len());
            for a in &args_ir {
                let mut t = std::collections::HashMap::new();
                argv.push(
                    crate::send_ir::rematerialize_send_ir(rt, a, None, &mut t)
                        .map_err(|e| format!("invoke arg: {:?}", e))?,
                );
            }
            let call_callable =
                |rt: &mut Runtime| rt.call_function(callable, Value::Undefined, argv);
            let ret = match timeout_ms {
                Some(t) if t.is_finite() && t > 0.0 => rt
                    .run_with_compartment_timeout(compartment_id, t, call_callable)
                    .map_err(|e| match e {
                        crate::RuntimeError::Interrupted => {
                            format!("invoke call: Compartment worker exceeded its {t} ms timeout")
                        }
                        other => format!("invoke call: {:?}", other),
                    })?,
                _ => call_callable(rt).map_err(|e| format!("invoke call: {:?}", e))?,
            };

            WORKER_CALLABLES.with(|m| {
                let mut map = m.borrow_mut();
                let reg = map
                    .entry(compartment_id)
                    .or_insert_with(|| crate::send_ir::CallableRegistry::new(compartment_id));
                let mut ctx = crate::send_ir::LowerCtx::with_callables(None, reg);
                crate::send_ir::lower_to_send_ir(rt, &ret, &mut ctx).map_err(|e| format!("{:?}", e))
            })
        })
        .unwrap_or_else(|| Err("worker runtime missing".to_string()));
        let _ = tx.send(result);
    })
    .expect("invoke-callable job submitted");
    rx
}

use std::sync::{Arc, Mutex};

pub fn lower_with_shared_arena(
    rt: &Runtime,
    v: &Value,
    arena: &Arc<Mutex<Tier2Arena<String>>>,
) -> Result<SendIr, crate::RuntimeError> {
    let mut guard = arena.lock().expect("shared string arena lock");
    let mut ctx = crate::send_ir::LowerCtx::new(Some(&mut guard));
    crate::send_ir::lower_to_send_ir(rt, v, &mut ctx)
}

pub fn cross_thread_send_shared(
    scheduler: &mut Scheduler,
    pool: &WorkerPool,
    target_compartment_id: u64,
    payload: SendIr,
    arena: Arc<Mutex<Tier2Arena<String>>>,
) -> std::sync::mpsc::Receiver<bool> {
    if !worker_message_payload_allowed(&payload) {
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(false);
        return rx;
    }
    let worker = scheduler.assign(target_compartment_id);
    let (tx, rx) = std::sync::mpsc::channel();
    pool.submit(worker, move || {
        let guard = arena
            .lock()
            .expect("shared string arena lock (worker side)");
        let delivered = deliver_to_compartment(target_compartment_id, &payload, Some(&guard));
        let _ = tx.send(delivered);
    })
    .expect("cross-thread shared-arena delivery job submitted");
    rx
}

fn worker_message_payload_allowed(payload: &SendIr) -> bool {
    !matches!(payload.disposition(), SendIrDisposition::CallableProxy)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_test_on_large_stack(f: impl FnOnce() + Send + 'static) {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(f)
            .expect("spawn large-stack worker_realm test")
            .join()
            .expect("large-stack worker_realm test completed");
    }

    #[test]
    fn build_realm_installs_descriptor_globals() {

        run_test_on_large_stack(|| {
            let desc = CompartmentDescriptor {
                initial_globals: vec![
                    ("answer".to_string(), DescriptorValue::Number(42.0)),
                    ("tag".to_string(), DescriptorValue::Str("hello".to_string())),
                ],
                boundary_policy: Some(1.0),
                timeout_ms: None,
                onmessage_source: None,
                module_loader_config: None,
                host_call_tx: None,
            };
            let handle = build_realm_from_descriptor(&desc);
            assert_eq!(handle.boundary_policy, Some(1.0));
            let answer = with_worker_runtime(|rt| rt.global_get("answer")).unwrap();
            assert!(matches!(answer, Value::Number(n) if n == 42.0));
            let tag = with_worker_runtime(|rt| rt.global_get("tag")).unwrap();
            assert!(matches!(tag, Value::String(ref s) if s.as_str() == "hello"));
        });
    }

    #[test]
    fn two_workers_build_isolated_realms() {
        let mut scheduler = Scheduler::new(2);
        let pool = WorkerPool::new(2);

        let mut rxs = Vec::new();
        for cid in 0u64..2 {
            let worker = scheduler.assign(cid);
            let (tx, rx) = std::sync::mpsc::channel::<(u64, usize, f64)>();
            let desc = CompartmentDescriptor {
                initial_globals: vec![(
                    "__worker_id".to_string(),
                    DescriptorValue::Number(cid as f64),
                )],
                boundary_policy: None,
                timeout_ms: None,
                onmessage_source: None,
                module_loader_config: None,
                host_call_tx: None,
            };
            pool.submit(worker, move || {
                let _ = build_realm_from_descriptor(&desc);

                let read = with_worker_runtime(|rt| rt.global_get("__worker_id"))
                    .and_then(|v| match v {
                        Value::Number(n) => Some(n),
                        _ => None,
                    })
                    .unwrap_or(-1.0);
                let _ = tx.send((cid, worker, read));
            })
            .expect("submit");
            rxs.push(rx);
        }

        let mut seen = Vec::new();
        for rx in rxs {
            let (cid, _worker, read) = rx.recv().expect("worker reported");

            assert_eq!(read, cid as f64, "realm must read its own __worker_id");
            seen.push(read);
        }
        seen.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(seen, vec![0.0, 1.0], "two distinct isolated realms built");

        pool.shutdown();
        scheduler.shutdown();
    }

    #[test]
    fn dispatch_create_realm_builds_on_assigned_worker() {
        let mut scheduler = Scheduler::new(2);
        let pool = WorkerPool::new(2);
        let desc = CompartmentDescriptor {
            initial_globals: vec![("x".to_string(), DescriptorValue::Boolean(true))],
            boundary_policy: None,
            timeout_ms: None,
            onmessage_source: None,
            module_loader_config: None,
            host_call_tx: None,
        };
        let rx = dispatch_create_realm(&mut scheduler, &pool, 7, desc);
        let built = rx.recv().expect("realm built");
        assert_eq!(built.compartment_id, 7);
        assert!(built.worker < 2);
        pool.shutdown();
        scheduler.shutdown();
    }
    use crate::send_ir::{lower_to_send_ir, LowerCtx};

    fn lower_global(
        src: &str,
        name: &str,
        arena: Option<&mut rusty_js_gc::Tier2Arena<String>>,
    ) -> Result<super::SendIr, String> {
        if arena.is_some() {
            let mut sender = Runtime::new();
            sender.install_intrinsics();
            sender
                .evaluate_script(src, "file://sender")
                .expect("sender script");
            let gt = sender.global_object.unwrap();
            let v = sender.object_get(gt, name);
            let mut ctx = LowerCtx::new(arena);
            return lower_to_send_ir(&sender, &v, &mut ctx).map_err(|e| format!("{e:?}"));
        }

        let src = src.to_string();
        let name = name.to_string();
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(move || {
                let mut sender = Runtime::new();
                sender.install_intrinsics();
                sender
                    .evaluate_script(&src, "file://sender")
                    .expect("sender script");
                let gt = sender.global_object.unwrap();
                let v = sender.object_get(gt, &name);
                let mut ctx = LowerCtx::new(None);
                lower_to_send_ir(&sender, &v, &mut ctx).map_err(|e| format!("{e:?}"))
            })
            .expect("spawn large-stack lower_global")
            .join()
            .expect("large-stack lower_global completed")
    }

    #[test]
    fn cross_thread_round_trip_delivers_and_invokes_onmessage() {
        let mut scheduler = Scheduler::new(2);
        let pool = WorkerPool::new(2);
        let cid = 100u64;
        let worker = scheduler.assign(cid);

        let (stx, srx) = std::sync::mpsc::channel::<()>();
        pool.submit(worker, move || {
            build_compartment_on_worker(
                cid,
                &CompartmentDescriptor {
                    initial_globals: vec![],
                    boundary_policy: None,
                    timeout_ms: None,
                    onmessage_source: Some(
                        "function(e){ globalThis.viaHandler = e.data.v; }".to_string(),
                    ),
                    module_loader_config: None,
                    host_call_tx: None,
                },
            );
            let _ = stx.send(());
        })
        .expect("setup submit");
        srx.recv().unwrap();

        let payload = lower_global("globalThis.m = { v: 42 };", "m", None).expect("lower");

        let rx = cross_thread_send(&mut scheduler, &pool, cid, payload);
        assert!(
            rx.recv().unwrap(),
            "payload delivered to the target worker realm"
        );

        let (rtx, rrx) = std::sync::mpsc::channel::<f64>();
        pool.submit(worker, move || {
            let n = match worker_read_compartment_global(cid, "viaHandler") {
                Value::Number(n) => n,
                _ => -1.0,
            };
            let _ = rtx.send(n);
        })
        .expect("readback submit");
        assert_eq!(
            rrx.recv().unwrap(),
            42.0,
            "onMessage invoked + payload re-materialized cross-thread"
        );
        pool.shutdown();
        scheduler.shutdown();
    }

    #[test]
    fn cross_thread_routes_by_stable_key() {
        let mut scheduler = Scheduler::new(1);
        let pool = WorkerPool::new(1);
        let registered = 300u64;
        let (stx, srx) = std::sync::mpsc::channel::<()>();
        let w = scheduler.assign(registered);
        pool.submit(w, move || {
            build_compartment_on_worker(
                registered,
                &CompartmentDescriptor {
                    initial_globals: vec![],
                    boundary_policy: None,
                    timeout_ms: None,
                    onmessage_source: None,
                    module_loader_config: None,
                    host_call_tx: None,
                },
            );
            let _ = stx.send(());
        })
        .expect("setup");
        srx.recv().unwrap();
        let p1 = lower_global("globalThis.m = { v: 1 };", "m", None).expect("lower");
        assert!(
            cross_thread_send(&mut scheduler, &pool, registered, p1)
                .recv()
                .unwrap(),
            "registered stable key delivers"
        );
        let p2 = lower_global("globalThis.m = { v: 1 };", "m", None).expect("lower");
        assert!(
            !cross_thread_send(&mut scheduler, &pool, 999, p2)
                .recv()
                .unwrap(),
            "unregistered stable key does not deliver (routing is by key, not pointer)"
        );
        pool.shutdown();
        scheduler.shutdown();
    }

    #[test]
    fn generic_worker_message_refuses_callable_proxy_payload() {
        let mut scheduler = Scheduler::new(1);
        let pool = WorkerPool::new(1);
        let callable = SendIr::Callable {
            compartment_id: 1,
            binding_id: 2,
            name: "cap".into(),
            length: 0,
        };
        assert!(
            !cross_thread_send(&mut scheduler, &pool, 1, callable.clone())
                .recv()
                .unwrap(),
            "generic worker messages must not deliver callable-proxy handles"
        );
        let err = cross_thread_request(&mut scheduler, &pool, 1, callable.clone())
            .recv()
            .unwrap()
            .expect_err("generic request must reject callable-proxy handles");
        assert!(
            err.contains("callable proxy"),
            "rejection should name the disposition, got {err}"
        );
        let arena = std::sync::Arc::new(std::sync::Mutex::new(Tier2Arena::<String>::default()));
        assert!(
            !cross_thread_send_shared(&mut scheduler, &pool, 1, callable, arena)
                .recv()
                .unwrap(),
            "shared-arena worker messages must not deliver callable-proxy handles"
        );
        pool.shutdown();
        scheduler.shutdown();
    }

    #[test]
    fn cross_thread_phase_a_rejection_is_synchronous() {

        assert!(
            lower_global("globalThis.s = Symbol('x');", "s", None).is_err(),
            "Symbol rejected synchronously at lowering, before any cross-thread enqueue"
        );

        assert!(
            lower_global("globalThis.m = { cap: function(){} };", "m", None).is_err(),
            "a function capability is rejected at lowering (N11: no cross-thread bypass)"
        );
    }

    #[test]
    fn nm_shared_worker_two_compartments_isolated() {
        let mut scheduler = Scheduler::new(1);
        let pool = WorkerPool::new(1);
        let a = 400u64;
        let b = 401u64;
        let wa = scheduler.assign(a);
        let wb = scheduler.assign(b);
        assert_eq!(wa, wb, "single worker: both compartments share the thread");
        let (stx, srx) = std::sync::mpsc::channel::<()>();
        pool.submit(wa, move || {
            build_compartment_on_worker(
                a,
                &CompartmentDescriptor {
                    initial_globals: vec![],
                    boundary_policy: None,
                    timeout_ms: None,
                    onmessage_source: None,
                    module_loader_config: None,
                    host_call_tx: None,
                },
            );
            build_compartment_on_worker(
                b,
                &CompartmentDescriptor {
                    initial_globals: vec![],
                    boundary_policy: None,
                    timeout_ms: None,
                    onmessage_source: None,
                    module_loader_config: None,
                    host_call_tx: None,
                },
            );
            let _ = stx.send(());
        })
        .expect("setup");
        srx.recv().unwrap();

        let payload = lower_global("globalThis.m = { v: 7 };", "m", None).expect("lower");
        assert!(cross_thread_send(&mut scheduler, &pool, a, payload)
            .recv()
            .unwrap());

        let (rtx, rrx) = std::sync::mpsc::channel::<(bool, bool)>();
        pool.submit(wa, move || {
            let a_has = matches!(
                worker_read_compartment_global(a, "message"),
                Value::Object(_)
            );
            let b_has = matches!(
                worker_read_compartment_global(b, "message"),
                Value::Object(_)
            );
            let _ = rtx.send((a_has, b_has));
        })
        .expect("readback");
        let (a_has, b_has) = rrx.recv().unwrap();
        assert!(a_has, "compartment A received its message");
        assert!(
            !b_has,
            "compartment B realm is isolated (no cross-contamination)"
        );
        pool.shutdown();
        scheduler.shutdown();
    }

    #[test]
    fn same_worker_compartment_intrinsic_mutation_detaches() {
        let mut scheduler = Scheduler::new(1);
        let pool = WorkerPool::new(1);
        let a = 402u64;
        let b = 403u64;
        let worker = scheduler.assign(a);
        assert_eq!(worker, scheduler.assign(b));
        let (tx, rx) = std::sync::mpsc::channel::<(String, String)>();
        pool.submit(worker, move || {
            build_compartment_on_worker(
                a,
                &CompartmentDescriptor {
                    initial_globals: vec![],
                    boundary_policy: None,
                    timeout_ms: None,
                    onmessage_source: Some(
                        "function(){ String.prototype.__cowMark = 'a'; Object.defineProperty(String.prototype, '__cowDefine', { value: 'd', configurable: true }); return ''.__cowMark + ':' + ''.__cowDefine; }"
                            .to_string(),
                    ),
                    module_loader_config: None,
                    host_call_tx: None,
                },
            );
            build_compartment_on_worker(
                b,
                &CompartmentDescriptor {
                    initial_globals: vec![],
                    boundary_policy: None,
                    timeout_ms: None,
                    onmessage_source: Some(
                        "function(){ return typeof ''.__cowMark + ':' + typeof ''.__cowDefine; }"
                            .to_string(),
                    ),
                    module_loader_config: None,
                    host_call_tx: None,
                },
            );
            let to_owned_string = |ir: SendIr| match ir {
                SendIr::Str(crate::send_ir::SendStr::Owned(s)) => s,
                other => panic!("expected owned string reply, got {other:?}"),
            };
            let a_ret =
                to_owned_string(request_from_compartment(a, &SendIr::Undefined).expect("A request"));
            let b_ret =
                to_owned_string(request_from_compartment(b, &SendIr::Undefined).expect("B request"));
            let _ = tx.send((a_ret, b_ret));
        })
        .expect("setup");
        let (a_ret, b_ret) = rx.recv().unwrap();
        assert_eq!(a_ret, "a:d");
        assert_eq!(
            b_ret, "undefined:undefined",
            "same-worker compartment B must not see A's String.prototype mutations"
        );
        pool.shutdown();
        scheduler.shutdown();
    }

    #[test]
    fn same_worker_compartment_intrinsic_prototype_mutation_detaches() {
        let mut scheduler = Scheduler::new(1);
        let pool = WorkerPool::new(1);
        let a = 404u64;
        let b = 405u64;
        let worker = scheduler.assign(a);
        assert_eq!(worker, scheduler.assign(b));
        let (tx, rx) = std::sync::mpsc::channel::<(String, String)>();
        pool.submit(worker, move || {
            build_compartment_on_worker(
                a,
                &CompartmentDescriptor {
                    initial_globals: vec![],
                    boundary_policy: None,
                    timeout_ms: None,
                    onmessage_source: Some(
                        r#"function(){
                            const viaReflect = { __cowProtoReflect: 'r' };
                            const viaObject = { __cowProtoObject: 'o' };
                            const viaProto = { __cowProtoSetter: 'p' };
                            const ok = Reflect.setPrototypeOf(String.prototype, viaReflect);
                            const objectRetIsFreshLookup =
                                Object.setPrototypeOf(Array.prototype, viaObject) === Array.prototype;
                            Number.prototype.__proto__ = viaProto;
                            return ok + ':' + objectRetIsFreshLookup + ':' +
                                ''.__cowProtoReflect + ':' +
                                [1].__cowProtoObject + ':' +
                                (1).__cowProtoSetter;
                        }"#
                            .to_string(),
                    ),
                    module_loader_config: None,
                    host_call_tx: None,
                },
            );
            build_compartment_on_worker(
                b,
                &CompartmentDescriptor {
                    initial_globals: vec![],
                    boundary_policy: None,
                    timeout_ms: None,
                    onmessage_source: Some(
                        r#"function(){
                            return typeof ''.__cowProtoReflect + ':' +
                                typeof [1].__cowProtoObject + ':' +
                                typeof (1).__cowProtoSetter;
                        }"#
                            .to_string(),
                    ),
                    module_loader_config: None,
                    host_call_tx: None,
                },
            );
            let to_owned_string = |ir: SendIr| match ir {
                SendIr::Str(crate::send_ir::SendStr::Owned(s)) => s,
                other => panic!("expected owned string reply, got {other:?}"),
            };
            let a_ret =
                to_owned_string(request_from_compartment(a, &SendIr::Undefined).expect("A request"));
            let b_ret =
                to_owned_string(request_from_compartment(b, &SendIr::Undefined).expect("B request"));
            let _ = tx.send((a_ret, b_ret));
        })
        .expect("setup");
        let (a_ret, b_ret) = rx.recv().unwrap();

        assert_eq!(a_ret, "true:false:r:o:p");
        assert_eq!(
            b_ret, "undefined:undefined:undefined",
            "same-worker compartment B must not see A's intrinsic prototype-link mutations"
        );
        pool.shutdown();
        scheduler.shutdown();
    }

    #[test]
    fn same_worker_compartment_namespace_intrinsic_mutation_detaches() {
        let mut scheduler = Scheduler::new(1);
        let pool = WorkerPool::new(1);
        let a = 406u64;
        let b = 407u64;
        let worker = scheduler.assign(a);
        assert_eq!(worker, scheduler.assign(b));
        let (tx, rx) = std::sync::mpsc::channel::<(String, String)>();
        pool.submit(worker, move || {
            build_compartment_on_worker(
                a,
                &CompartmentDescriptor {
                    initial_globals: vec![],
                    boundary_policy: None,
                    timeout_ms: None,
                    onmessage_source: Some(
                        r#"function(){
                            Math.__cowNsMath = 'm';
                            JSON.__cowNsJSON = 'j';
                            Reflect.__cowNsReflect = 'r';
                            Atomics.__cowNsAtomics = 'a';
                            return Math.__cowNsMath + ':' +
                                JSON.__cowNsJSON + ':' +
                                Reflect.__cowNsReflect + ':' +
                                Atomics.__cowNsAtomics;
                        }"#
                        .to_string(),
                    ),
                    module_loader_config: None,
                    host_call_tx: None,
                },
            );
            build_compartment_on_worker(
                b,
                &CompartmentDescriptor {
                    initial_globals: vec![],
                    boundary_policy: None,
                    timeout_ms: None,
                    onmessage_source: Some(
                        r#"function(){
                            return typeof Math.__cowNsMath + ':' +
                                typeof JSON.__cowNsJSON + ':' +
                                typeof Reflect.__cowNsReflect + ':' +
                                typeof Atomics.__cowNsAtomics;
                        }"#
                        .to_string(),
                    ),
                    module_loader_config: None,
                    host_call_tx: None,
                },
            );
            let to_owned_string = |ir: SendIr| match ir {
                SendIr::Str(crate::send_ir::SendStr::Owned(s)) => s,
                other => panic!("expected owned string reply, got {other:?}"),
            };
            let a_ret = to_owned_string(
                request_from_compartment(a, &SendIr::Undefined).expect("A request"),
            );
            let b_ret = to_owned_string(
                request_from_compartment(b, &SendIr::Undefined).expect("B request"),
            );
            let _ = tx.send((a_ret, b_ret));
        })
        .expect("setup");
        let (a_ret, b_ret) = rx.recv().unwrap();
        assert_eq!(a_ret, "m:j:r:a");
        assert_eq!(b_ret, "undefined:undefined:undefined:undefined");
        pool.shutdown();
        scheduler.shutdown();
    }

    #[test]
    fn same_worker_compartment_global_function_mutation_detaches() {
        let mut scheduler = Scheduler::new(1);
        let pool = WorkerPool::new(1);
        let a = 408u64;
        let b = 409u64;
        let worker = scheduler.assign(a);
        assert_eq!(worker, scheduler.assign(b));
        let (tx, rx) = std::sync::mpsc::channel::<(String, String)>();
        pool.submit(worker, move || {
            build_compartment_on_worker(
                a,
                &CompartmentDescriptor {
                    initial_globals: vec![],
                    boundary_policy: None,
                    timeout_ms: None,
                    onmessage_source: Some(
                        r#"function(){
                            eval.__cowGlobalEval = 'eval';
                            parseInt.__cowGlobalParseInt = 'parseInt';
                            decodeURIComponent.__cowGlobalDecodeURIComponent = 'decodeURIComponent';
                            __destr_array_rest.__cowGlobalDestrArrayRest = 'destr';
                            __super_get_base.__cowGlobalSuperGetBase = 'super';
                            return eval.__cowGlobalEval + ':' +
                                parseInt.__cowGlobalParseInt + ':' +
                                decodeURIComponent.__cowGlobalDecodeURIComponent + ':' +
                                __destr_array_rest.__cowGlobalDestrArrayRest + ':' +
                                __super_get_base.__cowGlobalSuperGetBase;
                        }"#
                        .to_string(),
                    ),
                    module_loader_config: None,
                    host_call_tx: None,
                },
            );
            build_compartment_on_worker(
                b,
                &CompartmentDescriptor {
                    initial_globals: vec![],
                    boundary_policy: None,
                    timeout_ms: None,
                    onmessage_source: Some(
                        r#"function(){
                            return typeof eval.__cowGlobalEval + ':' +
                                typeof parseInt.__cowGlobalParseInt + ':' +
                                typeof decodeURIComponent.__cowGlobalDecodeURIComponent + ':' +
                                typeof __destr_array_rest.__cowGlobalDestrArrayRest + ':' +
                                typeof __super_get_base.__cowGlobalSuperGetBase;
                        }"#
                        .to_string(),
                    ),
                    module_loader_config: None,
                    host_call_tx: None,
                },
            );
            let to_owned_string = |ir: SendIr| match ir {
                SendIr::Str(crate::send_ir::SendStr::Owned(s)) => s,
                other => panic!("expected owned string reply, got {other:?}"),
            };
            let a_ret = to_owned_string(
                request_from_compartment(a, &SendIr::Undefined).expect("A request"),
            );
            let b_ret = to_owned_string(
                request_from_compartment(b, &SendIr::Undefined).expect("B request"),
            );
            let _ = tx.send((a_ret, b_ret));
        })
        .expect("setup");
        let (a_ret, b_ret) = rx.recv().unwrap();
        assert_eq!(a_ret, "eval:parseInt:decodeURIComponent:destr:super");
        assert_eq!(
            b_ret, "undefined:undefined:undefined:undefined:undefined",
            "same-worker compartment B must not see A's global function/helper mutations"
        );
        pool.shutdown();
        scheduler.shutdown();
    }

    #[test]
    fn cross_thread_shared_string_arena_round_trip() {
        run_test_on_large_stack(|| {
            use std::sync::{Arc, Mutex};
            let mut scheduler = Scheduler::new(2);
            let pool = WorkerPool::new(2);
            let cid = 600u64;
            let worker = scheduler.assign(cid);

            let (stx, srx) = std::sync::mpsc::channel::<()>();
            pool.submit(worker, move || {
                build_compartment_on_worker(
                    cid,
                    &CompartmentDescriptor {
                        initial_globals: vec![],
                        boundary_policy: None,
                        timeout_ms: None,
                        onmessage_source: Some(
                            "function(e){ globalThis.got = e.data.s; }".to_string(),
                        ),
                        module_loader_config: None,
                        host_call_tx: None,
                    },
                );
                let _ = stx.send(());
            })
            .expect("setup");
            srx.recv().unwrap();

            let arena: Arc<Mutex<rusty_js_gc::Tier2Arena<String>>> =
                Arc::new(Mutex::new(rusty_js_gc::Tier2Arena::default()));

            let payload = {
                let mut sender = Runtime::new();
                sender.install_intrinsics();
                sender
                    .evaluate_script(r#"globalThis.m = { s: "shared-xyz" };"#, "file://s")
                    .unwrap();
                let gt = sender.global_object.unwrap();
                let mv = sender.object_get(gt, "m");
                lower_with_shared_arena(&sender, &mv, &arena).expect("lower shared")
            };
            if let SendIr::Composite { props, .. } = &payload {
                assert!(
                    matches!(
                        props.iter().find(|(k, _)| k == "s").map(|(_, v)| v),
                        Some(SendIr::Str(crate::send_ir::SendStr::Shared(_)))
                    ),
                    "string must lower to a Tier-2 Shared handle in the shared arena"
                );
            } else {
                panic!("expected Composite");
            }

            let rx = cross_thread_send_shared(&mut scheduler, &pool, cid, payload, arena.clone());
            assert!(rx.recv().unwrap(), "delivered cross-thread");

            let (rtx, rrx) = std::sync::mpsc::channel::<String>();
            pool.submit(worker, move || {
                let got = match worker_read_compartment_global(cid, "got") {
                    Value::String(s) => s.as_str().to_string(),
                    _ => String::new(),
                };
                let _ = rtx.send(got);
            })
            .expect("readback");
            assert_eq!(
                rrx.recv().unwrap(),
                "shared-xyz",
                "Shared string re-materialized cross-thread from the SAME arena (heap-unification)"
            );
            pool.shutdown();
            scheduler.shutdown();
        });
    }

    #[test]
    fn gc_cross_realm_x1_shared_value_survives_sender_collection() {
        std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(gc_cross_realm_x1_shared_value_survives_sender_collection_impl)
            .expect("spawn large-stack X1 probe")
            .join()
            .expect("X1 probe completed");
    }

    fn gc_cross_realm_x1_shared_value_survives_sender_collection_impl() {
        use std::sync::{Arc, Mutex};
        let cid = 601u64;
        build_compartment_on_worker(
            cid,
            &CompartmentDescriptor {
                initial_globals: vec![],
                boundary_policy: None,
                timeout_ms: None,
                onmessage_source: Some("function(e){ globalThis.got = e.data.s; }".to_string()),
                module_loader_config: None,
                host_call_tx: None,
            },
        );

        let arena: Arc<Mutex<rusty_js_gc::Tier2Arena<String>>> =
            Arc::new(Mutex::new(rusty_js_gc::Tier2Arena::default()));
        let payload = {
            let mut sender = Runtime::new();
            sender.install_intrinsics();
            sender.gc_stress = true;
            sender
                .evaluate_script(
                    r#"globalThis.m = { s: "x1-shared-survives" };"#,
                    "file://f3-x1-sender",
                )
                .unwrap();
            let gt = sender.global_object.unwrap();
            let mv = sender.object_get(gt, "m");
            let payload = lower_with_shared_arena(&sender, &mv, &arena).expect("lower shared");
            sender.collect();
            payload
        };

        let guard = arena.lock().expect("shared string arena lock");
        assert!(
            deliver_to_compartment(cid, &payload, Some(&guard)),
            "delivered into worker-hosted compartment"
        );
        let got = match worker_read_compartment_global(cid, "got") {
            Value::String(s) => s.as_str().to_string(),
            _ => String::new(),
        };
        assert_eq!(
            got, "x1-shared-survives",
            "X1: B's mediated Tier-2 value survives collection of A"
        );
    }

    #[test]
    fn gc_cross_realm_x2_shared_value_survives_concurrent_collection() {
        std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(gc_cross_realm_x2_shared_value_survives_concurrent_collection_impl)
            .expect("spawn large-stack X2 probe")
            .join()
            .expect("X2 probe completed");
    }

    fn gc_cross_realm_x2_shared_value_survives_concurrent_collection_impl() {
        use std::sync::{Arc, Barrier, Mutex};

        let arena: Arc<Mutex<rusty_js_gc::Tier2Arena<String>>> =
            Arc::new(Mutex::new(rusty_js_gc::Tier2Arena::default()));
        let payload = {
            let mut sender = Runtime::new();
            sender.install_intrinsics();
            sender
                .evaluate_script(
                    r#"globalThis.m = { s: "x2-shared-concurrent" };"#,
                    "file://f3-x2-sender",
                )
                .unwrap();
            let gt = sender.global_object.unwrap();
            let mv = sender.object_get(gt, "m");
            lower_with_shared_arena(&sender, &mv, &arena).expect("lower shared")
        };

        let barrier = Arc::new(Barrier::new(2));
        let mut handles = Vec::new();
        for cid in [602u64, 603u64] {
            let payload = payload.clone();
            let arena = arena.clone();
            let barrier = barrier.clone();
            handles.push(
                std::thread::Builder::new()
                    .stack_size(32 * 1024 * 1024)
                    .spawn(move || {
                        build_compartment_on_worker(
                            cid,
                            &CompartmentDescriptor {
                                initial_globals: vec![],
                                boundary_policy: None,
                                timeout_ms: None,
                                onmessage_source: Some(
                                    "function(e){ globalThis.got = e.data.s; }".to_string(),
                                ),
                                module_loader_config: None,
                                host_call_tx: None,
                            },
                        );

                        let guard = arena.lock().expect("shared string arena lock");
                        assert!(
                            deliver_to_compartment(cid, &payload, Some(&guard)),
                            "delivered into worker-hosted compartment"
                        );
                        drop(guard);

                        barrier.wait();
                        with_worker_runtime(|rt| {
                            rt.gc_stress = true;
                            rt.collect();
                        })
                        .expect("worker runtime present");

                        match worker_read_compartment_global(cid, "got") {
                            Value::String(s) => s.as_str().to_string(),
                            _ => String::new(),
                        }
                    })
                    .expect("spawn large-stack X2 worker"),
            );
        }

        for handle in handles {
            assert_eq!(
                handle.join().expect("X2 worker completed"),
                "x2-shared-concurrent",
                "X2: mediated Tier-2 value survives concurrent per-realm collection"
            );
        }
    }

    fn shared_handles_in_send_ir(ir: &SendIr, out: &mut Vec<rusty_js_gc::Tier2Handle>) {
        match ir {
            SendIr::Str(crate::send_ir::SendStr::Shared(h)) => out.push(*h),
            SendIr::Composite { props, .. } => {
                for (_, child) in props {
                    shared_handles_in_send_ir(child, out);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn gc_cross_realm_x3_shared_value_survives_sender_teardown_until_receiver_release() {
        std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(
                gc_cross_realm_x3_shared_value_survives_sender_teardown_until_receiver_release_impl,
            )
            .expect("spawn large-stack X3 probe")
            .join()
            .expect("X3 probe completed");
    }

    fn gc_cross_realm_x3_shared_value_survives_sender_teardown_until_receiver_release_impl() {
        use std::sync::{Arc, Mutex};

        let cid = 604u64;
        build_compartment_on_worker(
            cid,
            &CompartmentDescriptor {
                initial_globals: vec![],
                boundary_policy: None,
                timeout_ms: None,
                onmessage_source: Some("function(e){ globalThis.got = e.data.s; }".to_string()),
                module_loader_config: None,
                host_call_tx: None,
            },
        );

        let arena: Arc<Mutex<rusty_js_gc::Tier2Arena<String>>> =
            Arc::new(Mutex::new(rusty_js_gc::Tier2Arena::default()));
        let payload = {
            let mut sender = Runtime::new();
            sender.install_intrinsics();
            sender.gc_stress = true;
            sender
                .evaluate_script(
                    r#"globalThis.m = { s: "x3-teardown-survives" };"#,
                    "file://f3-x3-sender",
                )
                .unwrap();
            let gt = sender.global_object.unwrap();
            let mv = sender.object_get(gt, "m");
            lower_with_shared_arena(&sender, &mv, &arena).expect("lower shared")
        };

        let mut handles = Vec::new();
        shared_handles_in_send_ir(&payload, &mut handles);
        assert_eq!(
            handles.len(),
            1,
            "X3 fixture should carry exactly one shared Tier-2 handle"
        );

        {
            let mut guard = arena.lock().expect("shared string arena lock");
            for h in &handles {
                assert_eq!(guard.refcount(*h), Some(1), "A starts as the sole owner");
                guard.incref(*h);
                assert_eq!(
                    guard.refcount(*h),
                    Some(2),
                    "B's crossed clone owns a Tier-2 ref before A tears down"
                );
                assert!(!guard.decref(*h), "A teardown leaves B's hold live");
                assert_eq!(guard.refcount(*h), Some(1));
            }
            guard.advance_epoch();
            guard.advance_epoch();
            assert_eq!(
                guard.retire(),
                0,
                "B's live hold prevents retirement while A is gone"
            );
        }

        {
            let guard = arena.lock().expect("shared string arena lock");
            assert!(
                deliver_to_compartment(cid, &payload, Some(&guard)),
                "delivered into worker-hosted compartment after A teardown"
            );
        }

        with_worker_runtime(|rt| {
            rt.gc_stress = true;
            rt.collect();
        })
        .expect("worker runtime present");

        let got = match worker_read_compartment_global(cid, "got") {
            Value::String(s) => s.as_str().to_string(),
            _ => String::new(),
        };
        assert_eq!(
            got, "x3-teardown-survives",
            "X3: B observes the mediated value after A teardown and B collection"
        );

        {
            let mut guard = arena.lock().expect("shared string arena lock");
            for h in &handles {
                assert!(guard.decref(*h), "B release retires the final hold");
            }
            assert_eq!(guard.retire(), 0, "B release is still inside epoch grace");
            guard.advance_epoch();
            assert_eq!(guard.retire(), 0, "one epoch is still inside grace");
            guard.advance_epoch();
            assert_eq!(guard.retire(), 1, "two-epoch grace frees the shared value");
            assert_eq!(guard.live_count(), 0);
        }

        let got_after_free = match worker_read_compartment_global(cid, "got") {
            Value::String(s) => s.as_str().to_string(),
            _ => String::new(),
        };
        assert_eq!(
            got_after_free, "x3-teardown-survives",
            "B's rematerialized local value remains valid after releasing the Tier-2 hold"
        );
    }

    #[test]
    fn gc_cross_realm_x4_closure_message_transfer_rejects_fail_closed() {
        std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(gc_cross_realm_x4_closure_message_transfer_rejects_fail_closed_impl)
            .expect("spawn large-stack X4 probe")
            .join()
            .expect("X4 probe completed");
    }

    fn gc_cross_realm_x4_closure_message_transfer_rejects_fail_closed_impl() {
        let mut sender = Runtime::new();
        sender.install_intrinsics();
        sender.gc_stress = true;
        sender
            .evaluate_script(
                r#"
                const secret = 41;
                globalThis.payload = { f: function addOne() { return secret + 1; } };
                "#,
                "file://f3-x4-sender",
            )
            .expect("sender script");
        let gt = sender.global_object.unwrap();
        let payload = sender.object_get(gt, "payload");
        let mut ctx = LowerCtx::new(None);
        let err = lower_to_send_ir(&sender, &payload, &mut ctx)
            .expect_err("closure payload must not cross the message boundary");
        sender.collect();
        assert!(
            matches!(err, crate::RuntimeError::TypeError(ref m) if m.contains("function values are not transferable")),
            "X4: closure sharing over ordinary message boundary must fail closed, got {err:?}"
        );
    }

    #[test]
    fn f2_worker_callable_registry_roots_proxy_target_across_gc() {
        std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(f2_worker_callable_registry_roots_proxy_target_across_gc_impl)
            .expect("spawn large-stack F2 worker callable probe")
            .join()
            .expect("F2 worker callable probe completed");
    }

    fn f2_worker_callable_registry_roots_proxy_target_across_gc_impl() {
        let cid = 811u64;
        ensure_worker_rt();
        let (binding_id, callable_id) = with_worker_runtime(|rt| {
            rt.gc_stress = true;
            let callable = crate::intrinsics::make_native("f2Callable", |_rt, args| {
                Ok(Value::String(std::rc::Rc::new(
                    crate::value::JsString::from(format!("f2-callable-ok:{}", args.len())),
                )))
            });
            let callable_id = rt.alloc_object(callable);
            let binding_id = WORKER_CALLABLES.with(|m| {
                let mut map = m.borrow_mut();
                let reg = map
                    .entry(cid)
                    .or_insert_with(|| crate::send_ir::CallableRegistry::new(cid));
                reg.register(callable_id)
            });
            (binding_id, callable_id)
        })
        .expect("worker runtime present");

        with_worker_runtime(|rt| {
            rt.collect();
            assert!(
                !rt.heap.is_free(callable_id),
                "CallableRegistry root keeps a proxy-target callable live across worker GC"
            );
        })
        .expect("worker runtime present after registration");

        let result = with_worker_runtime(|rt| {
            let callable =
                WORKER_CALLABLES.with(|m| m.borrow().get(&cid).and_then(|reg| reg.get(binding_id)));
            let callable = callable.expect("registered callable still resolves");
            let ret = rt
                .call_function(
                    Value::Object(callable),
                    Value::Undefined,
                    vec![Value::Number(7.0), Value::Boolean(true)],
                )
                .expect("invoke callable after worker GC");
            WORKER_CALLABLES.with(|m| {
                let mut map = m.borrow_mut();
                let reg = map
                    .entry(cid)
                    .or_insert_with(|| crate::send_ir::CallableRegistry::new(cid));
                let mut ctx = crate::send_ir::LowerCtx::with_callables(None, reg);
                crate::send_ir::lower_to_send_ir(rt, &ret, &mut ctx)
            })
        })
        .expect("worker runtime present for invoke")
        .expect("lower invoke result");

        match result {
            SendIr::Str(crate::send_ir::SendStr::Owned(s)) => {
                assert_eq!(s, "f2-callable-ok:2");
            }
            other => panic!("expected owned string result, got {other:?}"),
        }
    }

    #[test]
    fn worker_root_provider_is_registered_on_worker_runtime_only() {
        std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(worker_root_provider_is_registered_on_worker_runtime_only_impl)
            .expect("spawn large-stack worker root-provider probe")
            .join()
            .expect("worker root-provider probe completed");
    }

    fn worker_root_provider_is_registered_on_worker_runtime_only_impl() {
        let cid = 812u64;
        let default_rt = Runtime::new();
        assert!(
            default_rt.host_root_sources.is_empty(),
            "ordinary runtimes should not globally enumerate worker thread-local roots"
        );

        build_compartment_on_worker(
            cid,
            &CompartmentDescriptor {
                initial_globals: vec![],
                boundary_policy: None,
                timeout_ms: None,
                onmessage_source: Some("function(e){ globalThis.seen = e.data; }".into()),
                module_loader_config: None,
                host_call_tx: None,
            },
        );
        let (global_this, handler) = WORKER_COMPARTMENTS.with(|m| {
            let map = m.borrow();
            let entry = map.get(&cid).expect("worker compartment registered");
            (entry.global_this, entry.onmessage.clone())
        });

        with_worker_runtime(|rt| {
            assert!(
                !rt.host_root_sources.is_empty(),
                "worker runtime registers its compartment/callable root provider"
            );
            let roots = rt.enumerate_roots();
            assert!(
                roots.contains(&global_this),
                "worker compartment global is rooted by owner worker root provider"
            );
            if let Value::Object(handler_id) = handler {
                assert!(
                    roots.contains(&handler_id),
                    "worker onMessage handler is rooted by owner worker root provider"
                );
            } else {
                panic!("expected compiled onMessage handler object");
            }
        })
        .expect("worker runtime present for root enumeration");

        teardown_worker_compartment(cid);
    }

    #[test]
    fn worker_teardown_releases_compartment_roots() {
        std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(worker_teardown_releases_compartment_roots_impl)
            .expect("spawn large-stack worker teardown root-release probe")
            .join()
            .expect("worker teardown root-release probe completed");
    }

    fn worker_teardown_releases_compartment_roots_impl() {
        let live_cid = 813u64;
        let dead_cid = 814u64;
        for (cid, tag) in [(live_cid, "live"), (dead_cid, "dead")] {
            build_compartment_on_worker(
                cid,
                &CompartmentDescriptor {
                    initial_globals: vec![],
                    boundary_policy: None,
                    timeout_ms: None,
                    onmessage_source: Some(format!(
                        "function(e){{ globalThis.kind = '{}'; globalThis.seen = e.data; }}",
                        tag
                    )),
                    module_loader_config: None,
                    host_call_tx: None,
                },
            );
        }

        let (live_global, live_handler, dead_global, dead_handler) =
            WORKER_COMPARTMENTS.with(|m| {
                let map = m.borrow();
                let live = map.get(&live_cid).expect("live compartment registered");
                let dead = map.get(&dead_cid).expect("dead compartment registered");
                (
                    live.global_this,
                    live.onmessage.clone(),
                    dead.global_this,
                    dead.onmessage.clone(),
                )
            });
        let dead_handler = match dead_handler {
            Value::Object(id) => id,
            _ => panic!("expected dead compartment onMessage object"),
        };
        let live_handler = match live_handler {
            Value::Object(id) => id,
            _ => panic!("expected live compartment onMessage object"),
        };

        with_worker_runtime(|rt| {
            let roots = rt.enumerate_roots();
            assert!(roots.contains(&live_global));
            assert!(roots.contains(&live_handler));
            assert!(roots.contains(&dead_global));
            assert!(roots.contains(&dead_handler));
        })
        .expect("worker runtime present before teardown");

        assert!(
            teardown_worker_compartment(dead_cid),
            "worker teardown removes the dead compartment registry entry"
        );

        with_worker_runtime(|rt| {
            let roots = rt.enumerate_roots();
            assert!(
                roots.contains(&live_global),
                "live compartment global remains rooted"
            );
            assert!(
                roots.contains(&live_handler),
                "live compartment handler remains rooted"
            );
            assert!(
                !roots.contains(&dead_global),
                "teardown removes dead compartment global from roots"
            );
            assert!(
                !roots.contains(&dead_handler),
                "teardown removes dead compartment handler from roots"
            );
            rt.collect();
            assert!(
                rt.heap.get(live_global).is_some(),
                "live compartment global survives owner runtime collection"
            );
            assert!(
                rt.heap.get(live_handler).is_some(),
                "live compartment handler survives owner runtime collection"
            );
            assert!(
                rt.heap.get(dead_global).is_none(),
                "dead compartment global is collectable after teardown"
            );
            assert!(
                rt.heap.get(dead_handler).is_none(),
                "dead compartment handler is collectable after teardown"
            );
        })
        .expect("worker runtime present after teardown");

        teardown_worker_compartment(live_cid);
    }

    #[test]
    fn stress_nm_compartments_concurrent_message_storm() {
        const M: usize = 4;
        const N: u64 = 32;
        const K: usize = 50;

        let mut scheduler = Scheduler::new(M);
        let pool = WorkerPool::new(M);

        let mut workers = Vec::new();
        for i in 0..N {
            let cid = 1000 + i;
            let w = scheduler.assign(cid);
            workers.push((cid, w));
            let (stx, srx) = std::sync::mpsc::channel::<()>();
            pool.submit(w, move || {
                build_compartment_on_worker(
                    cid,
                    &CompartmentDescriptor {
                        initial_globals: vec![],
                        boundary_policy: None,
                        timeout_ms: None,
                        onmessage_source: Some(
                            "function(e){ \
                               globalThis.sum = (globalThis.sum || 0) + e.data.v; \
                               globalThis.count = (globalThis.count || 0) + 1; }"
                                .to_string(),
                        ),
                        module_loader_config: None,
                        host_call_tx: None,
                    },
                );
                let _ = stx.send(());
            })
            .expect("setup submit");
            srx.recv().unwrap();
        }

        let mut receipts = Vec::with_capacity((N as usize) * K);
        for round in 0..K {
            for i in 0..N {
                let cid = 1000 + i;
                let v = (i + 1) as i64;
                let src = format!("globalThis.m = {{ v: {} }};", v);
                let payload = lower_global(&src, "m", None).expect("lower");
                let rx = cross_thread_send(&mut scheduler, &pool, cid, payload);
                receipts.push((cid, round, rx));
            }
        }

        for (cid, round, rx) in receipts {
            assert!(
                rx.recv().unwrap(),
                "delivery receipt for compartment {} round {}",
                cid,
                round
            );
        }

        for (cid, w) in workers {
            let i = cid - 1000;
            let v = (i + 1) as f64;
            let (rtx, rrx) = std::sync::mpsc::channel::<(f64, f64)>();
            pool.submit(w, move || {
                let sum = match worker_read_compartment_global(cid, "sum") {
                    Value::Number(n) => n,
                    _ => -1.0,
                };
                let count = match worker_read_compartment_global(cid, "count") {
                    Value::Number(n) => n,
                    _ => -1.0,
                };
                let _ = rtx.send((sum, count));
            })
            .expect("readback submit");
            let (sum, count) = rrx.recv().unwrap();
            assert_eq!(
                count, K as f64,
                "compartment {} received EXACTLY K messages (no loss/dup under concurrency)",
                cid
            );
            assert_eq!(sum, K as f64 * v,
                "compartment {} sum is its OWN v={} accumulated K times (isolation + no torn writes)", cid, v);
        }
        pool.shutdown();
        scheduler.shutdown();
    }

    #[test]
    fn gc_cross_realm_x5_nm_compartment_message_storm_under_gc_stress() {
        std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(gc_cross_realm_x5_nm_compartment_message_storm_under_gc_stress_impl)
            .expect("spawn large-stack X5 probe")
            .join()
            .expect("X5 probe completed");
    }

    fn gc_cross_realm_x5_nm_compartment_message_storm_under_gc_stress_impl() {
        use std::sync::{Arc, Barrier, Mutex};

        const N: usize = 16;
        const K: usize = 20;

        let arena: Arc<Mutex<rusty_js_gc::Tier2Arena<String>>> =
            Arc::new(Mutex::new(rusty_js_gc::Tier2Arena::default()));
        let mut payloads = Vec::with_capacity(N);
        for i in 0..N {
            let mut sender = Runtime::new();
            sender.install_intrinsics();
            sender.gc_stress = true;
            sender
                .evaluate_script(
                    &format!(r#"globalThis.m = {{ v: {}, s: "tenant-{}" }};"#, i + 1, i),
                    &format!("file://f3-x5-sender-{i}"),
                )
                .unwrap();
            let gt = sender.global_object.unwrap();
            let mv = sender.object_get(gt, "m");
            payloads.push(
                lower_with_shared_arena(&sender, &mv, &arena).expect("lower shared X5 payload"),
            );
            sender.collect();
        }

        let barrier = Arc::new(Barrier::new(N));
        let mut handles = Vec::with_capacity(N);
        for (i, payload) in payloads.into_iter().enumerate() {
            let cid = 700 + i as u64;
            let arena = arena.clone();
            let barrier = barrier.clone();
            handles.push(
                std::thread::Builder::new()
                    .stack_size(32 * 1024 * 1024)
                    .spawn(move || {
                        build_compartment_on_worker(
                            cid,
                            &CompartmentDescriptor {
                                initial_globals: vec![],
                                boundary_policy: None,
                                timeout_ms: None,
                                onmessage_source: Some(
                                    "function(e){ \
                                       globalThis.sum = (globalThis.sum || 0) + e.data.v; \
                                       globalThis.count = (globalThis.count || 0) + 1; \
                                       globalThis.tag = e.data.s; }"
                                        .to_string(),
                                ),
                                module_loader_config: None,
                                host_call_tx: None,
                            },
                        );

                        barrier.wait();
                        for round in 0..K {
                            let guard = arena.lock().expect("shared string arena lock");
                            assert!(
                                deliver_to_compartment(cid, &payload, Some(&guard)),
                                "X5 delivery into compartment {cid} round {round}"
                            );
                            drop(guard);

                            if round % 5 == 0 {
                                with_worker_runtime(|rt| {
                                    rt.gc_stress = true;
                                    rt.collect();
                                })
                                .expect("worker runtime present");
                            }
                        }
                        with_worker_runtime(|rt| {
                            rt.gc_stress = true;
                            rt.collect();
                        })
                        .expect("worker runtime present");

                        let sum = match worker_read_compartment_global(cid, "sum") {
                            Value::Number(n) => n,
                            _ => -1.0,
                        };
                        let count = match worker_read_compartment_global(cid, "count") {
                            Value::Number(n) => n,
                            _ => -1.0,
                        };
                        let tag = match worker_read_compartment_global(cid, "tag") {
                            Value::String(s) => s.as_str().to_string(),
                            _ => String::new(),
                        };
                        (i, sum, count, tag)
                    })
                    .expect("spawn large-stack X5 worker"),
            );
        }

        for handle in handles {
            let (i, sum, count, tag) = handle.join().expect("X5 worker completed");
            assert_eq!(count, K as f64, "X5 tenant {i} received every message");
            assert_eq!(
                sum,
                K as f64 * (i + 1) as f64,
                "X5 tenant {i} accumulated only its own numeric payload"
            );
            assert_eq!(
                tag,
                format!("tenant-{i}"),
                "X5 tenant {i} retained its mediated shared string payload"
            );
        }
    }

    #[cfg(test)]
    fn vm_rss_kb() -> u64 {
        std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with("VmRSS:"))
                    .map(|l| l.to_string())
            })
            .and_then(|l| l.split_whitespace().nth(1).and_then(|n| n.parse().ok()))
            .unwrap_or(0)
    }

    #[test]
    #[ignore]
    fn bench_context_cost() {
        const M: usize = 8;
        const K: u64 = 256;
        let pool = WorkerPool::new(M);

        let (wtx, wrx) = std::sync::mpsc::channel::<()>();
        for w in 0..M {
            let tx = wtx.clone();
            pool.submit(w, move || {
                ensure_worker_rt();
                let _ = tx.send(());
            })
            .unwrap();
        }
        for _ in 0..M {
            wrx.recv().unwrap();
        }
        let rss_base = vm_rss_kb();

        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let t0 = std::time::Instant::now();
        for i in 0..K {
            let cid = 5_000 + i;
            let w = (i as usize) % M;
            let tx = tx.clone();
            pool.submit(w, move || {
                build_compartment_on_worker(
                    cid,
                    &CompartmentDescriptor {
                        initial_globals: vec![],
                        boundary_policy: None,
                        timeout_ms: None,
                        onmessage_source: None,
                        module_loader_config: None,
                        host_call_tx: None,
                    },
                );
                let _ = tx.send(());
            })
            .unwrap();
        }
        for _ in 0..K {
            rx.recv().unwrap();
        }
        let dt = t0.elapsed();
        let rss_live = vm_rss_kb();

        eprintln!("\n=== CRUFT context cost (one engine, {} threads) ===", M);
        eprintln!("contexts (Compartments): {}", K);
        eprintln!(
            "creation time:           {:.3} ms total  ({:.1} us/context)",
            dt.as_secs_f64() * 1e3,
            dt.as_secs_f64() * 1e6 / K as f64
        );
        eprintln!("RSS base ({} engines):    {} KB", M, rss_base);
        eprintln!(
            "RSS with {} contexts live: {} KB  (+{} KB, {:.1} KB/context)",
            K,
            rss_live,
            rss_live.saturating_sub(rss_base),
            rss_live.saturating_sub(rss_base) as f64 / K as f64
        );
        pool.shutdown();
    }

    #[test]
    #[ignore]
    fn bench_compute_compartments() {
        const M: usize = 8;
        const ITERS: u64 = 10_000_000;
        let js = format!(
            "globalThis.__r = (function(){{ let s=0; for(let i=0;i<{};i++){{ s=(s+i*3+7)%1000003; }} return s; }})();",
            ITERS);
        let pool = WorkerPool::new(M);
        let (wtx, wrx) = std::sync::mpsc::channel::<()>();
        for w in 0..M {
            let tx = wtx.clone();
            pool.submit(w, move || {
                ensure_worker_rt();
                let _ = tx.send(());
            })
            .unwrap();
        }
        for _ in 0..M {
            wrx.recv().unwrap();
        }
        let run = |assign: &dyn Fn(usize) -> usize| -> std::time::Duration {
            let (tx, rx) = std::sync::mpsc::channel::<()>();
            let t0 = std::time::Instant::now();
            for i in 0..M {
                let js = js.clone();
                let tx = tx.clone();
                pool.submit(assign(i), move || {
                    with_worker_runtime(|rt| {
                        let _ = rt.evaluate_script(&js, "compute");
                    });
                    let _ = tx.send(());
                })
                .unwrap();
            }
            for _ in 0..M {
                rx.recv().unwrap();
            }
            t0.elapsed()
        };
        let par = run(&|i| i);
        let seq = run(&|_| 0);
        let speedup = seq.as_secs_f64() / par.as_secs_f64();
        eprintln!(
            "\n=== CRUFT compute across {} Compartment threads (K={} tasks, {}M iters each) ===",
            M,
            M,
            ITERS / 1_000_000
        );
        eprintln!(
            "sequential (1 thread, {} tasks): {:.0} ms  ({:.0} ms/task)",
            M,
            seq.as_secs_f64() * 1e3,
            seq.as_secs_f64() * 1e3 / M as f64
        );
        eprintln!(
            "parallel  ({} threads, {} tasks): {:.0} ms  ({:.0} ms/task amortized)",
            M,
            M,
            par.as_secs_f64() * 1e3,
            par.as_secs_f64() * 1e3 / M as f64
        );
        eprintln!(
            "SPEEDUP: {:.2}x ({:.0}% efficiency)   throughput: {:.2} tasks/s",
            speedup,
            speedup / M as f64 * 100.0,
            M as f64 / par.as_secs_f64()
        );
        eprintln!("(reference: node single-thread compute(10M) ~26 ms/task; cruft 1-thread ~2870 ms/task)");
        pool.shutdown();
    }

    #[test]
    #[ignore]
    fn bench_edge_threaded() {
        const M: usize = 8;
        const K: usize = 5000;
        let per = K / M;
        let worker_js = r#"globalThis.__acc = (function(){
  const HANDLER = `function(req){
    if (req.method !== 'POST') return JSON.stringify({status:405});
    let body; try { body = JSON.parse(req.body); } catch(e){ return JSON.stringify({status:400}); }
    if (!body.user || !body.items) return JSON.stringify({status:422});
    let total = 0; for (const it of body.items) total += it.price * it.qty;
    return JSON.stringify({status:200, user:body.user, count:body.items.length, total:total, path:req.path});
  }`;
  const REQ = { method:'POST', path:'/checkout', ts:1700000000,
    body: '{"user":"alice","items":[{"price":9.99,"qty":2},{"price":3.5,"qty":1},{"price":12,"qty":4}]}' };
  const reqLit = JSON.stringify(REQ);
  let acc = 0;
  for (let i=0;i<__PER__;i++){ const c = new Compartment({}); acc += c.evaluate('(' + HANDLER + ')(' + reqLit + ').length'); }
  return acc;
})();"#.replace("__PER__", &per.to_string());

        let pool = WorkerPool::new(M);

        let (wtx, wrx) = std::sync::mpsc::channel::<()>();
        for w in 0..M {
            let tx = wtx.clone();
            pool.submit(w, move || {
                ensure_worker_rt();
                let _ = tx.send(());
            })
            .unwrap();
        }
        for _ in 0..M {
            wrx.recv().unwrap();
        }
        let rss_base = vm_rss_kb();

        let (tx, rx) = std::sync::mpsc::channel::<f64>();
        let t0 = std::time::Instant::now();
        for w in 0..M {
            let js = worker_js.clone();
            let tx = tx.clone();
            pool.submit(w, move || {
                let acc = with_worker_runtime(|rt| {
                    let _ = rt.evaluate_script(&js, "edge");
                    match rt.global_get("__acc") {
                        Value::Number(n) => n,
                        _ => -1.0,
                    }
                })
                .unwrap_or(-1.0);
                let _ = tx.send(acc);
            })
            .unwrap();
        }
        let mut checksum = 0.0;
        for _ in 0..M {
            checksum += rx.recv().unwrap();
        }
        let dt = t0.elapsed();
        let rss_live = vm_rss_kb();

        eprintln!(
            "\n=== CRUFT edge (FAIR: {} worker threads, fresh Compartment/req) ===",
            M
        );
        eprintln!(
            "requests: {}  checksum: {} (expect {})",
            K,
            checksum,
            K * 72
        );
        eprintln!(
            "time: {:.1} ms = {:.0} req/s  ({:.0} us/req)",
            dt.as_secs_f64() * 1e3,
            K as f64 / dt.as_secs_f64(),
            dt.as_secs_f64() * 1e6 / K as f64
        );
        eprintln!(
            "peak RSS: {} KB (base {} KB, +{} KB)",
            rss_live,
            rss_base,
            rss_live.saturating_sub(rss_base)
        );
        pool.shutdown();
    }

    #[test]
    #[ignore]
    fn bench_messaging() {
        use std::sync::{Arc, Mutex};
        const K: usize = 2000;
        let mut scheduler = Scheduler::new(2);
        let pool = WorkerPool::new(2);
        let cid = 7000u64;
        let worker = scheduler.assign(cid);
        let (stx, srx) = std::sync::mpsc::channel::<()>();
        pool.submit(worker, move || {
            build_compartment_on_worker(
                cid,
                &CompartmentDescriptor {
                    initial_globals: vec![],
                    boundary_policy: None,
                    timeout_ms: None,
                    onmessage_source: None,
                    module_loader_config: None,
                    host_call_tx: None,
                },
            );
            let handler = with_worker_runtime(|rt| {

                rt.evaluate_script(
                    "globalThis.__h = (e) => { globalThis.n = e.data.s.length; };",
                    "w",
                )
                .ok();
                rt.global_get("__h")
            })
            .unwrap();
            set_worker_onmessage(cid, handler);
            let _ = stx.send(());
        })
        .unwrap();
        srx.recv().unwrap();

        eprintln!("\n=== CRUFT cross-context messaging (shared-immutable arena) ===");
        for &s_bytes in &[1_000usize, 100_000, 1_000_000] {
            let arena: Arc<Mutex<rusty_js_gc::Tier2Arena<String>>> =
                Arc::new(Mutex::new(rusty_js_gc::Tier2Arena::default()));

            let payload = {
                let mut sender = Runtime::new();
                sender.install_intrinsics();
                sender
                    .evaluate_script(
                        &format!("globalThis.m = {{ s: 'x'.repeat({}) }};", s_bytes),
                        "s",
                    )
                    .unwrap();
                let gt = sender.global_object.unwrap();
                let mv = sender.object_get(gt, "m");
                lower_with_shared_arena(&sender, &mv, &arena).expect("lower shared")
            };
            let t0 = std::time::Instant::now();
            let mut rxs = Vec::with_capacity(K);
            for _ in 0..K {
                rxs.push(cross_thread_send_shared(
                    &mut scheduler,
                    &pool,
                    cid,
                    payload.clone(),
                    arena.clone(),
                ));
            }
            for rx in rxs {
                rx.recv().unwrap();
            }
            let dt = t0.elapsed();
            eprintln!(
                "payload {:>9} B: {:>7.3} ms / {} msgs = {:.2} us/msg  ({:.0} MB/s effective)",
                s_bytes,
                dt.as_secs_f64() * 1e3,
                K,
                dt.as_secs_f64() * 1e6 / K as f64,
                (s_bytes as f64 * K as f64) / dt.as_secs_f64() / 1e6
            );
        }
        pool.shutdown();
        scheduler.shutdown();
    }

    #[test]
    #[ignore]
    fn bench_parallel_scaling() {
        const TOTAL_JOBS: usize = 32;
        const ITERS: u64 = 4_000_000;
        let heavy = format!(
            "let s=0; for(let i=0;i<{};i++){{ s=(s+i*3+7)%1000003; }} s;",
            ITERS
        );

        let run = |threads: usize| -> std::time::Duration {
            let pool = WorkerPool::new(threads);

            let (wtx, wrx) = std::sync::mpsc::channel::<()>();
            for w in 0..threads {
                let tx = wtx.clone();
                pool.submit(w, move || {
                    build_realm_from_descriptor(&CompartmentDescriptor {
                        initial_globals: vec![],
                        boundary_policy: None,
                        timeout_ms: None,
                        onmessage_source: None,
                        module_loader_config: None,
                        host_call_tx: None,
                    });
                    let _ = tx.send(());
                })
                .unwrap();
            }
            for _ in 0..threads {
                wrx.recv().unwrap();
            }

            let (tx, rx) = std::sync::mpsc::channel::<()>();
            let t0 = std::time::Instant::now();
            for j in 0..TOTAL_JOBS {
                let src = heavy.clone();
                let tx = tx.clone();
                pool.submit(j % threads, move || {
                    with_worker_runtime(|rt| {
                        let _ = rt.evaluate_script(&src, "bench");
                    });
                    let _ = tx.send(());
                })
                .unwrap();
            }
            for _ in 0..TOTAL_JOBS {
                rx.recv().unwrap();
            }
            let dt = t0.elapsed();
            pool.shutdown();
            dt
        };

        let m = WorkerPool::default_threads().min(8).max(2);
        let serial = run(1);
        let parallel = run(m);
        let speedup = serial.as_secs_f64() / parallel.as_secs_f64();
        let total_iters = (TOTAL_JOBS as u64) * ITERS;
        eprintln!("\n=== CRUFT parallel scaling (rusty-js-runtime engine) ===");
        eprintln!(
            "jobs={} iters/job={} threads_parallel={}",
            TOTAL_JOBS, ITERS, m
        );
        eprintln!(
            "serial   (1 thread):  {:>8.3} s  ({:.2} M-iter/s)",
            serial.as_secs_f64(),
            total_iters as f64 / serial.as_secs_f64() / 1e6
        );
        eprintln!(
            "parallel ({} threads): {:>8.3} s  ({:.2} M-iter/s)",
            m,
            parallel.as_secs_f64(),
            total_iters as f64 / parallel.as_secs_f64() / 1e6
        );
        eprintln!(
            "SPEEDUP: {:.2}x   (ideal {}x, efficiency {:.0}%)",
            speedup,
            m,
            speedup / m as f64 * 100.0
        );
    }

    #[test]
    fn healing_poisoned_compartment_does_not_perturb_neighbors() {
        const M: usize = 4;
        const N: u64 = 16;
        const K: usize = 50;
        const POISONED: u64 = 1000;

        let mut scheduler = Scheduler::new(M);
        let pool = WorkerPool::new(M);
        let mut workers = Vec::new();
        for i in 0..N {
            let cid = 1000 + i;
            let w = scheduler.assign(cid);
            workers.push((cid, w));
            let poisoned = cid == POISONED;
            let (stx, srx) = std::sync::mpsc::channel::<()>();
            pool.submit(w, move || {
                build_compartment_on_worker(
                    cid,
                    &CompartmentDescriptor {
                        initial_globals: vec![],
                        boundary_policy: None,
                        timeout_ms: None,
                        onmessage_source: Some(if poisoned {
                            "function(e){ \
                               globalThis.count = (globalThis.count || 0) + 1; \
                               throw new Error('boom'); }"
                                .to_string()
                        } else {
                            "function(e){ \
                               globalThis.sum = (globalThis.sum || 0) + e.data.v; \
                               globalThis.count = (globalThis.count || 0) + 1; }"
                                .to_string()
                        }),
                        module_loader_config: None,
                        host_call_tx: None,
                    },
                );
                let _ = stx.send(());
            })
            .expect("setup");
            srx.recv().unwrap();
        }

        let mut receipts = Vec::new();
        for _round in 0..K {
            for i in 0..N {
                let cid = 1000 + i;
                let v = (i + 1) as i64;
                let payload = lower_global(&format!("globalThis.m = {{ v: {} }};", v), "m", None)
                    .expect("lower");
                receipts.push(cross_thread_send(&mut scheduler, &pool, cid, payload));
            }
        }

        for rx in receipts {
            assert!(rx.recv().unwrap(), "delivered despite poison");
        }

        for (cid, w) in workers {
            let i = cid - 1000;
            let v = (i + 1) as f64;
            let (rtx, rrx) = std::sync::mpsc::channel::<(f64, f64)>();
            pool.submit(w, move || {
                let sum = match worker_read_compartment_global(cid, "sum") {
                    Value::Number(n) => n,
                    _ => 0.0,
                };
                let count = match worker_read_compartment_global(cid, "count") {
                    Value::Number(n) => n,
                    _ => -1.0,
                };
                let _ = rtx.send((sum, count));
            })
            .expect("readback");
            let (sum, count) = rrx.recv().unwrap();
            if cid == POISONED {

                assert_eq!(
                    count, K as f64,
                    "poisoned compartment still received K (throw contained, thread alive)"
                );
            } else {
                assert_eq!(
                    count, K as f64,
                    "healthy compartment {} pristine count despite poisoned neighbor",
                    cid
                );
                assert_eq!(
                    sum,
                    K as f64 * v,
                    "healthy compartment {} pristine sum -> fault did NOT leak across the boundary",
                    cid
                );
            }
        }
        pool.shutdown();
        scheduler.shutdown();
    }

    #[test]
    fn worker_compartment_records_module_loader_base() {
        let mut scheduler = Scheduler::new(2);
        let pool = WorkerPool::new(2);
        let cid = 700u64;
        let worker = scheduler.assign(cid);
        let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
        pool.submit(worker, move || {
            build_compartment_on_worker(
                cid,
                &CompartmentDescriptor {
                    initial_globals: vec![],
                    boundary_policy: None,
                    timeout_ms: None,
                    onmessage_source: None,
                    module_loader_config: Some(ModuleLoaderConfig {
                        base_url: "file:///a/".to_string(),
                    }),
                    host_call_tx: None,
                },
            );
            let _ = tx.send(worker_compartment_loader_base(cid));
        })
        .expect("submit");
        assert_eq!(
            rx.recv().unwrap().as_deref(),
            Some("file:///a/"),
            "worker realm records the Send module loader base"
        );
        pool.shutdown();
        scheduler.shutdown();
    }
}
