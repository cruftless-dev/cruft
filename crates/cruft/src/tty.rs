
use crate::register::{make_callable, make_subclassable, new_object, register_method};
use rusty_js_runtime::{Runtime, Value};

pub fn install(rt: &mut Runtime) {
    let t = new_object(rt);
    register_method(rt, t, "isatty", |_rt, _args| Ok(Value::Boolean(false)));

    let read_ctor = make_callable(rt, "ReadStream", |rt, _args| {
        let o = new_object(rt);
        rt.object_set(o, "isRaw".into(), Value::Boolean(false));
        rt.object_set(o, "isTTY".into(), Value::Boolean(false));
        register_method(rt, o, "setRawMode", |rt, _a| Ok(rt.current_this()));
        Ok(Value::Object(o))
    });
    make_subclassable(rt, read_ctor, None);
    if let Value::Object(proto) = rt.object_get(read_ctor, "prototype") {
        register_method(rt, proto, "setRawMode", |rt, _a| Ok(rt.current_this()));
    }
    rt.object_set(t, "ReadStream".into(), Value::Object(read_ctor));

    let write_ctor = make_callable(rt, "WriteStream", |rt, _args| {
        let o = new_object(rt);
        rt.object_set(o, "isTTY".into(), Value::Boolean(false));
        rt.object_set(o, "columns".into(), Value::Number(80.0));
        rt.object_set(o, "rows".into(), Value::Number(24.0));
        Ok(Value::Object(o))
    });
    make_subclassable(rt, write_ctor, None);
    if let Value::Object(proto) = rt.object_get(write_ctor, "prototype") {
        register_method(rt, proto, "getColorDepth", |_rt, _a| Ok(Value::Number(1.0)));
        register_method(rt, proto, "hasColors", |_rt, _a| Ok(Value::Boolean(false)));
        register_method(rt, proto, "getWindowSize", |rt, _a| {
            let arr = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
            rt.object_set(arr, "0".into(), Value::Number(80.0));
            rt.object_set(arr, "1".into(), Value::Number(24.0));
            rt.object_set(arr, "length".into(), Value::Number(2.0));
            Ok(Value::Object(arr))
        });
        register_method(rt, proto, "clearLine", |_rt, _a| Ok(Value::Boolean(true)));
        register_method(rt, proto, "clearScreenDown", |_rt, _a| {
            Ok(Value::Boolean(true))
        });
        register_method(rt, proto, "cursorTo", |_rt, _a| Ok(Value::Boolean(true)));
        register_method(rt, proto, "moveCursor", |_rt, _a| Ok(Value::Boolean(true)));
    }
    rt.object_set(t, "WriteStream".into(), Value::Object(write_ctor));

    rt.define_global_property("tty", Value::Object(t));
}
