
use crate::register::{new_object, register_method, set_constant};
use rusty_js_runtime::realm_adapter::{
    boundary_filter, realm_alloc, realm_evaluate, BoundaryPolicy, Endowments,
};
use rusty_js_runtime::value::Object as RtObject;
use rusty_js_runtime::value::ObjectRef;
use rusty_js_runtime::{Runtime, RuntimeError, Value};

pub fn contextify_in_place(rt: &mut Runtime, obj: ObjectRef) -> usize {

    let realm_idx = realm_alloc(
        rt,
        true,
        Endowments::new(),
        rusty_js_runtime::caps::CapMode::Compat,
    );
    for key in rt.ordinary_own_enumerable_string_keys(obj) {
        let value = rt.object_get(obj, &key);
        rt.realms[realm_idx].globals_overrides.insert(key, value);
    }
    let primordial_gt = rt.global_object;
    let mut to_install: Vec<(String, Value)> = Vec::new();
    for name in Runtime::intrinsic_name_allowlist() {

        if !matches!(rt.object_get(obj, name), Value::Undefined) {
            continue;
        }

        let cloned = rt.realms[realm_idx].globals_overrides.get(*name).cloned();
        let v = match cloned {
            Some(v) => v,
            None => match primordial_gt {
                Some(gt) => rt.object_get(gt, name),
                None => Value::Undefined,
            },
        };
        if !matches!(v, Value::Undefined) {
            to_install.push(((*name).to_string(), v));
        }
    }
    for (name, v) in to_install {
        rt.set_engine_sentinel(obj, &name, v);
    }

    rt.set_engine_sentinel(obj, "undefined", Value::Undefined);
    rt.realms[realm_idx]
        .globals_overrides
        .insert("undefined".to_string(), Value::Undefined);
    rt.object_set(obj, "__vm_realm".into(), Value::Number(realm_idx as f64));
    rt.object_set(obj, "__vm_context".into(), Value::Boolean(true));

    rt.object_set(obj, "globalThis".into(), Value::Object(obj));
    rt.realms[realm_idx].global = Some(obj);
    rt.realms[realm_idx]
        .globals_overrides
        .insert("globalThis".to_string(), Value::Object(obj));
    realm_idx
}

pub fn sync_context_global_after_eval(rt: &mut Runtime, ctx: ObjectRef, realm_idx: usize) {
    if let Value::Object(global) = rt.object_get(ctx, "globalThis") {
        if global != ctx {
            for key in rt.ordinary_own_enumerable_string_keys(global) {
                let value = rt.object_get(global, &key);
                rt.object_set(ctx, key, value);
            }
        }
    }
    rt.object_set(ctx, "globalThis".into(), Value::Object(ctx));
    rt.realms[realm_idx]
        .globals_overrides
        .insert("globalThis".to_string(), Value::Object(ctx));
}

fn copy_own_enumerable(rt: &mut Runtime, src: ObjectRef, dst: ObjectRef) {
    let keys = match rt.own_enumerable_string_keys_via(&Value::Object(src)) {
        Ok(Value::Object(arr)) => arr,
        _ => return,
    };
    let len = match rt.object_get(keys, "length") {
        Value::Number(n) => n as usize,
        _ => 0,
    };
    for i in 0..len {
        if let Value::String(k) = rt.object_get(keys, &i.to_string()) {
            let v = rt.object_get(src, k.as_str());
            rt.object_set(dst, k.to_string(), v);
        }
    }
}

fn ctx_realm(rt: &Runtime, ctx: ObjectRef) -> Result<usize, RuntimeError> {
    match rt.object_get(ctx, "__vm_realm") {
        Value::Number(n) => Ok(n as usize),
        _ => Err(RuntimeError::TypeError(
            "cruft:vm: argument is not a context (use createContext)".into(),
        )),
    }
}

fn source_arg(args: &[Value], who: &str) -> Result<String, RuntimeError> {
    match args.first() {
        Some(Value::String(s)) => Ok(s.as_str().to_string()),
        _ => Err(RuntimeError::TypeError(format!(
            "cruft:vm.{who}: source must be a string"
        ))),
    }
}

pub fn install_canonical(rt: &mut Runtime) {
    let ns = new_object(rt);

    register_method(rt, ns, "createContext", |rt, args| {
        let ctx = rt.alloc_object(RtObject::new_ordinary());
        if let Some(Value::Object(endow)) = args.first() {
            copy_own_enumerable(rt, *endow, ctx);
        }
        contextify_in_place(rt, ctx);
        Ok(Value::Object(ctx))
    });

    register_method(rt, ns, "run", |rt, args| {
        let source = source_arg(args, "run")?;
        let (realm_idx, global) = match args.get(1) {
            Some(Value::Object(ctx)) => (ctx_realm(rt, *ctx)?, *ctx),
            _ => {
                let ctx = rt.alloc_object(RtObject::new_ordinary());
                (contextify_in_place(rt, ctx), ctx)
            }
        };
        let url = format!("file://<cruft:vm:{}:run>", realm_idx);
        let value = realm_evaluate(rt, realm_idx, Some(global), &source, &url)?;
        boundary_filter(rt, value, BoundaryPolicy::NodeCompat)
    });

    register_method(rt, ns, "compile", |rt, args| {
        let source = source_arg(args, "compile")?;
        let handle = new_object(rt);
        set_constant(
            rt,
            handle,
            "source",
            Value::String(std::rc::Rc::new(rusty_js_runtime::value::JsString::from(
                source,
            ))),
        );
        register_method(rt, handle, "run", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(id) => id,
                _ => return Err(RuntimeError::TypeError("cruft:vm: detached handle".into())),
            };
            let source = match rt.object_get(this, "source") {
                Value::String(s) => s.as_str().to_string(),
                _ => {
                    return Err(RuntimeError::TypeError(
                        "cruft:vm: handle has no source".into(),
                    ))
                }
            };
            let (realm_idx, global) = match args.first() {
                Some(Value::Object(ctx)) => (ctx_realm(rt, *ctx)?, *ctx),
                _ => {
                    let ctx = rt.alloc_object(RtObject::new_ordinary());
                    (contextify_in_place(rt, ctx), ctx)
                }
            };
            let url = format!("file://<cruft:vm:{}:compile.run>", realm_idx);
            let value = realm_evaluate(rt, realm_idx, Some(global), &source, &url)?;
            boundary_filter(rt, value, BoundaryPolicy::NodeCompat)
        });
        Ok(Value::Object(handle))
    });

    rt.define_global_property("__cruft_vm", Value::Object(ns));
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusty_js_runtime::RealmCollectionError;

    #[test]
    fn contextify_records_foreign_global_for_gc_fail_closed_guard() {
        let mut rt = Runtime::new();
        let ctx = rt.alloc_object(RtObject::new_ordinary());
        let realm_idx = contextify_in_place(&mut rt, ctx);

        assert_eq!(rt.realms[realm_idx].global, Some(ctx));
        assert_eq!(rt.heap.owner(ctx), Some(0));
        let err = rt
            .collect_realm(realm_idx, std::iter::empty())
            .expect_err("node:vm foreign context global must keep per-realm GC fail-closed");
        let RealmCollectionError::CrossOwnerEdge(edge) = err else {
            panic!("expected cross-owner edge error, got {err:?}");
        };
        assert_eq!(edge.from, ctx);
        assert_eq!(edge.from_owner, realm_idx);
        assert_eq!(edge.to, ctx);
        assert_eq!(edge.to_owner, 0);
    }
}
