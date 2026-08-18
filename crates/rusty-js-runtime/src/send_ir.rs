
use std::collections::HashMap;
use std::rc::Rc;

use rusty_js_gc::{Tier2Arena, Tier2Handle};

use crate::interp::Runtime;
use crate::value::{InternalKind, Object, Value};
use crate::RuntimeError;

#[derive(Debug, Clone)]
pub enum SendStr {
    Owned(String),
    Shared(Tier2Handle),
}

#[derive(Debug, Clone)]
pub enum SendIr {
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),

    BigInt(String),
    Str(SendStr),

    ArrayBuffer(Vec<u8>),

    TypedArray {
        kind: String,
        is_buffer: bool,
        bytes: Vec<u8>,
    },

    SharedArrayBuffer {
        byte_length: usize,
        max_byte_length: usize,
        shared: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    },

    Composite {
        ref_id: u32,
        is_array: bool,
        proto_null: bool,
        props: Vec<(String, SendIr)>,
    },

    MapData {
        ref_id: u32,
        entries: Vec<(String, SendIr)>,

        orig_keys: Vec<(String, SendIr)>,
    },

    SetData {
        ref_id: u32,
        values: Vec<(String, SendIr)>,
    },

    RegExp {
        source: String,
        flags: String,
    },

    BoxedPrimitive {
        kind: String,
        value: Box<SendIr>,
    },

    ErrorObj {
        name: String,
        message: String,
        stack: String,
        cause: Option<Box<SendIr>>,
    },

    Ref(u32),

    Callable {
        compartment_id: u64,
        binding_id: u64,
        name: String,
        length: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SendIrDisposition {
    Clone,
    Transfer,
    SharedBacking,
    CallableProxy,
}

impl SendIr {
    pub fn disposition(&self) -> SendIrDisposition {
        match self {
            SendIr::ArrayBuffer(_) | SendIr::TypedArray { .. } => SendIrDisposition::Transfer,
            SendIr::SharedArrayBuffer { .. } => SendIrDisposition::SharedBacking,
            SendIr::Callable { .. } => SendIrDisposition::CallableProxy,
            SendIr::Undefined
            | SendIr::Null
            | SendIr::Boolean(_)
            | SendIr::Number(_)
            | SendIr::BigInt(_)
            | SendIr::Str(_)
            | SendIr::Composite { .. }
            | SendIr::MapData { .. }
            | SendIr::SetData { .. }
            | SendIr::RegExp { .. }
            | SendIr::BoxedPrimitive { .. }
            | SendIr::ErrorObj { .. }
            | SendIr::Ref(_) => SendIrDisposition::Clone,
        }
    }
}

const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<SendIr>();
};

#[derive(Debug, Default)]
pub struct CallableRegistry {
    pub compartment_id: u64,
    next_binding: u64,
    entries: HashMap<u64, rusty_js_gc::ObjectId>,
}

impl CallableRegistry {
    pub fn new(compartment_id: u64) -> Self {
        CallableRegistry {
            compartment_id,
            next_binding: 0,
            entries: HashMap::new(),
        }
    }

    pub fn register(&mut self, callable: rusty_js_gc::ObjectId) -> u64 {
        let id = self.next_binding;
        self.next_binding += 1;
        self.entries.insert(id, callable);
        id
    }

    pub fn get(&self, binding_id: u64) -> Option<rusty_js_gc::ObjectId> {
        self.entries.get(&binding_id).copied()
    }

    pub fn roots(&self) -> impl Iterator<Item = rusty_js_gc::ObjectId> + '_ {
        self.entries.values().copied()
    }
}

#[derive(Debug, Clone)]
pub struct CallableDescriptor {
    pub compartment_id: u64,
    pub binding_id: u64,
    pub name: String,
    pub length: u32,
}

pub fn callable_proxy_descriptor(ir: &SendIr) -> Option<CallableDescriptor> {
    match ir {
        SendIr::Callable {
            compartment_id,
            binding_id,
            name,
            length,
        } => Some(CallableDescriptor {
            compartment_id: *compartment_id,
            binding_id: *binding_id,
            name: name.clone(),
            length: *length,
        }),
        _ => None,
    }
}

pub struct LowerCtx<'a> {
    pub string_arena: Option<&'a mut Tier2Arena<String>>,

    pub callable_registry: Option<&'a mut CallableRegistry>,
    seen: HashMap<u32, u32>,
    next_ref: u32,
}

impl<'a> LowerCtx<'a> {
    pub fn new(string_arena: Option<&'a mut Tier2Arena<String>>) -> Self {
        LowerCtx {
            string_arena,
            callable_registry: None,
            seen: HashMap::new(),
            next_ref: 0,
        }
    }

    pub fn with_callables(
        string_arena: Option<&'a mut Tier2Arena<String>>,
        callable_registry: &'a mut CallableRegistry,
    ) -> Self {
        LowerCtx {
            string_arena,
            callable_registry: Some(callable_registry),
            seen: HashMap::new(),
            next_ref: 0,
        }
    }
}

pub fn lower_to_send_ir(
    rt: &Runtime,
    v: &Value,
    ctx: &mut LowerCtx,
) -> Result<SendIr, RuntimeError> {
    match v {
        Value::Undefined => Ok(SendIr::Undefined),
        Value::Null => Ok(SendIr::Null),
        Value::Boolean(b) => Ok(SendIr::Boolean(*b)),
        Value::Number(n) => Ok(SendIr::Number(*n)),
        Value::BigInt(b) => Ok(SendIr::BigInt(b.to_string())),
        Value::String(s) => {
            let sstr = match ctx.string_arena.as_mut() {
                Some(arena) => {
                    SendStr::Shared(arena.freeze((**s).to_string(), Vec::new()).map_err(|_| {
                        RuntimeError::TypeError(
                            "send IR: shared string arena rejected a cyclic freeze".into(),
                        )
                    })?)
                }
                None => SendStr::Owned((**s).to_string()),
            };
            Ok(SendIr::Str(sstr))
        }
        Value::Symbol(_) => Err(RuntimeError::TypeError(
            "send IR: Symbol values are not transferable across a Compartment boundary".into(),
        )),
        Value::Object(oid) => {

            if let Some(ref_id) = ctx.seen.get(&oid.0) {
                return Ok(SendIr::Ref(*ref_id));
            }

            if matches!(
                rt.obj(*oid).internal_kind,
                InternalKind::Function(_)
                    | InternalKind::Closure(_)
                    | InternalKind::BoundFunction(_)
            ) {
                match ctx.callable_registry.as_mut() {
                    Some(reg) => {
                        let compartment_id = reg.compartment_id;
                        let binding_id = reg.register(*oid);
                        let name = match rt.object_get(*oid, "name") {
                            Value::String(s) => s.as_str().to_string(),
                            _ => String::new(),
                        };
                        let length = match rt.object_get(*oid, "length") {
                            Value::Number(n) if n >= 0.0 => n as u32,
                            _ => 0,
                        };
                        return Ok(SendIr::Callable {
                            compartment_id,
                            binding_id,
                            name,
                            length,
                        });
                    }
                    None => {
                        return Err(RuntimeError::TypeError(
                            "send IR: function values are not transferable".into(),
                        ));
                    }
                }
            }

            if let Some(view) = rt.typed_array_views.get(oid) {
                let kind = view.element_kind.to_string();
                let backing = view.buffer;
                let byte_offset = view.byte_offset;
                let byte_len = match rt.object_get(*oid, "byteLength") {
                    Value::Number(n) if n >= 0.0 => n as usize,
                    _ => 0,
                };
                let is_buffer = rt.obj(*oid).is_buffer;
                let bytes = match rt.array_buffers.get(&backing) {
                    Some(rec) => rec
                        .data
                        .get(byte_offset..byte_offset + byte_len)
                        .map(|s| s.to_vec())
                        .unwrap_or_default(),
                    None => Vec::new(),
                };
                return Ok(SendIr::TypedArray {
                    kind,
                    is_buffer,
                    bytes,
                });
            }

            if let Some(rec) = rt.array_buffers.get(oid) {
                if rec.detached {
                    return Err(RuntimeError::TypeError(
                        "send IR: a detached ArrayBuffer cannot be transferred".into(),
                    ));
                }
                if let Some(shared) = &rec.shared {
                    return Ok(SendIr::SharedArrayBuffer {
                        byte_length: rec.byte_length,
                        max_byte_length: rec.max_byte_length,
                        shared: shared.clone(),
                    });
                }
                return Ok(SendIr::ArrayBuffer(rec.data.clone()));
            }

            if let InternalKind::RegExp(re) = &rt.obj(*oid).internal_kind {
                return Ok(SendIr::RegExp {
                    source: re.source.to_string(),
                    flags: re.flags.to_string(),
                });
            }

            {
                let boxed: Option<(&str, Value)> = match &rt.obj(*oid).internal_kind {
                    InternalKind::NumberWrapper(v) => Some(("Number", v.clone())),
                    InternalKind::StringWrapper(v) => Some(("String", v.clone())),
                    InternalKind::BooleanWrapper(v) => Some(("Boolean", v.clone())),
                    InternalKind::BigIntWrapper(v) => Some(("BigInt", v.clone())),
                    _ => None,
                };
                if let Some((kind, prim)) = boxed {
                    return Ok(SendIr::BoxedPrimitive {
                        kind: kind.to_string(),
                        value: Box::new(lower_to_send_ir(rt, &prim, ctx)?),
                    });
                }
            }

            if matches!(rt.obj(*oid).internal_kind, InternalKind::Error) {

                fn get_str_chain(
                    rt: &Runtime,
                    start: rusty_js_gc::ObjectId,
                    key: &str,
                ) -> Option<String> {
                    let mut cur = Some(start);
                    while let Some(id) = cur {
                        if let Some(d) = rt.obj(id).get_own(key) {
                            if let Value::String(s) = &d.value {
                                return Some(s.as_str().to_string());
                            }
                        }
                        cur = rt.obj(id).proto;
                    }
                    None
                }
                let name = get_str_chain(rt, *oid, "name").unwrap_or_else(|| "Error".to_string());
                let message = get_str_chain(rt, *oid, "message").unwrap_or_default();
                let stack = match rt.obj(*oid).get_own("__error_stack__") {
                    Some(d) => match &d.value {
                        Value::String(s) => s.as_str().to_string(),
                        _ => String::new(),
                    },
                    None => String::new(),
                };
                let cause_val = rt.obj(*oid).get_own("cause").map(|d| d.value.clone());
                let cause = match cause_val {
                    Some(cv) => Some(Box::new(lower_to_send_ir(rt, &cv, ctx)?)),
                    None => None,
                };
                return Ok(SendIr::ErrorObj {
                    name,
                    message,
                    stack,
                    cause,
                });
            }

            let is_weakmap = matches!(rt.object_get(*oid, "__is_weakmap"), Value::Boolean(true));
            if !is_weakmap {
                if let Value::Object(storage) = rt.object_get(*oid, "__map_data") {
                    let ref_id = ctx.next_ref;
                    ctx.next_ref += 1;
                    ctx.seen.insert(oid.0, ref_id);
                    let pairs: Vec<(String, Value)> = rt
                        .obj(storage)
                        .properties
                        .iter()
                        .map(|(k, d)| (k.to_string_content(), d.value.clone()))
                        .collect();
                    let mut entries = Vec::with_capacity(pairs.len());
                    for (k, v) in pairs {
                        entries.push((k, lower_to_send_ir(rt, &v, ctx)?));
                    }

                    let orig_pairs: Vec<(String, Value)> =
                        match rt.object_get(*oid, "__map_orig_keys") {
                            Value::Object(orig) => rt
                                .obj(orig)
                                .properties
                                .iter()
                                .map(|(k, d)| (k.to_string_content(), d.value.clone()))
                                .collect(),
                            _ => Vec::new(),
                        };
                    let mut orig_keys = Vec::with_capacity(orig_pairs.len());
                    for (k, v) in orig_pairs {
                        orig_keys.push((k, lower_to_send_ir(rt, &v, ctx)?));
                    }
                    return Ok(SendIr::MapData {
                        ref_id,
                        entries,
                        orig_keys,
                    });
                }
                if let Value::Object(storage) = rt.object_get(*oid, "__set_data") {
                    let ref_id = ctx.next_ref;
                    ctx.next_ref += 1;
                    ctx.seen.insert(oid.0, ref_id);
                    let pairs: Vec<(String, Value)> = rt
                        .obj(storage)
                        .properties
                        .iter()
                        .map(|(k, d)| (k.to_string_content(), d.value.clone()))
                        .collect();
                    let mut values = Vec::with_capacity(pairs.len());
                    for (k, v) in pairs {
                        values.push((k, lower_to_send_ir(rt, &v, ctx)?));
                    }
                    return Ok(SendIr::SetData { ref_id, values });
                }
            }

            let ref_id = ctx.next_ref;
            ctx.next_ref += 1;
            ctx.seen.insert(oid.0, ref_id);
            let is_array = matches!(rt.obj(*oid).internal_kind, InternalKind::Array);

            let proto_null = rt.obj(*oid).proto.is_none();

            let pairs: Vec<(String, Value)> = {
                let src = rt.obj(*oid);
                let mut out: Vec<(String, Value)> = Vec::new();

                if is_array {
                    let len = src.array_store_len();
                    for i in 0..len {
                        out.push((i.to_string(), src.array_store_get(i)));
                    }
                }
                if let Some(shape) = src.shape.as_ref() {
                    for (name, slot) in shape.iter_slots() {
                        let idx = slot as usize;
                        if let Some(val) = src.shape_values.get(idx) {
                            out.push((name.to_string(), val.clone()));
                        }
                    }
                }
                out.extend(
                    src.properties
                        .iter()
                        .filter(|(k, _)| !k.as_str().starts_with("@@"))
                        .map(|(k, d)| (k.to_string_content(), d.value.clone())),
                );
                out
            };
            let mut props = Vec::with_capacity(pairs.len());
            for (k, val) in pairs {
                props.push((k, lower_to_send_ir(rt, &val, ctx)?));
            }
            Ok(SendIr::Composite {
                ref_id,
                is_array,
                proto_null,
                props,
            })
        }
    }
}

pub fn rematerialize_send_ir(
    rt: &mut Runtime,
    ir: &SendIr,
    string_arena: Option<&Tier2Arena<String>>,
    table: &mut HashMap<u32, rusty_js_gc::ObjectId>,
) -> Result<Value, RuntimeError> {
    match ir {
        SendIr::Undefined => Ok(Value::Undefined),
        SendIr::Null => Ok(Value::Null),
        SendIr::Boolean(b) => Ok(Value::Boolean(*b)),
        SendIr::Number(n) => Ok(Value::Number(*n)),
        SendIr::BigInt(s) => match crate::bigint::JsBigInt::from_decimal(s) {
            Some(b) => Ok(Value::BigInt(Rc::new(b))),
            None => Err(RuntimeError::TypeError(
                "send IR: malformed BigInt payload".into(),
            )),
        },
        SendIr::Str(SendStr::Owned(s)) => Ok(Value::String(Rc::new(crate::value::JsString::from(
            s.clone(),
        )))),
        SendIr::Str(SendStr::Shared(h)) => {
            let arena = string_arena.ok_or_else(|| {
                RuntimeError::TypeError(
                    "send IR: a Shared string handle requires the string arena to re-materialize"
                        .into(),
                )
            })?;
            match arena.get(*h) {
                Some(s) => Ok(Value::String(Rc::new(crate::value::JsString::from(
                    s.clone(),
                )))),
                None => Err(RuntimeError::TypeError(
                    "send IR: dangling Shared string handle".into(),
                )),
            }
        }
        SendIr::ArrayBuffer(bytes) => {
            let ab_proto = match rt.global_get("ArrayBuffer") {
                Value::Object(ctor) => match rt.object_get(ctor, "prototype") {
                    Value::Object(pid) => Some(pid),
                    _ => None,
                },
                _ => None,
            };
            let mut o = Object::new_ordinary();
            o.set_own_internal(
                "__kind".into(),
                Value::String(Rc::new(crate::value::JsString::from("ArrayBuffer"))),
            );
            o.proto = ab_proto;
            let id = rt.alloc_object(o);
            rt.heap.note_external_alloc(bytes.len());
            rt.array_buffers.insert(
                id,
                crate::interp::ArrayBufferRecord {
                    byte_length: bytes.len(),
                    max_byte_length: bytes.len(),
                    backing_epoch: 0,
                    data: bytes.clone(),
                    detached: false,
                    untransferable: false,

                    shared: None,
                },
            );
            Ok(Value::Object(id))
        }
        SendIr::RegExp { source, flags } => {
            let id = crate::regexp::new_regexp(rt, source, flags)?;
            Ok(Value::Object(id))
        }
        SendIr::BoxedPrimitive { kind, value } => {
            let prim = rematerialize_send_ir(rt, value, string_arena, table)?;

            let ctor_name = if kind == "BigInt" {
                "Object"
            } else {
                kind.as_str()
            };
            let ctor = match rt.global_get(ctor_name) {
                Value::Object(id) => id,
                _ => {
                    return Err(RuntimeError::TypeError(
                        "send IR: cannot build boxed primitive".into(),
                    ))
                }
            };
            if kind == "BigInt" {
                rt.call_function(Value::Object(ctor), Value::Undefined, vec![prim])
            } else {
                let prev = rt.pending_new_target.take();
                rt.pending_new_target = Some(Value::Object(ctor));
                let out = rt.call_function(Value::Object(ctor), Value::Undefined, vec![prim]);
                rt.pending_new_target = prev;
                out
            }
        }
        SendIr::ErrorObj {
            name,
            message,
            stack,
            cause,
        } => {

            let ctor_name = match name.as_str() {
                "EvalError" | "RangeError" | "ReferenceError" | "SyntaxError" | "TypeError"
                | "URIError" => name.as_str(),
                _ => "Error",
            };
            let id = crate::intrinsics::make_error_instance(rt, ctor_name, message)
                .ok_or_else(|| RuntimeError::TypeError("send IR: cannot build Error".into()))?;
            rt.obj_mut(id).set_own_internal(
                "stack".into(),
                Value::String(Rc::new(crate::value::JsString::from(stack.clone()))),
            );
            if let Some(cause_ir) = cause {
                let cause_val = rematerialize_send_ir(rt, cause_ir, string_arena, table)?;
                rt.obj_mut(id).set_own_internal("cause".into(), cause_val);
            }
            Ok(Value::Object(id))
        }
        SendIr::TypedArray {
            kind,
            is_buffer,
            bytes,
        } => {

            let ab = rematerialize_send_ir(
                rt,
                &SendIr::ArrayBuffer(bytes.clone()),
                string_arena,
                table,
            )?;
            if *is_buffer {
                let buffer_ctor = rt.global_get("Buffer");
                let from = match &buffer_ctor {
                    Value::Object(cid) => rt.object_get(*cid, "from"),
                    _ => Value::Undefined,
                };
                return rt.call_function(from, buffer_ctor, vec![ab]);
            }
            let ctor = rt.global_get(kind);
            rt.construct(ctor, vec![ab])
        }
        SendIr::SharedArrayBuffer {
            byte_length,
            max_byte_length,
            shared,
        } => {
            let sab_proto = match rt.global_get("SharedArrayBuffer") {
                Value::Object(ctor) => match rt.object_get(ctor, "prototype") {
                    Value::Object(pid) => Some(pid),
                    _ => None,
                },
                _ => None,
            };
            let mut o = Object::new_ordinary();
            o.set_own_internal(
                "__kind".into(),
                Value::String(Rc::new(crate::value::JsString::from("SharedArrayBuffer"))),
            );
            o.proto = sab_proto;
            let id = rt.alloc_object(o);
            rt.array_buffers.insert(
                id,
                crate::interp::ArrayBufferRecord {
                    byte_length: *byte_length,
                    max_byte_length: *max_byte_length,
                    backing_epoch: 0,
                    data: Vec::new(),
                    detached: false,
                    untransferable: false,
                    shared: Some(shared.clone()),
                },
            );
            Ok(Value::Object(id))
        }
        SendIr::Ref(ref_id) => match table.get(ref_id) {
            Some(id) => Ok(Value::Object(*id)),
            None => Err(RuntimeError::TypeError(
                "send IR: forward Ref to an un-materialized Composite".into(),
            )),
        },
        SendIr::Callable {
            compartment_id,
            binding_id,
            name,
            length,
        } => {

            let mut o = Object::new_ordinary();
            o.set_own_internal(
                "__callable_compartment_id".into(),
                Value::Number(*compartment_id as f64),
            );
            o.set_own_internal(
                "__callable_binding_id".into(),
                Value::Number(*binding_id as f64),
            );
            o.set_own_internal("__callable_is_proxy_stub".into(), Value::Boolean(true));
            o.set_own_internal(
                "name".into(),
                Value::String(Rc::new(crate::value::JsString::from(name.clone()))),
            );
            o.set_own_internal("length".into(), Value::Number(*length as f64));
            Ok(Value::Object(rt.alloc_object(o)))
        }
        SendIr::Composite {
            ref_id,
            is_array,
            proto_null,
            props,
        } => {

            let is_date = !*is_array && !*proto_null && props.iter().any(|(k, _)| k == "__date_ms");
            let date_proto = if is_date {
                match rt.global_get("Date") {
                    Value::Object(ctor) => match rt.object_get(ctor, "prototype") {
                        Value::Object(pid) => Some(pid),
                        _ => None,
                    },
                    _ => None,
                }
            } else {
                None
            };
            let mut container = if *is_array {
                Object::new_array()
            } else {
                Object::new_ordinary()
            };
            if date_proto.is_some() {
                container.proto = date_proto;
            }
            let dst_id = if *proto_null {
                rt.alloc_object_with_explicit_null_proto(container)
            } else {
                rt.alloc_object(container)
            };
            table.insert(*ref_id, dst_id);
            for (k, child) in props {
                let v = rematerialize_send_ir(rt, child, string_arena, table)?;
                rt.object_set(dst_id, k.clone(), v);
            }

            if is_date {
                rt.mark_date_object(dst_id);
            }
            Ok(Value::Object(dst_id))
        }
        SendIr::MapData {
            ref_id,
            entries,
            orig_keys,
        } => {

            let proto = match rt.global_get("Map") {
                Value::Object(ctor) => match rt.object_get(ctor, "prototype") {
                    Value::Object(pid) => Some(pid),
                    _ => None,
                },
                _ => None,
            };
            let mut o = Object::new_ordinary();
            o.proto = proto;
            let dst_id = rt.alloc_object(o);
            let storage = rt.alloc_object(Object::new_dictionary());
            rt.set_engine_sentinel(dst_id, "__map_data", Value::Object(storage));
            table.insert(*ref_id, dst_id);
            for (k, child) in entries {
                let v = rematerialize_send_ir(rt, child, string_arena, table)?;
                rt.object_set(storage, k.clone(), v);
            }

            if !orig_keys.is_empty() {
                let orig = rt.alloc_object(Object::new_dictionary());
                rt.set_engine_sentinel(dst_id, "__map_orig_keys", Value::Object(orig));
                for (k, child) in orig_keys {
                    let v = rematerialize_send_ir(rt, child, string_arena, table)?;
                    rt.object_set(orig, k.clone(), v);
                }
            }
            Ok(Value::Object(dst_id))
        }
        SendIr::SetData { ref_id, values } => {
            let proto = match rt.global_get("Set") {
                Value::Object(ctor) => match rt.object_get(ctor, "prototype") {
                    Value::Object(pid) => Some(pid),
                    _ => None,
                },
                _ => None,
            };
            let mut o = Object::new_ordinary();
            o.proto = proto;
            let dst_id = rt.alloc_object(o);
            let storage = rt.alloc_object(Object::new_dictionary());
            rt.set_engine_sentinel(dst_id, "__set_data", Value::Object(storage));
            table.insert(*ref_id, dst_id);
            for (k, child) in values {
                let v = rematerialize_send_ir(rt, child, string_arena, table)?;
                rt.object_set(storage, k.clone(), v);
            }
            Ok(Value::Object(dst_id))
        }
    }
}

const SENDIR_V8_MAGIC: &[u8; 4] = b"CRv8";

fn w_u32(v: u32, out: &mut Vec<u8>) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn w_str(s: &str, out: &mut Vec<u8>) {
    w_u32(s.len() as u32, out);
    out.extend_from_slice(s.as_bytes());
}
fn send_str_to_string(s: &SendStr) -> Result<String, String> {
    match s {
        SendStr::Owned(v) => Ok(v.clone()),

        SendStr::Shared(_) => Err("v8.serialize: shared string arena unavailable".into()),
    }
}

pub fn send_ir_to_bytes(ir: &SendIr) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(64);
    out.extend_from_slice(SENDIR_V8_MAGIC);
    write_send_ir(ir, &mut out)?;
    Ok(out)
}

fn write_send_ir(ir: &SendIr, out: &mut Vec<u8>) -> Result<(), String> {
    match ir {
        SendIr::Undefined => out.push(0),
        SendIr::Null => out.push(1),
        SendIr::Boolean(b) => {
            out.push(2);
            out.push(*b as u8);
        }
        SendIr::Number(n) => {
            out.push(3);
            out.extend_from_slice(&n.to_le_bytes());
        }
        SendIr::BigInt(s) => {
            out.push(4);
            w_str(s, out);
        }
        SendIr::Str(s) => {
            out.push(5);
            w_str(&send_str_to_string(s)?, out);
        }
        SendIr::ArrayBuffer(bytes) => {
            out.push(6);
            w_u32(bytes.len() as u32, out);
            out.extend_from_slice(bytes);
        }
        SendIr::TypedArray {
            kind,
            is_buffer,
            bytes,
        } => {
            out.push(11);
            w_str(kind, out);
            out.push(*is_buffer as u8);
            w_u32(bytes.len() as u32, out);
            out.extend_from_slice(bytes);
        }
        SendIr::Composite {
            ref_id,
            is_array,
            proto_null,
            props,
        } => {
            out.push(7);
            w_u32(*ref_id, out);
            out.push(*is_array as u8);
            out.push(*proto_null as u8);
            w_u32(props.len() as u32, out);
            for (k, v) in props {
                w_str(k, out);
                write_send_ir(v, out)?;
            }
        }
        SendIr::MapData {
            ref_id,
            entries,
            orig_keys,
        } => {
            out.push(8);
            w_u32(*ref_id, out);
            w_u32(entries.len() as u32, out);
            for (k, v) in entries {
                w_str(k, out);
                write_send_ir(v, out)?;
            }
            w_u32(orig_keys.len() as u32, out);
            for (k, v) in orig_keys {
                w_str(k, out);
                write_send_ir(v, out)?;
            }
        }
        SendIr::SetData { ref_id, values } => {
            out.push(9);
            w_u32(*ref_id, out);
            w_u32(values.len() as u32, out);
            for (k, v) in values {
                w_str(k, out);
                write_send_ir(v, out)?;
            }
        }
        SendIr::RegExp { source, flags } => {
            out.push(12);
            w_str(source, out);
            w_str(flags, out);
        }
        SendIr::BoxedPrimitive { kind, value } => {
            out.push(14);
            w_str(kind, out);
            write_send_ir(value, out)?;
        }
        SendIr::ErrorObj {
            name,
            message,
            stack,
            cause,
        } => {
            out.push(13);
            w_str(name, out);
            w_str(message, out);
            w_str(stack, out);
            match cause {
                Some(c) => {
                    out.push(1);
                    write_send_ir(c, out)?;
                }
                None => out.push(0),
            }
        }
        SendIr::Ref(id) => {
            out.push(10);
            w_u32(*id, out);
        }
        SendIr::SharedArrayBuffer { .. } => {
            return Err("v8.serialize: SharedArrayBuffer cannot be serialized".into());
        }
        SendIr::Callable { .. } => {
            return Err("() could not be cloned.".into());
        }
    }
    Ok(())
}

fn r_u32(data: &[u8], pos: &mut usize) -> Result<u32, String> {
    let b = data
        .get(*pos..*pos + 4)
        .ok_or_else(|| "v8.deserialize: truncated".to_string())?;
    *pos += 4;
    Ok(u32::from_le_bytes(b.try_into().unwrap()))
}
fn r_str(data: &[u8], pos: &mut usize) -> Result<String, String> {
    let n = r_u32(data, pos)? as usize;
    let b = data
        .get(*pos..*pos + n)
        .ok_or_else(|| "v8.deserialize: truncated string".to_string())?;
    *pos += n;
    String::from_utf8(b.to_vec()).map_err(|_| "v8.deserialize: invalid utf8".to_string())
}

pub fn send_ir_from_bytes(data: &[u8]) -> Result<SendIr, String> {
    if data.len() < 4 || &data[0..4] != SENDIR_V8_MAGIC {
        return Err("v8.deserialize: bad or foreign serialization header".into());
    }
    let mut pos = 4usize;
    read_send_ir(data, &mut pos)
}

fn read_send_ir(data: &[u8], pos: &mut usize) -> Result<SendIr, String> {
    let tag = *data
        .get(*pos)
        .ok_or_else(|| "v8.deserialize: truncated tag".to_string())?;
    *pos += 1;
    let one = |data: &[u8], pos: &mut usize| -> Result<u8, String> {
        let b = *data
            .get(*pos)
            .ok_or_else(|| "v8.deserialize: truncated".to_string())?;
        *pos += 1;
        Ok(b)
    };
    Ok(match tag {
        0 => SendIr::Undefined,
        1 => SendIr::Null,
        2 => SendIr::Boolean(one(data, pos)? != 0),
        3 => {
            let b = data
                .get(*pos..*pos + 8)
                .ok_or_else(|| "v8.deserialize: truncated".to_string())?;
            *pos += 8;
            SendIr::Number(f64::from_le_bytes(b.try_into().unwrap()))
        }
        4 => SendIr::BigInt(r_str(data, pos)?),
        5 => SendIr::Str(SendStr::Owned(r_str(data, pos)?)),
        6 => {
            let n = r_u32(data, pos)? as usize;
            let b = data
                .get(*pos..*pos + n)
                .ok_or_else(|| "v8.deserialize: truncated".to_string())?
                .to_vec();
            *pos += n;
            SendIr::ArrayBuffer(b)
        }
        11 => {
            let kind = r_str(data, pos)?;
            let is_buffer = one(data, pos)? != 0;
            let n = r_u32(data, pos)? as usize;
            let bytes = data
                .get(*pos..*pos + n)
                .ok_or_else(|| "v8.deserialize: truncated typed array".to_string())?
                .to_vec();
            *pos += n;
            SendIr::TypedArray {
                kind,
                is_buffer,
                bytes,
            }
        }
        12 => {
            let source = r_str(data, pos)?;
            let flags = r_str(data, pos)?;
            SendIr::RegExp { source, flags }
        }
        14 => {
            let kind = r_str(data, pos)?;
            let value = Box::new(read_send_ir(data, pos)?);
            SendIr::BoxedPrimitive { kind, value }
        }
        13 => {
            let name = r_str(data, pos)?;
            let message = r_str(data, pos)?;
            let stack = r_str(data, pos)?;
            let has_cause = one(data, pos)? != 0;
            let cause = if has_cause {
                Some(Box::new(read_send_ir(data, pos)?))
            } else {
                None
            };
            SendIr::ErrorObj {
                name,
                message,
                stack,
                cause,
            }
        }
        7 => {
            let ref_id = r_u32(data, pos)?;
            let is_array = one(data, pos)? != 0;
            let proto_null = one(data, pos)? != 0;
            let n = r_u32(data, pos)? as usize;
            let mut props = Vec::with_capacity(n);
            for _ in 0..n {
                let k = r_str(data, pos)?;
                props.push((k, read_send_ir(data, pos)?));
            }
            SendIr::Composite {
                ref_id,
                is_array,
                proto_null,
                props,
            }
        }
        8 => {
            let ref_id = r_u32(data, pos)?;
            let ne = r_u32(data, pos)? as usize;
            let mut entries = Vec::with_capacity(ne);
            for _ in 0..ne {
                let k = r_str(data, pos)?;
                entries.push((k, read_send_ir(data, pos)?));
            }
            let nk = r_u32(data, pos)? as usize;
            let mut orig_keys = Vec::with_capacity(nk);
            for _ in 0..nk {
                let k = r_str(data, pos)?;
                orig_keys.push((k, read_send_ir(data, pos)?));
            }
            SendIr::MapData {
                ref_id,
                entries,
                orig_keys,
            }
        }
        9 => {
            let ref_id = r_u32(data, pos)?;
            let n = r_u32(data, pos)? as usize;
            let mut values = Vec::with_capacity(n);
            for _ in 0..n {
                let k = r_str(data, pos)?;
                values.push((k, read_send_ir(data, pos)?));
            }
            SendIr::SetData { ref_id, values }
        }
        10 => SendIr::Ref(r_u32(data, pos)?),
        _ => return Err(format!("v8.deserialize: unknown tag {tag}")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusty_js_gc::Tier2Arena;
    use std::collections::HashMap;

    fn run_test_on_large_stack(f: impl FnOnce() + Send + 'static) {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(f)
            .expect("spawn large-stack send_ir test runner")
            .join()
            .expect("large-stack send_ir test runner must not panic");
    }

    fn rt_with(src: &str) -> Runtime {
        let mut rt = Runtime::new();
        rt.install_intrinsics();
        rt.evaluate_script(src, "file://send-ir-test")
            .expect("script runs");
        rt
    }
    fn global(rt: &Runtime) -> rusty_js_gc::ObjectId {
        rt.global_object.expect("globalThis")
    }

    #[test]
    fn send_ir_disposition_classifies_cross_agent_alphabet() {
        assert_eq!(SendIr::Undefined.disposition(), SendIrDisposition::Clone);
        assert_eq!(
            SendIr::Composite {
                ref_id: 0,
                is_array: false,
                proto_null: false,
                props: Vec::new(),
            }
            .disposition(),
            SendIrDisposition::Clone
        );
        assert_eq!(
            SendIr::ArrayBuffer(vec![1, 2, 3]).disposition(),
            SendIrDisposition::Transfer
        );
        assert_eq!(
            SendIr::TypedArray {
                kind: "Uint8Array".into(),
                is_buffer: false,
                bytes: vec![1, 2, 3],
            }
            .disposition(),
            SendIrDisposition::Transfer
        );
        assert_eq!(
            SendIr::SharedArrayBuffer {
                byte_length: 4,
                max_byte_length: 4,
                shared: std::sync::Arc::new(std::sync::Mutex::new(vec![0; 4])),
            }
            .disposition(),
            SendIrDisposition::SharedBacking
        );
        assert_eq!(
            SendIr::Callable {
                compartment_id: 1,
                binding_id: 2,
                name: "f".into(),
                length: 0,
            }
            .disposition(),
            SendIrDisposition::CallableProxy
        );
    }

    #[test]
    fn shared_array_buffer_round_trip_preserves_shared_backing() {
        run_test_on_large_stack(|| {
            let rt = rt_with(
                r#"
                globalThis.sab = new SharedArrayBuffer(4);
                var ta = new Uint8Array(globalThis.sab);
                ta[0] = 17;
                ta[1] = 23;
                "#,
            );
            let gt = global(&rt);
            let src = match rt.object_get(gt, "sab") {
                Value::Object(id) => id,
                _ => panic!("expected source SharedArrayBuffer"),
            };
            let src_shared = rt
                .array_buffers
                .get(&src)
                .and_then(|rec| rec.shared.clone())
                .expect("source SAB should have shared backing");

            let mut ctx = LowerCtx::new(None);
            let ir = lower_to_send_ir(&rt, &Value::Object(src), &mut ctx).expect("lower SAB");
            assert_eq!(ir.disposition(), SendIrDisposition::SharedBacking);

            let mut target = Runtime::new();
            target.install_intrinsics();
            let mut table = HashMap::new();
            let out =
                rematerialize_send_ir(&mut target, &ir, None, &mut table).expect("rematerialize");
            let out_id = match out {
                Value::Object(id) => id,
                _ => panic!("expected rematerialized SharedArrayBuffer"),
            };
            let out_shared = target
                .array_buffers
                .get(&out_id)
                .and_then(|rec| rec.shared.clone())
                .expect("rematerialized SAB should have shared backing");

            assert!(
                std::sync::Arc::ptr_eq(&src_shared, &out_shared),
                "SAB rematerialization must preserve the exact shared backing Arc"
            );
            {
                let mut bytes = out_shared.lock().expect("shared backing lock");
                bytes[2] = 99;
            }
            let bytes = src_shared.lock().expect("source shared backing lock");
            assert_eq!(
                &bytes[..4],
                &[17, 23, 99, 0],
                "writes through the rematerialized backing must be visible to the source runtime"
            );
        });
    }

    #[test]
    fn shared_array_buffer_backing_survives_source_gc_when_target_retains_it() {
        run_test_on_large_stack(|| {
            let mut source = rt_with(
                r#"
                globalThis.sab = new SharedArrayBuffer(4);
                var ta = new Uint8Array(globalThis.sab);
                ta[0] = 11;
                "#,
            );
            let gt = global(&source);
            let src = match source.object_get(gt, "sab") {
                Value::Object(id) => id,
                _ => panic!("expected source SharedArrayBuffer"),
            };
            let src_shared = source
                .array_buffers
                .get(&src)
                .and_then(|rec| rec.shared.clone())
                .expect("source SAB should have shared backing");
            let weak = std::sync::Arc::downgrade(&src_shared);

            let mut ctx = LowerCtx::new(None);
            let ir = lower_to_send_ir(&source, &Value::Object(src), &mut ctx).expect("lower SAB");
            let mut target = Runtime::new();
            target.install_intrinsics();
            let mut table = HashMap::new();
            let out =
                rematerialize_send_ir(&mut target, &ir, None, &mut table).expect("rematerialize");
            let out_id = match out {
                Value::Object(id) => id,
                _ => panic!("expected rematerialized SharedArrayBuffer"),
            };
            drop(ir);
            drop(src_shared);

            source
                .evaluate_script(
                    "globalThis.sab = null; globalThis.ta = null;",
                    "file://sab-source-gc-retention-drop-roots",
                )
                .expect("drop source SAB roots");
            let _ = source.collect();
            drop(source);

            let retained = weak
                .upgrade()
                .expect("target SAB record should retain shared backing after source GC/drop");
            {
                let mut bytes = retained.lock().expect("retained shared backing lock");
                assert_eq!(bytes[0], 11);
                bytes[3] = 44;
            }
            let target_shared = target
                .array_buffers
                .get(&out_id)
                .and_then(|rec| rec.shared.clone())
                .expect("target SAB record should still have shared backing");
            let bytes = target_shared.lock().expect("target shared backing lock");
            assert_eq!(
                &bytes[..4],
                &[11, 0, 0, 44],
                "target runtime must retain the shared backing independently of the source heap"
            );
        });
    }

    #[test]
    fn round_trip_owned_strings_identity_equivalence() {
        run_test_on_large_stack(|| {
            let mut rt = rt_with(
                r#"
                globalThis.o = { a: 1, s: "hi", nested: { b: true } };
                "#,
            );
            let gt = global(&rt);
            let src = match rt.object_get(gt, "o") {
                Value::Object(id) => id,
                _ => panic!(),
            };
            let mut ctx = LowerCtx::new(None);
            let ir = lower_to_send_ir(&rt, &Value::Object(src), &mut ctx).expect("lower");
            let mut table = HashMap::new();
            let out = rematerialize_send_ir(&mut rt, &ir, None, &mut table).expect("rematerialize");
            let out_id = match out {
                Value::Object(id) => id,
                _ => panic!(),
            };
            assert_ne!(
                src, out_id,
                "re-materialized object must be a fresh identity"
            );
            assert!(matches!(rt.object_get(out_id, "a"), Value::Number(n) if n == 1.0));
            assert!(
                matches!(rt.object_get(out_id, "s"), Value::String(ref s) if s.as_str() == "hi")
            );
            let nested = match rt.object_get(out_id, "nested") {
                Value::Object(id) => id,
                _ => panic!("nested"),
            };
            assert!(matches!(rt.object_get(nested, "b"), Value::Boolean(true)));
        });
    }

    #[test]
    fn round_trip_shared_string_via_tier2_arena() {
        run_test_on_large_stack(|| {
            let mut rt = rt_with(r#" globalThis.o = { s: "shared-bytes" }; "#);
            let gt = global(&rt);
            let src = match rt.object_get(gt, "o") {
                Value::Object(id) => id,
                _ => panic!(),
            };
            let mut arena: Tier2Arena<String> = Tier2Arena::default();
            let ir = {
                let mut ctx = LowerCtx::new(Some(&mut arena));
                lower_to_send_ir(&rt, &Value::Object(src), &mut ctx).expect("lower")
            };

            if let SendIr::Composite { props, .. } = &ir {
                assert!(
                    matches!(
                        props.iter().find(|(k, _)| k == "s").map(|(_, v)| v),
                        Some(SendIr::Str(SendStr::Shared(_)))
                    ),
                    "string must lower to a Tier-2 Shared handle"
                );
            } else {
                panic!("expected Composite");
            }
            let mut table = HashMap::new();
            let out = rematerialize_send_ir(&mut rt, &ir, Some(&arena), &mut table)
                .expect("rematerialize");
            let out_id = match out {
                Value::Object(id) => id,
                _ => panic!(),
            };
            assert!(
                matches!(rt.object_get(out_id, "s"), Value::String(ref s) if s.as_str() == "shared-bytes")
            );
        });
    }

    #[test]
    fn round_trip_preserves_cycles_and_shared_subobjects() {
        run_test_on_large_stack(|| {
            let mut rt = rt_with(
                r#"
                var shared = { tag: 7 };
                var o = { first: shared, second: shared };
                o.loop = o;
                globalThis.o = o;
            "#,
            );
            let gt = global(&rt);
            let src = match rt.object_get(gt, "o") {
                Value::Object(id) => id,
                _ => panic!(),
            };
            let mut ctx = LowerCtx::new(None);
            let ir = lower_to_send_ir(&rt, &Value::Object(src), &mut ctx).expect("lower");
            let mut table = HashMap::new();
            let out = rematerialize_send_ir(&mut rt, &ir, None, &mut table).expect("rematerialize");
            let out_id = match out {
                Value::Object(id) => id,
                _ => panic!(),
            };

            assert!(
                matches!(rt.object_get(out_id, "loop"), Value::Object(id) if id == out_id),
                "self-cycle must re-materialize to the same identity"
            );

            let f = match rt.object_get(out_id, "first") {
                Value::Object(id) => id,
                _ => panic!(),
            };
            let s = match rt.object_get(out_id, "second") {
                Value::Object(id) => id,
                _ => panic!(),
            };
            assert_eq!(
                f, s,
                "a doubly-referenced sub-object must share one re-materialized identity"
            );
            assert!(matches!(rt.object_get(f, "tag"), Value::Number(n) if n == 7.0));
        });
    }

    #[test]
    fn lower_rejects_symbol_and_function() {
        run_test_on_large_stack(|| {
            let rt = rt_with(
                r#"
                globalThis.sym = Symbol("x");
                globalThis.fn = function () {};
                "#,
            );
            let gt = global(&rt);
            let sym = rt.object_get(gt, "sym");
            let mut ctx = LowerCtx::new(None);
            assert!(
                lower_to_send_ir(&rt, &sym, &mut ctx).is_err(),
                "Symbol must be rejected at lowering"
            );
            let f = rt.object_get(gt, "fn");
            let mut ctx2 = LowerCtx::new(None);
            assert!(
                lower_to_send_ir(&rt, &f, &mut ctx2).is_err(),
                "function must be rejected at lowering"
            );
        });
    }

    #[test]
    fn arraybuffer_round_trips_with_fresh_storage() {
        run_test_on_large_stack(|| {
            let mut rt = rt_with(r#" globalThis.ab = new ArrayBuffer(4); "#);
            let gt = global(&rt);
            let src = match rt.object_get(gt, "ab") {
                Value::Object(id) => id,
                _ => panic!(),
            };
            let mut ctx = LowerCtx::new(None);
            let ir = lower_to_send_ir(&rt, &Value::Object(src), &mut ctx).expect("lower");
            assert!(matches!(ir, SendIr::ArrayBuffer(ref b) if b.len() == 4));
            let mut table = HashMap::new();
            let out = rematerialize_send_ir(&mut rt, &ir, None, &mut table).expect("rematerialize");
            let out_id = match out {
                Value::Object(id) => id,
                _ => panic!(),
            };
            assert_ne!(
                src, out_id,
                "re-materialized ArrayBuffer is a fresh identity"
            );
            let rec = rt.array_buffers.get(&out_id).expect("real ArrayBuffer");
            assert!(!rec.detached && rec.byte_length == 4);
        });
    }

    #[test]
    fn round_trip_preserves_null_prototype() {
        run_test_on_large_stack(|| {
            let mut rt = rt_with(
                r#"
                var o = Object.create(null);
                o.tag = 42;
                globalThis.o = o;
            "#,
            );
            let gt = global(&rt);
            let src = match rt.object_get(gt, "o") {
                Value::Object(id) => id,
                _ => panic!(),
            };

            assert!(
                rt.obj(src).proto.is_none(),
                "source must be a null-proto object"
            );
            let mut ctx = LowerCtx::new(None);
            let ir = lower_to_send_ir(&rt, &Value::Object(src), &mut ctx).expect("lower");
            assert!(
                matches!(
                    ir,
                    SendIr::Composite {
                        proto_null: true,
                        ..
                    }
                ),
                "null-proto source must lower with proto_null=true"
            );
            let mut table = HashMap::new();
            let out = rematerialize_send_ir(&mut rt, &ir, None, &mut table).expect("rematerialize");
            let out_id = match out {
                Value::Object(id) => id,
                _ => panic!(),
            };
            assert!(
                rt.obj(out_id).proto.is_none(),
                "re-materialized object must carry a null prototype"
            );
            assert!(matches!(rt.object_get(out_id, "tag"), Value::Number(n) if n == 42.0));
        });
    }

    #[test]
    fn round_trip_ordinary_object_keeps_default_prototype() {
        run_test_on_large_stack(|| {
            let mut rt = rt_with(r#" globalThis.o = { tag: 1 }; "#);
            let gt = global(&rt);
            let src = match rt.object_get(gt, "o") {
                Value::Object(id) => id,
                _ => panic!(),
            };
            assert!(
                rt.obj(src).proto.is_some(),
                "ordinary source has a prototype"
            );
            let mut ctx = LowerCtx::new(None);
            let ir = lower_to_send_ir(&rt, &Value::Object(src), &mut ctx).expect("lower");
            assert!(
                matches!(
                    ir,
                    SendIr::Composite {
                        proto_null: false,
                        ..
                    }
                ),
                "ordinary source must lower with proto_null=false"
            );
            let mut table = HashMap::new();
            let out = rematerialize_send_ir(&mut rt, &ir, None, &mut table).expect("rematerialize");
            let out_id = match out {
                Value::Object(id) => id,
                _ => panic!(),
            };
            assert!(
                rt.obj(out_id).proto.is_some(),
                "ordinary clone must carry the target realm's default prototype"
            );
        });
    }

    #[test]
    fn round_trip_preserves_null_prototype_nested() {
        run_test_on_large_stack(|| {
            let mut rt = rt_with(
                r#"
                var inner = Object.create(null);
                inner.k = "v";
                globalThis.o = { outer: 1, inner: inner };
            "#,
            );
            let gt = global(&rt);
            let src = match rt.object_get(gt, "o") {
                Value::Object(id) => id,
                _ => panic!(),
            };
            let mut ctx = LowerCtx::new(None);
            let ir = lower_to_send_ir(&rt, &Value::Object(src), &mut ctx).expect("lower");
            let mut table = HashMap::new();
            let out = rematerialize_send_ir(&mut rt, &ir, None, &mut table).expect("rematerialize");
            let out_id = match out {
                Value::Object(id) => id,
                _ => panic!(),
            };

            assert!(rt.obj(out_id).proto.is_some(), "root keeps default proto");
            let inner = match rt.object_get(out_id, "inner") {
                Value::Object(id) => id,
                _ => panic!("inner"),
            };
            assert!(
                rt.obj(inner).proto.is_none(),
                "nested null-proto object stays null-proto"
            );
            assert!(matches!(rt.object_get(inner, "k"), Value::String(ref s) if s.as_str() == "v"));
        });
    }

    #[test]
    fn callable_without_registry_still_rejects() {
        run_test_on_large_stack(|| {
            let rt = rt_with(r#" globalThis.f = function named(a, b) {}; "#);
            let gt = global(&rt);
            let f = rt.object_get(gt, "f");
            let mut ctx = LowerCtx::new(None);
            assert!(
                lower_to_send_ir(&rt, &f, &mut ctx).is_err(),
                "a function must still reject when no callable registry is present"
            );
        });
    }

    #[test]
    fn callable_with_registry_lowers_and_registers() {
        run_test_on_large_stack(|| {
            let rt = rt_with(r#" globalThis.f = function named(a, b) {}; "#);
            let gt = global(&rt);
            let f_id = match rt.object_get(gt, "f") {
                Value::Object(id) => id,
                _ => panic!(),
            };
            let mut reg = CallableRegistry::new(7);
            let ir = {
                let mut ctx = LowerCtx::with_callables(None, &mut reg);
                lower_to_send_ir(&rt, &Value::Object(f_id), &mut ctx).expect("lower")
            };
            match &ir {
                SendIr::Callable {
                    compartment_id,
                    binding_id,
                    name,
                    length,
                } => {
                    assert_eq!(*compartment_id, 7);
                    assert_eq!(name, "named");
                    assert_eq!(*length, 2, "fn named(a,b) has length 2");

                    assert_eq!(
                        reg.get(*binding_id),
                        Some(f_id),
                        "registry resolves binding_id -> the registered callable"
                    );
                }
                other => panic!("expected SendIr::Callable, got {other:?}"),
            }

            assert!(
                reg.roots().any(|id| id == f_id),
                "registered callable is a root"
            );
        });
    }

    #[test]
    fn callable_proxy_descriptor_extracts_fields() {
        let ir = SendIr::Callable {
            compartment_id: 3,
            binding_id: 9,
            name: "g".into(),
            length: 1,
        };
        let d = callable_proxy_descriptor(&ir).expect("descriptor");
        assert_eq!(
            (d.compartment_id, d.binding_id, d.name.as_str(), d.length),
            (3, 9, "g", 1)
        );
        assert!(
            callable_proxy_descriptor(&SendIr::Null).is_none(),
            "non-callable variant has no proxy descriptor"
        );
    }

    #[test]
    fn callable_rematerializes_to_proxy_stub() {
        run_test_on_large_stack(|| {
            let mut rt = rt_with("globalThis.x = 1;");
            let ir = SendIr::Callable {
                compartment_id: 5,
                binding_id: 11,
                name: "h".into(),
                length: 0,
            };
            let mut table = HashMap::new();
            let out = rematerialize_send_ir(&mut rt, &ir, None, &mut table).expect("rematerialize");
            let id = match out {
                Value::Object(id) => id,
                _ => panic!("stub object"),
            };
            assert!(matches!(
                rt.object_get(id, "__callable_is_proxy_stub"),
                Value::Boolean(true)
            ));
            assert!(
                matches!(rt.object_get(id, "__callable_compartment_id"), Value::Number(n) if n == 5.0)
            );
            assert!(
                matches!(rt.object_get(id, "__callable_binding_id"), Value::Number(n) if n == 11.0)
            );
            assert!(matches!(rt.object_get(id, "name"), Value::String(ref s) if s.as_str() == "h"));
        });
    }
}
