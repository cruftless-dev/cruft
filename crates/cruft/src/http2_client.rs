
use crate::hpack::{HpackDecoder, HpackEncoder};
use crate::net::{install_emitter, net_emit};
use crate::register::{new_object, register_method};
use rusty_js_runtime::value::ObjectRef;
use rusty_js_runtime::{HostEnqueuePhase, Runtime, RuntimeError, Value};
use std::rc::Rc;

fn sval(s: &str) -> Value {
    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
        s.to_string(),
    )))
}

fn frame(ftype: u8, flags: u8, stream_id: u32, payload: &[u8]) -> Vec<u8> {
    let len = payload.len();
    let mut out = Vec::with_capacity(9 + len);
    out.push((len >> 16) as u8);
    out.push((len >> 8) as u8);
    out.push(len as u8);
    out.push(ftype);
    out.push(flags);
    out.extend_from_slice(&(stream_id & 0x7fff_ffff).to_be_bytes());
    out.extend_from_slice(payload);
    out
}

fn headers_block(payload: &[u8], flags: u8) -> Vec<u8> {
    let mut start = 0usize;
    let mut end = payload.len();
    if flags & 0x08 != 0 && !payload.is_empty() {
        let pad = payload[0] as usize;
        start = 1;
        end = end.saturating_sub(pad);
    }
    if flags & 0x20 != 0 {
        start += 5;
    }
    if start > end {
        return Vec::new();
    }
    payload[start..end].to_vec()
}

fn data_block(payload: &[u8], flags: u8) -> Vec<u8> {
    if flags & 0x08 != 0 && !payload.is_empty() {
        let pad = payload[0] as usize;
        let end = payload.len().saturating_sub(pad);
        if 1 <= end {
            return payload[1..end].to_vec();
        }
        return Vec::new();
    }
    payload.to_vec()
}

trait H2Io {
    fn write_all(&mut self, data: &[u8]) -> Result<(), String>;
    fn read(&mut self) -> Result<Vec<u8>, String>;
    fn close(&mut self);
}

struct TcpIo {
    id: u64,
}
impl H2Io for TcpIo {
    fn write_all(&mut self, data: &[u8]) -> Result<(), String> {
        rusty_sockets::stream_write_all(self.id, data).map_err(|e| format!("write: {e:?}"))
    }
    fn read(&mut self) -> Result<Vec<u8>, String> {
        rusty_sockets::stream_read(self.id, 65536).map_err(|e| format!("read: {e:?}"))
    }
    fn close(&mut self) {
        let _ = rusty_sockets::handle_close(self.id);
    }
}

struct TlsIo {
    session: rusty_tls::driver::TlsSession<rusty_tls::driver::TcpTlsTransport>,
    acc: Vec<u8>,
}
impl H2Io for TlsIo {
    fn write_all(&mut self, data: &[u8]) -> Result<(), String> {
        self.session
            .send_application_data(data)
            .map_err(|e| format!("tls write: {e:?}"))
    }
    fn read(&mut self) -> Result<Vec<u8>, String> {

        Ok(self
            .session
            .receive_application_data(&mut self.acc)
            .map(|c| c.to_vec())
            .unwrap_or_default())
    }
    fn close(&mut self) {}
}

fn h2_exchange(
    io: &mut dyn H2Io,
    headers: &[(String, String)],
    body: Option<&[u8]>,
) -> Result<(Vec<(String, String)>, Vec<u8>), String> {
    let mut out = Vec::new();
    out.extend_from_slice(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n");

    out.extend_from_slice(&frame(0x4, 0, 0, &[0x00, 0x04, 0x7f, 0xff, 0xff, 0xff]));

    out.extend_from_slice(&frame(0x8, 0, 0, &[0x3f, 0xff, 0x00, 0x00]));
    let mut enc = HpackEncoder::new();
    let hblock = enc.encode(headers);
    let h_flags = if body.is_none() { 0x04 | 0x01 } else { 0x04 };
    out.extend_from_slice(&frame(0x1, h_flags, 1, &hblock));
    if let Some(b) = body {
        out.extend_from_slice(&frame(0x0, 0x01, 1, b));
    }
    io.write_all(&out)?;

    let mut buf: Vec<u8> = Vec::new();
    let mut dec = HpackDecoder::new();
    let mut resp_headers: Vec<(String, String)> = Vec::new();
    let mut resp_body: Vec<u8> = Vec::new();
    let mut done = false;
    let mut guard = 0;
    while !done {
        guard += 1;
        if guard > 100_000 {
            return Err("h2: frame loop runaway".into());
        }
        let chunk = io.read()?;
        if chunk.is_empty() && buf.len() < 9 {
            break;
        }
        buf.extend_from_slice(&chunk);
        loop {
            if buf.len() < 9 {
                break;
            }
            let len = ((buf[0] as usize) << 16) | ((buf[1] as usize) << 8) | buf[2] as usize;
            if buf.len() < 9 + len {
                break;
            }
            let ftype = buf[3];
            let fflags = buf[4];
            let payload = buf[9..9 + len].to_vec();
            buf.drain(0..9 + len);
            match ftype {
                0x4 => {
                    if fflags & 0x01 == 0 {
                        let _ = io.write_all(&frame(0x4, 0x01, 0, &[]));
                    }
                }
                0x1 => {
                    let hb = headers_block(&payload, fflags);
                    resp_headers = dec.decode(&hb)?;
                    if fflags & 0x01 != 0 {
                        done = true;
                    }
                }
                0x0 => {
                    resp_body.extend_from_slice(&data_block(&payload, fflags));
                    if fflags & 0x01 != 0 {
                        done = true;
                    }
                }
                0x6 => {
                    if fflags & 0x01 == 0 {
                        let _ = io.write_all(&frame(0x6, 0x01, 0, &payload));
                    }
                }
                0x7 => {
                    done = true;
                }
                _ => {}
            }
        }
    }
    io.close();
    Ok((resp_headers, resp_body))
}

fn h2_request_tcp(
    host: &str,
    port: u16,
    headers: &[(String, String)],
    body: Option<&[u8]>,
) -> Result<(Vec<(String, String)>, Vec<u8>), String> {
    let id = rusty_sockets::stream_connect(&format!("{host}:{port}"))
        .map_err(|e| format!("connect: {e:?}"))?;
    let mut io = TcpIo { id };
    h2_exchange(&mut io, headers, body)
}

fn h2_request_tls(
    host: &str,
    port: u16,
    headers: &[(String, String)],
    body: Option<&[u8]>,
    insecure: bool,
) -> Result<(Vec<(String, String)>, Vec<u8>), String> {
    let trust =
        rusty_tls::store::TrustStore::load_system_default().map_err(|e| format!("trust: {e:?}"))?;
    let session = rusty_tls::driver::tls_connect_with_alpn_config(
        host,
        port,
        &trust,
        Some(&[b"h2"]),
        rusty_tls::driver::TlsClientConfig {
            insecure_skip_certificate_validation: insecure,
        },
    )
    .map_err(|e| format!("tls: {e:?}"))?;
    let mut io = TlsIo {
        session,
        acc: Vec::new(),
    };
    h2_exchange(&mut io, headers, body)
}

fn h2_request(
    host: &str,
    port: u16,
    scheme: &str,
    insecure: bool,
    headers: &[(String, String)],
    body: Option<&[u8]>,
) -> Result<(Vec<(String, String)>, Vec<u8>), String> {
    if scheme == "https" {
        h2_request_tls(host, port, headers, body, insecure)
    } else {
        h2_request_tcp(host, port, headers, body)
    }
}

fn obj_str(rt: &Runtime, obj: ObjectRef, key: &str) -> Option<String> {
    match rt.object_get(obj, key) {
        Value::String(s) => Some(s.as_str().to_string()),
        _ => None,
    }
}

fn build_headers(
    rt: &Runtime,
    session: ObjectRef,
    hdrs: Option<ObjectRef>,
) -> Vec<(String, String)> {
    let authority = obj_str(rt, session, "__h2_authority").unwrap_or_default();
    let scheme = obj_str(rt, session, "__h2_scheme").unwrap_or_else(|| "http".into());
    let mut method = "GET".to_string();
    let mut path = "/".to_string();
    let mut extra: Vec<(String, String)> = Vec::new();
    if let Some(h) = hdrs {
        for k in rt.ordinary_own_enumerable_string_keys(h) {
            let v = match rt.object_get(h, &k) {
                Value::String(s) => s.as_str().to_string(),
                Value::Number(n) => {
                    if n.fract() == 0.0 {
                        format!("{}", n as i64)
                    } else {
                        format!("{n}")
                    }
                }
                _ => continue,
            };
            match k.as_str() {
                ":method" => method = v,
                ":path" => path = v,
                ":authority" => {}
                ":scheme" => {}
                _ => extra.push((k.to_ascii_lowercase(), v)),
            }
        }
    }
    let mut out = vec![
        (":method".to_string(), method),
        (":path".to_string(), path),
        (":scheme".to_string(), scheme),
        (":authority".to_string(), authority),
    ];
    out.extend(extra);
    out
}

pub fn make_session(rt: &mut Runtime, url: &str, insecure: bool) -> ObjectRef {

    let rest = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url);
    let scheme = if url.starts_with("https://") {
        "https"
    } else {
        "http"
    };
    let authority = rest.split('/').next().unwrap_or(rest).to_string();
    let default_port = if scheme == "https" { 443 } else { 80 };
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse::<u16>().unwrap_or(default_port)),
        None => (authority.clone(), default_port),
    };
    let session = new_object(rt);
    rt.obj_mut(session)
        .set_own_internal("__http2_client_session__".into(), Value::Boolean(true));
    install_emitter(rt, session);
    rt.object_set(session, "__h2_host".into(), sval(&host));
    rt.object_set(session, "__h2_port".into(), Value::Number(port as f64));
    rt.object_set(session, "__h2_scheme".into(), sval(scheme));
    rt.object_set(session, "__h2_authority".into(), sval(&authority));
    rt.object_set(session, "__h2_insecure".into(), Value::Boolean(insecure));
    rt.object_set(session, "connecting".into(), Value::Boolean(false));
    rt.object_set(session, "closed".into(), Value::Boolean(false));

    register_method(rt, session, "request", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(t) => t,
            _ => return Ok(Value::Undefined),
        };
        let hdrs = match args.first() {
            Some(Value::Object(o)) => Some(*o),
            _ => None,
        };
        let host = obj_str(rt, this, "__h2_host").unwrap_or_default();
        let port = match rt.object_get(this, "__h2_port") {
            Value::Number(n) => n as u16,
            _ => 80,
        };
        let scheme = obj_str(rt, this, "__h2_scheme").unwrap_or_else(|| "http".into());
        let insecure = matches!(rt.object_get(this, "__h2_insecure"), Value::Boolean(true));
        let headers = build_headers(rt, this, hdrs);
        let realm = rt.current_realm;
        let stream = new_object(rt);
        install_emitter(rt, stream);

        rt.enqueue_host_phase_rooted(
            HostEnqueuePhase::HostCompletionMacrotask,
            "http2 request",
            vec![stream],
            move |rt| {
                let prior = rt.enter_realm(realm);
                let body_bytes: Vec<u8> = match rt.object_get(stream, "__h2_body") {
                    Value::String(s) => (0..s.as_str().len() / 2)
                        .filter_map(|i| u8::from_str_radix(&s.as_str()[i * 2..i * 2 + 2], 16).ok())
                        .collect(),
                    _ => Vec::new(),
                };
                let body_opt = if body_bytes.is_empty() {
                    None
                } else {
                    Some(body_bytes.as_slice())
                };
                match h2_request(&host, port, &scheme, insecure, &headers, body_opt) {
                    Ok((resp_headers, body)) => {
                        let hobj = new_object(rt);
                        for (k, v) in &resp_headers {
                            rt.object_set(hobj, k.clone(), sval(v));
                        }
                        net_emit(
                            rt,
                            stream,
                            "response",
                            vec![Value::Object(hobj), Value::Number(0.0)],
                        );
                        if !body.is_empty() {
                            let buf = crate::net::net_buffer_from_bytes(rt, &body);
                            net_emit(rt, stream, "data", vec![buf]);
                        }
                        net_emit(rt, stream, "end", Vec::new());
                        net_emit(rt, stream, "close", Vec::new());
                    }
                    Err(e) => {
                        net_emit(rt, stream, "error", vec![sval(&format!("http2: {e}"))]);
                    }
                }
                rt.exit_realm(prior);
                Ok(())
            },
        );
        for noop in ["setEncoding", "close", "resume", "pause"] {
            register_method(rt, stream, noop, |rt, _a| Ok(rt.current_this()));
        }

        register_method(rt, stream, "write", |rt, args| {
            if let Value::Object(this) = rt.current_this() {
                let mut cur = match rt.object_get(this, "__h2_body") {
                    Value::String(s) => s.as_str().to_string(),
                    _ => String::new(),
                };
                for b in crate::net::extract_bytes_pub(
                    rt,
                    &args.first().cloned().unwrap_or(Value::Undefined),
                ) {
                    cur.push_str(&format!("{b:02x}"));
                }
                rt.object_set(this, "__h2_body".into(), sval(&cur));
            }
            Ok(Value::Boolean(true))
        });
        register_method(rt, stream, "end", |rt, args| {
            if let Value::Object(this) = rt.current_this() {
                if let Some(v) = args.first() {
                    if !matches!(v, Value::Undefined | Value::Null) && !rt.is_callable(v) {
                        let mut cur = match rt.object_get(this, "__h2_body") {
                            Value::String(s) => s.as_str().to_string(),
                            _ => String::new(),
                        };
                        for b in crate::net::extract_bytes_pub(rt, v) {
                            cur.push_str(&format!("{b:02x}"));
                        }
                        rt.object_set(this, "__h2_body".into(), sval(&cur));
                    }
                }
            }
            Ok(rt.current_this())
        });
        Ok(Value::Object(stream))
    });
    for noop in ["close", "ref", "unref", "setTimeout", "ping", "goaway"] {
        register_method(rt, session, noop, |rt, _a| Ok(rt.current_this()));
    }
    session
}

use std::cell::RefCell;

struct H2Server {
    listener: u64,
    server_obj: ObjectRef,
    realm: usize,
}
thread_local! {
    static H2_SERVERS: RefCell<Vec<Option<H2Server>>> = const { RefCell::new(Vec::new()) };
}

pub fn make_http2_server(rt: &mut Runtime, handler: Option<Value>) -> ObjectRef {
    let server = new_object(rt);
    rt.obj_mut(server)
        .set_own_internal("__http2_server__".into(), Value::Boolean(true));
    install_emitter(rt, server);
    if let Some(h) = handler {
        if rt.is_callable(&h) {

            let on = rt.object_get(server, "on");
            if rt.is_callable(&on) {
                let _ = rt.call_function(on, Value::Object(server), vec![sval("stream"), h]);
            }
        }
    }
    register_method(rt, server, "listen", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(t) => t,
            _ => return Ok(Value::Undefined),
        };
        let port = match args.first() {
            Some(Value::Number(n)) => *n as u16,
            _ => 0,
        };
        let cb = args.iter().rev().find(|v| rt.is_callable(v)).cloned();
        match rusty_sockets::listener_bind(&format!("0.0.0.0:{port}")) {
            Ok((lid, _addr)) => {
                let _ = rusty_sockets::listener_set_nonblocking(lid, true);
                let realm = rt.current_realm;
                H2_SERVERS.with(|v| {
                    v.borrow_mut().push(Some(H2Server {
                        listener: lid,
                        server_obj: this,
                        realm,
                    }))
                });
                rt.set_engine_sentinel(this, "__h2_listener".into(), Value::Number(lid as f64));
                net_emit(rt, this, "listening", Vec::new());
                if let Some(cb) = cb {
                    let _ = rt.call_function(cb, Value::Undefined, Vec::new());
                }
            }
            Err(e) => {
                net_emit(rt, this, "error", vec![sval(&format!("listen: {e:?}"))]);
            }
        }
        Ok(rt.current_this())
    });
    register_method(rt, server, "close", |rt, _a| {
        let this = match rt.current_this() {
            Value::Object(t) => t,
            _ => return Ok(Value::Undefined),
        };
        if let Value::Number(lid) = rt.object_get(this, "__h2_listener") {
            H2_SERVERS.with(|v| {
                for s in v.borrow_mut().iter_mut() {
                    if s.as_ref().is_some_and(|x| x.listener == lid as u64) {
                        *s = None;
                    }
                }
            });
            let _ = rusty_sockets::handle_close(lid as u64);
        }
        net_emit(rt, this, "close", Vec::new());
        Ok(rt.current_this())
    });
    for noop in ["ref", "unref", "setTimeout"] {
        register_method(rt, server, noop, |rt, _a| Ok(rt.current_this()));
    }
    server
}

pub(crate) fn make_server_stream(rt: &mut Runtime) -> ObjectRef {
    let stream = new_object(rt);
    install_emitter(rt, stream);
    register_method(rt, stream, "respond", |rt, args| {
        if let Value::Object(this) = rt.current_this() {
            if let Some(Value::Object(h)) = args.first() {
                rt.object_set(this, "__resp_h".into(), Value::Object(*h));
            }
        }
        Ok(rt.current_this())
    });
    register_method(rt, stream, "end", |rt, args| {
        if let Value::Object(this) = rt.current_this() {
            if let Some(v) = args.first() {
                if !matches!(v, Value::Undefined | Value::Null) {
                    let bytes = crate::net::extract_bytes_pub(rt, v);
                    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
                    rt.object_set(this, "__resp_body".into(), sval(&hex));
                }
            }
            rt.object_set(this, "__resp_ended".into(), Value::Boolean(true));
        }
        Ok(rt.current_this())
    });
    for noop in ["write", "close", "setEncoding"] {
        register_method(rt, stream, noop, |rt, _a| Ok(rt.current_this()));
    }
    stream
}

pub(crate) fn extract_response(
    rt: &mut Runtime,
    stream: ObjectRef,
) -> (u16, Vec<(String, String)>, Vec<u8>) {
    let mut status = 200u16;
    let mut headers: Vec<(String, String)> = Vec::new();
    if let Value::Object(rh) = rt.object_get(stream, "__resp_h") {
        for k in rt.ordinary_own_enumerable_string_keys(rh) {
            let v = match rt.object_get(rh, &k) {
                Value::String(s) => s.as_str().to_string(),
                Value::Number(n) => {
                    if n.fract() == 0.0 {
                        format!("{}", n as i64)
                    } else {
                        format!("{n}")
                    }
                }
                _ => continue,
            };
            if k == ":status" {
                status = v.parse().unwrap_or(200);
            } else if !k.starts_with(':') {
                headers.push((k.to_ascii_lowercase(), v));
            }
        }
    }
    let body: Vec<u8> = match rt.object_get(stream, "__resp_body") {
        Value::String(s) => (0..s.as_str().len() / 2)
            .filter_map(|i| u8::from_str_radix(&s.as_str()[i * 2..i * 2 + 2], 16).ok())
            .collect(),
        _ => Vec::new(),
    };
    (status, headers, body)
}

struct H2cConn {
    stream_id: u64,
    conn: rusty_http2_conn::Http2Connection,
    server_obj: ObjectRef,
    realm: usize,
}
thread_local! { static H2C_CONNS: RefCell<Vec<Option<H2cConn>>> = const { RefCell::new(Vec::new()) }; }

pub fn collect_roots(roots: &mut Vec<ObjectRef>) {
    H2_SERVERS.with(|v| {
        for s in v.borrow().iter().flatten() {
            roots.push(s.server_obj);
        }
    });
    H2C_CONNS.with(|v| {
        for c in v.borrow().iter().flatten() {
            roots.push(c.server_obj);
        }
    });
}

pub fn collect_roots_for_runtime(_rt: &Runtime, roots: &mut Vec<ObjectRef>) {
    collect_roots(roots);
}

pub fn server_poll(rt: &mut Runtime) -> Result<bool, RuntimeError> {

    let servers: Vec<(u64, ObjectRef, usize)> = H2_SERVERS.with(|v| {
        v.borrow()
            .iter()
            .filter_map(|s| s.as_ref().map(|s| (s.listener, s.server_obj, s.realm)))
            .collect()
    });
    for (lid, server_obj, realm) in &servers {
        if let Ok(Some((cid, _))) = rusty_sockets::listener_try_accept(*lid) {
            let _ = rusty_sockets::stream_set_nonblocking(cid, true);
            H2C_CONNS.with(|v| {
                v.borrow_mut().push(Some(H2cConn {
                    stream_id: cid,
                    conn: rusty_http2_conn::Http2Connection::new(),
                    server_obj: *server_obj,
                    realm: *realm,
                }))
            });
        }
    }

    let ids: Vec<usize> = H2C_CONNS.with(|v| {
        (0..v.borrow().len())
            .filter(|i| v.borrow()[*i].is_some())
            .collect()
    });
    let mut fired = false;
    for i in ids {
        let cid = H2C_CONNS.with(|v| {
            v.borrow()
                .get(i)
                .and_then(|x| x.as_ref())
                .map(|c| c.stream_id)
        });
        let cid = match cid {
            Some(c) => c,
            None => continue,
        };
        let chunk = match rusty_sockets::stream_try_read(cid, 65536) {
            Ok(Some(b)) if b.is_empty() => {

                let _ = rusty_sockets::handle_close(cid);
                H2C_CONNS.with(|v| {
                    if let Some(slot) = v.borrow_mut().get_mut(i) {
                        *slot = None;
                    }
                });
                continue;
            }
            Ok(Some(b)) => b,
            _ => continue,
        };
        fired = true;

        let (server_obj, realm, requests) = H2C_CONNS.with(|v| {
            let mut b = v.borrow_mut();
            let c = match b.get_mut(i).and_then(|x| x.as_mut()) {
                Some(c) => c,
                None => return (None, 0usize, Vec::new()),
            };
            let feed = match c.conn.feed(&chunk) {
                Ok(f) => f,
                Err(_) => return (Some(c.server_obj), c.realm, Vec::new()),
            };
            if !feed.outbound.is_empty() {
                let _ = rusty_sockets::stream_write_all(c.stream_id, &feed.outbound);
            }
            (Some(c.server_obj), c.realm, feed.requests)
        });
        let server_obj = match server_obj {
            Some(s) => s,
            None => continue,
        };
        for req in requests {
            let prior = rt.enter_realm(realm);
            let hobj = new_object(rt);
            for (k, v) in &req.headers {
                rt.object_set(hobj, k.clone(), sval(v));
            }
            let stream = make_server_stream(rt);
            net_emit(
                rt,
                server_obj,
                "stream",
                vec![Value::Object(stream), Value::Object(hobj)],
            );
            if !req.body.is_empty() {
                let buf = crate::net::net_buffer_from_bytes(rt, &req.body);
                net_emit(rt, stream, "data", vec![buf]);
            }
            net_emit(rt, stream, "end", Vec::new());
            let (status, headers, body) = extract_response(rt, stream);
            let sid = req.stream_id;
            H2C_CONNS.with(|v| {
                if let Some(c) = v.borrow_mut().get_mut(i).and_then(|x| x.as_mut()) {
                    let frames = c.conn.respond(sid, status, &headers, &body);
                    let _ = rusty_sockets::stream_write_all(c.stream_id, &frames);
                }
            });
            rt.exit_realm(prior);
        }
    }
    if fired {
        return Ok(true);
    }
    let listening =
        !servers.is_empty() || H2C_CONNS.with(|v| v.borrow().iter().any(|s| s.is_some()));
    if listening {
        std::thread::sleep(std::time::Duration::from_millis(1));
        return Ok(true);
    }
    Ok(false)
}
