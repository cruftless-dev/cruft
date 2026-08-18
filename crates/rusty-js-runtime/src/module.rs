
use crate::interp::{inject_throw_into_frame, Frame, FrameSnapshot, Runtime, RuntimeError};
use crate::value::{
    CapturedBinding, InternalKind, Object, ObjectRef, PromiseReaction, PromiseStatus, Value,
};
use rusty_js_ast::Module as AstModule;
use rusty_js_bytecode::{
    CompiledModule, Constant, ExportBinding, ImportBindingKind, UpvalueSource,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

enum ModuleResume {

    Start,

    Value(Value),

    Throw(Value),
}

fn module_requests_in_source_order(ast: &AstModule) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for item in &ast.body {
        let spec: &str = match item {
            rusty_js_ast::ModuleItem::Import(imp) => {
                if imp.source_phase {
                    continue;
                }
                &imp.specifier.value
            }
            rusty_js_ast::ModuleItem::Export(export) => match export {
                rusty_js_ast::ExportDeclaration::Named {
                    source: Some(source),
                    ..
                }
                | rusty_js_ast::ExportDeclaration::StarFrom { source, .. }
                | rusty_js_ast::ExportDeclaration::StarAsFrom { source, .. } => &source.value,
                _ => continue,
            },
            _ => continue,
        };
        if seen.insert(spec.to_string()) {
            out.push(spec.to_string());
        }
    }
    out
}

fn file_url_specifier_path(specifier: &str) -> Option<std::path::PathBuf> {
    let rest = if let Some(rest) = specifier.strip_prefix("file://") {
        rest
    } else if let Some(rest) = specifier.strip_prefix("file:") {
        rest
    } else {
        return None;
    };
    if !rest.starts_with('/') {
        return None;
    }
    let end = rest.find(['?', '#']).unwrap_or(rest.len());
    Some(std::path::PathBuf::from(&rest[..end]))
}

fn data_url_base64_decode(input: &str) -> Option<Vec<u8>> {
    fn val(b: u8) -> Option<u8> {
        match b {
            b'A'..=b'Z' => Some(b - b'A'),
            b'a'..=b'z' => Some(b - b'a' + 26),
            b'0'..=b'9' => Some(b - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut chunk = 0u32;
    let mut n = 0u32;
    for b in input.bytes() {
        if b.is_ascii_whitespace() || b == b'=' {
            continue;
        }
        chunk = (chunk << 6) | val(b)? as u32;
        n += 1;
        if n == 4 {
            out.push((chunk >> 16) as u8);
            out.push((chunk >> 8) as u8);
            out.push(chunk as u8);
            chunk = 0;
            n = 0;
        }
    }
    match n {
        0 => {}
        2 => out.push((chunk >> 4) as u8),
        3 => {
            out.push((chunk >> 10) as u8);
            out.push((chunk >> 2) as u8);
        }
        _ => return None,
    }
    Some(out)
}

fn data_url_percent_decode(input: &str) -> Vec<u8> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h = (bytes[i + 1] as char).to_digit(16);
            let l = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (h, l) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

fn parse_data_url(url: &str) -> Option<(String, Vec<u8>)> {
    let rest = url.strip_prefix("data:")?;
    let comma = rest.find(',')?;
    let meta = &rest[..comma];
    let data = &rest[comma + 1..];
    let is_base64 = meta.to_ascii_lowercase().ends_with(";base64");
    let meta = if is_base64 {
        &meta[..meta.len() - ";base64".len()]
    } else {
        meta
    };
    let mediatype = meta
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let mediatype = if mediatype.is_empty() {
        "text/plain".to_string()
    } else {
        mediatype
    };
    let payload = if is_base64 {
        data_url_base64_decode(data)?
    } else {
        data_url_percent_decode(data)
    };
    Some((mediatype, payload))
}

fn strip_file_specifier_query_fragment(specifier: &str) -> &str {
    let is_file_ish = specifier.starts_with("./")
        || specifier.starts_with("../")
        || specifier.starts_with('/')
        || specifier.starts_with("file:");
    if !is_file_ish {
        return specifier;
    }
    let cut = specifier.find(['?', '#']).unwrap_or(specifier.len());
    &specifier[..cut]
}

fn module_job_roots(
    namespace: ObjectRef,
    snapshot: &FrameSnapshot,
    extra: &[Value],
) -> Vec<ObjectRef> {
    let mut roots = Vec::new();
    roots.push(namespace);
    snapshot.trace_object_refs(&mut roots);
    for value in extra {
        if let Value::Object(id) = value {
            roots.push(*id);
        }
    }
    roots
}

pub mod phase_profile {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::OnceLock;
    pub static PARSE_NS: AtomicU64 = AtomicU64::new(0);
    pub static COMPILE_NS: AtomicU64 = AtomicU64::new(0);
    pub static EVAL_NS: AtomicU64 = AtomicU64::new(0);

    pub static CJS_BODY_CALL_NS: AtomicU64 = AtomicU64::new(0);
    pub static CJS_BODY_CALL_COUNT: AtomicU64 = AtomicU64::new(0);
    pub static RESOLVE_NS: AtomicU64 = AtomicU64::new(0);
    pub static RESOLVE_COUNT: AtomicU64 = AtomicU64::new(0);
    pub static RESOLVE_CACHE_HITS: AtomicU64 = AtomicU64::new(0);
    pub static RESOLVE_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);
    pub static CJS_REQUIRE_BUILTIN_NS: AtomicU64 = AtomicU64::new(0);
    pub static CJS_REQUIRE_CACHE_NS: AtomicU64 = AtomicU64::new(0);
    pub static CJS_REQUIRE_LOAD_NS: AtomicU64 = AtomicU64::new(0);
    pub static CJS_REQUIRE_LOAD_EXCLUSIVE_NS: AtomicU64 = AtomicU64::new(0);
    pub static CJS_REQUIRE_EXPORT_NS: AtomicU64 = AtomicU64::new(0);
    pub static CJS_REQUIRE_ARG_NS: AtomicU64 = AtomicU64::new(0);
    pub static CJS_REQUIRE_CAPS_NS: AtomicU64 = AtomicU64::new(0);
    pub static CJS_REQUIRE_NATIVE_TOTAL_NS: AtomicU64 = AtomicU64::new(0);
    pub static CJS_REQUIRE_NATIVE_RESIDUAL_NS: AtomicU64 = AtomicU64::new(0);
    pub static CJS_REQUIRE_CLOSURE_NS: AtomicU64 = AtomicU64::new(0);
    pub static CJS_REQUIRE_STACK_SHADOW_NS: AtomicU64 = AtomicU64::new(0);
    pub static CJS_REQUIRE_INNER_NS: AtomicU64 = AtomicU64::new(0);
    pub static CJS_REQUIRE_RESOLVE_NS: AtomicU64 = AtomicU64::new(0);
    pub static CJS_REQUIRE_CALLS: AtomicU64 = AtomicU64::new(0);
    pub static CJS_REQUIRE_CACHE_HITS: AtomicU64 = AtomicU64::new(0);
    pub static CJS_REQUIRE_LOAD_CALLS: AtomicU64 = AtomicU64::new(0);
    pub static CJS_WRAPPER_SETUP_NS: AtomicU64 = AtomicU64::new(0);
    pub static CJS_WRAPPER_PARSE_NS: AtomicU64 = AtomicU64::new(0);
    pub static CJS_WRAPPER_STATIC_EXPORT_NS: AtomicU64 = AtomicU64::new(0);
    pub static CJS_WRAPPER_COMPILE_NS: AtomicU64 = AtomicU64::new(0);
    pub static CJS_WRAPPER_MODULE_EVAL_NS: AtomicU64 = AtomicU64::new(0);
    pub static CJS_WRAPPER_BODY_EXCLUSIVE_NS: AtomicU64 = AtomicU64::new(0);
    pub static CJS_WRAPPER_POST_BODY_NS: AtomicU64 = AtomicU64::new(0);
    pub static CJS_EVALUATE_CALLS: AtomicU64 = AtomicU64::new(0);
    pub static PREFLIGHT_CACHE_HITS: AtomicU64 = AtomicU64::new(0);
    pub static PREFLIGHT_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);
    pub static READ_NS: AtomicU64 = AtomicU64::new(0);
    pub static PREFLIGHT_NS: AtomicU64 = AtomicU64::new(0);
    pub static PREFLIGHT_CLASSIFY_NS: AtomicU64 = AtomicU64::new(0);
    pub static PREFLIGHT_PARSE_NS: AtomicU64 = AtomicU64::new(0);
    pub static PREFLIGHT_NAMED_EXPORT_VALIDATION_NS: AtomicU64 = AtomicU64::new(0);
    pub static EXPORT_NAME_COLLECTION_CALLS: AtomicU64 = AtomicU64::new(0);
    pub static EXPORT_NAME_COLLECTION_READ_NS: AtomicU64 = AtomicU64::new(0);
    pub static EXPORT_NAME_COLLECTION_PARSE_NS: AtomicU64 = AtomicU64::new(0);
    pub static EXPORT_NAME_COLLECTION_RESOLVE_NS: AtomicU64 = AtomicU64::new(0);
    pub static EXPORT_NAME_COLLECTION_STAR_EDGES: AtomicU64 = AtomicU64::new(0);
    pub static EXPORT_NAME_COLLECTION_CJS_LOAD_NS: AtomicU64 = AtomicU64::new(0);
    pub static EXPORT_NAME_COLLECTION_CJS_KEYS_NS: AtomicU64 = AtomicU64::new(0);
    pub static STATIC_DEPS_NS: AtomicU64 = AtomicU64::new(0);
    pub static STATIC_DEP_RESOLVE_NS: AtomicU64 = AtomicU64::new(0);
    pub static STATIC_DEP_LOAD_NS: AtomicU64 = AtomicU64::new(0);
    pub static STATIC_DEP_LOAD_EXCLUSIVE_NS: AtomicU64 = AtomicU64::new(0);
    pub static STATIC_DEP_POST_LOAD_NS: AtomicU64 = AtomicU64::new(0);
    pub static STATIC_DEP_EDGES: AtomicU64 = AtomicU64::new(0);
    pub static STATIC_DEP_IMPORT_EDGES: AtomicU64 = AtomicU64::new(0);
    pub static STATIC_DEP_REEXPORT_EDGES: AtomicU64 = AtomicU64::new(0);
    pub static STATIC_DEP_LOAD_CALLS: AtomicU64 = AtomicU64::new(0);
    pub static STATIC_DEP_LOAD_NEW: AtomicU64 = AtomicU64::new(0);
    pub static STATIC_DEP_LOAD_EXISTING_LINKING: AtomicU64 = AtomicU64::new(0);
    pub static STATIC_DEP_LOAD_EXISTING_EVALUATING: AtomicU64 = AtomicU64::new(0);
    pub static STATIC_DEP_LOAD_EXISTING_EVALUATED: AtomicU64 = AtomicU64::new(0);
    pub static STATIC_DEP_LOAD_EXISTING_FAILED: AtomicU64 = AtomicU64::new(0);
    pub static STATIC_DEP_LOAD_EXISTING_OTHER: AtomicU64 = AtomicU64::new(0);
    pub static STATIC_DEP_VISITED_BEFORE: AtomicU64 = AtomicU64::new(0);
    pub static STATIC_DEP_CYCLE_COLLAPSES: AtomicU64 = AtomicU64::new(0);
    pub static STATIC_DEP_WAIT_CHECKS: AtomicU64 = AtomicU64::new(0);
    pub static STATIC_DEP_WAIT_PUSHES: AtomicU64 = AtomicU64::new(0);
    pub static IMPORT_BINDINGS_NS: AtomicU64 = AtomicU64::new(0);
    pub static EXPORT_CELLS_NS: AtomicU64 = AtomicU64::new(0);
    pub static NAMESPACE_NS: AtomicU64 = AtomicU64::new(0);
    pub static MODULE_COUNT: AtomicU64 = AtomicU64::new(0);
    static ENABLED: OnceLock<bool> = OnceLock::new();
    pub fn enabled() -> bool {
        *ENABLED.get_or_init(|| {
            std::env::var("CRUFT_PROFILE_MODULE").is_ok() || std::env::var("CRUFT_PROFILE").is_ok()
        })
    }
    pub fn add(c: &'static AtomicU64, ns: u64) {
        c.fetch_add(ns, Ordering::Relaxed);
    }
    pub fn inc(c: &'static AtomicU64) {
        c.fetch_add(1, Ordering::Relaxed);
    }
    pub fn read(c: &'static AtomicU64) -> u64 {
        c.load(Ordering::Relaxed)
    }

    #[derive(Default)]
    struct StaticDepLoadRow {
        calls: u64,
        exclusive_ns: u64,
        total_ns: u64,
    }

    #[derive(Default)]
    struct CjsRequirePhaseRow {
        calls: u64,
        total_ns: u64,
        residual_ns: u64,
        builtin_ns: u64,
        resolve_ns: u64,
        cache_ns: u64,
        load_ns: u64,
        load_exclusive_ns: u64,
        export_ns: u64,
    }

    thread_local! {
        static CJS_REQUIRE_LOAD_CHILD_STACK: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };
        static STATIC_DEP_LOAD_PROFILE: RefCell<HashMap<String, StaticDepLoadRow>> = RefCell::new(HashMap::new());
        static CJS_REQUIRE_LOAD_PROFILE: RefCell<HashMap<String, StaticDepLoadRow>> = RefCell::new(HashMap::new());
        static CJS_REQUIRE_PHASE_PROFILE: RefCell<HashMap<String, CjsRequirePhaseRow>> = RefCell::new(HashMap::new());
    }

    pub fn cjs_require_load_enter() {
        CJS_REQUIRE_LOAD_CHILD_STACK.with(|stack| stack.borrow_mut().push(0));
    }

    pub fn cjs_require_load_exit(total_ns: u64) -> u64 {
        CJS_REQUIRE_LOAD_CHILD_STACK.with(|stack| {
            let mut stack = stack.borrow_mut();
            let child_ns = stack.pop().unwrap_or(0);
            if let Some(parent_child_ns) = stack.last_mut() {
                *parent_child_ns = parent_child_ns.saturating_add(total_ns);
            }
            total_ns.saturating_sub(child_ns)
        })
    }

    pub fn cjs_require_load_child_ns() -> u64 {
        CJS_REQUIRE_LOAD_CHILD_STACK.with(|stack| stack.borrow().last().copied().unwrap_or(0))
    }

    pub fn static_dep_load_profile_enabled() -> bool {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| std::env::var("CRUFT_STATIC_DEP_LOAD_PROFILE").is_ok())
    }

    pub fn static_dep_load_profile_filter() -> Option<String> {
        static FILTER: OnceLock<Option<String>> = OnceLock::new();
        FILTER
            .get_or_init(|| {
                std::env::var("CRUFT_STATIC_DEP_LOAD_PROFILE_FILTER")
                    .ok()
                    .filter(|v| !v.is_empty())
            })
            .clone()
    }

    pub fn cjs_require_load_profile_enabled() -> bool {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| std::env::var("CRUFT_CJS_REQUIRE_LOAD_PROFILE").is_ok())
    }

    pub fn cjs_require_load_profile_filter() -> Option<String> {
        static FILTER: OnceLock<Option<String>> = OnceLock::new();
        FILTER
            .get_or_init(|| {
                std::env::var("CRUFT_CJS_REQUIRE_LOAD_PROFILE_FILTER")
                    .ok()
                    .filter(|v| !v.is_empty())
            })
            .clone()
    }

    pub fn cjs_require_phase_profile_enabled() -> bool {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| std::env::var("CRUFT_CJS_REQUIRE_PHASE_PROFILE").is_ok())
    }

    pub fn cjs_require_phase_profile_filter() -> Option<String> {
        static FILTER: OnceLock<Option<String>> = OnceLock::new();
        FILTER
            .get_or_init(|| {
                std::env::var("CRUFT_CJS_REQUIRE_PHASE_PROFILE_FILTER")
                    .ok()
                    .filter(|v| !v.is_empty())
            })
            .clone()
    }

    pub fn record_static_dep_load_profile(label: String, total_ns: u64, exclusive_ns: u64) {
        if !static_dep_load_profile_enabled() {
            return;
        }
        let total = STATIC_DEP_LOAD_CALLS.load(Ordering::Relaxed);
        STATIC_DEP_LOAD_PROFILE.with(|profile| {
            let mut profile = profile.borrow_mut();
            let row = profile.entry(label).or_default();
            row.calls += 1;
            row.total_ns = row.total_ns.saturating_add(total_ns);
            row.exclusive_ns = row.exclusive_ns.saturating_add(exclusive_ns);
            if total <= 16 || total % 128 == 0 {
                let mut rows: Vec<_> = profile
                    .iter()
                    .map(|(label, row)| (label.clone(), row.calls, row.total_ns, row.exclusive_ns))
                    .collect();
                rows.sort_by(|a, b| b.3.cmp(&a.3).then_with(|| b.1.cmp(&a.1)));
                rows.truncate(8);
                let rows = rows
                    .into_iter()
                    .map(|(label, calls, total_ns, exclusive_ns)| {
                        format!(
                            "{label}:calls={calls}:total_ns={total_ns}:exclusive_ns={exclusive_ns}"
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" | ");
                eprintln!("[static-dep-load-profile] total={total} top=[{rows}]");
            }
        });
    }

    pub fn record_cjs_require_load_profile(label: String, total_ns: u64, exclusive_ns: u64) {
        if !cjs_require_load_profile_enabled() {
            return;
        }
        let total = CJS_REQUIRE_LOAD_CALLS.load(Ordering::Relaxed);
        CJS_REQUIRE_LOAD_PROFILE.with(|profile| {
            let mut profile = profile.borrow_mut();
            let row = profile.entry(label).or_default();
            row.calls += 1;
            row.total_ns = row.total_ns.saturating_add(total_ns);
            row.exclusive_ns = row.exclusive_ns.saturating_add(exclusive_ns);
            if total <= 16 || total % 16 == 0 {
                let mut rows: Vec<_> = profile
                    .iter()
                    .map(|(label, row)| (label.clone(), row.calls, row.total_ns, row.exclusive_ns))
                    .collect();
                rows.sort_by(|a, b| b.3.cmp(&a.3).then_with(|| b.1.cmp(&a.1)));
                rows.truncate(10);
                let rows = rows
                    .into_iter()
                    .map(|(label, calls, total_ns, exclusive_ns)| {
                        format!(
                            "{label}:calls={calls}:total_ns={total_ns}:exclusive_ns={exclusive_ns}"
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" | ");
                eprintln!("[cjs-require-load-profile] total={total} top=[{rows}]");
            }
        });
    }

    pub fn record_cjs_require_phase_profile(
        label: String,
        total_ns: u64,
        builtin_ns: u64,
        resolve_ns: u64,
        cache_ns: u64,
        load_ns: u64,
        load_exclusive_ns: u64,
        export_ns: u64,
    ) {
        if !cjs_require_phase_profile_enabled() {
            return;
        }
        let residual_ns = total_ns.saturating_sub(load_ns);
        let total = CJS_REQUIRE_CALLS.load(Ordering::Relaxed);
        CJS_REQUIRE_PHASE_PROFILE.with(|profile| {
            let mut profile = profile.borrow_mut();
            let row = profile.entry(label).or_default();
            row.calls += 1;
            row.total_ns = row.total_ns.saturating_add(total_ns);
            row.residual_ns = row.residual_ns.saturating_add(residual_ns);
            row.builtin_ns = row.builtin_ns.saturating_add(builtin_ns);
            row.resolve_ns = row.resolve_ns.saturating_add(resolve_ns);
            row.cache_ns = row.cache_ns.saturating_add(cache_ns);
            row.load_ns = row.load_ns.saturating_add(load_ns);
            row.load_exclusive_ns = row.load_exclusive_ns.saturating_add(load_exclusive_ns);
            row.export_ns = row.export_ns.saturating_add(export_ns);
            if total <= 16 || total % 16 == 0 {
                let mut rows: Vec<_> = profile
                    .iter()
                    .map(|(label, row)| {
                        (
                            label.clone(),
                            row.calls,
                            row.total_ns,
                            row.residual_ns,
                            row.builtin_ns,
                            row.resolve_ns,
                            row.cache_ns,
                            row.load_ns,
                            row.load_exclusive_ns,
                            row.export_ns,
                        )
                    })
                    .collect();
                rows.sort_by(|a, b| b.3.cmp(&a.3).then_with(|| b.2.cmp(&a.2)));
                rows.truncate(10);
                let rows = rows
                    .into_iter()
                    .map(
                        |(
                            label,
                            calls,
                            total_ns,
                            residual_ns,
                            builtin_ns,
                            resolve_ns,
                            cache_ns,
                            load_ns,
                            load_exclusive_ns,
                            export_ns,
                        )| {
                            format!(
                                "{label}:calls={calls}:total_ns={total_ns}:residual_ns={residual_ns}:builtin_ns={builtin_ns}:resolve_ns={resolve_ns}:cache_ns={cache_ns}:load_ns={load_ns}:load_exclusive_ns={load_exclusive_ns}:export_ns={export_ns}"
                            )
                        },
                    )
                    .collect::<Vec<_>>()
                    .join(" | ");
                eprintln!("[cjs-require-phase-profile] total={total} top=[{rows}]");
            }
        });
    }
}

fn cjs_load_trace(event: impl std::fmt::Display) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::OnceLock;

    static LIMIT: OnceLock<Option<usize>> = OnceLock::new();
    static EVENTS: AtomicUsize = AtomicUsize::new(0);
    const DEFAULT_LIMIT: usize = 256;

    let Some(limit) = *LIMIT.get_or_init(|| match std::env::var("CRUFT_CJS_LOAD_TRACE") {
        Ok(raw) => {
            let raw = raw.trim();
            if raw.is_empty() || raw == "0" || raw.eq_ignore_ascii_case("false") {
                None
            } else {
                Some(raw.parse::<usize>().unwrap_or(DEFAULT_LIMIT))
            }
        }
        Err(_) => None,
    }) else {
        return;
    };
    let event_no = EVENTS.fetch_add(1, Ordering::Relaxed);
    if event_no < limit {
        eprintln!("[cruft:cjs-load:{event_no}] {event}");
    } else if event_no == limit {
        eprintln!("[cruft:cjs-load] trace limit {limit} reached");
    }
}

fn cjs_wrapper_phase_profile_matches(url: &str) -> bool {
    static FILTER: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    let Some(filter) = FILTER
        .get_or_init(|| {
            std::env::var("CRUFT_CJS_WRAPPER_PHASE_PROFILE_FILTER")
                .ok()
                .filter(|v| !v.is_empty())
        })
        .as_deref()
    else {
        return false;
    };
    filter
        .split('|')
        .any(|part| !part.is_empty() && url.contains(part))
}

fn static_dep_load_edge_profile_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_STATIC_DEP_LOAD_PROFILE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn static_dep_load_edge_profile_filter() -> Option<&'static str> {
    static FILTER: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    FILTER
        .get_or_init(|| {
            std::env::var("CRUFT_STATIC_DEP_LOAD_PROFILE_FILTER")
                .ok()
                .filter(|v| !v.is_empty())
        })
        .as_deref()
}

fn static_dep_load_edge_profile_matches(parent: &str, spec: &str, resolved: &str) -> bool {
    if !static_dep_load_edge_profile_enabled() {
        return false;
    }
    match static_dep_load_edge_profile_filter() {
        Some(filter) => {
            parent.contains(filter) || spec.contains(filter) || resolved.contains(filter)
        }
        None => true,
    }
}

fn maybe_report_static_dep_load_profile(
    parent: &str,
    spec: &str,
    resolved: &str,
    type_attr: Option<&str>,
    elapsed_ns: u64,
    nested_static_deps_ns: u64,
) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static EVENTS: AtomicUsize = AtomicUsize::new(0);
    const LIMIT: usize = 512;

    if !static_dep_load_edge_profile_matches(parent, spec, resolved) {
        return;
    }
    let event_no = EVENTS.fetch_add(1, Ordering::Relaxed);
    if event_no < LIMIT {
        eprintln!(
            "[static-dep-load-profile] event={} parent={} spec={} resolved={} type_attr={} elapsed_ns={} nested_static_deps_ns={} exclusive_ns={}",
            event_no,
            parent,
            spec,
            resolved,
            type_attr.unwrap_or(""),
            elapsed_ns,
            nested_static_deps_ns,
            elapsed_ns.saturating_sub(nested_static_deps_ns)
        );
    } else if event_no == LIMIT {
        eprintln!("[static-dep-load-profile] trace limit {LIMIT} reached");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleStatus {
    Unlinked,
    Linking,
    Linked,

    Evaluating,

    EvaluatingAsync,
    Evaluated,
    Failed,
}

pub struct DeferredImportBinding {
    pub cell: crate::value::UpvalueCell,
    pub kind: rusty_js_bytecode::ImportBindingKind,

    pub is_cjs: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModuleKind {
    ESM,
    CJS,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ResolveExportResult {
    Resolved {
        module_url: String,
        binding_name: String,
    },
    NotFound,
    Ambiguous,

    Incomplete,
}

pub struct ModuleRecord {
    pub url: String,
    pub status: ModuleStatus,
    pub ast: Rc<AstModule>,
    pub bytecode: Rc<CompiledModule>,
    pub namespace: Option<ObjectRef>,
    pub eval_error: Option<RuntimeError>,

    pub kind: ModuleKind,

    pub cjs_exports: Option<Value>,

    pub export_cells: std::collections::HashMap<String, crate::value::UpvalueCell>,

    pub async_static_deps: Vec<String>,
    pub body_completed_waiting_async_deps: bool,
    pub pending_body_start: Option<FrameSnapshot>,
    pub async_evaluation_order: Option<u64>,
    pub async_cycle_root: Option<String>,
}

pub(crate) fn runtime_error_to_rejection_value(rt: &mut Runtime, e: &RuntimeError) -> Value {
    match e {
        RuntimeError::Thrown(v) => v.clone(),
        RuntimeError::TypeError(m) => {

            let (name, msg) = if let Some(node_msg) = m.strip_prefix("__node_resolve_error__:") {
                ("Error", node_msg.to_string())
            } else if let Some(node_msg) =
                crate::intrinsics::node_style_missing_package_projection(m)
            {
                ("Error", node_msg)
            } else {
                ("TypeError", m.clone())
            };
            crate::intrinsics::make_error_instance(rt, name, &msg)
                .map(|id| {
                    if name == "Error" {

                        crate::intrinsics::attach_node_resolution_code(rt, id, &msg, false);
                    }
                    Value::Object(id)
                })
                .unwrap_or_else(|| {
                    Value::String(Rc::new(crate::value::JsString::from(format!(
                        "{}: {}",
                        name, msg
                    ))))
                })
        }
        RuntimeError::RangeError(m) => crate::intrinsics::make_error_instance(rt, "RangeError", m)
            .map(Value::Object)
            .unwrap_or_else(|| {
                Value::String(Rc::new(crate::value::JsString::from(format!(
                    "RangeError: {}",
                    m
                ))))
            }),
        RuntimeError::ReferenceError(m) => {
            crate::intrinsics::make_error_instance(rt, "ReferenceError", m)
                .map(Value::Object)
                .unwrap_or_else(|| {
                    Value::String(Rc::new(crate::value::JsString::from(format!(
                        "ReferenceError: {}",
                        m
                    ))))
                })
        }
        RuntimeError::SyntaxError(m) | RuntimeError::CompileError(m) => {
            crate::intrinsics::make_error_instance(rt, "SyntaxError", m)
                .map(Value::Object)
                .unwrap_or_else(|| {
                    Value::String(Rc::new(crate::value::JsString::from(format!(
                        "SyntaxError: {}",
                        m
                    ))))
                })
        }
        other => Value::String(Rc::new(crate::value::JsString::from(format!(
            "{:?}",
            other
        )))),
    }
}

pub fn detect_module_kind(resolved_url: &str) -> ModuleKind {

    if resolved_url.starts_with("node:")
        || resolved_url.starts_with("cruft:")
        || resolved_url.starts_with("bun:") || resolved_url.starts_with("deno:")
    {
        return ModuleKind::ESM;
    }
    let path_str = match resolved_url.strip_prefix("file://") {
        Some(p) => p,
        None => return ModuleKind::CJS,
    };
    let path = std::path::Path::new(path_str);
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    match ext {
        "mjs" => ModuleKind::ESM,
        "cjs" => ModuleKind::CJS,

        "cts" => ModuleKind::CJS,
        "ts" | "mts" => {
            if ext == "mts" {
                return ModuleKind::ESM;
            }
            let mut cur = path.parent();
            while let Some(d) = cur {
                let candidate = d.join("package.json");
                if candidate.is_file() {
                    if let Ok(text) = std::fs::read_to_string(&candidate) {
                        if let Some(t) = scan_package_type(&text) {
                            return if t == "commonjs" {
                                ModuleKind::CJS
                            } else {
                                ModuleKind::ESM
                            };
                        }

                        return ModuleKind::ESM;
                    }
                }
                cur = d.parent();
            }

            ModuleKind::ESM
        }
        _ => {

            let mut cur = path.parent();
            while let Some(d) = cur {
                let candidate = d.join("package.json");
                if candidate.is_file() {
                    if let Ok(text) = std::fs::read_to_string(&candidate) {

                        if let Some(t) = scan_package_type(&text) {
                            return if t == "module" {
                                ModuleKind::ESM
                            } else {
                                ModuleKind::CJS
                            };
                        }

                        return ModuleKind::CJS;
                    }
                }
                cur = d.parent();
            }
            if let Ok(head) = read_source_head(path, 65536) {
                if source_has_esm_markers(&head) && !source_has_cjs_export_markers(&head) {
                    return ModuleKind::ESM;
                }
            }
            ModuleKind::CJS
        }
    }
}

fn url_under_node_modules(resolved_url: &str) -> bool {
    resolved_url.contains("/node_modules/")
}

fn read_source_head(path: &std::path::Path, n: usize) -> std::io::Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut buf = vec![0u8; n];
    let read = f.read(&mut buf)?;
    buf.truncate(read);
    Ok(String::from_utf8_lossy(&buf).to_string())
}

fn blank_comments_and_strings(src: &str) -> String {

    fn prev_token_completes_expression(out: &str) -> bool {
        let trimmed = out.trim_end();
        let Some(last) = trimmed.chars().last() else {
            return false;
        };
        if matches!(last, ')' | ']' | '.') {
            return true;
        }
        if last.is_alphanumeric() || last == '_' || last == '$' {

            let word: String = trimmed
                .chars()
                .rev()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            return !matches!(
                word.as_str(),
                "return"
                    | "typeof"
                    | "instanceof"
                    | "in"
                    | "of"
                    | "new"
                    | "delete"
                    | "void"
                    | "throw"
                    | "case"
                    | "do"
                    | "else"
                    | "yield"
                    | "await"
            );
        }
        false
    }
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '/' if chars.peek() == Some(&'/') => {
                while let Some(&n) = chars.peek() {
                    if n == '\n' {
                        break;
                    }
                    chars.next();
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                while let Some(n) = chars.next() {
                    if n == '\n' {
                        out.push('\n');
                    }
                    if n == '*' && chars.peek() == Some(&'/') {
                        chars.next();
                        break;
                    }
                }
            }
            '/' if !prev_token_completes_expression(&out) => {

                out.push('/');
                let mut in_class = false;
                while let Some(n) = chars.next() {
                    match n {
                        '\\' => {
                            chars.next();
                        }
                        '[' => in_class = true,
                        ']' => in_class = false,
                        '\n' => {

                            out.push('\n');
                            break;
                        }
                        '/' if !in_class => {
                            out.push('/');
                            while matches!(chars.peek(), Some(f) if f.is_alphanumeric()) {
                                chars.next();
                            }
                            break;
                        }
                        _ => {}
                    }
                }
            }
            '"' | '\'' | '`' => {
                let q = c;
                while let Some(n) = chars.next() {
                    if n == '\\' {
                        chars.next();
                    } else if n == q {
                        break;
                    } else if n == '\n' {
                        out.push('\n');
                    }
                }
            }
            _ => out.push(c),
        }
    }
    out
}

fn source_has_esm_markers(text: &str) -> bool {

    let mut t = text;
    if t.starts_with("#!") {
        if let Some(nl) = t.find('\n') {
            t = &t[nl + 1..];
        }
    }

    let stripped = blank_comments_and_strings(t);
    let t = stripped.as_str();

    for line in t.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("import") {
            let rest = &trimmed[6..];
            if rest.is_empty() {
                return true;
            }
            let c = rest.chars().next().unwrap();
            if c.is_whitespace() || c == '{' || c == '*' || c == '"' || c == '\'' || c == '(' {
                if c != '(' {
                    return true;
                }
            }
        }
        if trimmed.starts_with("export") {
            let rest = &trimmed[6..];
            if rest.is_empty() {
                return true;
            }
            let c = rest.chars().next().unwrap();
            if c.is_whitespace() || c == '{' || c == '*' || c == '"' || c == '\'' {
                return true;
            }
        }
    }

    for pat in [
        ";export{",
        ";export ",
        ";export*",
        "}export{",
        "}export ",
        "}export*",
        ";import{",
        ";import ",
        ";import*",
        ";import\"",
        ";import'",
        "}import{",
        "}import ",
        "}import*",
        "}import\"",
        "}import'",
    ] {
        if t.contains(pat) {
            return true;
        }
    }
    false
}

fn source_has_cjs_export_markers(text: &str) -> bool {
    let code = strip_js_comments_and_strings_for_marker_scan(text);
    code.contains("Object.defineProperty(exports,")
        || code.contains("module.exports")
        || code.contains("exports.")
        || code.contains("exports[")
}

#[allow(dead_code)]
fn source_has_free_require_call(text: &str) -> bool {
    let code = strip_js_comments_and_strings_for_marker_scan(text);
    let bytes = code.as_bytes();
    let needle = b"require";
    let mut i = 0;
    while let Some(pos) = code[i..].find("require") {
        let start = i + pos;
        let end = start + needle.len();
        let before = start.checked_sub(1).and_then(|idx| bytes.get(idx)).copied();
        let after = bytes.get(end).copied();
        let ident_before = before.is_some_and(is_js_ident_byte);
        let member_before = matches!(before, Some(b'.'));
        let ident_after = after.is_some_and(is_js_ident_byte);
        let mut j = end;
        while matches!(bytes.get(j), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            j += 1;
        }
        if !ident_before && !member_before && !ident_after && matches!(bytes.get(j), Some(b'(')) {
            return true;
        }
        i = end;
    }
    false
}

fn is_js_ident_byte(byte: u8) -> bool {
    byte == b'$' || byte == b'_' || byte.is_ascii_alphanumeric()
}

pub(crate) fn v8_parse_error_projection(
    source: &str,
    span_start: usize,
    eof_boundary: usize,
    internal: &str,
) -> Option<String> {
    let is_internal_shape = internal.starts_with("expected ")
        || internal.starts_with("unexpected token")
        || internal.starts_with("lex error");
    if !is_internal_shape {
        return None;
    }
    if internal.starts_with("lex error") {
        return Some("Invalid or unexpected token".to_string());
    }

    let mut i = span_start.min(source.len());
    let bytes = source.as_bytes();
    while i < source.len() && (bytes[i] as char).is_whitespace() {
        i += 1;
    }
    if i >= source.len() || i >= eof_boundary {
        return Some("Unexpected end of input".to_string());
    }
    let rest = &source[i..];
    let c = rest.chars().next().unwrap();

    if c == '\'' || c == '"' {
        return Some("Unexpected string".to_string());
    }
    if c == '`' {
        return Some("Unexpected template string".to_string());
    }

    if c.is_ascii_digit()
        || (c == '.' && rest.as_bytes().get(1).is_some_and(|b| b.is_ascii_digit()))
    {
        return Some("Unexpected number".to_string());
    }

    if c == '#' {
        let name: String = rest[1..]
            .chars()
            .take_while(|&ch| ch == '$' || ch == '_' || ch.is_alphanumeric())
            .collect();
        return Some(if name.is_empty() {
            "Invalid or unexpected token".to_string()
        } else {
            format!("Unexpected identifier '#{}'", name)
        });
    }

    if c == '$' || c == '_' || c.is_alphabetic() {
        let word: String = rest
            .chars()
            .take_while(|&ch| ch == '$' || ch == '_' || ch.is_alphanumeric())
            .collect();
        const KEYWORD_TOKENS: [&str; 36] = [
            "break",
            "case",
            "catch",
            "class",
            "const",
            "continue",
            "debugger",
            "default",
            "delete",
            "do",
            "else",
            "export",
            "extends",
            "false",
            "finally",
            "for",
            "function",
            "if",
            "import",
            "in",
            "instanceof",
            "new",
            "null",
            "return",
            "super",
            "switch",
            "this",
            "throw",
            "true",
            "try",
            "typeof",
            "var",
            "void",
            "while",
            "with",
            "async",
        ];
        return Some(if KEYWORD_TOKENS.contains(&word.as_str()) {
            format!("Unexpected token '{}'", word)
        } else if word == "enum" || word == "await" {
            "Unexpected reserved word".to_string()
        } else {
            format!("Unexpected identifier '{}'", word)
        });
    }

    const PUNCTS: [&str; 50] = [
        ">>>=", "...", "===", "!==", "**=", "<<=", ">>=", ">>>", "&&=", "||=", "??=", "=>", "==",
        "!=", "<=", ">=", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "&&", "||", "??", "?.",
        "++", "--", "**", "<<", ">>", "{", "}", "(", ")", "[", "]", ";", ",", "<", ">", "+", "-",
        "*", "/", "%", "&", "|",
    ];
    for p in PUNCTS {
        if rest.starts_with(p) {
            return Some(format!("Unexpected token '{}'", p));
        }
    }

    for p in ["^", "!", "~", "?", ":", "=", "."] {
        if rest.starts_with(p) {
            return Some(format!("Unexpected token '{}'", p));
        }
    }
    Some("Invalid or unexpected token".to_string())
}

pub(crate) fn format_public_parse_error(
    source: &str,
    e: &rusty_js_parser::ParseError,
    url: &str,
    wrapper_tag: &str,
    eof_boundary: usize,
) -> String {
    match v8_parse_error_projection(source, e.span.start, eof_boundary, &e.message) {
        Some(v8) => format!(
            "{} [in parse{}: {} @byte{}] @url={}",
            v8,
            wrapper_tag,
            e.message.replace(']', ")"),
            e.span.start,
            url
        ),

        None if e.message.starts_with("Unexpected ")
            || e.message.starts_with("Invalid or unexpected") =>
        {
            format!(
                "{} [in parse{} @byte{}] @url={}",
                e.message, wrapper_tag, e.span.start, url
            )
        }
        None => format!(
            "parse{}: {} @byte{} @url={}",
            wrapper_tag, e.message, e.span.start, url
        ),
    }
}

fn strip_js_comments_and_strings_for_marker_scan(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut string_quote: Option<char> = None;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
                out.push('\n');
            } else {
                out.push(' ');
            }
            continue;
        }
        if in_block_comment {
            if ch == '*' && matches!(chars.peek(), Some('/')) {
                chars.next();
                in_block_comment = false;
                out.push(' ');
                out.push(' ');
            } else if ch == '\n' {
                out.push('\n');
            } else {
                out.push(' ');
            }
            continue;
        }
        if let Some(quote) = string_quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote {
                string_quote = None;
            }
            out.push(if ch == '\n' { '\n' } else { ' ' });
            continue;
        }
        if ch == '/' {
            match chars.peek() {
                Some('/') => {
                    chars.next();
                    in_line_comment = true;
                    out.push(' ');
                    out.push(' ');
                    continue;
                }
                Some('*') => {
                    chars.next();
                    in_block_comment = true;
                    out.push(' ');
                    out.push(' ');
                    continue;
                }
                _ => {}
            }
        }
        if matches!(ch, '"' | '\'' | '`') {
            string_quote = Some(ch);
            escaped = false;
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out
}

fn scan_package_type(text: &str) -> Option<String> {

    let mut cursor = 0usize;
    while let Some(rel) = text[cursor..].find("\"type\"") {
        let key_pos = cursor + rel;
        let after = &text[key_pos + 6..];
        let Some(colon) = after.find(':') else {
            return None;
        };
        let after = &after[colon + 1..];
        let after = after.trim_start();
        if let Some(rest) = after.strip_prefix('"') {
            if let Some(end) = rest.find('"') {
                let v = &rest[..end];
                if v == "module" || v == "commonjs" {
                    return Some(v.to_string());
                }
            }
        }
        cursor = key_pos + 6;
    }
    None
}

pub enum HostHook {

    FinalizeModuleNamespace(
        Box<dyn Fn(&mut Runtime, &AstModule, ObjectRef, &str) -> Result<(), RuntimeError>>,
    ),

    PollIo(Box<dyn Fn(&mut Runtime) -> Result<bool, RuntimeError>>),

    ResolveBuiltinModule(
        Box<dyn Fn(&mut Runtime, &str) -> Result<Option<ObjectRef>, RuntimeError>>,
    ),

    LoadCruftScriptModule(
        Box<dyn Fn(&mut Runtime, &str, &str) -> Result<Option<ObjectRef>, RuntimeError>>,
    ),
}

#[derive(Default)]
pub struct HostHooks {
    pub finalize_namespace:
        Option<Box<dyn Fn(&mut Runtime, &AstModule, ObjectRef, &str) -> Result<(), RuntimeError>>>,
    pub poll_io: Option<Box<dyn Fn(&mut Runtime) -> Result<bool, RuntimeError>>>,
    pub resolve_builtin:
        Option<Box<dyn Fn(&mut Runtime, &str) -> Result<Option<ObjectRef>, RuntimeError>>>,
    pub load_cruftscript:
        Option<Box<dyn Fn(&mut Runtime, &str, &str) -> Result<Option<ObjectRef>, RuntimeError>>>,
}

impl Runtime {
    fn drain_module_microtasks(&mut self) -> Result<(), RuntimeError> {
        self.drain_module_microtasks_filtered(true)
    }

    fn drain_outermost_module_microtasks(&mut self) -> Result<(), RuntimeError> {
        while let Some(job) = self.job_queue.microtasks.pop_front() {
            crate::job_queue::check_microtask_budget(self, job.label)?;
            crate::job_queue::run_job_static(self, job)?;
            self.maybe_collect_between_jobs();
        }
        self.drain_module_microtasks_filtered(true)
    }

    fn drain_module_microtasks_filtered(
        &mut self,
        include_dynamic_import: bool,
    ) -> Result<(), RuntimeError> {
        let mut deferred = std::collections::VecDeque::new();
        loop {

            while let Some(job) = self.job_queue.nexttick.pop_front() {
                crate::job_queue::run_job_static(self, job)?;
                self.maybe_collect_between_jobs();
            }
            while let Some(job) = self.job_queue.microtasks.pop_front() {
                if !include_dynamic_import && job.label == "DynamicImportEvaluateJob" {
                    deferred.push_back(job);
                    continue;
                }
                crate::job_queue::check_microtask_budget(self, job.label)?;
                crate::job_queue::run_job_static(self, job)?;

                self.maybe_collect_between_jobs();
            }

            if self.job_queue.nexttick.is_empty() {
                break;
            }
        }
        while let Some(job) = deferred.pop_back() {
            self.job_queue.microtasks.push_front(job);
        }
        Ok(())
    }

    pub(crate) fn register_dynamic_import_waiter(
        &mut self,
        url: &str,
        promise: ObjectRef,
        namespace: ObjectRef,
    ) {
        let waiters = self
            .pending_dynamic_imports
            .entry(url.to_string())
            .or_insert_with(Vec::new);
        if let Some(active) = self.active_dynamic_import_loaders.remove(url) {
            for active_promise in active {
                if active_promise != promise
                    && !waiters.iter().any(|(queued, _)| *queued == active_promise)
                {
                    waiters.push((active_promise, namespace));
                }
            }
        }
        if !waiters.iter().any(|(queued, _)| *queued == promise) {
            waiters.push((promise, namespace));
        }
    }

    pub(crate) fn has_pending_dynamic_import_waiters(&self, url: &str) -> bool {
        self.pending_dynamic_imports
            .get(url)
            .is_some_and(|waiters| !waiters.is_empty())
    }

    pub(crate) fn push_active_dynamic_import_loader(&mut self, url: &str, promise: ObjectRef) {
        self.active_dynamic_import_loaders
            .entry(url.to_string())
            .or_insert_with(Vec::new)
            .push(promise);
    }

    pub(crate) fn remove_active_dynamic_import_loader(&mut self, url: &str, promise: ObjectRef) {
        if let Some(loaders) = self.active_dynamic_import_loaders.get_mut(url) {
            loaders.retain(|queued| *queued != promise);
            if loaders.is_empty() {
                self.active_dynamic_import_loaders.remove(url);
            }
        }
    }

    fn resolve_dynamic_import_waiters(&mut self, url: &str, namespace: ObjectRef) {
        if let Some(waiters) = self.pending_dynamic_imports.remove(url) {
            for (promise, waiter_ns) in waiters {
                let ns = if waiter_ns == namespace {
                    namespace
                } else {
                    waiter_ns
                };
                let owner = self.heap.owner(promise).unwrap_or(self.current_realm);
                match crate::intrinsics::realm_boundary_value_for_realm(
                    self,
                    owner,
                    &Value::Object(ns),
                ) {
                    Ok(v) => crate::promise::resolve_promise(self, promise, v),
                    Err(RuntimeError::Thrown(thrown)) => {
                        crate::promise::reject_promise(self, promise, thrown);
                    }
                    Err(e) => crate::intrinsics::reject_clone_error_promise(self, promise, e),
                }
            }
        }
    }

    fn module_async_deps_ready(&self, deps: &[String]) -> bool {
        deps.iter().all(|dep| {
            matches!(
                self.module_get(dep).map(|rec| rec.borrow().status),
                Some(ModuleStatus::Evaluated)
            )
        })
    }

    fn ensure_module_async_evaluation_order(&mut self, rec: &Rc<RefCell<ModuleRecord>>) {
        if rec.borrow().async_evaluation_order.is_some() {
            return;
        }
        let order = self.next_module_async_evaluation_order;
        self.next_module_async_evaluation_order =
            self.next_module_async_evaluation_order.saturating_add(1);
        rec.borrow_mut().async_evaluation_order = Some(order);
    }

    fn settle_body_completed_async_modules(&mut self) {
        loop {
            let mut records: Vec<Rc<RefCell<ModuleRecord>>> = if self.current_realm == 0 {
                self.modules.values().cloned().collect()
            } else {
                self.realm_module_registries
                    .get(&self.current_realm)
                    .map(|registry| registry.values().cloned().collect())
                    .unwrap_or_default()
            };
            let mut progressed = false;
            records.sort_by_key(|rec| rec.borrow().async_evaluation_order.unwrap_or(u64::MAX));
            for rec in records {
                let ready = {
                    let r = rec.borrow();
                    matches!(
                        r.status,
                        ModuleStatus::Evaluating | ModuleStatus::EvaluatingAsync
                    ) && r.body_completed_waiting_async_deps
                        && self.module_async_deps_ready(&r.async_static_deps)
                };
                if !ready {
                    continue;
                }
                let (url, namespace) = {
                    let mut r = rec.borrow_mut();
                    r.status = ModuleStatus::Evaluated;
                    r.body_completed_waiting_async_deps = false;
                    (r.url.clone(), r.namespace)
                };
                if let Some(namespace) = namespace {
                    if let Some(deferred) = self.pending_live_bindings.remove(&url) {
                        for d in deferred {
                            let v = self
                                .resolve_import_binding_value(namespace, &d.kind, d.is_cjs, &url);
                            *d.cell.borrow_mut() = v;
                        }
                    }
                    self.resolve_dynamic_import_waiters(&url, namespace);
                    progressed = true;
                }
            }
            if !progressed {
                break;
            }
        }
        self.enqueue_ready_pending_module_bodies();
    }

    fn enqueue_ready_pending_module_bodies(&mut self) {
        loop {
            let mut records: Vec<Rc<RefCell<ModuleRecord>>> = if self.current_realm == 0 {
                self.modules.values().cloned().collect()
            } else {
                self.realm_module_registries
                    .get(&self.current_realm)
                    .map(|registry| registry.values().cloned().collect())
                    .unwrap_or_default()
            };
            let mut progressed = false;
            records.sort_by_key(|rec| rec.borrow().async_evaluation_order.unwrap_or(u64::MAX));
            for rec in records {
                let ready = {
                    let r = rec.borrow();
                    r.status == ModuleStatus::Evaluating
                        && r.pending_body_start.is_some()
                        && self.module_async_deps_ready(&r.async_static_deps)
                };
                if !ready {
                    continue;
                }
                let Some((url, namespace, snapshot)) = ({
                    let mut r = rec.borrow_mut();
                    match (r.namespace, r.pending_body_start.take()) {
                        (Some(namespace), Some(snapshot)) => {
                            Some((r.url.clone(), namespace, snapshot))
                        }
                        _ => None,
                    }
                }) else {
                    continue;
                };
                self.enqueue_module_body_start(url, namespace, rec.clone(), snapshot);
                progressed = true;
            }
            if !progressed {
                break;
            }
        }
    }

    pub(crate) fn reject_dynamic_import_waiters(&mut self, url: &str, reason: Value) -> usize {
        let mut delivered = 0;
        if let Some(waiters) = self.pending_dynamic_imports.remove(url) {
            for (promise, _) in waiters {
                crate::promise::reject_promise(self, promise, reason.clone());
                delivered += 1;
            }
        }
        delivered
    }

    fn propagate_async_module_rejection(&mut self, failed_url: &str, reason: Value) -> usize {
        let records: Vec<Rc<RefCell<ModuleRecord>>> = if self.current_realm == 0 {
            self.modules.values().cloned().collect()
        } else {
            self.realm_module_registries
                .get(&self.current_realm)
                .map(|registry| registry.values().cloned().collect())
                .unwrap_or_default()
        };
        let mut delivered = 0;
        for rec in records {
            let should_reject = {
                let r = rec.borrow();
                r.status == ModuleStatus::Evaluating
                    && (r.body_completed_waiting_async_deps || r.pending_body_start.is_some())
                    && r.async_static_deps.iter().any(|dep| dep == failed_url)
            };
            if !should_reject {
                continue;
            }
            let url = {
                let mut r = rec.borrow_mut();
                r.status = ModuleStatus::Failed;
                r.eval_error = Some(RuntimeError::Thrown(reason.clone()));
                r.body_completed_waiting_async_deps = false;
                r.pending_body_start = None;
                r.url.clone()
            };
            delivered += self.reject_dynamic_import_waiters(&url, reason.clone());
            delivered += self.propagate_async_module_rejection(&url, reason.clone());
        }
        delivered
    }

    fn mark_async_cycle_comembers_failed(
        &mut self,
        failed_url: &str,
        error: &RuntimeError,
        reason_val: &Value,
    ) -> usize {
        let root = self
            .module_get(failed_url)
            .and_then(|r| r.borrow().async_cycle_root.clone())
            .unwrap_or_else(|| failed_url.to_string());
        let records: Vec<Rc<RefCell<ModuleRecord>>> = if self.current_realm == 0 {
            self.modules.values().cloned().collect()
        } else {
            self.realm_module_registries
                .get(&self.current_realm)
                .map(|registry| registry.values().cloned().collect())
                .unwrap_or_default()
        };
        let mut delivered = 0;
        for rec in records {
            let (m_url, is_comember) = {
                let r = rec.borrow();
                let cr = r.async_cycle_root.clone().unwrap_or_else(|| r.url.clone());
                (
                    r.url.clone(),
                    cr == root && r.url != failed_url && r.status != ModuleStatus::Failed,
                )
            };
            if is_comember {
                {
                    let mut r = rec.borrow_mut();
                    r.status = ModuleStatus::Failed;
                    r.eval_error = Some(error.clone());
                }
                delivered += self.reject_dynamic_import_waiters(&m_url, reason_val.clone());
            }
        }
        delivered
    }

    fn enqueue_module_await_resume(
        &mut self,
        url: String,
        namespace: ObjectRef,
        record: Rc<RefCell<ModuleRecord>>,
        snapshot: FrameSnapshot,
        awaited_value: Value,
    ) {
        self.ensure_module_async_evaluation_order(&record);

        if matches!(record.borrow().status, ModuleStatus::Evaluating) {
            record.borrow_mut().status = ModuleStatus::EvaluatingAsync;
        }
        let pending_promise = if let Value::Object(id) = awaited_value.clone() {
            let state = {
                let obj = self.obj(id);
                match &obj.internal_kind {
                    InternalKind::Promise(ps) => Some((ps.status, ps.value.clone())),
                    _ => None,
                }
            };
            match state {
                Some((PromiseStatus::Pending, _)) => Some(id),
                Some((PromiseStatus::Fulfilled, value)) => {
                    self.enqueue_module_await_resume(url, namespace, record, snapshot, value);
                    return;
                }
                Some((PromiseStatus::Rejected, reason)) => {

                    self.pending_unhandled.remove(&id);

                    let roots = module_job_roots(namespace, &snapshot, &[reason.clone()]);
                    let als_ctx = snapshot.als_context.clone();
                    self.enqueue_microtask_rooted_with_async_context(
                        "ModuleAwaitRejectJob",
                        roots,
                        als_ctx,
                        move |rt| {
                            let _ = rt.resume_suspended_module(
                                url,
                                namespace,
                                record,
                                snapshot,
                                ModuleResume::Throw(reason),
                            );
                            Ok(())
                        },
                    );
                    return;
                }
                None => None,
            }
        } else {
            None
        };

        if let Some(promise_id) = pending_promise {

            let snapshot_roots = module_job_roots(namespace, &snapshot, &[]);
            let fulfill_url = url.clone();
            let fulfill_record = record.clone();
            let fulfill_snapshot = snapshot.clone();
            let fulfill_fn = crate::intrinsics::make_native_with_length("", 1, move |rt, args| {
                let value = args.first().cloned().unwrap_or(Value::Undefined);
                rt.enqueue_module_await_resume(
                    fulfill_url.clone(),
                    namespace,
                    fulfill_record.clone(),
                    fulfill_snapshot.clone(),
                    value,
                );
                Ok(Value::Undefined)
            });

            let reject_url = url;
            let reject_record = record;
            let reject_snapshot = snapshot.clone();
            let reject_fn = crate::intrinsics::make_native_with_length("", 1, move |rt, args| {
                let reason = args.first().cloned().unwrap_or(Value::Undefined);

                let saved_als = rt.als_context_replace(reject_snapshot.als_context.clone());
                let _ = rt.resume_suspended_module(
                    reject_url.clone(),
                    namespace,
                    reject_record.clone(),
                    reject_snapshot.clone(),
                    ModuleResume::Throw(reason),
                );
                let _ = rt.als_context_replace(saved_als);
                Ok(Value::Undefined)
            });

            let fulfill_id = self.alloc_object(fulfill_fn);
            let reject_id = self.alloc_object(reject_fn);
            for id in [fulfill_id, reject_id] {
                if let InternalKind::Function(fi) = &mut self.obj_mut(id).internal_kind {
                    fi.roots = snapshot_roots.clone();
                }
            }
            let _reaction_roots = self.push_temporary_value_roots(&[
                Value::Object(fulfill_id),
                Value::Object(reject_id),
                Value::Object(promise_id),
            ]);
            let chain = crate::promise::new_promise(self);
            let promise = self.obj_mut(promise_id);
            if let InternalKind::Promise(ps) = &mut promise.internal_kind {
                ps.fulfill_reactions.push(PromiseReaction {
                    handler: Some(crate::value::PromiseReactionHandler::Callable(
                        Value::Object(fulfill_id),
                    )),
                    chain,
                    cap_resolve: None,
                    cap_reject: None,
                });
                ps.reject_reactions.push(PromiseReaction {
                    handler: Some(crate::value::PromiseReactionHandler::Callable(
                        Value::Object(reject_id),
                    )),
                    chain,
                    cap_resolve: None,
                    cap_reject: None,
                });
            }
            return;
        }

        let roots = module_job_roots(namespace, &snapshot, &[awaited_value.clone()]);
        let als_ctx = snapshot.als_context.clone();
        self.enqueue_microtask_rooted_with_async_context(
            "ModuleAwaitResumeJob",
            roots,
            als_ctx,
            move |rt| {

                rt.async_module_failure_delivered = false;
                let result = rt.resume_suspended_module(
                    url,
                    namespace,
                    record,
                    snapshot,
                    ModuleResume::Value(awaited_value),
                );
                if rt.async_module_failure_delivered {
                    rt.async_module_failure_delivered = false;
                    Ok(())
                } else {
                    result
                }
            },
        );
    }

    fn enqueue_module_body_start(
        &mut self,
        url: String,
        namespace: ObjectRef,
        record: Rc<RefCell<ModuleRecord>>,
        snapshot: FrameSnapshot,
    ) {
        let roots = module_job_roots(namespace, &snapshot, &[]);
        self.enqueue_microtask_rooted("ModuleBodyStartJob", roots, move |rt| {
            rt.resume_suspended_module(url, namespace, record, snapshot, ModuleResume::Start)
        });
    }

    fn resume_suspended_module(
        &mut self,
        url: String,
        namespace: ObjectRef,
        record: Rc<RefCell<ModuleRecord>>,
        snapshot: FrameSnapshot,
        resume: ModuleResume,
    ) -> Result<(), RuntimeError> {
        let mut frame = Frame::from(&snapshot);
        match resume {
            ModuleResume::Start => {}
            ModuleResume::Value(value) => frame.push(value),
            ModuleResume::Throw(reason) => {

                if let Err(e) = inject_throw_into_frame(&mut frame, reason) {
                    let e = self.esm_scope_reference_error_projection(e, &url);
                    self.module_post_eval_trace
                        .insert(url.clone(), format!("kind=ESM threw: {:?}", e));
                    {
                        let mut r = record.borrow_mut();
                        r.status = ModuleStatus::Failed;
                        r.eval_error = Some(e.clone());
                    }
                    let rejection = runtime_error_to_rejection_value(self, &e);
                    let delivered = self.reject_dynamic_import_waiters(&url, rejection.clone())
                        + self.propagate_async_module_rejection(&url, rejection.clone())
                        + self.mark_async_cycle_comembers_failed(&url, &e, &rejection);
                    self.async_module_failure_delivered = delivered > 0;
                    return Err(e);
                }
            }
        }
        self.current_module_url.push(url.clone());
        let run_result = self.with_global_bindings_suspended(
            &["exports", "require", "module", "__filename", "__dirname"],
            |rt| rt.with_direct_eval_global_shadows_suspended(|rt| rt.run_frame_module(&mut frame)),
        );
        self.current_module_url.pop();
        match run_result {
            Ok(_) => {
                let bytecode = record.borrow().bytecode.clone();
                let mut locals = frame.locals.clone();
                for (i, slot) in locals.iter_mut().enumerate() {
                    if let Some(Some(cell)) = frame.local_cells.get(i) {
                        *slot = cell.borrow().clone();
                    }
                }
                for eb in &bytecode.exports {
                    if let ExportBinding::Local { exported, local } = eb {
                        let v = locals
                            .get(*local as usize)
                            .cloned()
                            .unwrap_or(Value::Undefined);
                        self.obj_mut(namespace)
                            .set_own_module_export(exported.clone(), v);
                    }
                }

                let mut reexport_cells: Vec<(String, crate::value::UpvalueCell)> = Vec::new();
                for eb in &bytecode.exports {
                    match eb {
                        ExportBinding::Local { .. } => {}
                        ExportBinding::Named {
                            exported,
                            source_specifier,
                            imported,
                        } => {
                            let Ok(resolved) =
                                self.resolve_module_full(&url, source_specifier, ModuleKind::ESM)
                            else {
                                continue;
                            };
                            let v = if let Some(cell) =
                                self.module_resolve_export_cell(&resolved, imported)
                            {
                                let v = cell.borrow().clone();
                                reexport_cells.push((exported.clone(), cell));
                                v
                            } else if let Some(src_ns) = self
                                .module_get(&resolved)
                                .and_then(|r| r.borrow().namespace)
                            {
                                self.object_get(src_ns, imported)
                            } else {
                                continue;
                            };
                            self.obj_mut(namespace)
                                .set_own_module_export(exported.clone(), v);
                        }
                        ExportBinding::Star { source_specifier } => {
                            let Ok(resolved) =
                                self.resolve_module_full(&url, source_specifier, ModuleKind::ESM)
                            else {
                                continue;
                            };
                            let mut seen = std::collections::HashSet::new();
                            let mut keys = std::collections::HashSet::new();
                            let _ = self
                                .collect_module_export_names(&url, &resolved, &mut seen, &mut keys);
                            let src_ns = self
                                .module_get(&resolved)
                                .and_then(|r| r.borrow().namespace);
                            for key in keys {
                                if key == "default" {
                                    continue;
                                }
                                let mut rset = std::collections::HashSet::new();
                                if !matches!(
                                    self.module_resolve_export(&url, &key, &mut rset),
                                    ResolveExportResult::Resolved { .. }
                                        | ResolveExportResult::Incomplete
                                ) {
                                    continue;
                                }
                                let v = if let Some(cell) =
                                    self.module_resolve_export_cell(&url, &key)
                                {
                                    let v = cell.borrow().clone();
                                    reexport_cells.push((key.clone(), cell));
                                    v
                                } else if let Some(src_ns) = src_ns {
                                    self.object_get(src_ns, &key)
                                } else {
                                    continue;
                                };
                                self.obj_mut(namespace).set_own_module_export(key, v);
                            }
                        }
                        ExportBinding::StarAs {
                            exported,
                            source_specifier,
                        } => {
                            let Ok(resolved) =
                                self.resolve_module_full(&url, source_specifier, ModuleKind::ESM)
                            else {
                                continue;
                            };
                            if let Some(src_ns) = self
                                .module_get(&resolved)
                                .and_then(|r| r.borrow().namespace)
                            {
                                self.obj_mut(namespace)
                                    .set_own_module_export(exported.clone(), Value::Object(src_ns));
                            }
                        }
                    }
                }
                self.finalize_module_namespace_exotic(namespace);
                {
                    let mut r = record.borrow_mut();
                    r.body_completed_waiting_async_deps =
                        !self.module_async_deps_ready(&r.async_static_deps);
                    if !r.body_completed_waiting_async_deps {
                        r.status = ModuleStatus::Evaluated;
                    }
                    let mut export_cells = std::collections::HashMap::new();
                    for eb in &bytecode.exports {
                        if let ExportBinding::Local { exported, local } = eb {
                            let cell = frame.promote_local(*local as usize);
                            export_cells.insert(exported.clone(), cell);
                        }
                    }
                    for (exported, cell) in reexport_cells {
                        export_cells.entry(exported).or_insert(cell);
                    }
                    r.export_cells = export_cells;
                }
                let key_count = self.obj(namespace).properties.len();
                self.module_post_eval_trace.insert(
                    url.clone(),
                    format!("kind=ESM key_count={} status=Evaluated", key_count),
                );
                if !record.borrow().body_completed_waiting_async_deps {
                    if let Some(deferred) = self.pending_live_bindings.remove(&url) {
                        for d in deferred {
                            let v = self
                                .resolve_import_binding_value(namespace, &d.kind, d.is_cjs, &url);
                            *d.cell.borrow_mut() = v;
                        }
                    }
                    self.resolve_dynamic_import_waiters(&url, namespace);
                    self.settle_body_completed_async_modules();
                    self.enqueue_ready_pending_module_bodies();
                }
                Ok(())
            }
            Err(RuntimeError::ModuleAwaitSuspended(next_snapshot, next_value)) => {
                self.enqueue_module_await_resume(url, namespace, record, next_snapshot, next_value);
                Ok(())
            }
            Err(e) => {
                let e = self.esm_scope_reference_error_projection(e, &url);
                self.module_post_eval_trace
                    .insert(url.clone(), format!("kind=ESM threw: {:?}", e));
                let mut r = record.borrow_mut();
                r.status = ModuleStatus::Failed;
                r.eval_error = Some(e.clone());
                drop(r);
                let rejection = runtime_error_to_rejection_value(self, &e);
                let delivered = self.reject_dynamic_import_waiters(&url, rejection.clone())
                    + self.propagate_async_module_rejection(&url, rejection.clone())
                    + self.mark_async_cycle_comembers_failed(&url, &e, &rejection);
                self.async_module_failure_delivered = delivered > 0;
                Err(e)
            }
        }
    }

    pub fn make_abstract_module_source_object(
        &mut self,
        specifier: &str,
        resolved_url: &str,
    ) -> ObjectRef {
        let proto = self
            .global_object
            .and_then(|gt| match self.object_get(gt, "$262") {
                Value::Object(t262) => match self.object_get(t262, "AbstractModuleSource") {
                    Value::Object(ctor) => match self.object_get(ctor, "prototype") {
                        Value::Object(proto) => Some(proto),
                        _ => None,
                    },
                    _ => None,
                },
                _ => None,
            });

        let mut intermediate = Object::new_ordinary();
        intermediate.proto = proto;
        let intermediate_id = self.alloc_object(intermediate);
        let mut obj = Object::new_ordinary();
        obj.proto = Some(intermediate_id);
        let id = self.alloc_object(obj);
        self.object_set(
            id,
            "__source_phase_specifier".into(),
            Value::String(Rc::new(crate::value::JsString::from(specifier.to_string()))),
        );
        self.object_set(
            id,
            "__source_phase_url".into(),
            Value::String(Rc::new(crate::value::JsString::from(
                resolved_url.to_string(),
            ))),
        );
        self.object_set(
            id,
            "__source_phase_class".into(),
            Value::String(Rc::new(crate::value::JsString::from(
                "AbstractModuleSource",
            ))),
        );
        id
    }

    pub fn install_host_hook(&mut self, hook: HostHook) {
        match hook {
            HostHook::FinalizeModuleNamespace(f) => {
                self.host_hooks.finalize_namespace = Some(f);
            }
            HostHook::PollIo(f) => {
                self.host_hooks.poll_io = Some(f);
            }
            HostHook::ResolveBuiltinModule(f) => {
                self.host_hooks.resolve_builtin = Some(f);
            }
            HostHook::LoadCruftScriptModule(f) => {
                self.host_hooks.load_cruftscript = Some(f);
            }
        }
    }

    pub fn resolve_module(parent_url: &str, specifier: &str) -> Result<String, RuntimeError> {

        if specifier.starts_with("node:")
            || specifier.starts_with("cruft:")
            || specifier.starts_with("bun:") || specifier.starts_with("deno:")
        {
            return Ok(specifier.to_string());
        }
        if specifier == "."
            || specifier == ".."
            || specifier.starts_with("./")
            || specifier.starts_with("../")
        {
            let parent_path = parent_url.strip_prefix("file://").ok_or_else(|| {
                RuntimeError::TypeError(format!(
                    "relative specifier '{}' requires a file:// parent URL (got '{}')",
                    specifier, parent_url
                ))
            })?;
            let parent = std::path::Path::new(parent_path);
            let parent_dir = parent.parent().unwrap_or_else(|| std::path::Path::new("/"));
            let candidate = parent_dir.join(specifier);
            return probe_with_extensions(&candidate, specifier);
        }
        if let Some(candidate) = file_url_specifier_path(specifier) {
            return probe_with_extensions(&candidate, specifier);
        }

        if specifier.starts_with('/') {
            let candidate = std::path::PathBuf::from(specifier);
            return probe_with_extensions(&candidate, specifier);
        }
        Err(RuntimeError::TypeError(format!(
            "bare specifier '{}' requires resolve_module_full (caller did not thread the Runtime)",
            specifier
        )))
    }

    fn lockfile_export_keys(&mut self, url: &str) -> Option<Vec<String>> {
        let path = url.strip_prefix("file://").unwrap_or(url);
        if self.module_map_bridge.is_none() {
            self.module_map_bridge = Some(discover_and_load_bridge(std::path::Path::new(path)));
        }
        let bridge = self.module_map_bridge.as_ref()?;
        if bridge.is_empty() {
            return None;
        }
        if let Some(shape) = bridge.export_shape_for_path(path) {
            return Some(shape.lower_node_keys());
        }

        if bridge.package_for_path(path).is_some() {
            return Some(vec!["default".to_string(), "module.exports".to_string()]);
        }
        None
    }

    fn cjs_static_export_keys_for_url(&mut self, url: &str) -> Option<Vec<String>> {
        if let Some(keys) = self.cjs_static_export_keys.get(url).cloned() {
            return Some(keys);
        }
        let keys = self
            .lockfile_export_keys(url)
            .or_else(|| cjs_static_export_keys_from_resolved_source(url))?;
        self.cjs_static_export_keys
            .insert(url.to_string(), keys.clone());
        Some(keys)
    }

    fn resolve_export_value(&mut self, oid: ObjectRef, k: &str) -> Option<Value> {
        if let Some(getter) = self.find_getter(oid, k) {
            return Some(
                self.call_function(getter, Value::Object(oid), Vec::new())
                    .unwrap_or(Value::Undefined),
            );
        }
        match self.object_get(oid, k) {
            Value::Undefined => None,
            v => Some(v),
        }
    }

    fn resolve_cjs_named_export_value(&mut self, url: &str, raw: &Value, k: &str) -> Option<Value> {

        if k == "default" {
            return Some(raw.clone());
        }
        let final_oid = match raw {
            Value::Object(oid) => Some(*oid),
            _ => None,
        };
        let _ = url;
        Some(
            final_oid
                .and_then(|o| self.resolve_export_value(o, k))
                .unwrap_or(Value::Undefined),
        )
    }

    fn module_map_lookup(&mut self, pkg_dir: &std::path::Path, subpath: &str) -> Option<String> {
        if self.module_map_bridge.is_none() {
            self.module_map_bridge = Some(discover_and_load_bridge(pkg_dir));
        }
        let bridge = self.module_map_bridge.as_ref()?;
        if bridge.is_empty() {
            return None;
        }

        let key = if subpath.is_empty() { "." } else { subpath };
        bridge.lookup(&pkg_dir.to_string_lossy(), key)
    }

    fn esm_scope_reference_error_projection(&mut self, e: RuntimeError, url: &str) -> RuntimeError {
        let RuntimeError::ReferenceError(msg) = &e else {
            return e;
        };
        let (head, trailer) = crate::intrinsics::split_diag_decorations(msg);
        let Some(name) = head.strip_suffix(" is not defined") else {
            return e;
        };
        if !matches!(
            name,
            "exports" | "require" | "module" | "__filename" | "__dirname"
        ) {
            return e;
        }
        let mut new_msg = format!("{} is not defined in ES module scope", name);
        if name == "require" {
            new_msg.push_str(", you can use import instead");
        }
        let path = url.strip_prefix("file://").unwrap_or(url);
        if path.ends_with(".js") {

            let mut dir = std::path::Path::new(path).parent();
            while let Some(d) = dir {
                let pkg_path = d.join("package.json");
                if pkg_path.is_file() {
                    let is_type_module = self
                        .read_package_json(&pkg_path)
                        .ok()
                        .and_then(|p| {
                            p.raw
                                .get("type")
                                .and_then(|t| t.as_str())
                                .map(|t| t == "module")
                        })
                        .unwrap_or(false);
                    if is_type_module {
                        new_msg.push_str(&format!(
                            "\nThis file is being treated as an ES module because it has a '.js' file extension and '{}' contains \"type\": \"module\". To treat it as a CommonJS script, rename it to use the '.cjs' file extension.",
                            pkg_path.display()
                        ));
                    }
                    break;
                }
                dir = d.parent();
            }
        }
        if !trailer.is_empty() {
            new_msg.push(' ');
            new_msg.push_str(trailer);
        }
        RuntimeError::ReferenceError(new_msg)
    }

    fn esm_node_package_relative_requires_exact_file(
        &mut self,
        parent_url: &str,
        specifier: &str,
    ) -> bool {
        if !(specifier == "." || specifier.starts_with("./")) {
            return false;
        }
        let parent_path = parent_url.strip_prefix("file://").unwrap_or(parent_url);
        if !parent_path.contains("/node_modules/") {
            return false;
        }
        let mut cur = Some(std::path::Path::new(parent_path));
        while let Some(path) = cur {
            let pkg_path = if path.is_dir() {
                path.join("package.json")
            } else {
                path.parent()
                    .unwrap_or_else(|| std::path::Path::new("/"))
                    .join("package.json")
            };
            if pkg_path.is_file() {
                let Ok(pkg) = self.read_package_json(&pkg_path) else {
                    return false;
                };
                let Some(pkg_dir) = pkg_path.parent() else {
                    return false;
                };
                let parent = std::path::Path::new(parent_path);
                let Some(entry) = resolve_within_package(pkg_dir, &pkg, "", ModuleKind::ESM) else {
                    cur = path.parent();
                    continue;
                };
                if parent == entry {
                    return true;
                }
                if entry.extension().is_none() && parent == with_suffix(&entry, ".js") {
                    return true;
                }
            }
            cur = path.parent();
        }
        false
    }

    pub fn resolve_module_full(
        &mut self,
        parent_url: &str,
        specifier: &str,
        importer_kind: ModuleKind,
    ) -> Result<String, RuntimeError> {

        if importer_kind == ModuleKind::ESM {
            self.check_import_closure(parent_url, specifier)?;
        }

        let specifier = if importer_kind == ModuleKind::ESM {
            strip_file_specifier_query_fragment(specifier)
        } else {
            specifier
        };
        let cache_key = if self.module_resolve_cache_enabled {
            Some((
                self.current_realm,
                importer_kind,
                parent_url.to_string(),
                specifier.to_string(),
            ))
        } else {
            None
        };
        if let Some(key) = cache_key.as_ref() {
            if let Some(resolved) = self.module_resolve_cache.get(key) {
                if phase_profile::enabled() {
                    phase_profile::inc(&phase_profile::RESOLVE_CACHE_HITS);
                }
                return Ok(resolved.clone());
            }
        }
        if phase_profile::enabled() {
            phase_profile::inc(&phase_profile::RESOLVE_CACHE_MISSES);
        }
        let resolved = self.resolve_module_full_uncached(parent_url, specifier, importer_kind)?;
        if let Some(key) = cache_key {
            self.module_resolve_cache.insert(key, resolved.clone());
        }
        Ok(resolved)
    }

    fn resolve_module_full_uncached(
        &mut self,
        parent_url: &str,
        specifier: &str,
        importer_kind: ModuleKind,
    ) -> Result<String, RuntimeError> {
        let _resolve_prof = phase_profile::enabled();
        let _resolve_t0 = if _resolve_prof {
            Some(std::time::Instant::now())
        } else {
            None
        };
        struct ResolveProfileGuard(Option<std::time::Instant>);
        impl Drop for ResolveProfileGuard {
            fn drop(&mut self) {
                if let Some(t) = self.0 {
                    phase_profile::inc(&phase_profile::RESOLVE_COUNT);
                    phase_profile::add(&phase_profile::RESOLVE_NS, t.elapsed().as_nanos() as u64);
                }
            }
        }
        let _resolve_profile_guard = ResolveProfileGuard(_resolve_t0);

        if specifier.starts_with("data:") {
            return Ok(specifier.to_string());
        }

        let userland_shadows_alias = |name: &str| -> bool {
            let parent_path = parent_url.strip_prefix("file://").unwrap_or(parent_url);
            let parent_path = std::path::Path::new(parent_path);
            let start_dir = if parent_path.is_dir() {
                parent_path
            } else {
                match parent_path.parent() {
                    Some(d) => d,
                    None => return false,
                }
            };
            walk_up_for_pkg(start_dir, name).is_some()
        };
        let aliased = match specifier {
            "readable-stream" if !userland_shadows_alias("readable-stream") => {
                Some("node:readable-stream")
            }
            "readable-stream/duplex"
            | "readable-stream/readable"
            | "readable-stream/writable"
            | "readable-stream/transform"
            | "readable-stream/passthrough"
                if !userland_shadows_alias("readable-stream") =>
            {
                Some("node:stream")
            }
            "safe-buffer" if !userland_shadows_alias("safe-buffer") => Some("node:buffer"),
            "buffer" if !specifier.starts_with("./") => Some("node:buffer"),
            "events" if !specifier.starts_with("./") => Some("node:events"),
            "util" if !specifier.starts_with("./") => Some("node:util"),
            _ => None,
        };
        if let Some(target) = aliased {
            return Runtime::resolve_module(parent_url, target);
        }

        if specifier == "."
            || specifier == ".."
            || specifier.starts_with("node:")
            || specifier.starts_with("cruft:") || specifier.starts_with("bun:") || specifier.starts_with("deno:")
            || specifier.starts_with("./")
            || specifier.starts_with("../")
            || specifier.starts_with("file:")
            || specifier.starts_with('/')
        {
            if matches!(importer_kind, ModuleKind::ESM)
                && self.esm_node_package_relative_requires_exact_file(parent_url, specifier)
            {
                return resolve_esm_node_package_relative_exact(parent_url, specifier);
            }

            let result = Runtime::resolve_module(parent_url, specifier);
            if matches!(importer_kind, ModuleKind::ESM)
                && !specifier.starts_with("node:")
                && !specifier.starts_with("cruft:")
                && !specifier.starts_with("bun:") && !specifier.starts_with("deno:")
            {
                if let Err(RuntimeError::TypeError(m)) = &result {
                    if m.starts_with("module not found:") {
                        if let Some(parent_path) = parent_url.strip_prefix("file://") {
                            let parent = std::path::Path::new(parent_path);
                            let parent_dir =
                                parent.parent().unwrap_or_else(|| std::path::Path::new("/"));
                            let candidate = if let Some(path) = file_url_specifier_path(specifier) {
                                path
                            } else if specifier.starts_with('/') {
                                std::path::PathBuf::from(specifier)
                            } else {
                                parent_dir.join(specifier)
                            };
                            let display = lexically_normalize_path(&candidate);
                            return Err(RuntimeError::TypeError(format!(
                                "__node_resolve_error__:Cannot find module '{}' imported from {}",
                                display.display(),
                                parent_path
                            )));
                        }
                    }
                }
            }
            return result;
        }

        if specifier.starts_with('#') {
            let parent_path_str = parent_url.strip_prefix("file://").unwrap_or(parent_url);
            let parent_path = std::path::Path::new(parent_path_str);
            let start_dir = parent_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("/"));
            let mut cur: Option<&std::path::Path> = Some(start_dir);
            while let Some(d) = cur {
                let candidate = d.join("package.json");
                if candidate.is_file() {
                    let pkg = self.read_package_json(&candidate)?;
                    if let Some(imports) = pkg.raw.get("imports") {
                        if imports.get(specifier).is_some() {

                            if let Some(resolved) = self.module_map_lookup(d, specifier) {
                                return probe_with_extensions(
                                    &std::path::PathBuf::from(resolved),
                                    specifier,
                                );
                            }
                        }
                        if let Some(target) = imports.get(specifier) {
                            if let Some(rel) = resolve_exports_target(target, "", importer_kind) {
                                let pkg_dir = d;
                                let candidate_path = pkg_dir.join(strip_dot_slash(&rel));
                                return probe_with_extensions(&candidate_path, specifier);
                            }
                        }

                        if let Some(map) = imports.as_object() {
                            let mut best: Option<(usize, usize, String, &serde_json::Value)> = None;
                            for (key, target) in map {
                                let Some(star) = key.find('*') else {
                                    continue;
                                };
                                if key[star + 1..].contains('*') {
                                    continue;
                                }
                                let prefix = &key[..star];
                                let suffix = &key[star + 1..];
                                if specifier.len() >= prefix.len() + suffix.len()
                                    && specifier.starts_with(prefix)
                                    && specifier.ends_with(suffix)
                                {
                                    let capture = specifier
                                        [prefix.len()..specifier.len() - suffix.len()]
                                        .to_string();
                                    let take = match &best {
                                        None => true,
                                        Some((bp, bs, _, _)) => {
                                            prefix.len() > *bp
                                                || (prefix.len() == *bp && suffix.len() > *bs)
                                        }
                                    };
                                    if take {
                                        best = Some((prefix.len(), suffix.len(), capture, target));
                                    }
                                }
                            }
                            if let Some((_, _, capture, target)) = best {
                                if let Some(rel) =
                                    resolve_exports_target(target, &capture, importer_kind)
                                {
                                    if rel.starts_with('.') {
                                        let candidate_path = d.join(strip_dot_slash(&rel));
                                        return probe_with_extensions(&candidate_path, specifier);
                                    }
                                }
                            }
                        }
                    }
                }
                cur = d.parent();
            }
            return Err(RuntimeError::TypeError(format!(
                "package-internal import '{}' not found in any enclosing package.json's `imports` field",
                specifier
            )));
        }

        let (pkg_name, subpath) = split_bare_specifier(specifier).ok_or_else(|| {
            RuntimeError::TypeError(format!(
                "bare specifier '{}' is malformed (empty or invalid scope/name)",
                specifier
            ))
        })?;

        if is_node_builtin(&pkg_name) && (specifier == pkg_name || !subpath.is_empty()) {
            if subpath.is_empty() {
                return Ok(format!("node:{}", pkg_name));
            }
            let tail = subpath.strip_prefix("./").unwrap_or(&subpath);
            return Ok(format!("node:{}/{}", pkg_name, tail));
        }

        let parent_path_str = parent_url.strip_prefix("file://").unwrap_or(parent_url);
        let parent_path = std::path::Path::new(parent_path_str);
        let start_dir = if parent_path.is_dir() {
            parent_path.to_path_buf()
        } else {
            parent_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("/"))
                .to_path_buf()
        };

        let self_ref_dir: Option<std::path::PathBuf> = {
            let mut cur: Option<&std::path::Path> = Some(start_dir.as_path());
            let mut found = None;
            while let Some(d) = cur {
                let pj = d.join("package.json");
                if pj.is_file() {
                    if let Ok(parsed) = self.read_package_json(&pj) {
                        if parsed.name.as_deref() == Some(pkg_name.as_str())
                            && parsed.raw.get("exports").is_some()
                        {
                            found = Some(d.to_path_buf());
                        }
                    }

                    break;
                }
                cur = d.parent();
            }
            found
        };

        let pkg_dir = match self_ref_dir.or_else(|| walk_up_for_pkg(&start_dir, &pkg_name)) {
            Some(d) => d,
            None => {
                return Err(RuntimeError::TypeError(format!(
                    "__node_cjs_missing_module__:{}|{}|bare specifier '{}' not found: walked up from '{}' looking for node_modules/{}",
                    specifier,
                    parent_path.display(),
                    specifier,
                    start_dir.display(),
                    pkg_name
                )));
            }
        };

        let pkg_json_path = pkg_dir.join("package.json");
        let pkg = if pkg_json_path.is_file() {
            self.read_package_json(&pkg_json_path)?
        } else {
            Rc::new(ParsedPackageJson {
                raw: serde_json::Value::Object(serde_json::Map::new()),
                name: None,
                main: None,
                module_field: None,
                type_field: None,
            })
        };

        if subpath.is_empty() {
            if let Some(map) = pkg.raw.get("exports").and_then(|v| v.as_object()) {
                let keys_are_subpath_style = map.keys().any(|k| k.starts_with('.'));
                if keys_are_subpath_style && map.get(".").is_none() {
                    return Err(RuntimeError::TypeError(format!(
                        "__node_resolve_error__:No \"exports\" main defined in {} imported from {}",
                        pkg_json_path.display(),
                        parent_path.display()
                    )));
                }
            }
        }

        let module_map_candidate = if matches!(importer_kind, ModuleKind::ESM) {
            self.module_map_lookup(&pkg_dir, &subpath)
        } else {
            None
        };
        let candidate = match module_map_candidate {
            Some(p) => std::path::PathBuf::from(p),
            None => match resolve_within_package(&pkg_dir, &pkg, &subpath, importer_kind) {
                Some(candidate) => candidate,
                None if subpath.is_empty() && pkg.raw.get("exports").is_some() => {
                    return Err(RuntimeError::TypeError(format!(
                        "__node_resolve_error__:No \"exports\" main defined in {} imported from {}",
                        pkg_json_path.display(),
                        parent_path.display()
                    )));
                }

                None if !subpath.is_empty() && pkg.raw.get("exports").is_some() => {
                    let req = if subpath.starts_with("./") {
                        subpath.clone()
                    } else {
                        format!("./{}", strip_dot_slash(&subpath))
                    };
                    let msg = match importer_kind {
                        ModuleKind::ESM => format!(
                            "__node_resolve_error__:Package subpath '{}' is not defined by \"exports\" in {} imported from {}",
                            req,
                            pkg_json_path.display(),
                            parent_path.display()
                        ),
                        ModuleKind::CJS => format!(
                            "__node_resolve_error__:Package subpath '{}' is not defined by \"exports\" in {}",
                            req,
                            pkg_json_path.display()
                        ),
                    };
                    return Err(RuntimeError::TypeError(msg));
                }
                None => {
                    return Err(RuntimeError::TypeError(format!(
                        "bare specifier '{}' resolved to package '{}' but no entry matched subpath '{}'",
                        specifier,
                        pkg_dir.display(),
                        subpath
                    )));
                }
            },
        };

        if candidate.extension().and_then(|s| s.to_str()) == Some("json") {
            if candidate.is_file() {
                let canonical = std::fs::canonicalize(&candidate).map_err(|e| {
                    RuntimeError::TypeError(format!(
                        "canonicalize '{}': {}",
                        candidate.display(),
                        e
                    ))
                })?;
                return Ok(format!("file://{}", canonical.display()));
            }
        }

        if pkg.raw.get("exports").is_some() && matches!(importer_kind, ModuleKind::ESM) {
            if candidate.is_file() {
                let canonical = std::fs::canonicalize(&candidate).map_err(|e| {
                    RuntimeError::TypeError(format!(
                        "canonicalize '{}': {}",
                        candidate.display(),
                        e
                    ))
                })?;
                let resolved_url = format!("file://{}", canonical.display());

                return Ok(resolved_url);
            }
            let display = candidate
                .components()
                .collect::<std::path::PathBuf>()
                .display()
                .to_string();
            if candidate.is_dir() {

                return Err(unsupported_dir_import_error(
                    &candidate,
                    &display,
                    specifier,
                    parent_path.display(),
                ));
            }
            return Err(RuntimeError::TypeError(format!(
                "__node_resolve_error__:Cannot find module '{}' imported from {}",
                display,
                parent_path.display()
            )));
        }

        if !subpath.is_empty()
            && pkg.raw.get("exports").is_none()
            && matches!(importer_kind, ModuleKind::ESM)
        {
            return resolve_esm_node_package_subpath_exact(&candidate, specifier, parent_path);
        }

        let resolved_url = match probe_with_extensions(&candidate, specifier) {
            Ok(url) => url,
            Err(_e) if matches!(importer_kind, ModuleKind::CJS) => {
                return Err(RuntimeError::TypeError(format!(
                    "__node_cjs_missing_module__:{}|{}|package subpath '{}' not found from '{}'",
                    specifier,
                    parent_path.display(),
                    specifier,
                    parent_path.display()
                )));
            }
            Err(_e)
                if subpath.is_empty()
                    && pkg.raw.get("exports").is_none()
                    && matches!(importer_kind, ModuleKind::ESM) =>
            {

                let target = match pkg.main.as_deref() {
                    Some("") => pkg_dir.display().to_string(),
                    Some(m) => {
                        format!("{}/{}", pkg_dir.display(), m.trim_start_matches("./"))
                    }
                    None => format!("{}/index.js", pkg_dir.display()),
                };
                return Err(RuntimeError::TypeError(format!(
                    "__node_resolve_error__:Cannot find package '{}' imported from {}",
                    target,
                    parent_path.display()
                )));
            }
            Err(e) => return Err(e),
        };

        let rule = {
            let has_exports = pkg.raw.get("exports").is_some();
            let chosen_str = candidate.display().to_string();
            let from_module_field = pkg
                .module_field
                .as_ref()
                .map(|m| chosen_str.contains(m.trim_start_matches("./")))
                .unwrap_or(false);
            let from_main = pkg
                .main
                .as_ref()
                .map(|m| chosen_str.contains(m.trim_start_matches("./")))
                .unwrap_or(false);
            if has_exports {
                "exports"
            } else if from_module_field {
                "module-field"
            } else if from_main {
                "main"
            } else {
                "index-fallback"
            }
        };
        let trace = format!(
            "spec='{}' chose={} via={} (alternatives: main={:?} module={:?} type={:?})",
            specifier,
            candidate.display(),
            rule,
            pkg.main,
            pkg.module_field,
            pkg.type_field,
        );
        self.module_resolution_trace
            .insert(resolved_url.clone(), trace);
        Ok(resolved_url)
    }

    pub fn read_package_json(
        &mut self,
        path: &std::path::Path,
    ) -> Result<Rc<ParsedPackageJson>, RuntimeError> {
        let key = path.to_path_buf();
        if let Some(p) = self.pkg_json_cache.get(&key) {
            return Ok(p.clone());
        }
        let text = std::fs::read_to_string(path).map_err(|e| {
            RuntimeError::TypeError(format!(
                "package.json read failed at '{}': {}",
                path.display(),
                e
            ))
        })?;
        let parsed = parse_package_json(&text).map_err(|e| {
            RuntimeError::TypeError(format!(
                "package.json parse failed at '{}': {}",
                path.display(),
                e
            ))
        })?;
        let rc = Rc::new(parsed);
        self.pkg_json_cache.insert(key, rc.clone());
        Ok(rc)
    }

    pub(crate) fn module_get(
        &self,
        url: &str,
    ) -> Option<std::rc::Rc<std::cell::RefCell<ModuleRecord>>> {
        if self.current_realm == 0 {
            self.modules.get(url).cloned()
        } else {
            self.realm_module_registries
                .get(&self.current_realm)
                .and_then(|m| m.get(url).cloned())
        }
    }

    pub(crate) fn module_remove(&mut self, url: &str) {
        if self.current_realm == 0 {
            self.modules.remove(url);
        } else if let Some(m) = self.realm_module_registries.get_mut(&self.current_realm) {
            m.remove(url);
        }
        self.module_resolve_cache.clear();
        self.module_preflight_complete_cache.clear();
    }

    pub(crate) fn module_insert(
        &mut self,
        url: String,
        record: std::rc::Rc<std::cell::RefCell<ModuleRecord>>,
    ) {
        if self.current_realm == 0 {
            self.modules.insert(url, record);
        } else {
            self.realm_module_registries
                .entry(self.current_realm)
                .or_default()
                .insert(url, record);
        }
    }

    pub fn module_export_cell(
        &self,
        url: &str,
        exported: &str,
    ) -> Option<crate::value::UpvalueCell> {
        self.module_get(url)
            .and_then(|rec| rec.borrow().export_cells.get(exported).cloned())
    }

    pub fn module_export_cell_names(&self, url: &str) -> Vec<String> {
        let mut names: Vec<String> = self
            .module_get(url)
            .map(|rec| rec.borrow().export_cells.keys().cloned().collect())
            .unwrap_or_default();
        names.sort();
        names
    }

    fn module_resolve_export_cell(
        &mut self,
        url: &str,
        exported: &str,
    ) -> Option<crate::value::UpvalueCell> {
        if let Some(cell) = self.module_export_cell(url, exported) {
            return Some(cell);
        }
        let mut rset = std::collections::HashSet::new();
        match self.module_resolve_export(url, exported, &mut rset) {
            ResolveExportResult::Resolved {
                module_url,
                binding_name,
            } if binding_name != "*namespace*"
                && binding_name != "*deferred-namespace*"
                && binding_name != "*source*" =>
            {
                if module_url == url {
                    self.module_export_cell(&module_url, exported)
                        .or_else(|| self.module_export_cell(&module_url, &binding_name))
                } else {
                    self.module_export_cell(&module_url, &binding_name)
                        .or_else(|| self.module_export_cell(&module_url, exported))
                }
            }
            _ => None,
        }
    }

    fn module_export_local_kind(
        &self,
        url: &str,
        exported: &str,
    ) -> Option<rusty_js_ast::VariableKind> {
        self.module_get(url).and_then(|rec| {
            let r = rec.borrow();
            r.bytecode.exports.iter().find_map(|eb| match eb {
                ExportBinding::Local { exported: e, local } if e == exported => {
                    r.bytecode.locals.get(*local as usize).map(|desc| desc.kind)
                }
                _ => None,
            })
        })
    }

    pub fn load_module(&mut self, url: &str) -> Result<ObjectRef, RuntimeError> {
        self.load_module_with_preferred_kind(url, None)
    }

    pub fn load_module_as(
        &mut self,
        url: &str,
        preferred_kind: ModuleKind,
    ) -> Result<ObjectRef, RuntimeError> {
        self.load_module_with_preferred_kind(url, Some(preferred_kind))
    }

    fn validate_deferred_module_load(&mut self, url: &str) -> Result<(), RuntimeError> {
        let mut seen = std::collections::HashSet::new();
        self.validate_deferred_module_load_inner(url, &mut seen)
    }

    pub fn evaluate_deferred_async_dependencies(
        &mut self,
        url: &str,
    ) -> Result<bool, RuntimeError> {
        Ok(!self
            .evaluate_deferred_async_dependency_urls(url)?
            .is_empty())
    }

    fn evaluate_deferred_async_dependency_urls(
        &mut self,
        url: &str,
    ) -> Result<Vec<String>, RuntimeError> {
        let mut seen = std::collections::HashSet::new();
        let mut async_urls = Vec::new();
        self.collect_deferred_async_dependencies(url, &mut seen, &mut async_urls)?;

        for dep_url in async_urls.iter() {
            let _ = self.load_module_as(&dep_url, ModuleKind::ESM)?;
        }
        Ok(async_urls)
    }

    fn collect_deferred_async_dependencies(
        &mut self,
        url: &str,
        seen: &mut std::collections::HashSet<String>,
        async_urls: &mut Vec<String>,
    ) -> Result<bool, RuntimeError> {
        if url.starts_with("node:") || url.starts_with("cruft:") || url.starts_with("bun:") || url.starts_with("deno:") {
            return Ok(false);
        }
        if !seen.insert(url.to_string()) {
            return Ok(false);
        }

        if url.ends_with(".json") || url.ends_with(".node") || url.ends_with(".fts") {
            return Ok(false);
        }
        if self.module_get(url).is_some() {
            if self.module_scc_evaluated(url) {
                return Ok(false);
            }

            let wait_on = self
                .module_get(url)
                .and_then(|r| r.borrow().async_cycle_root.clone())
                .unwrap_or_else(|| url.to_string());
            let root_suspended = self
                .module_get(&wait_on)
                .map(|r| matches!(r.borrow().status, ModuleStatus::EvaluatingAsync))
                .unwrap_or(false);
            if root_suspended && !async_urls.iter().any(|u| u == &wait_on) {
                async_urls.push(wait_on);
            }
            return Ok(true);
        }
        let Some(path) = url.strip_prefix("file://") else {
            return Ok(false);
        };
        let source = std::fs::read_to_string(path).map_err(|e| {
            RuntimeError::TypeError(format!("module load: cannot read '{}': {}", path, e))
        })?;
        let source = if path.ends_with(".ts")
            || path.ends_with(".mts")
            || path.ends_with(".cts")
            || path.ends_with(".tsx")
        {
            if let Some(err) = node_modules_ts_strip_error(path) {
                return Err(err);
            }
            match ts_resolve::transform::ts_source_to_js_for_path(path, &source) {
                Ok((stripped_src, _witnesses)) => stripped_src,
                Err(_) => source,
            }
        } else {
            source
        };
        let parsed_ast = rusty_js_parser::parse_module_goal(&source).map_err(|e| {
            RuntimeError::SyntaxError(format_public_parse_error(
                &source,
                &e,
                url,
                "",
                source.len(),
            ))
        })?;
        rusty_js_bytecode::compile_module_goal_with_url_force_strict(&source, url, false, true)
            .map_err(|e| RuntimeError::SyntaxError(format!("compile: {}", e.message)))?;

        let requested = module_requests_in_source_order(&parsed_ast);
        let mut has_async = source.contains("await ");
        for spec in requested {
            let resolved = self.resolve_module_full(url, &spec, ModuleKind::ESM)?;
            if self.collect_deferred_async_dependencies(&resolved, seen, async_urls)? {
                has_async = true;
            }
        }
        if has_async && source.contains("await ") && !async_urls.iter().any(|u| u == url) {
            async_urls.push(url.to_string());
        }
        Ok(has_async)
    }

    fn validate_deferred_module_load_inner(
        &mut self,
        url: &str,
        seen: &mut std::collections::HashSet<String>,
    ) -> Result<(), RuntimeError> {
        if url.starts_with("node:") || url.starts_with("cruft:") || url.starts_with("bun:") || url.starts_with("deno:") {
            return Ok(());
        }
        if !seen.insert(url.to_string()) {
            return Ok(());
        }

        if url.ends_with(".json") || url.ends_with(".node") || url.ends_with(".fts") {
            return Ok(());
        }
        if let Some(rec) = self.module_get(url) {
            let r = rec.borrow();
            if r.status == ModuleStatus::Failed {
                match r.eval_error.as_ref() {
                    Some(RuntimeError::SyntaxError(_)) | Some(RuntimeError::CompileError(_)) => {
                        return Err(r.eval_error.clone().unwrap());
                    }
                    _ => return Ok(()),
                }
            }
            return Ok(());
        }
        let Some(path) = url.strip_prefix("file://") else {
            return Err(RuntimeError::TypeError(format!(
                "module load: unsupported URL '{}'",
                url
            )));
        };
        let source = std::fs::read_to_string(path).map_err(|e| {
            RuntimeError::TypeError(format!("module load: cannot read '{}': {}", path, e))
        })?;
        let source = if path.ends_with(".ts")
            || path.ends_with(".mts")
            || path.ends_with(".cts")
            || path.ends_with(".tsx")
        {
            if let Some(err) = node_modules_ts_strip_error(path) {
                return Err(err);
            }
            match ts_resolve::transform::ts_source_to_js_for_path(path, &source) {
                Ok((stripped_src, _witnesses)) => stripped_src,
                Err(_) => source,
            }
        } else {
            source
        };
        let parsed_ast = rusty_js_parser::parse_module_goal(&source).map_err(|e| {
            RuntimeError::SyntaxError(format_public_parse_error(
                &source,
                &e,
                url,
                "",
                source.len(),
            ))
        })?;
        rusty_js_bytecode::compile_module_goal_with_url_force_strict(&source, url, false, true)
            .map_err(|e| RuntimeError::SyntaxError(format!("compile: {}", e.message)))?;

        let requested = module_requests_in_source_order(&parsed_ast);
        for spec in requested {
            let resolved = self.resolve_module_full(url, &spec, ModuleKind::ESM)?;
            self.validate_deferred_module_load_inner(&resolved, seen)?;
        }
        Ok(())
    }

    pub fn load_text_module(&mut self, url: &str) -> Result<ObjectRef, RuntimeError> {
        let cache_url = format!("{url}#with=type:text");
        if let Some(rec) = self.module_get(&cache_url) {
            let r = rec.borrow();
            if r.status == ModuleStatus::Failed {
                return Err(r.eval_error.clone().unwrap_or_else(|| {
                    RuntimeError::TypeError(format!("module '{}' evaluation failed", cache_url))
                }));
            }
            if let Some(ns) = r.namespace {
                return Ok(ns);
            }
        }
        let Some(path) = url.strip_prefix("file://") else {
            return Err(RuntimeError::TypeError(format!(
                "text module load: unsupported URL '{}'",
                url
            )));
        };
        let source = std::fs::read_to_string(path).map_err(|e| {
            RuntimeError::TypeError(format!("text module load: cannot read '{}': {}", path, e))
        })?;
        self.evaluate_text_module(&source, &cache_url)
    }

    fn load_module_with_preferred_kind(
        &mut self,
        url: &str,
        preferred_kind: Option<ModuleKind>,
    ) -> Result<ObjectRef, RuntimeError> {
        if let Some(rec) = self.module_get(url) {
            let r = rec.borrow();
            if r.status == ModuleStatus::Failed {
                return Err(r.eval_error.clone().unwrap_or_else(|| {
                    RuntimeError::TypeError(format!("module '{}' evaluation failed", url))
                }));
            }
            if let Some(ns) = r.namespace {

                return Ok(ns);
            }
        }

        if let Some((name, version)) = self.compartment_for_module_url(url) {
            let crealm = self.compartment_realm_for(&name, &version);
            if crealm != self.current_realm {
                let hit = self
                    .realm_module_registries
                    .get(&crealm)
                    .and_then(|m| m.get(url).cloned());
                if let Some(rec) = hit {
                    let r = rec.borrow();
                    if r.status == ModuleStatus::Failed {
                        return Err(r.eval_error.clone().unwrap_or_else(|| {
                            RuntimeError::TypeError(format!("module '{}' evaluation failed", url))
                        }));
                    }
                    if let Some(ns) = r.namespace {
                        return Ok(ns);
                    }
                }
            }
        }
        if url.starts_with("node:") || url.starts_with("cruft:") || url.starts_with("bun:") || url.starts_with("deno:") {
            return self.resolve_builtin_namespace(url);
        }

        if url.starts_with("data:") {
            let (mediatype, payload) = parse_data_url(url).ok_or_else(|| {
                RuntimeError::TypeError(format!("module load: malformed data: URL '{}'", url))
            })?;
            let source = String::from_utf8(payload).map_err(|_| {
                RuntimeError::TypeError(format!(
                    "module load: data: URL payload is not valid UTF-8 '{}'",
                    url
                ))
            })?;

            if mediatype == "application/json" {
                return self.evaluate_json_module(&source, url);
            }
            if mediatype == "text/javascript" || mediatype == "application/javascript" {
                self.pending_parse_goal = Some(true);
                return self.evaluate_module(&source, url);
            }
            let msg = format!("Unknown module format: {} for URL {}", mediatype, url);
            if let Some(err) = crate::intrinsics::make_error_instance(self, "RangeError", &msg) {
                self.object_set(
                    err,
                    "code".into(),
                    Value::String(Rc::new(crate::value::JsString::from(
                        "ERR_UNKNOWN_MODULE_FORMAT",
                    ))),
                );
                return Err(RuntimeError::Thrown(Value::Object(err)));
            }
            return Err(RuntimeError::RangeError(msg));
        }
        if let Some(stripped) = url.strip_prefix("file://") {

            if stripped.ends_with(".node") {
                if let Some(v) = self.napi_module_cache.get(url).cloned() {

                    if let Value::Object(id) = v {
                        return Ok(id);
                    }
                }
                let caller = self.current_caps_caller();
                self.caps
                    .require_native_addon_load(stripped, &caller)
                    .map_err(|e| RuntimeError::TypeError(e.to_string()))?;
                let exports = crate::napi::load_napi_module(self, stripped)?;
                self.napi_module_cache
                    .insert(url.to_string(), exports.clone());

                if let Value::Object(id) = exports {
                    return Ok(id);
                }
                return Err(RuntimeError::TypeError(format!(
                    "napi: load returned non-object from '{}'",
                    stripped
                )));
            }
            if stripped.ends_with(".fts") {
                return self.load_cruftscript_namespace(url, stripped);
            }
            if matches!(preferred_kind, Some(ModuleKind::ESM))
                && !esm_loader_extension_admitted(stripped)
            {
                return Err(esm_unknown_extension_error(stripped));
            }
            let exact_phase_profile = module_load_exact_phase_profile_start(url);
            let t_read = if phase_profile::enabled() {
                Some(std::time::Instant::now())
            } else {
                None
            };
            let source = std::fs::read_to_string(stripped).map_err(|e| {
                RuntimeError::TypeError(format!("module load: cannot read '{}': {}", stripped, e))
            })?;
            if let Some(t) = t_read {
                phase_profile::add(&phase_profile::READ_NS, t.elapsed().as_nanos() as u64);
            }

            let source = if stripped.ends_with(".ts")
                || stripped.ends_with(".mts")
                || stripped.ends_with(".cts")
                || stripped.ends_with(".tsx")
            {
                if let Some(err) = node_modules_ts_strip_error(stripped) {
                    return Err(err);
                }
                match ts_resolve::transform::ts_source_to_js_for_path(stripped, &source) {
                    Ok((stripped_src, _witnesses)) => stripped_src,
                    Err(_) => source,
                }
            } else {
                source
            };

            if stripped.ends_with(".json") {
                let result = self.evaluate_json_module(&source, url);
                module_load_exact_phase_profile_finish(
                    url,
                    ModuleKind::ESM,
                    exact_phase_profile,
                    result.as_ref().map(|_| ()),
                );
                return result;
            }

            let is_bundle_runtime =
                source.contains("__nccwpck_require__") || source.contains("__webpack_require__");
            let has_cjs_markers = source_has_cjs_export_markers(&source) || is_bundle_runtime;
            let has_esm_markers = source_has_esm_markers(&source);
            let package_kind = detect_module_kind(url);
            let graph_forced_esm = matches!(preferred_kind, Some(ModuleKind::ESM))
                && !has_cjs_markers
                && !has_esm_markers
                && (package_kind == ModuleKind::ESM || !url_under_node_modules(url));
            let kind = Self::classify_loaded_js_module_kind(
                preferred_kind,
                package_kind,
                has_esm_markers,
                has_cjs_markers,
                is_bundle_runtime,
                graph_forced_esm,
            );
            match kind {
                ModuleKind::ESM if graph_forced_esm => {
                    self.graph_forced_esm_urls.insert(url.to_string());
                }
                _ => {}
            }
            match kind {
                ModuleKind::ESM => {

                    self.pending_parse_goal = Some(true);

                    match self.compartment_for_module_url(url) {
                        Some((name, version)) => {
                            let realm = self.compartment_realm_for(&name, &version);
                            let result = self.evaluate_module_in_realm(&source, url, realm);
                            module_load_exact_phase_profile_finish(
                                url,
                                kind,
                                exact_phase_profile,
                                result.as_ref().map(|_| ()),
                            );
                            result
                        }
                        None => {
                            let result = self.evaluate_module(&source, url);
                            module_load_exact_phase_profile_finish(
                                url,
                                kind,
                                exact_phase_profile,
                                result.as_ref().map(|_| ()),
                            );
                            result
                        }
                    }
                }
                ModuleKind::CJS => {
                    let result = self.evaluate_cjs_module(&source, url);
                    module_load_exact_phase_profile_finish(
                        url,
                        kind,
                        exact_phase_profile,
                        result.as_ref().map(|_| ()),
                    );
                    result
                }
            }
        } else {
            Err(RuntimeError::TypeError(format!(
                "load_module: unsupported URL scheme '{}'",
                url
            )))
        }
    }

    fn classify_loaded_js_module_kind(
        preferred_kind: Option<ModuleKind>,
        package_kind: ModuleKind,
        has_esm_markers: bool,
        has_cjs_markers: bool,
        is_bundle_runtime: bool,
        graph_forced_esm: bool,
    ) -> ModuleKind {
        if package_kind == ModuleKind::ESM && !is_bundle_runtime {

            ModuleKind::ESM
        } else if matches!(preferred_kind, Some(ModuleKind::ESM))
            && has_esm_markers
            && !is_bundle_runtime
        {

            ModuleKind::ESM
        } else if has_esm_markers && !has_cjs_markers {
            ModuleKind::ESM
        } else if graph_forced_esm {

            ModuleKind::ESM
        } else {
            package_kind
        }
    }

    pub fn cjs_exports_of(&self, url: &str) -> Option<Value> {
        self.module_get(url)
            .and_then(|r| r.borrow().cjs_exports.clone())
    }

    pub fn module_kind_of(&self, url: &str) -> Option<ModuleKind> {
        self.module_get(url).map(|r| r.borrow().kind)
    }

    fn module_resolve_export(
        &mut self,
        url: &str,
        name: &str,
        resolve_set: &mut std::collections::HashSet<(String, String)>,
    ) -> ResolveExportResult {
        if !resolve_set.insert((url.to_string(), name.to_string())) {
            return ResolveExportResult::NotFound;
        }
        if url.starts_with("node:") || url.starts_with("cruft:") || url.starts_with("bun:") || url.starts_with("deno:") {

            if let Ok(ns) = self.resolve_builtin_namespace(url) {
                if self
                    .ordinary_own_enumerable_string_keys(ns)
                    .into_iter()
                    .any(|key| key == name)
                {
                    return ResolveExportResult::Resolved {
                        module_url: url.to_string(),
                        binding_name: name.to_string(),
                    };
                }
            }
        }
        if self.module_kind_of(url) == Some(ModuleKind::CJS) {

            let _ = self.load_module_as(url, ModuleKind::CJS);
            if self
                .cjs_static_export_keys_for_url(url)
                .is_some_and(|keys| keys.iter().any(|k| k == name))
            {
                return ResolveExportResult::Resolved {
                    module_url: url.to_string(),
                    binding_name: name.to_string(),
                };
            }
        }
        let ast = match self.module_get(url) {
            Some(r) => r.borrow().ast.clone(),

            None => return ResolveExportResult::Incomplete,
        };

        if let Some(entry) = ast
            .local_export_entries
            .iter()
            .find(|e| e.export_name.as_deref() == Some(name))
        {
            if let Some(local_name) = entry.local_name.as_deref() {
                if let Some(import_entry) = ast
                    .import_entries
                    .iter()
                    .find(|i| i.local_name == local_name)
                {
                    if matches!(import_entry.import_name, rusty_js_ast::ImportName::Source) {
                        let module_url = if import_entry.module_request == "<module source>" {
                            import_entry.module_request.clone()
                        } else {
                            match self.resolve_module_full(
                                url,
                                &import_entry.module_request,
                                ModuleKind::ESM,
                            ) {
                                Ok(child) => child,
                                Err(_) => return ResolveExportResult::NotFound,
                            }
                        };
                        return ResolveExportResult::Resolved {
                            module_url,
                            binding_name: "*source*".to_string(),
                        };
                    }
                    if let Ok(child) =
                        self.resolve_module_full(url, &import_entry.module_request, ModuleKind::ESM)
                    {
                        let import_name = match &import_entry.import_name {
                            rusty_js_ast::ImportName::Single(inner) => inner.as_str(),
                            rusty_js_ast::ImportName::Default => "default",
                            rusty_js_ast::ImportName::Namespace => "*namespace*",
                            rusty_js_ast::ImportName::Source => "*source*",
                        };
                        if import_name == "*namespace*" {

                            return ResolveExportResult::Resolved {
                                module_url: child,
                                binding_name: if import_entry.import_defer {
                                    "*deferred-namespace*".to_string()
                                } else {
                                    import_name.to_string()
                                },
                            };
                        }
                        return self.module_resolve_export(&child, import_name, resolve_set);
                    }
                    return ResolveExportResult::NotFound;
                }
            }
            return ResolveExportResult::Resolved {
                module_url: url.to_string(),
                binding_name: entry.local_name.clone().unwrap_or_else(|| name.to_string()),
            };
        }

        for e in &ast.indirect_export_entries {
            if e.export_name.as_deref() == Some(name) {
                let Some(req) = e.module_request.clone() else {
                    continue;
                };
                let inner = match &e.import_name {
                    Some(rusty_js_ast::ExportImportName::Single(s)) => s.clone(),
                    Some(rusty_js_ast::ExportImportName::Default) => "default".to_string(),

                    Some(rusty_js_ast::ExportImportName::All)
                    | Some(rusty_js_ast::ExportImportName::AllButDefault) => {
                        if let Ok(child) = self.resolve_module_full(url, &req, ModuleKind::ESM) {
                            return ResolveExportResult::Resolved {
                                module_url: child,
                                binding_name: "*namespace*".to_string(),
                            };
                        }
                        return ResolveExportResult::NotFound;
                    }
                    None => name.to_string(),
                };
                if let Ok(child) = self.resolve_module_full(url, &req, ModuleKind::ESM) {
                    return self.module_resolve_export(&child, &inner, resolve_set);
                }
                return ResolveExportResult::NotFound;
            }
        }

        if name == "default" {
            return ResolveExportResult::NotFound;
        }

        let stars: Vec<String> = ast
            .star_export_entries
            .iter()
            .filter_map(|e| e.module_request.clone())
            .collect();
        let mut found: Option<(String, String)> = None;
        let mut saw_incomplete = false;
        for req in stars {
            if let Ok(child) = self.resolve_module_full(url, &req, ModuleKind::ESM) {
                match self.module_resolve_export(&child, name, resolve_set) {
                    ResolveExportResult::Resolved {
                        module_url,
                        binding_name,
                    } => {
                        let identity = (module_url, binding_name);
                        if let Some(existing) = &found {
                            if existing != &identity {
                                return ResolveExportResult::Ambiguous;
                            }
                        } else {
                            found = Some(identity);
                        }
                    }
                    ResolveExportResult::Ambiguous => return ResolveExportResult::Ambiguous,
                    ResolveExportResult::Incomplete => saw_incomplete = true,
                    ResolveExportResult::NotFound => {}
                }
            }
        }
        match found {
            Some((module_url, binding_name)) => ResolveExportResult::Resolved {
                module_url,
                binding_name,
            },
            None if saw_incomplete => ResolveExportResult::Incomplete,
            None => ResolveExportResult::NotFound,
        }
    }

    pub fn resolve_import_binding_value(
        &mut self,
        ns: ObjectRef,
        kind: &ImportBindingKind,
        is_cjs: bool,
        resolved_url: &str,
    ) -> Value {
        let cjs_raw = if is_cjs {
            self.cjs_exports_of(resolved_url)
        } else {
            None
        };
        match (kind, &cjs_raw) {
            (ImportBindingKind::Source, _) => {
                Value::Object(self.make_abstract_module_source_object(resolved_url, resolved_url))
            }
            (ImportBindingKind::Default, Some(raw)) => raw.clone(),
            (ImportBindingKind::Namespace | ImportBindingKind::DeferredNamespace, Some(raw)) => {
                Value::Object(self.cjs_namespace_view_at(raw.clone(), Some(resolved_url)))
            }

            (ImportBindingKind::Named(n), Some(raw)) if n == "default" => raw.clone(),
            (ImportBindingKind::Named(n), Some(raw)) => match raw {
                Value::Object(_) => self
                    .resolve_cjs_named_export_value(resolved_url, raw, n)
                    .unwrap_or(Value::Undefined),
                _ => Value::Undefined,
            },
            (ImportBindingKind::Default, None) => self.object_get(ns, "default"),
            (ImportBindingKind::Namespace, None) => Value::Object(ns),
            (ImportBindingKind::DeferredNamespace, None) => Value::Object(ns),
            (ImportBindingKind::Named(n), None) => {
                if let Some(getter) = self.find_getter(ns, n) {
                    self.call_function(getter, Value::Object(ns), Vec::new())
                        .unwrap_or(Value::Undefined)
                } else {
                    self.object_get(ns, n)
                }
            }

            (ImportBindingKind::DeferredNamespace, _) => Value::Object(ns),
        }
    }

    pub fn resolve_builtin_namespace(
        &mut self,
        specifier: &str,
    ) -> Result<ObjectRef, RuntimeError> {

        match self.try_resolve_builtin(specifier)? {
            Some(ns) => Ok(ns),
            None => Err(RuntimeError::TypeError(format!(
                "{}",
                Self::unknown_builtin_module_message(specifier)
            ))),
        }
    }

    fn unknown_builtin_module_message(specifier: &str) -> String {
        if specifier == "cruft:postgres" {
            return "unknown built-in module 'cruft:postgres'; Postcrust is currently exposed through `cruft:orm`: use `require(\"cruft:orm\").openPostgres(...)` or `import { openPostgres } from \"cruft:orm\"`"
                .to_string();
        }

        for scheme in ["cruft:", "node:", "bun:", "deno:"] {
            if let Some(name) = specifier.strip_prefix(scheme) {
                return format!(
                    "'{scheme}' is a reserved cruft runtime namespace; '{name}' is not a \
                     built-in module under it (reserved specifiers never resolve to node_modules)"
                );
            }
        }
        format!(
            "unknown built-in module '{}' (no host hook installed or hook returned None)",
            specifier
        )
    }

    fn load_cruftscript_namespace(
        &mut self,
        url: &str,
        path: &str,
    ) -> Result<ObjectRef, RuntimeError> {
        if let Some(rec) = self.module_get(url) {
            if let Some(ns) = rec.borrow().namespace {
                return Ok(ns);
            }
        }
        let source = std::fs::read_to_string(path).map_err(|e| {
            RuntimeError::TypeError(format!(
                "cruftscript module load: cannot read '{}': {}",
                path, e
            ))
        })?;
        let hook = self.host_hooks.load_cruftscript.take();
        let result = match &hook {
            Some(f) => f(self, url, &source),
            None => Ok(None),
        };
        self.host_hooks.load_cruftscript = hook;
        let ns = match result? {
            Some(o) => o,
            None => {
                return Err(RuntimeError::TypeError(format!(
                    "cruftscript module '{}' requires a host CruftScript loader hook",
                    url
                )))
            }
        };
        self.finalize_module_namespace_exotic(ns);

        let empty_ast = Rc::new(AstModule {
            span: rusty_js_ast::Span::new(0, 0),
            body: Vec::new(),
            import_entries: Vec::new(),
            local_export_entries: Vec::new(),
            indirect_export_entries: Vec::new(),
            star_export_entries: Vec::new(),
        });
        let empty_bc = Rc::new(CompiledModule {
            bytecode: Vec::new(),
            constants: Default::default(),
            locals: Vec::new(),
            catch_param_names: Vec::new(),
            source_map: Vec::new(),
            source_text: None,
            imports: Vec::new(),
            exports: Vec::new(),
            reexport_sources: Vec::new(),
            side_effect_imports: Vec::new(),
            construct_tags: Vec::new(),
            line_starts: Vec::new(),
            eval_var_env_is_global: false,
            global_env_alias: false,
            script_var_deletable: false,
            eval_outer_locals: Vec::new(),
            module_hoisted_functions: Vec::new(),
            strict: false,
        });
        self.module_insert(
            url.to_string(),
            Rc::new(RefCell::new(ModuleRecord {
                url: url.to_string(),
                status: ModuleStatus::Evaluated,
                ast: empty_ast,
                bytecode: empty_bc,
                namespace: Some(ns),
                eval_error: None,
                kind: ModuleKind::ESM,
                cjs_exports: None,
                export_cells: std::collections::HashMap::new(),
                async_static_deps: Vec::new(),
                body_completed_waiting_async_deps: false,
                pending_body_start: None,
                async_evaluation_order: None,
                async_cycle_root: None,
            })),
        );
        Ok(ns)
    }

    pub fn make_deferred_module_namespace(&mut self, url: &str) -> ObjectRef {
        if let Some(rec) = self.module_get(url) {
            if let Some(ns) = rec.borrow().namespace {
                return ns;
            }
        }
        let ns = self.alloc_object(Object::new_ordinary());
        self.finalize_module_namespace_exotic(ns);
        ns
    }

    pub fn make_deferred_namespace(
        &mut self,
        importer_url: &str,
        resolved: &str,
    ) -> Result<ObjectRef, RuntimeError> {

        if let Some(cached) = self.deferred_namespace_cache.get(resolved).copied() {
            return Ok(cached);
        }
        let ns_obj = self.alloc_object(Object::new_module_namespace());
        self.deferred_namespace_cache
            .insert(resolved.to_string(), ns_obj);
        self.module_namespace_urls
            .insert(ns_obj, resolved.to_string());
        let mut exports: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

        if let Some(path) = resolved.strip_prefix("file://") {

            let is_non_js_leaf =
                path.ends_with(".json") || path.ends_with(".node") || path.ends_with(".fts");
            if !is_non_js_leaf {
                match std::fs::read_to_string(path) {
                    Ok(src) => {
                        if let Err(e) = rusty_js_parser::parse_module(&src) {
                            return Err(RuntimeError::SyntaxError(format!(
                                "compile: {}",
                                e.message
                            )));
                        }
                    }
                    Err(_) => {

                        return Err(RuntimeError::TypeError(format!(
                            "module load: cannot read '{}'",
                            path
                        )));
                    }
                }
            }
        }
        let _ = self.collect_module_export_names(importer_url, resolved, &mut seen, &mut exports);

        let mut sorted_names: Vec<&String> = exports.iter().collect();
        sorted_names.sort();
        for name in sorted_names {
            self.obj_mut(ns_obj).dict_mut().insert(
                crate::value::PropertyKey::String(name.clone()),
                crate::value::PropertyDescriptor {
                    value: Value::Undefined,
                    writable: true,
                    enumerable: true,
                    configurable: false,
                    getter: None,
                    setter: None,
                },
            );
        }
        self.obj_mut(ns_obj).dict_mut().insert(
            crate::value::PropertyKey::String("@@toStringTag".to_string()),
            crate::value::PropertyDescriptor {
                value: Value::String(Rc::new(crate::value::JsString::from(
                    "Deferred Module".to_string(),
                ))),
                writable: false,
                enumerable: false,
                configurable: false,
                getter: None,
                setter: None,
            },
        );
        self.deferred_namespaces.insert(ns_obj, exports);
        Ok(ns_obj)
    }

    fn collect_module_export_names(
        &mut self,
        importer_url: &str,
        resolved: &str,
        seen: &mut std::collections::HashSet<String>,
        out: &mut std::collections::HashSet<String>,
    ) -> Result<(), RuntimeError> {
        if phase_profile::enabled() {
            phase_profile::inc(&phase_profile::EXPORT_NAME_COLLECTION_CALLS);
        }
        if !seen.insert(resolved.to_string()) {
            return Ok(());
        }
        if self.module_kind_of(resolved) == Some(ModuleKind::CJS) {
            let t_keys = phase_profile::enabled().then(std::time::Instant::now);
            if let Some(keys) = self.cjs_static_export_keys_for_url(resolved) {
                if let Some(t) = t_keys {
                    phase_profile::add(
                        &phase_profile::EXPORT_NAME_COLLECTION_CJS_KEYS_NS,
                        t.elapsed().as_nanos() as u64,
                    );
                }
                out.extend(keys.iter().cloned());
                return Ok(());
            }
        }

        if resolved.starts_with("node:")
            || resolved.starts_with("cruft:")
            || resolved.starts_with("bun:") || resolved.starts_with("deno:")
        {
            if let Ok(Some(ns)) = self.try_resolve_builtin(resolved) {

                if let Ok(Value::Object(arr)) = self.own_property_names_via(&Value::Object(ns)) {
                    let len = self.array_length(arr);
                    for i in 0..len {
                        if let Value::String(s) = self.object_get(arr, &i.to_string()) {
                            out.insert(s.as_str().to_string());
                        }
                    }
                }
                out.insert("default".to_string());
            }
            return Ok(());
        }
        let Some(path) = resolved.strip_prefix("file://") else {
            return Ok(());
        };
        let t_read = phase_profile::enabled().then(std::time::Instant::now);
        let Ok(source) = std::fs::read_to_string(path) else {
            return Ok(());
        };
        if let Some(t) = t_read {
            phase_profile::add(
                &phase_profile::EXPORT_NAME_COLLECTION_READ_NS,
                t.elapsed().as_nanos() as u64,
            );
        }
        let has_cjs_markers = source_has_cjs_export_markers(&source)
            || source.contains("__nccwpck_require__")
            || source.contains("__webpack_require__");
        let has_esm_markers = source_has_esm_markers(&source);
        if has_cjs_markers && !has_esm_markers {

            let t_load = phase_profile::enabled().then(std::time::Instant::now);
            let _ = self.load_module_as(resolved, ModuleKind::CJS);
            if let Some(t) = t_load {
                phase_profile::add(
                    &phase_profile::EXPORT_NAME_COLLECTION_CJS_LOAD_NS,
                    t.elapsed().as_nanos() as u64,
                );
            }
            let t_keys = phase_profile::enabled().then(std::time::Instant::now);
            if let Some(keys) = self.cjs_static_export_keys_for_url(resolved) {
                if let Some(t) = t_keys {
                    phase_profile::add(
                        &phase_profile::EXPORT_NAME_COLLECTION_CJS_KEYS_NS,
                        t.elapsed().as_nanos() as u64,
                    );
                }
                out.extend(keys);
            }
            return Ok(());
        }

        let t_parse = phase_profile::enabled().then(std::time::Instant::now);
        let Ok(module) = rusty_js_parser::parse_module_goal(&source) else {
            let t_load = phase_profile::enabled().then(std::time::Instant::now);
            let _ = self.load_module_as(resolved, ModuleKind::CJS);
            if let Some(t) = t_load {
                phase_profile::add(
                    &phase_profile::EXPORT_NAME_COLLECTION_CJS_LOAD_NS,
                    t.elapsed().as_nanos() as u64,
                );
            }
            let t_keys = phase_profile::enabled().then(std::time::Instant::now);
            if let Some(keys) = self.cjs_static_export_keys_for_url(resolved) {
                if let Some(t) = t_keys {
                    phase_profile::add(
                        &phase_profile::EXPORT_NAME_COLLECTION_CJS_KEYS_NS,
                        t.elapsed().as_nanos() as u64,
                    );
                }
                out.extend(keys);
            }
            return Ok(());
        };
        if let Some(t) = t_parse {
            phase_profile::add(
                &phase_profile::EXPORT_NAME_COLLECTION_PARSE_NS,
                t.elapsed().as_nanos() as u64,
            );
        }
        for e in &module.local_export_entries {
            if let Some(n) = &e.export_name {
                out.insert(n.clone());
            }
        }
        for e in &module.indirect_export_entries {
            if let Some(n) = &e.export_name {
                out.insert(n.clone());
            }
        }
        for e in &module.star_export_entries {
            if let Some(n) = &e.export_name {
                out.insert(n.clone());
            } else if let Some(req) = &e.module_request {
                if phase_profile::enabled() {
                    phase_profile::inc(&phase_profile::EXPORT_NAME_COLLECTION_STAR_EDGES);
                }
                let t_resolve = phase_profile::enabled().then(std::time::Instant::now);
                if let Ok(child) = self.resolve_module_full(resolved, req, ModuleKind::ESM) {
                    if let Some(t) = t_resolve {
                        phase_profile::add(
                            &phase_profile::EXPORT_NAME_COLLECTION_RESOLVE_NS,
                            t.elapsed().as_nanos() as u64,
                        );
                    }
                    let _ = self.collect_module_export_names(importer_url, &child, seen, out);
                }
            }
        }
        let _ = importer_url;
        Ok(())
    }

    pub fn deferred_module_namespace_should_trigger(&self, obj: ObjectRef, name: &str) -> bool {

        if !self.deferred_namespaces.contains_key(&obj) {
            return false;
        }

        name != "then"
            && !name.starts_with("@@")
            && !name.starts_with('#')
            && !name.starts_with("__private_in__:")
    }

    pub fn maybe_trigger_deferred_for_key(
        &mut self,
        target: &Value,
        key: &Value,
    ) -> Result<(), RuntimeError> {
        if self.deferred_namespaces.is_empty() {
            return Ok(());
        }
        if let Value::Object(id) = target {
            let name = match key {
                Value::String(s) => s.as_str().to_string(),
                Value::Symbol(_) => return Ok(()),
                other => crate::abstract_ops::to_string(other).as_str().to_string(),
            };
            if self.deferred_module_namespace_should_trigger(*id, &name) {
                self.trigger_deferred_module_namespace(*id)?;
            }
        }
        Ok(())
    }

    pub fn maybe_trigger_deferred_all(&mut self, target: &Value) -> Result<(), RuntimeError> {
        if self.deferred_namespaces.is_empty() {
            return Ok(());
        }
        if let Value::Object(id) = target {
            if self.deferred_namespaces.contains_key(id) {
                self.trigger_deferred_module_namespace(*id)?;
            }
        }
        Ok(())
    }

    pub fn trigger_deferred_module_namespace(
        &mut self,
        obj: ObjectRef,
    ) -> Result<(), RuntimeError> {
        if !self.deferred_namespaces.contains_key(&obj) {
            return Ok(());
        }
        let Some(url) = self.module_namespace_urls.get(&obj).cloned() else {
            self.deferred_namespaces.remove(&obj);
            return Ok(());
        };

        if !self.module_scc_evaluated(&url) {
            let mut seen = std::collections::HashSet::new();
            if !self.module_ready_for_sync(&url, &mut seen) {
                return Err(RuntimeError::TypeError(format!(
                    "Cannot access deferred module namespace of '{}' while it is evaluating",
                    url
                )));
            }
        }

        let real_ns = match self.load_module_as(&url, ModuleKind::ESM) {
            Ok(ns) => ns,
            Err(e) => return Err(e),
        };
        self.deferred_namespaces.remove(&obj);
        if real_ns != obj {
            let props: Vec<(crate::value::PropertyKey, crate::value::PropertyDescriptor)> = self
                .obj(real_ns)
                .properties
                .iter()
                .map(|(k, d)| (k.clone(), d.clone()))
                .collect();
            for (k, d) in props {
                self.obj_mut(obj).dict_mut().insert(k, d);
            }
        }
        Ok(())
    }

    fn module_scc_evaluated(&self, url: &str) -> bool {
        let Some(rec) = self.module_get(url) else {
            return false;
        };
        let (status, root) = {
            let r = rec.borrow();
            (r.status, r.async_cycle_root.clone())
        };
        if !matches!(status, ModuleStatus::Evaluated) {
            return false;
        }
        match root {
            Some(root) if root != url => self
                .module_get(&root)
                .map(|r| matches!(r.borrow().status, ModuleStatus::Evaluated))
                .unwrap_or(true),
            _ => true,
        }
    }

    fn module_ready_for_sync(
        &mut self,
        url: &str,
        seen: &mut std::collections::HashSet<String>,
    ) -> bool {
        if !seen.insert(url.to_string()) {
            return true;
        }
        if self.module_scc_evaluated(url) {
            return true;
        }
        let status = self.module_get(url).map(|r| r.borrow().status);
        match status {
            Some(ModuleStatus::Evaluated) => return false,

            Some(ModuleStatus::Linking)
            | Some(ModuleStatus::Evaluating)
            | Some(ModuleStatus::EvaluatingAsync) => return false,
            _ => {}
        }

        let mut requested: Vec<String> = Vec::new();
        let bytecode = match self.module_get(url) {
            Some(rec) => Some(rec.borrow().bytecode.clone()),
            None => url
                .strip_prefix("file://")
                .and_then(|p| std::fs::read_to_string(p).ok())
                .and_then(|src| {
                    rusty_js_bytecode::compile_module_goal_with_url_force_strict(
                        &src, url, false, true,
                    )
                    .ok()
                    .map(std::rc::Rc::new)
                }),
        };
        let Some(bytecode) = bytecode else {
            return true;
        };
        requested.extend(bytecode.side_effect_imports.iter().cloned());
        requested.extend(bytecode.reexport_sources.iter().cloned());
        requested.extend(
            bytecode
                .imports
                .iter()
                .filter(|ib| !matches!(ib.kind, ImportBindingKind::Source))
                .map(|ib| ib.module_request.clone()),
        );
        for spec in requested {
            if let Ok(dep) = self.resolve_module_full(url, &spec, ModuleKind::ESM) {
                if !self.module_ready_for_sync(&dep, seen) {
                    return false;
                }
            }
        }
        true
    }

    fn finalize_module_namespace_exotic(&mut self, namespace: ObjectRef) {
        let mut entries: Vec<(crate::value::PropertyKey, crate::value::PropertyDescriptor)> = self
            .obj(namespace)
            .properties
            .iter()
            .map(|(k, d)| (k.clone(), d.clone()))
            .collect();
        for (key, desc) in entries.iter_mut() {
            if key.as_str() == "@@toStringTag" || key.as_str().starts_with("__") {
                continue;
            }
            desc.enumerable = true;
            desc.configurable = false;
        }
        entries.sort_by(|(a, _), (b, _)| {
            let a_tag = a.as_str() == "@@toStringTag";
            let b_tag = b.as_str() == "@@toStringTag";
            match (a_tag, b_tag) {
                (false, false) => a.as_str().cmp(b.as_str()),
                (false, true) => std::cmp::Ordering::Less,
                (true, false) => std::cmp::Ordering::Greater,
                (true, true) => std::cmp::Ordering::Equal,
            }
        });
        let mut normalized = indexmap::IndexMap::new();
        for (key, desc) in entries {
            normalized.insert(key, desc);
        }
        normalized.insert(
            crate::value::PropertyKey::String("@@toStringTag".to_string()),
            crate::value::PropertyDescriptor {
                value: Value::String(std::rc::Rc::new(crate::value::JsString::from(
                    "Module".to_string(),
                ))),
                writable: false,
                enumerable: false,
                configurable: false,
                getter: None,
                setter: None,
            },
        );
        let o = self.obj_mut(namespace);
        o.properties = normalized;
        o.extensible = false;
        o.proto = None;
    }

    pub fn trigger_deferred_module_namespace_in_chain(
        &mut self,
        start: ObjectRef,
        key: &str,
    ) -> Result<(), RuntimeError> {
        let mut cur = Some(start);
        while let Some(id) = cur {
            if self.deferred_module_namespace_should_trigger(id, key) {
                self.trigger_deferred_module_namespace(id)?;
                return Ok(());
            }
            cur = self.obj(id).proto;
        }
        Ok(())
    }

    pub fn evaluate_script(&mut self, source: &str, url: &str) -> Result<ObjectRef, RuntimeError> {
        self.evaluate_script_with_global_env_alias(source, url, true)
    }

    pub(crate) fn evaluate_script_with_global_env_alias(
        &mut self,
        source: &str,
        url: &str,
        global_env_alias: bool,
    ) -> Result<ObjectRef, RuntimeError> {

        self.pending_script_mode = true;
        self.pending_script_global_env_alias = global_env_alias;
        let prior_parse_goal = self.pending_parse_goal;
        self.pending_parse_goal = Some(false);
        let r = self.evaluate_module(source, url);
        self.pending_parse_goal = prior_parse_goal;
        self.pending_script_mode = false;
        self.pending_script_global_env_alias = false;
        r
    }

    fn preflight_static_module_requests_source_order(
        &mut self,
        url: &str,
        ast: &AstModule,
        seen: &mut std::collections::HashSet<String>,
    ) -> Result<(), RuntimeError> {
        let cache_key = if self.module_preflight_cache_enabled {
            Some((self.current_realm, url.to_string()))
        } else {
            None
        };
        if let Some(key) = cache_key.as_ref() {
            if self.module_preflight_complete_cache.contains(key) {
                if phase_profile::enabled() {
                    phase_profile::inc(&phase_profile::PREFLIGHT_CACHE_HITS);
                }
                return Ok(());
            }
        }
        if !seen.insert(url.to_string()) {
            return Ok(());
        }
        if phase_profile::enabled() {
            phase_profile::inc(&phase_profile::PREFLIGHT_CACHE_MISSES);
        }
        let mut pending_named_validations: Vec<(&str, String, Vec<rusty_js_ast::ImportSpecifier>)> =
            Vec::new();
        for item in &ast.body {
            let spec = match item {
                rusty_js_ast::ModuleItem::Import(imp) => {

                    if imp.source_phase {

                        if imp.specifier.value != "<module source>" {
                            let resolved = self.resolve_module_full(
                                url,
                                imp.specifier.value.as_str(),
                                ModuleKind::ESM,
                            )?;
                            self.preflight_resolved_esm_module_source_order(&resolved, seen)?;
                        }
                        continue;
                    }

                    let type_attr = imp.attributes.iter().find_map(|a| {
                        let k = match &a.key {
                            rusty_js_ast::ModuleExportName::Ident(b) => b.name.as_str(),
                            rusty_js_ast::ModuleExportName::String { value, .. } => value.as_str(),
                        };
                        (k == "type").then(|| a.value.as_str())
                    });
                    if matches!(type_attr, Some("bytes") | Some("text")) {
                        continue;
                    }
                    Some(imp.specifier.value.as_str())
                }
                rusty_js_ast::ModuleItem::Export(export) => match export {
                    rusty_js_ast::ExportDeclaration::Named {
                        source: Some(source),
                        ..
                    }
                    | rusty_js_ast::ExportDeclaration::StarFrom { source, .. }
                    | rusty_js_ast::ExportDeclaration::StarAsFrom { source, .. } => {
                        Some(source.value.as_str())
                    }
                    _ => None,
                },
                _ => None,
            };
            let Some(spec) = spec else {
                continue;
            };
            let resolved = self.resolve_module_full(url, spec, ModuleKind::ESM)?;
            if let rusty_js_ast::ModuleItem::Import(imp) = item {
                let is_builtin = resolved.starts_with("node:")
                    || resolved.starts_with("cruft:")
                    || resolved.starts_with("bun:") || resolved.starts_with("deno:");
                if (resolved.ends_with(".mjs") || is_builtin) && !imp.named_imports.is_empty() {
                    pending_named_validations.push((
                        spec,
                        resolved.clone(),
                        imp.named_imports.clone(),
                    ));
                }
            }
            self.preflight_resolved_esm_module_source_order(&resolved, seen)?;
        }
        for (spec, resolved, named_imports) in pending_named_validations {
            let t_named_exports = phase_profile::enabled().then(std::time::Instant::now);
            let mut seen_names = std::collections::HashSet::new();
            let mut names = std::collections::HashSet::new();
            self.collect_module_export_names(url, &resolved, &mut seen_names, &mut names)?;
            if let Some(t) = t_named_exports {
                phase_profile::add(
                    &phase_profile::PREFLIGHT_NAMED_EXPORT_VALIDATION_NS,
                    t.elapsed().as_nanos() as u64,
                );
            }

            let is_builtin = resolved.starts_with("node:")
                || resolved.starts_with("cruft:")
                || resolved.starts_with("bun:") || resolved.starts_with("deno:");
            if is_builtin && names.is_empty() {
                continue;
            }
            for named in &named_imports {
                let imported = match &named.imported {
                    rusty_js_ast::ModuleExportName::Ident(b) => b.name.as_str(),
                    rusty_js_ast::ModuleExportName::String { value, .. } => value.as_str(),
                };
                if !names.contains(imported) {
                    return Err(RuntimeError::SyntaxError(format!(
                        "The requested module '{}' does not provide an export named '{}'",
                        spec, imported
                    )));
                }
            }
        }
        if let Some(key) = cache_key {
            self.module_preflight_complete_cache.insert(key);
        }
        Ok(())
    }

    fn preflight_resolved_esm_module_source_order(
        &mut self,
        resolved: &str,
        seen: &mut std::collections::HashSet<String>,
    ) -> Result<(), RuntimeError> {
        if resolved.starts_with("node:")
            || resolved.starts_with("cruft:")
            || resolved.starts_with("bun:") || resolved.starts_with("deno:")
        {
            return Ok(());
        }
        if let Some(rec) = self.module_get(resolved) {
            let (kind, ast) = {
                let r = rec.borrow();
                (r.kind, r.ast.clone())
            };
            if kind == ModuleKind::ESM {
                return self.preflight_static_module_requests_source_order(resolved, &ast, seen);
            }
            return Ok(());
        }
        let Some(path) = resolved.strip_prefix("file://") else {
            return Ok(());
        };
        if path.ends_with(".json") || path.ends_with(".node") || path.ends_with(".fts") {
            return Ok(());
        }
        if !esm_loader_extension_admitted(path) {
            return Err(esm_unknown_extension_error(path));
        }
        let source = std::fs::read_to_string(path).map_err(|e| {
            RuntimeError::TypeError(format!("module load: cannot read '{}': {}", path, e))
        })?;
        let source = if path.ends_with(".ts")
            || path.ends_with(".mts")
            || path.ends_with(".cts")
            || path.ends_with(".tsx")
        {
            if let Some(err) = node_modules_ts_strip_error(path) {
                return Err(err);
            }
            match ts_resolve::transform::ts_source_to_js_for_path(path, &source) {
                Ok((stripped_src, _witnesses)) => stripped_src,
                Err(_) => source,
            }
        } else {
            source
        };
        let t_classify = phase_profile::enabled().then(std::time::Instant::now);
        let is_bundle_runtime =
            source.contains("__nccwpck_require__") || source.contains("__webpack_require__");
        let has_cjs_markers = source_has_cjs_export_markers(&source) || is_bundle_runtime;
        let has_esm_markers = source_has_esm_markers(&source);
        let package_kind = detect_module_kind(resolved);
        let graph_forced_esm =
            !has_cjs_markers && !has_esm_markers && package_kind == ModuleKind::ESM;
        if let Some(t) = t_classify {
            phase_profile::add(
                &phase_profile::PREFLIGHT_CLASSIFY_NS,
                t.elapsed().as_nanos() as u64,
            );
        }

        if !(has_esm_markers || graph_forced_esm) || is_bundle_runtime {
            return Ok(());
        }
        let t_parse = phase_profile::enabled().then(std::time::Instant::now);
        let ast = rusty_js_parser::parse_module_goal(&source).map_err(|e| {
            RuntimeError::SyntaxError(format_public_parse_error(
                &source,
                &e,
                resolved,
                "",
                source.len(),
            ))
        })?;
        if let Some(t) = t_parse {
            phase_profile::add(
                &phase_profile::PREFLIGHT_PARSE_NS,
                t.elapsed().as_nanos() as u64,
            );
        }
        self.preflight_static_module_requests_source_order(resolved, &ast, seen)
    }

    pub fn run_repl_script(&mut self, source: &str, url: &str) -> Result<Value, RuntimeError> {
        let compiled = rusty_js_bytecode::compile_repl_with_url(source, url).map_err(|e| {
            RuntimeError::CompileError(format!("compile: {} @url={}", e.message, url))
        })?;
        self.current_module_url.push(url.to_string());
        let result = self.run_module(&compiled);
        self.current_module_url.pop();
        result
    }

    pub fn run_script(&mut self, source: &str, url: &str) -> Result<Value, RuntimeError> {
        if !self.engine_helpers.contains_key("__script_var_global_bind") {
            self.install_intrinsics();
        }
        let compiled =
            rusty_js_bytecode::compile_script_goal_with_url(source, url).map_err(|e| {
                RuntimeError::CompileError(format!("compile: {} @url={}", e.message, url))
            })?;

        crate::intrinsics::eval_global_declaration_instantiation_guard(self, source, true)?;

        if compiled.eval_var_env_is_global {
            let tdz = Value::Symbol(std::rc::Rc::clone(&self.tdz_sentinel));
            for desc in compiled.locals.iter() {
                if desc.top_level_lexical && !desc.name.starts_with("<scoped@") {
                    let name = crate::interp::Runtime::direct_eval_binding_name(&desc.name);
                    self.global_lexical_bindings
                        .entry(name.clone())
                        .or_insert_with(|| tdz.clone());
                    if matches!(desc.kind, rusty_js_ast::VariableKind::Const) {
                        self.global_immutable_lexical_bindings.insert(name);
                    }
                }
            }
        }

        self.current_module_url.push(url.to_string());
        let result = self.run_module(&compiled);
        self.current_module_url.pop();
        result
    }

    pub fn evaluate_module_in_realm(
        &mut self,
        source: &str,
        url: &str,
        realm_idx: usize,
    ) -> Result<ObjectRef, RuntimeError> {
        let prior = self.enter_realm(realm_idx);
        let result = self.evaluate_module(source, url);
        self.exit_realm(prior);
        result
    }

    fn compartment_for_module_url(&mut self, url: &str) -> Option<(String, String)> {
        let path = url.strip_prefix("file://")?;
        if self.module_map_bridge.is_none() {
            self.module_map_bridge = Some(discover_and_load_bridge(std::path::Path::new(path)));
        }
        let bridge = self.module_map_bridge.as_ref()?;
        if bridge.is_empty() {
            return None;
        }
        bridge.package_for_path(path)
    }

    pub fn register_module_integrity(&mut self, url: &str, sri: &str) {
        self.module_integrity_expectations
            .insert(url.to_string(), sri.to_string());
    }

    pub fn compute_sri_sha512(bytes: &[u8]) -> String {
        let digest = rusty_web_crypto::digest_sha512(bytes);
        format!("sha512-{}", rusty_js_basen::encode_base64(digest.as_ref()))
    }

    pub fn verify_module_integrity(&self, url: &str, source: &str) -> Result<(), RuntimeError> {
        let expected = match self.module_integrity_expectations.get(url) {
            Some(e) => e,
            None => return Ok(()),
        };

        let actual_sri = Self::compute_sri_sha512(source.as_bytes());

        if actual_sri == *expected {
            Ok(())
        } else {
            Err(RuntimeError::TypeError(format!(
                "Integrity check failed for module {url}: recorded SRI {expected} \
                 does not match sha-512 of the loaded source ({actual_sri})"
            )))
        }
    }

    pub fn evaluate_module(&mut self, source: &str, url: &str) -> Result<ObjectRef, RuntimeError> {

        self.verify_module_integrity(url, source)?;
        let _prof = phase_profile::enabled();

        let t0 = if _prof {
            Some(std::time::Instant::now())
        } else {
            None
        };

        let parse_goal = self.pending_parse_goal;

        self.pending_parse_goal = None;
        let is_direct_eval = std::mem::replace(&mut self.pending_eval_is_direct, false);
        let publish_global_lexicals =
            std::mem::replace(&mut self.pending_publish_global_lexicals, false);
        let script_mode = std::mem::replace(&mut self.pending_script_mode, false);
        let script_global_env_alias =
            std::mem::replace(&mut self.pending_script_global_env_alias, false);
        let force_strict = std::mem::replace(&mut self.pending_eval_force_strict, false);
        let script_var_deletable = std::mem::replace(&mut self.pending_eval_var_deletable, false);
        let direct_eval_super_context = self.pending_direct_eval_super_context.take();
        let eval_outer_locals_vals = std::mem::take(&mut self.pending_eval_outer_locals);
        let eval_outer_const_locals = std::mem::take(&mut self.pending_eval_outer_const_locals);
        let eval_outer_local_cells = std::mem::take(&mut self.pending_eval_outer_local_cells);
        let eval_outer_captured_bindings =
            std::mem::take(&mut self.pending_eval_outer_captured_bindings);
        let eval_with_env_stack = std::mem::take(&mut self.pending_eval_with_env_stack);
        let eval_with_active_with_count =
            std::mem::replace(&mut self.pending_eval_with_active_with_count, 0);
        let ast = match parse_goal {
            Some(true) => rusty_js_parser::parse_module_goal(source),
            Some(false) => rusty_js_parser::parse_script(source),
            None => rusty_js_parser::parse_module(source),
        }
        .map_err(|e| {

            RuntimeError::SyntaxError(format_public_parse_error(source, &e, url, "", source.len()))
        })?;

        if !is_direct_eval {
            rusty_js_parser::private_names_valid::validate_all_private_names(&ast, source)
                .map_err(|e| {
                    RuntimeError::CompileError(format!(
                        "parse: {} @byte{} @url={}",
                        e.message, e.span.start, url
                    ))
                })?;
        }
        if let Some(t) = t0 {
            phase_profile::add(&phase_profile::PARSE_NS, t.elapsed().as_nanos() as u64);
        }
        let ast_rc = Rc::new(ast);
        let t1 = if _prof {
            Some(std::time::Instant::now())
        } else {
            None
        };

        let eval_outer_local_names: Vec<String> = eval_outer_locals_vals
            .iter()
            .map(|(n, _)| n.clone())
            .collect();
        let eval_outer_const_local_names: Vec<String> =
            eval_outer_const_locals.into_iter().collect();
        let bytecode = if script_mode {
            rusty_js_bytecode::compile_script_goal_with_url_and_super_context_force_strict(
                source,
                url,
                direct_eval_super_context,
                force_strict,
                &eval_outer_local_names,
                script_var_deletable,
                script_global_env_alias,
                eval_with_env_stack.len() as u32,
            )
        } else {

            match parse_goal {
                Some(module_goal) => rusty_js_bytecode::compile_module_goal_with_url_force_strict(
                    source,
                    url,
                    force_strict,
                    module_goal,
                ),
                None => rusty_js_bytecode::compile_module_with_url_force_strict(
                    source,
                    url,
                    force_strict,
                ),
            }
        }

        .map_err(|e| RuntimeError::SyntaxError(format!("compile: {}", e.message)))?;
        if let Some(t) = t1 {
            phase_profile::add(&phase_profile::COMPILE_NS, t.elapsed().as_nanos() as u64);
        }
        if _prof {
            phase_profile::MODULE_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        let bytecode_rc = Rc::new(bytecode);

        if bytecode_rc.eval_var_env_is_global && publish_global_lexicals {
            let tdz = Value::Symbol(std::rc::Rc::clone(&self.tdz_sentinel));
            for desc in bytecode_rc.locals.iter() {
                if desc.top_level_lexical && !desc.name.starts_with("<scoped@") {
                    let name = crate::interp::Runtime::direct_eval_binding_name(&desc.name);
                    self.global_lexical_bindings
                        .entry(name.clone())
                        .or_insert_with(|| tdz.clone());
                    if matches!(desc.kind, rusty_js_ast::VariableKind::Const) {
                        self.global_immutable_lexical_bindings.insert(name);
                    }
                }
            }
        }

        let namespace = self.alloc_object(Object::new_module_namespace());
        self.module_namespace_urls
            .insert(namespace, url.to_string());
        let record = Rc::new(RefCell::new(ModuleRecord {
            url: url.to_string(),
            status: ModuleStatus::Linking,
            ast: ast_rc.clone(),
            bytecode: bytecode_rc.clone(),
            namespace: Some(namespace),
            eval_error: None,
            kind: ModuleKind::ESM,
            cjs_exports: None,
            export_cells: std::collections::HashMap::new(),
            async_static_deps: Vec::new(),
            body_completed_waiting_async_deps: false,
            pending_body_start: None,
            async_evaluation_order: None,
            async_cycle_root: None,
        }));
        self.module_insert(url.to_string(), record.clone());
        {
            let mut prelink_export_cells =
                std::collections::HashMap::<String, crate::value::UpvalueCell>::new();
            let mut cells_by_local =
                std::collections::HashMap::<u16, crate::value::UpvalueCell>::new();
            for eb in &bytecode_rc.exports {
                if let ExportBinding::Local { exported, local } = eb {
                    let cell = if let Some(cell) = cells_by_local.get(local).cloned() {
                        cell
                    } else {
                        let initial = bytecode_rc
                            .locals
                            .get(*local as usize)
                            .map(|desc| desc.kind)
                            .filter(|kind| !matches!(kind, rusty_js_ast::VariableKind::Var))
                            .map(|_| Value::Symbol(std::rc::Rc::clone(&self.tdz_sentinel)))
                            .unwrap_or(Value::Undefined);
                        let cell = std::rc::Rc::new(std::cell::RefCell::new(initial));
                        cells_by_local.insert(*local, cell.clone());
                        cell
                    };
                    prelink_export_cells.insert(exported.clone(), cell);
                }
            }
            record.borrow_mut().export_cells = prelink_export_cells;
        }

        let mut frame = Frame::new_module(&bytecode_rc);

        frame.this_value = self
            .pending_eval_this
            .clone()
            .unwrap_or_else(|| self.current_this.clone());
        frame.this_cell = self.pending_eval_this_cell.clone();
        frame.derived_initial_this = self.pending_eval_derived_initial_this.clone();
        frame.new_target = self.pending_eval_new_target.clone();
        frame.private_home = self.pending_eval_private_home;
        frame.captured_with_env_count = eval_with_env_stack
            .len()
            .saturating_sub(eval_with_active_with_count);
        frame.with_env_stack = eval_with_env_stack;

        frame.source_url = url;

        let meta_obj = self.alloc_object_with_explicit_null_proto(Object::new_ordinary());
        self.object_set(
            meta_obj,
            "url".to_string(),
            Value::String(Rc::new(crate::value::JsString::from(url.to_string()))),
        );
        let dir_str = {
            let path = url.strip_prefix("file://").unwrap_or(url);
            let p = std::path::Path::new(path);
            p.parent()
                .map(|d| d.display().to_string())
                .unwrap_or_default()
        };
        self.object_set(
            meta_obj,
            "dir".to_string(),
            Value::String(Rc::new(crate::value::JsString::from(dir_str.clone()))),
        );

        if let Some(path) = url.strip_prefix("file://") {
            self.object_set(
                meta_obj,
                "filename".to_string(),
                Value::String(Rc::new(crate::value::JsString::from(path.to_string()))),
            );
            self.object_set(
                meta_obj,
                "dirname".to_string(),
                Value::String(Rc::new(crate::value::JsString::from(dir_str))),
            );
        }

        let import_meta_parent_url = url.to_string();
        let import_meta_resolve_fn: crate::value::NativeFn = Rc::new(move |rt, args| {
            let spec = match args.first() {
                Some(v) => rt.coerce_to_string(v)?,
                None => crate::abstract_ops::to_string(&Value::Undefined)
                    .as_str()
                    .to_string(),
            };
            match rt.resolve_module_full(&import_meta_parent_url, &spec, ModuleKind::ESM) {
                Ok(resolved) => Ok(Value::String(Rc::new(crate::value::JsString::from(
                    resolved,
                )))),
                Err(e) => Err(RuntimeError::TypeError(format!(
                    "Cannot resolve module \"{}\": {:?}",
                    spec, e
                ))),
            }
        });
        let mut resolve_props = indexmap::IndexMap::new();
        crate::value::install_function_meta_props(&mut resolve_props, "resolve", 1.0);
        let import_meta_resolve_obj = Object {
            proto: None,
            extensible: true,
            properties: resolve_props,
            internal_kind: crate::value::InternalKind::Function(Box::new(
                crate::value::FunctionInternals {
                    name: "resolve".to_string(),
                    length: 1,
                    native: import_meta_resolve_fn,
                    is_constructor: false,
                    creation_realm: 0,
                    roots: Vec::new(),
                },
            )),

            ..Default::default()
        };
        let import_meta_resolve_id = self.alloc_object(import_meta_resolve_obj);
        self.object_set(
            meta_obj,
            "resolve".to_string(),
            Value::Object(import_meta_resolve_id),
        );
        frame.import_meta = Some(meta_obj);
        for init in &bytecode_rc.module_hoisted_functions {
            let (proto, captures) = match bytecode_rc.constants.get(init.function_const) {
                Some(Constant::Function(p)) => (p.clone(), init.captures.as_slice()),
                Some(Constant::LazyFunction(lazy)) => {
                    let captures = if lazy.captures().is_empty() {
                        init.captures.as_slice()
                    } else {
                        lazy.captures()
                    };
                    (lazy.proto_rc(), captures)
                }
                _ => continue,
            };
            let closure_id = self.make_closure_object_from_proto(&mut frame, proto, false);
            let mut upvalues = Vec::with_capacity(captures.len());
            for capture in captures {
                match capture.source {
                    UpvalueSource::Local(slot) => {
                        upvalues.push(frame.promote_local(slot as usize));
                    }
                    UpvalueSource::Upvalue(idx) => {
                        if let Some(cell) = frame.upvalues.get(idx as usize) {
                            upvalues.push(cell.clone());
                        }
                    }
                }
            }
            if let InternalKind::Closure(c) = &mut self.obj_mut(closure_id).internal_kind {
                c.upvalues = upvalues;
            }
            let value = Value::Object(closure_id);
            frame.write_local(init.slot as usize, value.clone());
            for eb in &bytecode_rc.exports {
                if let ExportBinding::Local { exported, local } = eb {
                    if *local == init.slot {
                        if let Some(cell) = record.borrow().export_cells.get(exported).cloned() {
                            *cell.borrow_mut() = value.clone();
                        }
                    }
                }
            }
        }

        let outermost_module_eval = self.module_evaluation_depth == 0;
        self.module_evaluation_depth += 1;

        let mut reexport_namespaces: HashMap<String, ObjectRef> = HashMap::new();

        let mut preflight_seen = std::collections::HashSet::new();
        let _static_dep_parent_frame_roots = crate::interp::FrameRootGuard::push(&frame);
        let t_preflight = if phase_profile::enabled() {
            Some(std::time::Instant::now())
        } else {
            None
        };
        self.preflight_static_module_requests_source_order(url, &ast_rc, &mut preflight_seen)?;
        if let Some(t) = t_preflight {
            phase_profile::add(&phase_profile::PREFLIGHT_NS, t.elapsed().as_nanos() as u64);
        }

        if outermost_module_eval && !self.module_scc_stack.is_empty() {
            self.module_scc_stack.clear();
            self.module_scc_on_stack.clear();
        }
        let dfs_is_root_visit = !self.module_dfs_index.contains_key(url);
        if dfs_is_root_visit {
            let idx = self.next_module_dfs_index;
            self.next_module_dfs_index = self.next_module_dfs_index.saturating_add(1);
            self.module_dfs_index.insert(url.to_string(), idx);
            self.module_dfs_lowlink.insert(url.to_string(), idx);
            self.module_scc_stack.push(url.to_string());
            self.module_scc_on_stack.insert(url.to_string());
        }
        let mut evaluating_static_deps: Vec<String> = Vec::new();

        let mut has_deferred_async_dep = false;
        let t_static_deps = if phase_profile::enabled() {
            Some(std::time::Instant::now())
        } else {
            None
        };
        for item in &ast_rc.body {
            let (spec, import_defer, is_import_declaration, type_attr) = match item {
                rusty_js_ast::ModuleItem::Import(imp) => {
                    if imp.source_phase {
                        continue;
                    }
                    let type_attr = imp.attributes.iter().find_map(|a| {
                        let k = match &a.key {
                            rusty_js_ast::ModuleExportName::Ident(b) => b.name.as_str(),
                            rusty_js_ast::ModuleExportName::String { value, .. } => value.as_str(),
                        };
                        (k == "type").then(|| a.value.clone())
                    });
                    (&imp.specifier.value, imp.import_defer, true, type_attr)
                }
                rusty_js_ast::ModuleItem::Export(export) => match export {
                    rusty_js_ast::ExportDeclaration::Named {
                        source: Some(source),
                        attributes,
                        ..
                    }
                    | rusty_js_ast::ExportDeclaration::StarFrom {
                        source, attributes, ..
                    }
                    | rusty_js_ast::ExportDeclaration::StarAsFrom {
                        source, attributes, ..
                    } => {
                        let type_attr = attributes.iter().find_map(|a| {
                            let k = match &a.key {
                                rusty_js_ast::ModuleExportName::Ident(b) => b.name.as_str(),
                                rusty_js_ast::ModuleExportName::String { value, .. } => {
                                    value.as_str()
                                }
                            };
                            (k == "type").then(|| a.value.clone())
                        });
                        (&source.value, false, false, type_attr)
                    }
                    _ => continue,
                },
                _ => continue,
            };
            let t_static_dep_resolve = if phase_profile::enabled() {
                Some(std::time::Instant::now())
            } else {
                None
            };
            let resolved = self.resolve_module_full(url, spec, ModuleKind::ESM)?;
            if let Some(t) = t_static_dep_resolve {
                phase_profile::add(
                    &phase_profile::STATIC_DEP_RESOLVE_NS,
                    t.elapsed().as_nanos() as u64,
                );
            }
            if phase_profile::enabled() {
                phase_profile::inc(&phase_profile::STATIC_DEP_EDGES);
                if is_import_declaration {
                    phase_profile::inc(&phase_profile::STATIC_DEP_IMPORT_EDGES);
                } else {
                    phase_profile::inc(&phase_profile::STATIC_DEP_REEXPORT_EDGES);
                }
            }
            if import_defer {
                self.validate_deferred_module_load(&resolved)?;
                let deferred_async_deps =
                    self.evaluate_deferred_async_dependency_urls(&resolved)?;
                if !deferred_async_deps.is_empty() {
                    has_deferred_async_dep = true;
                    for dep in deferred_async_deps {
                        if !evaluating_static_deps.iter().any(|u| u == &dep) {
                            evaluating_static_deps.push(dep);
                        }
                    }
                }
                continue;
            }
            if resolved.starts_with("node:")
                || resolved.starts_with("cruft:")
                || resolved.starts_with("bun:") || resolved.starts_with("deno:")
            {
                let _parent_frame_roots = self.push_frame_temporary_roots(&frame);
                let ns = self.resolve_builtin_namespace(&resolved)?;
                if matches!(item, rusty_js_ast::ModuleItem::Export(_)) {
                    reexport_namespaces.insert(spec.clone(), ns);
                }
            } else {
                if resolved.ends_with(".json")
                    && !matches!(
                        type_attr.as_deref(),
                        Some("json") | Some("bytes") | Some("text")
                    )
                {
                    return Err(json_import_attribute_error(&resolved));
                }

                let dep_visited_before = self.module_dfs_index.contains_key(&resolved);
                if dep_visited_before && phase_profile::enabled() {
                    phase_profile::inc(&phase_profile::STATIC_DEP_VISITED_BEFORE);
                }
                let dep_status_for_profile = if phase_profile::enabled() {
                    Some(self.module_get(&resolved).map(|rec| rec.borrow().status))
                } else {
                    None
                };
                let prof_static_dep_load_status = |status: Option<Option<ModuleStatus>>| {
                    if !phase_profile::enabled() {
                        return;
                    }
                    match status {
                        Some(None) => phase_profile::inc(&phase_profile::STATIC_DEP_LOAD_NEW),
                        Some(Some(ModuleStatus::Linking)) => {
                            phase_profile::inc(&phase_profile::STATIC_DEP_LOAD_EXISTING_LINKING)
                        }
                        Some(Some(ModuleStatus::Evaluating)) => {
                            phase_profile::inc(&phase_profile::STATIC_DEP_LOAD_EXISTING_EVALUATING)
                        }
                        Some(Some(ModuleStatus::Evaluated)) => {
                            phase_profile::inc(&phase_profile::STATIC_DEP_LOAD_EXISTING_EVALUATED)
                        }
                        Some(Some(ModuleStatus::Failed)) => {
                            phase_profile::inc(&phase_profile::STATIC_DEP_LOAD_EXISTING_FAILED)
                        }
                        Some(Some(_)) => {
                            phase_profile::inc(&phase_profile::STATIC_DEP_LOAD_EXISTING_OTHER)
                        }
                        None => {}
                    }
                };
                let ns = match type_attr.as_deref() {
                    Some("json") => {
                        let _parent_frame_roots = self.push_frame_temporary_roots(&frame);
                        if phase_profile::enabled() {
                            phase_profile::inc(&phase_profile::STATIC_DEP_LOAD_CALLS);
                        }
                        prof_static_dep_load_status(dep_status_for_profile);
                        let t_static_dep_load = if phase_profile::enabled() {
                            Some((
                                std::time::Instant::now(),
                                phase_profile::read(&phase_profile::STATIC_DEPS_NS),
                            ))
                        } else {
                            None
                        };
                        let ns = self.load_module_as(&resolved, ModuleKind::ESM)?;
                        if let Some((t, nested_static_deps_before)) = t_static_dep_load {
                            let elapsed = t.elapsed().as_nanos() as u64;
                            let nested_static_deps_delta =
                                phase_profile::read(&phase_profile::STATIC_DEPS_NS)
                                    .saturating_sub(nested_static_deps_before);
                            maybe_report_static_dep_load_profile(
                                url,
                                spec,
                                &resolved,
                                type_attr.as_deref(),
                                elapsed,
                                nested_static_deps_delta,
                            );
                            let exclusive = elapsed.saturating_sub(nested_static_deps_delta);
                            phase_profile::add(&phase_profile::STATIC_DEP_LOAD_NS, elapsed);
                            phase_profile::add(
                                &phase_profile::STATIC_DEP_LOAD_EXCLUSIVE_NS,
                                exclusive,
                            );
                            if phase_profile::static_dep_load_profile_enabled()
                                && static_dep_load_profile_matches(url, spec, &resolved)
                            {
                                phase_profile::record_static_dep_load_profile(
                                    static_dep_load_profile_label(&resolved),
                                    elapsed,
                                    exclusive,
                                );
                            }
                        }
                        ns
                    }
                    Some("text") => {
                        let _parent_frame_roots = self.push_frame_temporary_roots(&frame);
                        self.load_text_module(&resolved)?
                    }
                    Some("bytes") => {
                        let _parent_frame_roots = self.push_frame_temporary_roots(&frame);
                        self.load_bytes_module(&resolved)?
                    }
                    _ => {
                        let _parent_frame_roots = self.push_frame_temporary_roots(&frame);
                        if phase_profile::enabled() {
                            phase_profile::inc(&phase_profile::STATIC_DEP_LOAD_CALLS);
                        }
                        prof_static_dep_load_status(dep_status_for_profile);
                        let t_static_dep_load = if phase_profile::enabled() {
                            Some((
                                std::time::Instant::now(),
                                phase_profile::read(&phase_profile::STATIC_DEPS_NS),
                            ))
                        } else {
                            None
                        };
                        let ns = self.load_module_as(&resolved, ModuleKind::ESM)?;
                        if let Some((t, nested_static_deps_before)) = t_static_dep_load {
                            let elapsed = t.elapsed().as_nanos() as u64;
                            let nested_static_deps_delta =
                                phase_profile::read(&phase_profile::STATIC_DEPS_NS)
                                    .saturating_sub(nested_static_deps_before);
                            maybe_report_static_dep_load_profile(
                                url,
                                spec,
                                &resolved,
                                type_attr.as_deref(),
                                elapsed,
                                nested_static_deps_delta,
                            );
                            let exclusive = elapsed.saturating_sub(nested_static_deps_delta);
                            phase_profile::add(&phase_profile::STATIC_DEP_LOAD_NS, elapsed);
                            phase_profile::add(
                                &phase_profile::STATIC_DEP_LOAD_EXCLUSIVE_NS,
                                exclusive,
                            );
                            if phase_profile::static_dep_load_profile_enabled()
                                && static_dep_load_profile_matches(url, spec, &resolved)
                            {
                                phase_profile::record_static_dep_load_profile(
                                    static_dep_load_profile_label(&resolved),
                                    elapsed,
                                    exclusive,
                                );
                            }
                        }
                        ns
                    }
                };
                let t_static_dep_post_load = if phase_profile::enabled() {
                    Some(std::time::Instant::now())
                } else {
                    None
                };
                if matches!(item, rusty_js_ast::ModuleItem::Export(_)) {
                    reexport_namespaces.insert(spec.clone(), ns);
                }
                if !is_import_declaration {
                    if let Some(t) = t_static_dep_post_load {
                        phase_profile::add(
                            &phase_profile::STATIC_DEP_POST_LOAD_NS,
                            t.elapsed().as_nanos() as u64,
                        );
                    }
                    continue;
                }

                {
                    let dep_on_stack = self.module_scc_on_stack.contains(&resolved);
                    let relax = if !dep_visited_before {
                        self.module_dfs_lowlink.get(&resolved).copied()
                    } else if dep_on_stack {
                        self.module_dfs_index.get(&resolved).copied()
                    } else {
                        None
                    };
                    if let (Some(cur), Some(r)) = (self.module_dfs_lowlink.get(url).copied(), relax)
                    {
                        if r < cur {
                            self.module_dfs_lowlink.insert(url.to_string(), r);
                        }
                    }
                }

                let collapsed_cycle_backedge = self
                    .module_get(&resolved)
                    .map(|rec| {
                        let mut r = rec.borrow_mut();
                        let before = r.async_static_deps.len();
                        r.async_static_deps.retain(|dep| dep != url);
                        let collapsed = before != r.async_static_deps.len();
                        if collapsed && phase_profile::enabled() {
                            phase_profile::inc(&phase_profile::STATIC_DEP_CYCLE_COLLAPSES);
                        }
                        collapsed
                    })
                    .unwrap_or(false);
                let wait_resolved = if collapsed_cycle_backedge {
                    resolved.clone()
                } else {
                    self.module_get(&resolved)
                        .and_then(|rec| rec.borrow().async_cycle_root.clone())
                        .unwrap_or_else(|| resolved.clone())
                };
                let dependency_requires_async_wait = self
                    .module_get(&wait_resolved)
                    .map(|rec| {
                        if phase_profile::enabled() {
                            phase_profile::inc(&phase_profile::STATIC_DEP_WAIT_CHECKS);
                        }
                        let r = rec.borrow();
                        matches!(
                            r.status,
                            ModuleStatus::Evaluating | ModuleStatus::EvaluatingAsync
                        ) || (matches!(r.status, ModuleStatus::Linking)
                            && (r.pending_body_start.is_some()
                                || r.body_completed_waiting_async_deps
                                || !r.async_static_deps.is_empty()
                                || r.async_evaluation_order.is_some()))
                    })
                    .unwrap_or(false);
                if wait_resolved != url && dependency_requires_async_wait {
                    if phase_profile::enabled() {
                        phase_profile::inc(&phase_profile::STATIC_DEP_WAIT_PUSHES);
                    }
                    evaluating_static_deps.push(wait_resolved);
                }
                if let Some(t) = t_static_dep_post_load {
                    phase_profile::add(
                        &phase_profile::STATIC_DEP_POST_LOAD_NS,
                        t.elapsed().as_nanos() as u64,
                    );
                }
            }
        }
        if let Some(t) = t_static_deps {
            phase_profile::add(
                &phase_profile::STATIC_DEPS_NS,
                t.elapsed().as_nanos() as u64,
            );
        }

        if dfs_is_root_visit {
            let idx = self.module_dfs_index.get(url).copied();
            let lowlink = self.module_dfs_lowlink.get(url).copied();
            if idx.is_some() && idx == lowlink {
                while let Some(member) = self.module_scc_stack.pop() {
                    self.module_scc_on_stack.remove(&member);
                    if let Some(rec) = self.module_get(&member) {
                        rec.borrow_mut().async_cycle_root = Some(url.to_string());
                    }
                    if member == *url {
                        break;
                    }
                }
            }
        }
        self.module_evaluation_depth = self.module_evaluation_depth.saturating_sub(1);

        let indirect_exports: Vec<(String, String)> = ast_rc
            .indirect_export_entries
            .iter()
            .filter_map(|e| {
                let export_name = e.export_name.clone()?;
                let req = e.module_request.clone()?;
                Some((export_name, req))
            })
            .collect();
        for (export_name, req) in &indirect_exports {
            let resolved = self.resolve_module_full(url, req, ModuleKind::ESM)?;
            if resolved.starts_with("node:")
                || resolved.starts_with("cruft:")
                || resolved.starts_with("bun:") || resolved.starts_with("deno:")
            {
                continue;
            }
            let mut rset = std::collections::HashSet::new();
            match self.module_resolve_export(url, export_name, &mut rset) {

                ResolveExportResult::Resolved { .. } | ResolveExportResult::Incomplete => {}
                ResolveExportResult::NotFound => {
                    return Err(RuntimeError::SyntaxError(format!(
                        "The requested module '{}' does not provide an export named '{}'",
                        req, export_name
                    )));
                }
                ResolveExportResult::Ambiguous => {
                    return Err(RuntimeError::SyntaxError(format!(
                        "The requested module '{}' contains conflicting star exports for name '{}'",
                        req, export_name
                    )));
                }
            }
        }

        let mut import_values: Vec<(
            u16,
            Value,
            Option<crate::value::UpvalueCell>,
            Option<(String, bool, ImportBindingKind)>,
        )> = Vec::with_capacity(bytecode_rc.imports.len());
        let t_import_bindings = if phase_profile::enabled() {
            Some(std::time::Instant::now())
        } else {
            None
        };
        for ib in &bytecode_rc.imports {
            if matches!(ib.kind, ImportBindingKind::Source)
                && ib.module_request == "<module source>"
            {
                let source =
                    self.make_abstract_module_source_object(&ib.module_request, &ib.module_request);
                import_values.push((ib.slot, Value::Object(source), None, None));
                continue;
            }
            let resolved = self.resolve_module_full(url, &ib.module_request, ModuleKind::ESM)?;
            if matches!(ib.kind, ImportBindingKind::Source) {

                let is_builtin = resolved.starts_with("node:")
                    || resolved.starts_with("cruft:")
                    || resolved.starts_with("bun:") || resolved.starts_with("deno:");
                if is_builtin {
                    self.resolve_builtin_namespace(&resolved)?;
                } else {
                    self.load_module_as(&resolved, ModuleKind::ESM)?;
                }
                return Err(RuntimeError::SyntaxError(format!(
                    "source phase import unavailable for '{}'",
                    resolved
                )));
            }

            if matches!(ib.kind, ImportBindingKind::DeferredNamespace) {
                let dns = self.make_deferred_namespace(url, &resolved)?;
                import_values.push((ib.slot, Value::Object(dns), None, None));
                continue;
            }

            if matches!(ib.type_attr.as_deref(), Some("text") | Some("bytes")) {
                let ns_obj = if ib.type_attr.as_deref() == Some("bytes") {
                    self.load_bytes_module(&resolved)?
                } else {
                    self.load_text_module(&resolved)?
                };
                let v = match &ib.kind {
                    ImportBindingKind::Namespace => Value::Object(ns_obj),
                    _ => self.object_get(ns_obj, "default"),
                };
                import_values.push((ib.slot, v, None, None));
                continue;
            }
            if resolved.ends_with(".json") && ib.type_attr.as_deref() != Some("json") {
                return Err(json_import_attribute_error(&resolved));
            }
            let is_builtin = resolved.starts_with("node:")
                || resolved.starts_with("cruft:")
                || resolved.starts_with("bun:") || resolved.starts_with("deno:");
            let ns = if is_builtin {
                self.resolve_builtin_namespace(&resolved)?
            } else {
                self.load_module_as(&resolved, ModuleKind::ESM)?
            };

            let needs_live_binding = !is_builtin && {
                match self.module_get(&resolved) {
                    Some(rec) => !matches!(rec.borrow().status, ModuleStatus::Evaluated),
                    None => false,
                }
            };

            let is_cruftscript = resolved.ends_with(".fts");
            let cjs_raw = if is_builtin
                || is_cruftscript
                || self.module_kind_of(&resolved) == Some(ModuleKind::ESM)
            {
                None
            } else {
                self.cjs_exports_of(&resolved)
            };
            let mut v = match (&ib.kind, &cjs_raw) {
                (ImportBindingKind::Source, _) => Value::Object(
                    self.make_abstract_module_source_object(&ib.module_request, &resolved),
                ),
                (ImportBindingKind::Default, Some(raw)) => raw.clone(),
                (
                    ImportBindingKind::Namespace | ImportBindingKind::DeferredNamespace,
                    Some(raw),
                ) => Value::Object(self.cjs_namespace_view_at(raw.clone(), Some(&resolved))),
                (ImportBindingKind::Named(n), Some(raw)) if n == "default" => raw.clone(),
                (ImportBindingKind::Named(n), Some(raw)) => {
                    let cjs_keys = self.cjs_static_export_keys_for_url(&resolved);
                    if cjs_keys
                        .as_ref()
                        .is_some_and(|keys| !keys.iter().any(|k| k == n))
                    {

                        let has = |nm: &str| {
                            cjs_keys
                                .as_ref()
                                .map(|k| k.iter().any(|kk| kk == nm))
                                .unwrap_or(false)
                        };
                        let named: Vec<String> = bytecode_rc
                            .imports
                            .iter()
                            .filter(|o| o.module_request == ib.module_request)
                            .filter_map(|o| match &o.kind {
                                ImportBindingKind::Named(nm) if nm != "default" => Some(nm.clone()),
                                _ => None,
                            })
                            .collect();
                        let mut missing: Vec<&String> =
                            named.iter().filter(|nm| !has(nm)).collect();
                        missing.sort();
                        let first = missing.first().map(|s| s.as_str()).unwrap_or(n.as_str());
                        let req = &ib.module_request;
                        let list = if named.is_empty() {
                            n.clone()
                        } else {
                            named.join(", ")
                        };
                        return Err(RuntimeError::SyntaxError(format!(
                            "Named export '{first}' not found. The requested module '{req}' is a CommonJS module, which may not support all module.exports as named exports.\nCommonJS modules can always be imported via the default export, for example using:\n\nimport pkg from '{req}';\nconst {{ {list} }} = pkg;"
                        )));
                    }
                    match raw {

                        Value::Object(_) => self
                            .resolve_cjs_named_export_value(&resolved, raw, n)
                            .unwrap_or(Value::Undefined),
                        _ => Value::Undefined,
                    }
                }
                (ImportBindingKind::Default, None) => {
                    if is_cruftscript {
                        let d = self.object_get(ns, "default");
                        if matches!(d, Value::Undefined) {
                            return Err(RuntimeError::SyntaxError(format!(
                                "The requested CruftScript module '{}' does not provide a default export",
                                ib.module_request
                            )));
                        }
                        d
                    } else {

                        let mut resolved_namespace = None;
                        if !is_builtin {
                            let mut rset = std::collections::HashSet::new();
                            match self.module_resolve_export(&resolved, "default", &mut rset) {
                                ResolveExportResult::Resolved {
                                    module_url,
                                    binding_name,
                                } if binding_name == "*namespace*" => {
                                    resolved_namespace =
                                        Some(self.load_module_as(&module_url, ModuleKind::ESM)?);
                                }
                                ResolveExportResult::Resolved { .. }
                                | ResolveExportResult::Incomplete => {}
                                ResolveExportResult::NotFound => {
                                    return Err(RuntimeError::SyntaxError(format!(
                                    "The requested module '{}' does not provide an export named 'default'",
                                    ib.module_request
                                )));
                                }
                                ResolveExportResult::Ambiguous => {
                                    return Err(RuntimeError::SyntaxError(format!(
                                    "The requested module '{}' contains conflicting star exports for name 'default'",
                                    ib.module_request
                                )));
                                }
                            }
                        }
                        if let Some(resolved_ns) = resolved_namespace {
                            Value::Object(resolved_ns)
                        } else {
                            let d = self.object_get(ns, "default");
                            if is_builtin && matches!(d, Value::Undefined) {
                                Value::Object(ns)
                            } else {
                                d
                            }
                        }
                    }
                }
                (ImportBindingKind::Namespace, None) => Value::Object(ns),
                (ImportBindingKind::DeferredNamespace, None) => Value::Object(ns),
                (ImportBindingKind::Named(n), None) if is_builtin => {

                    if let Some(getter) = self.find_getter(ns, n) {
                        self.call_function(getter, Value::Object(ns), Vec::new())?
                    } else {
                        self.object_get(ns, n)
                    }
                }
                (ImportBindingKind::Named(n), None) if is_cruftscript => {
                    let v = if let Some(getter) = self.find_getter(ns, n) {
                        self.call_function(getter, Value::Object(ns), Vec::new())?
                    } else {
                        self.object_get(ns, n)
                    };
                    if matches!(v, Value::Undefined) {
                        return Err(RuntimeError::SyntaxError(format!(
                            "The requested CruftScript module '{}' does not provide an export named '{}'",
                            ib.module_request, n
                        )));
                    }
                    v
                }
                (ImportBindingKind::Named(n), None) => {

                    let mut rset = std::collections::HashSet::new();
                    match self.module_resolve_export(&resolved, n, &mut rset) {

                        ResolveExportResult::Resolved {
                            module_url,
                            binding_name,
                        } if binding_name == "*source*" => Value::Object(
                            self.make_abstract_module_source_object(&module_url, &module_url),
                        ),
                        ResolveExportResult::Resolved {
                            module_url,
                            binding_name,
                        } if binding_name == "*deferred-namespace*" => {

                            Value::Object(self.make_deferred_namespace(&resolved, &module_url)?)
                        }
                        ResolveExportResult::Resolved {
                            module_url,
                            binding_name,
                        } if binding_name == "*namespace*" => {
                            Value::Object(self.load_module_as(&module_url, ModuleKind::ESM)?)
                        }
                        ResolveExportResult::Resolved { .. } => {
                            if let Some(getter) = self.find_getter(ns, n) {
                                self.call_function(getter, Value::Object(ns), Vec::new())?
                            } else {
                                self.object_get(ns, n)
                            }
                        }
                        ResolveExportResult::Incomplete => {
                            if let Some(getter) = self.find_getter(ns, n) {
                                self.call_function(getter, Value::Object(ns), Vec::new())?
                            } else {
                                self.object_get(ns, n)
                            }
                        }
                        ResolveExportResult::NotFound => {
                            return Err(RuntimeError::SyntaxError(format!(
                                "The requested module '{}' does not provide an export named '{}'",
                                ib.module_request, n
                            )));
                        }
                        ResolveExportResult::Ambiguous => {
                            return Err(RuntimeError::SyntaxError(format!(
                                "The requested module '{}' contains conflicting star exports for name '{}'",
                                ib.module_request, n
                            )));
                        }
                    }
                }

                (ImportBindingKind::DeferredNamespace, _) => {
                    unreachable!("deferred namespace import is handled at the pre-load intercept")
                }
            };
            if needs_live_binding && matches!(v, Value::Undefined) {
                let exported_name = match &ib.kind {
                    ImportBindingKind::Default => Some("default"),
                    ImportBindingKind::Named(name) => Some(name.as_str()),
                    _ => None,
                };
                if let Some(exported_name) = exported_name {
                    if self
                        .module_export_local_kind(&resolved, exported_name)
                        .map(|kind| !matches!(kind, rusty_js_ast::VariableKind::Var))
                        .unwrap_or(false)
                    {
                        v = Value::Symbol(std::rc::Rc::clone(&self.tdz_sentinel));
                    }
                }
            }
            let deferred_meta = if needs_live_binding {
                let is_cjs = matches!(self.module_kind_of(&resolved), Some(ModuleKind::CJS));
                Some((resolved.clone(), is_cjs, ib.kind.clone()))
            } else {
                None
            };
            let live_cell = if !is_builtin && !is_cruftscript && cjs_raw.is_none() {
                match &ib.kind {
                    ImportBindingKind::Default => {
                        self.module_resolve_export_cell(&resolved, "default")
                    }
                    ImportBindingKind::Named(name) => {
                        self.module_resolve_export_cell(&resolved, name)
                    }
                    _ => None,
                }
            } else {
                None
            };
            import_values.push((ib.slot, v, live_cell, deferred_meta));
        }
        if let Some(t) = t_import_bindings {
            phase_profile::add(
                &phase_profile::IMPORT_BINDINGS_NS,
                t.elapsed().as_nanos() as u64,
            );
        }

        let mut export_cells: std::collections::HashMap<String, crate::value::UpvalueCell> =
            record.borrow().export_cells.clone();
        let mut cells_by_local = std::collections::HashMap::<u16, crate::value::UpvalueCell>::new();
        let t_export_cells = if phase_profile::enabled() {
            Some(std::time::Instant::now())
        } else {
            None
        };
        for eb in &bytecode_rc.exports {
            if let ExportBinding::Local { exported, local } = eb {
                let cell = if let Some(cell) = cells_by_local.get(local).cloned() {
                    cell
                } else if let Some(cell) = export_cells.get(exported).cloned() {
                    let slot = *local as usize;
                    while frame.locals.len() <= slot {
                        frame.locals.push(Value::Undefined);
                    }
                    while frame.local_cells.len() <= slot {
                        frame.local_cells.push(None);
                    }
                    let frame_initial = frame.locals.get(slot).cloned().unwrap_or(Value::Undefined);
                    if !matches!(frame_initial, Value::Undefined) {
                        *cell.borrow_mut() = frame_initial;
                    }
                    frame.locals[slot] = cell.borrow().clone();
                    frame.local_cells[slot] = Some(cell.clone());
                    cells_by_local.insert(*local, cell.clone());
                    cell
                } else {
                    let cell = frame.promote_local(*local as usize);
                    cells_by_local.insert(*local, cell.clone());
                    cell
                };
                if matches!(*cell.borrow(), Value::Undefined) {
                    if self
                        .module_export_local_kind(url, exported)
                        .map(|kind| !matches!(kind, rusty_js_ast::VariableKind::Var))
                        .unwrap_or(false)
                    {
                        *cell.borrow_mut() = Value::Symbol(std::rc::Rc::clone(&self.tdz_sentinel));
                    }
                }
                export_cells.insert(exported.clone(), cell);
            }
        }
        for eb in &bytecode_rc.exports {
            if let ExportBinding::StarAs {
                exported,
                source_specifier,
            } = eb
            {
                if let Some(src_ns) = reexport_namespaces.get(source_specifier) {
                    export_cells.insert(
                        exported.clone(),
                        std::rc::Rc::new(std::cell::RefCell::new(Value::Object(*src_ns))),
                    );
                }
            }
        }
        for eb in &bytecode_rc.exports {
            match eb {
                ExportBinding::Named {
                    exported,
                    source_specifier,
                    imported,
                } => {
                    if let Ok(resolved) =
                        self.resolve_module_full(url, source_specifier, ModuleKind::ESM)
                    {
                        if let Some(src_ns) = reexport_namespaces.get(source_specifier) {
                            let v = self.object_get(*src_ns, imported);
                            if !matches!(v, Value::Undefined) {
                                export_cells.insert(
                                    exported.clone(),
                                    std::rc::Rc::new(std::cell::RefCell::new(v)),
                                );
                                continue;
                            }
                        }
                        if let Some(source_cell) =
                            self.module_resolve_export_cell(&resolved, imported)
                        {
                            export_cells.insert(exported.clone(), source_cell);
                            continue;
                        }
                        let source_evaluated = self
                            .module_get(&resolved)
                            .map(|rec| matches!(rec.borrow().status, ModuleStatus::Evaluated))
                            .unwrap_or(false);
                        if source_evaluated {
                            if let Some(src_ns) = reexport_namespaces.get(source_specifier) {
                                let v = self.object_get(*src_ns, imported);
                                export_cells.insert(
                                    exported.clone(),
                                    std::rc::Rc::new(std::cell::RefCell::new(v)),
                                );
                            }
                        }
                    }
                }
                ExportBinding::Star { source_specifier } => {
                    let resolved = self
                        .resolve_module_full(url, source_specifier, ModuleKind::ESM)
                        .ok();
                    if let (Some(resolved), Some(src_ns)) = (
                        resolved.as_deref(),
                        reexport_namespaces.get(source_specifier),
                    ) {
                        let source_evaluated = self
                            .module_get(resolved)
                            .map(|rec| matches!(rec.borrow().status, ModuleStatus::Evaluated))
                            .unwrap_or(false);
                        let mut seen = std::collections::HashSet::new();
                        let mut keys = std::collections::HashSet::new();
                        let _ =
                            self.collect_module_export_names(url, resolved, &mut seen, &mut keys);
                        for key in keys {
                            if key == "default" {
                                continue;
                            }
                            let mut rset = std::collections::HashSet::new();
                            if matches!(
                                self.module_resolve_export(url, &key, &mut rset),
                                ResolveExportResult::Resolved { .. }
                                    | ResolveExportResult::Incomplete
                            ) {
                                if let Some(cell) = self.module_resolve_export_cell(url, &key) {
                                    export_cells.insert(key, cell);
                                } else if source_evaluated {
                                    let v = self.object_get(*src_ns, &key);
                                    export_cells
                                        .insert(key, std::rc::Rc::new(std::cell::RefCell::new(v)));
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        for eb in &bytecode_rc.exports {
            let ExportBinding::Named {
                exported,
                source_specifier,
                imported,
            } = eb
            else {
                continue;
            };
            let Ok(resolved) = self.resolve_module_full(url, source_specifier, ModuleKind::ESM)
            else {
                continue;
            };
            if resolved != url {
                continue;
            }
            if let Some(cell) = export_cells.get(imported).cloned() {
                export_cells.insert(exported.clone(), cell);
            }
        }
        record.borrow_mut().export_cells = export_cells.clone();
        if let Some(t) = t_export_cells {
            phase_profile::add(
                &phase_profile::EXPORT_CELLS_NS,
                t.elapsed().as_nanos() as u64,
            );
        }

        for (slot, v, live_cell, deferred) in &import_values {
            frame.write_local(*slot as usize, v.clone());
            if let Some(source_cell) = live_cell {
                let s = *slot as usize;
                while frame.locals.len() <= s {
                    frame.locals.push(Value::Undefined);
                }
                while frame.local_cells.len() <= s {
                    frame.local_cells.push(None);
                }
                frame.locals[s] = source_cell.borrow().clone();
                frame.local_cells[s] = Some(source_cell.clone());
                for eb in &bytecode_rc.exports {
                    if let ExportBinding::Local { exported, local } = eb {
                        if *local == *slot {
                            export_cells.insert(exported.clone(), source_cell.clone());
                        }
                    }
                }
                continue;
            }

            if let Some((source_url, is_cjs, kind)) = deferred {
                let export_name = match kind {
                    ImportBindingKind::Default => Some("default"),
                    ImportBindingKind::Named(name) => Some(name.as_str()),
                    _ => None,
                };
                if let Some(export_name) = export_name {
                    if !*is_cjs {
                        if let Some(source_cell) =
                            self.module_resolve_export_cell(source_url, export_name)
                        {
                            let s = *slot as usize;
                            while frame.locals.len() <= s {
                                frame.locals.push(Value::Undefined);
                            }
                            while frame.local_cells.len() <= s {
                                frame.local_cells.push(None);
                            }
                            frame.locals[s] = source_cell.borrow().clone();
                            frame.local_cells[s] = Some(source_cell);
                            for eb in &bytecode_rc.exports {
                                if let ExportBinding::Local { exported, local } = eb {
                                    if *local == *slot {
                                        if let Some(cell) =
                                            self.module_resolve_export_cell(source_url, export_name)
                                        {
                                            export_cells.insert(exported.clone(), cell);
                                        }
                                    }
                                }
                            }
                            continue;
                        }
                    }
                }
                let source_evaluated = self
                    .module_get(source_url)
                    .map(|rec| matches!(rec.borrow().status, ModuleStatus::Evaluated))
                    .unwrap_or(false);
                if source_evaluated {
                    continue;
                }
                let cell = frame.promote_local(*slot as usize);
                self.pending_live_bindings
                    .entry(source_url.clone())
                    .or_insert_with(Vec::new)
                    .push(DeferredImportBinding {
                        cell,
                        kind: kind.clone(),
                        is_cjs: *is_cjs,
                    });
            }
        }
        record.borrow_mut().export_cells = export_cells.clone();

        for (name, slot) in &bytecode_rc.eval_outer_locals {
            let s = *slot as usize;

            let logical_name = Runtime::direct_eval_binding_name(name);
            if let Some((_, cell)) = eval_outer_local_cells
                .iter()
                .find(|(n, _)| Runtime::direct_eval_binding_name(n) == logical_name)
            {
                while frame.locals.len() <= s {
                    frame.locals.push(crate::value::Value::Undefined);
                }
                while frame.local_cells.len() <= s {
                    frame.local_cells.push(None);
                }
                while frame.local_captured_bindings.len() <= s {
                    frame.local_captured_bindings.push(None);
                }
                frame.locals[s] = cell.borrow().clone();
                frame.local_cells[s] = Some(cell.clone());
                let captured_binding = if eval_outer_const_local_names
                    .iter()
                    .any(|n| Runtime::direct_eval_binding_name(n) == logical_name)
                {
                    CapturedBinding::ImmutableSelfName {
                        name: logical_name.clone(),
                        cell: cell.clone(),
                    }
                } else {
                    eval_outer_captured_bindings
                        .iter()
                        .find(|(n, _)| Runtime::direct_eval_binding_name(n) == logical_name)
                        .map(|(_, binding)| binding.clone())
                        .unwrap_or_else(|| CapturedBinding::Cell(cell.clone()))
                };
                frame.local_captured_bindings[s] = Some(captured_binding);
                if let Some(CapturedBinding::EvalVarShadow { name, cell }) =
                    frame.local_captured_bindings[s].clone()
                {
                    frame.eval_var_shadow_cells.insert(name, cell);
                }
            } else if let Some((_, value)) = eval_outer_locals_vals
                .iter()
                .find(|(n, _)| Runtime::direct_eval_binding_name(n) == logical_name)
            {
                frame.write_local(s, value.clone());
                let cell = frame.promote_local(s);
                if eval_outer_const_local_names
                    .iter()
                    .any(|n| Runtime::direct_eval_binding_name(n) == logical_name)
                {
                    while frame.local_captured_bindings.len() <= s {
                        frame.local_captured_bindings.push(None);
                    }
                    frame.local_captured_bindings[s] = Some(CapturedBinding::ImmutableSelfName {
                        name: logical_name,
                        cell,
                    });
                }
            }
        }

        if bytecode_rc.eval_var_env_is_global {
            let seeds: Vec<(usize, String)> = frame
                .locals_names
                .iter()
                .enumerate()
                .filter(|(_, d)| d.depth == 0 && matches!(d.kind, rusty_js_ast::VariableKind::Var))
                .map(|(slot, d)| (slot, d.name.clone()))
                .collect();
            for (slot, name) in seeds {
                if let Some(gt) = self.global_object {
                    if self.obj(gt).has_own_str(&name) {
                        let v = self.object_get(gt, &name);
                        frame.write_local(slot, v);
                    }
                }
            }
        }
        if !evaluating_static_deps.is_empty() || has_deferred_async_dep {
            self.ensure_module_async_evaluation_order(&record);

            if matches!(record.borrow().status, ModuleStatus::Evaluating) {
                record.borrow_mut().status = ModuleStatus::EvaluatingAsync;
            }
            {
                let mut r = record.borrow_mut();
                r.status = ModuleStatus::Evaluating;
                r.async_static_deps = evaluating_static_deps.clone();
            }
            let snapshot = FrameSnapshot::from_frame(&frame, None);
            record.borrow_mut().pending_body_start = Some(snapshot);
            self.enqueue_ready_pending_module_bodies();
            return Ok(namespace);
        }

        self.current_module_url.push(url.to_string());
        if let Some(rec) = self.module_get(url) {
            rec.borrow_mut().status = ModuleStatus::Evaluating;
        }
        let t_eval = if phase_profile::enabled() {
            Some(std::time::Instant::now())
        } else {
            None
        };
        record.borrow_mut().status = ModuleStatus::Evaluating;
        let run_result = self.with_global_bindings_suspended(
            &["exports", "require", "module", "__filename", "__dirname"],
            |rt| rt.with_direct_eval_global_shadows_suspended(|rt| rt.run_frame_module(&mut frame)),
        );
        if let Some(t) = t_eval {
            phase_profile::add(&phase_profile::EVAL_NS, t.elapsed().as_nanos() as u64);
        }
        self.current_module_url.pop();

        match run_result {
            Ok(_) => {}
            Err(RuntimeError::ModuleAwaitSuspended(snapshot, value)) => {
                self.enqueue_module_await_resume(
                    url.to_string(),
                    namespace,
                    record.clone(),
                    snapshot,
                    value,
                );
                if outermost_module_eval {
                    self.drain_module_microtasks()?;
                }
                return Ok(namespace);
            }
            Err(e) => {
                let e = self.esm_scope_reference_error_projection(e, url);
                self.module_post_eval_trace
                    .insert(url.to_string(), format!("kind=ESM threw: {:?}", e));
                let mut r = record.borrow_mut();
                r.status = ModuleStatus::Failed;
                r.eval_error = Some(e.clone());
                return Err(e);
            }
        }

        let mut locals = frame.locals.clone();
        for (i, slot) in locals.iter_mut().enumerate() {
            if let Some(Some(cell)) = frame.local_cells.get(i) {
                *slot = cell.borrow().clone();
            }
        }
        if bytecode_rc.eval_var_env_is_global && publish_global_lexicals {
            for (slot, desc) in bytecode_rc.locals.iter().enumerate() {
                if desc.depth == 0
                    && !desc.name.starts_with("<scoped@")
                    && matches!(
                        desc.kind,
                        rusty_js_ast::VariableKind::Let | rusty_js_ast::VariableKind::Const
                    )
                {
                    let name = crate::interp::Runtime::direct_eval_binding_name(&desc.name);
                    let value = locals.get(slot).cloned().unwrap_or(Value::Undefined);
                    if matches!(desc.kind, rusty_js_ast::VariableKind::Const) {
                        self.global_immutable_lexical_bindings.insert(name.clone());
                    }
                    self.global_lexical_bindings.insert(name, value);
                }
            }
        }

        let t_namespace = if phase_profile::enabled() {
            Some(std::time::Instant::now())
        } else {
            None
        };
        for eb in &bytecode_rc.exports {
            match eb {
                ExportBinding::Local { exported, local } => {
                    let v = locals
                        .get(*local as usize)
                        .cloned()
                        .unwrap_or(Value::Undefined);
                    self.obj_mut(namespace)
                        .set_own_module_export(exported.clone(), v);
                }
                ExportBinding::Named {
                    exported,
                    source_specifier,
                    imported,
                } => {

                    let mut v = match self
                        .resolve_module_full(url, source_specifier, ModuleKind::ESM)
                        .ok()
                        .and_then(|resolved| self.module_resolve_export_cell(&resolved, imported))
                    {
                        Some(cell) => cell.borrow().clone(),
                        None => match reexport_namespaces.get(source_specifier) {
                            Some(src_ns) => self.object_get(*src_ns, imported),
                            None => Value::Undefined,
                        },
                    };
                    if matches!(v, Value::Undefined) {
                        if let Ok(resolved) =
                            self.resolve_module_full(url, source_specifier, ModuleKind::ESM)
                        {
                            if resolved == url {
                                for local_export in &bytecode_rc.exports {
                                    if let ExportBinding::Local {
                                        exported: local_name,
                                        local,
                                    } = local_export
                                    {
                                        if local_name == imported {
                                            v = locals
                                                .get(*local as usize)
                                                .cloned()
                                                .unwrap_or(Value::Undefined);
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    self.obj_mut(namespace)
                        .set_own_module_export(exported.clone(), v);
                }
                ExportBinding::Star { source_specifier } => {

                    let keys_values: Vec<(String, Value)> = match (
                        self.resolve_module_full(url, source_specifier, ModuleKind::ESM),
                        reexport_namespaces.get(source_specifier).copied(),
                    ) {
                        (Ok(resolved), Some(src_ns)) => {
                            let mut seen = std::collections::HashSet::new();
                            let mut keys = std::collections::HashSet::new();
                            let _ = self
                                .collect_module_export_names(url, &resolved, &mut seen, &mut keys);
                            let mut out = Vec::new();
                            for key in keys {
                                if key == "default" {
                                    continue;
                                }
                                let mut rset = std::collections::HashSet::new();
                                if matches!(
                                    self.module_resolve_export(url, &key, &mut rset),
                                    ResolveExportResult::Resolved { .. }
                                        | ResolveExportResult::Incomplete
                                ) {
                                    let v = self
                                        .module_resolve_export_cell(url, &key)
                                        .map(|cell| cell.borrow().clone())
                                        .unwrap_or_else(|| self.object_get(src_ns, &key));
                                    out.push((key, v));
                                }
                            }
                            out
                        }
                        _ => Vec::new(),
                    };
                    for (k, v) in keys_values {
                        self.obj_mut(namespace).set_own_module_export(k, v);
                    }
                }
                ExportBinding::StarAs {
                    exported,
                    source_specifier,
                } => {

                    let v = match reexport_namespaces.get(source_specifier) {
                        Some(src_ns) => Value::Object(*src_ns),
                        None => Value::Undefined,
                    };
                    self.obj_mut(namespace)
                        .set_own_module_export(exported.clone(), v);
                }
            }
        }
        if let Some(t) = t_namespace {
            phase_profile::add(&phase_profile::NAMESPACE_NS, t.elapsed().as_nanos() as u64);
        }

        let walk_path = url
            .strip_prefix("file://")
            .map(|p| std::path::PathBuf::from(p));
        let mut pkg_dual_shape = false;
        if let Some(mut p) = walk_path {
            p.pop();
            let mut steps = 0;
            while steps < 32 {
                let candidate = p.join("package.json");
                if candidate.is_file() {
                    if let Ok(pkg) = self.read_package_json(&candidate) {

                        let norm = |s: &Option<String>| {
                            s.as_deref()
                                .map(|v| v.strip_prefix("./").unwrap_or(v).to_string())
                        };
                        pkg_dual_shape = pkg.main.is_some()
                            && pkg.module_field.is_some()
                            && norm(&pkg.main) != norm(&pkg.module_field)
                            && pkg.raw.get("exports").is_none();
                    }
                    break;
                }
                if !p.pop() {
                    break;
                }
                steps += 1;
            }
        }

        let needs_default_synth = matches!(self.object_get(namespace, "default"), Value::Undefined);
        if needs_default_synth {
            if pkg_dual_shape {

                let pairs: Vec<(String, Value)> = {
                    let o = self.obj(namespace);
                    let mut out: Vec<(String, Value)> = Vec::new();
                    if let Some(shape) = o.shape.as_ref() {
                        for (name, slot) in shape.iter_slots() {
                            if name == "default" {
                                continue;
                            }
                            let idx = slot as usize;
                            if let Some(v) = o.shape_values.get(idx) {
                                out.push((name.to_string(), v.clone()));
                            }
                        }
                    }
                    out.extend(o.properties.iter().filter_map(|(k, d)| {
                        let key = k.as_str();
                        if key == "default" || key == "@@toStringTag" || key.starts_with("__") {
                            None
                        } else {
                            Some((k.to_string_content(), d.value.clone()))
                        }
                    }));
                    out
                };
                if !pairs.is_empty() {
                    let synth = self.alloc_object(Object::new_ordinary());
                    for (k, v) in pairs {
                        self.object_set(synth, k, v);
                    }
                    self.obj_mut(namespace)
                        .set_own_module_export("default".to_string(), Value::Object(synth));
                }
            }
        }

        if pkg_dual_shape {
            let default_v = self.object_get(namespace, "default");
            if let Value::Object(default_id) = default_v {
                let mut mirrored_data: Vec<(String, Value)> = Vec::new();
                let mut mirrored_getters: Vec<(String, Value)> = Vec::new();
                {
                    let o = self.obj(default_id);
                    if let Some(shape) = o.shape.as_ref() {
                        for (name, slot) in shape.iter_slots() {
                            if matches!(name, "__esModule" | "caller" | "arguments") {
                                continue;
                            }
                            let idx = slot as usize;
                            if let Some(v) = o.shape_values.get(idx) {
                                mirrored_data.push((name.to_string(), v.clone()));
                            }
                        }
                    }
                    for (k, d) in o.properties.iter() {
                        let name = k.as_str();
                        if matches!(name, "__esModule" | "caller" | "arguments")
                            || name.starts_with("@@")
                        {
                            continue;
                        }
                        if let Some(getter) = &d.getter {
                            mirrored_getters.push((name.to_string(), getter.clone()));
                        } else {
                            mirrored_data.push((name.to_string(), d.value.clone()));
                        }
                    }
                }
                let mut mirrored = mirrored_data;
                for (k, getter) in mirrored_getters {
                    let v = self
                        .call_function(getter, Value::Object(default_id), Vec::new())
                        .unwrap_or(Value::Undefined);
                    mirrored.push((k, v));
                }
                for (k, v) in mirrored {
                    if matches!(self.object_get(namespace, &k), Value::Undefined) {
                        self.obj_mut(namespace).set_own_module_export(k, v);
                    }
                }
            }
        }

        if let Some(hook) = self.host_hooks.finalize_namespace.take() {
            hook(self, &ast_rc, namespace, url)?;
            self.host_hooks.finalize_namespace = Some(hook);
        }

        self.finalize_module_namespace_exotic(namespace);

        {
            let mut r = record.borrow_mut();
            r.status = ModuleStatus::Evaluated;
            r.export_cells = export_cells;
        }
        self.settle_body_completed_async_modules();
        self.enqueue_ready_pending_module_bodies();

        let key_count = self.obj(namespace).properties.len();
        self.module_post_eval_trace.insert(
            url.to_string(),
            format!("kind=ESM key_count={} status=Evaluated", key_count),
        );

        if let Some(deferred) = self.pending_live_bindings.remove(url) {
            for d in deferred {
                let v = self.resolve_import_binding_value(namespace, &d.kind, d.is_cjs, url);
                *d.cell.borrow_mut() = v;
            }
        }
        self.resolve_dynamic_import_waiters(url, namespace);
        if outermost_module_eval {
            self.drain_outermost_module_microtasks()?;
        }

        Ok(namespace)
    }

    fn cjs_preparse_mark_parentless_reexport_targets(&mut self, source: &str, url: &str) {
        let mut queue: Vec<(String, String)> = Vec::new();
        queue.push((url.to_string(), source.to_string()));
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        seen.insert(url.to_string());
        while let Some((parent, src)) = queue.pop() {

            let source_shape =
                crate::cjs_export_resolution::extract_static_export_shape_from_cjs_source(&src);
            let wrapped = format!(
                "export default (function (exports, module, require, __filename, __dirname) {{\n{}\n}});\n",
                src
            );
            let shape = rusty_js_parser::parse_module(&wrapped)
                .ok()
                .and_then(|ast| {
                    crate::cjs_export_resolution::extract_static_export_shape_from_cjs_wrapper(&ast)
                })
                .map(|ast_shape| {
                    crate::cjs_export_resolution::merge_static_export_shapes(
                        ast_shape,
                        source_shape.clone(),
                    )
                })
                .unwrap_or(source_shape);
            let mut specs: Vec<String> = Vec::new();
            specs.extend(shape.module_exports_require_specs.iter().cloned());
            specs.extend(shape.cjs_star_reexport_specs.iter().cloned());
            specs.extend(shape.cjs_object_keys_reexport_specs.iter().cloned());
            for spec in specs {
                let Ok(resolved) = self.resolve_module_full(&parent, &spec, ModuleKind::CJS) else {
                    continue;
                };
                if !seen.insert(resolved.clone()) {
                    continue;
                }
                self.cjs_preparse_parentless.insert(resolved.clone());
                if let Some(path) = resolved.strip_prefix("file://") {
                    if let Ok(child_src) = std::fs::read_to_string(path) {
                        queue.push((resolved, child_src));
                    }
                }
            }
        }
    }

    pub fn evaluate_cjs_module(
        &mut self,
        source: &str,
        url: &str,
    ) -> Result<ObjectRef, RuntimeError> {
        cjs_load_trace(format_args!("evaluate-enter url={url}"));
        let cjs_prof = phase_profile::enabled();
        let cjs_wrapper_phase_profile = cjs_wrapper_phase_profile_matches(url);
        let mut cjs_wrapper_setup_ns = 0u64;
        let mut cjs_wrapper_parse_ns = 0u64;
        let mut cjs_wrapper_static_export_ns = 0u64;
        let mut cjs_wrapper_compile_ns = 0u64;
        let mut cjs_wrapper_module_eval_ns = 0u64;
        let mut cjs_wrapper_body_ns = 0u64;
        let mut cjs_wrapper_body_exclusive_ns = 0u64;
        let mut cjs_wrapper_post_body_ns = 0u64;
        if cjs_prof {
            phase_profile::inc(&phase_profile::CJS_EVALUATE_CALLS);
        }
        let t_cjs_setup = cjs_prof.then(std::time::Instant::now);

        self.verify_module_integrity(url, source)?;

        if self.cjs_require_stack.is_empty() {
            self.cjs_preparse_mark_parentless_reexport_targets(source, url);
        }

        let placeholder = self.alloc_object(Object::new_module_namespace());

        let initial_exports_obj = self.alloc_object(Object::new_ordinary());
        let initial_exports = Value::Object(initial_exports_obj);
        let empty_ast = Rc::new(AstModule {
            span: rusty_js_ast::Span::new(0, 0),
            body: Vec::new(),
            import_entries: Vec::new(),
            local_export_entries: Vec::new(),
            indirect_export_entries: Vec::new(),
            star_export_entries: Vec::new(),
        });
        let empty_bc = Rc::new(CompiledModule {
            bytecode: Vec::new(),
            constants: Default::default(),
            locals: Vec::new(),
            catch_param_names: Vec::new(),
            source_map: Vec::new(),
            source_text: None,
            imports: Vec::new(),
            exports: Vec::new(),
            reexport_sources: Vec::new(),
            side_effect_imports: Vec::new(),
            construct_tags: Vec::new(),
            line_starts: Vec::new(),
            eval_var_env_is_global: false,
            global_env_alias: false,
            script_var_deletable: false,
            eval_outer_locals: Vec::new(),
            module_hoisted_functions: Vec::new(),
            strict: false,
        });
        let record = Rc::new(RefCell::new(ModuleRecord {
            url: url.to_string(),
            status: ModuleStatus::Linking,
            ast: empty_ast,
            bytecode: empty_bc,
            namespace: Some(placeholder),
            eval_error: None,
            kind: ModuleKind::CJS,
            cjs_exports: Some(initial_exports.clone()),
            export_cells: std::collections::HashMap::new(),
            async_static_deps: Vec::new(),
            body_completed_waiting_async_deps: false,
            pending_body_start: None,
            async_evaluation_order: None,
            async_cycle_root: None,
        }));
        self.module_insert(url.to_string(), record.clone());

        let source_no_shebang = if source.starts_with("#!") {
            match source.find('\n') {
                Some(nl) => &source[nl + 1..],
                None => "",
            }
        } else {
            source
        };
        let is_bundle_runtime = source_no_shebang.contains("__nccwpck_require__")
            || source_no_shebang.contains("__webpack_require__");
        if source_has_esm_markers(source_no_shebang) && !is_bundle_runtime {
            if let Err(e) = rusty_js_parser::parse_module_goal(source_no_shebang) {
                return Err(RuntimeError::SyntaxError(format_public_parse_error(
                    source_no_shebang,
                    &e,
                    url,
                    "",
                    source_no_shebang.len(),
                )));
            }
        }

        let wrapped = format!(
            "export default (function (exports, module, require, __filename, __dirname) {{\n{}\n}});\n",
            source_no_shebang
        );
        if let Some(t) = t_cjs_setup {
            let ns = t.elapsed().as_nanos() as u64;
            cjs_wrapper_setup_ns = cjs_wrapper_setup_ns.saturating_add(ns);
            phase_profile::add(&phase_profile::CJS_WRAPPER_SETUP_NS, ns);
        }

        let _prof = cjs_prof;
        let t0 = if _prof {
            Some(std::time::Instant::now())
        } else {
            None
        };

        let _parse_profile_source =
            rusty_js_parser::parser::parse_profile::SourceLabelGuard::new(url);
        let ast = rusty_js_parser::parse_module(&wrapped).map_err(|e| {

            let body_end = wrapped.len().saturating_sub(4);
            RuntimeError::CompileError(format_public_parse_error(
                &wrapped,
                &e,
                url,
                " (cjs wrapper)",
                body_end,
            ))
        })?;
        if let Some(t) = t0 {
            let ns = t.elapsed().as_nanos() as u64;
            cjs_wrapper_parse_ns = ns;
            phase_profile::add(&phase_profile::PARSE_NS, ns);
            phase_profile::add(&phase_profile::CJS_WRAPPER_PARSE_NS, ns);
        }
        let t_static_exports = _prof.then(std::time::Instant::now);
        let source_export_shape =
            crate::cjs_export_resolution::extract_static_export_shape_from_cjs_source(
                source_no_shebang,
            );
        let static_export_shape =
            crate::cjs_export_resolution::extract_static_export_shape_from_cjs_wrapper(&ast)
                .map(|shape| {
                    crate::cjs_export_resolution::merge_static_export_shapes(
                        shape,
                        source_export_shape.clone(),
                    )
                })
                .unwrap_or(source_export_shape);
        let module_exports_require_reexport = static_export_shape.module_exports_require_reexport;
        let module_exports_direct_require_reexport =
            static_export_shape.module_exports_direct_require_reexport;
        let module_exports_require_specs = static_export_shape.module_exports_require_specs.clone();
        let cjs_star_reexport_specs = static_export_shape.cjs_star_reexport_specs.clone();
        let cjs_object_keys_reexport_specs =
            static_export_shape.cjs_object_keys_reexport_specs.clone();
        let mut static_export_keys = Some(static_export_shape.lower_node_keys());
        if let Some(keys) = static_export_keys.clone() {
            self.cjs_static_export_keys.insert(url.to_string(), keys);
        }
        if let Some(t) = t_static_exports {
            let ns = t.elapsed().as_nanos() as u64;
            cjs_wrapper_static_export_ns = ns;
            phase_profile::add(&phase_profile::CJS_WRAPPER_STATIC_EXPORT_NS, ns);
        }
        let _ast_rc = Rc::new(ast);
        let t1 = if _prof {
            Some(std::time::Instant::now())
        } else {
            None
        };
        let compile_phase_before = if cjs_wrapper_phase_profile {
            use rusty_js_bytecode::compile_profile as cp;
            use std::sync::atomic::Ordering;
            Some((
                cp::PARSED_MODULE_CALLS.load(Ordering::Relaxed),
                cp::PARSED_MODULE_LINE_STARTS_NS.load(Ordering::Relaxed),
                cp::PARSED_MODULE_SOURCE_TEXT_NS.load(Ordering::Relaxed),
                cp::PARSED_MODULE_SOURCE_URL_NS.load(Ordering::Relaxed),
                cp::PARSED_MODULE_CONFIG_NS.load(Ordering::Relaxed),
                cp::PARSED_MODULE_LOWER_NS.load(Ordering::Relaxed),
                cp::COMPILE_MODULE_CALLS.load(Ordering::Relaxed),
                cp::COMPILE_MODULE_STRICT_NS.load(Ordering::Relaxed),
                cp::COMPILE_MODULE_EVAL_IMPORTS_NS.load(Ordering::Relaxed),
                cp::COMPILE_MODULE_PREALLOC_NS.load(Ordering::Relaxed),
                cp::COMPILE_MODULE_HOIST_NS.load(Ordering::Relaxed),
                cp::COMPILE_MODULE_BODY_NS.load(Ordering::Relaxed),
                cp::COMPILE_MODULE_EXPORTS_NS.load(Ordering::Relaxed),
                cp::COMPILE_MODULE_ASSEMBLE_NS.load(Ordering::Relaxed),
                cp::EXPORT_DEFAULT_EXPR_NS.load(Ordering::Relaxed),
                cp::NAME_HINT_FUNCTION_PROTO_NS.load(Ordering::Relaxed),
                cp::NAME_HINT_FUNCTION_INTERN_EMIT_NS.load(Ordering::Relaxed),
                cp::FUNCTION_PROTO_CALLS.load(Ordering::Relaxed),
                cp::FUNCTION_PROTO_SETUP_NS.load(Ordering::Relaxed),
                cp::FUNCTION_PROTO_PARAMS_NS.load(Ordering::Relaxed),
                cp::FUNCTION_PROTO_PREALLOC_NS.load(Ordering::Relaxed),
                cp::FUNCTION_PROTO_HOIST_NS.load(Ordering::Relaxed),
                cp::FUNCTION_PROTO_BODY_NS.load(Ordering::Relaxed),
                cp::FUNCTION_PROTO_ASSEMBLE_NS.load(Ordering::Relaxed),
            ))
        } else {
            None
        };
        let bytecode = rusty_js_bytecode::compile_parsed_module_with_url_force_strict(
            &wrapped, url, &_ast_rc, false,
        )
        .map_err(|e| RuntimeError::CompileError(format!("compile (cjs wrapper): {}", e.message)))?;
        if let Some(t) = t1 {
            let ns = t.elapsed().as_nanos() as u64;
            cjs_wrapper_compile_ns = ns;
            phase_profile::add(&phase_profile::COMPILE_NS, ns);
            phase_profile::add(&phase_profile::CJS_WRAPPER_COMPILE_NS, ns);
        }
        if let Some(before) = compile_phase_before {
            use rusty_js_bytecode::compile_profile as cp;
            use std::sync::atomic::Ordering;
            let after = (
                cp::PARSED_MODULE_CALLS.load(Ordering::Relaxed),
                cp::PARSED_MODULE_LINE_STARTS_NS.load(Ordering::Relaxed),
                cp::PARSED_MODULE_SOURCE_TEXT_NS.load(Ordering::Relaxed),
                cp::PARSED_MODULE_SOURCE_URL_NS.load(Ordering::Relaxed),
                cp::PARSED_MODULE_CONFIG_NS.load(Ordering::Relaxed),
                cp::PARSED_MODULE_LOWER_NS.load(Ordering::Relaxed),
                cp::COMPILE_MODULE_CALLS.load(Ordering::Relaxed),
                cp::COMPILE_MODULE_STRICT_NS.load(Ordering::Relaxed),
                cp::COMPILE_MODULE_EVAL_IMPORTS_NS.load(Ordering::Relaxed),
                cp::COMPILE_MODULE_PREALLOC_NS.load(Ordering::Relaxed),
                cp::COMPILE_MODULE_HOIST_NS.load(Ordering::Relaxed),
                cp::COMPILE_MODULE_BODY_NS.load(Ordering::Relaxed),
                cp::COMPILE_MODULE_EXPORTS_NS.load(Ordering::Relaxed),
                cp::COMPILE_MODULE_ASSEMBLE_NS.load(Ordering::Relaxed),
                cp::EXPORT_DEFAULT_EXPR_NS.load(Ordering::Relaxed),
                cp::NAME_HINT_FUNCTION_PROTO_NS.load(Ordering::Relaxed),
                cp::NAME_HINT_FUNCTION_INTERN_EMIT_NS.load(Ordering::Relaxed),
                cp::FUNCTION_PROTO_CALLS.load(Ordering::Relaxed),
                cp::FUNCTION_PROTO_SETUP_NS.load(Ordering::Relaxed),
                cp::FUNCTION_PROTO_PARAMS_NS.load(Ordering::Relaxed),
                cp::FUNCTION_PROTO_PREALLOC_NS.load(Ordering::Relaxed),
                cp::FUNCTION_PROTO_HOIST_NS.load(Ordering::Relaxed),
                cp::FUNCTION_PROTO_BODY_NS.load(Ordering::Relaxed),
                cp::FUNCTION_PROTO_ASSEMBLE_NS.load(Ordering::Relaxed),
            );
            eprintln!(
                "[cjs-wrapper-compile-phase-profile] url={} parsed_calls={} line_starts_ns={} source_text_ns={} source_url_ns={} config_ns={} lower_ns={} compile_module_calls={} strict_ns={} eval_imports_ns={} prealloc_ns={} hoist_ns={} body_ns={} exports_ns={} assemble_ns={} export_default_expr_ns={} name_hint_function_proto_ns={} name_hint_function_intern_emit_ns={} function_proto_calls={} function_proto_setup_ns={} function_proto_params_ns={} function_proto_prealloc_ns={} function_proto_hoist_ns={} function_proto_body_ns={} function_proto_assemble_ns={}",
                url,
                after.0.saturating_sub(before.0),
                after.1.saturating_sub(before.1),
                after.2.saturating_sub(before.2),
                after.3.saturating_sub(before.3),
                after.4.saturating_sub(before.4),
                after.5.saturating_sub(before.5),
                after.6.saturating_sub(before.6),
                after.7.saturating_sub(before.7),
                after.8.saturating_sub(before.8),
                after.9.saturating_sub(before.9),
                after.10.saturating_sub(before.10),
                after.11.saturating_sub(before.11),
                after.12.saturating_sub(before.12),
                after.13.saturating_sub(before.13),
                after.14.saturating_sub(before.14),
                after.15.saturating_sub(before.15),
                after.16.saturating_sub(before.16),
                after.17.saturating_sub(before.17),
                after.18.saturating_sub(before.18),
                after.19.saturating_sub(before.19),
                after.20.saturating_sub(before.20),
                after.21.saturating_sub(before.21),
                after.22.saturating_sub(before.22),
                after.23.saturating_sub(before.23),
            );
        }
        if _prof {
            phase_profile::MODULE_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        let bytecode_rc = Rc::new(bytecode);

        let mut frame = Frame::new_module(&bytecode_rc);
        frame.source_url = url;
        self.current_module_url.push(url.to_string());
        if let Some(rec) = self.module_get(url) {
            rec.borrow_mut().status = ModuleStatus::Evaluating;
        }
        let t_eval = if phase_profile::enabled() {
            Some(std::time::Instant::now())
        } else {
            None
        };
        let run_result = self.run_frame_module(&mut frame);
        if let Some(t) = t_eval {
            let ns = t.elapsed().as_nanos() as u64;
            cjs_wrapper_module_eval_ns = ns;
            phase_profile::add(&phase_profile::EVAL_NS, ns);
            phase_profile::add(&phase_profile::CJS_WRAPPER_MODULE_EVAL_NS, ns);
        }
        self.current_module_url.pop();
        if let Err(e) = &run_result {
            self.module_post_eval_trace
                .insert(url.to_string(), format!("kind=CJS-wrapper threw: {:?}", e));
            let mut r = record.borrow_mut();
            r.status = ModuleStatus::Failed;
            r.eval_error = Some(e.clone());
        }
        run_result?;
        let locals = frame.locals.clone();
        let t_cjs_fn_setup = cjs_prof.then(std::time::Instant::now);

        let wrapper_fn: Value = bytecode_rc
            .exports
            .iter()
            .find_map(|eb| {
                if let rusty_js_bytecode::ExportBinding::Local { exported, local } = eb {
                    if exported == "default" {
                        return locals.get(*local as usize).cloned();
                    }
                }
                None
            })
            .unwrap_or(Value::Undefined);

        let (filename, dirname) = filename_dirname_from_url(url);
        let filename_v = Value::String(Rc::new(crate::value::JsString::from(filename)));
        let dirname_v = Value::String(Rc::new(crate::value::JsString::from(dirname)));

        let require_url = url.to_string();
        let require_fn: crate::value::NativeFn = Rc::new(move |rt, args| {
            let cjs_require_profile = phase_profile::enabled();
            let t_native_total = cjs_require_profile.then(std::time::Instant::now);
            let mut native_arg_ns = 0u64;
            let mut native_caps_ns = 0u64;
            let mut native_dispatch_ns = 0u64;
            let t_arg = cjs_require_profile.then(std::time::Instant::now);
            let spec = match args.first() {
                Some(Value::String(s)) => s.as_str().to_string(),
                _ => {
                    return Err(RuntimeError::TypeError(
                        "require: argument must be a string specifier".into(),
                    ))
                }
            };
            if let Some(t) = t_arg {
                native_arg_ns = t.elapsed().as_nanos() as u64;
                phase_profile::add(&phase_profile::CJS_REQUIRE_ARG_NS, native_arg_ns);
            }

            let t_caps = cjs_require_profile.then(std::time::Instant::now);
            let grant = match args.get(1) {
                Some(Value::Object(opts)) => rt.require_caps_grant_from_options(*opts),
                _ => None,
            };
            if let Some(t) = t_caps {
                native_caps_ns = t.elapsed().as_nanos() as u64;
                phase_profile::add(&phase_profile::CJS_REQUIRE_CAPS_NS, native_caps_ns);
            }
            let t_dispatch = cjs_require_profile.then(std::time::Instant::now);
            let result = match grant {
                Some(g) => {
                    let result = rt.cjs_require_with_grant(&require_url, &spec, &g);
                    if let Some(t) = t_dispatch {
                        native_dispatch_ns = t.elapsed().as_nanos() as u64;
                        phase_profile::add(
                            &phase_profile::CJS_REQUIRE_INNER_NS,
                            native_dispatch_ns,
                        );
                    }
                    result
                }
                None => {
                    let result = rt.cjs_require(&require_url, &spec);
                    if let Some(t) = t_dispatch {
                        native_dispatch_ns = t.elapsed().as_nanos() as u64;
                        phase_profile::add(
                            &phase_profile::CJS_REQUIRE_INNER_NS,
                            native_dispatch_ns,
                        );
                    }
                    result
                }
            };
            if let Some(t) = t_native_total {
                let total_ns = t.elapsed().as_nanos() as u64;
                let accounted_ns = native_arg_ns
                    .saturating_add(native_caps_ns)
                    .saturating_add(native_dispatch_ns);
                phase_profile::add(&phase_profile::CJS_REQUIRE_NATIVE_TOTAL_NS, total_ns);
                phase_profile::add(
                    &phase_profile::CJS_REQUIRE_NATIVE_RESIDUAL_NS,
                    total_ns.saturating_sub(accounted_ns),
                );
            }
            result
        });
        let mut require_props = indexmap::IndexMap::new();
        crate::value::install_function_meta_props(&mut require_props, "require", 1.0);
        let require_obj = Object {
            proto: None,
            extensible: true,
            properties: require_props,
            internal_kind: crate::value::InternalKind::Function(Box::new(
                crate::value::FunctionInternals {
                    name: "require".to_string(),
                    length: 1,
                    native: require_fn,
                    is_constructor: true,
                    creation_realm: 0,
                    roots: Vec::new(),
                },
            )),

            ..Default::default()
        };
        let require_id = self.alloc_object(require_obj);

        let require_resolve_url = url.to_string();
        let require_resolve_fn: crate::value::NativeFn = std::rc::Rc::new(move |rt, args| {
            let spec = match args.first() {
                Some(Value::String(s)) => s.as_str().to_string(),
                other => return Err(require_resolve_invalid_request_error(rt, other)),
            };
            if let Some(builtin) = node_builtin_resolve_public(&spec) {
                return Ok(Value::String(std::rc::Rc::new(
                    crate::value::JsString::from(builtin),
                )));
            }
            if spec.starts_with("node:") {
                return Err(
                    rt.node_style_cjs_require_error(RuntimeError::TypeError(format!(
                        "__node_cjs_missing_module__:{}|{}|",
                        spec, require_resolve_url
                    ))),
                );
            }
            if let Some(result) = require_resolve_with_custom_paths(rt, &spec, args.get(1)) {
                return result;
            }
            if let Some(path) = resolve_bare_node_modules_file(&require_resolve_url, &spec) {
                return Ok(Value::String(std::rc::Rc::new(
                    crate::value::JsString::from(path),
                )));
            }

            let resolved =
                match rt.resolve_module_full(&require_resolve_url, &spec, ModuleKind::CJS) {
                    Ok(r) => r,
                    Err(e) => return Err(rt.node_style_cjs_require_error(e)),
                };

            let path = resolved
                .strip_prefix("file://")
                .unwrap_or(&resolved)
                .to_string();
            Ok(Value::String(std::rc::Rc::new(
                crate::value::JsString::from(path),
            )))
        });
        let mut resolve_props = indexmap::IndexMap::new();
        crate::value::install_function_meta_props(&mut resolve_props, "resolve", 1.0);
        let require_resolve_obj = Object {
            proto: None,
            extensible: true,
            properties: resolve_props,
            internal_kind: crate::value::InternalKind::Function(Box::new(
                crate::value::FunctionInternals {
                    name: "resolve".to_string(),
                    length: 1,
                    native: require_resolve_fn,
                    is_constructor: true,
                    creation_realm: 0,
                    roots: Vec::new(),
                },
            )),

            ..Default::default()
        };
        let require_resolve_id = self.alloc_object(require_resolve_obj);

        let require_paths_url = url.to_string();
        let require_paths_fn: crate::value::NativeFn = std::rc::Rc::new(move |rt, args| {
            let spec = match args.first() {
                Some(Value::String(s)) => s.as_str().to_string(),
                other => return Err(require_resolve_invalid_request_error(rt, other)),
            };
            Ok(require_resolve_paths_value(rt, &require_paths_url, &spec))
        });
        let mut paths_props = indexmap::IndexMap::new();
        crate::value::install_function_meta_props(&mut paths_props, "paths", 1.0);
        let require_paths_obj = Object {
            proto: None,
            extensible: true,
            properties: paths_props,
            internal_kind: crate::value::InternalKind::Function(Box::new(
                crate::value::FunctionInternals {
                    name: "paths".to_string(),
                    length: 1,
                    native: require_paths_fn,
                    is_constructor: true,
                    creation_realm: 0,
                    roots: Vec::new(),
                },
            )),

            ..Default::default()
        };
        let require_paths_id = self.alloc_object(require_paths_obj);
        self.object_set(
            require_resolve_id,
            "paths".into(),
            Value::Object(require_paths_id),
        );
        self.object_set(
            require_id,
            "resolve".into(),
            Value::Object(require_resolve_id),
        );

        let require_cache = match self.global_get("__cruft_cjs_require_cache") {
            Value::Object(id) => id,
            _ => {
                let id = self.alloc_object(Object::new_ordinary());
                self.define_global_property("__cruft_cjs_require_cache", Value::Object(id));
                id
            }
        };
        self.object_set(require_id, "cache".into(), Value::Object(require_cache));

        let require_extensions = self.alloc_object(Object::new_ordinary());
        self.object_set(
            require_id,
            "extensions".into(),
            Value::Object(require_extensions),
        );
        let require_v = Value::Object(require_id);

        let module_id = self.alloc_object(Object::new_ordinary());
        self.object_set(module_id, "exports".to_string(), initial_exports.clone());
        self.cjs_live_module_exports
            .insert(module_id, url.to_string());

        self.cjs_original_exports
            .insert(url.to_string(), initial_exports_obj);
        let module_v = Value::Object(module_id);
        if let Some(t) = t_cjs_fn_setup {
            let ns = t.elapsed().as_nanos() as u64;
            cjs_wrapper_setup_ns = cjs_wrapper_setup_ns.saturating_add(ns);
            phase_profile::add(&phase_profile::CJS_WRAPPER_SETUP_NS, ns);
        }

        self.current_module_url.push(url.to_string());
        let t_cjs_body = phase_profile::enabled().then(std::time::Instant::now);
        let body_child_load_before = if t_cjs_body.is_some() {
            phase_profile::cjs_require_load_child_ns()
        } else {
            0
        };
        let call_result = self.with_direct_eval_global_shadows_suspended(|rt| {
            rt.call_function(
                wrapper_fn,
                Value::Undefined,
                vec![
                    initial_exports.clone(),
                    module_v.clone(),
                    require_v,
                    filename_v,
                    dirname_v,
                ],
            )
        });
        if let Some(t) = t_cjs_body {
            let ns = t.elapsed().as_nanos() as u64;
            cjs_wrapper_body_ns = ns;
            let child_load_ns =
                phase_profile::cjs_require_load_child_ns().saturating_sub(body_child_load_before);
            cjs_wrapper_body_exclusive_ns = ns.saturating_sub(child_load_ns);
            phase_profile::inc(&phase_profile::CJS_BODY_CALL_COUNT);
            phase_profile::add(&phase_profile::CJS_BODY_CALL_NS, ns);
            phase_profile::add(
                &phase_profile::CJS_WRAPPER_BODY_EXCLUSIVE_NS,
                cjs_wrapper_body_exclusive_ns,
            );
        }
        self.current_module_url.pop();
        self.cjs_live_module_exports.remove(&module_id);
        let _ = call_result?;
        let t_cjs_post_body = cjs_prof.then(std::time::Instant::now);

        let final_exports = match self.find_getter(module_id, "exports") {
            Some(getter) => self.call_function(getter, Value::Object(module_id), Vec::new())?,
            None => self.object_get(module_id, "exports"),
        };

        if let Value::Object(cache_id) = self.global_get("__cruft_cjs_require_cache") {
            let filename = url
                .strip_prefix("file://")
                .map(|s| s.to_string())
                .unwrap_or_else(|| url.to_string());
            self.object_set(
                module_id,
                "id".into(),
                Value::String(Rc::new(crate::value::JsString::from(filename.clone()))),
            );
            self.object_set(
                module_id,
                "filename".into(),
                Value::String(Rc::new(crate::value::JsString::from(filename.clone()))),
            );
            self.object_set(module_id, "loaded".into(), Value::Boolean(true));
            self.object_set(cache_id, filename, Value::Object(module_id));
        }

        {
            let mut r = record.borrow_mut();
            r.cjs_exports = Some(final_exports.clone());
            r.status = ModuleStatus::Evaluated;
        }

        let exports_reassigned = match (&final_exports, &initial_exports) {
            (Value::Object(a), Value::Object(b)) => a != b,
            _ => true,
        };

        if module_exports_require_reexport {
            for spec in &module_exports_require_specs {
                if let Ok(resolved) = self.resolve_module_full(url, spec, ModuleKind::CJS) {
                    let _ = self.load_module_as(&resolved, ModuleKind::CJS);
                    if module_exports_direct_require_reexport
                        && !self
                            .cjs_exports_of(&resolved)
                            .as_ref()
                            .is_some_and(|candidate| Value::same_value(candidate, &final_exports))
                    {
                        continue;
                    }
                    if let Some(source_keys) = self
                        .cjs_static_export_keys
                        .get(&resolved)
                        .cloned()
                        .or_else(|| cjs_static_export_keys_from_resolved_source(&resolved))
                    {
                        let keys = static_export_keys.get_or_insert_with(|| {
                            vec!["default".to_string(), "module.exports".to_string()]
                        });
                        for k in source_keys {
                            if !keys.iter().any(|existing| existing == &k) {
                                keys.push(k);
                            }
                        }
                    }
                }
            }
            if let Some(keys) = &static_export_keys {
                self.cjs_static_export_keys
                    .insert(url.to_string(), keys.clone());
            }
        }

        if !cjs_star_reexport_specs.is_empty() || !cjs_object_keys_reexport_specs.is_empty() {
            for spec in cjs_star_reexport_specs
                .iter()
                .chain(cjs_object_keys_reexport_specs.iter())
            {
                if let Ok(resolved) = self.resolve_module_full(url, spec, ModuleKind::CJS) {
                    let _ = self.load_module_as(&resolved, ModuleKind::CJS);
                    if let Some(source_keys) = self
                        .cjs_static_export_keys
                        .get(&resolved)
                        .cloned()
                        .or_else(|| cjs_static_export_keys_from_resolved_source(&resolved))
                    {
                        let keys = static_export_keys.get_or_insert_with(|| {
                            vec!["default".to_string(), "module.exports".to_string()]
                        });
                        for k in source_keys {
                            if matches!(k.as_str(), "default" | "module.exports") {
                                continue;
                            }
                            if !keys.iter().any(|existing| existing == &k) {
                                keys.push(k);
                            }
                        }
                    }
                }
            }
            if let Some(keys) = &static_export_keys {
                self.cjs_static_export_keys
                    .insert(url.to_string(), keys.clone());
            }
        }

        self.populate_cjs_namespace_view_at(
            placeholder,
            &final_exports,
            exports_reassigned,
            Some(url),
            static_export_keys,
        );

        let key_count = self.obj(placeholder).properties.len();
        self.module_post_eval_trace.insert(
            url.to_string(),
            format!(
                "kind=CJS key_count={} exports_reassigned={}",
                key_count, exports_reassigned
            ),
        );
        cjs_load_trace(format_args!("evaluate-complete url={url}"));
        if let Some(t) = t_cjs_post_body {
            let ns = t.elapsed().as_nanos() as u64;
            cjs_wrapper_post_body_ns = ns;
            phase_profile::add(&phase_profile::CJS_WRAPPER_POST_BODY_NS, ns);
        }
        if cjs_wrapper_phase_profile {
            let accounted_ns = cjs_wrapper_setup_ns
                .saturating_add(cjs_wrapper_parse_ns)
                .saturating_add(cjs_wrapper_static_export_ns)
                .saturating_add(cjs_wrapper_compile_ns)
                .saturating_add(cjs_wrapper_module_eval_ns)
                .saturating_add(cjs_wrapper_body_ns)
                .saturating_add(cjs_wrapper_post_body_ns);
            eprintln!(
                "[cjs-wrapper-phase-profile] url={} setup_ns={} parse_ns={} static_export_ns={} compile_ns={} module_eval_ns={} body_ns={} body_exclusive_ns={} post_body_ns={} accounted_ns={}",
                url,
                cjs_wrapper_setup_ns,
                cjs_wrapper_parse_ns,
                cjs_wrapper_static_export_ns,
                cjs_wrapper_compile_ns,
                cjs_wrapper_module_eval_ns,
                cjs_wrapper_body_ns,
                cjs_wrapper_body_exclusive_ns,
                cjs_wrapper_post_body_ns,
                accounted_ns
            );
        }

        Ok(placeholder)
    }

    pub fn cjs_namespace_view(&mut self, exports: Value) -> ObjectRef {
        self.cjs_namespace_view_at(exports, None)
    }

    pub fn cjs_namespace_view_at(&mut self, exports: Value, url: Option<&str>) -> ObjectRef {
        let ns = self.alloc_object(Object::new_module_namespace());

        let _ns_root = self.push_temporary_value_roots(&[Value::Object(ns), exports.clone()]);

        let static_export_keys = url.and_then(|u| self.cjs_static_export_keys.get(u).cloned());
        self.populate_cjs_namespace_view_at(ns, &exports, true, url, static_export_keys);
        ns
    }

    pub fn evaluate_json_module(
        &mut self,
        source: &str,
        url: &str,
    ) -> Result<ObjectRef, RuntimeError> {
        let value = crate::intrinsics::json_parse(self, source).map_err(|e| {

            RuntimeError::SyntaxError(format!("parse (json module): {:?} @url={}", e, url))
        })?;
        let ns = self.alloc_object(Object::new_module_namespace());
        self.object_set(ns, "default".to_string(), value.clone());

        let empty_ast = Rc::new(AstModule {
            span: rusty_js_ast::Span::new(0, 0),
            body: Vec::new(),
            import_entries: Vec::new(),

            local_export_entries: vec![rusty_js_ast::ExportEntry {
                export_name: Some("default".to_string()),
                module_request: None,
                import_name: None,
                local_name: Some("default".to_string()),
            }],
            indirect_export_entries: Vec::new(),
            star_export_entries: Vec::new(),
        });
        let empty_bc = Rc::new(CompiledModule {
            bytecode: Vec::new(),
            constants: Default::default(),
            locals: Vec::new(),
            catch_param_names: Vec::new(),
            source_map: Vec::new(),
            source_text: None,
            imports: Vec::new(),
            exports: Vec::new(),
            reexport_sources: Vec::new(),
            side_effect_imports: Vec::new(),
            construct_tags: Vec::new(),
            line_starts: Vec::new(),
            eval_var_env_is_global: false,
            global_env_alias: false,
            script_var_deletable: false,
            eval_outer_locals: Vec::new(),
            module_hoisted_functions: Vec::new(),
            strict: false,
        });
        self.module_insert(
            url.to_string(),
            Rc::new(RefCell::new(ModuleRecord {
                url: url.to_string(),
                status: ModuleStatus::Evaluated,
                ast: empty_ast,
                bytecode: empty_bc,
                namespace: Some(ns),
                eval_error: None,
                kind: ModuleKind::ESM,
                cjs_exports: Some(value.clone()),
                export_cells: {
                    let mut m = std::collections::HashMap::new();
                    m.insert(
                        "default".to_string(),
                        crate::value::new_upvalue_cell(value.clone()),
                    );
                    m
                },
                async_static_deps: Vec::new(),
                body_completed_waiting_async_deps: false,
                pending_body_start: None,
                async_evaluation_order: None,
                async_cycle_root: None,
            })),
        );
        Ok(ns)
    }

    pub fn evaluate_text_module(
        &mut self,
        source: &str,
        url: &str,
    ) -> Result<ObjectRef, RuntimeError> {
        let value = Value::String(Rc::new(crate::value::JsString::from(source.to_string())));
        let ns = self.alloc_object(Object::new_module_namespace());
        self.object_set(ns, "default".to_string(), value.clone());
        let empty_ast = Rc::new(AstModule {
            span: rusty_js_ast::Span::new(0, 0),
            body: Vec::new(),
            import_entries: Vec::new(),
            local_export_entries: Vec::new(),
            indirect_export_entries: Vec::new(),
            star_export_entries: Vec::new(),
        });
        let empty_bc = Rc::new(CompiledModule {
            bytecode: Vec::new(),
            constants: Default::default(),
            locals: Vec::new(),
            catch_param_names: Vec::new(),
            source_map: Vec::new(),
            source_text: None,
            imports: Vec::new(),
            exports: Vec::new(),
            reexport_sources: Vec::new(),
            side_effect_imports: Vec::new(),
            construct_tags: Vec::new(),
            line_starts: Vec::new(),
            eval_var_env_is_global: false,
            global_env_alias: false,
            script_var_deletable: false,
            eval_outer_locals: Vec::new(),
            module_hoisted_functions: Vec::new(),
            strict: false,
        });
        self.module_insert(
            url.to_string(),
            Rc::new(RefCell::new(ModuleRecord {
                url: url.to_string(),
                status: ModuleStatus::Evaluated,
                ast: empty_ast,
                bytecode: empty_bc,
                namespace: Some(ns),
                eval_error: None,
                kind: ModuleKind::ESM,
                cjs_exports: Some(value),
                export_cells: std::collections::HashMap::new(),
                async_static_deps: Vec::new(),
                body_completed_waiting_async_deps: false,
                pending_body_start: None,
                async_evaluation_order: None,
                async_cycle_root: None,
            })),
        );
        Ok(ns)
    }

    pub fn load_bytes_module(&mut self, url: &str) -> Result<ObjectRef, RuntimeError> {
        let cache_url = format!("{url}#with=type:bytes");
        if let Some(rec) = self.module_get(&cache_url) {
            let r = rec.borrow();
            if r.status == ModuleStatus::Failed {
                return Err(r.eval_error.clone().unwrap_or_else(|| {
                    RuntimeError::TypeError(format!("module '{}' evaluation failed", cache_url))
                }));
            }
            if let Some(ns) = r.namespace {
                return Ok(ns);
            }
        }
        let Some(path) = url.strip_prefix("file://") else {
            return Err(RuntimeError::TypeError(format!(
                "bytes module load: unsupported URL '{}'",
                url
            )));
        };
        let bytes = std::fs::read(path).map_err(|e| {
            RuntimeError::TypeError(format!("bytes module load: cannot read '{}': {}", path, e))
        })?;
        self.evaluate_bytes_module(bytes, &cache_url)
    }

    pub fn evaluate_bytes_module(
        &mut self,
        bytes: Vec<u8>,
        url: &str,
    ) -> Result<ObjectRef, RuntimeError> {

        let ta = self.alloc_uint8_array_from_bytes(&bytes);
        if let Some(buf) = self.typed_array_views.get(&ta).map(|v| v.buffer) {
            self.obj_mut(buf)
                .set_own_internal("__cruft_immutable_arraybuffer".into(), Value::Boolean(true));
        }
        let value = Value::Object(ta);
        let ns = self.alloc_object(Object::new_module_namespace());
        self.object_set(ns, "default".to_string(), value.clone());
        let empty_ast = Rc::new(AstModule {
            span: rusty_js_ast::Span::new(0, 0),
            body: Vec::new(),
            import_entries: Vec::new(),
            local_export_entries: Vec::new(),
            indirect_export_entries: Vec::new(),
            star_export_entries: Vec::new(),
        });
        let empty_bc = Rc::new(CompiledModule {
            bytecode: Vec::new(),
            constants: Default::default(),
            locals: Vec::new(),
            catch_param_names: Vec::new(),
            source_map: Vec::new(),
            source_text: None,
            imports: Vec::new(),
            exports: Vec::new(),
            reexport_sources: Vec::new(),
            side_effect_imports: Vec::new(),
            construct_tags: Vec::new(),
            line_starts: Vec::new(),
            eval_var_env_is_global: false,
            global_env_alias: false,
            script_var_deletable: false,
            eval_outer_locals: Vec::new(),
            module_hoisted_functions: Vec::new(),
            strict: false,
        });
        self.module_insert(
            url.to_string(),
            Rc::new(RefCell::new(ModuleRecord {
                url: url.to_string(),
                status: ModuleStatus::Evaluated,
                ast: empty_ast,
                bytecode: empty_bc,
                namespace: Some(ns),
                eval_error: None,
                kind: ModuleKind::ESM,
                cjs_exports: Some(value),
                export_cells: std::collections::HashMap::new(),
                async_static_deps: Vec::new(),
                body_completed_waiting_async_deps: false,
                pending_body_start: None,
                async_evaluation_order: None,
                async_cycle_root: None,
            })),
        );
        Ok(ns)
    }

    fn populate_cjs_namespace_view(
        &mut self,
        ns: ObjectRef,
        exports: &Value,
        exports_reassigned: bool,
    ) {
        self.populate_cjs_namespace_view_at(ns, exports, exports_reassigned, None, None)
    }

    fn populate_cjs_namespace_view_at(
        &mut self,
        ns: ObjectRef,
        exports: &Value,
        exports_reassigned: bool,
        url: Option<&str>,
        static_export_keys: Option<Vec<String>>,
    ) {
        self.obj_mut(ns).properties.retain(|k, _| {
            let key = k.as_str();
            key == "@@toStringTag" || key.starts_with("__")
        });

        {
            if let Some(keys) = static_export_keys
                .or_else(|| url.and_then(|u| self.cjs_static_export_keys.get(u).cloned()))
                .or_else(|| url.and_then(|u| self.lockfile_export_keys(u)))
            {
                let final_oid = match exports {
                    Value::Object(oid) => Some(*oid),
                    _ => None,
                };
                for k in keys {
                    if k == "default" || k == "module.exports" {
                        self.object_set(ns, k, exports.clone());
                        continue;
                    }

                    let resolved = final_oid
                        .and_then(|o| self.resolve_export_value(o, &k))
                        .unwrap_or(Value::Undefined);
                    self.object_set(ns, k, resolved);
                }
                return;
            }
        }

        if cjs_node_interop_enabled() {
            if let Value::Object(oid) = exports {
                self.object_set(ns, "default".to_string(), exports.clone());
                self.object_set(ns, "module.exports".to_string(), exports.clone());
                if !exports_reassigned {
                    let named: Vec<(String, Value, Option<Value>)> = {
                        let o = self.obj(*oid);
                        let mut out: Vec<(String, Value, Option<Value>)> = Vec::new();
                        if let Some(shape) = o.shape.as_ref() {
                            for (name, slot) in shape.iter_slots() {
                                if matches!(name, "__esModule" | "caller" | "arguments") {
                                    continue;
                                }
                                let idx = slot as usize;
                                if let Some(v) = o.shape_values.get(idx) {
                                    out.push((name.to_string(), v.clone(), None));
                                }
                            }
                        }
                        out.extend(
                            o.properties
                                .iter()
                                .filter(|(k, _)| {
                                    !matches!(k.as_str(), "__esModule" | "caller" | "arguments")
                                })
                                .map(|(k, d)| {
                                    (k.to_string_content(), d.value.clone(), d.getter.clone())
                                }),
                        );
                        out
                    };
                    for (k, v, getter) in named {
                        let resolved = if let Some(g) = getter {
                            self.call_function(g, Value::Object(*oid), Vec::new())
                                .unwrap_or(Value::Undefined)
                        } else {
                            v
                        };
                        self.object_set(ns, k, resolved);
                    }
                }

                let esmod_v = self.object_get(*oid, "__esModule");
                if !matches!(esmod_v, Value::Undefined) {
                    self.object_set(ns, "__esModule".to_string(), esmod_v);
                }
            } else {
                self.object_set(ns, "default".to_string(), exports.clone());
                self.object_set(ns, "module.exports".to_string(), exports.clone());
            }
            return;
        }
        match exports {
            Value::Object(oid) => {

                let esmod_v = self.object_get(*oid, "__esModule");
                let exports_is_fn = matches!(
                    self.obj(*oid).internal_kind,
                    crate::value::InternalKind::Function(_)
                        | crate::value::InternalKind::Closure(_)
                        | crate::value::InternalKind::BoundFunction(_)
                );

                let pkg_has_exports_field = match url {
                    Some(u) => package_has_exports_field_walk(u),
                    None => false,
                };
                let strip_fn_intrinsics = exports_is_fn && matches!(esmod_v, Value::Boolean(true));

                let _ = pkg_has_exports_field;
                let strip_prototype_only = false;

                let exports_is_callable = matches!(
                    self.obj(*oid).internal_kind,
                    crate::value::InternalKind::Function(_)
                        | crate::value::InternalKind::Closure(_)
                        | crate::value::InternalKind::BoundFunction(_)
                );
                let super_proto_names: Option<std::collections::HashSet<String>> =
                    if exports_is_callable {
                        let super_ctor = self.obj(*oid).proto;
                        match super_ctor {
                            Some(sid) => {
                                let super_proto_v = self.object_get(sid, "prototype");
                                match super_proto_v {
                                    Value::Object(spid) => {
                                        let names: std::collections::HashSet<String> = self
                                            .obj(spid)
                                            .properties
                                            .iter()
                                            .map(|(k, _)| k.to_string_content())
                                            .collect();

                                        if names.len() > 1 {
                                            Some(names)
                                        } else {
                                            None
                                        }
                                    }
                                    _ => None,
                                }
                            }
                            None => None,
                        }
                    } else {
                        None
                    };
                let triples: Vec<(String, Value, Option<Value>)> = {
                    let o = self.obj(*oid);
                    let mut out: Vec<(String, Value, Option<Value>)> = Vec::new();
                    if let Some(shape) = o.shape.as_ref() {
                        for (name, slot) in shape.iter_slots() {
                            if name == "__esModule" {
                                continue;
                            }
                            if cjs_namespace_filter_package_key(url, name) {
                                continue;
                            }
                            if strip_fn_intrinsics
                                && matches!(name, "name" | "length" | "prototype")
                            {
                                continue;
                            }
                            if strip_prototype_only && name == "prototype" {
                                continue;
                            }
                            let idx = slot as usize;
                            if let Some(v) = o.shape_values.get(idx) {
                                out.push((name.to_string(), v.clone(), None));
                            }
                        }
                    }
                    out.extend(
                        o.properties
                            .iter()
                            .filter(|(k, d)| {
                                if k.as_str() == "__esModule" {
                                    return false;
                                }
                                if cjs_namespace_filter_package_key(url, k.as_str()) {
                                    return false;
                                }
                                if strip_fn_intrinsics
                                    && matches!(k.as_str(), "name" | "length" | "prototype")
                                {
                                    return false;
                                }
                                if strip_prototype_only && k.as_str() == "prototype" {
                                    return false;
                                }

                                if matches!(k.as_str(), "caller" | "arguments") {
                                    return false;
                                }

                                if let Some(super_names) = &super_proto_names {
                                    if !d.enumerable {
                                        let kn = k.as_str();
                                        let is_fn_intrinsic =
                                            matches!(kn, "name" | "length" | "prototype");
                                        if !is_fn_intrinsic && !super_names.contains(kn) {
                                            return false;
                                        }
                                    }
                                }
                                true
                            })
                            .map(|(k, d)| {
                                (k.to_string_content(), d.value.clone(), d.getter.clone())
                            }),
                    );
                    out
                };
                let mut pairs: Vec<(String, Value)> = Vec::with_capacity(triples.len());
                for (k, v, getter) in triples {
                    let resolved = if let Some(g) = getter {
                        self.call_function(g, Value::Object(*oid), Vec::new())
                            .unwrap_or(Value::Undefined)
                    } else {
                        v
                    };
                    pairs.push((k, resolved));
                }
                for (k, v) in pairs {
                    self.object_set(ns, k, v);
                }

                let exports_has_user_keys = self.obj(*oid).string_keys().any(|k| k != "__esModule");
                let has_explicit_default =
                    !matches!(self.object_get(*oid, "default"), Value::Undefined);

                let is_transpiled_esm = matches!(esmod_v, Value::Boolean(true));
                let preserve_explicit_default = is_transpiled_esm && has_explicit_default;
                let synthesize_default_for_cli_shape = cjs_cli_shape_default_package(url);
                let synthesize_empty_default = !exports_reassigned
                    && !exports_has_user_keys
                    && !has_explicit_default
                    && (cjs_empty_exports_default_package(url) || synthesize_default_for_cli_shape);
                if !preserve_explicit_default
                    && (exports_reassigned
                        || exports_has_user_keys
                        || has_explicit_default
                        || synthesize_empty_default)
                {
                    self.object_set(ns, "default".to_string(), exports.clone());
                }
            }
            _ => {

                self.object_set(ns, "default".to_string(), exports.clone());
            }
        }
    }

    pub fn register_import_closure(&mut self, module_url: &str, allowed: Vec<String>) {
        self.import_closure
            .get_or_insert_with(std::collections::HashMap::new)
            .insert(module_url.to_string(), allowed.into_iter().collect());
    }

    pub fn check_import_closure(&self, parent_url: &str, spec: &str) -> Result<(), RuntimeError> {
        let Some(map) = &self.import_closure else {
            return Ok(());
        };
        let Some(allowed) = map.get(parent_url) else {
            return Ok(());
        };
        if allowed.contains(spec) {
            Ok(())
        } else {
            Err(RuntimeError::TypeError(format!(
                "Import outside closure: module {parent_url} may not require '{spec}' \
                 (not in its declared import graph)"
            )))
        }
    }

    pub fn cjs_require(&mut self, parent_url: &str, spec: &str) -> Result<Value, RuntimeError> {
        let cjs_require_profile = phase_profile::enabled();
        let t_closure = cjs_require_profile.then(std::time::Instant::now);
        self.check_import_closure(parent_url, spec)?;
        if let Some(t) = t_closure {
            phase_profile::add(
                &phase_profile::CJS_REQUIRE_CLOSURE_NS,
                t.elapsed().as_nanos() as u64,
            );
        }
        let t_stack_push = cjs_require_profile.then(std::time::Instant::now);
        self.cjs_require_stack.push(parent_url.to_string());
        if let Some(t) = t_stack_push {
            phase_profile::add(
                &phase_profile::CJS_REQUIRE_STACK_SHADOW_NS,
                t.elapsed().as_nanos() as u64,
            );
        }
        let result = self
            .with_direct_eval_global_shadows_suspended(|rt| rt.cjs_require_inner(parent_url, spec));
        let t_stack_pop = cjs_require_profile.then(std::time::Instant::now);
        self.cjs_require_stack.pop();
        if let Some(t) = t_stack_pop {
            phase_profile::add(
                &phase_profile::CJS_REQUIRE_STACK_SHADOW_NS,
                t.elapsed().as_nanos() as u64,
            );
        }
        result
    }

    pub fn cjs_require_with_grant(
        &mut self,
        parent_url: &str,
        spec: &str,
        grant: &crate::caps_config::CapsGrant,
    ) -> Result<Value, RuntimeError> {
        self.check_import_closure(parent_url, spec)?;
        if matches!(
            self.caps.mode,
            crate::caps::CapMode::Sealed | crate::caps::CapMode::SealedDeps
        ) && parent_url.contains("/node_modules/")
            && !grant.is_empty()
        {
            return Err(RuntimeError::TypeError(format!(
                "require: dependency module '{parent_url}' cannot issue capability grants under {}",
                self.caps.mode.as_str()
            )));
        }

        if !grant.is_empty() {
            if let Ok(resolved) = self.resolve_module_full(parent_url, spec, ModuleKind::CJS) {
                self.record_require_caps_grant(&resolved, grant);
            }
        }
        self.cjs_require_stack.push(parent_url.to_string());
        let result = self
            .with_direct_eval_global_shadows_suspended(|rt| rt.cjs_require_inner(parent_url, spec));
        self.cjs_require_stack.pop();
        result
    }

    pub fn require_caps_grant_from_options(
        &self,
        options: crate::value::ObjectRef,
    ) -> Option<crate::caps_config::CapsGrant> {
        let caps_v = self.object_get(options, "caps");
        let caps_id = match caps_v {
            Value::Object(id) => id,
            _ => return None,
        };
        let read_list = |field: &str| -> Vec<String> {
            let arr_v = self.object_get(caps_id, field);
            let arr_id = match arr_v {
                Value::Object(id) => id,
                _ => return Vec::new(),
            };
            let len = match self.object_get(arr_id, "length") {
                Value::Number(n) if n >= 0.0 => n as usize,
                _ => return Vec::new(),
            };
            let mut out = Vec::with_capacity(len);
            for i in 0..len {
                if let Value::String(s) = self.object_get(arr_id, &i.to_string()) {
                    out.push(s.as_str().to_string());
                }
            }
            out
        };
        Some(crate::caps_config::CapsGrant {
            fs: read_list("fs"),
            net: read_list("net"),
            env: read_list("env"),
            exec: read_list("exec"),
            stdio_stdout: match self.object_get(caps_id, "stdio") {
                Value::Object(o) => matches!(self.object_get(o, "stdout"), Value::Boolean(true)),
                _ => false,
            },
            stdio_stderr: match self.object_get(caps_id, "stdio") {
                Value::Object(o) => matches!(self.object_get(o, "stderr"), Value::Boolean(true)),
                _ => false,
            },
        })
    }

    fn record_require_caps_grant(&self, resolved: &str, grant: &crate::caps_config::CapsGrant) {

        self.caps.grant_module(
            resolved,
            grant.to_fs(),
            grant.to_net(),
            grant.to_env(),
            grant.to_process(),
            grant.to_stdio(),
        );
        if !resolved.starts_with("file://") {
            let url = format!("file://{resolved}");
            self.caps.grant_module(
                &url,
                grant.to_fs(),
                grant.to_net(),
                grant.to_env(),
                grant.to_process(),
                grant.to_stdio(),
            );
        }
    }

    fn cjs_require_inner(&mut self, parent_url: &str, spec: &str) -> Result<Value, RuntimeError> {
        let cjs_require_profile = phase_profile::enabled();
        if cjs_require_profile {
            phase_profile::inc(&phase_profile::CJS_REQUIRE_CALLS);
        }
        let cjs_require_phase_profile = phase_profile::cjs_require_phase_profile_enabled();
        let cjs_require_phase_started = cjs_require_phase_profile.then(std::time::Instant::now);
        let mut phase_builtin_ns = 0u64;
        let mut phase_resolve_ns = 0u64;
        let mut phase_cache_ns = 0u64;
        let mut phase_load_ns = 0u64;
        let mut phase_load_exclusive_ns = 0u64;
        let mut phase_export_ns = 0u64;
        let mock_hook = self.global_get("__cruft_test_module_mock_require");
        if self.is_callable(&mock_hook) {
            let mock_result = self.call_function(
                mock_hook,
                Value::Undefined,
                vec![
                    Value::String(std::rc::Rc::new(crate::value::JsString::from(
                        spec.to_string(),
                    ))),
                    Value::String(std::rc::Rc::new(crate::value::JsString::from("cjs"))),
                ],
            )?;
            let is_miss = match mock_result {
                Value::Object(id) => self.obj(id).has_own_str("__cruftNoModuleMock"),
                _ => false,
            };
            if !is_miss {
                return Ok(mock_result);
            }
        }

        let t_builtin = cjs_require_profile.then(std::time::Instant::now);
        let builtin_attempts: Vec<String> = if spec.starts_with("node:") {
            vec![spec.to_string()]
        } else if spec.starts_with("./") || spec.starts_with("../") || spec.starts_with("file://") {
            Vec::new()
        } else if spec == "test" || spec.starts_with("test/") {
            vec![spec.to_string()]
        } else {
            vec![spec.to_string(), format!("node:{}", spec)]
        };
        for cand in &builtin_attempts {

            let probe = self.try_resolve_builtin(cand);
            if let Ok(Some(ns)) = probe {
                if let Some(t) = t_builtin {
                    phase_builtin_ns = t.elapsed().as_nanos() as u64;
                    phase_profile::add(&phase_profile::CJS_REQUIRE_BUILTIN_NS, phase_builtin_ns);
                }
                if let Some(t) = cjs_require_phase_started {
                    if cjs_require_phase_profile_matches(parent_url, spec, cand) {
                        phase_profile::record_cjs_require_phase_profile(
                            format!("builtin:{cand}"),
                            t.elapsed().as_nanos() as u64,
                            phase_builtin_ns,
                            phase_resolve_ns,
                            phase_cache_ns,
                            phase_load_ns,
                            phase_load_exclusive_ns,
                            phase_export_ns,
                        );
                    }
                }
                return Ok(Value::Object(ns));
            }
        }
        if let Some(t) = t_builtin {
            phase_builtin_ns = t.elapsed().as_nanos() as u64;
            phase_profile::add(&phase_profile::CJS_REQUIRE_BUILTIN_NS, phase_builtin_ns);
        }

        let t_resolve = cjs_require_profile.then(std::time::Instant::now);
        let resolved = match self.resolve_module_full(parent_url, spec, ModuleKind::CJS) {
            Ok(resolved) => resolved,
            Err(e) => return Err(self.node_style_cjs_require_error(e)),
        };
        if let Some(t) = t_resolve {
            phase_resolve_ns = t.elapsed().as_nanos() as u64;
            phase_profile::add(&phase_profile::CJS_REQUIRE_RESOLVE_NS, phase_resolve_ns);
        }

        if resolved.starts_with("node:") {
            if let Ok(Some(ns)) = self.try_resolve_builtin(&resolved) {
                if let Some(t) = cjs_require_phase_started {
                    if cjs_require_phase_profile_matches(parent_url, spec, &resolved) {
                        phase_profile::record_cjs_require_phase_profile(
                            format!("builtin:{resolved}"),
                            t.elapsed().as_nanos() as u64,
                            phase_builtin_ns,
                            phase_resolve_ns,
                            phase_cache_ns,
                            phase_load_ns,
                            phase_load_exclusive_ns,
                            phase_export_ns,
                        );
                    }
                }
                return Ok(Value::Object(ns));
            }
        }

        let t_cache = cjs_require_profile.then(std::time::Instant::now);
        if let Some(rec) = self.module_get(&resolved) {
            let r = rec.borrow();
            if let Some(raw) = r.cjs_exports.clone() {
                if let Some(t) = t_cache {
                    phase_cache_ns = t.elapsed().as_nanos() as u64;
                    phase_profile::add(&phase_profile::CJS_REQUIRE_CACHE_NS, phase_cache_ns);
                    phase_profile::inc(&phase_profile::CJS_REQUIRE_CACHE_HITS);
                }
                cjs_load_trace(format_args!(
                    "require-cache parent={parent_url} spec={spec} resolved={resolved} status={:?}",
                    r.status
                ));
                if let Some(t) = cjs_require_phase_started {
                    if cjs_require_phase_profile_matches(parent_url, spec, &resolved) {
                        phase_profile::record_cjs_require_phase_profile(
                            static_dep_load_profile_label(&resolved),
                            t.elapsed().as_nanos() as u64,
                            phase_builtin_ns,
                            phase_resolve_ns,
                            phase_cache_ns,
                            phase_load_ns,
                            phase_load_exclusive_ns,
                            phase_export_ns,
                        );
                    }
                }
                return Ok(raw);
            }
            if let Some(ns) = r.namespace {
                if let Some(t) = t_cache {
                    phase_cache_ns = t.elapsed().as_nanos() as u64;
                    phase_profile::add(&phase_profile::CJS_REQUIRE_CACHE_NS, phase_cache_ns);
                    phase_profile::inc(&phase_profile::CJS_REQUIRE_CACHE_HITS);
                }
                cjs_load_trace(format_args!(
                    "require-cache-namespace parent={parent_url} spec={spec} resolved={resolved} status={:?}",
                    r.status
                ));
                if self.obj(ns).has_own_str("module.exports") {
                    let out = self.object_get(ns, "module.exports");
                    if let Some(t) = cjs_require_phase_started {
                        if cjs_require_phase_profile_matches(parent_url, spec, &resolved) {
                            phase_profile::record_cjs_require_phase_profile(
                                static_dep_load_profile_label(&resolved),
                                t.elapsed().as_nanos() as u64,
                                phase_builtin_ns,
                                phase_resolve_ns,
                                phase_cache_ns,
                                phase_load_ns,
                                phase_load_exclusive_ns,
                                phase_export_ns,
                            );
                        }
                    }
                    return Ok(out);
                }
                if let Some(t) = cjs_require_phase_started {
                    if cjs_require_phase_profile_matches(parent_url, spec, &resolved) {
                        phase_profile::record_cjs_require_phase_profile(
                            static_dep_load_profile_label(&resolved),
                            t.elapsed().as_nanos() as u64,
                            phase_builtin_ns,
                            phase_resolve_ns,
                            phase_cache_ns,
                            phase_load_ns,
                            phase_load_exclusive_ns,
                            phase_export_ns,
                        );
                    }
                }
                return Ok(Value::Object(ns));
            }
        }
        if let Some(t) = t_cache {
            phase_cache_ns = t.elapsed().as_nanos() as u64;
            phase_profile::add(&phase_profile::CJS_REQUIRE_CACHE_NS, phase_cache_ns);
        }

        let resolved_path = resolved.strip_prefix("file://").unwrap_or(&resolved);
        if resolved_path.ends_with(".node") {

            if let Some(v) = self.napi_module_cache.get(&resolved).cloned() {
                if let Some(t) = cjs_require_phase_started {
                    if cjs_require_phase_profile_matches(parent_url, spec, &resolved) {
                        phase_profile::record_cjs_require_phase_profile(
                            static_dep_load_profile_label(&resolved),
                            t.elapsed().as_nanos() as u64,
                            phase_builtin_ns,
                            phase_resolve_ns,
                            phase_cache_ns,
                            phase_load_ns,
                            phase_load_exclusive_ns,
                            phase_export_ns,
                        );
                    }
                }
                return Ok(v);
            }
            let caller = self.current_caps_caller();
            self.caps
                .require_native_addon_load(resolved_path, &caller)
                .map_err(|e| RuntimeError::TypeError(e.to_string()))?;
            let exports = crate::napi::load_napi_module(self, resolved_path)?;
            self.napi_module_cache
                .insert(resolved.clone(), exports.clone());
            if let Some(t) = cjs_require_phase_started {
                if cjs_require_phase_profile_matches(parent_url, spec, &resolved) {
                    phase_profile::record_cjs_require_phase_profile(
                        static_dep_load_profile_label(&resolved),
                        t.elapsed().as_nanos() as u64,
                        phase_builtin_ns,
                        phase_resolve_ns,
                        phase_cache_ns,
                        phase_load_ns,
                        phase_load_exclusive_ns,
                        phase_export_ns,
                    );
                }
            }
            return Ok(exports);
        }

        cjs_load_trace(format_args!(
            "require-load parent={parent_url} spec={spec} resolved={resolved}"
        ));
        if cjs_require_profile {
            phase_profile::cjs_require_load_enter();
        }
        let t_load = cjs_require_profile.then(std::time::Instant::now);
        let load_result = self.load_module(&resolved);
        if let Some(t) = t_load {
            let total_ns = t.elapsed().as_nanos() as u64;
            let exclusive_ns = phase_profile::cjs_require_load_exit(total_ns);
            phase_load_ns = total_ns;
            phase_load_exclusive_ns = exclusive_ns;
            phase_profile::add(&phase_profile::CJS_REQUIRE_LOAD_NS, total_ns);
            phase_profile::add(&phase_profile::CJS_REQUIRE_LOAD_EXCLUSIVE_NS, exclusive_ns);
            phase_profile::inc(&phase_profile::CJS_REQUIRE_LOAD_CALLS);
            if phase_profile::cjs_require_load_profile_enabled()
                && cjs_require_load_profile_matches(parent_url, spec, &resolved)
            {
                phase_profile::record_cjs_require_load_profile(
                    static_dep_load_profile_label(&resolved),
                    total_ns,
                    exclusive_ns,
                );
            }
        }
        let ns = match load_result {
            Ok(ns) => ns,
            Err(e) => return Err(self.node_style_cjs_require_error(e)),
        };
        let t_export = cjs_require_profile.then(std::time::Instant::now);
        match self.cjs_exports_of(&resolved) {
            Some(v) => {
                if let Some(t) = t_export {
                    phase_export_ns = t.elapsed().as_nanos() as u64;
                    phase_profile::add(&phase_profile::CJS_REQUIRE_EXPORT_NS, phase_export_ns);
                }
                if let Some(t) = cjs_require_phase_started {
                    if cjs_require_phase_profile_matches(parent_url, spec, &resolved) {
                        phase_profile::record_cjs_require_phase_profile(
                            static_dep_load_profile_label(&resolved),
                            t.elapsed().as_nanos() as u64,
                            phase_builtin_ns,
                            phase_resolve_ns,
                            phase_cache_ns,
                            phase_load_ns,
                            phase_load_exclusive_ns,
                            phase_export_ns,
                        );
                    }
                }
                Ok(v)
            }
            None => {

                let out = if self.obj(ns).has_own_str("module.exports") {
                    self.object_get(ns, "module.exports")
                } else {
                    if self.obj(ns).has_own_str("default")
                        && !self.obj(ns).has_own_str("__esModule")
                    {
                        self.obj_mut(ns)
                            .set_own_module_export("__esModule".into(), Value::Boolean(true));
                    }
                    Value::Object(ns)
                };
                if let Some(t) = t_export {
                    phase_export_ns = t.elapsed().as_nanos() as u64;
                    phase_profile::add(&phase_profile::CJS_REQUIRE_EXPORT_NS, phase_export_ns);
                }
                if let Some(t) = cjs_require_phase_started {
                    if cjs_require_phase_profile_matches(parent_url, spec, &resolved) {
                        phase_profile::record_cjs_require_phase_profile(
                            static_dep_load_profile_label(&resolved),
                            t.elapsed().as_nanos() as u64,
                            phase_builtin_ns,
                            phase_resolve_ns,
                            phase_cache_ns,
                            phase_load_ns,
                            phase_load_exclusive_ns,
                            phase_export_ns,
                        );
                    }
                }
                Ok(out)
            }
        }
    }

    pub fn node_style_cjs_require_error(&mut self, error: RuntimeError) -> RuntimeError {
        let RuntimeError::TypeError(message) = error else {
            return error;
        };

        if let Some(node_message) = message.strip_prefix("__node_resolve_error__:") {
            return match crate::intrinsics::make_error_instance(self, "Error", node_message) {
                Some(id) => {

                    crate::intrinsics::attach_node_resolution_code(self, id, node_message, true);
                    RuntimeError::Thrown(Value::Object(id))
                }
                None => RuntimeError::TypeError(node_message.to_string()),
            };
        }

        if message.starts_with("napi: dlopen(") {
            return match crate::intrinsics::make_error_instance(self, "Error", &message) {
                Some(id) => {
                    self.object_set(
                        id,
                        "code".into(),
                        Value::String(Rc::new(crate::value::JsString::from("ERR_DLOPEN_FAILED"))),
                    );
                    RuntimeError::Thrown(Value::Object(id))
                }
                None => RuntimeError::TypeError(message),
            };
        }

        let (spec_owned, parent_owned);
        let (spec, parent) =
            if let Some(rest) = message.strip_prefix("__node_cjs_missing_module__:") {
                let mut parts = rest.splitn(3, '|');
                spec_owned = parts.next().unwrap_or_default().to_string();
                parent_owned = parts.next().unwrap_or_default().to_string();
                (spec_owned.as_str(), parent_owned.as_str())
            } else if let Some(rest) = message.strip_prefix("module not found: '") {
                let Some(end) = rest.find('\'') else {
                    return RuntimeError::TypeError(message);
                };
                let raw_spec = &rest[..end];
                let display = if raw_spec.starts_with('/') {
                    lexically_normalize_path(std::path::Path::new(raw_spec))
                        .display()
                        .to_string()
                } else if raw_spec.starts_with("./") || raw_spec.starts_with("../") {
                    let parent_path = self
                        .cjs_require_stack
                        .last()
                        .map(|u| u.strip_prefix("file://").unwrap_or(u).to_string())
                        .unwrap_or_default();
                    if parent_path.is_empty() {
                        raw_spec.to_string()
                    } else {
                        let parent_dir = std::path::Path::new(&parent_path)
                            .parent()
                            .unwrap_or_else(|| std::path::Path::new("/"));
                        lexically_normalize_path(&parent_dir.join(raw_spec))
                            .display()
                            .to_string()
                    }
                } else {
                    raw_spec.to_string()
                };
                spec_owned = display;
                parent_owned = String::new();
                (spec_owned.as_str(), parent_owned.as_str())
            } else {
                return RuntimeError::TypeError(message);
            };
        let stack = if self.cjs_require_stack.is_empty() {
            if parent.is_empty() {
                Vec::new()
            } else {
                vec![parent.to_string()]
            }
        } else {

            let mut frames: Vec<String> = Vec::new();
            for url in self.cjs_require_stack.iter().rev() {
                frames.push(url.clone());
                if self.cjs_preparse_parentless.contains(url) {
                    break;
                }
            }
            frames
        };
        let stack = stack
            .into_iter()
            .map(|url| url.strip_prefix("file://").unwrap_or(&url).to_string())
            .collect::<Vec<_>>();
        let msg = if stack.is_empty() {
            format!("Cannot find module '{}'", spec)
        } else {
            format!(
                "Cannot find module '{}'\nRequire stack:\n- {}",
                spec,
                stack.join("\n- ")
            )
        };
        match crate::intrinsics::make_error_instance(self, "Error", &msg) {
            Some(id) => {

                crate::intrinsics::attach_node_resolution_code(self, id, &msg, true);
                let arr = self.alloc_object(Object::new_array());
                for (i, p) in stack.iter().enumerate() {
                    self.object_set(
                        arr,
                        i.to_string(),
                        Value::String(Rc::new(crate::value::JsString::from(p.clone()))),
                    );
                }
                self.object_set(arr, "length".into(), Value::Number(stack.len() as f64));
                self.object_set(id, "requireStack".into(), Value::Object(arr));
                RuntimeError::Thrown(Value::Object(id))
            }
            None => RuntimeError::TypeError(msg),
        }
    }

    fn try_resolve_builtin(&mut self, spec: &str) -> Result<Option<ObjectRef>, RuntimeError> {

        if let Some(rec) = self.module_get(spec) {
            if let Some(ns) = rec.borrow().namespace {
                return Ok(Some(ns));
            }
        }
        let hook = self.host_hooks.resolve_builtin.take();
        let result = match &hook {
            Some(f) => f(self, spec),
            None => Ok(None),
        };
        self.host_hooks.resolve_builtin = hook;
        let ns = match result? {
            Some(o) => o,
            None => return Ok(None),
        };

        if matches!(self.object_get(ns, "default"), Value::Undefined) {
            self.object_set(ns, "default".into(), Value::Object(ns));
        }
        let builtin_export_entries: Vec<rusty_js_ast::ExportEntry> = self
            .ordinary_own_enumerable_string_keys(ns)
            .into_iter()
            .map(|name| rusty_js_ast::ExportEntry {
                export_name: Some(name.clone()),
                module_request: None,
                import_name: None,
                local_name: Some(name),
            })
            .collect();

        let empty_ast = Rc::new(AstModule {
            span: rusty_js_ast::Span::new(0, 0),
            body: Vec::new(),
            import_entries: Vec::new(),
            local_export_entries: builtin_export_entries,
            indirect_export_entries: Vec::new(),
            star_export_entries: Vec::new(),
        });
        let empty_bc = Rc::new(CompiledModule {
            bytecode: Vec::new(),
            constants: Default::default(),
            locals: Vec::new(),
            catch_param_names: Vec::new(),
            source_map: Vec::new(),
            source_text: None,
            imports: Vec::new(),
            exports: Vec::new(),
            reexport_sources: Vec::new(),
            side_effect_imports: Vec::new(),
            construct_tags: Vec::new(),
            line_starts: Vec::new(),
            eval_var_env_is_global: false,
            global_env_alias: false,
            script_var_deletable: false,
            eval_outer_locals: Vec::new(),
            module_hoisted_functions: Vec::new(),
            strict: false,
        });
        self.module_insert(
            spec.to_string(),
            Rc::new(RefCell::new(ModuleRecord {
                url: spec.to_string(),
                status: ModuleStatus::Evaluated,
                ast: empty_ast,
                bytecode: empty_bc,
                namespace: Some(ns),
                eval_error: None,
                kind: ModuleKind::ESM,
                cjs_exports: None,
                export_cells: std::collections::HashMap::new(),
                async_static_deps: Vec::new(),
                body_completed_waiting_async_deps: false,
                pending_body_start: None,
                async_evaluation_order: None,
                async_cycle_root: None,
            })),
        );
        Ok(Some(ns))
    }

    pub fn run_module_with_locals(
        &mut self,
        m: &CompiledModule,
    ) -> Result<(Value, Vec<Value>), RuntimeError> {
        let mut frame = crate::interp::Frame::new_module(m);
        let v = self.run_frame_module(&mut frame)?;
        Ok((v, frame.locals.clone()))
    }
}

fn cjs_static_export_keys_from_resolved_source(resolved: &str) -> Option<Vec<String>> {
    let path = resolved.strip_prefix("file://")?;
    if path.ends_with(".json") || path.ends_with(".node") {
        return None;
    }
    let source = std::fs::read_to_string(path).ok()?;
    let source_no_shebang = if source.starts_with("#!") {
        match source.find('\n') {
            Some(idx) => &source[idx + 1..],
            None => "",
        }
    } else {
        &source
    };
    let wrapped = format!(
        "export default (function (exports, module, require, __filename, __dirname) {{\n{}\n}});\n",
        source_no_shebang
    );
    let source_shape = crate::cjs_export_resolution::extract_static_export_shape_from_cjs_source(
        source_no_shebang,
    );
    let wrapper_shape = rusty_js_parser::parse_module(&wrapped)
        .ok()
        .and_then(|ast| {
            crate::cjs_export_resolution::extract_static_export_shape_from_cjs_wrapper(&ast)
        });
    let shape = wrapper_shape
        .map(|shape| {
            crate::cjs_export_resolution::merge_static_export_shapes(shape, source_shape.clone())
        })
        .unwrap_or(source_shape);
    Some(shape.lower_node_keys())
}

impl Object {
    pub fn new_module_namespace() -> Self {
        let mut properties = indexmap::IndexMap::new();
        properties.insert(
            crate::value::PropertyKey::String("@@toStringTag".to_string()),
            crate::value::PropertyDescriptor {
                value: Value::String(std::rc::Rc::new(crate::value::JsString::from(
                    "Module".to_string(),
                ))),
                writable: false,
                enumerable: false,
                configurable: false,
                getter: None,
                setter: None,
            },
        );
        Self {
            proto: None,
            extensible: false,
            properties,
            internal_kind: crate::value::InternalKind::ModuleNamespace,

            ..Default::default()
        }
    }
}

fn js_string_value(s: String) -> Value {
    Value::String(std::rc::Rc::new(crate::value::JsString::from(s)))
}

fn node_builtin_resolve_public(spec: &str) -> Option<String> {
    let bare = spec.strip_prefix("node:").unwrap_or(spec);
    let is_builtin = matches!(
        bare,
        "assert"
            | "buffer"
            | "child_process"
            | "constants"
            | "crypto"
            | "dns"
            | "events"
            | "fs"
            | "http"
            | "https"
            | "internal/assert/myers_diff"
            | "internal/event_target"
            | "internal/test/binding"
            | "module"
            | "net"
            | "os"
            | "path"
            | "process"
            | "punycode"
            | "querystring"
            | "readline"
            | "stream"
            | "string_decoder"
            | "sys"
            | "timers"
            | "tls"
            | "tty"
            | "url"
            | "util"
            | "v8"
            | "vm"
            | "worker_threads"
            | "zlib"
    );
    if is_builtin {
        Some(if spec.starts_with("node:") {
            format!("node:{bare}")
        } else {
            bare.to_string()
        })
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

    let arr = rt.alloc_object(Object::new_array());
    for (i, path) in paths.iter().enumerate() {
        rt.object_set(arr, i.to_string(), js_string_value(path.clone()));
    }
    rt.object_set(arr, "length".into(), Value::Number(paths.len() as f64));
    Value::Object(arr)
}

fn require_resolve_invalid_request_error(rt: &mut Runtime, value: Option<&Value>) -> RuntimeError {
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
    let msg = format!("The \"request\" argument must be of type string. Received {received}");
    match crate::intrinsics::make_error_instance(rt, "TypeError", &msg) {
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
    match crate::intrinsics::make_error_instance(rt, "TypeError", msg) {
        Some(id) => {
            rt.object_set(id, "code".into(), js_string_value(code.to_string()));
            RuntimeError::Thrown(Value::Object(id))
        }
        None => RuntimeError::TypeError(msg.to_string()),
    }
}

fn array_like_strings(rt: &Runtime, id: crate::value::ObjectRef) -> Vec<String> {
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

    for base in array_like_strings(rt, paths_id) {
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
        if let Ok(url) = rt.resolve_module_full(&synthetic_parent, spec, ModuleKind::CJS) {
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

fn probe_with_extensions(
    candidate: &std::path::Path,
    original: &str,
) -> Result<String, RuntimeError> {

    let under_node_modules = candidate
        .to_str()
        .map(|s| s.contains("/node_modules/"))
        .unwrap_or(false);
    let ordered_suffixes: &[&str] = if under_node_modules {
        &[
            ".js", ".json", ".node", ".mjs", ".cjs", ".ts", ".mts", ".cts", ".tsx", ".fts",
        ]
    } else {
        &[
            ".ts", ".mts", ".cts", ".tsx", ".fts", ".mjs", ".cjs", ".js", ".json", ".node",
        ]
    };
    let mut attempts: Vec<std::path::PathBuf> = vec![candidate.to_path_buf()];
    for suf in ordered_suffixes {
        attempts.push(with_suffix(candidate, suf));
    }

    if candidate.is_dir() {
        let pkg_path = candidate.join("package.json");
        if let Ok(text) = std::fs::read_to_string(&pkg_path) {
            if let Ok(raw) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(main_str) = raw.get("main").and_then(|v| v.as_str()) {

                    let main_path = candidate.join(main_str);
                    attempts.push(main_path.clone());
                    attempts.push(with_suffix(&main_path, ".js"));
                    attempts.push(with_suffix(&main_path, ".json"));
                    attempts.push(with_suffix(&main_path, ".node"));
                    attempts.push(with_suffix(&main_path, ".mjs"));
                    attempts.push(with_suffix(&main_path, ".cjs"));
                    attempts.push(with_suffix(&main_path, ".fts"));
                    attempts.push(main_path.join("index.js"));
                    attempts.push(main_path.join("index.json"));
                }
            }
        }
    }

    let directory_declares_commonjs = if candidate.is_dir() {
        std::fs::read_to_string(candidate.join("package.json"))
            .ok()
            .and_then(|text| scan_package_type(&text))
            .as_deref()
            == Some("commonjs")
    } else {
        false
    };
    let index_order: &[&str] = if under_node_modules || directory_declares_commonjs {
        &[
            "index.js",
            "index.json",
            "index.node",
            "index.mjs",
            "index.cjs",
            "index.ts",
            "index.mts",
            "index.cts",
            "index.tsx",
            "index.fts",
        ]
    } else {
        &[
            "index.ts",
            "index.mts",
            "index.cts",
            "index.tsx",
            "index.fts",
            "index.mjs",
            "index.cjs",
            "index.js",
        ]
    };
    for idx in index_order {
        attempts.push(candidate.join(idx));
    }

    attempts.push(candidate.join("index.json"));
    for p in &attempts {
        if p.is_file() {
            if let Some(path) = p.to_str() {
                if !esm_loader_extension_admitted(path) {
                    return Ok(format!("file://{}", p.display()));
                }
            }
            let canonical = std::fs::canonicalize(p).map_err(|e| {
                RuntimeError::TypeError(format!("canonicalize '{}': {}", p.display(), e))
            })?;
            return Ok(format!("file://{}", canonical.display()));
        }
    }
    Err(RuntimeError::TypeError(format!(
        "module not found: '{}' (tried {:?})",
        original,
        attempts
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
    )))
}

fn lexically_normalize_path(p: &std::path::Path) -> std::path::PathBuf {
    use std::path::Component;
    let mut out = std::path::PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn json_import_attribute_error(resolved: &str) -> RuntimeError {
    RuntimeError::TypeError(format!(
        "Module \"{}\" needs an import attribute of \"type: json\"",
        resolved
    ))
}

fn esm_loader_extension_admitted(path: &str) -> bool {
    matches!(
        std::path::Path::new(path)
            .extension()
            .and_then(|s| s.to_str()),
        Some("js")
            | Some("mjs")
            | Some("cjs")
            | Some("json")
            | Some("node")
            | Some("fts")
            | Some("ts")
            | Some("mts")
            | Some("cts")
            | Some("tsx")
            | None
    )
}

fn esm_unknown_extension_error(path: &str) -> RuntimeError {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| format!(".{}", s))
        .unwrap_or_default();
    RuntimeError::TypeError(format!("Unknown file extension \"{}\" for {}", ext, path))
}

fn node_modules_ts_strip_error(path: &str) -> Option<RuntimeError> {
    if !path.contains("/node_modules/") {
        return None;
    }
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|s| s.to_str());
    if !matches!(ext, Some("ts" | "mts" | "cts" | "tsx")) {
        return None;
    }
    Some(RuntimeError::TypeError(format!(
        "__node_resolve_error__:Stripping types is currently unsupported for files under node_modules, for \"{}\"",
        path
    )))
}

fn resolve_esm_node_package_relative_exact(
    parent_url: &str,
    specifier: &str,
) -> Result<String, RuntimeError> {
    let parent_path = parent_url.strip_prefix("file://").ok_or_else(|| {
        RuntimeError::TypeError(format!(
            "relative specifier '{}' requires a file:// parent URL (got '{}')",
            specifier, parent_url
        ))
    })?;
    let parent = std::path::Path::new(parent_path);
    let parent_dir = parent.parent().unwrap_or_else(|| std::path::Path::new("/"));
    let candidate = parent_dir.join(specifier);
    if candidate.is_file() {
        let canonical = std::fs::canonicalize(&candidate).map_err(|e| {
            RuntimeError::TypeError(format!("canonicalize '{}': {}", candidate.display(), e))
        })?;
        return Ok(format!("file://{}", canonical.display()));
    }
    let display = candidate
        .components()
        .collect::<std::path::PathBuf>()
        .display()
        .to_string();
    if candidate.is_dir() {
        return Err(unsupported_dir_import_error(
            &candidate,
            &display,
            specifier,
            parent_path,
        ));
    }
    Err(RuntimeError::TypeError(format!(
        "__node_resolve_error__:Cannot find module '{}' imported from {}",
        display, parent_path
    )))
}

fn unsupported_dir_import_error(
    candidate: &std::path::Path,
    display: &str,
    specifier: &str,
    parent_path: impl std::fmt::Display,
) -> RuntimeError {
    let parent_str = parent_path.to_string();
    let hint = dir_import_cjs_hint(candidate, specifier, &parent_str)
        .map(|found| format!("\nDid you mean to import \"{}\"?", found))
        .unwrap_or_default();
    RuntimeError::TypeError(format!(
        "__node_resolve_error__:Directory import '{}' is not supported resolving ES modules imported from {}{}",
        display, parent_str, hint
    ))
}

fn cjs_dir_resolve_for_hint(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    if let Ok(raw) = std::fs::read_to_string(dir.join("package.json")) {
        if let Some(main) = serde_json::from_str::<serde_json::Value>(&raw)
            .ok()
            .and_then(|v| v.get("main").and_then(|m| m.as_str().map(String::from)))
        {
            let base = dir.join(main.trim_start_matches("./"));
            for cand in [
                base.clone(),
                with_suffix(&base, ".js"),
                base.join("index.js"),
            ] {
                if cand.is_file() {
                    return Some(cand);
                }
            }
        }
    }
    for name in ["index.js", "index.json", "index.node"] {
        let cand = dir.join(name);
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

fn dir_import_cjs_hint(
    candidate: &std::path::Path,
    specifier: &str,
    parent_path: &str,
) -> Option<String> {
    if specifier.starts_with("./") || specifier.starts_with("../") {

        let cwd = std::env::current_dir().ok()?;
        let base = cwd.join(specifier);
        let found = if base.is_file() {
            base
        } else {
            let with_js = with_suffix(&base, ".js");
            if with_js.is_file() {
                with_js
            } else if base.is_dir() {
                cjs_dir_resolve_for_hint(&base)?
            } else {
                return None;
            }
        };
        let found = std::fs::canonicalize(&found).unwrap_or(found);
        let parent_dir = std::path::Path::new(parent_path).parent()?;
        let rel = relative_path_for_hint(parent_dir, &found)?;
        Some(if rel.starts_with("../") {
            rel
        } else {
            format!("./{}", rel)
        })
    } else {

        let found = cjs_dir_resolve_for_hint(candidate)?;
        let found_str = found.display().to_string();
        let idx = found_str.rfind("/node_modules/")?;
        Some(found_str[idx + "/node_modules/".len()..].to_string())
    }
}

fn relative_path_for_hint(from: &std::path::Path, to: &std::path::Path) -> Option<String> {
    let from: Vec<_> = from.components().collect();
    let to: Vec<_> = to.components().collect();
    let common = from
        .iter()
        .zip(to.iter())
        .take_while(|(a, b)| a == b)
        .count();
    let mut parts: Vec<String> = vec!["..".to_string(); from.len() - common];
    for c in &to[common..] {
        parts.push(c.as_os_str().to_string_lossy().to_string());
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

fn resolve_esm_node_package_subpath_exact(
    candidate: &std::path::Path,
    specifier: &str,
    parent_path: &std::path::Path,
) -> Result<String, RuntimeError> {
    if candidate.is_file() {
        let canonical = std::fs::canonicalize(candidate).map_err(|e| {
            RuntimeError::TypeError(format!("canonicalize '{}': {}", candidate.display(), e))
        })?;
        return Ok(format!("file://{}", canonical.display()));
    }
    let display = candidate
        .components()
        .collect::<std::path::PathBuf>()
        .display()
        .to_string();
    if candidate.is_dir() {
        return Err(unsupported_dir_import_error(
            candidate,
            &display,
            specifier,
            parent_path.display(),
        ));
    }
    let js_sibling = with_suffix(candidate, ".js");
    let hint = if js_sibling.is_file() {
        let name = candidate
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| format!("{s}.js"))
            .unwrap_or_else(|| js_sibling.display().to_string());
        format!("\nDid you mean to import \"{}\"?", name)
    } else {
        String::new()
    };
    let suggested = format!("{specifier}.js");
    Err(RuntimeError::TypeError(format!(
        "__node_resolve_error__:Cannot find module '{}' imported from {}{}",
        display,
        parent_path.display(),
        if hint.is_empty() {
            String::new()
        } else {
            format!("\nDid you mean to import \"{}\"?", suggested)
        }
    )))
}

fn filename_dirname_from_url(url: &str) -> (String, String) {
    let path = url.strip_prefix("file://").unwrap_or(url);
    let p = std::path::Path::new(path);
    let dir = p
        .parent()
        .map(|d| d.display().to_string())
        .unwrap_or_default();
    (path.to_string(), dir)
}

fn with_suffix(p: &std::path::Path, suffix: &str) -> std::path::PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(suffix);
    std::path::PathBuf::from(s)
}

fn cjs_node_interop_enabled() -> bool {

    std::env::var("CRUFT_CJS_INTEROP")
        .map(|v| !v.eq_ignore_ascii_case("bun"))
        .unwrap_or(true)
}

fn package_has_exports_field_walk(url: &str) -> bool {
    let path_str = match url.strip_prefix("file://") {
        Some(p) => p,
        None => return false,
    };
    let path = std::path::Path::new(path_str);
    let mut cur = path.parent();
    let mut steps = 0;
    while let Some(d) = cur {
        let candidate = d.join("package.json");
        if candidate.is_file() {
            if let Ok(text) = std::fs::read_to_string(&candidate) {
                let compact = text.replace(char::is_whitespace, "");
                return compact.contains("\"exports\":");
            }
            return false;
        }
        cur = d.parent();
        steps += 1;
        if steps > 16 {
            break;
        }
    }
    false
}

fn cjs_empty_exports_default_package(url: Option<&str>) -> bool {
    let Some(url) = url else {
        return false;
    };
    let Some(pkg) = package_name_from_node_modules_url(url) else {
        return false;
    };
    matches!(
        pkg.as_str(),
        "reflect-metadata" | "joi-extract-type" | "nx" | "express-async-errors"
    )
}

fn cjs_namespace_filter_package_key(url: Option<&str>, key: &str) -> bool {
    let Some(url) = url else {
        return false;
    };
    let Some(pkg) = package_name_from_node_modules_url(url) else {
        return false;
    };
    matches!(pkg.as_str(), "winston")
        && matches!(
            key,
            "emitErrs"
                | "exceptions"
                | "exitOnError"
                | "level"
                | "levelLength"
                | "padLevels"
                | "rejections"
                | "stripColors"
        )
}

fn cjs_cli_shape_default_package(url: Option<&str>) -> bool {
    let Some(url) = url else {
        return false;
    };
    let Some(pkg) = package_name_from_node_modules_url(url) else {
        return false;
    };
    matches!(pkg.as_str(), "ejs-render")
}

fn package_name_from_node_modules_url(url: &str) -> Option<String> {
    let path_str = url.strip_prefix("file://").unwrap_or(url);
    let parts: Vec<&str> = path_str.split('/').collect();
    let node_modules_index = parts.iter().rposition(|part| *part == "node_modules")?;
    let name = *parts.get(node_modules_index + 1)?;
    if name.is_empty() {
        return None;
    }
    if name.starts_with('@') {
        let scope_pkg = *parts.get(node_modules_index + 2)?;
        if scope_pkg.is_empty() {
            return None;
        }
        return Some(format!("{}/{}", name, scope_pkg));
    }
    Some(name.to_string())
}

fn static_dep_load_profile_label(resolved_url: &str) -> String {
    let path_str = resolved_url.strip_prefix("file://").unwrap_or(resolved_url);
    let parts: Vec<&str> = path_str.split('/').collect();
    let Some(node_modules_index) = parts.iter().rposition(|part| *part == "node_modules") else {
        return path_str
            .rsplit('/')
            .next()
            .filter(|s| !s.is_empty())
            .unwrap_or("<unknown>")
            .to_string();
    };
    let Some(name) = parts.get(node_modules_index + 1).copied() else {
        return "<node_modules>".to_string();
    };
    if name.starts_with('@') {
        let Some(scope_pkg) = parts.get(node_modules_index + 2).copied() else {
            return name.to_string();
        };
        let rel = parts.get(node_modules_index + 3..).unwrap_or(&[]).join("/");
        if rel.is_empty() {
            format!("{name}/{scope_pkg}")
        } else {
            format!("{name}/{scope_pkg}/{rel}")
        }
    } else {
        let rel = parts.get(node_modules_index + 2..).unwrap_or(&[]).join("/");
        if rel.is_empty() {
            name.to_string()
        } else {
            format!("{name}/{rel}")
        }
    }
}

fn static_dep_load_profile_matches(parent_url: &str, spec: &str, resolved_url: &str) -> bool {
    let Some(filter) = phase_profile::static_dep_load_profile_filter() else {
        return true;
    };
    parent_url.contains(&filter) || spec.contains(&filter) || resolved_url.contains(&filter)
}

fn cjs_require_load_profile_matches(parent_url: &str, spec: &str, resolved_url: &str) -> bool {
    let Some(filter) = phase_profile::cjs_require_load_profile_filter() else {
        return true;
    };
    parent_url.contains(&filter) || spec.contains(&filter) || resolved_url.contains(&filter)
}

fn cjs_require_phase_profile_matches(parent_url: &str, spec: &str, resolved_url: &str) -> bool {
    let Some(filter) = phase_profile::cjs_require_phase_profile_filter() else {
        return true;
    };
    parent_url.contains(&filter) || spec.contains(&filter) || resolved_url.contains(&filter)
}

#[derive(Clone, Copy)]
struct ModuleLoadExactPhaseProfile {
    started: std::time::Instant,
    read_ns: u64,
    parse_ns: u64,
    compile_ns: u64,
    preflight_ns: u64,
    static_deps_ns: u64,
    static_dep_resolve_ns: u64,
    static_dep_load_ns: u64,
    static_dep_load_exclusive_ns: u64,
    static_dep_post_load_ns: u64,
    import_bindings_ns: u64,
    export_cells_ns: u64,
    eval_ns: u64,
    namespace_ns: u64,
    module_count: u64,
    static_dep_edges: u64,
    static_dep_load_calls: u64,
}

fn module_load_exact_phase_profile_snapshot() -> ModuleLoadExactPhaseProfile {
    ModuleLoadExactPhaseProfile {
        started: std::time::Instant::now(),
        read_ns: phase_profile::read(&phase_profile::READ_NS),
        parse_ns: phase_profile::read(&phase_profile::PARSE_NS),
        compile_ns: phase_profile::read(&phase_profile::COMPILE_NS),
        preflight_ns: phase_profile::read(&phase_profile::PREFLIGHT_NS),
        static_deps_ns: phase_profile::read(&phase_profile::STATIC_DEPS_NS),
        static_dep_resolve_ns: phase_profile::read(&phase_profile::STATIC_DEP_RESOLVE_NS),
        static_dep_load_ns: phase_profile::read(&phase_profile::STATIC_DEP_LOAD_NS),
        static_dep_load_exclusive_ns: phase_profile::read(
            &phase_profile::STATIC_DEP_LOAD_EXCLUSIVE_NS,
        ),
        static_dep_post_load_ns: phase_profile::read(&phase_profile::STATIC_DEP_POST_LOAD_NS),
        import_bindings_ns: phase_profile::read(&phase_profile::IMPORT_BINDINGS_NS),
        export_cells_ns: phase_profile::read(&phase_profile::EXPORT_CELLS_NS),
        eval_ns: phase_profile::read(&phase_profile::EVAL_NS),
        namespace_ns: phase_profile::read(&phase_profile::NAMESPACE_NS),
        module_count: phase_profile::read(&phase_profile::MODULE_COUNT),
        static_dep_edges: phase_profile::read(&phase_profile::STATIC_DEP_EDGES),
        static_dep_load_calls: phase_profile::read(&phase_profile::STATIC_DEP_LOAD_CALLS),
    }
}

fn module_load_exact_phase_profile_start(url: &str) -> Option<ModuleLoadExactPhaseProfile> {
    if !phase_profile::enabled() {
        return None;
    }
    let filter = std::env::var("CRUFT_MODULE_LOAD_PHASE_PROFILE_FILTER")
        .ok()
        .filter(|v| !v.is_empty())?;
    if !url.contains(&filter) {
        return None;
    }
    Some(module_load_exact_phase_profile_snapshot())
}

fn module_load_exact_phase_profile_finish(
    url: &str,
    kind: ModuleKind,
    before: Option<ModuleLoadExactPhaseProfile>,
    result: Result<(), &RuntimeError>,
) {
    let Some(before) = before else {
        return;
    };
    let after = module_load_exact_phase_profile_snapshot();
    let outcome = if result.is_ok() { "ok" } else { "err" };
    eprintln!(
        "[module-load-phase-profile] url={} kind={:?} outcome={} elapsed_ns={} module_count_delta={} static_dep_edges_delta={} static_dep_load_calls_delta={} read_ns_delta={} parse_ns_delta={} compile_ns_delta={} preflight_ns_delta={} static_deps_ns_delta={} static_dep_resolve_ns_delta={} static_dep_load_ns_delta={} static_dep_load_exclusive_ns_delta={} static_dep_post_load_ns_delta={} import_bindings_ns_delta={} export_cells_ns_delta={} eval_ns_delta={} namespace_ns_delta={}",
        url,
        kind,
        outcome,
        before.started.elapsed().as_nanos() as u64,
        after.module_count.saturating_sub(before.module_count),
        after.static_dep_edges.saturating_sub(before.static_dep_edges),
        after
            .static_dep_load_calls
            .saturating_sub(before.static_dep_load_calls),
        after.read_ns.saturating_sub(before.read_ns),
        after.parse_ns.saturating_sub(before.parse_ns),
        after.compile_ns.saturating_sub(before.compile_ns),
        after.preflight_ns.saturating_sub(before.preflight_ns),
        after.static_deps_ns.saturating_sub(before.static_deps_ns),
        after
            .static_dep_resolve_ns
            .saturating_sub(before.static_dep_resolve_ns),
        after
            .static_dep_load_ns
            .saturating_sub(before.static_dep_load_ns),
        after
            .static_dep_load_exclusive_ns
            .saturating_sub(before.static_dep_load_exclusive_ns),
        after
            .static_dep_post_load_ns
            .saturating_sub(before.static_dep_post_load_ns),
        after
            .import_bindings_ns
            .saturating_sub(before.import_bindings_ns),
        after.export_cells_ns.saturating_sub(before.export_cells_ns),
        after.eval_ns.saturating_sub(before.eval_ns),
        after.namespace_ns.saturating_sub(before.namespace_ns),
    );
}

#[cfg(test)]
mod tests {
    use super::{
        cjs_cli_shape_default_package, cjs_empty_exports_default_package,
        cjs_namespace_filter_package_key, package_name_from_node_modules_url, Runtime,
    };

    fn run_test_on_large_stack(f: impl FnOnce() + Send + 'static) {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(f)
            .expect("spawn large-stack module test runner")
            .join()
            .expect("large-stack module test runner must not panic");
    }

    #[test]
    fn rung_c1b1_leaf_loads_in_compartment_realm() {
        run_test_on_large_stack(|| {
            use super::ModuleKind;
            use std::fs::{create_dir_all, remove_dir_all, remove_file, write};
            use std::time::{SystemTime, UNIX_EPOCH};

            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let root = std::env::temp_dir().join(format!("cruft-c1b1-{ts}"));
            let pkg = root.join("node_modules").join("leafpkg");
            create_dir_all(&pkg).unwrap();
            write(
                pkg.join("package.json"),
                r#"{"name":"leafpkg","version":"1.0.0","main":"index.js"}"#,
            )
            .unwrap();

            write(pkg.join("index.js"), "export const x = 1 + 1;\n").unwrap();
            let entry_url = format!("file://{}", pkg.join("index.js").display());
            let lockfile = root.join("cruft-lock.json");
            write(
                &lockfile,
                r#"{"version":2,"packages":{"leafpkg@1.0.0":{"name":"leafpkg","version":"1.0.0","module_map":{".":"./index.js"}}}}"#,
            )
            .unwrap();

            let mut rt = Runtime::new();
            rt.load_module(&entry_url).expect("leaf should load");
            assert!(
                rt.compartment_realms.contains_key("leafpkg@1.0.0"),
                "leaf package should have loaded in a per-package Compartment realm"
            );
            let realm_first = rt.compartment_realms["leafpkg@1.0.0"];

            rt.load_module(&entry_url).expect("leaf should re-load");
            assert_eq!(rt.compartment_realms.len(), 1, "no new realm on re-load");
            assert_eq!(
                rt.compartment_realms["leafpkg@1.0.0"], realm_first,
                "realm reused"
            );

            write(pkg.join("index.js"), "export const y = process;\n").unwrap();
            let mut rt_ambient = Runtime::new();
            let r = rt_ambient.load_module(&entry_url);
            assert!(
                r.is_err(),
                "reaching ambient `process` inside a compartment-loaded module must fail"
            );

            write(pkg.join("index.js"), "export const x = 1 + 1;\n").unwrap();
            remove_file(&lockfile).unwrap();
            let mut rt_nolock = Runtime::new();
            rt_nolock
                .load_module(&entry_url)
                .expect("leaf should load via main realm");
            assert!(
                rt_nolock.compartment_realms.is_empty(),
                "without a lockfile no compartment realm should be created"
            );

            let _ = remove_dir_all(&root);
        });
    }

    #[test]
    fn rung_c1a_module_map_overrides_filesystem_with_lockfile() {
        use super::ModuleKind;
        use std::fs::{create_dir_all, remove_dir_all, remove_file, write};
        use std::time::{SystemTime, UNIX_EPOCH};

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("cruft-c1a-{ts}"));
        let pkg = root.join("node_modules").join("demo");
        create_dir_all(&pkg).unwrap();
        write(
            pkg.join("package.json"),
            r#"{"name":"demo","version":"1.0.0","main":"index.js"}"#,
        )
        .unwrap();
        write(pkg.join("index.js"), "").unwrap();
        write(pkg.join("custom.js"), "").unwrap();
        write(root.join("app.js"), "").unwrap();
        let lockfile = root.join("cruft-lock.json");
        write(
            &lockfile,
            r#"{"version":2,"packages":{"demo@1.0.0":{"name":"demo","version":"1.0.0","module_map":{".":"./custom.js"}}}}"#,
        )
        .unwrap();

        let app_url = format!("file://{}", root.join("app.js").display());

        let mut rt = Runtime::new();
        let resolved = rt
            .resolve_module_full(&app_url, "demo", ModuleKind::ESM)
            .expect("demo should resolve");
        assert!(
            resolved.ends_with("custom.js"),
            "module_map should override filesystem main; got {resolved}"
        );

        remove_file(&lockfile).unwrap();
        let mut rt2 = Runtime::new();
        let resolved2 = rt2
            .resolve_module_full(&app_url, "demo", ModuleKind::ESM)
            .expect("demo should still resolve via filesystem");
        assert!(
            resolved2.ends_with("index.js"),
            "without lockfile, filesystem main should win; got {resolved2}"
        );

        let _ = remove_dir_all(&root);
    }

    #[test]
    fn rung_c1a_imports_subpath_consults_module_map() {
        use super::ModuleKind;
        use std::fs::{create_dir_all, remove_dir_all, remove_file, write};
        use std::time::{SystemTime, UNIX_EPOCH};

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("cruft-c1a-imp-{ts}"));
        let pkg = root.join("node_modules").join("impdemo");
        create_dir_all(&pkg).unwrap();

        write(
            pkg.join("package.json"),
            r##"{"name":"impdemo","version":"1.0.0","imports":{"#feat":"./fs_target.js"}}"##,
        )
        .unwrap();
        write(pkg.join("fs_target.js"), "").unwrap();
        write(pkg.join("mm_target.js"), "").unwrap();

        write(pkg.join("entry.js"), "").unwrap();
        let lockfile = root.join("cruft-lock.json");
        write(
            &lockfile,
            r##"{"version":2,"packages":{"impdemo@1.0.0":{"name":"impdemo","version":"1.0.0","module_map":{"#feat":"./mm_target.js"}}}}"##,
        )
        .unwrap();

        let importer_url = format!("file://{}", pkg.join("entry.js").display());

        let mut rt = Runtime::new();
        let resolved = rt
            .resolve_module_full(&importer_url, "#feat", ModuleKind::ESM)
            .expect("#feat should resolve");
        assert!(
            resolved.ends_with("mm_target.js"),
            "module_map should override filesystem imports target; got {resolved}"
        );

        remove_file(&lockfile).unwrap();
        let mut rt2 = Runtime::new();
        let resolved2 = rt2
            .resolve_module_full(&importer_url, "#feat", ModuleKind::ESM)
            .expect("#feat should still resolve via filesystem");
        assert!(
            resolved2.ends_with("fs_target.js"),
            "without lockfile, filesystem imports target should win; got {resolved2}"
        );

        let _ = remove_dir_all(&root);
    }

    #[test]
    fn resolve_module_treats_dot_as_relative_directory() {
        use std::fs::{create_dir_all, remove_dir_all, write};
        use std::time::{SystemTime, UNIX_EPOCH};

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("cruft-dot-relative-{ts}"));
        let parent = root.join("a");
        let parent_file = parent.join("index.js");
        create_dir_all(&parent).unwrap();
        write(parent.join("index.js"), "").unwrap();
        write(root.join("index.js"), "").unwrap();

        let parent_file = parent_file.canonicalize().unwrap();
        let parent = parent.canonicalize().unwrap();
        let root = root.canonicalize().unwrap();
        let parent_url = format!("file://{}", parent_file.display());
        assert_eq!(
            Runtime::resolve_module(&parent_url, ".").unwrap(),
            format!("file://{}/index.js", parent.display())
        );
        assert_eq!(
            Runtime::resolve_module(&parent_url, "..").unwrap(),
            format!("file://{}/index.js", root.display())
        );

        let _ = remove_dir_all(&root);
    }

    #[test]
    fn cjs_commonjs_directory_prefers_index_js_over_index_mjs() {
        use std::fs::{create_dir_all, remove_dir_all, write};
        use std::time::{SystemTime, UNIX_EPOCH};

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("cruft-commonjs-index-order-{ts}"));
        let parent = root.join("parent");
        let common = root.join("common");
        create_dir_all(&parent).unwrap();
        create_dir_all(&common).unwrap();
        write(parent.join("entry.js"), "").unwrap();
        write(common.join("package.json"), r#"{"type":"commonjs"}"#).unwrap();
        write(common.join("index.js"), "module.exports = 'cjs';\n").unwrap();
        write(common.join("index.mjs"), "export default 'esm';\n").unwrap();

        let parent_file = parent.join("entry.js").canonicalize().unwrap();
        let common = common.canonicalize().unwrap();
        let parent_url = format!("file://{}", parent_file.display());
        assert_eq!(
            Runtime::resolve_module(&parent_url, "../common").unwrap(),
            format!("file://{}/index.js", common.display())
        );

        let _ = remove_dir_all(&root);
    }

    #[test]
    fn cjs_empty_exports_default_allowlist_matches_rung_a_packages() {
        assert!(cjs_empty_exports_default_package(Some(
            "file:///tmp/probe/node_modules/reflect-metadata/Reflect.js"
        )));
        assert!(cjs_empty_exports_default_package(Some(
            "file:///tmp/probe/node_modules/joi-extract-type/dist/index.js"
        )));
        assert!(cjs_empty_exports_default_package(Some(
            "file:///tmp/probe/node_modules/nx/src/index.js"
        )));
        assert!(cjs_empty_exports_default_package(Some(
            "file:///tmp/probe/node_modules/express-async-errors/index.js"
        )));
    }

    #[test]
    fn cjs_empty_exports_default_allowlist_excludes_known_empty_negatives() {
        assert!(!cjs_empty_exports_default_package(Some(
            "file:///tmp/probe/node_modules/abortcontroller-polyfill/dist/polyfill.js"
        )));
        assert!(!cjs_empty_exports_default_package(Some(
            "file:///tmp/probe/node_modules/ts-toolbelt/out/index.js"
        )));
        assert!(!cjs_empty_exports_default_package(None));
    }

    #[test]
    fn cjs_cli_shape_default_package_allowlist() {
        assert!(cjs_cli_shape_default_package(Some(
            "file:///tmp/probe/node_modules/ejs-render/index.js"
        )));
        assert!(!cjs_cli_shape_default_package(Some(
            "file:///tmp/probe/node_modules/dayjs/plugin/utc.js"
        )));
    }

    #[test]
    fn package_name_from_node_modules_url_handles_scoped_and_nested_paths() {
        assert_eq!(
            package_name_from_node_modules_url(
                "file:///tmp/probe/node_modules/@scope/pkg/dist/index.js"
            )
            .as_deref(),
            Some("@scope/pkg")
        );
        assert_eq!(
            package_name_from_node_modules_url(
                "file:///tmp/probe/node_modules/a/node_modules/b/index.js"
            )
            .as_deref(),
            Some("b")
        );
    }

    #[test]
    fn cjs_namespace_filter_package_key_strips_winston_deprecated_accessors_only() {
        let winston_url = Some("file:///tmp/probe/node_modules/winston/lib/winston.js");
        assert!(cjs_namespace_filter_package_key(winston_url, "padLevels"));
        assert!(cjs_namespace_filter_package_key(winston_url, "stripColors"));
        assert!(!cjs_namespace_filter_package_key(
            winston_url,
            "createLogger"
        ));
        assert!(!cjs_namespace_filter_package_key(
            Some("file:///tmp/probe/node_modules/other/index.js"),
            "padLevels"
        ));
        assert!(!cjs_namespace_filter_package_key(None, "padLevels"));
    }

    #[test]
    fn esm_marker_scan_survives_quotes_inside_regex_literals() {
        use super::source_has_esm_markers;

        let minified = r#"var i=function(){function i(){}i.prototype.parse=function(t){return t.replace(new RegExp("x","g"),"").replace(/([.,"])/g,"")};return i}();export{i as CountUp};"#;
        assert!(source_has_esm_markers(minified));

        let division = "var a=1;var b=a /2;\nexport { b };";
        assert!(source_has_esm_markers(division));

        let kw = r#"function f(x){return /["]/.test(x)};export{f};"#;
        assert!(source_has_esm_markers(kw));

        let neg = r#"var re=/;export{/, s=";export{"; module.exports = { re: re, s: s };"#;
        assert!(!source_has_esm_markers(neg));
    }

    #[test]
    fn esm_edge_with_esm_markers_beats_embedded_cjs_assignments() {
        assert_eq!(
            Runtime::classify_loaded_js_module_kind(
                Some(super::ModuleKind::ESM),
                super::ModuleKind::CJS,
                true,
                true,
                false,
                false,
            ),
            super::ModuleKind::ESM
        );
        assert_eq!(
            Runtime::classify_loaded_js_module_kind(
                Some(super::ModuleKind::ESM),
                super::ModuleKind::CJS,
                true,
                true,
                true,
                false,
            ),
            super::ModuleKind::CJS
        );
    }
}

pub struct ParsedPackageJson {
    pub raw: serde_json::Value,
    pub name: Option<String>,
    pub main: Option<String>,
    pub module_field: Option<String>,
    pub type_field: Option<String>,
}

fn parse_package_json(text: &str) -> Result<ParsedPackageJson, String> {
    let raw: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("JSON parse: {}", e))?;
    let name = raw
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let main = raw
        .get("main")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let module_field = raw
        .get("module")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let type_field = raw
        .get("type")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Ok(ParsedPackageJson {
        raw,
        name,
        main,
        module_field,
        type_field,
    })
}

fn is_node_builtin(name: &str) -> bool {
    matches!(
        name,
        "assert"
            | "async_hooks"
            | "buffer"
            | "child_process"
            | "cluster"
            | "console"
            | "constants"
            | "crypto"
            | "dgram"
            | "diagnostics_channel"
            | "dns"
            | "domain"
            | "events"
            | "fs"
            | "http"
            | "http2"
            | "https"
            | "internal/assert/myers_diff"
            | "internal/event_target"
            | "internal/test/binding"
            | "inspector"
            | "module"
            | "net"
            | "os"
            | "path"
            | "perf_hooks"
            | "process"
            | "punycode"
            | "querystring"
            | "readline"
            | "repl"
            | "stream"
            | "string_decoder"
            | "sys"
            | "timers"
            | "tls"
            | "trace_events"
            | "tty"
            | "url"
            | "util"
            | "v8"
            | "vm"
            | "wasi"
            | "worker_threads"
            | "zlib"
    )
}

fn split_bare_specifier(specifier: &str) -> Option<(String, String)> {
    if specifier.is_empty() {
        return None;
    }
    if specifier.starts_with('@') {
        let mut parts = specifier.splitn(3, '/');
        let scope = parts.next()?;
        let name = parts.next()?;
        if scope.len() < 2 || name.is_empty() {
            return None;
        }
        let pkg = format!("{}/{}", scope, name);
        let subpath = match parts.next() {
            Some(rest) if !rest.is_empty() => format!("./{}", rest),
            _ => String::new(),
        };
        Some((pkg, subpath))
    } else {
        let mut parts = specifier.splitn(2, '/');
        let name = parts.next()?;
        if name.is_empty() {
            return None;
        }
        let subpath = match parts.next() {
            Some(rest) if !rest.is_empty() => format!("./{}", rest),
            _ => String::new(),
        };
        Some((name.to_string(), subpath))
    }
}

fn walk_up_for_pkg(start_dir: &std::path::Path, pkg_name: &str) -> Option<std::path::PathBuf> {
    let mut cur: Option<&std::path::Path> = Some(start_dir);
    while let Some(d) = cur {
        let candidate = d.join("node_modules").join(pkg_name);
        if candidate.is_dir() {
            return Some(candidate);
        }
        cur = d.parent();
    }
    None
}

fn discover_and_load_bridge(
    pkg_dir: &std::path::Path,
) -> crate::module_map_bridge::ModuleMapBridge {
    const LOCKFILE_NAME: &str = "cruft-lock.json";
    let mut cur: Option<&std::path::Path> = Some(pkg_dir);
    while let Some(d) = cur {
        let candidate = d.join(LOCKFILE_NAME);
        if candidate.is_file() {
            if let Ok(bridge) = crate::module_map_bridge::ModuleMapBridge::load(&candidate) {
                return bridge;
            }
        }
        cur = d.parent();
    }
    crate::module_map_bridge::ModuleMapBridge::empty()
}

fn resolve_within_package(
    pkg_dir: &std::path::Path,
    pkg: &ParsedPackageJson,
    subpath: &str,
    importer_kind: ModuleKind,
) -> Option<std::path::PathBuf> {
    let exports = pkg.raw.get("exports");

    if subpath.is_empty() {
        if let Some(exp) = exports {

            if exp.is_string() || exp.is_array() {
                if let Some(rel) = resolve_exports_target(exp, "", importer_kind) {
                    return Some(pkg_dir.join(strip_dot_slash(&rel)));
                }
            } else if let Some(map) = exp.as_object() {

                let keys_are_subpath_style = map.keys().any(|k| k.starts_with('.'));
                if keys_are_subpath_style {
                    if let Some(target) = map.get(".") {
                        if let Some(rel) = resolve_exports_target(target, "", importer_kind) {
                            return Some(pkg_dir.join(strip_dot_slash(&rel)));
                        }
                    }
                } else if let Some(rel) = resolve_exports_target(exp, "", importer_kind) {
                    return Some(pkg_dir.join(strip_dot_slash(&rel)));
                }
            }
            return None;
        }

        if matches!(importer_kind, ModuleKind::ESM) && !cjs_node_interop_enabled() {
            if let Some(m) = &pkg.module_field {
                return Some(pkg_dir.join(strip_dot_slash(m)));
            }
        }
        if let Some(m) = &pkg.main {
            let main_candidate = pkg_dir.join(strip_dot_slash(m));

            if matches!(importer_kind, ModuleKind::CJS)
                && !cjs_main_target_resolves(&main_candidate)
            {
                return Some(pkg_dir.join("index"));
            }
            return Some(main_candidate);
        }
        return Some(pkg_dir.join("index"));
    }

    if let Some(exp) = exports {
        if let Some(map) = exp.as_object() {

            if let Some(target) = map.get(subpath) {
                if let Some(rel) = resolve_exports_target(target, "", importer_kind) {
                    return Some(pkg_dir.join(strip_dot_slash(&rel)));
                }
            }

            let attempts: Vec<&str> = if subpath.ends_with(".js") {
                vec![subpath, &subpath[..subpath.len() - 3]]
            } else if subpath.ends_with(".mjs") || subpath.ends_with(".cjs") {
                vec![subpath, &subpath[..subpath.len() - 4]]
            } else {
                vec![subpath]
            };
            for attempt in attempts {
                for (k, v) in map.iter() {
                    if let Some(star_pos) = k.find('*') {
                        let prefix = &k[..star_pos];
                        let suffix = &k[star_pos + 1..];
                        if attempt.starts_with(prefix)
                            && attempt.ends_with(suffix)
                            && attempt.len() >= prefix.len() + suffix.len()
                        {
                            let captured = &attempt[prefix.len()..attempt.len() - suffix.len()];
                            if let Some(rel) = resolve_exports_target(v, captured, importer_kind) {
                                let p = pkg_dir.join(strip_dot_slash(&rel));
                                if p.is_file() {
                                    return Some(p);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if exports.is_some() {
        return None;
    }

    Some(pkg_dir.join(strip_dot_slash(subpath)))
}

fn package_exports_main_is_string_target(exports: Option<&serde_json::Value>) -> bool {
    match exports {
        Some(serde_json::Value::String(_)) => true,
        Some(serde_json::Value::Object(map)) => match map.get(".") {
            Some(serde_json::Value::String(_)) => true,
            _ => false,
        },
        _ => false,
    }
}

fn cjs_main_target_resolves(base: &std::path::Path) -> bool {
    if base.is_file() {
        return true;
    }
    const EXTS: &[&str] = &[
        ".js", ".cjs", ".mjs", ".json", ".node", ".ts", ".mts", ".cts", ".tsx", ".fts",
    ];
    for e in EXTS {
        if with_suffix(base, e).is_file() {
            return true;
        }
    }
    for idx in [
        "index.js",
        "index.cjs",
        "index.mjs",
        "index.json",
        "index.node",
    ] {
        if base.join(idx).is_file() {
            return true;
        }
    }
    false
}

fn strip_dot_slash(s: &str) -> &str {
    s.strip_prefix("./").unwrap_or(s)
}

fn resolve_exports_target(
    target: &serde_json::Value,
    capture: &str,
    importer_kind: ModuleKind,
) -> Option<String> {
    match target {
        serde_json::Value::String(s) => Some(substitute_wildcard(s, capture)),
        serde_json::Value::Array(arr) => {

            for item in arr {
                if let Some(r) = resolve_exports_target(item, capture, importer_kind) {
                    return Some(r);
                }
            }
            None
        }
        serde_json::Value::Object(map) => {

            let active: &[&str] = match importer_kind {

                ModuleKind::ESM => &["node", "import", "module-sync", "default"],
                ModuleKind::CJS => &["node", "require", "module-sync", "default"],
            };
            for (key, value) in map.iter() {
                if active.contains(&key.as_str()) {
                    if let Some(r) = resolve_exports_target(value, capture, importer_kind) {
                        return Some(r);
                    }
                }
            }
            None
        }
        serde_json::Value::Null => None,
        _ => None,
    }
}

fn substitute_wildcard(target: &str, capture: &str) -> String {
    if capture.is_empty() || !target.contains('*') {
        return target.to_string();
    }
    target.replacen('*', capture, 1)
}

#[cfg(test)]
mod realm_import_pass2_per_realm_registry_tests {
    use super::*;
    use crate::interp::Runtime;

    fn dummy_record(url: &str) -> Rc<RefCell<ModuleRecord>> {
        let empty_ast = Rc::new(AstModule {
            span: rusty_js_ast::Span::new(0, 0),
            body: Vec::new(),
            import_entries: Vec::new(),
            local_export_entries: Vec::new(),
            indirect_export_entries: Vec::new(),
            star_export_entries: Vec::new(),
        });
        let empty_bc = Rc::new(CompiledModule {
            bytecode: Vec::new(),
            constants: Default::default(),
            locals: Vec::new(),
            catch_param_names: Vec::new(),
            source_map: Vec::new(),
            source_text: None,
            imports: Vec::new(),
            exports: Vec::new(),
            reexport_sources: Vec::new(),
            side_effect_imports: Vec::new(),
            construct_tags: Vec::new(),
            line_starts: Vec::new(),
            eval_var_env_is_global: false,
            global_env_alias: false,
            script_var_deletable: false,
            eval_outer_locals: Vec::new(),
            module_hoisted_functions: Vec::new(),
            strict: false,
        });
        Rc::new(RefCell::new(ModuleRecord {
            url: url.to_string(),
            status: ModuleStatus::Evaluated,
            ast: empty_ast,
            bytecode: empty_bc,
            namespace: None,
            eval_error: None,
            kind: ModuleKind::ESM,
            cjs_exports: None,
            export_cells: std::collections::HashMap::new(),
            async_static_deps: Vec::new(),
            body_completed_waiting_async_deps: false,
            pending_body_start: None,
            async_evaluation_order: None,
            async_cycle_root: None,
        }))
    }

    #[test]
    fn realm_zero_uses_global_map_no_new_allocation() {
        let mut rt = Runtime::new();
        assert_eq!(rt.current_realm, 0);
        rt.module_insert("./m.js".to_string(), dummy_record("./m.js"));
        assert!(
            rt.module_get("./m.js").is_some(),
            "realm-0 read hits the global map"
        );
        assert!(
            rt.modules.contains_key("./m.js"),
            "realm-0 write lands in the legacy global map"
        );
        assert!(
            rt.realm_module_registries.is_empty(),
            "realm-0-only activity must allocate ZERO per-Realm registries (lazy baseline)"
        );
    }

    #[test]
    fn same_specifier_two_realms_two_distinct_records() {
        let mut rt = Runtime::new();
        rt.current_realm = 1;
        let rec_a = dummy_record("./m.js");
        rt.module_insert("./m.js".to_string(), rec_a.clone());
        rt.current_realm = 2;
        let rec_b = dummy_record("./m.js");
        rt.module_insert("./m.js".to_string(), rec_b.clone());
        rt.current_realm = 1;
        let got1 = rt.module_get("./m.js").expect("realm-1 record");
        rt.current_realm = 2;
        let got2 = rt.module_get("./m.js").expect("realm-2 record");
        assert!(Rc::ptr_eq(&got1, &rec_a), "realm-1 sees its own record");
        assert!(Rc::ptr_eq(&got2, &rec_b), "realm-2 sees its own record");
        assert!(
            !Rc::ptr_eq(&got1, &got2),
            "same specifier in two Realms must be two DISTINCT ModuleRecord instances"
        );
    }

    #[test]
    fn non_realm_zero_load_does_not_bleed() {
        let mut rt = Runtime::new();
        rt.current_realm = 1;
        rt.module_insert("./x.js".to_string(), dummy_record("./x.js"));
        rt.current_realm = 0;
        assert!(
            rt.module_get("./x.js").is_none(),
            "realm-1 module must not appear in the realm-0 global cache"
        );
        assert!(
            !rt.modules.contains_key("./x.js"),
            "global map untouched by a realm-1 load"
        );
        rt.current_realm = 2;
        assert!(
            rt.module_get("./x.js").is_none(),
            "sibling realm-2 must not see realm-1's module"
        );
    }

    #[test]
    fn pkg_json_cache_is_realm_shared() {
        let mut rt = Runtime::new();
        let path = std::path::PathBuf::from("/pkg/package.json");
        rt.pkg_json_cache.insert(
            path.clone(),
            Rc::new(ParsedPackageJson {
                raw: serde_json::Value::Null,
                name: None,
                main: None,
                module_field: None,
                type_field: None,
            }),
        );
        rt.current_realm = 7;
        assert!(
            rt.pkg_json_cache.contains_key(&path),
            "pkg_json_cache is immutable-shareable: readable across Realms, not rekeyed"
        );
    }

    #[test]
    fn node_builtin_namespace_is_per_realm() {
        let mut rt = Runtime::new();
        rt.install_host_hook(HostHook::ResolveBuiltinModule(Box::new(|rt, spec| {
            if spec != "node:fs" {
                return Ok(None);
            }
            let ns = rt.alloc_object(Object::new_ordinary());
            rt.object_set(
                ns,
                "tag".into(),
                Value::String(Rc::new(crate::value::JsString::from("fs"))),
            );
            Ok(Some(ns))
        })));

        let realm_1 = rt.allocate_realm();
        let realm_2 = rt.allocate_realm();

        rt.current_realm = realm_1;
        let fs_r1 = rt.resolve_builtin_namespace("node:fs").expect("realm-1 fs");
        rt.object_set(fs_r1, "realm".into(), Value::Number(1.0));

        rt.current_realm = realm_2;
        let fs_r2 = rt.resolve_builtin_namespace("node:fs").expect("realm-2 fs");
        rt.object_set(fs_r2, "realm".into(), Value::Number(2.0));

        assert_ne!(
            fs_r1, fs_r2,
            "same node:* specifier in two Realms must produce distinct namespace objects"
        );
        assert_eq!(rt.object_get(fs_r1, "realm"), Value::Number(1.0));
        assert_eq!(rt.object_get(fs_r2, "realm"), Value::Number(2.0));

        let rec_r1 = rt
            .realm_module_registries
            .get(&realm_1)
            .and_then(|m| m.get("node:fs"))
            .expect("realm-1 builtin record")
            .clone();
        let rec_r2 = rt
            .realm_module_registries
            .get(&realm_2)
            .and_then(|m| m.get("node:fs"))
            .expect("realm-2 builtin record")
            .clone();
        assert!(
            !Rc::ptr_eq(&rec_r1, &rec_r2),
            "node:* builtin ModuleRecords are per-Realm, while backing hook code is shared"
        );
        assert!(
            !rt.modules.contains_key("node:fs"),
            "non-zero Realm builtin resolution must not populate the main Realm cache"
        );
    }

    #[test]
    fn load_module_accepts_node_builtin_url() {
        let mut rt = Runtime::new();
        rt.install_host_hook(HostHook::ResolveBuiltinModule(Box::new(|rt, spec| {
            if spec != "node:process" {
                return Ok(None);
            }
            let ns = rt.alloc_object(Object::new_ordinary());
            rt.object_set(
                ns,
                "pid".into(),
                Value::String(Rc::new(crate::value::JsString::from("process"))),
            );
            Ok(Some(ns))
        })));

        let ns = rt
            .load_module("node:process")
            .expect("node builtin URL should route through host hook");
        assert_eq!(
            rt.object_get(ns, "pid"),
            Value::String(Rc::new(crate::value::JsString::from("process")))
        );
    }
}
