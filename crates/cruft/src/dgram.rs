
use crate::net::{install_emitter, net_buffer_from_bytes, net_emit};
use crate::register::{new_object, register_method};
use rusty_js_runtime::caps::{self, ModuleId, ModuleProvenance};
use rusty_js_runtime::value::ObjectRef;
use rusty_js_runtime::{AgentId, Runtime, RuntimeError, Value};
use std::cell::RefCell;
use std::rc::Rc;

fn caller_module_id(rt: &Runtime) -> ModuleId {
    let url = rt.current_module_url.last().cloned().unwrap_or_default();
    let provenance = if url.contains("/node_modules/") {
        ModuleProvenance::Dependency
    } else if url.starts_with("node:") {
        ModuleProvenance::Builtin
    } else {
        ModuleProvenance::Application
    };
    ModuleId { url, provenance }
}

struct DgramSock {
    agent_id: AgentId,
    handle: u64,
    socket_object: ObjectRef,
    realm: usize,
}

thread_local! {
    static DGRAM_SOCKETS: RefCell<Vec<Option<DgramSock>>> = const { RefCell::new(Vec::new()) };
}

fn sval(s: &str) -> Value {
    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
        s.to_string(),
    )))
}

fn resolve_for_family(address: &str, port: u16, ipv6: bool) -> String {
    use std::net::ToSocketAddrs;
    if let Ok(addrs) = (address, port).to_socket_addrs() {
        let all: Vec<_> = addrs.collect();
        if let Some(a) = all.iter().find(|a| a.is_ipv6() == ipv6) {
            return a.to_string();
        }
        if let Some(a) = all.first() {
            return a.to_string();
        }
    }
    format!("{address}:{port}")
}

fn handle_of(rt: &Runtime, obj: ObjectRef) -> Option<u64> {
    match rt.object_get(obj, "__dgram_handle") {
        Value::Number(n) => Some(n as u64),
        _ => None,
    }
}

fn dgram_is_js_array(rt: &mut Runtime, id: ObjectRef) -> bool {
    if let Value::Object(arr) = rt.global_get("Array") {
        let f = rt.object_get(arr, "isArray");
        if rt.is_callable(&f) {
            if let Ok(Value::Boolean(b)) =
                rt.call_function(f, Value::Undefined, vec![Value::Object(id)])
            {
                return b;
            }
        }
    }
    false
}

fn msg_bytes(rt: &mut Runtime, v: &Value) -> Vec<u8> {
    match v {
        Value::String(s) => s.as_bytes().to_vec(),
        Value::Object(id) => {

            if dgram_is_js_array(rt, *id) {
                let len = match rt.object_get(*id, "length") {
                    Value::Number(n) => n as usize,
                    _ => 0,
                };
                let mut out = Vec::new();
                for i in 0..len {
                    let el = rt.object_get(*id, &i.to_string());
                    out.extend(msg_bytes(rt, &el));
                }
                return out;
            }
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

pub fn poll_io(rt: &mut Runtime) -> Result<bool, RuntimeError> {
    let agent_id = rt.agent_id();
    let socks: Vec<(u64, ObjectRef, usize)> = DGRAM_SOCKETS.with(|v| {
        v.borrow()
            .iter()
            .filter_map(|s| {
                s.as_ref().and_then(|s| {
                    (s.agent_id == agent_id).then_some((s.handle, s.socket_object, s.realm))
                })
            })
            .collect()
    });
    let has_any = !socks.is_empty();
    for (handle, sock, realm) in socks {
        match rusty_sockets::udp_try_recv(handle, 65536) {
            Ok(Some((bytes, from))) => {
                let prior = rt.enter_realm(realm);
                let buf = net_buffer_from_bytes(rt, &bytes);

                let (addr, port) = match from.rsplit_once(':') {
                    Some((a, p)) => (
                        a.trim_matches(['[', ']']).to_string(),
                        p.parse::<f64>().unwrap_or(0.0),
                    ),
                    None => (from.clone(), 0.0),
                };
                let family = if addr.contains(':') { "IPv6" } else { "IPv4" };
                let rinfo = new_object(rt);
                rt.object_set(rinfo, "address".into(), sval(&addr));
                rt.object_set(rinfo, "family".into(), sval(family));
                rt.object_set(rinfo, "port".into(), Value::Number(port));
                rt.object_set(rinfo, "size".into(), Value::Number(bytes.len() as f64));
                net_emit(rt, sock, "message", vec![buf, Value::Object(rinfo)]);
                rt.exit_realm(prior);
                return Ok(true);
            }
            Ok(None) => {}
            Err(_) => {}
        }
    }
    if has_any {
        std::thread::sleep(std::time::Duration::from_millis(1));
        Ok(true)
    } else {
        Ok(false)
    }
}

fn make_socket(rt: &mut Runtime, sock_type: &str) -> ObjectRef {
    let obj = new_object(rt);
    install_emitter(rt, obj);
    rt.object_set(obj, "__dgram_type".into(), sval(sock_type));

    rt.object_set(obj, "type".into(), sval(sock_type));

    register_method(rt, obj, "bind", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(t) => t,
            _ => return Ok(Value::Undefined),
        };

        let port = match args.first() {
            Some(Value::Number(n)) => *n as u16,
            Some(Value::Object(o)) => match rt.object_get(*o, "port") {
                Value::Number(n) => n as u16,
                _ => 0,
            },
            _ => 0,
        };
        let addr = match args.get(1) {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => "0.0.0.0".to_string(),
        };
        let cb = args.iter().rev().find(|v| rt.is_callable(v)).cloned();
        let caller = caller_module_id(rt);
        let gate = rt.caps.require_net(
            &caps::Net::none(),
            caps::NetOp::Listen {
                host: addr.clone(),
                port,
            },
            &caller,
        );
        if let Err(e) = gate {
            net_emit(rt, this, "error", vec![sval(&e.to_string())]);
            return Ok(Value::Undefined);
        }
        match rusty_sockets::udp_bind(&format!("{addr}:{port}")) {
            Ok((handle, _local)) => {
                rt.object_set(this, "__dgram_handle".into(), Value::Number(handle as f64));
                let realm = rt.current_realm;
                DGRAM_SOCKETS.with(|v| {
                    v.borrow_mut().push(Some(DgramSock {
                        agent_id: rt.agent_id(),
                        handle,
                        socket_object: this,
                        realm,
                    }))
                });
                net_emit(rt, this, "listening", Vec::new());
                if let Some(cb) = cb {
                    let _ = rt.call_function(cb, Value::Undefined, Vec::new());
                }
            }
            Err(e) => {
                net_emit(rt, this, "error", vec![sval(&format!("dgram bind: {e:?}"))]);
            }
        }
        Ok(rt.current_this())
    });

    register_method(rt, obj, "send", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(t) => t,
            _ => return Ok(Value::Undefined),
        };
        let mut bytes = msg_bytes(rt, &args.first().cloned().unwrap_or(Value::Undefined));

        let nums: Vec<f64> = args[1..]
            .iter()
            .filter_map(|v| {
                if let Value::Number(n) = v {
                    Some(*n)
                } else {
                    None
                }
            })
            .collect();
        let address = args[1..]
            .iter()
            .find_map(|v| {
                if let Value::String(s) = v {
                    Some(s.as_str().to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "127.0.0.1".to_string());
        let port = if nums.len() >= 3 {
            let (off, len) = (nums[0] as usize, nums[1] as usize);
            if off <= bytes.len() {
                bytes = bytes[off..(off + len).min(bytes.len())].to_vec();
            }
            nums[2] as u16
        } else {
            *nums.last().unwrap_or(&0.0) as u16
        };
        let cb = args.iter().rev().find(|v| rt.is_callable(v)).cloned();

        if handle_of(rt, this).is_none() {
            if let Ok((handle, _)) = rusty_sockets::udp_bind("0.0.0.0:0") {
                rt.object_set(this, "__dgram_handle".into(), Value::Number(handle as f64));
                let realm = rt.current_realm;
                DGRAM_SOCKETS.with(|v| {
                    v.borrow_mut().push(Some(DgramSock {
                        agent_id: rt.agent_id(),
                        handle,
                        socket_object: this,
                        realm,
                    }))
                });
            }
        }
        let ipv6 = matches!(rt.object_get(this, "__dgram_type"), Value::String(ref t) if t.as_str() == "udp6");
        let target = resolve_for_family(&address, port, ipv6);
        let res = match handle_of(rt, this) {
            Some(h) => rusty_sockets::udp_send_to(h, &bytes, &target),
            None => Err(rusty_sockets::SocketError::NotFound),
        };
        if let Some(cb) = cb {
            let err = match res {
                Ok(_) => Value::Null,
                Err(e) => sval(&format!("{e:?}")),
            };
            let _ = rt.call_function(cb, Value::Undefined, vec![err]);
        }
        Ok(Value::Undefined)
    });

    register_method(rt, obj, "address", |rt, _a| {
        let this = match rt.current_this() {
            Value::Object(t) => t,
            _ => return Ok(Value::Undefined),
        };
        let out = new_object(rt);
        if let Some(h) = handle_of(rt, this) {
            if let Ok(local) = rusty_sockets::udp_local_addr(h) {
                let (addr, port) = match local.rsplit_once(':') {
                    Some((a, p)) => (
                        a.trim_matches(['[', ']']).to_string(),
                        p.parse::<f64>().unwrap_or(0.0),
                    ),
                    None => (local, 0.0),
                };
                let family = if addr.contains(':') { "IPv6" } else { "IPv4" };
                rt.object_set(out, "address".into(), sval(&addr));
                rt.object_set(out, "family".into(), sval(family));
                rt.object_set(out, "port".into(), Value::Number(port));
            }
        }
        Ok(Value::Object(out))
    });

    register_method(rt, obj, "close", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(t) => t,
            _ => return Ok(Value::Undefined),
        };
        if let Some(h) = handle_of(rt, this) {
            let agent_id = rt.agent_id();
            DGRAM_SOCKETS.with(|v| {
                for s in v.borrow_mut().iter_mut() {
                    if s.as_ref()
                        .is_some_and(|x| x.agent_id == agent_id && x.handle == h)
                    {
                        *s = None;
                    }
                }
            });
            let _ = rusty_sockets::handle_close(h);
        }
        net_emit(rt, this, "close", Vec::new());
        if let Some(cb) = args.iter().find(|v| rt.is_callable(v)).cloned() {
            let _ = rt.call_function(cb, Value::Undefined, Vec::new());
        }
        Ok(Value::Undefined)
    });

    for noop in [
        "setBroadcast",
        "setTTL",
        "setMulticastTTL",
        "addMembership",
        "dropMembership",
        "ref",
        "unref",
        "setRecvBufferSize",
        "setSendBufferSize",
        "connect",
        "disconnect",
    ] {
        register_method(rt, obj, noop, |rt, _a| Ok(rt.current_this()));
    }
    obj
}

pub fn install(rt: &mut Runtime) {
    let ns = new_object(rt);
    register_method(rt, ns, "createSocket", |rt, args| {
        let sock_type = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            Some(Value::Object(o)) => match rt.object_get(*o, "type") {
                Value::String(s) => s.as_str().to_string(),
                _ => "udp4".to_string(),
            },
            _ => "udp4".to_string(),
        };

        if sock_type != "udp4" && sock_type != "udp6" {
            let msg = "Bad socket type specified. Valid types are: udp4, udp6";
            let err = match rusty_js_runtime::intrinsics::make_error_instance(rt, "TypeError", msg)
            {
                Some(id) => {
                    rt.object_set(id, "code".into(), sval("ERR_SOCKET_BAD_TYPE"));
                    Value::Object(id)
                }
                None => sval(msg),
            };
            return Err(RuntimeError::Thrown(err));
        }
        let sock = make_socket(rt, &sock_type);

        if let Some(cb) = args.get(1) {
            if rt.is_callable(cb) {
                let on = rt.object_get(sock, "on");
                if rt.is_callable(&on) {
                    let _ = rt.call_function(
                        on,
                        Value::Object(sock),
                        vec![sval("message"), cb.clone()],
                    );
                }
            }
        }
        Ok(Value::Object(sock))
    });

    let socket_ctor = crate::register::make_callable(rt, "Socket", |rt, _a| Ok(rt.current_this()));
    rt.object_set(ns, "Socket".into(), Value::Object(socket_ctor));
    rt.define_global_property("dgram", Value::Object(ns));
}

#[cfg(test)]
mod tests {
    use super::{DgramSock, DGRAM_SOCKETS};
    use rusty_js_runtime::{AgentId, Object, Runtime};

    #[test]
    fn dgram_registry_is_scoped_by_runtime_agent_id() {
        let mut rt_a = Runtime::new_with_agent_id(AgentId::from_raw(901));
        let mut rt_b = Runtime::new_with_agent_id(AgentId::from_raw(902));
        let sock_b = rt_b.alloc_object(Object::new_ordinary());

        DGRAM_SOCKETS.with(|sockets| {
            sockets.borrow_mut().clear();
            sockets.borrow_mut().push(Some(DgramSock {
                agent_id: rt_b.agent_id(),
                handle: 0,
                socket_object: sock_b,
                realm: rt_b.current_realm,
            }));
        });

        assert!(
            !super::poll_io(&mut rt_a).expect("poll agent A"),
            "agent A must not observe or keep alive agent B's UDP socket"
        );
        assert!(
            super::poll_io(&mut rt_b).expect("poll agent B"),
            "agent B owns UDP socket liveness"
        );

        DGRAM_SOCKETS.with(|sockets| sockets.borrow_mut().clear());
    }
}
