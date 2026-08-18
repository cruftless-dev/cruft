
use crate::register::{
    make_callable, make_callable_rooted, make_callable_with_length_rooted, new_object,
    register_method,
};
use rusty_js_runtime::abstract_ops;
use rusty_js_runtime::caps;
use rusty_js_runtime::caps::{ModuleId, ModuleProvenance};
use rusty_js_runtime::interp::{ArrayBufferRecord, TypedArrayViewRecord};
use rusty_js_runtime::value::{Object as RtObject, ObjectRef, PropertyDescriptor, PropertyKey};
use rusty_js_runtime::{Runtime, RuntimeError, Value};
use std::rc::Rc;

const NODE_DEFAULT_CIPHER_LIST: &str = r#"TLS_AES_256_GCM_SHA384:TLS_CHACHA20_POLY1305_SHA256:TLS_AES_128_GCM_SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES256-GCM-SHA384:ECDHE-ECDSA-AES256-GCM-SHA384:DHE-RSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-SHA256:DHE-RSA-AES128-SHA256:ECDHE-RSA-AES256-SHA384:DHE-RSA-AES256-SHA384:ECDHE-RSA-AES256-SHA256:DHE-RSA-AES256-SHA256:HIGH:!aNULL:!eNULL:!EXPORT:!DES:!RC4:!MD5:!PSK:!SRP:!CAMELLIA"#;

const NODE_BUILTIN_MODULES: &[&str] = &[
    "assert",
    "buffer",
    "child_process",
    "constants",
    "crypto",
    "dns",
    "events",
    "fs",
    "http",
    "https",
    "module",
    "net",
    "os",
    "path",
    "process",
    "punycode",
    "querystring",
    "readline",
    "stream",
    "string_decoder",
    "sys",
    "timers",
    "tls",
    "tty",
    "url",
    "util",
    "v8",
    "vm",
    "worker_threads",
    "zlib",

    "assert/strict",
    "async_hooks",
    "cluster",
    "console",
    "dgram",
    "diagnostics_channel",
    "dns/promises",
    "domain",
    "fs/promises",
    "http2",
    "inspector",
    "inspector/promises",
    "path/posix",
    "path/win32",
    "perf_hooks",
    "readline/promises",
    "repl",
    "stream/consumers",
    "stream/promises",
    "stream/web",
    "timers/promises",
    "trace_events",
    "util/types",
    "wasi",
];

const NODE_PREFIX_ONLY_BUILTINS: &[&str] = &["sqlite", "test", "test/reporters"];

fn base64_decode(s: &str) -> Vec<u8> {
    let mut lut = [255u8; 128];
    for (i, c) in b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
        .iter()
        .enumerate()
    {
        lut[*c as usize] = i as u8;
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits = 0;
    for c in s.bytes() {
        if c == b'=' {
            break;
        }
        if c >= 128 {
            continue;
        }
        let v = lut[c as usize];
        if v == 255 {
            continue;
        }
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xff) as u8);
        }
    }
    out
}

fn base64_encode(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i + 2 < bytes.len() {
        let b = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8) | (bytes[i + 2] as u32);
        out.push(T[((b >> 18) & 0x3f) as usize] as char);
        out.push(T[((b >> 12) & 0x3f) as usize] as char);
        out.push(T[((b >> 6) & 0x3f) as usize] as char);
        out.push(T[(b & 0x3f) as usize] as char);
        i += 3;
    }
    let rem = bytes.len() - i;
    if rem == 1 {
        let b = (bytes[i] as u32) << 16;
        out.push(T[((b >> 18) & 0x3f) as usize] as char);
        out.push(T[((b >> 12) & 0x3f) as usize] as char);
        out.push_str("==");
    } else if rem == 2 {
        let b = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8);
        out.push(T[((b >> 18) & 0x3f) as usize] as char);
        out.push(T[((b >> 12) & 0x3f) as usize] as char);
        out.push(T[((b >> 6) & 0x3f) as usize] as char);
        out.push('=');
    }
    out
}

fn encode_buffer_write_value(s: &str, encoding: &str) -> Vec<u8> {
    match encoding {
        "hex" => {
            let mut v = Vec::with_capacity(s.len() / 2);
            let chars: Vec<char> = s.chars().collect();
            let mut i = 0;
            while i + 1 < chars.len() {
                let hi = chars[i].to_digit(16);
                let lo = chars[i + 1].to_digit(16);
                match (hi, lo) {
                    (Some(h), Some(l)) => v.push(((h << 4) | l) as u8),
                    _ => break,
                }
                i += 2;
            }
            v
        }
        "base64" | "base64url" => {
            let mut normalized = s.replace('-', "+").replace('_', "/");
            while normalized.len() % 4 != 0 {
                normalized.push('=');
            }
            base64_decode(&normalized)
        }
        "latin1" | "binary" => s.chars().map(|c| c as u8).collect(),
        "ascii" => s.chars().map(|c| (c as u32 & 0x7f) as u8).collect(),
        _ => s.as_bytes().to_vec(),
    }
}

fn normalize_buffer_encoding(v: Option<&Value>) -> String {
    let raw = v
        .and_then(|v| match v {
            Value::String(s) => Some(s.as_str().to_ascii_lowercase()),
            _ => None,
        })
        .unwrap_or_else(|| "utf8".to_string());
    raw.replace('-', "")
}

fn buffer_like_bytes(rt: &mut Runtime, value: &Value) -> Vec<u8> {
    match value {
        Value::Object(id) => {
            if let Some(bytes) = rt.typed_array_view_bytes(*id) {
                return bytes;
            }
            let len = match rt.object_get(*id, "length") {
                Value::Number(n) if n.is_finite() && n > 0.0 => n as usize,
                _ => 0,
            };
            let mut bytes = Vec::with_capacity(len);
            for i in 0..len {
                match rt.object_get(*id, &i.to_string()) {
                    Value::Number(n) => bytes.push(n as u8),
                    _ => bytes.push(0),
                }
            }
            bytes
        }
        Value::String(s) => s.as_bytes().to_vec(),
        _ => Vec::new(),
    }
}

fn decode_transcode_input(bytes: &[u8], encoding: &str) -> Result<String, RuntimeError> {
    match encoding {
        "utf8" => Ok(String::from_utf8_lossy(bytes).to_string()),
        "latin1" | "binary" => Ok(bytes.iter().map(|b| *b as char).collect()),
        "ascii" => Ok(bytes.iter().map(|b| (*b & 0x7f) as char).collect()),
        "utf16le" | "ucs2" => {
            let mut units = Vec::with_capacity(bytes.len() / 2);
            let mut i = 0;
            while i + 1 < bytes.len() {
                units.push(u16::from_le_bytes([bytes[i], bytes[i + 1]]));
                i += 2;
            }
            Ok(char::decode_utf16(units)
                .map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER))
                .collect())
        }
        _ => Err(RuntimeError::TypeError(format!(
            "Unable to transcode Buffer from {encoding}"
        ))),
    }
}

fn encode_transcode_output(s: &str, encoding: &str) -> Result<Vec<u8>, RuntimeError> {
    match encoding {
        "utf8" => Ok(s.as_bytes().to_vec()),
        "latin1" | "binary" => Ok(s.chars().map(|c| c as u8).collect()),
        "ascii" => Ok(s.chars().map(|c| (c as u32 & 0x7f) as u8).collect()),
        "utf16le" | "ucs2" => {
            let mut bytes = Vec::with_capacity(s.len() * 2);
            for unit in s.encode_utf16() {
                bytes.extend_from_slice(&unit.to_le_bytes());
            }
            Ok(bytes)
        }
        _ => Err(RuntimeError::TypeError(format!(
            "Unable to transcode Buffer to {encoding}"
        ))),
    }
}

fn check_clock_ns(rt: &Runtime, op: caps::ClockOp) -> Result<(), RuntimeError> {
    let url = rt.current_module_url.last().cloned().unwrap_or_default();
    let provenance = if url.contains("/node_modules/") {
        ModuleProvenance::Dependency
    } else if url.starts_with("node:") {
        ModuleProvenance::Builtin
    } else {
        ModuleProvenance::Application
    };
    let caller = ModuleId { url, provenance };
    rt.caps
        .require_clock(&caps::Clock::disabled(), op, &caller)
        .map_err(|e| RuntimeError::TypeError(e.to_string()))
}

fn stub(
    module: &'static str,
    method: &'static str,
) -> impl Fn(&mut Runtime, &[Value]) -> Result<Value, RuntimeError> {
    move |_rt, _args| {
        Err(RuntimeError::Thrown(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(format!(
                "TypeError: node:{module}.{method} not yet implemented (Tier-Ω.5.bb stub)"
            )),
        ))))
    }
}

pub fn install_child_process(rt: &mut Runtime) {

    crate::child_process::install(rt);
}

pub fn install_tls(rt: &mut Runtime) {

    crate::tls::install_canonical(rt);
    crate::tls::install(rt);
}

fn rl_write(rt: &mut Runtime, stream: &Value, text: &str) {
    if let Value::Object(st) = stream {
        let w = rt.object_get(*st, "write");
        if rt.is_callable(&w) {
            let s = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                text.to_string(),
            )));
            let _ = rt.call_function(w, stream.clone(), vec![s]);
        }
    }
}
fn rl_num(args: &[Value], i: usize) -> i64 {
    match args.get(i) {
        Some(Value::Number(n)) => *n as i64,
        _ => 0,
    }
}

fn rl_make_interface(
    rt: &mut Runtime,
    opts: Option<rusty_js_runtime::ObjectRef>,
    promise_q: bool,
) -> rusty_js_runtime::ObjectRef {
    let iface = new_object(rt);
    rt.obj_mut(iface)
        .set_own_internal("__readline_interface__".into(), Value::Boolean(true));
    crate::net::install_emitter(rt, iface);

    let input = opts
        .map(|o| rt.object_get(o, "input"))
        .unwrap_or(Value::Undefined);
    if let Value::Object(input_obj) = input {
        rt.object_set(
            iface,
            "__rl_buf".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                String::new(),
            ))),
        );

        rt.object_set(
            input_obj,
            "__net_encoding".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                "utf8".to_string(),
            ))),
        );
        let rl_str = |rt: &mut Runtime, obj: rusty_js_runtime::ObjectRef, key: &str| -> String {
            match rt.object_get(obj, key) {
                Value::String(s) => s.as_str().to_string(),
                _ => String::new(),
            }
        };
        let data_cb = crate::register::native_function(rt, "onLineData", move |rt, args| {
            let chunk = match args.first() {
                Some(Value::String(s)) => s.as_str().to_string(),
                Some(v @ Value::Object(_)) => {

                    let bytes = buffer_like_bytes(rt, v);
                    if bytes.is_empty() {
                        rusty_js_runtime::abstract_ops::to_string(v)
                            .as_str()
                            .to_string()
                    } else {
                        String::from_utf8_lossy(&bytes).into_owned()
                    }
                }
                Some(v) => rusty_js_runtime::abstract_ops::to_string(v)
                    .as_str()
                    .to_string(),
                None => String::new(),
            };
            let mut buf = rl_str(rt, iface, "__rl_buf");
            buf.push_str(&chunk);
            while let Some(i) = buf.find('\n') {
                let line = buf[..i].trim_end_matches('\r').to_string();
                buf.replace_range(..=i, "");
                crate::net::net_emit(
                    rt,
                    iface,
                    "line",
                    vec![Value::String(Rc::new(
                        rusty_js_runtime::value::JsString::from(line),
                    ))],
                );
            }
            rt.object_set(
                iface,
                "__rl_buf".into(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(buf))),
            );
            Ok(Value::Undefined)
        });
        let end_cb = crate::register::native_function(rt, "onLineEnd", move |rt, _a| {
            let rest = rl_str(rt, iface, "__rl_buf");
            if !rest.is_empty() {
                crate::net::net_emit(
                    rt,
                    iface,
                    "line",
                    vec![Value::String(Rc::new(
                        rusty_js_runtime::value::JsString::from(rest),
                    ))],
                );
                rt.object_set(
                    iface,
                    "__rl_buf".into(),
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                        String::new(),
                    ))),
                );
            }
            rt.object_set(iface, "__rl_closed".into(), Value::Boolean(true));
            crate::net::net_emit(rt, iface, "close", Vec::new());
            Ok(Value::Undefined)
        });
        let on = rt.object_get(input_obj, "on");
        if rt.is_callable(&on) {
            let ev = |s: &str| {
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    s.to_string(),
                )))
            };
            let _ = rt.call_function(
                on.clone(),
                Value::Object(input_obj),
                vec![ev("data"), data_cb],
            );
            let _ = rt.call_function(on, Value::Object(input_obj), vec![ev("end"), end_cb]);
        }
    }
    let output = opts
        .map(|o| rt.object_get(o, "output"))
        .unwrap_or(Value::Undefined);
    let is_tty = matches!(&output, Value::Object(o) if matches!(rt.object_get(*o, "isTTY"), Value::Boolean(true)));
    rt.set_engine_sentinel(iface, "__rl_output", output);
    rt.object_set(iface, "terminal".into(), Value::Boolean(is_tty));
    rt.set_engine_sentinel(
        iface,
        "__rl_prompt",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "> ".to_string(),
        ))),
    );
    register_method(rt, iface, "setPrompt", |rt, args| {
        if let Value::Object(this) = rt.current_this() {
            rt.set_engine_sentinel(
                this,
                "__rl_prompt",
                args.first().cloned().unwrap_or(Value::Undefined),
            );
        }
        Ok(Value::Undefined)
    });
    register_method(rt, iface, "getPrompt", |rt, _a| {
        if let Value::Object(this) = rt.current_this() {
            return Ok(rt.object_get(this, "__rl_prompt"));
        }
        Ok(Value::Undefined)
    });
    register_method(rt, iface, "prompt", |rt, _a| {
        if let Value::Object(this) = rt.current_this() {
            let out = rt.object_get(this, "__rl_output");

            if matches!(rt.object_get(this, "terminal"), Value::Boolean(true)) {
                rl_write(rt, &out, "\x1b[1G\x1b[0J");
            }
            if let Value::String(s) = rt.object_get(this, "__rl_prompt") {
                rl_write(rt, &out, s.as_str());
            }
        }
        Ok(Value::Undefined)
    });
    register_method(rt, iface, "write", |rt, args| {

        if let Value::Object(this) = rt.current_this() {
            if matches!(rt.object_get(this, "terminal"), Value::Boolean(true)) {
                let out = rt.object_get(this, "__rl_output");
                if let Some(Value::String(s)) = args.first() {
                    rl_write(rt, &out, s.as_str());
                }
            }
        }
        Ok(Value::Undefined)
    });
    register_method(rt, iface, "pause", |rt, _a| Ok(rt.current_this()));
    register_method(rt, iface, "resume", |rt, _a| Ok(rt.current_this()));
    register_method(rt, iface, "close", |rt, _a| {
        if let Value::Object(this) = rt.current_this() {
            rt.object_set(this, "__rl_closed".into(), Value::Boolean(true));
            crate::net::net_emit(rt, this, "close", Vec::new());
        }
        Ok(Value::Undefined)
    });

    register_method(rt, iface, "@@asyncIterator", |rt, _a| {
        use rusty_js_runtime::promise::{new_promise, resolve_promise};
        let this = match rt.current_this() {
            Value::Object(t) => t,
            _ => return Ok(Value::Undefined),
        };
        let it = new_object(rt);
        let queue = rt.alloc_object(RtObject::new_array());
        let waiters = rt.alloc_object(RtObject::new_array());
        rt.set_engine_sentinel(it, "__rlq", Value::Object(queue));
        rt.set_engine_sentinel(it, "__rlw", Value::Object(waiters));
        rt.set_engine_sentinel(it, "__rld", Value::Boolean(false));
        let arr_len = |rt: &mut Runtime, a: rusty_js_runtime::ObjectRef| -> usize {
            match rt.object_get(a, "length") {
                Value::Number(n) if n > 0.0 => n as usize,
                _ => 0,
            }
        };
        let arr_push = |rt: &mut Runtime, a: rusty_js_runtime::ObjectRef, v: Value| {
            let f = rt.object_get(a, "push");
            if rt.is_callable(&f) {
                let _ = rt.call_function(f, Value::Object(a), vec![v]);
            }
        };
        let arr_shift = |rt: &mut Runtime, a: rusty_js_runtime::ObjectRef| -> Value {
            let f = rt.object_get(a, "shift");
            if rt.is_callable(&f) {
                rt.call_function(f, Value::Object(a), vec![])
                    .unwrap_or(Value::Undefined)
            } else {
                Value::Undefined
            }
        };
        let result = |rt: &mut Runtime, v: Value, done: bool| -> Value {
            let r = new_object(rt);
            rt.object_set(r, "value".into(), v);
            rt.object_set(r, "done".into(), Value::Boolean(done));
            Value::Object(r)
        };
        let on_line = make_callable(rt, "rl.ai.line", move |rt, a| {
            let line = a.first().cloned().unwrap_or(Value::Undefined);
            let w = match rt.object_get(it, "__rlw") {
                Value::Object(w) => w,
                _ => return Ok(Value::Undefined),
            };
            if arr_len(rt, w) > 0 {
                if let Value::Object(p) = arr_shift(rt, w) {
                    let res = result(rt, line, false);
                    resolve_promise(rt, p, res);
                    return Ok(Value::Undefined);
                }
            }
            if let Value::Object(q) = rt.object_get(it, "__rlq") {
                arr_push(rt, q, line);
            }
            Ok(Value::Undefined)
        });
        let on_close = make_callable(rt, "rl.ai.close", move |rt, _a| {
            rt.set_engine_sentinel(it, "__rld", Value::Boolean(true));
            if let Value::Object(w) = rt.object_get(it, "__rlw") {
                while arr_len(rt, w) > 0 {
                    if let Value::Object(p) = arr_shift(rt, w) {
                        let res = result(rt, Value::Undefined, true);
                        resolve_promise(rt, p, res);
                    } else {
                        break;
                    }
                }
            }
            Ok(Value::Undefined)
        });
        let on = rt.object_get(this, "on");
        if rt.is_callable(&on) {
            let ev = |s: &str| Value::String(Rc::new(rusty_js_runtime::value::JsString::from(s)));
            let _ = rt.call_function(
                on.clone(),
                Value::Object(this),
                vec![ev("line"), Value::Object(on_line)],
            );
            let _ = rt.call_function(
                on,
                Value::Object(this),
                vec![ev("close"), Value::Object(on_close)],
            );
        }

        if matches!(rt.object_get(this, "__rl_closed"), Value::Boolean(true)) {
            rt.set_engine_sentinel(it, "__rld", Value::Boolean(true));
        }
        register_method(rt, it, "next", |rt, _a| {
            use rusty_js_runtime::promise::{new_promise, resolve_promise};
            let it = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let alen = |rt: &mut Runtime, a: rusty_js_runtime::ObjectRef| -> usize {
                match rt.object_get(a, "length") {
                    Value::Number(n) if n > 0.0 => n as usize,
                    _ => 0,
                }
            };
            let ashift = |rt: &mut Runtime, a: rusty_js_runtime::ObjectRef| -> Value {
                let f = rt.object_get(a, "shift");
                if rt.is_callable(&f) {
                    rt.call_function(f, Value::Object(a), vec![])
                        .unwrap_or(Value::Undefined)
                } else {
                    Value::Undefined
                }
            };
            let mkres = |rt: &mut Runtime, v: Value, done: bool| -> Value {
                let r = new_object(rt);
                rt.object_set(r, "value".into(), v);
                rt.object_set(r, "done".into(), Value::Boolean(done));
                Value::Object(r)
            };
            if let Value::Object(q) = rt.object_get(it, "__rlq") {
                if alen(rt, q) > 0 {
                    let line = ashift(rt, q);
                    let p = new_promise(rt);
                    let res = mkres(rt, line, false);
                    resolve_promise(rt, p, res);
                    return Ok(Value::Object(p));
                }
            }
            if matches!(rt.object_get(it, "__rld"), Value::Boolean(true)) {
                let p = new_promise(rt);
                let res = mkres(rt, Value::Undefined, true);
                resolve_promise(rt, p, res);
                return Ok(Value::Object(p));
            }
            let p = new_promise(rt);
            if let Value::Object(w) = rt.object_get(it, "__rlw") {
                let f = rt.object_get(w, "push");
                if rt.is_callable(&f) {
                    let _ = rt.call_function(f, Value::Object(w), vec![Value::Object(p)]);
                }
            }
            Ok(Value::Object(p))
        });
        register_method(rt, it, "return", |rt, _a| {
            use rusty_js_runtime::promise::{new_promise, resolve_promise};
            let p = new_promise(rt);
            let r = new_object(rt);
            rt.object_set(r, "value".into(), Value::Undefined);
            rt.object_set(r, "done".into(), Value::Boolean(true));
            resolve_promise(rt, p, Value::Object(r));
            Ok(Value::Object(p))
        });
        register_method(rt, it, "@@asyncIterator", |rt, _a| Ok(rt.current_this()));
        Ok(Value::Object(it))
    });

    let pq = promise_q;
    register_method(rt, iface, "question", move |rt, args| {
        let this = match rt.current_this() {
            Value::Object(t) => t,
            _ => return Ok(Value::Undefined),
        };
        let out = rt.object_get(this, "__rl_output");
        if let Some(Value::String(q)) = args.first() {
            rl_write(rt, &out, q.as_str());
        }
        let empty = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            String::new(),
        )));
        if pq {
            let p = rusty_js_runtime::promise::new_promise(rt);
            rusty_js_runtime::promise::resolve_promise(rt, p, empty);
            Ok(Value::Object(p))
        } else {
            if let Some(cb) = args.iter().rev().find(|v| rt.is_callable(v)).cloned() {
                let _ = rt.call_function(cb, Value::Undefined, vec![empty]);
            }
            Ok(Value::Undefined)
        }
    });
    iface
}

fn rl_install_cursor_methods(rt: &mut Runtime, ns: rusty_js_runtime::ObjectRef) {
    register_method(rt, ns, "cursorTo", |rt, args| {
        let stream = args.first().cloned().unwrap_or(Value::Undefined);
        let x = rl_num(&args, 1);
        let esc = match args.get(2) {
            Some(Value::Number(y)) => format!("\x1b[{};{}H", *y as i64 + 1, x + 1),
            _ => format!("\x1b[{}G", x + 1),
        };
        rl_write(rt, &stream, &esc);
        if let Some(cb) = args.iter().rev().find(|v| rt.is_callable(v)).cloned() {
            let _ = rt.call_function(cb, Value::Undefined, Vec::new());
        }
        Ok(Value::Boolean(true))
    });
    register_method(rt, ns, "moveCursor", |rt, args| {
        let stream = args.first().cloned().unwrap_or(Value::Undefined);
        let (dx, dy) = (rl_num(&args, 1), rl_num(&args, 2));
        let mut esc = String::new();
        if dx > 0 {
            esc.push_str(&format!("\x1b[{}C", dx));
        } else if dx < 0 {
            esc.push_str(&format!("\x1b[{}D", -dx));
        }
        if dy > 0 {
            esc.push_str(&format!("\x1b[{}B", dy));
        } else if dy < 0 {
            esc.push_str(&format!("\x1b[{}A", -dy));
        }
        rl_write(rt, &stream, &esc);
        Ok(Value::Boolean(true))
    });
    register_method(rt, ns, "clearLine", |rt, args| {
        let stream = args.first().cloned().unwrap_or(Value::Undefined);
        let dir = rl_num(&args, 1);
        let esc = match dir {
            d if d < 0 => "\x1b[1K",
            d if d > 0 => "\x1b[0K",
            _ => "\x1b[2K",
        };
        rl_write(rt, &stream, esc);
        Ok(Value::Boolean(true))
    });
    register_method(rt, ns, "clearScreenDown", |rt, args| {
        let stream = args.first().cloned().unwrap_or(Value::Undefined);
        rl_write(rt, &stream, "\x1b[0J");
        Ok(Value::Boolean(true))
    });
    register_method(rt, ns, "emitKeypressEvents", |_rt, _a| Ok(Value::Undefined));
}

pub fn install_readline(rt: &mut Runtime) {
    let ns = new_object(rt);
    register_method(rt, ns, "createInterface", |rt, args| {
        let opts = match args.first() {
            Some(Value::Object(o)) => Some(*o),
            _ => None,
        };
        Ok(Value::Object(rl_make_interface(rt, opts, false)))
    });
    register_method(rt, ns, "Interface", |rt, _a| Ok(rt.current_this()));
    rl_install_cursor_methods(rt, ns);
    if let Value::Object(iface) = rt.object_get(ns, "Interface") {
        crate::register::make_subclassable(
            rt,
            iface,
            crate::register::proto_of_global_ctor(rt, "events"),
        );
    }

    let prom = new_object(rt);
    register_method(rt, prom, "createInterface", |rt, args| {
        let opts = match args.first() {
            Some(Value::Object(o)) => Some(*o),
            _ => None,
        };
        Ok(Value::Object(rl_make_interface(rt, opts, true)))
    });
    register_method(rt, prom, "Interface", |rt, _a| Ok(rt.current_this()));
    register_method(rt, prom, "Readline", |rt, _a| Ok(rt.current_this()));
    rt.object_set(ns, "promises".into(), Value::Object(prom));
    rt.define_global_property("readline_promises", Value::Object(prom));
    rt.define_global_property("readline", Value::Object(ns));
}

pub fn install_constants(rt: &mut Runtime) {

    let ns = new_object(rt);

    const CONSTS: &[(&str, f64)] = &[
        ("COPYFILE_EXCL", 1.0),
        ("COPYFILE_FICLONE", 2.0),
        ("COPYFILE_FICLONE_FORCE", 4.0),
        ("DH_CHECK_P_NOT_PRIME", 1.0),
        ("DH_CHECK_P_NOT_SAFE_PRIME", 2.0),
        ("DH_NOT_SUITABLE_GENERATOR", 8.0),
        ("DH_UNABLE_TO_CHECK_GENERATOR", 4.0),
        ("E2BIG", 7.0),
        ("EACCES", 13.0),
        ("EADDRINUSE", 98.0),
        ("EADDRNOTAVAIL", 99.0),
        ("EAFNOSUPPORT", 97.0),
        ("EAGAIN", 11.0),
        ("EALREADY", 114.0),
        ("EBADF", 9.0),
        ("EBADMSG", 74.0),
        ("EBUSY", 16.0),
        ("ECANCELED", 125.0),
        ("ECHILD", 10.0),
        ("ECONNABORTED", 103.0),
        ("ECONNREFUSED", 111.0),
        ("ECONNRESET", 104.0),
        ("EDEADLK", 35.0),
        ("EDESTADDRREQ", 89.0),
        ("EDOM", 33.0),
        ("EDQUOT", 122.0),
        ("EEXIST", 17.0),
        ("EFAULT", 14.0),
        ("EFBIG", 27.0),
        ("EHOSTUNREACH", 113.0),
        ("EIDRM", 43.0),
        ("EILSEQ", 84.0),
        ("EINPROGRESS", 115.0),
        ("EINTR", 4.0),
        ("EINVAL", 22.0),
        ("EIO", 5.0),
        ("EISCONN", 106.0),
        ("EISDIR", 21.0),
        ("ELOOP", 40.0),
        ("EMFILE", 24.0),
        ("EMLINK", 31.0),
        ("EMSGSIZE", 90.0),
        ("EMULTIHOP", 72.0),
        ("ENAMETOOLONG", 36.0),
        ("ENETDOWN", 100.0),
        ("ENETRESET", 102.0),
        ("ENETUNREACH", 101.0),
        ("ENFILE", 23.0),
        ("ENGINE_METHOD_ALL", 65535.0),
        ("ENGINE_METHOD_CIPHERS", 64.0),
        ("ENGINE_METHOD_DH", 4.0),
        ("ENGINE_METHOD_DIGESTS", 128.0),
        ("ENGINE_METHOD_DSA", 2.0),
        ("ENGINE_METHOD_EC", 2048.0),
        ("ENGINE_METHOD_NONE", 0.0),
        ("ENGINE_METHOD_PKEY_ASN1_METHS", 1024.0),
        ("ENGINE_METHOD_PKEY_METHS", 512.0),
        ("ENGINE_METHOD_RAND", 8.0),
        ("ENGINE_METHOD_RSA", 1.0),
        ("ENOBUFS", 105.0),
        ("ENODATA", 61.0),
        ("ENODEV", 19.0),
        ("ENOENT", 2.0),
        ("ENOEXEC", 8.0),
        ("ENOLCK", 37.0),
        ("ENOLINK", 67.0),
        ("ENOMEM", 12.0),
        ("ENOMSG", 42.0),
        ("ENOPROTOOPT", 92.0),
        ("ENOSPC", 28.0),
        ("ENOSR", 63.0),
        ("ENOSTR", 60.0),
        ("ENOSYS", 38.0),
        ("ENOTCONN", 107.0),
        ("ENOTDIR", 20.0),
        ("ENOTEMPTY", 39.0),
        ("ENOTSOCK", 88.0),
        ("ENOTSUP", 95.0),
        ("ENOTTY", 25.0),
        ("ENXIO", 6.0),
        ("EOPNOTSUPP", 95.0),
        ("EOVERFLOW", 75.0),
        ("EPERM", 1.0),
        ("EPIPE", 32.0),
        ("EPROTO", 71.0),
        ("EPROTONOSUPPORT", 93.0),
        ("EPROTOTYPE", 91.0),
        ("ERANGE", 34.0),
        ("EROFS", 30.0),
        ("ESPIPE", 29.0),
        ("ESRCH", 3.0),
        ("ESTALE", 116.0),
        ("ETIME", 62.0),
        ("ETIMEDOUT", 110.0),
        ("ETXTBSY", 26.0),
        ("EWOULDBLOCK", 11.0),
        ("EXDEV", 18.0),
        ("F_OK", 0.0),
        ("OPENSSL_VERSION_NUMBER", 810549344.0),
        ("O_APPEND", 1024.0),
        ("O_CREAT", 64.0),
        ("O_DIRECT", 16384.0),
        ("O_DIRECTORY", 65536.0),
        ("O_DSYNC", 4096.0),
        ("O_EXCL", 128.0),
        ("O_NOATIME", 262144.0),
        ("O_NOCTTY", 256.0),
        ("O_NOFOLLOW", 131072.0),
        ("O_NONBLOCK", 2048.0),
        ("O_RDONLY", 0.0),
        ("O_RDWR", 2.0),
        ("O_SYNC", 1052672.0),
        ("O_TRUNC", 512.0),
        ("O_WRONLY", 1.0),
        ("POINT_CONVERSION_COMPRESSED", 2.0),
        ("POINT_CONVERSION_HYBRID", 6.0),
        ("POINT_CONVERSION_UNCOMPRESSED", 4.0),
        ("PRIORITY_ABOVE_NORMAL", -7.0),
        ("PRIORITY_BELOW_NORMAL", 10.0),
        ("PRIORITY_HIGH", -14.0),
        ("PRIORITY_HIGHEST", -20.0),
        ("PRIORITY_LOW", 19.0),
        ("PRIORITY_NORMAL", 0.0),
        ("RSA_NO_PADDING", 3.0),
        ("RSA_PKCS1_OAEP_PADDING", 4.0),
        ("RSA_PKCS1_PADDING", 1.0),
        ("RSA_PKCS1_PSS_PADDING", 6.0),
        ("RSA_PSS_SALTLEN_AUTO", -2.0),
        ("RSA_PSS_SALTLEN_DIGEST", -1.0),
        ("RSA_PSS_SALTLEN_MAX_SIGN", -2.0),
        ("RSA_X931_PADDING", 5.0),
        ("RTLD_DEEPBIND", 8.0),
        ("RTLD_GLOBAL", 256.0),
        ("RTLD_LAZY", 1.0),
        ("RTLD_LOCAL", 0.0),
        ("RTLD_NOW", 2.0),
        ("R_OK", 4.0),
        ("SIGABRT", 6.0),
        ("SIGALRM", 14.0),
        ("SIGBUS", 7.0),
        ("SIGCHLD", 17.0),
        ("SIGCONT", 18.0),
        ("SIGFPE", 8.0),
        ("SIGHUP", 1.0),
        ("SIGILL", 4.0),
        ("SIGINT", 2.0),
        ("SIGIO", 29.0),
        ("SIGIOT", 6.0),
        ("SIGKILL", 9.0),
        ("SIGPIPE", 13.0),
        ("SIGPOLL", 29.0),
        ("SIGPROF", 27.0),
        ("SIGPWR", 30.0),
        ("SIGQUIT", 3.0),
        ("SIGSEGV", 11.0),
        ("SIGSTKFLT", 16.0),
        ("SIGSTOP", 19.0),
        ("SIGSYS", 31.0),
        ("SIGTERM", 15.0),
        ("SIGTRAP", 5.0),
        ("SIGTSTP", 20.0),
        ("SIGTTIN", 21.0),
        ("SIGTTOU", 22.0),
        ("SIGURG", 23.0),
        ("SIGUSR1", 10.0),
        ("SIGUSR2", 12.0),
        ("SIGVTALRM", 26.0),
        ("SIGWINCH", 28.0),
        ("SIGXCPU", 24.0),
        ("SIGXFSZ", 25.0),
        ("SSL_OP_ALL", 2147485776.0),
        ("SSL_OP_ALLOW_NO_DHE_KEX", 1024.0),
        ("SSL_OP_ALLOW_UNSAFE_LEGACY_RENEGOTIATION", 262144.0),
        ("SSL_OP_CIPHER_SERVER_PREFERENCE", 4194304.0),
        ("SSL_OP_CISCO_ANYCONNECT", 32768.0),
        ("SSL_OP_COOKIE_EXCHANGE", 8192.0),
        ("SSL_OP_CRYPTOPRO_TLSEXT_BUG", 2147483648.0),
        ("SSL_OP_DONT_INSERT_EMPTY_FRAGMENTS", 2048.0),
        ("SSL_OP_LEGACY_SERVER_CONNECT", 4.0),
        ("SSL_OP_NO_COMPRESSION", 131072.0),
        ("SSL_OP_NO_ENCRYPT_THEN_MAC", 524288.0),
        ("SSL_OP_NO_QUERY_MTU", 4096.0),
        ("SSL_OP_NO_RENEGOTIATION", 1073741824.0),
        ("SSL_OP_NO_SESSION_RESUMPTION_ON_RENEGOTIATION", 65536.0),
        ("SSL_OP_NO_SSLv2", 0.0),
        ("SSL_OP_NO_SSLv3", 33554432.0),
        ("SSL_OP_NO_TICKET", 16384.0),
        ("SSL_OP_NO_TLSv1", 67108864.0),
        ("SSL_OP_NO_TLSv1_1", 268435456.0),
        ("SSL_OP_NO_TLSv1_2", 134217728.0),
        ("SSL_OP_NO_TLSv1_3", 536870912.0),
        ("SSL_OP_PRIORITIZE_CHACHA", 2097152.0),
        ("SSL_OP_TLS_ROLLBACK_BUG", 8388608.0),
        ("S_IFBLK", 24576.0),
        ("S_IFCHR", 8192.0),
        ("S_IFDIR", 16384.0),
        ("S_IFIFO", 4096.0),
        ("S_IFLNK", 40960.0),
        ("S_IFMT", 61440.0),
        ("S_IFREG", 32768.0),
        ("S_IFSOCK", 49152.0),
        ("S_IRGRP", 32.0),
        ("S_IROTH", 4.0),
        ("S_IRUSR", 256.0),
        ("S_IRWXG", 56.0),
        ("S_IRWXO", 7.0),
        ("S_IRWXU", 448.0),
        ("S_IWGRP", 16.0),
        ("S_IWOTH", 2.0),
        ("S_IWUSR", 128.0),
        ("S_IXGRP", 8.0),
        ("S_IXOTH", 1.0),
        ("S_IXUSR", 64.0),
        ("TLS1_1_VERSION", 770.0),
        ("TLS1_2_VERSION", 771.0),
        ("TLS1_3_VERSION", 772.0),
        ("TLS1_VERSION", 769.0),
        ("UV_DIRENT_BLOCK", 7.0),
        ("UV_DIRENT_CHAR", 6.0),
        ("UV_DIRENT_DIR", 2.0),
        ("UV_DIRENT_FIFO", 4.0),
        ("UV_DIRENT_FILE", 1.0),
        ("UV_DIRENT_LINK", 3.0),
        ("UV_DIRENT_SOCKET", 5.0),
        ("UV_DIRENT_UNKNOWN", 0.0),
        ("UV_FS_COPYFILE_EXCL", 1.0),
        ("UV_FS_COPYFILE_FICLONE", 2.0),
        ("UV_FS_COPYFILE_FICLONE_FORCE", 4.0),
        ("UV_FS_O_FILEMAP", 0.0),
        ("UV_FS_SYMLINK_DIR", 1.0),
        ("UV_FS_SYMLINK_JUNCTION", 2.0),
        ("W_OK", 2.0),
        ("X_OK", 1.0),
    ];
    for (name, val) in CONSTS {
        rt.object_set(ns, (*name).to_string(), Value::Number(*val));
    }
    #[cfg(target_os = "macos")]
    {
        const DARWIN_DELETE: &[&str] = &[
            "O_DIRECT",
            "O_NOATIME",
            "RTLD_DEEPBIND",
            "SIGPOLL",
            "SIGPWR",
            "SIGSTKFLT",
        ];
        for name in DARWIN_DELETE {
            let _ = rt.delete_own_via(
                &Value::Object(ns),
                &Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    (*name).to_string(),
                ))),
            );
        }

        const DARWIN_NUMERIC: &[(&str, f64)] = &[
            ("EADDRINUSE", 48.0),
            ("EADDRNOTAVAIL", 49.0),
            ("EAFNOSUPPORT", 47.0),
            ("EAGAIN", 35.0),
            ("EALREADY", 37.0),
            ("EBADMSG", 94.0),
            ("ECANCELED", 89.0),
            ("ECONNABORTED", 53.0),
            ("ECONNREFUSED", 61.0),
            ("ECONNRESET", 54.0),
            ("EDEADLK", 11.0),
            ("EDESTADDRREQ", 39.0),
            ("EDQUOT", 69.0),
            ("EHOSTUNREACH", 65.0),
            ("EIDRM", 90.0),
            ("EILSEQ", 92.0),
            ("EINPROGRESS", 36.0),
            ("EISCONN", 56.0),
            ("ELOOP", 62.0),
            ("EMSGSIZE", 40.0),
            ("EMULTIHOP", 95.0),
            ("ENAMETOOLONG", 63.0),
            ("ENETDOWN", 50.0),
            ("ENETRESET", 52.0),
            ("ENETUNREACH", 51.0),
            ("ENOBUFS", 55.0),
            ("ENODATA", 96.0),
            ("ENOLCK", 77.0),
            ("ENOLINK", 97.0),
            ("ENOMSG", 91.0),
            ("ENOPROTOOPT", 42.0),
            ("ENOSR", 98.0),
            ("ENOSTR", 99.0),
            ("ENOSYS", 78.0),
            ("ENOTCONN", 57.0),
            ("ENOTEMPTY", 66.0),
            ("ENOTSOCK", 38.0),
            ("ENOTSUP", 45.0),
            ("EOPNOTSUPP", 102.0),
            ("EOVERFLOW", 84.0),
            ("EPROTO", 100.0),
            ("EPROTONOSUPPORT", 43.0),
            ("EPROTOTYPE", 41.0),
            ("ESTALE", 70.0),
            ("ETIME", 101.0),
            ("ETIMEDOUT", 60.0),
            ("EWOULDBLOCK", 35.0),
            ("OPENSSL_VERSION_NUMBER", 811597856.0),
            ("O_APPEND", 8.0),
            ("O_CREAT", 512.0),
            ("O_DIRECTORY", 1048576.0),
            ("O_DSYNC", 4194304.0),
            ("O_EXCL", 2048.0),
            ("O_NOCTTY", 131072.0),
            ("O_NOFOLLOW", 256.0),
            ("O_NONBLOCK", 4.0),
            ("O_SYNC", 128.0),
            ("O_SYMLINK", 2097152.0),
            ("O_TRUNC", 1024.0),
            ("RTLD_GLOBAL", 8.0),
            ("RTLD_LOCAL", 4.0),
            ("SIGBUS", 10.0),
            ("SIGCHLD", 20.0),
            ("SIGCONT", 19.0),
            ("SIGINFO", 29.0),
            ("SIGIO", 23.0),
            ("SIGSTOP", 17.0),
            ("SIGSYS", 12.0),
            ("SIGTSTP", 18.0),
            ("SIGURG", 16.0),
            ("SIGUSR1", 30.0),
            ("SIGUSR2", 31.0),
        ];
        for (name, val) in DARWIN_NUMERIC {
            rt.object_set(ns, (*name).to_string(), Value::Number(*val));
        }
    }
    rt.object_set(
        ns,
        "defaultCoreCipherList".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            NODE_DEFAULT_CIPHER_LIST.to_string(),
        ))),
    );
    rt.define_global_property("constants", Value::Object(ns));
}

pub fn install_constants_default_cipher_list(rt: &mut Runtime) {

    rt.materialize_lazy_host_module("constants");
    let Value::Object(constants) = rt.global_get("constants") else {
        return;
    };
    if !matches!(
        rt.object_get(constants, "defaultCipherList"),
        Value::Undefined
    ) {
        return;
    }
    rt.define_data_property_attrs(
        constants,
        "defaultCipherList",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            NODE_DEFAULT_CIPHER_LIST.to_string(),
        ))),
        false,
        true,
        false,
    );
}

fn sd_check_byte(b: u8) -> i32 {
    if b <= 0x7F {
        0
    } else if b >> 5 == 0x06 {
        2
    } else if b >> 4 == 0x0E {
        3
    } else if b >> 3 == 0x1E {
        4
    } else if b >> 6 == 0x02 {
        -1
    } else {
        -2
    }
}

fn sd_utf8_incomplete_tail(buf: &[u8]) -> usize {
    let len = buf.len();
    if len == 0 {
        return 0;
    }
    let nb = sd_check_byte(buf[len - 1]);
    if nb >= 0 {
        return if nb >= 2 { 1 } else { 0 };
    }
    if nb == -2 || len < 2 {
        return 0;
    }
    let nb2 = sd_check_byte(buf[len - 2]);
    if nb2 >= 0 {
        return if nb2 >= 3 { 2 } else { 0 };
    }
    if nb2 == -2 || len < 3 {
        return 0;
    }
    let nb3 = sd_check_byte(buf[len - 3]);
    if nb3 >= 0 {
        return if nb3 >= 4 { 3 } else { 0 };
    }
    0
}

fn sd_to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{:02x}", x)).collect()
}
fn sd_from_hex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap_or(0))
        .collect()
}

fn sd_chunk_bytes(rt: &Runtime, id: rusty_js_runtime::ObjectRef) -> Vec<u8> {
    let len = match rt.object_get(id, "length") {
        Value::Number(n) => n as usize,
        _ => 0,
    };
    (0..len)
        .map(|i| match rt.object_get(id, &i.to_string()) {
            Value::Number(n) => n as u8,
            _ => 0,
        })
        .collect()
}

fn sd_decode(enc: &str, prev: &[u8], chunk: &[u8]) -> (String, Vec<u8>) {
    let mut bytes = Vec::with_capacity(prev.len() + chunk.len());
    bytes.extend_from_slice(prev);
    bytes.extend_from_slice(chunk);
    let len = bytes.len();
    match enc {
        "utf8" | "utf-8" => {
            let tail = sd_utf8_incomplete_tail(&bytes);
            let split = len - tail;
            (
                String::from_utf8_lossy(&bytes[..split]).into_owned(),
                bytes[split..].to_vec(),
            )
        }
        "base64" | "base64url" => {
            let keep = len % 3;
            let split = len - keep;
            (base64_encode(&bytes[..split]), bytes[split..].to_vec())
        }
        "hex" => (sd_to_hex(&bytes), Vec::new()),
        "latin1" | "binary" => (bytes.iter().map(|b| *b as char).collect(), Vec::new()),
        "ascii" => (
            bytes.iter().map(|b| (b & 0x7f) as char).collect(),
            Vec::new(),
        ),
        "utf16le" | "ucs2" | "utf-16le" => {
            let mut keep = len % 2;

            let complete = len - keep;
            if complete >= 2 {
                let last = u16::from_le_bytes([bytes[complete - 2], bytes[complete - 1]]);
                if (0xD800..=0xDBFF).contains(&last) {
                    keep += 2;
                }
            }
            let split = len - keep;
            let units: Vec<u16> = bytes[..split]
                .chunks(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            (String::from_utf16_lossy(&units), bytes[split..].to_vec())
        }
        _ => (String::from_utf8_lossy(&bytes).into_owned(), Vec::new()),
    }
}

fn sd_flush(enc: &str, buf: &[u8]) -> String {
    if buf.is_empty() {
        return String::new();
    }
    match enc {
        "base64" | "base64url" => base64_encode(buf),
        "utf16le" | "ucs2" | "utf-16le" => {

            let units: Vec<u16> = buf
                .chunks(2)
                .filter(|c| c.len() == 2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            let mut s = String::from_utf16_lossy(&units);
            if buf.len() % 2 == 1 {
                s.push('\u{FFFD}');
            }
            s
        }
        "hex" | "latin1" | "binary" | "ascii" => String::new(),
        _ => String::from_utf8_lossy(buf).into_owned(),
    }
}

fn sd_encoding(rt: &Runtime, id: rusty_js_runtime::ObjectRef) -> String {
    match rt.object_get(id, "encoding") {
        Value::String(s) => s.as_str().to_lowercase(),
        _ => "utf8".to_string(),
    }
}

pub fn install_string_decoder(rt: &mut Runtime) {

    let ns = new_object(rt);

    let ctor = make_callable(rt, "StringDecoder", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => rt.alloc_object(RtObject::new_ordinary()),
        };
        let encoding = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => "utf8".to_string(),
        };
        rt.object_set(
            this,
            "encoding".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(encoding))),
        );

        rt.set_engine_sentinel(
            this,
            "__sd_buf",
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                String::new(),
            ))),
        );
        Ok(Value::Object(this))
    });
    let proto = match rt.object_get(ctor, "prototype") {
        Value::Object(p) => p,
        _ => {
            let p = new_object(rt);
            rt.object_set(ctor, "prototype".into(), Value::Object(p));
            p
        }
    };
    register_method(rt, proto, "write", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => {
                return Ok(Value::String(Rc::new(
                    rusty_js_runtime::value::JsString::from(String::new()),
                )))
            }
        };

        if let Some(Value::String(s)) = args.first() {
            return Ok(Value::String(s.clone()));
        }
        let enc = sd_encoding(rt, this);
        let prev = match rt.object_get(this, "__sd_buf") {
            Value::String(s) => sd_from_hex(s.as_str()),
            _ => Vec::new(),
        };
        let chunk = match args.first() {
            Some(Value::Object(oid)) => sd_chunk_bytes(rt, *oid),
            _ => Vec::new(),
        };
        let (out, newbuf) = sd_decode(&enc, &prev, &chunk);
        rt.set_engine_sentinel(
            this,
            "__sd_buf",
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(sd_to_hex(
                &newbuf,
            )))),
        );
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(out),
        )))
    });
    register_method(rt, proto, "end", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => {
                return Ok(Value::String(Rc::new(
                    rusty_js_runtime::value::JsString::from(String::new()),
                )))
            }
        };
        let enc = sd_encoding(rt, this);
        let prev = match rt.object_get(this, "__sd_buf") {
            Value::String(s) => sd_from_hex(s.as_str()),
            _ => Vec::new(),
        };
        let chunk = match args.first() {
            Some(Value::Object(oid)) => sd_chunk_bytes(rt, *oid),
            Some(Value::String(s)) => return Ok(Value::String(s.clone())),
            _ => Vec::new(),
        };
        let (mut out, newbuf) = sd_decode(&enc, &prev, &chunk);
        out.push_str(&sd_flush(&enc, &newbuf));
        rt.set_engine_sentinel(
            this,
            "__sd_buf",
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                String::new(),
            ))),
        );
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(out),
        )))
    });
    rt.object_set(ns, "StringDecoder".into(), Value::Object(ctor));
    rt.define_global_property("string_decoder", Value::Object(ns));
}

pub(crate) fn intrinsic_buffer_from_bytes(rt: &mut Runtime, bytes: &[u8]) -> Value {
    let mut ab = RtObject::new_ordinary();
    ab.proto = rt.array_buffer_prototype;
    let ab_id = rt.alloc_object(ab);
    rt.array_buffers.insert(
        ab_id,
        ArrayBufferRecord {
            byte_length: bytes.len(),
            max_byte_length: bytes.len(),
            backing_epoch: 0,
            data: bytes.to_vec(),
            detached: false,
            untransferable: false,
            shared: None,
        },
    );

    let mut o = RtObject::new_ordinary();

    o.proto = rt.intrinsic_buffer_prototype_id;
    o.set_own("length".into(), Value::Number(bytes.len() as f64));
    o.set_own("byteLength".into(), Value::Number(bytes.len() as f64));
    o.set_own("byteOffset".into(), Value::Number(0.0));
    o.set_own("buffer".into(), Value::Object(ab_id));
    o.set_own_internal("__is_buffer__".into(), Value::Boolean(true));
    o.set_own_internal(
        "__kind".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "Uint8Array",
        ))),
    );
    o.set_own_internal(
        "__ta_kind".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "Uint8Array",
        ))),
    );
    o.set_own_internal(
        "@@toStringTag".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "Uint8Array",
        ))),
    );
    o.is_buffer = true;
    let id = rt.alloc_object(o);
    let _buf_root = rt.push_temporary_value_roots(&[Value::Object(id)]);
    rt.register_typed_array_view(
        id,
        TypedArrayViewRecord {
            buffer: ab_id,
            byte_offset: 0,
            fixed_length: Some(bytes.len()),
            bytes_per_element: 1,
            element_kind: Rc::<str>::from("Uint8Array"),
        },
    );
    for (i, b) in bytes.iter().enumerate() {
        rt.object_set(id, i.to_string(), Value::Number(*b as f64));
    }
    install_buffer_methods(rt, id);
    Value::Object(id)
}

const NODE_BUFFER_POOL_SIZE: usize = 8192;

fn intrinsic_buffer_from_pooled_bytes(
    rt: &mut Runtime,
    buffer_ctor: rusty_js_runtime::ObjectRef,
    bytes: &[u8],
) -> Value {
    if bytes.is_empty() || bytes.len() > (NODE_BUFFER_POOL_SIZE / 2) {
        return intrinsic_buffer_from_bytes(rt, bytes);
    }
    let current_pool = match rt.object_get(buffer_ctor, "__cruft_buffer_pool") {
        Value::Object(id) if rt.array_buffers.contains_key(&id) => Some(id),
        _ => None,
    };
    let current_offset = match rt.object_get(buffer_ctor, "__cruft_buffer_pool_offset") {
        Value::Number(n) if n.is_finite() && n >= 0.0 => n as usize,
        _ => 0,
    };
    let needs_new = current_pool
        .and_then(|id| rt.array_buffers.get(&id).map(|r| r.byte_len()))
        .map(|len| current_offset.saturating_add(bytes.len()) > len)
        .unwrap_or(true);
    let (ab_id, offset) = if needs_new {
        let mut ab = RtObject::new_ordinary();
        ab.proto = rt.array_buffer_prototype;
        let ab_id = rt.alloc_object(ab);
        rt.array_buffers.insert(
            ab_id,
            ArrayBufferRecord {
                byte_length: NODE_BUFFER_POOL_SIZE,
                max_byte_length: NODE_BUFFER_POOL_SIZE,
                backing_epoch: 0,
                data: vec![0u8; NODE_BUFFER_POOL_SIZE],
                detached: false,
                untransferable: true,
                shared: None,
            },
        );
        rt.set_engine_sentinel(buffer_ctor, "__cruft_buffer_pool", Value::Object(ab_id));
        (ab_id, 0usize)
    } else {
        (current_pool.unwrap(), current_offset)
    };
    if let Some(rec) = rt.array_buffers.get_mut(&ab_id) {
        rec.write_bytes(offset, bytes);
    }
    let next_offset = (offset + bytes.len() + 7) & !7;
    rt.set_engine_sentinel(
        buffer_ctor,
        "__cruft_buffer_pool_offset",
        Value::Number(next_offset as f64),
    );

    let mut o = RtObject::new_ordinary();
    o.set_own("length".into(), Value::Number(bytes.len() as f64));
    o.set_own("byteLength".into(), Value::Number(bytes.len() as f64));
    o.set_own("byteOffset".into(), Value::Number(offset as f64));
    o.set_own("buffer".into(), Value::Object(ab_id));
    o.set_own_internal("__is_buffer__".into(), Value::Boolean(true));
    o.set_own_internal(
        "__kind".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "Uint8Array",
        ))),
    );
    o.set_own_internal(
        "__ta_kind".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "Uint8Array",
        ))),
    );
    o.set_own_internal(
        "@@toStringTag".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "Uint8Array",
        ))),
    );
    o.is_buffer = true;
    let id = rt.alloc_object(o);
    let _buf_root = rt.push_temporary_value_roots(&[Value::Object(id)]);
    rt.register_typed_array_view(
        id,
        TypedArrayViewRecord {
            buffer: ab_id,
            byte_offset: offset,
            fixed_length: Some(bytes.len()),
            bytes_per_element: 1,
            element_kind: Rc::<str>::from("Uint8Array"),
        },
    );
    for (i, b) in bytes.iter().enumerate() {
        rt.object_set(id, i.to_string(), Value::Number(*b as f64));
    }
    install_buffer_methods(rt, id);
    Value::Object(id)
}

fn install_buffer_species_constructor(rt: &mut Runtime, buffer_ctor: rusty_js_runtime::ObjectRef) {
    let fast_buffer = make_callable(rt, "FastBuffer", |rt, args| {
        let bytes = match args.first() {
            Some(Value::Object(buffer_id)) if rt.array_buffers.contains_key(buffer_id) => {
                let offset = match args.get(1) {
                    Some(Value::Number(n)) => (*n).max(0.0) as usize,
                    _ => 0,
                };
                let requested_len = match args.get(2) {
                    Some(Value::Number(n)) => Some((*n).max(0.0) as usize),
                    _ => None,
                };
                let end = match rt.array_buffers.get(buffer_id) {
                    Some(rec) => {
                        let total = rec.byte_len();
                        requested_len
                            .map(|len| offset.saturating_add(len).min(total))
                            .unwrap_or(total)
                    }
                    None => offset,
                };
                rt.array_buffers
                    .get(buffer_id)
                    .map(|rec| rec.read_bytes(offset, end))
                    .unwrap_or_default()
            }
            Some(Value::Object(view_id)) => {
                let offset_arg = match args.get(1) {
                    Some(Value::Number(n)) => (*n).max(0.0) as usize,
                    _ => 0,
                };
                let len_arg = match args.get(2) {
                    Some(Value::Number(n)) => Some((*n).max(0.0) as usize),
                    _ => None,
                };
                if let Some(view) = rt.typed_array_views.get(view_id).cloned() {
                    let start = view.byte_offset.saturating_add(offset_arg);
                    let available = view
                        .fixed_length
                        .unwrap_or_else(|| {
                            rt.array_buffers
                                .get(&view.buffer)
                                .map(|rec| rec.byte_len().saturating_sub(view.byte_offset))
                                .unwrap_or(0)
                                / view.bytes_per_element.max(1)
                        })
                        .saturating_mul(view.bytes_per_element.max(1));
                    let end = len_arg
                        .map(|len| start.saturating_add(len).min(view.byte_offset + available))
                        .unwrap_or(view.byte_offset + available);
                    rt.array_buffers
                        .get(&view.buffer)
                        .map(|rec| rec.read_bytes(start, end))
                        .unwrap_or_default()
                } else {
                    let len = match rt.object_get(*view_id, "length") {
                        Value::Number(n) => n as usize,
                        _ => 0,
                    };
                    let start = offset_arg.min(len);
                    let end = len_arg
                        .map(|n| start.saturating_add(n).min(len))
                        .unwrap_or(len);
                    (start..end)
                        .map(|i| match rt.object_get(*view_id, &i.to_string()) {
                            Value::Number(n) => n as u8,
                            _ => 0,
                        })
                        .collect()
                }
            }
            _ => Vec::new(),
        };
        Ok(intrinsic_buffer_from_bytes(rt, &bytes))
    });
    let species_desc = PropertyDescriptor {
        value: Value::Object(fast_buffer),
        writable: false,
        enumerable: false,
        configurable: true,
        getter: None,
        setter: None,
    };
    rt.obj_mut(buffer_ctor).properties.insert(
        PropertyKey::String("@@species".into()),
        species_desc.clone(),
    );
    if let Value::Object(sym_ctor) = rt.global_get("Symbol") {
        if let Value::Symbol(sym) = rt.object_get(sym_ctor, "species") {
            rt.obj_mut(buffer_ctor)
                .properties
                .insert(PropertyKey::Symbol(sym), species_desc);
        }
    }
}

pub(crate) fn install_buffer_methods(rt: &mut Runtime, id: rusty_js_runtime::ObjectRef) {
    register_method(rt, id, "slice", |rt, args| {
        let this_id = match rt.current_this() {
            Value::Object(o) => o,
            _ => {
                return Err(RuntimeError::TypeError(
                    "Buffer.slice: this must be a Buffer".into(),
                ))
            }
        };
        let len = match rt.object_get(this_id, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        let start = args
            .first()
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as i64)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let end = args
            .get(1)
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as i64)
                } else {
                    None
                }
            })
            .unwrap_or(len as i64);
        let start = (if start < 0 {
            (len as i64 + start).max(0)
        } else {
            start
        })
        .min(len as i64) as usize;
        let end = (if end < 0 {
            (len as i64 + end).max(0)
        } else {
            end
        })
        .min(len as i64) as usize;
        let slice_len = end.saturating_sub(start);
        let mut o = RtObject::new_ordinary();
        o.set_own("length".into(), Value::Number(slice_len as f64));
        o.set_own_internal("__is_buffer__".into(), Value::Boolean(true));
        o.set_own_internal("__buffer_parent__".into(), Value::Object(this_id));
        o.set_own_internal("__buffer_offset__".into(), Value::Number(start as f64));
        o.set_own_internal(
            "@@toStringTag".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                "Uint8Array",
            ))),
        );
        let new_id = rt.alloc_object(o);
        install_buffer_methods(rt, new_id);
        Ok(Value::Object(new_id))
    });
    register_method(rt, id, "toString", |rt, args| {
        let this_id = match rt.current_this() {
            Value::Object(o) => o,
            _ => {
                return Ok(Value::String(Rc::new(
                    rusty_js_runtime::value::JsString::from(String::new()),
                )))
            }
        };
        let enc = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => "utf8".into(),
        };
        let len = match rt.object_get(this_id, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        let mut bytes: Vec<u8> = Vec::with_capacity(len);
        for i in 0..len {
            if let Value::Number(n) = rt.object_get(this_id, &i.to_string()) {
                bytes.push(n as u8);
            }
        }
        let out = match enc.as_str() {
            "hex" => bytes
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>(),
            "base64" => base64_encode(&bytes),
            "base64url" => base64_encode(&bytes)
                .trim_end_matches('=')
                .replace('+', "-")
                .replace('/', "_"),
            "latin1" | "binary" => bytes.iter().map(|b| *b as char).collect::<String>(),
            "ascii" => bytes.iter().map(|b| (b & 0x7f) as char).collect::<String>(),
            _ => String::from_utf8_lossy(&bytes).to_string(),
        };
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(out),
        )))
    });
    register_method(rt, id, "latin1Slice", |rt, args| {
        let this_id = match rt.current_this() {
            Value::Object(o) => o,
            _ => {
                return Err(RuntimeError::TypeError(
                    "Buffer.latin1Slice: this must be a Buffer".into(),
                ))
            }
        };
        let len = match rt.object_get(this_id, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        let start = args
            .first()
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as i64)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let end = args
            .get(1)
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as i64)
                } else {
                    None
                }
            })
            .unwrap_or(len as i64);
        let start = (if start < 0 {
            (len as i64 + start).max(0)
        } else {
            start
        })
        .min(len as i64) as usize;
        let end = (if end < 0 {
            (len as i64 + end).max(0)
        } else {
            end
        })
        .min(len as i64) as usize;
        let mut out = String::new();
        for i in start..end {
            match rt.object_get(this_id, &i.to_string()) {
                Value::Number(n) => out.push((n as u8) as char),
                _ => out.push('\0'),
            }
        }
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(out),
        )))
    });
    register_method(rt, id, "copy", |rt, args| {
        let this_id = match rt.current_this() {
            Value::Object(o) => o,
            _ => return Ok(Value::Number(0.0)),
        };
        let target = match args.first() {
            Some(Value::Object(id)) => *id,
            _ => return Ok(Value::Number(0.0)),
        };
        let target_start = args
            .get(1)
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as usize)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let src_start = args
            .get(2)
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as usize)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let src_len = match rt.object_get(this_id, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        let src_end = args
            .get(3)
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as usize)
                } else {
                    None
                }
            })
            .unwrap_or(src_len)
            .min(src_len);
        let count = src_end.saturating_sub(src_start);
        for i in 0..count {
            let v = rt.object_get(this_id, &(src_start + i).to_string());
            rt.object_set(target, (target_start + i).to_string(), v);
        }
        Ok(Value::Number(count as f64))
    });
    register_method(rt, id, "write", |rt, args| {
        let this_id = match rt.current_this() {
            Value::Object(o) => o,
            _ => return Ok(Value::Number(0.0)),
        };
        let text = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => return Ok(Value::Number(0.0)),
        };
        let encoding = if let Some(Value::String(e)) = args.get(2) {
            e.as_str().to_string()
        } else if let Some(Value::String(e)) = args.get(1) {
            if matches!(args.get(2), None) {
                e.as_str().to_string()
            } else {
                "utf8".into()
            }
        } else {
            "utf8".into()
        };
        let offset = args
            .get(1)
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as usize)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let start = args
            .get(2)
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as usize)
                } else {
                    None
                }
            })
            .unwrap_or(usize::MAX);
        let bytes = encode_buffer_write_value(&text, &encoding.to_ascii_lowercase());
        let len = match rt.object_get(this_id, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        let mut written = 0usize;
        if offset < len {
            for (i, b) in bytes
                .iter()
                .take(start.min(bytes.len()).min(len.saturating_sub(offset)))
                .enumerate()
            {
                rt.object_set(this_id, (offset + i).to_string(), Value::Number(*b as f64));
                written += 1;
            }
        }
        Ok(Value::Number(written as f64))
    });
    register_method(rt, id, "writeInt32BE", |rt, args| {
        let this_id = match rt.current_this() {
            Value::Object(o) => o,
            _ => return Ok(Value::Number(0.0)),
        };
        let value = args
            .first()
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as i32)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let offset = args
            .get(1)
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as usize)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let len = match rt.object_get(this_id, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        if offset.saturating_add(4) <= len {
            let v = (value as u32).to_be_bytes();
            for i in 0..4 {
                rt.object_set(
                    this_id,
                    (offset + i).to_string(),
                    Value::Number(v[i] as f64),
                );
            }
            Ok(Value::Number(4.0))
        } else {
            Ok(Value::Number(0.0))
        }
    });
    register_method(rt, id, "writeUInt32BE", |rt, args| {
        let this_id = match rt.current_this() {
            Value::Object(o) => o,
            _ => return Ok(Value::Number(0.0)),
        };
        let value = args
            .first()
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as u32)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let offset = args
            .get(1)
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as usize)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let len = match rt.object_get(this_id, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        if offset.saturating_add(4) <= len {
            for i in 0..4 {
                rt.object_set(
                    this_id,
                    (offset + i).to_string(),
                    Value::Number((value.to_be_bytes()[i]) as f64),
                );
            }
            Ok(Value::Number(4.0))
        } else {
            Ok(Value::Number(0.0))
        }
    });
    register_method(rt, id, "writeUInt32LE", |rt, args| {
        let this_id = match rt.current_this() {
            Value::Object(o) => o,
            _ => return Ok(Value::Number(0.0)),
        };
        let value = args
            .first()
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as u32)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let offset = args
            .get(1)
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as usize)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let len = match rt.object_get(this_id, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        if offset.saturating_add(4) <= len {
            for i in 0..4 {
                rt.object_set(
                    this_id,
                    (offset + i).to_string(),
                    Value::Number((value.to_le_bytes()[i]) as f64),
                );
            }
            Ok(Value::Number(4.0))
        } else {
            Ok(Value::Number(0.0))
        }
    });
    register_method(rt, id, "writeUInt16BE", |rt, args| {
        let this_id = match rt.current_this() {
            Value::Object(o) => o,
            _ => return Ok(Value::Number(0.0)),
        };
        let value = args
            .first()
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as u16)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let offset = args
            .get(1)
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as usize)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let len = match rt.object_get(this_id, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        if offset.saturating_add(2) <= len {
            let b = value.to_be_bytes();
            rt.object_set(this_id, (offset).to_string(), Value::Number(b[0] as f64));
            rt.object_set(
                this_id,
                (offset + 1).to_string(),
                Value::Number(b[1] as f64),
            );
            Ok(Value::Number(2.0))
        } else {
            Ok(Value::Number(0.0))
        }
    });
    register_method(rt, id, "writeUInt16LE", |rt, args| {
        let this_id = match rt.current_this() {
            Value::Object(o) => o,
            _ => return Ok(Value::Number(0.0)),
        };
        let value = args
            .first()
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as u16)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let offset = args
            .get(1)
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as usize)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let len = match rt.object_get(this_id, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        if offset.saturating_add(2) <= len {
            let b = value.to_le_bytes();
            rt.object_set(this_id, (offset).to_string(), Value::Number(b[0] as f64));
            rt.object_set(
                this_id,
                (offset + 1).to_string(),
                Value::Number(b[1] as f64),
            );
            Ok(Value::Number(2.0))
        } else {
            Ok(Value::Number(0.0))
        }
    });
    let write_uint8 =
        make_callable_with_length_rooted(rt, "writeUInt8", 1, Vec::new(), |rt, args| {
            let this_id = match rt.current_this() {
                Value::Object(o) => o,
                _ => return Ok(Value::Number(0.0)),
            };
            let value = args
                .first()
                .and_then(|v| {
                    if let Value::Number(n) = v {
                        Some(*n as u8)
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            let offset = args
                .get(1)
                .and_then(|v| {
                    if let Value::Number(n) = v {
                        Some(*n as usize)
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            let len = match rt.object_get(this_id, "length") {
                Value::Number(n) => n as usize,
                _ => 0,
            };
            if offset < len {
                rt.object_set(this_id, offset.to_string(), Value::Number(value as f64));
                Ok(Value::Number((offset + 1) as f64))
            } else {
                Ok(Value::Number(0.0))
            }
        });
    rt.object_set(id, "writeUInt8".into(), Value::Object(write_uint8));
    register_method(rt, id, "subarray", |rt, args| {
        let this_id = match rt.current_this() {
            Value::Object(o) => o,
            _ => {
                return Err(RuntimeError::TypeError(
                    "Buffer.subarray: this must be a Buffer".into(),
                ))
            }
        };
        let len = match rt.object_get(this_id, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        let start = args
            .first()
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as i64)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let end = args
            .get(1)
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as i64)
                } else {
                    None
                }
            })
            .unwrap_or(len as i64);
        let start = (if start < 0 {
            (len as i64 + start).max(0)
        } else {
            start
        })
        .min(len as i64) as usize;
        let end = (if end < 0 {
            (len as i64 + end).max(0)
        } else {
            end
        })
        .min(len as i64) as usize;
        let slice_len = end.saturating_sub(start);
        let mut o = RtObject::new_ordinary();
        o.set_own("length".into(), Value::Number(slice_len as f64));
        o.set_own_internal("__is_buffer__".into(), Value::Boolean(true));
        o.set_own_internal("__buffer_parent__".into(), Value::Object(this_id));
        o.set_own_internal("__buffer_offset__".into(), Value::Number(start as f64));
        o.set_own_internal(
            "@@toStringTag".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                "Uint8Array",
            ))),
        );
        let new_id = rt.alloc_object(o);
        install_buffer_methods(rt, new_id);
        Ok(Value::Object(new_id))
    });
    register_method(rt, id, "readUInt8", |rt, args| {
        let this_id = match rt.current_this() {
            Value::Object(o) => o,
            _ => return Ok(Value::Number(0.0)),
        };
        let offset = args
            .first()
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as usize)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        match rt.object_get(this_id, &offset.to_string()) {
            Value::Number(n) => Ok(Value::Number(n)),
            _ => Ok(Value::Number(0.0)),
        }
    });

    register_method(rt, id, "every", |rt, args| {
        let this_id = match rt.current_this() {
            Value::Object(o) => o,
            _ => return Ok(Value::Boolean(true)),
        };
        let cb = args.first().cloned().unwrap_or(Value::Undefined);
        if !rt.is_callable(&cb) {
            return Err(RuntimeError::TypeError(
                "Buffer.every: callback is not a function".into(),
            ));
        }
        let len = match rt.object_get(this_id, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        for i in 0..len {
            let v = rt.object_get(this_id, &i.to_string());
            let r = rt.call_function(
                cb.clone(),
                Value::Undefined,
                vec![v, Value::Number(i as f64), Value::Object(this_id)],
            )?;
            if !rusty_js_runtime::abstract_ops::to_boolean(&r) {
                return Ok(Value::Boolean(false));
            }
        }
        Ok(Value::Boolean(true))
    });

    fn buf_len(rt: &mut Runtime, id: rusty_js_runtime::ObjectRef) -> usize {
        match rt.object_get(id, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        }
    }
    fn buf_read_bytes(
        rt: &mut Runtime,
        id: rusty_js_runtime::ObjectRef,
        offset: usize,
        n: usize,
    ) -> Option<Vec<u8>> {
        let len = buf_len(rt, id);
        if offset.checked_add(n).map(|e| e > len).unwrap_or(true) {
            return None;
        }
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            match rt.object_get(id, &(offset + i).to_string()) {
                Value::Number(v) => out.push(v as u8),
                _ => out.push(0),
            }
        }
        Some(out)
    }
    fn buf_write_bytes(
        rt: &mut Runtime,
        id: rusty_js_runtime::ObjectRef,
        offset: usize,
        bytes: &[u8],
    ) -> bool {
        let len = buf_len(rt, id);
        if offset
            .checked_add(bytes.len())
            .map(|e| e > len)
            .unwrap_or(true)
        {
            return false;
        }
        for (i, b) in bytes.iter().enumerate() {
            rt.object_set(id, (offset + i).to_string(), Value::Number(*b as f64));
        }
        true
    }
    fn buf_oor(method: &str) -> RuntimeError {
        RuntimeError::RangeError(format!(
            "Buffer.{method}: offset is outside the bounds of the Buffer (ERR_OUT_OF_RANGE)"
        ))
    }
    fn buf_offset_arg(args: &[Value], idx: usize) -> Result<usize, RuntimeError> {
        match args.get(idx) {
            Some(Value::Number(n)) => {
                if *n < 0.0 || !n.is_finite() {
                    return Err(RuntimeError::RangeError(format!(
                        "Buffer: offset must be a non-negative integer (ERR_OUT_OF_RANGE), got {n}"
                    )));
                }
                Ok(*n as usize)
            }
            Some(_) | None => Ok(0),
        }
    }
    fn buf_this(
        rt: &mut Runtime,
        method: &str,
    ) -> Result<rusty_js_runtime::ObjectRef, RuntimeError> {
        match rt.current_this() {
            Value::Object(o) => Ok(o),
            _ => Err(RuntimeError::TypeError(format!(
                "Buffer.{method}: this must be a Buffer"
            ))),
        }
    }

    macro_rules! reg_read {
        ($name:literal, $n:expr, $conv:expr) => {
            register_method(rt, id, $name, |rt, args| {
                let this_id = buf_this(rt, $name)?;
                let offset = buf_offset_arg(args, 0)?;
                let bytes =
                    buf_read_bytes(rt, this_id, offset, $n).ok_or_else(|| buf_oor($name))?;
                Ok($conv(&bytes))
            });
        };
    }
    macro_rules! reg_write {
        ($name:literal, $n:expr, $encode:expr) => {
            register_method(rt, id, $name, |rt, args| {
                let this_id = buf_this(rt, $name)?;
                let raw = match args.first() {
                    Some(v) => v.clone(),
                    None => Value::Number(0.0),
                };
                let offset = buf_offset_arg(args, 1)?;
                let bytes: [u8; $n] = $encode(&raw);
                if !buf_write_bytes(rt, this_id, offset, &bytes) {
                    return Err(buf_oor($name));
                }
                Ok(Value::Number((offset + $n) as f64))
            });
        };
    }

    fn as_f64(v: &Value) -> f64 {
        match v {
            Value::Number(n) => *n,
            Value::Boolean(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            _ => 0.0,
        }
    }
    fn as_u64(v: &Value) -> u64 {
        match v {
            Value::BigInt(n) => n
                .to_decimal()
                .parse::<i128>()
                .ok()
                .map(|x| x as u64)
                .unwrap_or(0),
            Value::Number(n) => *n as u64,
            _ => 0,
        }
    }
    fn as_i64(v: &Value) -> i64 {
        match v {
            Value::BigInt(n) => n.to_decimal().parse::<i64>().unwrap_or(0),
            Value::Number(n) => *n as i64,
            _ => 0,
        }
    }
    fn mk_bigint_u64(v: u64) -> Value {
        Value::BigInt(std::rc::Rc::new(
            rusty_js_runtime::bigint::JsBigInt::from_u64(v),
        ))
    }
    fn mk_bigint_i64(v: i64) -> Value {
        Value::BigInt(std::rc::Rc::new(
            rusty_js_runtime::bigint::JsBigInt::from_i64(v),
        ))
    }
    fn uint_le(bytes: &[u8]) -> u64 {
        bytes
            .iter()
            .enumerate()
            .fold(0u64, |acc, (i, b)| acc | ((*b as u64) << (8 * i)))
    }
    fn uint_be(bytes: &[u8]) -> u64 {
        bytes.iter().fold(0u64, |acc, b| (acc << 8) | (*b as u64))
    }
    fn sign_extend(v: u64, byte_len: usize) -> i64 {
        let bits = byte_len * 8;
        let shift = 64usize.saturating_sub(bits);
        ((v << shift) as i64) >> shift
    }
    fn var_len(args: &[Value]) -> Result<usize, RuntimeError> {
        let n = match args.get(2) {
            Some(Value::Number(n)) => *n as usize,
            _ => 0,
        };
        if (1..=6).contains(&n) {
            Ok(n)
        } else {
            Err(RuntimeError::RangeError(
                "Buffer: byteLength must be between 1 and 6 (ERR_OUT_OF_RANGE)".into(),
            ))
        }
    }
    fn read_var(
        rt: &mut Runtime,
        args: &[Value],
        method: &str,
        signed: bool,
        le: bool,
    ) -> Result<Value, RuntimeError> {
        let this_id = buf_this(rt, method)?;
        let offset = buf_offset_arg(args, 0)?;
        let byte_len = match args.get(1) {
            Some(Value::Number(n)) => *n as usize,
            _ => 0,
        };
        if !(1..=6).contains(&byte_len) {
            return Err(RuntimeError::RangeError(
                "Buffer: byteLength must be between 1 and 6 (ERR_OUT_OF_RANGE)".into(),
            ));
        }
        let bytes = buf_read_bytes(rt, this_id, offset, byte_len).ok_or_else(|| buf_oor(method))?;
        let raw = if le { uint_le(&bytes) } else { uint_be(&bytes) };
        let n = if signed {
            sign_extend(raw, byte_len) as f64
        } else {
            raw as f64
        };
        Ok(Value::Number(n))
    }
    fn write_var(
        rt: &mut Runtime,
        args: &[Value],
        method: &str,
        signed: bool,
        le: bool,
    ) -> Result<Value, RuntimeError> {
        let this_id = buf_this(rt, method)?;
        let value = as_f64(&args.first().cloned().unwrap_or(Value::Number(0.0))) as i128;
        let offset = buf_offset_arg(args, 1)?;
        let byte_len = var_len(args)?;
        let bits = byte_len * 8;
        let min = if signed { -(1i128 << (bits - 1)) } else { 0 };
        let max = if signed {
            (1i128 << (bits - 1)) - 1
        } else {
            (1i128 << bits) - 1
        };
        if value < min || value > max {
            return Err(RuntimeError::RangeError(
                "Buffer: value is out of range (ERR_OUT_OF_RANGE)".into(),
            ));
        }
        let mut raw = if value < 0 {
            ((1i128 << bits) + value) as u64
        } else {
            value as u64
        };
        let mut bytes = vec![0u8; byte_len];
        if le {
            for b in &mut bytes {
                *b = (raw & 0xff) as u8;
                raw >>= 8;
            }
        } else {
            for b in bytes.iter_mut().rev() {
                *b = (raw & 0xff) as u8;
                raw >>= 8;
            }
        }
        if !buf_write_bytes(rt, this_id, offset, &bytes) {
            return Err(buf_oor(method));
        }
        Ok(Value::Number((offset + byte_len) as f64))
    }

    register_method(rt, id, "readUIntLE", |rt, args| {
        read_var(rt, args, "readUIntLE", false, true)
    });
    register_method(rt, id, "readUIntBE", |rt, args| {
        read_var(rt, args, "readUIntBE", false, false)
    });
    reg_read!("readUInt16LE", 2, |b: &[u8]| Value::Number(
        u16::from_le_bytes([b[0], b[1]]) as f64
    ));
    reg_read!("readUInt16BE", 2, |b: &[u8]| Value::Number(
        u16::from_be_bytes([b[0], b[1]]) as f64
    ));
    reg_read!("readUInt32LE", 4, |b: &[u8]| Value::Number(
        u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64
    ));
    reg_read!("readUInt32BE", 4, |b: &[u8]| Value::Number(
        u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as f64
    ));

    register_method(rt, id, "readIntLE", |rt, args| {
        read_var(rt, args, "readIntLE", true, true)
    });
    register_method(rt, id, "readIntBE", |rt, args| {
        read_var(rt, args, "readIntBE", true, false)
    });
    reg_read!("readInt8", 1, |b: &[u8]| Value::Number(
        i8::from_le_bytes([b[0]]) as f64
    ));
    reg_read!("readInt16LE", 2, |b: &[u8]| Value::Number(
        i16::from_le_bytes([b[0], b[1]]) as f64
    ));
    reg_read!("readInt16BE", 2, |b: &[u8]| Value::Number(
        i16::from_be_bytes([b[0], b[1]]) as f64
    ));
    reg_read!("readInt32LE", 4, |b: &[u8]| Value::Number(
        i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64
    ));
    reg_read!("readInt32BE", 4, |b: &[u8]| Value::Number(
        i32::from_be_bytes([b[0], b[1], b[2], b[3]]) as f64
    ));

    reg_read!("readFloatLE", 4, |b: &[u8]| Value::Number(
        f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64
    ));
    reg_read!("readFloatBE", 4, |b: &[u8]| Value::Number(
        f32::from_be_bytes([b[0], b[1], b[2], b[3]]) as f64
    ));
    reg_read!("readDoubleLE", 8, |b: &[u8]| Value::Number(
        f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
    ));
    reg_read!("readDoubleBE", 8, |b: &[u8]| Value::Number(
        f64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
    ));

    reg_read!("readBigUInt64LE", 8, |b: &[u8]| mk_bigint_u64(
        u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
    ));
    reg_read!("readBigUInt64BE", 8, |b: &[u8]| mk_bigint_u64(
        u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
    ));
    reg_read!("readBigInt64LE", 8, |b: &[u8]| mk_bigint_i64(
        i64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
    ));
    reg_read!("readBigInt64BE", 8, |b: &[u8]| mk_bigint_i64(
        i64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
    ));

    register_method(rt, id, "writeUIntLE", |rt, args| {
        write_var(rt, args, "writeUIntLE", false, true)
    });
    register_method(rt, id, "writeUIntBE", |rt, args| {
        write_var(rt, args, "writeUIntBE", false, false)
    });
    register_method(rt, id, "writeIntLE", |rt, args| {
        write_var(rt, args, "writeIntLE", true, true)
    });
    register_method(rt, id, "writeIntBE", |rt, args| {
        write_var(rt, args, "writeIntBE", true, false)
    });
    {
        let write_int8 =
            make_callable_with_length_rooted(rt, "writeInt8", 1, Vec::new(), |rt, args| {
                let this_id = buf_this(rt, "writeInt8")?;
                let raw = match args.first() {
                    Some(v) => v.clone(),
                    None => Value::Number(0.0),
                };
                let offset = buf_offset_arg(args, 1)?;
                let bytes: [u8; 1] = (as_f64(&raw) as i8).to_le_bytes();
                if !buf_write_bytes(rt, this_id, offset, &bytes) {
                    return Err(buf_oor("writeInt8"));
                }
                Ok(Value::Number((offset + 1) as f64))
            });
        rt.object_set(id, "writeInt8".into(), Value::Object(write_int8));
    }
    reg_write!("writeInt16LE", 2, |v: &Value| (as_f64(v) as i16)
        .to_le_bytes());
    reg_write!("writeInt16BE", 2, |v: &Value| (as_f64(v) as i16)
        .to_be_bytes());
    reg_write!("writeInt32LE", 4, |v: &Value| (as_f64(v) as i32)
        .to_le_bytes());
    reg_write!("writeFloatLE", 4, |v: &Value| (as_f64(v) as f32)
        .to_le_bytes());
    reg_write!("writeFloatBE", 4, |v: &Value| (as_f64(v) as f32)
        .to_be_bytes());
    reg_write!("writeDoubleLE", 8, |v: &Value| as_f64(v).to_le_bytes());
    reg_write!("writeDoubleBE", 8, |v: &Value| as_f64(v).to_be_bytes());
    reg_write!("writeBigUInt64LE", 8, |v: &Value| as_u64(v).to_le_bytes());
    reg_write!("writeBigUInt64BE", 8, |v: &Value| as_u64(v).to_be_bytes());
    reg_write!("writeBigInt64LE", 8, |v: &Value| as_i64(v).to_le_bytes());
    reg_write!("writeBigInt64BE", 8, |v: &Value| as_i64(v).to_be_bytes());

    register_method(rt, id, "indexOf", |rt, args| {
        let this_id = match rt.current_this() {
            Value::Object(o) => o,
            _ => return Ok(Value::Number(-1.0)),
        };
        let len = match rt.object_get(this_id, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        let needle_bytes: Vec<u8> = match args.first() {
            Some(Value::Number(n)) => vec![*n as u8],
            Some(Value::String(s)) => s.as_bytes().to_vec(),
            Some(Value::Object(nid)) => {
                let nl = match rt.object_get(*nid, "length") {
                    Value::Number(n) => n as usize,
                    _ => 0,
                };
                (0..nl)
                    .filter_map(|i| match rt.object_get(*nid, &i.to_string()) {
                        Value::Number(n) => Some(n as u8),
                        _ => None,
                    })
                    .collect()
            }
            _ => return Ok(Value::Number(-1.0)),
        };
        for start in 0..=len.saturating_sub(needle_bytes.len()) {
            let mut all = true;
            for (j, b) in needle_bytes.iter().enumerate() {
                if let Value::Number(n) = rt.object_get(this_id, &(start + j).to_string()) {
                    if n as u8 != *b {
                        all = false;
                        break;
                    }
                } else {
                    all = false;
                    break;
                }
            }
            if all {
                return Ok(Value::Number(start as f64));
            }
        }
        Ok(Value::Number(-1.0))
    });
    register_method(rt, id, "equals", |rt, args| {
        let this_id = match rt.current_this() {
            Value::Object(o) => o,
            _ => return Ok(Value::Boolean(false)),
        };
        let other = match args.first() {
            Some(Value::Object(o)) => *o,
            _ => return Ok(Value::Boolean(false)),
        };
        let l1 = match rt.object_get(this_id, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        let l2 = match rt.object_get(other, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        if l1 != l2 {
            return Ok(Value::Boolean(false));
        }
        for i in 0..l1 {
            if rt.object_get(this_id, &i.to_string()) != rt.object_get(other, &i.to_string()) {
                return Ok(Value::Boolean(false));
            }
        }
        Ok(Value::Boolean(true))
    });

    register_method(rt, id, "values", |rt, _args| {
        let this_id = match rt.current_this() {
            Value::Object(o) => o,
            _ => return Ok(Value::Undefined),
        };
        let len = match rt.object_get(this_id, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        let arr = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
        for i in 0..len {
            let v = rt.object_get(this_id, &i.to_string());
            rt.object_set(arr, i.to_string(), v);
        }
        rt.object_set(arr, "length".into(), Value::Number(len as f64));
        Ok(Value::Object(arr))
    });
    register_method(rt, id, "keys", |rt, _args| {
        let this_id = match rt.current_this() {
            Value::Object(o) => o,
            _ => return Ok(Value::Undefined),
        };
        let len = match rt.object_get(this_id, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        let arr = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
        for i in 0..len {
            rt.object_set(arr, i.to_string(), Value::Number(i as f64));
        }
        rt.object_set(arr, "length".into(), Value::Number(len as f64));
        Ok(Value::Object(arr))
    });
    register_method(rt, id, "entries", |rt, _args| {
        let this_id = match rt.current_this() {
            Value::Object(o) => o,
            _ => return Ok(Value::Undefined),
        };
        let len = match rt.object_get(this_id, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        let arr = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
        for i in 0..len {
            let v = rt.object_get(this_id, &i.to_string());
            let pair = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
            rt.object_set(pair, "0".into(), Value::Number(i as f64));
            rt.object_set(pair, "1".into(), v);
            rt.object_set(pair, "length".into(), Value::Number(2.0));
            rt.object_set(arr, i.to_string(), Value::Object(pair));
        }
        rt.object_set(arr, "length".into(), Value::Number(len as f64));
        Ok(Value::Object(arr))
    });

    register_method(rt, id, "@@iterator", |rt, _args| {
        let this_id = match rt.current_this() {
            Value::Object(o) => o,
            _ => return Ok(Value::Undefined),
        };
        let len = match rt.object_get(this_id, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        let iter = RtObject::new_ordinary();
        let iter_id = rt.alloc_object(iter);

        rt.obj_mut(iter_id)
            .set_own_internal("__i".into(), Value::Number(0.0));
        rt.obj_mut(iter_id)
            .set_own_internal("__src".into(), Value::Object(this_id));
        rt.obj_mut(iter_id)
            .set_own_internal("__len".into(), Value::Number(len as f64));
        register_method(rt, iter_id, "next", |rt, _args| {
            let it = match rt.current_this() {
                Value::Object(o) => o,
                _ => return Ok(Value::Undefined),
            };
            let i = match rt.object_get(it, "__i") {
                Value::Number(n) => n as usize,
                _ => 0,
            };
            let len = match rt.object_get(it, "__len") {
                Value::Number(n) => n as usize,
                _ => 0,
            };
            let src_id = match rt.object_get(it, "__src") {
                Value::Object(o) => o,
                _ => return Ok(Value::Undefined),
            };
            let result = RtObject::new_ordinary();
            let result_id = rt.alloc_object(result);
            if i >= len {
                rt.object_set(result_id, "value".into(), Value::Undefined);
                rt.object_set(result_id, "done".into(), Value::Boolean(true));
            } else {
                let v = rt.object_get(src_id, &i.to_string());
                rt.object_set(result_id, "value".into(), v);
                rt.object_set(result_id, "done".into(), Value::Boolean(false));
                rt.obj_mut(it)
                    .set_own_internal("__i".into(), Value::Number((i + 1) as f64));
            }
            Ok(Value::Object(result_id))
        });
        Ok(Value::Object(iter_id))
    });
}

pub fn install_buffer(rt: &mut Runtime) {
    const NODE_BUFFER_KMAX_LENGTH: f64 = 9_007_199_254_740_991.0;

    fn buffer_size_arg(rt: &mut Runtime, args: &[Value]) -> Result<usize, RuntimeError> {
        let n = match args.first() {
            None | Some(Value::Undefined) => {
                return Err(node_code_type_error(
                    rt,
                    "ERR_INVALID_ARG_TYPE",
                    "The \"size\" argument must be of type number. Received undefined",
                ))
            }
            Some(Value::Number(n)) => *n,
            Some(_) => {
                return Err(node_code_type_error(
                    rt,
                    "ERR_INVALID_ARG_TYPE",
                    "The \"size\" argument must be of type number.",
                ))
            }
        };
        if n.is_nan() || n < 0.0 || !n.is_finite() || n > NODE_BUFFER_KMAX_LENGTH {
            return Err(node_code_range_error(
                rt,
                "ERR_OUT_OF_RANGE",
                "The value of \"size\" is out of range. It must be >= 0 && <= 9007199254740991.",
            ));
        }
        Ok(n as usize)
    }

    let ns = new_object(rt);

    let buf_ctor = make_callable(rt, "Buffer", |rt, args| {
        match args.first() {
            Some(Value::Number(_)) => {
                let n = buffer_size_arg(rt, args)?;
                Ok(intrinsic_buffer_from_bytes(rt, &vec![0u8; n]))
            }
            Some(Value::String(s)) => {
                let s = s.as_str().to_string();
                Ok(intrinsic_buffer_from_bytes(rt, s.as_bytes()))
            }
            None | Some(Value::Undefined) => Err(node_code_type_error(
                rt,
                "ERR_INVALID_ARG_TYPE",
                "The first argument must be of type string or an instance of Buffer, ArrayBuffer, or Array or an Array-like Object. Received undefined",
            )),
            _ => {

                Ok(intrinsic_buffer_from_bytes(rt, &[]))
            }
        }
    });
    register_method(rt, buf_ctor, "from", move |rt, args| {

        let mut bytes: Vec<u8> = Vec::new();
        let mut source_string: Option<String> = None;
        match args.first() {
            Some(Value::String(s)) => {
                let st = s.as_str().to_string();
                let enc = match args.get(1) {
                    Some(Value::String(s)) => s.as_str().to_string(),
                    _ => "utf8".into(),
                };
                bytes = match enc.as_str() {
                    "hex" => {
                        let mut v = Vec::with_capacity(st.len() / 2);
                        let chars: Vec<char> = st.chars().collect();
                        let mut i = 0;
                        while i + 1 < chars.len() {
                            let hi = chars[i].to_digit(16);
                            let lo = chars[i + 1].to_digit(16);
                            match (hi, lo) {
                                (Some(h), Some(l)) => v.push(((h << 4) | l) as u8),
                                _ => break,
                            }
                            i += 2;
                        }
                        v
                    }
                    "base64" | "base64url" => {
                        let mut normalized = st.replace('-', "+").replace('_', "/");
                        while normalized.len() % 4 != 0 {
                            normalized.push('=');
                        }
                        base64_decode(&normalized)
                    }
                    "latin1" | "binary" => st.chars().map(|c| c as u8).collect(),
                    "ascii" => st.chars().map(|c| (c as u32 & 0x7f) as u8).collect(),
                    _ => st.as_bytes().to_vec(),
                };
                source_string = Some(st);
            }
            Some(Value::Object(src_id)) => {
                let src = *src_id;
                let len = match rt.object_get(src, "length") {
                    Value::Number(n) => n as usize,
                    _ => 0,
                };
                bytes.reserve(len);
                for i in 0..len {
                    match rt.object_get(src, &i.to_string()) {
                        Value::Number(n) => bytes.push(n as u8),
                        _ => bytes.push(0),
                    }
                }
            }
            _ => {}
        }
        if source_string.is_some() {
            Ok(intrinsic_buffer_from_pooled_bytes(rt, buf_ctor, &bytes))
        } else {
            Ok(intrinsic_buffer_from_bytes(rt, &bytes))
        }
    });
    register_method(rt, buf_ctor, "alloc", |rt, args| {
        let n = buffer_size_arg(rt, args)?;

        let fill: Vec<u8> = match args.get(1) {
            Some(Value::Number(b)) => vec![*b as i64 as u8],
            Some(Value::String(s)) => s.as_bytes().to_vec(),
            _ => vec![0u8],
        };
        let fill = if fill.is_empty() { vec![0u8] } else { fill };
        let bytes: Vec<u8> = (0..n).map(|i| fill[i % fill.len()]).collect();
        Ok(intrinsic_buffer_from_bytes(rt, &bytes))
    });

    register_method(rt, buf_ctor, "allocUnsafeSlow", |rt, args| {
        let n = buffer_size_arg(rt, args)?;
        Ok(intrinsic_buffer_from_bytes(rt, &vec![0u8; n]))
    });

    register_method(rt, buf_ctor, "allocUnsafe", |rt, args| {
        let n = buffer_size_arg(rt, args)?;

        Ok(intrinsic_buffer_from_bytes(rt, &vec![0u8; n]))
    });

    register_method(rt, buf_ctor, "compare", |rt, args| {
        let read = |v: &Value| -> Vec<u8> {
            match v {
                Value::String(s) => s.as_bytes().to_vec(),
                Value::Object(id) => {
                    let len = match rt.object_get(*id, "length") {
                        Value::Number(n) => n as usize,
                        _ => 0,
                    };
                    (0..len)
                        .map(|i| match rt.object_get(*id, &i.to_string()) {
                            Value::Number(n) => n as u8,
                            Value::String(s) if !s.is_empty() => s.as_bytes()[0],
                            _ => 0,
                        })
                        .collect()
                }
                _ => Vec::new(),
            }
        };
        let a = read(&args.first().cloned().unwrap_or(Value::Undefined));
        let b = read(&args.get(1).cloned().unwrap_or(Value::Undefined));
        Ok(Value::Number(match a.cmp(&b) {
            std::cmp::Ordering::Less => -1.0,
            std::cmp::Ordering::Equal => 0.0,
            std::cmp::Ordering::Greater => 1.0,
        }))
    });
    register_method(rt, buf_ctor, "concat", |rt, args| {
        let list = match args.first() {
            Some(Value::Object(id)) => *id,
            _ => {
                return Err(RuntimeError::TypeError(
                    "Buffer.concat: expected array".into(),
                ))
            }
        };
        let len = rt.array_length(list);
        let mut bytes: Vec<u8> = Vec::new();
        for i in 0..len {
            if let Value::Object(b) = rt.object_get(list, &i.to_string()) {
                let bl = match rt.object_get(b, "length") {
                    Value::Number(n) => n as usize,
                    _ => 0,
                };
                for j in 0..bl {
                    if let Value::Number(n) = rt.object_get(b, &j.to_string()) {
                        bytes.push(n as u8);
                    }
                }
            }
        }

        if let Some(Value::Number(total)) = args.get(1) {
            bytes.resize((*total).max(0.0) as usize, 0u8);
        }
        let mut o = RtObject::new_ordinary();
        o.set_own("length".into(), Value::Number(bytes.len() as f64));
        o.set_own_internal("__is_buffer__".into(), Value::Boolean(true));
        let id = rt.alloc_object(o);
        for (i, b) in bytes.iter().enumerate() {
            rt.object_set(id, i.to_string(), Value::Number(*b as f64));
        }
        install_buffer_methods(rt, id);
        Ok(Value::Object(id))
    });
    register_method(rt, buf_ctor, "isBuffer", |rt, args| {

        if let Some(Value::Object(id)) = args.first() {
            if matches!(rt.object_get(*id, "__is_buffer__"), Value::Boolean(true)) {
                return Ok(Value::Boolean(true));
            }
        }
        Ok(Value::Boolean(false))
    });
    register_method(rt, buf_ctor, "byteLength", |rt, args| {
        let v = args.first().cloned().unwrap_or(Value::Undefined);
        let n = match &v {
            Value::String(s) => s.as_bytes().len(),
            _ => 0,
        };
        Ok(Value::Number(n as f64))
    });

    let buf_proto = new_object(rt);
    rt.set_own_frozen_property(buf_ctor, "prototype".into(), Value::Object(buf_proto));
    install_buffer_species_constructor(rt, buf_ctor);
    rt.object_set(ns, "Buffer".into(), Value::Object(buf_ctor));

    let buf_constants = new_object(rt);
    rt.object_set(
        buf_constants,
        "MAX_LENGTH".into(),
        Value::Number(9_007_199_254_740_991.0),
    );
    rt.object_set(
        buf_constants,
        "MAX_STRING_LENGTH".into(),
        Value::Number(536_870_888.0),
    );

    rt.object_set(ns, "constants".into(), Value::Object(buf_constants));

    match rt.global_get("Blob") {
        Value::Object(id) => {
            rt.object_set(ns, "Blob".into(), Value::Object(id));
        }
        _ => register_method(rt, ns, "Blob", stub("buffer", "Blob")),
    }

    match rt.global_get("File") {
        Value::Object(id) => {
            rt.object_set(ns, "File".into(), Value::Object(id));
        }
        _ => register_method(rt, ns, "File", stub("buffer", "File")),
    }
    register_method(rt, ns, "SlowBuffer", |rt, args| {

        let buf_global = match rt.global_get("Buffer") {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let from = rt.object_get(buf_global, "alloc");
        rt.call_function(from, Value::Undefined, args.to_vec())
    });
    install_buffer_inspect_max_bytes_accessor(rt, ns);
    rt.object_set(
        ns,
        "kMaxLength".into(),
        Value::Number(9_007_199_254_740_991.0),
    );
    rt.object_set(ns, "kStringMaxLength".into(), Value::Number(536_870_888.0));
    register_method(rt, ns, "isAscii", |rt, args| {
        let value = args.first().unwrap_or(&Value::Undefined);
        let bytes = buffer_encoding_probe_bytes(rt, value, "isAscii")?;
        for byte in bytes {
            if byte >= 0x80 {
                return Ok(Value::Boolean(false));
            }
        }
        Ok(Value::Boolean(true))
    });
    register_method(rt, ns, "isUtf8", |rt, args| {
        let value = args.first().unwrap_or(&Value::Undefined);
        let bytes = buffer_encoding_probe_bytes(rt, value, "isUtf8")?;
        Ok(Value::Boolean(std::str::from_utf8(&bytes).is_ok()))
    });

    let atob_v = rt.global_get("atob");
    if !matches!(atob_v, Value::Undefined) {
        rt.object_set(ns, "atob".into(), atob_v);
    }
    let btoa_v = rt.global_get("btoa");
    if !matches!(btoa_v, Value::Undefined) {
        rt.object_set(ns, "btoa".into(), btoa_v);
    }
    register_method(
        rt,
        ns,
        "resolveObjectURL",
        stub("buffer", "resolveObjectURL"),
    );
    register_method(rt, ns, "transcode", |rt, args| {
        let input = args.first().cloned().unwrap_or(Value::Undefined);
        let from_enc = normalize_buffer_encoding(args.get(1));
        let to_enc = normalize_buffer_encoding(args.get(2));
        let bytes = buffer_like_bytes(rt, &input);
        let decoded = decode_transcode_input(&bytes, &from_enc)?;
        let out = encode_transcode_output(&decoded, &to_enc)?;
        Ok(intrinsic_buffer_from_bytes(rt, &out))
    });
    let _ = rt.delete_own_via(
        &Value::Object(ns),
        &Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "SlowBuffer".to_string(),
        ))),
    );
    rt.define_global_property("buffer", Value::Object(ns));

    let intrinsic_buffer = rt.global_get("Buffer");
    if let Value::Object(intrinsic_id) = intrinsic_buffer {
        install_buffer_species_constructor(rt, intrinsic_id);
        rt.object_set(ns, "Buffer".into(), Value::Object(intrinsic_id));
    } else {
        rt.object_set(ns, "Buffer".into(), Value::Object(buf_ctor));
        rt.define_global_property("Buffer", Value::Object(buf_ctor));
    }
}

pub fn install_buffer_canonical(rt: &mut Runtime) {
    let ns = new_object(rt);
    let buf_ctor = rt.global_get("Buffer");
    if !matches!(buf_ctor, Value::Undefined) {
        rt.object_set(ns, "Buffer".into(), buf_ctor);
    }
    rt.object_set(ns, "default".into(), Value::Object(ns));
    rt.define_global_property("__cruft_buffer", Value::Object(ns));
}

const HTTP2_SETTINGS: &[(&str, u16, u32)] = &[
    ("headerTableSize", 0x1, 4096),
    ("enablePush", 0x2, 1),
    ("initialWindowSize", 0x4, 65_535),
    ("maxFrameSize", 0x5, 16_384),
    ("maxConcurrentStreams", 0x3, u32::MAX),
    ("maxHeaderListSize", 0x6, u32::MAX),
];

fn http2_settings_object(rt: &mut Runtime, include_defaults: bool) -> ObjectRef {
    let obj = new_object(rt);
    if include_defaults {
        for (name, _id, default) in HTTP2_SETTINGS {
            rt.object_set(obj, (*name).into(), Value::Number(*default as f64));
        }
    }
    obj
}

fn http2_setting_name(id: u16) -> Option<&'static str> {
    HTTP2_SETTINGS
        .iter()
        .find_map(|(name, setting_id, _)| (*setting_id == id).then_some(*name))
}

fn http2_setting_id(name: &str) -> Option<u16> {
    HTTP2_SETTINGS
        .iter()
        .find_map(|(setting_name, id, _)| (*setting_name == name).then_some(*id))
}

fn http2_setting_value(rt: &Runtime, opts: ObjectRef, name: &str) -> Option<u32> {
    match rt.object_get(opts, name) {
        Value::Undefined => None,
        Value::Boolean(value) => Some(if value { 1 } else { 0 }),
        Value::Number(value) if value.is_finite() && value >= 0.0 => Some(value as u32),
        _ => None,
    }
}

pub fn install_http2(rt: &mut Runtime) {
    let ns = new_object(rt);
    register_method(rt, ns, "createSecureServer", |rt, args| {
        crate::tls::do_create_node_http2_secure_server(rt, args)
    });
    register_method(rt, ns, "createServer", |rt, args| {
        let handler = args.iter().find(|v| rt.is_callable(v)).cloned();
        Ok(Value::Object(crate::http2_client::make_http2_server(
            rt, handler,
        )))
    });

    register_method(rt, ns, "connect", |rt, args| {
        let url = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => {
                return Err(rusty_js_runtime::RuntimeError::TypeError(
                    "http2.connect: authority required".into(),
                ))
            }
        };
        let insecure = match args.get(1) {
            Some(Value::Object(o)) => matches!(
                rt.object_get(*o, "rejectUnauthorized"),
                Value::Boolean(false)
            ),
            _ => false,
        };
        Ok(Value::Object(crate::http2_client::make_session(
            rt, &url, insecure,
        )))
    });
    register_method(rt, ns, "getDefaultSettings", |rt, _args| {
        Ok(Value::Object(http2_settings_object(rt, true)))
    });
    register_method(rt, ns, "getPackedSettings", |rt, args| {
        let Value::Object(opts) = args.first().cloned().unwrap_or(Value::Undefined) else {
            return Ok(intrinsic_buffer_from_bytes(rt, &[]));
        };
        let mut bytes = Vec::new();
        for (name, _id, _default) in HTTP2_SETTINGS {
            let Some(value) = http2_setting_value(rt, opts, name) else {
                continue;
            };
            let id = http2_setting_id(name).unwrap();
            bytes.extend_from_slice(&id.to_be_bytes());
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        Ok(intrinsic_buffer_from_bytes(rt, &bytes))
    });
    register_method(rt, ns, "getUnpackedSettings", |rt, args| {
        let obj = new_object(rt);
        let Some(input) = args.first() else {
            return Ok(Value::Object(obj));
        };
        let bytes = buffer_encoding_probe_bytes(rt, input, "http2.getUnpackedSettings")?;
        for chunk in bytes.chunks_exact(6) {
            let id = u16::from_be_bytes([chunk[0], chunk[1]]);
            let value = u32::from_be_bytes([chunk[2], chunk[3], chunk[4], chunk[5]]);
            if let Some(name) = http2_setting_name(id) {
                if name == "enablePush" {
                    rt.object_set(obj, name.into(), Value::Boolean(value != 0));
                } else {
                    rt.object_set(obj, name.into(), Value::Number(value as f64));
                }
            }
        }
        Ok(Value::Object(obj))
    });
    register_method(rt, ns, "performServerHandshake", |rt, _args| {
        Ok(Value::Object(new_object(rt)))
    });
    for cls in ["Http2ServerRequest", "Http2ServerResponse"] {
        let c = make_callable(rt, cls, |rt, _a| Ok(rt.current_this()));
        let proto = new_object(rt);
        rt.object_set(proto, "constructor".into(), Value::Object(c));
        rt.object_set(c, "prototype".into(), Value::Object(proto));
        rt.object_set(ns, cls.to_string(), Value::Object(c));
    }
    {
        let c = new_object(rt);

        for (k, v) in [
            ("NGHTTP2_ERR_FRAME_SIZE_ERROR", -522.0),
            ("NGHTTP2_SESSION_SERVER", 0.0),
            ("NGHTTP2_SESSION_CLIENT", 1.0),
            ("NGHTTP2_STREAM_STATE_IDLE", 1.0),
            ("NGHTTP2_STREAM_STATE_OPEN", 2.0),
            ("NGHTTP2_STREAM_STATE_RESERVED_LOCAL", 3.0),
            ("NGHTTP2_STREAM_STATE_RESERVED_REMOTE", 4.0),
            ("NGHTTP2_STREAM_STATE_HALF_CLOSED_LOCAL", 5.0),
            ("NGHTTP2_STREAM_STATE_HALF_CLOSED_REMOTE", 6.0),
            ("NGHTTP2_STREAM_STATE_CLOSED", 7.0),
            ("NGHTTP2_FLAG_NONE", 0.0),
            ("NGHTTP2_FLAG_END_STREAM", 1.0),
            ("NGHTTP2_FLAG_END_HEADERS", 4.0),
            ("NGHTTP2_FLAG_ACK", 1.0),
            ("NGHTTP2_FLAG_PADDED", 8.0),
            ("NGHTTP2_FLAG_PRIORITY", 32.0),
            ("DEFAULT_SETTINGS_HEADER_TABLE_SIZE", 4096.0),
            ("DEFAULT_SETTINGS_ENABLE_PUSH", 1.0),
            ("DEFAULT_SETTINGS_MAX_CONCURRENT_STREAMS", 4294967295.0),
            ("DEFAULT_SETTINGS_INITIAL_WINDOW_SIZE", 65535.0),
            ("DEFAULT_SETTINGS_MAX_FRAME_SIZE", 16384.0),
            ("DEFAULT_SETTINGS_MAX_HEADER_LIST_SIZE", 65535.0),
            ("DEFAULT_SETTINGS_ENABLE_CONNECT_PROTOCOL", 0.0),
            ("MAX_MAX_FRAME_SIZE", 16777215.0),
            ("MIN_MAX_FRAME_SIZE", 16384.0),
            ("MAX_INITIAL_WINDOW_SIZE", 2147483647.0),
            ("NGHTTP2_SETTINGS_HEADER_TABLE_SIZE", 1.0),
            ("NGHTTP2_SETTINGS_ENABLE_PUSH", 2.0),
            ("NGHTTP2_SETTINGS_MAX_CONCURRENT_STREAMS", 3.0),
            ("NGHTTP2_SETTINGS_INITIAL_WINDOW_SIZE", 4.0),
            ("NGHTTP2_SETTINGS_MAX_FRAME_SIZE", 5.0),
            ("NGHTTP2_SETTINGS_MAX_HEADER_LIST_SIZE", 6.0),
            ("NGHTTP2_SETTINGS_ENABLE_CONNECT_PROTOCOL", 8.0),
            ("PADDING_STRATEGY_NONE", 0.0),
            ("PADDING_STRATEGY_ALIGNED", 1.0),
            ("PADDING_STRATEGY_MAX", 2.0),
            ("PADDING_STRATEGY_CALLBACK", 1.0),
            ("NGHTTP2_NO_ERROR", 0.0),
            ("NGHTTP2_PROTOCOL_ERROR", 1.0),
            ("NGHTTP2_INTERNAL_ERROR", 2.0),
            ("NGHTTP2_FLOW_CONTROL_ERROR", 3.0),
            ("NGHTTP2_SETTINGS_TIMEOUT", 4.0),
            ("NGHTTP2_STREAM_CLOSED", 5.0),
            ("NGHTTP2_FRAME_SIZE_ERROR", 6.0),
            ("NGHTTP2_REFUSED_STREAM", 7.0),
            ("NGHTTP2_CANCEL", 8.0),
            ("NGHTTP2_COMPRESSION_ERROR", 9.0),
            ("NGHTTP2_CONNECT_ERROR", 10.0),
            ("NGHTTP2_ENHANCE_YOUR_CALM", 11.0),
            ("NGHTTP2_INADEQUATE_SECURITY", 12.0),
            ("NGHTTP2_HTTP_1_1_REQUIRED", 13.0),
            ("NGHTTP2_DEFAULT_WEIGHT", 16.0),
            ("HTTP_STATUS_CONTINUE", 100.0),
            ("HTTP_STATUS_SWITCHING_PROTOCOLS", 101.0),
            ("HTTP_STATUS_PROCESSING", 102.0),
            ("HTTP_STATUS_EARLY_HINTS", 103.0),
            ("HTTP_STATUS_OK", 200.0),
            ("HTTP_STATUS_CREATED", 201.0),
            ("HTTP_STATUS_ACCEPTED", 202.0),
            ("HTTP_STATUS_NON_AUTHORITATIVE_INFORMATION", 203.0),
            ("HTTP_STATUS_NO_CONTENT", 204.0),
            ("HTTP_STATUS_RESET_CONTENT", 205.0),
            ("HTTP_STATUS_PARTIAL_CONTENT", 206.0),
            ("HTTP_STATUS_MULTI_STATUS", 207.0),
            ("HTTP_STATUS_ALREADY_REPORTED", 208.0),
            ("HTTP_STATUS_IM_USED", 226.0),
            ("HTTP_STATUS_MULTIPLE_CHOICES", 300.0),
            ("HTTP_STATUS_MOVED_PERMANENTLY", 301.0),
            ("HTTP_STATUS_FOUND", 302.0),
            ("HTTP_STATUS_SEE_OTHER", 303.0),
            ("HTTP_STATUS_NOT_MODIFIED", 304.0),
            ("HTTP_STATUS_USE_PROXY", 305.0),
            ("HTTP_STATUS_TEMPORARY_REDIRECT", 307.0),
            ("HTTP_STATUS_PERMANENT_REDIRECT", 308.0),
            ("HTTP_STATUS_BAD_REQUEST", 400.0),
            ("HTTP_STATUS_UNAUTHORIZED", 401.0),
            ("HTTP_STATUS_PAYMENT_REQUIRED", 402.0),
            ("HTTP_STATUS_FORBIDDEN", 403.0),
            ("HTTP_STATUS_NOT_FOUND", 404.0),
            ("HTTP_STATUS_METHOD_NOT_ALLOWED", 405.0),
            ("HTTP_STATUS_NOT_ACCEPTABLE", 406.0),
            ("HTTP_STATUS_PROXY_AUTHENTICATION_REQUIRED", 407.0),
            ("HTTP_STATUS_REQUEST_TIMEOUT", 408.0),
            ("HTTP_STATUS_CONFLICT", 409.0),
            ("HTTP_STATUS_GONE", 410.0),
            ("HTTP_STATUS_LENGTH_REQUIRED", 411.0),
            ("HTTP_STATUS_PRECONDITION_FAILED", 412.0),
            ("HTTP_STATUS_PAYLOAD_TOO_LARGE", 413.0),
            ("HTTP_STATUS_URI_TOO_LONG", 414.0),
            ("HTTP_STATUS_UNSUPPORTED_MEDIA_TYPE", 415.0),
            ("HTTP_STATUS_RANGE_NOT_SATISFIABLE", 416.0),
            ("HTTP_STATUS_EXPECTATION_FAILED", 417.0),
            ("HTTP_STATUS_TEAPOT", 418.0),
            ("HTTP_STATUS_MISDIRECTED_REQUEST", 421.0),
            ("HTTP_STATUS_UNPROCESSABLE_ENTITY", 422.0),
            ("HTTP_STATUS_LOCKED", 423.0),
            ("HTTP_STATUS_FAILED_DEPENDENCY", 424.0),
            ("HTTP_STATUS_TOO_EARLY", 425.0),
            ("HTTP_STATUS_UPGRADE_REQUIRED", 426.0),
            ("HTTP_STATUS_PRECONDITION_REQUIRED", 428.0),
            ("HTTP_STATUS_TOO_MANY_REQUESTS", 429.0),
            ("HTTP_STATUS_REQUEST_HEADER_FIELDS_TOO_LARGE", 431.0),
            ("HTTP_STATUS_UNAVAILABLE_FOR_LEGAL_REASONS", 451.0),
            ("HTTP_STATUS_INTERNAL_SERVER_ERROR", 500.0),
            ("HTTP_STATUS_NOT_IMPLEMENTED", 501.0),
            ("HTTP_STATUS_BAD_GATEWAY", 502.0),
            ("HTTP_STATUS_SERVICE_UNAVAILABLE", 503.0),
            ("HTTP_STATUS_GATEWAY_TIMEOUT", 504.0),
            ("HTTP_STATUS_HTTP_VERSION_NOT_SUPPORTED", 505.0),
            ("HTTP_STATUS_VARIANT_ALSO_NEGOTIATES", 506.0),
            ("HTTP_STATUS_INSUFFICIENT_STORAGE", 507.0),
            ("HTTP_STATUS_LOOP_DETECTED", 508.0),
            ("HTTP_STATUS_BANDWIDTH_LIMIT_EXCEEDED", 509.0),
            ("HTTP_STATUS_NOT_EXTENDED", 510.0),
            ("HTTP_STATUS_NETWORK_AUTHENTICATION_REQUIRED", 511.0),
        ] {
            rt.object_set(c, k.to_string(), Value::Number(v));
        }
        for (k, v) in [
            ("HTTP2_HEADER_STATUS", ":status"),
            ("HTTP2_HEADER_METHOD", ":method"),
            ("HTTP2_HEADER_AUTHORITY", ":authority"),
            ("HTTP2_HEADER_SCHEME", ":scheme"),
            ("HTTP2_HEADER_PATH", ":path"),
            ("HTTP2_HEADER_PROTOCOL", ":protocol"),
            ("HTTP2_HEADER_ACCEPT_ENCODING", "accept-encoding"),
            ("HTTP2_HEADER_ACCEPT_LANGUAGE", "accept-language"),
            ("HTTP2_HEADER_ACCEPT_RANGES", "accept-ranges"),
            ("HTTP2_HEADER_ACCEPT", "accept"),
            (
                "HTTP2_HEADER_ACCESS_CONTROL_ALLOW_CREDENTIALS",
                "access-control-allow-credentials",
            ),
            (
                "HTTP2_HEADER_ACCESS_CONTROL_ALLOW_HEADERS",
                "access-control-allow-headers",
            ),
            (
                "HTTP2_HEADER_ACCESS_CONTROL_ALLOW_METHODS",
                "access-control-allow-methods",
            ),
            (
                "HTTP2_HEADER_ACCESS_CONTROL_ALLOW_ORIGIN",
                "access-control-allow-origin",
            ),
            (
                "HTTP2_HEADER_ACCESS_CONTROL_EXPOSE_HEADERS",
                "access-control-expose-headers",
            ),
            (
                "HTTP2_HEADER_ACCESS_CONTROL_REQUEST_HEADERS",
                "access-control-request-headers",
            ),
            (
                "HTTP2_HEADER_ACCESS_CONTROL_REQUEST_METHOD",
                "access-control-request-method",
            ),
            ("HTTP2_HEADER_AGE", "age"),
            ("HTTP2_HEADER_AUTHORIZATION", "authorization"),
            ("HTTP2_HEADER_CACHE_CONTROL", "cache-control"),
            ("HTTP2_HEADER_CONNECTION", "connection"),
            ("HTTP2_HEADER_CONTENT_DISPOSITION", "content-disposition"),
            ("HTTP2_HEADER_CONTENT_ENCODING", "content-encoding"),
            ("HTTP2_HEADER_CONTENT_LENGTH", "content-length"),
            ("HTTP2_HEADER_CONTENT_TYPE", "content-type"),
            ("HTTP2_HEADER_COOKIE", "cookie"),
            ("HTTP2_HEADER_DATE", "date"),
            ("HTTP2_HEADER_ETAG", "etag"),
            ("HTTP2_HEADER_FORWARDED", "forwarded"),
            ("HTTP2_HEADER_HOST", "host"),
            ("HTTP2_HEADER_IF_MODIFIED_SINCE", "if-modified-since"),
            ("HTTP2_HEADER_IF_NONE_MATCH", "if-none-match"),
            ("HTTP2_HEADER_IF_RANGE", "if-range"),
            ("HTTP2_HEADER_LAST_MODIFIED", "last-modified"),
            ("HTTP2_HEADER_LINK", "link"),
            ("HTTP2_HEADER_LOCATION", "location"),
            ("HTTP2_HEADER_RANGE", "range"),
            ("HTTP2_HEADER_REFERER", "referer"),
            ("HTTP2_HEADER_SERVER", "server"),
            ("HTTP2_HEADER_SET_COOKIE", "set-cookie"),
            (
                "HTTP2_HEADER_STRICT_TRANSPORT_SECURITY",
                "strict-transport-security",
            ),
            ("HTTP2_HEADER_TRANSFER_ENCODING", "transfer-encoding"),
            ("HTTP2_HEADER_TE", "te"),
            (
                "HTTP2_HEADER_UPGRADE_INSECURE_REQUESTS",
                "upgrade-insecure-requests",
            ),
            ("HTTP2_HEADER_UPGRADE", "upgrade"),
            ("HTTP2_HEADER_USER_AGENT", "user-agent"),
            ("HTTP2_HEADER_VARY", "vary"),
            (
                "HTTP2_HEADER_X_CONTENT_TYPE_OPTIONS",
                "x-content-type-options",
            ),
            ("HTTP2_HEADER_X_FRAME_OPTIONS", "x-frame-options"),
            ("HTTP2_HEADER_KEEP_ALIVE", "keep-alive"),
            ("HTTP2_HEADER_PROXY_CONNECTION", "proxy-connection"),
            ("HTTP2_HEADER_X_XSS_PROTECTION", "x-xss-protection"),
            ("HTTP2_HEADER_ALT_SVC", "alt-svc"),
            (
                "HTTP2_HEADER_CONTENT_SECURITY_POLICY",
                "content-security-policy",
            ),
            ("HTTP2_HEADER_EARLY_DATA", "early-data"),
            ("HTTP2_HEADER_EXPECT_CT", "expect-ct"),
            ("HTTP2_HEADER_ORIGIN", "origin"),
            ("HTTP2_HEADER_PURPOSE", "purpose"),
            ("HTTP2_HEADER_TIMING_ALLOW_ORIGIN", "timing-allow-origin"),
            ("HTTP2_HEADER_X_FORWARDED_FOR", "x-forwarded-for"),
            ("HTTP2_HEADER_PRIORITY", "priority"),
            ("HTTP2_HEADER_ACCEPT_CHARSET", "accept-charset"),
            (
                "HTTP2_HEADER_ACCESS_CONTROL_MAX_AGE",
                "access-control-max-age",
            ),
            ("HTTP2_HEADER_ALLOW", "allow"),
            ("HTTP2_HEADER_CONTENT_LANGUAGE", "content-language"),
            ("HTTP2_HEADER_CONTENT_LOCATION", "content-location"),
            ("HTTP2_HEADER_CONTENT_MD5", "content-md5"),
            ("HTTP2_HEADER_CONTENT_RANGE", "content-range"),
            ("HTTP2_HEADER_DNT", "dnt"),
            ("HTTP2_HEADER_EXPECT", "expect"),
            ("HTTP2_HEADER_EXPIRES", "expires"),
            ("HTTP2_HEADER_FROM", "from"),
            ("HTTP2_HEADER_IF_MATCH", "if-match"),
            ("HTTP2_HEADER_IF_UNMODIFIED_SINCE", "if-unmodified-since"),
            ("HTTP2_HEADER_MAX_FORWARDS", "max-forwards"),
            ("HTTP2_HEADER_PREFER", "prefer"),
            ("HTTP2_HEADER_PROXY_AUTHENTICATE", "proxy-authenticate"),
            ("HTTP2_HEADER_PROXY_AUTHORIZATION", "proxy-authorization"),
            ("HTTP2_HEADER_REFRESH", "refresh"),
            ("HTTP2_HEADER_RETRY_AFTER", "retry-after"),
            ("HTTP2_HEADER_TRAILER", "trailer"),
            ("HTTP2_HEADER_TK", "tk"),
            ("HTTP2_HEADER_VIA", "via"),
            ("HTTP2_HEADER_WARNING", "warning"),
            ("HTTP2_HEADER_WWW_AUTHENTICATE", "www-authenticate"),
            ("HTTP2_HEADER_HTTP2_SETTINGS", "http2-settings"),
            ("HTTP2_METHOD_ACL", "ACL"),
            ("HTTP2_METHOD_BASELINE_CONTROL", "BASELINE-CONTROL"),
            ("HTTP2_METHOD_BIND", "BIND"),
            ("HTTP2_METHOD_CHECKIN", "CHECKIN"),
            ("HTTP2_METHOD_CHECKOUT", "CHECKOUT"),
            ("HTTP2_METHOD_CONNECT", "CONNECT"),
            ("HTTP2_METHOD_COPY", "COPY"),
            ("HTTP2_METHOD_DELETE", "DELETE"),
            ("HTTP2_METHOD_GET", "GET"),
            ("HTTP2_METHOD_HEAD", "HEAD"),
            ("HTTP2_METHOD_LABEL", "LABEL"),
            ("HTTP2_METHOD_LINK", "LINK"),
            ("HTTP2_METHOD_LOCK", "LOCK"),
            ("HTTP2_METHOD_MERGE", "MERGE"),
            ("HTTP2_METHOD_MKACTIVITY", "MKACTIVITY"),
            ("HTTP2_METHOD_MKCALENDAR", "MKCALENDAR"),
            ("HTTP2_METHOD_MKCOL", "MKCOL"),
            ("HTTP2_METHOD_MKREDIRECTREF", "MKREDIRECTREF"),
            ("HTTP2_METHOD_MKWORKSPACE", "MKWORKSPACE"),
            ("HTTP2_METHOD_MOVE", "MOVE"),
            ("HTTP2_METHOD_OPTIONS", "OPTIONS"),
            ("HTTP2_METHOD_ORDERPATCH", "ORDERPATCH"),
            ("HTTP2_METHOD_PATCH", "PATCH"),
            ("HTTP2_METHOD_POST", "POST"),
            ("HTTP2_METHOD_PRI", "PRI"),
            ("HTTP2_METHOD_PROPFIND", "PROPFIND"),
            ("HTTP2_METHOD_PROPPATCH", "PROPPATCH"),
            ("HTTP2_METHOD_PUT", "PUT"),
            ("HTTP2_METHOD_REBIND", "REBIND"),
            ("HTTP2_METHOD_REPORT", "REPORT"),
            ("HTTP2_METHOD_SEARCH", "SEARCH"),
            ("HTTP2_METHOD_TRACE", "TRACE"),
            ("HTTP2_METHOD_UNBIND", "UNBIND"),
            ("HTTP2_METHOD_UNCHECKOUT", "UNCHECKOUT"),
            ("HTTP2_METHOD_UNLINK", "UNLINK"),
            ("HTTP2_METHOD_UNLOCK", "UNLOCK"),
            ("HTTP2_METHOD_UPDATE", "UPDATE"),
            ("HTTP2_METHOD_UPDATEREDIRECTREF", "UPDATEREDIRECTREF"),
            ("HTTP2_METHOD_VERSION_CONTROL", "VERSION-CONTROL"),
        ] {
            rt.object_set(
                c,
                k.to_string(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(v))),
            );
        }
        rt.object_set(ns, "constants".into(), Value::Object(c));
    }
    rt.object_set(
        ns,
        "sensitiveHeaders".into(),
        Value::Symbol(Rc::new("nodejs.http2.sensitiveHeaders".to_string())),
    );
    rt.define_global_property("http2", Value::Object(ns));
}

pub fn install_safe_regex2_compat(rt: &mut Runtime) {
    let safe_regex = make_callable(rt, "safeRegex", |_rt, _args| Ok(Value::Boolean(true)));
    rt.object_set(safe_regex, "default".into(), Value::Object(safe_regex));
    rt.object_set(safe_regex, "safeRegex".into(), Value::Object(safe_regex));
    rt.define_global_property("__safe_regex2_compat", Value::Object(safe_regex));
}

pub fn install_dns(rt: &mut Runtime) {
    crate::dns::install_canonical(rt);
    crate::dns::install(rt);
}

fn js_string_value(s: String) -> Value {
    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(s)))
}

fn node_builtin_resolve_public(spec: &str) -> Option<String> {
    let bare = spec.strip_prefix("node:").unwrap_or(spec);
    if NODE_BUILTIN_MODULES.contains(&bare) {
        if spec.starts_with("node:") {
            Some(format!("node:{bare}"))
        } else {
            Some(bare.to_string())
        }
    } else {
        None
    }
}

fn parent_dir_for_require_paths(parent_url: &str) -> String {
    let stripped = parent_url.strip_prefix("file://").unwrap_or(parent_url);
    let path = std::path::Path::new(stripped);
    let dir = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or_else(|| std::path::Path::new("."))
    };
    dir.to_string_lossy().to_string()
}

fn node_modules_paths_from(start: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = std::path::PathBuf::from(start);
    loop {
        out.push(cur.join("node_modules").to_string_lossy().to_string());
        if !cur.pop() {
            break;
        }
    }
    out
}

fn resolve_bare_node_modules_file(parent_url: &str, spec: &str) -> Option<String> {
    if spec.is_empty()
        || spec.starts_with('.')
        || spec.starts_with('/')
        || spec.starts_with("file:")
        || spec.starts_with("node:")
    {
        return None;
    }
    let mut parts = spec.split('/');
    let first = parts.next()?;
    let (package, rest): (String, Vec<&str>) = if first.starts_with('@') {
        let second = parts.next()?;
        (format!("{first}/{second}"), parts.collect())
    } else {
        (first.to_string(), parts.collect())
    };
    for nm in node_modules_paths_from(&parent_dir_for_require_paths(parent_url)) {
        let mut base = std::path::Path::new(&nm).join(&package);
        for part in &rest {
            base = base.join(part);
        }
        let file_candidate = base.with_extension("js");
        if file_candidate.is_file() {
            return Some(file_candidate.to_string_lossy().to_string());
        }
        let index_candidate = base.join("index.js");
        if index_candidate.is_file() {
            return Some(index_candidate.to_string_lossy().to_string());
        }
    }
    None
}

fn direct_node_modules_file(base: &std::path::Path, spec: &str) -> Option<String> {
    if spec.is_empty()
        || spec.starts_with('.')
        || spec.starts_with('/')
        || spec.starts_with("file:")
        || spec.starts_with("node:")
        || spec.contains('/')
    {
        return None;
    }
    if base.file_name().and_then(|s| s.to_str()) != Some("node_modules") {
        return None;
    }
    if base
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        == Some("node_modules")
    {
        return None;
    }
    let candidate = base.join(format!("{spec}.js"));
    if candidate.is_file() {
        Some(candidate.to_string_lossy().to_string())
    } else {
        None
    }
}

fn require_resolve_paths_value(rt: &mut Runtime, parent_url: &str, spec: &str) -> Value {
    if node_builtin_resolve_public(spec).is_some() {
        return Value::Null;
    }

    let parent_dir = parent_dir_for_require_paths(parent_url);
    let paths = if spec == "." || spec == ".." || spec.starts_with("./") || spec.starts_with("../")
    {
        vec![parent_dir]
    } else {
        node_modules_paths_from(&parent_dir)
    };

    let arr_id = rt.alloc_object(RtObject::new_array());
    for (i, path) in paths.iter().enumerate() {
        rt.object_set(arr_id, i.to_string(), js_string_value(path.clone()));
    }
    rt.object_set(arr_id, "length".into(), Value::Number(paths.len() as f64));
    Value::Object(arr_id)
}

fn require_resolve_invalid_request(rt: &mut Runtime, value: Option<&Value>) -> RuntimeError {
    let received = match value {
        Some(Value::Number(n)) if n.fract() == 0.0 => format!("type number ({})", *n as i64),
        Some(Value::Number(n)) => format!("type number ({n})"),
        Some(Value::Boolean(b)) => format!("type boolean ({b})"),
        Some(Value::Null) => "null".to_string(),
        Some(Value::Undefined) | None => "undefined".to_string(),
        Some(Value::Object(_)) => "an instance of Object".to_string(),
        Some(Value::String(_)) => "type string".to_string(),
        Some(Value::BigInt(_)) => "type bigint".to_string(),
        Some(Value::Symbol(_)) => "type symbol".to_string(),
    };
    let suffix = if received == "null" || received == "undefined" || received.starts_with("an ") {
        format!(" Received {received}")
    } else {
        format!(" Received {received}")
    };
    let msg = format!("The \"request\" argument must be of type string.{suffix}");
    match rusty_js_runtime::intrinsics::make_error_instance(rt, "TypeError", &msg) {
        Some(id) => {
            rt.object_set(
                id,
                "code".into(),
                js_string_value("ERR_INVALID_ARG_TYPE".to_string()),
            );
            RuntimeError::Thrown(Value::Object(id))
        }
        None => RuntimeError::TypeError(msg),
    }
}

fn node_code_type_error(rt: &mut Runtime, code: &str, msg: &str) -> RuntimeError {
    match rusty_js_runtime::intrinsics::make_error_instance(rt, "TypeError", msg) {
        Some(id) => {
            rt.object_set(id, "code".into(), js_string_value(code.to_string()));
            RuntimeError::Thrown(Value::Object(id))
        }
        None => RuntimeError::TypeError(msg.to_string()),
    }
}

fn node_code_range_error(rt: &mut Runtime, code: &str, msg: &str) -> RuntimeError {
    match rusty_js_runtime::intrinsics::make_error_instance(rt, "RangeError", msg) {
        Some(id) => {
            rt.object_set(id, "code".into(), js_string_value(code.to_string()));
            RuntimeError::Thrown(Value::Object(id))
        }
        None => RuntimeError::RangeError(msg.to_string()),
    }
}

fn node_code_error(rt: &mut Runtime, name: &str, code: &str, msg: &str) -> RuntimeError {
    match rusty_js_runtime::intrinsics::make_error_instance(rt, name, msg) {
        Some(id) => {
            rt.object_set(id, "code".into(), js_string_value(code.to_string()));
            RuntimeError::Thrown(Value::Object(id))
        }
        None => RuntimeError::TypeError(msg.to_string()),
    }
}

fn install_buffer_inspect_max_bytes_accessor(rt: &mut Runtime, ns: ObjectRef) {
    if let Value::Object(global) = rt.global_get("globalThis") {
        rt.object_set(
            global,
            "__cruft_buffer_inspect_max_bytes".into(),
            Value::Number(50.0),
        );
    }
    let getter = make_callable(rt, "get INSPECT_MAX_BYTES", |rt, _args| {
        Ok(match rt.global_get("__cruft_buffer_inspect_max_bytes") {
            Value::Number(n) => Value::Number(n),
            _ => Value::Number(50.0),
        })
    });
    let setter = make_callable(rt, "set INSPECT_MAX_BYTES", |rt, args| {
        let value = args.first().unwrap_or(&Value::Undefined);
        let n = match value {
            Value::Number(n) => *n,
            _ => {
                return Err(node_code_type_error(
                    rt,
                    "ERR_INVALID_ARG_TYPE",
                    "The \"INSPECT_MAX_BYTES\" property must be of type number.",
                ))
            }
        };
        if n.is_nan() || n < 0.0 {
            return Err(node_code_range_error(
                rt,
                "ERR_OUT_OF_RANGE",
                "The value of \"INSPECT_MAX_BYTES\" is out of range.",
            ));
        }
        if let Value::Object(global) = rt.global_get("globalThis") {
            rt.object_set(
                global,
                "__cruft_buffer_inspect_max_bytes".into(),
                Value::Number(n),
            );
        }
        Ok(Value::Undefined)
    });
    rt.obj_mut(ns).dict_mut().insert(
        PropertyKey::String("INSPECT_MAX_BYTES".into()),
        PropertyDescriptor {
            value: Value::Undefined,
            writable: false,
            enumerable: true,
            configurable: true,
            getter: Some(Value::Object(getter)),
            setter: Some(Value::Object(setter)),
        },
    );
}

fn buffer_encoding_arg_type(value: &Value) -> &'static str {
    match value {
        Value::Undefined => "undefined",
        Value::Null => "null",
        Value::String(_) => "string",
        Value::Number(_) => "number",
        Value::Boolean(_) => "boolean",
        Value::BigInt(_) => "bigint",
        Value::Symbol(_) => "symbol",
        Value::Object(_) => "object",
    }
}

fn buffer_encoding_probe_bytes(
    rt: &mut Runtime,
    value: &Value,
    name: &str,
) -> Result<Vec<u8>, RuntimeError> {
    let Value::Object(id) = value else {
        let msg = format!(
            "The \"input\" argument must be an instance of ArrayBuffer, Buffer, TypedArray, or DataView. Received type {}",
            buffer_encoding_arg_type(value)
        );
        return Err(node_code_type_error(rt, "ERR_INVALID_ARG_TYPE", &msg));
    };

    if let Some(rec) = rt.array_buffers.get(id) {
        if rec.detached {
            return Err(node_code_error(
                rt,
                "Error",
                "ERR_INVALID_STATE",
                &format!("{name}: ArrayBuffer is detached"),
            ));
        }
        return Ok(rec.to_bytes());
    }

    if let Some(view) = rt.typed_array_views.get(id).cloned() {
        let Some(rec) = rt.array_buffers.get(&view.buffer) else {
            let msg = "The \"input\" argument must be an instance of ArrayBuffer, Buffer, TypedArray, or DataView.";
            return Err(node_code_type_error(rt, "ERR_INVALID_ARG_TYPE", msg));
        };
        if rec.detached {
            return Err(node_code_error(
                rt,
                "Error",
                "ERR_INVALID_STATE",
                &format!("{name}: ArrayBuffer is detached"),
            ));
        }
        let len = view.fixed_length.unwrap_or_else(|| {
            rec.byte_len().saturating_sub(view.byte_offset) / view.bytes_per_element.max(1)
        });
        let byte_len = len.saturating_mul(view.bytes_per_element.max(1));
        return Ok(rec.read_bytes(view.byte_offset, view.byte_offset.saturating_add(byte_len)));
    }

    let msg = "The \"input\" argument must be an instance of ArrayBuffer, Buffer, TypedArray, or DataView. Received an instance of Object";
    Err(node_code_type_error(rt, "ERR_INVALID_ARG_TYPE", msg))
}

fn array_like_strings(rt: &Runtime, id: rusty_js_runtime::value::ObjectRef) -> Vec<String> {
    let len = match rt.object_get(id, "length") {
        Value::Number(n) if n.is_finite() && n > 0.0 => n as usize,
        _ => 0,
    };
    let mut out = Vec::new();
    for i in 0..len {
        if let Value::String(s) = rt.object_get(id, &i.to_string()) {
            out.push(s.as_str().to_string());
        }
    }
    out
}

fn require_resolve_with_custom_paths(
    rt: &mut Runtime,
    spec: &str,
    opts: Option<&Value>,
) -> Option<Result<Value, RuntimeError>> {
    let opts_id = match opts {
        Some(Value::Object(id)) => *id,
        _ => return None,
    };
    let paths_value = rt.object_get(opts_id, "paths");
    if matches!(paths_value, Value::Undefined) {
        return None;
    }
    let paths_id = match paths_value {
        Value::Object(id) => id,
        _ => {
            return Some(Err(node_code_type_error(
                rt,
                "ERR_INVALID_ARG_VALUE",
                "The argument 'paths' must be an array of strings.",
            )))
        }
    };

    let paths = array_like_strings(rt, paths_id);
    for base in paths {
        let base_path = std::path::PathBuf::from(&base);
        let abs = if base_path.is_absolute() {
            base_path
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join(base_path)
        };
        if abs.file_name().and_then(|s| s.to_str()) == Some("node_modules")
            && abs
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
                == Some("node_modules")
        {
            continue;
        }
        if let Some(path) = direct_node_modules_file(&abs, spec) {
            return Some(Ok(js_string_value(path)));
        }
        let synthetic_parent = format!(
            "file://{}/__cruft_require_resolve.js",
            abs.to_string_lossy()
        );
        if let Ok(url) =
            rt.resolve_module_full(&synthetic_parent, spec, rusty_js_runtime::ModuleKind::CJS)
        {
            return Some(Ok(js_string_value(
                url.strip_prefix("file://")
                    .map(|s| s.to_string())
                    .unwrap_or(url),
            )));
        }
        if let Some(path) = resolve_bare_node_modules_file(&synthetic_parent, spec) {
            return Some(Ok(js_string_value(path)));
        }
    }
    Some(Err(rt.node_style_cjs_require_error(
        RuntimeError::TypeError(format!("__node_cjs_missing_module__:{spec}||")),
    )))
}

fn project_create_require_cache_entry(
    rt: &mut Runtime,
    cache_id: ObjectRef,
    parent: &str,
    spec: &str,
    exports: &Value,
) {
    let resolved = resolve_bare_node_modules_file(parent, spec).or_else(|| {
        rt.resolve_module_full(parent, spec, rusty_js_runtime::ModuleKind::CJS)
            .ok()
            .map(|url| {
                url.strip_prefix("file://")
                    .map(|s| s.to_string())
                    .unwrap_or(url)
            })
    });
    let Some(filename) = resolved else {
        return;
    };
    let module_obj = rt.alloc_object(RtObject::new_ordinary());
    rt.object_set(
        module_obj,
        "id".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            filename.clone(),
        ))),
    );
    rt.object_set(
        module_obj,
        "filename".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            filename.clone(),
        ))),
    );
    rt.object_set(module_obj, "exports".into(), exports.clone());
    rt.object_set(module_obj, "loaded".into(), Value::Boolean(true));
    let children = rt.alloc_object(RtObject::new_array());
    rt.object_set(children, "length".into(), Value::Number(0.0));
    rt.object_set(module_obj, "children".into(), Value::Object(children));
    rt.object_set(cache_id, filename, Value::Object(module_obj));
}

pub fn install_global_require(rt: &mut Runtime) {
    let require_obj = crate::register::make_callable(rt, "require", |rt, args| {
        let spec = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => {
                return Err(rusty_js_runtime::RuntimeError::TypeError(
                    "require: specifier must be a string".into(),
                ))
            }
        };
        let parent = rt.current_module_url.last().cloned().unwrap_or_default();

        match args.get(1) {
            Some(Value::Object(opts)) => match rt.require_caps_grant_from_options(*opts) {
                Some(g) => rt.cjs_require_with_grant(&parent, &spec, &g),
                None => rt.cjs_require(&parent, &spec),
            },
            _ => rt.cjs_require(&parent, &spec),
        }
    });

    let resolve_fn = crate::register::make_callable(rt, "resolve", |rt, args| {
        let spec = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            other => return Err(require_resolve_invalid_request(rt, other)),
        };
        let parent = rt.current_module_url.last().cloned().unwrap_or_default();
        if let Some(builtin) = node_builtin_resolve_public(&spec) {
            return Ok(js_string_value(builtin));
        }
        if spec.starts_with("node:") {
            return Err(
                rt.node_style_cjs_require_error(RuntimeError::TypeError(format!(
                    "__node_cjs_missing_module__:{}|{}|",
                    spec, parent
                ))),
            );
        }
        if let Some(result) = require_resolve_with_custom_paths(rt, &spec, args.get(1)) {
            return result;
        }
        if let Some(path) = resolve_bare_node_modules_file(&parent, &spec) {
            return Ok(js_string_value(path));
        }
        match rt.resolve_module_full(&parent, &spec, rusty_js_runtime::ModuleKind::CJS) {
            Ok(url) => Ok(Value::String(std::rc::Rc::new(
                rusty_js_runtime::value::JsString::from(
                    url.strip_prefix("file://")
                        .map(|s| s.to_string())
                        .unwrap_or(url),
                ),
            ))),

            Err(e) => Err(rt.node_style_cjs_require_error(e)),
        }
    });
    let paths_fn = crate::register::make_callable(rt, "paths", |rt, args| {
        let spec = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            other => return Err(require_resolve_invalid_request(rt, other)),
        };
        let parent = rt.current_module_url.last().cloned().unwrap_or_default();
        Ok(require_resolve_paths_value(rt, &parent, &spec))
    });
    rt.object_set(resolve_fn, "paths".into(), Value::Object(paths_fn));
    rt.object_set(require_obj, "resolve".into(), Value::Object(resolve_fn));
    if let Value::Object(module_id) = rt.global_get("module") {
        let cache = rt.object_get(module_id, "_cache");
        if !matches!(cache, Value::Undefined) {
            rt.object_set(require_obj, "cache".into(), cache);
        }
        let extensions = rt.object_get(module_id, "_extensions");
        if !matches!(extensions, Value::Undefined) {
            rt.object_set(require_obj, "extensions".into(), extensions);
        }
    }
    rt.define_global_property("require", Value::Object(require_obj));
}

pub fn install_module(rt: &mut Runtime) {

    let proto = new_object(rt);

    register_method(rt, proto, "require", |rt, args| {
        let spec = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "Module.prototype.require: specifier must be a string".into(),
                ))
            }
        };
        let parent = rt.current_module_url.last().cloned().unwrap_or_default();

        match args.get(1) {
            Some(Value::Object(opts)) => match rt.require_caps_grant_from_options(*opts) {
                Some(g) => rt.cjs_require_with_grant(&parent, &spec, &g),
                None => rt.cjs_require(&parent, &spec),
            },
            _ => rt.cjs_require(&parent, &spec),
        }
    });
    register_method(rt, proto, "_compile", |rt, args| {
        let source = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            Some(other) => rusty_js_runtime::abstract_ops::to_string(other)
                .as_str()
                .to_string(),
            None => String::new(),
        };
        let filename = match args.get(1) {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => match rt.current_this() {
                Value::Object(id) => match rt.object_get(id, "filename") {
                    Value::String(s) => s.as_str().to_string(),
                    _ => String::from("[module]"),
                },
                _ => String::from("[module]"),
            },
        };
        let module_obj = match rt.current_this() {
            Value::Object(obj) => obj,
            _ => {
                return Err(RuntimeError::TypeError(
                    "Module._compile: receiver must be a Module object".into(),
                ))
            }
        };
        if matches!(rt.object_get(module_obj, "exports"), Value::Undefined) {
            let exports = rt.alloc_object(RtObject::new_ordinary());
            rt.object_set(module_obj, "exports".into(), Value::Object(exports));
        }
        rt.object_set(
            module_obj,
            "filename".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                filename.clone(),
            ))),
        );
        let dirname = std::path::Path::new(&filename)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let file_url = if filename.starts_with("file://") {
            filename.clone()
        } else {
            format!("file://{}", filename)
        };
        let source_no_shebang = if source.starts_with("#!") {
            match source.find('\n') {
                Some(nl) => &source[nl + 1..],
                None => "",
            }
        } else {
            &source
        };
        let global = match rt.global_object {
            Some(global) => global,
            None => {
                return Err(RuntimeError::TypeError(
                    "Module._compile: no active global object".into(),
                ))
            }
        };
        rt.object_set(
            global,
            "__cruft_compile_module".into(),
            Value::Object(module_obj),
        );
        let wrapped = format!(
            "(function() {{\n\
             const module = globalThis.__cruft_compile_module;\n\
             const __filename = {:?};\n\
             const __dirname = {:?};\n\
             const __cruft_outer_require = globalThis.require;\n\
             const __cruft_compile_Module = __cruft_outer_require(\"module\");\n\
             const __cruft_compile_require = module.require || __cruft_compile_Module.createRequire({:?});\n\
             module.require = __cruft_compile_require;\n\
             const exports = module.exports;\n\
             (function(exports, require, module, __filename, __dirname) {{\n{}\n\
             }}).call(exports, exports, __cruft_compile_require, module, __filename, __dirname);\n\
             module.loaded = true;\n\
             }})();",
            filename, dirname, file_url, source_no_shebang
        );
        let result = rt.run_script(&wrapped, &file_url).map(|_| Value::Undefined);
        rt.object_set(global, "__cruft_compile_module".into(), Value::Undefined);
        result
    });
    register_method(rt, proto, "load", |rt, args| {
        let filename = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            Some(other) => rusty_js_runtime::abstract_ops::to_string(other)
                .as_str()
                .to_string(),
            None => String::new(),
        };
        if let Value::Object(this) = rt.current_this() {
            rt.object_set(
                this,
                "filename".into(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(filename))),
            );
            rt.object_set(this, "loaded".into(), Value::Boolean(true));
        }
        Ok(Value::Undefined)
    });

    let ctor = make_callable(rt, "Module", |rt, args| {
        let id = match args.first() {
            Some(Value::String(s)) => Value::String(s.clone()),
            _ => Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                String::new(),
            ))),
        };
        let parent = args.get(1).cloned().unwrap_or(Value::Undefined);
        let this = match rt.current_this() {
            Value::Object(obj) => obj,
            _ => rt.alloc_object(RtObject::new_ordinary()),
        };
        let empty_exports = rt.alloc_object(RtObject::new_ordinary());
        rt.object_set(this, "id".into(), id);
        rt.object_set(this, "exports".into(), Value::Object(empty_exports));
        rt.object_set(this, "parent".into(), parent);
        rt.object_set(this, "filename".into(), Value::Null);
        rt.object_set(this, "loaded".into(), Value::Boolean(false));
        let children = rt.alloc_object(RtObject::new_array());
        rt.object_set(children, "length".into(), Value::Number(0.0));
        rt.object_set(this, "children".into(), Value::Object(children));
        Ok(Value::Object(this))
    });
    rt.set_own_frozen_property(ctor, "prototype".into(), Value::Object(proto));
    rt.obj_mut(proto)
        .set_own_internal("constructor".into(), Value::Object(ctor));

    register_method(rt, ctor, "_resolveFilename", |rt, args| {
        let spec = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "Module._resolveFilename: request must be a string".into(),
                ))
            }
        };

        let parent = match args.get(1) {
            Some(Value::Object(id)) => match rt.object_get(*id, "filename") {
                Value::String(s) => s.as_str().to_string(),
                _ => rt.current_module_url.last().cloned().unwrap_or_default(),
            },
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => rt.current_module_url.last().cloned().unwrap_or_default(),
        };
        match rt.resolve_module_full(&parent, &spec, rusty_js_runtime::ModuleKind::CJS) {
            Ok(url) => Ok(Value::String(Rc::new(
                rusty_js_runtime::value::JsString::from(
                    url.strip_prefix("file://")
                        .map(|s| s.to_string())
                        .unwrap_or(url),
                ),
            ))),
            Err(e) => Err(e),
        }
    });

    register_method(rt, ctor, "_load", |rt, args| {
        let spec = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "Module._load: request must be a string".into(),
                ))
            }
        };
        let parent = match args.get(1) {
            Some(Value::Object(id)) => match rt.object_get(*id, "filename") {
                Value::String(s) => s.as_str().to_string(),
                _ => rt.current_module_url.last().cloned().unwrap_or_default(),
            },
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => rt.current_module_url.last().cloned().unwrap_or_default(),
        };

        match args.get(1) {
            Some(Value::Object(opts)) => match rt.require_caps_grant_from_options(*opts) {
                Some(g) => rt.cjs_require_with_grant(&parent, &spec, &g),
                None => rt.cjs_require(&parent, &spec),
            },
            _ => rt.cjs_require(&parent, &spec),
        }
    });
    let ns = ctor;

    register_method(rt, ns, "createRequire", move |rt, args| {
        let parent_url = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            Some(Value::Object(id)) => {

                let href = rt.object_get(*id, "href");
                let url = rt.object_get(*id, "url");
                match (href, url) {
                    (Value::String(s), _) | (_, Value::String(s)) => s.as_str().to_string(),
                    _ => String::new(),
                }
            }
            _ => String::new(),
        };

        let parent_url = if parent_url.starts_with('/') && !parent_url.contains("://") {
            format!("file://{parent_url}")
        } else {
            parent_url
        };
        let parent_for_req = parent_url.clone();
        let parent_for_res = parent_url.clone();
        let cache_for_req = match rt.global_get("__cruft_cjs_require_cache") {
            Value::Object(id) => id,
            _ => {
                let id = match rt.object_get(ns, "_cache") {
                    Value::Object(id) => id,
                    _ => new_object(rt),
                };
                rt.define_global_property("__cruft_cjs_require_cache", Value::Object(id));
                id
            }
        };
        let require_obj = crate::register::make_callable(rt, "require", move |rt, args| {
            let spec = match args.first() {
                Some(Value::String(s)) => s.as_str().to_string(),
                _ => {
                    return Err(rusty_js_runtime::RuntimeError::TypeError(
                        "require: specifier must be a string".into(),
                    ))
                }
            };

            let result = match args.get(1) {
                Some(Value::Object(opts)) => match rt.require_caps_grant_from_options(*opts) {
                    Some(g) => rt.cjs_require_with_grant(&parent_for_req, &spec, &g),
                    None => rt.cjs_require(&parent_for_req, &spec),
                },
                _ => rt.cjs_require(&parent_for_req, &spec),
            };
            if let Ok(exports) = &result {
                project_create_require_cache_entry(
                    rt,
                    cache_for_req,
                    &parent_for_req,
                    &spec,
                    exports,
                );
            }
            if let (Value::Object(cache_id), Ok(exports)) =
                (rt.global_get("__cruft_cjs_require_cache"), &result)
            {
                project_create_require_cache_entry(rt, cache_id, &parent_for_req, &spec, exports);
            }
            result
        });

        let paths_parent = parent_url.clone();
        let resolve_fn = crate::register::make_callable(rt, "resolve", move |rt, args| {
            let spec = match args.first() {
                Some(Value::String(s)) => s.as_str().to_string(),
                other => return Err(require_resolve_invalid_request(rt, other)),
            };
            if let Some(builtin) = node_builtin_resolve_public(&spec) {
                return Ok(js_string_value(builtin));
            }
            if spec.starts_with("node:") {
                return Err(
                    rt.node_style_cjs_require_error(RuntimeError::TypeError(format!(
                        "__node_cjs_missing_module__:{}|{}|",
                        spec, parent_for_res
                    ))),
                );
            }
            if let Some(result) = require_resolve_with_custom_paths(rt, &spec, args.get(1)) {
                return result;
            }
            if let Some(path) = resolve_bare_node_modules_file(&parent_for_res, &spec) {
                return Ok(js_string_value(path));
            }
            match rt.resolve_module_full(&parent_for_res, &spec, rusty_js_runtime::ModuleKind::CJS)
            {
                Ok(url) => Ok(Value::String(std::rc::Rc::new(
                    rusty_js_runtime::value::JsString::from(
                        url.strip_prefix("file://")
                            .map(|s| s.to_string())
                            .unwrap_or(url),
                    ),
                ))),

                Err(e) => Err(rt.node_style_cjs_require_error(e)),
            }
        });
        let paths_fn = crate::register::make_callable(rt, "paths", move |rt, args| {
            let spec = match args.first() {
                Some(Value::String(s)) => s.as_str().to_string(),
                other => return Err(require_resolve_invalid_request(rt, other)),
            };
            Ok(require_resolve_paths_value(rt, &paths_parent, &spec))
        });
        rt.object_set(resolve_fn, "paths".into(), Value::Object(paths_fn));
        rt.object_set(require_obj, "resolve".into(), Value::Object(resolve_fn));

        let cache = match rt.global_get("__cruft_cjs_require_cache") {
            Value::Object(id) => Value::Object(id),
            _ => rt.object_get(ns, "_cache"),
        };
        if !matches!(cache, Value::Undefined) {
            rt.object_set(require_obj, "cache".into(), cache);
        }
        let extensions = rt.object_get(ns, "_extensions");
        if !matches!(extensions, Value::Undefined) {
            rt.object_set(require_obj, "extensions".into(), extensions);
        }
        Ok(Value::Object(require_obj))
    });

    let arr = RtObject::new_array();
    let arr_id = rt.alloc_object(arr);

    let entries: Vec<String> = NODE_BUILTIN_MODULES
        .iter()
        .map(|n| (*n).to_string())
        .chain(
            NODE_PREFIX_ONLY_BUILTINS
                .iter()
                .map(|n| format!("node:{n}")),
        )
        .collect();
    for (i, name) in entries.iter().enumerate() {
        rt.object_set(
            arr_id,
            i.to_string(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                name.clone(),
            ))),
        );
    }
    rt.object_set(arr_id, "length".into(), Value::Number(entries.len() as f64));
    rt.object_set(ns, "builtinModules".into(), Value::Object(arr_id));

    rt.object_set(ns, "Module".into(), Value::Object(ns));

    const WRAP_HEAD: &str = "(function (exports, require, module, __filename, __dirname) { ";
    const WRAP_TAIL: &str = "\n});";
    {
        let arr = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
        rt.object_set(
            arr,
            "0".into(),
            Value::String(std::rc::Rc::new(rusty_js_runtime::value::JsString::from(
                WRAP_HEAD,
            ))),
        );
        rt.object_set(
            arr,
            "1".into(),
            Value::String(std::rc::Rc::new(rusty_js_runtime::value::JsString::from(
                WRAP_TAIL,
            ))),
        );
        rt.object_set(ns, "wrapper".into(), Value::Object(arr));
    }
    register_method(rt, ns, "wrap", |_rt, args| {
        let script = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            Some(other) => rusty_js_runtime::abstract_ops::to_string(other)
                .as_str()
                .to_string(),
            None => String::new(),
        };
        Ok(Value::String(std::rc::Rc::new(
            rusty_js_runtime::value::JsString::from(format!("{WRAP_HEAD}{script}{WRAP_TAIL}")),
        )))
    });
    {
        let c = make_callable(rt, "SourceMap", |rt, _a| Ok(rt.current_this()));
        rt.object_set(ns, "SourceMap".into(), Value::Object(c));
    }
    {
        let o = new_object(rt);
        rt.object_set(ns, "_cache".into(), Value::Object(o));
    }
    {
        let o = new_object(rt);
        register_method(rt, o, ".js", |_rt, _a| Ok(Value::Undefined));
        register_method(rt, o, ".json", |_rt, _a| Ok(Value::Undefined));
        register_method(rt, o, ".node", |_rt, _a| Ok(Value::Undefined));
        rt.object_set(ns, "_extensions".into(), Value::Object(o));
    }
    {
        let o = new_object(rt);
        rt.object_set(ns, "_pathCache".into(), Value::Object(o));
    }
    {
        let o = new_object(rt);
        rt.object_set(ns, "constants".into(), Value::Object(o));
    }
    register_method(rt, ns, "_findPath", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, ns, "_initPaths", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, ns, "_nodeModulePaths", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, ns, "_preloadModules", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, ns, "_resolveLookupPaths", |_rt, _a| {
        Ok(Value::Undefined)
    });
    register_method(rt, ns, "enableCompileCache", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, ns, "findPackageJSON", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, ns, "findSourceMap", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, ns, "flushCompileCache", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, ns, "getCompileCacheDir", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, ns, "getSourceMapsSupport", |_rt, _a| {
        Ok(Value::Undefined)
    });

    register_method(rt, ns, "isBuiltin", |_rt, a| {
        let name = match a.first() {
            Some(Value::String(s)) => s.to_string(),
            _ => return Ok(Value::Boolean(false)),
        };
        let had_prefix = name.starts_with("node:");
        let bare = name.strip_prefix("node:").unwrap_or(&name);

        let is_builtin = NODE_BUILTIN_MODULES.contains(&bare)
            || (had_prefix && NODE_PREFIX_ONLY_BUILTINS.contains(&bare));
        Ok(Value::Boolean(is_builtin))
    });
    register_method(rt, ns, "register", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, ns, "registerHooks", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, ns, "runMain", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, ns, "setSourceMapsSupport", |_rt, _a| {
        Ok(Value::Undefined)
    });
    register_method(rt, ns, "stripTypeScriptTypes", |_rt, _a| {
        Ok(Value::Undefined)
    });
    register_method(rt, ns, "syncBuiltinESMExports", |_rt, _a| {
        Ok(Value::Undefined)
    });
    {
        let ga = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
        rt.object_set(ns, "globalPaths".into(), Value::Object(ga));
    }
    rt.define_global_property("module", Value::Object(ns));
    rt.define_global_property("__node_module", Value::Object(ns));
}

pub fn install_all(rt: &mut Runtime) {
    install_all_eager(rt);
    install_constants(rt);
    install_all_deferrable(rt);
}

pub fn install_all_eager(rt: &mut Runtime) {
    rt.register_lazy_host_module_hidden(&["string_decoder"], |rt| install_string_decoder(rt));
    install_buffer(rt);
    install_buffer_canonical(rt);
    install_module(rt);
    install_global_require(rt);
    install_safe_regex2_compat(rt);
    install_performance(rt);
    install_dom_exception(rt);
}

pub fn install_all_deferrable(rt: &mut Runtime) {
    install_child_process(rt);
    install_tls(rt);
    install_readline(rt);
    install_worker_threads(rt);
    rt.register_lazy_host_module(&["cluster"], |rt| install_cluster(rt));
    rt.register_lazy_host_module(&["repl"], |rt| install_repl(rt));
    rt.register_lazy_host_module(&["trace_events"], |rt| install_trace_events(rt));
    install_dns(rt);
    install_http2(rt);
    rt.register_lazy_host_module(&["diagnostics_channel"], |rt| install_diagnostics_channel(rt));
    install_v8(rt);
    rt.register_lazy_host_module(&["inspector"], |rt| install_inspector(rt));
    install_vm(rt);
    install_punycode(rt);
    install_async_hooks(rt);
    install_internal_event_target(rt);
    install_internal_webstreams_util(rt);
    install_domain(rt);
}

pub fn install_internal_event_target(rt: &mut Runtime) {
    let ns = new_object(rt);
    for name in ["Event", "EventTarget", "CustomEvent"] {
        let value = rt.global_get(name);
        if !matches!(value, Value::Undefined) {
            rt.object_set(ns, name.into(), value);
        }
    }
    rt.object_set(
        ns,
        "kWeakHandler".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "__cruft_kWeakHandler",
        ))),
    );
    rt.object_set(ns, "NodeEventTarget".into(), rt.global_get("EventTarget"));
    rt.define_global_property("__cruft_internal_event_target", Value::Object(ns));
}

pub fn install_internal_webstreams_util(rt: &mut Runtime) {
    let ns = new_object(rt);
    rt.object_set(
        ns,
        "kState".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from("kState"))),
    );
    rt.define_global_property("__cruft_internal_webstreams_util", Value::Object(ns));
}

pub fn install_domain(rt: &mut Runtime) {
    let ns = new_object(rt);
    register_method(rt, ns, "create", |rt, _a| {
        let d = rt.alloc_object(RtObject::new_ordinary());
        rt.obj_mut(d)
            .set_own_internal("__domain_handle__".into(), Value::Boolean(true));
        register_method(rt, d, "run", |rt, args| {
            let cb = args.first().cloned().unwrap_or(Value::Undefined);
            rt.call_function(cb, Value::Undefined, Vec::new())
        });
        register_method(rt, d, "add", |rt, _a| Ok(rt.current_this()));
        register_method(rt, d, "remove", |rt, _a| Ok(rt.current_this()));
        register_method(rt, d, "bind", |rt, args| {
            Ok(args.first().cloned().unwrap_or(rt.current_this()))
        });
        register_method(rt, d, "intercept", |rt, args| {
            Ok(args.first().cloned().unwrap_or(rt.current_this()))
        });
        register_method(rt, d, "on", |rt, _a| Ok(rt.current_this()));
        register_method(rt, d, "once", |rt, _a| Ok(rt.current_this()));
        register_method(rt, d, "emit", |_rt, _a| Ok(Value::Boolean(false)));
        register_method(rt, d, "enter", |_rt, _a| Ok(Value::Undefined));
        register_method(rt, d, "exit", |_rt, _a| Ok(Value::Undefined));
        register_method(rt, d, "dispose", |_rt, _a| Ok(Value::Undefined));
        let arr = rt.alloc_object(RtObject::new_array());
        rt.object_set(d, "members".into(), Value::Object(arr));
        Ok(Value::Object(d))
    });
    rt.object_set(ns, "active".into(), Value::Null);
    let dom_class = make_callable(rt, "Domain", |rt, _a| Ok(rt.current_this()));
    {
        let pr = new_object(rt);
        rt.object_set(pr, "constructor".into(), Value::Object(dom_class));
        rt.object_set(dom_class, "prototype".into(), Value::Object(pr));
    }
    rt.object_set(ns, "Domain".into(), Value::Object(dom_class));
    {
        let st = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
        rt.object_set(ns, "_stack".into(), Value::Object(st));
    }
    {
        let cr = rt.object_get(ns, "create");
        rt.object_set(ns, "createDomain".into(), cr);
    }
    rt.define_global_property("domain", Value::Object(ns));
}

pub fn install_dom_exception(rt: &mut Runtime) {
    let proto = new_object(rt);
    crate::register::set_constant(
        rt,
        proto,
        "name",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from("Error"))),
    );
    crate::register::set_constant(
        rt,
        proto,
        "message",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(""))),
    );
    crate::register::set_constant(rt, proto, "code", Value::Number(0.0));
    let ctor = make_callable(rt, "DOMException", move |rt, args| {
        let mut obj = RtObject::new_ordinary();
        obj.proto = Some(proto);
        let inst = rt.alloc_object(obj);
        let msg = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            Some(v) => rusty_js_runtime::abstract_ops::to_string(v)
                .as_str()
                .to_string(),
            None => String::new(),
        };
        let name = match args.get(1) {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => "Error".into(),
        };
        rt.object_set(
            inst,
            "message".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(msg))),
        );
        rt.object_set(
            inst,
            "name".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                name.clone(),
            ))),
        );

        let code = match name.as_str() {
            "IndexSizeError" => 1.0,
            "HierarchyRequestError" => 3.0,
            "WrongDocumentError" => 4.0,
            "InvalidCharacterError" => 5.0,
            "NoModificationAllowedError" => 7.0,
            "NotFoundError" => 8.0,
            "NotSupportedError" => 9.0,
            "InUseAttributeError" => 10.0,
            "InvalidStateError" => 11.0,
            "SyntaxError" => 12.0,
            "InvalidModificationError" => 13.0,
            "NamespaceError" => 14.0,
            "InvalidAccessError" => 15.0,
            "TypeMismatchError" => 17.0,
            "SecurityError" => 18.0,
            "NetworkError" => 19.0,
            "AbortError" => 20.0,
            "URLMismatchError" => 21.0,
            "QuotaExceededError" => 22.0,
            "TimeoutError" => 23.0,
            "InvalidNodeTypeError" => 24.0,
            "DataCloneError" => 25.0,
            _ => 0.0,
        };
        rt.object_set(inst, "code".into(), Value::Number(code));
        rt.object_set(
            inst,
            "stack".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(""))),
        );
        Ok(Value::Object(inst))
    });
    rt.set_own_frozen_property(ctor, "prototype".into(), Value::Object(proto));
    rt.obj_mut(proto)
        .set_own_internal("constructor".into(), Value::Object(ctor));

    for (name, code) in &[
        ("INDEX_SIZE_ERR", 1),
        ("HIERARCHY_REQUEST_ERR", 3),
        ("WRONG_DOCUMENT_ERR", 4),
        ("INVALID_CHARACTER_ERR", 5),
        ("NO_MODIFICATION_ALLOWED_ERR", 7),
        ("NOT_FOUND_ERR", 8),
        ("NOT_SUPPORTED_ERR", 9),
        ("INUSE_ATTRIBUTE_ERR", 10),
        ("INVALID_STATE_ERR", 11),
        ("SYNTAX_ERR", 12),
        ("INVALID_MODIFICATION_ERR", 13),
        ("NAMESPACE_ERR", 14),
        ("INVALID_ACCESS_ERR", 15),
        ("SECURITY_ERR", 18),
        ("NETWORK_ERR", 19),
        ("ABORT_ERR", 20),
        ("URL_MISMATCH_ERR", 21),
        ("QUOTA_EXCEEDED_ERR", 22),
        ("TIMEOUT_ERR", 23),
        ("INVALID_NODE_TYPE_ERR", 24),
        ("DATA_CLONE_ERR", 25),
    ] {
        crate::register::set_constant(rt, ctor, name, Value::Number(*code as f64));
        crate::register::set_constant(rt, proto, name, Value::Number(*code as f64));
    }
    rt.define_global_property("DOMException", Value::Object(ctor));
}

fn value_as_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.as_str().to_string()),
        _ => None,
    }
}

fn perf_string_arg(args: &[Value], index: usize, default: &str) -> String {
    args.get(index)
        .and_then(value_as_string)
        .unwrap_or_else(|| default.to_string())
}

fn perf_entries_store(
    rt: &mut Runtime,
    perf: rusty_js_runtime::ObjectRef,
) -> rusty_js_runtime::ObjectRef {
    match rt.object_get(perf, "__perf_entries") {
        Value::Object(entries) => entries,
        _ => {
            let entries = rt.alloc_object(RtObject::new_array());
            rt.object_set(entries, "length".into(), Value::Number(0.0));
            rt.obj_mut(perf)
                .set_own_internal("__perf_entries".into(), Value::Object(entries));
            entries
        }
    }
}

fn perf_make_entry(
    rt: &mut Runtime,
    name: &str,
    entry_type: &str,
    start_time: f64,
    duration: f64,
) -> rusty_js_runtime::ObjectRef {
    let entry = rt.alloc_object(RtObject::new_ordinary());
    rt.object_set(
        entry,
        "name".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(name))),
    );
    rt.object_set(
        entry,
        "entryType".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(entry_type))),
    );
    rt.object_set(entry, "startTime".into(), Value::Number(start_time));
    rt.object_set(entry, "duration".into(), Value::Number(duration));
    entry
}

fn perf_push_entry(
    rt: &mut Runtime,
    perf: rusty_js_runtime::ObjectRef,
    entry: rusty_js_runtime::ObjectRef,
    before_index: Option<usize>,
) {
    let entries = perf_entries_store(rt, perf);
    let len = rt.array_length(entries);
    let insert_at = before_index.unwrap_or(len).min(len);
    for i in (insert_at..len).rev() {
        let v = rt.object_get(entries, &i.to_string());
        rt.object_set(entries, (i + 1).to_string(), v);
    }
    rt.object_set(entries, insert_at.to_string(), Value::Object(entry));
    rt.object_set(entries, "length".into(), Value::Number((len + 1) as f64));
}

fn perf_find_mark_index(
    rt: &mut Runtime,
    perf: rusty_js_runtime::ObjectRef,
    name: &str,
) -> Option<usize> {
    let entries = perf_entries_store(rt, perf);
    let len = rt.array_length(entries);
    for i in 0..len {
        let Value::Object(entry) = rt.object_get(entries, &i.to_string()) else {
            continue;
        };
        if matches!(rt.object_get(entry, "entryType"), Value::String(ref s) if s.as_str() == "mark")
            && matches!(rt.object_get(entry, "name"), Value::String(ref s) if s.as_str() == name)
        {
            return Some(i);
        }
    }
    None
}

fn perf_entries_array(
    rt: &mut Runtime,
    perf: rusty_js_runtime::ObjectRef,
    entry_type: Option<&str>,
    name: Option<&str>,
) -> rusty_js_runtime::ObjectRef {
    let entries = perf_entries_store(rt, perf);
    let out = rt.alloc_object(RtObject::new_array());
    let mut out_len = 0usize;
    let len = rt.array_length(entries);
    for i in 0..len {
        let Value::Object(entry) = rt.object_get(entries, &i.to_string()) else {
            continue;
        };
        if let Some(expected) = entry_type {
            if !matches!(rt.object_get(entry, "entryType"), Value::String(ref s) if s.as_str() == expected)
            {
                continue;
            }
        }
        if let Some(expected) = name {
            if !matches!(rt.object_get(entry, "name"), Value::String(ref s) if s.as_str() == expected)
            {
                continue;
            }
        }
        rt.object_set(out, out_len.to_string(), Value::Object(entry));
        out_len += 1;
    }
    rt.object_set(out, "length".into(), Value::Number(out_len as f64));
    out
}

fn perf_filter_entries(
    rt: &mut Runtime,
    perf: rusty_js_runtime::ObjectRef,
    entry_type: Option<&str>,
    name: Option<&str>,
) {
    let keep = perf_entries_array(rt, perf, None, None);
    let entries = perf_entries_store(rt, perf);
    let mut out_len = 0usize;
    let len = rt.array_length(keep);
    for i in 0..len {
        let Value::Object(entry) = rt.object_get(keep, &i.to_string()) else {
            continue;
        };
        let type_match = entry_type
            .map(|expected| matches!(rt.object_get(entry, "entryType"), Value::String(ref s) if s.as_str() == expected))
            .unwrap_or(false);
        let name_match = name
            .map(|expected| matches!(rt.object_get(entry, "name"), Value::String(ref s) if s.as_str() == expected))
            .unwrap_or(true);
        if type_match && name_match {
            continue;
        }
        rt.object_set(entries, out_len.to_string(), Value::Object(entry));
        out_len += 1;
    }
    rt.object_set(entries, "length".into(), Value::Number(out_len as f64));
}

fn perf_notify_observers(
    rt: &mut Runtime,
    perf: rusty_js_runtime::ObjectRef,
    entry: rusty_js_runtime::ObjectRef,
) -> Result<(), RuntimeError> {
    let observers = match rt.object_get(perf, "__perf_observers") {
        Value::Object(observers) => observers,
        _ => return Ok(()),
    };
    let entry_type = match rt.object_get(entry, "entryType") {
        Value::String(s) => s.as_str().to_string(),
        _ => String::new(),
    };
    let len = rt.array_length(observers);
    for i in 0..len {
        let Value::Object(observer) = rt.object_get(observers, &i.to_string()) else {
            continue;
        };

        let subscribed = match rt.object_get(observer, "__perf_observe_types") {
            Value::String(s) => s.as_str().split(',').any(|t| t == entry_type),
            _ => {
                entry_type == "measure"
                    && matches!(
                        rt.object_get(observer, "__perf_observe_measure"),
                        Value::Boolean(true)
                    )
            }
        };
        if !subscribed {
            continue;
        }
        let callback = rt.object_get(observer, "__perf_callback");
        let Value::Object(callback) = callback else {
            continue;
        };
        let list = rt.alloc_object(RtObject::new_ordinary());
        let records = rt.alloc_object(RtObject::new_array());
        rt.object_set(records, "0".into(), Value::Object(entry));
        rt.object_set(records, "length".into(), Value::Number(1.0));
        rt.obj_mut(list)
            .set_own_internal("__perf_records".into(), Value::Object(records));
        register_method(rt, list, "getEntries", |rt, _args| {
            let records = match rt.current_this() {
                Value::Object(this) => rt.object_get(this, "__perf_records"),
                _ => Value::Undefined,
            };
            match records {
                Value::Object(records) => Ok(Value::Object(records)),
                _ => {
                    let empty = rt.alloc_object(RtObject::new_array());
                    rt.object_set(empty, "length".into(), Value::Number(0.0));
                    Ok(Value::Object(empty))
                }
            }
        });
        rt.call_function(
            Value::Object(callback),
            Value::Undefined,
            vec![Value::Object(list)],
        )?;
    }
    Ok(())
}

pub fn install_performance(rt: &mut Runtime) {
    let perf_time_origin = {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs_f64() * 1000.0)
            .unwrap_or(0.0)
    };
    let perf_started = std::time::Instant::now();
    let perf = new_object(rt);
    let entries = rt.alloc_object(RtObject::new_array());
    rt.object_set(entries, "length".into(), Value::Number(0.0));
    rt.obj_mut(perf)
        .set_own_internal("__perf_entries".into(), Value::Object(entries));
    let observers = rt.alloc_object(RtObject::new_array());
    rt.object_set(observers, "length".into(), Value::Number(0.0));
    rt.obj_mut(perf)
        .set_own_internal("__perf_observers".into(), Value::Object(observers));
    register_method(rt, perf, "now", move |rt, _a| {
        check_clock_ns(rt, caps::ClockOp::HighResolution)?;
        Ok(Value::Number(perf_started.elapsed().as_secs_f64() * 1000.0))
    });
    crate::register::set_constant(rt, perf, "timeOrigin", Value::Number(perf_time_origin));

    register_method(rt, perf, "timerify", move |rt, args| {
        let fn_v = args.first().cloned().unwrap_or(Value::Undefined);
        if !rt.is_callable(&fn_v) {
            return Err(node_code_type_error(
                rt,
                "ERR_INVALID_ARG_TYPE",
                "The \"fn\" argument must be of type function.",
            ));
        }
        let fn_name = match &fn_v {
            Value::Object(id) => match rt.object_get(*id, "name") {
                Value::String(s) => s.as_str().to_string(),
                _ => String::new(),
            },
            _ => String::new(),
        };
        let roots = match &fn_v {
            Value::Object(id) => vec![*id, perf],
            _ => vec![perf],
        };
        let inner_name = fn_name.clone();
        let wrapper = make_callable_rooted(rt, &fn_name, roots, move |rt, wargs| {
            let this = rt.current_this();
            let start = perf_started.elapsed().as_secs_f64() * 1000.0;
            let ret = rt.call_function(fn_v.clone(), this, wargs.to_vec())?;
            let end = perf_started.elapsed().as_secs_f64() * 1000.0;
            let entry = perf_make_entry(rt, &inner_name, "function", start, end - start);
            perf_push_entry(rt, perf, entry, None);
            let _ = perf_notify_observers(rt, perf, entry);
            Ok(ret)
        });
        Ok(Value::Object(wrapper))
    });
    register_method(rt, perf, "mark", move |rt, args| {
        let name = perf_string_arg(args, 0, "undefined");
        let entry = perf_make_entry(rt, &name, "mark", 0.0, 0.0);

        if let Some(Value::Object(opts)) = args.get(1) {
            let detail = rt.object_get(*opts, "detail");
            if !matches!(detail, Value::Undefined) {
                rt.object_set(entry, "detail".into(), detail);
            }
        }
        perf_push_entry(rt, perf, entry, None);

        perf_notify_observers(rt, perf, entry)?;
        Ok(Value::Object(entry))
    });
    register_method(rt, perf, "measure", move |rt, args| {
        let name = perf_string_arg(args, 0, "undefined");
        let start_name = args.get(1).and_then(value_as_string);
        let entry = perf_make_entry(rt, &name, "measure", 0.0, 0.0);
        let before = start_name
            .as_deref()
            .and_then(|_| args.get(2).and_then(value_as_string))
            .and_then(|end| perf_find_mark_index(rt, perf, &end));
        perf_push_entry(rt, perf, entry, before);
        perf_notify_observers(rt, perf, entry)?;
        Ok(Value::Object(entry))
    });
    register_method(rt, perf, "clearMarks", move |rt, args| {
        let name = args.first().and_then(value_as_string);
        perf_filter_entries(rt, perf, Some("mark"), name.as_deref());
        Ok(Value::Undefined)
    });
    register_method(rt, perf, "clearMeasures", move |rt, args| {
        let name = args.first().and_then(value_as_string);
        perf_filter_entries(rt, perf, Some("measure"), name.as_deref());
        Ok(Value::Undefined)
    });
    register_method(rt, perf, "getEntries", move |rt, _a| {
        Ok(Value::Object(perf_entries_array(rt, perf, None, None)))
    });
    register_method(rt, perf, "getEntriesByName", move |rt, args| {
        let name = args.first().and_then(value_as_string);
        Ok(Value::Object(perf_entries_array(
            rt,
            perf,
            None,
            name.as_deref(),
        )))
    });
    register_method(rt, perf, "getEntriesByType", move |rt, args| {
        let ty = args.first().and_then(value_as_string);
        Ok(Value::Object(perf_entries_array(
            rt,
            perf,
            ty.as_deref(),
            None,
        )))
    });
    register_method(rt, perf, "markResourceTiming", |_rt, _a| {
        Ok(Value::Undefined)
    });
    register_method(rt, perf, "clearResourceTimings", |_rt, _a| {
        Ok(Value::Undefined)
    });
    register_method(rt, perf, "eventLoopUtilization", |rt, _a| {
        let o = new_object(rt);
        rt.object_set(o, "idle".into(), Value::Number(0.0));
        rt.object_set(o, "active".into(), Value::Number(0.0));
        rt.object_set(o, "utilization".into(), Value::Number(0.0));
        Ok(Value::Object(o))
    });

    {
        let s = |txt: &str| Value::String(Rc::new(rusty_js_runtime::value::JsString::from(txt)));
        let nt = new_object(rt);
        rt.object_set(nt, "name".into(), s("node"));
        rt.object_set(nt, "entryType".into(), s("node"));
        rt.object_set(nt, "startTime".into(), Value::Number(0.0));
        rt.object_set(nt, "duration".into(), Value::Number(0.0));
        for k in [
            "nodeStart",
            "v8Start",
            "bootstrapComplete",
            "environment",
            "loopStart",
            "idleTime",
        ] {
            rt.object_set(nt, k.into(), Value::Number(0.0));
        }
        rt.object_set(nt, "loopExit".into(), Value::Number(-1.0));
        let uv = new_object(rt);
        rt.object_set(uv, "loopCount".into(), Value::Number(0.0));
        rt.object_set(uv, "events".into(), Value::Number(0.0));
        rt.object_set(uv, "eventsWaiting".into(), Value::Number(0.0));
        rt.object_set(nt, "uvMetricsInfo".into(), Value::Object(uv));

        let to_json = crate::register::make_callable(rt, "toJSON", |rt, _a| Ok(rt.current_this()));
        rt.obj_mut(nt)
            .set_own_internal("toJSON".into(), Value::Object(to_json));
        rt.object_set(perf, "nodeTiming".into(), Value::Object(nt));
    }

    rt.obj_mut(perf).dict_mut().insert(
        PropertyKey::String("@@toStringTag".into()),
        PropertyDescriptor {
            value: Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                "Performance".to_string(),
            ))),
            writable: false,
            enumerable: false,
            configurable: true,
            getter: None,
            setter: None,
        },
    );
    rt.define_global_property("performance", Value::Object(perf));

    let po_ctor = make_callable(rt, "PerformanceObserver", move |rt, args| {
        let inst = rt.alloc_object(RtObject::new_ordinary());
        if let Some(Value::Object(callback)) = args.first() {
            rt.obj_mut(inst)
                .set_own_internal("__perf_callback".into(), Value::Object(*callback));
        }
        if let Value::Object(observers) = rt.object_get(perf, "__perf_observers") {
            let len = rt.array_length(observers);
            rt.object_set(observers, len.to_string(), Value::Object(inst));
            rt.object_set(observers, "length".into(), Value::Number((len + 1) as f64));
        }
        register_method(rt, inst, "observe", |rt, args| {
            let mut types: Vec<String> = Vec::new();
            if let Some(Value::Object(options)) = args.first() {
                if let Value::Object(arr) = rt.object_get(*options, "entryTypes") {
                    let len = rt.array_length(arr);
                    for i in 0..len {
                        if let Value::String(s) = rt.object_get(arr, &i.to_string()) {
                            types.push(s.as_str().to_string());
                        }
                    }
                }

                if let Value::String(s) = rt.object_get(*options, "type") {
                    types.push(s.as_str().to_string());
                }
            }
            if let Value::Object(this) = rt.current_this() {
                let joined = types.join(",");
                rt.obj_mut(this).set_own_internal(
                    "__perf_observe_types".into(),
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(joined))),
                );
                if types.iter().any(|t| t == "measure") {
                    rt.obj_mut(this)
                        .set_own_internal("__perf_observe_measure".into(), Value::Boolean(true));
                }
            }
            Ok(Value::Undefined)
        });
        register_method(rt, inst, "disconnect", |rt, _a| {

            if let Value::Object(this) = rt.current_this() {
                rt.obj_mut(this).set_own_internal(
                    "__perf_observe_types".into(),
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                        String::new(),
                    ))),
                );
                rt.obj_mut(this)
                    .set_own_internal("__perf_observe_measure".into(), Value::Boolean(false));
            }
            Ok(Value::Undefined)
        });
        register_method(rt, inst, "takeRecords", |rt, _a| {
            let arr = rt.alloc_object(RtObject::new_array());
            rt.object_set(arr, "length".into(), Value::Number(0.0));
            Ok(Value::Object(arr))
        });
        Ok(Value::Object(inst))
    });
    let po_proto = new_object(rt);
    rt.set_own_frozen_property(po_ctor, "prototype".into(), Value::Object(po_proto));
    rt.obj_mut(po_proto)
        .set_own_internal("constructor".into(), Value::Object(po_ctor));
    let st_arr = rt.alloc_object(RtObject::new_array());
    for (i, t) in ["mark", "measure", "resource", "navigation", "function"]
        .iter()
        .enumerate()
    {
        rt.object_set(
            st_arr,
            i.to_string(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from((*t)))),
        );
    }
    rt.object_set(st_arr, "length".into(), Value::Number(5.0));
    rt.object_set(po_ctor, "supportedEntryTypes".into(), Value::Object(st_arr));
    rt.define_global_property("PerformanceObserver", Value::Object(po_ctor));

    let ph = new_object(rt);
    let _ph_root = rt.push_temporary_value_roots(&[Value::Object(ph)]);
    rt.object_set(ph, "performance".into(), Value::Object(perf));
    rt.object_set(ph, "PerformanceObserver".into(), Value::Object(po_ctor));
    register_method(rt, ph, "monitorEventLoopDelay", |rt, _a| {
        let h = rt.alloc_object(RtObject::new_ordinary());
        let _h_root = rt.push_temporary_value_roots(&[Value::Object(h)]);
        rt.obj_mut(h).set_own_internal(
            "__perf_event_loop_delay_monitor__".into(),
            Value::Boolean(true),
        );
        register_method(rt, h, "enable", |_rt, _a| Ok(Value::Undefined));
        register_method(rt, h, "disable", |_rt, _a| Ok(Value::Undefined));
        register_method(rt, h, "reset", |_rt, _a| Ok(Value::Undefined));
        register_method(rt, h, "percentile", |_rt, _a| Ok(Value::Number(0.0)));
        crate::register::set_constant(rt, h, "min", Value::Number(0.0));
        crate::register::set_constant(rt, h, "max", Value::Number(0.0));
        crate::register::set_constant(rt, h, "mean", Value::Number(0.0));
        Ok(Value::Object(h))
    });
    register_method(rt, ph, "createHistogram", |rt, _a| {
        let h = rt.alloc_object(RtObject::new_ordinary());
        rt.obj_mut(h)
            .set_own_internal("__perf_histogram__".into(), Value::Boolean(true));
        register_method(rt, h, "record", |_rt, _a| Ok(Value::Undefined));
        register_method(rt, h, "reset", |_rt, _a| Ok(Value::Undefined));
        Ok(Value::Object(h))
    });
    {
        let c = make_callable(rt, "Performance", |rt, _a| Ok(rt.current_this()));
        rt.object_set(ph, "Performance".into(), Value::Object(c));
    }
    {
        let c = make_callable(rt, "PerformanceEntry", |rt, _a| Ok(rt.current_this()));
        rt.object_set(ph, "PerformanceEntry".into(), Value::Object(c));
    }
    {
        let c = make_callable(rt, "PerformanceMark", |rt, _a| Ok(rt.current_this()));
        rt.object_set(ph, "PerformanceMark".into(), Value::Object(c));
    }
    {
        let c = make_callable(rt, "PerformanceMeasure", |rt, _a| Ok(rt.current_this()));
        rt.object_set(ph, "PerformanceMeasure".into(), Value::Object(c));
    }
    {
        let c = make_callable(rt, "PerformanceObserverEntryList", |rt, _a| {
            Ok(rt.current_this())
        });
        rt.object_set(ph, "PerformanceObserverEntryList".into(), Value::Object(c));
    }
    {
        let c = make_callable(rt, "PerformanceResourceTiming", |rt, _a| {
            Ok(rt.current_this())
        });
        rt.object_set(ph, "PerformanceResourceTiming".into(), Value::Object(c));
    }
    {
        let o = new_object(rt);
        rt.object_set(ph, "constants".into(), Value::Object(o));
    }
    register_method(rt, ph, "eventLoopUtilization", |_rt, _a| {
        Ok(Value::Undefined)
    });
    register_method(rt, ph, "timerify", |_rt, _a| Ok(Value::Undefined));
    rt.define_global_property("perf_hooks", Value::Object(ph));
}

pub fn install_async_hooks(rt: &mut Runtime) {
    let ns = new_object(rt);
    rt.async_resource_destroy_callback = Some(async_hooks_emit_destroy_id_for_global);
    rt.object_set(ns, "__next_async_id__".into(), Value::Number(2.0));
    rt.object_set(ns, "__execution_async_id__".into(), Value::Number(0.0));
    let root_execution_resource = new_object(rt);
    rt.object_set(
        ns,
        "__execution_async_resource__".into(),
        Value::Object(root_execution_resource),
    );
    rt.object_set(ns, "__enabled_hook_count__".into(), Value::Number(0.0));
    let hooks = rt.alloc_object(RtObject::new_array());
    rt.object_set(hooks, "length".into(), Value::Number(0.0));
    rt.object_set(ns, "__async_hooks_hooks__".into(), Value::Object(hooks));

    let ar_proto = new_object(rt);
    let ar_ctor = make_callable_with_length_rooted(
        rt,
        "AsyncResource",
        1,
        vec![ar_proto, ns],
        move |rt, args| {
            let inst = async_resource_construct(rt, ar_proto, ns, args)?;
            Ok(Value::Object(inst))
        },
    );
    rt.set_own_frozen_property(ar_ctor, "prototype".into(), Value::Object(ar_proto));
    rt.obj_mut(ar_proto)
        .set_own_internal("constructor".into(), Value::Object(ar_ctor));
    let ns_for_run = ns;
    register_method(rt, ar_proto, "runInAsyncScope", move |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let cb = args.first().cloned().unwrap_or(Value::Undefined);
        if !rt.is_callable(&cb) {
            return Err(node_code_type_error(
                rt,
                "ERR_INVALID_ARG_TYPE",
                "The \"fn\" argument must be of type function.",
            ));
        }
        let this_arg = args.get(1).cloned().unwrap_or(Value::Undefined);
        let cb_args: Vec<Value> = args.iter().skip(2).cloned().collect();
        let id = async_resource_id(rt, this);
        async_hooks_call_with_execution_resource(rt, ns_for_run, this, id, cb, this_arg, cb_args)
    });
    register_method(rt, ar_proto, "emitDestroy", |rt, _a| {
        let this = rt.current_this();
        if let Value::Object(id) = this {
            if matches!(
                rt.object_get(id, "__async_destroy_emitted__"),
                Value::Boolean(true)
            ) {
                return Ok(Value::Object(id));
            }
            let async_id = async_resource_id(rt, id);
            rt.object_set(id, "__async_destroy_emitted__".into(), Value::Boolean(true));
            rt.unregister_async_resource_destroy_cell(async_id);
            async_hooks_emit_destroy_for_global(rt, Value::Object(id))?;
            Ok(Value::Object(id))
        } else {
            Ok(this)
        }
    });
    register_method(rt, ar_proto, "asyncId", |rt, _a| {
        let id = match rt.current_this() {
            Value::Object(id) => async_resource_id(rt, id),
            _ => 0.0,
        };
        Ok(Value::Number(id))
    });
    register_method(rt, ar_proto, "triggerAsyncId", |rt, _a| {
        let id = match rt.current_this() {
            Value::Object(id) => match rt.object_get(id, "__trigger_async_id__") {
                Value::Number(n) => n,
                _ => 0.0,
            },
            _ => 0.0,
        };
        Ok(Value::Number(id))
    });
    let ns_for_bind = ns;
    register_method(rt, ar_proto, "bind", move |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let cb = args.first().cloned().unwrap_or(Value::Undefined);
        let this_arg = args.get(1).cloned().unwrap_or(Value::Undefined);
        async_resource_bound_function(rt, ns_for_bind, this, cb, this_arg)
    });
    let ns_for_static_bind = ns;
    let ar_proto_for_static_bind = ar_proto;
    let static_bind =
        make_callable_with_length_rooted(rt, "bind", 1, vec![ns, ar_proto], move |rt, args| {
            let cb = args.first().cloned().unwrap_or(Value::Undefined);
            let this_arg = args.get(2).cloned().unwrap_or(Value::Undefined);
            let resource_type = js_string_value("bound".to_string());
            let resource = async_resource_construct(
                rt,
                ar_proto_for_static_bind,
                ns_for_static_bind,
                std::slice::from_ref(&resource_type),
            )?;
            async_resource_bound_function(rt, ns_for_static_bind, resource, cb, this_arg)
        });
    rt.object_set(ar_ctor, "bind".into(), Value::Object(static_bind));
    rt.object_set(ns, "AsyncResource".into(), Value::Object(ar_ctor));

    let als_proto = new_object(rt);
    let als_proto_for_ctor = als_proto;
    let als_ctor = make_callable(rt, "AsyncLocalStorage", move |rt, args| {
        if matches!(args.first(), Some(Value::Null)) {
            return Err(node_code_type_error(
                rt,
                "ERR_INVALID_ARG_TYPE",
                "The \"options\" argument must be of type object.",
            ));
        }

        let inst = match rt.current_this() {
            Value::Object(this) => this,
            _ => {
                let i = rt.alloc_object(RtObject::new_ordinary());
                rt.set_object_prototype_internal(i, Some(als_proto_for_ctor));
                i
            }
        };
        let mut default_value = Value::Undefined;
        let mut name_value = js_string_value(String::new());
        if let Some(Value::Object(options)) = args.first() {
            default_value = rt.object_get(*options, "defaultValue");
            let name = rt.object_get(*options, "name");
            if !matches!(name, Value::Undefined) {
                name_value = js_string_value(abstract_ops::to_string(&name).as_str().to_string());
            }
        }
        rt.object_set(inst, "__als_default__".into(), default_value);
        rt.object_set(inst, "name".into(), name_value);
        Ok(Value::Object(inst))
    });

    register_method(rt, als_proto, "getStore", |rt, _a| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        if rt.als_has_store(this) {
            Ok(rt.als_get_store(this))
        } else {
            Ok(rt.object_get(this, "__als_default__"))
        }
    });
    register_method(rt, als_proto, "run", |rt, args| {

        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let store = args.first().cloned().unwrap_or(Value::Undefined);
        let cb = args.get(1).cloned().unwrap_or(Value::Undefined);
        let cb_args: Vec<Value> = args.iter().skip(2).cloned().collect();
        let had_prev = rt.als_has_store(this);
        let prev = rt.als_get_store(this);
        rt.als_set_store(this, store);
        let result = rt.call_function(cb, Value::Undefined, cb_args);
        if had_prev {
            rt.als_set_store(this, prev);
        } else {
            rt.als_clear_store(this);
        }
        result
    });
    register_method(rt, als_proto, "enterWith", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let store = args.first().cloned().unwrap_or(Value::Undefined);
        rt.als_set_store(this, store);
        Ok(Value::Undefined)
    });
    register_method(rt, als_proto, "exit", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let cb = args.first().cloned().unwrap_or(Value::Undefined);
        let cb_args: Vec<Value> = args.iter().skip(1).cloned().collect();
        let had_prev = rt.als_has_store(this);
        let prev = rt.als_get_store(this);
        rt.als_clear_store(this);
        let result = rt.call_function(cb, Value::Undefined, cb_args);
        if had_prev {
            rt.als_set_store(this, prev);
        } else {
            rt.als_clear_store(this);
        }
        result
    });
    register_method(rt, als_proto, "withScope", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let store = args.first().cloned().unwrap_or(Value::Undefined);
        let had_prev = rt.als_has_store(this);
        let prev = rt.als_get_store(this);
        rt.als_set_store(this, store);

        let scope = rt.alloc_object(RtObject::new_ordinary());
        rt.object_set(scope, "__als_scope_storage".into(), Value::Object(this));
        rt.object_set(
            scope,
            "__als_scope_had_prev".into(),
            Value::Boolean(had_prev),
        );
        rt.object_set(scope, "__als_scope_prev".into(), prev);
        rt.object_set(scope, "__als_scope_disposed".into(), Value::Boolean(false));

        let dispose = make_callable(rt, "dispose", |rt, _args| {
            let scope = match rt.current_this() {
                Value::Object(id) => id,
                _ => return Ok(Value::Undefined),
            };
            if matches!(
                rt.object_get(scope, "__als_scope_disposed"),
                Value::Boolean(true)
            ) {
                return Ok(Value::Undefined);
            }
            rt.object_set(scope, "__als_scope_disposed".into(), Value::Boolean(true));
            let als_id = match rt.object_get(scope, "__als_scope_storage") {
                Value::Object(id) => id,
                _ => return Ok(Value::Undefined),
            };
            let had_prev = matches!(
                rt.object_get(scope, "__als_scope_had_prev"),
                Value::Boolean(true)
            );
            if had_prev {
                let prev = rt.object_get(scope, "__als_scope_prev");
                rt.als_set_store(als_id, prev);
            } else {
                rt.als_clear_store(als_id);
            }
            Ok(Value::Undefined)
        });
        rt.object_set(scope, "dispose".into(), Value::Object(dispose));
        rt.object_set(scope, "@@dispose".into(), Value::Object(dispose));
        Ok(Value::Object(scope))
    });
    register_method(rt, als_proto, "disable", |rt, _a| {
        if let Value::Object(id) = rt.current_this() {
            rt.als_clear_store(id);
        }
        Ok(Value::Undefined)
    });
    rt.set_own_frozen_property(als_ctor, "prototype".into(), Value::Object(als_proto));
    rt.obj_mut(als_proto)
        .set_own_internal("constructor".into(), Value::Object(als_ctor));
    let ns_for_als_bind = ns;
    let ar_proto_for_als_bind = ar_proto;
    let als_bind =
        make_callable_with_length_rooted(rt, "bind", 1, vec![ns, ar_proto], move |rt, args| {
            let cb = args.first().cloned().unwrap_or(Value::Undefined);
            let resource_type = js_string_value("bound".to_string());
            let resource = async_resource_construct(
                rt,
                ar_proto_for_als_bind,
                ns_for_als_bind,
                std::slice::from_ref(&resource_type),
            )?;
            async_resource_bound_function(rt, ns_for_als_bind, resource, cb, Value::Undefined)
        });
    rt.object_set(als_ctor, "bind".into(), Value::Object(als_bind));
    register_method(rt, als_ctor, "snapshot", |rt, _a| {
        let captured = rt.als_context_snapshot();
        let snapshot = make_callable(rt, "snapshot", move |rt, args| {
            let cb = args.first().cloned().unwrap_or(Value::Undefined);
            let cb_args: Vec<Value> = args.iter().skip(1).cloned().collect();
            let saved = rt.als_context_replace(captured.clone());
            let result = rt.call_function(cb, Value::Undefined, cb_args);
            rt.als_context_replace(saved);
            result
        });
        Ok(Value::Object(snapshot))
    });
    rt.object_set(ns, "AsyncLocalStorage".into(), Value::Object(als_ctor));

    let ns_for_execution = ns;
    register_method(rt, ns, "executionAsyncId", move |rt, _a| {
        Ok(
            match rt.object_get(ns_for_execution, "__execution_async_id__") {
                Value::Number(n) => Value::Number(n),
                _ => Value::Number(0.0),
            },
        )
    });
    let ns_for_trigger = ns;
    register_method(rt, ns, "triggerAsyncId", move |rt, _a| {
        Ok(
            match rt.object_get(ns_for_trigger, "__trigger_async_id__") {
                Value::Number(n) => Value::Number(n),
                _ => Value::Number(0.0),
            },
        )
    });
    register_method(rt, ns, "executionAsyncResource", |rt, _a| {
        match rt.global_get("async_hooks") {
            Value::Object(ns) => match rt.object_get(ns, "__execution_async_resource__") {
                Value::Object(resource) => Ok(Value::Object(resource)),
                _ => Ok(Value::Object(new_object(rt))),
            },
            _ => Ok(Value::Object(new_object(rt))),
        }
    });
    let ns_for_hook = ns;
    register_method(rt, ns, "createHook", move |rt, args| {
        if let Some(Value::Object(options)) = args.first() {
            for name in ["init", "before", "after", "destroy", "promiseResolve"] {
                let callback = rt.object_get(*options, name);
                if !matches!(callback, Value::Undefined) && !rt.is_callable(&callback) {
                    let msg = format!("hook.{name} must be a function");
                    return Err(node_code_type_error(rt, "ERR_ASYNC_CALLBACK", &msg));
                }
            }
        }
        let hook = rt.alloc_object(RtObject::new_ordinary());
        rt.object_set(hook, "__async_hooks_hook__".into(), Value::Boolean(true));
        rt.object_set(
            hook,
            "__async_hooks_enabled__".into(),
            Value::Boolean(false),
        );
        if let Some(Value::Object(options)) = args.first() {
            for (name, slot) in [
                ("init", "__async_hook_init__"),
                ("before", "__async_hook_before__"),
                ("after", "__async_hook_after__"),
                ("destroy", "__async_hook_destroy__"),
                ("promiseResolve", "__async_hook_promise_resolve__"),
            ] {
                rt.object_set(hook, slot.into(), rt.object_get(*options, name));
            }
        }
        if let Value::Object(hooks) = rt.object_get(ns_for_hook, "__async_hooks_hooks__") {
            let len = match rt.object_get(hooks, "length") {
                Value::Number(n) if n.is_finite() && n >= 0.0 => n as usize,
                _ => 0,
            };
            rt.object_set(hooks, len.to_string(), Value::Object(hook));
            rt.object_set(hooks, "length".into(), Value::Number((len + 1) as f64));
        }
        register_method(rt, hook, "enable", move |rt, _a| {
            let this = rt.current_this();
            if let Value::Object(hook) = this {
                if !matches!(
                    rt.object_get(hook, "__async_hooks_enabled__"),
                    Value::Boolean(true)
                ) {
                    rt.object_set(hook, "__async_hooks_enabled__".into(), Value::Boolean(true));
                    async_hooks_adjust_enabled(rt, ns_for_hook, 1.0);
                }
                Ok(Value::Object(hook))
            } else {
                Ok(this)
            }
        });
        let ns_for_disable = ns_for_hook;
        register_method(rt, hook, "disable", move |rt, _a| {
            let this = rt.current_this();
            if let Value::Object(hook) = this {
                if matches!(
                    rt.object_get(hook, "__async_hooks_enabled__"),
                    Value::Boolean(true)
                ) {
                    rt.object_set(
                        hook,
                        "__async_hooks_enabled__".into(),
                        Value::Boolean(false),
                    );
                    async_hooks_adjust_enabled(rt, ns_for_disable, -1.0);
                }
                Ok(Value::Object(hook))
            } else {
                Ok(this)
            }
        });
        Ok(Value::Object(hook))
    });

    {
        let o = new_object(rt);
        rt.object_set(ns, "asyncWrapProviders".into(), Value::Object(o));
    }
    let internal = new_object(rt);
    let symbols = new_object(rt);
    rt.object_set(
        symbols,
        "async_id_symbol".into(),
        Value::Symbol(Rc::new("nodejs.async_id_symbol".to_string())),
    );
    rt.object_set(
        symbols,
        "trigger_async_id_symbol".into(),
        Value::Symbol(Rc::new("nodejs.trigger_async_id_symbol".to_string())),
    );
    rt.object_set(internal, "symbols".into(), Value::Object(symbols));
    let ns_for_enabled = ns;
    register_method(rt, internal, "enabledHooksExist", move |rt, _a| {
        Ok(Value::Boolean(
            async_hooks_enabled_count(rt, ns_for_enabled) > 0.0,
        ))
    });
    rt.define_global_property("__cruft_internal_async_hooks", Value::Object(internal));
    rt.define_global_property("async_hooks", Value::Object(ns));
}

pub(crate) fn async_hooks_emit_init_for_global(
    rt: &mut Runtime,
    type_name: &str,
    resource: Value,
) -> Result<Option<f64>, RuntimeError> {
    let ns = match rt.global_get("async_hooks") {
        Value::Object(id) => id,
        _ => return Ok(None),
    };
    let async_id = async_hooks_next_id(rt, ns);
    let mut trigger = async_hooks_current_execution_id(rt, ns);
    if type_name == "Immediate" && matches!(trigger, Value::Number(0.0)) {
        trigger = Value::Number(1.0);
    }
    if let Value::Object(resource_id) = resource.clone() {

        rt.set_engine_sentinel(resource_id, "__async_id__", Value::Number(async_id));
        rt.set_engine_sentinel(resource_id, "__trigger_async_id__", trigger.clone());
    }
    async_hooks_emit_init_for_resource(rt, ns, async_id, type_name, trigger, resource)?;
    Ok(Some(async_id))
}

fn async_hooks_emit_init_for_resource(
    rt: &mut Runtime,
    ns: ObjectRef,
    async_id: f64,
    type_name: &str,
    trigger: Value,
    resource: Value,
) -> Result<(), RuntimeError> {
    if async_hooks_enabled_count(rt, ns) <= 0.0 {
        return Ok(());
    }
    let hooks = match rt.object_get(ns, "__async_hooks_hooks__") {
        Value::Object(id) => id,
        _ => return Ok(()),
    };
    let len = match rt.object_get(hooks, "length") {
        Value::Number(n) if n.is_finite() && n >= 0.0 => n as usize,
        _ => 0,
    };
    for idx in 0..len {
        let hook = match rt.object_get(hooks, idx.to_string().as_str()) {
            Value::Object(id) => id,
            _ => continue,
        };
        if !matches!(
            rt.object_get(hook, "__async_hooks_enabled__"),
            Value::Boolean(true)
        ) {
            continue;
        }
        let cb = rt.object_get(hook, "__async_hook_init__");
        if !rt.is_callable(&cb) {
            continue;
        }
        rt.call_function(
            cb,
            Value::Object(hook),
            vec![
                Value::Number(async_id),
                js_string_value(type_name.to_string()),
                trigger.clone(),
                resource.clone(),
            ],
        )?;
    }
    Ok(())
}

pub(crate) fn async_hooks_emit_destroy_for_global(
    rt: &mut Runtime,
    resource: Value,
) -> Result<(), RuntimeError> {
    let async_id = match resource {
        Value::Object(id) => match rt.object_get(id, "__async_id__") {
            Value::Number(n) if n.is_finite() && n >= 0.0 => n,
            _ => return Ok(()),
        },
        _ => return Ok(()),
    };
    async_hooks_emit_destroy_id_for_global(rt, async_id)
}

pub(crate) fn async_hooks_emit_destroy_id_for_global(
    rt: &mut Runtime,
    async_id: f64,
) -> Result<(), RuntimeError> {
    let ns = match rt.global_get("async_hooks") {
        Value::Object(id) => id,
        _ => return Ok(()),
    };
    if async_hooks_enabled_count(rt, ns) <= 0.0 {
        return Ok(());
    }
    let hooks = match rt.object_get(ns, "__async_hooks_hooks__") {
        Value::Object(id) => id,
        _ => return Ok(()),
    };
    let len = match rt.object_get(hooks, "length") {
        Value::Number(n) if n.is_finite() && n >= 0.0 => n as usize,
        _ => 0,
    };
    for idx in 0..len {
        let hook = match rt.object_get(hooks, idx.to_string().as_str()) {
            Value::Object(id) => id,
            _ => continue,
        };
        if !matches!(
            rt.object_get(hook, "__async_hooks_enabled__"),
            Value::Boolean(true)
        ) {
            continue;
        }
        let cb = rt.object_get(hook, "__async_hook_destroy__");
        if !rt.is_callable(&cb) {
            continue;
        }
        rt.call_function(cb, Value::Object(hook), vec![Value::Number(async_id)])?;
    }
    Ok(())
}

fn async_hooks_emit_lifecycle_for_resource(
    rt: &mut Runtime,
    ns: ObjectRef,
    slot: &str,
    async_id: f64,
) {
    if async_hooks_enabled_count(rt, ns) <= 0.0 {
        return;
    }
    let hooks = match rt.object_get(ns, "__async_hooks_hooks__") {
        Value::Object(id) => id,
        _ => return,
    };
    let len = match rt.object_get(hooks, "length") {
        Value::Number(n) if n.is_finite() && n >= 0.0 => n as usize,
        _ => 0,
    };
    for idx in 0..len {
        let hook = match rt.object_get(hooks, idx.to_string().as_str()) {
            Value::Object(id) => id,
            _ => continue,
        };
        if !matches!(
            rt.object_get(hook, "__async_hooks_enabled__"),
            Value::Boolean(true)
        ) {
            continue;
        }
        let cb = rt.object_get(hook, slot);
        if !rt.is_callable(&cb) {
            continue;
        }
        if let Err(error) = rt.call_function(cb, Value::Object(hook), vec![Value::Number(async_id)])
        {
            rt.record_async_hook_fatal_exception(error);
            return;
        }
    }
}

fn make_throwing_accessor_getter(rt: &mut Runtime, name: &str) -> ObjectRef {
    make_callable(rt, name, |_rt, _args| {
        Err(RuntimeError::TypeError(
            "Illegal invocation: incompatible receiver".into(),
        ))
    })
}

fn install_throwing_accessor(rt: &mut Runtime, proto: ObjectRef, name: &str) {
    let getter = make_throwing_accessor_getter(rt, &format!("get {name}"));
    rt.obj_mut(proto).dict_mut().insert(
        PropertyKey::String(name.into()),
        PropertyDescriptor {
            value: Value::Undefined,
            writable: false,
            enumerable: false,
            configurable: true,
            getter: Some(Value::Object(getter)),
            setter: None,
        },
    );
}

fn make_internal_test_stream_wrap_binding(rt: &mut Runtime, ctor_name: &str) -> ObjectRef {
    let module = new_object(rt);
    let stream_wrap_proto = new_object(rt);
    for property in ["bytesRead", "fd", "_externalStream"] {
        install_throwing_accessor(rt, stream_wrap_proto, property);
    }

    let ctor = make_callable(rt, ctor_name, |rt, _args| Ok(rt.current_this()));
    let proto = new_object(rt);
    rt.set_object_prototype_internal(proto, Some(stream_wrap_proto));
    rt.object_set(proto, "constructor".into(), Value::Object(ctor));
    rt.object_set(ctor, "prototype".into(), Value::Object(proto));
    rt.object_set(module, ctor_name.into(), Value::Object(ctor));
    module
}

fn make_internal_test_udp_wrap_binding(rt: &mut Runtime) -> ObjectRef {
    let module = new_object(rt);
    let ctor = make_callable(rt, "UDP", |rt, _args| Ok(rt.current_this()));
    let proto = new_object(rt);
    install_throwing_accessor(rt, proto, "fd");
    rt.object_set(proto, "constructor".into(), Value::Object(ctor));
    rt.object_set(ctor, "prototype".into(), Value::Object(proto));
    rt.object_set(module, "UDP".into(), Value::Object(ctor));
    module
}

fn make_internal_test_constants_binding(rt: &mut Runtime) -> ObjectRef {
    let constants = rt.alloc_object_with_explicit_null_proto(RtObject::new_ordinary());
    for group in ["crypto", "fs", "internal", "trace", "zlib"] {
        let obj = rt.alloc_object_with_explicit_null_proto(RtObject::new_ordinary());
        rt.object_set(constants, group.into(), Value::Object(obj));
    }

    let os = rt.alloc_object_with_explicit_null_proto(RtObject::new_ordinary());
    rt.object_set(os, "UV_UDP_REUSEADDR".into(), Value::Number(4.0));
    for group in ["dlopen", "errno", "priority", "signals"] {
        let obj = rt.alloc_object_with_explicit_null_proto(RtObject::new_ordinary());
        rt.object_set(os, group.into(), Value::Object(obj));
    }
    rt.object_set(constants, "os".into(), Value::Object(os));
    constants
}

pub fn make_internal_test_binding_module(rt: &mut Runtime) -> ObjectRef {
    let ns = new_object(rt);
    register_method(rt, ns, "internalBinding", |rt, args| {
        let name = match args.first() {
            Some(Value::String(name)) => name.as_str(),
            _ => "",
        };
        if name == "util" {
            let util = new_object(rt);
            register_method(rt, util, "arrayBufferViewHasBuffer", |rt, args| {
                let Some(Value::Object(id)) = args.first() else {
                    return Ok(Value::Boolean(false));
                };
                let Some(view) = rt.typed_array_views.get(id) else {
                    return Ok(Value::Boolean(false));
                };
                let byte_length = rt
                    .array_buffers
                    .get(&view.buffer)
                    .and_then(|buf| {
                        if buf.detached || view.byte_offset > buf.byte_length {
                            return Some(0);
                        }
                        view.fixed_length
                            .map(|len| len.saturating_mul(view.bytes_per_element))
                            .or_else(|| Some(buf.byte_length.saturating_sub(view.byte_offset)))
                    })
                    .unwrap_or(0);
                if byte_length <= 64
                    && !matches!(
                        rt.object_get(*id, "__node_test_buffer_observed__"),
                        Value::Boolean(true)
                    )
                {
                    rt.obj_mut(*id).set_own_internal(
                        "__node_test_buffer_observed__".into(),
                        Value::Boolean(true),
                    );
                    return Ok(Value::Boolean(false));
                }
                Ok(Value::Boolean(true))
            });
            return Ok(Value::Object(util));
        }
        if name == "tty_wrap" {
            return Ok(Value::Object(make_internal_test_stream_wrap_binding(
                rt, "TTY",
            )));
        }
        if name == "udp_wrap" {
            return Ok(Value::Object(make_internal_test_udp_wrap_binding(rt)));
        }
        if name == "constants" {
            return Ok(Value::Object(make_internal_test_constants_binding(rt)));
        }
        if name != "async_wrap" {
            return Ok(Value::Object(new_object(rt)));
        }
        let async_wrap = new_object(rt);
        register_method(rt, async_wrap, "queueDestroyAsyncId", |rt, args| {
            let async_id = match args.first() {
                Some(Value::Number(n)) if n.is_finite() && n.fract() == 0.0 && *n >= 0.0 => *n,
                _ => return Ok(Value::Undefined),
            };
            rt.enqueue_nexttick_rooted("async_wrap.queueDestroyAsyncId", Vec::new(), move |rt| {
                async_hooks_emit_destroy_id_for_global(rt, async_id)
            });
            Ok(Value::Undefined)
        });
        Ok(Value::Object(async_wrap))
    });
    ns
}

pub fn make_internal_errors_module(rt: &mut Runtime) -> ObjectRef {
    let ns = new_object(rt);
    let codes = new_object(rt);
    let err_out_of_range = make_callable(rt, "ERR_OUT_OF_RANGE", |rt, _args| {
        match rt.current_this() {
            Value::Object(id) => {
                rt.object_set(
                    id,
                    "name".into(),
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                        "RangeError",
                    ))),
                );
                rt.object_set(
                    id,
                    "code".into(),
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                        "ERR_OUT_OF_RANGE",
                    ))),
                );
                rt.object_set(
                    id,
                    "message".into(),
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                        "The value is out of range.",
                    ))),
                );
                Ok(Value::Object(id))
            }
            _ => Err(node_code_range_error(
                rt,
                "ERR_OUT_OF_RANGE",
                "The value is out of range.",
            )),
        }
    });
    rt.object_set(
        codes,
        "ERR_OUT_OF_RANGE".into(),
        Value::Object(err_out_of_range),
    );
    rt.object_set(ns, "codes".into(), Value::Object(codes));
    ns
}

fn async_hooks_current_execution_id(rt: &Runtime, ns: ObjectRef) -> Value {
    match rt.object_get(ns, "__execution_async_id__") {
        Value::Number(n) if n.is_finite() && n >= 0.0 => Value::Number(n),
        _ => Value::Number(0.0),
    }
}

fn async_hooks_next_id(rt: &mut Runtime, ns: ObjectRef) -> f64 {
    let next = match rt.object_get(ns, "__next_async_id__") {
        Value::Number(n) if n.is_finite() && n >= 1.0 => n,
        _ => 2.0,
    };
    rt.object_set(ns, "__next_async_id__".into(), Value::Number(next + 1.0));
    next
}

fn async_hooks_enabled_count(rt: &Runtime, ns: ObjectRef) -> f64 {
    match rt.object_get(ns, "__enabled_hook_count__") {
        Value::Number(n) if n.is_finite() && n > 0.0 => n,
        _ => 0.0,
    }
}

fn async_hooks_adjust_enabled(rt: &mut Runtime, ns: ObjectRef, delta: f64) {
    let next = (async_hooks_enabled_count(rt, ns) + delta).max(0.0);
    rt.object_set(ns, "__enabled_hook_count__".into(), Value::Number(next));
}

fn async_resource_id(rt: &Runtime, resource: ObjectRef) -> f64 {
    match rt.object_get(resource, "__async_id__") {
        Value::Number(n) if n.is_finite() => n,
        _ => 0.0,
    }
}

fn async_resource_trigger_id(rt: &Runtime, resource: ObjectRef) -> Value {
    match rt.object_get(resource, "__trigger_async_id__") {
        Value::Number(n) if n.is_finite() => Value::Number(n),
        _ => Value::Number(0.0),
    }
}

fn async_resource_construct(
    rt: &mut Runtime,
    proto: ObjectRef,
    ns: ObjectRef,
    args: &[Value],
) -> Result<ObjectRef, RuntimeError> {
    let Some(type_arg) = args.first() else {
        return Err(node_code_type_error(
            rt,
            "ERR_INVALID_ARG_TYPE",
            "The \"type\" argument must be of type string.",
        ));
    };
    let Value::String(type_name) = type_arg else {
        return Err(node_code_type_error(
            rt,
            "ERR_INVALID_ARG_TYPE",
            "The \"type\" argument must be of type string.",
        ));
    };
    if type_name.is_empty() {
        return Err(node_code_type_error(
            rt,
            "ERR_ASYNC_TYPE",
            "The \"type\" argument must be a non-empty string.",
        ));
    }
    let trigger = match args.get(1) {
        Some(Value::Number(n)) if n.is_finite() && n.fract() == 0.0 && *n >= 0.0 => *n,
        Some(Value::Number(_)) => {
            return Err(node_code_range_error(
                rt,
                "ERR_INVALID_ASYNC_ID",
                "The \"triggerAsyncId\" argument must be an unsigned integer.",
            ));
        }
        _ => 0.0,
    };
    let inst = rt.alloc_object(RtObject::new_ordinary());
    rt.set_object_prototype_internal(inst, Some(proto));
    let async_id = async_hooks_next_id(rt, ns);

    rt.set_engine_sentinel(inst, "__async_id__", Value::Number(async_id));
    rt.set_engine_sentinel(inst, "__trigger_async_id__", Value::Number(trigger));
    async_hooks_emit_init_for_resource(
        rt,
        ns,
        async_id,
        type_name.as_str(),
        Value::Number(trigger),
        Value::Object(inst),
    )?;
    rt.register_async_resource_destroy_cell(inst, async_id);
    Ok(inst)
}

fn async_hooks_call_with_execution_id(
    rt: &mut Runtime,
    ns: ObjectRef,
    id: f64,
    cb: Value,
    this_arg: Value,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let prev = rt.object_get(ns, "__execution_async_id__");
    rt.object_set(ns, "__execution_async_id__".into(), Value::Number(id));
    let result = rt.call_function(cb, this_arg, args);
    rt.object_set(ns, "__execution_async_id__".into(), prev);
    result
}

pub(crate) fn async_hooks_call_with_execution_resource(
    rt: &mut Runtime,
    ns: ObjectRef,
    resource: ObjectRef,
    id: f64,
    cb: Value,
    this_arg: Value,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let prev_id = rt.object_get(ns, "__execution_async_id__");
    let prev_trigger = rt.object_get(ns, "__trigger_async_id__");
    let prev_resource = rt.object_get(ns, "__execution_async_resource__");
    rt.object_set(ns, "__execution_async_id__".into(), Value::Number(id));
    rt.object_set(
        ns,
        "__trigger_async_id__".into(),
        async_resource_trigger_id(rt, resource),
    );
    rt.object_set(
        ns,
        "__execution_async_resource__".into(),
        Value::Object(resource),
    );
    async_hooks_emit_lifecycle_for_resource(rt, ns, "__async_hook_before__", id);
    let result = rt.call_function(cb, this_arg, args);
    async_hooks_emit_lifecycle_for_resource(rt, ns, "__async_hook_after__", id);
    rt.object_set(ns, "__execution_async_resource__".into(), prev_resource);
    rt.object_set(ns, "__trigger_async_id__".into(), prev_trigger);
    rt.object_set(ns, "__execution_async_id__".into(), prev_id);
    result
}

pub(crate) fn async_hooks_call_with_global_resource(
    rt: &mut Runtime,
    resource: ObjectRef,
    cb: Value,
    this_arg: Value,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let ns = match rt.global_get("async_hooks") {
        Value::Object(ns) => ns,
        _ => return rt.call_function(cb, this_arg, args),
    };
    let id = async_resource_id(rt, resource);
    async_hooks_call_with_execution_resource(rt, ns, resource, id, cb, this_arg, args)
}

pub(crate) fn async_hooks_call_with_global_resource_and_microtasks(
    rt: &mut Runtime,
    resource: ObjectRef,
    cb: Value,
    this_arg: Value,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let ns = match rt.global_get("async_hooks") {
        Value::Object(ns) => ns,
        _ => return rt.call_function(cb, this_arg, args),
    };
    let id = async_resource_id(rt, resource);
    let prev_id = rt.object_get(ns, "__execution_async_id__");
    let prev_trigger = rt.object_get(ns, "__trigger_async_id__");
    let prev_resource = rt.object_get(ns, "__execution_async_resource__");
    rt.object_set(ns, "__execution_async_id__".into(), Value::Number(id));
    rt.object_set(
        ns,
        "__trigger_async_id__".into(),
        async_resource_trigger_id(rt, resource),
    );
    rt.object_set(
        ns,
        "__execution_async_resource__".into(),
        Value::Object(resource),
    );
    let root_key = format!("async-hooks-execution-resource:{resource:?}");
    rt.retain_host_roots(root_key.clone(), vec![Value::Object(resource)]);
    async_hooks_emit_lifecycle_for_resource(rt, ns, "__async_hook_before__", id);
    let result = rt.call_function(cb, this_arg, args);
    async_hooks_emit_lifecycle_for_resource(rt, ns, "__async_hook_after__", id);
    let mut microtask_result = Ok(());
    if result.is_ok() {
        loop {
            match rusty_js_runtime::job_queue::pump_one_microtask(rt) {
                Ok(true) => {}
                Ok(false) => break,
                Err(err) => {
                    microtask_result = Err(err);
                    break;
                }
            }
        }
    }
    rt.release_host_roots(&root_key);
    rt.object_set(ns, "__execution_async_resource__".into(), prev_resource);
    rt.object_set(ns, "__trigger_async_id__".into(), prev_trigger);
    rt.object_set(ns, "__execution_async_id__".into(), prev_id);
    microtask_result?;
    result
}

fn async_resource_bound_function(
    rt: &mut Runtime,
    ns: ObjectRef,
    resource: ObjectRef,
    cb: Value,
    this_arg: Value,
) -> Result<Value, RuntimeError> {
    if !rt.is_callable(&cb) {
        return Err(node_code_type_error(
            rt,
            "ERR_INVALID_ARG_TYPE",
            "The \"fn\" argument must be of type function.",
        ));
    }
    let length = match &cb {
        Value::Object(id) => match rt.object_get(*id, "length") {
            Value::Number(n) if n.is_finite() && n > 0.0 => n as u32,
            _ => 0,
        },
        _ => 0,
    };
    let cb_root = match cb {
        Value::Object(id) => id,
        _ => resource,
    };
    let id = async_resource_id(rt, resource);

    let captured = rt.als_context_snapshot();
    let bound = make_callable_with_length_rooted(
        rt,
        "bound",
        length,
        vec![ns, resource, cb_root],
        move |rt, args| {
            let call_this = if matches!(this_arg, Value::Undefined) {
                rt.current_this()
            } else {
                this_arg.clone()
            };
            let saved = rt.als_context_replace(captured.clone());
            let result = async_hooks_call_with_execution_id(
                rt,
                ns,
                id,
                cb.clone(),
                call_this,
                args.to_vec(),
            );
            rt.als_context_replace(saved);
            result
        },
    );
    Ok(Value::Object(bound))
}

fn punycode_arg_string(args: &[Value], i: usize) -> String {
    args.get(i)
        .map(|v| abstract_ops::to_string(v).as_str().to_string())
        .unwrap_or_default()
}

fn punycode_array_from_codepoints(
    rt: &mut Runtime,
    points: impl IntoIterator<Item = u32>,
) -> Value {
    let arr = rt.alloc_object(RtObject::new_array());
    let mut len = 0usize;
    for cp in points {
        rt.object_set(arr, len.to_string(), Value::Number(cp as f64));
        len += 1;
    }
    rt.object_set(arr, "length".into(), Value::Number(len as f64));
    Value::Object(arr)
}

fn punycode_ucs2_encode_arg(rt: &Runtime, value: Option<&Value>) -> Result<String, RuntimeError> {
    let Some(Value::Object(id)) = value else {
        return Ok(String::new());
    };
    let len = match rt.object_get(*id, "length") {
        Value::Number(n) if n.is_finite() && n > 0.0 => n.floor() as usize,
        _ => 0,
    };
    let mut out = String::new();
    for i in 0..len {
        let n = abstract_ops::to_number(&rt.object_get(*id, &i.to_string()));
        if !n.is_finite() || n < 0.0 || n > 0x10ffff as f64 || n.fract() != 0.0 {
            return Err(RuntimeError::RangeError(format!(
                "punycode.ucs2.encode: invalid code point {}",
                n
            )));
        }
        let cp = n as u32;
        let Some(ch) = char::from_u32(cp) else {
            return Err(RuntimeError::RangeError(format!(
                "punycode.ucs2.encode: invalid code point {}",
                cp
            )));
        };
        out.push(ch);
    }
    Ok(out)
}

pub fn install_punycode(rt: &mut Runtime) {
    let ns = new_object(rt);
    register_method(rt, ns, "encode", |_rt, args| {
        let s = punycode_arg_string(args, 0);
        rusty_js_punycode::encode(&s)
            .map(|out| Value::String(Rc::new(rusty_js_runtime::value::JsString::from(out))))
            .map_err(|e| RuntimeError::TypeError(format!("punycode.encode: {:?}", e)))
    });
    register_method(rt, ns, "decode", |_rt, args| {
        let s = punycode_arg_string(args, 0);
        rusty_js_punycode::decode(&s)
            .map(|out| Value::String(Rc::new(rusty_js_runtime::value::JsString::from(out))))
            .map_err(|e| RuntimeError::TypeError(format!("punycode.decode: {:?}", e)))
    });
    register_method(rt, ns, "toASCII", |_rt, args| {
        let s = punycode_arg_string(args, 0);
        rusty_js_idna::to_ascii(&s)
            .map(|out| Value::String(Rc::new(rusty_js_runtime::value::JsString::from(out))))
            .map_err(|e| RuntimeError::TypeError(format!("punycode.toASCII: {:?}", e)))
    });
    register_method(rt, ns, "toUnicode", |_rt, args| {
        let s = punycode_arg_string(args, 0);
        rusty_js_idna::to_unicode(&s)
            .map(|out| Value::String(Rc::new(rusty_js_runtime::value::JsString::from(out))))
            .map_err(|e| RuntimeError::TypeError(format!("punycode.toUnicode: {:?}", e)))
    });
    crate::register::set_constant(
        rt,
        ns,
        "version",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from("2.1.0"))),
    );
    let ucs2 = new_object(rt);
    register_method(rt, ucs2, "encode", |rt, args| {
        punycode_ucs2_encode_arg(rt, args.first())
            .map(|s| Value::String(Rc::new(rusty_js_runtime::value::JsString::from(s))))
    });
    register_method(rt, ucs2, "decode", |rt, args| {
        let s = punycode_arg_string(args, 0);
        Ok(punycode_array_from_codepoints(
            rt,
            s.chars().map(|c| c as u32),
        ))
    });
    crate::register::set_constant(rt, ns, "ucs2", Value::Object(ucs2));
    rt.define_global_property("punycode", Value::Object(ns));
}

fn v8_values_to_buffer(rt: &mut Runtime, vals: &[Value]) -> Result<Value, RuntimeError> {
    let arr = rt.array_of_via(Value::Undefined, vals)?;
    let mut ctx = rusty_js_runtime::send_ir::LowerCtx::new(None);
    let ir = rusty_js_runtime::send_ir::lower_to_send_ir(rt, &arr, &mut ctx)?;
    let bytes =
        rusty_js_runtime::send_ir::send_ir_to_bytes(&ir).map_err(RuntimeError::TypeError)?;
    Ok(intrinsic_buffer_from_bytes(rt, &bytes))
}

fn v8_object_to_bytes(rt: &mut Runtime, val: &Value) -> Vec<u8> {
    match val {
        Value::Object(id) => {
            let len = match rt.object_get(*id, "length") {
                Value::Number(n) => n as usize,
                _ => 0,
            };
            let mut out = Vec::with_capacity(len);
            for i in 0..len {
                if let Value::Number(n) = rt.object_get(*id, &i.to_string()) {
                    out.push(n as u8);
                }
            }
            out
        }
        _ => Vec::new(),
    }
}

fn v8_install_serializer_methods(rt: &mut Runtime, this: ObjectRef) {
    rt.object_set(this, "__v8_len".into(), Value::Number(0.0));
    register_method(rt, this, "writeHeader", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, this, "writeValue", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let len = match rt.object_get(this, "__v8_len") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        let v = args.first().cloned().unwrap_or(Value::Undefined);
        rt.object_set(this, format!("__v8_v{len}"), v);
        rt.object_set(this, "__v8_len".into(), Value::Number((len + 1) as f64));
        Ok(Value::Undefined)
    });
    register_method(rt, this, "releaseBuffer", |rt, _a| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let len = match rt.object_get(this, "__v8_len") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        let mut vals = Vec::with_capacity(len);
        for i in 0..len {
            vals.push(rt.object_get(this, &format!("__v8_v{i}")));
        }
        v8_values_to_buffer(rt, &vals)
    });

    for m in &[
        "transferArrayBuffer",
        "writeDouble",
        "writeUint32",
        "writeUint64",
        "writeRawBytes",
        "_setTreatArrayBufferViewsAsHostObjects",
    ] {
        register_method(rt, this, m, |_rt, _a| Ok(Value::Undefined));
    }
}

fn v8_install_deserializer_methods(rt: &mut Runtime, this: ObjectRef, buffer: Value) {
    rt.object_set(this, "__v8_buf".into(), buffer);
    rt.object_set(this, "__v8_idx".into(), Value::Number(0.0));
    rt.object_set(this, "__v8_len".into(), Value::Number(0.0));
    register_method(rt, this, "readHeader", |rt, _a| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let buf = rt.object_get(this, "__v8_buf");
        let bytes = v8_object_to_bytes(rt, &buf);
        let ir = rusty_js_runtime::send_ir::send_ir_from_bytes(&bytes)
            .map_err(RuntimeError::TypeError)?;
        let mut table = std::collections::HashMap::new();
        let arr = rusty_js_runtime::send_ir::rematerialize_send_ir(rt, &ir, None, &mut table)?;

        let vals = rt.create_list_from_array_like(&arr)?;
        for (i, v) in vals.iter().enumerate() {
            rt.object_set(this, format!("__v8_v{i}"), v.clone());
        }
        rt.object_set(this, "__v8_len".into(), Value::Number(vals.len() as f64));
        rt.object_set(this, "__v8_idx".into(), Value::Number(0.0));
        Ok(Value::Undefined)
    });
    register_method(rt, this, "readValue", |rt, _a| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let idx = match rt.object_get(this, "__v8_idx") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        let v = rt.object_get(this, &format!("__v8_v{idx}"));
        rt.object_set(this, "__v8_idx".into(), Value::Number((idx + 1) as f64));
        Ok(v)
    });
    for m in &[
        "transferArrayBuffer",
        "readDouble",
        "readUint32",
        "readUint64",
        "readRawBytes",
        "getWireFormatVersion",
        "_readHostObject",
    ] {
        register_method(rt, this, m, |_rt, _a| Ok(Value::Undefined));
    }
}

pub fn install_v8(rt: &mut Runtime) {
    let ns = new_object(rt);
    register_method(rt, ns, "getHeapStatistics", |rt, _args| {
        let o = new_object(rt);
        for (k, v) in &[
            ("total_heap_size", 1024.0 * 1024.0 * 64.0),
            ("total_heap_size_executable", 1024.0 * 1024.0),
            ("total_physical_size", 1024.0 * 1024.0 * 64.0),
            ("total_available_size", 1024.0 * 1024.0 * 1024.0),
            ("used_heap_size", 1024.0 * 1024.0 * 16.0),
            ("heap_size_limit", 1024.0 * 1024.0 * 1024.0 * 2.0),
            ("malloced_memory", 1024.0 * 64.0),
            ("peak_malloced_memory", 1024.0 * 128.0),
            ("does_zap_garbage", 0.0),
            ("number_of_native_contexts", 1.0),
            ("number_of_detached_contexts", 0.0),
            ("total_global_handles_size", 1024.0 * 8.0),
            ("used_global_handles_size", 1024.0 * 4.0),
            ("external_memory", 0.0),
        ] {
            rt.object_set(o, (*k).into(), Value::Number(*v));
        }
        Ok(Value::Object(o))
    });
    register_method(rt, ns, "getHeapSpaceStatistics", |rt, _args| {
        Ok(Value::Object(rt.alloc_object(RtObject::new_array())))
    });
    register_method(rt, ns, "getHeapCodeStatistics", |rt, _args| {
        Ok(Value::Object(new_object(rt)))
    });

    register_method(rt, ns, "serialize", |rt, args| {
        let value = args.first().cloned().unwrap_or(Value::Undefined);
        let mut ctx = rusty_js_runtime::send_ir::LowerCtx::new(None);
        let ir = rusty_js_runtime::send_ir::lower_to_send_ir(rt, &value, &mut ctx)?;
        let bytes =
            rusty_js_runtime::send_ir::send_ir_to_bytes(&ir).map_err(RuntimeError::TypeError)?;
        Ok(intrinsic_buffer_from_bytes(rt, &bytes))
    });
    register_method(rt, ns, "deserialize", |rt, args| {
        let bytes = match args.first() {
            Some(Value::Object(id)) => {
                let len = match rt.object_get(*id, "length") {
                    Value::Number(n) => n as usize,
                    _ => 0,
                };
                let mut out = Vec::with_capacity(len);
                for i in 0..len {
                    if let Value::Number(n) = rt.object_get(*id, &i.to_string()) {
                        out.push(n as u8);
                    }
                }
                out
            }
            _ => Vec::new(),
        };
        let ir = rusty_js_runtime::send_ir::send_ir_from_bytes(&bytes)
            .map_err(RuntimeError::TypeError)?;
        let mut table = std::collections::HashMap::new();
        rusty_js_runtime::send_ir::rematerialize_send_ir(rt, &ir, None, &mut table)
    });
    for m in &["writeHeapSnapshot", "setFlagsFromString"] {
        register_method(rt, ns, m, stub("v8", m));
    }

    register_method(rt, ns, "cachedDataVersionTag", |_rt, _a| {
        Ok(Value::Number(3540625431.0))
    });
    {
        let c = make_callable(rt, "DefaultDeserializer", |rt, a| {
            if let Value::Object(this) = rt.current_this() {
                let buf = a.first().cloned().unwrap_or(Value::Undefined);
                v8_install_deserializer_methods(rt, this, buf);
            }
            Ok(rt.current_this())
        });
        rt.object_set(ns, "DefaultDeserializer".into(), Value::Object(c));
    }
    {
        let c = make_callable(rt, "DefaultSerializer", |rt, _a| {
            if let Value::Object(this) = rt.current_this() {
                v8_install_serializer_methods(rt, this);
            }
            Ok(rt.current_this())
        });
        rt.object_set(ns, "DefaultSerializer".into(), Value::Object(c));
    }
    {
        let c = make_callable(rt, "Deserializer", |rt, a| {
            if let Value::Object(this) = rt.current_this() {
                let buf = a.first().cloned().unwrap_or(Value::Undefined);
                v8_install_deserializer_methods(rt, this, buf);
            }
            Ok(rt.current_this())
        });
        rt.object_set(ns, "Deserializer".into(), Value::Object(c));
    }
    {
        let c = make_callable(rt, "Serializer", |rt, _a| {
            if let Value::Object(this) = rt.current_this() {
                v8_install_serializer_methods(rt, this);
            }
            Ok(rt.current_this())
        });
        rt.object_set(ns, "Serializer".into(), Value::Object(c));
    }
    {
        let c = make_callable(rt, "GCProfiler", |rt, _a| Ok(rt.current_this()));
        rt.object_set(ns, "GCProfiler".into(), Value::Object(c));
    }
    register_method(rt, ns, "getCppHeapStatistics", |_rt, _a| {
        Ok(Value::Undefined)
    });
    register_method(rt, ns, "getHeapSnapshot", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, ns, "isStringOneByteRepresentation", |_rt, _a| {
        Ok(Value::Undefined)
    });
    register_method(rt, ns, "queryObjects", |rt, args| {
        let ctor = args.first().cloned().unwrap_or(Value::Undefined);
        if !rt.is_callable(&ctor) {
            return Err(node_code_type_error(
                rt,
                "ERR_INVALID_ARG_TYPE",
                "The \"constructor\" argument must be a function.",
            ));
        }
        rt.collect();
        let ids: Vec<_> = rt.heap.live_object_ids().collect();
        let mut count = 0.0;
        for id in ids {
            if rt.ordinary_has_instance(&Value::Object(id), &ctor)? {
                count += 1.0;
            }
        }
        Ok(Value::Number(count))
    });
    register_method(rt, ns, "setHeapSnapshotNearHeapLimit", |_rt, _a| {
        Ok(Value::Undefined)
    });
    register_method(rt, ns, "startCpuProfile", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, ns, "startHeapProfile", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, ns, "stopCoverage", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, ns, "takeCoverage", |_rt, _a| Ok(Value::Undefined));
    {
        let o = new_object(rt);
        rt.object_set(ns, "promiseHooks".into(), Value::Object(o));
    }
    {
        let o = new_object(rt);
        rt.object_set(ns, "startupSnapshot".into(), Value::Object(o));
    }
    rt.define_global_property("v8", Value::Object(ns));
}

pub fn install_inspector(rt: &mut Runtime) {
    let ns = new_object(rt);
    for m in &["open", "close", "waitForDebugger"] {
        register_method(rt, ns, m, stub("inspector", m));
    }

    register_method(rt, ns, "url", |_rt, _a| Ok(Value::Undefined));
    {
        let c = make_callable(rt, "Session", |rt, _a| Ok(rt.current_this()));
        rt.object_set(ns, "Session".into(), Value::Object(c));
    }
    {
        let o = new_object(rt);
        rt.object_set(ns, "Network".into(), Value::Object(o));
    }
    {
        let o = new_object(rt);
        rt.object_set(ns, "NetworkResources".into(), Value::Object(o));
    }
    {
        let o = new_object(rt);
        rt.object_set(ns, "DOMStorage".into(), Value::Object(o));
    }
    {
        let o = new_object(rt);
        rt.object_set(ns, "console".into(), Value::Object(o));
    }
    rt.define_global_property("inspector", Value::Object(ns));
}

fn vm_compile_function_expression(body: &str) -> String {
    let trimmed = body.trim();
    if let Some(rest) = trimmed.strip_prefix("return ") {
        return rest.trim_end_matches(';').trim().to_string();
    }
    trimmed.to_string()
}

fn vm_options_filename_url(rt: &Runtime, options: Option<&Value>) -> Option<String> {
    let Value::Object(id) = options? else {
        return None;
    };
    let Value::String(filename) = rt.object_get(*id, "filename") else {
        return None;
    };
    let filename = filename.as_str();
    if filename.is_empty() {
        return None;
    }
    if filename.starts_with("file://") {
        Some(filename.to_string())
    } else {
        Some(format!("file://{filename}"))
    }
}

fn vm_context_completion_value(
    rt: &mut Runtime,
    ctx: rusty_js_runtime::ObjectRef,
    source: &str,
    fallback: Value,
) -> Value {
    let Some(last) = source.rsplit(';').find_map(|part| {
        let p = part.trim();
        if p.is_empty() {
            None
        } else {
            Some(p)
        }
    }) else {
        return fallback;
    };
    let mut chars = last.chars();
    let Some(first) = chars.next() else {
        return fallback;
    };
    if !(first == '_' || first == '$' || first.is_ascii_alphabetic()) {
        return fallback;
    }
    if !chars.all(|c| c == '_' || c == '$' || c.is_ascii_alphanumeric()) {
        return fallback;
    }
    match rt.object_get(ctx, last) {
        Value::Undefined => fallback,
        value => value,
    }
}

fn vm_context_boundary_value(
    rt: &mut Runtime,
    target_realm: usize,
    value: Value,
) -> Result<Value, RuntimeError> {
    if !rt.is_callable(&value) {
        use rusty_js_runtime::realm_adapter::{boundary_filter, BoundaryPolicy};

        return boundary_filter(rt, value, BoundaryPolicy::SharedHeapIdentity);
    }
    let target = value;
    let roots = match target {
        Value::Object(id) => vec![id],
        _ => Vec::new(),
    };
    let wrapper = make_callable_rooted(rt, "vmContextFunction", roots, move |rt, args| {
        let prior_realm = rt.enter_realm(target_realm);
        let result = rt.call_function(target.clone(), Value::Undefined, args.to_vec());
        rt.exit_realm(prior_realm);
        result
    });
    Ok(Value::Object(wrapper))
}

pub fn install_vm(rt: &mut Runtime) {
    use rusty_js_runtime::realm_adapter::{boundary_filter, realm_evaluate, BoundaryPolicy};

    crate::vm::install_canonical(rt);

    let ns = new_object(rt);

    register_method(rt, ns, "createContext", |rt, args| {
        let obj = match args.first() {
            Some(Value::Object(id)) => *id,
            _ => rt.alloc_object(RtObject::new_ordinary()),
        };
        crate::vm::contextify_in_place(rt, obj);
        Ok(Value::Object(obj))
    });

    register_method(rt, ns, "isContext", |rt, args| match args.first() {
        Some(Value::Object(id)) => Ok(Value::Boolean(matches!(
            rt.object_get(*id, "__vm_context"),
            Value::Boolean(true)
        ))),
        _ => Ok(Value::Boolean(false)),
    });

    register_method(rt, ns, "runInContext", |rt, args| {
        let source = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "vm.runInContext: source must be a string".into(),
                ))
            }
        };
        let ctx_id = match args.get(1) {
            Some(Value::Object(id)) => *id,
            _ => {
                return Err(RuntimeError::TypeError(
                    "vm.runInContext: contextifiedObject required".into(),
                ))
            }
        };
        let realm_idx = match rt.object_get(ctx_id, "__vm_realm") {
            Value::Number(n) => n as usize,
            _ => {
                return Err(RuntimeError::TypeError(
                    "vm.runInContext: contextifiedObject is not a context".into(),
                ))
            }
        };
        let url = format!("file://<vm:{}:runInContext>", realm_idx);
        let value = realm_evaluate(rt, realm_idx, Some(ctx_id), &source, &url)?;
        crate::vm::sync_context_global_after_eval(rt, ctx_id, realm_idx);
        let value = vm_context_completion_value(rt, ctx_id, &source, value);
        vm_context_boundary_value(rt, realm_idx, value)
    });

    register_method(rt, ns, "runInNewContext", |rt, args| {
        let source = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "vm.runInNewContext: source must be a string".into(),
                ))
            }
        };
        let obj = match args.get(1) {
            Some(Value::Object(id)) => *id,
            _ => rt.alloc_object(RtObject::new_ordinary()),
        };
        let realm_idx = crate::vm::contextify_in_place(rt, obj);
        let url = vm_options_filename_url(rt, args.get(2))
            .unwrap_or_else(|| format!("file://<vm:{}:runInNewContext>", realm_idx));
        let value = realm_evaluate(rt, realm_idx, Some(obj), &source, &url)?;
        crate::vm::sync_context_global_after_eval(rt, obj, realm_idx);
        let value = vm_context_completion_value(rt, obj, &source, value);
        vm_context_boundary_value(rt, realm_idx, value)
    });

    register_method(rt, ns, "runInThisContext", |rt, args| {
        let source = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "vm.runInThisContext: source must be a string".into(),
                ))
            }
        };
        let current = rt.current_realm;
        let url = format!("file://<vm:{}:runInThisContext>", current);
        let value = realm_evaluate(rt, current, rt.global_object, &source, &url)?;
        boundary_filter(rt, value, BoundaryPolicy::NodeCompat)
    });

    register_method(rt, ns, "compileFunction", |rt, args| {
        let body = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "vm.compileFunction: code must be a string".into(),
                ))
            }
        };
        let mut params = Vec::new();
        if let Some(Value::Object(arr)) = args.get(1) {
            let len = rt.array_length(*arr);
            for i in 0..len {
                if let Value::String(s) = rt.object_get(*arr, &i.to_string()) {
                    params.push(s.as_str().to_string());
                }
            }
        }
        let mut context_extension = None;
        if let Some(Value::Object(opts)) = args.get(2) {
            if let Value::Object(exts) = rt.object_get(*opts, "contextExtensions") {
                if let Value::Object(ext) = rt.object_get(exts, "0") {
                    context_extension = Some(ext);
                }
            }
        }
        let source = vm_compile_function_expression(&body);

        if let Err(e) = rusty_js_parser::parse_script(&source) {
            return Err(RuntimeError::SyntaxError(e.message));
        }
        let fn_obj = make_callable(rt, "compiledFunction", move |rt, call_args| {
            let ctx = match context_extension {
                Some(ctx) => {
                    if !matches!(rt.object_get(ctx, "__vm_context"), Value::Boolean(true)) {
                        crate::vm::contextify_in_place(rt, ctx);
                    }
                    ctx
                }
                None => {
                    let obj = rt.alloc_object(RtObject::new_ordinary());
                    crate::vm::contextify_in_place(rt, obj);
                    obj
                }
            };
            for (i, name) in params.iter().enumerate() {
                let v = call_args.get(i).cloned().unwrap_or(Value::Undefined);
                rt.object_set(ctx, name.clone(), v);
            }
            let realm_idx = match rt.object_get(ctx, "__vm_realm") {
                Value::Number(n) => n as usize,
                _ => {
                    return Err(RuntimeError::TypeError(
                        "vm.compileFunction: context extension is not a context".into(),
                    ))
                }
            };
            let url = format!("file://<vm:{}:compileFunction>", realm_idx);
            let value = realm_evaluate(rt, realm_idx, Some(ctx), &source, &url)?;
            crate::vm::sync_context_global_after_eval(rt, ctx, realm_idx);

            boundary_filter(rt, value, BoundaryPolicy::SharedHeapIdentity)
        });
        Ok(Value::Object(fn_obj))
    });
    register_method(rt, ns, "measureMemory", |rt, args| {
        let mut mode = "summary".to_string();
        if let Some(Value::Object(options)) = args.first() {
            if let Value::String(s) = rt.object_get(*options, "mode") {
                mode = s.as_str().to_string();
            }
        }
        if mode != "summary" && mode != "detailed" {
            return Err(RuntimeError::TypeError(format!(
                "The property 'options.mode' must be one of: 'summary', 'detailed'. Received '{}'",
                mode
            )));
        }

        let memory_range = |rt: &mut Runtime| {
            let range = rt.alloc_object(RtObject::new_array());
            rt.object_set(range, "0".into(), Value::Number(0.0));
            rt.object_set(range, "1".into(), Value::Number(0.0));
            rt.object_set(range, "length".into(), Value::Number(2.0));
            Value::Object(range)
        };
        let memory_entry = |rt: &mut Runtime| {
            let entry = rt.alloc_object(RtObject::new_ordinary());
            rt.object_set(entry, "jsMemoryEstimate".into(), Value::Number(0.0));
            let range = memory_range(rt);
            rt.object_set(entry, "jsMemoryRange".into(), range);
            Value::Object(entry)
        };

        let summary = rt.alloc_object(RtObject::new_ordinary());
        let total = memory_entry(rt);
        rt.object_set(summary, "total".into(), total);
        if mode == "detailed" {
            let current = memory_entry(rt);
            rt.object_set(summary, "current".into(), current);
            let other = rt.alloc_object(RtObject::new_array());
            rt.object_set(other, "length".into(), Value::Number(0.0));
            rt.object_set(summary, "other".into(), Value::Object(other));
        }
        let wasm = rt.alloc_object(RtObject::new_ordinary());
        let wasm_code = memory_entry(rt);
        rt.object_set(wasm, "code".into(), wasm_code);
        let wasm_metadata = memory_entry(rt);
        rt.object_set(wasm, "metadata".into(), wasm_metadata);
        rt.object_set(summary, "WebAssembly".into(), Value::Object(wasm));
        let promise = rusty_js_runtime::promise::new_promise(rt);
        rusty_js_runtime::promise::resolve_promise(rt, promise, Value::Object(summary));
        Ok(Value::Object(promise))
    });

    {
        let proto = new_object(rt);
        register_method(rt, proto, "runInThisContext", |rt, _args| {
            let this_id = match rt.current_this() {
                Value::Object(id) => id,
                _ => {
                    return Err(RuntimeError::TypeError(
                        "vm.Script.prototype.runInThisContext: this is not a Script".into(),
                    ))
                }
            };
            let source = match rt.object_get(this_id, "__vm_script_source") {
                Value::String(s) => s.as_str().to_string(),
                _ => {
                    return Err(RuntimeError::TypeError(
                        "vm.Script.prototype.runInThisContext: missing source".into(),
                    ))
                }
            };
            let current = rt.current_realm;
            let url = format!("file://<vm:{}:Script.runInThisContext>", current);
            let value = realm_evaluate(rt, current, rt.global_object, &source, &url)?;
            boundary_filter(rt, value, BoundaryPolicy::NodeCompat)
        });
        register_method(rt, proto, "runInContext", |rt, args| {
            let this_id = match rt.current_this() {
                Value::Object(id) => id,
                _ => {
                    return Err(RuntimeError::TypeError(
                        "vm.Script.prototype.runInContext: this is not a Script".into(),
                    ))
                }
            };
            let source = match rt.object_get(this_id, "__vm_script_source") {
                Value::String(s) => s.as_str().to_string(),
                _ => {
                    return Err(RuntimeError::TypeError(
                        "vm.Script.prototype.runInContext: missing source".into(),
                    ))
                }
            };
            let ctx_id = match args.first() {
                Some(Value::Object(id)) => *id,
                _ => {
                    return Err(RuntimeError::TypeError(
                        "vm.Script.prototype.runInContext: contextifiedObject required".into(),
                    ))
                }
            };
            let realm_idx = match rt.object_get(ctx_id, "__vm_realm") {
                Value::Number(n) => n as usize,
                _ => {
                    return Err(RuntimeError::TypeError(
                        "vm.Script.prototype.runInContext: contextifiedObject is not a context"
                            .into(),
                    ))
                }
            };
            let url = format!("file://<vm:{}:Script.runInContext>", realm_idx);
            let value = realm_evaluate(rt, realm_idx, Some(ctx_id), &source, &url)?;
            crate::vm::sync_context_global_after_eval(rt, ctx_id, realm_idx);
            let value = vm_context_completion_value(rt, ctx_id, &source, value);
            vm_context_boundary_value(rt, realm_idx, value)
        });
        register_method(rt, proto, "runInNewContext", |rt, args| {
            let this_id = match rt.current_this() {
                Value::Object(id) => id,
                _ => {
                    return Err(RuntimeError::TypeError(
                        "vm.Script.prototype.runInNewContext: this is not a Script".into(),
                    ))
                }
            };
            let source = match rt.object_get(this_id, "__vm_script_source") {
                Value::String(s) => s.as_str().to_string(),
                _ => {
                    return Err(RuntimeError::TypeError(
                        "vm.Script.prototype.runInNewContext: missing source".into(),
                    ))
                }
            };
            let obj = match args.first() {
                Some(Value::Object(id)) => *id,
                _ => rt.alloc_object(RtObject::new_ordinary()),
            };

            let realm_idx = crate::vm::contextify_in_place(rt, obj);
            let url = vm_options_filename_url(rt, args.get(1))
                .unwrap_or_else(|| format!("file://<vm:{}:Script.runInNewContext>", realm_idx));
            let value = realm_evaluate(rt, realm_idx, Some(obj), &source, &url)?;
            crate::vm::sync_context_global_after_eval(rt, obj, realm_idx);
            let value = vm_context_completion_value(rt, obj, &source, value);
            vm_context_boundary_value(rt, realm_idx, value)
        });
        register_method(rt, proto, "createCachedData", |rt, _a| {
            let buf = rt.alloc_object(RtObject::new_array());
            rt.object_set(buf, "length".into(), Value::Number(0.0));
            Ok(Value::Object(buf))
        });
        let proto_for_ctor = proto;
        let ctor = make_callable(rt, "Script", move |rt, args| {
            let source = match args.first() {
                Some(Value::String(s)) => s.as_str().to_string(),
                _ => {
                    return Err(RuntimeError::TypeError(
                        "vm.Script: source must be a string".into(),
                    ))
                }
            };

            if let Err(e) = rusty_js_parser::parse_script(&source) {
                return Err(RuntimeError::SyntaxError(e.message));
            }
            let inst = rt.alloc_object(RtObject::new_ordinary());

            rt.set_object_prototype_internal(inst, Some(proto_for_ctor));
            rt.object_set(
                inst,
                "__vm_script_source".into(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(source))),
            );
            Ok(Value::Object(inst))
        });
        rt.set_own_frozen_property(ctor, "prototype".into(), Value::Object(proto));
        rt.obj_mut(proto)
            .set_own_internal("constructor".into(), Value::Object(ctor));
        rt.object_set(ns, "Script".into(), Value::Object(ctor));
    }

    for cls in &["SourceTextModule", "SyntheticModule"] {
        let proto = new_object(rt);
        let ctor = make_callable(rt, cls, move |rt, _args| {
            let inst = rt.alloc_object(RtObject::new_ordinary());
            register_method(rt, inst, "runInThisContext", |_rt, _a| Ok(Value::Undefined));
            register_method(rt, inst, "runInContext", |_rt, _a| Ok(Value::Undefined));
            register_method(rt, inst, "runInNewContext", |_rt, _a| Ok(Value::Undefined));
            register_method(rt, inst, "createCachedData", |rt, _a| {
                let buf = rt.alloc_object(RtObject::new_array());
                rt.object_set(buf, "length".into(), Value::Number(0.0));
                Ok(Value::Object(buf))
            });
            Ok(Value::Object(inst))
        });
        rt.set_own_frozen_property(ctor, "prototype".into(), Value::Object(proto));
        rt.obj_mut(proto)
            .set_own_internal("constructor".into(), Value::Object(ctor));
        rt.object_set(ns, (*cls).into(), Value::Object(ctor));
    }

    {
        let c = new_object(rt);
        rt.object_set(ns, "constants".into(), Value::Object(c));
    }
    register_method(rt, ns, "createScript", |rt, _a| Ok(rt.current_this()));
    for k in ["SourceTextModule", "SyntheticModule"] {
        let _ = rt.delete_own_via(
            &Value::Object(ns),
            &Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                k.to_string(),
            ))),
        );
    }
    rt.define_global_property("vm", Value::Object(ns));
}

pub fn install_diagnostics_channel(rt: &mut Runtime) {
    let ns = new_object(rt);

    let registry = new_object(rt);
    rt.define_global_property("__cruft_dc_registry", Value::Object(registry));
    register_method(rt, ns, "channel", |rt, args| {
        let name = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            Some(v) => abstract_ops::to_string(v).as_str().to_string(),
            None => String::new(),
        };
        let registry = match rt.global_get("__cruft_dc_registry") {
            Value::Object(r) => r,
            _ => new_object(rt),
        };
        if let Value::Object(existing) = rt.object_get(registry, &name) {
            return Ok(Value::Object(existing));
        }
        let ch = new_object(rt);
        rt.obj_mut(ch)
            .set_own_internal("__diagnostics_channel__".into(), Value::Boolean(true));
        let subs = rt.alloc_object(RtObject::new_array());
        rt.set_engine_sentinel(ch, "__dc_subs", Value::Object(subs));
        let stores = rt.alloc_object(RtObject::new_array());
        rt.set_engine_sentinel(ch, "__dc_stores", Value::Object(stores));
        rt.object_set(
            ch,
            "name".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                name.clone(),
            ))),
        );
        rt.object_set(ch, "hasSubscribers".into(), Value::Boolean(false));
        register_method(rt, ch, "subscribe", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let cb = args.first().cloned().unwrap_or(Value::Undefined);
            if rt.is_callable(&cb) {
                if let Value::Object(subs) = rt.object_get(this, "__dc_subs") {
                    let push = rt.object_get(subs, "push");
                    if rt.is_callable(&push) {
                        let _ = rt.call_function(push, Value::Object(subs), vec![cb]);
                    }
                }
                rt.object_set(this, "hasSubscribers".into(), Value::Boolean(true));
            }
            Ok(Value::Undefined)
        });
        register_method(rt, ch, "publish", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let msg = args.first().cloned().unwrap_or(Value::Undefined);
            let name = rt.object_get(this, "name");
            if let Value::Object(stores) = rt.object_get(this, "__dc_stores") {
                let len = match rt.object_get(stores, "length") {
                    Value::Number(n) if n > 0.0 => n as usize,
                    _ => 0,
                };
                for i in 0..len {
                    let binding = rt.object_get(stores, &i.to_string());
                    let Value::Object(binding_id) = binding else {
                        continue;
                    };
                    let storage = rt.object_get(binding_id, "storage");
                    let transform = rt.object_get(binding_id, "transform");
                    let store = if rt.is_callable(&transform) {
                        rt.call_function(transform, Value::Undefined, vec![msg.clone()])
                            .unwrap_or(Value::Undefined)
                    } else {
                        msg.clone()
                    };
                    let enter = match &storage {
                        Value::Object(storage_id) => rt.object_get(*storage_id, "enterWith"),
                        _ => Value::Undefined,
                    };
                    if rt.is_callable(&enter) {
                        let _ = rt.call_function(enter, storage, vec![store]);
                    }
                }
            }
            if let Value::Object(subs) = rt.object_get(this, "__dc_subs") {
                let len = match rt.object_get(subs, "length") {
                    Value::Number(n) if n > 0.0 => n as usize,
                    _ => 0,
                };
                for i in 0..len {
                    let cb = rt.object_get(subs, &i.to_string());
                    if rt.is_callable(&cb) {
                        let _ =
                            rt.call_function(cb, Value::Undefined, vec![msg.clone(), name.clone()]);
                    }
                }
            }
            Ok(Value::Undefined)
        });
        register_method(rt, ch, "bindStore", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let storage = args.first().cloned().unwrap_or(Value::Undefined);
            let transform = args.get(1).cloned().unwrap_or(Value::Undefined);
            if let Value::Object(stores) = rt.object_get(this, "__dc_stores") {
                let binding = new_object(rt);
                rt.object_set(binding, "storage".into(), storage);
                rt.object_set(binding, "transform".into(), transform);
                let push = rt.object_get(stores, "push");
                if rt.is_callable(&push) {
                    let _ =
                        rt.call_function(push, Value::Object(stores), vec![Value::Object(binding)]);
                }
            }
            Ok(Value::Undefined)
        });
        register_method(rt, ch, "unsubscribe", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Boolean(false)),
            };
            let target = args.first().cloned().unwrap_or(Value::Undefined);
            let mut removed = false;
            if let Value::Object(subs) = rt.object_get(this, "__dc_subs") {
                let len = match rt.object_get(subs, "length") {
                    Value::Number(n) if n > 0.0 => n as usize,
                    _ => 0,
                };
                let kept = rt.alloc_object(RtObject::new_array());
                let push = rt.object_get(kept, "push");
                for i in 0..len {
                    let cb = rt.object_get(subs, &i.to_string());
                    let same =
                        matches!((&cb, &target), (Value::Object(a), Value::Object(b)) if a == b);
                    if same {
                        removed = true;
                    } else if rt.is_callable(&push) {
                        let _ = rt.call_function(push.clone(), Value::Object(kept), vec![cb]);
                    }
                }
                rt.set_engine_sentinel(this, "__dc_subs", Value::Object(kept));
                let still = matches!(rt.object_get(kept, "length"), Value::Number(n) if n > 0.0);
                rt.object_set(this, "hasSubscribers".into(), Value::Boolean(still));
            }
            Ok(Value::Boolean(removed))
        });
        rt.object_set(registry, name, Value::Object(ch));
        Ok(Value::Object(ch))
    });
    register_method(rt, ns, "tracingChannel", move |rt, args| {
        let base = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            Some(v) => abstract_ops::to_string(v).as_str().to_string(),
            None => String::new(),
        };
        let ch = new_object(rt);
        rt.obj_mut(ch)
            .set_own_internal("__diagnostics_channel__".into(), Value::Boolean(true));
        rt.object_set(ch, "hasSubscribers".into(), Value::Boolean(false));
        let channel_fn = rt.object_get(ns, "channel");
        for m in ["start", "end", "asyncStart", "asyncEnd", "error"] {
            let full = format!("tracing:{base}:{m}");
            let channel = if rt.is_callable(&channel_fn) {
                rt.call_function(
                    channel_fn.clone(),
                    Value::Object(ns),
                    vec![Value::String(Rc::new(
                        rusty_js_runtime::value::JsString::from(full),
                    ))],
                )
                .unwrap_or(Value::Undefined)
            } else {
                Value::Undefined
            };
            rt.object_set(ch, m.to_string(), channel);
        }

        for m in ["traceSync", "tracePromise"] {
            register_method(rt, ch, m, |rt, args| {
                let func = args.first().cloned().unwrap_or(Value::Undefined);
                if !rt.is_callable(&func) {
                    return Ok(Value::Undefined);
                }
                let this_arg = args.get(2).cloned().unwrap_or(Value::Undefined);
                let call_args: Vec<Value> = args.get(3..).map(|s| s.to_vec()).unwrap_or_default();
                rt.call_function(func, this_arg, call_args)
            });
        }

        register_method(rt, ch, "traceCallback", |rt, args| {
            let func = args.first().cloned().unwrap_or(Value::Undefined);
            if !rt.is_callable(&func) {
                return Ok(Value::Undefined);
            }
            let this_arg = args.get(3).cloned().unwrap_or(Value::Undefined);
            let call_args: Vec<Value> = args.get(4..).map(|s| s.to_vec()).unwrap_or_default();
            rt.call_function(func, this_arg, call_args)
        });
        Ok(Value::Object(ch))
    });
    register_method(rt, ns, "subscribe", move |rt, args| {
        let name = args.first().cloned().unwrap_or(Value::Undefined);
        let cb = args.get(1).cloned().unwrap_or(Value::Undefined);
        let channel_fn = rt.object_get(ns, "channel");
        if rt.is_callable(&channel_fn) {
            if let Value::Object(ch) =
                rt.call_function(channel_fn, Value::Object(ns), vec![name])?
            {
                let subscribe = rt.object_get(ch, "subscribe");
                if rt.is_callable(&subscribe) {
                    let _ = rt.call_function(subscribe, Value::Object(ch), vec![cb]);
                }
            }
        }
        Ok(Value::Undefined)
    });
    register_method(rt, ns, "unsubscribe", move |rt, args| {
        let name = args.first().cloned().unwrap_or(Value::Undefined);
        let cb = args.get(1).cloned().unwrap_or(Value::Undefined);
        let channel_fn = rt.object_get(ns, "channel");
        if rt.is_callable(&channel_fn) {
            if let Value::Object(ch) =
                rt.call_function(channel_fn, Value::Object(ns), vec![name])?
            {
                let unsubscribe = rt.object_get(ch, "unsubscribe");
                if rt.is_callable(&unsubscribe) {
                    return rt.call_function(unsubscribe, Value::Object(ch), vec![cb]);
                }
            }
        }
        Ok(Value::Boolean(false))
    });
    register_method(rt, ns, "hasSubscribers", move |rt, args| {
        let name = args.first().cloned().unwrap_or(Value::Undefined);
        let channel_fn = rt.object_get(ns, "channel");
        if rt.is_callable(&channel_fn) {
            if let Value::Object(ch) =
                rt.call_function(channel_fn, Value::Object(ns), vec![name])?
            {
                return Ok(rt.object_get(ch, "hasSubscribers"));
            }
        }
        Ok(Value::Boolean(false))
    });
    {
        let c = make_callable(rt, "Channel", |rt, _a| Ok(rt.current_this()));
        rt.object_set(ns, "Channel".into(), Value::Object(c));
    }
    {
        let c = make_callable(rt, "BoundedChannel", |rt, _a| Ok(rt.current_this()));
        rt.object_set(ns, "BoundedChannel".into(), Value::Object(c));
    }
    register_method(rt, ns, "boundedChannel", |_rt, _a| Ok(Value::Undefined));
    rt.define_global_property("diagnostics_channel", Value::Object(ns));
}

fn sym(rt: &mut Runtime, name: &str) -> Value {
    let _ = rt;
    Value::Symbol(Rc::new(format!("nodejs.{name}")))
}
fn mk_ctor(rt: &mut Runtime, ns: rusty_js_runtime::ObjectRef, name: &str) {
    let c = make_callable(rt, name, |rt, _a| Ok(rt.current_this()));
    let proto = new_object(rt);
    rt.object_set(proto, "constructor".into(), Value::Object(c));
    rt.object_set(c, "prototype".into(), Value::Object(proto));
    rt.object_set(ns, name.to_string(), Value::Object(c));
}
fn mk_obj(rt: &mut Runtime, ns: rusty_js_runtime::ObjectRef, name: &str) {
    let o = new_object(rt);
    rt.object_set(ns, name.to_string(), Value::Object(o));
}

pub fn install_worker_threads(rt: &mut Runtime) {
    if let Value::Object(existing) = rt.global_get("worker_threads") {
        if !matches!(
            rt.object_get(existing, "parentPort"),
            Value::Null | Value::Undefined
        ) {
            rt.define_global_property("__cruft_worker", Value::Object(existing));
            return;
        }
    }
    let ns = new_object(rt);
    rt.object_set(ns, "isMainThread".into(), Value::Boolean(true));
    rt.object_set(ns, "isInternalThread".into(), Value::Boolean(false));
    rt.object_set(ns, "threadId".into(), Value::Number(0.0));
    rt.object_set(
        ns,
        "threadName".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "MainThread".to_string(),
        ))),
    );
    rt.object_set(ns, "parentPort".into(), Value::Null);
    rt.object_set(ns, "workerData".into(), Value::Null);
    let share = sym(rt, "worker.SHARE_ENV");
    rt.object_set(ns, "SHARE_ENV".into(), share);
    mk_obj(rt, ns, "resourceLimits");
    mk_obj(rt, ns, "locks");
    mk_ctor(rt, ns, "MessagePort");
    match rt.global_get("BroadcastChannel") {
        Value::Object(c) => rt.object_set(ns, "BroadcastChannel".into(), Value::Object(c)),
        _ => mk_ctor(rt, ns, "BroadcastChannel"),
    }
    {
        let mc = make_callable(rt, "MessageChannel", |rt, _a| {
            Ok(Value::Object(crate::ipc::make_message_channel(rt)))
        });
        rt.object_set(ns, "MessageChannel".into(), Value::Object(mc));
    }

    {
        let worker_ctor = make_callable(rt, "Worker", |rt, args| {
            spawn_worker_from_args(rt, args).map(Value::Object)
        });
        let pr = new_object(rt);
        rt.object_set(pr, "constructor".into(), Value::Object(worker_ctor));
        rt.object_set(worker_ctor, "prototype".into(), Value::Object(pr));
        rt.object_set(ns, "Worker".into(), Value::Object(worker_ctor));
    }
    for f in [
        "isMarkedAsUntransferable",
        "markAsUntransferable",
        "markAsUncloneable",
        "moveMessagePortToContext",
        "postMessageToThread",
    ] {
        register_method(rt, ns, f, |_rt, _a| Ok(Value::Undefined));
    }

    {
        let env_store = new_object(rt);
        rt.obj_mut(ns)
            .set_own_internal("__env_data".into(), Value::Object(env_store));
        register_method(rt, ns, "setEnvironmentData", move |rt, args| {
            let key = match args.first() {
                Some(v) => abstract_ops::to_string(v).as_str().to_string(),
                None => "undefined".to_string(),
            };
            let val = args.get(1).cloned().unwrap_or(Value::Undefined);
            rt.object_set(env_store, key, val);
            Ok(Value::Undefined)
        });
        register_method(rt, ns, "getEnvironmentData", move |rt, args| {
            let key = match args.first() {
                Some(v) => abstract_ops::to_string(v).as_str().to_string(),
                None => "undefined".to_string(),
            };
            Ok(rt.object_get(env_store, &key))
        });
    }

    register_method(rt, ns, "receiveMessageOnPort", |rt, args| {
        let port = match args.first() {
            Some(Value::Object(id)) => *id,
            _ => return Ok(Value::Undefined),
        };
        let q = match rt.object_get(port, "__msg_queue") {
            Value::Object(q) => q,
            _ => return Ok(Value::Undefined),
        };
        let len = rt.array_length(q);
        if len == 0 {
            return Ok(Value::Undefined);
        }
        let msg = rt.object_get(q, "0");
        for i in 1..len {
            let v = rt.object_get(q, &i.to_string());
            rt.object_set(q, (i - 1).to_string(), v);
        }
        rt.object_set(q, "length".into(), Value::Number((len - 1) as f64));
        let out = new_object(rt);
        rt.object_set(out, "message".into(), msg);
        Ok(Value::Object(out))
    });
    rt.define_global_property("worker_threads", Value::Object(ns));
    rt.define_global_property("__cruft_worker", Value::Object(ns));
}

fn spawn_worker_from_args(
    rt: &mut Runtime,
    args: &[Value],
) -> Result<ObjectRef, rusty_js_runtime::RuntimeError> {
    let options = match args.get(1) {
        Some(Value::Object(o)) => Some(*o),
        _ => None,
    };
    let eval_mode =
        options.is_some_and(|o| matches!(rt.object_get(o, "eval"), Value::Boolean(true)));
    let file = if eval_mode {
        let source = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => {
                return Err(rusty_js_runtime::RuntimeError::TypeError(
                    "Worker: eval source must be a string".into(),
                ))
            }
        };
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "cruft-worker-eval-{}-{nonce}.js",
            std::process::id()
        ));
        std::fs::write(&path, source).map_err(|e| {
            rusty_js_runtime::RuntimeError::TypeError(format!("Worker: eval write: {e}"))
        })?;
        path.to_string_lossy().to_string()
    } else {
        match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            Some(Value::Object(o)) => worker_file_url_to_path(rt, *o)?,
            _ => {
                return Err(rusty_js_runtime::RuntimeError::TypeError(
                    "Worker: filename must be a string or file URL".into(),
                ))
            }
        }
    };
    let wd = options
        .map(|o| rt.object_get(o, "workerData"))
        .unwrap_or(Value::Undefined);
    crate::ipc::spawn_inprocess_worker(rt, &file, &wd)
}

pub fn install_web_worker(rt: &mut Runtime) {
    let _ = rt.run_script(
        r#"
        (function () {
          if (globalThis.Worker && globalThis.Worker.__cruft_web_worker_projection) return;
          class Worker {
            constructor(url, options) {
              const w = new globalThis.__cruft_worker.Worker(url, options);
              Object.defineProperty(this, "__node_worker", { value: w, enumerable: false });
              Object.defineProperty(this, "__listeners", { value: Object.create(null), enumerable: false });
              this.onmessage = null;
              this.onmessageerror = null;
              this.onerror = null;
              this.ononline = null;
              this.onexit = null;
              w.on("message", (data) => this.dispatchEvent({ type: "message", data }));
              w.on("messageerror", (data) => this.dispatchEvent({ type: "messageerror", data }));
              w.on("error", (error) => this.dispatchEvent({ type: "error", error }));
              w.on("online", () => this.dispatchEvent({ type: "online" }));
              w.on("exit", (code) => this.dispatchEvent({ type: "exit", data: code }));
            }
            postMessage(data, transfer) { return this.__node_worker.postMessage(data, transfer); }
            terminate() { return this.__node_worker.terminate(); }
            addEventListener(type, fn) {
              if (typeof fn !== "function") return;
              (this.__listeners[type] || (this.__listeners[type] = [])).push(fn);
            }
            removeEventListener(type, fn) {
              const list = this.__listeners[type];
              if (!list) return;
              this.__listeners[type] = list.filter((candidate) => candidate !== fn);
            }
            dispatchEvent(event) {
              if (!event || !event.type) return true;
              const handler = this["on" + event.type];
              if (typeof handler === "function") handler.call(this, event);
              const list = this.__listeners[event.type] || [];
              for (const fn of list.slice()) fn.call(this, event);
              return true;
            }
          }
          function defineEventAccessor(name) {
            const slot = "__" + name;
            Object.defineProperty(Worker.prototype, name, {
              configurable: true,
              enumerable: true,
              get() { return this[slot] || null; },
              set(fn) { this[slot] = (fn === null || typeof fn === "function") ? fn : null; }
            });
          }
          defineEventAccessor("onmessage");
          defineEventAccessor("onmessageerror");
          defineEventAccessor("onerror");
          defineEventAccessor("ononline");
          defineEventAccessor("onexit");
          Object.defineProperty(Worker, "__cruft_web_worker_projection", { value: true });
          globalThis.Worker = Worker;
        })();
        "#,
        "file://__cruft_web_worker_bootstrap",
    );
}

fn worker_file_url_to_path(
    rt: &mut Runtime,
    object: rusty_js_runtime::value::ObjectRef,
) -> Result<String, rusty_js_runtime::RuntimeError> {
    for slot in ["__url_href__", "href"] {
        if let Value::String(href) = rt.object_get(object, slot) {
            let href = href.as_str();
            if let Some(rest) = href.strip_prefix("file://") {
                let path = rest.split_once(['?', '#']).map(|(p, _)| p).unwrap_or(rest);
                return Ok(worker_percent_decode_path(path));
            }
        }
    }
    Err(rusty_js_runtime::RuntimeError::TypeError(
        "Worker: filename must be a string or file URL".into(),
    ))
}

fn worker_percent_decode_path(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push(((hi << 4) | lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn install_cluster(rt: &mut Runtime) {
    let ns = new_object(rt);
    rt.object_set(ns, "isPrimary".into(), Value::Boolean(true));
    rt.object_set(ns, "isMaster".into(), Value::Boolean(true));
    rt.object_set(ns, "isWorker".into(), Value::Boolean(false));
    rt.object_set(ns, "SCHED_NONE".into(), Value::Number(1.0));
    rt.object_set(ns, "SCHED_RR".into(), Value::Number(2.0));
    rt.object_set(ns, "schedulingPolicy".into(), Value::Number(2.0));
    rt.object_set(ns, "_eventsCount".into(), Value::Number(0.0));
    rt.object_set(ns, "_maxListeners".into(), Value::Undefined);
    mk_obj(rt, ns, "_events");
    mk_obj(rt, ns, "settings");
    mk_obj(rt, ns, "workers");
    mk_ctor(rt, ns, "Worker");
    for f in ["disconnect", "fork", "setupMaster", "setupPrimary"] {
        register_method(rt, ns, f, |_rt, _a| Ok(Value::Undefined));
    }

    if let Value::Object(events_ctor) = rt.global_get("events") {
        if let Value::Object(ee_proto) = rt.object_get(events_ctor, "prototype") {
            rt.set_object_prototype_internal(ns, Some(ee_proto));
        }
    }
    rt.define_global_property("cluster", Value::Object(ns));
}

pub fn install_repl(rt: &mut Runtime) {
    let ns = new_object(rt);
    mk_ctor(rt, ns, "REPLServer");
    mk_ctor(rt, ns, "Recoverable");
    let sloppy = sym(rt, "repl.sloppy");
    let strict = sym(rt, "repl.strict");
    rt.object_set(ns, "REPL_MODE_SLOPPY".into(), sloppy);
    rt.object_set(ns, "REPL_MODE_STRICT".into(), strict);
    for f in ["start", "writer"] {
        register_method(rt, ns, f, |rt, _a| Ok(rt.current_this()));
    }
    register_method(rt, ns, "isValidSyntax", |_rt, _a| Ok(Value::Boolean(true)));
    rt.define_global_property("repl", Value::Object(ns));
}

pub fn install_trace_events(rt: &mut Runtime) {
    let ns = new_object(rt);
    register_method(rt, ns, "createTracing", |rt, _a| {
        let t = new_object(rt);
        rt.object_set(t, "__trace_events_tracing__".into(), Value::Boolean(true));
        register_method(rt, t, "enable", |rt, _a| Ok(rt.current_this()));
        register_method(rt, t, "disable", |rt, _a| Ok(rt.current_this()));
        rt.object_set(t, "enabled".into(), Value::Boolean(false));
        Ok(Value::Object(t))
    });
    register_method(rt, ns, "getEnabledCategories", |_rt, _a| {
        Ok(Value::Undefined)
    });
    rt.define_global_property("trace_events", Value::Object(ns));
}
