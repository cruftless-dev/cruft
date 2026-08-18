
use crate::register::{arg_string, make_callable, new_object, register_method, set_constant};
use rusty_js_runtime::value::Object;
use rusty_js_runtime::{Runtime, RuntimeError, Value};
use std::rc::Rc;

fn sval(s: &str) -> Value {
    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
        s.to_string(),
    )))
}

fn resolve_to_absolute(p: &str) -> String {
    use std::path::{Component, Path, PathBuf};
    let path = Path::new(p);
    let wants_trailing_sep = p.ends_with('/') || p.ends_with(std::path::MAIN_SEPARATOR);
    let mut out = if path.is_absolute() {
        PathBuf::from("/")
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))
    };
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::RootDir => out = PathBuf::from("/"),
            Component::Normal(s) => out.push(s),
            Component::Prefix(_) => {}
        }
    }
    let mut s = out.to_string_lossy().into_owned();
    if wants_trailing_sep && s != "/" && !s.ends_with('/') {
        s.push('/');
    }
    s
}

fn file_url_to_path_str(s: &str) -> Result<String, ()> {
    let rest = s.strip_prefix("file://").unwrap_or(s);
    #[cfg(windows)]
    {
        let bytes = rest.as_bytes();
        if rest.starts_with(r"\\?\") {
            let drive = &bytes[4..];
            if drive.len() >= 3
                && drive[0].is_ascii_alphabetic()
                && drive[1] == b':'
                && matches!(drive[2], b'/' | b'\\')
            {
                return Ok(percent_decode_path(&rest.replace('\\', "/")));
            }
        }
        if bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'/' | b'\\')
        {
            return Ok(percent_decode_path(&rest.replace('\\', "/")));
        }
    }
    let (host, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    if !host.is_empty() && host != "localhost" {
        return Err(());
    }
    let path = if path.is_empty() { "/" } else { path };
    Ok(percent_decode_path(path))
}

fn invalid_file_url_host_error(rt: &mut Runtime) -> RuntimeError {
    let platform = match std::env::consts::OS {
        "macos" => "darwin",
        other => other,
    };
    let msg = format!(
        "File URL host must be \"localhost\" or empty on {}",
        platform
    );
    let ctor = rt.global_get("TypeError");
    if rt.is_callable(&ctor) {
        if let Ok(Value::Object(id)) = rt.construct(ctor, vec![sval(&msg)]) {
            rt.object_set(id, "code".into(), sval("ERR_INVALID_FILE_URL_HOST"));
            return RuntimeError::Thrown(Value::Object(id));
        }
    }
    RuntimeError::TypeError(msg)
}

fn percent_decode_path(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h1 = (bytes[i + 1] as char).to_digit(16);
            let h2 = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h1), Some(h2)) = (h1, h2) {
                out.push(((h1 << 4) | h2) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn raw_legacy_path(s: &str) -> Option<String> {
    let after = s.find("://")? + 3;
    let rest = &s[after..];
    let auth_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let path_and = &rest[auth_end..];
    let path_end = path_and.find(['?', '#']).unwrap_or(path_and.len());
    let p = &path_and[..path_end];
    Some(if p.is_empty() {
        "/".to_string()
    } else {
        p.to_string()
    })
}

fn raw_legacy_port(s: &str) -> Option<String> {
    let after = s.find("://")? + 3;
    let rest = &s[after..];
    let auth_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let auth = &rest[..auth_end];
    let hostport = auth.rsplit_once('@').map(|(_, h)| h).unwrap_or(auth);
    let (_, port) = hostport.rsplit_once(':')?;
    if port.chars().all(|c| c.is_ascii_digit()) && !port.is_empty() {
        Some(port.to_string())
    } else {
        None
    }
}

fn split_legacy_relative(s: &str) -> (&str, Option<&str>, Option<&str>) {
    let (before_hash, hash) = match s.split_once('#') {
        Some((p, h)) => (p, Some(h)),
        None => (s, None),
    };
    let (path, query) = match before_hash.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (before_hash, None),
    };
    (path, query, hash)
}

fn set_query_pair(rt: &mut Runtime, qo: rusty_js_runtime::value::ObjectRef, k: String, v: String) {
    match rt.object_get(qo, &k) {
        Value::String(prev) => {
            let arr = rt.alloc_object(Object::new_array());
            rt.object_set(arr, "0".into(), Value::String(prev));
            rt.object_set(arr, "1".into(), sval(&v));
            rt.object_set(qo, k, Value::Object(arr));
        }
        Value::Object(arr) => {
            let idx = rt.array_length(arr).to_string();
            rt.object_set(arr, idx, sval(&v));
        }
        _ => rt.object_set(qo, k, sval(&v)),
    }
}

fn legacy_query_object(rt: &mut Runtime, q: Option<&str>) -> rusty_js_runtime::value::ObjectRef {
    let qo = rt.alloc_object(Object::new_ordinary());
    if let Some(q) = q {
        for (k, v) in rusty_form_urlencoded::parse(q) {
            set_query_pair(rt, qo, k, v);
        }
    }
    qo
}

fn legacy_mailto_object(rt: &mut Runtime, s: &str, parse_query: bool) -> Value {
    let o = rt.alloc_object(Object::new_ordinary());
    let rest = s.strip_prefix("mailto:").unwrap_or_default();
    let (addr, query, hash) = split_legacy_relative(rest);
    let (auth, host) = addr.split_once('@').unwrap_or((addr, ""));
    let host_val = || {
        if host.is_empty() {
            Value::Null
        } else {
            sval(host)
        }
    };

    rt.object_set(o, "protocol".into(), sval("mailto:"));
    rt.object_set(o, "slashes".into(), Value::Null);
    rt.object_set(
        o,
        "auth".into(),
        if auth.is_empty() {
            Value::Null
        } else {
            sval(auth)
        },
    );
    rt.object_set(o, "host".into(), host_val());
    rt.object_set(o, "port".into(), Value::Null);
    rt.object_set(o, "hostname".into(), host_val());
    rt.object_set(
        o,
        "hash".into(),
        hash.map(|h| sval(&format!("#{h}"))).unwrap_or(Value::Null),
    );
    rt.object_set(
        o,
        "search".into(),
        query.map(|q| sval(&format!("?{q}"))).unwrap_or(Value::Null),
    );
    let query_value = if parse_query {
        Value::Object(legacy_query_object(rt, query))
    } else {
        query.map(sval).unwrap_or(Value::Null)
    };
    rt.object_set(o, "query".into(), query_value);
    rt.object_set(o, "pathname".into(), Value::Null);
    rt.object_set(o, "path".into(), Value::Null);
    rt.object_set(o, "href".into(), sval(s));
    Value::Object(o)
}

fn legacy_relative_resolve(from: &str, to: &str) -> Option<String> {
    if !from.starts_with('/') || to.starts_with('/') || to.starts_with("//") || to.contains(':') {
        return None;
    }
    let (from_path, _, _) = split_legacy_relative(from);
    let base = from_path.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
    Some(format!("{}/{}", base, to))
}

fn legacy_unparseable_absolute_object(
    rt: &mut Runtime,
    s: &str,
    parse_query: bool,
) -> Option<Value> {
    let scheme_end = s.find("://")?;
    let protocol = &s[..scheme_end + 1];
    let rest = &s[scheme_end + 3..];
    let auth_end = rest
        .find(['/', '?', '#', '"', ' ', '<', '>', '`'])
        .unwrap_or(rest.len());
    let authority = &rest[..auth_end];
    if authority.is_empty() {
        return None;
    }
    let host = authority
        .rsplit_once('@')
        .map(|(_, h)| h)
        .unwrap_or(authority);
    let auth = authority.rsplit_once('@').map(|(a, _)| a);
    let tail = &rest[auth_end..];
    let (path_part, query_raw, hash_raw) = split_legacy_relative(tail);
    let pathname = if path_part.is_empty()
        && (query_raw.is_some() || hash_raw.is_some() || !tail.is_empty())
    {
        "/".to_string()
    } else if !path_part.is_empty() && !path_part.starts_with('/') {
        format!("/{path_part}")
    } else {
        path_part.to_string()
    };
    let o = rt.alloc_object(Object::new_ordinary());

    rt.object_set(o, "protocol".into(), sval(protocol));
    rt.object_set(o, "slashes".into(), Value::Boolean(true));
    rt.object_set(o, "auth".into(), auth.map(sval).unwrap_or(Value::Null));
    rt.object_set(o, "host".into(), sval(host));
    rt.object_set(o, "port".into(), Value::Null);
    rt.object_set(o, "hostname".into(), sval(host));
    rt.object_set(
        o,
        "hash".into(),
        hash_raw
            .map(|h| sval(&format!("#{h}")))
            .unwrap_or(Value::Null),
    );
    rt.object_set(
        o,
        "search".into(),
        query_raw
            .map(|q| sval(&format!("?{q}")))
            .unwrap_or(Value::Null),
    );
    if parse_query {
        let query = legacy_query_object(rt, query_raw);
        rt.object_set(o, "query".into(), Value::Object(query));
    } else {
        rt.object_set(
            o,
            "query".into(),
            query_raw.map(sval).unwrap_or(Value::Null),
        );
    }
    rt.object_set(o, "pathname".into(), sval(&pathname));
    let search = query_raw.map(|q| format!("?{q}")).unwrap_or_default();
    rt.object_set(o, "path".into(), sval(&format!("{pathname}{search}")));
    rt.object_set(o, "href".into(), sval(s));
    Some(Value::Object(o))
}

fn append_legacy_query_from_object(
    rt: &mut Runtime,
    id: rusty_js_runtime::value::ObjectRef,
) -> String {
    fn encode_component(s: &str) -> String {
        let mut out = String::new();
        for b in s.as_bytes() {
            match *b {
                b'A'..=b'Z'
                | b'a'..=b'z'
                | b'0'..=b'9'
                | b'-'
                | b'_'
                | b'.'
                | b'!'
                | b'~'
                | b'*'
                | b'\''
                | b'('
                | b')' => out.push(*b as char),
                _ => out.push_str(&format!("%{b:02X}")),
            }
        }
        out
    }

    let mut parts = Vec::new();
    for key in rt.ordinary_own_enumerable_string_keys(id) {
        let v = rt.object_get(id, &key);
        let encoded_key = encode_component(&key);
        match v {
            Value::Object(arr) => {
                let len = rt.array_length(arr);
                for idx in 0..len {
                    let item = rt.object_get(arr, &idx.to_string());
                    let item = rusty_js_runtime::abstract_ops::to_string(&item)
                        .as_str()
                        .to_string();
                    parts.push(format!("{encoded_key}={}", encode_component(&item)));
                }
            }
            Value::Undefined => parts.push(format!("{encoded_key}=")),
            other => {
                let s = rusty_js_runtime::abstract_ops::to_string(&other)
                    .as_str()
                    .to_string();
                parts.push(format!("{encoded_key}={}", encode_component(&s)));
            }
        }
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("?{}", parts.join("&"))
    }
}

fn legacy_url_object(
    rt: &mut Runtime,
    u: &rusty_js_url::Url,
    parse_query: bool,
    raw_path: Option<&str>,
    raw_port: Option<&str>,
    raw_href: Option<&str>,
) -> Value {
    let o = rt.alloc_object(Object::new_ordinary());
    let href = raw_href
        .map(str::to_string)
        .unwrap_or_else(|| u.serialize());
    let auth = if !u.username.is_empty() || !u.password.is_empty() {
        if !u.password.is_empty() {
            format!("{}:{}", u.username, u.password)
        } else {
            u.username.clone()
        }
    } else {
        String::new()
    };
    let mut hostname = u.hostname();

    if hostname.starts_with('[') && hostname.ends_with(']') && hostname.len() >= 2 {
        hostname = hostname[1..hostname.len() - 1].to_string();
    }
    let legacy_host_too_long = href.contains("://") && hostname.len() > 255;
    if legacy_host_too_long {
        hostname.clear();
    }
    let port = raw_port
        .map(str::to_string)
        .or_else(|| u.port.map(|p| p.to_string()));
    let host = u.host_str();
    let mut host = if let Some(raw_port) = raw_port {
        match &u.host {
            Some(h) if !h.is_empty() => format!("{h}:{raw_port}"),
            _ => host,
        }
    } else {
        host
    };
    if legacy_host_too_long {
        host.clear();
    }
    let search = u.query.as_ref().map(|q| format!("?{q}"));

    let computed_path = u.pathname();
    let pathname = raw_path.unwrap_or(computed_path.as_str());

    rt.object_set(o, "protocol".into(), sval(&u.protocol()));
    rt.object_set(o, "slashes".into(), Value::Boolean(href.contains("://")));
    rt.object_set(
        o,
        "auth".into(),
        if auth.is_empty() {
            Value::Null
        } else {
            sval(&auth)
        },
    );
    rt.object_set(
        o,
        "host".into(),
        if u.host.is_none() {
            Value::Null
        } else {
            sval(&host)
        },
    );
    rt.object_set(
        o,
        "port".into(),
        match &port {
            Some(p) => sval(p),
            None => Value::Null,
        },
    );
    rt.object_set(
        o,
        "hostname".into(),
        if u.host.is_none() {
            Value::Null
        } else {
            sval(&hostname)
        },
    );
    rt.object_set(
        o,
        "hash".into(),
        match &u.fragment {
            Some(f) => sval(&format!("#{f}")),
            None => Value::Null,
        },
    );
    rt.object_set(
        o,
        "search".into(),
        match &search {
            Some(s) => sval(s),
            None => Value::Null,
        },
    );
    if parse_query {
        let qo = legacy_query_object(rt, u.query.as_deref());
        rt.object_set(o, "query".into(), Value::Object(qo));
    } else {
        rt.object_set(
            o,
            "query".into(),
            match &u.query {
                Some(q) => sval(q),
                None => Value::Null,
            },
        );
    }
    rt.object_set(o, "pathname".into(), sval(pathname));
    rt.object_set(
        o,
        "path".into(),
        sval(&format!("{}{}", pathname, search.unwrap_or_default())),
    );
    rt.object_set(o, "href".into(), sval(&href));
    Value::Object(o)
}

fn get_via_str(
    rt: &mut Runtime,
    id: rusty_js_runtime::value::ObjectRef,
    key: &str,
) -> Option<String> {
    match rt.get_via(&Value::Object(id), &sval(key)) {
        Ok(Value::String(s)) => Some(s.as_str().to_string()),
        _ => None,
    }
}

fn obj_str(rt: &mut Runtime, id: rusty_js_runtime::value::ObjectRef, key: &str) -> Option<String> {
    get_via_str(rt, id, key)
}

fn encode_legacy_format_pathname(pathname: &str) -> String {
    let mut out = String::with_capacity(pathname.len());
    for ch in pathname.chars() {
        match ch {
            ' ' => out.push_str("%20"),
            '"' => out.push_str("%22"),
            '#' => out.push_str("%23"),
            '?' => out.push_str("%3F"),
            _ => out.push(ch),
        }
    }
    out
}

fn encode_legacy_format_auth(auth: &str) -> String {
    let mut out = String::with_capacity(auth.len());
    for b in auth.as_bytes() {
        match *b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')'
            | b':'
            | b'%' => out.push(*b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn encode_legacy_format_search(search: &str) -> String {
    search.replace('#', "%23")
}

fn legacy_format_host(rt: &mut Runtime, id: rusty_js_runtime::value::ObjectRef) -> Option<String> {
    if let Some(host) = obj_str(rt, id, "host").filter(|h| !h.is_empty()) {
        return Some(host);
    }
    let mut hostname = obj_str(rt, id, "hostname").filter(|h| !h.is_empty())?;
    if hostname.contains(':') && !hostname.starts_with('[') && !hostname.ends_with(']') {
        hostname = format!("[{hostname}]");
    }
    if let Some(port) = obj_str(rt, id, "port").filter(|p| !p.is_empty()) {
        hostname.push(':');
        hostname.push_str(&port);
    }
    Some(hostname)
}

fn format_legacy_url_object(
    rt: &mut Runtime,
    id: rusty_js_runtime::value::ObjectRef,
) -> Result<Value, RuntimeError> {
    let protocol = obj_str(rt, id, "protocol");
    if protocol.is_some()
        || obj_str(rt, id, "pathname").is_some()
        || obj_str(rt, id, "search").is_some()
        || matches!(rt.object_get(id, "query"), Value::Object(_))
        || obj_str(rt, id, "hash").is_some()
    {
        let proto = protocol.map(|proto| {
            if proto.ends_with(':') {
                proto
            } else {
                format!("{proto}:")
            }
        });
        let slashes = matches!(rt.object_get(id, "slashes"), Value::Boolean(true))
            || matches!(
                proto.as_deref(),
                Some("http:" | "https:" | "ftp:" | "file:" | "ws:" | "wss:")
            );
        let auth = obj_str(rt, id, "auth")
            .filter(|a| !a.is_empty())
            .map(|a| encode_legacy_format_auth(&a));
        let host = legacy_format_host(rt, id);
        let mut pathname = obj_str(rt, id, "pathname")
            .map(|p| encode_legacy_format_pathname(&p))
            .unwrap_or_default();
        if host.is_some() && !pathname.is_empty() && !pathname.starts_with('/') {
            pathname.insert(0, '/');
        }
        let mut search = obj_str(rt, id, "search").unwrap_or_default();
        if search.is_empty() {
            if let Value::Object(qo) = rt.object_get(id, "query") {
                search = append_legacy_query_from_object(rt, qo);
            }
        } else if !search.starts_with('?') {
            search.insert(0, '?');
        }
        search = encode_legacy_format_search(&search);
        let mut hash = obj_str(rt, id, "hash").unwrap_or_default();
        if !hash.is_empty() && !hash.starts_with('#') {
            hash.insert(0, '#');
        }
        let mut out = proto.unwrap_or_default();
        if slashes {
            out.push_str("//");
        }
        if let Some(a) = auth {
            out.push_str(&a);
            out.push('@');
        }
        if let Some(h) = host {
            out.push_str(&h);
        }
        out.push_str(&pathname);
        out.push_str(&search);
        out.push_str(&hash);
        return Ok(sval(&out));
    }
    Ok(sval(&obj_str(rt, id, "href").unwrap_or_default()))
}

pub fn install_canonical(rt: &mut Runtime) {
    let ns = new_object(rt);

    register_method(rt, ns, "fileURLToPath", |rt, args| {
        let s = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            Some(Value::Object(id)) => match get_via_str(rt, *id, "href") {
                Some(s) => s,
                None => rusty_js_runtime::abstract_ops::to_string(args.first().unwrap())
                    .as_str()
                    .to_string(),
            },
            Some(v) => rusty_js_runtime::abstract_ops::to_string(v)
                .as_str()
                .to_string(),
            None => {
                return Err(RuntimeError::TypeError(
                    "cruft:url.fileURLToPath: missing argument".into(),
                ))
            }
        };
        let path = file_url_to_path_str(&s).map_err(|_| invalid_file_url_host_error(rt))?;
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(path),
        )))
    });
    register_method(rt, ns, "pathToFileURL", |rt, args| {
        let p = resolve_to_absolute(&arg_string(args, 0));
        let file_url = rusty_js_url::path_to_file_href(&p).map_err(|_| {
            RuntimeError::TypeError(format!(
                "cruft:url.pathToFileURL: path must be absolute: {p}"
            ))
        })?;
        let ctor = rt.global_get("URL");
        rt.construct(ctor, vec![sval(&file_url)])
    });
    register_method(rt, ns, "domainToASCII", |_rt, args| {
        let d = arg_string(args, 0);
        Ok(sval(&rusty_js_idna::to_ascii_url(&d).unwrap_or_default()))
    });
    register_method(rt, ns, "domainToUnicode", |_rt, args| {
        let d = arg_string(args, 0);
        Ok(sval(&rusty_js_idna::to_unicode(&d).unwrap_or_default()))
    });

    let url_g = rt.global_get("URL");
    if !matches!(url_g, Value::Undefined) {
        set_constant(rt, ns, "URL", url_g);
    }
    let usp_g = rt.global_get("URLSearchParams");
    if !matches!(usp_g, Value::Undefined) {
        set_constant(rt, ns, "URLSearchParams", usp_g);
    }

    set_constant(rt, ns, "default", Value::Object(ns));
    rt.define_global_property("__cruft_url", Value::Object(ns));
}

pub fn install(rt: &mut Runtime) {
    let url_ns = new_object(rt);

    register_method(rt, url_ns, "fileURLToPath", |rt, args| {
        let s = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            Some(Value::Object(id)) => match get_via_str(rt, *id, "href") {
                Some(s) => s,
                None => rusty_js_runtime::abstract_ops::to_string(args.first().unwrap())
                    .as_str()
                    .to_string(),
            },
            Some(v) => rusty_js_runtime::abstract_ops::to_string(v)
                .as_str()
                .to_string(),
            None => {
                return Err(RuntimeError::TypeError(
                    "url.fileURLToPath: missing argument".into(),
                ))
            }
        };
        let path = file_url_to_path_str(&s).map_err(|_| invalid_file_url_host_error(rt))?;
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(path),
        )))
    });

    register_method(rt, url_ns, "pathToFileURL", |rt, args| {
        let p = resolve_to_absolute(&arg_string(args, 0));
        let file_url = rusty_js_url::path_to_file_href(&p).map_err(|_| {
            RuntimeError::TypeError(format!("url.pathToFileURL: path must be absolute: {p}"))
        })?;
        let ctor = rt.global_get("URL");
        rt.construct(ctor, vec![sval(&file_url)])
    });

    register_method(rt, url_ns, "parse", |rt, args| {
        let s = arg_string(args, 0);
        let parse_query = matches!(args.get(1), Some(Value::Boolean(true)));
        if s.starts_with("mailto:") {
            return Ok(legacy_mailto_object(rt, &s, parse_query));
        }
        match rusty_js_url::parse(&s, None) {
            Ok(u) => Ok(legacy_url_object(
                rt,
                &u,
                parse_query,
                raw_legacy_path(&s).as_deref(),
                raw_legacy_port(&s).as_deref(),
                raw_legacy_port(&s).as_ref().map(|_| s.as_str()),
            )),
            Err(_) => {
                if let Some(o) = legacy_unparseable_absolute_object(rt, &s, parse_query) {
                    return Ok(o);
                }

                let o = rt.alloc_object(Object::new_ordinary());
                for k in [
                    "protocol", "slashes", "auth", "host", "port", "hostname", "hash",
                ] {
                    rt.object_set(o, k.into(), Value::Null);
                }
                let (path_part, query_raw, hash_raw) = split_legacy_relative(&s);
                rt.object_set(o, "pathname".into(), sval(&path_part));
                rt.object_set(
                    o,
                    "hash".into(),
                    hash_raw
                        .map(|h| sval(&format!("#{h}")))
                        .unwrap_or(Value::Null),
                );
                rt.object_set(
                    o,
                    "search".into(),
                    match &query_raw {
                        Some(q) => sval(&format!("?{q}")),
                        None => Value::Null,
                    },
                );
                if parse_query {

                    let qo = legacy_query_object(rt, query_raw);
                    rt.object_set(o, "query".into(), Value::Object(qo));
                } else {
                    rt.object_set(
                        o,
                        "query".into(),
                        match &query_raw {
                            Some(q) => sval(q),
                            None => Value::Null,
                        },
                    );
                }
                let search = query_raw.map(|q| format!("?{q}")).unwrap_or_default();
                rt.object_set(o, "path".into(), sval(&format!("{path_part}{search}")));
                rt.object_set(o, "href".into(), sval(&s));
                Ok(Value::Object(o))
            }
        }
    });

    register_method(rt, url_ns, "format", move |rt, args| {
        let id = match args.first() {
            Some(Value::String(s)) => {
                let parse = rt.object_get(url_ns, "parse");
                let parsed =
                    rt.call_function(parse, Value::Object(url_ns), vec![Value::String(s.clone())])?;
                match parsed {
                    Value::Object(id) => id,
                    _ => return Ok(sval("")),
                }
            }
            Some(Value::Object(id)) => *id,
            _ => return Ok(sval("")),
        };

        if matches!(rt.object_get(id, "__url_href__"), Value::String(_)) {
            let opt_bool = |rt: &mut Runtime, key: &str| -> bool {
                match args.get(1) {
                    Some(Value::Object(o)) => {
                        !matches!(rt.object_get(*o, key), Value::Boolean(false))
                    }
                    _ => true,
                }
            };
            let auth = opt_bool(rt, "auth");
            let fragment = opt_bool(rt, "fragment");
            let search = opt_bool(rt, "search");

            let unicode = match args.get(1) {
                Some(Value::Object(o)) => {
                    matches!(rt.object_get(*o, "unicode"), Value::Boolean(true))
                }
                _ => false,
            };
            let g = |rt: &mut Runtime, k: &str| get_via_str(rt, id, k).unwrap_or_default();
            let mut out = format!("{}//", g(rt, "protocol"));
            let username = g(rt, "username");
            let password = g(rt, "password");
            if auth && (!username.is_empty() || !password.is_empty()) {
                out.push_str(&username);
                if !password.is_empty() {
                    out.push(':');
                    out.push_str(&password);
                }
                out.push('@');
            }
            if unicode {

                let hostname = g(rt, "hostname");
                let decoded = rusty_js_idna::to_unicode(&hostname).unwrap_or(hostname);
                out.push_str(&decoded);
                let port = g(rt, "port");
                if !port.is_empty() {
                    out.push(':');
                    out.push_str(&port);
                }
            } else {
                out.push_str(&g(rt, "host"));
            }
            out.push_str(&g(rt, "pathname"));
            if search {
                out.push_str(&g(rt, "search"));
            }
            if fragment {
                out.push_str(&g(rt, "hash"));
            }
            return Ok(sval(&out));
        }
        format_legacy_url_object(rt, id)
    });

    register_method(rt, url_ns, "resolve", |_rt, args| {
        let from = arg_string(args, 0);
        let to = arg_string(args, 1);
        match rusty_js_url::join_to_href(&from, &to) {
            Ok(href) => Ok(sval(&href)),
            Err(_) => Ok(sval(
                &legacy_relative_resolve(&from, &to).unwrap_or_else(|| to.clone()),
            )),
        }
    });

    let url_class = make_callable(rt, "Url", |rt, _args| {
        let o = rt.alloc_object(Object::new_ordinary());
        for k in [
            "protocol", "slashes", "auth", "host", "port", "hostname", "hash", "search", "query",
            "pathname", "path", "href",
        ] {
            rt.object_set(o, k.into(), Value::Null);
        }
        Ok(Value::Object(o))
    });
    set_constant(rt, url_ns, "Url", Value::Object(url_class));

    register_method(rt, url_ns, "domainToASCII", |_rt, args| {
        let d = arg_string(args, 0);
        Ok(sval(&rusty_js_idna::to_ascii_url(&d).unwrap_or_default()))
    });
    register_method(rt, url_ns, "domainToUnicode", |_rt, args| {
        let d = arg_string(args, 0);
        Ok(sval(&rusty_js_idna::to_unicode(&d).unwrap_or_default()))
    });

    register_method(rt, url_ns, "urlToHttpOptions", |rt, args| {
        let id = match args.first() {
            Some(Value::Object(id)) => *id,
            _ => {
                return Err(RuntimeError::TypeError(
                    "url.urlToHttpOptions: argument must be a URL".into(),
                ))
            }
        };
        let o = rt.alloc_object(Object::new_ordinary());
        let source_v = Value::Object(id);
        let out_v = Value::Object(o);
        let _option_roots = rt.push_temporary_value_roots(&[source_v.clone(), out_v.clone()]);

        for k in ["protocol", "hostname", "hash", "search", "pathname"] {

            let v = rt.get_via(&source_v, &sval(k)).unwrap_or(Value::Undefined);
            let _value_roots =
                rt.push_temporary_value_roots(&[source_v.clone(), out_v.clone(), v.clone()]);
            rt.object_set(o, k.into(), v);
        }
        let pathname = obj_str(rt, id, "pathname").unwrap_or_default();
        let search = obj_str(rt, id, "search").unwrap_or_default();
        rt.object_set(o, "path".into(), sval(&format!("{pathname}{search}")));
        let href_v = rt
            .get_via(&source_v, &sval("href"))
            .unwrap_or(Value::Undefined);
        rt.object_set(o, "href".into(), href_v);

        let port_v = rt
            .get_via(&source_v, &sval("port"))
            .unwrap_or(Value::Undefined);
        if let Value::String(s) = &port_v {
            if !s.is_empty() {
                match s.as_str().parse::<f64>() {
                    Ok(n) => rt.object_set(o, "port".into(), Value::Number(n)),
                    Err(_) => rt.object_set(o, "port".into(), Value::String(s.clone())),
                }
            }
        }

        let user = obj_str(rt, id, "username").unwrap_or_default();
        let pass = obj_str(rt, id, "password").unwrap_or_default();
        if !user.is_empty() || !pass.is_empty() {
            rt.object_set(o, "auth".into(), sval(&format!("{user}:{pass}")));
        }
        Ok(Value::Object(o))
    });

    let url_g = rt.global_get("URL");
    if !matches!(url_g, Value::Undefined) {
        set_constant(rt, url_ns, "URL", url_g);
    } else {
        register_method(rt, url_ns, "URL", |_rt, _args| {
            Err(RuntimeError::TypeError(
                "node:url URL constructor: not yet implemented (Tier-Ω.5.s stub)".into(),
            ))
        });
    }
    let usp_g = rt.global_get("URLSearchParams");
    if !matches!(usp_g, Value::Undefined) {
        set_constant(rt, url_ns, "URLSearchParams", usp_g);
    } else {
        register_method(rt, url_ns, "URLSearchParams", |_rt, _args| {
            Err(RuntimeError::TypeError(
                "node:url URLSearchParams: not yet implemented (Tier-Ω.5.s stub)".into(),
            ))
        });
    }

    {
        let c = crate::register::make_callable(rt, "URLPattern", |rt, _a| Ok(rt.current_this()));
        let p = new_object(rt);
        rt.object_set(p, "constructor".into(), Value::Object(c));
        rt.object_set(c, "prototype".into(), Value::Object(p));
        rt.object_set(url_ns, "URLPattern".into(), Value::Object(c));
    }
    register_method(rt, url_ns, "fileURLToPathBuffer", |_rt, _a| {
        Ok(Value::Undefined)
    });
    register_method(rt, url_ns, "resolveObject", |_rt, _a| Ok(Value::Undefined));
    rt.define_global_property("url", Value::Object(url_ns));
}
