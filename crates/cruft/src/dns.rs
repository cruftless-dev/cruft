
use crate::register::{new_object, register_method, set_constant};
use rusty_js_runtime::caps::{self, ModuleId, ModuleProvenance};
use rusty_js_runtime::promise::{new_promise, reject_promise, resolve_promise};
use rusty_js_runtime::{HostEnqueuePhase, Object, Runtime, RuntimeError, Value};
use std::cell::RefCell;
use std::net::{IpAddr, ToSocketAddrs};
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

thread_local! {
    static NEXT_DNS_CALLBACK_ID: RefCell<u64> = RefCell::new(1);

    static DNS_RESULT_ORDER: RefCell<String> = RefCell::new(String::from("verbatim"));

    static DNS_SERVERS: RefCell<Vec<String>> = RefCell::new(Vec::new());
    static DNS_SERVERS_PROMISES: RefCell<Vec<String>> = RefCell::new(Vec::new());
}

fn dns_servers_get(rt: &mut Runtime, promises: bool) -> Value {
    let arr = rt.alloc_object(Object::new_array());
    let servers = if promises {
        DNS_SERVERS_PROMISES.with(|v| v.borrow().clone())
    } else {
        DNS_SERVERS.with(|v| v.borrow().clone())
    };
    for (i, s) in servers.iter().enumerate() {
        rt.object_set(
            arr,
            i.to_string(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(s.clone()))),
        );
    }
    rt.object_set(arr, "length".into(), Value::Number(servers.len() as f64));
    Value::Object(arr)
}

fn dns_normalize_server(s: &str) -> Option<String> {
    if let Some(rest) = s.strip_prefix('[') {
        let (inner, after) = rest.split_once(']')?;
        inner.parse::<std::net::Ipv6Addr>().ok()?;
        if let Some(port) = after.strip_prefix(':') {
            let port: u16 = port.parse().ok()?;
            return Some(if port == 53 {
                inner.to_string()
            } else {
                format!("[{inner}]:{port}")
            });
        }
        return if after.is_empty() {
            Some(inner.to_string())
        } else {
            None
        };
    }
    let colons = s.matches(':').count();
    if colons == 0 {
        s.parse::<std::net::Ipv4Addr>().ok()?;
        return Some(s.to_string());
    }
    if colons == 1 {
        let (ip, port) = s.split_once(':')?;
        ip.parse::<std::net::Ipv4Addr>().ok()?;
        let port: u16 = port.parse().ok()?;
        return Some(if port == 53 {
            ip.to_string()
        } else {
            format!("{ip}:{port}")
        });
    }
    s.parse::<std::net::Ipv6Addr>().ok()?;
    Some(s.to_string())
}

fn dns_received_suffix(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => format!(" Received type string ('{}')", s.as_str()),
        Some(Value::Number(n)) if n.is_nan() => " Received type number (NaN)".to_string(),
        Some(Value::Number(n)) => format!(
            " Received type number ({})",
            rusty_js_runtime::abstract_ops::number_to_string(*n)
        ),
        Some(Value::Boolean(b)) => format!(" Received type boolean ({b})"),
        Some(Value::Null) => " Received null".to_string(),
        Some(Value::Undefined) | None => " Received undefined".to_string(),
        Some(Value::Object(_)) => " Received an instance of Object".to_string(),
        _ => String::new(),
    }
}

fn dns_arg_type_error(rt: &mut Runtime, msg: &str) -> RuntimeError {
    match rusty_js_runtime::intrinsics::make_error_instance(rt, "TypeError", msg) {
        Some(id) => {
            rt.object_set(
                id,
                "code".into(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    "ERR_INVALID_ARG_TYPE",
                ))),
            );
            RuntimeError::Thrown(Value::Object(id))
        }
        None => RuntimeError::TypeError(msg.to_string()),
    }
}

fn dns_invalid_ip_error(rt: &mut Runtime, ip: &str) -> RuntimeError {
    let msg = format!("Invalid IP address: {ip}");
    match rusty_js_runtime::intrinsics::make_error_instance(rt, "TypeError", &msg) {
        Some(id) => {
            rt.object_set(
                id,
                "code".into(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    "ERR_INVALID_IP_ADDRESS",
                ))),
            );
            RuntimeError::Thrown(Value::Object(id))
        }
        None => RuntimeError::TypeError(msg),
    }
}

fn dns_set_servers(
    rt: &mut Runtime,
    args: &[Value],
    promises: bool,
) -> Result<Value, RuntimeError> {
    let arr = match args.first() {
        Some(Value::Object(id)) if is_js_array(rt, *id) => *id,
        other => {
            return Err(dns_arg_type_error(
                rt,
                &format!(
                    "The \"servers\" argument must be an instance of Array.{}",
                    dns_received_suffix(other)
                ),
            ))
        }
    };
    let len = rt.array_length(arr);
    let mut normalized = Vec::with_capacity(len);
    for i in 0..len {
        let el = rt.object_get(arr, &i.to_string());
        let s = match &el {
            Value::String(s) => s.as_str().to_string(),
            _ => {
                return Err(dns_arg_type_error(
                    rt,
                    &format!(
                        "The \"servers[{i}]\" argument must be of type string.{}",
                        dns_received_suffix(Some(&el))
                    ),
                ))
            }
        };
        match dns_normalize_server(&s) {
            Some(n) => normalized.push(n),
            None => return Err(dns_invalid_ip_error(rt, &s)),
        }
    }
    DNS_SERVERS_PROMISES.with(|v| *v.borrow_mut() = normalized.clone());
    if !promises {
        DNS_SERVERS.with(|v| *v.borrow_mut() = normalized);
    }
    Ok(Value::Undefined)
}

fn is_js_array(rt: &mut Runtime, id: rusty_js_runtime::ObjectRef) -> bool {
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

fn dns_result_order_get() -> String {
    DNS_RESULT_ORDER.with(|o| o.borrow().clone())
}

fn dns_set_result_order(rt: &mut Runtime, args: &[Value]) -> Result<Value, RuntimeError> {
    let (order, received) = match args.first() {
        Some(Value::String(s)) => (Some(s.as_str().to_string()), format!("'{}'", s.as_str())),
        Some(Value::Number(n)) => (None, rusty_js_runtime::abstract_ops::number_to_string(*n)),
        other => (
            None,
            match other {
                Some(Value::Boolean(b)) => b.to_string(),
                Some(Value::Undefined) | None => "undefined".to_string(),
                Some(Value::Null) => "null".to_string(),
                _ => "value".to_string(),
            },
        ),
    };
    if let Some(o) = order {
        if matches!(o.as_str(), "verbatim" | "ipv4first" | "ipv6first") {
            DNS_RESULT_ORDER.with(|slot| *slot.borrow_mut() = o);
            return Ok(Value::Undefined);
        }
    }
    Err(dns_invalid_arg_value(
        rt,
        &format!(
            "The argument 'dnsOrder' must be one of: 'verbatim', 'ipv4first', 'ipv6first'. Received {received}"
        ),
    ))
}

fn dns_callback_root_key(id: u64) -> String {
    format!("dns:callback:{id}")
}

fn retain_dns_callback(rt: &mut Runtime, cb: &Value) -> String {
    let id = NEXT_DNS_CALLBACK_ID.with(|c| {
        let mut c = c.borrow_mut();
        let id = *c;
        *c += 1;
        id
    });
    let key = dns_callback_root_key(id);
    rt.retain_host_roots(key.clone(), vec![cb.clone()]);
    key
}

fn resolve_one(host: &str, family: u8) -> Result<(String, u8), String> {
    let (mut v4, mut v6, mut first) = (None, None, None);
    for a in (host, 0u16).to_socket_addrs().map_err(|e| format!("{e}"))? {
        let entry = match a.ip() {
            IpAddr::V4(ip) => (ip.to_string(), 4u8),
            IpAddr::V6(ip) => (ip.to_string(), 6u8),
        };
        if first.is_none() {
            first = Some(entry.clone());
        }
        match a.ip() {
            IpAddr::V4(_) if v4.is_none() => v4 = Some(entry),
            IpAddr::V6(_) if v6.is_none() => v6 = Some(entry),
            _ => {}
        }
    }

    let pick = match family {
        6 => v6.or(v4),
        4 => v4.or(v6),
        _ => first,
    };
    pick.ok_or_else(|| format!("getaddrinfo ENOTFOUND {host}"))
}

fn resolve_all(host: &str, family: u8) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for a in (host, 0u16).to_socket_addrs().map_err(|e| format!("{e}"))? {
        match (a.ip(), family) {
            (IpAddr::V4(ip), 4) => out.push(ip.to_string()),
            (IpAddr::V6(ip), 6) => out.push(ip.to_string()),
            _ => {}
        }
    }
    if out.is_empty() {
        return Err(format!("queryA ENODATA {host}"));
    }
    Ok(out)
}

fn resolve_all_pairs(host: &str, family: u8) -> Result<Vec<(String, u8)>, String> {
    let mut out: Vec<(String, u8)> = Vec::new();
    for a in (host, 0u16).to_socket_addrs().map_err(|e| format!("{e}"))? {
        let (ip, fam) = match a.ip() {
            IpAddr::V4(ip) => (ip.to_string(), 4u8),
            IpAddr::V6(ip) => (ip.to_string(), 6u8),
        };
        if (family == 0 || family == fam) && !out.iter().any(|(a, f)| *a == ip && *f == fam) {
            out.push((ip, fam));
        }
    }
    if out.is_empty() {
        return Err(format!("getaddrinfo ENOTFOUND {host}"));
    }
    Ok(out)
}

fn dns_lookup_all_array(rt: &mut Runtime, pairs: Vec<(String, u8)>) -> Value {
    let arr = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
    for (i, (addr, fam)) in pairs.into_iter().enumerate() {
        let entry = dns_lookup_object(rt, addr, fam);
        rt.object_set(arr, i.to_string(), entry);
    }
    Value::Object(arr)
}

fn gate(rt: &Runtime, host: &str) -> Result<(), RuntimeError> {
    let caller = caller_module_id(rt);
    rt.caps
        .require_net(
            &caps::Net::none(),
            caps::NetOp::Connect {
                host: host.to_string(),
                port: 0,
            },
            &caller,
        )
        .map_err(|e| RuntimeError::TypeError(e.to_string()))
}

fn dns_error(rt: &mut Runtime, msg: &str, code: &str) -> Value {
    dns_error_host(rt, msg, code, "")
}

fn dns_error_host(rt: &mut Runtime, msg: &str, code: &str, host: &str) -> Value {
    let ctor = rt.global_get("Error");
    let err = match rt.construct(
        ctor,
        vec![Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(msg.to_string()),
        ))],
    ) {
        Ok(Value::Object(id)) => id,
        _ => new_object(rt),
    };
    rt.object_set(
        err,
        "code".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            code.to_string(),
        ))),
    );
    let errno = match code {
        "ENOTFOUND" => -3008,
        "ENODATA" => -3007,
        "EAI_AGAIN" => -3001,
        "ESERVFAIL" => -3002,
        "EREFUSED" => -3010,
        _ => -3008,
    };
    rt.object_set(err, "errno".into(), Value::Number(errno as f64));
    rt.object_set(
        err,
        "syscall".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "getaddrinfo",
        ))),
    );
    if !host.is_empty() {
        rt.object_set(
            err,
            "hostname".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                host.to_string(),
            ))),
        );
    }
    Value::Object(err)
}

fn dns_lookup_object(rt: &mut Runtime, addr: String, fam: u8) -> Value {
    let o = new_object(rt);
    let out_v = Value::Object(o);
    let _roots = rt.push_temporary_value_roots(std::slice::from_ref(&out_v));
    rt.object_set(
        o,
        "address".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(addr))),
    );
    rt.object_set(o, "family".into(), Value::Number(fam as f64));
    out_v
}

fn dns_string_array(rt: &mut Runtime, ips: &[String]) -> Value {
    let arr = rt.alloc_object(Object::new_array());
    let arr_v = Value::Object(arr);
    let _roots = rt.push_temporary_value_roots(std::slice::from_ref(&arr_v));
    for (i, ip) in ips.iter().enumerate() {
        rt.object_set(
            arr,
            i.to_string(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(ip.clone()))),
        );
    }
    rt.object_set(arr, "length".into(), Value::Number(ips.len() as f64));
    arr_v
}

fn dns_invalid_arg_value(rt: &mut Runtime, msg: &str) -> RuntimeError {
    match rusty_js_runtime::intrinsics::make_error_instance(rt, "TypeError", msg) {
        Some(id) => {
            rt.object_set(
                id,
                "code".into(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    "ERR_INVALID_ARG_VALUE",
                ))),
            );
            RuntimeError::Thrown(Value::Object(id))
        }
        None => RuntimeError::TypeError(msg.to_string()),
    }
}

fn host_arg(args: &[Value]) -> Result<String, RuntimeError> {
    match args.first() {
        Some(Value::String(s)) => Ok(s.to_string()),
        _ => Err(RuntimeError::TypeError(
            "dns: hostname must be a string".into(),
        )),
    }
}

fn node_host_arg(rt: &mut Runtime, args: &[Value]) -> Result<String, RuntimeError> {
    match args.first() {
        Some(Value::String(s)) => Ok(s.to_string()),
        _ => Err(dns_invalid_arg_value(
            rt,
            "The argument 'hostname' is invalid. Received null",
        )),
    }
}

pub fn install_canonical(rt: &mut Runtime) {
    let ns = new_object(rt);

    register_method(rt, ns, "lookup", |rt, args| {
        let host = host_arg(args)?;
        let family = match args.get(1) {
            Some(Value::Number(n)) => *n as u8,
            _ => 0,
        };
        gate(rt, &host)?;
        let p = new_promise(rt);
        match resolve_one(&host, family) {
            Ok((addr, fam)) => {
                let out = dns_lookup_object(rt, addr, fam);
                let _promise_roots =
                    rt.push_temporary_value_roots(&[Value::Object(p), out.clone()]);
                resolve_promise(rt, p, out);
            }
            Err(e) => {
                let err = dns_error_host(rt, &e, "ENOTFOUND", &host);
                reject_promise(rt, p, err);
            }
        }
        Ok(Value::Object(p))
    });

    for fam in [4u8, 6u8] {
        let name = if fam == 4 { "resolve4" } else { "resolve6" };
        register_method(rt, ns, name, move |rt, args| {
            let host = host_arg(args)?;
            gate(rt, &host)?;
            let p = new_promise(rt);
            match resolve_all(&host, fam) {
                Ok(ips) => {
                    let out = dns_string_array(rt, &ips);
                    let _promise_roots =
                        rt.push_temporary_value_roots(&[Value::Object(p), out.clone()]);
                    resolve_promise(rt, p, out);
                }
                Err(e) => {
                    let err = dns_error(rt, &e, "ENODATA");
                    reject_promise(rt, p, err);
                }
            }
            Ok(Value::Object(p))
        });
    }

    rt.define_global_property("__cruft_dns", Value::Object(ns));
}

use crate::dns_proto::{self, Rr};

fn dns_sval(rt: &Runtime, s: &str) -> Value {
    let _ = rt;
    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
        s.to_string(),
    )))
}

fn dns_build(rt: &mut Runtime, rrs: Vec<Rr>) -> Value {

    if matches!(rrs.first(), Some(Rr::Soa { .. })) {
        if let Some(Rr::Soa {
            nsname,
            hostmaster,
            serial,
            refresh,
            retry,
            expire,
            minttl,
        }) = rrs.into_iter().next()
        {
            let o = new_object(rt);
            rt.object_set(o, "nsname".into(), dns_sval(rt, &nsname));
            rt.object_set(o, "hostmaster".into(), dns_sval(rt, &hostmaster));
            rt.object_set(o, "serial".into(), Value::Number(serial as f64));
            rt.object_set(o, "refresh".into(), Value::Number(refresh as f64));
            rt.object_set(o, "retry".into(), Value::Number(retry as f64));
            rt.object_set(o, "expire".into(), Value::Number(expire as f64));
            rt.object_set(o, "minttl".into(), Value::Number(minttl as f64));
            rt.object_set(o, "type".into(), dns_sval(rt, "SOA"));
            return Value::Object(o);
        }
        return Value::Undefined;
    }
    let arr = rt.alloc_object(Object::new_array());
    let mut i = 0usize;
    for rr in rrs {
        let v = match rr {
            Rr::A(x) | Rr::Aaaa(x) | Rr::Name(x) => dns_sval(rt, &x),
            Rr::Mx { priority, exchange } => {
                let o = new_object(rt);
                rt.object_set(o, "exchange".into(), dns_sval(rt, &exchange));
                rt.object_set(o, "priority".into(), Value::Number(priority as f64));
                rt.object_set(o, "type".into(), dns_sval(rt, "MX"));
                Value::Object(o)
            }
            Rr::Txt(strs) => {
                let ta = rt.alloc_object(Object::new_array());
                for (j, st) in strs.iter().enumerate() {
                    rt.object_set(ta, j.to_string(), dns_sval(rt, st));
                }
                rt.object_set(ta, "length".into(), Value::Number(strs.len() as f64));
                Value::Object(ta)
            }
            Rr::Srv {
                priority,
                weight,
                port,
                name,
            } => {
                let o = new_object(rt);
                rt.object_set(o, "priority".into(), Value::Number(priority as f64));
                rt.object_set(o, "weight".into(), Value::Number(weight as f64));
                rt.object_set(o, "port".into(), Value::Number(port as f64));
                rt.object_set(o, "name".into(), dns_sval(rt, &name));
                Value::Object(o)
            }
            Rr::Soa { .. } => continue,
        };
        rt.object_set(arr, i.to_string(), v);
        i += 1;
    }
    rt.object_set(arr, "length".into(), Value::Number(i as f64));
    Value::Object(arr)
}

fn dns_qtype(rt: &mut Runtime, rrtype: &str) -> Result<u16, RuntimeError> {
    let qtype = match rrtype {
        "A" => dns_proto::T_A,
        "AAAA" => dns_proto::T_AAAA,
        "CNAME" => dns_proto::T_CNAME,
        "MX" => dns_proto::T_MX,
        "TXT" => dns_proto::T_TXT,
        "NS" => dns_proto::T_NS,
        "SOA" => dns_proto::T_SOA,
        "SRV" => dns_proto::T_SRV,
        "PTR" => dns_proto::T_PTR,
        _ => {
            return Err(dns_invalid_arg_value(
                rt,
                &format!("The argument 'rrtype' is invalid. Received '{rrtype}'"),
            ))
        }
    };
    Ok(qtype)
}

fn dns_query_label(qtype: u16) -> &'static str {
    match qtype {
        dns_proto::T_A => "queryA",
        dns_proto::T_AAAA => "queryAaaa",
        dns_proto::T_CNAME => "queryCname",
        dns_proto::T_MX => "queryMx",
        dns_proto::T_TXT => "queryTxt",
        dns_proto::T_NS => "queryNs",
        dns_proto::T_SOA => "querySoa",
        dns_proto::T_SRV => "querySrv",
        dns_proto::T_PTR => "queryPtr",
        _ => "query",
    }
}

fn dns_query_error(rt: &mut Runtime, host: &str, qtype: u16, code: &str) -> Value {
    dns_error(
        rt,
        &format!("{} {code} {host}", dns_query_label(qtype)),
        code,
    )
}

fn dns_reverse_name(ip: &str) -> String {
    if ip.contains(':') {

        return format!("{ip}.ip6.arpa");
    }
    let parts: Vec<&str> = ip.split('.').collect();
    let mut r: Vec<&str> = parts.clone();
    r.reverse();
    format!("{}.in-addr.arpa", r.join("."))
}

fn dns_reverse_empty_ok(ip: &str, err: &str) -> bool {
    matches!(ip, "127.0.0.1" | "::1") && matches!(err, "ENOTFOUND" | "ENODATA")
}

fn dns_service_name(port: u16) -> String {
    match port {
        21 => "ftp",
        22 => "ssh",
        25 => "smtp",
        53 => "domain",
        80 => "http",
        110 => "pop3",
        143 => "imap",
        443 => "https",
        587 => "submission",
        993 => "imaps",
        995 => "pop3s",
        _ => return port.to_string(),
    }
    .to_string()
}

fn dns_lookup_service(ip: &str, port: u16) -> Result<(String, String), String> {
    let host = match ip {
        "127.0.0.1" | "::1" => "localhost".to_string(),
        _ => return Err(format!("getHostByAddr ENOTFOUND {ip}")),
    };
    Ok((host, dns_service_name(port)))
}

fn dns_query(rt: &mut Runtime, host: &str, qtype: u16) -> Result<Value, String> {
    let server = dns_proto::system_resolver();
    let rrs = dns_proto::query(&server, host, qtype)?;
    Ok(dns_build(rt, rrs))
}

fn dns_install_constants(rt: &mut Runtime, obj: rusty_js_runtime::value::ObjectRef) {
    let strs: &[(&str, &str)] = &[
        ("ADDRGETNETWORKPARAMS", "EADDRGETNETWORKPARAMS"),
        ("BADFAMILY", "EBADFAMILY"),
        ("BADFLAGS", "EBADFLAGS"),
        ("BADHINTS", "EBADHINTS"),
        ("BADNAME", "EBADNAME"),
        ("BADQUERY", "EBADQUERY"),
        ("BADRESP", "EBADRESP"),
        ("BADSTR", "EBADSTR"),
        ("CANCELLED", "ECANCELLED"),
        ("CONNREFUSED", "ECONNREFUSED"),
        ("DESTRUCTION", "EDESTRUCTION"),
        ("EOF", "EOF"),
        ("FILE", "EFILE"),
        ("FORMERR", "EFORMERR"),
        ("LOADIPHLPAPI", "ELOADIPHLPAPI"),
        ("NODATA", "ENODATA"),
        ("NOMEM", "ENOMEM"),
        ("NONAME", "ENONAME"),
        ("NOTFOUND", "ENOTFOUND"),
        ("NOTIMP", "ENOTIMP"),
        ("NOTINITIALIZED", "ENOTINITIALIZED"),
        ("REFUSED", "EREFUSED"),
        ("SERVFAIL", "ESERVFAIL"),
        ("TIMEOUT", "ETIMEOUT"),
    ];
    for (k, v) in strs {
        rt.object_set(
            obj,
            (*k).to_string(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                (*v).to_string(),
            ))),
        );
    }
}

pub fn install(rt: &mut Runtime) {
    let ns = new_object(rt);

    register_method(rt, ns, "lookup", |rt, args| {
        let host = node_host_arg(rt, args)?;
        let mut family = 0u8;
        let mut all = false;
        match args.get(1) {

            Some(Value::Number(n)) => family = *n as u8,

            Some(Value::Object(o)) => {
                if let Value::Number(n) = rt.object_get(*o, "family") {
                    family = n as u8;
                }
                all = matches!(rt.object_get(*o, "all"), Value::Boolean(true));
            }
            _ => {}
        }
        let cb = args.iter().rev().cloned().find(|v| rt.is_callable(v));
        gate(rt, &host)?;

        if all {
            let res = resolve_all_pairs(&host, family);
            if let Some(cb) = cb {
                let cb_root = retain_dns_callback(rt, &cb);
                match res {
                    Ok(pairs) => rt.enqueue_host_phase_rooted(
                        HostEnqueuePhase::HostCompletionMacrotask,
                        "dns.lookup all cb",
                        Vec::new(),
                        move |rt| {
                            let cb_v = cb.clone();
                            let arr = dns_lookup_all_array(rt, pairs.clone());
                            let _call_roots =
                                rt.push_temporary_value_roots(&[cb_v.clone(), arr.clone()]);
                            let _ =
                                rt.call_function(cb_v, Value::Undefined, vec![Value::Null, arr]);
                            rt.release_host_roots(&cb_root);
                            Ok(())
                        },
                    ),
                    Err(e) => rt.enqueue_host_phase_rooted(
                        HostEnqueuePhase::HostCompletionMacrotask,
                        "dns.lookup all err",
                        Vec::new(),
                        move |rt| {
                            let cb_v = cb.clone();
                            let err = dns_error_host(rt, &e, "ENOTFOUND", &host);
                            let _call_roots =
                                rt.push_temporary_value_roots(&[cb_v.clone(), err.clone()]);
                            let _ = rt.call_function(cb_v, Value::Undefined, vec![err]);
                            rt.release_host_roots(&cb_root);
                            Ok(())
                        },
                    ),
                }
            }
            return Ok(Value::Undefined);
        }
        let res = resolve_one(&host, family);
        if let Some(cb) = cb {
            let cb_root = retain_dns_callback(rt, &cb);
            match res {
                Ok((addr, fam)) => rt.enqueue_host_phase_rooted(
                    HostEnqueuePhase::HostCompletionMacrotask,
                    "dns.lookup cb",
                    Vec::new(),
                    move |rt| {
                        let cb_v = cb.clone();
                        let _call_roots =
                            rt.push_temporary_value_roots(std::slice::from_ref(&cb_v));
                        let _ = rt.call_function(
                            cb_v,
                            Value::Undefined,
                            vec![
                                Value::Null,
                                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                                    addr,
                                ))),
                                Value::Number(fam as f64),
                            ],
                        );
                        rt.release_host_roots(&cb_root);
                        Ok(())
                    },
                ),
                Err(e) => rt.enqueue_host_phase_rooted(
                    HostEnqueuePhase::HostCompletionMacrotask,
                    "dns.lookup err",
                    Vec::new(),
                    move |rt| {
                        let cb_v = cb.clone();
                        let err = dns_error_host(rt, &e, "ENOTFOUND", &host);
                        let _call_roots =
                            rt.push_temporary_value_roots(&[cb_v.clone(), err.clone()]);
                        let _ = rt.call_function(cb_v, Value::Undefined, vec![err]);
                        rt.release_host_roots(&cb_root);
                        Ok(())
                    },
                ),
            }
        }
        Ok(Value::Undefined)
    });

    for (name, qtype) in [
        ("resolve4", dns_proto::T_A),
        ("resolve6", dns_proto::T_AAAA),
    ] {
        register_method(rt, ns, name, move |rt, args| {
            let host = node_host_arg(rt, args)?;
            let cb = args.iter().rev().cloned().find(|v| rt.is_callable(v));
            gate(rt, &host)?;
            let res = dns_query(rt, &host, qtype);
            if let Some(cb) = cb {
                let cb_root = retain_dns_callback(rt, &cb);
                match res {
                    Ok(out) => rt.enqueue_host_phase_rooted(
                        HostEnqueuePhase::HostCompletionMacrotask,
                        "dns.resolve cb",
                        Vec::new(),
                        move |rt| {
                            let cb_v = cb.clone();
                            let _call_roots =
                                rt.push_temporary_value_roots(&[cb_v.clone(), out.clone()]);
                            let _ =
                                rt.call_function(cb_v, Value::Undefined, vec![Value::Null, out]);
                            rt.release_host_roots(&cb_root);
                            Ok(())
                        },
                    ),
                    Err(e) => rt.enqueue_host_phase_rooted(
                        HostEnqueuePhase::HostCompletionMacrotask,
                        "dns.resolve err",
                        Vec::new(),
                        move |rt| {
                            let cb_v = cb.clone();
                            let err = dns_query_error(rt, &host, qtype, &e);
                            let _call_roots =
                                rt.push_temporary_value_roots(&[cb_v.clone(), err.clone()]);
                            let _ = rt.call_function(cb_v, Value::Undefined, vec![err]);
                            rt.release_host_roots(&cb_root);
                            Ok(())
                        },
                    ),
                }
            }
            Ok(Value::Undefined)
        });
    }

    let promises = new_object(rt);
    register_method(rt, promises, "lookup", |rt, args| {
        let p = new_promise(rt);
        let host = match node_host_arg(rt, args) {
            Ok(host) => host,
            Err(RuntimeError::Thrown(err)) => {
                reject_promise(rt, p, err);
                return Ok(Value::Object(p));
            }
            Err(err) => return Err(err),
        };
        let mut family = 0u8;
        let mut all = false;
        match args.get(1) {
            Some(Value::Number(n)) => family = *n as u8,
            Some(Value::Object(o)) => {
                if let Value::Number(n) = rt.object_get(*o, "family") {
                    family = n as u8;
                }
                all = matches!(rt.object_get(*o, "all"), Value::Boolean(true));
            }
            _ => {}
        }
        gate(rt, &host)?;

        if all {
            match resolve_all_pairs(&host, family) {
                Ok(pairs) => {
                    let out = dns_lookup_all_array(rt, pairs);
                    let _promise_roots =
                        rt.push_temporary_value_roots(&[Value::Object(p), out.clone()]);
                    resolve_promise(rt, p, out);
                }
                Err(e) => {
                    let err = dns_error_host(rt, &e, "ENOTFOUND", &host);
                    reject_promise(rt, p, err);
                }
            }
            return Ok(Value::Object(p));
        }
        match resolve_one(&host, family) {
            Ok((addr, fam)) => {
                let out = dns_lookup_object(rt, addr, fam);
                let _promise_roots =
                    rt.push_temporary_value_roots(&[Value::Object(p), out.clone()]);
                resolve_promise(rt, p, out);
            }
            Err(e) => {
                let err = dns_error_host(rt, &e, "ENOTFOUND", &host);
                reject_promise(rt, p, err);
            }
        }
        Ok(Value::Object(p))
    });

    for (name, qtype) in [
        ("resolve4", dns_proto::T_A),
        ("resolve6", dns_proto::T_AAAA),
    ] {
        register_method(rt, promises, name, move |rt, args| {
            let host = node_host_arg(rt, args)?;
            gate(rt, &host)?;
            let p = new_promise(rt);
            match dns_query(rt, &host, qtype) {
                Ok(out) => {
                    let _promise_roots =
                        rt.push_temporary_value_roots(&[Value::Object(p), out.clone()]);
                    resolve_promise(rt, p, out);
                }
                Err(e) => {
                    let err = dns_query_error(rt, &host, qtype, &e);
                    reject_promise(rt, p, err);
                }
            }
            Ok(Value::Object(p))
        });
    }
    register_method(rt, promises, "getServers", |rt, _a| {
        Ok(dns_servers_get(rt, true))
    });
    register_method(rt, promises, "setServers", |rt, args| {
        dns_set_servers(rt, args, true)
    });
    register_method(rt, promises, "getDefaultResultOrder", |_rt, _a| {
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(dns_result_order_get()),
        )))
    });
    register_method(rt, promises, "setDefaultResultOrder", |rt, args| {
        dns_set_result_order(rt, args)
    });

    for (m, qt) in [
        ("resolveCname", dns_proto::T_CNAME),
        ("resolveMx", dns_proto::T_MX),
        ("resolveTxt", dns_proto::T_TXT),
        ("resolveNs", dns_proto::T_NS),
        ("resolveSoa", dns_proto::T_SOA),
        ("resolveSrv", dns_proto::T_SRV),
        ("resolvePtr", dns_proto::T_PTR),
    ] {
        register_method(rt, promises, m, move |rt, args| {
            let host = node_host_arg(rt, args)?;
            gate(rt, &host)?;
            let p = new_promise(rt);
            match dns_query(rt, &host, qt) {
                Ok(v) => resolve_promise(rt, p, v),
                Err(e) => {
                    let err = dns_query_error(rt, &host, qt, &e);
                    reject_promise(rt, p, err);
                }
            }
            Ok(Value::Object(p))
        });
    }
    register_method(rt, promises, "resolve", |rt, args| {
        let host = node_host_arg(rt, args)?;
        let rrtype = match args.get(1) {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => "A".to_string(),
        };
        gate(rt, &host)?;
        let p = new_promise(rt);
        let qtype = dns_qtype(rt, &rrtype)?;
        match dns_query(rt, &host, qtype) {
            Ok(v) => resolve_promise(rt, p, v),
            Err(e) => {
                let err = dns_query_error(rt, &host, qtype, &e);
                reject_promise(rt, p, err);
            }
        }
        Ok(Value::Object(p))
    });
    register_method(rt, promises, "reverse", |rt, args| {
        let ip = node_host_arg(rt, args)?;
        gate(rt, &ip)?;
        let p = new_promise(rt);
        match dns_query(rt, &dns_reverse_name(&ip), dns_proto::T_PTR) {
            Ok(v) => resolve_promise(rt, p, v),
            Err(e) => {
                if dns_reverse_empty_ok(&ip, &e) {
                    let out = dns_string_array(rt, &[]);
                    resolve_promise(rt, p, out);
                } else {
                    let err = dns_error(rt, &e, &e);
                    reject_promise(rt, p, err);
                }
            }
        }
        Ok(Value::Object(p))
    });
    register_method(rt, promises, "lookupService", |rt, args| {
        let ip = node_host_arg(rt, args)?;
        let port = match args.get(1) {
            Some(Value::Number(n)) if *n >= 0.0 && *n <= 65535.0 => *n as u16,
            _ => {
                return Err(RuntimeError::TypeError(
                    "dns.lookupService: port must be a number".into(),
                ))
            }
        };
        gate(rt, &ip)?;
        let p = new_promise(rt);
        match dns_lookup_service(&ip, port) {
            Ok((host, service)) => {
                let out = new_object(rt);
                rt.object_set(out, "hostname".into(), dns_sval(rt, &host));
                rt.object_set(out, "service".into(), dns_sval(rt, &service));
                resolve_promise(rt, p, Value::Object(out));
            }
            Err(e) => {
                let err = dns_error(rt, &e, "ENOTFOUND");
                reject_promise(rt, p, err);
            }
        }
        Ok(Value::Object(p))
    });
    for m in ["resolveAny", "resolveCaa", "resolveNaptr", "resolveTlsa"] {
        let name = m;
        register_method(rt, promises, m, move |rt, _a| {
            let p = new_promise(rt);
            let err = dns_error(rt, &format!("{name}: not implemented"), "ENOTIMP");
            reject_promise(rt, p, err);
            Ok(Value::Object(p))
        });
    }

    install_dns_resolver_class(rt, promises);

    set_constant(rt, ns, "promises", Value::Object(promises));

    dns_install_constants(rt, promises);
    rt.define_global_property("dns_promises", Value::Object(promises));

    register_method(rt, ns, "getServers", |rt, _a| {
        Ok(dns_servers_get(rt, false))
    });
    register_method(rt, ns, "setServers", |rt, args| {
        dns_set_servers(rt, args, false)
    });

    register_method(rt, ns, "getDefaultResultOrder", |_rt, _a| {
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(dns_result_order_get()),
        )))
    });
    register_method(rt, ns, "setDefaultResultOrder", |rt, args| {
        dns_set_result_order(rt, args)
    });

    for (m, qt) in [
        ("resolveCname", dns_proto::T_CNAME),
        ("resolveMx", dns_proto::T_MX),
        ("resolveTxt", dns_proto::T_TXT),
        ("resolveNs", dns_proto::T_NS),
        ("resolveSoa", dns_proto::T_SOA),
        ("resolveSrv", dns_proto::T_SRV),
        ("resolvePtr", dns_proto::T_PTR),
    ] {
        register_method(rt, ns, m, move |rt, args| {
            let host = node_host_arg(rt, args)?;
            let cb = args.iter().rev().cloned().find(|v| rt.is_callable(v));
            gate(rt, &host)?;
            let res = dns_query(rt, &host, qt);
            if let Some(cb) = cb {
                match res {
                    Ok(v) => {
                        let _ = rt.call_function(cb, Value::Undefined, vec![Value::Null, v]);
                    }
                    Err(e) => {
                        let err = dns_query_error(rt, &host, qt, &e);
                        let _ = rt.call_function(cb, Value::Undefined, vec![err]);
                    }
                }
            }
            Ok(Value::Undefined)
        });
    }
    register_method(rt, ns, "resolve", |rt, args| {
        let host = node_host_arg(rt, args)?;
        let rrtype = match args.get(1) {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => "A".to_string(),
        };
        let cb = args.iter().rev().cloned().find(|v| rt.is_callable(v));
        gate(rt, &host)?;
        let qtype = dns_qtype(rt, &rrtype)?;
        let res = dns_query(rt, &host, qtype);
        if let Some(cb) = cb {
            match res {
                Ok(v) => {
                    let _ = rt.call_function(cb, Value::Undefined, vec![Value::Null, v]);
                }
                Err(e) => {
                    let err = dns_query_error(rt, &host, qtype, &e);
                    let _ = rt.call_function(cb, Value::Undefined, vec![err]);
                }
            }
        }
        Ok(Value::Undefined)
    });
    register_method(rt, ns, "reverse", |rt, args| {
        let ip = node_host_arg(rt, args)?;
        let cb = args.iter().rev().cloned().find(|v| rt.is_callable(v));
        gate(rt, &ip)?;
        let res = dns_query(rt, &dns_reverse_name(&ip), dns_proto::T_PTR);
        if let Some(cb) = cb {
            match res {
                Ok(v) => {
                    let _ = rt.call_function(cb, Value::Undefined, vec![Value::Null, v]);
                }
                Err(e) => {
                    if dns_reverse_empty_ok(&ip, &e) {
                        let out = dns_string_array(rt, &[]);
                        let _ = rt.call_function(cb, Value::Undefined, vec![Value::Null, out]);
                    } else {
                        let err = dns_error(rt, &e, &e);
                        let _ = rt.call_function(cb, Value::Undefined, vec![err]);
                    }
                }
            }
        }
        Ok(Value::Undefined)
    });

    dns_install_constants(rt, ns);
    for (k, v) in [("ADDRCONFIG", 32.0), ("ALL", 16.0), ("V4MAPPED", 8.0)] {
        rt.object_set(ns, k.to_string(), Value::Number(v));
    }
    register_method(rt, ns, "lookupService", |rt, args| {
        let ip = node_host_arg(rt, args)?;
        let port = match args.get(1) {
            Some(Value::Number(n)) if *n >= 0.0 && *n <= 65535.0 => *n as u16,
            _ => {
                return Err(RuntimeError::TypeError(
                    "dns.lookupService: port must be a number".into(),
                ))
            }
        };
        let cb = args.iter().rev().cloned().find(|v| rt.is_callable(v));
        gate(rt, &ip)?;
        let res = dns_lookup_service(&ip, port);
        if let Some(cb) = cb {
            match res {
                Ok((host, service)) => {
                    let _ = rt.call_function(
                        cb,
                        Value::Undefined,
                        vec![Value::Null, dns_sval(rt, &host), dns_sval(rt, &service)],
                    );
                }
                Err(e) => {
                    let err = dns_error(rt, &e, "ENOTFOUND");
                    let _ = rt.call_function(cb, Value::Undefined, vec![err]);
                }
            }
        }
        Ok(Value::Undefined)
    });
    for m in ["resolveAny", "resolveCaa", "resolveNaptr", "resolveTlsa"] {
        let nm = m;
        register_method(rt, ns, m, move |rt, args| {
            if let Some(cb) = args.iter().rev().find(|v| rt.is_callable(v)).cloned() {
                let err = dns_error(rt, &format!("{nm}: not implemented"), "ENOTIMP");
                let _ = rt.call_function(cb, Value::Undefined, vec![err]);
            }
            Ok(Value::Undefined)
        });
    }

    install_dns_resolver_class(rt, ns);
    rt.define_global_property("dns", Value::Object(ns));
}

fn install_dns_resolver_class(rt: &mut Runtime, host_ns: rusty_js_runtime::ObjectRef) {
    let resolver =
        crate::register::make_callable_rooted(rt, "Resolver", vec![host_ns], move |rt, _a| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(rt.current_this()),
            };
            for m in [
                "resolve",
                "resolve4",
                "resolve6",
                "resolveAny",
                "resolveMx",
                "resolveTxt",
                "resolveSrv",
                "resolveNs",
                "resolveCname",
                "resolveSoa",
                "resolvePtr",
                "resolveCaa",
                "resolveNaptr",
                "reverse",
                "getServers",
                "setServers",
            ] {
                let f = rt.object_get(host_ns, m);
                if rt.is_callable(&f) {
                    rt.object_set(this, m.to_string(), f);
                }
            }
            register_method(rt, this, "cancel", |_rt, _a| Ok(Value::Undefined));
            register_method(rt, this, "setLocalAddress", |_rt, _a| Ok(Value::Undefined));
            Ok(rt.current_this())
        });
    let proto = new_object(rt);
    rt.object_set(proto, "constructor".into(), Value::Object(resolver));
    rt.object_set(resolver, "prototype".into(), Value::Object(proto));
    rt.object_set(host_ns, "Resolver".into(), Value::Object(resolver));
}
