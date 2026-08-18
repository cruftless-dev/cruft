
use crate::register::{make_callable, new_object, register_method};
use rusty_js_runtime::value::ObjectRef;
use rusty_js_runtime::{AgentId, Runtime, RuntimeError, Value};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

const ACCEPTED_SLOT: &str = "__cruft_ws_accepted";
const SERVER_FRAME_LIMITS: rusty_websocket::FrameLimits =
    rusty_websocket::FrameLimits::new(16 * 1024 * 1024, 64 * 1024 * 1024);

#[derive(Clone, Copy)]
enum WsRole {
    Server,
    Client,
}

#[derive(Clone)]
pub(crate) struct WsSessionConfig {
    limits: rusty_websocket::FrameLimits,
    protocol: Option<String>,
}

impl Default for WsSessionConfig {
    fn default() -> Self {
        Self {
            limits: SERVER_FRAME_LIMITS,
            protocol: None,
        }
    }
}

struct ActiveWsSession {
    agent_id: AgentId,
    stream_id: u64,
    realm: usize,
    session_object: ObjectRef,
    handlers: Option<ObjectRef>,
    role: WsRole,
    read_buffer: Vec<u8>,
    limits: rusty_websocket::FrameLimits,
    reassembler: rusty_websocket::MessageReassembler,
    closed: bool,
}

#[derive(Default)]
struct WsPerfStats {
    poll_calls: u64,
    read_ready: u64,
    messages: u64,
    read_ns: u64,
    decode_ns: u64,
    materialize_ns: u64,
    callback_ns: u64,
    encode_ns: u64,
    write_ns: u64,
}

thread_local! {
    static PENDING_ACCEPT: RefCell<Option<(AgentId, ObjectRef, Option<ObjectRef>, WsSessionConfig)>> = const { RefCell::new(None) };
    static WS_SESSIONS: RefCell<Vec<Option<ActiveWsSession>>> = RefCell::new(Vec::new());
    static WS_PERF: RefCell<WsPerfStats> = RefCell::new(WsPerfStats::default());
}

static WS_PERF_ENABLED: OnceLock<bool> = OnceLock::new();
static WS_PERF_EVERY: OnceLock<u64> = OnceLock::new();

fn ws_perf_enabled() -> bool {
    *WS_PERF_ENABLED.get_or_init(|| std::env::var_os("CRUFT_WS_PERF_TRACE").is_some())
}

fn ws_perf_every() -> u64 {
    *WS_PERF_EVERY.get_or_init(|| {
        std::env::var("CRUFT_WS_PERF_TRACE_EVERY")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|v| *v > 0)
            .unwrap_or(100)
    })
}

fn elapsed_ns(start: Instant) -> u64 {
    start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}

fn ws_perf_add(update: impl FnOnce(&mut WsPerfStats)) {
    if !ws_perf_enabled() {
        return;
    }
    WS_PERF.with(|stats| update(&mut stats.borrow_mut()));
}

fn ws_perf_note_message() {
    if !ws_perf_enabled() {
        return;
    }
    let every = ws_perf_every();
    WS_PERF.with(|stats| {
        let mut stats = stats.borrow_mut();
        stats.messages += 1;
        if stats.messages % every == 0 {
            eprintln!(
                "CRUFT_WS_PERF poll_calls={} read_ready={} messages={} read_ns={} decode_ns={} materialize_ns={} callback_ns={} encode_ns={} write_ns={}",
                stats.poll_calls,
                stats.read_ready,
                stats.messages,
                stats.read_ns,
                stats.decode_ns,
                stats.materialize_ns,
                stats.callback_ns,
                stats.encode_ns,
                stats.write_ns
            );
        }
    });
}

fn install_error_class(rt: &mut Runtime, ns: rusty_js_runtime::value::ObjectRef, name: &str) {
    let ctor = make_callable(rt, name, |rt, _args| Ok(rt.current_this()));
    let proto = new_object(rt);
    rt.object_set(proto, "constructor".into(), Value::Object(ctor));
    rt.object_set(ctor, "prototype".into(), Value::Object(proto));
    rt.object_set(ns, name.into(), Value::Object(ctor));
}

fn value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.as_str().to_string()),
        Value::Number(n) => {
            Some(rusty_js_runtime::abstract_ops::to_string(&Value::Number(*n)).to_string())
        }
        Value::Boolean(b) => Some(b.to_string()),
        Value::Null | Value::Undefined => None,
        other => Some(
            rusty_js_runtime::abstract_ops::to_string(other)
                .as_str()
                .to_string(),
        ),
    }
}

fn header_get(
    rt: &mut Runtime,
    headers: &Value,
    name: &str,
) -> Result<Option<String>, RuntimeError> {
    let Value::Object(headers_id) = headers else {
        return Ok(None);
    };
    let get = rt.object_get(*headers_id, "get");
    if rt.is_callable(&get) {
        let arg = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            name.to_string(),
        )));
        let result = rt.call_function(get, Value::Object(*headers_id), vec![arg])?;
        return Ok(value_to_string(&result));
    }
    let lower = name.to_ascii_lowercase();
    let direct = rt.object_get(*headers_id, &lower);
    if !matches!(direct, Value::Undefined) {
        return Ok(value_to_string(&direct));
    }
    let direct = rt.object_get(*headers_id, name);
    if !matches!(direct, Value::Undefined) {
        return Ok(value_to_string(&direct));
    }
    Ok(None)
}

pub(crate) fn is_upgrade_request(rt: &mut Runtime, request: &Value) -> Result<bool, RuntimeError> {
    let Value::Object(request_id) = request else {
        return Ok(false);
    };
    let method = rt.object_get(*request_id, "method");
    if !value_to_string(&method)
        .map(|m| m.eq_ignore_ascii_case("GET"))
        .unwrap_or(false)
    {
        return Ok(false);
    }
    let headers = rt.object_get(*request_id, "headers");
    let Value::Object(headers_id) = headers else {
        return Ok(false);
    };
    if !rt.is_callable(&rt.object_get(headers_id, "get")) {
        return Ok(false);
    }
    let connection = header_get(rt, &headers, "connection")?;
    let has_upgrade_token = connection
        .as_deref()
        .map(|value| {
            value
                .split(',')
                .any(|part| part.trim().eq_ignore_ascii_case("upgrade"))
        })
        .unwrap_or(false);
    let upgrade = header_get(rt, &headers, "upgrade")?;
    let has_websocket_upgrade = upgrade
        .as_deref()
        .map(|value| value.trim().eq_ignore_ascii_case("websocket"))
        .unwrap_or(false);
    let key = header_get(rt, &headers, "sec-websocket-key")?;
    let has_key = key
        .as_deref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let version = header_get(rt, &headers, "sec-websocket-version")?;
    let has_version_13 = version
        .as_deref()
        .map(|value| value.trim() == "13")
        .unwrap_or(false);
    Ok(has_upgrade_token && has_websocket_upgrade && has_key && has_version_13)
}

pub(crate) fn accept_websocket(
    rt: &mut Runtime,
    request: &Value,
    handlers_arg: Option<&Value>,
) -> Result<Value, RuntimeError> {
    if !is_upgrade_request(rt, request)? {
        return Ok(Value::Null);
    }
    let Value::Object(request_id) = request else {
        return Ok(Value::Null);
    };
    let headers = rt.object_get(*request_id, "headers");
    let key = header_get(rt, &headers, "sec-websocket-key")?.ok_or_else(|| {
        RuntimeError::TypeError("cruft:ws.acceptWebSocket: missing Sec-WebSocket-Key".into())
    })?;
    let version =
        header_get(rt, &headers, "sec-websocket-version")?.unwrap_or_else(|| "13".to_string());
    let origin = header_get(rt, &headers, "origin")?;
    rusty_websocket::validate_server_handshake_request(
        &key,
        &version,
        origin.as_deref(),
        rusty_websocket::HandshakePolicy::allow_any(),
    )
    .map_err(|e| RuntimeError::TypeError(format!("cruft:ws.acceptWebSocket: {e}")))?;

    let handlers = match handlers_arg {
        Some(Value::Object(id)) => Some(*id),
        _ => match rt.object_get(*request_id, "__cruft_ws_handlers") {
            Value::Object(id) => Some(id),
            _ => None,
        }
        .or_else(|| match rt.object_get(*request_id, "handlers") {
            Value::Object(id) => Some(id),
            _ => None,
        }),
    };
    let offered_protocols =
        client_offered_protocols(header_get(rt, &headers, "sec-websocket-protocol")?);
    let selected_protocol =
        select_protocol(&protocols_from_handlers(rt, handlers), &offered_protocols);
    let extensions = header_get(rt, &headers, "sec-websocket-extensions")?;
    if extensions
        .as_deref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
    {

    }

    let response = new_object(rt);
    rt.object_set(response, "status".into(), Value::Number(101.0));
    rt.object_set(response, ACCEPTED_SLOT.into(), Value::Boolean(true));
    let response_headers = new_object(rt);
    for (name, value) in [
        ("upgrade", "websocket".to_string()),
        ("connection", "Upgrade".to_string()),
        (
            "sec-websocket-accept",
            rusty_websocket::derive_accept(key.trim()),
        ),
    ] {
        rt.object_set(
            response_headers,
            name.into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(value))),
        );
    }
    if let Some(protocol) = selected_protocol.clone() {
        rt.object_set(
            response_headers,
            "sec-websocket-protocol".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(protocol))),
        );
    }
    rt.object_set(response, "headers".into(), Value::Object(response_headers));
    rt.object_set(
        response,
        "body".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            String::new(),
        ))),
    );
    let mut config = config_from_handlers(rt, handlers);
    config.protocol = selected_protocol;
    PENDING_ACCEPT.with(|pending| {
        *pending.borrow_mut() = Some((rt.agent_id(), *request_id, handlers, config));
    });
    Ok(Value::Object(response))
}

pub(crate) fn take_pending_accept_for_request(
    rt: &Runtime,
    request: ObjectRef,
) -> Option<(ObjectRef, Option<ObjectRef>, WsSessionConfig)> {
    let agent_id = rt.agent_id();
    PENDING_ACCEPT.with(|pending| {
        let mut slot = pending.borrow_mut();
        match slot.take() {
            Some((owner, req, handlers, config)) if owner == agent_id && req == request => {
                Some((req, handlers, config))
            }
            other => {
                *slot = other;
                None
            }
        }
    })
}

fn ws_emit_handler(
    rt: &mut Runtime,
    handlers: Option<ObjectRef>,
    name: &str,
    args: Vec<Value>,
) -> Result<(), RuntimeError> {
    let Some(handlers) = handlers else {
        return Ok(());
    };
    let cb = rt.object_get(handlers, name);
    if rt.is_callable(&cb) {
        let _ = rt.call_function(cb, Value::Object(handlers), args)?;
    }
    Ok(())
}

fn uint8_payload_arg(rt: &Runtime, name: &str, args: &[Value]) -> Result<Vec<u8>, RuntimeError> {
    match args.first() {
        Some(Value::Object(id)) => rt.typed_array_view_bytes(*id).ok_or_else(|| {
            RuntimeError::TypeError(format!("cruft:ws.{name}: expected Uint8Array"))
        }),
        None | Some(Value::Undefined) => Ok(Vec::new()),
        _ => Err(RuntimeError::TypeError(format!(
            "cruft:ws.{name}: expected Uint8Array"
        ))),
    }
}

fn make_bytes_message(rt: &mut Runtime, kind: &str, payload: &[u8]) -> ObjectRef {
    let bytes = rt.alloc_uint8_array_from_bytes(payload);
    let msg = new_object(rt);
    rt.object_set(
        msg,
        "kind".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(kind))),
    );
    rt.object_set(msg, "bytes".into(), Value::Object(bytes));
    msg
}

fn usize_prop_or_default(rt: &Runtime, object: ObjectRef, name: &str, default: usize) -> usize {
    match rt.object_get(object, name) {
        Value::Number(n) if n.is_finite() && n > 0.0 => n.floor() as usize,
        _ => default,
    }
}

fn config_from_handlers(rt: &Runtime, handlers: Option<ObjectRef>) -> WsSessionConfig {
    let mut config = WsSessionConfig::default();
    if let Some(handlers) = handlers {
        config.limits = rusty_websocket::FrameLimits::new(
            usize_prop_or_default(
                rt,
                handlers,
                "maxFramePayload",
                SERVER_FRAME_LIMITS.max_frame_payload,
            ),
            usize_prop_or_default(
                rt,
                handlers,
                "maxMessagePayload",
                SERVER_FRAME_LIMITS.max_message_payload,
            ),
        );
    }
    config
}

fn protocol_token_valid(s: &str) -> bool {
    !s.is_empty()
        && s.bytes().all(|b| {
            matches!(
                b,
                b'!' | b'#'..=b'\'' | b'*' | b'+' | b'-' | b'.' | b'0'..=b'9'
                    | b'A'..=b'Z' | b'^' | b'_' | b'`' | b'a'..=b'z' | b'|' | b'~'
            )
        })
}

fn push_protocol_token(out: &mut Vec<String>, token: &str) {
    let token = token.trim();
    if protocol_token_valid(token) && !out.iter().any(|p| p == token) {
        out.push(token.to_string());
    }
}

fn protocols_from_value(rt: &mut Runtime, value: &Value) -> Vec<String> {
    let mut out = Vec::new();
    match value {
        Value::String(s) => {
            for part in s.as_str().split(',') {
                push_protocol_token(&mut out, part);
            }
        }
        Value::Object(id) => {
            let len = rt.array_length(*id).min(64);
            for i in 0..len {
                if let Some(s) = value_to_string(&rt.object_get(*id, &i.to_string())) {
                    push_protocol_token(&mut out, &s);
                }
            }
        }
        _ => {}
    }
    out
}

fn protocols_from_handlers(rt: &mut Runtime, handlers: Option<ObjectRef>) -> Vec<String> {
    let Some(handlers) = handlers else {
        return Vec::new();
    };
    protocols_from_value(rt, &rt.object_get(handlers, "protocols"))
}

fn client_offered_protocols(header: Option<String>) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(header) = header {
        for part in header.split(',') {
            push_protocol_token(&mut out, part);
        }
    }
    out
}

fn select_protocol(allowed: &[String], offered: &[String]) -> Option<String> {
    allowed
        .iter()
        .find(|candidate| offered.iter().any(|offered| offered == *candidate))
        .cloned()
}

fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|idx| idx + 4)
}

fn client_mask() -> Result<[u8; 4], RuntimeError> {
    let mut mask = [0u8; 4];
    rusty_web_crypto::get_random_values(&mut mask)
        .map_err(|e| RuntimeError::TypeError(format!("cruft:ws: mask generation failed: {e}")))?;
    Ok(mask)
}

fn encode_outbound_frame(
    role: WsRole,
    opcode: rusty_websocket::Opcode,
    payload: Vec<u8>,
) -> Result<Vec<u8>, RuntimeError> {
    let frame = rusty_websocket::Frame {
        fin: true,
        opcode,
        payload,
        mask: match role {
            WsRole::Server => None,
            WsRole::Client => Some(client_mask()?),
        },
    };
    match role {
        WsRole::Server => rusty_websocket::encode_server_frame(&frame),
        WsRole::Client => rusty_websocket::encode_client_frame(&frame),
    }
    .map_err(|e| RuntimeError::TypeError(format!("cruft:ws: {e}")))
}

fn close_policy_for_ws_error(err: &rusty_websocket::WsError) -> (u16, &'static str) {
    match err {
        rusty_websocket::WsError::PayloadTooLong | rusty_websocket::WsError::MessageTooLong => {
            (1009, "message too big")
        }
        rusty_websocket::WsError::InvalidTextUtf8 => (1007, "invalid text payload"),
        _ => (1002, "protocol error"),
    }
}

fn enforce_outbound_message_cap(
    name: &str,
    payload_len: usize,
    limits: rusty_websocket::FrameLimits,
) -> Result<(), RuntimeError> {
    if payload_len > limits.max_message_payload {
        return Err(RuntimeError::TypeError(format!(
            "cruft:ws.{name}: message too big"
        )));
    }
    Ok(())
}

fn make_session_object(
    rt: &mut Runtime,
    stream_id: u64,
    role: WsRole,
    config: &WsSessionConfig,
) -> ObjectRef {
    let session = new_object(rt);
    rt.object_set(session, "readyState".into(), Value::Number(1.0));
    let limits = config.limits;
    rt.object_set(
        session,
        "protocol".into(),
        config
            .protocol
            .clone()
            .map(|protocol| {
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(protocol)))
            })
            .unwrap_or(Value::String(Rc::new(
                rusty_js_runtime::value::JsString::from(String::new()),
            ))),
    );
    register_method(rt, session, "sendText", move |rt, args| {
        if is_session_closed(rt, stream_id) {
            return Err(RuntimeError::TypeError(
                "cruft:ws.sendText: WebSocket is closed".into(),
            ));
        }
        let text = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            Some(v) => rusty_js_runtime::abstract_ops::to_string(v)
                .as_str()
                .to_string(),
            None => String::new(),
        };
        enforce_outbound_message_cap("sendText", text.len(), limits)?;
        let encode_start = Instant::now();
        let bytes = encode_outbound_frame(role, rusty_websocket::Opcode::Text, text.into_bytes())
            .map_err(|e| RuntimeError::TypeError(format!("cruft:ws.sendText: {e:?}")))?;
        ws_perf_add(|stats| stats.encode_ns += elapsed_ns(encode_start));
        let write_start = Instant::now();
        rusty_sockets::stream_write_all(stream_id, &bytes)
            .map_err(|e| RuntimeError::TypeError(format!("cruft:ws.sendText: {e:?}")))?;
        ws_perf_add(|stats| stats.write_ns += elapsed_ns(write_start));
        Ok(Value::Undefined)
    });
    register_method(rt, session, "sendBytes", move |rt, args| {
        if is_session_closed(rt, stream_id) {
            return Err(RuntimeError::TypeError(
                "cruft:ws.sendBytes: WebSocket is closed".into(),
            ));
        }
        let bytes = uint8_payload_arg(rt, "sendBytes", args)?;
        enforce_outbound_message_cap("sendBytes", bytes.len(), limits)?;
        let encode_start = Instant::now();
        let encoded = encode_outbound_frame(role, rusty_websocket::Opcode::Binary, bytes)
            .map_err(|e| RuntimeError::TypeError(format!("cruft:ws.sendBytes: {e:?}")))?;
        ws_perf_add(|stats| stats.encode_ns += elapsed_ns(encode_start));
        let write_start = Instant::now();
        rusty_sockets::stream_write_all(stream_id, &encoded)
            .map_err(|e| RuntimeError::TypeError(format!("cruft:ws.sendBytes: {e:?}")))?;
        ws_perf_add(|stats| stats.write_ns += elapsed_ns(write_start));
        Ok(Value::Undefined)
    });
    register_method(rt, session, "sendPing", move |rt, args| {
        if is_session_closed(rt, stream_id) {
            return Err(RuntimeError::TypeError(
                "cruft:ws.sendPing: WebSocket is closed".into(),
            ));
        }
        let payload = uint8_payload_arg(rt, "sendPing", args)?;
        let encoded = encode_outbound_frame(role, rusty_websocket::Opcode::Ping, payload)
            .map_err(|e| RuntimeError::TypeError(format!("cruft:ws.sendPing: {e:?}")))?;
        rusty_sockets::stream_write_all(stream_id, &encoded)
            .map_err(|e| RuntimeError::TypeError(format!("cruft:ws.sendPing: {e:?}")))?;
        Ok(Value::Undefined)
    });
    register_method(rt, session, "sendPong", move |rt, args| {
        if is_session_closed(rt, stream_id) {
            return Err(RuntimeError::TypeError(
                "cruft:ws.sendPong: WebSocket is closed".into(),
            ));
        }
        let payload = uint8_payload_arg(rt, "sendPong", args)?;
        let encoded = encode_outbound_frame(role, rusty_websocket::Opcode::Pong, payload)
            .map_err(|e| RuntimeError::TypeError(format!("cruft:ws.sendPong: {e:?}")))?;
        rusty_sockets::stream_write_all(stream_id, &encoded)
            .map_err(|e| RuntimeError::TypeError(format!("cruft:ws.sendPong: {e:?}")))?;
        Ok(Value::Undefined)
    });
    register_method(rt, session, "close", move |rt, _args| {
        if is_session_closed(rt, stream_id) {
            return Ok(Value::Undefined);
        }
        if let Ok(bytes) = encode_outbound_frame(role, rusty_websocket::Opcode::Close, Vec::new()) {
            let _ = rusty_sockets::stream_write_all(stream_id, &bytes);
        }
        let _ = rusty_sockets::handle_close(stream_id);
        if let Value::Object(this) = rt.current_this() {
            rt.object_set(this, "readyState".into(), Value::Number(3.0));
        }
        mark_session_closed(rt, stream_id);
        Ok(Value::Undefined)
    });
    session
}

fn is_session_closed(rt: &Runtime, stream_id: u64) -> bool {
    let agent_id = rt.agent_id();
    WS_SESSIONS.with(|sessions| {
        sessions
            .borrow()
            .iter()
            .flatten()
            .find(|session| session.agent_id == agent_id && session.stream_id == stream_id)
            .map(|session| session.closed)
            .unwrap_or(true)
    })
}

fn mark_session_closed(rt: &Runtime, stream_id: u64) {
    let agent_id = rt.agent_id();
    WS_SESSIONS.with(|sessions| {
        for entry in sessions.borrow_mut().iter_mut().flatten() {
            if entry.agent_id == agent_id && entry.stream_id == stream_id {
                entry.closed = true;
            }
        }
    });
}

pub(crate) fn register_server_session(
    rt: &mut Runtime,
    stream_id: u64,
    realm: usize,
    handlers: Option<ObjectRef>,
    config: WsSessionConfig,
) -> Result<(), RuntimeError> {
    let session_object = make_session_object(rt, stream_id, WsRole::Server, &config);
    if let Some(handlers) = handlers {
        rt.object_set(session_object, "handlers".into(), Value::Object(handlers));
    }
    WS_SESSIONS.with(|sessions| {
        sessions.borrow_mut().push(Some(ActiveWsSession {
            agent_id: rt.agent_id(),
            stream_id,
            realm,
            session_object,
            handlers,
            role: WsRole::Server,
            read_buffer: Vec::new(),
            limits: config.limits,
            reassembler: rusty_websocket::MessageReassembler::new(config.limits),
            closed: false,
        }));
    });
    let prior = rt.enter_realm(realm);
    ws_emit_handler(rt, handlers, "open", vec![Value::Object(session_object)])?;
    rt.exit_realm(prior);
    Ok(())
}

fn connect_websocket(
    rt: &mut Runtime,
    url: &Value,
    handlers_arg: Option<&Value>,
) -> Result<Value, RuntimeError> {
    let Some(url) = value_to_string(url) else {
        return Err(RuntimeError::TypeError(
            "cruft:ws.connectWebSocket: expected ws:// URL".into(),
        ));
    };
    let (scheme, host, port, target) = crate::http_client::parse_url(&url)
        .ok_or_else(|| RuntimeError::TypeError("cruft:ws.connectWebSocket: invalid URL".into()))?;
    if scheme != "ws" {
        return Err(RuntimeError::TypeError(
            "cruft:ws.connectWebSocket: only ws:// is implemented; wss:// is routed to P-WEBSOCKET-CLIENT-CONNECT-ADMISSION".into(),
        ));
    }
    let key = rusty_websocket::generate_key()
        .map_err(|e| RuntimeError::TypeError(format!("cruft:ws.connectWebSocket: {e}")))?;
    let requested_protocols = match handlers_arg {
        Some(Value::Object(id)) => protocols_from_handlers(rt, Some(*id)),
        Some(value) => protocols_from_value(rt, value),
        None => Vec::new(),
    };
    let mut headers = vec![
        ("Host".into(), format!("{host}:{port}")),
        ("Connection".into(), "Upgrade".into()),
        ("Upgrade".into(), "websocket".into()),
        ("Sec-WebSocket-Key".into(), key.clone()),
        ("Sec-WebSocket-Version".into(), "13".into()),
    ];
    if !requested_protocols.is_empty() {
        headers.push((
            "Sec-WebSocket-Protocol".into(),
            requested_protocols.join(", "),
        ));
    }
    let request = rusty_http_codec::try_serialize_request("GET", &target, &headers, &[])
        .map_err(|e| RuntimeError::TypeError(format!("cruft:ws.connectWebSocket: {e}")))?;
    let stream_id = rusty_sockets::stream_connect(&format!("{host}:{port}"))
        .map_err(|e| RuntimeError::TypeError(format!("cruft:ws.connectWebSocket: {e}")))?;
    rusty_sockets::stream_write_all(stream_id, &request)
        .map_err(|e| RuntimeError::TypeError(format!("cruft:ws.connectWebSocket: {e}")))?;
    let _ = rusty_sockets::stream_set_nonblocking(stream_id, true);
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut response = Vec::new();
    while Instant::now() < deadline {
        match rusty_sockets::stream_try_read(stream_id, 65536) {
            Ok(Some(bytes)) if bytes.is_empty() => break,
            Ok(Some(bytes)) => {
                response.extend_from_slice(&bytes);
                if header_end(&response).is_some() {
                    break;
                }
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(1)),
            Err(e) => {
                let _ = rusty_sockets::handle_close(stream_id);
                return Err(RuntimeError::TypeError(format!(
                    "cruft:ws.connectWebSocket: {e}"
                )));
            }
        }
    }
    let head_end = header_end(&response).ok_or_else(|| {
        let _ = rusty_sockets::handle_close(stream_id);
        RuntimeError::TypeError("cruft:ws.connectWebSocket: bad handshake: truncated".into())
    })?;
    let parsed = rusty_http_codec::parse_response(&response[..head_end]).map_err(|e| {
        let _ = rusty_sockets::handle_close(stream_id);
        RuntimeError::TypeError(format!("cruft:ws.connectWebSocket: bad handshake: {e:?}"))
    })?;
    let initial_read_buffer = response[head_end..].to_vec();
    let accept = header_value(&parsed.headers, "sec-websocket-accept").unwrap_or("");
    let selected_protocol = header_value(&parsed.headers, "sec-websocket-protocol")
        .map(str::trim)
        .filter(|protocol| !protocol.is_empty())
        .map(str::to_string);
    if selected_protocol
        .as_ref()
        .map(|protocol| !requested_protocols.iter().any(|p| p == protocol))
        .unwrap_or(false)
    {
        let _ = rusty_sockets::handle_close(stream_id);
        return Err(RuntimeError::TypeError(
            "cruft:ws.connectWebSocket: server selected unrequested protocol".into(),
        ));
    }
    if parsed.status != 101
        || !header_value(&parsed.headers, "upgrade")
            .map(|v| v.eq_ignore_ascii_case("websocket"))
            .unwrap_or(false)
        || !rusty_websocket::verify_accept(&key, accept.trim())
    {
        let _ = rusty_sockets::handle_close(stream_id);
        return Err(RuntimeError::TypeError(
            "cruft:ws.connectWebSocket: server rejected WebSocket upgrade".into(),
        ));
    }
    let handlers = match handlers_arg {
        Some(Value::Object(id)) => Some(*id),
        _ => None,
    };
    let mut config = config_from_handlers(rt, handlers);
    config.protocol = selected_protocol;
    let session_object = make_session_object(rt, stream_id, WsRole::Client, &config);
    if let Some(handlers) = handlers {
        rt.object_set(session_object, "handlers".into(), Value::Object(handlers));
    }
    WS_SESSIONS.with(|sessions| {
        sessions.borrow_mut().push(Some(ActiveWsSession {
            agent_id: rt.agent_id(),
            stream_id,
            realm: rt.current_realm,
            session_object,
            handlers,
            role: WsRole::Client,
            read_buffer: initial_read_buffer,
            limits: config.limits,
            reassembler: rusty_websocket::MessageReassembler::new(config.limits),
            closed: false,
        }));
    });
    ws_emit_handler(rt, handlers, "open", vec![Value::Object(session_object)])?;
    Ok(Value::Object(session_object))
}

pub fn collect_roots(roots: &mut Vec<ObjectRef>) {
    let agent_id = AgentId::DEFAULT;
    collect_roots_for_agent(agent_id, roots);
}

pub fn collect_roots_for_runtime(rt: &Runtime, roots: &mut Vec<ObjectRef>) {
    collect_roots_for_agent(rt.agent_id(), roots);
}

fn collect_roots_for_agent(agent_id: AgentId, roots: &mut Vec<ObjectRef>) {
    WS_SESSIONS.with(|sessions| {
        for session in sessions.borrow().iter().flatten() {
            if session.agent_id != agent_id {
                continue;
            }
            roots.push(session.session_object);
            if let Some(handlers) = session.handlers {
                roots.push(handlers);
            }
        }
    });
}

pub fn has_active_sessions() -> bool {
    has_active_sessions_for_agent(AgentId::DEFAULT)
}

pub fn has_active_sessions_for_runtime(rt: &Runtime) -> bool {
    has_active_sessions_for_agent(rt.agent_id())
}

fn has_active_sessions_for_agent(agent_id: AgentId) -> bool {
    WS_SESSIONS.with(|sessions| {
        sessions
            .borrow()
            .iter()
            .flatten()
            .any(|session| session.agent_id == agent_id && !session.closed)
    })
}

pub fn poll_io(rt: &mut Runtime) -> Result<bool, RuntimeError> {
    ws_perf_add(|stats| stats.poll_calls += 1);
    let agent_id = rt.agent_id();
    let sessions: Vec<(
        usize,
        u64,
        usize,
        ObjectRef,
        Option<ObjectRef>,
        WsRole,
        bool,
    )> = WS_SESSIONS.with(|sessions| {
        sessions
            .borrow()
            .iter()
            .enumerate()
            .filter_map(|(idx, entry)| {
                entry.as_ref().and_then(|session| {
                    (session.agent_id == agent_id && !session.closed).then_some((
                        idx,
                        session.stream_id,
                        session.realm,
                        session.session_object,
                        session.handlers,
                        session.role,
                        !session.read_buffer.is_empty(),
                    ))
                })
            })
            .collect()
    });
    for (idx, stream_id, realm, session_object, handlers, role, has_buffer) in sessions {
        let _ = rusty_sockets::stream_set_nonblocking(stream_id, true);
        let read_start = Instant::now();
        let mut read = rusty_sockets::stream_try_read(stream_id, 65536);
        ws_perf_add(|stats| stats.read_ns += elapsed_ns(read_start));
        let _ = rusty_sockets::stream_set_nonblocking(stream_id, false);
        if has_buffer && matches!(read, Ok(None)) {
            read = Ok(Some(Vec::new()));
        }
        match read {
            Ok(Some(bytes)) if !bytes.is_empty() || has_buffer => {
                ws_perf_add(|stats| stats.read_ready += 1);
                let decode_start = Instant::now();
                let (messages, protocol_error): (
                    Vec<rusty_websocket::Message>,
                    Option<(u16, &'static str, String)>,
                ) = WS_SESSIONS.with(|sessions| {
                    let mut sessions = sessions.borrow_mut();
                    let Some(Some(session)) = sessions.get_mut(idx) else {
                        return Ok::<
                            (
                                Vec<rusty_websocket::Message>,
                                Option<(u16, &'static str, String)>,
                            ),
                            RuntimeError,
                        >((Vec::new(), None));
                    };
                    if !bytes.is_empty() {
                        session.read_buffer.extend_from_slice(&bytes);
                    }
                    let mut cursor = session.read_buffer.as_slice();
                    let mut consumed = 0usize;
                    let mut out = Vec::new();
                    while !cursor.is_empty() {
                        let decoded = match session.role {
                            WsRole::Server => rusty_websocket::decode_server_frame_with_limits(
                                cursor,
                                session.limits,
                            ),
                            WsRole::Client => rusty_websocket::decode_client_frame_with_limits(
                                cursor,
                                session.limits,
                            ),
                        };
                        let (frame, used) = match decoded {
                            Ok(decoded) => decoded,
                            Err(rusty_websocket::WsError::UnexpectedEnd) => break,
                            Err(e) => {
                                session.read_buffer.clear();
                                session.closed = true;
                                let (code, reason) = close_policy_for_ws_error(&e);
                                return Ok((Vec::new(), Some((code, reason, e.to_string()))));
                            }
                        };
                        cursor = &cursor[used..];
                        consumed += used;
                        match session.reassembler.push_frame(frame) {
                            Ok(Some(message)) => out.push(message),
                            Ok(None) => {}
                            Err(e) => {
                                session.read_buffer.clear();
                                session.closed = true;
                                let (code, reason) = close_policy_for_ws_error(&e);
                                return Ok((Vec::new(), Some((code, reason, e.to_string()))));
                            }
                        }
                    }
                    if consumed > 0 {
                        session.read_buffer.drain(..consumed);
                    }
                    Ok((out, None))
                })?;
                ws_perf_add(|stats| stats.decode_ns += elapsed_ns(decode_start));
                let prior = rt.enter_realm(realm);
                if let Some((close_code, close_reason, event_reason)) = protocol_error {
                    rt.object_set(session_object, "readyState".into(), Value::Number(3.0));
                    let close_payload =
                        rusty_websocket::encode_close(Some(close_code), close_reason);
                    if let Ok(bytes) =
                        encode_outbound_frame(role, rusty_websocket::Opcode::Close, close_payload)
                    {
                        let _ = rusty_sockets::stream_write_all(stream_id, &bytes);
                    }
                    let _ = rusty_sockets::handle_close(stream_id);
                    mark_session_closed(rt, stream_id);
                    let event = new_object(rt);
                    rt.object_set(event, "code".into(), Value::Number(close_code as f64));
                    rt.object_set(
                        event,
                        "reason".into(),
                        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                            event_reason,
                        ))),
                    );
                    rt.object_set(event, "clean".into(), Value::Boolean(false));
                    ws_emit_handler(
                        rt,
                        handlers,
                        "close",
                        vec![Value::Object(session_object), Value::Object(event)],
                    )?;
                    rt.exit_realm(prior);
                    return Ok(true);
                }
                for message in messages {
                    match message.opcode {
                        rusty_websocket::Opcode::Text => {
                            let materialize_start = Instant::now();
                            let text = String::from_utf8(message.payload).map_err(|_| {
                                RuntimeError::TypeError("cruft:ws: invalid text payload".into())
                            })?;
                            let msg = new_object(rt);
                            rt.object_set(
                                msg,
                                "kind".into(),
                                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                                    "text",
                                ))),
                            );
                            rt.object_set(
                                msg,
                                "text".into(),
                                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                                    text,
                                ))),
                            );
                            ws_perf_add(|stats| {
                                stats.materialize_ns += elapsed_ns(materialize_start)
                            });
                            let callback_start = Instant::now();
                            ws_emit_handler(
                                rt,
                                handlers,
                                "message",
                                vec![Value::Object(session_object), Value::Object(msg)],
                            )?;
                            ws_perf_add(|stats| stats.callback_ns += elapsed_ns(callback_start));
                            ws_perf_note_message();
                        }
                        rusty_websocket::Opcode::Binary => {
                            let materialize_start = Instant::now();
                            let msg = make_bytes_message(rt, "binary", &message.payload);
                            ws_perf_add(|stats| {
                                stats.materialize_ns += elapsed_ns(materialize_start)
                            });
                            let callback_start = Instant::now();
                            ws_emit_handler(
                                rt,
                                handlers,
                                "message",
                                vec![Value::Object(session_object), Value::Object(msg)],
                            )?;
                            ws_perf_add(|stats| stats.callback_ns += elapsed_ns(callback_start));
                            ws_perf_note_message();
                        }
                        rusty_websocket::Opcode::Ping => {
                            if let Ok(bytes) = encode_outbound_frame(
                                role,
                                rusty_websocket::Opcode::Pong,
                                message.payload.clone(),
                            ) {
                                let _ = rusty_sockets::stream_write_all(stream_id, &bytes);
                            }
                            let msg = make_bytes_message(rt, "ping", &message.payload);
                            ws_emit_handler(
                                rt,
                                handlers,
                                "ping",
                                vec![Value::Object(session_object), Value::Object(msg)],
                            )?;
                        }
                        rusty_websocket::Opcode::Pong => {
                            let msg = make_bytes_message(rt, "pong", &message.payload);
                            ws_emit_handler(
                                rt,
                                handlers,
                                "pong",
                                vec![Value::Object(session_object), Value::Object(msg)],
                            )?;
                        }
                        rusty_websocket::Opcode::Close => {
                            rt.object_set(session_object, "readyState".into(), Value::Number(3.0));
                            let close = rusty_websocket::decode_close(&message.payload)
                                .map_err(|e| RuntimeError::TypeError(format!("cruft:ws: {e}")))?;
                            if let Ok(bytes) = encode_outbound_frame(
                                role,
                                rusty_websocket::Opcode::Close,
                                Vec::new(),
                            ) {
                                let _ = rusty_sockets::stream_write_all(stream_id, &bytes);
                            }
                            let _ = rusty_sockets::handle_close(stream_id);
                            mark_session_closed(rt, stream_id);
                            let event = new_object(rt);
                            rt.object_set(
                                event,
                                "code".into(),
                                close
                                    .code
                                    .map(|code| Value::Number(code as f64))
                                    .unwrap_or(Value::Undefined),
                            );
                            rt.object_set(
                                event,
                                "reason".into(),
                                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                                    close.reason,
                                ))),
                            );
                            rt.object_set(event, "clean".into(), Value::Boolean(true));
                            ws_emit_handler(
                                rt,
                                handlers,
                                "close",
                                vec![Value::Object(session_object), Value::Object(event)],
                            )?;
                        }
                        _ => {}
                    }
                }
                rt.exit_realm(prior);
                return Ok(true);
            }
            Ok(Some(_)) => {
                rt.object_set(session_object, "readyState".into(), Value::Number(3.0));
                let _ = rusty_sockets::handle_close(stream_id);
                mark_session_closed(rt, stream_id);
                return Ok(true);
            }
            Ok(None) => {}
            Err(_) => {
                let _ = rusty_sockets::handle_close(stream_id);
                mark_session_closed(rt, stream_id);
                return Ok(true);
            }
        }
    }

    Ok(false)
}

pub fn install_canonical(rt: &mut Runtime) {
    let ns = new_object(rt);

    register_method(rt, ns, "acceptWebSocket", |rt, args| {
        let request = args.first().unwrap_or(&Value::Undefined);
        accept_websocket(rt, request, args.get(1))
    });
    register_method(rt, ns, "connectWebSocket", |rt, args| {
        let url = args.first().unwrap_or(&Value::Undefined);
        connect_websocket(rt, url, args.get(1))
    });
    register_method(rt, ns, "isUpgradeRequest", |rt, args| {
        let request = args.first().unwrap_or(&Value::Undefined);
        Ok(Value::Boolean(is_upgrade_request(rt, request)?))
    });

    install_error_class(rt, ns, "WebSocketProtocolError");
    install_error_class(rt, ns, "WebSocketClosedError");
    install_error_class(rt, ns, "WebSocketResourceError");

    rt.define_global_property("__cruft_ws", Value::Object(ns));
}

#[cfg(test)]
mod tests {
    use super::{
        has_active_sessions_for_runtime, is_session_closed, mark_session_closed,
        take_pending_accept_for_request, ActiveWsSession, WsRole, WsSessionConfig, PENDING_ACCEPT,
        WS_SESSIONS,
    };
    use rusty_js_runtime::{AgentId, Object, Runtime};

    #[test]
    fn pending_accept_registry_is_scoped_by_runtime_agent_id() {
        let mut rt_a = Runtime::new_with_agent_id(AgentId::from_raw(701));
        let mut rt_b = Runtime::new_with_agent_id(AgentId::from_raw(702));
        let request_b = rt_b.alloc_object(Object::new_ordinary());
        let handlers_b = rt_b.alloc_object(Object::new_ordinary());

        PENDING_ACCEPT.with(|pending| {
            *pending.borrow_mut() = Some((
                rt_b.agent_id(),
                request_b,
                Some(handlers_b),
                WsSessionConfig::default(),
            ));
        });

        assert!(
            take_pending_accept_for_request(&rt_a, request_b).is_none(),
            "agent A must not consume agent B's pending websocket accept"
        );
        assert!(
            take_pending_accept_for_request(&rt_b, request_b).is_some(),
            "agent B owns the pending websocket accept"
        );

        PENDING_ACCEPT.with(|pending| *pending.borrow_mut() = None);
    }

    #[test]
    fn websocket_session_registry_is_scoped_by_runtime_agent_id() {
        let rt_a = Runtime::new_with_agent_id(AgentId::from_raw(711));
        let mut rt_b = Runtime::new_with_agent_id(AgentId::from_raw(712));
        let session_b = rt_b.alloc_object(Object::new_ordinary());

        WS_SESSIONS.with(|sessions| {
            sessions.borrow_mut().clear();
            sessions.borrow_mut().push(Some(ActiveWsSession {
                agent_id: rt_b.agent_id(),
                stream_id: 777,
                realm: rt_b.current_realm,
                session_object: session_b,
                handlers: None,
                role: WsRole::Server,
                read_buffer: Vec::new(),
                limits: super::SERVER_FRAME_LIMITS,
                reassembler: rusty_websocket::MessageReassembler::new(super::SERVER_FRAME_LIMITS),
                closed: false,
            }));
        });

        assert!(!has_active_sessions_for_runtime(&rt_a));
        assert!(has_active_sessions_for_runtime(&rt_b));
        assert!(
            is_session_closed(&rt_a, 777),
            "agent A must treat agent B's websocket session as unavailable"
        );
        assert!(!is_session_closed(&rt_b, 777));
        mark_session_closed(&rt_a, 777);
        assert!(
            !is_session_closed(&rt_b, 777),
            "agent A close must not mutate agent B's websocket session"
        );
        mark_session_closed(&rt_b, 777);
        assert!(is_session_closed(&rt_b, 777));

        WS_SESSIONS.with(|sessions| sessions.borrow_mut().clear());
    }
}
