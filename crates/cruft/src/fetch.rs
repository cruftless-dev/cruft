
use crate::register::{make_callable, make_callable_with_length, register_method};
use rusty_js_runtime::caps::{self, ModuleId, ModuleProvenance};
use rusty_js_runtime::value::ObjectRef;
use rusty_js_runtime::{AgentId, Object, Runtime, RuntimeError, Value};
use std::net::{IpAddr, ToSocketAddrs};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const FETCH_TIMEOUT_MS: u64 = 30_000;
const FETCH_MAX_RESPONSE_BODY_BYTES: usize = 64 * 1024 * 1024;
const FETCH_INTERNAL_POLICY_HINT: &str = concat!(
    "by Cruft's default SSRF/internal-network policy; ",
    "pass --allow-net-loopback or set CRUFT_ALLOW_NET_LOOPBACK=1 ",
    "for local development loopback fetches"
);

fn notify_agent_wake(wake: &Arc<(std::sync::Mutex<u64>, std::sync::Condvar)>) {
    let (lock, cv) = &**wake;
    if let Ok(mut generation) = lock.lock() {
        *generation = generation.wrapping_add(1);
        cv.notify_all();
    }
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

pub fn install(rt: &mut Runtime) {

    let f = make_callable_with_length(rt, "fetch", 1, do_fetch);
    rt.define_global_property("fetch", Value::Object(f));
}

fn type_error(rt: &mut Runtime, message: &str) -> Value {
    let ctor = rt.global_get("TypeError");
    match rt.construct(ctor, vec![js_string(message)]) {
        Ok(v @ Value::Object(_)) => v,
        _ => js_string(message),
    }
}

fn network_error(rt: &mut Runtime, detail: String) -> Value {
    let ctor = rt.global_get("TypeError");
    match rt.construct(ctor, vec![js_string("fetch failed")]) {
        Ok(Value::Object(e)) => {
            let ector = rt.global_get("Error");
            let cause = match rt.construct(ector, vec![js_string(detail.clone())]) {
                Ok(v @ Value::Object(_)) => v,
                _ => js_string(detail),
            };
            rt.object_set(e, "cause".into(), cause);
            Value::Object(e)
        }
        _ => js_string("fetch failed"),
    }
}

fn reject(rt: &mut Runtime, p: ObjectRef, msg: String) -> Result<Value, RuntimeError> {
    let err = type_error(rt, &msg);
    rusty_js_runtime::promise::reject_promise(rt, p, err);
    Ok(Value::Object(p))
}

fn reject_network(rt: &mut Runtime, p: ObjectRef, detail: String) -> Result<Value, RuntimeError> {
    let err = network_error(rt, detail);
    rusty_js_runtime::promise::reject_promise(rt, p, err);
    Ok(Value::Object(p))
}

fn js_string(s: impl Into<String>) -> Value {
    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(s.into())))
}

fn abort_error(rt: &mut Runtime) -> Value {
    let ctor = rt.global_get("Error");
    let msg = js_string("This operation was aborted");
    match rt.construct(ctor, vec![msg.clone()]) {
        Ok(Value::Object(e)) => {
            rt.object_set(e, "name".into(), js_string("AbortError"));
            rt.object_set(e, "code".into(), Value::Number(20.0));
            Value::Object(e)
        }
        _ => msg,
    }
}

fn reject_abort(rt: &mut Runtime, p: ObjectRef) -> Result<Value, RuntimeError> {
    let err = abort_error(rt);
    rusty_js_runtime::promise::reject_promise(rt, p, err);
    Ok(Value::Object(p))
}

fn do_fetch(rt: &mut Runtime, args: &[Value]) -> Result<Value, RuntimeError> {
    let p = rusty_js_runtime::promise::new_promise(rt);

    let url = match args.first() {
        Some(Value::String(s)) => s.to_string(),
        Some(Value::Object(id)) => match rt.object_get(*id, "url") {
            Value::String(s) => s.to_string(),
            _ => match rt.object_get(*id, "href") {
                Value::String(s) => s.to_string(),
                _ => String::new(),
            },
        },
        _ => String::new(),
    };
    if url.is_empty() {
        return reject(rt, p, "fetch: a URL is required".into());
    }

    let mut method = "GET".to_string();
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut body: Vec<u8> = Vec::new();
    let mut redirect = FetchRedirect::Follow;
    let mut signal: Option<ObjectRef> = None;
    if let Some(Value::Object(init)) = args.get(1) {
        if let Value::Object(sig) = rt.object_get(*init, "signal") {
            if matches!(rt.object_get(sig, "aborted"), Value::Boolean(true)) {
                return reject_abort(rt, p);
            }
            signal = Some(sig);
        }
        if let Value::String(m) = rt.object_get(*init, "method") {
            method = m.to_uppercase();
        }
        if let Value::Object(h) = rt.object_get(*init, "headers") {
            for k in rt.ordinary_own_enumerable_string_keys(h) {
                if let Value::String(v) = rt.object_get(h, &k) {
                    headers.push((k, v.to_string()));
                }
            }
        }
        if let Value::String(b) = rt.object_get(*init, "body") {
            body = b.as_bytes().to_vec();
        }
        if let Value::String(r) = rt.object_get(*init, "redirect") {
            redirect = match r.as_str() {
                "follow" => FetchRedirect::Follow,
                "manual" => FetchRedirect::Manual,
                "error" => FetchRedirect::Error,
                other => {
                    return reject(rt, p, format!("fetch: unsupported redirect mode '{other}'"))
                }
            };
        }
    }
    if let Err(e) = validate_fetch_request_headers(&headers) {
        return reject(rt, p, e);
    }

    if let Some(data) = parse_data_url(&url) {
        return match data {
            Ok(data) => resolve_data_url_fetch(rt, p, &url, data),
            Err(e) => reject(rt, p, e),
        };
    }

    let (scheme, host, port, target) = match crate::http_client::parse_url(&url) {
        Some(t) => t,
        None => return reject(rt, p, format!("fetch: invalid URL '{url}'")),
    };
    if scheme != "http" && scheme != "https" {
        return reject(rt, p, format!("fetch: unsupported scheme '{scheme}:'"));
    }

    let caller = caller_module_id(rt);
    let net_op = caps::NetOp::Connect {
        host: host.clone(),
        port,
    };
    let allow_internal_egress = rt.caps.has_explicit_net_grant(&net_op, &caller);
    if let Err(e) = rt.caps.require_net(
        &caps::Net::none(),
        caps::NetOp::Connect {
            host: host.clone(),
            port,
        },
        &caller,
    ) {
        return reject(rt, p, e.to_string());
    }

    let realm = rt.current_realm;
    let (tx, rx) = std::sync::mpsc::channel();
    let root_key = next_pending_fetch_root_key();
    let cancel = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicBool::new(false));
    let abort_listener = signal.map(|_sig| {
        let cancel = cancel.clone();
        let root_key = root_key.clone();
        make_callable(rt, "__fetch_abort", move |rt, _args| {
            cancel.store(true, Ordering::SeqCst);
            abort_pending_fetch(rt, &root_key)?;
            Ok(Value::Undefined)
        })
    });
    let start = FetchRequestPlan {
        url: url.clone(),
        scheme,
        host,
        port,
        target,
        method,
        headers,
        body,
        allow_internal_egress,
    };
    let worker_cancel = cancel.clone();
    let worker_done = done.clone();
    let wake = rt.agent_wake_handle();
    std::thread::spawn(move || {
        let watchdog_cancel = worker_cancel.clone();
        let watchdog_done = worker_done.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(FETCH_TIMEOUT_MS));
            if !watchdog_done.load(Ordering::SeqCst) {
                watchdog_cancel.store(true, Ordering::SeqCst);
            }
        });
        match fetch_streaming_with_redirects(start, redirect, &worker_cancel) {
            Ok(mut s) => {
                if tx
                    .send(FetchMsg::Head {
                        head: s.response.head.clone(),
                        url: s.url.clone(),
                        redirected: s.redirected,
                    })
                    .is_err()
                {
                    worker_done.store(true, Ordering::SeqCst);
                    return;
                }
                notify_agent_wake(&wake);
                let mut body_bytes = 0usize;
                loop {
                    match s.response.next_chunk_cancelled(Some(&worker_cancel)) {
                        Ok(Some(c)) => {
                            body_bytes = match checked_fetch_body_bytes(body_bytes, c.len()) {
                                Ok(n) => n,
                                Err(e) => {
                                    let _ = tx.send(FetchMsg::Err(format!(
                                        "{e}: limit is {} bytes",
                                        FETCH_MAX_RESPONSE_BODY_BYTES,
                                    )));
                                    notify_agent_wake(&wake);
                                    worker_done.store(true, Ordering::SeqCst);
                                    return;
                                }
                            };
                            if tx.send(FetchMsg::Chunk(c)).is_err() {
                                worker_done.store(true, Ordering::SeqCst);
                                return;
                            }
                            notify_agent_wake(&wake);
                        }
                        Ok(None) => {
                            let _ = tx.send(FetchMsg::End);
                            notify_agent_wake(&wake);
                            worker_done.store(true, Ordering::SeqCst);
                            return;
                        }
                        Err(e) => {
                            let _ = tx.send(FetchMsg::Err(e));
                            notify_agent_wake(&wake);
                            worker_done.store(true, Ordering::SeqCst);
                            return;
                        }
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(FetchMsg::Err(e));
                notify_agent_wake(&wake);
                worker_done.store(true, Ordering::SeqCst);
            }
        }
    });
    rt.retain_host_roots(
        root_key.clone(),
        fetch_roots(p, None, signal, abort_listener),
    );
    if let (Some(sig), Some(listener)) = (signal, abort_listener) {
        let add = rt.object_get(sig, "addEventListener");
        if rt.is_callable(&add) {
            let _ = rt.call_function(
                add,
                Value::Object(sig),
                vec![js_string("abort"), Value::Object(listener)],
            );
        }
    }
    PENDING_FETCHES.with(|v| {
        v.borrow_mut().push(Some(PendingFetch {
            agent_id: rt.agent_id(),
            rx,
            promise: p,
            url,
            realm,
            root_key,
            ctrl: None,
            cancel,
            signal,
            abort_listener,
            done,
        }));
    });
    Ok(Value::Object(p))
}

enum FetchMsg {
    Head {
        head: rusty_http_codec::ResponseHead,
        url: String,
        redirected: bool,
    },
    Chunk(Vec<u8>),
    End,
    Err(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FetchRedirect {
    Follow,
    Manual,
    Error,
}

struct FetchRequestPlan {
    url: String,
    scheme: String,
    host: String,
    port: u16,
    target: String,
    method: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    allow_internal_egress: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FetchOrigin {
    scheme: String,
    host: String,
    port: u16,
}

impl FetchRequestPlan {
    fn origin(&self) -> FetchOrigin {
        FetchOrigin {
            scheme: self.scheme.clone(),
            host: self.host.clone(),
            port: self.port,
        }
    }
}

struct FetchStreamingResult {
    response: crate::http_client::StreamingResponse,
    url: String,
    redirected: bool,
}

struct PendingFetch {
    agent_id: AgentId,
    rx: std::sync::mpsc::Receiver<FetchMsg>,
    promise: ObjectRef,
    url: String,
    realm: usize,
    root_key: String,

    ctrl: Option<ObjectRef>,
    cancel: Arc<AtomicBool>,
    signal: Option<ObjectRef>,
    abort_listener: Option<ObjectRef>,
    done: Arc<AtomicBool>,
}

thread_local! {
    static PENDING_FETCHES: std::cell::RefCell<Vec<Option<PendingFetch>>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static NEXT_FETCH_ID: std::cell::RefCell<u64> = const { std::cell::RefCell::new(1) };
}

fn remove_abort_listener(rt: &mut Runtime, sig: ObjectRef, listener: ObjectRef) {
    let remove = rt.object_get(sig, "removeEventListener");
    if rt.is_callable(&remove) {
        let _ = rt.call_function(
            remove,
            Value::Object(sig),
            vec![js_string("abort"), Value::Object(listener)],
        );
    }
}

fn fetch_roots(
    promise: ObjectRef,
    holder: Option<ObjectRef>,
    signal: Option<ObjectRef>,
    abort_listener: Option<ObjectRef>,
) -> Vec<Value> {
    let mut roots = vec![Value::Object(promise)];
    if let Some(holder) = holder {
        roots.push(Value::Object(holder));
    }
    if let Some(sig) = signal {
        roots.push(Value::Object(sig));
    }
    if let Some(listener) = abort_listener {
        roots.push(Value::Object(listener));
    }
    roots
}

fn abort_pending_fetch(rt: &mut Runtime, root_key: &str) -> Result<(), RuntimeError> {
    let agent_id = rt.agent_id();
    let pending = PENDING_FETCHES.with(|v| {
        let mut vv = v.borrow_mut();
        let idx = vv.iter().position(|slot| {
            slot.as_ref()
                .is_some_and(|p| p.agent_id == agent_id && p.root_key == root_key)
        })?;
        vv[idx].take()
    });
    let Some(pending) = pending else {
        return Ok(());
    };
    pending.cancel.store(true, Ordering::SeqCst);
    pending.done.store(true, Ordering::SeqCst);
    if let (Some(sig), Some(listener)) = (pending.signal, pending.abort_listener) {
        remove_abort_listener(rt, sig, listener);
    }
    let err = abort_error(rt);
    if let Some(holder) = pending.ctrl {
        stream_call(rt, holder, "error", vec![err.clone()]);
    }
    rusty_js_runtime::promise::reject_promise(rt, pending.promise, err);
    rt.release_host_roots(root_key);
    PENDING_FETCHES.with(|v| v.borrow_mut().retain(|x| x.is_some()));
    Ok(())
}

fn next_pending_fetch_root_key() -> String {
    let id = NEXT_FETCH_ID.with(|next| {
        let mut next = next.borrow_mut();
        let id = *next;
        *next = next.wrapping_add(1).max(1);
        id
    });
    format!("fetch:{id}")
}

fn fetch_streaming_with_redirects(
    mut plan: FetchRequestPlan,
    redirect: FetchRedirect,
    cancel: &AtomicBool,
) -> Result<FetchStreamingResult, String> {
    let mut redirected = false;
    for hop in 0..=20 {
        if cancel.load(Ordering::SeqCst) {
            return Err("aborted".into());
        }
        validate_fetch_egress_target(&plan.host, plan.port, plan.allow_internal_egress)?;
        let req = crate::http_client::try_build_request(
            &plan.method,
            &plan.target,
            &plan.host,
            plan.port,
            &plan.headers,
            &plan.body,
        )?;
        let mut response = crate::http_client::round_trip_streaming_cancelled_with_connect_timeout(
            &plan.scheme,
            &plan.host,
            plan.port,
            &req,
            false,
            None,
            Some(cancel),
            Some(FETCH_TIMEOUT_MS),
        )?;
        if !is_redirect_status(response.head.status) {
            return Ok(FetchStreamingResult {
                response,
                url: plan.url,
                redirected,
            });
        }
        match redirect {
            FetchRedirect::Manual => {
                return Ok(FetchStreamingResult {
                    response,
                    url: plan.url,
                    redirected,
                });
            }
            FetchRedirect::Error => {
                return Err(format!("fetch: redirect encountered for {}", plan.url));
            }
            FetchRedirect::Follow => {}
        }
        if hop == 20 {
            return Err(format!("fetch: too many redirects from {}", plan.url));
        }
        let location = header_value(&response.head.headers, "location")
            .ok_or_else(|| format!("fetch: redirect without Location from {}", plan.url))?;
        let next_url = resolve_fetch_location(&plan.url, &location)?;
        while response.next_chunk_cancelled(Some(cancel))?.is_some() {}
        let (scheme, host, port, target) = crate::http_client::parse_url(&next_url)
            .ok_or_else(|| format!("fetch: invalid redirect URL '{next_url}'"))?;
        if scheme != "http" && scheme != "https" {
            return Err(format!("fetch: unsupported redirect scheme '{scheme}:'"));
        }
        let current_origin = plan.origin();
        let next_origin = FetchOrigin {
            scheme: scheme.clone(),
            host: host.clone(),
            port,
        };
        if matches!(response.head.status, 301 | 302 | 303)
            && plan.method != "GET"
            && plan.method != "HEAD"
        {
            plan.method = "GET".to_string();
            plan.body.clear();
            plan.headers
                .retain(|(name, _)| !name.eq_ignore_ascii_case("content-length"));
        }
        strip_redirect_credentials(&mut plan.headers, &current_origin, &next_origin);
        plan = FetchRequestPlan {
            url: next_url,
            scheme,
            host,
            port,
            target,
            method: plan.method,
            headers: plan.headers,
            body: plan.body,
            allow_internal_egress: plan.allow_internal_egress,
        };
        redirected = true;
    }
    Err("fetch: redirect loop exhausted".to_string())
}

fn is_redirect_status(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

fn strip_redirect_credentials(
    headers: &mut Vec<(String, String)>,
    from: &FetchOrigin,
    to: &FetchOrigin,
) {
    if same_fetch_origin(from, to) && !is_secure_to_insecure_redirect(from, to) {
        return;
    }
    headers.retain(|(name, _)| !is_fetch_credential_header(name));
}

fn same_fetch_origin(a: &FetchOrigin, b: &FetchOrigin) -> bool {
    a.scheme == b.scheme && a.host.eq_ignore_ascii_case(&b.host) && a.port == b.port
}

fn is_secure_to_insecure_redirect(from: &FetchOrigin, to: &FetchOrigin) -> bool {
    from.scheme == "https" && to.scheme == "http"
}

fn is_fetch_credential_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "authorization" | "cookie" | "proxy-authorization"
    )
}

fn validate_fetch_request_headers(headers: &[(String, String)]) -> Result<(), String> {
    for (name, value) in headers {
        validate_fetch_header_name(name)?;
        validate_fetch_header_value(value)?;
        if is_forbidden_fetch_request_header(name) {
            return Err(format!("fetch: forbidden request header '{name}'"));
        }
    }
    Ok(())
}

fn validate_fetch_header_name(name: &str) -> Result<(), String> {
    if name.is_empty() || !name.bytes().all(is_http_token_byte) {
        return Err(format!("fetch: invalid request header name '{name}'"));
    }
    Ok(())
}

fn validate_fetch_header_value(value: &str) -> Result<(), String> {
    if value.bytes().any(|b| matches!(b, b'\r' | b'\n' | 0)) {
        return Err("fetch: invalid request header value".to_string());
    }
    Ok(())
}

fn is_http_token_byte(b: u8) -> bool {
    matches!(
        b,
        b'!' | b'#'
            | b'$'
            | b'%'
            | b'&'
            | b'\''
            | b'*'
            | b'+'
            | b'-'
            | b'.'
            | b'^'
            | b'_'
            | b'`'
            | b'|'
            | b'~'
            | b'0'..=b'9'
            | b'A'..=b'Z'
            | b'a'..=b'z'
    )
}

fn is_forbidden_fetch_request_header(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "accept-charset"
            | "accept-encoding"
            | "access-control-request-headers"
            | "access-control-request-method"
            | "connection"
            | "content-length"
            | "cookie"
            | "cookie2"
            | "date"
            | "dnt"
            | "expect"
            | "host"
            | "keep-alive"
            | "origin"
            | "permissions-policy"
            | "referer"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "via"
    ) || lower.starts_with("proxy-")
        || lower.starts_with("sec-")
}

struct DataUrlPayload {
    mime_type: String,
    bytes: Vec<u8>,
}

fn parse_data_url(url: &str) -> Option<Result<DataUrlPayload, String>> {
    let rest = url.strip_prefix("data:")?;
    let Some(comma) = rest.find(',') else {
        return Some(Err("fetch: invalid data URL".to_string()));
    };
    let meta = &rest[..comma];
    let payload = &rest[comma + 1..];
    let mut mime_type = String::new();
    let mut base64 = false;
    for (idx, part) in meta.split(';').enumerate() {
        if idx == 0 {
            mime_type = part.to_string();
        } else if part.eq_ignore_ascii_case("base64") {
            base64 = true;
        } else if !mime_type.is_empty() {
            mime_type.push(';');
            mime_type.push_str(part);
        }
    }
    if mime_type.is_empty() {
        mime_type = "text/plain;charset=US-ASCII".to_string();
    }
    let bytes = if base64 {
        rusty_js_basen::decode_base64(payload)
            .map_err(|_| "fetch: invalid data URL base64 payload".to_string())
    } else {
        Ok(rusty_js_percent_encoding::decode_lenient(payload))
    };
    Some(bytes.map(|bytes| DataUrlPayload { mime_type, bytes }))
}

fn resolve_data_url_fetch(
    rt: &mut Runtime,
    promise: ObjectRef,
    url: &str,
    data: DataUrlPayload,
) -> Result<Value, RuntimeError> {
    let h_obj = rt.alloc_object(Object::new_ordinary());
    rt.object_set(
        h_obj,
        "content-type".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            data.mime_type,
        ))),
    );
    let init = rt.alloc_object(Object::new_ordinary());
    rt.object_set(init, "status".into(), Value::Number(200.0));
    rt.object_set(
        init,
        "statusText".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from("OK"))),
    );
    rt.object_set(init, "headers".into(), Value::Object(h_obj));
    let body = crate::http::http_buffer_from_bytes(rt, &data.bytes);
    let response_ctor = rt.global_get("Response");
    let resp = rt.construct(response_ctor, vec![body, Value::Object(init)])?;
    if let Value::Object(id) = &resp {
        rt.object_set(
            *id,
            "url".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                url.to_string(),
            ))),
        );
        rt.object_set(
            *id,
            "type".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from("basic"))),
        );
    }
    rusty_js_runtime::promise::resolve_promise(rt, promise, resp);
    Ok(Value::Object(promise))
}

fn checked_fetch_body_bytes(current: usize, next: usize) -> Result<usize, &'static str> {
    match current.checked_add(next) {
        Some(n) if n <= FETCH_MAX_RESPONSE_BODY_BYTES => Ok(n),
        _ => Err("response body exceeded fetch limit"),
    }
}

fn validate_fetch_egress_target(host: &str, port: u16, allow_internal: bool) -> Result<(), String> {
    let canonical = canonical_fetch_host(host)?;
    if !allow_internal && is_forbidden_fetch_host_name(&canonical) {
        return Err(format!(
            "fetch: blocked internal host '{host}' {FETCH_INTERNAL_POLICY_HINT}"
        ));
    }
    if let Ok(ip) = canonical.parse::<IpAddr>() {
        return validate_fetch_ip(host, ip, allow_internal);
    }
    match (canonical.as_str(), port).to_socket_addrs() {
        Ok(addrs) => {
            for addr in addrs {
                validate_fetch_ip(host, addr.ip(), allow_internal)?;
            }
            Ok(())
        }
        Err(_) => Ok(()),
    }
}

fn canonical_fetch_host(host: &str) -> Result<String, String> {
    let trimmed = host.trim().trim_end_matches('.').to_ascii_lowercase();
    if trimmed.is_empty() {
        return Err("fetch: empty host".to_string());
    }
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        return Ok(trimmed[1..trimmed.len() - 1].to_string());
    }
    Ok(trimmed)
}

fn is_forbidden_fetch_host_name(host: &str) -> bool {
    matches!(host, "localhost" | "localhost.localdomain")
        || host.ends_with(".localhost")
        || host.ends_with(".local")
}

fn validate_fetch_ip(original_host: &str, ip: IpAddr, allow_internal: bool) -> Result<(), String> {
    if allow_internal {
        return Ok(());
    }
    let blocked = match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_multicast()
                || v4.octets() == [169, 254, 169, 254]
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    };
    if blocked {
        Err(format!(
            "fetch: blocked internal address '{original_host}' {FETCH_INTERNAL_POLICY_HINT}"
        ))
    } else {
        Ok(())
    }
}

fn header_value(headers: &[(String, String)], name: &str) -> Option<String> {
    headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.clone())
}

fn resolve_fetch_location(base: &str, loc: &str) -> Result<String, String> {
    if loc.starts_with("http://") || loc.starts_with("https://") {
        return Ok(loc.to_string());
    }
    let (scheme, host, port, target) = crate::http_client::parse_url(base)
        .ok_or_else(|| format!("fetch: invalid base URL '{base}'"))?;
    let port_suffix = match (scheme.as_str(), port) {
        ("http", 80) | ("https", 443) => String::new(),
        _ => format!(":{port}"),
    };
    if loc.starts_with('/') {
        return Ok(format!("{scheme}://{host}{port_suffix}{loc}"));
    }
    let base_dir = match target.rfind('/') {
        Some(0) | None => "/",
        Some(i) => &target[..=i],
    };
    Ok(format!("{scheme}://{host}{port_suffix}{base_dir}{loc}"))
}

pub fn fetch_poll_io(rt: &mut Runtime) -> Result<bool, RuntimeError> {
    use std::sync::mpsc::TryRecvError;
    let agent_id = rt.agent_id();

    let ready: Option<(
        usize,
        ObjectRef,
        String,
        usize,
        String,
        Option<ObjectRef>,
        Option<ObjectRef>,
        Option<ObjectRef>,
        Arc<AtomicBool>,
        Vec<FetchMsg>,
    )> = PENDING_FETCHES.with(|v| {
        let mut vv = v.borrow_mut();
        for (i, slot) in vv.iter_mut().enumerate() {
            let Some(p) = slot.as_mut() else { continue };
            if p.agent_id != agent_id {
                continue;
            }
            let mut msgs = Vec::new();
            loop {
                match p.rx.try_recv() {
                    Ok(m) => msgs.push(m),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {

                        let said_how_it_ended = msgs
                            .iter()
                            .any(|m| matches!(m, FetchMsg::End | FetchMsg::Err(_)));
                        if !said_how_it_ended {
                            msgs.push(FetchMsg::Err("fetch thread terminated".to_string()));
                        }
                        break;
                    }
                }
            }
            if !msgs.is_empty() {
                return Some((
                    i,
                    p.promise,
                    p.url.clone(),
                    p.realm,
                    p.root_key.clone(),
                    p.ctrl,
                    p.signal,
                    p.abort_listener,
                    p.done.clone(),
                    msgs,
                ));
            }
        }
        None
    });

    let Some((idx, promise, _url, realm, root_key, mut ctrl, signal, abort_listener, done, msgs)) =
        ready
    else {
        return Ok(false);
    };

    let prior = rt.enter_realm(realm);
    let mut finished = false;
    for m in msgs {
        match m {
            FetchMsg::Head {
                head,
                url,
                redirected,
            } => {

                match build_response(rt, &url, redirected, &head) {
                    Ok((resp, holder)) => {
                        ctrl = Some(holder);

                        rt.retain_host_roots(
                            root_key.clone(),
                            fetch_roots(promise, Some(holder), signal, abort_listener),
                        );
                        rusty_js_runtime::promise::resolve_promise(rt, promise, resp);
                    }
                    Err(e) => {
                        let _ = reject_network(rt, promise, format!("{e:?}"));
                        finished = true;
                    }
                }
            }
            FetchMsg::Chunk(bytes) => {
                if let Some(holder) = ctrl {
                    let chunk = crate::http::http_buffer_from_bytes(rt, &bytes);
                    stream_call(rt, holder, "enqueue", vec![chunk]);
                }
            }
            FetchMsg::End => {
                if let Some(holder) = ctrl {
                    stream_call(rt, holder, "close", Vec::new());
                }
                finished = true;
            }
            FetchMsg::Err(e) => {
                match ctrl {

                    Some(holder) => {
                        let msg = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                            format!("fetch: {e}"),
                        )));
                        stream_call(rt, holder, "error", vec![msg]);
                    }
                    None => {
                        let _ = reject_network(rt, promise, e);
                    }
                }
                finished = true;
            }
        }
    }
    rt.exit_realm(prior);

    if finished {
        done.store(true, Ordering::SeqCst);
        if let (Some(sig), Some(listener)) = (signal, abort_listener) {
            remove_abort_listener(rt, sig, listener);
        }
        rt.release_host_roots(&root_key);
        PENDING_FETCHES.with(|v| {
            let mut vv = v.borrow_mut();
            if let Some(slot) = vv.get_mut(idx) {
                *slot = None;
            }
            vv.retain(|x| x.is_some());
        });
    } else {
        PENDING_FETCHES.with(|v| {
            let mut vv = v.borrow_mut();
            if let Some(Some(p)) = vv.get_mut(idx) {
                if p.agent_id == agent_id {
                    p.ctrl = ctrl;
                }
            }
        });
    }
    Ok(true)
}

fn stream_call(rt: &mut Runtime, holder: ObjectRef, method: &str, args: Vec<Value>) {
    let ctrl = match rt.object_get(holder, "__ctrl") {
        Value::Object(c) => c,

        _ => return,
    };
    let f = rt.object_get(ctrl, method);
    if rt.is_callable(&f) {
        let _ = rt.call_function(f, Value::Object(ctrl), args);
    }
}

pub fn has_pending_fetch(rt: &Runtime) -> bool {
    let agent_id = rt.agent_id();
    PENDING_FETCHES.with(|v| {
        v.borrow()
            .iter()
            .any(|x| x.as_ref().is_some_and(|p| p.agent_id == agent_id))
    })
}

fn build_response(
    rt: &mut Runtime,
    url: &str,
    redirected: bool,
    pr: &rusty_http_codec::ResponseHead,
) -> Result<(Value, ObjectRef), RuntimeError> {
    let h_obj = rt.alloc_object(Object::new_ordinary());
    for (n, v) in &pr.headers {
        rt.object_set(
            h_obj,
            n.to_ascii_lowercase(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(v.clone()))),
        );
    }
    let init = rt.alloc_object(Object::new_ordinary());
    rt.object_set(init, "status".into(), Value::Number(pr.status as f64));
    rt.object_set(
        init,
        "statusText".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            pr.reason.clone(),
        ))),
    );
    rt.object_set(init, "headers".into(), Value::Object(h_obj));

    let (body, holder) = make_stream_body(rt)?;
    let response_ctor = rt.global_get("Response");
    let resp = rt.construct(response_ctor, vec![body, Value::Object(init)])?;
    if let Value::Object(id) = &resp {
        rt.object_set(
            *id,
            "url".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                url.to_string(),
            ))),
        );
        rt.object_set(*id, "redirected".into(), Value::Boolean(redirected));

        if let Value::Object(hid) = rt.object_get(*id, "headers") {
            if let Value::Object(bag) = rt.object_get(hid, "__headers") {
                for (n, v) in &pr.headers {
                    rt.object_set(
                        bag,
                        n.to_ascii_lowercase(),
                        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(v.clone()))),
                    );
                }
            }
        }
    }
    Ok((resp, holder))
}

fn make_stream_body(rt: &mut Runtime) -> Result<(Value, ObjectRef), RuntimeError> {
    let holder = rt.alloc_object(Object::new_ordinary());
    let source = rt.alloc_object(Object::new_ordinary());
    register_method(rt, source, "start", move |rt, args| {
        let ctrl = args.first().cloned().unwrap_or(Value::Undefined);
        rt.set_engine_sentinel(holder, "__ctrl", ctrl);
        Ok(Value::Undefined)
    });

    rt.materialize_lazy_global("ReadableStream");
    let rs_ctor = rt.global_get("ReadableStream");
    let stream = rt.construct(rs_ctor, vec![Value::Object(source)])?;
    Ok((stream, holder))
}

#[cfg(test)]
mod tests {
    use super::{
        checked_fetch_body_bytes, notify_agent_wake, parse_data_url, strip_redirect_credentials,
        validate_fetch_egress_target, validate_fetch_request_headers, FetchOrigin, PendingFetch,
        FETCH_MAX_RESPONSE_BODY_BYTES, FETCH_TIMEOUT_MS, PENDING_FETCHES,
    };
    use rusty_js_runtime::{AgentId, Object, Runtime};
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    fn headers() -> Vec<(String, String)> {
        vec![
            ("Authorization".into(), "Bearer secret".into()),
            ("Cookie".into(), "sid=secret".into()),
            ("Proxy-Authorization".into(), "Basic secret".into()),
            ("X-Keep".into(), "ok".into()),
        ]
    }

    #[test]
    fn fetch_completion_notify_wakes_owner_runtime() {
        let rt = Runtime::new_with_agent_id(AgentId::from_raw(303));
        let wake = rt.agent_wake_handle();
        let before = rt.agent_wake_generation();

        notify_agent_wake(&wake);

        assert_ne!(
            rt.agent_wake_generation(),
            before,
            "fetch completion producer must advance the owner runtime wake"
        );
    }

    #[test]
    fn redirect_credentials_survive_same_origin() {
        let origin = FetchOrigin {
            scheme: "http".into(),
            host: "Example.TEST".into(),
            port: 80,
        };
        let mut headers = headers();
        strip_redirect_credentials(&mut headers, &origin, &origin);
        assert_eq!(headers.len(), 4);
    }

    #[test]
    fn redirect_credentials_strip_cross_origin_but_keep_other_headers() {
        let from = FetchOrigin {
            scheme: "http".into(),
            host: "example.test".into(),
            port: 80,
        };
        let to = FetchOrigin {
            scheme: "http".into(),
            host: "example.test".into(),
            port: 8080,
        };
        let mut headers = headers();
        strip_redirect_credentials(&mut headers, &from, &to);
        assert_eq!(headers, vec![("X-Keep".into(), "ok".into())]);
    }

    #[test]
    fn redirect_credentials_strip_on_https_to_http_downgrade() {
        let from = FetchOrigin {
            scheme: "https".into(),
            host: "example.test".into(),
            port: 443,
        };
        let to = FetchOrigin {
            scheme: "http".into(),
            host: "example.test".into(),
            port: 80,
        };
        let mut headers = headers();
        strip_redirect_credentials(&mut headers, &from, &to);
        assert_eq!(headers, vec![("X-Keep".into(), "ok".into())]);
    }

    #[test]
    fn fetch_request_headers_accept_safe_extension_headers() {
        let headers = vec![
            ("X-Trace".into(), "abc123".into()),
            ("Content-Type".into(), "text/plain".into()),
        ];
        assert!(validate_fetch_request_headers(&headers).is_ok());
    }

    #[test]
    fn fetch_request_headers_reject_control_values() {
        for value in ["ok\r\nInjected: yes", "ok\n", "ok\0tail"] {
            let headers = vec![("X-Test".into(), value.into())];
            assert!(validate_fetch_request_headers(&headers).is_err());
        }
    }

    #[test]
    fn fetch_request_headers_reject_invalid_names() {
        for name in ["", "Bad Name", "Bad:Name", "Bad\rName"] {
            let headers = vec![(name.into(), "ok".into())];
            assert!(validate_fetch_request_headers(&headers).is_err());
        }
    }

    #[test]
    fn fetch_request_headers_reject_forbidden_names() {
        for name in [
            "Host",
            "Content-Length",
            "Connection",
            "Transfer-Encoding",
            "Cookie",
            "Sec-Fetch-Mode",
            "Proxy-Authorization",
        ] {
            let headers = vec![(name.into(), "ok".into())];
            assert!(validate_fetch_request_headers(&headers).is_err());
        }
    }

    #[test]
    fn fetch_egress_allows_public_literal_and_dns_name() {
        assert!(validate_fetch_egress_target("93.184.216.34", 80, false).is_ok());
        assert!(validate_fetch_egress_target("example.com", 80, false).is_ok());
    }

    #[test]
    fn fetch_egress_rejects_localhost_names() {
        for host in ["localhost", "LOCALHOST.", "api.localhost", "service.local"] {
            assert!(validate_fetch_egress_target(host, 80, false).is_err());
        }
    }

    #[test]
    fn fetch_egress_localhost_error_names_policy_and_opt_in() {
        let err = validate_fetch_egress_target("localhost", 80, false).unwrap_err();
        assert!(err.contains("blocked internal host 'localhost'"));
        assert!(err.contains("default SSRF/internal-network policy"));
        assert!(err.contains("--allow-net-loopback"));
        assert!(err.contains("CRUFT_ALLOW_NET_LOOPBACK=1"));
    }

    #[test]
    fn fetch_egress_rejects_internal_ipv4_literals() {
        for host in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.0.1",
            "169.254.1.1",
            "169.254.169.254",
            "0.0.0.0",
            "224.0.0.1",
        ] {
            assert!(validate_fetch_egress_target(host, 80, false).is_err());
        }
    }

    #[test]
    fn fetch_egress_internal_address_error_names_policy_and_opt_in() {
        let err = validate_fetch_egress_target("127.0.0.1", 80, false).unwrap_err();
        assert!(err.contains("blocked internal address '127.0.0.1'"));
        assert!(err.contains("default SSRF/internal-network policy"));
        assert!(err.contains("--allow-net-loopback"));
        assert!(err.contains("CRUFT_ALLOW_NET_LOOPBACK=1"));
    }

    #[test]
    fn fetch_egress_rejects_internal_ipv6_literals() {
        for host in ["::1", "[::1]", "fc00::1", "fd00::1", "fe80::1", "::"] {
            assert!(validate_fetch_egress_target(host, 80, false).is_err());
        }
    }

    #[test]
    fn fetch_egress_allows_internal_addresses_with_explicit_grant() {
        for host in ["localhost", "127.0.0.1", "::1", "[::1]", "10.1.2.3"] {
            assert!(validate_fetch_egress_target(host, 80, true).is_ok());
        }
    }

    #[test]
    fn fetch_body_limit_counter_rejects_overflow_and_over_limit() {
        assert_eq!(
            checked_fetch_body_bytes(FETCH_MAX_RESPONSE_BODY_BYTES - 1, 1),
            Ok(FETCH_MAX_RESPONSE_BODY_BYTES)
        );
        assert!(checked_fetch_body_bytes(FETCH_MAX_RESPONSE_BODY_BYTES, 1).is_err());
        assert!(checked_fetch_body_bytes(usize::MAX, 1).is_err());
    }

    #[test]
    fn data_url_parser_decodes_default_text_percent_and_base64() {
        let plain = parse_data_url("data:,hello%20d3")
            .expect("data url")
            .expect("parse");
        assert_eq!(plain.mime_type, "text/plain;charset=US-ASCII");
        assert_eq!(plain.bytes, b"hello d3");

        let typed = parse_data_url("data:application/json,%7B%22ok%22%3Atrue%7D")
            .expect("data url")
            .expect("parse");
        assert_eq!(typed.mime_type, "application/json");
        assert_eq!(typed.bytes, br#"{"ok":true}"#);

        let b64 = parse_data_url("data:text/plain;base64,aGVsbG8=")
            .expect("data url")
            .expect("parse");
        assert_eq!(b64.mime_type, "text/plain");
        assert_eq!(b64.bytes, b"hello");
    }

    #[test]
    fn fetch_timeout_budget_is_nonzero() {
        assert!(FETCH_TIMEOUT_MS > 0);
    }

    #[test]
    fn pending_fetch_registry_is_scoped_by_runtime_agent_id() {
        let mut rt_a = Runtime::new_with_agent_id(AgentId::from_raw(301));
        let mut rt_b = Runtime::new_with_agent_id(AgentId::from_raw(302));
        let promise_b = rt_b.alloc_object(Object::new_ordinary());
        let (_tx_b, rx_b) = std::sync::mpsc::channel();

        PENDING_FETCHES.with(|v| {
            v.borrow_mut().clear();
            v.borrow_mut().push(Some(PendingFetch {
                agent_id: rt_b.agent_id(),
                rx: rx_b,
                promise: promise_b,
                url: "http://example.test/".to_string(),
                realm: rt_b.current_realm,
                root_key: "fetch:test-agent-b".to_string(),
                ctrl: None,
                cancel: Arc::new(AtomicBool::new(false)),
                signal: None,
                abort_listener: None,
                done: Arc::new(AtomicBool::new(false)),
            }));
        });

        assert!(!super::has_pending_fetch(&rt_a));
        assert!(super::has_pending_fetch(&rt_b));
        assert!(
            !super::fetch_poll_io(&mut rt_a).expect("poll agent A"),
            "agent A must not harvest agent B's pending fetch"
        );
        assert!(
            super::has_pending_fetch(&rt_b),
            "agent B pending fetch must remain after agent A poll"
        );

        PENDING_FETCHES.with(|v| v.borrow_mut().clear());
        rt_b.release_host_roots("fetch:test-agent-b");
    }
}
