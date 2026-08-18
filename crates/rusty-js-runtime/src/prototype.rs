
use crate::abstract_ops;
use crate::interp::{Runtime, RuntimeError};
use crate::value::{
    BoundFunctionInternals, FunctionInternals, InternalKind, NativeFn, Object, ObjectRef,
    PromiseReaction, PromiseStatus, Value,
};
use std::rc::Rc;

impl Runtime {

    pub fn install_prototypes(&mut self) {

        let object_proto = self.alloc_object(Object::new_ordinary());
        self.object_prototype = Some(object_proto);

        let array_proto = self.alloc_object(Object::new_ordinary());
        self.obj_mut(array_proto).internal_kind = crate::value::InternalKind::Array;
        let function_proto = self.alloc_object(Object::new_ordinary());
        let async_fn_proto = self.alloc_object(Object::new_ordinary());
        self.obj_mut(async_fn_proto).proto = Some(function_proto);
        let promise_proto = self.alloc_object(Object::new_ordinary());
        let string_proto = self.alloc_object(Object::new_ordinary());
        let number_proto = self.alloc_object(Object::new_ordinary());

        self.obj_mut(string_proto).internal_kind = crate::value::InternalKind::StringWrapper(
            Value::String(std::rc::Rc::new(crate::value::JsString::from(""))),
        );
        self.obj_mut(number_proto).internal_kind =
            crate::value::InternalKind::NumberWrapper(Value::Number(0.0));

        self.obj_mut(function_proto).internal_kind =
            crate::value::InternalKind::Function(Box::new(crate::value::FunctionInternals {
                name: String::new(),
                length: 0,
                native: std::rc::Rc::new(|_, _| Ok(Value::Undefined)),
                is_constructor: false,
                creation_realm: 0,
                roots: Vec::new(),
            }));
        self.array_prototype = Some(array_proto);
        self.function_prototype = Some(function_proto);
        self.async_function_prototype = Some(async_fn_proto);
        self.promise_prototype = Some(promise_proto);
        self.string_prototype = Some(string_proto);
        self.number_prototype = Some(number_proto);

        self.realms[0].object_prototype = Some(object_proto);
        self.realms[0].array_prototype = Some(array_proto);
        self.realms[0].function_prototype = Some(function_proto);
        self.realms[0].async_function_prototype = Some(async_fn_proto);
        self.realms[0].promise_prototype = Some(promise_proto);
        self.realms[0].string_prototype = Some(string_proto);
        self.realms[0].number_prototype = Some(number_proto);

        install_object_proto(self, object_proto);
        install_array_proto(self, array_proto);
        install_string_proto(self, string_proto);
        install_function_proto(self, function_proto);
        install_async_function_constructor(self, async_fn_proto);
        install_promise_proto(self, promise_proto);
        install_number_proto(self, number_proto);

        let iter_proto = self.alloc_object(Object::new_ordinary());
        let gen_proto = self.alloc_object(Object::new_ordinary());
        self.obj_mut(gen_proto).proto = Some(iter_proto);
        let gen_fn_proto = self.alloc_object(Object::new_ordinary());
        self.obj_mut(gen_fn_proto).proto = Some(function_proto);
        self.obj_mut(gen_fn_proto).dict_mut().insert(
            "prototype".into(),
            crate::value::PropertyDescriptor {
                value: Value::Object(gen_proto),
                writable: false,
                enumerable: false,
                configurable: true,
                getter: None,
                setter: None,
            },
        );
        self.iterator_prototype = Some(iter_proto);
        self.generator_prototype = Some(gen_proto);
        self.generator_function_prototype = Some(gen_fn_proto);
        self.realms[0].iterator_prototype = Some(iter_proto);
        self.realms[0].generator_prototype = Some(gen_proto);
        self.realms[0].generator_function_prototype = Some(gen_fn_proto);
        install_iterator_proto(self, iter_proto);
        install_generator_proto(self, gen_proto);
        install_generator_function_constructor(self, gen_fn_proto, false);
        install_generator_intrinsic_proto_meta(self, gen_proto, gen_fn_proto, "Generator");

        let async_iter_proto = self.alloc_object(Object::new_ordinary());
        let async_gen_proto = self.alloc_object(Object::new_ordinary());
        self.obj_mut(async_gen_proto).proto = Some(async_iter_proto);
        let async_gen_fn_proto = self.alloc_object(Object::new_ordinary());
        self.obj_mut(async_gen_fn_proto).proto = Some(function_proto);
        self.obj_mut(async_gen_fn_proto).dict_mut().insert(
            "prototype".into(),
            crate::value::PropertyDescriptor {
                value: Value::Object(async_gen_proto),
                writable: false,
                enumerable: false,
                configurable: true,
                getter: None,
                setter: None,
            },
        );
        self.async_iterator_prototype = Some(async_iter_proto);
        self.async_generator_prototype = Some(async_gen_proto);
        self.async_generator_function_prototype = Some(async_gen_fn_proto);
        self.realms[0].async_iterator_prototype = Some(async_iter_proto);
        self.realms[0].async_generator_prototype = Some(async_gen_proto);
        self.realms[0].async_generator_function_prototype = Some(async_gen_fn_proto);
        install_async_iterator_proto(self, async_iter_proto);
        install_async_generator_proto(self, async_gen_proto);
        install_generator_function_constructor(self, async_gen_fn_proto, true);
        install_generator_intrinsic_proto_meta(
            self,
            async_gen_proto,
            async_gen_fn_proto,
            "AsyncGenerator",
        );
    }
}

fn dynamic_async_function(rt: &mut Runtime, args: &[Value]) -> Result<Value, RuntimeError> {
    let dynfn_new_target = rt.current_new_target.clone();
    let body = match args.last() {
        Some(v) => abstract_ops::to_string(v).as_str().to_string(),
        None => String::new(),
    };
    let params: Vec<String> = if args.len() > 1 {
        args[..args.len() - 1]
            .iter()
            .map(|v| abstract_ops::to_string(v).as_str().to_string())
            .collect()
    } else {
        Vec::new()
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    static AFC_COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = AFC_COUNTER.fetch_add(1, Ordering::Relaxed);
    let url = format!("file://<AsyncFunction:{}>", n);
    let stash_key = format!("__afc_out_{}", n);
    let source = format!(
        "{} = async function anonymous({}\n) {{\n{}\n}};",
        stash_key,
        params.join(","),
        body
    );
    match rt.evaluate_module(&source, &url) {
        Ok(_ns) => {
            let result = rt.global_get(&stash_key);
            if let Some(gt) = rt.global_object {
                rt.obj_mut(gt).remove_str(&stash_key);
            }
            if let (Value::Object(fid), Some(Value::Object(nt)), Some(fallback)) =
                (&result, &dynfn_new_target, rt.async_function_prototype)
            {
                let proto = rt.get_prototype_from_constructor(
                    *nt,
                    |rr| rr.async_function_prototype,
                    fallback,
                )?;
                rt.obj_mut(*fid).proto = Some(proto);
            }
            Ok(result)
        }
        Err(e) => Err(e),
    }
}

pub(crate) fn dynamic_async_function_in_realm(
    rt: &mut Runtime,
    args: &[Value],
    realm_idx: usize,
    global_this: ObjectRef,
) -> Result<Value, RuntimeError> {
    let body = match args.last() {
        Some(v) => abstract_ops::to_string(v).as_str().to_string(),
        None => String::new(),
    };
    let params: Vec<String> = if args.len() > 1 {
        args[..args.len() - 1]
            .iter()
            .map(|v| abstract_ops::to_string(v).as_str().to_string())
            .collect()
    } else {
        Vec::new()
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    static RAFC_COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = RAFC_COUNTER.fetch_add(1, Ordering::Relaxed);
    let stash_key = format!("__rafc_out_{}", n);
    let source = format!(
        "{} = async function anonymous({}\n) {{\n{}\n}};",
        stash_key,
        params.join(","),
        body
    );
    let result = rt.eval_source_globalish_in_realm(source, realm_idx, global_this)?;
    rt.obj_mut(global_this).remove_str(&stash_key);
    Ok(result)
}

fn install_async_function_constructor(rt: &mut Runtime, host: ObjectRef) {
    let native: NativeFn = Rc::new(dynamic_async_function);
    let mut properties = indexmap::IndexMap::new();
    crate::value::install_function_meta_props(&mut properties, "AsyncFunction", 1.0);
    properties.insert(
        crate::value::PropertyKey::String("prototype".to_string()),
        crate::value::PropertyDescriptor {
            value: Value::Object(host),
            writable: false,
            enumerable: false,
            configurable: false,
            getter: None,
            setter: None,
        },
    );
    let fn_obj = Object {
        proto: rt.function_prototype,
        extensible: true,
        properties,
        internal_kind: InternalKind::Function(Box::new(FunctionInternals {
            name: "AsyncFunction".to_string(),
            length: 1,
            native,
            is_constructor: true,
            creation_realm: 0,
            roots: Vec::new(),
        })),

        ..Default::default()
    };
    let fn_id = rt.alloc_object(fn_obj);
    rt.obj_mut(host).dict_mut().insert(
        crate::value::PropertyKey::String("constructor".to_string()),
        crate::value::PropertyDescriptor {
            value: Value::Object(fn_id),
            writable: false,
            enumerable: false,
            configurable: true,
            getter: None,
            setter: None,
        },
    );
    rt.obj_mut(host).dict_mut().insert(
        crate::value::PropertyKey::String("@@toStringTag".to_string()),
        crate::value::PropertyDescriptor {
            value: Value::String(Rc::new(crate::value::JsString::from(
                "AsyncFunction".to_string(),
            ))),
            writable: false,
            enumerable: false,
            configurable: true,
            getter: None,
            setter: None,
        },
    );
}

fn dynamic_generator_function(
    rt: &mut Runtime,
    args: &[Value],
    is_async: bool,
) -> Result<Value, RuntimeError> {
    let body = match args.last() {
        Some(v) => abstract_ops::to_string(v).as_str().to_string(),
        None => String::new(),
    };
    let params: Vec<String> = if args.len() > 1 {
        args[..args.len() - 1]
            .iter()
            .map(|v| abstract_ops::to_string(v).as_str().to_string())
            .collect()
    } else {
        Vec::new()
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    static GFC_COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = GFC_COUNTER.fetch_add(1, Ordering::Relaxed);
    let url = if is_async {
        format!("file://<AsyncGeneratorFunction:{}>", n)
    } else {
        format!("file://<GeneratorFunction:{}>", n)
    };
    let stash_key = format!("__gfc_out_{}", n);
    let head = if is_async {
        "async function*"
    } else {
        "function*"
    };
    let source = format!(
        "{} = {} anonymous({}\n) {{\n{}\n}};",
        stash_key,
        head,
        params.join(","),
        body
    );

    let dynfn_new_target = rt.current_new_target.clone();
    let realm_global = if rt.current_realm != 0 {
        rt.realms.get(rt.current_realm).and_then(|r| r.global)
    } else {
        None
    };
    let cur_realm_for_eval = rt.current_realm;
    let eval_result = match realm_global {
        Some(rg) => rt
            .eval_source_globalish_in_realm(source, cur_realm_for_eval, rg)
            .map(|_| ()),
        None => rt.evaluate_module(&source, &url).map(|_| ()),
    };
    match eval_result {
        Ok(_ns) => {
            let result = match realm_global {
                Some(rg) => rt.object_get(rg, &stash_key),
                None => rt.global_get(&stash_key),
            };
            if let Some(gt) = realm_global.or(rt.global_object) {
                rt.obj_mut(gt).remove_str(&stash_key);
            }
            if let (Value::Object(fid), Some(global)) = (&result, realm_global) {
                if let crate::value::InternalKind::Closure(c) = &mut rt.obj_mut(*fid).internal_kind
                {
                    c.creation_realm = cur_realm_for_eval;
                    c.creation_global = Some(global);
                }
            }

            if let (Value::Object(fid), Some(Value::Object(nt))) = (&result, dynfn_new_target) {
                let fallback = if is_async {
                    rt.async_generator_function_prototype
                } else {
                    rt.generator_function_prototype
                };
                if let Some(fallback) = fallback {
                    let proto = rt.get_prototype_from_constructor(
                        nt,
                        |rr| {
                            if is_async {
                                rr.async_generator_function_prototype
                            } else {
                                rr.generator_function_prototype
                            }
                        },
                        fallback,
                    )?;
                    rt.obj_mut(*fid).proto = Some(proto);
                }
            }
            Ok(result)
        }
        Err(e) => Err(e),
    }
}

pub(crate) fn dynamic_generator_function_in_realm(
    rt: &mut Runtime,
    args: &[Value],
    is_async: bool,
    realm_idx: usize,
    global_this: ObjectRef,
) -> Result<Value, RuntimeError> {
    let body = match args.last() {
        Some(v) => abstract_ops::to_string(v).as_str().to_string(),
        None => String::new(),
    };
    let params: Vec<String> = if args.len() > 1 {
        args[..args.len() - 1]
            .iter()
            .map(|v| abstract_ops::to_string(v).as_str().to_string())
            .collect()
    } else {
        Vec::new()
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    static RGFC_COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = RGFC_COUNTER.fetch_add(1, Ordering::Relaxed);
    let stash_key = format!("__rgfc_out_{}", n);
    let head = if is_async {
        "async function*"
    } else {
        "function*"
    };
    let source = format!(
        "{} = {} anonymous({}\n) {{\n{}\n}};",
        stash_key,
        head,
        params.join(","),
        body
    );
    let result = rt.eval_source_globalish_in_realm(source, realm_idx, global_this)?;
    rt.obj_mut(global_this).remove_str(&stash_key);
    Ok(result)
}

fn install_generator_function_constructor(
    rt: &mut Runtime,
    host: ObjectRef,
    is_async: bool,
) -> ObjectRef {
    let name = if is_async {
        "AsyncGeneratorFunction"
    } else {
        "GeneratorFunction"
    };
    let native: NativeFn = Rc::new(move |rt, args| dynamic_generator_function(rt, args, is_async));
    let mut properties = indexmap::IndexMap::new();
    crate::value::install_function_meta_props(&mut properties, name, 1.0);
    properties.insert(
        crate::value::PropertyKey::String("prototype".to_string()),
        crate::value::PropertyDescriptor {
            value: Value::Object(host),
            writable: false,
            enumerable: false,
            configurable: false,
            getter: None,
            setter: None,
        },
    );
    let fn_obj = Object {
        proto: rt.function_prototype,
        extensible: true,
        properties,
        internal_kind: InternalKind::Function(Box::new(FunctionInternals {
            name: name.to_string(),
            length: 1,
            native,
            is_constructor: true,
            creation_realm: 0,
            roots: Vec::new(),
        })),

        ..Default::default()
    };
    let fn_id = rt.alloc_object(fn_obj);
    rt.obj_mut(host).dict_mut().insert(
        crate::value::PropertyKey::String("constructor".to_string()),
        crate::value::PropertyDescriptor {
            value: Value::Object(fn_id),
            writable: false,
            enumerable: false,
            configurable: true,
            getter: None,
            setter: None,
        },
    );
    rt.obj_mut(host).dict_mut().insert(
        crate::value::PropertyKey::String("@@toStringTag".to_string()),
        crate::value::PropertyDescriptor {
            value: Value::String(Rc::new(crate::value::JsString::from(name.to_string()))),
            writable: false,
            enumerable: false,
            configurable: true,
            getter: None,
            setter: None,
        },
    );
    fn_id
}

fn install_generator_intrinsic_proto_meta(
    rt: &mut Runtime,
    host: ObjectRef,
    constructor: ObjectRef,
    tag: &str,
) {
    rt.obj_mut(host).dict_mut().insert(
        crate::value::PropertyKey::String("constructor".to_string()),
        crate::value::PropertyDescriptor {
            value: Value::Object(constructor),
            writable: false,
            enumerable: false,
            configurable: true,
            getter: None,
            setter: None,
        },
    );

    if let Value::Object(sym_ctor) = rt.global_get("Symbol") {
        if let Value::Symbol(sym) = rt.object_get(sym_ctor, "toStringTag") {
            rt.obj_mut(host).dict_mut().insert(
                crate::value::PropertyKey::Symbol(sym),
                crate::value::PropertyDescriptor {
                    value: Value::String(Rc::new(crate::value::JsString::from(tag.to_string()))),
                    writable: false,
                    enumerable: false,
                    configurable: true,
                    getter: None,
                    setter: None,
                },
            );
        }
    }
}

fn generator_this(rt: &mut Runtime, label: &str) -> Result<ObjectRef, RuntimeError> {
    let this_id = match rt.current_this() {
        Value::Object(id) => id,
        _ => {
            return Err(RuntimeError::TypeError(format!(
                "{label}: this is not an object"
            )))
        }
    };
    if !matches!(rt.obj(this_id).internal_kind, InternalKind::Generator(_)) {
        return Err(RuntimeError::TypeError(format!(
            "{label}: this is not a generator"
        )));
    }
    Ok(this_id)
}

fn install_iterator_proto(rt: &mut Runtime, host: ObjectRef) {
    register_intrinsic_method(rt, host, "@@iterator", 0, |rt, _args| Ok(rt.current_this()));
}

fn async_generator_this(rt: &mut Runtime, label: &str) -> Result<ObjectRef, RuntimeError> {
    let this_id = generator_this(rt, label)?;
    let is_async = matches!(
        &rt.obj(this_id).internal_kind,
        InternalKind::Generator(g) if g.is_async
    );
    if !is_async {
        return Err(RuntimeError::TypeError(format!(
            "{label}: this is not an async generator"
        )));
    }
    Ok(this_id)
}

fn install_generator_proto(rt: &mut Runtime, host: ObjectRef) {
    register_intrinsic_method(rt, host, "next", 1, |rt, args| {
        let this_id = generator_this(rt, "Generator.prototype.next")?;
        let sent = args.first().cloned().unwrap_or(Value::Undefined);
        rt.generator_next_scaffold(this_id, sent)
    });
    register_intrinsic_method(rt, host, "return", 1, |rt, args| {
        let this_id = generator_this(rt, "Generator.prototype.return")?;
        let value = args.first().cloned().unwrap_or(Value::Undefined);
        rt.generator_return_scaffold(this_id, value)
    });
    register_intrinsic_method(rt, host, "throw", 1, |rt, args| {
        let this_id = generator_this(rt, "Generator.prototype.throw")?;
        let thrown = args.first().cloned().unwrap_or(Value::Undefined);
        rt.generator_throw_scaffold(this_id, thrown)
    });
}

fn async_generator_rejected_type_error(rt: &mut Runtime, message: String) -> Value {
    let promise = crate::promise::new_promise(rt);
    let reason = crate::intrinsics::make_error_instance(rt, "TypeError", &message)
        .map(Value::Object)
        .unwrap_or_else(|| {
            Value::String(Rc::new(crate::value::JsString::from(format!(
                "TypeError({message})"
            ))))
        });
    crate::promise::reject_promise(rt, promise, reason);
    Value::Object(promise)
}

fn async_generator_settle_proto(
    rt: &mut Runtime,
    outcome: Result<Value, RuntimeError>,
) -> Result<Value, RuntimeError> {
    match outcome {
        Ok(result) => {
            let promise = crate::promise::new_promise(rt);
            crate::promise::resolve_promise(rt, promise, result);
            Ok(Value::Object(promise))
        }
        Err(RuntimeError::TypeError(message)) => {
            Ok(async_generator_rejected_type_error(rt, message))
        }
        Err(other) => Err(other),
    }
}

fn async_iterator_dispose_rejection_reason(rt: &mut Runtime, e: RuntimeError) -> Value {
    match e {
        RuntimeError::Thrown(v) => v,
        other => {
            let (kind, msg) = match &other {
                RuntimeError::TypeError(m) => ("TypeError", m.clone()),
                RuntimeError::RangeError(m) => ("RangeError", m.clone()),
                RuntimeError::ReferenceError(m) => ("ReferenceError", m.clone()),
                RuntimeError::SyntaxError(m) => ("SyntaxError", m.clone()),
                o => ("Error", format!("{:?}", o)),
            };
            crate::intrinsics::make_error_instance(rt, kind, &msg)
                .map(Value::Object)
                .unwrap_or_else(|| {
                    Value::String(Rc::new(crate::value::JsString::from(format!(
                        "{}({})",
                        kind, msg
                    ))))
                })
        }
    }
}

fn async_iterator_dispose_reject(rt: &mut Runtime, promise: ObjectRef, e: RuntimeError) {
    let reason = async_iterator_dispose_rejection_reason(rt, e);
    crate::promise::reject_promise(rt, promise, reason);
}

fn async_iterator_dispose_adopt(rt: &mut Runtime, inner: ObjectRef, outer: ObjectRef) {
    let fulfill_outer = outer;
    let on_fulfilled = crate::intrinsics::make_native_non_ctor("", 1, move |rt, _args| {
        crate::promise::resolve_promise(rt, fulfill_outer, Value::Undefined);
        Ok(Value::Undefined)
    });
    let reject_outer = outer;
    let on_rejected = crate::intrinsics::make_native_non_ctor("", 1, move |rt, args| {
        let reason = args.first().cloned().unwrap_or(Value::Undefined);
        crate::promise::reject_promise(rt, reject_outer, reason);
        Ok(Value::Undefined)
    });
    let on_fulfilled_id = rt.alloc_object(on_fulfilled);
    let on_rejected_id = rt.alloc_object(on_rejected);
    let _ = rt.promise_then_via(&[
        Value::Object(inner),
        Value::Object(on_fulfilled_id),
        Value::Object(on_rejected_id),
    ]);
}

fn install_async_iterator_proto(rt: &mut Runtime, host: ObjectRef) {
    let native: NativeFn = Rc::new(|rt, _args| Ok(rt.current_this()));
    let mut properties = indexmap::IndexMap::new();
    crate::value::install_function_meta_props(&mut properties, "[Symbol.asyncIterator]", 0.0);
    let fn_obj = Object {
        proto: None,
        extensible: true,
        properties,
        internal_kind: InternalKind::Function(Box::new(FunctionInternals {
            name: "[Symbol.asyncIterator]".into(),
            length: 0,
            native,
            is_constructor: false,
            creation_realm: 0,
            roots: Vec::new(),
        })),
        ..Default::default()
    };
    let fn_id = rt.alloc_object(fn_obj);
    rt.obj_mut(host).dict_mut().insert(
        crate::value::PropertyKey::String("@@asyncIterator".into()),
        crate::value::PropertyDescriptor {
            value: Value::Object(fn_id),
            writable: true,
            enumerable: false,
            configurable: true,
            getter: None,
            setter: None,
        },
    );

    let dispose_native: NativeFn = Rc::new(|rt, _args| {
        let outer = crate::promise::new_promise(rt);
        let this_v = rt.current_this();
        let return_v = match rt.spec_get(&this_v, "return") {
            Ok(v) => v,
            Err(e) => {
                async_iterator_dispose_reject(rt, outer, e);
                return Ok(Value::Object(outer));
            }
        };
        if matches!(return_v, Value::Undefined | Value::Null) {
            crate::promise::resolve_promise(rt, outer, Value::Undefined);
            return Ok(Value::Object(outer));
        }
        if !rt.is_callable(&return_v) {
            async_iterator_dispose_reject(
                rt,
                outer,
                RuntimeError::TypeError("AsyncIterator asyncDispose return is not callable".into()),
            );
            return Ok(Value::Object(outer));
        }

        let result = match rt.call_function(return_v, this_v, vec![]) {
            Ok(v) => v,
            Err(e) => {
                async_iterator_dispose_reject(rt, outer, e);
                return Ok(Value::Object(outer));
            }
        };
        let wrapped = match rt.promise_resolve_via(&result) {
            Ok(Value::Object(id)) => id,
            Ok(v) => {
                crate::promise::resolve_promise(rt, outer, v);
                return Ok(Value::Object(outer));
            }
            Err(e) => {
                async_iterator_dispose_reject(rt, outer, e);
                return Ok(Value::Object(outer));
            }
        };
        async_iterator_dispose_adopt(rt, wrapped, outer);
        Ok(Value::Object(outer))
    });
    let mut dispose_properties = indexmap::IndexMap::new();
    crate::value::install_function_meta_props(
        &mut dispose_properties,
        "[Symbol.asyncDispose]",
        0.0,
    );
    let dispose_fn_obj = Object {
        proto: None,
        extensible: true,
        properties: dispose_properties,
        internal_kind: InternalKind::Function(Box::new(FunctionInternals {
            name: "[Symbol.asyncDispose]".into(),
            length: 0,
            native: dispose_native,
            is_constructor: false,
            creation_realm: 0,
            roots: Vec::new(),
        })),
        ..Default::default()
    };
    let dispose_fn_id = rt.alloc_object(dispose_fn_obj);

    rt.obj_mut(host).dict_mut().insert(
        crate::value::PropertyKey::String("@@asyncDispose".into()),
        crate::value::PropertyDescriptor {
            value: Value::Object(dispose_fn_id),
            writable: true,
            enumerable: false,
            configurable: true,
            getter: None,
            setter: None,
        },
    );
}

fn install_async_generator_proto(rt: &mut Runtime, host: ObjectRef) {
    register_intrinsic_method(rt, host, "next", 1, |rt, args| {
        let this_id = match async_generator_this(rt, "AsyncGenerator.prototype.next") {
            Ok(id) => id,
            Err(RuntimeError::TypeError(message)) => {
                return Ok(async_generator_rejected_type_error(rt, message))
            }
            Err(e) => return Err(e),
        };
        let sent = args.first().cloned().unwrap_or(Value::Undefined);

        Ok(rt.async_generator_step_or_enqueue(
            this_id,
            crate::value::AsyncGenRequestKind::Next,
            sent,
        ))
    });
    register_intrinsic_method(rt, host, "return", 1, |rt, args| {
        let this_id = match async_generator_this(rt, "AsyncGenerator.prototype.return") {
            Ok(id) => id,
            Err(RuntimeError::TypeError(message)) => {
                return Ok(async_generator_rejected_type_error(rt, message))
            }
            Err(e) => return Err(e),
        };
        let value = args.first().cloned().unwrap_or(Value::Undefined);
        Ok(rt.async_generator_step_or_enqueue(
            this_id,
            crate::value::AsyncGenRequestKind::Return,
            value,
        ))
    });
    register_intrinsic_method(rt, host, "throw", 1, |rt, args| {
        let this_id = match async_generator_this(rt, "AsyncGenerator.prototype.throw") {
            Ok(id) => id,
            Err(RuntimeError::TypeError(message)) => {
                return Ok(async_generator_rejected_type_error(rt, message))
            }
            Err(e) => return Err(e),
        };
        let thrown = args.first().cloned().unwrap_or(Value::Undefined);
        Ok(rt.async_generator_step_or_enqueue(
            this_id,
            crate::value::AsyncGenRequestKind::Throw,
            thrown,
        ))
    });
}

fn install_object_proto(rt: &mut Runtime, host: ObjectRef) {

    register_intrinsic_method(rt, host, "toString", 0, |rt, args| {
        crate::generated::object_prototype_to_string(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "hasOwnProperty", 1, |rt, args| {
        crate::generated::object_prototype_has_own_property(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "valueOf", 0, |rt, _args| {

        rt.to_object_strict_via(&rt.current_this())
    });

    register_intrinsic_method(rt, host, "__defineGetter__", 2, |rt, args| {
        crate::generated::object_proto_define_getter(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "__defineSetter__", 2, |rt, args| {
        crate::generated::object_proto_define_setter(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "__lookupGetter__", 1, |rt, args| {
        crate::generated::object_proto_lookup_getter(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "__lookupSetter__", 1, |rt, args| {
        crate::generated::object_proto_lookup_setter(rt, rt.current_this(), args)
    });

    register_intrinsic_method(rt, host, "propertyIsEnumerable", 1, |rt, args| {
        crate::generated::object_prototype_property_is_enumerable(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "isPrototypeOf", 1, |rt, args| {

        let v = args.first().cloned().unwrap_or(Value::Undefined);
        if !matches!(v, Value::Object(_)) {
            return Ok(Value::Boolean(false));
        }

        let o = rt.to_object_strict_via(&rt.current_this())?;
        crate::generated::object_prototype_is_prototype_of(rt, o, args)
    });
    register_intrinsic_method(rt, host, "toLocaleString", 0, |rt, args| {
        crate::generated::object_prototype_to_locale_string(rt, rt.current_this(), args)
    });
}

fn install_array_proto(rt: &mut Runtime, host: ObjectRef) {

    register_intrinsic_method(rt, host, "toString", 0, |rt, args| {
        crate::generated::array_prototype_to_string(rt, rt.current_this(), args)
    });

    register_intrinsic_method(rt, host, "push", 1, |rt, args| {
        crate::generated::array_prototype_push(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "pop", 0, |rt, args| {
        crate::generated::array_prototype_pop(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "shift", 0, |rt, args| {
        crate::generated::array_prototype_shift(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "unshift", 1, |rt, args| {
        crate::generated::array_prototype_unshift(rt, rt.current_this(), args)
    });

    register_intrinsic_method(rt, host, "indexOf", 1, |rt, args| {
        crate::generated::array_prototype_index_of(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "includes", 1, |rt, args| {
        crate::generated::array_prototype_includes(rt, rt.current_this(), args)
    });

    register_intrinsic_method(rt, host, "reverse", 0, |rt, args| {
        crate::generated::array_prototype_reverse(rt, rt.current_this(), args)
    });

    register_intrinsic_method(rt, host, "slice", 2, |rt, args| {
        crate::generated::array_prototype_slice(rt, rt.current_this(), args)
    });

    register_intrinsic_method(rt, host, "splice", 2, |rt, args| {
        crate::generated::array_prototype_splice(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "concat", 1, |rt, args| {
        crate::generated::array_prototype_concat(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "join", 1, |rt, args| {
        crate::generated::array_prototype_join(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "at", 1, |rt, args| {
        crate::generated::array_prototype_at(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "fill", 1, |rt, args| {
        crate::generated::array_prototype_fill(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "flat", 0, |rt, args| {
        crate::generated::array_prototype_flat(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "flatMap", 1, |rt, args| {
        crate::generated::array_prototype_flat_map(rt, rt.current_this(), args)
    });

    register_intrinsic_method(rt, host, "map", 1, |rt, args| {
        if let Some(result) = rt.try_array_proto_map_fast(args) {
            return result;
        }
        crate::generated::array_prototype_map(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "forEach", 1, |rt, args| {

        crate::generated::array_prototype_for_each(rt, rt.current_this(), args)
    });

    register_intrinsic_method(rt, host, "filter", 1, |rt, args| {
        if let Some(result) = rt.try_array_proto_filter_fast(args) {
            return result;
        }
        crate::generated::array_prototype_filter(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "reduce", 1, |rt, args| {
        if let Some(result) = rt.try_array_proto_reduce_fast(args) {
            return result;
        }
        crate::generated::array_prototype_reduce(rt, rt.current_this(), args)
    });

    register_intrinsic_method(rt, host, "find", 1, |rt, args| {
        crate::generated::array_prototype_find(rt, rt.current_this(), args)
    });

    register_intrinsic_method(rt, host, "some", 1, |rt, args| {
        crate::generated::array_prototype_some(rt, rt.current_this(), args)
    });

    register_intrinsic_method(rt, host, "sort", 1, |rt, args| {
        crate::generated::array_prototype_sort(rt, rt.current_this(), args)
    });

    register_intrinsic_method(rt, host, "every", 1, |rt, args| {
        crate::generated::array_prototype_every(rt, rt.current_this(), args)
    });

    register_intrinsic_method(rt, host, "entries", 0, |rt, args| {
        crate::generated::array_prototype_entries(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "keys", 0, |rt, args| {
        crate::generated::array_prototype_keys(rt, rt.current_this(), args)
    });
    let values_fn = register_intrinsic_method(rt, host, "values", 0, |rt, args| {
        crate::generated::array_prototype_values(rt, rt.current_this(), args)
    });
    rt.intrinsic_array_iterator_method_id = Some(values_fn);
    rt.obj_mut(host).dict_mut().insert(
        crate::value::PropertyKey::String("@@iterator".to_string()),
        crate::value::PropertyDescriptor {
            value: Value::Object(values_fn),
            writable: true,
            enumerable: false,
            configurable: true,
            getter: None,
            setter: None,
        },
    );

    register_intrinsic_method(rt, host, "findIndex", 1, |rt, args| {
        crate::generated::array_prototype_find_index(rt, rt.current_this(), args)
    });

    register_intrinsic_method(rt, host, "findLast", 1, |rt, args| {
        crate::generated::array_prototype_find_last(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "findLastIndex", 1, |rt, args| {
        crate::generated::array_prototype_find_last_index(rt, rt.current_this(), args)
    });

    register_intrinsic_method(rt, host, "reduceRight", 1, |rt, args| {
        crate::generated::array_prototype_reduce_right(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "lastIndexOf", 1, |rt, args| {
        crate::generated::array_prototype_last_index_of(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "copyWithin", 2, |rt, args| {
        crate::generated::array_prototype_copy_within(rt, rt.current_this(), args)
    });

    register_intrinsic_method(rt, host, "toReversed", 0, |rt, args| {
        crate::generated::array_prototype_to_reversed(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "toSorted", 1, |rt, args| {
        crate::generated::array_prototype_to_sorted(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "toSpliced", 2, |rt, args| {
        crate::generated::array_prototype_to_spliced(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "with", 2, |rt, args| {
        crate::generated::array_prototype_with(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "toLocaleString", 0, |rt, args| {
        crate::generated::array_prototype_to_locale_string(rt, rt.current_this(), args)
    });

    let unscopables = rt.alloc_object(Object::new_ordinary());
    rt.obj_mut(unscopables).proto = None;
    for name in [
        "at",
        "copyWithin",
        "entries",
        "fill",
        "find",
        "findIndex",
        "findLast",
        "findLastIndex",
        "flat",
        "flatMap",
        "includes",
        "keys",
        "toReversed",
        "toSorted",
        "toSpliced",
        "values",
    ] {
        rt.obj_mut(unscopables)
            .set_own(name.into(), Value::Boolean(true));
    }
    rt.obj_mut(host).dict_mut().insert(
        crate::value::PropertyKey::String("@@unscopables".to_string()),
        crate::value::PropertyDescriptor {
            value: Value::Object(unscopables),
            writable: false,
            enumerable: false,
            configurable: true,
            getter: None,
            setter: None,
        },
    );
}

fn clamp_index(i: i64, len: i64) -> i64 {
    let v = if i < 0 { (len + i).max(0) } else { i.min(len) };
    v
}

fn is_regexp_like(rt: &mut Runtime, v: &Value) -> Result<bool, RuntimeError> {
    let id = match v {
        Value::Object(id) => *id,
        _ => return Ok(false),
    };
    let matcher = rt.read_property(id, "@@match")?;
    match matcher {
        Value::Undefined => Ok(matches!(rt.obj(id).internal_kind, InternalKind::RegExp(_))),
        _ => Ok(crate::abstract_ops::to_boolean(&matcher)),
    }
}

pub(crate) fn get_well_known_property_exact(
    rt: &mut Runtime,
    target: ObjectRef,
    wks_full: &str,
) -> Result<Value, RuntimeError> {
    let short = wks_full.strip_prefix("@@").unwrap_or(wks_full);
    let symbol = match rt.global_get("Symbol") {
        Value::Object(symbol_ctor) => match rt.object_get(symbol_ctor, short) {
            Value::Symbol(rc) => Some(rc),
            _ => None,
        },
        _ => None,
    };
    let mut cur = Some(target);
    while let Some(id) = cur {
        let hit = {
            let o = rt.obj(id);
            if let Some(rc) = symbol.as_ref() {
                o.properties
                    .get(&crate::value::PropertyKey::Symbol(rc.clone()))
                    .or_else(|| {
                        o.properties.iter().find_map(|(key, desc)| match key {
                            crate::value::PropertyKey::Symbol(stored)
                                if stored.as_str() == rc.as_str() =>
                            {
                                Some(desc)
                            }
                            _ => None,
                        })
                    })

                    .or_else(|| o.get_own(wks_full))
                    .cloned()
            } else {
                o.get_own(wks_full).cloned()
            }
        };
        if let Some(desc) = hit {
            if let Some(getter) = desc.getter {
                if !matches!(getter, Value::Undefined) {
                    return rt.call_function(getter, Value::Object(target), Vec::new());
                }
            }
            return Ok(desc.value);
        }
        cur = rt.obj(id).proto;
    }
    Ok(Value::Undefined)
}

fn get_string_property_exact(
    rt: &mut Runtime,
    target: ObjectRef,
    key: &str,
) -> Result<Value, RuntimeError> {
    let mut cur = Some(target);
    while let Some(id) = cur {
        let hit = rt.obj(id).get_own(key).cloned();
        if let Some(desc) = hit {
            if let Some(getter) = desc.getter {
                if !matches!(getter, Value::Undefined) {
                    return rt.call_function(getter, Value::Object(target), Vec::new());
                }
            }
            return Ok(desc.value);
        }
        cur = rt.obj(id).proto;
    }
    Ok(Value::Undefined)
}

fn install_string_proto(rt: &mut Runtime, host: ObjectRef) {

    register_generated_string_to_upper_case(rt, host);
    register_generated_string_to_lower_case(rt, host);
    register_intrinsic_method(rt, host, "toLocaleLowerCase", 0, |rt, args| {
        let this = rt.current_this();
        crate::generated::string_prototype_to_locale_lower_case(rt, this, args)
    });
    register_intrinsic_method(rt, host, "toLocaleUpperCase", 0, |rt, args| {
        let this = rt.current_this();
        crate::generated::string_prototype_to_locale_upper_case(rt, this, args)
    });

    register_generated_string_trim(rt, host);
    register_generated_string_trim_start(rt, host);
    let trim_start = rt.object_get(host, "trimStart");
    rt.obj_mut(host)
        .set_own_internal("trimLeft".into(), trim_start);
    register_generated_string_trim_end(rt, host);
    let trim_end = rt.object_get(host, "trimEnd");
    rt.obj_mut(host)
        .set_own_internal("trimRight".into(), trim_end);
    register_intrinsic_method(rt, host, "isWellFormed", 0, |rt, _args| {
        let this = rt.current_this();
        rt.string_proto_is_well_formed_via(&this)
    });
    register_intrinsic_method(rt, host, "toWellFormed", 0, |rt, _args| {
        let this = rt.current_this();
        rt.string_proto_to_well_formed_via(&this)
    });

    register_intrinsic_method(rt, host, "normalize", 0, |rt, args| {
        let this = rt.current_this();
        let form = args.first().cloned().unwrap_or(Value::Undefined);
        crate::generated::string_prototype_normalize(rt, this, std::slice::from_ref(&form))
    });

    register_generated_string_char_at(rt, host);
    register_generated_string_char_code_at(rt, host);
    register_intrinsic_method(rt, host, "concat", 1, |rt, args| {
        let this = rt.current_this();
        crate::generated::string_prototype_concat(rt, this, args)
    });

    register_intrinsic_method(rt, host, "localeCompare", 1, |rt, args| {
        let this = rt.current_this();
        crate::generated::string_prototype_locale_compare(rt, this, args)
    });

    register_generated_string_code_point_at(rt, host);

    register_generated_string_slice(rt, host);
    register_generated_string_substr(rt, host);
    register_generated_string_substring(rt, host);
    register_generated_string_index_of(rt, host);
    register_generated_string_last_index_of(rt, host);
    register_generated_string_includes(rt, host);
    register_generated_string_starts_with(rt, host);
    register_generated_string_ends_with(rt, host);
    register_intrinsic_method(rt, host, "split", 2, |rt, args| {
        let this = rt.current_this();
        let sep = args.first().cloned().unwrap_or(Value::Undefined);
        let limit = args.get(1).cloned().unwrap_or(Value::Undefined);
        crate::generated::string_prototype_split(rt, this, &[sep, limit])
    });

    register_generated_string_repeat(rt, host);

    register_intrinsic_method(rt, host, "matchAll", 1, |rt, args| {
        let receiver = rt.current_this();
        rt.require_object_coercible(&receiver)?;

        let arg = args.first().cloned().unwrap_or(Value::Undefined);
        if !matches!(arg, Value::Undefined | Value::Null) {
            if let Value::Object(arg_id) = &arg {
                if rt.is_regexp_like_via(&arg)? {
                    let flags = get_string_property_exact(rt, *arg_id, "flags")?;
                    rt.require_object_coercible(&flags)?;
                    if !rt.to_string_strict(&flags)?.contains('g') {
                        return Err(RuntimeError::TypeError(
                            "String.prototype.matchAll called with a non-global RegExp argument"
                                .into(),
                        ));
                    }
                }
                let m = get_well_known_property_exact(rt, *arg_id, "@@matchAll")?;
                if !matches!(m, Value::Undefined | Value::Null) {
                    if !rt.is_callable(&m) {
                        return Err(RuntimeError::TypeError(
                            "String.prototype.matchAll: @@matchAll is not callable".into(),
                        ));
                    }
                    return rt.call_function(m, arg.clone(), vec![receiver]);
                }
            }
        }
        let s = rt.to_string_strict(&receiver)?;
        let regex_arg = args.first().cloned().unwrap_or(Value::Undefined);
        let ctor = rt.global_get("RegExp");
        let regex_v = rt.construct(
            ctor,
            vec![
                regex_arg,
                Value::String(Rc::new(crate::value::JsString::from("g"))),
            ],
        )?;
        let regex_id = match &regex_v {
            Value::Object(id) => *id,
            _ => {
                return Err(crate::interp::RuntimeError::TypeError(
                    "matchAll RegExpCreate did not return an object".into(),
                ))
            }
        };
        let matcher = get_well_known_property_exact(rt, regex_id, "@@matchAll")?;
        if !rt.is_callable(&matcher) {
            return Err(RuntimeError::TypeError(
                "String.prototype.matchAll: created RegExp @@matchAll is not callable".into(),
            ));
        }
        rt.call_function(
            matcher,
            regex_v,
            vec![Value::String(Rc::new(crate::value::JsString::from(s)))],
        )
    });

    register_generated_string_pad_start(rt, host);
    register_generated_string_pad_end(rt, host);
    register_intrinsic_method(rt, host, "replace", 2, |rt, args| {
        let this = rt.current_this();
        let search = args.first().cloned().unwrap_or(Value::Undefined);
        let repl = args.get(1).cloned().unwrap_or(Value::Undefined);
        crate::generated::string_prototype_replace(rt, this, &[search, repl])
    });
    register_intrinsic_method(rt, host, "replaceAll", 2, |rt, args| {
        let this = rt.current_this();
        let search = args.first().cloned().unwrap_or(Value::Undefined);
        let repl = args.get(1).cloned().unwrap_or(Value::Undefined);
        crate::generated::string_prototype_replace_all(rt, this, &[search, repl])
    });
    register_generated_string_at(rt, host);
    register_intrinsic_method(rt, host, "toString", 0, |rt, _args| {

        let this = rt.current_this();
        let t = rt.unwrap_primitive(&this);
        match t {
            Value::String(s) => Ok(Value::String(s)),
            _ => Err(RuntimeError::TypeError(
                "String.prototype.toString: this is not a String".into(),
            )),
        }
    });
    register_intrinsic_method(rt, host, "valueOf", 0, |rt, _args| {
        let this = rt.current_this();
        let t = rt.unwrap_primitive(&this);
        match t {
            Value::String(s) => Ok(Value::String(s)),
            _ => Err(RuntimeError::TypeError(
                "String.prototype.valueOf: this is not a String".into(),
            )),
        }
    });
    register_intrinsic_method(rt, host, "@@iterator", 0, |rt, _args| {

        let this = rt.current_this();
        rt.require_object_coercible(&this)?;
        let js = rt.to_js_string_strict(&this)?;
        Ok(Value::Object(crate::iterator::make_string_iterator(
            rt, &js,
        )))
    });
    install_annex_b_string_html_methods(rt, host);
}

fn install_annex_b_string_html_methods(rt: &mut Runtime, host: ObjectRef) {
    for (name, length, tag, attr) in [
        ("anchor", 1, "a", Some("name")),
        ("big", 0, "big", None),
        ("blink", 0, "blink", None),
        ("bold", 0, "b", None),
        ("fixed", 0, "tt", None),
        ("fontcolor", 1, "font", Some("color")),
        ("fontsize", 1, "font", Some("size")),
        ("italics", 0, "i", None),
        ("link", 1, "a", Some("href")),
        ("small", 0, "small", None),
        ("strike", 0, "strike", None),
        ("sub", 0, "sub", None),
        ("sup", 0, "sup", None),
    ] {
        register_annex_b_string_html_method(rt, host, name, length, tag, attr);
    }
}

fn register_annex_b_string_html_method(
    rt: &mut Runtime,
    host: ObjectRef,
    name: &str,
    length: u32,
    tag: &'static str,
    attr: Option<&'static str>,
) {
    register_intrinsic_method(rt, host, name, length, move |rt, args| {
        let this = rt.current_this();
        rt.require_object_coercible(&this)?;
        let s = rt.to_string_strict(&this)?;
        let result = match attr {
            Some(attr_name) => {
                let value = args.first().cloned().unwrap_or(Value::Undefined);
                let attr_value = rt.to_string_strict(&value)?;
                format!(
                    "<{tag} {attr_name}=\"{}\">{s}</{tag}>",
                    annex_b_html_escape_double_quoted_attr(&attr_value)
                )
            }
            None => format!("<{tag}>{s}</{tag}>"),
        };
        Ok(Value::String(Rc::new(crate::value::JsString::from(result))))
    });
}

fn annex_b_html_escape_double_quoted_attr(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

fn install_function_proto(rt: &mut Runtime, host: ObjectRef) {
    rt.obj_mut(host).dict_mut().insert(
        crate::value::PropertyKey::String("length".into()),
        crate::value::PropertyDescriptor {
            value: Value::Number(0.0),
            writable: false,
            enumerable: false,
            configurable: true,
            getter: None,
            setter: None,
        },
    );
    rt.obj_mut(host).dict_mut().insert(
        crate::value::PropertyKey::String("name".into()),
        crate::value::PropertyDescriptor {
            value: Value::String(Rc::new(crate::value::JsString::from(String::new()))),
            writable: false,
            enumerable: false,
            configurable: true,
            getter: None,
            setter: None,
        },
    );

    register_intrinsic_method(rt, host, "toString", 0, |rt, args| {
        crate::generated::function_prototype_to_string(rt, rt.current_this(), args)
    });
    register_intrinsic_method(rt, host, "call", 1, |rt, args| {
        let f = rt.current_this();

        if !rt.is_callable(&f) {
            return Err(RuntimeError::TypeError(
                "Function.prototype.call called on non-callable receiver".into(),
            ));
        }
        let this_arg = args.first().cloned().unwrap_or(Value::Undefined);
        let rest: Vec<Value> = args.iter().skip(1).cloned().collect();
        if let Some(result) = rt.try_function_call_captured_add_store_forward(&f, rest.as_slice()) {
            return result;
        }

        rt.pending_tail_call = Some((f, this_arg, rest));
        Ok(Value::Undefined)
    });
    register_intrinsic_method(rt, host, "apply", 2, |rt, args| {
        let f = rt.current_this();

        if !rt.is_callable(&f) {
            return Err(RuntimeError::TypeError(
                "Function.prototype.apply called on non-callable receiver".into(),
            ));
        }
        let this_arg = args.first().cloned().unwrap_or(Value::Undefined);
        let arr_v = args.get(1).cloned().unwrap_or(Value::Undefined);

        let call_args: Vec<Value> = match arr_v {
            Value::Null | Value::Undefined => Vec::new(),
            Value::Object(aid) => {
                let av = Value::Object(aid);
                rt.create_list_from_array_like(&av)?
            }
            _ => {
                return Err(RuntimeError::TypeError(
                    "apply: argsArray must be an Array".into(),
                ))
            }
        };

        rt.pending_tail_call = Some((f, this_arg, call_args));
        Ok(Value::Undefined)
    });
    register_intrinsic_method(rt, host, "bind", 1, |rt, args| {
        let target = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Err(RuntimeError::TypeError("bind: this is not callable".into())),
        };

        if !rt.is_callable(&Value::Object(target)) {
            return Err(RuntimeError::TypeError("bind: this is not callable".into()));
        }
        let bound_this = args.first().cloned().unwrap_or(Value::Undefined);
        let bound_args: Vec<Value> = args.iter().skip(1).cloned().collect();

        let n_bound = bound_args.len() as f64;

        let length_key = Value::String(std::rc::Rc::new(crate::value::JsString::from("length")));
        let has_own_length = !matches!(
            rt.object_get_own_property_descriptor_via(&Value::Object(target), &length_key)?,
            Value::Undefined
        );
        let bound_length = if has_own_length {
            match rt.spec_get(&Value::Object(target), "length")? {
                Value::Number(n) if n == f64::INFINITY => f64::INFINITY,
                Value::Number(n) => {
                    let int = if n.is_nan() { 0.0 } else { n.trunc() };
                    (int - n_bound).max(0.0)
                }
                _ => 0.0,
            }
        } else {
            0.0
        };

        let target_name = match rt.spec_get(&Value::Object(target), "name")? {
            Value::String(s) => (*s).clone(),
            _ => crate::value::JsString::wellformed(String::new()),
        };
        let bound_name = format!("bound {}", target_name);
        let mut properties = indexmap::IndexMap::new();
        crate::value::install_function_meta_props(&mut properties, &bound_name, bound_length);
        let target_proto = rt.reflect_get_prototype_of_via(&Value::Object(target))?;
        let explicit_null_proto = matches!(target_proto, Value::Null);
        let bf = Object {
            proto: match target_proto {
                Value::Object(id) => Some(id),
                Value::Null => None,
                _ => None,
            },
            extensible: true,
            properties,
            internal_kind: InternalKind::BoundFunction(Box::new(BoundFunctionInternals {
                target,
                this: bound_this,
                args: bound_args,
            })),

            ..Default::default()
        };
        let id = if explicit_null_proto {
            rt.alloc_object_with_explicit_null_proto(bf)
        } else {
            rt.alloc_object(bf)
        };
        Ok(Value::Object(id))
    });
    install_function_has_instance(rt, host);

    let thrower = crate::intrinsics::make_native("%ThrowTypeError%", |_rt, _args| {
        Err(RuntimeError::TypeError(
            "'caller', 'callee', and 'arguments' properties may not be accessed on strict mode functions or the arguments objects for calls to them".into(),
        ))
    });
    let thrower_id = rt.alloc_object(thrower);
    crate::intrinsics::finalize_throw_type_error(rt, thrower_id);

    for poison in ["caller", "arguments"] {
        rt.obj_mut(host).dict_mut().insert(
            poison.into(),
            crate::value::PropertyDescriptor {
                value: Value::Undefined,
                writable: false,
                enumerable: false,
                configurable: true,
                getter: Some(Value::Object(thrower_id)),
                setter: Some(Value::Object(thrower_id)),
            },
        );
    }
}

fn ordinary_get_prototype_of_object(
    rt: &mut Runtime,
    id: ObjectRef,
) -> Result<Option<ObjectRef>, RuntimeError> {
    if let Some((target, handler)) = rt.proxy_target_handler_checked(id)? {
        let trap = rt.object_get(handler, "getPrototypeOf");
        if !matches!(trap, Value::Undefined) {
            if !rt.is_callable(&trap) {
                return Err(RuntimeError::TypeError(
                    "Proxy 'getPrototypeOf' trap is not callable".into(),
                ));
            }
            let handler_proto =
                rt.call_function(trap, Value::Object(handler), vec![Value::Object(target)])?;
            let proto = match handler_proto {
                Value::Object(pid) => Some(pid),
                Value::Null => None,
                _ => {
                    return Err(RuntimeError::TypeError(
                        "Proxy 'getPrototypeOf' trap returned non-Object non-Null".into(),
                    ))
                }
            };
            if !rt.obj(target).extensible && proto != rt.obj(target).proto {
                return Err(RuntimeError::TypeError(
                    "Proxy 'getPrototypeOf' trap returned proto inconsistent with non-extensible target".into(),
                ));
            }
            return Ok(proto);
        }
        return Ok(rt.obj(target).proto);
    }
    Ok(rt.obj(id).proto)
}

fn ordinary_has_instance(
    rt: &mut Runtime,
    constructor: Value,
    value: Value,
) -> Result<bool, RuntimeError> {
    if !rt.is_callable(&constructor) {
        return Ok(false);
    }
    if let Value::Object(constructor_id) = constructor {
        if let InternalKind::BoundFunction(bound) = &rt.obj(constructor_id).internal_kind {
            return ordinary_has_instance(rt, Value::Object(bound.target), value);
        }
        let Value::Object(value_id) = value else {
            return Ok(false);
        };
        let target_proto = match rt.spec_get(&Value::Object(constructor_id), "prototype")? {
            Value::Object(pid) => pid,
            _ => return Err(RuntimeError::TypeError(
                "Function.prototype[Symbol.hasInstance]: constructor prototype is not an object"
                    .into(),
            )),
        };
        let mut cur = value_id;
        while let Some(id) = ordinary_get_prototype_of_object(rt, cur)? {
            if id == target_proto {
                return Ok(true);
            }
            cur = id;
        }
    }
    Ok(false)
}

fn install_function_has_instance(rt: &mut Runtime, host: ObjectRef) {
    let native: NativeFn = Rc::new(|rt, args| {
        let value = args.first().cloned().unwrap_or(Value::Undefined);
        Ok(Value::Boolean(ordinary_has_instance(
            rt,
            rt.current_this(),
            value,
        )?))
    });
    let mut properties = indexmap::IndexMap::new();
    crate::value::install_function_meta_props(&mut properties, "[Symbol.hasInstance]", 1.0);
    let fn_obj = Object {
        proto: rt.function_prototype,
        extensible: true,
        properties,
        internal_kind: InternalKind::Function(Box::new(FunctionInternals {
            name: "[Symbol.hasInstance]".to_string(),
            length: 1,
            native,
            is_constructor: false,
            creation_realm: 0,
            roots: Vec::new(),
        })),

        ..Default::default()
    };
    let fn_id = rt.alloc_object(fn_obj);
    rt.obj_mut(host).dict_mut().insert(
        crate::value::PropertyKey::String("@@hasInstance".to_string()),
        crate::value::PropertyDescriptor {
            value: Value::Object(fn_id),
            writable: false,
            enumerable: false,
            configurable: false,
            getter: None,
            setter: None,
        },
    );
}

fn install_promise_proto(rt: &mut Runtime, host: ObjectRef) {

    register_intrinsic_method(rt, host, "then", 2, |rt, args| {
        let mut a: Vec<Value> = Vec::with_capacity(args.len() + 1);
        a.push(rt.current_this());
        a.extend(args.iter().cloned());
        crate::generated::promise_prototype_then(rt, rt.current_this(), &a)
    });
    register_intrinsic_method(rt, host, "catch", 1, |rt, args| {
        let mut a: Vec<Value> = Vec::with_capacity(args.len() + 1);
        a.push(rt.current_this());
        a.extend(args.iter().cloned());
        crate::generated::promise_prototype_catch(rt, rt.current_this(), &a)
    });
}

fn promise_then_impl(
    rt: &mut Runtime,
    source: ObjectRef,
    on_fulfilled: Option<Value>,
    on_rejected: Option<Value>,
) -> Result<Value, RuntimeError> {
    let chain = crate::promise::new_promise(rt);
    let (status, value) = {
        let s = rt.obj(source);
        match &s.internal_kind {
            InternalKind::Promise(ps) => (ps.status, ps.value.clone()),
            _ => {
                return Err(RuntimeError::TypeError(
                    "then: source is not a Promise".into(),
                ))
            }
        }
    };
    match status {
        PromiseStatus::Pending => {
            let src = rt.obj_mut(source);
            if let InternalKind::Promise(ps) = &mut src.internal_kind {
                ps.fulfill_reactions.push(PromiseReaction {
                    handler: on_fulfilled.map(crate::value::PromiseReactionHandler::Callable),
                    chain,
                    cap_resolve: None,
                    cap_reject: None,
                });
                ps.reject_reactions.push(PromiseReaction {
                    handler: on_rejected.map(crate::value::PromiseReactionHandler::Callable),
                    chain,
                    cap_resolve: None,
                    cap_reject: None,
                });
            }
        }
        PromiseStatus::Fulfilled => {
            enqueue_handler(rt, on_fulfilled, value, chain, false);
        }
        PromiseStatus::Rejected => {
            rt.pending_unhandled.remove(&source);
            enqueue_handler(rt, on_rejected, value, chain, true);
        }
    }
    Ok(Value::Object(chain))
}

fn enqueue_handler(
    rt: &mut Runtime,
    handler: Option<Value>,
    value: Value,
    chain: ObjectRef,
    is_rejected: bool,
) {
    let mut roots = Vec::new();
    if let Some(Value::Object(id)) = &handler {
        roots.push(*id);
    }
    if let Value::Object(id) = &value {
        roots.push(*id);
    }
    roots.push(chain);
    rt.enqueue_microtask_rooted("PromiseReactionJob", roots, move |rt| {
        match handler {
            Some(h) => match rt.call_function(h, Value::Undefined, vec![value]) {
                Ok(r) => {
                    crate::promise::resolve_promise(rt, chain, r);
                }
                Err(e) => {
                    let thrown = match e {
                        RuntimeError::Thrown(v) => v,
                        other => Value::String(Rc::new(crate::value::JsString::from(format!(
                            "{:?}",
                            other
                        )))),
                    };
                    crate::promise::reject_promise(rt, chain, thrown);
                }
            },
            None => {
                if is_rejected {
                    crate::promise::reject_promise(rt, chain, value);
                } else {
                    crate::promise::resolve_promise(rt, chain, value);
                }
            }
        }
        Ok(())
    });
}

fn install_number_proto(rt: &mut Runtime, host: ObjectRef) {

    register_intrinsic_method(rt, host, "valueOf", 0, |rt, _args| {
        let this = rt.current_this();
        crate::generated::number_prototype_value_of(rt, this, &[])
    });
    register_intrinsic_method(rt, host, "toString", 1, |rt, args| {
        crate::generated::number_prototype_to_string(rt, rt.current_this(), args)
    });

    register_intrinsic_method(rt, host, "toFixed", 1, |rt, args| {
        let this = rt.current_this();
        let digits = args.first().cloned().unwrap_or(Value::Undefined);
        crate::generated::number_prototype_to_fixed(rt, this, std::slice::from_ref(&digits))
    });

    register_intrinsic_method(rt, host, "toExponential", 1, |rt, args| {
        let this = rt.current_this();
        let digits = args.first().cloned().unwrap_or(Value::Undefined);
        crate::generated::number_prototype_to_exponential(rt, this, std::slice::from_ref(&digits))
    });

    register_intrinsic_method(rt, host, "toPrecision", 1, |rt, args| {
        let this = rt.current_this();
        let precision = args.first().cloned().unwrap_or(Value::Undefined);
        crate::generated::number_prototype_to_precision(rt, this, std::slice::from_ref(&precision))
    });
    register_intrinsic_method(rt, host, "toLocaleString", 0, |rt, args| {
        crate::generated::number_prototype_to_locale_string(rt, rt.current_this(), args)
    });

}

fn arg_string(args: &[Value], i: usize) -> String {
    args.get(i)
        .map(|v| abstract_ops::to_string(v).as_str().to_string())
        .unwrap_or_default()
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
            is_constructor: true,
            creation_realm: 0,
            roots: Vec::new(),
        })),

        ..Default::default()
    };
    let fn_id = rt.alloc_object(fn_obj);
    rt.object_set(host, name.into(), Value::Object(fn_id));
}

pub(crate) fn to_array_this(rt: &mut Runtime) -> Result<ObjectRef, RuntimeError> {
    match rt.current_this() {
        Value::Object(id) => Ok(id),
        Value::Undefined | Value::Null => Err(RuntimeError::TypeError(
            "Array.prototype method called on null or undefined".into(),
        )),
        Value::Boolean(b) => {

            let mut o = Object::new_ordinary();
            o.set_own_internal("__primitive".into(), Value::Boolean(b));

            if let Value::Object(bid) = rt.global_get("Boolean") {
                if let Value::Object(p) = rt.object_get(bid, "prototype") {
                    o.proto = Some(p);
                }
            }
            Ok(rt.alloc_object(o))
        }
        Value::Number(n) => {
            let mut o = Object::new_ordinary();
            o.set_own_internal("__primitive".into(), Value::Number(n));
            if let Some(p) = rt.number_prototype {
                o.proto = Some(p);
            }
            Ok(rt.alloc_object(o))
        }
        Value::String(s) => {

            let mut o = Object::new_ordinary();
            o.set_own_internal("__primitive__".into(), Value::String(s.clone()));
            o.internal_kind = crate::value::InternalKind::StringWrapper(Value::String(s.clone()));
            let units = s.code_units();
            for (i, unit) in units.iter().enumerate() {
                o.set_own_string_index(
                    i.to_string(),
                    Value::String(Rc::new(crate::value::JsString::from_code_units(vec![
                        *unit,
                    ]))),
                );
            }
            o.set_own_frozen("length".into(), Value::Number(units.len() as f64));
            if let Some(p) = rt.string_prototype {
                o.proto = Some(p);
            }
            Ok(rt.alloc_object(o))
        }
        Value::BigInt(_) | Value::Symbol(_) => Err(RuntimeError::TypeError(
            "Array.prototype method called on BigInt/Symbol".into(),
        )),
    }
}

fn register_intrinsic_method<F>(
    rt: &mut Runtime,
    host: ObjectRef,
    name: &str,
    length: u32,
    f: F,
) -> ObjectRef
where
    F: Fn(&mut Runtime, &[Value]) -> Result<Value, RuntimeError> + 'static,
{
    let native: NativeFn = Rc::new(f);
    let mut properties = indexmap::IndexMap::new();

    let display_name = if let Some(sym) = name.strip_prefix("@@") {
        format!("[Symbol.{sym}]")
    } else {
        name.to_string()
    };
    crate::value::install_function_meta_props(&mut properties, &display_name, length as f64);
    let fn_obj = Object {
        proto: None,
        extensible: true,
        properties,
        internal_kind: InternalKind::Function(Box::new(FunctionInternals {
            name: display_name.clone(),
            length,
            native,
            is_constructor: false,
            creation_realm: 0,
            roots: Vec::new(),
        })),

        ..Default::default()
    };
    let fn_id = rt.alloc_object(fn_obj);
    rt.obj_mut(host).dict_mut().insert(
        crate::value::PropertyKey::String(name.to_string()),
        crate::value::PropertyDescriptor {
            value: Value::Object(fn_id),
            writable: true,
            enumerable: false,
            configurable: true,
            getter: None,
            setter: None,
        },
    );
    fn_id
}

fn register_generated_string_char_code_at(rt: &mut Runtime, host: ObjectRef) -> ObjectRef {
    let spec = crate::native_api_manifest::string_char_code_at_generated_registration_spec();
    register_intrinsic_method(rt, host, spec.property, spec.length, |rt, args| {
        let this = rt.current_this();
        let args = crate::native_api_manifest::string_char_code_at_generated_validation_args(args);
        crate::generated::string_prototype_char_code_at(rt, this, &args)
    })
}

fn register_generated_string_to_lower_case(rt: &mut Runtime, host: ObjectRef) -> ObjectRef {
    let spec =
        crate::native_api_manifest_generated::string_to_lower_case_generated_registration_spec();
    register_intrinsic_method(rt, host, spec.property, spec.length, |rt, args| {
        let this = rt.current_this();
        let args =
            crate::native_api_manifest_generated::string_to_lower_case_generated_validation_args(
                args,
            );
        crate::generated::string_prototype_to_lower_case(rt, this, &args)
    })
}

fn register_generated_string_to_upper_case(rt: &mut Runtime, host: ObjectRef) -> ObjectRef {
    let spec =
        crate::native_api_manifest_generated::string_to_upper_case_generated_registration_spec();
    register_intrinsic_method(rt, host, spec.property, spec.length, |rt, args| {
        let this = rt.current_this();
        let args =
            crate::native_api_manifest_generated::string_to_upper_case_generated_validation_args(
                args,
            );
        crate::generated::string_prototype_to_upper_case(rt, this, &args)
    })
}

fn register_generated_string_trim(rt: &mut Runtime, host: ObjectRef) -> ObjectRef {
    let spec = crate::native_api_manifest_generated::string_trim_generated_registration_spec();
    register_intrinsic_method(rt, host, spec.property, spec.length, |rt, args| {
        let this = rt.current_this();
        let args =
            crate::native_api_manifest_generated::string_trim_generated_validation_args(args);
        crate::generated::string_prototype_trim(rt, this, &args)
    })
}

fn register_generated_string_trim_start(rt: &mut Runtime, host: ObjectRef) -> ObjectRef {
    let spec =
        crate::native_api_manifest_generated::string_trim_start_generated_registration_spec();
    register_intrinsic_method(rt, host, spec.property, spec.length, |rt, args| {
        let this = rt.current_this();
        let args =
            crate::native_api_manifest_generated::string_trim_start_generated_validation_args(args);
        crate::generated::string_prototype_trim_start(rt, this, &args)
    })
}

fn register_generated_string_trim_end(rt: &mut Runtime, host: ObjectRef) -> ObjectRef {
    let spec = crate::native_api_manifest_generated::string_trim_end_generated_registration_spec();
    register_intrinsic_method(rt, host, spec.property, spec.length, |rt, args| {
        let this = rt.current_this();
        let args =
            crate::native_api_manifest_generated::string_trim_end_generated_validation_args(args);
        crate::generated::string_prototype_trim_end(rt, this, &args)
    })
}

fn register_generated_string_repeat(rt: &mut Runtime, host: ObjectRef) -> ObjectRef {
    let spec = crate::native_api_manifest_generated::string_repeat_generated_registration_spec();
    register_intrinsic_method(rt, host, spec.property, spec.length, |rt, args| {
        let this = rt.current_this();
        let args =
            crate::native_api_manifest_generated::string_repeat_generated_validation_args(args);
        crate::generated::string_prototype_repeat(rt, this, &args)
    })
}

fn register_generated_string_pad_start(rt: &mut Runtime, host: ObjectRef) -> ObjectRef {
    let spec = crate::native_api_manifest_generated::string_pad_start_generated_registration_spec();
    register_intrinsic_method(rt, host, spec.property, spec.length, |rt, args| {
        let this = rt.current_this();
        let target =
            crate::native_api_manifest_generated::string_pad_start_generated_validation_args(args)
                [0]
            .clone();
        let pad = args.get(1).cloned().unwrap_or(Value::Undefined);
        crate::generated::string_prototype_pad_start(rt, this, &[target, pad])
    })
}

fn register_generated_string_pad_end(rt: &mut Runtime, host: ObjectRef) -> ObjectRef {
    let spec = crate::native_api_manifest_generated::string_pad_end_generated_registration_spec();
    register_intrinsic_method(rt, host, spec.property, spec.length, |rt, args| {
        let this = rt.current_this();
        let target =
            crate::native_api_manifest_generated::string_pad_end_generated_validation_args(args)[0]
                .clone();
        let pad = args.get(1).cloned().unwrap_or(Value::Undefined);
        crate::generated::string_prototype_pad_end(rt, this, &[target, pad])
    })
}

fn register_generated_string_index_of(rt: &mut Runtime, host: ObjectRef) -> ObjectRef {
    let spec = crate::native_api_manifest_generated::string_index_of_generated_registration_spec();
    register_intrinsic_method(rt, host, spec.property, spec.length, |rt, args| {
        let this = rt.current_this();
        let a =
            crate::native_api_manifest_generated::string_index_of_generated_validation_args(args)
                [0]
            .clone();
        let b = args.get(1).cloned().unwrap_or(Value::Undefined);
        crate::generated::string_prototype_index_of(rt, this, &[a, b])
    })
}

fn register_generated_string_last_index_of(rt: &mut Runtime, host: ObjectRef) -> ObjectRef {
    let spec =
        crate::native_api_manifest_generated::string_last_index_of_generated_registration_spec();
    register_intrinsic_method(rt, host, spec.property, spec.length, |rt, args| {
        let this = rt.current_this();
        let a =
            crate::native_api_manifest_generated::string_last_index_of_generated_validation_args(
                args,
            )[0]
            .clone();
        let b = args.get(1).cloned().unwrap_or(Value::Undefined);
        crate::generated::string_prototype_last_index_of(rt, this, &[a, b])
    })
}

fn register_generated_string_char_at(rt: &mut Runtime, host: ObjectRef) -> ObjectRef {
    let spec = crate::native_api_manifest_generated::string_char_at_generated_registration_spec();
    register_intrinsic_method(rt, host, spec.property, spec.length, |rt, args| {
        let this = rt.current_this();
        let args =
            crate::native_api_manifest_generated::string_char_at_generated_validation_args(args);
        crate::generated::string_prototype_char_at(rt, this, &args)
    })
}

fn register_generated_string_at(rt: &mut Runtime, host: ObjectRef) -> ObjectRef {
    let spec = crate::native_api_manifest_generated::string_at_generated_registration_spec();
    register_intrinsic_method(rt, host, spec.property, spec.length, |rt, args| {
        let this = rt.current_this();
        let args = crate::native_api_manifest_generated::string_at_generated_validation_args(args);
        crate::generated::string_prototype_at(rt, this, &args)
    })
}

fn register_generated_string_slice(rt: &mut Runtime, host: ObjectRef) -> ObjectRef {
    let spec = crate::native_api_manifest_generated::string_slice_generated_registration_spec();
    register_intrinsic_method(rt, host, spec.property, spec.length, |rt, args| {
        let this = rt.current_this();
        let args =
            crate::native_api_manifest_generated::string_slice_generated_validation_args(args);
        crate::generated::string_prototype_slice(rt, this, &args)
    })
}

fn register_generated_string_substring(rt: &mut Runtime, host: ObjectRef) -> ObjectRef {
    let spec = crate::native_api_manifest_generated::string_substring_generated_registration_spec();
    register_intrinsic_method(rt, host, spec.property, spec.length, |rt, args| {
        let this = rt.current_this();
        let args =
            crate::native_api_manifest_generated::string_substring_generated_validation_args(args);
        crate::generated::string_prototype_substring(rt, this, &args)
    })
}

fn register_generated_string_substr(rt: &mut Runtime, host: ObjectRef) -> ObjectRef {
    let spec = crate::native_api_manifest_generated::string_substr_generated_registration_spec();
    register_intrinsic_method(rt, host, spec.property, spec.length, |rt, args| {
        let this = rt.current_this();
        let args =
            crate::native_api_manifest_generated::string_substr_generated_validation_args(args);
        crate::generated::string_prototype_substr(rt, this, &args)
    })
}

fn register_generated_string_code_point_at(rt: &mut Runtime, host: ObjectRef) -> ObjectRef {
    let spec =
        crate::native_api_manifest_generated::string_code_point_at_generated_registration_spec();
    register_intrinsic_method(rt, host, spec.property, spec.length, |rt, args| {
        let this = rt.current_this();
        let args =
            crate::native_api_manifest_generated::string_code_point_at_generated_validation_args(
                args,
            );
        crate::generated::string_prototype_code_point_at(rt, this, &args)
    })
}

fn register_generated_string_includes(rt: &mut Runtime, host: ObjectRef) -> ObjectRef {
    let spec = crate::native_api_manifest_generated::string_includes_generated_registration_spec();
    register_intrinsic_method(rt, host, spec.property, spec.length, |rt, args| {
        let this = rt.current_this();
        let a =
            crate::native_api_manifest_generated::string_includes_generated_validation_args(args)
                [0]
            .clone();
        let b = args.get(1).cloned().unwrap_or(Value::Undefined);
        crate::generated::string_prototype_includes(rt, this, &[a, b])
    })
}

fn register_generated_string_starts_with(rt: &mut Runtime, host: ObjectRef) -> ObjectRef {
    let spec =
        crate::native_api_manifest_generated::string_starts_with_generated_registration_spec();
    register_intrinsic_method(rt, host, spec.property, spec.length, |rt, args| {
        let this = rt.current_this();
        let a = crate::native_api_manifest_generated::string_starts_with_generated_validation_args(
            args,
        )[0]
        .clone();
        let b = args.get(1).cloned().unwrap_or(Value::Undefined);
        crate::generated::string_prototype_starts_with(rt, this, &[a, b])
    })
}

fn register_generated_string_ends_with(rt: &mut Runtime, host: ObjectRef) -> ObjectRef {
    let spec = crate::native_api_manifest_generated::string_ends_with_generated_registration_spec();
    register_intrinsic_method(rt, host, spec.property, spec.length, |rt, args| {
        let this = rt.current_this();
        let a =
            crate::native_api_manifest_generated::string_ends_with_generated_validation_args(args)
                [0]
            .clone();
        let b = args.get(1).cloned().unwrap_or(Value::Undefined);
        crate::generated::string_prototype_ends_with(rt, this, &[a, b])
    })
}
