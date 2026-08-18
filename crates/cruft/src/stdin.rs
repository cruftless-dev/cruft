
use std::cell::RefCell;
use std::io::Read;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::net::{install_emitter, net_buffer_from_bytes, net_emit};
use crate::register::{make_callable_rooted, new_object, register_method, set_constant};
use rusty_js_runtime::value::{InternalKind, JsString, ObjectRef};
use rusty_js_runtime::{AgentId, Runtime, RuntimeError, Value};

#[derive(Default)]
struct Shared {
    data: Vec<u8>,
    eof: bool,
}

struct StdinState {
    agent_id: AgentId,
    obj: ObjectRef,
    realm: usize,
    shared: Arc<Mutex<Shared>>,

    started: bool,

    paused: bool,

    ended: bool,
}

thread_local! {
    static STDIN: RefCell<Vec<StdinState>> = const { RefCell::new(Vec::new()) };
}

pub fn install(rt: &mut Runtime, process: ObjectRef) {
    let stdin = new_object(rt);

    let stdin_tty = std::io::IsTerminal::is_terminal(&std::io::stdin());
    if stdin_tty {
        rt.object_set(stdin, "isTTY".into(), Value::Boolean(true));
    }
    rt.object_set(stdin, "fd".into(), Value::Number(0.0));

    rt.object_set(stdin, "readable".into(), Value::Boolean(true));
    rt.object_set(stdin, "readableEnded".into(), Value::Boolean(false));
    rt.object_set(stdin, "readableFlowing".into(), Value::Null);
    rt.object_set(stdin, "readableLength".into(), Value::Number(0.0));
    rt.object_set(stdin, "readableObjectMode".into(), Value::Boolean(false));
    rt.object_set(
        stdin,
        "readableHighWaterMark".into(),
        Value::Number(65536.0),
    );
    rt.object_set(stdin, "destroyed".into(), Value::Boolean(false));
    install_emitter(rt, stdin);

    crate::stream::install_async_iterator(rt, stdin);

    register_method(rt, stdin, "resume", |rt, _a| {
        resume(rt);
        Ok(rt.current_this())
    });
    register_method(rt, stdin, "pause", |rt, _a| {
        pause(rt);
        Ok(rt.current_this())
    });
    register_method(rt, stdin, "setEncoding", |rt, args| {
        if let Value::Object(this) = rt.current_this() {
            if let Some(Value::String(e)) = args.first() {
                rt.object_set(this, "__net_encoding".into(), Value::String(e.clone()));
            }
        }
        Ok(rt.current_this())
    });

    register_method(rt, stdin, "read", |_rt, _a| Ok(Value::Null));

    register_method(rt, stdin, "pipe", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(t) => t,
            _ => return Ok(Value::Undefined),
        };
        let dest = match args.first() {
            Some(Value::Object(d)) => *d,
            _ => return Ok(rt.current_this()),
        };
        let sval = |s: &str| {
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                s.to_string(),
            )))
        };
        let on = rt.object_get(this, "on");
        if !rt.is_callable(&on) {
            return Ok(Value::Object(dest));
        }
        let on_data = make_callable_rooted(rt, "stdin.pipe.onData", vec![dest], move |rt, a| {
            let chunk = a.first().cloned().unwrap_or(Value::Undefined);
            let w = rt.object_get(dest, "write");
            if rt.is_callable(&w) {
                let _ = rt.call_function(w, Value::Object(dest), vec![chunk]);
            }
            Ok(Value::Undefined)
        });
        let dv = sval("data");
        let _ = rt.call_function(
            on.clone(),
            Value::Object(this),
            vec![dv, Value::Object(on_data)],
        );
        let on_end = make_callable_rooted(rt, "stdin.pipe.onEnd", vec![dest], move |rt, _a| {
            let e = rt.object_get(dest, "end");
            if rt.is_callable(&e) {
                let _ = rt.call_function(e, Value::Object(dest), Vec::new());
            }
            Ok(Value::Undefined)
        });
        let ev = sval("end");
        let _ = rt.call_function(on, Value::Object(this), vec![ev, Value::Object(on_end)]);

        resume(rt);
        Ok(Value::Object(dest))
    });
    register_method(rt, stdin, "unpipe", |rt, _a| Ok(rt.current_this()));
    register_method(rt, stdin, "destroy", |rt, _a| {
        if let Value::Object(t) = rt.current_this() {
            rt.object_set(t, "destroyed".into(), Value::Boolean(true));
        }
        Ok(rt.current_this())
    });
    register_method(rt, stdin, "isPaused", |_rt, _a| Ok(Value::Boolean(false)));
    register_method(rt, stdin, "unshift", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, stdin, "wrap", |rt, _a| Ok(rt.current_this()));
    register_method(rt, stdin, "push", |_rt, _a| Ok(Value::Boolean(true)));

    for m in ["ref", "unref"] {
        register_method(rt, stdin, m, |rt, _a| Ok(rt.current_this()));
    }

    if stdin_tty {
        register_method(rt, stdin, "setRawMode", |rt, _a| Ok(rt.current_this()));
    }

    let realm = rt.current_realm;
    register(rt, stdin, realm);
    set_constant(rt, process, "stdin", Value::Object(stdin));
}

pub fn register(rt: &Runtime, obj: ObjectRef, realm: usize) {
    let agent_id = rt.agent_id();
    STDIN.with(|c| {
        let mut entries = c.borrow_mut();
        entries.retain(|s| s.agent_id != agent_id);
        entries.push(StdinState {
            agent_id,
            obj,
            realm,
            shared: Arc::new(Mutex::new(Shared::default())),
            started: false,
            paused: false,
            ended: false,
        });
    });
}

pub fn resume(rt: &Runtime) {
    let agent_id = rt.agent_id();
    STDIN.with(|c| {
        let mut entries = c.borrow_mut();
        let Some(st) = entries.iter_mut().find(|s| s.agent_id == agent_id) else {
            return;
        };
        st.paused = false;
        if st.started || st.ended {
            return;
        }
        st.started = true;
        let sh = st.shared.clone();
        std::thread::spawn(move || {
            let mut input = std::io::stdin();
            let mut buf = [0u8; 65536];
            loop {
                match input.read(&mut buf) {
                    Ok(0) => {
                        sh.lock().unwrap().eof = true;
                        break;
                    }
                    Ok(n) => sh.lock().unwrap().data.extend_from_slice(&buf[..n]),
                    Err(_) => {
                        sh.lock().unwrap().eof = true;
                        break;
                    }
                }
            }
        });
    });
}

pub fn pause(rt: &Runtime) {
    let agent_id = rt.agent_id();
    STDIN.with(|c| {
        if let Some(st) = c.borrow_mut().iter_mut().find(|s| s.agent_id == agent_id) {
            st.paused = true;
        }
    });
}

pub fn has_pending(rt: &Runtime) -> bool {
    let agent_id = rt.agent_id();
    STDIN.with(|c| {
        c.borrow()
            .iter()
            .find(|s| s.agent_id == agent_id)
            .map(|s| s.started && !s.paused && !s.ended)
            .unwrap_or(false)
    })
}

fn encoding_of(rt: &Runtime, obj: ObjectRef) -> Option<String> {
    match rt.object_get(obj, "__net_encoding") {
        Value::String(s) => Some(s.as_str().to_string()),
        _ => None,
    }
}

fn has_data_listener(rt: &mut Runtime, obj: ObjectRef) -> bool {
    let reg = match rt.object_get(obj, "__listeners") {
        Value::Object(id) => id,
        _ => return false,
    };
    match rt.object_get(reg, "data") {
        Value::Object(arr) if matches!(rt.obj(arr).internal_kind, InternalKind::Array) => {
            rt.array_length(arr) > 0
        }
        Value::Undefined | Value::Null => false,

        other => rt.is_callable(&other),
    }
}

pub fn poll_io(rt: &mut Runtime) -> Result<bool, RuntimeError> {
    let agent_id = rt.agent_id();
    let Some((obj, realm, started, paused, ended)) = STDIN.with(|c| {
        c.borrow()
            .iter()
            .find(|s| s.agent_id == agent_id)
            .map(|s| (s.obj, s.realm, s.started, s.paused, s.ended))
    }) else {
        return Ok(false);
    };
    if ended {
        return Ok(false);
    }

    if !started {
        if has_data_listener(rt, obj) {
            resume(rt);
            return Ok(true);
        }
        return Ok(false);
    }
    if paused {
        return Ok(false);
    }

    let (data, eof) = STDIN.with(|c| {
        let b = c.borrow();
        let st = b.iter().find(|s| s.agent_id == agent_id).unwrap();
        let mut s = st.shared.lock().unwrap();
        (std::mem::take(&mut s.data), s.eof)
    });

    if !data.is_empty() {
        let prior = rt.enter_realm(realm);
        let chunk = match encoding_of(rt, obj).as_deref() {
            Some("utf8") | Some("utf-8") => Value::String(Rc::new(JsString::from(
                String::from_utf8_lossy(&data).into_owned(),
            ))),
            _ => net_buffer_from_bytes(rt, &data),
        };
        net_emit(rt, obj, "data", vec![chunk]);
        rt.exit_realm(prior);
        return Ok(true);
    }
    if eof {
        STDIN.with(|c| {
            if let Some(st) = c.borrow_mut().iter_mut().find(|s| s.agent_id == agent_id) {
                st.ended = true;
            }
        });
        let prior = rt.enter_realm(realm);
        net_emit(rt, obj, "end", Vec::new());
        rt.exit_realm(prior);
        return Ok(true);
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusty_js_runtime::{AgentId, Runtime};

    #[test]
    fn stdin_registry_is_scoped_by_runtime_agent_id() {
        STDIN.with(|c| c.borrow_mut().clear());
        let mut rt_a = Runtime::new_with_agent_id(AgentId::from_raw(1101));
        let rt_b = Runtime::new_with_agent_id(AgentId::from_raw(1102));
        let obj_a = rt_a.alloc_object(rusty_js_runtime::value::Object::new_ordinary());
        let obj_b = rt_a.alloc_object(rusty_js_runtime::value::Object::new_ordinary());
        register(&rt_a, obj_a, rt_a.current_realm);
        register(&rt_b, obj_b, rt_b.current_realm);
        STDIN.with(|c| {
            if let Some(st) = c
                .borrow_mut()
                .iter_mut()
                .find(|s| s.agent_id == rt_b.agent_id())
            {
                st.started = true;
            }
        });
        assert!(!has_pending(&rt_a));
        assert!(has_pending(&rt_b));
        pause(&rt_a);
        assert!(has_pending(&rt_b));
        STDIN.with(|c| c.borrow_mut().clear());
    }
}
