
use crate::register::{new_object, register_method};
use rusty_js_runtime::{Runtime, Value};

fn http_agent_prototype(rt: &Runtime) -> Option<rusty_js_runtime::ObjectRef> {
    let http = match rt.global_get("http") {
        Value::Object(h) => h,
        _ => return None,
    };
    let agent = match rt.object_get(http, "Agent") {
        Value::Object(a) => a,
        _ => return None,
    };
    match rt.object_get(agent, "prototype") {
        Value::Object(p) => Some(p),
        _ => None,
    }
}

fn https_agent_get_name(rt: &Runtime, opts: Option<&Value>) -> String {
    let opts = match opts {
        Some(Value::Object(o)) => *o,
        _ => {

            return format!("localhost::{}", ":".repeat(20));
        }
    };
    let host = match rt.object_get(opts, "host") {
        Value::String(s) if !s.as_str().is_empty() => s.as_str().to_string(),
        _ => "localhost".to_string(),
    };
    let port = match rt.object_get(opts, "port") {
        Value::Number(n) => format!("{}", n as u64),
        Value::String(s) => s.as_str().to_string(),
        _ => String::new(),
    };
    let local = match rt.object_get(opts, "localAddress") {
        Value::String(s) => s.as_str().to_string(),
        _ => String::new(),
    };
    let mut name = format!("{host}:{port}:{local}");
    if let Value::Number(f) = rt.object_get(opts, "family") {
        let fam = f as i64;
        if fam == 4 || fam == 6 {
            name.push_str(&format!(":{fam}"));
        }
    }

    let str_field = |rt: &Runtime, key: &str| -> String {
        match rt.object_get(opts, key) {
            Value::String(s) if !s.as_str().is_empty() => s.as_str().to_string(),
            _ => String::new(),
        }
    };
    let mut push = |v: String| {
        name.push(':');
        name.push_str(&v);
    };
    push(str_field(rt, "ca"));
    push(str_field(rt, "cert"));
    push(str_field(rt, "clientCertEngine"));
    push(str_field(rt, "ciphers"));
    push(str_field(rt, "key"));
    push(str_field(rt, "pfx"));

    push(match rt.object_get(opts, "rejectUnauthorized") {
        Value::Boolean(b) => b.to_string(),
        Value::Number(n) => format!("{n}"),
        _ => String::new(),
    });

    push(match rt.object_get(opts, "servername") {
        Value::String(s) if !s.as_str().is_empty() && s.as_str() != host => s.as_str().to_string(),
        _ => String::new(),
    });
    push(str_field(rt, "minVersion"));
    push(str_field(rt, "maxVersion"));
    push(str_field(rt, "secureProtocol"));
    push(str_field(rt, "crl"));
    push(match rt.object_get(opts, "honorCipherOrder") {
        Value::Boolean(b) => b.to_string(),
        _ => String::new(),
    });
    push(str_field(rt, "ecdhCurve"));
    push(str_field(rt, "dhparam"));
    push(match rt.object_get(opts, "secureOptions") {
        Value::Number(n) => format!("{}", n as i64),
        _ => String::new(),
    });
    push(str_field(rt, "sessionIdContext"));
    push(str_field(rt, "sigalgs"));
    push(str_field(rt, "privateKeyIdentifier"));
    push(str_field(rt, "privateKeyEngine"));
    name
}

pub fn install(rt: &mut Runtime) {
    let tls_server_ready = match rt.global_get("tls") {
        Value::Object(tls) => matches!(rt.object_get(tls, "Server"), Value::Object(_)),
        _ => false,
    };
    if !tls_server_ready {
        crate::tls::install(rt);
    }
    let https = new_object(rt);
    let https_server_proto = {
        let c = crate::register::make_callable(rt, "Server", |rt, _a| Ok(rt.current_this()));
        let pr = crate::register::new_object(rt);
        rt.object_set(pr, "constructor".into(), Value::Object(c));
        rt.object_set(c, "prototype".into(), Value::Object(pr));
        rt.object_set(https, "Server".into(), Value::Object(c));
        pr
    };

    register_method(rt, https, "request", |rt, args| {
        crate::http::client_request(rt, true, args)
    });
    register_method(rt, https, "get", |rt, args| {
        let req = crate::http::client_request(rt, true, args)?;
        if let Value::Object(id) = &req {

            let id = *id;
            rt.enqueue_microtask_rooted("https.get.end", vec![id], move |rt| {
                let end = rt.object_get(id, "end");
                if rt.is_callable(&end) {
                    rt.call_function(end, Value::Object(id), Vec::new())?;
                }
                Ok(())
            });
        }
        Ok(req)
    });
    register_method(rt, https, "createServer", move |rt, args| {
        crate::tls::do_create_https_server_with_proto(rt, args, Some(https_server_proto))
    });

    register_method(rt, https, "Agent", |rt, args| {

        let id = match rt.current_this() {
            Value::Object(this) => this,
            _ => rt.alloc_object(rusty_js_runtime::Object::new_ordinary()),
        };
        crate::http::http_agent_reflect_options(rt, id, args.first());

        register_method(rt, id, "getName", |rt, args| {
            Ok(Value::String(std::rc::Rc::new(
                rusty_js_runtime::value::JsString::from(https_agent_get_name(rt, args.first())),
            )))
        });
        Ok(Value::Object(id))
    });

    if let Value::Object(agent) = rt.object_get(https, "Agent") {
        let parent = http_agent_prototype(rt)
            .or_else(|| crate::register::proto_of_global_ctor(rt, "events"));
        crate::register::make_subclassable(rt, agent, parent);
    }

    {

        let o = crate::register::new_object(rt);
        if let Value::Object(agent_ctor) = rt.object_get(https, "Agent") {
            if let Value::Object(agent_proto) = rt.object_get(agent_ctor, "prototype") {
                rt.set_object_prototype_internal(o, Some(agent_proto));
            }
        }

        crate::http::http_agent_reflect_options(rt, o, None);
        rt.object_set(
            o,
            "protocol".into(),
            Value::String(std::rc::Rc::new(rusty_js_runtime::value::JsString::from(
                "https:",
            ))),
        );
        rt.object_set(https, "globalAgent".into(), Value::Object(o));
    }
    rt.define_global_property("https", Value::Object(https));
}
