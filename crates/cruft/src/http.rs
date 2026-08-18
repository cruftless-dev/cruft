
use crate::register::{native_function, new_object, register_method, set_constant};
use rusty_js_runtime::caps::{self, ModuleId, ModuleProvenance};
use rusty_js_runtime::value::{ObjectRef, PropertyKey};
use rusty_js_runtime::{AgentId, HostEnqueuePhase, Runtime, RuntimeError, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

const HTTP_WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const HTTP_MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn notify_agent_wake(wake: &std::sync::Arc<(std::sync::Mutex<u64>, std::sync::Condvar)>) {
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

const SERVER_SLOT: &str = "__cruft_http_server_id";
const BODY_SLOT: &str = "__cruft_http_body";
const HEADERS_SLOT: &str = "__cruft_http_headers";

const WH_WIRE_SLOT: &str = "__cruft_http_wh_wire_headers";

const HEADER_CASE_SLOT: &str = "__cruft_http_header_case";
const REQUEST_LISTENERS_SLOT: &str = "__cruft_http_request_listeners";

const CRUFT_FETCH_HANDLER_SLOT: &str = "__cruft_fetch_handler";
const REQUEST_ONCE_SLOT: &str = "__cruft_http_once";
const REQ_BODY_BUF_SLOT: &str = "__cruft_http_req_body";
const REQ_DATA_LISTENERS_SLOT: &str = "__cruft_http_req_data_listeners";
const REQ_END_LISTENERS_SLOT: &str = "__cruft_http_req_end_listeners";

const REQ_BODY_DELIVERED_SLOT: &str = "__cruft_http_req_body_delivered";
const REQ_CLOSE_LISTENERS_SLOT: &str = "__cruft_http_req_close_listeners";
const RES_LISTENERS_SLOT: &str = "__cruft_http_res_listeners";
const BOUNDARY_CALLABLE_FACADE_SLOT: &str = "__cruft_boundary_callable_facade";
const AGENT_SOCKET_SLOT: &str = "__cruft_http_agent_socket";
const AGENT_SOCKET_FREE_SLOT: &str = "__cruft_http_agent_socket_free";
const CLIENT_SOCKET_SLOT: &str = "__cruft_http_client_socket";
const SOCKET_LAST_ASYNC_ID_SLOT: &str = "__cruft_http_socket_last_async_id";
const SERVER_RESPONSE_REQUEST_SLOT: &str = "__cruft_http_server_response_request";
const HTTP_RESOURCE_DESTROYED_SLOT: &str = "__cruft_http_resource_destroyed";
const STATIC_DIR_ROOT_SLOT: &str = "__cruft_static_dir_root";
const STATIC_DIR_INDEX_SLOT: &str = "__cruft_static_dir_index";

fn emitter_event_key(rt: &mut Runtime, v: &Value) -> Result<String, RuntimeError> {
    match v {
        Value::Symbol(sym) => Ok(format!("@@event-symbol:{}", sym.as_str())),
        other => value_to_string(rt, other),
    }
}

fn emit_res_event(
    rt: &mut Runtime,
    obj: ObjectRef,
    event: &str,
    args: Vec<Value>,
) -> Result<bool, RuntimeError> {
    let listeners = match rt.object_get(obj, RES_LISTENERS_SLOT) {
        Value::Object(id) => id,
        _ => return Ok(false),
    };
    let arr = match rt.object_get(listeners, event) {
        Value::Object(a) => a,
        _ => return Ok(false),
    };
    let len = rt.array_length(arr);
    let mut fns = Vec::new();
    for i in 0..len {
        let f = rt.object_get(arr, &i.to_string());
        if rt.is_callable(&f) {
            fns.push(f);
        }
    }
    let had = !fns.is_empty();
    for f in fns {
        rt.call_function(f, Value::Object(obj), args.clone())?;
    }
    Ok(had)
}
const MAX_REQUEST_BYTES: usize = rusty_http_codec::MAX_DECODED_BODY_BYTES + 64 * 1024;

fn client_abort_error(rt: &mut Runtime) -> Value {
    let s = |v: &str| Value::String(std::rc::Rc::new(rusty_js_runtime::value::JsString::from(v)));
    let ctor = rt.global_get("Error");
    let msg = s("The operation was aborted");
    match rt.construct(ctor, vec![msg.clone()]) {
        Ok(Value::Object(e)) => {
            rt.object_set(e, "name".into(), s("AbortError"));
            rt.object_set(e, "code".into(), s("ABORT_ERR"));
            Value::Object(e)
        }
        _ => msg,
    }
}

fn client_fire_abort(rt: &mut Runtime, req: ObjectRef) {
    if matches!(
        rt.object_get(req, "__cruft_abort_fired"),
        Value::Boolean(true)
    ) {
        return;
    }
    rt.set_engine_sentinel(req, "__cruft_abort_fired", Value::Boolean(true));
    rt.set_engine_sentinel(req, "__cruft_aborted", Value::Boolean(true));
    rt.object_set(req, "destroyed".into(), Value::Boolean(true));
    let err = client_abort_error(rt);
    let _ = emit_res_event(rt, req, "error", vec![err]);

    let _ = emit_res_event(rt, req, "close", Vec::new());
}

fn client_econnreset_error(rt: &mut Runtime) -> Value {
    let s = |v: &str| Value::String(std::rc::Rc::new(rusty_js_runtime::value::JsString::from(v)));
    let ctor = rt.global_get("Error");
    let msg = s("socket hang up");
    match rt.construct(ctor, vec![msg.clone()]) {
        Ok(Value::Object(e)) => {
            rt.object_set(e, "code".into(), s("ECONNRESET"));
            Value::Object(e)
        }
        _ => msg,
    }
}

fn client_destroy(rt: &mut Runtime, req: ObjectRef, err: Value, is_abort: bool) {
    if matches!(
        rt.object_get(req, "__cruft_destroyed"),
        Value::Boolean(true)
    ) || matches!(
        rt.object_get(req, "__cruft_abort_fired"),
        Value::Boolean(true)
    ) {
        return;
    }

    if matches!(
        rt.object_get(req, "__cruft_completed"),
        Value::Boolean(true)
    ) {
        rt.object_set(req, "destroyed".into(), Value::Boolean(true));
        if is_abort {
            rt.object_set(req, "aborted".into(), Value::Boolean(true));
        }
        return;
    }
    rt.set_engine_sentinel(req, "__cruft_destroyed", Value::Boolean(true));
    rt.set_engine_sentinel(req, "__cruft_abort_fired", Value::Boolean(true));
    rt.set_engine_sentinel(req, "__cruft_aborted", Value::Boolean(true));
    rt.object_set(req, "destroyed".into(), Value::Boolean(true));
    if is_abort {
        rt.object_set(req, "aborted".into(), Value::Boolean(true));
    }
    rt.enqueue_host_phase_rooted(
        HostEnqueuePhase::HostCompletionMacrotask,
        "http.request.destroy",
        vec![req],
        move |rt| {
            if is_abort {
                let _ = emit_res_event(rt, req, "abort", Vec::new());
            }
            let _ = emit_res_event(rt, req, "error", vec![err.clone()]);
            let _ = emit_res_event(rt, req, "close", Vec::new());
            Ok(())
        },
    );
}

fn set_async_id_symbol_property(rt: &mut Runtime, object: ObjectRef, value: f64) {
    let symbol = match rt.global_get("__cruft_internal_async_hooks") {
        Value::Object(internal) => match rt.object_get(internal, "symbols") {
            Value::Object(symbols) => match rt.object_get(symbols, "async_id_symbol") {
                Value::Symbol(sym) => Some(sym),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    };
    if let Some(sym) = symbol {
        rt.obj_mut(object).properties.insert(
            PropertyKey::Symbol(sym),
            rusty_js_runtime::value::PropertyDescriptor {
                value: Value::Number(value),
                writable: true,
                enumerable: false,
                configurable: true,
                getter: None,
                setter: None,
            },
        );
    }
}

fn http_agent_socket_for_request(
    rt: &mut Runtime,
    agent: ObjectRef,
) -> Result<ObjectRef, RuntimeError> {
    let socket = match rt.object_get(agent, AGENT_SOCKET_SLOT) {
        Value::Object(socket) => socket,
        _ => {
            let socket = new_object(rt);
            rt.object_set(socket, "writable".into(), Value::Boolean(true));
            rt.object_set(socket, "readable".into(), Value::Boolean(true));
            rt.set_engine_sentinel(agent, AGENT_SOCKET_SLOT, Value::Object(socket));
            socket
        }
    };
    if matches!(
        rt.object_get(agent, AGENT_SOCKET_FREE_SLOT),
        Value::Boolean(true)
    ) {
        if let Value::Number(old_id) = rt.object_get(socket, SOCKET_LAST_ASYNC_ID_SLOT) {
            crate::node_stubs::async_hooks_emit_destroy_id_for_global(rt, old_id)?;
        }
    }
    let async_id = crate::node_stubs::async_hooks_emit_init_for_global(
        rt,
        "TCPSOCKETWRAP",
        Value::Object(socket),
    )?
    .unwrap_or(0.0);
    rt.set_engine_sentinel(socket, SOCKET_LAST_ASYNC_ID_SLOT, Value::Number(async_id));
    set_async_id_symbol_property(rt, socket, async_id);
    rt.set_engine_sentinel(agent, AGENT_SOCKET_FREE_SLOT, Value::Boolean(false));
    Ok(socket)
}

fn http_agent_mark_socket_free(rt: &mut Runtime, socket: ObjectRef) {
    set_async_id_symbol_property(rt, socket, -1.0);
    if let Value::Object(agent) = rt.object_get(socket, "__cruft_http_agent_owner") {
        rt.set_engine_sentinel(agent, AGENT_SOCKET_FREE_SLOT, Value::Boolean(true));
    }
}

fn emit_http_resource_destroy_once(
    rt: &mut Runtime,
    resource: ObjectRef,
) -> Result<(), RuntimeError> {
    if matches!(
        rt.object_get(resource, HTTP_RESOURCE_DESTROYED_SLOT),
        Value::Boolean(true)
    ) {
        return Ok(());
    }
    rt.set_engine_sentinel(resource, HTTP_RESOURCE_DESTROYED_SLOT, Value::Boolean(true));
    crate::node_stubs::async_hooks_emit_destroy_for_global(rt, Value::Object(resource))
}

pub(crate) fn http_buffer_from_bytes(rt: &mut Runtime, bytes: &[u8]) -> Value {

    crate::node_stubs::intrinsic_buffer_from_bytes(rt, bytes)
}

#[derive(Clone)]
struct ActiveHttpServer {
    agent_id: AgentId,
    listener_handle: u64,
    bound_addr: String,
    handler_realm: usize,
    server_object: ObjectRef,
    refed: bool,
}

thread_local! {
    static HTTP_SERVERS: RefCell<Vec<Option<ActiveHttpServer>>> = RefCell::new(Vec::new());
}

fn next_server_id(server: ActiveHttpServer) -> usize {
    HTTP_SERVERS.with(|servers| {
        let mut servers = servers.borrow_mut();
        if let Some((idx, slot)) = servers.iter_mut().enumerate().find(|(_, s)| s.is_none()) {
            *slot = Some(server);
            idx
        } else {
            servers.push(Some(server));
            servers.len() - 1
        }
    })
}

fn get_server(id: usize) -> Option<ActiveHttpServer> {
    HTTP_SERVERS.with(|servers| servers.borrow().get(id).and_then(|s| s.clone()))
}

fn get_server_for_runtime(rt: &Runtime, id: usize) -> Option<ActiveHttpServer> {
    let agent_id = rt.agent_id();
    get_server(id).filter(|server| server.agent_id == agent_id)
}

fn remove_server(id: usize) -> Option<ActiveHttpServer> {
    HTTP_SERVERS.with(|servers| servers.borrow_mut().get_mut(id).and_then(|s| s.take()))
}

fn remove_server_for_runtime(rt: &Runtime, id: usize) -> Option<ActiveHttpServer> {
    let agent_id = rt.agent_id();
    HTTP_SERVERS.with(|servers| {
        let mut servers = servers.borrow_mut();
        let slot = servers.get_mut(id)?;
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

fn set_server_refed(rt: &Runtime, id: usize, refed: bool) -> bool {
    let agent_id = rt.agent_id();
    HTTP_SERVERS.with(|servers| {
        if let Some(Some(server)) = servers.borrow_mut().get_mut(id) {
            if server.agent_id != agent_id {
                return false;
            }
            server.refed = refed;
            true
        } else {
            false
        }
    })
}

fn set_internal_slot(rt: &mut Runtime, obj: ObjectRef, name: &str, value: Value) {
    rt.set_engine_sentinel(obj, name, value);
}

fn value_to_string(rt: &mut Runtime, v: &Value) -> Result<String, RuntimeError> {
    rt.coerce_to_string(v)
}

fn is_js_array(rt: &mut Runtime, id: ObjectRef) -> bool {
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

fn value_to_bytes(rt: &mut Runtime, v: &Value) -> Result<Vec<u8>, RuntimeError> {

    if let Value::Object(id) = v {
        if let Some(bytes) = rt.typed_array_view_bytes(*id) {
            return Ok(bytes);
        }
        if let Value::Number(n) = rt.object_get(*id, "length") {
            let len = n.max(0.0) as usize;
            let mut out = Vec::with_capacity(len);
            for i in 0..len {
                out.push(match rt.object_get(*id, &i.to_string()) {
                    Value::Number(b) => b as u8,
                    _ => 0,
                });
            }
            return Ok(out);
        }
    }
    Ok(value_to_string(rt, v)?.into_bytes())
}

fn percent_decode_static_path(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                if i + 2 >= bytes.len() {
                    return None;
                }
                let hi = (bytes[i + 1] as char).to_digit(16)?;
                let lo = (bytes[i + 2] as char).to_digit(16)?;
                out.push(((hi << 4) | lo) as u8);
                i += 3;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

fn static_request_path(url: &str) -> &str {
    let after_authority = url
        .find("://")
        .and_then(|scheme| url[scheme + 3..].find('/').map(|slash| scheme + 3 + slash))
        .unwrap_or(0);
    let path_and_query = &url[after_authority..];
    let end = path_and_query
        .find(['?', '#'])
        .unwrap_or(path_and_query.len());
    &path_and_query[..end]
}

fn clean_static_relative_path(path: &str) -> Option<PathBuf> {
    if path.contains('\0') || path.contains('\\') {
        return None;
    }
    let decoded = percent_decode_static_path(path)?;
    if decoded.contains('\0') || decoded.contains('\\') {
        return None;
    }
    let mut rel = PathBuf::new();
    for component in Path::new(decoded.trim_start_matches('/')).components() {
        match component {
            Component::Normal(part) => rel.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(rel)
}

fn mime_for_static_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "txt" => "text/plain; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

fn make_static_file_response(
    rt: &mut Runtime,
    path: &Path,
    meta: &std::fs::Metadata,
    bytes: Vec<u8>,
    request: ObjectRef,
) -> Result<Value, RuntimeError> {
    let response = new_object(rt);
    let method = match rt.object_get(request, "method") {
        Value::String(s) => s.as_str().to_ascii_uppercase(),
        _ => "GET".to_string(),
    };
    let mtime_secs = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let etag = format!("W/\"{:x}-{:x}\"", meta.len(), mtime_secs);
    let last_modified = format_http_date(mtime_secs);

    let if_match = static_request_header(rt, request, "if-match");
    let if_none_match = static_request_header(rt, request, "if-none-match");
    let if_unmodified_since = static_request_header(rt, request, "if-unmodified-since");
    let if_modified_since = static_request_header(rt, request, "if-modified-since");
    let range = static_request_header(rt, request, "range");
    let mut status = 200u16;
    let mut body_bytes = bytes;
    let mut content_range = None;
    if if_match
        .as_deref()
        .map(|v| !static_etag_list_matches(v, &etag))
        .unwrap_or(false)
        || if_unmodified_since
            .as_deref()
            .and_then(parse_http_date)
            .map(|since| mtime_secs > since)
            .unwrap_or(false)
    {
        status = 412;
        body_bytes = Vec::new();
    } else if if_none_match
        .as_deref()
        .map(|v| static_etag_list_matches(v, &etag))
        .unwrap_or(false)
    {
        status = 304;
        body_bytes = Vec::new();
    } else if if_modified_since
        .as_deref()
        .and_then(parse_http_date)
        .map(|since| mtime_secs <= since)
        .unwrap_or(false)
    {
        status = 304;
        body_bytes = Vec::new();
    } else if method != "HEAD" {
        if let Some(range) = range {
            if let Some((range_status, sliced, header)) =
                apply_single_static_range(&range, &body_bytes)
            {
                status = range_status;
                body_bytes = sliced;
                content_range = Some(header);
            }
        }
    }
    if method == "HEAD" {
        body_bytes = Vec::new();
    }

    rt.object_set(response, "status".into(), Value::Number(status as f64));

    let headers = new_object(rt);
    rt.object_set(
        headers,
        "content-type".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            mime_for_static_path(path),
        ))),
    );
    rt.object_set(
        headers,
        "accept-ranges".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from("bytes"))),
    );
    rt.object_set(
        headers,
        "last-modified".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            last_modified,
        ))),
    );
    rt.object_set(
        headers,
        "etag".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(etag))),
    );
    if let Some(content_range) = content_range {
        rt.object_set(
            headers,
            "content-range".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                content_range,
            ))),
        );
    }
    rt.object_set(response, "headers".into(), Value::Object(headers));

    let body = String::from_utf8_lossy(&body_bytes).into_owned();
    rt.object_set(
        response,
        "body".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(body))),
    );
    rt.object_set(
        response,
        "__body_bytes".into(),
        Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from_latin1_bytes(body_bytes),
        )),
    );
    Ok(Value::Object(response))
}

fn make_static_dir_resolver(rt: &mut Runtime, root: String, index: Option<String>) -> ObjectRef {
    let resolver = new_object(rt);
    rt.object_set(
        resolver,
        STATIC_DIR_ROOT_SLOT.into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(root))),
    );
    if let Some(index) = index {
        rt.object_set(
            resolver,
            STATIC_DIR_INDEX_SLOT.into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(index))),
        );
    }
    register_method(rt, resolver, "respond", |rt, args| {
        let resolver = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Null),
        };
        let request = args.first().cloned().unwrap_or(Value::Undefined);
        static_dir_respond(rt, resolver, &request)
    });
    resolver
}

fn maybe_push_date_header(rt: &mut Runtime, res: ObjectRef, headers: &mut Vec<(String, String)>) {
    if matches!(rt.object_get(res, "sendDate"), Value::Boolean(false)) {
        return;
    }
    if headers.iter().any(|(n, _)| n.eq_ignore_ascii_case("date")) {
        return;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    headers.push(("Date".into(), format_http_date(now)));
}

fn format_http_date(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let hour = rem / 3_600;
    let minute = (rem % 3_600) / 60;
    let second = rem % 60;
    let (year, month, day) = civil_from_days(days);
    let weekday = HTTP_WEEKDAYS[((days + 4).rem_euclid(7)) as usize];
    format!(
        "{}, {:02} {} {:04} {:02}:{:02}:{:02} GMT",
        weekday,
        day,
        HTTP_MONTHS[(month - 1) as usize],
        year,
        hour,
        minute,
        second
    )
}

fn parse_http_date(s: &str) -> Option<u64> {
    let (_, rest) = s.split_once(',')?;
    let mut parts = rest.trim().split_ascii_whitespace();
    let day = parts.next()?.parse::<u32>().ok()?;
    let month_s = parts.next()?;
    let month = HTTP_MONTHS
        .iter()
        .position(|m| m.eq_ignore_ascii_case(month_s))? as u32
        + 1;
    let year = parts.next()?.parse::<i32>().ok()?;
    let time = parts.next()?;
    if !parts.next()?.eq_ignore_ascii_case("GMT") || parts.next().is_some() {
        return None;
    }
    let mut time_parts = time.split(':');
    let hour = time_parts.next()?.parse::<u32>().ok()?;
    let minute = time_parts.next()?.parse::<u32>().ok()?;
    let second = time_parts.next()?.parse::<u32>().ok()?;
    if time_parts.next().is_some()
        || !(1..=12).contains(&month)
        || day == 0
        || day > days_in_month(year, month)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    let days = days_from_civil(year, month, day);
    if days < 0 {
        return None;
    }
    Some(days as u64 * 86_400 + hour as u64 * 3_600 + minute as u64 * 60 + second as u64)
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    (year as i32, m as u32, d as u32)
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let mut y = year as i64;
    let m = month as i64;
    let d = day as i64;
    y -= (m <= 2) as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = m + if m > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn static_request_header(rt: &mut Runtime, request: ObjectRef, name: &str) -> Option<String> {
    let headers = match rt.object_get(request, "headers") {
        Value::Object(id) => id,
        _ => return None,
    };
    let bag = match rt.object_get(headers, "__headers") {
        Value::Object(id) => id,
        _ => headers,
    };
    match rt.object_get(bag, name) {
        Value::String(s) => Some(s.as_str().to_string()),
        _ => None,
    }
}

fn static_etag_list_matches(value: &str, etag: &str) -> bool {
    value
        .split(',')
        .map(|part| part.trim())
        .any(|part| part == etag || part == "*")
}

fn apply_single_static_range(range: &str, bytes: &[u8]) -> Option<(u16, Vec<u8>, String)> {
    let spec = range.strip_prefix("bytes=")?.trim();
    if spec.contains(',') {
        return None;
    }
    let len = bytes.len() as u64;
    let (start, end) = if let Some(suffix) = spec.strip_prefix('-') {
        let n = suffix.parse::<u64>().ok()?;
        if n == 0 || len == 0 {
            return Some((416, Vec::new(), format!("bytes */{len}")));
        }
        (len.saturating_sub(n), len - 1)
    } else {
        let (start_s, end_s) = spec.split_once('-')?;
        let start = start_s.parse::<u64>().ok()?;
        let end = if end_s.is_empty() {
            len.saturating_sub(1)
        } else {
            end_s.parse::<u64>().ok()?
        };
        (start, end)
    };
    if len == 0 || start >= len || end < start {
        return Some((416, Vec::new(), format!("bytes */{len}")));
    }
    let end = end.min(len - 1);
    let sliced = bytes[start as usize..=end as usize].to_vec();
    Some((206, sliced, format!("bytes {start}-{end}/{len}")))
}

fn static_dir_respond(
    rt: &mut Runtime,
    resolver: ObjectRef,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let root = match rt.object_get(resolver, STATIC_DIR_ROOT_SLOT) {
        Value::String(s) => PathBuf::from(s.as_str()),
        _ => return Ok(Value::Null),
    };
    let index = match rt.object_get(resolver, STATIC_DIR_INDEX_SLOT) {
        Value::String(s) => Some(s.as_str().to_string()),
        _ => None,
    };
    let request = match request {
        Value::Object(id) => *id,
        _ => return Ok(Value::Null),
    };
    let url = match rt.object_get(request, "url") {
        Value::String(s) => s.as_str().to_string(),
        _ => return Ok(Value::Null),
    };
    let rel = match clean_static_relative_path(static_request_path(&url)) {
        Some(rel) => rel,
        None => return Ok(Value::Null),
    };
    let root_canon = match std::fs::canonicalize(&root) {
        Ok(p) => p,
        Err(_) => return Ok(Value::Null),
    };
    let mut target = root_canon.join(rel);
    let meta = match std::fs::metadata(&target) {
        Ok(m) => m,
        Err(_) => return Ok(Value::Null),
    };
    if meta.is_dir() {
        let Some(index) = index else {
            return Ok(Value::Null);
        };
        target = target.join(index);
    }
    let target_canon = match std::fs::canonicalize(&target) {
        Ok(p) => p,
        Err(_) => return Ok(Value::Null),
    };
    if !target_canon.starts_with(&root_canon) {
        return Ok(Value::Null);
    }
    let meta = match std::fs::metadata(&target_canon) {
        Ok(m) if m.is_file() => m,
        _ => return Ok(Value::Null),
    };
    let bytes = match std::fs::read(&target_canon) {
        Ok(b) => b,
        Err(_) => return Ok(Value::Null),
    };

    make_static_file_response(rt, &target_canon, &meta, bytes, request)
}

fn make_static_adapter_request(rt: &mut Runtime, request: ObjectRef, url: String) -> ObjectRef {
    let adapted = new_object(rt);
    rt.object_set(
        adapted,
        "url".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(url))),
    );
    let method = rt.object_get(request, "method");
    if !matches!(method, Value::Undefined) {
        rt.object_set(adapted, "method".into(), method);
    }
    let headers = rt.object_get(request, "headers");
    if !matches!(headers, Value::Undefined) {
        rt.object_set(adapted, "headers".into(), headers);
    }
    adapted
}

fn make_static_not_found_response(rt: &mut Runtime) -> Value {
    let response = new_object(rt);
    rt.object_set(response, "status".into(), Value::Number(404.0));
    rt.object_set(
        response,
        "body".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "Not Found",
        ))),
    );
    Value::Object(response)
}

fn make_bun_static_routes_handler(
    rt: &mut Runtime,
    routes: ObjectRef,
    fetch: Value,
) -> Result<ObjectRef, RuntimeError> {
    let mut static_routes = Vec::<(String, ObjectRef)>::new();
    for key in rt.ordinary_own_enumerable_string_keys(routes) {
        let Some(prefix) = key.strip_suffix('*') else {
            continue;
        };
        if !prefix.ends_with('/') {
            continue;
        }
        let entry = match rt.object_get(routes, &key) {
            Value::Object(id) => id,
            _ => continue,
        };
        let dir = match rt.object_get(entry, "dir") {
            Value::String(s) => s.as_str().to_string(),
            _ => continue,
        };
        let index = Some("index.html".to_string());
        let resolver = make_static_dir_resolver(rt, dir, index);
        static_routes.push((prefix.to_string(), resolver));
    }
    let mut roots: Vec<ObjectRef> = static_routes
        .iter()
        .map(|(_, resolver)| *resolver)
        .collect();
    if let Value::Object(id) = &fetch {
        roots.push(*id);
    }
    Ok(crate::register::make_callable_rooted(
        rt,
        "Bun.serve.routes.fetch",
        roots,
        move |rt, args| {
            let request = match args.first() {
                Some(Value::Object(id)) => *id,
                _ => return Ok(make_static_not_found_response(rt)),
            };
            let url = match rt.object_get(request, "url") {
                Value::String(s) => s.as_str().to_string(),
                _ => String::new(),
            };
            let path = static_request_path(&url).to_string();
            for (prefix, resolver) in &static_routes {
                if path.starts_with(prefix) {
                    let suffix = format!("/{}", &path[prefix.len()..]);
                    let adapted = make_static_adapter_request(rt, request, suffix);
                    let response = static_dir_respond(rt, *resolver, &Value::Object(adapted))?;
                    if !matches!(response, Value::Null | Value::Undefined) {
                        return Ok(response);
                    }
                }
            }
            if rt.is_callable(&fetch) {
                rt.call_function(fetch.clone(), Value::Undefined, args.to_vec())
            } else {
                Ok(make_static_not_found_response(rt))
            }
        },
    ))
}

fn make_serve_websocket_handler(rt: &mut Runtime, fetch: Value, websocket: ObjectRef) -> ObjectRef {
    let mut roots = vec![websocket];
    if let Value::Object(id) = fetch {
        roots.push(id);
    }
    crate::register::make_callable_rooted(
        rt,
        "cruft:serve.websocket.fetch",
        roots,
        move |rt, args| {
            let request = args.first().cloned().unwrap_or(Value::Undefined);
            if crate::ws::is_upgrade_request(rt, &request)? {
                return crate::ws::accept_websocket(rt, &request, Some(&Value::Object(websocket)));
            }
            if rt.is_callable(&fetch) {
                rt.call_function(fetch.clone(), Value::Undefined, args.to_vec())
            } else {
                Ok(make_static_not_found_response(rt))
            }
        },
    )
}

fn serve_from_options(
    rt: &mut Runtime,
    opts: ObjectRef,
    net_cap: caps::Net,
    label: &str,
) -> Result<Value, RuntimeError> {
    let mut handler = if let Value::Object(routes) = rt.object_get(opts, "routes") {
        let fetch = rt.object_get(opts, "fetch");
        Value::Object(make_bun_static_routes_handler(rt, routes, fetch)?)
    } else {
        rt.object_get(opts, "handler")
    };
    if !rt.is_callable(&handler) {
        handler = rt.object_get(opts, "fetch");
    }
    if let Value::Object(websocket) = rt.object_get(opts, "websocket") {
        handler = Value::Object(make_serve_websocket_handler(rt, handler, websocket));
    }
    if !rt.is_callable(&handler) {
        return Err(RuntimeError::TypeError(format!(
            "{label}: handler/fetch must be callable or routes must include static dir entries"
        )));
    }
    let port = match rt.object_get(opts, "port") {
        Value::Number(n) => Value::Number(n),
        Value::String(s) => Value::String(s),
        _ => return Err(RuntimeError::TypeError(format!("{label}: port required"))),
    };
    let hostname = rt.object_get(opts, "hostname");
    let on_listen = rt.object_get(opts, "onListen");
    let tls = rt.object_get(opts, "tls");

    let server = if matches!(tls, Value::Object(_)) {
        let created = crate::tls::do_create_https_server(rt, &[tls, handler.clone()])?;
        if let Value::Object(sid) = created {
            set_internal_slot(rt, sid, CRUFT_FETCH_HANDLER_SLOT, handler);
            sid
        } else {
            return Err(RuntimeError::TypeError(format!(
                "{label}: createSecureServer did not return a server"
            )));
        }
    } else {
        make_cruft_http_server(rt, handler, net_cap)?
    };

    let mut listen_args = vec![port];
    if matches!(hostname, Value::String(_)) {
        listen_args.push(hostname);
    }
    if rt.is_callable(&on_listen) {
        listen_args.push(on_listen);
    }
    let listen = rt.object_get(server, "listen");
    rt.call_function(listen, Value::Object(server), listen_args)?;
    Ok(Value::Object(server))
}

fn current_body_bytes(rt: &mut Runtime, this: ObjectRef) -> Vec<u8> {
    match rt.object_get(this, BODY_SLOT) {
        Value::String(s) => s.as_bytes().to_vec(),
        Value::Object(id) => rt.typed_array_view_bytes(id).unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn append_body_chunk(rt: &mut Runtime, this: ObjectRef, chunk: &Value) -> Result<(), RuntimeError> {
    let mut bytes = current_body_bytes(rt, this);
    bytes.extend_from_slice(&value_to_bytes(rt, chunk)?);
    let buf = rt.alloc_uint8_array_from_bytes(&bytes);
    set_internal_slot(rt, this, BODY_SLOT, Value::Object(buf));
    Ok(())
}

const STREAM_ID_SLOT: &str = "__cruft_res_stream_id";
const STREAM_HDR_SENT: &str = "__cruft_res_hdr_sent";
const ASYNC_BUFFERED_SLOT: &str = "__cruft_res_async_buffered";
const ASYNC_KEEP_ALIVE_SLOT: &str = "__cruft_res_async_keep_alive";

struct StreamConn {
    agent_id: AgentId,
    stream_id: u64,
    res: ObjectRef,
    root_key: String,
}

thread_local! {
    static STREAMING_CONNS: std::cell::RefCell<Vec<StreamConn>> =
        std::cell::RefCell::new(Vec::new());

    static WENT_ASYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };

    static PARKED_CONNS: std::cell::RefCell<std::collections::HashMap<u64, (AgentId, usize)>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

const KEEP_ALIVE_TIMEOUT_MS: u64 = 5000;

pub fn collect_roots(roots: &mut Vec<ObjectRef>) {
    collect_roots_for_agent(AgentId::DEFAULT, roots);
}

pub fn collect_roots_for_runtime(rt: &Runtime, roots: &mut Vec<ObjectRef>) {
    collect_roots_for_agent(rt.agent_id(), roots);
}

fn collect_roots_for_agent(agent_id: AgentId, roots: &mut Vec<ObjectRef>) {
    HTTP_SERVERS.with(|v| {
        for s in v.borrow().iter().flatten() {
            if s.agent_id != agent_id {
                continue;
            }
            roots.push(s.server_object);
        }
    });
    STREAMING_CONNS.with(|v| {
        for c in v.borrow().iter() {
            if c.agent_id != agent_id {
                continue;
            }
            roots.push(c.res);
        }
    });
}

fn register_streaming_conn(rt: &mut Runtime, stream_id: u64, res: ObjectRef) {
    let agent_id = rt.agent_id();
    let root_key = format!("__cruft_stream_conn_{stream_id}");
    rt.retain_host_roots(root_key.clone(), vec![Value::Object(res)]);

    let _ = rusty_sockets::stream_set_nonblocking(stream_id, true);
    STREAMING_CONNS.with(|v| {
        v.borrow_mut().push(StreamConn {
            agent_id,
            stream_id,
            res,
            root_key,
        })
    });
    WENT_ASYNC.with(|c| c.set(true));
}

fn register_buffered_async_conn(
    rt: &mut Runtime,
    stream_id: u64,
    res: ObjectRef,
    keep_alive: bool,
) {
    rt.set_engine_sentinel(res, ASYNC_BUFFERED_SLOT, Value::Boolean(true));
    rt.set_engine_sentinel(res, ASYNC_KEEP_ALIVE_SLOT, Value::Boolean(keep_alive));
    register_streaming_conn(rt, stream_id, res);
}

fn drain_streaming_conns(rt: &mut Runtime) -> bool {
    let agent_id = rt.agent_id();

    let mut reap: Vec<(u64, ObjectRef, String)> = Vec::new();
    STREAMING_CONNS.with(|v| {
        for c in v.borrow().iter() {
            if c.agent_id != agent_id {
                continue;
            }
            let ended = matches!(
                rt.object_get(c.res, "__cruft_http_ended"),
                Value::Boolean(true)
            );
            if ended {
                reap.push((c.stream_id, c.res, c.root_key.clone()));
            }
        }
    });
    if !reap.is_empty() {
        let reaped: std::collections::HashSet<&str> =
            reap.iter().map(|(_, _, k)| k.as_str()).collect();
        STREAMING_CONNS.with(|v| {
            v.borrow_mut()
                .retain(|c| !reaped.contains(c.root_key.as_str()))
        });
        for (sid, _res, key) in &reap {
            let _ = rusty_sockets::handle_close(*sid);
            rt.release_host_roots(key);
        }
    }
    STREAMING_CONNS.with(|v| v.borrow().iter().any(|c| c.agent_id == agent_id))
}

pub fn has_streaming_conns() -> bool {
    let agent_id = AgentId::DEFAULT;
    STREAMING_CONNS.with(|v| v.borrow().iter().any(|c| c.agent_id == agent_id))
}

pub fn has_streaming_conns_for_runtime(rt: &Runtime) -> bool {
    let agent_id = rt.agent_id();
    STREAMING_CONNS.with(|v| v.borrow().iter().any(|c| c.agent_id == agent_id))
}

fn res_stream_id(rt: &mut Runtime, this: ObjectRef) -> Option<u64> {
    match rt.object_get(this, STREAM_ID_SLOT) {
        Value::Number(n) => Some(n as u64),
        _ => None,
    }
}

fn chunk_frame(data: &[u8]) -> Vec<u8> {
    let mut out = format!("{:x}\r\n", data.len()).into_bytes();
    out.extend_from_slice(data);
    out.extend_from_slice(b"\r\n");
    out
}

fn chunked_terminator(rt: &mut Runtime, res: ObjectRef) -> Vec<u8> {
    let mut out = b"0\r\n".to_vec();
    if let Value::Object(tid) = rt.object_get(res, "__trailers") {
        for key in rt.ordinary_own_enumerable_string_keys(tid) {
            match rt.object_get(tid, &key) {
                Value::Undefined => continue,
                v => {
                    if let Ok(s) = value_to_string(rt, &v) {
                        out.extend_from_slice(format!("{}: {}\r\n", key, s).as_bytes());
                    }
                }
            }
        }
    }
    out.extend_from_slice(b"\r\n");
    out
}

fn record_header_case(rt: &mut Runtime, owner: ObjectRef, original: &str) {
    let map = match rt.object_get(owner, HEADER_CASE_SLOT) {
        Value::Object(id) => id,
        _ => {
            let m = rt.alloc_object(rusty_js_runtime::Object::new_ordinary());
            rt.set_engine_sentinel(owner, HEADER_CASE_SLOT, Value::Object(m));
            m
        }
    };
    rt.object_set(
        map,
        original.to_ascii_lowercase(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            original.to_string(),
        ))),
    );
}

fn canonical_header_case(lower: &str) -> String {
    lower
        .split('-')
        .map(|seg| {
            let mut ch = seg.chars();
            match ch.next() {
                Some(f) => f.to_ascii_uppercase().to_string() + ch.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join("-")
}

fn display_header_name(rt: &mut Runtime, owner: ObjectRef, lower: &str) -> String {
    if let Value::Object(map) = rt.object_get(owner, HEADER_CASE_SLOT) {
        if let Value::String(s) = rt.object_get(map, lower) {
            return s.as_str().to_string();
        }
    }
    canonical_header_case(lower)
}

fn collect_wire_headers(
    rt: &mut Runtime,
    res: ObjectRef,
    slot: &str,
    headers: &mut Vec<(String, String)>,
) {
    if let Value::Object(hid) = rt.object_get(res, slot) {
        for key in rt.ordinary_own_enumerable_string_keys(hid) {
            let display = display_header_name(rt, res, &key);
            match rt.object_get(hid, &key) {
                Value::Undefined => continue,
                Value::Object(id) if is_js_array(rt, id) => {
                    let len = rt.array_length(id);
                    for i in 0..len {
                        let item = rt.object_get(id, &i.to_string());
                        if let Ok(v) = value_to_string(rt, &item) {
                            headers.push((display.clone(), v));
                        }
                    }
                }
                v => {
                    if let Ok(s) = value_to_string(rt, &v) {
                        headers.push((display.clone(), s));
                    }
                }
            }
        }
    }
}

fn build_stream_head(rt: &mut Runtime, res: ObjectRef) -> Vec<u8> {
    let status = match rt.object_get(res, "statusCode") {
        Value::Number(n) => n as u16,
        _ => 200,
    };
    let reason = match rt.object_get(res, "statusMessage") {
        Value::String(s) => s.as_str().to_string(),
        _ => status_reason(status).to_string(),
    };
    let mut headers: Vec<(String, String)> = Vec::new();
    collect_wire_headers(rt, res, HEADERS_SLOT, &mut headers);
    collect_wire_headers(rt, res, WH_WIRE_SLOT, &mut headers);
    maybe_push_date_header(rt, res, &mut headers);
    if !headers
        .iter()
        .any(|(n, _)| n.eq_ignore_ascii_case("connection"))
    {
        let keep = matches!(
            rt.object_get(res, ASYNC_KEEP_ALIVE_SLOT),
            Value::Boolean(true)
        );
        headers.push((
            "Connection".into(),
            if keep { "keep-alive" } else { "close" }.into(),
        ));
    }
    let has_framing = headers.iter().any(|(n, _)| {
        n.eq_ignore_ascii_case("content-length") || n.eq_ignore_ascii_case("transfer-encoding")
    });

    if !has_framing && !response_is_head(rt, res) {
        headers.push(("Transfer-Encoding".into(), "chunked".into()));
    }
    let mut s = format!("HTTP/1.1 {status} {reason}\r\n");
    for (k, v) in &headers {
        s.push_str(&format!("{k}: {v}\r\n"));
    }
    s.push_str("\r\n");
    s.into_bytes()
}

fn make_listener_record(rt: &mut Runtime, listener: Value, once: bool) -> ObjectRef {
    let record = new_object(rt);
    set_internal_slot(rt, record, "listener", listener);
    set_internal_slot(rt, record, REQUEST_ONCE_SLOT, Value::Boolean(once));
    record
}

fn request_listeners(rt: &mut Runtime, server: ObjectRef) -> ObjectRef {
    match rt.object_get(server, REQUEST_LISTENERS_SLOT) {
        Value::Object(id) => id,
        _ => {
            let arr = rt.alloc_object(rusty_js_runtime::Object::new_array());
            rt.object_set(arr, "length".into(), Value::Number(0.0));
            set_internal_slot(rt, server, REQUEST_LISTENERS_SLOT, Value::Object(arr));
            arr
        }
    }
}

fn add_request_listener(
    rt: &mut Runtime,
    server: ObjectRef,
    listener: Value,
    once: bool,
) -> Result<(), RuntimeError> {
    if !rt.is_callable(&listener) {
        return Err(RuntimeError::TypeError(
            "server.on: listener must be callable".into(),
        ));
    }
    let arr = request_listeners(rt, server);
    let len = rt.array_length(arr);
    let record = make_listener_record(rt, listener, once);
    rt.object_set(arr, len.to_string(), Value::Object(record));
    rt.object_set(arr, "length".into(), Value::Number((len + 1) as f64));
    Ok(())
}

fn prepend_request_listener(
    rt: &mut Runtime,
    server: ObjectRef,
    listener: Value,
    once: bool,
) -> Result<(), RuntimeError> {
    if !rt.is_callable(&listener) {
        return Err(RuntimeError::TypeError(
            "server.prependListener: listener must be callable".into(),
        ));
    }
    let arr = request_listeners(rt, server);
    let len = rt.array_length(arr);
    for i in (0..len).rev() {
        let v = rt.object_get(arr, &i.to_string());
        rt.object_set(arr, (i + 1).to_string(), v);
    }
    let record = make_listener_record(rt, listener, once);
    rt.object_set(arr, "0".into(), Value::Object(record));
    rt.object_set(arr, "length".into(), Value::Number((len + 1) as f64));
    Ok(())
}

const EVENT_LISTENERS_SLOT: &str = "__cruft_http_event_listeners";

fn add_named_listener(rt: &mut Runtime, server: ObjectRef, event: &str, listener: Value) {
    add_named_listener_at(rt, server, event, listener, false);
}

fn add_named_listener_at(
    rt: &mut Runtime,
    server: ObjectRef,
    event: &str,
    listener: Value,
    prepend: bool,
) {
    if !rt.is_callable(&listener) {
        return;
    }
    let map = match rt.object_get(server, EVENT_LISTENERS_SLOT) {
        Value::Object(id) => id,
        _ => {
            let m = new_object(rt);
            set_internal_slot(rt, server, EVENT_LISTENERS_SLOT, Value::Object(m));
            m
        }
    };
    let arr = match rt.object_get(map, event) {
        Value::Object(id) => id,
        _ => {
            let a = rt.alloc_object(rusty_js_runtime::Object::new_array());
            rt.object_set(a, "length".into(), Value::Number(0.0));
            rt.object_set(map, event.into(), Value::Object(a));
            a
        }
    };
    let len = rt.array_length(arr);
    if prepend {
        for i in (0..len).rev() {
            let v = rt.object_get(arr, &i.to_string());
            rt.object_set(arr, (i + 1).to_string(), v);
        }
        rt.object_set(arr, "0".into(), listener);
    } else {
        rt.object_set(arr, len.to_string(), listener);
    }
    rt.object_set(arr, "length".into(), Value::Number((len + 1) as f64));
}

fn emit_named_event(rt: &mut Runtime, server: ObjectRef, event: &str, args: Vec<Value>) {
    let map = match rt.object_get(server, EVENT_LISTENERS_SLOT) {
        Value::Object(id) => id,
        _ => return,
    };
    let arr = match rt.object_get(map, event) {
        Value::Object(id) => id,
        _ => return,
    };
    let len = rt.array_length(arr);
    let listeners: Vec<Value> = (0..len)
        .map(|i| rt.object_get(arr, &i.to_string()))
        .collect();
    for cb in listeners {
        if rt.is_callable(&cb) {
            let _ = rt.call_function(cb, Value::Object(server), args.clone());
        }
    }
}

fn current_server_id(rt: &mut Runtime) -> Result<usize, RuntimeError> {
    let this_id = match rt.current_this() {
        Value::Object(id) => id,
        _ => {
            return Err(RuntimeError::TypeError(
                "node:http Server method: invalid receiver".into(),
            ))
        }
    };
    match rt.object_get(this_id, SERVER_SLOT) {
        Value::Number(n) => Ok(n as usize),
        _ => Err(RuntimeError::TypeError(
            "node:http Server method: missing server id".into(),
        )),
    }
}

fn make_server_object(
    rt: &mut Runtime,
    handler: Value,
    net_cap: caps::Net,
) -> Result<ObjectRef, RuntimeError> {
    let server = new_object(rt);
    rt.obj_mut(server)
        .set_own_internal("__http_server__".into(), Value::Boolean(true));
    rt.object_set(server, "listening".into(), Value::Boolean(false));
    rt.object_set(server, "keepAliveTimeout".into(), Value::Number(5000.0));
    rt.object_set(server, "requestTimeout".into(), Value::Number(300000.0));
    rt.object_set(server, "timeout".into(), Value::Number(0.0));

    rt.object_set(server, "headersTimeout".into(), Value::Number(60000.0));
    rt.object_set(server, "maxRequestsPerSocket".into(), Value::Number(0.0));
    rt.object_set(server, "maxHeadersCount".into(), Value::Null);
    let listeners = rt.alloc_object(rusty_js_runtime::Object::new_array());
    rt.object_set(listeners, "length".into(), Value::Number(0.0));
    set_internal_slot(rt, server, REQUEST_LISTENERS_SLOT, Value::Object(listeners));
    if rt.is_callable(&handler) {
        add_request_listener(rt, server, handler.clone(), false)?;
    }

    register_method(rt, server, "listen", move |rt, args| {
        let this_id = match rt.current_this() {
            Value::Object(id) => id,
            _ => {
                return Err(RuntimeError::TypeError(
                    "server.listen: invalid receiver".into(),
                ))
            }
        };
        if matches!(rt.object_get(this_id, SERVER_SLOT), Value::Number(_)) {
            return Ok(Value::Object(this_id));
        }

        let opts = match args.first() {
            Some(Value::Object(id)) if !rt.is_callable(&Value::Object(*id)) => Some(*id),
            _ => None,
        };
        let (port, host) = if let Some(o) = opts {
            let port = match rt.object_get(o, "port") {
                Value::Number(n) => n as u16,
                Value::String(s) => s.parse::<u16>().unwrap_or(0),
                _ => 0,
            };
            let host = match rt.object_get(o, "host") {
                Value::String(s) => s.as_str().to_string(),
                _ => "127.0.0.1".to_string(),
            };
            (port, host)
        } else {
            let port = match args.first() {
                Some(Value::Number(n)) => *n as u16,
                Some(Value::String(s)) => s.parse::<u16>().unwrap_or(0),
                _ => 0,
            };
            let host = match args.get(1) {
                Some(Value::String(s)) => s.as_str().to_string(),
                _ => "127.0.0.1".to_string(),
            };
            (port, host)
        };
        let callback = args.iter().find(|v| rt.is_callable(v)).cloned();

        rt.caps
            .require_net(
                &net_cap,
                caps::NetOp::Listen {
                    host: host.clone(),
                    port,
                },
                &ModuleId::builtin("node:http"),
            )
            .map_err(|e| RuntimeError::TypeError(e.to_string()))?;

        let (listener_handle, bound_addr) =
            rusty_sockets::listener_bind_async(&format!("{host}:{port}"))
                .map_err(|e| RuntimeError::TypeError(format!("server.listen: {e:?}")))?;
        let server_id = next_server_id(ActiveHttpServer {
            agent_id: rt.agent_id(),
            listener_handle,
            bound_addr: bound_addr.clone(),
            handler_realm: rt.current_realm,
            server_object: this_id,
            refed: true,
        });

        rt.set_engine_sentinel(this_id, SERVER_SLOT, Value::Number(server_id as f64));
        rt.set_engine_sentinel(
            this_id,
            "__cruft_http_bound_addr",
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(bound_addr))),
        );
        rt.set_engine_sentinel(this_id, "_handle", Value::Boolean(true));
        rt.object_set(this_id, "listening".into(), Value::Boolean(true));

        let mut roots = vec![this_id];
        if let Some(Value::Object(cb_id)) = &callback {
            roots.push(*cb_id);
        }
        rt.enqueue_microtask_rooted("http.server.listening", roots, move |rt| {
            emit_named_event(rt, this_id, "listening", Vec::new());
            if let Some(cb) = callback {
                if rt.is_callable(&cb) {
                    rt.call_function(cb, Value::Object(this_id), Vec::new())?;
                }
            }
            Ok(())
        });
        Ok(Value::Object(this_id))
    });

    register_method(rt, server, "address", |rt, _args| {
        let this_id = match rt.current_this() {
            Value::Object(id) => id,
            _ => {
                return Err(RuntimeError::TypeError(
                    "node:http Server method: invalid receiver".into(),
                ))
            }
        };
        let id = match rt.object_get(this_id, SERVER_SLOT) {
            Value::Number(n) => n as usize,
            _ => return Ok(Value::Null),
        };
        let Some(server) = get_server_for_runtime(rt, id) else {
            return Ok(Value::Null);
        };
        let out = new_object(rt);
        let (host, port) = split_bound_addr(&server.bound_addr);
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

    register_method(rt, server, "close", |rt, args| {
        let this = rt.current_this();

        let id_opt = match &this {
            Value::Object(id) => match rt.object_get(*id, SERVER_SLOT) {
                Value::Number(n) => Some(n as usize),
                _ => None,
            },
            _ => None,
        };
        let mut was_running = false;
        if let Some(id) = id_opt {
            if let Some(server) = remove_server_for_runtime(rt, id) {
                was_running = true;
                let _ = rusty_sockets::listener_stop_async(server.listener_handle);
                rt.object_set(
                    server.server_object,
                    "listening".into(),
                    Value::Boolean(false),
                );
                rt.set_engine_sentinel(server.server_object, "_handle", Value::Null);
                emit_named_event(rt, server.server_object, "close", Vec::new());
            }
        }
        let cb = args.iter().find(|v| rt.is_callable(v)).cloned();
        if was_running {

            if let Some(cb) = cb {
                let _ = rt.call_function(cb, this.clone(), vec![Value::Undefined])?;
            }
        } else if let Value::Object(sid) = &this {

            let sid = *sid;
            let mut roots = vec![sid];
            if let Some(Value::Object(cb_id)) = &cb {
                roots.push(*cb_id);
            }
            rt.enqueue_host_phase_rooted(
                HostEnqueuePhase::HostCompletionMacrotask,
                "http.server.close",
                roots,
                move |rt| {
                    emit_named_event(rt, sid, "close", Vec::new());
                    if let Some(cb) = cb {
                        let err =
                            coded_error(rt, "ERR_SERVER_NOT_RUNNING", "Server is not running.");
                        let _ = rt.call_function(cb, Value::Object(sid), vec![err]);
                    }
                    Ok(())
                },
            );
        }
        Ok(this)
    });

    register_method(rt, server, "unref", |rt, _args| {
        let id = current_server_id(rt)?;
        let _ = set_server_refed(rt, id, false);
        Ok(rt.current_this())
    });
    register_method(rt, server, "ref", |rt, _args| {
        let id = current_server_id(rt)?;
        let _ = set_server_refed(rt, id, true);
        Ok(rt.current_this())
    });
    register_method(rt, server, "hasRef", |rt, _args| {
        let id = current_server_id(rt)?;
        let refed = get_server_for_runtime(rt, id)
            .map(|server| server.refed)
            .unwrap_or(false);
        Ok(Value::Boolean(refed))
    });

    register_method(rt, server, "setTimeout", |rt, args| {
        let this_id = match rt.current_this() {
            Value::Object(id) => id,
            _ => {
                return Err(RuntimeError::TypeError(
                    "server.setTimeout: invalid receiver".into(),
                ))
            }
        };
        if let Some(Value::Number(ms)) = args.first() {
            rt.object_set(this_id, "timeout".into(), Value::Number(*ms));
        }
        Ok(Value::Object(this_id))
    });

    register_method(rt, server, "on", |rt, args| {
        let this_id = match rt.current_this() {
            Value::Object(id) => id,
            _ => {
                return Err(RuntimeError::TypeError(
                    "server.on: invalid receiver".into(),
                ))
            }
        };
        let event = match args.first() {
            Some(v) => emitter_event_key(rt, v)?,
            None => String::new(),
        };
        if event == "request" {
            add_request_listener(
                rt,
                this_id,
                args.get(1).cloned().unwrap_or(Value::Undefined),
                false,
            )?;
        } else {
            add_named_listener(
                rt,
                this_id,
                &event,
                args.get(1).cloned().unwrap_or(Value::Undefined),
            );
        }
        Ok(Value::Object(this_id))
    });
    register_method(rt, server, "addListener", |rt, args| {
        let on = rt.object_get(
            match rt.current_this() {
                Value::Object(id) => id,
                _ => {
                    return Err(RuntimeError::TypeError(
                        "server.addListener: invalid receiver".into(),
                    ))
                }
            },
            "on",
        );
        rt.call_function(on, rt.current_this(), args.to_vec())
    });
    register_method(rt, server, "once", |rt, args| {
        let this_id = match rt.current_this() {
            Value::Object(id) => id,
            _ => {
                return Err(RuntimeError::TypeError(
                    "server.once: invalid receiver".into(),
                ))
            }
        };
        let event = match args.first() {
            Some(v) => emitter_event_key(rt, v)?,
            None => String::new(),
        };
        if event == "request" {
            add_request_listener(
                rt,
                this_id,
                args.get(1).cloned().unwrap_or(Value::Undefined),
                true,
            )?;
        } else {

            add_named_listener(
                rt,
                this_id,
                &event,
                args.get(1).cloned().unwrap_or(Value::Undefined),
            );
        }
        Ok(Value::Object(this_id))
    });
    register_method(rt, server, "prependListener", |rt, args| {
        let this_id = match rt.current_this() {
            Value::Object(id) => id,
            _ => {
                return Err(RuntimeError::TypeError(
                    "server.prependListener: invalid receiver".into(),
                ))
            }
        };
        let event = match args.first() {
            Some(v) => emitter_event_key(rt, v)?,
            None => String::new(),
        };
        if event == "request" {
            prepend_request_listener(
                rt,
                this_id,
                args.get(1).cloned().unwrap_or(Value::Undefined),
                false,
            )?;
        } else {
            add_named_listener_at(
                rt,
                this_id,
                &event,
                args.get(1).cloned().unwrap_or(Value::Undefined),
                true,
            );
        }
        Ok(Value::Object(this_id))
    });
    register_method(rt, server, "prependOnceListener", |rt, args| {
        let this_id = match rt.current_this() {
            Value::Object(id) => id,
            _ => {
                return Err(RuntimeError::TypeError(
                    "server.prependOnceListener: invalid receiver".into(),
                ))
            }
        };
        let event = match args.first() {
            Some(v) => emitter_event_key(rt, v)?,
            None => String::new(),
        };
        if event == "request" {
            prepend_request_listener(
                rt,
                this_id,
                args.get(1).cloned().unwrap_or(Value::Undefined),
                true,
            )?;
        } else {
            add_named_listener_at(
                rt,
                this_id,
                &event,
                args.get(1).cloned().unwrap_or(Value::Undefined),
                true,
            );
        }
        Ok(Value::Object(this_id))
    });

    for m in ["removeListener", "off", "removeAllListeners"] {
        register_method(rt, server, m, |rt, _a| Ok(rt.current_this()));
    }
    register_method(rt, server, "emit", |rt, args| {
        let this_id = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Boolean(false)),
        };
        let event = match args.first() {
            Some(v) => value_to_string(rt, v)?,
            None => return Ok(Value::Boolean(false)),
        };
        emit_named_event(
            rt,
            this_id,
            &event,
            args.get(1..).map(|s| s.to_vec()).unwrap_or_default(),
        );
        Ok(Value::Boolean(true))
    });

    rt.set_engine_sentinel(server, "__cruft_http_handler", handler);

    rt.materialize_lazy_host_module("net");
    let ee_proto = match rt.global_get("events") {
        Value::Object(e) => match rt.object_get(e, "prototype") {
            Value::Object(p) => Some(p),
            _ => None,
        },
        _ => None,
    };
    if let (Value::Object(hsp), Value::Object(nsp)) = (
        rt.global_get("__cruft_http_server_proto"),
        rt.global_get("__cruft_net_server_proto"),
    ) {
        if let Some(eep) = ee_proto {
            if !matches!(rt.obj(nsp).proto, Some(p) if p == eep) {
                rt.set_object_prototype_internal(nsp, Some(eep));
            }
        }
        if !matches!(rt.obj(hsp).proto, Some(p) if p == nsp) {
            rt.set_object_prototype_internal(hsp, Some(nsp));
        }
        rt.set_object_prototype_internal(server, Some(hsp));
    }
    Ok(server)
}

fn split_bound_addr(addr: &str) -> (String, u16) {
    match addr.rsplit_once(':') {
        Some((host, port)) => (
            host.trim_matches(['[', ']']).to_string(),
            port.parse().unwrap_or(0),
        ),
        None => (addr.to_string(), 0),
    }
}

fn make_request_object(rt: &mut Runtime, req: &rusty_http_codec::ParsedRequest) -> ObjectRef {
    let obj = new_object(rt);
    rt.object_set(
        obj,
        "method".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            req.method.clone(),
        ))),
    );
    rt.object_set(
        obj,
        "url".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            req.target.clone(),
        ))),
    );
    rt.object_set(
        obj,
        "httpVersion".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            req.version.trim_start_matches("HTTP/").to_string(),
        ))),
    );

    let ver = req.version.trim_start_matches("HTTP/");
    let (major, minor) = match ver.split_once('.') {
        Some((a, b)) => (
            a.parse::<f64>().unwrap_or(1.0),
            b.parse::<f64>().unwrap_or(1.0),
        ),
        None => (1.0, 1.0),
    };
    rt.object_set(obj, "httpVersionMajor".into(), Value::Number(major));
    rt.object_set(obj, "httpVersionMinor".into(), Value::Number(minor));
    rt.object_set(obj, "aborted".into(), Value::Boolean(false));
    rt.object_set(obj, "complete".into(), Value::Boolean(false));

    let trailers = new_object(rt);
    for (name, value) in &req.trailers {
        rt.object_set(
            trailers,
            name.to_ascii_lowercase(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                value.clone(),
            ))),
        );
    }
    rt.object_set(obj, "trailers".into(), Value::Object(trailers));
    let raw_trailers = rt.alloc_object(rusty_js_runtime::Object::new_array());
    let mut rt_len = 0.0;
    for (name, value) in &req.trailers {
        rt.object_set(
            raw_trailers,
            rt_len.to_string(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                name.clone(),
            ))),
        );
        rt.object_set(
            raw_trailers,
            (rt_len + 1.0).to_string(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                value.clone(),
            ))),
        );
        rt_len += 2.0;
    }
    rt.object_set(raw_trailers, "length".into(), Value::Number(rt_len));
    rt.object_set(obj, "rawTrailers".into(), Value::Object(raw_trailers));
    let headers = new_object(rt);
    for (name, value) in &req.headers {
        rt.object_set(
            headers,
            name.to_ascii_lowercase(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                value.clone(),
            ))),
        );
    }
    rt.object_set(obj, "headers".into(), Value::Object(headers));

    let raw_headers = rt.alloc_object(rusty_js_runtime::Object::new_array());
    let mut raw_idx = 0usize;
    for (name, value) in &req.headers {
        rt.object_set(
            raw_headers,
            raw_idx.to_string(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                name.clone(),
            ))),
        );
        raw_idx += 1;
        rt.object_set(
            raw_headers,
            raw_idx.to_string(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                value.clone(),
            ))),
        );
        raw_idx += 1;
    }
    rt.object_set(raw_headers, "length".into(), Value::Number(raw_idx as f64));
    rt.object_set(obj, "rawHeaders".into(), Value::Object(raw_headers));

    let body_buf = http_buffer_from_bytes(rt, &req.body);
    rt.set_engine_sentinel(obj, REQ_BODY_BUF_SLOT, body_buf);
    let data_listeners = rt.alloc_object(rusty_js_runtime::Object::new_array());
    rt.object_set(data_listeners, "length".into(), Value::Number(0.0));
    rt.set_engine_sentinel(obj, REQ_DATA_LISTENERS_SLOT, Value::Object(data_listeners));
    let end_listeners = rt.alloc_object(rusty_js_runtime::Object::new_array());
    rt.object_set(end_listeners, "length".into(), Value::Number(0.0));
    rt.set_engine_sentinel(obj, REQ_END_LISTENERS_SLOT, Value::Object(end_listeners));
    let close_listeners = rt.alloc_object(rusty_js_runtime::Object::new_array());
    rt.object_set(close_listeners, "length".into(), Value::Number(0.0));
    rt.set_engine_sentinel(
        obj,
        REQ_CLOSE_LISTENERS_SLOT,
        Value::Object(close_listeners),
    );

    let on_impl = |rt: &mut Runtime, args: &[Value]| -> Result<Value, RuntimeError> {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let event = match args.first() {
            Some(v) => emitter_event_key(rt, v)?,
            None => String::new(),
        };
        let listener = args.get(1).cloned().unwrap_or(Value::Undefined);
        if !rt.is_callable(&listener) {
            return Ok(Value::Object(this));
        }
        let slot = match event.as_str() {
            "data" => REQ_DATA_LISTENERS_SLOT,
            "end" => REQ_END_LISTENERS_SLOT,
            "close" => REQ_CLOSE_LISTENERS_SLOT,
            _ => return Ok(Value::Object(this)),
        };
        if let Value::Object(arr) = rt.object_get(this, slot) {
            let len = rt.array_length(arr);
            rt.object_set(arr, len.to_string(), listener);
            rt.object_set(arr, "length".into(), Value::Number((len + 1) as f64));
        }

        schedule_request_body_delivery(rt, this);
        Ok(Value::Object(this))
    };
    register_method(rt, obj, "on", on_impl);
    register_method(rt, obj, "addListener", on_impl);
    register_method(rt, obj, "once", on_impl);

    register_method(rt, obj, "resume", |rt, _args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        schedule_request_body_delivery(rt, this);
        Ok(Value::Object(this))
    });
    register_method(rt, obj, "pause", |rt, _args| Ok(rt.current_this()));
    register_method(rt, obj, "setEncoding", |rt, _args| Ok(rt.current_this()));

    register_method(rt, obj, "unpipe", |rt, _args| Ok(rt.current_this()));
    register_method(rt, obj, "isPaused", |_rt, _args| Ok(Value::Boolean(false)));
    register_method(rt, obj, "pipe", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let dest = args.first().cloned().unwrap_or(Value::Undefined);
        let s = |rt: &mut Runtime, x: &str| {
            let _ = rt;
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(x)))
        };
        if let Value::Object(dest_id) = dest {
            let on = rt.object_get(this, "on");
            if rt.is_callable(&on) {
                let on_data =
                    crate::register::make_callable(rt, "req.pipe.onData", move |rt, a| {
                        let chunk = a.first().cloned().unwrap_or(Value::Undefined);
                        let w = rt.object_get(dest_id, "write");
                        if rt.is_callable(&w) {
                            let _ = rt.call_function(w, Value::Object(dest_id), vec![chunk]);
                        }
                        Ok(Value::Undefined)
                    });
                let ev_data = s(rt, "data");
                let _ = rt.call_function(
                    on.clone(),
                    Value::Object(this),
                    vec![ev_data, Value::Object(on_data)],
                );
                let on_end = crate::register::make_callable(rt, "req.pipe.onEnd", move |rt, _a| {
                    let e = rt.object_get(dest_id, "end");
                    if rt.is_callable(&e) {
                        let _ = rt.call_function(e, Value::Object(dest_id), Vec::new());
                    }
                    Ok(Value::Undefined)
                });
                let ev_end = s(rt, "end");
                let _ =
                    rt.call_function(on, Value::Object(this), vec![ev_end, Value::Object(on_end)]);
            }
        }
        Ok(dest)
    });

    for m in ["removeListener", "off", "removeAllListeners"] {
        register_method(rt, obj, m, |rt, _a| Ok(rt.current_this()));
    }

    rt.object_set(obj, "destroyed".into(), Value::Boolean(false));
    register_method(rt, obj, "destroy", |rt, _a| {
        if let Value::Object(t) = rt.current_this() {
            rt.object_set(t, "destroyed".into(), Value::Boolean(true));
        }
        Ok(rt.current_this())
    });

    link_incoming_prototype(rt, obj);

    crate::stream::install_async_iterator(rt, obj);
    obj
}

fn emit_request_close(rt: &mut Runtime, req: ObjectRef) -> Result<(), RuntimeError> {
    let arr = match rt.object_get(req, REQ_CLOSE_LISTENERS_SLOT) {
        Value::Object(arr) => arr,
        _ => return Ok(()),
    };
    let len = rt.array_length(arr);
    for i in 0..len {
        let cb = rt.object_get(arr, &i.to_string());
        if rt.is_callable(&cb) {
            rt.call_function(cb, Value::Object(req), Vec::new())?;
        }
    }
    Ok(())
}

fn drive_request_body(rt: &mut Runtime, req: ObjectRef) -> Result<(), RuntimeError> {

    if let Value::Boolean(true) = rt.object_get(req, REQ_BODY_DELIVERED_SLOT) {
        return Ok(());
    }

    let data_count = match rt.object_get(req, REQ_DATA_LISTENERS_SLOT) {
        Value::Object(arr) => rt.array_length(arr),
        _ => 0,
    };
    let end_count = match rt.object_get(req, REQ_END_LISTENERS_SLOT) {
        Value::Object(arr) => rt.array_length(arr),
        _ => 0,
    };
    if data_count == 0 && end_count == 0 {
        return Ok(());
    }

    rt.set_engine_sentinel(req, REQ_BODY_DELIVERED_SLOT, Value::Boolean(true));

    let body = rt.object_get(req, REQ_BODY_BUF_SLOT);
    let body_len = match &body {
        Value::Object(id) => match rt.object_get(*id, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        },
        _ => 0,
    };
    if body_len > 0 {
        if let Value::Object(arr) = rt.object_get(req, REQ_DATA_LISTENERS_SLOT) {
            let len = rt.array_length(arr);
            for i in 0..len {
                let cb = rt.object_get(arr, &i.to_string());
                if rt.is_callable(&cb) {
                    rt.call_function(cb, Value::Object(req), vec![body.clone()])?;
                }
            }
        }
    }

    rt.object_set(req, "complete".into(), Value::Boolean(true));
    if let Value::Object(arr) = rt.object_get(req, REQ_END_LISTENERS_SLOT) {
        let len = rt.array_length(arr);
        for i in 0..len {
            let cb = rt.object_get(arr, &i.to_string());
            if rt.is_callable(&cb) {
                rt.call_function(cb, Value::Object(req), Vec::new())?;
            }
        }
    }
    Ok(())
}

fn schedule_request_body_delivery(rt: &mut Runtime, req: ObjectRef) {
    rt.enqueue_microtask_rooted("http.req.flow", vec![req], move |rt| {
        drive_request_body(rt, req)
    });
}

fn ensure_response_proto(rt: &mut Runtime) -> ObjectRef {
    if let Value::Object(p) = rt.global_get("__cruft_http_res_proto") {
        return p;
    }
    let proto = new_object(rt);
    register_method(rt, proto, "setHeader", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        if let Some(e) = headers_sent_error(rt, this, "set") {
            return Err(e);
        }
        let headers = match rt.object_get(this, HEADERS_SLOT) {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let original = match args.first() {
            Some(v) => value_to_string(rt, v)?,
            None => String::new(),
        };
        record_header_case(rt, this, &original);
        let name = original.to_ascii_lowercase();

        let value = match args.get(1) {
            Some(v @ Value::Object(id)) if is_js_array(rt, *id) => v.clone(),
            Some(v) => Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                value_to_string(rt, v)?,
            ))),
            None => Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                String::new(),
            ))),
        };
        rt.object_set(headers, name, value);
        Ok(rt.current_this())
    });
    register_method(rt, proto, "getHeader", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let headers = match rt.object_get(this, HEADERS_SLOT) {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let name = match args.first() {
            Some(v) => value_to_string(rt, v)?,
            None => String::new(),
        }
        .to_ascii_lowercase();
        Ok(rt.object_get(headers, &name))
    });
    register_method(rt, proto, "removeHeader", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        if let Some(e) = headers_sent_error(rt, this, "remove") {
            return Err(e);
        }
        let headers = match rt.object_get(this, HEADERS_SLOT) {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let name = match args.first() {
            Some(v) => value_to_string(rt, v)?,
            None => String::new(),
        }
        .to_ascii_lowercase();

        let _ = rt.delete_own_via(
            &Value::Object(headers),
            &Value::String(Rc::new(rusty_js_runtime::value::JsString::from(name))),
        );
        Ok(rt.current_this())
    });
    register_method(rt, proto, "writeHead", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };

        if let Some(e) = headers_sent_error(rt, this, "write") {
            return Err(e);
        }
        if let Some(Value::Number(n)) = args.first() {
            rt.object_set(this, "statusCode".into(), Value::Number(*n));
        }
        let header_arg = if let Some(Value::String(s)) = args.get(1) {
            rt.object_set(this, "statusMessage".into(), Value::String(s.clone()));
            args.get(2).cloned()
        } else {

            let code = match rt.object_get(this, "statusCode") {
                Value::Number(n) => n as u16,
                _ => 200,
            };
            rt.object_set(
                this,
                "statusMessage".into(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    status_reason(code),
                ))),
            );
            args.get(1).cloned()
        };
        if let Some(Value::Object(hid)) = header_arg {
            let headers = match rt.object_get(this, HEADERS_SLOT) {
                Value::Object(id) => id,
                _ => return Ok(Value::Undefined),
            };

            let had_prior = !rt.ordinary_own_enumerable_string_keys(headers).is_empty();
            let target = if had_prior {
                headers
            } else {
                let wire = new_object(rt);
                rt.set_engine_sentinel(this, WH_WIRE_SLOT, Value::Object(wire));
                wire
            };
            for key in rt.ordinary_own_enumerable_string_keys(hid) {
                let raw = rt.object_get(hid, &key);
                record_header_case(rt, this, &key);

                let value = match &raw {
                    Value::Object(id) if is_js_array(rt, *id) => raw.clone(),
                    other => Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                        value_to_string(rt, other)?,
                    ))),
                };
                rt.object_set(target, key.to_ascii_lowercase(), value);
            }
        }

        rt.object_set(this, "headersSent".into(), Value::Boolean(true));

        rt.set_engine_sentinel(this, "__cruft_http_writehead", Value::Boolean(true));
        Ok(rt.current_this())
    });
    register_method(rt, proto, "write", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };

        if let Some(sid) = res_stream_id(rt, this) {
            if !matches!(rt.object_get(this, STREAM_HDR_SENT), Value::Boolean(true)) {
                let head = build_stream_head(rt, this);
                let _ = rusty_sockets::stream_write_all(sid, &head);
                rt.set_engine_sentinel(this, STREAM_HDR_SENT, Value::Boolean(true));
                rt.object_set(this, "headersSent".into(), Value::Boolean(true));
            }
            if let Some(chunk) = args.first() {
                let bytes = value_to_bytes(rt, chunk)?;
                if !bytes.is_empty() {
                    let _ = rusty_sockets::stream_write_all(sid, &chunk_frame(&bytes));
                }
            }
        } else if let Some(chunk) = args.first() {
            append_body_chunk(rt, this, chunk)?;
        }

        rt.set_engine_sentinel(this, "__cruft_http_streamed", Value::Boolean(true));
        Ok(Value::Boolean(true))
    });
    register_method(rt, proto, "end", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };

        let streaming = matches!(rt.object_get(this, STREAM_HDR_SENT), Value::Boolean(true));
        if let (Some(sid), true) = (res_stream_id(rt, this), streaming) {
            if let Some(chunk) = args.first() {
                let bytes = value_to_bytes(rt, chunk)?;
                if !bytes.is_empty() {
                    let _ = rusty_sockets::stream_write_all(sid, &chunk_frame(&bytes));
                }
            }
            let term = chunked_terminator(rt, this);
            let _ = rusty_sockets::stream_write_all(sid, &term);
        } else if let Some(chunk) = args.first() {
            append_body_chunk(rt, this, chunk)?;
        }
        rt.object_set(this, "writableEnded".into(), Value::Boolean(true));
        rt.object_set(this, "writableFinished".into(), Value::Boolean(true));
        rt.object_set(this, "finished".into(), Value::Boolean(true));
        rt.object_set(this, "headersSent".into(), Value::Boolean(true));
        rt.set_engine_sentinel(this, "__cruft_http_ended", Value::Boolean(true));
        if matches!(
            rt.object_get(this, ASYNC_BUFFERED_SLOT),
            Value::Boolean(true)
        ) {
            if let Some(sid) = res_stream_id(rt, this) {
                let keep = matches!(
                    rt.object_get(this, ASYNC_KEEP_ALIVE_SLOT),
                    Value::Boolean(true)
                );
                if let Ok(resp) = response_to_wire(rt, this, keep) {
                    let _ = rusty_sockets::stream_write_all(sid, &resp);
                }
            }
        }

        emit_res_event(rt, this, "finish", vec![])?;
        emit_res_event(rt, this, "close", vec![])?;
        if let Value::Object(req) = rt.object_get(this, SERVER_RESPONSE_REQUEST_SLOT) {
            emit_request_close(rt, req)?;
            emit_http_resource_destroy_once(rt, req)?;
        }
        Ok(Value::Undefined)
    });

    let res_on = |rt: &mut Runtime, args: &[Value]| -> Result<Value, RuntimeError> {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let event = match args.first() {
            Some(v) => emitter_event_key(rt, v)?,
            None => String::new(),
        };
        let listener = args.get(1).cloned().unwrap_or(Value::Undefined);
        if rt.is_callable(&listener) {
            let listeners = match rt.object_get(this, RES_LISTENERS_SLOT) {
                Value::Object(id) => id,
                _ => {
                    let o = new_object(rt);
                    rt.set_engine_sentinel(this, RES_LISTENERS_SLOT, Value::Object(o));
                    o
                }
            };
            let arr = match rt.object_get(listeners, &event) {
                Value::Object(a) => a,
                _ => {
                    let a = rt.alloc_object(rusty_js_runtime::Object::new_array());
                    rt.object_set(listeners, event.clone(), Value::Object(a));
                    a
                }
            };
            let len = rt.array_length(arr);
            rt.object_set(arr, len.to_string(), listener);
            rt.object_set(arr, "length".into(), Value::Number((len + 1) as f64));
        }
        Ok(Value::Object(this))
    };
    register_method(rt, proto, "on", res_on);
    register_method(rt, proto, "once", res_on);
    register_method(rt, proto, "addListener", res_on);
    register_method(rt, proto, "prependListener", res_on);
    register_method(rt, proto, "removeListener", |rt, _a| Ok(rt.current_this()));
    register_method(rt, proto, "removeAllListeners", |rt, _a| {
        Ok(rt.current_this())
    });
    register_method(rt, proto, "emit", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Boolean(false)),
        };
        let event = match args.first() {
            Some(v) => value_to_string(rt, v)?,
            None => String::new(),
        };
        let rest: Vec<Value> = if args.len() > 1 {
            args[1..].to_vec()
        } else {
            vec![]
        };
        Ok(Value::Boolean(emit_res_event(rt, this, &event, rest)?))
    });
    register_method(rt, proto, "listeners", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let event = match args.first() {
            Some(v) => value_to_string(rt, v)?,
            None => String::new(),
        };
        if let Value::Object(listeners) = rt.object_get(this, RES_LISTENERS_SLOT) {
            if let Value::Object(a) = rt.object_get(listeners, &event) {
                return Ok(Value::Object(a));
            }
        }
        Ok(Value::Object(
            rt.alloc_object(rusty_js_runtime::Object::new_array()),
        ))
    });

    register_method(rt, proto, "addTrailers", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        if let Some(v) = args.first() {
            rt.object_set(this, "__trailers".into(), v.clone());
        }
        Ok(Value::Undefined)
    });
    register_method(rt, proto, "flushHeaders", |_rt, _a| Ok(Value::Undefined));

    register_method(rt, proto, "setTimeout", |rt, args| {

        let this = rt.current_this();
        if let (Value::Object(id), Some(cb)) = (&this, args.get(1)) {
            if rt.is_callable(cb) {
                rt.object_set(*id, "__timeout_cb".into(), cb.clone());
            }
        }
        Ok(this)
    });
    register_method(rt, proto, "writeContinue", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, proto, "writeProcessing", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, proto, "writeEarlyHints", |rt, args| {

        if let Some(cb) = args.get(1) {
            if rt.is_callable(cb) {
                let cb = cb.clone();
                let roots = crate::timer::roots_for_callback(&cb, &[]);
                rt.enqueue_host_phase_rooted(
                    HostEnqueuePhase::HostCompletionMacrotask,
                    "writeEarlyHints callback",
                    roots,
                    move |rt| {
                        let _ = rt.call_function(cb, Value::Undefined, vec![]);
                        Ok(())
                    },
                );
            }
        }
        Ok(Value::Undefined)
    });
    register_method(rt, proto, "assignSocket", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, proto, "detachSocket", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, proto, "_implicitHeader", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, proto, "cork", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, proto, "uncork", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, proto, "hasHeader", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Boolean(false)),
        };
        let name = match args.first() {
            Some(v) => value_to_string(rt, v)?.to_lowercase(),
            None => return Ok(Value::Boolean(false)),
        };
        if let Value::Object(h) = rt.object_get(this, HEADERS_SLOT) {
            return Ok(Value::Boolean(!matches!(
                rt.object_get(h, &name),
                Value::Undefined
            )));
        }
        Ok(Value::Boolean(false))
    });
    register_method(rt, proto, "getHeaderNames", |rt, _a| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let out = rt.alloc_object(rusty_js_runtime::Object::new_array());
        if let Value::Object(h) = rt.object_get(this, HEADERS_SLOT) {
            for (i, key) in rt
                .ordinary_own_enumerable_string_keys(h)
                .into_iter()
                .enumerate()
            {
                rt.object_set(
                    out,
                    i.to_string(),
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(key))),
                );
            }
        }
        Ok(Value::Object(out))
    });

    register_method(rt, proto, "getRawHeaderNames", |rt, _a| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let case_map = match rt.object_get(this, HEADER_CASE_SLOT) {
            Value::Object(id) => Some(id),
            _ => None,
        };
        let out = rt.alloc_object(rusty_js_runtime::Object::new_array());
        if let Value::Object(h) = rt.object_get(this, HEADERS_SLOT) {
            for (i, key) in rt
                .ordinary_own_enumerable_string_keys(h)
                .into_iter()
                .enumerate()
            {
                let raw = case_map
                    .and_then(|m| match rt.object_get(m, &key) {
                        Value::String(s) => Some(s.as_str().to_string()),
                        _ => None,
                    })
                    .unwrap_or_else(|| key.clone());
                rt.object_set(
                    out,
                    i.to_string(),
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(raw))),
                );
            }
        }
        Ok(Value::Object(out))
    });

    register_method(rt, proto, "getHeaders", |rt, _a| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let out = new_object(rt);

        rt.set_object_prototype_internal(out, None);
        if let Value::Object(h) = rt.object_get(this, HEADERS_SLOT) {
            for key in rt.ordinary_own_enumerable_string_keys(h) {
                let v = rt.object_get(h, &key);
                if !matches!(v, Value::Undefined) {
                    rt.object_set(out, key, v);
                }
            }
        }
        Ok(Value::Object(out))
    });

    register_method(rt, proto, "appendHeader", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        if let Some(e) = headers_sent_error(rt, this, "append") {
            return Err(e);
        }
        let headers = match rt.object_get(this, HEADERS_SLOT) {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let original = match args.first() {
            Some(v) => value_to_string(rt, v)?,
            None => String::new(),
        };
        record_header_case(rt, this, &original);
        let name = original.to_ascii_lowercase();
        let new_val = match args.get(1) {
            Some(v) => Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                value_to_string(rt, v)?,
            ))),
            None => Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                String::new(),
            ))),
        };
        match rt.object_get(headers, &name) {
            Value::Undefined => {
                rt.object_set(headers, name, new_val);
            }
            Value::Object(eid) if is_js_array(rt, eid) => {
                let len = match rt.object_get(eid, "length") {
                    Value::Number(n) => n as usize,
                    _ => 0,
                };
                rt.object_set(eid, len.to_string(), new_val);
                rt.object_set(eid, "length".into(), Value::Number((len + 1) as f64));
            }
            other => {
                let arr = rt.alloc_object(rusty_js_runtime::Object::new_array());
                rt.object_set(arr, "0".into(), other);
                rt.object_set(arr, "1".into(), new_val);
                rt.object_set(arr, "length".into(), Value::Number(2.0));
                rt.object_set(headers, name, Value::Object(arr));
            }
        }
        Ok(rt.current_this())
    });

    register_method(rt, proto, "setHeaders", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        if let Some(e) = headers_sent_error(rt, this, "set") {
            return Err(e);
        }
        let arg = args.first().cloned().unwrap_or(Value::Undefined);
        let for_each = match &arg {
            Value::Object(id) => rt.object_get(*id, "forEach"),
            _ => Value::Undefined,
        };
        if !rt.is_callable(&for_each) {
            return Err(RuntimeError::TypeError(
                "The \"headers\" argument must be an instance of Headers or Map".to_string(),
            ));
        }
        let cb = crate::register::make_callable_rooted(
            rt,
            "ServerResponse.setHeaders.cb",
            vec![this],
            move |rt, cbargs| {
                let value = cbargs.first().cloned().unwrap_or(Value::Undefined);
                let key = cbargs.get(1).cloned().unwrap_or(Value::Undefined);
                let headers = match rt.object_get(this, HEADERS_SLOT) {
                    Value::Object(id) => id,
                    _ => return Ok(Value::Undefined),
                };
                let original = value_to_string(rt, &key)?;
                record_header_case(rt, this, &original);
                let name = original.to_ascii_lowercase();

                let stored = match &value {
                    Value::Object(id) if is_js_array(rt, *id) => value.clone(),
                    other => Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                        value_to_string(rt, other)?,
                    ))),
                };
                rt.object_set(headers, name, stored);
                Ok(Value::Undefined)
            },
        );
        rt.call_function(for_each, arg.clone(), vec![Value::Object(cb)])?;
        Ok(rt.current_this())
    });
    rt.define_global_property("__cruft_http_res_proto", Value::Object(proto));
    proto
}

fn make_response_object(rt: &mut Runtime) -> ObjectRef {
    let obj = new_object(rt);
    let headers = new_object(rt);
    rt.object_set(obj, "statusCode".into(), Value::Number(200.0));

    rt.object_set(obj, "statusMessage".into(), Value::Undefined);
    rt.object_set(obj, "headersSent".into(), Value::Boolean(false));

    rt.object_set(obj, "sendDate".into(), Value::Boolean(true));
    rt.object_set(obj, "strictContentLength".into(), Value::Boolean(false));
    rt.object_set(obj, "writableFinished".into(), Value::Boolean(false));

    rt.set_engine_sentinel(obj, HEADERS_SLOT, Value::Object(headers));
    rt.set_engine_sentinel(
        obj,
        BODY_SLOT,
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            String::new(),
        ))),
    );

    let __res_proto = ensure_response_proto(rt);
    rt.set_object_prototype_internal(obj, Some(__res_proto));
    let res_listeners = new_object(rt);
    rt.set_engine_sentinel(obj, RES_LISTENERS_SLOT, Value::Object(res_listeners));
    rt.object_set(obj, "writableEnded".into(), Value::Boolean(false));
    rt.object_set(obj, "finished".into(), Value::Boolean(false));

    let socket = new_object(rt);
    rt.object_set(obj, "socket".into(), Value::Object(socket));
    rt.object_set(obj, "connection".into(), Value::Object(socket));

    obj
}

fn make_constructed_incoming_message(rt: &mut Runtime) -> ObjectRef {
    let req = new_object(rt);
    rt.object_set(req, "method".into(), Value::Null);
    rt.object_set(
        req,
        "url".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            String::new(),
        ))),
    );
    let headers = new_object(rt);
    rt.object_set(req, "headers".into(), Value::Object(headers));
    let raw_headers = rt.alloc_object(rusty_js_runtime::Object::new_array());
    rt.object_set(req, "rawHeaders".into(), Value::Object(raw_headers));
    rt.object_set(req, "httpVersion".into(), Value::Null);
    rt.object_set(req, "complete".into(), Value::Boolean(false));
    rt.object_set(req, "readable".into(), Value::Boolean(true));
    rt.object_set(req, "readableEnded".into(), Value::Boolean(false));
    rt.object_set(req, "statusCode".into(), Value::Null);
    rt.object_set(req, "statusMessage".into(), Value::Null);
    let socket = new_object(rt);
    rt.object_set(req, "socket".into(), Value::Object(socket));
    rt.object_set(req, "connection".into(), Value::Object(socket));
    let listeners = new_object(rt);
    rt.set_engine_sentinel(req, RES_LISTENERS_SLOT, Value::Object(listeners));
    attach_emitter(rt, req);
    register_method(rt, req, "emit", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Boolean(false)),
        };
        let event = match args.first() {
            Some(v) => value_to_string(rt, v)?,
            None => return Ok(Value::Boolean(false)),
        };
        let rest = args.get(1..).map(|s| s.to_vec()).unwrap_or_default();
        Ok(Value::Boolean(emit_res_event(rt, this, &event, rest)?))
    });
    for noop in ["resume", "pause", "setEncoding", "destroy"] {
        register_method(rt, req, noop, |rt, _a| Ok(rt.current_this()));
    }
    req
}

fn request_keep_alive(parsed: &rusty_http_codec::ParsedRequest) -> bool {
    let conn = parsed
        .headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("connection"))
        .map(|(_, v)| v.trim().to_ascii_lowercase());
    match conn.as_deref() {
        Some(c) if c.contains("close") => false,
        Some(c) if c.contains("keep-alive") => true,
        _ => parsed.version.trim() == "HTTP/1.1",
    }
}

fn request_is_upgrade(parsed: &rusty_http_codec::ParsedRequest) -> bool {
    let has_upgrade_token = parsed
        .headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("connection"))
        .map(|(_, v)| {
            v.split(',')
                .any(|part| part.trim().eq_ignore_ascii_case("upgrade"))
        })
        .unwrap_or(false);
    let has_upgrade_header = parsed
        .headers
        .iter()
        .any(|(n, v)| n.eq_ignore_ascii_case("upgrade") && !v.trim().is_empty());
    has_upgrade_token && has_upgrade_header
}

fn response_says_close(resp: &[u8]) -> bool {
    let head_end = resp
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| i + 4)
        .unwrap_or(resp.len());
    String::from_utf8_lossy(&resp[..head_end])
        .lines()
        .any(|l| match l.split_once(':') {
            Some((n, v)) => {
                n.trim().eq_ignore_ascii_case("connection")
                    && v.trim().to_ascii_lowercase().contains("close")
            }
            None => false,
        })
}

fn response_to_wire(
    rt: &mut Runtime,
    res: ObjectRef,
    keep_alive: bool,
) -> Result<Vec<u8>, RuntimeError> {
    let status = match rt.object_get(res, "statusCode") {
        Value::Number(n) => n as u16,
        _ => 200,
    };
    let reason = match rt.object_get(res, "statusMessage") {
        Value::String(s) => s.as_str().to_string(),
        _ => status_reason(status).to_string(),
    };
    let body = match rt.object_get(res, BODY_SLOT) {
        Value::String(s) => s.as_bytes().to_vec(),
        v => value_to_bytes(rt, &v)?,
    };
    let mut headers = Vec::new();
    collect_wire_headers(rt, res, HEADERS_SLOT, &mut headers);
    collect_wire_headers(rt, res, WH_WIRE_SLOT, &mut headers);
    maybe_push_date_header(rt, res, &mut headers);
    if !headers
        .iter()
        .any(|(n, _)| n.eq_ignore_ascii_case("connection"))
    {
        headers.push((
            "Connection".into(),
            if keep_alive { "keep-alive" } else { "close" }.into(),
        ));
    }

    if response_is_head(rt, res) || matches!(status, 204 | 304) {
        return rusty_http_codec::try_serialize_response_head(status, &reason, &headers)
            .map_err(|e| RuntimeError::TypeError(e.to_string()));
    }

    let streamed = matches!(
        rt.object_get(res, "__cruft_http_streamed"),
        Value::Boolean(true)
    ) || matches!(
        rt.object_get(res, "__cruft_http_writehead"),
        Value::Boolean(true)
    );
    let has_cl = headers
        .iter()
        .any(|(n, _)| n.eq_ignore_ascii_case("content-length"));
    if streamed && !has_cl {
        headers.push(("Transfer-Encoding".into(), "chunked".into()));
        let mut chunked = Vec::new();
        if !body.is_empty() {
            chunked.extend_from_slice(&chunk_frame(&body));
        }
        chunked.extend_from_slice(&chunked_terminator(rt, res));
        return rusty_http_codec::try_serialize_response(status, &reason, &headers, &chunked)
            .map_err(|e| RuntimeError::TypeError(e.to_string()));
    }
    rusty_http_codec::try_serialize_response(status, &reason, &headers, &body)
        .map_err(|e| RuntimeError::TypeError(e.to_string()))
}

fn response_is_head(rt: &mut Runtime, res: ObjectRef) -> bool {
    if let Value::Object(req) = rt.object_get(res, "req") {
        if let Value::String(m) = rt.object_get(req, "method") {
            return m.as_str().eq_ignore_ascii_case("HEAD");
        }
    }
    false
}

struct ReadRequest {
    request: Vec<u8>,
    leftover: Vec<u8>,
}

fn read_request(stream_id: u64, idle: Duration, mut buf: Vec<u8>) -> Result<ReadRequest, String> {

    let _ = rusty_sockets::stream_set_nonblocking(stream_id, true);
    let start = Instant::now();
    let overall = Duration::from_millis(500);

    let mut expect_100_handled = false;
    let out = loop {
        match rusty_http_codec::message_consumed_len(&buf) {
            Ok(n) if buf.len() >= n => {
                let leftover = buf.split_off(n);
                break Ok(ReadRequest {
                    request: buf,
                    leftover,
                });
            }
            Err(rusty_http_codec::CodecError::Truncated)
            | Err(rusty_http_codec::CodecError::ContentLengthMismatch) => {}
            Err(rusty_http_codec::CodecError::LimitExceeded(e)) => break Err(e),
            Err(_) => {
                break Ok(ReadRequest {
                    request: buf,
                    leftover: Vec::new(),
                })
            }
            _ => {}
        }

        if !expect_100_handled {
            if let Some(hdr_end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                expect_100_handled = true;
                let head = buf[..hdr_end].to_ascii_lowercase();
                if head.windows(12).any(|w| w == b"100-continue") {
                    let _ = rusty_sockets::stream_write_all(
                        stream_id,
                        b"HTTP/1.1 100 Continue\r\n\r\n",
                    );
                }
            }
        }
        let elapsed = start.elapsed();
        if elapsed >= overall {
            break if buf.is_empty() {
                Ok(ReadRequest {
                    request: Vec::new(),
                    leftover: Vec::new(),
                })
            } else {
                Err("incomplete request".to_string())
            };
        }
        if buf.is_empty() && elapsed >= idle {
            break Ok(ReadRequest {
                request: Vec::new(),
                leftover: Vec::new(),
            });
        }
        match rusty_sockets::stream_try_read(stream_id, 8192) {
            Ok(Some(chunk)) if chunk.is_empty() => {
                break if buf.is_empty() {
                    Ok(ReadRequest {
                        request: Vec::new(),
                        leftover: Vec::new(),
                    })
                } else {
                    Err("incomplete request".to_string())
                }
            }
            Ok(Some(chunk)) => {
                buf.extend_from_slice(&chunk);
                if buf.len() > MAX_REQUEST_BYTES {
                    break Err("request too large".to_string());
                }
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(1)),
            Err(e) => break Err(format!("{e:?}")),
        }
    };
    let _ = rusty_sockets::stream_set_nonblocking(stream_id, false);
    out
}

fn process_server_upgrade(
    rt: &mut Runtime,
    server: &ActiveHttpServer,
    parsed: &rusty_http_codec::ParsedRequest,
    request_bytes: &[u8],
    stream_id: u64,
) -> Result<bool, RuntimeError> {
    let Some(head_start) = find_header_end(request_bytes) else {
        return Ok(false);
    };
    let req = make_request_object(rt, parsed);
    let socket = crate::net::make_socket(rt, stream_id, server.handler_realm);
    rt.object_set(req, "socket".into(), Value::Object(socket));
    rt.object_set(req, "connection".into(), Value::Object(socket));
    let head = http_buffer_from_bytes(rt, &request_bytes[head_start..]);
    let prior = rt.enter_realm(server.handler_realm);
    emit_named_event(
        rt,
        server.server_object,
        "upgrade",
        vec![Value::Object(req), Value::Object(socket), head],
    );
    rt.exit_realm(prior);
    WENT_ASYNC.with(|c| c.set(true));
    Ok(true)
}

pub(crate) fn request_complete(buf: &[u8]) -> bool {
    match rusty_http_codec::message_consumed_len(buf) {
        Ok(n) => buf.len() >= n,
        Err(rusty_http_codec::CodecError::Truncated)
        | Err(rusty_http_codec::CodecError::ContentLengthMismatch) => false,
        Err(_) => true,
    }
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|idx| idx + 4)
}

fn poll_io_inner(
    rt: &mut Runtime,
    listener_wait_ms_override: Option<u64>,
    sticky_active: bool,
) -> Result<bool, RuntimeError> {
    let agent_id = rt.agent_id();
    let ids: Vec<(usize, u64, bool)> = HTTP_SERVERS.with(|servers| {
        servers
            .borrow()
            .iter()
            .enumerate()
            .filter_map(|(idx, s)| {
                s.as_ref().and_then(|srv| {
                    if srv.agent_id == agent_id {
                        Some((idx, srv.listener_handle, srv.refed))
                    } else {
                        None
                    }
                })
            })
            .collect()
    });

    let streaming = drain_streaming_conns(rt);

    let parked_pending =
        PARKED_CONNS.with(|m| m.borrow().values().any(|(owner, _)| *owner == agent_id));
    let has_active =
        ids.iter().any(|(_, h, refed)| *h != 0 && *refed) || streaming || parked_pending;
    let listener_wait_ms = listener_wait_ms_override.unwrap_or_else(|| {
        if streaming || crate::timer::has_pending(rt) || crate::fetch::has_pending_fetch(rt) {
            0
        } else {

            1
        }
    });

    loop {
        match rusty_sockets::readiness_poll(0) {
            Ok(Some(rusty_sockets::AsyncEvent::Readable { stream_id })) => {
                let owner = PARKED_CONNS.with(|m| m.borrow_mut().remove(&stream_id));
                if let Some((owner_agent_id, server_id)) = owner {
                    if owner_agent_id != agent_id {
                        PARKED_CONNS.with(|m| {
                            m.borrow_mut()
                                .insert(stream_id, (owner_agent_id, server_id))
                        });
                        continue;
                    }
                    rt.enqueue_host_phase_rooted(
                        HostEnqueuePhase::HostCompletionMacrotask,
                        "http server keep-alive resume",
                        Vec::new(),
                        move |rt| {
                            resume_connection(rt, server_id, stream_id);
                            Ok(())
                        },
                    );
                    return Ok(true);
                }

                let _ = rusty_sockets::handle_close(stream_id);
            }
            _ => break,
        }
    }
    for (server_id, listener_handle, _) in ids {

        if listener_handle == 0 {
            continue;
        }
        match rusty_sockets::listener_poll(listener_handle, listener_wait_ms) {
            Ok(Some(rusty_sockets::AsyncEvent::Connection { stream_id, .. })) => {
                rt.enqueue_host_phase_rooted(
                    HostEnqueuePhase::HostCompletionMacrotask,
                    "http server request",
                    Vec::new(),
                    move |rt| {
                        handle_connection(rt, server_id, stream_id);
                        Ok(())
                    },
                );
                return Ok(true);
            }
            Ok(Some(rusty_sockets::AsyncEvent::Closed)) => {
                let _ = remove_server_for_runtime(rt, server_id);
            }
            Ok(Some(rusty_sockets::AsyncEvent::Error(_))) => {
                let _ = remove_server_for_runtime(rt, server_id);
            }

            Ok(Some(rusty_sockets::AsyncEvent::Readable { .. })) => {}
            Ok(None) => {}
            Err(e) => return Err(RuntimeError::TypeError(format!("http poll_io: {e:?}"))),
        }
    }
    Ok(sticky_active && has_active)
}

pub fn poll_io(rt: &mut Runtime) -> Result<bool, RuntimeError> {
    poll_io_inner(rt, None, true)
}

pub fn poll_io_nonsticky(rt: &mut Runtime) -> Result<bool, RuntimeError> {
    poll_io_inner(rt, Some(0), false)
}

fn handle_connection(rt: &mut Runtime, server_id: usize, stream_id: u64) {
    if get_server_for_runtime(rt, server_id).is_none() {
        let _ = rusty_sockets::handle_close(stream_id);
        return;
    }

    serve_connection(rt, server_id, stream_id, Vec::new(), true);
}

fn resume_connection(rt: &mut Runtime, server_id: usize, stream_id: u64) {
    if get_server_for_runtime(rt, server_id).is_none() {
        let _ = rusty_sockets::handle_close(stream_id);
        return;
    }
    serve_connection(rt, server_id, stream_id, Vec::new(), false);
}

fn serve_connection(
    rt: &mut Runtime,
    server_id: usize,
    stream_id: u64,
    mut pending: Vec<u8>,
    _first_dispatch: bool,
) {
    loop {

        let idle = Duration::from_millis(50);
        let bytes = match read_request(stream_id, idle, pending) {
            Ok(ReadRequest { request, leftover }) if !request.is_empty() => {
                pending = leftover;
                request
            }
            Ok(_) => break,
            Err(e) if e.contains("request too large") || e.contains("decoded body exceeds") => {
                let response = rusty_http_codec::serialize_response(
                    413,
                    "Payload Too Large",
                    &[("connection".into(), "close".into())],
                    b"",
                );
                let _ = rusty_sockets::stream_write_all(stream_id, &response);
                break;
            }
            Err(_) => break,
        };
        let (response, keep_alive) = process_request_bytes(rt, server_id, &bytes, Some(stream_id));
        if WENT_ASYNC.with(|c| c.replace(false)) {

            return;
        }

        let _ = crate::net::remove_socket_by_stream(rt, stream_id);
        if rusty_sockets::stream_write_all(stream_id, &response).is_err() {
            break;
        }
        if !keep_alive {
            break;
        }

        if pending.is_empty() {
            if rusty_sockets::stream_register_readable(stream_id, KEEP_ALIVE_TIMEOUT_MS).is_ok() {
                PARKED_CONNS.with(|m| m.borrow_mut().insert(stream_id, (rt.agent_id(), server_id)));
                return;
            }

            break;
        }
    }
    let _ = rusty_sockets::handle_close(stream_id);
}

pub(crate) fn process_request_bytes(
    rt: &mut Runtime,
    server_id: usize,
    request_bytes: &[u8],

    stream_id: Option<u64>,
) -> (Vec<u8>, bool) {
    let Some(server) = get_server_for_runtime(rt, server_id) else {
        return (
            rusty_http_codec::serialize_response(
                500,
                "Internal Server Error",
                &[("connection".into(), "close".into())],
                b"",
            ),
            false,
        );
    };
    match rusty_http_codec::parse_request(request_bytes).map_err(|e| e.to_string()) {
        Ok(parsed) => {
            let keep = request_keep_alive(&parsed);

            let pure = rt.object_get(server.server_object, CRUFT_FETCH_HANDLER_SLOT);
            if rt.is_callable(&pure) {
                return process_request_pure(rt, &server, &parsed, pure, keep, stream_id);
            }
            if request_is_upgrade(&parsed) {
                if let Some(sid) = stream_id {
                    match process_server_upgrade(rt, &server, &parsed, request_bytes, sid) {
                        Ok(true) => return (Vec::new(), true),
                        Ok(false) => {}
                        Err(_) => {
                            return (
                                rusty_http_codec::serialize_response(
                                    500,
                                    "Internal Server Error",
                                    &[("connection".into(), "close".into())],
                                    b"",
                                ),
                                false,
                            )
                        }
                    }
                }
            }
            let req = make_request_object(rt, &parsed);
            let res = make_response_object(rt);
            rt.set_engine_sentinel(res, SERVER_RESPONSE_REQUEST_SLOT, Value::Object(req));

            rt.object_set(res, "req".into(), Value::Object(req));

            if let Some(sid) = stream_id {
                let socket = crate::net::make_socket(rt, sid, server.handler_realm);
                rt.object_set(req, "socket".into(), Value::Object(socket));
                rt.object_set(req, "connection".into(), Value::Object(socket));
                rt.object_set(res, "socket".into(), Value::Object(socket));
                rt.object_set(res, "connection".into(), Value::Object(socket));
            }

            if let Some(sid) = stream_id {
                rt.set_engine_sentinel(res, STREAM_ID_SLOT, Value::Number(sid as f64));
                rt.set_engine_sentinel(res, ASYNC_KEEP_ALIVE_SLOT, Value::Boolean(keep));
            }
            let prior = rt.enter_realm(server.handler_realm);

            let mut call_result =
                dispatch_request(rt, server.server_object, req, res).and_then(|listener_results| {
                    drive_request_body(rt, req)?;

                    drop(listener_results);
                    Ok(())
                });

            if call_result.is_ok() {
                let mut ticks = 0u32;
                loop {
                    let ended = matches!(
                        rt.object_get(res, "__cruft_http_ended"),
                        Value::Boolean(true)
                    );
                    if ended {
                        break;
                    }
                    match rusty_js_runtime::job_queue::pump_one_tick(rt) {
                        Ok(true) => {}
                        Ok(false) => {

                            let napi = rusty_js_runtime::napi::drain_main_inbox(rt) > 0;
                            let due = crate::timer::drain_due_pairs_for_runtime(rt);
                            if !due.is_empty() {
                                for (id, cb, args, repeat, async_context, async_resource) in due {
                                    let roots = crate::timer::roots_for_callback_with_resource(
                                        &cb,
                                        &args,
                                        async_resource,
                                    );
                                    rt.enqueue_host_phase_rooted_with_async_context(
                                        HostEnqueuePhase::TimerCallbackMacrotask,
                                        "timer callback",
                                        roots,
                                        async_context,
                                        move |rt| {
                                            if let Some(resource) = async_resource {
                                                let _ = crate::node_stubs::async_hooks_call_with_global_resource_and_microtasks(
                                                    rt,
                                                    resource,
                                                    cb,
                                                    Value::Undefined,
                                                    args,
                                                );
                                            } else {
                                                let _ = rt.call_function(cb, Value::Undefined, args);
                                            }
                                            if !repeat {
                                                if let Some(resource) = async_resource {
                                                crate::node_stubs::async_hooks_emit_destroy_for_global(
                                                    rt,
                                                    Value::Object(resource),
                                                )?;
                                                }
                                            }
                                            Ok(())
                                        },
                                    );
                                    if !repeat {
                                        crate::timer::release_roots(rt, id);
                                    }
                                }
                                continue;
                            }
                            if napi {
                                continue;
                            }

                            if let Some(sid) = stream_id {
                                if matches!(
                                    rt.object_get(res, STREAM_HDR_SENT),
                                    Value::Boolean(true)
                                ) {
                                    register_streaming_conn(rt, sid, res);
                                    break;
                                }
                                register_buffered_async_conn(rt, sid, res, keep);
                                break;
                            }
                            if crate::timer::has_pending(rt) {

                                let wait = crate::timer::next_due_ms(rt).unwrap_or(1).clamp(1, 25);
                                std::thread::sleep(std::time::Duration::from_millis(wait));
                                continue;
                            }
                            break;
                        }
                        Err(e) => {
                            call_result = Err(e);
                            break;
                        }
                    }
                    ticks += 1;
                    if ticks > 500_000 {
                        break;
                    }
                }
            }
            rt.exit_realm(prior);
            if let Err(err) = &call_result {
                let _ = err;
                (
                    rusty_http_codec::serialize_response(
                        500,
                        "Internal Server Error",
                        &[("connection".into(), "close".into())],
                        b"",
                    ),
                    false,
                )
            } else if matches!(rt.object_get(res, STREAM_HDR_SENT), Value::Boolean(true))
                || WENT_ASYNC.with(|c| c.get())
            {

                (Vec::new(), keep)
            } else {
                let resp = response_to_wire(rt, res, keep).unwrap_or_else(|_| {
                    rusty_http_codec::serialize_response(
                        500,
                        "Internal Server Error",
                        &[("connection".into(), "close".into())],
                        b"",
                    )
                });
                let keep = keep && !response_says_close(&resp);
                (resp, keep)
            }
        }
        Err(e) if e.contains("limit exceeded") => (
            rusty_http_codec::serialize_response(
                413,
                "Payload Too Large",
                &[("connection".into(), "close".into())],
                b"",
            ),
            false,
        ),
        Err(_) => (
            rusty_http_codec::serialize_response(
                400,
                "Bad Request",
                &[("connection".into(), "close".into())],
                b"",
            ),
            false,
        ),
    }
}

pub(crate) fn register_https_handler(
    rt: &mut Runtime,
    server_object: ObjectRef,
    handler: Value,
) -> Result<usize, RuntimeError> {
    let listeners = rt.alloc_object(rusty_js_runtime::Object::new_array());
    rt.object_set(listeners, "length".into(), Value::Number(0.0));
    set_internal_slot(
        rt,
        server_object,
        REQUEST_LISTENERS_SLOT,
        Value::Object(listeners),
    );
    if rt.is_callable(&handler) {
        add_request_listener(rt, server_object, handler, false)?;
    }
    Ok(next_server_id(ActiveHttpServer {
        agent_id: rt.agent_id(),
        listener_handle: 0,
        bound_addr: String::new(),
        handler_realm: rt.current_realm,
        server_object,
        refed: false,
    }))
}

fn dispatch_request(
    rt: &mut Runtime,
    server_object: ObjectRef,
    req: ObjectRef,
    res: ObjectRef,
) -> Result<Vec<Value>, RuntimeError> {
    let arr = request_listeners(rt, server_object);
    let len = rt.array_length(arr);
    let mut calls = Vec::new();
    let mut keep = Vec::new();
    for i in 0..len {
        let item = rt.object_get(arr, &i.to_string());
        let Value::Object(record) = item else {
            continue;
        };
        let listener = rt.object_get(record, "listener");
        if !rt.is_callable(&listener) {
            continue;
        }
        let once = matches!(
            rt.object_get(record, REQUEST_ONCE_SLOT),
            Value::Boolean(true)
        );
        calls.push(listener);
        if !once {
            keep.push(Value::Object(record));
        }
    }
    for (i, v) in keep.iter().enumerate() {
        rt.object_set(arr, i.to_string(), v.clone());
    }
    for i in keep.len()..(len as usize) {
        rt.object_set(arr, i.to_string(), Value::Undefined);
    }
    rt.object_set(arr, "length".into(), Value::Number(keep.len() as f64));
    let _ = crate::node_stubs::async_hooks_emit_init_for_global(
        rt,
        "HTTPINCOMINGMESSAGE",
        Value::Object(req),
    )?;
    let mut results = Vec::new();
    for listener in calls {
        let result = crate::node_stubs::async_hooks_call_with_global_resource(
            rt,
            req,
            listener,
            Value::Object(server_object),
            vec![Value::Object(req), Value::Object(res)],
        )?;
        results.push(result);
    }
    Ok(results)
}

fn attach_emitter(rt: &mut Runtime, obj: ObjectRef) {
    let on = |rt: &mut Runtime, args: &[Value]| -> Result<Value, RuntimeError> {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let event = match args.first() {
            Some(v) => emitter_event_key(rt, v)?,
            None => String::new(),
        };
        let listener = args.get(1).cloned().unwrap_or(Value::Undefined);
        if rt.is_callable(&listener) {
            let listeners = match rt.object_get(this, RES_LISTENERS_SLOT) {
                Value::Object(id) => id,
                _ => {
                    let o = new_object(rt);
                    rt.set_engine_sentinel(this, RES_LISTENERS_SLOT, Value::Object(o));
                    o
                }
            };
            let arr = match rt.object_get(listeners, &event) {
                Value::Object(a) => a,
                _ => {
                    let a = rt.alloc_object(rusty_js_runtime::Object::new_array());
                    rt.object_set(listeners, event.clone(), Value::Object(a));
                    a
                }
            };
            let len = rt.array_length(arr);
            rt.object_set(arr, len.to_string(), listener);
            rt.object_set(arr, "length".into(), Value::Number((len + 1) as f64));
        }
        Ok(Value::Object(this))
    };
    register_method(rt, obj, "on", on);
    register_method(rt, obj, "once", on);
    register_method(rt, obj, "addListener", on);
    register_method(rt, obj, "removeListener", |rt, _a| Ok(rt.current_this()));
    register_method(rt, obj, "off", |rt, _a| Ok(rt.current_this()));

    let prepend = |rt: &mut Runtime, args: &[Value]| -> Result<Value, RuntimeError> {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let event = match args.first() {
            Some(v) => emitter_event_key(rt, v)?,
            None => String::new(),
        };
        let listener = args.get(1).cloned().unwrap_or(Value::Undefined);
        if rt.is_callable(&listener) {
            let listeners = match rt.object_get(this, RES_LISTENERS_SLOT) {
                Value::Object(id) => id,
                _ => {
                    let o = new_object(rt);
                    rt.set_engine_sentinel(this, RES_LISTENERS_SLOT, Value::Object(o));
                    o
                }
            };
            let arr = match rt.object_get(listeners, &event) {
                Value::Object(a) => a,
                _ => {
                    let a = rt.alloc_object(rusty_js_runtime::Object::new_array());
                    rt.object_set(listeners, event.clone(), Value::Object(a));
                    a
                }
            };

            let len = rt.array_length(arr);
            for i in (0..len).rev() {
                let v = rt.object_get(arr, &i.to_string());
                rt.object_set(arr, (i + 1).to_string(), v);
            }
            rt.object_set(arr, "0".into(), listener);
            rt.object_set(arr, "length".into(), Value::Number((len + 1) as f64));
        }
        Ok(Value::Object(this))
    };
    register_method(rt, obj, "prependListener", prepend);
    register_method(rt, obj, "prependOnceListener", prepend);
}

const RES_CHUNKS: &str = "__cruft_res_chunks";

const RES_ENDED: &str = "__cruft_res_ended";

const RES_DATA_IDX: &str = "__cruft_res_data_idx";

const RES_END_EMITTED: &str = "__cruft_res_end_emitted";

const RES_WAITERS: &str = "__cruft_res_waiters";

fn res_array_slot(rt: &mut Runtime, res: ObjectRef, slot: &str) -> ObjectRef {
    match rt.object_get(res, slot) {
        Value::Object(a) => a,
        _ => {
            let a = rt.alloc_object(rusty_js_runtime::Object::new_array());
            rt.set_engine_sentinel(res, slot, Value::Object(a));
            a
        }
    }
}

fn array_push(rt: &mut Runtime, arr: ObjectRef, v: Value) {
    let len = rt.array_length(arr);
    rt.object_set(arr, len.to_string(), v);
    rt.object_set(arr, "length".into(), Value::Number((len + 1) as f64));
}

fn res_num(rt: &mut Runtime, res: ObjectRef, slot: &str) -> usize {
    match rt.object_get(res, slot) {
        Value::Number(n) if n >= 0.0 => n as usize,
        _ => 0,
    }
}

fn res_push_chunk(rt: &mut Runtime, res: ObjectRef, bytes: &[u8]) {
    let buf = http_buffer_from_bytes(rt, bytes);
    let chunks = res_array_slot(rt, res, RES_CHUNKS);
    array_push(rt, chunks, buf);
    let _ = emit_res_event(rt, res, "readable", vec![]);
    if matches!(
        rt.object_get(res, "__cruft_res_flowing"),
        Value::Boolean(true)
    ) {
        deliver_res_body(rt, res);
    }
    res_wake_waiters(rt, res);
}

fn res_mark_ended(rt: &mut Runtime, res: ObjectRef) {
    rt.set_engine_sentinel(res, RES_ENDED, Value::Boolean(true));
    if matches!(
        rt.object_get(res, "__cruft_res_flowing"),
        Value::Boolean(true)
    ) {
        deliver_res_body(rt, res);
    }
    res_wake_waiters(rt, res);
}

fn deliver_res_body(rt: &mut Runtime, res: ObjectRef) {
    let chunks = res_array_slot(rt, res, RES_CHUNKS);
    loop {
        let len = rt.array_length(chunks);
        let idx = res_num(rt, res, RES_DATA_IDX);
        if idx >= len {
            break;
        }
        let chunk = rt.object_get(chunks, &idx.to_string());

        rt.set_engine_sentinel(res, RES_DATA_IDX, Value::Number((idx + 1) as f64));
        let _ = emit_res_event(rt, res, "data", vec![chunk]);
    }
    let ended = matches!(rt.object_get(res, RES_ENDED), Value::Boolean(true));
    let end_emitted = matches!(rt.object_get(res, RES_END_EMITTED), Value::Boolean(true));
    if ended && !end_emitted {
        rt.set_engine_sentinel(res, RES_END_EMITTED, Value::Boolean(true));
        rt.object_set(res, "readableEnded".into(), Value::Boolean(true));
        let _ = emit_res_event(rt, res, "end", vec![]);
    }
}

fn res_wake_waiters(rt: &mut Runtime, res: ObjectRef) {
    let waiters = res_array_slot(rt, res, RES_WAITERS);
    let len = rt.array_length(waiters);
    if len == 0 {
        return;
    }
    let mut still_waiting: Vec<Value> = Vec::new();
    for i in 0..len {
        let w = match rt.object_get(waiters, &i.to_string()) {
            Value::Object(w) => w,
            _ => continue,
        };
        let p = match rt.object_get(w, "__p") {
            Value::Object(p) => p,
            _ => continue,
        };
        let it = match rt.object_get(w, "__it") {
            Value::Object(it) => it,
            _ => continue,
        };
        match res_iter_result(rt, res, it) {
            Some(result) => rusty_js_runtime::promise::resolve_promise(rt, p, result),
            None => still_waiting.push(Value::Object(w)),
        }
    }
    let fresh = rt.alloc_object(rusty_js_runtime::Object::new_array());
    for w in still_waiting {
        array_push(rt, fresh, w);
    }
    rt.set_engine_sentinel(res, RES_WAITERS, Value::Object(fresh));
}

fn res_iter_result(rt: &mut Runtime, res: ObjectRef, it: ObjectRef) -> Option<Value> {
    let chunks = res_array_slot(rt, res, RES_CHUNKS);
    let len = rt.array_length(chunks);
    let idx = match rt.object_get(it, "__idx") {
        Value::Number(n) if n >= 0.0 => n as usize,
        _ => 0,
    };
    let result = new_object(rt);
    if idx < len {
        let chunk = rt.object_get(chunks, &idx.to_string());
        rt.set_engine_sentinel(it, "__idx", Value::Number((idx + 1) as f64));
        rt.object_set(result, "value".into(), chunk);
        rt.object_set(result, "done".into(), Value::Boolean(false));
        return Some(Value::Object(result));
    }
    if matches!(rt.object_get(res, RES_ENDED), Value::Boolean(true)) {
        rt.object_set(result, "value".into(), Value::Undefined);
        rt.object_set(result, "done".into(), Value::Boolean(true));
        return Some(Value::Object(result));
    }
    None
}

fn res_on_with_flow(rt: &mut Runtime, args: &[Value]) -> Result<Value, RuntimeError> {
    let this = match rt.current_this() {
        Value::Object(id) => id,
        _ => return Ok(Value::Undefined),
    };
    let event = match args.first() {
        Some(v) => emitter_event_key(rt, v)?,
        None => String::new(),
    };
    let listener = args.get(1).cloned().unwrap_or(Value::Undefined);
    if rt.is_callable(&listener) {
        let listeners = match rt.object_get(this, RES_LISTENERS_SLOT) {
            Value::Object(id) => id,
            _ => {
                let o = new_object(rt);
                rt.set_engine_sentinel(this, RES_LISTENERS_SLOT, Value::Object(o));
                o
            }
        };
        let arr = match rt.object_get(listeners, &event) {
            Value::Object(a) => a,
            _ => {
                let a = rt.alloc_object(rusty_js_runtime::Object::new_array());
                rt.object_set(listeners, event.clone(), Value::Object(a));
                a
            }
        };
        let len = rt.array_length(arr);
        rt.object_set(arr, len.to_string(), listener);
        rt.object_set(arr, "length".into(), Value::Number((len + 1) as f64));
    }
    if event == "data" || event == "readable" {
        rt.object_set(this, "__cruft_res_flowing".into(), Value::Boolean(true));
        rt.enqueue_microtask_rooted("http.res.flow", vec![this], move |rt| {
            deliver_res_body(rt, this);
            Ok(())
        });
    } else if event == "end"
        && matches!(rt.object_get(this, RES_ENDED), Value::Boolean(true))
        && !matches!(rt.object_get(this, RES_END_EMITTED), Value::Boolean(true))
    {
        rt.enqueue_microtask_rooted("http.res.end-late", vec![this], move |rt| {
            deliver_res_body(rt, this);
            Ok(())
        });
    }
    Ok(Value::Object(this))
}

fn make_incoming(rt: &mut Runtime, pr: &rusty_http_codec::ResponseHead) -> ObjectRef {
    let res = new_object(rt);
    rt.object_set(res, "statusCode".into(), Value::Number(pr.status as f64));
    rt.object_set(
        res,
        "statusMessage".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            pr.reason.clone(),
        ))),
    );
    let headers = new_object(rt);
    let raw_headers = rt.alloc_object(rusty_js_runtime::Object::new_array());
    for (n, v) in &pr.headers {
        let len = rt.array_length(raw_headers);
        rt.object_set(
            raw_headers,
            len.to_string(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(n.clone()))),
        );
        rt.object_set(
            raw_headers,
            (len + 1).to_string(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(v.clone()))),
        );
        rt.object_set(
            raw_headers,
            "length".into(),
            Value::Number((len + 2) as f64),
        );
        let key = n.to_ascii_lowercase();

        let existing = rt.object_get(headers, &key);
        let new_v = match existing {
            Value::Undefined => {
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(v.clone())))
            }
            Value::String(prev) if key == "set-cookie" => {
                let arr = rt.alloc_object(rusty_js_runtime::Object::new_array());
                rt.object_set(arr, "0".into(), Value::String(prev));
                rt.object_set(
                    arr,
                    "1".into(),
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(v.clone()))),
                );
                rt.object_set(arr, "length".into(), Value::Number(2.0));
                Value::Object(arr)
            }
            Value::Object(arr) if key == "set-cookie" => {
                let len = rt.array_length(arr);
                rt.object_set(
                    arr,
                    len.to_string(),
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(v.clone()))),
                );
                rt.object_set(arr, "length".into(), Value::Number((len + 1) as f64));
                Value::Object(arr)
            }
            Value::String(prev) => Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                format!("{}, {}", prev.as_str(), v),
            ))),
            other => other,
        };
        rt.object_set(headers, key, new_v);
    }
    rt.object_set(res, "headers".into(), Value::Object(headers));
    rt.object_set(res, "rawHeaders".into(), Value::Object(raw_headers));
    rt.object_set(
        res,
        "httpVersion".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from("1.1"))),
    );
    rt.object_set(res, "complete".into(), Value::Boolean(true));

    let trailers = new_object(rt);
    rt.object_set(res, "trailers".into(), Value::Object(trailers));
    let raw_trailers = rt.alloc_object(rusty_js_runtime::Object::new_array());
    rt.object_set(raw_trailers, "length".into(), Value::Number(0.0));
    rt.object_set(res, "rawTrailers".into(), Value::Object(raw_trailers));
    let l = new_object(rt);
    rt.set_engine_sentinel(res, RES_LISTENERS_SLOT, Value::Object(l));
    attach_emitter(rt, res);
    for noop in ["setEncoding", "pause", "destroy"] {
        register_method(rt, res, noop, |rt, _a| Ok(rt.current_this()));
    }

    register_method(rt, res, "resume", |rt, _a| {
        if let Value::Object(this) = rt.current_this() {
            rt.object_set(this, "__cruft_res_flowing".into(), Value::Boolean(true));
            rt.enqueue_microtask_rooted("http.res.resume", vec![this], move |rt| {
                deliver_res_body(rt, this);
                Ok(())
            });
        }
        Ok(rt.current_this())
    });

    register_method(rt, res, "on", res_on_with_flow);
    register_method(rt, res, "once", res_on_with_flow);
    register_method(rt, res, "addListener", res_on_with_flow);

    let chunks = rt.alloc_object(rusty_js_runtime::Object::new_array());
    rt.set_engine_sentinel(res, RES_CHUNKS, Value::Object(chunks));
    rt.set_engine_sentinel(res, RES_ENDED, Value::Boolean(false));
    rt.set_engine_sentinel(res, RES_DATA_IDX, Value::Number(0.0));
    rt.set_engine_sentinel(res, RES_END_EMITTED, Value::Boolean(false));
    rt.object_set(res, "readable".into(), Value::Boolean(true));

    register_method(rt, res, "pipe", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(t) => t,
            _ => return Ok(Value::Undefined),
        };
        let dest = match args.first() {
            Some(Value::Object(d)) => *d,
            _ => return Ok(rt.current_this()),
        };
        let s = |rt: &mut Runtime, x: &str| {
            let _ = rt;
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(x)))
        };
        let on = rt.object_get(this, "on");
        if !rt.is_callable(&on) {
            return Ok(Value::Object(dest));
        }
        let on_end = crate::register::make_callable(rt, "res.pipe.onEnd", move |rt, _a| {
            let e = rt.object_get(dest, "end");
            if rt.is_callable(&e) {
                let _ = rt.call_function(e, Value::Object(dest), Vec::new());
            }
            Ok(Value::Undefined)
        });
        let ev_end = s(rt, "end");
        let _ = rt.call_function(
            on.clone(),
            Value::Object(this),
            vec![ev_end, Value::Object(on_end)],
        );
        let on_data = crate::register::make_callable(rt, "res.pipe.onData", move |rt, a| {
            let chunk = a.first().cloned().unwrap_or(Value::Undefined);
            let w = rt.object_get(dest, "write");
            if rt.is_callable(&w) {
                let _ = rt.call_function(w, Value::Object(dest), vec![chunk]);
            }
            Ok(Value::Undefined)
        });
        let ev_data = s(rt, "data");
        let _ = rt.call_function(
            on,
            Value::Object(this),
            vec![ev_data, Value::Object(on_data)],
        );
        let d_emit = rt.object_get(dest, "emit");
        if rt.is_callable(&d_emit) {
            let ev_pipe = s(rt, "pipe");
            let _ = rt.call_function(
                d_emit,
                Value::Object(dest),
                vec![ev_pipe, Value::Object(this)],
            );
        }

        rt.object_set(this, "__cruft_res_flowing".into(), Value::Boolean(true));
        rt.enqueue_microtask_rooted("http.res.pipe.flow", vec![this], move |rt| {
            deliver_res_body(rt, this);
            Ok(())
        });
        Ok(Value::Object(dest))
    });

    register_method(rt, res, "read", |rt, _a| {
        let this = match rt.current_this() {
            Value::Object(t) => t,
            _ => return Ok(Value::Null),
        };

        let chunks = res_array_slot(rt, this, RES_CHUNKS);
        let len = rt.array_length(chunks);
        let idx = res_num(rt, this, "__cruft_res_read_idx");
        if idx >= len {
            return Ok(Value::Null);
        }
        let mut bytes: Vec<u8> = Vec::new();
        for i in idx..len {
            let c = rt.object_get(chunks, &i.to_string());
            bytes.extend(value_to_bytes(rt, &c)?);
        }
        rt.set_engine_sentinel(this, "__cruft_res_read_idx", Value::Number(len as f64));
        Ok(http_buffer_from_bytes(rt, &bytes))
    });
    register_method(rt, res, "toArray", |rt, _args| {
        use rusty_js_runtime::promise::{new_promise, reject_promise, resolve_promise};

        let this = match rt.current_this() {
            Value::Object(t) => t,
            _ => return Ok(Value::Undefined),
        };
        let promise = new_promise(rt);
        let chunks = rt.alloc_object(rusty_js_runtime::Object::new_array());
        rt.object_set(chunks, "length".into(), Value::Number(0.0));

        let on_data = crate::register::make_callable(rt, "http.res.toArray.data", move |rt, a| {
            let chunk = a.first().cloned().unwrap_or(Value::Undefined);
            let len = rt.array_length(chunks);
            rt.object_set(chunks, len.to_string(), chunk);
            rt.object_set(chunks, "length".into(), Value::Number((len + 1) as f64));
            Ok(Value::Undefined)
        });
        let on_end = crate::register::make_callable(rt, "http.res.toArray.end", move |rt, _a| {
            resolve_promise(rt, promise, Value::Object(chunks));
            Ok(Value::Undefined)
        });
        let on_error =
            crate::register::make_callable(rt, "http.res.toArray.error", move |rt, a| {
                let error = a.first().cloned().unwrap_or(Value::Undefined);
                reject_promise(rt, promise, error);
                Ok(Value::Undefined)
            });
        let sv = |s: &str| Value::String(Rc::new(rusty_js_runtime::value::JsString::from(s)));
        let on = rt.object_get(this, "on");
        if rt.is_callable(&on) {
            rt.call_function(
                on.clone(),
                Value::Object(this),
                vec![sv("error"), Value::Object(on_error)],
            )?;
            rt.call_function(
                on.clone(),
                Value::Object(this),
                vec![sv("end"), Value::Object(on_end)],
            )?;
            rt.call_function(
                on,
                Value::Object(this),
                vec![sv("data"), Value::Object(on_data)],
            )?;
        }
        Ok(Value::Object(promise))
    });

    register_method(rt, res, "@@asyncIterator", move |rt, _a| {
        let this = match rt.current_this() {
            Value::Object(t) => t,
            _ => return Ok(Value::Undefined),
        };
        let iter = new_object(rt);
        rt.set_engine_sentinel(iter, "__res", Value::Object(this));
        rt.set_engine_sentinel(iter, "__idx", Value::Number(0.0));
        register_method(rt, iter, "next", |rt, _a| {
            let it = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let res = match rt.object_get(it, "__res") {
                Value::Object(r) => r,
                _ => return Ok(Value::Undefined),
            };
            let p = rusty_js_runtime::promise::new_promise(rt);
            match res_iter_result(rt, res, it) {
                Some(result) => rusty_js_runtime::promise::resolve_promise(rt, p, result),
                None => {

                    let w = new_object(rt);
                    rt.set_engine_sentinel(w, "__p", Value::Object(p));
                    rt.set_engine_sentinel(w, "__it", Value::Object(it));
                    let waiters = res_array_slot(rt, res, RES_WAITERS);
                    array_push(rt, waiters, Value::Object(w));
                }
            }
            Ok(Value::Object(p))
        });
        register_method(rt, iter, "return", |rt, _a| {
            let p = rusty_js_runtime::promise::new_promise(rt);
            let result = new_object(rt);
            rt.object_set(result, "value".into(), Value::Undefined);
            rt.object_set(result, "done".into(), Value::Boolean(true));
            rusty_js_runtime::promise::resolve_promise(rt, p, Value::Object(result));
            Ok(Value::Object(p))
        });
        register_method(rt, iter, "@@asyncIterator", |rt, _a| Ok(rt.current_this()));
        Ok(Value::Object(iter))
    });

    link_incoming_prototype(rt, res);
    res
}

fn client_error_value(rt: &mut Runtime, msg: String) -> Value {
    let m = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(msg)));
    let ctor = rt.global_get("Error");
    if rt.is_callable(&ctor) {
        if let Ok(e) = rt.construct(ctor, vec![m.clone()]) {
            return e;
        }
    }
    m
}

fn client_connect_error_value(rt: &mut Runtime, msg: &str, request: ObjectRef) -> Value {
    let s = |x: &str| {
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            x.to_string(),
        )))
    };
    let lower = msg.to_ascii_lowercase();
    let classify = if lower.contains("connection refused") || lower.contains("os error 111") {
        Some(("ECONNREFUSED", -111i64, "connect"))
    } else if lower.contains("os error 110") || lower.contains("timed out") {
        Some(("ETIMEDOUT", -110, "connect"))
    } else if lower.contains("not known")
        || lower.contains("failed to lookup")
        || lower.contains("name or service")
    {
        Some(("ENOTFOUND", -3008, "getaddrinfo"))
    } else {
        None
    };
    let Some((code, errno, syscall)) = classify else {
        return client_error_value(rt, msg.to_string());
    };
    let url = match rt.object_get(request, "__cruft_url") {
        Value::String(u) => u.to_string(),
        _ => String::new(),
    };
    let (host, port) = match crate::http_client::parse_url(&url) {
        Some((_scheme, host, port, _target)) => (host, port),
        None => (String::new(), 0u16),
    };
    let node_msg = if syscall == "getaddrinfo" {
        format!("getaddrinfo {code} {host}")
    } else {
        format!("connect {code} {host}:{port}")
    };
    let ctor = rt.global_get("Error");
    let err = match rt.construct(ctor, vec![s(&node_msg)]) {
        Ok(Value::Object(id)) => id,
        _ => return s(&node_msg),
    };
    rt.object_set(err, "code".into(), s(code));
    rt.object_set(err, "errno".into(), Value::Number(errno as f64));
    rt.object_set(err, "syscall".into(), s(syscall));
    if syscall == "connect" {
        rt.object_set(err, "address".into(), s(&host));
        rt.object_set(err, "port".into(), Value::Number(port as f64));
    } else {
        rt.object_set(err, "hostname".into(), s(&host));
    }
    Value::Object(err)
}

pub(crate) fn http_agent_reflect_options(rt: &mut Runtime, id: ObjectRef, opts: Option<&Value>) {
    let get_num = |rt: &mut Runtime, key: &str, default: f64| -> f64 {
        if let Some(Value::Object(o)) = opts {
            if let Value::Number(n) = rt.object_get(*o, key) {
                return n;
            }
        }
        default
    };
    let get_bool = |rt: &mut Runtime, key: &str| -> bool {
        if let Some(Value::Object(o)) = opts {
            return matches!(rt.object_get(*o, key), Value::Boolean(true));
        }
        false
    };
    let keep_alive = get_bool(rt, "keepAlive");
    let keep_alive_msecs = get_num(rt, "keepAliveMsecs", 1000.0);
    let max_sockets = get_num(rt, "maxSockets", f64::INFINITY);
    let max_free_sockets = get_num(rt, "maxFreeSockets", 256.0);
    let max_total_sockets = get_num(rt, "maxTotalSockets", f64::INFINITY);
    rt.object_set(id, "keepAlive".into(), Value::Boolean(keep_alive));
    rt.object_set(id, "keepAliveMsecs".into(), Value::Number(keep_alive_msecs));
    rt.object_set(id, "maxSockets".into(), Value::Number(max_sockets));
    rt.object_set(id, "maxFreeSockets".into(), Value::Number(max_free_sockets));
    rt.object_set(
        id,
        "maxTotalSockets".into(),
        Value::Number(max_total_sockets),
    );
    for slot in ["sockets", "freeSockets", "requests"] {
        let bag = new_object(rt);
        rt.object_set(id, slot.into(), Value::Object(bag));
    }

    register_method(rt, id, "destroy", |rt, _a| {
        if let Value::Object(this) = rt.current_this() {
            if let Value::Object(socket) = rt.object_get(this, AGENT_SOCKET_SLOT) {
                http_agent_mark_socket_free(rt, socket);
            }
        }
        Ok(Value::Undefined)
    });

    register_method(rt, id, "getName", |rt, args| {
        let opts = match args.first() {
            Some(Value::Object(o)) => *o,
            _ => {
                return Ok(Value::String(Rc::new(
                    rusty_js_runtime::value::JsString::from("localhost::"),
                )))
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
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(name),
        )))
    });
}

fn link_incoming_prototype(rt: &mut Runtime, obj: ObjectRef) {
    if let Value::Object(proto) = rt.global_get("__cruft_http_im_proto") {
        rt.set_object_prototype_internal(obj, Some(proto));
    }
}

fn headers_sent_error(rt: &mut Runtime, this: ObjectRef, verb: &str) -> Option<RuntimeError> {
    if matches!(rt.object_get(this, "headersSent"), Value::Boolean(true)) {
        Some(RuntimeError::Thrown(coded_error(
            rt,
            "ERR_HTTP_HEADERS_SENT",
            &format!("Cannot {verb} headers after they are sent to the client"),
        )))
    } else {
        None
    }
}

fn coded_error(rt: &mut Runtime, code: &str, msg: &str) -> Value {
    let m = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
        msg.to_string(),
    )));
    let ctor = rt.global_get("Error");
    if rt.is_callable(&ctor) {
        if let Ok(Value::Object(id)) = rt.construct(ctor, vec![m.clone()]) {
            rt.object_set(
                id,
                "code".into(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    code.to_string(),
                ))),
            );
            return Value::Object(id);
        }
    }
    m
}

fn client_request_dispatch(rt: &mut Runtime, this: ObjectRef) -> Result<(), RuntimeError> {
    if matches!(
        rt.object_get(this, "__cruft_dispatched"),
        Value::Boolean(true)
    ) {
        return Ok(());
    }
    rt.set_engine_sentinel(this, "__cruft_dispatched", Value::Boolean(true));

    if matches!(rt.object_get(this, "__cruft_aborted"), Value::Boolean(true)) {
        return Ok(());
    }
    let url = match rt.object_get(this, "__cruft_url") {
        Value::String(s) => s.to_string(),
        _ => String::new(),
    };
    let method = match rt.object_get(this, "__cruft_method") {
        Value::String(s) => s.to_string(),
        _ => "GET".to_string(),
    };
    let body = match rt.object_get(this, "__cruft_body") {
        Value::String(s) => s.as_bytes().to_vec(),
        _ => Vec::new(),
    };
    let mut headers: Vec<(String, String)> = Vec::new();
    if let Value::Object(h) = rt.object_get(this, "__cruft_headers") {
        for k in rt.ordinary_own_enumerable_string_keys(h) {
            let v = rt.object_get(h, &k);
            if !matches!(v, Value::Undefined) {
                headers.push((k, value_to_string(rt, &v)?));
            }
        }
    }
    let (scheme, host, port, target) = match crate::http_client::parse_url(&url) {
        Some(t) => t,
        None => {
            emit_res_event(
                rt,
                this,
                "error",
                vec![Value::String(Rc::new(
                    rusty_js_runtime::value::JsString::from(format!("invalid URL '{url}'")),
                ))],
            )?;
            return Ok(());
        }
    };
    let caller = caller_module_id(rt);
    if let Err(e) = rt.caps.require_net(
        &caps::Net::none(),
        caps::NetOp::Connect {
            host: host.clone(),
            port,
        },
        &caller,
    ) {
        let ev = client_error_value(rt, e.to_string());
        emit_res_event(rt, this, "error", vec![ev])?;
        return Ok(());
    }
    let reqbytes =
        match crate::http_client::try_build_request(&method, &target, &host, port, &headers, &body)
        {
            Ok(bytes) => bytes,
            Err(e) => {
                let ev = client_error_value(rt, e);
                emit_res_event(rt, this, "error", vec![ev])?;
                return Ok(());
            }
        };
    let insecure = matches!(
        rt.object_get(this, "__cruft_insecure"),
        Value::Boolean(true)
    );
    let ca_pem = match rt.object_get(this, "__cruft_ca") {
        Value::String(s) => Some(s.as_str().to_string()),
        _ => None,
    };
    let realm = rt.current_realm;

    let is_head = method.eq_ignore_ascii_case("HEAD");
    let (tx, rx) = std::sync::mpsc::channel();
    let wake = rt.agent_wake_handle();
    std::thread::spawn(move || {
        match crate::http_client::round_trip_streaming(
            &scheme,
            &host,
            port,
            &reqbytes,
            insecure,
            ca_pem.as_deref(),
        ) {
            Ok(mut s) => {
                if s.head.status == 101 {
                    if let Some(stream_id) = s.take_plain_upgrade_stream() {
                        let _ = tx.send(ClientMsg::Upgrade(s.head.clone(), stream_id, Vec::new()));
                        notify_agent_wake(&wake);
                        return;
                    }
                }
                if tx.send(ClientMsg::Head(s.head.clone())).is_err() {
                    return;
                }
                notify_agent_wake(&wake);
                if is_head {
                    let _ = tx.send(ClientMsg::End);
                    notify_agent_wake(&wake);
                    return;
                }
                loop {
                    match s.next_chunk() {
                        Ok(Some(c)) => {
                            if tx.send(ClientMsg::Chunk(c)).is_err() {
                                return;
                            }
                            notify_agent_wake(&wake);
                        }
                        Ok(None) => {
                            let tr = s.trailers();
                            if !tr.is_empty() {
                                let _ = tx.send(ClientMsg::Trailers(tr));
                                notify_agent_wake(&wake);
                            }
                            let _ = tx.send(ClientMsg::End);
                            notify_agent_wake(&wake);
                            return;
                        }
                        Err(e) => {
                            let _ = tx.send(ClientMsg::Err(e));
                            notify_agent_wake(&wake);
                            return;
                        }
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(ClientMsg::Err(e));
                notify_agent_wake(&wake);
            }
        }
    });
    let async_context = rt.als_context_snapshot();
    let mut roots = vec![Value::Object(this)];
    roots.extend(async_context.keys().copied().map(Value::Object));
    roots.extend(async_context.values().cloned());
    let root_key = next_pending_client_req_root_key();
    rt.retain_host_roots(root_key.clone(), roots);
    PENDING_CLIENT_REQS.with(|v| {
        v.borrow_mut().push(Some(PendingClientReq {
            agent_id: rt.agent_id(),
            rx,
            request: this,
            realm,
            root_key,
            res: None,
            async_context,
        }));
    });
    Ok(())
}

fn ensure_client_request_proto(rt: &mut Runtime) -> ObjectRef {
    if let Value::Object(p) = rt.global_get("__cruft_http_creq_proto") {
        return p;
    }
    let proto = new_object(rt);
    register_method(rt, proto, "write", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Boolean(false)),
        };

        if let Some(v) = args.first() {
            if !matches!(v, Value::Null | Value::Undefined) {
                let mut s = match rt.object_get(this, "__cruft_body") {
                    Value::String(s) => s.to_string(),
                    _ => String::new(),
                };
                s.push_str(&value_to_string(rt, v)?);
                rt.set_engine_sentinel(
                    this,
                    "__cruft_body",
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(s))),
                );

                let declared_cl = match rt.object_get(this, "__cruft_headers") {
                    Value::Object(headers) => {
                        let cl = match rt.object_get(headers, "Content-Length") {
                            Value::Undefined => rt.object_get(headers, "content-length"),
                            v => v,
                        };
                        match cl {
                            Value::Number(n) if n.is_finite() && n >= 0.0 => Some(n as usize),
                            Value::String(s) => s.as_str().trim().parse::<usize>().ok(),
                            _ => None,
                        }
                    }
                    _ => None,
                };
                let body_len = match rt.object_get(this, "__cruft_body") {
                    Value::String(s) => s.as_str().len(),
                    _ => 0,
                };
                let body_complete = declared_cl.is_some_and(|cl| body_len >= cl);
                let dispatch_on_write = matches!(
                    rt.object_get(this, "__cruft_dispatch_on_write"),
                    Value::Boolean(true)
                );
                if body_complete || dispatch_on_write {
                    client_request_dispatch(rt, this)?;
                }
            }
        }
        Ok(Value::Boolean(true))
    });
    register_method(rt, proto, "setHeader", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        if let (Some(n), Some(v)) = (args.first(), args.get(1)) {
            let name = value_to_string(rt, n)?;
            let val = value_to_string(rt, v)?;
            if let Value::Object(h) = rt.object_get(this, "__cruft_headers") {
                let lower = name.to_ascii_lowercase();
                let value = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(val)));
                rt.object_set(h, name, value.clone());
                rt.object_set(h, lower, value);
            }
        }
        Ok(rt.current_this())
    });
    register_method(rt, proto, "getHeader", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let name = match args.first() {
            Some(v) => value_to_string(rt, v)?,
            None => return Ok(Value::Undefined),
        };
        let headers = match rt.object_get(this, "__cruft_headers") {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let direct = rt.object_get(headers, &name);
        if !matches!(direct, Value::Undefined) {
            return Ok(direct);
        }
        Ok(rt.object_get(headers, &name.to_ascii_lowercase()))
    });

    register_method(rt, proto, "removeHeader", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let name = match args.first() {
            Some(v) => value_to_string(rt, v)?,
            None => return Ok(Value::Undefined),
        };
        if let Value::Object(headers) = rt.object_get(this, "__cruft_headers") {
            rt.object_set(headers, name.clone(), Value::Undefined);
            rt.object_set(headers, name.to_ascii_lowercase(), Value::Undefined);
        }
        Ok(Value::Undefined)
    });
    for noop in [
        "setTimeout",
        "flushHeaders",
        "setNoDelay",
        "setSocketKeepAlive",
    ] {
        register_method(rt, proto, noop, |rt, _a| Ok(rt.current_this()));
    }

    register_method(rt, proto, "abort", |rt, _a| {
        if let Value::Object(this) = rt.current_this() {
            let err = client_econnreset_error(rt);
            client_destroy(rt, this, err, true);
        }
        Ok(rt.current_this())
    });

    register_method(rt, proto, "destroy", |rt, a| {
        if let Value::Object(this) = rt.current_this() {
            let err = match a.first() {
                Some(v @ Value::Object(_)) => v.clone(),
                _ => client_econnreset_error(rt),
            };
            client_destroy(rt, this, err, false);
        }
        Ok(rt.current_this())
    });
    register_method(rt, proto, "end", move |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };

        rt.object_set(this, "writable".into(), Value::Boolean(false));
        rt.object_set(this, "writableEnded".into(), Value::Boolean(true));

        if let Some(v) = args.first() {
            if !rt.is_callable(v) && !matches!(v, Value::Null | Value::Undefined) {
                let mut s = match rt.object_get(this, "__cruft_body") {
                    Value::String(s) => s.to_string(),
                    _ => String::new(),
                };
                s.push_str(&value_to_string(rt, v)?);
                rt.set_engine_sentinel(
                    this,
                    "__cruft_body",
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(s))),
                );
            }
        }
        client_request_dispatch(rt, this)?;
        Ok(Value::Undefined)
    });
    rt.define_global_property("__cruft_http_creq_proto", Value::Object(proto));
    proto
}

pub(crate) fn client_request(
    rt: &mut Runtime,
    default_https: bool,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let mut url = String::new();
    let mut method = "GET".to_string();
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut opt: Option<ObjectRef> = None;
    let mut agent_opt: Option<ObjectRef> = None;
    let get_string = |rt: &mut Runtime, id: ObjectRef, key: &str| -> Option<String> {
        match rt.spec_get(&Value::Object(id), key) {
            Ok(Value::String(s)) => Some(s.to_string()),
            _ => None,
        }
    };
    match args.first() {
        Some(Value::String(s)) => url = s.to_string(),
        Some(Value::Object(id)) => {

            if let Some(href) = get_string(rt, *id, "href") {
                url = href;
            } else {
                opt = Some(*id);
            }
        }
        _ => {}
    }

    if let Some(v @ Value::Object(id)) = args.get(1) {
        if !rt.is_callable(v) {
            opt = Some(*id);
        }
    }
    let mut cb = Value::Undefined;
    for a in args {
        if rt.is_callable(a) {
            cb = a.clone();
            break;
        }
    }
    if let Some(o) = opt {
        if let Value::String(m) = rt.object_get(o, "method") {
            method = m.to_uppercase();
        }
        if let Value::Object(h) = rt.object_get(o, "headers") {
            for k in rt.ordinary_own_enumerable_string_keys(h) {
                let v = rt.object_get(h, &k);
                if !matches!(v, Value::Undefined) {
                    headers.push((k, value_to_string(rt, &v)?));
                }
            }
        }
        if let Value::Object(agent) = rt.object_get(o, "agent") {
            agent_opt = Some(agent);
        }
        if url.is_empty() {
            let scheme = get_string(rt, o, "protocol")
                .map(|s| s.trim_end_matches(':').to_string())
                .unwrap_or_else(|| if default_https { "https" } else { "http" }.to_string());
            let host = get_string(rt, o, "hostname")
                .or_else(|| get_string(rt, o, "host"))
                .unwrap_or_else(|| "localhost".to_string());
            let port = match rt.object_get(o, "port") {
                Value::Number(n) => format!(":{}", n as u64),
                Value::String(s) => format!(":{}", s),
                _ => String::new(),
            };
            let path = match rt.object_get(o, "path") {
                Value::String(s) => s.to_string(),
                _ => {
                    let pathname = get_string(rt, o, "pathname").unwrap_or_else(|| "/".to_string());
                    let search = get_string(rt, o, "search").unwrap_or_default();
                    format!("{pathname}{search}")
                }
            };
            url = format!("{scheme}://{host}{port}{path}");
        }
    }

    let req = new_object(rt);
    let l = new_object(rt);
    rt.set_engine_sentinel(req, RES_LISTENERS_SLOT, Value::Object(l));
    attach_emitter(rt, req);

    rt.object_set(req, "aborted".into(), Value::Boolean(false));
    rt.object_set(req, "destroyed".into(), Value::Boolean(false));

    if let Some(o) = opt {
        if let Value::Object(sig) = rt.object_get(o, "signal") {
            rt.set_engine_sentinel(req, "__cruft_signal", Value::Object(sig));
            let req_ref = req;
            let listener =
                crate::register::make_callable(rt, "http.request.onAbort", move |rt, _a| {
                    client_fire_abort(rt, req_ref);
                    Ok(Value::Undefined)
                });
            let add = rt.object_get(sig, "addEventListener");
            if rt.is_callable(&add) {
                let ev = Value::String(std::rc::Rc::new(rusty_js_runtime::value::JsString::from(
                    "abort",
                )));
                let _ =
                    rt.call_function(add, Value::Object(sig), vec![ev, Value::Object(listener)]);
            }
            if matches!(rt.object_get(sig, "aborted"), Value::Boolean(true)) {
                rt.set_engine_sentinel(req, "__cruft_aborted", Value::Boolean(true));
                let req_ref = req;
                rt.enqueue_host_phase_rooted(
                    HostEnqueuePhase::HostCompletionMacrotask,
                    "http.request.preAbort",
                    vec![req],
                    move |rt| {
                        client_fire_abort(rt, req_ref);
                        Ok(())
                    },
                );
            }
        }
    }

    rt.object_set(req, "writable".into(), Value::Boolean(true));
    rt.object_set(req, "writableEnded".into(), Value::Boolean(false));

    rt.object_set(
        req,
        "method".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            method.clone(),
        ))),
    );
    let req_path = crate::http_client::parse_url(&url)
        .map(|(_, _, _, target)| target)
        .unwrap_or_else(|| "/".to_string());
    rt.object_set(
        req,
        "path".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(req_path))),
    );
    rt.set_engine_sentinel(
        req,
        "__cruft_url",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(url))),
    );
    rt.set_engine_sentinel(
        req,
        "__cruft_method",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(method))),
    );
    let hdrs = new_object(rt);
    for (n, v) in &headers {
        rt.object_set(
            hdrs,
            n.clone(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(v.clone()))),
        );
    }
    rt.set_engine_sentinel(req, "__cruft_headers", Value::Object(hdrs));
    rt.set_engine_sentinel(
        req,
        "__cruft_body",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            String::new(),
        ))),
    );
    let _ = crate::node_stubs::async_hooks_emit_init_for_global(
        rt,
        "HTTPCLIENTREQUEST",
        Value::Object(req),
    )?;

    let insecure = opt
        .map(|o| {
            matches!(
                rt.object_get(o, "rejectUnauthorized"),
                Value::Boolean(false)
            )
        })
        .unwrap_or(false);
    rt.set_engine_sentinel(req, "__cruft_insecure", Value::Boolean(insecure));

    if let Some(o) = opt {
        let ca_v = rt.object_get(o, "ca");
        let ca_pem = match ca_v {
            Value::String(s) => Some(s.as_str().to_string()),
            Value::Object(id) => {
                let ts = rt.object_get(id, "toString");
                if rt.is_callable(&ts) {
                    match rt.call_function(ts, Value::Object(id), Vec::new()) {
                        Ok(Value::String(s)) => Some(s.as_str().to_string()),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(pem) = ca_pem {
            rt.set_engine_sentinel(
                req,
                "__cruft_ca",
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(pem))),
            );
        }
    }
    rt.set_engine_sentinel(req, "__cruft_cb", cb);
    if let Some(agent) = agent_opt {
        let socket = http_agent_socket_for_request(rt, agent)?;
        rt.set_engine_sentinel(socket, "__cruft_http_agent_owner", Value::Object(agent));
        rt.object_set(req, "socket".into(), Value::Object(socket));
        rt.object_set(req, "connection".into(), Value::Object(socket));
        rt.set_engine_sentinel(req, CLIENT_SOCKET_SLOT, Value::Object(socket));
        rt.set_engine_sentinel(req, "__cruft_dispatch_on_write", Value::Boolean(true));
    }

    let creq_proto = ensure_client_request_proto(rt);
    rt.set_object_prototype_internal(req, Some(creq_proto));
    Ok(Value::Object(req))
}

enum ClientMsg {
    Head(rusty_http_codec::ResponseHead),
    Upgrade(rusty_http_codec::ResponseHead, u64, Vec<u8>),
    Chunk(Vec<u8>),
    Trailers(Vec<(String, String)>),
    End,
    Err(String),
}

struct PendingClientReq {
    agent_id: AgentId,
    rx: std::sync::mpsc::Receiver<ClientMsg>,
    request: ObjectRef,
    realm: usize,
    root_key: String,

    res: Option<ObjectRef>,

    async_context: HashMap<ObjectRef, Value>,
}

thread_local! {
    static PENDING_CLIENT_REQS: RefCell<Vec<Option<PendingClientReq>>> =
        const { RefCell::new(Vec::new()) };
    static NEXT_PENDING_CLIENT_REQ_ID: RefCell<u64> = const { RefCell::new(1) };
}

fn next_pending_client_req_root_key() -> String {
    let id = NEXT_PENDING_CLIENT_REQ_ID.with(|next| {
        let mut next = next.borrow_mut();
        let id = *next;
        *next = next.wrapping_add(1).max(1);
        id
    });
    format!("http-client:{id}")
}

pub fn client_poll_io(rt: &mut Runtime) -> Result<bool, RuntimeError> {
    use std::sync::mpsc::TryRecvError;
    let agent_id = rt.agent_id();

    let ready: Option<(
        usize,
        ObjectRef,
        usize,
        String,
        Option<ObjectRef>,
        HashMap<ObjectRef, Value>,
        Vec<ClientMsg>,
    )> = PENDING_CLIENT_REQS.with(|v| {
        let mut vv = v.borrow_mut();
        for (i, slot) in vv.iter_mut().enumerate() {
            let Some(p) = slot.as_mut() else { continue };
            if p.agent_id != agent_id {
                continue;
            }
            let msgs = match p.rx.try_recv() {
                Ok(m) => vec![m],
                Err(TryRecvError::Empty) => Vec::new(),
                Err(TryRecvError::Disconnected) => {
                    vec![ClientMsg::Err(
                        "client request thread terminated".to_string(),
                    )]
                }
            };
            if !msgs.is_empty() {
                return Some((
                    i,
                    p.request,
                    p.realm,
                    p.root_key.clone(),
                    p.res,
                    p.async_context.clone(),
                    msgs,
                ));
            }
        }
        None
    });

    let Some((idx, request, realm, root_key, mut res, async_context, msgs)) = ready else {

        return Ok(false);
    };

    let prior = rt.enter_realm(realm);
    let saved_async_context = rt.als_context_replace(async_context);
    let mut finished = false;
    let mut result: Result<(), RuntimeError> = Ok(());

    if matches!(
        rt.object_get(request, "__cruft_aborted"),
        Value::Boolean(true)
    ) {
        finished = true;
    }
    for m in msgs {
        if finished {
            break;
        }
        match m {
            ClientMsg::Upgrade(head, stream_id, head_bytes) => {
                let r = make_incoming(rt, &head);
                let socket = crate::net::make_socket(rt, stream_id, realm);
                let head = crate::net::net_buffer_from_bytes(rt, &head_bytes);
                let _ = emit_res_event(
                    rt,
                    request,
                    "upgrade",
                    vec![Value::Object(r), Value::Object(socket), head],
                );
                finished = true;
            }
            ClientMsg::Head(head) => {
                let r = make_incoming(rt, &head);
                if let Value::Object(socket) = rt.object_get(request, CLIENT_SOCKET_SLOT) {
                    rt.object_set(r, "socket".into(), Value::Object(socket));
                    rt.object_set(r, "connection".into(), Value::Object(socket));
                }
                res = Some(r);

                rt.retain_host_roots(
                    root_key.clone(),
                    vec![Value::Object(request), Value::Object(r)],
                );
                let cb = rt.object_get(request, "__cruft_cb");
                if rt.is_callable(&cb) {
                    let _ = rt.call_function(cb, Value::Undefined, vec![Value::Object(r)]);
                }
                let _ = emit_res_event(rt, request, "response", vec![Value::Object(r)]);

                if matches!(
                    rt.object_get(r, "__cruft_res_flowing"),
                    Value::Boolean(true)
                ) {
                    deliver_res_body(rt, r);
                }
            }
            ClientMsg::Chunk(bytes) => {
                if let Some(r) = res {
                    res_push_chunk(rt, r, &bytes);
                }
            }
            ClientMsg::Trailers(pairs) => {

                if let Some(r) = res {
                    let trailers = new_object(rt);
                    let raw = rt.alloc_object(rusty_js_runtime::Object::new_array());
                    let mut n = 0.0;
                    for (name, value) in &pairs {
                        rt.object_set(
                            trailers,
                            name.to_ascii_lowercase(),
                            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                                value.clone(),
                            ))),
                        );
                        rt.object_set(
                            raw,
                            n.to_string(),
                            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                                name.clone(),
                            ))),
                        );
                        rt.object_set(
                            raw,
                            (n + 1.0).to_string(),
                            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                                value.clone(),
                            ))),
                        );
                        n += 2.0;
                    }
                    rt.object_set(raw, "length".into(), Value::Number(n));
                    rt.object_set(r, "trailers".into(), Value::Object(trailers));
                    rt.object_set(r, "rawTrailers".into(), Value::Object(raw));
                }
            }
            ClientMsg::End => {
                if let Some(r) = res {
                    res_mark_ended(rt, r);
                    if let Value::Object(socket) = rt.object_get(r, "socket") {
                        http_agent_mark_socket_free(rt, socket);
                    }
                }
                if let Err(e) = emit_http_resource_destroy_once(rt, request) {
                    result = Err(e);
                    break;
                }
                finished = true;
            }
            ClientMsg::Err(e) => {

                match res {
                    Some(r) => {
                        let ev = client_error_value(rt, e);
                        let _ = emit_res_event(rt, r, "error", vec![ev]);
                    }
                    None => {

                        let ev = client_connect_error_value(rt, &e, request);
                        let _ = emit_res_event(rt, request, "error", vec![ev]);
                    }
                }
                finished = true;
            }
        }
    }
    rt.als_context_replace(saved_async_context);
    rt.exit_realm(prior);
    result?;

    if finished {

        if !matches!(
            rt.object_get(request, "__cruft_aborted"),
            Value::Boolean(true)
        ) && !matches!(
            rt.object_get(request, "__cruft_completed"),
            Value::Boolean(true)
        ) {
            rt.set_engine_sentinel(request, "__cruft_completed", Value::Boolean(true));
            let _ = emit_res_event(rt, request, "close", Vec::new());
        }
        rt.release_host_roots(&root_key);
        PENDING_CLIENT_REQS.with(|v| {
            let mut vv = v.borrow_mut();
            if let Some(slot) = vv.get_mut(idx) {
                *slot = None;
            }
            vv.retain(|x| x.is_some());
        });
    } else {

        PENDING_CLIENT_REQS.with(|v| {
            let mut vv = v.borrow_mut();
            if let Some(Some(p)) = vv.get_mut(idx) {
                if p.agent_id == agent_id {
                    p.res = res;
                }
            }
        });
    }
    Ok(true)
}

pub fn has_pending_client(rt: &Runtime) -> bool {
    let agent_id = rt.agent_id();
    PENDING_CLIENT_REQS.with(|v| {
        v.borrow()
            .iter()
            .any(|x| x.as_ref().is_some_and(|p| p.agent_id == agent_id))
    })
}

fn make_http_namespace(rt: &mut Runtime, net_cap: caps::Net) -> ObjectRef {
    let http = new_object(rt);

    register_method(rt, http, "request", |rt, args| {
        client_request(rt, false, args)
    });
    register_method(rt, http, "get", |rt, args| {
        let req = client_request(rt, false, args)?;
        if let Value::Object(id) = &req {

            let id = *id;
            rt.enqueue_microtask_rooted("http.get.end", vec![id], move |rt| {
                let end = rt.object_get(id, "end");
                if rt.is_callable(&end) {
                    rt.call_function(end, Value::Object(id), Vec::new())?;
                }
                Ok(())
            });
        }
        Ok(req)
    });
    register_method(rt, http, "createServer", move |rt, args| {
        let handler = match args {
            [first, ..] if rt.is_callable(first) => first.clone(),
            [_, second, ..] if rt.is_callable(second) => second.clone(),
            _ => Value::Undefined,
        };
        let server = make_server_object(rt, handler, net_cap.clone())?;
        Ok(Value::Object(server))
    });

    register_method(rt, http, "Agent", |rt, args| {

        let id = match rt.current_this() {
            Value::Object(this) => this,
            _ => rt.alloc_object(rusty_js_runtime::Object::new_ordinary()),
        };

        http_agent_reflect_options(rt, id, args.first());
        Ok(Value::Object(id))
    });

    if let Value::Object(agent) = rt.object_get(http, "Agent") {
        crate::register::make_subclassable(
            rt,
            agent,
            crate::register::proto_of_global_ctor(rt, "events"),
        );
    }

    let codes = new_object(rt);
    for (code, msg) in &[
        (100, "Continue"),
        (101, "Switching Protocols"),
        (102, "Processing"),
        (103, "Early Hints"),
        (200, "OK"),
        (201, "Created"),
        (202, "Accepted"),
        (203, "Non-Authoritative Information"),
        (204, "No Content"),
        (205, "Reset Content"),
        (206, "Partial Content"),
        (207, "Multi-Status"),
        (208, "Already Reported"),
        (226, "IM Used"),
        (300, "Multiple Choices"),
        (301, "Moved Permanently"),
        (302, "Found"),
        (303, "See Other"),
        (304, "Not Modified"),
        (305, "Use Proxy"),
        (307, "Temporary Redirect"),
        (308, "Permanent Redirect"),
        (400, "Bad Request"),
        (401, "Unauthorized"),
        (402, "Payment Required"),
        (403, "Forbidden"),
        (404, "Not Found"),
        (405, "Method Not Allowed"),
        (406, "Not Acceptable"),
        (407, "Proxy Authentication Required"),
        (408, "Request Timeout"),
        (409, "Conflict"),
        (410, "Gone"),
        (411, "Length Required"),
        (412, "Precondition Failed"),
        (413, "Payload Too Large"),
        (414, "URI Too Long"),
        (415, "Unsupported Media Type"),
        (416, "Range Not Satisfiable"),
        (417, "Expectation Failed"),
        (418, "I'm a Teapot"),
        (421, "Misdirected Request"),
        (422, "Unprocessable Entity"),
        (423, "Locked"),
        (424, "Failed Dependency"),
        (425, "Too Early"),
        (426, "Upgrade Required"),
        (428, "Precondition Required"),
        (429, "Too Many Requests"),
        (431, "Request Header Fields Too Large"),
        (451, "Unavailable For Legal Reasons"),
        (500, "Internal Server Error"),
        (501, "Not Implemented"),
        (502, "Bad Gateway"),
        (503, "Service Unavailable"),
        (504, "Gateway Timeout"),
        (505, "HTTP Version Not Supported"),
        (506, "Variant Also Negotiates"),
        (507, "Insufficient Storage"),
        (508, "Loop Detected"),
        (509, "Bandwidth Limit Exceeded"),
        (510, "Not Extended"),
        (511, "Network Authentication Required"),
    ] {
        set_constant(
            rt,
            codes,
            &code.to_string(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from((*msg)))),
        );
    }
    set_constant(rt, http, "STATUS_CODES", Value::Object(codes));

    let methods = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
    let names = [
        "ACL",
        "BIND",
        "CHECKOUT",
        "CONNECT",
        "COPY",
        "DELETE",
        "GET",
        "HEAD",
        "LINK",
        "LOCK",
        "M-SEARCH",
        "MERGE",
        "MKACTIVITY",
        "MKCALENDAR",
        "MKCOL",
        "MOVE",
        "NOTIFY",
        "OPTIONS",
        "PATCH",
        "POST",
        "PROPFIND",
        "PROPPATCH",
        "PURGE",
        "PUT",
        "QUERY",
        "REBIND",
        "REPORT",
        "SEARCH",
        "SOURCE",
        "SUBSCRIBE",
        "TRACE",
        "UNBIND",
        "UNLINK",
        "UNLOCK",
        "UNSUBSCRIBE",
    ];
    for (i, n) in names.iter().enumerate() {
        rt.object_set(
            methods,
            i.to_string(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from((*n)))),
        );
    }
    rt.object_set(methods, "length".into(), Value::Number(names.len() as f64));
    set_constant(rt, http, "METHODS", Value::Object(methods));

    let sr_proto = ensure_response_proto(rt);
    let sr_ctor = crate::register::make_callable_rooted(
        rt,
        "ServerResponse",
        vec![sr_proto],
        move |rt, args| {
            match args.first() {
                None | Some(Value::Undefined) => {
                    return Err(RuntimeError::TypeError(
                        "Cannot read properties of undefined (reading 'method')".into(),
                    ));
                }
                Some(Value::Null) => {
                    return Err(RuntimeError::TypeError(
                        "Cannot read properties of null (reading 'method')".into(),
                    ));
                }
                _ => {}
            }
            let obj = make_response_object(rt);
            rt.set_object_prototype_internal(obj, Some(sr_proto));

            if let Some(v @ Value::Object(_)) = args.first() {
                rt.object_set(obj, "req".into(), v.clone());
            }
            Ok(Value::Object(obj))
        },
    );
    rt.set_own_frozen_property(sr_ctor, "prototype".into(), Value::Object(sr_proto));
    rt.obj_mut(sr_proto)
        .set_own_internal("constructor".into(), Value::Object(sr_ctor));
    set_constant(rt, http, "ServerResponse", Value::Object(sr_ctor));

    let im_proto = new_object(rt);
    let im_ctor = crate::register::make_callable_rooted(
        rt,
        "IncomingMessage",
        vec![im_proto],
        move |rt, _args| {
            let obj = make_constructed_incoming_message(rt);
            rt.set_object_prototype_internal(obj, Some(im_proto));
            Ok(Value::Object(obj))
        },
    );
    rt.set_own_frozen_property(im_ctor, "prototype".into(), Value::Object(im_proto));
    rt.obj_mut(im_proto)
        .set_own_internal("constructor".into(), Value::Object(im_ctor));
    set_constant(rt, http, "IncomingMessage", Value::Object(im_ctor));

    rt.define_global_property("__cruft_http_im_proto", Value::Object(im_proto));

    for class_name in &["Server", "ClientRequest"] {

        let proto = if *class_name == "ClientRequest" {
            ensure_client_request_proto(rt)
        } else {
            new_object(rt)
        };
        let ctor = crate::register::make_callable(rt, class_name, |_rt, _args| {
            Err(RuntimeError::TypeError(
                "node:http class constructor not yet implemented (Tier-Ω.5.xxxxxx stub)".into(),
            ))
        });
        rt.set_own_frozen_property(ctor, "prototype".into(), Value::Object(proto));
        rt.obj_mut(proto)
            .set_own_internal("constructor".into(), Value::Object(ctor));
        if *class_name == "Server" {

            rt.define_global_property("__cruft_http_server_proto", Value::Object(proto));
        }
        set_constant(rt, http, class_name, Value::Object(ctor));
    }

    http
}

fn process_request_pure(
    rt: &mut Runtime,
    server: &ActiveHttpServer,
    parsed: &rusty_http_codec::ParsedRequest,
    handler: Value,
    keep: bool,

    stream_id: Option<u64>,
) -> (Vec<u8>, bool) {
    let prior = rt.enter_realm(server.handler_realm);

    let request = new_object(rt);
    rt.object_set(
        request,
        "method".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            parsed.method.clone(),
        ))),
    );

    let host_hdr = parsed
        .headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("host"))
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| "localhost".to_string());
    let abs_url = format!("http://{}{}", host_hdr, parsed.target);
    rt.object_set(
        request,
        "url".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(abs_url))),
    );

    let headers_init = new_object(rt);
    for (n, v) in &parsed.headers {
        rt.object_set(
            headers_init,
            n.to_ascii_lowercase(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(v.clone()))),
        );
    }
    let headers_ctor = rt.global_get("Headers");
    let headers_val = if rt.is_callable(&headers_ctor) {
        rt.construct(headers_ctor, vec![Value::Object(headers_init)])
            .unwrap_or(Value::Object(headers_init))
    } else {
        Value::Object(headers_init)
    };
    rt.object_set(request, "headers".into(), headers_val);
    let body_str = String::from_utf8_lossy(&parsed.body).into_owned();
    rt.object_set(
        request,
        "body".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(body_str))),
    );

    register_method(rt, request, "text", |rt, _args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let body = rt.object_get(this, "body");
        let p = rusty_js_runtime::promise::new_promise(rt);
        rusty_js_runtime::promise::resolve_promise(rt, p, body);
        Ok(Value::Object(p))
    });
    register_method(rt, request, "json", |rt, _args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let body = rt.object_get(this, "body");
        let parsed_v = if let Value::Object(json) = rt.global_get("JSON") {
            let parse = rt.object_get(json, "parse");
            if rt.is_callable(&parse) {
                rt.call_function(parse, Value::Undefined, vec![body])?
            } else {
                Value::Undefined
            }
        } else {
            Value::Undefined
        };
        let p = rusty_js_runtime::promise::new_promise(rt);
        rusty_js_runtime::promise::resolve_promise(rt, p, parsed_v);
        Ok(Value::Object(p))
    });

    let handler_res = rt.call_function(handler, Value::Undefined, vec![Value::Object(request)]);

    if let (Ok(Value::Object(pid)), Some(sid)) = (&handler_res, stream_id) {
        let pending = matches!(
            &rt.obj(*pid).internal_kind,
            rusty_js_runtime::value::InternalKind::Promise(ps)
                if ps.status == rusty_js_runtime::value::PromiseStatus::Pending
        );
        if pending {
            let pid = *pid;
            let handler_realm = server.handler_realm;
            let on_ok = native_function(rt, "cruftHttpAsyncResponse", move |rt, args| {
                let v = args.first().cloned().unwrap_or(Value::Undefined);
                let accepted_ws = crate::ws::take_pending_accept_for_request(rt, request)
                    .map(|(_, handlers, config)| (handlers, config));
                let (status, headers, body) = interpret_pure_response(rt, v);
                let resp = finalize_pure_response(status, headers, body, false);
                let _ = rusty_sockets::stream_write_all(sid, &resp);
                if status == 101 {
                    let (handlers, config) = accepted_ws.unwrap_or((None, Default::default()));
                    let _ = crate::ws::register_server_session(
                        rt,
                        sid,
                        handler_realm,
                        handlers,
                        config,
                    );
                } else {
                    let _ = rusty_sockets::handle_close(sid);
                }
                Ok(Value::Undefined)
            });
            let on_err = native_function(rt, "cruftHttpAsyncReject", move |_rt, _args| {
                let resp = finalize_pure_response(
                    500,
                    Vec::new(),
                    b"Internal Server Error".to_vec(),
                    false,
                );
                let _ = rusty_sockets::stream_write_all(sid, &resp);
                let _ = rusty_sockets::handle_close(sid);
                Ok(Value::Undefined)
            });
            let then = rt.object_get(pid, "then");
            let _ = rt.call_function(then, Value::Object(pid), vec![on_ok, on_err]);
            WENT_ASYNC.with(|c| c.set(true));
            rt.exit_realm(prior);
            return (Vec::new(), false);
        }
    }

    let result = handler_res.and_then(|v| settle_value(rt, v));
    let accepted_ws = result.as_ref().ok().and_then(|_| {
        crate::ws::take_pending_accept_for_request(rt, request)
            .map(|(_, handlers, config)| (handlers, config))
    });
    let (status, headers, body) = match result {
        Ok(v) => interpret_pure_response(rt, v),
        Err(_) => (500, Vec::new(), b"Internal Server Error".to_vec()),
    };
    rt.exit_realm(prior);
    let resp = finalize_pure_response(status, headers, body, keep);
    if status == 101 {
        if let Some(sid) = stream_id {
            let _ = rusty_sockets::stream_write_all(sid, &resp);
            let (handlers, config) = accepted_ws.unwrap_or((None, Default::default()));
            let _ =
                crate::ws::register_server_session(rt, sid, server.handler_realm, handlers, config);
            WENT_ASYNC.with(|c| c.set(true));
            return (Vec::new(), false);
        }
        return (resp, false);
    }
    let keep = keep && !response_says_close(&resp);
    (resp, keep)
}

fn finalize_pure_response(
    status: u16,
    mut headers: Vec<(String, String)>,
    body: Vec<u8>,
    keep: bool,
) -> Vec<u8> {
    if !headers
        .iter()
        .any(|(n, _)| n.eq_ignore_ascii_case("connection"))
    {
        headers.push((
            "connection".into(),
            if keep { "keep-alive" } else { "close" }.into(),
        ));
    }
    let reason = status_reason(status);
    rusty_http_codec::serialize_response(status, reason, &headers, &body)
}

fn status_reason(status: u16) -> &'static str {
    match status {
        100 => "Continue",
        101 => "Switching Protocols",
        102 => "Processing",
        103 => "Early Hints",
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        203 => "Non-Authoritative Information",
        204 => "No Content",
        205 => "Reset Content",
        206 => "Partial Content",
        207 => "Multi-Status",
        208 => "Already Reported",
        226 => "IM Used",
        300 => "Multiple Choices",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        305 => "Use Proxy",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        402 => "Payment Required",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        407 => "Proxy Authentication Required",
        408 => "Request Timeout",
        409 => "Conflict",
        410 => "Gone",
        411 => "Length Required",
        412 => "Precondition Failed",
        413 => "Payload Too Large",
        414 => "URI Too Long",
        415 => "Unsupported Media Type",
        416 => "Range Not Satisfiable",
        417 => "Expectation Failed",
        418 => "I'm a Teapot",
        421 => "Misdirected Request",
        422 => "Unprocessable Entity",
        423 => "Locked",
        424 => "Failed Dependency",
        425 => "Too Early",
        426 => "Upgrade Required",
        428 => "Precondition Required",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        451 => "Unavailable For Legal Reasons",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        505 => "HTTP Version Not Supported",
        506 => "Variant Also Negotiates",
        507 => "Insufficient Storage",
        508 => "Loop Detected",
        509 => "Bandwidth Limit Exceeded",
        510 => "Not Extended",
        511 => "Network Authentication Required",
        _ => "unknown",
    }
}

fn settle_value(rt: &mut Runtime, v: Value) -> Result<Value, RuntimeError> {
    use rusty_js_runtime::value::{InternalKind, PromiseStatus};
    let id = match &v {
        Value::Object(id) if matches!(rt.obj(*id).internal_kind, InternalKind::Promise(_)) => *id,
        _ => return Ok(v),
    };
    let mut ticks = 0u32;
    loop {
        if let InternalKind::Promise(ps) = &rt.obj(id).internal_kind {
            match ps.status {
                PromiseStatus::Fulfilled => return Ok(ps.value.clone()),
                PromiseStatus::Rejected => {
                    return Err(RuntimeError::TypeError(
                        "cruft:http: handler promise rejected".into(),
                    ))
                }
                PromiseStatus::Pending => {}
            }
        }
        match rusty_js_runtime::job_queue::pump_one_tick(rt) {
            Ok(true) => {}
            Ok(false) => {

                let napi = rusty_js_runtime::napi::drain_main_inbox(rt) > 0;
                let due = crate::timer::drain_due_pairs_for_runtime(rt);
                if !due.is_empty() {
                    for (id, cb, args, repeat, async_context, async_resource) in due {
                        let roots = crate::timer::roots_for_callback_with_resource(
                            &cb,
                            &args,
                            async_resource,
                        );
                        rt.enqueue_host_phase_rooted_with_async_context(
                            HostEnqueuePhase::TimerCallbackMacrotask,
                            "timer callback",
                            roots,
                            async_context,
                            move |rt| {
                                if let Some(resource) = async_resource {
                                    let _ = crate::node_stubs::async_hooks_call_with_global_resource_and_microtasks(
                                        rt,
                                        resource,
                                        cb,
                                        Value::Undefined,
                                        args,
                                    );
                                } else {
                                    let _ = rt.call_function(cb, Value::Undefined, args);
                                }
                                if !repeat {
                                    if let Some(resource) = async_resource {
                                    crate::node_stubs::async_hooks_emit_destroy_for_global(
                                        rt,
                                        Value::Object(resource),
                                    )?;
                                    }
                                }
                                Ok(())
                            },
                        );
                        if !repeat {
                            crate::timer::release_roots(rt, id);
                        }
                    }
                } else if !napi {

                }
            }
            Err(e) => return Err(e),
        }
        ticks += 1;
        if ticks > 500_000 {
            return Err(RuntimeError::TypeError(
                "cruft:http: handler did not settle".into(),
            ));
        }
    }
}

fn interpret_pure_response(rt: &mut Runtime, v: Value) -> (u16, Vec<(String, String)>, Vec<u8>) {
    match v {
        Value::String(s) => (
            200,
            vec![("content-type".into(), "text/plain; charset=utf-8".into())],
            s.as_bytes().to_vec(),
        ),
        Value::Object(o) => {
            let status = match rt.object_get(o, "status") {
                Value::Number(n) => n as u16,
                _ => 200,
            };

            let body = match rt.object_get(o, "__body_bytes") {
                Value::String(s) => s.as_str().chars().map(|c| c as u8).collect(),
                _ => match rt.object_get(o, "body") {
                    Value::String(s) => s.as_bytes().to_vec(),
                    Value::Undefined => Vec::new(),
                    other => value_to_bytes(rt, &other).unwrap_or_default(),
                },
            };
            let mut headers = Vec::new();
            if let Value::Object(hid) = rt.object_get(o, "headers") {

                let bag = match rt.object_get(hid, "__headers") {
                    Value::Object(b) => b,
                    _ => hid,
                };
                for key in rt.ordinary_own_enumerable_string_keys(bag) {
                    let val = rt.object_get(bag, &key);
                    if let Ok(s) = value_to_string(rt, &val) {
                        headers.push((key.clone(), s));
                    }
                }
            }
            (status, headers, body)
        }
        _ => (200, Vec::new(), Vec::new()),
    }
}

pub(crate) fn set_cruft_fetch_handler(rt: &mut Runtime, server_object: ObjectRef, handler: Value) {
    set_internal_slot(rt, server_object, CRUFT_FETCH_HANDLER_SLOT, handler);
}

fn make_cruft_http_server(
    rt: &mut Runtime,
    handler: Value,
    net_cap: caps::Net,
) -> Result<ObjectRef, RuntimeError> {
    let server = make_server_object(rt, Value::Undefined, net_cap)?;
    set_internal_slot(rt, server, CRUFT_FETCH_HANDLER_SLOT, handler);
    Ok(server)
}

pub fn install_canonical(rt: &mut Runtime) {
    let ns = new_object(rt);
    let net_cap = caps::Net::full();
    let cap = net_cap.clone();
    register_method(rt, ns, "createServer", move |rt, args| {
        let handler = match args.first() {
            Some(h) if rt.is_callable(h) => h.clone(),
            _ => Value::Undefined,
        };
        let server = make_cruft_http_server(rt, handler, cap.clone())?;
        Ok(Value::Object(server))
    });

    register_method(rt, ns, "createSecureServer", |rt, args| {
        let handler = args.get(1).cloned().unwrap_or(Value::Undefined);
        let server = crate::tls::do_create_https_server(rt, args)?;
        if let Value::Object(sid) = server {
            set_internal_slot(rt, sid, CRUFT_FETCH_HANDLER_SLOT, handler);
        }
        Ok(server)
    });

    register_method(rt, ns, "request", |rt, args| {
        client_request(rt, false, args)
    });
    rt.define_global_property("__cruft_http", Value::Object(ns));

    let serve_ns = new_object(rt);
    register_method(rt, serve_ns, "staticDir", |rt, args| {
        let root = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "cruft:serve.staticDir: root path string required".into(),
                ))
            }
        };
        let options = args.get(1).cloned().unwrap_or(Value::Undefined);
        let index = match options {
            Value::Object(id) => match rt.object_get(id, "index") {
                Value::String(s) => Some(s.as_str().to_string()),
                Value::Boolean(false) | Value::Null => None,
                _ => Some("index.html".to_string()),
            },
            _ => Some("index.html".to_string()),
        };
        let resolver = make_static_dir_resolver(rt, root, index);
        Ok(Value::Object(resolver))
    });
    let serve_cap = net_cap.clone();
    register_method(rt, serve_ns, "serve", move |rt, args| {
        let opts = match args.first() {
            Some(Value::Object(id)) => *id,
            _ => {
                return Err(RuntimeError::TypeError(
                    "cruft:serve.serve: options object required".into(),
                ))
            }
        };
        serve_from_options(rt, opts, serve_cap.clone(), "cruft:serve.serve")
    });
    rt.define_global_property("__cruft_serve", Value::Object(serve_ns));

    let bun_ns = new_object(rt);
    let bun_cap = net_cap.clone();
    register_method(rt, bun_ns, "serve", move |rt, args| {
        let opts = match args.first() {
            Some(Value::Object(id)) => *id,
            _ => {
                return Err(RuntimeError::TypeError(
                    "Bun.serve: options object required".into(),
                ))
            }
        };
        serve_from_options(rt, opts, bun_cap.clone(), "Bun.serve")
    });
    rt.define_global_property("Bun", Value::Object(bun_ns));
}

pub fn install(rt: &mut Runtime) {
    let http = make_http_namespace(rt, caps::Net::none());
    register_method(rt, http, "__cruft_makeLoopbackFacade", |rt, _args| {
        let ns = make_http_namespace(rt, caps::Net::loopback_server());
        rt.set_engine_sentinel(ns, BOUNDARY_CALLABLE_FACADE_SLOT, Value::Boolean(true));
        Ok(Value::Object(ns))
    });
    let facade_factory =
        crate::register::make_callable(rt, "__cruft_makeHttpFacade", |rt, _args| {
            let ns = make_http_namespace(rt, caps::Net::loopback_server());
            rt.set_engine_sentinel(ns, BOUNDARY_CALLABLE_FACADE_SLOT, Value::Boolean(true));
            Ok(Value::Object(ns))
        });
    rt.define_global_property("__cruft_makeHttpFacade", Value::Object(facade_factory));
    {
        let c = crate::register::make_callable(rt, "CloseEvent", |rt, _a| Ok(rt.current_this()));
        let p = new_object(rt);
        rt.object_set(p, "constructor".into(), Value::Object(c));
        rt.object_set(c, "prototype".into(), Value::Object(p));
        rt.object_set(http, "CloseEvent".into(), Value::Object(c));
    }
    {
        let c = crate::register::make_callable(rt, "MessageEvent", |rt, _a| Ok(rt.current_this()));
        let p = new_object(rt);
        rt.object_set(p, "constructor".into(), Value::Object(c));
        rt.object_set(c, "prototype".into(), Value::Object(p));
        rt.object_set(http, "MessageEvent".into(), Value::Object(c));
    }
    {
        let c =
            crate::register::make_callable(rt, "OutgoingMessage", |rt, _a| Ok(rt.current_this()));
        let p = new_object(rt);
        rt.object_set(p, "constructor".into(), Value::Object(c));
        rt.object_set(c, "prototype".into(), Value::Object(p));
        rt.object_set(http, "OutgoingMessage".into(), Value::Object(c));
    }
    {
        let c = crate::register::make_callable(rt, "WebSocket", |rt, _a| Ok(rt.current_this()));
        let p = new_object(rt);
        rt.object_set(p, "constructor".into(), Value::Object(c));
        rt.object_set(c, "prototype".into(), Value::Object(p));
        rt.object_set(http, "WebSocket".into(), Value::Object(c));
    }
    {

        let o = new_object(rt);
        if let Value::Object(agent_ctor) = rt.object_get(http, "Agent") {
            if let Value::Object(agent_proto) = rt.object_get(agent_ctor, "prototype") {
                rt.set_object_prototype_internal(o, Some(agent_proto));
            }
        }
        http_agent_reflect_options(rt, o, None);
        rt.object_set(
            o,
            "protocol".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from("http:"))),
        );
        rt.object_set(http, "globalAgent".into(), Value::Object(o));
    }
    rt.object_set(http, "maxHeaderSize".into(), Value::Number(16384f64));
    register_method(rt, http, "_connectionListener", |_rt, _a| {
        Ok(Value::Undefined)
    });
    register_method(rt, http, "setGlobalProxyFromEnv", |_rt, _a| {
        Ok(Value::Undefined)
    });
    register_method(rt, http, "setMaxIdleHTTPParsers", |_rt, _a| {
        Ok(Value::Undefined)
    });
    register_method(rt, http, "validateHeaderName", |rt, a| {

        let name = match a.first() {
            Some(Value::String(s)) => s.to_string(),
            _ => String::new(),
        };
        let is_string = matches!(a.first(), Some(Value::String(_)));
        let valid = is_string
            && !name.is_empty()
            && name.chars().all(|c| {
                c.is_ascii() && {
                    let b = c as u8;
                    b.is_ascii_alphanumeric()
                        || matches!(
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
                        )
                }
            });
        if !valid {
            let msg = format!("Header name must be a valid HTTP token [\"{name}\"]");
            return Err(RuntimeError::Thrown(coded_error(
                rt,
                "ERR_INVALID_HTTP_TOKEN",
                &msg,
            )));
        }
        Ok(Value::Undefined)
    });
    register_method(rt, http, "validateHeaderValue", |rt, a| {

        let name = match a.first() {
            Some(Value::String(s)) => s.to_string(),
            _ => String::new(),
        };
        match a.get(1) {
            None | Some(Value::Undefined) => {
                let msg = format!("Invalid value \"undefined\" for header \"{name}\"");
                Err(RuntimeError::Thrown(coded_error(
                    rt,
                    "ERR_HTTP_INVALID_HEADER_VALUE",
                    &msg,
                )))
            }
            Some(Value::String(s)) => {
                let bad = s.to_string().chars().any(|c| {
                    let u = c as u32;
                    !(u == 0x09 || (0x20..=0x7e).contains(&u) || (0x80..=0xff).contains(&u))
                });
                if bad {
                    let msg = format!("Invalid character in header content [\"{name}\"]");
                    return Err(RuntimeError::Thrown(coded_error(
                        rt,
                        "ERR_INVALID_CHAR",
                        &msg,
                    )));
                }
                Ok(Value::Undefined)
            }
            _ => Ok(Value::Undefined),
        }
    });
    {
        let common = new_object(rt);
        let parser_ctor = crate::register::make_callable(rt, "HTTPParser", |rt, _args| {
            Ok(Value::Object(new_object(rt)))
        });
        rt.object_set(parser_ctor, "REQUEST".into(), Value::Number(0.0));
        rt.object_set(parser_ctor, "RESPONSE".into(), Value::Number(1.0));
        let parsers = new_object(rt);
        register_method(rt, parsers, "alloc", |rt, _args| {
            let parser = new_object(rt);
            register_method(rt, parser, "initialize", |rt, _args| Ok(rt.current_this()));
            register_method(rt, parser, "close", |rt, _args| Ok(rt.current_this()));
            Ok(Value::Object(parser))
        });
        register_method(rt, common, "freeParser", |_rt, _args| Ok(Value::Undefined));
        rt.object_set(common, "parsers".into(), Value::Object(parsers));
        rt.object_set(common, "HTTPParser".into(), Value::Object(parser_ctor));
        rt.define_global_property("_http_common", Value::Object(common));
    }
    let _ = rt.delete_own_via(
        &Value::Object(http),
        &Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "__cruft_makeLoopbackFacade".to_string(),
        ))),
    );
    rt.define_global_property("http", Value::Object(http));
}

#[cfg(test)]
mod tests {
    use super::{
        drain_streaming_conns, get_server_for_runtime, has_streaming_conns_for_runtime,
        notify_agent_wake, remove_server_for_runtime, request_complete, set_server_refed,
        ActiveHttpServer, PendingClientReq, StreamConn, HTTP_SERVERS, PENDING_CLIENT_REQS,
        STREAMING_CONNS,
    };
    use rusty_js_runtime::{AgentId, Object, Runtime};
    use std::collections::HashMap;

    #[test]
    fn http_client_completion_notify_wakes_owner_runtime() {
        let rt = Runtime::new_with_agent_id(AgentId::from_raw(403));
        let wake = rt.agent_wake_handle();
        let before = rt.agent_wake_generation();

        notify_agent_wake(&wake);

        assert_ne!(
            rt.agent_wake_generation(),
            before,
            "HTTP client completion producer must advance the owner runtime wake"
        );
    }

    #[test]
    fn request_complete_uses_codec_framing_for_smuggling_rejection() {
        let cl_te = b"POST / HTTP/1.1\r\nHost: x\r\nContent-Length: 4\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\nG";
        assert!(request_complete(cl_te));

        let dup_cl =
            b"POST / HTTP/1.1\r\nHost: x\r\nContent-Length: 4\r\nContent-Length: 5\r\n\r\nhello";
        assert!(request_complete(dup_cl));
    }

    #[test]
    fn request_complete_waits_for_incomplete_content_length_body() {
        let partial = b"POST / HTTP/1.1\r\nHost: x\r\nContent-Length: 5\r\n\r\nhe";
        assert!(!request_complete(partial));

        let complete = b"POST / HTTP/1.1\r\nHost: x\r\nContent-Length: 5\r\n\r\nhello";
        assert!(request_complete(complete));
    }

    #[test]
    fn request_complete_waits_for_incomplete_chunked_body() {
        let partial = b"POST / HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhe";
        assert!(!request_complete(partial));

        let complete =
            b"POST / HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n";
        assert!(request_complete(complete));
    }

    #[test]
    fn pending_client_registry_is_scoped_by_runtime_agent_id() {
        let mut rt_a = Runtime::new_with_agent_id(AgentId::from_raw(401));
        let mut rt_b = Runtime::new_with_agent_id(AgentId::from_raw(402));
        let request_b = rt_b.alloc_object(Object::new_ordinary());
        let (_tx_b, rx_b) = std::sync::mpsc::channel();

        PENDING_CLIENT_REQS.with(|v| {
            v.borrow_mut().clear();
            v.borrow_mut().push(Some(PendingClientReq {
                agent_id: rt_b.agent_id(),
                rx: rx_b,
                request: request_b,
                realm: rt_b.current_realm,
                root_key: "http-client:test-agent-b".to_string(),
                res: None,
                async_context: HashMap::new(),
            }));
        });

        assert!(!super::has_pending_client(&rt_a));
        assert!(super::has_pending_client(&rt_b));
        assert!(
            !super::client_poll_io(&mut rt_a).expect("poll agent A"),
            "agent A must not harvest agent B's pending client request"
        );
        assert!(
            super::has_pending_client(&rt_b),
            "agent B pending client request must remain after agent A poll"
        );

        PENDING_CLIENT_REQS.with(|v| v.borrow_mut().clear());
        rt_b.release_host_roots("http-client:test-agent-b");
    }

    #[test]
    fn server_registry_is_scoped_by_runtime_agent_id() {
        let rt_a = Runtime::new_with_agent_id(AgentId::from_raw(601));
        let mut rt_b = Runtime::new_with_agent_id(AgentId::from_raw(602));
        let server_b = rt_b.alloc_object(Object::new_ordinary());

        HTTP_SERVERS.with(|v| {
            v.borrow_mut().clear();
            v.borrow_mut().push(Some(ActiveHttpServer {
                agent_id: rt_b.agent_id(),
                listener_handle: 0,
                bound_addr: "127.0.0.1:0".to_string(),
                handler_realm: rt_b.current_realm,
                server_object: server_b,
                refed: true,
            }));
        });

        assert!(get_server_for_runtime(&rt_a, 0).is_none());
        assert!(get_server_for_runtime(&rt_b, 0).is_some());
        assert!(!set_server_refed(&rt_a, 0, false));
        assert!(
            get_server_for_runtime(&rt_b, 0)
                .expect("agent B server")
                .refed
        );
        assert!(set_server_refed(&rt_b, 0, false));
        assert!(
            !get_server_for_runtime(&rt_b, 0)
                .expect("agent B server")
                .refed
        );
        assert!(remove_server_for_runtime(&rt_a, 0).is_none());
        assert!(remove_server_for_runtime(&rt_b, 0).is_some());

        HTTP_SERVERS.with(|v| v.borrow_mut().clear());
    }

    #[test]
    fn streaming_registry_is_scoped_by_runtime_agent_id() {
        let mut rt_a = Runtime::new_with_agent_id(AgentId::from_raw(611));
        let mut rt_b = Runtime::new_with_agent_id(AgentId::from_raw(612));
        let res_b = rt_b.alloc_object(Object::new_ordinary());
        rt_b.object_set(
            res_b,
            "__cruft_http_ended".into(),
            rusty_js_runtime::Value::Boolean(true),
        );

        STREAMING_CONNS.with(|v| {
            v.borrow_mut().clear();
            v.borrow_mut().push(StreamConn {
                agent_id: rt_b.agent_id(),
                stream_id: 0,
                res: res_b,
                root_key: "http-stream:test-agent-b".to_string(),
            });
        });

        assert!(!has_streaming_conns_for_runtime(&rt_a));
        assert!(has_streaming_conns_for_runtime(&rt_b));
        assert!(
            !drain_streaming_conns(&mut rt_a),
            "agent A must not reap or observe agent B streaming registry"
        );
        assert!(has_streaming_conns_for_runtime(&rt_b));

        STREAMING_CONNS.with(|v| v.borrow_mut().clear());
    }
}
