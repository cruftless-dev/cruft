
use crate::register::{make_callable, new_object, register_method};
use rusty_js_runtime::caps::{self, ModuleId};
use rusty_js_runtime::value::ObjectRef;
use rusty_js_runtime::{AgentId, HostEnqueuePhase, Object, Runtime, RuntimeError, Value};
use std::cell::RefCell;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::rc::Rc;
use std::str::FromStr;

const NET_LISTENERS_SLOT: &str = "__listeners";
const BLOCKLIST_RULES_SLOT: &str = "__blocklist_rules";

fn js_string(s: &str) -> Value {
    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(s)))
}

fn net_error(rt: &mut Runtime, name: &str, code: &str, msg: &str) -> RuntimeError {
    let message = format!("{code}: {msg}");
    match rusty_js_runtime::intrinsics::make_error_instance(rt, name, &message) {
        Some(id) => {
            rt.object_set(id, "code".into(), js_string(code));
            RuntimeError::Thrown(Value::Object(id))
        }
        None => RuntimeError::TypeError(message),
    }
}

fn invalid_arg_type(rt: &mut Runtime, msg: &str) -> RuntimeError {
    net_error(rt, "TypeError", "ERR_INVALID_ARG_TYPE", msg)
}

fn invalid_arg_value(rt: &mut Runtime, msg: &str) -> RuntimeError {
    net_error(rt, "TypeError", "ERR_INVALID_ARG_VALUE", msg)
}

fn out_of_range(rt: &mut Runtime, msg: &str) -> RuntimeError {
    net_error(rt, "RangeError", "ERR_OUT_OF_RANGE", msg)
}

fn make_string_array(rt: &mut Runtime, values: &[String]) -> ObjectRef {
    let mut arr = Object::new_array();
    for (idx, value) in values.iter().enumerate() {
        arr.set_own(idx.to_string(), js_string(value));
    }
    arr.set_own("length".into(), Value::Number(values.len() as f64));
    rt.alloc_object(arr)
}

fn blocklist_rules(rt: &Runtime, id: ObjectRef) -> Vec<String> {
    match rt.object_get(id, BLOCKLIST_RULES_SLOT) {
        Value::String(s) => s
            .as_str()
            .split('\n')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn set_blocklist_rules(rt: &mut Runtime, id: ObjectRef, rules: Vec<String>) {
    let joined = rules.join("\n");
    let arr = make_string_array(rt, &rules);
    rt.set_engine_sentinel(id, BLOCKLIST_RULES_SLOT, js_string(&joined));
    rt.object_set(id, "rules".into(), Value::Object(arr));
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BlockFamily {
    V4,
    V6,
}

fn family_from_arg(
    rt: &mut Runtime,
    v: Option<&Value>,
    default: BlockFamily,
) -> Result<BlockFamily, RuntimeError> {
    match v {
        None | Some(Value::Undefined) => Ok(default),
        Some(Value::String(s)) => match s.as_str().to_ascii_lowercase().as_str() {
            "ipv4" => Ok(BlockFamily::V4),
            "ipv6" => Ok(BlockFamily::V6),
            _ => Err(invalid_arg_value(rt, "invalid IP family")),
        },
        _ => Err(invalid_arg_type(rt, "family must be a string")),
    }
}

fn socket_address_parts(rt: &Runtime, id: ObjectRef) -> Option<(String, BlockFamily)> {
    if !rt.obj(id).has_own_str("__socket_address") {
        return None;
    }
    let address = match rt.object_get(id, "address") {
        Value::String(s) => s.as_str().to_string(),
        _ => return None,
    };
    let family = match rt.object_get(id, "family") {
        Value::String(s) if s.as_str().eq_ignore_ascii_case("ipv6") => BlockFamily::V6,
        _ => BlockFamily::V4,
    };
    Some((address, family))
}

fn address_arg(rt: &mut Runtime, v: Option<&Value>) -> Result<(String, BlockFamily), RuntimeError> {
    match v {
        Some(Value::String(s)) => {
            let raw = s.as_str().to_string();
            if Ipv4Addr::from_str(&raw).is_ok() {
                Ok((raw, BlockFamily::V4))
            } else if Ipv6Addr::from_str(&raw).is_ok() {
                Ok((raw, BlockFamily::V6))
            } else {
                Err(invalid_arg_value(rt, "invalid IP address"))
            }
        }
        Some(Value::Object(id)) => socket_address_parts(rt, *id)
            .ok_or_else(|| invalid_arg_type(rt, "address must be a string or SocketAddress")),
        _ => Err(invalid_arg_type(
            rt,
            "address must be a string or SocketAddress",
        )),
    }
}

fn normalize_addr(raw: &str, family: BlockFamily) -> Option<(BlockFamily, u128, String)> {
    match family {
        BlockFamily::V4 => Ipv4Addr::from_str(raw)
            .ok()
            .map(|ip| (BlockFamily::V4, u32::from(ip) as u128, ip.to_string())),
        BlockFamily::V6 => {
            let ip = Ipv6Addr::from_str(raw).ok()?;
            if let Some(v4) = ip.to_ipv4_mapped() {
                Some((BlockFamily::V4, u32::from(v4) as u128, v4.to_string()))
            } else {
                Some((BlockFamily::V6, u128::from(ip), ip.to_string()))
            }
        }
    }
}

fn family_label(family: BlockFamily) -> &'static str {
    match family {
        BlockFamily::V4 => "IPv4",
        BlockFamily::V6 => "IPv6",
    }
}

fn parse_rule(rule: &str) -> Option<(&str, BlockFamily, &str)> {
    let (kind, rest) = rule.split_once(": ")?;
    let (family_s, body) = rest.split_once(' ')?;
    let family = match family_s {
        "IPv4" => BlockFamily::V4,
        "IPv6" => BlockFamily::V6,
        _ => return None,
    };
    Some((kind, family, body))
}

fn blocklist_rule_matches(rule: &str, raw: &str, family: BlockFamily) -> bool {
    let Some((kind, rule_family, body)) = parse_rule(rule) else {
        return false;
    };
    let Some((input_family, input_num, _)) = normalize_addr(raw, family) else {
        return false;
    };
    if input_family != rule_family {
        return false;
    }
    match kind {
        "Address" => normalize_addr(body, rule_family)
            .map(|(_, n, _)| n == input_num)
            .unwrap_or(false),
        "Range" => {
            let Some((start, end)) = body.split_once('-') else {
                return false;
            };
            let Some((_, start_n, _)) = normalize_addr(start, rule_family) else {
                return false;
            };
            let Some((_, end_n, _)) = normalize_addr(end, rule_family) else {
                return false;
            };
            input_num >= start_n && input_num <= end_n
        }
        "Subnet" => {
            let Some((base, prefix_s)) = body.split_once('/') else {
                return false;
            };
            let Ok(prefix) = prefix_s.parse::<u32>() else {
                return false;
            };
            let bits = if rule_family == BlockFamily::V4 {
                32
            } else {
                128
            };
            if prefix > bits {
                return false;
            }
            let Some((_, base_n, _)) = normalize_addr(base, rule_family) else {
                return false;
            };
            let mask = if prefix == 0 {
                0
            } else {
                u128::MAX << (128 - prefix)
            };
            let shift = 128 - bits;
            ((input_num << shift) & mask) == ((base_n << shift) & mask)
        }
        _ => false,
    }
}

fn parse_json_string_array(input: &str) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let mut chars = input.trim().chars().peekable();
    if chars.next()? != '[' {
        return None;
    }
    loop {
        while matches!(chars.peek(), Some(c) if c.is_whitespace() || *c == ',') {
            chars.next();
        }
        match chars.peek() {
            Some(']') => {
                chars.next();
                break;
            }
            Some('"') => {
                chars.next();
                let mut s = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => {
                            let next = chars.next()?;
                            s.push(match next {
                                '"' => '"',
                                '\\' => '\\',
                                '/' => '/',
                                'n' => '\n',
                                'r' => '\r',
                                't' => '\t',
                                other => other,
                            });
                        }
                        other => s.push(other),
                    }
                }
                out.push(s);
            }
            _ => return None,
        }
    }
    Some(out)
}

fn port_from_value(v: Value) -> Option<u16> {
    match v {
        Value::Undefined | Value::Null => None,
        other => {
            let n = rusty_js_runtime::abstract_ops::to_number(&other);
            if n.is_finite() && n >= 0.0 && n <= u16::MAX as f64 {
                Some(n as u16)
            } else {
                None
            }
        }
    }
}

#[derive(Clone)]
struct ActiveNetServer {
    agent_id: AgentId,
    listener_handle: u64,
    realm: usize,
    server_object: ObjectRef,
}

#[derive(Clone)]
struct ActiveNetSocket {
    agent_id: AgentId,
    stream_id: u64,
    realm: usize,
    socket_object: ObjectRef,
    encoding: Option<String>,

    timeout_ms: Option<u64>,
    last_activity: std::time::Instant,
    timeout_fired: bool,
}

fn socket_mark_activity(rt: &Runtime, stream_id: u64) {
    let agent_id = rt.agent_id();
    NET_SOCKETS.with(|v| {
        for slot in v.borrow_mut().iter_mut().flatten() {
            if slot.agent_id == agent_id && slot.stream_id == stream_id {
                slot.last_activity = std::time::Instant::now();
                slot.timeout_fired = false;
            }
        }
    });
}

fn socket_set_timeout(rt: &Runtime, stream_id: u64, ms: u64) {
    let agent_id = rt.agent_id();
    NET_SOCKETS.with(|v| {
        for slot in v.borrow_mut().iter_mut().flatten() {
            if slot.agent_id == agent_id && slot.stream_id == stream_id {
                slot.timeout_ms = if ms == 0 { None } else { Some(ms) };
                slot.last_activity = std::time::Instant::now();
                slot.timeout_fired = false;
            }
        }
    });
}

thread_local! {
    static NET_SERVERS: RefCell<Vec<Option<ActiveNetServer>>> = const { RefCell::new(Vec::new()) };
    static NET_SOCKETS: RefCell<Vec<Option<ActiveNetSocket>>> = const { RefCell::new(Vec::new()) };
}

pub fn collect_roots(roots: &mut Vec<ObjectRef>) {
    collect_roots_for_agent(AgentId::DEFAULT, roots);
}

pub fn collect_roots_for_runtime(rt: &Runtime, roots: &mut Vec<ObjectRef>) {
    collect_roots_for_agent(rt.agent_id(), roots);
}

fn collect_roots_for_agent(agent_id: AgentId, roots: &mut Vec<ObjectRef>) {
    NET_SERVERS.with(|v| {
        for s in v.borrow().iter().flatten() {
            if s.agent_id != agent_id {
                continue;
            }
            roots.push(s.server_object);
        }
    });
    NET_SOCKETS.with(|v| {
        for s in v.borrow().iter().flatten() {
            if s.agent_id != agent_id {
                continue;
            }
            roots.push(s.socket_object);
        }
    });
}

fn put_server(s: ActiveNetServer) -> usize {
    NET_SERVERS.with(|v| {
        let mut v = v.borrow_mut();
        v.push(Some(s));
        v.len() - 1
    })
}
fn get_server(id: usize) -> Option<ActiveNetServer> {
    NET_SERVERS.with(|v| v.borrow().get(id).and_then(|s| s.clone()))
}
fn get_server_for_runtime(rt: &Runtime, id: usize) -> Option<ActiveNetServer> {
    let agent_id = rt.agent_id();
    get_server(id).filter(|server| server.agent_id == agent_id)
}
fn remove_server(id: usize) -> Option<ActiveNetServer> {
    NET_SERVERS.with(|v| v.borrow_mut().get_mut(id).and_then(|slot| slot.take()))
}
fn remove_server_for_runtime(rt: &Runtime, id: usize) -> Option<ActiveNetServer> {
    let agent_id = rt.agent_id();
    NET_SERVERS.with(|v| {
        let mut v = v.borrow_mut();
        let slot = v.get_mut(id)?;
        if slot
            .as_ref()
            .is_some_and(|server| server.agent_id == agent_id)
        {
            slot.take()
        } else {
            None
        }
    })
}
fn put_socket(s: ActiveNetSocket) -> usize {
    NET_SOCKETS.with(|v| {
        let mut v = v.borrow_mut();
        v.push(Some(s));
        v.len() - 1
    })
}
fn remove_socket(id: usize) -> Option<ActiveNetSocket> {
    NET_SOCKETS.with(|v| v.borrow_mut().get_mut(id).and_then(|slot| slot.take()))
}

pub(crate) fn remove_socket_by_stream(rt: &Runtime, stream_id: u64) -> Vec<usize> {
    let agent_id = rt.agent_id();
    NET_SOCKETS.with(|v| {
        let mut removed = Vec::new();
        for (idx, slot) in v.borrow_mut().iter_mut().enumerate() {
            if slot
                .as_ref()
                .map(|s| s.agent_id == agent_id && s.stream_id == stream_id)
                .unwrap_or(false)
            {
                *slot = None;
                removed.push(idx);
            }
        }
        removed
    })
}

fn ee_proto(rt: &Runtime) -> Option<ObjectRef> {
    match rt.global_get("events") {
        Value::Object(ctor) => match rt.object_get(ctor, "prototype") {
            Value::Object(p) => Some(p),
            _ => None,
        },
        _ => None,
    }
}

fn emitter_unwrap(rt: &Runtime, slot: &Value) -> Option<(Value, bool)> {
    match slot {
        Value::Object(id) => {
            let once = rt.object_get(*id, "__once");
            if rt.is_callable(&once) {
                Some((once, true))
            } else if rt.is_callable(slot) {
                Some((slot.clone(), false))
            } else {
                None
            }
        }
        v if rt.is_callable(v) => Some((v.clone(), false)),
        _ => None,
    }
}

pub(crate) fn install_emitter(rt: &mut Runtime, obj: ObjectRef) {
    if let Some(p) = ee_proto(rt) {
        let _ = rt.ordinary_set_prototype_of(obj, Some(p));
    }
}

pub(crate) fn install_emitter_methods_own(rt: &mut Runtime, obj: ObjectRef) {
    install_emitter(rt, obj);
    if let Some(p) = ee_proto(rt) {
        for m in [
            "on",
            "once",
            "emit",
            "addListener",
            "removeListener",
            "removeAllListeners",
            "prependListener",
            "prependOnceListener",
            "off",
            "listeners",
            "rawListeners",
            "listenerCount",
            "eventNames",
            "setMaxListeners",
            "getMaxListeners",
        ] {
            let v = rt.object_get(p, m);
            if rt.is_callable(&v) {
                rt.object_set(obj, m.to_string(), v);
            }
        }
    }
}

fn make_ctor_subclassable_ee(rt: &mut Runtime, ctor: ObjectRef) {
    let proto = new_object(rt);
    if let Some(p) = ee_proto(rt) {
        rt.set_object_prototype_internal(proto, Some(p));
    }
    rt.object_set(proto, "constructor".into(), Value::Object(ctor));
    rt.object_set(ctor, "prototype".into(), Value::Object(proto));
}

pub(crate) fn net_emit(rt: &mut Runtime, obj: ObjectRef, event: &str, args: Vec<Value>) -> bool {
    let registry = match rt.object_get(obj, NET_LISTENERS_SLOT) {
        Value::Object(id) => id,
        _ => return false,
    };
    let mut to_call: Vec<(Value, bool)> = Vec::new();
    let slot = rt.object_get(registry, event);
    let mut array_len = None;
    match slot {
        Value::Object(arr)
            if matches!(
                rt.obj(arr).internal_kind,
                rusty_js_runtime::value::InternalKind::Array
            ) =>
        {
            let len = rt.array_length(arr);
            array_len = Some((arr, len));
            for i in 0..len {
                let slot = rt.object_get(arr, &i.to_string());
                if let Some(pair) = emitter_unwrap(rt, &slot) {
                    to_call.push(pair);
                }
            }
        }
        other => {
            if let Some(pair) = emitter_unwrap(rt, &other) {
                to_call.push(pair);
            }
        }
    }
    if to_call.is_empty() {
        return false;
    }
    for (cb, _) in &to_call {
        let _ = rt.call_function(cb.clone(), Value::Object(obj), args.clone());
    }

    if to_call.iter().any(|(_, once)| *once) {
        if let Some((arr, len)) = array_len {
            let mut kept: Vec<Value> = Vec::new();
            for i in 0..len {
                let slot = rt.object_get(arr, &i.to_string());
                let is_once = matches!(&slot, Value::Object(id) if rt.is_callable(&rt.object_get(*id, "__once")));
                if !is_once {
                    kept.push(slot);
                }
            }
            for (i, v) in kept.iter().enumerate() {
                rt.object_set(arr, i.to_string(), v.clone());
            }
            for i in kept.len()..(len as usize) {
                rt.object_set(arr, i.to_string(), Value::Undefined);
            }
            rt.object_set(arr, "length".into(), Value::Number(kept.len() as f64));
        } else {
            rt.object_set(registry, event.to_string(), Value::Undefined);
        }
    }
    true
}

fn string_arg(rt: &Runtime, args: &[Value], i: usize) -> String {
    match args.get(i) {
        Some(Value::String(s)) => s.as_str().to_string(),
        Some(v) => rusty_js_runtime::abstract_ops::to_string(v)
            .as_str()
            .to_string(),
        None => String::new(),
    }
}

pub(crate) fn net_buffer_from_bytes(rt: &mut Runtime, bytes: &[u8]) -> Value {
    crate::node_stubs::intrinsic_buffer_from_bytes(rt, bytes)
}

fn value_to_bytes(rt: &Runtime, v: &Value, encoding: Option<&str>) -> Vec<u8> {
    match v {
        Value::String(s) => s.as_str().as_bytes().to_vec(),
        _ if encoding.is_some() => rusty_js_runtime::abstract_ops::to_string(v)
            .as_str()
            .as_bytes()
            .to_vec(),
        Value::Object(id) => {
            let len = match rt.object_get(*id, "length") {
                Value::Number(n) if n >= 0.0 => n as usize,
                _ => 0,
            };
            (0..len)
                .map(|i| match rt.object_get(*id, &i.to_string()) {
                    Value::Number(n) => n as i64 as u8,
                    _ => 0,
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

fn emit_socket_close(rt: &mut Runtime, sock: ObjectRef) {
    let had_error = matches!(rt.object_get(sock, "__net_had_error"), Value::Boolean(true));
    net_emit(rt, sock, "close", vec![Value::Boolean(had_error)]);
}

fn net_error_object(rt: &mut Runtime, code: &str, message: &str) -> Value {
    let o = new_object(rt);
    rt.object_set(
        o,
        "message".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            message.to_string(),
        ))),
    );
    rt.object_set(
        o,
        "code".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            code.to_string(),
        ))),
    );
    Value::Object(o)
}

fn net_listen_error_code(message: &str) -> &'static str {
    let lower = message.to_ascii_lowercase();
    if lower.contains("address already in use") || lower.contains("addrinuse") {
        "EADDRINUSE"
    } else if lower.contains("cannot assign requested address")
        || lower.contains("address not available")
        || lower.contains("addrnotavail")
    {
        "EADDRNOTAVAIL"
    } else if lower.contains("permission denied") || lower.contains("access") {
        "EACCES"
    } else {
        "ERR_SERVER_LISTEN"
    }
}

fn split_host_port(addr: &str) -> (String, u16) {
    match addr.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse().unwrap_or(0)),
        None => (addr.to_string(), 0),
    }
}

pub(crate) fn make_socket(rt: &mut Runtime, stream_id: u64, realm: usize) -> ObjectRef {
    let obj = new_object(rt);
    install_emitter(rt, obj);
    rt.set_engine_sentinel(obj, "__cruft_net_socket", Value::Boolean(true));
    rt.object_set(
        obj,
        "readyState".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from("open"))),
    );
    rt.object_set(obj, "readable".into(), Value::Boolean(true));
    rt.object_set(obj, "writable".into(), Value::Boolean(true));
    rt.object_set(obj, "bytesRead".into(), Value::Number(0.0));
    rt.object_set(obj, "bytesWritten".into(), Value::Number(0.0));

    let readable_state = rt.alloc_object(Object::new_ordinary());
    rt.object_set(readable_state, "endEmitted".into(), Value::Boolean(false));
    rt.object_set(readable_state, "length".into(), Value::Number(0.0));
    rt.object_set(obj, "_readableState".into(), Value::Object(readable_state));
    let writable_state = rt.alloc_object(Object::new_ordinary());
    rt.object_set(writable_state, "ended".into(), Value::Boolean(false));
    rt.object_set(writable_state, "finished".into(), Value::Boolean(false));
    rt.object_set(obj, "_writableState".into(), Value::Object(writable_state));
    if let Ok(peer) = rusty_sockets::stream_peer_addr(stream_id) {
        let (h, p) = split_host_port(&peer);
        rt.object_set(
            obj,
            "remoteAddress".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(h))),
        );
        rt.object_set(obj, "remotePort".into(), Value::Number(p as f64));
        rt.object_set(
            obj,
            "remoteFamily".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from("IPv4"))),
        );
    }
    if let Ok(local) = rusty_sockets::stream_local_addr(stream_id) {
        let (h, p) = split_host_port(&local);
        rt.object_set(
            obj,
            "localAddress".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(h))),
        );
        rt.object_set(obj, "localPort".into(), Value::Number(p as f64));
    }
    rt.set_engine_sentinel(obj, "__net_stream_id", Value::Number(stream_id as f64));

    rt.object_set(obj, "connecting".into(), Value::Boolean(false));
    rt.object_set(obj, "pending".into(), Value::Boolean(false));
    rt.object_set(obj, "destroyed".into(), Value::Boolean(false));

    register_method(rt, obj, "address", |rt, _a| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let addr = rt.object_get(this, "localAddress");
        let port = rt.object_get(this, "localPort");
        let family = match &addr {
            Value::String(s) if s.as_str().contains(':') => "IPv6",
            _ => "IPv4",
        };
        let o = new_object(rt);
        rt.object_set(o, "address".into(), addr);
        rt.object_set(
            o,
            "family".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(family))),
        );
        rt.object_set(o, "port".into(), port);
        Ok(Value::Object(o))
    });

    register_method(rt, obj, "write", move |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Boolean(false)),
        };
        let sid = stream_id_of(rt, this);
        let enc = encoding_of(rt, this);
        let bytes = match args.first() {
            Some(v) => value_to_bytes(rt, v, enc.as_deref()),
            None => Vec::new(),
        };
        if let Some(sid) = sid {
            if rusty_sockets::stream_write_all(sid, &bytes).is_ok() {
                let prior = num_prop(rt, this, "bytesWritten");
                rt.object_set(
                    this,
                    "bytesWritten".into(),
                    Value::Number(prior + bytes.len() as f64),
                );
            }
            socket_mark_activity(rt, sid);
        }

        if let Some(cb) = args.iter().rev().find(|v| rt.is_callable(v)).cloned() {
            let _ = rt.call_function(cb, Value::Undefined, Vec::new());
        }
        Ok(Value::Boolean(true))
    });
    register_method(rt, obj, "end", move |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let sid = stream_id_of(rt, this);
        let enc = encoding_of(rt, this);
        if let Some(v) = args.first() {
            if !rt.is_callable(v) {
                let bytes = value_to_bytes(rt, v, enc.as_deref());
                if let Some(sid) = sid {
                    let _ = rusty_sockets::stream_write_all(sid, &bytes);
                }
            }
        }

        if let Some(sid) = sid {
            let _ = rusty_sockets::stream_shutdown_write(sid);
        }
        if let Value::Object(this) = rt.current_this() {
            rt.object_set(this, "writable".into(), Value::Boolean(false));
            if let Value::Object(ws) = rt.object_get(this, "_writableState") {
                rt.object_set(ws, "ended".into(), Value::Boolean(true));
                rt.object_set(ws, "finished".into(), Value::Boolean(true));
            }
        }
        net_emit(rt, this, "finish", Vec::new());
        Ok(Value::Undefined)
    });
    register_method(rt, obj, "destroy", |rt, a| {
        if let Value::Object(this) = rt.current_this() {

            let destroy_error = match a.first() {
                Some(v @ Value::Object(_)) => Some(v.clone()),
                _ => None,
            };
            if destroy_error.is_some() {
                rt.object_set(this, "__net_had_error".into(), Value::Boolean(true));
            }
            if let Some(sid) = stream_id_of(rt, this) {

                remove_socket_by_stream(rt, sid);

                let _ = rusty_sockets::handle_close(sid);
            }
            rt.object_set(
                this,
                "readyState".into(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from("closed"))),
            );
            rt.object_set(this, "readable".into(), Value::Boolean(false));
            rt.object_set(this, "writable".into(), Value::Boolean(false));
            rt.object_set(this, "destroyed".into(), Value::Boolean(true));
            if let Value::Object(rs) = rt.object_get(this, "_readableState") {
                rt.object_set(rs, "endEmitted".into(), Value::Boolean(true));
                rt.object_set(rs, "length".into(), Value::Number(0.0));
            }
            if let Value::Object(ws) = rt.object_get(this, "_writableState") {
                rt.object_set(ws, "ended".into(), Value::Boolean(true));
                rt.object_set(ws, "finished".into(), Value::Boolean(true));
            }
            if let Some(err) = destroy_error {
                net_emit(rt, this, "error", vec![err]);
            }
            emit_socket_close(rt, this);
        }
        Ok(rt.current_this())
    });
    register_method(rt, obj, "setEncoding", |rt, args| {
        if let Value::Object(this) = rt.current_this() {
            let enc = string_arg(rt, args, 0);
            rt.set_engine_sentinel(
                this,
                "__net_encoding",
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(enc))),
            );
        }
        Ok(rt.current_this())
    });
    register_method(rt, obj, "setTimeout", |rt, args| {

        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(rt.current_this()),
        };
        let ms = match args.first() {
            Some(Value::Number(n)) => *n as u64,
            _ => 0,
        };
        rt.object_set(this, "timeout".into(), Value::Number(ms as f64));
        if let Some(sid) = stream_id_of(rt, this) {
            socket_set_timeout(rt, sid, ms);
        }
        if let Some(cb) = args.iter().skip(1).find(|v| rt.is_callable(v)).cloned() {

            if let Value::Object(registry) = rt.object_get(this, NET_LISTENERS_SLOT) {
                let arr = match rt.object_get(registry, "timeout") {
                    Value::Object(a) => a,
                    _ => {
                        let a = rt.alloc_object(Object::new_array());
                        rt.object_set(a, "length".into(), Value::Number(0.0));
                        rt.object_set(registry, "timeout".into(), Value::Object(a));
                        a
                    }
                };
                let len = rt.array_length(arr);
                rt.object_set(arr, len.to_string(), cb);
                rt.object_set(arr, "length".into(), Value::Number((len + 1) as f64));
            }
        }
        Ok(rt.current_this())
    });
    register_method(rt, obj, "read", |rt, _args| {

        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Null),
        };
        match rt.object_get(this, "__net_pending_read") {
            Value::Object(chunk) => {
                rt.object_set(this, "__net_pending_read".into(), Value::Undefined);
                if let Value::Object(rs) = rt.object_get(this, "_readableState") {
                    rt.object_set(rs, "length".into(), Value::Number(0.0));
                }
                Ok(Value::Object(chunk))
            }
            _ => Ok(Value::Null),
        }
    });
    for noop in [
        "pause",
        "resume",
        "setNoDelay",
        "setKeepAlive",
        "cork",
        "uncork",
        "ref",
        "unref",
    ] {
        register_method(rt, obj, noop, |rt, _a| Ok(rt.current_this()));
    }

    let socket_idx = put_socket(ActiveNetSocket {
        agent_id: rt.agent_id(),
        stream_id,
        realm,
        socket_object: obj,
        encoding: None,
        timeout_ms: None,
        last_activity: std::time::Instant::now(),
        timeout_fired: false,
    });

    crate::stream::install_async_iterator(rt, obj);
    obj
}

fn make_pending_unix_socket(rt: &mut Runtime, path: String) -> ObjectRef {
    let obj = new_object(rt);
    install_emitter(rt, obj);
    rt.set_engine_sentinel(obj, "__cruft_net_socket", Value::Boolean(true));
    rt.object_set(
        obj,
        "readyState".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from("opening"))),
    );
    rt.object_set(obj, "connecting".into(), Value::Boolean(true));
    rt.object_set(obj, "bytesRead".into(), Value::Number(0.0));
    rt.object_set(obj, "bytesWritten".into(), Value::Number(0.0));
    rt.object_set(
        obj,
        "path".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            path.clone(),
        ))),
    );
    for noop in [
        "write",
        "end",
        "pause",
        "resume",
        "setNoDelay",
        "setKeepAlive",
        "ref",
        "unref",
    ] {
        register_method(rt, obj, noop, |rt, _a| Ok(rt.current_this()));
    }
    register_method(rt, obj, "destroy", |rt, a| {
        if let Value::Object(this) = rt.current_this() {
            let destroy_error = match a.first() {
                Some(v @ Value::Object(_)) => Some(v.clone()),
                _ => None,
            };
            if destroy_error.is_some() {
                rt.object_set(this, "__net_had_error".into(), Value::Boolean(true));
            }
            rt.object_set(
                this,
                "readyState".into(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from("closed"))),
            );
            rt.object_set(this, "connecting".into(), Value::Boolean(false));
            if let Some(err) = destroy_error {
                net_emit(rt, this, "error", vec![err]);
            }
            emit_socket_close(rt, this);
        }
        Ok(rt.current_this())
    });
    let socket = obj;
    rt.enqueue_host_phase_rooted(
        HostEnqueuePhase::HostCompletionMacrotask,
        "net.unix connect error",
        vec![socket],
        move |rt| {
            rt.object_set(
                socket,
                "readyState".into(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from("closed"))),
            );
            rt.object_set(socket, "connecting".into(), Value::Boolean(false));
            let err = net_error_object(rt, "ENOENT", &format!("connect ENOENT {path}"));
            rt.object_set(socket, "__net_had_error".into(), Value::Boolean(true));
            net_emit(rt, socket, "error", vec![err]);
            Ok(())
        },
    );
    obj
}

fn stream_id_of(rt: &Runtime, obj: ObjectRef) -> Option<u64> {
    match rt.object_get(obj, "__net_stream_id") {
        Value::Number(n) => Some(n as u64),
        _ => None,
    }
}
fn encoding_of(rt: &Runtime, obj: ObjectRef) -> Option<String> {
    match rt.object_get(obj, "__net_encoding") {
        Value::String(s) => Some(s.as_str().to_string()),
        _ => None,
    }
}
fn num_prop(rt: &Runtime, obj: ObjectRef, key: &str) -> f64 {
    match rt.object_get(obj, key) {
        Value::Number(n) => n,
        _ => 0.0,
    }
}

fn make_server(rt: &mut Runtime, connection_listener: Value, net_cap: caps::Net) -> ObjectRef {
    let obj = new_object(rt);
    install_emitter(rt, obj);

    if let Value::Object(sp) = rt.global_get("__cruft_net_server_proto") {
        if let Some(ee) = ee_proto(rt) {
            if !matches!(rt.obj(sp).proto, Some(p) if p == ee) {
                rt.set_object_prototype_internal(sp, Some(ee));
            }
        }
        let _ = rt.ordinary_set_prototype_of(obj, Some(sp));
    }
    rt.object_set(obj, "listening".into(), Value::Boolean(false));
    if rt.is_callable(&connection_listener) {

        let registry = match rt.object_get(obj, NET_LISTENERS_SLOT) {
            Value::Object(id) => id,
            _ => {
                let bag = rt.alloc_object(Object::new_ordinary());
                rt.object_set(obj, NET_LISTENERS_SLOT.into(), Value::Object(bag));
                bag
            }
        };
        let arr = rt.alloc_object(Object::new_array());
        rt.object_set(arr, "0".into(), connection_listener);
        rt.object_set(arr, "length".into(), Value::Number(1.0));
        rt.object_set(registry, "connection".into(), Value::Object(arr));
    }

    let cap = net_cap;
    register_method(rt, obj, "listen", move |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => {
                return Err(RuntimeError::TypeError(
                    "server.listen: invalid receiver".into(),
                ))
            }
        };

        let (port, host) = parse_listen_args(rt, args);
        rt.caps
            .require_net(
                &cap,
                caps::NetOp::Listen {
                    host: host.clone(),
                    port,
                },
                &ModuleId::builtin("node:net"),
            )
            .map_err(|e| RuntimeError::TypeError(e.to_string()))?;
        let (listener_handle, bound_addr) =
            match rusty_sockets::listener_bind_async(&format!("{host}:{port}")) {
                Ok(bound) => bound,
                Err(e) => {
                    let message = format!("server.listen: {e:?}");
                    let code = net_listen_error_code(&message);
                    rt.enqueue_host_phase_rooted(
                        HostEnqueuePhase::HostCompletionMacrotask,
                        "net.server listen error",
                        vec![this],
                        move |rt| {
                            let err = net_error_object(rt, code, &message);
                            rt.object_set(this, "__net_had_error".into(), Value::Boolean(true));
                            net_emit(rt, this, "error", vec![err]);
                            Ok(())
                        },
                    );
                    return Ok(Value::Object(this));
                }
            };
        rt.set_engine_sentinel(
            this,
            "__net_bound_addr",
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(bound_addr))),
        );
        rt.object_set(this, "listening".into(), Value::Boolean(true));
        let realm = rt.current_realm;
        let server_idx = put_server(ActiveNetServer {
            agent_id: rt.agent_id(),
            listener_handle,
            realm,
            server_object: this,
        });

        rt.enqueue_host_phase_rooted(
            HostEnqueuePhase::HostCompletionMacrotask,
            "net.server listening",
            vec![this],
            move |rt| {
                net_emit(rt, this, "listening", Vec::new());
                Ok(())
            },
        );
        if let Some(cb) = args.iter().find(|v| rt.is_callable(v)).cloned() {
            let mut roots = vec![this];
            if let Value::Object(o) = &cb {
                roots.push(*o);
            }
            rt.enqueue_host_phase_rooted(
                HostEnqueuePhase::HostCompletionMacrotask,
                "net.server listen cb",
                roots,
                move |rt| {
                    let _ = rt.call_function(cb, Value::Object(this), Vec::new());
                    Ok(())
                },
            );
        }
        Ok(Value::Object(this))
    });
    register_method(rt, obj, "address", |rt, _a| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Null),
        };
        let bound = match rt.object_get(this, "__net_bound_addr") {
            Value::String(s) => s.as_str().to_string(),
            _ => return Ok(Value::Null),
        };
        let (host, port) = split_host_port(&bound);
        let out = new_object(rt);
        rt.object_set(
            out,
            "address".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(host))),
        );
        rt.object_set(
            out,
            "family".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from("IPv4"))),
        );
        rt.object_set(out, "port".into(), Value::Number(port as f64));
        Ok(Value::Object(out))
    });

    register_method(rt, obj, "getConnections", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let cb = args.first().cloned().unwrap_or(Value::Undefined);
        if !rt.is_callable(&cb) {
            return Ok(Value::Undefined);
        }
        let socket_objs: Vec<ObjectRef> = NET_SOCKETS.with(|v| {
            let agent_id = rt.agent_id();
            v.borrow()
                .iter()
                .filter_map(|s| {
                    s.as_ref()
                        .and_then(|s| (s.agent_id == agent_id).then_some(s.socket_object))
                })
                .collect()
        });
        let count = socket_objs
            .iter()
            .filter(|&&so| matches!(rt.object_get(so, "server"), Value::Object(srv) if srv == this))
            .count();
        rt.enqueue_host_phase_rooted(
            HostEnqueuePhase::HostCompletionMacrotask,
            "net.server getConnections cb",
            vec![this],
            move |rt| {
                let _ = rt.call_function(
                    cb,
                    Value::Null,
                    vec![Value::Null, Value::Number(count as f64)],
                );
                Ok(())
            },
        );
        Ok(Value::Object(this))
    });
    register_method(rt, obj, "close", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };

        let handle = NET_SERVERS.with(|v| {
            let agent_id = rt.agent_id();
            v.borrow()
                .iter()
                .flatten()
                .find(|s| s.agent_id == agent_id && s.server_object == this)
                .map(|s| s.listener_handle)
        });
        let was_running = handle.is_some();
        if let Some(h) = handle {
            let _ = rusty_sockets::listener_stop_async(h);
            let idx = NET_SERVERS.with(|v| {
                let agent_id = rt.agent_id();
                v.borrow().iter().position(|s| {
                    s.as_ref()
                        .map(|s| s.agent_id == agent_id && s.server_object == this)
                        .unwrap_or(false)
                })
            });
            if let Some(idx) = idx {
                remove_server_for_runtime(rt, idx);
            }
        }
        rt.object_set(this, "listening".into(), Value::Boolean(false));
        if let Some(cb) = args.iter().find(|v| rt.is_callable(v)).cloned() {

            let err = if was_running {
                Value::Undefined
            } else {
                net_error_object(rt, "ERR_SERVER_NOT_RUNNING", "Server is not running.")
            };
            let _ = rt.call_function(cb, Value::Object(this), vec![err]);
        }
        net_emit(rt, this, "close", Vec::new());
        Ok(Value::Object(this))
    });
    for noop in ["ref", "unref"] {
        register_method(rt, obj, noop, |rt, _a| Ok(rt.current_this()));
    }
    obj
}

fn parse_listen_args(rt: &Runtime, args: &[Value]) -> (u16, String) {
    match args.first() {
        Some(Value::Number(n)) => {
            let host = match args.get(1) {
                Some(Value::String(s)) => s.as_str().to_string(),
                _ => "127.0.0.1".to_string(),
            };
            (*n as u16, host)
        }
        Some(Value::Object(o)) => {
            let port = match rt.object_get(*o, "port") {
                Value::Number(n) => n as u16,
                _ => 0,
            };
            let host = match rt.object_get(*o, "host") {
                Value::String(s) => s.as_str().to_string(),
                _ => "127.0.0.1".to_string(),
            };
            (port, host)
        }
        _ => (0, "127.0.0.1".to_string()),
    }
}

fn do_connect(
    rt: &mut Runtime,
    args: &[Value],
    net_cap: &caps::Net,
) -> Result<Value, RuntimeError> {

    let (port, host) = match args.first() {
        Some(Value::String(path)) => {
            return Ok(Value::Object(make_pending_unix_socket(
                rt,
                path.as_str().to_string(),
            )))
        }
        Some(Value::Number(n)) => {
            let host = match args.get(1) {
                Some(Value::String(s)) => s.as_str().to_string(),
                _ => "127.0.0.1".to_string(),
            };
            (*n as u16, host)
        }
        Some(Value::Object(o)) => {
            if let Value::String(path) = rt.object_get(*o, "path") {
                return Ok(Value::Object(make_pending_unix_socket(
                    rt,
                    path.as_str().to_string(),
                )));
            }
            let port = port_from_value(rt.object_get(*o, "port")).unwrap_or(0);
            let host = match rt.object_get(*o, "host") {
                Value::String(s) => s.as_str().to_string(),
                _ => "127.0.0.1".to_string(),
            };
            (port, host)
        }
        _ => {
            return Err(RuntimeError::TypeError(
                "net.connect: port or options required".into(),
            ))
        }
    };
    rt.caps
        .require_net(
            net_cap,
            caps::NetOp::Connect {
                host: host.clone(),
                port,
            },
            &ModuleId::builtin("node:net"),
        )
        .map_err(|e| RuntimeError::TypeError(e.to_string()))?;
    let stream_id = match rusty_sockets::stream_connect(&format!("{host}:{port}")) {
        Ok(id) => id,
        Err(e) => {

            let dbg = format!("{e:?}").to_ascii_lowercase();
            let (code, errno) = if dbg.contains("refused") {
                ("ECONNREFUSED", -111)
            } else if dbg.contains("not found") || dbg.contains("resolve") {
                ("ENOTFOUND", -3008)
            } else if dbg.contains("timed out") || dbg.contains("timeout") {
                ("ETIMEDOUT", -110)
            } else {
                ("ECONNREFUSED", -111)
            };
            let sock = new_object(rt);
            install_emitter(rt, sock);
            rt.object_set(sock, "connecting".into(), Value::Boolean(true));
            for noop in [
                "write",
                "end",
                "pause",
                "resume",
                "setNoDelay",
                "setKeepAlive",
                "ref",
                "unref",
            ] {
                register_method(rt, sock, noop, |rt, _a| Ok(rt.current_this()));
            }
            register_method(rt, sock, "destroy", |rt, _a| Ok(rt.current_this()));
            let host2 = host.clone();
            rt.enqueue_host_phase_rooted(
                HostEnqueuePhase::HostCompletionMacrotask,
                "net.socket connect error",
                vec![sock],
                move |rt| {
                    rt.object_set(sock, "connecting".into(), Value::Boolean(false));
                    rt.object_set(
                        sock,
                        "readyState".into(),
                        Value::String(Rc::new(rusty_js_runtime::value::JsString::from("closed"))),
                    );
                    let err = net_error_object(rt, code, &format!("connect {code} {host2}:{port}"));
                    if let Value::Object(eid) = err {
                        rt.object_set(eid, "errno".into(), Value::Number(errno as f64));
                        rt.object_set(eid, "syscall".into(), js_string("connect"));
                        rt.object_set(eid, "address".into(), js_string(&host2));
                        rt.object_set(eid, "port".into(), Value::Number(port as f64));
                    }
                    rt.object_set(sock, "__net_had_error".into(), Value::Boolean(true));
                    rt.object_set(sock, "destroyed".into(), Value::Boolean(true));
                    rt.object_set(sock, "readable".into(), Value::Boolean(false));
                    rt.object_set(sock, "writable".into(), Value::Boolean(false));
                    net_emit(rt, sock, "error", vec![err]);

                    emit_socket_close(rt, sock);
                    Ok(())
                },
            );
            return Ok(Value::Object(sock));
        }
    };
    let realm = rt.current_realm;
    let sock = make_socket(rt, stream_id, realm);

    rt.enqueue_host_phase_rooted(
        HostEnqueuePhase::HostCompletionMacrotask,
        "net.socket connect",
        vec![sock],
        move |rt| {
            net_emit(rt, sock, "connect", Vec::new());
            Ok(())
        },
    );
    if let Some(cb) = args.iter().find(|v| rt.is_callable(v)).cloned() {
        let mut roots = vec![sock];
        if let Value::Object(o) = &cb {
            roots.push(*o);
        }
        rt.enqueue_host_phase_rooted(
            HostEnqueuePhase::HostCompletionMacrotask,
            "net.socket connect cb",
            roots,
            move |rt| {
                let _ = rt.call_function(cb, Value::Object(sock), Vec::new());
                Ok(())
            },
        );
    }
    Ok(Value::Object(sock))
}

pub fn harvest_socket_io(rt: &mut Runtime) -> Result<bool, RuntimeError> {
    poll_active_sockets(rt, false)
}

pub fn poll_io(rt: &mut Runtime) -> Result<bool, RuntimeError> {

    let agent_id = rt.agent_id();
    let servers: Vec<(usize, u64, usize, ObjectRef)> = NET_SERVERS.with(|v| {
        v.borrow()
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                s.as_ref().and_then(|s| {
                    (s.agent_id == agent_id).then_some((
                        i,
                        s.listener_handle,
                        s.realm,
                        s.server_object,
                    ))
                })
            })
            .collect()
    });
    for (sid, handle, realm, server_object) in servers {
        match rusty_sockets::listener_poll(handle, 0) {
            Ok(Some(rusty_sockets::AsyncEvent::Connection { stream_id, .. })) => {
                let prior = rt.enter_realm(realm);
                let socket = make_socket(rt, stream_id, realm);
                rt.object_set(socket, "server".into(), Value::Object(server_object));
                net_emit(rt, server_object, "connection", vec![Value::Object(socket)]);
                rt.exit_realm(prior);
                return Ok(true);
            }
            Ok(Some(rusty_sockets::AsyncEvent::Closed))
            | Ok(Some(rusty_sockets::AsyncEvent::Error(_))) => {
                remove_server_for_runtime(rt, sid);
            }

            Ok(Some(rusty_sockets::AsyncEvent::Readable { .. })) => {}
            Ok(None) => {}
            Err(_) => {
                remove_server_for_runtime(rt, sid);
            }
        }
    }

    poll_active_sockets(rt, true)
}

fn poll_active_sockets(rt: &mut Runtime, sticky_idle: bool) -> Result<bool, RuntimeError> {

    let agent_id = rt.agent_id();
    let sockets: Vec<(usize, u64, usize, ObjectRef, Option<String>)> = NET_SOCKETS.with(|v| {
        v.borrow()
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                s.as_ref().and_then(|s| {
                    (s.agent_id == agent_id).then_some((
                        i,
                        s.stream_id,
                        s.realm,
                        s.socket_object,
                        s.encoding.clone(),
                    ))
                })
            })
            .collect()
    });
    let has_sockets = !sockets.is_empty();
    for (idx, stream_id, realm, socket_object, _enc) in sockets {

        let _ = rusty_sockets::stream_set_nonblocking(stream_id, true);
        let read = rusty_sockets::stream_try_read(stream_id, 65536);
        let _ = rusty_sockets::stream_set_nonblocking(stream_id, false);
        match read {
            Ok(Some(bytes)) if !bytes.is_empty() => {
                let prior = rt.enter_realm(realm);
                let n = bytes.len();
                let prior_read = num_prop(rt, socket_object, "bytesRead");
                rt.object_set(
                    socket_object,
                    "bytesRead".into(),
                    Value::Number(prior_read + n as f64),
                );
                let enc = encoding_of(rt, socket_object);
                let chunk = match enc {
                    Some(e) if matches!(e.as_str(), "utf8" | "utf-8") => {
                        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                            String::from_utf8_lossy(&bytes).into_owned(),
                        )))
                    }
                    _ => net_buffer_from_bytes(rt, &bytes),
                };
                rt.object_set(socket_object, "__net_pending_read".into(), chunk.clone());
                if let Value::Object(rs) = rt.object_get(socket_object, "_readableState") {
                    rt.object_set(rs, "length".into(), Value::Number(n as f64));
                }
                net_emit(rt, socket_object, "readable", Vec::new());
                net_emit(rt, socket_object, "data", vec![chunk]);
                rt.exit_realm(prior);
                socket_mark_activity(rt, stream_id);
                return Ok(true);
            }
            Ok(Some(_)) => {

                let prior = rt.enter_realm(realm);
                rt.object_set(
                    socket_object,
                    "readyState".into(),
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from("closed"))),
                );
                rt.object_set(socket_object, "readable".into(), Value::Boolean(false));
                rt.object_set(socket_object, "writable".into(), Value::Boolean(false));
                if let Value::Object(rs) = rt.object_get(socket_object, "_readableState") {
                    rt.object_set(rs, "endEmitted".into(), Value::Boolean(true));
                    rt.object_set(rs, "length".into(), Value::Number(0.0));
                }
                if let Value::Object(ws) = rt.object_get(socket_object, "_writableState") {
                    rt.object_set(ws, "ended".into(), Value::Boolean(true));
                    rt.object_set(ws, "finished".into(), Value::Boolean(true));
                }
                net_emit(rt, socket_object, "end", Vec::new());
                emit_socket_close(rt, socket_object);
                rt.exit_realm(prior);
                let _ = rusty_sockets::handle_close(stream_id);
                remove_socket(idx);
                return Ok(true);
            }
            Ok(None) => {}
            Err(_) => {
                let prior = rt.enter_realm(realm);
                let err = net_error_object(rt, "ECONNRESET", "socket read error");
                rt.object_set(socket_object, "readable".into(), Value::Boolean(false));
                rt.object_set(socket_object, "writable".into(), Value::Boolean(false));
                if let Value::Object(rs) = rt.object_get(socket_object, "_readableState") {
                    rt.object_set(rs, "endEmitted".into(), Value::Boolean(true));
                    rt.object_set(rs, "length".into(), Value::Number(0.0));
                }
                if let Value::Object(ws) = rt.object_get(socket_object, "_writableState") {
                    rt.object_set(ws, "ended".into(), Value::Boolean(true));
                    rt.object_set(ws, "finished".into(), Value::Boolean(true));
                }
                rt.object_set(
                    socket_object,
                    "__net_had_error".into(),
                    Value::Boolean(true),
                );
                net_emit(rt, socket_object, "error", vec![err]);
                emit_socket_close(rt, socket_object);
                rt.exit_realm(prior);
                let _ = rusty_sockets::handle_close(stream_id);
                remove_socket(idx);
                return Ok(true);
            }
        }
    }

    let now = std::time::Instant::now();
    let timed_out: Vec<(usize, ObjectRef)> = NET_SOCKETS.with(|v| {
        let mut out = Vec::new();
        for s in v.borrow_mut().iter_mut().flatten() {
            if s.agent_id != agent_id {
                continue;
            }
            if let Some(ms) = s.timeout_ms {
                if !s.timeout_fired && now.duration_since(s.last_activity).as_millis() as u64 >= ms
                {
                    s.timeout_fired = true;
                    out.push((s.realm, s.socket_object));
                }
            }
        }
        out
    });
    if !timed_out.is_empty() {
        for (realm, socket_object) in timed_out {
            let prior = rt.enter_realm(realm);
            net_emit(rt, socket_object, "timeout", Vec::new());
            rt.exit_realm(prior);
        }
        return Ok(true);
    }

    if sticky_idle && has_sockets {
        std::thread::sleep(std::time::Duration::from_millis(2));
        return Ok(true);
    }
    Ok(false)
}

fn make_net_namespace(rt: &mut Runtime, net_cap: caps::Net) -> ObjectRef {
    let net = new_object(rt);

    register_method(rt, net, "isIPv4", |_rt, args| {
        let s = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => String::new(),
        };
        Ok(Value::Boolean(Ipv4Addr::from_str(&s).is_ok()))
    });
    register_method(rt, net, "isIPv6", |_rt, args| {
        let s = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => String::new(),
        };
        Ok(Value::Boolean(Ipv6Addr::from_str(&s).is_ok()))
    });
    register_method(rt, net, "isIP", |_rt, args| {
        let s = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => String::new(),
        };
        let v = if Ipv4Addr::from_str(&s).is_ok() {
            4.0
        } else if Ipv6Addr::from_str(&s).is_ok() {
            6.0
        } else {
            0.0
        };
        Ok(Value::Number(v))
    });

    let cap_for_create = net_cap.clone();
    register_method(rt, net, "createServer", move |rt, args| {

        let listener = args
            .iter()
            .find(|v| rt.is_callable(v))
            .cloned()
            .unwrap_or(Value::Undefined);
        Ok(Value::Object(make_server(
            rt,
            listener,
            cap_for_create.clone(),
        )))
    });
    let cap_for_server_ctor = net_cap.clone();
    let server_ctor = make_callable(rt, "Server", move |rt, args| {
        let listener = args
            .iter()
            .find(|v| rt.is_callable(v))
            .cloned()
            .unwrap_or(Value::Undefined);
        Ok(Value::Object(make_server(
            rt,
            listener,
            cap_for_server_ctor.clone(),
        )))
    });
    make_ctor_subclassable_ee(rt, server_ctor);
    rt.object_set(net, "Server".into(), Value::Object(server_ctor));

    if let Value::Object(sp) = rt.object_get(server_ctor, "prototype") {
        rt.define_global_property("__cruft_net_server_proto", Value::Object(sp));
    }

    let cap_for_connect = net_cap.clone();
    register_method(rt, net, "connect", move |rt, args| {
        do_connect(rt, args, &cap_for_connect)
    });
    let cap_for_create_conn = net_cap.clone();
    register_method(rt, net, "createConnection", move |rt, args| {
        do_connect(rt, args, &cap_for_create_conn)
    });

    let cap_for_socket = net_cap.clone();
    let socket_ctor = make_callable(rt, "Socket", move |rt, _args| {
        let obj = new_object(rt);
        install_emitter(rt, obj);
        rt.set_engine_sentinel(obj, "__cruft_net_socket", Value::Boolean(true));

        rt.object_set(
            obj,
            "readyState".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from("open"))),
        );
        rt.object_set(obj, "connecting".into(), Value::Boolean(false));
        rt.object_set(obj, "pending".into(), Value::Boolean(true));
        rt.object_set(obj, "destroyed".into(), Value::Boolean(false));
        rt.object_set(obj, "bytesRead".into(), Value::Number(0.0));
        rt.object_set(obj, "bytesWritten".into(), Value::Number(0.0));

        register_method(rt, obj, "address", |rt, _a| {
            Ok(Value::Object(new_object(rt)))
        });
        let cap = cap_for_socket.clone();
        register_method(rt, obj, "connect", move |rt, args| {

            let connected = do_connect(rt, args, &cap)?;
            if let (Value::Object(this), Value::Object(src)) = (rt.current_this(), &connected) {
                if let Some(sid) = stream_id_of(rt, *src) {
                    rt.set_engine_sentinel(this, "__net_stream_id", Value::Number(sid as f64));
                    rt.object_set(
                        this,
                        "readyState".into(),
                        Value::String(Rc::new(rusty_js_runtime::value::JsString::from("open"))),
                    );
                }
            }
            Ok(rt.current_this())
        });
        for noop in [
            "pause",
            "resume",
            "setNoDelay",
            "setKeepAlive",
            "setTimeout",
            "destroySoon",
            "ref",
            "unref",
        ] {
            register_method(rt, obj, noop, |rt, _a| Ok(rt.current_this()));
        }

        register_method(rt, obj, "write", |_rt, _a| Ok(Value::Boolean(true)));
        register_method(rt, obj, "end", |rt, _a| {
            if let Value::Object(this) = rt.current_this() {
                rt.object_set(this, "writable".into(), Value::Boolean(false));
            }
            Ok(rt.current_this())
        });
        register_method(rt, obj, "setEncoding", |rt, a| {
            if let (Value::Object(this), Some(Value::String(e))) = (rt.current_this(), a.first()) {
                rt.object_set(this, "__net_encoding".into(), Value::String(e.clone()));
            }
            Ok(rt.current_this())
        });
        register_method(rt, obj, "cork", |_rt, _a| Ok(Value::Undefined));
        register_method(rt, obj, "uncork", |_rt, _a| Ok(Value::Undefined));
        register_method(rt, obj, "pipe", |rt, a| {
            Ok(a.first().cloned().unwrap_or_else(|| rt.current_this()))
        });
        register_method(rt, obj, "unpipe", |rt, _a| Ok(rt.current_this()));
        register_method(rt, obj, "destroy", |rt, a| {
            if let Value::Object(this) = rt.current_this() {

                if matches!(rt.object_get(this, "destroyed"), Value::Boolean(true)) {
                    return Ok(rt.current_this());
                }
                let destroy_error = match a.first() {
                    Some(v @ Value::Object(_)) => Some(v.clone()),
                    _ => None,
                };
                if let Some(sid) = stream_id_of(rt, this) {
                    remove_socket_by_stream(rt, sid);
                    let _ = rusty_sockets::handle_close(sid);
                }
                rt.object_set(
                    this,
                    "readyState".into(),
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from("closed"))),
                );
                rt.object_set(this, "readable".into(), Value::Boolean(false));
                rt.object_set(this, "writable".into(), Value::Boolean(false));
                rt.object_set(this, "destroyed".into(), Value::Boolean(true));
                if let Some(err) = destroy_error {
                    net_emit(rt, this, "error", vec![err]);
                }
                emit_socket_close(rt, this);
            }
            Ok(rt.current_this())
        });
        Ok(Value::Object(obj))
    });
    make_ctor_subclassable_ee(rt, socket_ctor);
    rt.object_set(net, "Socket".into(), Value::Object(socket_ctor));

    net
}

pub fn install(rt: &mut Runtime) {
    let net = make_net_namespace(rt, caps::Net::loopback_server());
    {
        let c = crate::register::make_callable(rt, "BlockList", |rt, _a| {
            let this = match rt.current_this() {
                Value::Object(id) => id,
                _ => new_object(rt),
            };
            rt.set_engine_sentinel(this, "__block_list", Value::Boolean(true));
            set_blocklist_rules(rt, this, Vec::new());
            Ok(Value::Object(this))
        });
        let p = new_object(rt);
        rt.object_set(p, "constructor".into(), Value::Object(c));
        register_method(rt, p, "addAddress", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(id) if rt.obj(id).has_own_str("__block_list") => id,
                _ => return Err(invalid_arg_type(rt, "invalid BlockList receiver")),
            };
            let (raw, inferred) = address_arg(rt, args.first())?;
            let family = family_from_arg(rt, args.get(1), inferred)?;
            let Some((canon_family, _, canon)) = normalize_addr(&raw, family) else {
                return Err(invalid_arg_value(rt, "invalid IP address"));
            };
            let mut rules = blocklist_rules(rt, this);
            rules.insert(
                0,
                format!("Address: {} {}", family_label(canon_family), canon),
            );
            set_blocklist_rules(rt, this, rules);
            Ok(Value::Undefined)
        });
        register_method(rt, p, "addRange", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(id) if rt.obj(id).has_own_str("__block_list") => id,
                _ => return Err(invalid_arg_type(rt, "invalid BlockList receiver")),
            };
            let (start_raw, inferred) = address_arg(rt, args.first())?;
            let (end_raw, _) = address_arg(rt, args.get(1))?;
            let family = family_from_arg(rt, args.get(2), inferred)?;
            let Some((start_family, start_n, start)) = normalize_addr(&start_raw, family) else {
                return Err(invalid_arg_value(rt, "invalid start address"));
            };
            let Some((end_family, end_n, end)) = normalize_addr(&end_raw, family) else {
                return Err(invalid_arg_value(rt, "invalid end address"));
            };
            if start_family != end_family || start_n > end_n {
                return Err(invalid_arg_value(rt, "invalid address range"));
            }
            let mut rules = blocklist_rules(rt, this);
            rules.insert(
                0,
                format!("Range: {} {}-{}", family_label(start_family), start, end),
            );
            set_blocklist_rules(rt, this, rules);
            Ok(Value::Undefined)
        });
        register_method(rt, p, "addSubnet", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(id) if rt.obj(id).has_own_str("__block_list") => id,
                _ => return Err(invalid_arg_type(rt, "invalid BlockList receiver")),
            };
            if matches!(args.get(2), Some(v) if !matches!(v, Value::Undefined | Value::String(_))) {
                return Err(invalid_arg_type(rt, "family must be a string"));
            }
            let (raw, inferred) = address_arg(rt, args.first())?;
            let prefix = match args.get(1) {
                Some(Value::Number(n)) if n.is_finite() && n.fract() == 0.0 => *n as i32,
                Some(Value::Number(_)) => return Err(out_of_range(rt, "prefix is out of range")),
                _ => return Err(invalid_arg_type(rt, "prefix must be a number")),
            };
            let family = family_from_arg(rt, args.get(2), inferred)?;
            let max = if family == BlockFamily::V4 { 32 } else { 128 };
            if prefix < 0 || prefix > max {
                return Err(out_of_range(rt, "prefix is out of range"));
            }
            let Some((canon_family, _, canon)) = normalize_addr(&raw, family) else {
                return Err(invalid_arg_value(rt, "invalid subnet address"));
            };
            let mut rules = blocklist_rules(rt, this);
            rules.insert(
                0,
                format!(
                    "Subnet: {} {}/{}",
                    family_label(canon_family),
                    canon,
                    prefix
                ),
            );
            set_blocklist_rules(rt, this, rules);
            Ok(Value::Undefined)
        });
        register_method(rt, p, "check", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(id) if rt.obj(id).has_own_str("__block_list") => id,
                _ => return Err(invalid_arg_type(rt, "invalid BlockList receiver")),
            };
            if matches!(args.get(1), Some(v) if !matches!(v, Value::Undefined | Value::String(_))) {
                return Err(invalid_arg_type(rt, "family must be a string"));
            }
            let (raw, inferred) = address_arg(rt, args.first())?;
            let family = match args.get(1) {
                Some(_) => family_from_arg(rt, args.get(1), inferred)?,
                None => match args.first() {
                    Some(Value::Object(_)) => inferred,
                    _ => BlockFamily::V4,
                },
            };
            Ok(Value::Boolean(
                blocklist_rules(rt, this)
                    .iter()
                    .any(|rule| blocklist_rule_matches(rule, &raw, family)),
            ))
        });
        register_method(rt, p, "toJSON", |rt, _args| {
            let this = match rt.current_this() {
                Value::Object(id) if rt.obj(id).has_own_str("__block_list") => id,
                _ => return Err(invalid_arg_type(rt, "invalid BlockList receiver")),
            };
            Ok(Value::Object(make_string_array(
                rt,
                &blocklist_rules(rt, this),
            )))
        });
        register_method(rt, p, "fromJSON", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(id) if rt.obj(id).has_own_str("__block_list") => id,
                _ => return Err(invalid_arg_type(rt, "invalid BlockList receiver")),
            };
            let items = match args.first() {
                Some(Value::Object(arr)) => {
                    let len = match rt.object_get(*arr, "length") {
                        Value::Number(n) if n >= 0.0 => n as usize,
                        _ => return Err(invalid_arg_type(rt, "rules must be an array")),
                    };
                    let mut v = Vec::new();
                    for i in 0..len {
                        match rt.object_get(*arr, &i.to_string()) {
                            Value::String(s) => v.push(s.as_str().to_string()),
                            _ => return Err(invalid_arg_type(rt, "rules must be strings")),
                        }
                    }
                    v
                }
                Some(Value::String(s)) => parse_json_string_array(s.as_str())
                    .ok_or_else(|| invalid_arg_type(rt, "rules must be a JSON string array"))?,
                _ => {
                    return Err(invalid_arg_type(
                        rt,
                        "rules must be an array or JSON string",
                    ))
                }
            };
            let mut rules = blocklist_rules(rt, this);
            for item in items {
                if parse_rule(&item).is_some() {
                    rules.push(item);
                }
            }
            set_blocklist_rules(rt, this, rules);
            Ok(Value::Undefined)
        });
        let is_block_list = crate::register::make_callable(rt, "isBlockList", |rt, args| {
            Ok(Value::Boolean(matches!(
                args.first(),
                Some(Value::Object(id)) if rt.obj(*id).has_own_str("__block_list")
            )))
        });
        rt.object_set(c, "isBlockList".into(), Value::Object(is_block_list));
        rt.object_set(c, "prototype".into(), Value::Object(p));
        rt.object_set(net, "BlockList".into(), Value::Object(c));
    }
    {
        let c = crate::register::make_callable(rt, "SocketAddress", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(id) => id,
                _ => new_object(rt),
            };

            let opts = match args.first() {
                Some(Value::Object(id)) => Some(*id),
                _ => None,
            };
            let address = match opts.map(|o| rt.object_get(o, "address")) {
                Some(Value::String(s)) => s.as_str().to_string(),
                _ => "127.0.0.1".to_string(),
            };
            let port = match opts.map(|o| rt.object_get(o, "port")) {
                Some(Value::Number(n)) if n.is_finite() && n >= 0.0 => n,
                _ => 0.0,
            };
            let flowlabel = match opts.map(|o| rt.object_get(o, "flowlabel")) {
                Some(Value::Number(n)) if n.is_finite() && n >= 0.0 => n,
                _ => 0.0,
            };
            let family_arg = opts.map(|o| rt.object_get(o, "family"));
            let family = family_from_arg(rt, family_arg.as_ref(), {
                if Ipv6Addr::from_str(&address).is_ok() {
                    BlockFamily::V6
                } else {
                    BlockFamily::V4
                }
            })?;
            let Some((canon_family, _, canon)) = normalize_addr(&address, family) else {
                return Err(invalid_arg_value(rt, "invalid SocketAddress address"));
            };
            rt.set_engine_sentinel(this, "__socket_address", Value::Boolean(true));
            rt.object_set(this, "address".into(), js_string(&canon));
            rt.object_set(this, "port".into(), Value::Number(port));
            rt.object_set(this, "flowlabel".into(), Value::Number(flowlabel));
            rt.object_set(
                this,
                "family".into(),
                js_string(if canon_family == BlockFamily::V6 {
                    "ipv6"
                } else {
                    "ipv4"
                }),
            );
            Ok(Value::Object(this))
        });
        let p = new_object(rt);
        rt.object_set(p, "constructor".into(), Value::Object(c));
        rt.object_set(c, "prototype".into(), Value::Object(p));
        rt.object_set(net, "SocketAddress".into(), Value::Object(c));
    }
    {
        let c = crate::register::make_callable(rt, "Stream", |rt, _a| Ok(rt.current_this()));
        let p = new_object(rt);
        rt.object_set(p, "constructor".into(), Value::Object(c));
        rt.object_set(c, "prototype".into(), Value::Object(p));
        rt.object_set(net, "Stream".into(), Value::Object(c));
    }
    register_method(rt, net, "_createServerHandle", |_rt, _a| {
        Ok(Value::Undefined)
    });
    register_method(rt, net, "_normalizeArgs", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, net, "setDefaultAutoSelectFamily", |_rt, _a| {
        Ok(Value::Undefined)
    });
    register_method(
        rt,
        net,
        "setDefaultAutoSelectFamilyAttemptTimeout",
        |_rt, _a| Ok(Value::Undefined),
    );
    register_method(rt, net, "getDefaultAutoSelectFamily", |_rt, _a| {
        Ok(Value::Boolean(true))
    });
    register_method(
        rt,
        net,
        "getDefaultAutoSelectFamilyAttemptTimeout",
        |_rt, _a| Ok(Value::Number(250.0)),
    );
    rt.define_global_property("net", Value::Object(net));
}

pub(crate) fn extract_bytes_pub(rt: &mut Runtime, v: &Value) -> Vec<u8> {
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
                    _ => 0,
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        collect_roots_for_runtime, get_server_for_runtime, remove_server_for_runtime,
        remove_socket_by_stream, socket_mark_activity, socket_set_timeout, ActiveNetServer,
        ActiveNetSocket, NET_SERVERS, NET_SOCKETS,
    };
    use rusty_js_runtime::{AgentId, Object, Runtime};

    #[test]
    fn net_server_registry_is_scoped_by_runtime_agent_id() {
        let mut rt_a = Runtime::new_with_agent_id(AgentId::from_raw(801));
        let mut rt_b = Runtime::new_with_agent_id(AgentId::from_raw(802));
        let server_b = rt_b.alloc_object(Object::new_ordinary());

        NET_SERVERS.with(|servers| {
            servers.borrow_mut().clear();
            servers.borrow_mut().push(Some(ActiveNetServer {
                agent_id: rt_b.agent_id(),
                listener_handle: 0,
                realm: rt_b.current_realm,
                server_object: server_b,
            }));
        });

        assert!(get_server_for_runtime(&rt_a, 0).is_none());
        assert!(get_server_for_runtime(&rt_b, 0).is_some());
        let mut roots_a = Vec::new();
        collect_roots_for_runtime(&rt_a, &mut roots_a);
        assert!(!roots_a.contains(&server_b));
        let mut roots_b = Vec::new();
        collect_roots_for_runtime(&rt_b, &mut roots_b);
        assert!(roots_b.contains(&server_b));
        assert!(remove_server_for_runtime(&rt_a, 0).is_none());
        assert!(remove_server_for_runtime(&rt_b, 0).is_some());

        NET_SERVERS.with(|servers| servers.borrow_mut().clear());
    }

    #[test]
    fn net_socket_registry_is_scoped_by_runtime_agent_id() {
        let rt_a = Runtime::new_with_agent_id(AgentId::from_raw(811));
        let mut rt_b = Runtime::new_with_agent_id(AgentId::from_raw(812));
        let socket_b = rt_b.alloc_object(Object::new_ordinary());
        let old_activity = std::time::Instant::now() - std::time::Duration::from_secs(10);

        NET_SOCKETS.with(|sockets| {
            sockets.borrow_mut().clear();
            sockets.borrow_mut().push(Some(ActiveNetSocket {
                agent_id: rt_b.agent_id(),
                stream_id: 999,
                realm: rt_b.current_realm,
                socket_object: socket_b,
                encoding: None,
                timeout_ms: None,
                last_activity: old_activity,
                timeout_fired: false,
            }));
        });

        socket_set_timeout(&rt_a, 999, 1);
        socket_mark_activity(&rt_a, 999);
        NET_SOCKETS.with(|sockets| {
            let sockets = sockets.borrow();
            let socket = sockets[0].as_ref().expect("agent B socket");
            assert!(socket.timeout_ms.is_none());
            assert_eq!(socket.last_activity, old_activity);
        });
        assert!(
            remove_socket_by_stream(&rt_a, 999).is_empty(),
            "agent A must not remove agent B's socket"
        );
        assert_eq!(remove_socket_by_stream(&rt_b, 999), vec![0]);

        NET_SOCKETS.with(|sockets| sockets.borrow_mut().clear());
    }
}
