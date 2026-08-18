
use crate::net::{install_emitter, net_buffer_from_bytes, net_emit};
use crate::register::{new_object, register_method};
use rusty_js_runtime::caps::{self, ModuleId, ModuleProvenance};
use rusty_js_runtime::promise::{new_promise, reject_promise, resolve_promise};
use rusty_js_runtime::value::ObjectRef;
use rusty_js_runtime::{AgentId, Runtime, RuntimeError, Value};
use rusty_tls::driver::{
    tls_connect, tls_connect_with_config, TcpTlsTransport, TlsClientConfig, TlsSession,
};
use rusty_tls::record::TlsError;
use rusty_tls::store::TrustStore;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Condvar, Mutex};

fn platform_root_certificate_pems() -> Vec<String> {
    #[cfg(not(windows))]
    {
        for path in [
            "/etc/ssl/certs/ca-certificates.crt",
            "/etc/pki/tls/certs/ca-bundle.crt",
            "/etc/ssl/cert.pem",
            "/etc/ssl/ca-bundle.pem",
        ] {
            if let Ok(contents) = std::fs::read_to_string(path) {
                let certs = split_pem_certificates(&contents);
                if !certs.is_empty() {
                    return certs;
                }
            }
        }
    }

    Vec::new()
}

fn split_pem_certificates(pem: &str) -> Vec<String> {
    let mut certs = Vec::new();
    let mut rest = pem;
    const BEGIN: &str = "-----BEGIN CERTIFICATE-----";
    const END: &str = "-----END CERTIFICATE-----";
    while let Some(start) = rest.find(BEGIN) {
        rest = &rest[start..];
        let Some(end) = rest.find(END) else {
            break;
        };
        let end_idx = end + END.len();
        let mut block = rest[..end_idx].to_string();
        block.push('\n');
        certs.push(block);
        rest = &rest[end_idx..];
    }
    certs
}

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

struct TlsRecord {
    agent_id: AgentId,
    session: TlsSession<TcpTlsTransport>,
    accumulator: Vec<u8>,
    socket: ObjectRef,
    realm: usize,
}

struct PendingTlsConnect {
    agent_id: AgentId,
    rx: std::sync::mpsc::Receiver<Result<TlsSession<TcpTlsTransport>, String>>,
    socket: ObjectRef,
    callback: Value,
    realm: usize,
    root_key: String,
}

thread_local! {
    static TLS_SESSIONS: RefCell<Vec<Option<TlsRecord>>> = const { RefCell::new(Vec::new()) };
    static PENDING_TLS_CONNECTS: RefCell<Vec<Option<PendingTlsConnect>>> =
        const { RefCell::new(Vec::new()) };
    static NEXT_PENDING_TLS_CONNECT_ID: RefCell<u64> = const { RefCell::new(1) };
}

fn notify_agent_wake(wake: &Arc<(Mutex<u64>, Condvar)>) {
    let (lock, cv) = &**wake;
    let mut generation = lock.lock().unwrap();
    *generation = generation.wrapping_add(1);
    cv.notify_all();
}

fn register_session(rec: TlsRecord) -> usize {
    TLS_SESSIONS.with(|v| {
        let mut v = v.borrow_mut();
        v.push(Some(rec));
        v.len() - 1
    })
}

fn tls_id_of(rt: &Runtime, obj: ObjectRef) -> Option<usize> {
    match rt.object_get(obj, "__tls_id") {
        Value::Number(n) => Some(n as usize),
        _ => None,
    }
}

fn next_pending_tls_connect_root_key() -> String {
    let id = NEXT_PENDING_TLS_CONNECT_ID.with(|next| {
        let mut next = next.borrow_mut();
        let id = *next;
        *next = next.wrapping_add(1).max(1);
        id
    });
    format!("tls-connect:{id}")
}

fn has_pending_tls_connect(rt: &Runtime) -> bool {
    let agent_id = rt.agent_id();
    PENDING_TLS_CONNECTS.with(|v| {
        v.borrow()
            .iter()
            .any(|x| x.as_ref().is_some_and(|p| p.agent_id == agent_id))
    })
}

fn poll_pending_tls_connect(rt: &mut Runtime) -> Result<bool, RuntimeError> {
    use std::sync::mpsc::TryRecvError;
    let agent_id = rt.agent_id();
    let done: Option<(
        ObjectRef,
        Value,
        usize,
        String,
        Result<TlsSession<TcpTlsTransport>, String>,
    )> = PENDING_TLS_CONNECTS.with(|v| {
        let mut pending = v.borrow_mut();
        for slot in pending.iter_mut() {
            if slot.as_ref().is_some_and(|p| p.agent_id != agent_id) {
                continue;
            }
            let ready = match slot.as_ref() {
                Some(p) => match p.rx.try_recv() {
                    Ok(result) => Some(result),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => {
                        Some(Err("tls.connect worker terminated".to_string()))
                    }
                },
                None => None,
            };
            if let Some(result) = ready {
                let p = slot.take().unwrap();
                return Some((p.socket, p.callback, p.realm, p.root_key, result));
            }
        }
        None
    });
    let Some((socket, callback, realm, root_key, result)) = done else {
        return Ok(false);
    };
    let prior = rt.enter_realm(realm);
    match result {
        Ok(session) => {
            if let Err(e) = session.transport.set_nonblocking(true) {
                let err = make_err(rt, format!("tls: set_nonblocking {e:?}"));
                net_emit(rt, socket, "error", vec![err]);
                net_emit(rt, socket, "close", Vec::new());
            } else {
                let id = register_session(TlsRecord {
                    agent_id,
                    session,
                    accumulator: Vec::new(),
                    socket,
                    realm,
                });
                rt.set_engine_sentinel(socket, "__tls_id", Value::Number(id as f64));
                net_emit(rt, socket, "secureConnect", Vec::new());
                net_emit(rt, socket, "connect", Vec::new());
                if rt.is_callable(&callback) {
                    let _ = rt.call_function(callback, Value::Object(socket), Vec::new());
                }
            }
        }
        Err(e) => {
            let err = make_err(rt, format!("tls.connect: {e}"));
            net_emit(rt, socket, "error", vec![err]);
            net_emit(rt, socket, "close", Vec::new());
        }
    }
    rt.exit_realm(prior);
    rt.release_host_roots(&root_key);
    PENDING_TLS_CONNECTS.with(|v| v.borrow_mut().retain(|x| x.is_some()));
    Ok(true)
}

fn write_bytes(rt: &mut Runtime, v: Option<&Value>) -> Vec<u8> {
    match v {
        Some(Value::String(s)) => s.as_bytes().to_vec(),
        Some(Value::Object(id)) => {
            let len = rt.array_length(*id);
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

pub fn poll_io(rt: &mut Runtime) -> Result<bool, RuntimeError> {
    if poll_pending_tls_connect(rt)? {
        return Ok(true);
    }

    let server_did = server_poll_io(rt)?;
    enum Ev {
        Data(Vec<u8>, ObjectRef, usize),
        Close(usize, ObjectRef, usize),
    }
    let ev = TLS_SESSIONS.with(|v| -> Option<Ev> {
        let mut sessions = v.borrow_mut();
        for i in 0..sessions.len() {
            if let Some(rec) = sessions[i].as_mut() {
                if rec.agent_id != rt.agent_id() {
                    continue;
                }
                let (sock, realm) = (rec.socket, rec.realm);
                match rec.session.receive_application_data(&mut rec.accumulator) {
                    Ok(chunk) if !chunk.is_empty() => return Some(Ev::Data(chunk, sock, realm)),
                    Ok(_) => {}
                    Err(TlsError::WouldBlock) => {}
                    Err(_) => return Some(Ev::Close(i, sock, realm)),
                }
            }
        }
        None
    });
    match ev {
        Some(Ev::Data(chunk, socket, realm)) => {
            let prior = rt.enter_realm(realm);
            let buf = net_buffer_from_bytes(rt, &chunk);
            net_emit(rt, socket, "data", vec![buf]);
            rt.exit_realm(prior);
            Ok(true)
        }
        Some(Ev::Close(idx, socket, realm)) => {
            let prior = rt.enter_realm(realm);
            net_emit(rt, socket, "end", Vec::new());
            net_emit(rt, socket, "close", Vec::new());
            rt.exit_realm(prior);
            TLS_SESSIONS.with(|v| {
                if let Some(s) = v.borrow_mut().get_mut(idx) {
                    if s.as_ref().is_some_and(|rec| rec.agent_id == rt.agent_id()) {
                        *s = None;
                    }
                }
            });
            Ok(true)
        }
        None => {

            let agent_id = rt.agent_id();
            let has_client_sessions = TLS_SESSIONS.with(|v| {
                v.borrow()
                    .iter()
                    .any(|s| s.as_ref().is_some_and(|rec| rec.agent_id == agent_id))
            });
            let has_servers = TLS_SERVERS.with(|v| {
                v.borrow()
                    .iter()
                    .any(|s| s.as_ref().is_some_and(|rec| rec.agent_id == agent_id))
            });
            let has_server_conns = TLS_SERVER_CONNS.with(|c| {
                c.borrow()
                    .iter()
                    .any(|s| s.as_ref().is_some_and(|rec| rec.agent_id == agent_id))
            });
            let has_open = has_client_sessions
                || has_servers
                || has_server_conns
                || has_pending_tls_connect(rt);
            if has_open {
                if has_server_conns && crate::http::has_pending_client(rt) {
                    return Ok(true);
                }
                if !has_client_sessions && !has_servers && !has_server_conns {
                    let observed = rt.agent_wake_generation();
                    if poll_pending_tls_connect(rt)? {
                        return Ok(true);
                    }
                    let _ = rt.wait_agent_wake_timeout(observed, std::time::Duration::from_secs(1));
                } else {
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
                Ok(true)
            } else {
                Ok(server_did)
            }
        }
    }
}

fn make_tls_socket(rt: &mut Runtime, realm: usize) -> ObjectRef {
    let _ = realm;
    let obj = new_object(rt);
    install_emitter(rt, obj);
    install_node_js_stream_wrap_shape(rt, obj, Value::Undefined);
    rt.object_set(obj, "authorized".into(), Value::Boolean(true));
    rt.object_set(obj, "encrypted".into(), Value::Boolean(true));

    register_method(rt, obj, "write", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Boolean(false)),
        };
        let id = match tls_id_of(rt, this) {
            Some(i) => i,
            None => return Ok(Value::Boolean(false)),
        };
        let bytes = write_bytes(rt, args.first());
        let ok = TLS_SESSIONS.with(|v| {
            if let Some(Some(rec)) = v.borrow_mut().get_mut(id) {
                rec.agent_id == rt.agent_id() && rec.session.send_application_data(&bytes).is_ok()
            } else {
                false
            }
        });
        Ok(Value::Boolean(ok))
    });

    register_method(rt, obj, "end", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        if let Some(id) = tls_id_of(rt, this) {
            let bytes = write_bytes(rt, args.first());
            if !bytes.is_empty() {
                TLS_SESSIONS.with(|v| {
                    if let Some(Some(rec)) = v.borrow_mut().get_mut(id) {
                        if rec.agent_id == rt.agent_id() {
                            let _ = rec.session.send_application_data(&bytes);
                        }
                    }
                });
            }
        }
        Ok(rt.current_this())
    });

    register_method(rt, obj, "setEncoding", |rt, _a| Ok(rt.current_this()));
    register_method(rt, obj, "setNoDelay", |rt, _a| Ok(rt.current_this()));
    register_method(rt, obj, "setTimeout", |rt, _a| Ok(rt.current_this()));
    register_method(rt, obj, "destroy", |rt, _a| {
        if let Value::Object(this) = rt.current_this() {
            if let Some(id) = tls_id_of(rt, this) {
                TLS_SESSIONS.with(|v| {
                    if let Some(s) = v.borrow_mut().get_mut(id) {
                        if s.as_ref().is_some_and(|rec| rec.agent_id == rt.agent_id()) {
                            *s = None;
                        }
                    }
                });
            }
        }
        Ok(rt.current_this())
    });
    obj
}

fn install_node_js_stream_wrap_shape(rt: &mut Runtime, socket: ObjectRef, stream: Value) {
    let handle = new_object(rt);
    let parent_wrap = new_object(rt);
    let constructor_holder = new_object(rt);
    register_method(rt, constructor_holder, "JSStreamSocket", |rt, args| {
        let receiver = match rt.current_this() {
            Value::Object(id) => id,
            _ => new_object(rt),
        };
        install_emitter(rt, receiver);
        let stream = args.first().cloned().unwrap_or(Value::Undefined);
        install_node_js_stream_wrap_shape(rt, receiver, stream);
        Ok(Value::Object(receiver))
    });
    rt.object_set(
        parent_wrap,
        "constructor".into(),
        rt.object_get(constructor_holder, "JSStreamSocket"),
    );
    rt.object_set(parent_wrap, "stream".into(), stream);
    rt.object_set(handle, "_parentWrap".into(), Value::Object(parent_wrap));
    register_method(rt, handle, "getpeername", |rt, args| {
        if let Some(Value::Object(out)) = args.first() {
            rt.object_set(*out, "family".into(), Value::Undefined);
            rt.object_set(*out, "address".into(), Value::Undefined);
            rt.object_set(*out, "port".into(), Value::Undefined);
        }
        Ok(Value::Undefined)
    });
    rt.object_set(socket, "_handle".into(), Value::Object(handle));
}

fn do_connect(rt: &mut Runtime, args: &[Value]) -> Result<Value, RuntimeError> {
    let (port, host) = match args.first() {
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
                _ => 443,
            };
            let host = match rt.object_get(*o, "host") {
                Value::String(s) => s.as_str().to_string(),
                _ => match rt.object_get(*o, "servername") {
                    Value::String(s) => s.as_str().to_string(),
                    _ => "127.0.0.1".to_string(),
                },
            };
            (port, host)
        }
        _ => {
            return Err(RuntimeError::TypeError(
                "tls.connect: port or options required".into(),
            ))
        }
    };
    let caller = caller_module_id(rt);
    rt.caps
        .require_net(
            &caps::Net::none(),
            caps::NetOp::Connect {
                host: host.clone(),
                port,
            },
            &caller,
        )
        .map_err(|e| RuntimeError::TypeError(e.to_string()))?;

    let ca_pem = match args.first() {
        Some(Value::Object(o)) => match rt.object_get(*o, "ca") {
            Value::String(s) => Some(s.as_str().to_string()),
            _ => None,
        },
        _ => None,
    };
    let insecure = match args.first() {
        Some(Value::Object(o)) => matches!(
            rt.object_get(*o, "rejectUnauthorized"),
            Value::Boolean(false)
        ),
        _ => false,
    };
    let realm = rt.current_realm;
    let socket = make_tls_socket(rt, realm);
    let callback = args
        .iter()
        .find(|v| rt.is_callable(v))
        .cloned()
        .unwrap_or(Value::Undefined);
    let (tx, rx) = std::sync::mpsc::channel();
    let wake = rt.agent_wake_handle();
    std::thread::spawn(move || {
        let result = (|| -> Result<TlsSession<TcpTlsTransport>, String> {
            let trust = if let Some(ca) = ca_pem {
                let mut ts = TrustStore::new();
                ts.add_pem_bundle(&ca)
                    .map_err(|e| format!("ca bundle {e:?}"))?;
                ts
            } else {
                TrustStore::load_system_default().map_err(|e| format!("trust store {e:?}"))?
            };
            tls_connect_with_config(
                &host,
                port,
                &trust,
                TlsClientConfig {
                    insecure_skip_certificate_validation: insecure,
                },
            )
            .map_err(|e| format!("{e:?}"))
        })();
        if tx.send(result).is_ok() {
            notify_agent_wake(&wake);
        }
    });
    let mut roots = vec![Value::Object(socket)];
    if let Value::Object(_) = callback {
        roots.push(callback.clone());
    }
    let root_key = next_pending_tls_connect_root_key();
    rt.retain_host_roots(root_key.clone(), roots);
    PENDING_TLS_CONNECTS.with(|v| {
        v.borrow_mut().push(Some(PendingTlsConnect {
            agent_id: rt.agent_id(),
            rx,
            socket,
            callback,
            realm,
            root_key,
        }));
    });
    Ok(Value::Object(socket))
}

const TLS_REG_SLOT: &str = "__listeners";

fn make_err(rt: &mut Runtime, message: String) -> Value {
    let ctor = rt.global_get("Error");
    match rt.construct(
        ctor,
        vec![Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(message.clone()),
        ))],
    ) {
        Ok(v) => v,
        _ => Value::String(Rc::new(rusty_js_runtime::value::JsString::from(message))),
    }
}

fn add_listener(rt: &mut Runtime, conn: ObjectRef, event: &str, listener: Value) {
    if !rt.is_callable(&listener) {
        return;
    }
    let registry = match rt.object_get(conn, TLS_REG_SLOT) {
        Value::Object(id) => id,
        _ => {
            let r = new_object(rt);
            rt.set_engine_sentinel(conn, TLS_REG_SLOT, Value::Object(r));
            r
        }
    };
    let arr = match rt.object_get(registry, event) {
        Value::Object(a) => a,
        _ => {
            let a = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
            rt.object_set(a, "length".into(), Value::Number(0.0));
            rt.object_set(registry, event.into(), Value::Object(a));
            a
        }
    };
    let len = rt.array_length(arr);
    rt.object_set(arr, len.to_string(), listener);
    rt.object_set(arr, "length".into(), Value::Number((len + 1) as f64));
}

fn make_canonical_conn(rt: &mut Runtime) -> ObjectRef {
    let conn = new_object(rt);
    let registry = new_object(rt);
    rt.set_engine_sentinel(conn, TLS_REG_SLOT, Value::Object(registry));
    rt.object_set(conn, "encrypted".into(), Value::Boolean(true));

    register_method(rt, conn, "write", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Boolean(false)),
        };
        let id = match tls_id_of(rt, this) {
            Some(i) => i,
            None => return Ok(Value::Boolean(false)),
        };
        let bytes = write_bytes(rt, args.first());
        let ok = TLS_SESSIONS.with(|v| {
            if let Some(Some(rec)) = v.borrow_mut().get_mut(id) {
                rec.agent_id == rt.agent_id() && rec.session.send_application_data(&bytes).is_ok()
            } else {
                false
            }
        });
        Ok(Value::Boolean(ok))
    });
    register_method(rt, conn, "close", |rt, _a| {
        if let Value::Object(this) = rt.current_this() {
            if let Some(id) = tls_id_of(rt, this) {
                TLS_SESSIONS.with(|v| {
                    if let Some(s) = v.borrow_mut().get_mut(id) {
                        if s.as_ref().is_some_and(|rec| rec.agent_id == rt.agent_id()) {
                            *s = None;
                        }
                    }
                });
            }
        }
        Ok(Value::Undefined)
    });
    for (method, event) in [
        ("onData", "data"),
        ("onClose", "close"),
        ("onError", "error"),
    ] {
        register_method(rt, conn, method, move |rt, args| {
            let this = match rt.current_this() {
                Value::Object(id) => id,
                _ => return Ok(Value::Undefined),
            };
            let cb = args.first().cloned().unwrap_or(Value::Undefined);
            add_listener(rt, this, event, cb);
            Ok(Value::Object(this))
        });
    }
    conn
}

fn canonical_connect(rt: &mut Runtime, args: &[Value]) -> Result<Value, RuntimeError> {
    let opts = match args.first() {
        Some(Value::Object(o)) => *o,
        _ => {
            return Err(RuntimeError::TypeError(
                "cruft:tls.connect: an options object with host + port is required".into(),
            ))
        }
    };
    let host = match rt.object_get(opts, "host") {
        Value::String(s) => s.as_str().to_string(),
        _ => match rt.object_get(opts, "servername") {
            Value::String(s) => s.as_str().to_string(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "cruft:tls.connect: host required".into(),
                ))
            }
        },
    };
    let port = match rt.object_get(opts, "port") {
        Value::Number(n) => n as u16,
        _ => {
            return Err(RuntimeError::TypeError(
                "cruft:tls.connect: port required".into(),
            ))
        }
    };
    let caller = caller_module_id(rt);
    rt.caps
        .require_net(
            &caps::Net::none(),
            caps::NetOp::Connect {
                host: host.clone(),
                port,
            },
            &caller,
        )
        .map_err(|e| RuntimeError::TypeError(e.to_string()))?;

    let p = new_promise(rt);
    let trust = match TrustStore::load_system_default() {
        Ok(t) => t,
        Err(e) => {
            let err = make_err(rt, format!("tls: trust store {e:?}"));
            reject_promise(rt, p, err);
            return Ok(Value::Object(p));
        }
    };
    let session = match tls_connect(&host, port, &trust) {
        Ok(s) => s,
        Err(e) => {
            let err = make_err(rt, format!("cruft:tls.connect: {e:?}"));
            reject_promise(rt, p, err);
            return Ok(Value::Object(p));
        }
    };
    if let Err(e) = session.transport.set_nonblocking(true) {
        let err = make_err(rt, format!("tls: set_nonblocking {e:?}"));
        reject_promise(rt, p, err);
        return Ok(Value::Object(p));
    }
    let realm = rt.current_realm;
    let conn = make_canonical_conn(rt);
    let id = register_session(TlsRecord {
        agent_id: rt.agent_id(),
        session,
        accumulator: Vec::new(),
        socket: conn,
        realm,
    });
    rt.set_engine_sentinel(conn, "__tls_id", Value::Number(id as f64));
    resolve_promise(rt, p, Value::Object(conn));
    Ok(Value::Object(p))
}

pub fn install_canonical(rt: &mut Runtime) {
    let ns = new_object(rt);
    register_method(rt, ns, "connect", |rt, args| canonical_connect(rt, args));
    register_method(rt, ns, "createServer", |rt, args| {
        do_create_tls_server_canonical(rt, args)
    });
    rt.define_global_property("__cruft_tls", Value::Object(ns));
}

use rusty_tls::handshake::{aead_decrypt_record, aead_encrypt_record, TrafficKeys};
use rusty_tls::server::{
    parse_cert_chain_pem, parse_ec_p256_private_key_pem, parse_rsa_private_key_pem, ServerConfig,
};
use rusty_tls::{ServerHandshakeMachine, CIPHER_AES_128_GCM_SHA256, GROUP_SECP256R1, GROUP_X25519};

struct TlsServerRec {
    agent_id: AgentId,
    listener_handle: u64,
    config: Option<ServerConfig>,
    server_object: ObjectRef,
    realm: usize,

    http_server_id: Option<usize>,

    canonical: bool,
}

struct TlsServerConn {
    agent_id: AgentId,
    stream_id: u64,
    machine: Option<ServerHandshakeMachine>,
    server_app: Option<TrafficKeys>,
    client_app: Option<TrafficKeys>,
    server_seq: u64,
    client_seq: u64,
    rbuf: Vec<u8>,
    socket: ObjectRef,
    server_object: ObjectRef,
    realm: usize,
    http_server_id: Option<usize>,
    http_rbuf: Vec<u8>,
    canonical: bool,

    h2: Option<rusty_http2_conn::Http2Connection>,

    node_h2_mode: bool,
}

thread_local! {
    static TLS_SERVERS: RefCell<Vec<Option<TlsServerRec>>> = const { RefCell::new(Vec::new()) };
    static TLS_SERVER_CONNS: RefCell<Vec<Option<TlsServerConn>>> = const { RefCell::new(Vec::new()) };
}

pub fn collect_roots(roots: &mut Vec<ObjectRef>) {
    collect_roots_for_agent(AgentId::DEFAULT, roots);
}

pub fn collect_roots_for_runtime(rt: &Runtime, roots: &mut Vec<ObjectRef>) {
    collect_roots_for_agent(rt.agent_id(), roots);
}

fn collect_roots_for_agent(agent_id: AgentId, roots: &mut Vec<ObjectRef>) {
    TLS_SERVERS.with(|v| {
        for s in v.borrow().iter().flatten() {
            if s.agent_id != agent_id {
                continue;
            }
            roots.push(s.server_object);
        }
    });
    TLS_SERVER_CONNS.with(|v| {
        for c in v.borrow().iter().flatten() {
            if c.agent_id != agent_id {
                continue;
            }
            roots.push(c.socket);
            roots.push(c.server_object);
        }
    });
}

fn frame_record(content_type: u8, fragment: &[u8]) -> Vec<u8> {
    let mut r = Vec::with_capacity(5 + fragment.len());
    r.push(content_type);
    r.extend_from_slice(&[0x03, 0x03]);
    r.push((fragment.len() >> 8) as u8);
    r.push((fragment.len() & 0xFF) as u8);
    r.extend_from_slice(fragment);
    r
}

fn take_record(buf: &[u8]) -> Option<(u8, Vec<u8>, usize)> {
    if buf.len() < 5 {
        return None;
    }
    let len = ((buf[3] as usize) << 8) | (buf[4] as usize);
    if buf.len() < 5 + len {
        return None;
    }
    Some((buf[0], buf[5..5 + len].to_vec(), 5 + len))
}

fn tls_sconn_id_of(rt: &Runtime, obj: ObjectRef) -> Option<usize> {
    match rt.object_get(obj, "__tls_sconn_id") {
        Value::Number(n) => Some(n as usize),
        _ => None,
    }
}

fn make_server_tls_socket(rt: &mut Runtime) -> ObjectRef {
    let obj = new_object(rt);
    install_emitter(rt, obj);
    rt.object_set(obj, "encrypted".into(), Value::Boolean(true));
    register_method(rt, obj, "write", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Boolean(false)),
        };
        let bytes = write_bytes(rt, args.first());
        let ok = match tls_sconn_id_of(rt, this) {
            Some(id) => server_conn_send(rt, id, &bytes),
            None => false,
        };
        Ok(Value::Boolean(ok))
    });
    register_method(rt, obj, "end", |rt, args| {
        if let Value::Object(this) = rt.current_this() {
            let bytes = write_bytes(rt, args.first());
            if let Some(id) = tls_sconn_id_of(rt, this) {
                if !bytes.is_empty() {
                    server_conn_send(rt, id, &bytes);
                }
                let agent_id = rt.agent_id();
                TLS_SERVER_CONNS.with(|conns| {
                    if let Some(conn) = conns
                        .borrow_mut()
                        .get_mut(id)
                        .and_then(|slot| slot.as_mut())
                    {
                        if conn.agent_id != agent_id {
                            return;
                        }
                        if let Some(keys) = conn.server_app.clone() {
                            if let Ok(ct) = aead_encrypt_record(&keys, conn.server_seq, 21, &[1, 0])
                            {
                                conn.server_seq += 1;
                                let _ = rusty_sockets::stream_write_all(
                                    conn.stream_id,
                                    &frame_record(23, &ct),
                                );
                            }
                        }
                        let _ = rusty_sockets::stream_shutdown_write(conn.stream_id);
                    }
                });
            }
        }
        Ok(rt.current_this())
    });
    register_method(rt, obj, "setEncoding", |rt, _a| Ok(rt.current_this()));
    register_method(rt, obj, "setNoDelay", |rt, _a| Ok(rt.current_this()));
    register_method(rt, obj, "destroy", |rt, _a| Ok(rt.current_this()));

    register_method(rt, obj, "onData", |rt, args| {
        if let (Value::Object(this), Some(cb)) = (rt.current_this(), args.first()) {
            rt.set_engine_sentinel(this, "__cruft_ondata", cb.clone());
        }
        Ok(rt.current_this())
    });
    register_method(rt, obj, "onClose", |rt, args| {
        if let (Value::Object(this), Some(cb)) = (rt.current_this(), args.first()) {
            rt.set_engine_sentinel(this, "__cruft_onclose", cb.clone());
        }
        Ok(rt.current_this())
    });
    obj
}

fn server_conn_send(rt: &Runtime, id: usize, bytes: &[u8]) -> bool {
    let agent_id = rt.agent_id();
    TLS_SERVER_CONNS.with(|conns| {
        let mut conns = conns.borrow_mut();
        let conn = match conns.get_mut(id).and_then(|c| c.as_mut()) {
            Some(c) => c,
            None => return false,
        };
        if conn.agent_id != agent_id {
            return false;
        }
        let keys = match &conn.server_app {
            Some(k) => k,
            None => return false,
        };
        let ct = match aead_encrypt_record(keys, conn.server_seq, 23, bytes) {
            Ok(c) => c,
            Err(_) => return false,
        };
        conn.server_seq += 1;
        rusty_sockets::stream_write_all(conn.stream_id, &frame_record(23, &ct)).is_ok()
    })
}

fn pem_option(rt: &mut Runtime, v: Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.as_str().to_string()),
        Value::Object(id) => {
            let receiver = Value::Object(id);
            let _receiver_roots = rt.push_temporary_value_roots(std::slice::from_ref(&receiver));
            let ts = rt.object_get(id, "toString");
            if rt.is_callable(&ts) {
                let _call_roots = rt.push_temporary_value_roots(&[receiver.clone(), ts.clone()]);
                if let Ok(Value::String(s)) = rt.call_function(ts, receiver, Vec::new()) {
                    return Some(s.as_str().to_string());
                }
            }
            None
        }
        _ => None,
    }
}

fn server_config_from_options(
    rt: &mut Runtime,
    opts: ObjectRef,
) -> Result<ServerConfig, RuntimeError> {
    let opts_v = Value::Object(opts);
    let _opts_roots = rt.push_temporary_value_roots(std::slice::from_ref(&opts_v));
    let cert_v = rt.object_get(opts, "cert");
    let _cert_roots = rt.push_temporary_value_roots(&[opts_v.clone(), cert_v.clone()]);
    let cert_pem = match pem_option(rt, cert_v) {
        Some(s) => s,
        None => {
            return Err(RuntimeError::TypeError(
                "tls.createServer: `cert` (PEM) required".into(),
            ))
        }
    };
    let key_v = rt.object_get(opts, "key");
    let _key_roots = rt.push_temporary_value_roots(&[opts_v.clone(), key_v.clone()]);
    let key_pem = match pem_option(rt, key_v) {
        Some(s) => s,
        None => {
            return Err(RuntimeError::TypeError(
                "tls.createServer: `key` (PEM) required".into(),
            ))
        }
    };
    let cert_chain = parse_cert_chain_pem(&cert_pem);
    if cert_chain.is_empty() {
        return Err(RuntimeError::TypeError(
            "tls.createServer: no certificate in `cert`".into(),
        ));
    }

    let (signing_key, rsa_key) = match parse_ec_p256_private_key_pem(&key_pem) {
        Ok(k) => (k, None),
        Err(ec_err) => match parse_rsa_private_key_pem(&key_pem) {
            Ok(nd) => (Vec::new(), Some(nd)),
            Err(rsa_err) => {
                return Err(RuntimeError::TypeError(format!(
                    "tls.createServer: key parse: not EC ({ec_err}) nor RSA ({rsa_err})"
                )))
            }
        },
    };

    let alpn_protocols = match rt.object_get(opts, "ALPNProtocols") {
        Value::Object(arr) => {
            let _alpn_roots = rt.push_temporary_value_roots(&[opts_v.clone(), Value::Object(arr)]);
            let len = rt.array_length(arr);
            let mut v = Vec::new();
            for i in 0..len {
                if let Value::String(s) = rt.object_get(arr, &i.to_string()) {
                    v.push(s.as_bytes().to_vec());
                }
            }
            if v.is_empty() {
                vec![b"http/1.1".to_vec()]
            } else {
                v
            }
        }
        _ => vec![b"http/1.1".to_vec()],
    };
    Ok(ServerConfig {
        cert_chain,
        signing_key,
        rsa_key,
        suites: vec![CIPHER_AES_128_GCM_SHA256],
        groups: vec![GROUP_X25519, GROUP_SECP256R1],
        alpn_protocols,
    })
}

fn install_tls_listen(
    rt: &mut Runtime,
    server: ObjectRef,
    config: Option<ServerConfig>,
    http_server_id: Option<usize>,
    canonical: bool,
) {
    let pending = Rc::new(RefCell::new(Some(config)));
    let hid = http_server_id;
    let is_canon = canonical;
    let pending_for_secure_context = pending.clone();
    register_method(rt, server, "setSecureContext", move |rt, args| {
        let opts = match args.first() {
            Some(Value::Object(id)) => *id,
            _ => return Ok(Value::Undefined),
        };
        let mut config = match server_config_from_options(rt, opts) {
            Ok(config) => config,
            Err(RuntimeError::TypeError(msg))
                if msg.contains("`cert` (PEM) required")
                    || msg.contains("`key` (PEM) required") =>
            {
                return Ok(Value::Undefined);
            }
            Err(err) => return Err(err),
        };
        if hid.is_some() {
            config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        }
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        if let Value::Number(n) = rt.object_get(this, "__tls_server_id") {
            TLS_SERVERS.with(|v| {
                if let Some(Some(rec)) = v.borrow_mut().get_mut(n as usize) {
                    if rec.agent_id == rt.agent_id() {
                        rec.config = Some(config);
                    }
                }
            });
        } else {
            *pending_for_secure_context.borrow_mut() = Some(Some(config));
        }
        Ok(Value::Undefined)
    });
    register_method(rt, server, "listen", move |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
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
        let host = match args.get(1) {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => "127.0.0.1".to_string(),
        };
        rt.caps
            .require_net(
                &caps::Net::full(),
                caps::NetOp::Listen {
                    host: host.clone(),
                    port,
                },
                &ModuleId::builtin("node:tls"),
            )
            .map_err(|e| RuntimeError::TypeError(e.to_string()))?;
        let config = match pending.borrow_mut().take() {
            Some(c) => c,
            None => {
                return Err(RuntimeError::TypeError(
                    "tls server already listening".into(),
                ))
            }
        };
        let (handle, bound_addr) = rusty_sockets::listener_bind_async(&format!("{host}:{port}"))
            .map_err(|e| RuntimeError::TypeError(format!("tls.listen: {e:?}")))?;
        rt.set_engine_sentinel(
            this,
            "__tls_bound_addr",
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(bound_addr))),
        );
        rt.object_set(this, "listening".into(), Value::Boolean(true));
        let realm = rt.current_realm;
        let server_idx = TLS_SERVERS.with(|v| {
            let mut servers = v.borrow_mut();
            servers.push(Some(TlsServerRec {
                agent_id: rt.agent_id(),
                listener_handle: handle,
                config,
                server_object: this,
                realm,
                http_server_id: hid,
                canonical: is_canon,
            }));
            servers.len() - 1
        });
        rt.set_engine_sentinel(this, "__tls_server_id", Value::Number(server_idx as f64));

        if let Some(cb) = args.iter().find(|v| rt.is_callable(v)).cloned() {
            let _ = rt.call_function(cb, Value::Object(this), vec![]);
        }
        net_emit(rt, this, "listening", vec![]);
        Ok(rt.current_this())
    });
    register_method(rt, server, "close", |rt, args| {
        let this = rt.current_this();
        if let Value::Object(id) = this {
            if let Value::Number(n) = rt.object_get(id, "__tls_server_id") {
                let agent_id = rt.agent_id();
                TLS_SERVERS.with(|v| {
                    if let Some(slot) = v.borrow_mut().get_mut(n as usize) {
                        if slot.as_ref().is_some_and(|rec| rec.agent_id == agent_id) {
                            *slot = None;
                        }
                    }
                });
            }
            let mut freed_sockets: Vec<ObjectRef> = Vec::new();
            let agent_id = rt.agent_id();
            TLS_SERVER_CONNS.with(|conns| {
                for slot in conns.borrow_mut().iter_mut() {
                    if slot
                        .as_ref()
                        .map(|conn| conn.agent_id == agent_id && conn.server_object == id)
                        .unwrap_or(false)
                    {
                        if let Some(conn) = slot.take() {
                            freed_sockets.push(conn.socket);
                            let _ = rusty_sockets::handle_close(conn.stream_id);
                        }
                    }
                }
            });
            rt.object_set(id, "listening".into(), Value::Boolean(false));
        }
        if let Some(cb) = args.iter().find(|v| rt.is_callable(v)).cloned() {
            let _ = rt.call_function(cb, this.clone(), vec![]);
        }
        Ok(this)
    });
    register_method(rt, server, "address", |rt, _a| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Null),
        };
        let bound = match rt.object_get(this, "__tls_bound_addr") {
            Value::String(s) => s.as_str().to_string(),
            _ => return Ok(Value::Null),
        };
        let (host, port) = split_bound_addr(&bound);
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
}

fn split_bound_addr(bound: &str) -> (String, u16) {
    if let Some((host, port)) = bound.rsplit_once(':') {
        (host.to_string(), port.parse::<u16>().unwrap_or(0))
    } else {
        (bound.to_string(), 0)
    }
}

fn do_create_server(
    rt: &mut Runtime,
    args: &[Value],
    server_proto: Option<ObjectRef>,
) -> Result<Value, RuntimeError> {

    let opts = match args.first() {
        Some(Value::Object(o)) => *o,
        _ => {
            return Err(RuntimeError::TypeError(
                "tls.createServer: options object required".into(),
            ))
        }
    };
    let config = match server_config_from_options(rt, opts) {
        Ok(config) => Some(config),
        Err(RuntimeError::TypeError(msg))
            if msg.contains("`cert` (PEM) required") || msg.contains("`key` (PEM) required") =>
        {
            None
        }
        Err(err) => return Err(err),
    };
    let server = new_object(rt);
    if let Some(proto) = server_proto {
        install_emitter(rt, server);
        rt.set_object_prototype_internal(server, Some(proto));
    } else {
        install_emitter(rt, server);
    }
    rt.obj_mut(server)
        .set_own_internal("__tls_server__".into(), Value::Boolean(true));
    if let Some(Value::Object(cb)) = args.get(1) {
        net_emit_add_listener(rt, server, "secureConnection", Value::Object(*cb));
    }
    install_tls_listen(rt, server, config, None, false);
    Ok(Value::Object(server))
}

pub fn do_create_tls_server_canonical(
    rt: &mut Runtime,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let opts = match args.first() {
        Some(Value::Object(o)) => *o,
        _ => {
            return Err(RuntimeError::TypeError(
                "cruft:tls.createServer: options object with cert + key required".into(),
            ))
        }
    };
    let config = server_config_from_options(rt, opts)?;
    let server = new_object(rt);
    rt.obj_mut(server)
        .set_own_internal("__tls_server__".into(), Value::Boolean(true));
    if let Some(cb) = args.get(1) {
        if rt.is_callable(cb) {
            rt.set_engine_sentinel(server, "__cruft_oncon", cb.clone());
        }
    }
    register_method(rt, server, "onConnection", |rt, args| {
        if let (Value::Object(this), Some(cb)) = (rt.current_this(), args.first()) {
            rt.set_engine_sentinel(this, "__cruft_oncon", cb.clone());
        }
        Ok(rt.current_this())
    });
    install_tls_listen(rt, server, Some(config), None, true);
    Ok(Value::Object(server))
}

pub fn do_create_https_server(rt: &mut Runtime, args: &[Value]) -> Result<Value, RuntimeError> {
    do_create_https_server_with_proto(rt, args, None)
}

pub fn do_create_https_server_with_proto(
    rt: &mut Runtime,
    args: &[Value],
    https_server_proto: Option<ObjectRef>,
) -> Result<Value, RuntimeError> {
    ensure_tls_server_namespace(rt);

    let (opts, handler) = match args.first() {
        Some(Value::Object(o)) => (Some(*o), args.get(1).cloned().unwrap_or(Value::Undefined)),
        Some(v) if rt.is_callable(v) => (None, v.clone()),
        _ => (None, Value::Undefined),
    };

    let config = match opts {
        Some(o) if matches!(pem_option(rt, rt.object_get(o, "cert")), Some(_)) => {
            let mut c = server_config_from_options(rt, o)?;

            c.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
            Some(c)
        }
        _ => None,
    };
    let server = new_object(rt);
    rt.obj_mut(server)
        .set_own_internal("__tls_server__".into(), Value::Boolean(true));
    install_emitter(rt, server);
    link_https_server_prototype(rt, server, https_server_proto);

    rt.object_set(server, "listening".into(), Value::Boolean(false));
    rt.object_set(server, "timeout".into(), Value::Number(0.0));
    rt.object_set(server, "keepAliveTimeout".into(), Value::Number(5000.0));
    rt.object_set(server, "requestTimeout".into(), Value::Number(300000.0));
    rt.object_set(server, "headersTimeout".into(), Value::Number(60000.0));
    rt.object_set(server, "maxHeadersCount".into(), Value::Null);
    register_method(rt, server, "setTimeout", |rt, args| {
        let this_id = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        if let Some(Value::Number(ms)) = args.first() {
            rt.object_set(this_id, "timeout".into(), Value::Number(*ms));
        }
        Ok(Value::Object(this_id))
    });
    let http_id = crate::http::register_https_handler(rt, server, handler)?;
    install_tls_listen(rt, server, config, Some(http_id), false);
    Ok(Value::Object(server))
}

fn ensure_tls_server_namespace(rt: &mut Runtime) {
    let ready = match rt.global_get("tls") {
        Value::Object(tls) => matches!(rt.object_get(tls, "Server"), Value::Object(_)),
        _ => false,
    };
    if !ready {
        install(rt);
    }
}

fn namespace_ctor_proto(rt: &Runtime, namespace: &str, ctor_name: &str) -> Option<ObjectRef> {
    let ns = match rt.global_get(namespace) {
        Value::Object(id) => id,
        _ => return None,
    };
    let ctor = match rt.object_get(ns, ctor_name) {
        Value::Object(id) => id,
        _ => return None,
    };
    match rt.object_get(ctor, "prototype") {
        Value::Object(id) => Some(id),
        _ => None,
    }
}

fn link_https_server_prototype(
    rt: &mut Runtime,
    server: ObjectRef,
    https_server_proto: Option<ObjectRef>,
) {
    let https_proto = https_server_proto.or_else(|| namespace_ctor_proto(rt, "https", "Server"));
    let tls_proto = namespace_ctor_proto(rt, "tls", "Server");
    let net_proto = match rt.global_get("__cruft_net_server_proto") {
        Value::Object(id) => Some(id),
        _ => None,
    }
    .or_else(|| namespace_ctor_proto(rt, "net", "Server"));
    let ee_proto = match rt.global_get("events") {
        Value::Object(events) => match rt.object_get(events, "EventEmitter") {
            Value::Object(ctor) => match rt.object_get(ctor, "prototype") {
                Value::Object(id) => Some(id),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    };
    if let (Some(nsp), Some(eep)) = (net_proto, ee_proto) {
        rt.set_object_prototype_internal(nsp, Some(eep));
    }
    let net_proto = net_proto.or_else(|| match rt.global_get("__cruft_net_server_proto") {
        Value::Object(id) => Some(id),
        _ => None,
    });
    if let (Some(tsp), Some(nsp)) = (tls_proto, net_proto) {
        rt.set_object_prototype_internal(tsp, Some(nsp));
    }
    if let (Some(hsp), Some(tsp)) = (https_proto, tls_proto) {
        rt.set_object_prototype_internal(hsp, Some(tsp));
    }
    if let Some(hsp) = https_proto {
        rt.set_object_prototype_internal(server, Some(hsp));
    }
}

pub fn do_create_cruft_http2_server(
    rt: &mut Runtime,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let opts = match args.first() {
        Some(Value::Object(o)) => *o,
        _ => {
            return Err(RuntimeError::TypeError(
                "cruft:http2.createSecureServer: options object with cert + key required".into(),
            ))
        }
    };
    let mut config = server_config_from_options(rt, opts)?;
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    let handler = match args.get(1) {
        Some(h) if rt.is_callable(h) => h.clone(),
        _ => Value::Undefined,
    };
    let server = new_object(rt);
    install_emitter(rt, server);

    let http_id = crate::http::register_https_handler(rt, server, Value::Undefined)?;
    crate::http::set_cruft_fetch_handler(rt, server, handler);
    install_tls_listen(rt, server, Some(config), Some(http_id), false);
    Ok(Value::Object(server))
}

pub fn do_create_node_http2_secure_server(
    rt: &mut Runtime,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let opts = match args.first() {
        Some(Value::Object(o)) => *o,
        _ => {
            return Err(RuntimeError::TypeError(
                "http2.createSecureServer: options object with cert + key required".into(),
            ))
        }
    };
    let mut config = server_config_from_options(rt, opts)?;
    config.alpn_protocols = vec![b"h2".to_vec()];
    let server = new_object(rt);
    install_emitter(rt, server);
    rt.set_engine_sentinel(server, "__h2_node_mode", Value::Boolean(true));
    if let Some(h) = args.get(1) {
        if rt.is_callable(h) {
            let on = rt.object_get(server, "on");
            if rt.is_callable(&on) {
                let _ = rt.call_function(
                    on,
                    Value::Object(server),
                    vec![
                        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                            "stream".to_string(),
                        ))),
                        h.clone(),
                    ],
                );
            }
        }
    }
    install_tls_listen(rt, server, Some(config), None, false);
    Ok(Value::Object(server))
}

pub fn install_http2_canonical(rt: &mut Runtime) {
    let ns = new_object(rt);
    register_method(rt, ns, "createSecureServer", |rt, args| {
        do_create_cruft_http2_server(rt, args)
    });
    rt.define_global_property("__cruft_http2", Value::Object(ns));
}

fn net_emit_add_listener(rt: &mut Runtime, obj: ObjectRef, event: &str, cb: Value) {
    let on = rt.object_get(obj, "on");
    let _ = rt.call_function(
        on,
        Value::Object(obj),
        vec![
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(event))),
            cb,
        ],
    );
}

pub fn server_poll_io(rt: &mut Runtime) -> Result<bool, RuntimeError> {

    let agent_id = rt.agent_id();
    let accepted: Option<(
        u64,
        ObjectRef,
        usize,
        Option<ServerConfig>,
        Option<usize>,
        bool,
    )> = TLS_SERVERS.with(|v| {
        for rec in v.borrow().iter().flatten() {
            if rec.agent_id != agent_id {
                continue;
            }
            if let Ok(Some(rusty_sockets::AsyncEvent::Connection { stream_id, .. })) =
                rusty_sockets::listener_poll(rec.listener_handle, 0)
            {
                return Some((
                    stream_id,
                    rec.server_object,
                    rec.realm,
                    rec.config.clone(),
                    rec.http_server_id,
                    rec.canonical,
                ));
            }
        }
        None
    });
    if let Some((stream_id, server_object, realm, config, http_server_id, canonical)) = accepted {
        let config = match config {
            Some(config) => config,
            None => {
                let _ = rusty_sockets::handle_close(stream_id);
                return Ok(true);
            }
        };
        let _ = rusty_sockets::stream_set_nonblocking(stream_id, true);
        let prior = rt.enter_realm(realm);
        let socket = make_server_tls_socket(rt);
        let machine = ServerHandshakeMachine::new(config)
            .map_err(|e| RuntimeError::TypeError(format!("tls server: {e}")))?;
        let id = TLS_SERVER_CONNS.with(|c| {
            let mut c = c.borrow_mut();
            c.push(Some(TlsServerConn {
                agent_id,
                stream_id,
                machine: Some(machine),
                server_app: None,
                client_app: None,
                server_seq: 0,
                client_seq: 0,
                rbuf: Vec::new(),
                socket,
                server_object,
                realm,
                http_server_id,
                http_rbuf: Vec::new(),
                canonical,
                h2: None,
                node_h2_mode: matches!(
                    rt.object_get(server_object, "__h2_node_mode"),
                    Value::Boolean(true)
                ),
            }));
            c.len() - 1
        });
        rt.set_engine_sentinel(socket, "__tls_sconn_id", Value::Number(id as f64));

        rt.exit_realm(prior);
        return Ok(true);
    }

    enum Out {
        Secure(ObjectRef, ObjectRef, usize, bool),
        Data(ObjectRef, usize, Vec<u8>, bool),
        Close(usize, ObjectRef, usize),
        Https(usize, usize, Vec<u8>),
        Http2(usize, Vec<u8>),
    }
    let out = TLS_SERVER_CONNS.with(|conns| -> Option<Out> {
        let mut conns = conns.borrow_mut();
        for i in 0..conns.len() {
            let conn = match conns[i].as_mut() {
                Some(c) => c,
                None => continue,
            };
            if conn.agent_id != agent_id {
                continue;
            }
            let read = rusty_sockets::stream_try_read(conn.stream_id, 65536)
                .ok()
                .flatten();
            let closed = matches!(&read, Some(b) if b.is_empty());
            if conn.machine.is_some() {
                if let Some(bytes) = &read {
                    let m = conn.machine.as_mut().unwrap();
                    match m.feed(bytes) {
                        Ok(outb) => {
                            if !outb.is_empty() {
                                let _ = rusty_sockets::stream_write_all(conn.stream_id, &outb);
                            }
                            if m.is_complete() {
                                conn.server_app = m.server_app.take();
                                conn.client_app = m.client_app.take();

                                if (conn.http_server_id.is_some() || conn.node_h2_mode)
                                    && m.negotiated_alpn.as_deref() == Some(b"h2")
                                {
                                    conn.h2 = Some(rusty_http2_conn::Http2Connection::new());
                                }
                                let leftover = m.drain_buffered();
                                conn.machine = None;
                                conn.rbuf = leftover;
                                return Some(Out::Secure(
                                    conn.socket,
                                    conn.server_object,
                                    conn.realm,
                                    conn.canonical,
                                ));
                            }
                        }
                        Err(_) => return Some(Out::Close(i, conn.socket, conn.realm)),
                    }
                } else if closed {
                    return Some(Out::Close(i, conn.socket, conn.realm));
                }
                continue;
            }

            if let Some(bytes) = &read {
                conn.rbuf.extend_from_slice(bytes);
            }
            if let Some((ct, frag, used)) = take_record(&conn.rbuf) {
                conn.rbuf.drain(0..used);
                if ct == 23 {
                    let keys = conn.client_app.clone();
                    if let Some(keys) = keys {
                        match aead_decrypt_record(&keys, conn.client_seq, &frag) {
                            Ok((inner_ct, pt)) => {
                                conn.client_seq += 1;
                                match inner_ct {
                                    23 => {
                                        if conn.h2.is_some() {

                                            return Some(Out::Http2(i, pt));
                                        } else if let Some(http_id) = conn.http_server_id {

                                            conn.http_rbuf.extend_from_slice(&pt);
                                            if crate::http::request_complete(&conn.http_rbuf) {
                                                let req = std::mem::take(&mut conn.http_rbuf);
                                                return Some(Out::Https(i, http_id, req));
                                            }

                                        } else {
                                            return Some(Out::Data(
                                                conn.socket,
                                                conn.realm,
                                                pt,
                                                conn.canonical,
                                            ));
                                        }
                                    }
                                    21 => return Some(Out::Close(i, conn.socket, conn.realm)),
                                    _ => {}
                                }
                            }
                            Err(_) => return Some(Out::Close(i, conn.socket, conn.realm)),
                        }
                    }
                }

            } else if closed {
                return Some(Out::Close(i, conn.socket, conn.realm));
            }
        }
        None
    });
    match out {
        Some(Out::Secure(socket, server_object, realm, canonical)) => {
            let prior = rt.enter_realm(realm);
            if canonical {

                let h = rt.object_get(server_object, "__cruft_oncon");
                if rt.is_callable(&h) {
                    let _ = rt.call_function(
                        h,
                        Value::Object(server_object),
                        vec![Value::Object(socket)],
                    );
                }
            } else {
                net_emit(rt, socket, "secureConnect", vec![]);
                net_emit(
                    rt,
                    server_object,
                    "secureConnection",
                    vec![Value::Object(socket)],
                );
            }
            rt.exit_realm(prior);
            Ok(true)
        }
        Some(Out::Data(socket, realm, chunk, canonical)) => {
            let prior = rt.enter_realm(realm);
            let buf = net_buffer_from_bytes(rt, &chunk);
            if canonical {

                let h = rt.object_get(socket, "__cruft_ondata");
                if rt.is_callable(&h) {
                    let _ = rt.call_function(h, Value::Object(socket), vec![buf]);
                }
            } else {
                net_emit(rt, socket, "data", vec![buf]);
            }
            rt.exit_realm(prior);
            Ok(true)
        }
        Some(Out::Close(idx, socket, realm)) => {
            let prior = rt.enter_realm(realm);
            net_emit(rt, socket, "end", vec![]);
            net_emit(rt, socket, "close", vec![]);
            rt.exit_realm(prior);
            TLS_SERVER_CONNS.with(|c| {
                if let Some(s) = c.borrow_mut().get_mut(idx) {
                    if s.as_ref().is_some_and(|conn| conn.agent_id == agent_id) {
                        *s = None;
                    }
                }
            });
            Ok(true)
        }
        Some(Out::Https(idx, http_id, req)) => {

            let (response, keep_alive) =
                crate::http::process_request_bytes(rt, http_id, &req, None);
            let mut dropped_socket: Option<ObjectRef> = None;
            TLS_SERVER_CONNS.with(|c| {
                let mut conns = c.borrow_mut();
                if let Some(conn) = conns.get_mut(idx).and_then(|x| x.as_mut()) {
                    if conn.agent_id != agent_id {
                        return;
                    }
                    if let Some(keys) = conn.server_app.clone() {
                        for chunk in response.chunks(16384) {
                            if let Ok(ct) = aead_encrypt_record(&keys, conn.server_seq, 23, chunk) {
                                conn.server_seq += 1;
                                let _ = rusty_sockets::stream_write_all(
                                    conn.stream_id,
                                    &frame_record(23, &ct),
                                );
                            }
                        }
                    }

                    if !keep_alive {
                        let sid = conn.stream_id;
                        dropped_socket = Some(conn.socket);
                        conns[idx] = None;
                        let _ = rusty_sockets::handle_close(sid);
                    }
                }
            });
            Ok(true)
        }
        Some(Out::Http2(idx, bytes)) => {

            let (http_id, server_obj, h2_stream_mode, h2_realm, requests) =
                TLS_SERVER_CONNS.with(|c| {
                    let mut conns = c.borrow_mut();
                    let conn = match conns.get_mut(idx).and_then(|x| x.as_mut()) {
                        Some(c) => c,
                        None => return (None, None, false, 0usize, Vec::new()),
                    };
                    if conn.agent_id != agent_id {
                        return (None, None, false, 0usize, Vec::new());
                    }
                    let so = conn.server_object;
                    let nm = conn.node_h2_mode;
                    let rlm = conn.realm;
                    let feed = match conn.h2.as_mut().unwrap().feed(&bytes) {
                        Ok(f) => f,
                        Err(_) => return (conn.http_server_id, Some(so), nm, rlm, Vec::new()),
                    };
                    if !feed.outbound.is_empty() {
                        encrypt_write_app(conn, &feed.outbound);
                    }
                    (conn.http_server_id, Some(so), nm, rlm, feed.requests)
                });

            if h2_stream_mode {
                let so = server_obj.unwrap();
                let prior = rt.enter_realm(h2_realm);
                for req in requests {
                    let hobj = new_object(rt);
                    for (k, v) in &req.headers {
                        rt.object_set(
                            hobj,
                            k.clone(),
                            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                                v.clone(),
                            ))),
                        );
                    }
                    let stream = crate::http2_client::make_server_stream(rt);
                    crate::net::net_emit(
                        rt,
                        so,
                        "stream",
                        vec![Value::Object(stream), Value::Object(hobj)],
                    );
                    if !req.body.is_empty() {
                        let buf = crate::net::net_buffer_from_bytes(rt, &req.body);
                        crate::net::net_emit(rt, stream, "data", vec![buf]);
                    }
                    crate::net::net_emit(rt, stream, "end", Vec::new());
                    let (status, headers, body) = crate::http2_client::extract_response(rt, stream);
                    let sid = req.stream_id;
                    TLS_SERVER_CONNS.with(|c| {
                        if let Some(conn) = c.borrow_mut().get_mut(idx).and_then(|x| x.as_mut()) {
                            if conn.agent_id != agent_id {
                                return;
                            }
                            let frames = conn
                                .h2
                                .as_mut()
                                .unwrap()
                                .respond(sid, status, &headers, &body);
                            encrypt_write_app(conn, &frames);
                        }
                    });
                }
                rt.exit_realm(prior);
                return Ok(true);
            }
            if let Some(http_id) = http_id {
                for req in requests {
                    let h1 = synthesize_http1_request(&req);
                    let (resp_bytes, _keep) =
                        crate::http::process_request_bytes(rt, http_id, &h1, None);
                    let (status, headers, body) = parse_http1_response(&resp_bytes);
                    TLS_SERVER_CONNS.with(|c| {
                        if let Some(conn) = c.borrow_mut().get_mut(idx).and_then(|x| x.as_mut()) {
                            if conn.agent_id != agent_id {
                                return;
                            }
                            let frames = conn.h2.as_mut().unwrap().respond(
                                req.stream_id,
                                status,
                                &headers,
                                &body,
                            );
                            encrypt_write_app(conn, &frames);
                        }
                    });
                }
            }
            Ok(true)
        }
        None => {
            let has_open = TLS_SERVERS.with(|v| {
                v.borrow()
                    .iter()
                    .any(|s| s.as_ref().is_some_and(|rec| rec.agent_id == agent_id))
            }) || TLS_SERVER_CONNS.with(|c| {
                c.borrow()
                    .iter()
                    .any(|s| s.as_ref().is_some_and(|rec| rec.agent_id == agent_id))
            });
            Ok(has_open && false)
        }
    }
}

fn encrypt_write_app(conn: &mut TlsServerConn, bytes: &[u8]) {
    if let Some(keys) = conn.server_app.clone() {
        for chunk in bytes.chunks(16384) {
            if let Ok(ct) = aead_encrypt_record(&keys, conn.server_seq, 23, chunk) {
                conn.server_seq += 1;
                let _ = rusty_sockets::stream_write_all(conn.stream_id, &frame_record(23, &ct));
            }
        }
    }
}

fn synthesize_http1_request(req: &rusty_http2_conn::Http2Request) -> Vec<u8> {
    let method = req.method().unwrap_or("GET");
    let path = req.path().unwrap_or("/");
    let mut s = format!("{method} {path} HTTP/1.1\r\n");
    let mut has_host = false;
    for (n, v) in &req.headers {
        if n.starts_with(':') {
            if n == ":authority" {
                s.push_str(&format!("host: {v}\r\n"));
                has_host = true;
            }
            continue;
        }
        let l = n.to_ascii_lowercase();
        if l == "content-length" || l == "host" {
            continue;
        }
        if l == "host" {
            has_host = true;
        }
        s.push_str(&format!("{n}: {v}\r\n"));
    }
    if !has_host {
        s.push_str("host: localhost\r\n");
    }
    s.push_str(&format!("content-length: {}\r\n\r\n", req.body.len()));
    let mut out = s.into_bytes();
    out.extend_from_slice(&req.body);
    out
}

fn parse_http1_response(bytes: &[u8]) -> (u16, Vec<(String, String)>, Vec<u8>) {
    match rusty_http_codec::parse_response(bytes) {
        Ok(r) => {
            let headers = r
                .headers
                .into_iter()
                .filter(|(n, _)| {
                    let l = n.to_ascii_lowercase();
                    l != "connection"
                        && l != "transfer-encoding"
                        && l != "keep-alive"
                        && l != "upgrade"
                        && l != "content-length"
                })
                .collect();
            (r.status, headers, r.body)
        }
        Err(_) => (502, Vec::new(), Vec::new()),
    }
}

pub fn install(rt: &mut Runtime) {
    if matches!(namespace_ctor_proto(rt, "tls", "Server"), Some(_)) {
        return;
    }
    let ns = new_object(rt);
    register_method(rt, ns, "connect", |rt, args| do_connect(rt, args));
    register_method(rt, ns, "createSecureContext", |rt, _a| {
        Ok(Value::Object(new_object(rt)))
    });

    let mut server_proto = None;
    for cls in &["TLSSocket", "Server"] {
        let class_name = (*cls).to_string();
        let cid = crate::register::make_callable(rt, cls, move |rt, args| {
            let inst = new_object(rt);
            install_emitter(rt, inst);
            if class_name == "TLSSocket" {
                let stream = args.first().cloned().unwrap_or(Value::Undefined);
                install_node_js_stream_wrap_shape(rt, inst, stream);
            }
            Ok(Value::Object(inst))
        });
        let proto = new_object(rt);
        if *cls == "Server" {
            install_emitter(rt, proto);
            server_proto = Some(proto);
        }
        rt.object_set(proto, "constructor".into(), Value::Object(cid));
        rt.object_set(cid, "prototype".into(), Value::Object(proto));
        rt.object_set(ns, (*cls).into(), Value::Object(cid));
    }
    let node_server_proto = server_proto;
    register_method(rt, ns, "createServer", move |rt, args| {
        do_create_server(rt, args, node_server_proto)
    });
    rt.object_set(ns, "CLIENT_RENEG_LIMIT".into(), Value::Number(3f64));
    rt.object_set(ns, "CLIENT_RENEG_WINDOW".into(), Value::Number(600f64));
    rt.object_set(ns, "DEFAULT_CIPHERS".into(), Value::String(Rc::new(rusty_js_runtime::value::JsString::from("TLS_AES_256_GCM_SHA384:TLS_CHACHA20_POLY1305_SHA256:TLS_AES_128_GCM_SHA256:HIGH:!aNULL:!eNULL:!EXPORT:!DES:!RC4:!MD5:!PSK:!SRP:!CAMELLIA".to_string()))));
    rt.object_set(
        ns,
        "DEFAULT_ECDH_CURVE".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "auto".to_string(),
        ))),
    );
    rt.object_set(
        ns,
        "DEFAULT_MAX_VERSION".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "TLSv1.3".to_string(),
        ))),
    );
    rt.object_set(
        ns,
        "DEFAULT_MIN_VERSION".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "TLSv1.2".to_string(),
        ))),
    );
    {
        let c = crate::register::make_callable(rt, "SecureContext", |rt, _a| Ok(rt.current_this()));
        let p = new_object(rt);
        rt.object_set(p, "constructor".into(), Value::Object(c));
        rt.object_set(c, "prototype".into(), Value::Object(p));
        rt.object_set(ns, "SecureContext".into(), Value::Object(c));
    }
    {
        let a = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
        for pem in platform_root_certificate_pems() {
            let idx = match rt.object_get(a, "length") {
                Value::Number(n) if n >= 0.0 => n as usize,
                _ => 0,
            };
            rt.object_set(
                a,
                idx.to_string(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(pem))),
            );
            rt.object_set(a, "length".into(), Value::Number((idx + 1) as f64));
        }
        let _ = rt.object_freeze_via(&Value::Object(a));
        rt.object_set(ns, "rootCertificates".into(), Value::Object(a));
    }
    register_method(rt, ns, "convertALPNProtocols", |rt, args| {

        let out = match args.get(1) {
            Some(Value::Object(id)) => *id,
            _ => return Ok(Value::Undefined),
        };
        let stored: Option<Value> = match args.first() {
            Some(Value::Object(pid)) => {
                let pid = *pid;
                if rt.obj(pid).is_buffer {
                    Some(Value::Object(pid))
                } else {
                    let is_typed =
                        matches!(rt.object_get(pid, "BYTES_PER_ELEMENT"), Value::Number(_));
                    let mut wire = Vec::new();
                    if is_typed {
                        let len = match rt.object_get(pid, "length") {
                            Value::Number(n) if n >= 0.0 => n as usize,
                            _ => 0,
                        };
                        for i in 0..len {
                            if let Value::Number(n) = rt.object_get(pid, &i.to_string()) {
                                wire.push(n as u8);
                            }
                        }
                    } else {
                        let len = rt.array_length(pid);
                        for i in 0..len {
                            if let Value::String(s) = rt.object_get(pid, &i.to_string()) {
                                let b = s.as_bytes();
                                wire.push(b.len() as u8);
                                wire.extend_from_slice(b);
                            }
                        }
                    }
                    Some(net_buffer_from_bytes(rt, &wire))
                }
            }
            _ => None,
        };
        if let Some(v) = stored {
            rt.object_set(out, "ALPNProtocols".into(), v);
        }
        Ok(Value::Undefined)
    });
    register_method(rt, ns, "getCACertificates", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, ns, "setDefaultCACertificates", |_rt, _a| {
        Ok(Value::Undefined)
    });
    register_method(rt, ns, "checkServerIdentity", |_rt, _a| {
        Ok(Value::Undefined)
    });
    register_method(rt, ns, "getCiphers", |rt, _a| {

        let arr = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
        for (i, name) in ["ecdhe-ecdsa-aes128-gcm-sha256", "tls_aes_128_gcm_sha256"]
            .iter()
            .enumerate()
        {
            rt.object_set(
                arr,
                i.to_string(),
                Value::String(std::rc::Rc::new(rusty_js_runtime::value::JsString::from(
                    *name,
                ))),
            );
        }
        Ok(Value::Object(arr))
    });
    rt.define_global_property("tls", Value::Object(ns));
}

#[cfg(test)]
mod tests {
    use super::{
        collect_roots_for_runtime, has_pending_tls_connect, notify_agent_wake,
        poll_pending_tls_connect, PendingTlsConnect, TlsServerConn, TlsServerRec,
        PENDING_TLS_CONNECTS, TLS_SERVERS, TLS_SERVER_CONNS,
    };
    use rusty_js_runtime::{AgentId, Object, Runtime, Value};
    use std::sync::mpsc;

    #[test]
    fn tls_pending_connect_completion_notifies_owner_wake() {
        let rt = Runtime::new_with_agent_id(AgentId::from_raw(1021));
        let observed = rt.agent_wake_generation();
        let wake = rt.agent_wake_handle();
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            tx.send(()).expect("send synthetic tls completion");
            notify_agent_wake(&wake);
        });

        rx.recv().expect("receive synthetic tls completion");
        assert!(
            rt.wait_agent_wake_timeout(observed, std::time::Duration::from_secs(1)),
            "TLS pending-connect producer must wake the owning runtime"
        );
    }

    #[test]
    fn pending_tls_connect_registry_is_scoped_by_runtime_agent_id() {
        let mut rt_a = Runtime::new_with_agent_id(AgentId::from_raw(1001));
        let mut rt_b = Runtime::new_with_agent_id(AgentId::from_raw(1002));
        let socket_b = rt_b.alloc_object(Object::new_ordinary());
        let (_tx, rx) = mpsc::channel();

        PENDING_TLS_CONNECTS.with(|pending| {
            pending.borrow_mut().clear();
            pending.borrow_mut().push(Some(PendingTlsConnect {
                agent_id: rt_b.agent_id(),
                rx,
                socket: socket_b,
                callback: Value::Undefined,
                realm: rt_b.current_realm,
                root_key: "tls-connect:test-agent-b".to_string(),
            }));
        });

        assert!(!has_pending_tls_connect(&rt_a));
        assert!(has_pending_tls_connect(&rt_b));
        assert!(
            !poll_pending_tls_connect(&mut rt_a).expect("poll agent A"),
            "agent A must not harvest agent B's pending TLS connect"
        );
        assert!(has_pending_tls_connect(&rt_b));

        PENDING_TLS_CONNECTS.with(|pending| pending.borrow_mut().clear());
    }

    #[test]
    fn tls_server_roots_are_scoped_by_runtime_agent_id() {
        let mut rt_a = Runtime::new_with_agent_id(AgentId::from_raw(1011));
        let mut rt_b = Runtime::new_with_agent_id(AgentId::from_raw(1012));
        let server_b = rt_b.alloc_object(Object::new_ordinary());
        let socket_b = rt_b.alloc_object(Object::new_ordinary());

        TLS_SERVERS.with(|servers| {
            servers.borrow_mut().clear();
            servers.borrow_mut().push(Some(TlsServerRec {
                agent_id: rt_b.agent_id(),
                listener_handle: 0,
                config: None,
                server_object: server_b,
                realm: rt_b.current_realm,
                http_server_id: None,
                canonical: false,
            }));
        });
        TLS_SERVER_CONNS.with(|conns| {
            conns.borrow_mut().clear();
            conns.borrow_mut().push(Some(TlsServerConn {
                agent_id: rt_b.agent_id(),
                stream_id: 0,
                machine: None,
                server_app: None,
                client_app: None,
                server_seq: 0,
                client_seq: 0,
                rbuf: Vec::new(),
                socket: socket_b,
                server_object: server_b,
                realm: rt_b.current_realm,
                http_server_id: None,
                http_rbuf: Vec::new(),
                canonical: false,
                h2: None,
                node_h2_mode: false,
            }));
        });

        let mut roots_a = Vec::new();
        collect_roots_for_runtime(&rt_a, &mut roots_a);
        assert!(!roots_a.contains(&server_b));
        assert!(!roots_a.contains(&socket_b));

        let mut roots_b = Vec::new();
        collect_roots_for_runtime(&rt_b, &mut roots_b);
        assert!(roots_b.contains(&server_b));
        assert!(roots_b.contains(&socket_b));

        TLS_SERVERS.with(|servers| servers.borrow_mut().clear());
        TLS_SERVER_CONNS.with(|conns| conns.borrow_mut().clear());
    }
}
