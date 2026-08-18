use crate::register::{make_callable, new_object, register_method};
use rusty_js_runtime::{Runtime, RuntimeError, Value};

fn detach_array_buffer(rt: &mut Runtime, args: &[Value]) -> Result<Value, RuntimeError> {
    let id = match args.first() {
        Some(Value::Object(id)) if rt.array_buffers.contains_key(id) => *id,
        Some(Value::Object(id)) => match rt.typed_array_views.get(id) {
            Some(view) => view.buffer,
            None => {
                return Err(RuntimeError::TypeError(
                    "$262.detachArrayBuffer: argument must be an ArrayBuffer".into(),
                ))
            }
        },
        _ => {
            return Err(RuntimeError::TypeError(
                "$262.detachArrayBuffer: argument must be an ArrayBuffer".into(),
            ))
        }
    };
    rt.detach_array_buffer(id)?;
    Ok(Value::Undefined)
}

fn gc(rt: &mut Runtime, args: &[Value]) -> Result<Value, RuntimeError> {
    rt.request_collect();
    if let Some(Value::Object(opts)) = args.first() {
        if let Value::String(s) = rt.object_get(*opts, "execution") {
            if s.as_str() == "async" {
                let p = rusty_js_runtime::promise::new_promise(rt);
                rusty_js_runtime::promise::resolve_promise(rt, p, Value::Undefined);
                return Ok(Value::Object(p));
            }
        }
    }
    Ok(Value::Undefined)
}

pub fn install(rt: &mut Runtime) {
    let detach_helper = make_callable(rt, "__cruft_detach_array_buffer", detach_array_buffer);
    rt.set_engine_helper_with_satb(
        "__cruft_detach_array_buffer".into(),
        Value::Object(detach_helper),
    );
    let gc_helper = make_callable(rt, "__cruft_gc", gc);
    rt.engine_helpers
        .insert("__cruft_gc".into(), Value::Object(gc_helper));

    if std::env::var_os("CRUFT_NODE_CORE_TEST").is_some() {
        let gc_global = make_callable(rt, "gc", gc);
        rt.define_global_property("gc", Value::Object(gc_global));
    }

    if std::env::var_os("T262_TEST_PATH").is_none() {
        return;
    }

    let host = new_object(rt);
    register_method(rt, host, "detachArrayBuffer", detach_array_buffer);
    register_method(rt, host, "gc", gc);
    rt.define_global_property("$262", Value::Object(host));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test262_gc_requests_safe_point_collection() {
        let mut rt = Runtime::new();

        assert!(!rt.collect_requested);
        assert!(matches!(gc(&mut rt, &[]), Ok(Value::Undefined)));
        assert!(
            rt.collect_requested,
            "$262.gc must request the runtime safe-point collector"
        );
    }
}
