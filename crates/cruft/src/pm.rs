
use crate::register::{new_object, register_method};
use rusty_js_pm::resolver::{self, Placement, DEFAULT_REGISTRY};
use rusty_js_runtime::value::Object;
use rusty_js_runtime::{Runtime, RuntimeError, Value};
use std::rc::Rc;

fn str_val(s: &str) -> Value {
    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
        s.to_string(),
    )))
}

fn read_roots(rt: &mut Runtime, v: &Value) -> Result<Vec<(String, String)>, RuntimeError> {
    let arr = match v {
        Value::Object(id) => *id,
        _ => {
            return Err(RuntimeError::TypeError(
                "cruft:pm: roots must be an array".into(),
            ))
        }
    };
    let len = match rt.object_get(arr, "length") {
        Value::Number(n) => n as usize,
        _ => 0,
    };
    let mut roots = Vec::with_capacity(len);
    for i in 0..len {
        let el = rt.object_get(arr, &i.to_string());
        let oid = match el {
            Value::Object(id) => id,
            _ => {
                return Err(RuntimeError::TypeError(
                    "cruft:pm: each root must be an object {name, range}".into(),
                ))
            }
        };
        let name = prop_string(rt, oid, "name");
        let range = prop_string(rt, oid, "range");
        roots.push((name, range));
    }
    Ok(roots)
}

fn prop_string(rt: &mut Runtime, oid: rusty_js_runtime::value::ObjectRef, key: &str) -> String {
    match rt.object_get(oid, key) {
        Value::String(s) => s.as_str().to_string(),
        other => rusty_js_runtime::abstract_ops::to_string(&other)
            .as_str()
            .to_string(),
    }
}

fn build_placements(rt: &mut Runtime, placements: &[Placement]) -> Value {
    let arr = rt.alloc_object(Object::new_array());
    for (i, p) in placements.iter().enumerate() {
        let o = rt.alloc_object(Object::new_ordinary());
        rt.object_set(o, "name".into(), str_val(&p.dep.name));
        rt.object_set(o, "version".into(), str_val(&p.dep.version));

        let nest = rt.alloc_object(Object::new_array());
        for (j, ancestor) in p.nest_under.iter().enumerate() {
            rt.object_set(nest, j.to_string().into(), str_val(ancestor));
        }
        rt.object_set(
            nest,
            "length".into(),
            Value::Number(p.nest_under.len() as f64),
        );
        rt.object_set(o, "nest_under".into(), Value::Object(nest));
        rt.object_set(arr, i.to_string().into(), Value::Object(o));
    }
    rt.object_set(arr, "length".into(), Value::Number(placements.len() as f64));
    Value::Object(arr)
}

fn empty_array(rt: &mut Runtime) -> Value {
    let a = rt.alloc_object(Object::new_array());
    rt.object_set(a, "length".into(), Value::Number(0.0));
    Value::Object(a)
}

pub fn install(rt: &mut Runtime) {
    let pm = new_object(rt);

    register_method(rt, pm, "canonicalKey", |rt, args| {
        let roots = read_roots(rt, args.get(0).unwrap_or(&Value::Undefined))?;
        Ok(str_val(&resolver::canonical_resolve_key(&roots)))
    });

    register_method(rt, pm, "resolve", |rt, args| {
        let roots = read_roots(rt, args.get(0).unwrap_or(&Value::Undefined))?;
        let closure_hash = resolver::canonical_resolve_key(&roots);

        let result = rt.alloc_object(Object::new_ordinary());
        rt.object_set(result, "closure_hash".into(), str_val(&closure_hash));

        match resolver::resolve_closure_prefetch(DEFAULT_REGISTRY, &roots, &mut |_| {}) {

            Ok((placements, _peer_demanded)) => {
                let pl = build_placements(rt, &placements);
                rt.object_set(result, "placements".into(), pl);
                rt.object_set(result, "partial".into(), Value::Boolean(false));
            }
            Err(e) => {
                let empty = empty_array(rt);
                rt.object_set(result, "placements".into(), empty);
                rt.object_set(result, "partial".into(), Value::Boolean(true));
                rt.object_set(result, "error".into(), str_val(&format!("{e}")));
            }
        }

        let caps = empty_array(rt);
        rt.object_set(result, "capability_grants".into(), caps);

        Ok(Value::Object(result))
    });

    rt.define_global_property("__cruft_pm", Value::Object(pm));
}
