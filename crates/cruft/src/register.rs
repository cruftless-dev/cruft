
use rusty_js_runtime::value::{FunctionInternals, InternalKind, NativeFn, Object, ObjectRef};
use rusty_js_runtime::{Runtime, RuntimeError, Value};
use std::rc::Rc;

pub fn new_object(rt: &mut Runtime) -> ObjectRef {
    rt.alloc_object(Object::new_ordinary())
}

pub fn register_method<F>(rt: &mut Runtime, host: ObjectRef, name: &str, f: F)
where
    F: Fn(&mut Runtime, &[Value]) -> Result<Value, RuntimeError> + 'static,
{
    let _host_root = rt.push_temporary_value_roots(&[Value::Object(host)]);
    let native: NativeFn = Rc::new(f);
    let mut properties = indexmap::IndexMap::new();
    rusty_js_runtime::value::install_function_meta_props(&mut properties, name, 0.0);
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
    let _fn_root = rt.push_temporary_value_roots(&[Value::Object(fn_id)]);
    rt.object_set(host, name.into(), Value::Object(fn_id));
}

pub fn register_method_internal<F>(rt: &mut Runtime, host: ObjectRef, name: &str, f: F)
where
    F: Fn(&mut Runtime, &[Value]) -> Result<Value, RuntimeError> + 'static,
{
    let _host_root = rt.push_temporary_value_roots(&[Value::Object(host)]);
    let native: NativeFn = Rc::new(f);
    let mut properties = indexmap::IndexMap::new();
    rusty_js_runtime::value::install_function_meta_props(&mut properties, name, 0.0);
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
    let _fn_root = rt.push_temporary_value_roots(&[Value::Object(fn_id)]);
    rt.obj_mut(host)
        .set_own_internal(name.into(), Value::Object(fn_id));
}

pub fn set_constant(rt: &mut Runtime, host: ObjectRef, name: &str, value: Value) {
    let roots = match &value {
        Value::Object(id) => vec![Value::Object(host), Value::Object(*id)],
        _ => vec![Value::Object(host)],
    };
    let _roots = rt.push_temporary_value_roots(&roots);
    rt.object_set(host, name.into(), value);
}

pub fn native_function<F>(rt: &mut Runtime, name: &str, f: F) -> Value
where
    F: Fn(&mut Runtime, &[Value]) -> Result<Value, RuntimeError> + 'static,
{
    let native: NativeFn = Rc::new(f);
    let mut properties = indexmap::IndexMap::new();
    rusty_js_runtime::value::install_function_meta_props(&mut properties, name, 0.0);
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
    Value::Object(rt.alloc_object(fn_obj))
}

pub fn proto_of_global_ctor(rt: &Runtime, name: &str) -> Option<ObjectRef> {
    match rt.global_get(name) {
        Value::Object(c) => match rt.object_get(c, "prototype") {
            Value::Object(p) => Some(p),
            _ => None,
        },
        _ => None,
    }
}

pub fn make_subclassable(rt: &mut Runtime, ctor: ObjectRef, parent_proto: Option<ObjectRef>) {
    if !matches!(rt.object_get(ctor, "prototype"), Value::Undefined) {
        return;
    }
    let proto = new_object(rt);
    if let Some(p) = parent_proto {
        rt.set_object_prototype_internal(proto, Some(p));
    }
    rt.object_set(proto, "constructor".into(), Value::Object(ctor));
    rt.object_set(ctor, "prototype".into(), Value::Object(proto));
}

pub fn make_callable<F>(rt: &mut Runtime, name: &str, f: F) -> ObjectRef
where
    F: Fn(&mut Runtime, &[Value]) -> Result<Value, RuntimeError> + 'static,
{
    make_callable_with_length_rooted(rt, name, 0, Vec::new(), f)
}

pub fn make_callable_with_length<F>(rt: &mut Runtime, name: &str, length: u32, f: F) -> ObjectRef
where
    F: Fn(&mut Runtime, &[Value]) -> Result<Value, RuntimeError> + 'static,
{
    make_callable_with_length_rooted(rt, name, length, Vec::new(), f)
}

pub fn make_callable_rooted<F>(
    rt: &mut Runtime,
    name: &str,
    roots: Vec<ObjectRef>,
    f: F,
) -> ObjectRef
where
    F: Fn(&mut Runtime, &[Value]) -> Result<Value, RuntimeError> + 'static,
{
    make_callable_with_length_rooted(rt, name, 0, roots, f)
}

pub fn make_callable_with_length_rooted<F>(
    rt: &mut Runtime,
    name: &str,
    length: u32,
    roots: Vec<ObjectRef>,
    f: F,
) -> ObjectRef
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
            length,
            native,
            is_constructor: true,
            creation_realm: 0,
            roots,
        })),

        ..Default::default()
    };
    rt.alloc_object(fn_obj)
}

pub fn arg_string(args: &[Value], i: usize) -> String {
    use rusty_js_runtime::abstract_ops;
    args.get(i)
        .map(|v| abstract_ops::to_string(v).as_str().to_string())
        .unwrap_or_default()
}

pub fn arg_number(args: &[Value], i: usize) -> f64 {
    use rusty_js_runtime::abstract_ops;
    args.get(i).map(abstract_ops::to_number).unwrap_or(f64::NAN)
}
