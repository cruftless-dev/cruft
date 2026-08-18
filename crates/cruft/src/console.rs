
use crate::register::{make_callable, new_object, register_method};
use rusty_js_runtime::{Runtime, Value};

const METHODS: &[&str] = &[
    "dir",
    "dirxml",
    "table",
    "assert",
    "trace",
    "time",
    "timeEnd",
    "timeLog",
    "count",
    "countReset",
    "group",
    "groupCollapsed",
    "groupEnd",
    "clear",
];

fn console_format(rt: &mut Runtime, args: &[Value]) -> String {
    let util = rt.global_get("util");
    if let Value::Object(u) = util {
        let fmt = rt.object_get(u, "format");
        if rt.is_callable(&fmt) {
            if let Ok(Value::String(s)) = rt.call_function(fmt, util, args.to_vec()) {
                return s.as_str().to_string();
            }
        }
    }
    String::new()
}

fn console_assert(
    rt: &mut Runtime,
    args: &[Value],
) -> Result<Value, rusty_js_runtime::RuntimeError> {
    let cond = args.first().cloned().unwrap_or(Value::Undefined);
    if !rusty_js_runtime::abstract_ops::to_boolean(&cond) {
        let rest: &[Value] = if args.len() > 1 { &args[1..] } else { &[] };
        let s = if rest.is_empty() {
            "Assertion failed".to_string()
        } else if let Value::String(first) = &rest[0] {
            let mut merged: Vec<Value> = rest.to_vec();
            merged[0] = Value::String(std::rc::Rc::new(rusty_js_runtime::value::JsString::from(
                format!("Assertion failed: {}", first.as_str()),
            )));
            console_format(rt, &merged)
        } else {
            let mut all: Vec<Value> = Vec::with_capacity(rest.len() + 1);
            all.push(Value::String(std::rc::Rc::new(
                rusty_js_runtime::value::JsString::from("Assertion failed"),
            )));
            all.extend_from_slice(rest);
            console_format(rt, &all)
        };
        console_write(rt, "stderr", &s);
    }
    Ok(Value::Undefined)
}

fn console_write(rt: &mut Runtime, stream: &str, text: &str) {
    let text = rusty_js_runtime::intrinsics::console_apply_group_indent(text);
    let line = Value::String(std::rc::Rc::new(rusty_js_runtime::value::JsString::from(
        format!("{}\n", text.as_str()),
    )));

    let slot = if stream == "stderr" {
        "__console_stderr"
    } else {
        "__console_stdout"
    };
    let target = match rt.current_this() {
        Value::Object(t) => rt.object_get(t, slot),
        _ => Value::Undefined,
    };
    let target = match target {
        s @ Value::Object(_) => s,
        _ => match rt.global_get("process") {
            Value::Object(p) => rt.object_get(p, stream),
            _ => Value::Undefined,
        },
    };
    if let Value::Object(st) = target {
        let w = rt.object_get(st, "write");
        if rt.is_callable(&w) {
            let _ = rt.call_function(w, Value::Object(st), vec![line]);
        }
    }
}

pub fn install(rt: &mut Runtime) {
    let global_console = match rt.global_get("console") {
        Value::Object(c) => c,
        _ => return,
    };
    let module = new_object(rt);

    let mut pairs: Vec<(String, Value)> = Vec::new();
    for m in METHODS {
        let v = rt.object_get(global_console, m);
        if rt.is_callable(&v) {
            rt.object_set(module, (*m).to_string(), v.clone());
            pairs.push(((*m).to_string(), v));
        }
    }

    for m in ["log", "info", "debug"] {
        register_method(rt, module, m, |rt, args| {
            let s = console_format(rt, args);
            console_write(rt, "stdout", &s);
            Ok(Value::Undefined)
        });
    }
    for m in ["warn", "error"] {
        register_method(rt, module, m, |rt, args| {
            let s = console_format(rt, args);
            console_write(rt, "stderr", &s);
            Ok(Value::Undefined)
        });
    }

    for m in ["log", "info", "debug"] {
        register_method(rt, global_console, m, |rt, args| {
            let s = console_format(rt, args);
            console_write(rt, "stdout", &s);
            Ok(Value::Undefined)
        });
    }
    for m in ["warn", "error"] {
        register_method(rt, global_console, m, |rt, args| {
            let s = console_format(rt, args);
            console_write(rt, "stderr", &s);
            Ok(Value::Undefined)
        });
    }

    register_method(rt, global_console, "assert", console_assert);
    register_method(rt, module, "assert", console_assert);

    for m in ["timeStamp", "profile", "profileEnd"] {
        register_method(rt, module, m, |_rt, _a| Ok(Value::Undefined));
    }

    register_method(rt, module, "createTask", |rt, _a| {
        let task = new_object(rt);
        register_method(rt, task, "run", |rt, a| {
            if let Some(f) = a.first() {
                if rt.is_callable(f) {
                    return rt.call_function(f.clone(), Value::Undefined, a[1..].to_vec());
                }
            }
            Ok(Value::Undefined)
        });
        Ok(Value::Object(task))
    });

    register_method(rt, module, "context", move |_rt, _a| {
        Ok(Value::Object(module))
    });

    let ctor_pairs = pairs.clone();
    let console_ctor = make_callable(rt, "Console", move |rt, a| {
        let inst = match rt.current_this() {
            Value::Object(t) => t,
            _ => rt.alloc_object(rusty_js_runtime::value::Object::new_ordinary()),
        };
        for (n, v) in &ctor_pairs {
            rt.object_set(inst, n.clone(), v.clone());
        }

        let (stdout_v, stderr_v) = match a.first() {
            Some(Value::Object(o)) if matches!(rt.object_get(*o, "stdout"), Value::Object(_)) => {
                (rt.object_get(*o, "stdout"), rt.object_get(*o, "stderr"))
            }
            Some(v @ Value::Object(_)) => {
                (v.clone(), a.get(1).cloned().unwrap_or(Value::Undefined))
            }
            _ => (Value::Undefined, Value::Undefined),
        };
        if let Value::Object(_) = stdout_v {
            let stderr_v = match stderr_v {
                s @ Value::Object(_) => s,
                _ => stdout_v.clone(),
            };
            rt.set_engine_sentinel(inst, "__console_stdout", stdout_v);
            rt.set_engine_sentinel(inst, "__console_stderr", stderr_v);
        }

        for m in ["log", "info", "debug"] {
            register_method(rt, inst, m, |rt, args| {
                let s = console_format(rt, args);
                console_write(rt, "stdout", &s);
                Ok(Value::Undefined)
            });
        }
        for m in ["warn", "error"] {
            register_method(rt, inst, m, |rt, args| {
                let s = console_format(rt, args);
                console_write(rt, "stderr", &s);
                Ok(Value::Undefined)
            });
        }

        register_method(rt, inst, "assert", |rt, args| console_assert(rt, args));
        Ok(Value::Object(inst))
    });
    let proto = new_object(rt);
    rt.object_set(proto, "constructor".into(), Value::Object(console_ctor));
    rt.object_set(console_ctor, "prototype".into(), Value::Object(proto));
    rt.object_set(module, "Console".into(), Value::Object(console_ctor));
    rt.object_set(
        global_console,
        "Console".into(),
        Value::Object(console_ctor),
    );

    rt.define_global_property("node_console", Value::Object(module));
}
