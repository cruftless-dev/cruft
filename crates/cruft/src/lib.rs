
pub mod assert;
pub mod child_process;
pub mod console;
pub mod crizzle;
pub mod crypto;
pub mod dgram;
pub mod deno;
pub mod reserved;
pub mod dns;
pub mod dns_proto;
pub mod events;
pub mod fetch;
pub mod fs;
pub mod host_surfaces;
pub mod hpack;
pub mod http;
pub mod http2_client;
pub mod http_client;
pub mod https;
pub mod ipc;
pub mod module_ns;
pub mod net;
pub mod node_stubs;
pub mod os;
pub mod path;
pub mod platform;
pub mod pm;
pub mod press;
pub mod process;
pub mod querystring;
pub mod register;
pub mod spawn;
pub mod sqlite;
pub mod stdin;
pub mod stream;
pub mod test262_frontmatter;
pub mod test262_host;
pub mod test_runner;
pub mod timer;
pub mod tls;
pub mod tty;
pub mod url;
pub mod util;
pub mod vm;
pub mod wasm;
pub mod ws;
pub mod zlib;

use rusty_js_ast::Span;
use rusty_js_runtime::value::{
    FunctionInternals, InternalKind, NativeFn, Object, ObjectRef, PromiseStatus, PropertyKey,
};
use rusty_js_runtime::{job_queue, HostHook, Runtime, RuntimeError, Value};
use std::collections::HashMap;
use std::rc::Rc;

pub fn install_cruft_host(rt: &mut Runtime, argv: Vec<String>) {
    let __host_t0 = std::env::var("CRUFT_BOOT_TRACE")
        .ok()
        .map(|_| std::time::Instant::now());
    macro_rules! ht {
        ($t:expr, $call:expr) => {{
            let __s = std::time::Instant::now();
            $call;
            if $t.is_some() {
                eprintln!(
                    "[boot-trace]   host {:<32} = {:.2} ms",
                    stringify!($call),
                    __s.elapsed().as_secs_f64() * 1000.0
                );
            }
        }};
    }
    install_cruftscript_boundary_audit_helper(rt);
    install_cruftscript_endowment_helper(rt);

    rt.register_root_source(net::collect_roots_for_runtime);
    rt.register_root_source(http::collect_roots_for_runtime);
    rt.register_root_source(http2_client::collect_roots_for_runtime);
    rt.register_root_source(tls::collect_roots_for_runtime);
    rt.register_root_source(wasm::collect_roots_for_runtime);
    rt.register_root_source(spawn::collect_roots_for_runtime);
    rt.register_root_source(ws::collect_roots_for_runtime);

    rt.register_lazy_host_module(&["path", "__cruft_path"], |rt| {
        path::install(rt);
        path::install_canonical(rt);
    });

    rt.register_lazy_host_module(&["os", "__cruft_os"], |rt| {
        os::install(rt);
        os::install_canonical(rt);
    });
    process::install(rt, argv);
    ht!(__host_t0, process::install_canonical(rt));

    ht!(__host_t0, fs::install_poll_io(rt));
    rt.register_lazy_host_module(&["fs", "fs_promises", "__cruft_fs"], |rt| {
        fs::install(rt);
        fs::install_canonical(rt);
        if let Value::Object(ns) = rt.global_get("fs") {
            for ctor in ["Stats", "Dirent", "Dir"] {
                if let Value::Object(ctor_id) = rt.object_get(ns, ctor) {
                    crate::register::make_subclassable(rt, ctor_id, None);
                }
            }
        }
    });
    rt.register_lazy_host_module(&["dns", "dns_promises", "__cruft_dns"], |rt| {
        dns::install_canonical(rt);
        dns::install(rt);
    });

    ht!(__host_t0, timer::install(rt));

    ht!(__host_t0, module_ns::install(rt));

    ht!(__host_t0, http::install(rt));
    ht!(__host_t0, http::install_canonical(rt));
    ht!(__host_t0, ws::install_canonical(rt));
    ht!(__host_t0, tls::install_http2_canonical(rt));

    rt.register_lazy_host_module(&["net"], |rt| net::install(rt));
    ht!(__host_t0, fetch::install(rt));
    ht!(__host_t0, spawn::install(rt));

    rt.register_lazy_host_module_hidden(&["__bun_sqlite", "__node_sqlite", "__cruft_sqlite"], |rt| sqlite::install(rt));

    rt.register_lazy_host_module_hidden(&["__crizzle"], |rt| crizzle::install(rt));

    rt.register_lazy_host_module(&["crypto"], |rt| crypto::install(rt));

    rt.register_lazy_host_module_hidden(&["__cruft_press"], |rt| press::install(rt));

    rt.register_lazy_host_module_hidden(&["__cruft_pm"], |rt| pm::install(rt));

    rt.register_lazy_host_module_hidden(&["__node_assert", "__node_assert_strict"], |rt| assert::install(rt));

    rt.register_lazy_host_module(&["https"], |rt| https::install(rt));

    rt.register_lazy_host_module_hidden(&["stream"], |rt| {
        stream::install(rt);
        stream::wire_event_emitter_alias(rt);
        stream::install_iterator_helpers(rt);
    });
    ht!(__host_t0, url::install(rt));
    ht!(__host_t0, url::install_canonical(rt));

    rt.register_lazy_host_module(&["querystring", "__cruft_querystring"], |rt| {
        querystring::install(rt);
        querystring::install_canonical(rt);
    });

    util::install(rt);
    rt.register_lazy_host_module(&["zlib"], |rt| zlib::install(rt));

    rt.register_lazy_host_module(
        &[
            "__cruft_node_test",
            "__cruft_test",
            "__cruft_test_reporters",
            "__cruft_internal_test_runner_snapshot",
            "__cruft_internal_test_runner_utils",
            "__cruft_internal_assert_myers_diff",
            "__cruft_internal_timers",
        ],
        |rt| test_runner::install(rt),
    );
    ht!(__host_t0, console::install(rt));

    rt.register_lazy_host_module(&["dgram"], |rt| dgram::install(rt));
    rt.register_lazy_host_module(&["tty"], |rt| tty::install(rt));
    ht!(__host_t0, events::install(rt));
    ht!(__host_t0, events::install_canonical(rt));
    if let rusty_js_runtime::Value::Object(process) = rt.global_get("process") {
        process::wire_event_emitter_prototype(rt, process);
        process::install_stdio_event_emitters(rt, process);
    }

    ht!(__host_t0, node_stubs::install_all_eager(rt));

    ht!(__host_t0, node_stubs::install_web_worker(rt));

    rt.register_lazy_host_module(&["constants"], |rt| node_stubs::install_constants(rt));

    rt.register_lazy_host_module(&["child_process"], |rt| node_stubs::install_child_process(rt));
    rt.register_lazy_host_module(&["tls", "__cruft_tls"], |rt| node_stubs::install_tls(rt));
    rt.register_lazy_host_module(&["readline", "readline_promises"], |rt| {
        node_stubs::install_readline(rt)
    });
    rt.register_lazy_host_module(&["worker_threads", "__cruft_worker"], |rt| {
        node_stubs::install_worker_threads(rt)
    });
    rt.register_lazy_host_module(&["cluster"], |rt| node_stubs::install_cluster(rt));
    rt.register_lazy_host_module(&["repl"], |rt| node_stubs::install_repl(rt));
    rt.register_lazy_host_module(&["trace_events"], |rt| node_stubs::install_trace_events(rt));
    rt.register_lazy_host_module(&["http2"], |rt| node_stubs::install_http2(rt));
    rt.register_lazy_host_module(&["diagnostics_channel"], |rt| {
        node_stubs::install_diagnostics_channel(rt)
    });
    rt.register_lazy_host_module(&["v8"], |rt| node_stubs::install_v8(rt));
    rt.register_lazy_host_module(&["inspector"], |rt| node_stubs::install_inspector(rt));
    rt.register_lazy_host_module(&["vm", "__cruft_vm"], |rt| node_stubs::install_vm(rt));
    rt.register_lazy_host_module(&["punycode"], |rt| node_stubs::install_punycode(rt));
    rt.register_lazy_host_module(&["async_hooks"], |rt| node_stubs::install_async_hooks(rt));
    rt.register_lazy_host_module(&["domain"], |rt| node_stubs::install_domain(rt));
    rt.register_lazy_host_module(&["__cruft_internal_event_target"], |rt| {
        node_stubs::install_internal_event_target(rt)
    });
    rt.register_lazy_host_module(&["__cruft_internal_webstreams_util"], |rt| {
        node_stubs::install_internal_webstreams_util(rt)
    });

    for (module, ctor, parent) in [
        ("buffer", "SlowBuffer", Some("Buffer")),

        ("url", "Url", None),
        ("string_decoder", "StringDecoder", None),

        ("__node_assert", "AssertionError", Some("Error")),
        ("__node_assert", "CallTracker", None),
    ] {
        if let Value::Object(ns) = rt.global_get(module) {
            if let Value::Object(ctor_id) = rt.object_get(ns, ctor) {
                let parent_proto =
                    parent.and_then(|p| crate::register::proto_of_global_ctor(rt, p));
                crate::register::make_subclassable(rt, ctor_id, parent_proto);
            }
        }
    }
    ht!(__host_t0, test262_host::install(rt));
    install_builtin_module_resolver(rt);
    install_cruftscript_module_loader(rt);

    ht!(__host_t0, wasm::install(rt));
    ht!(__host_t0, wasm::install_wasi(rt));
    ht!(__host_t0, deno::install(rt));
    rt.install_global_this_refresh();

    if let rusty_js_runtime::Value::Object(process) = rt.global_get("process") {
        ipc::install_child_bootstrap(rt, process);

        stdin::install(rt, process);
    }
    if let Some(t0) = __host_t0 {
        eprintln!(
            "[boot-trace] install_cruft_host = {:.2} ms",
            t0.elapsed().as_secs_f64() * 1000.0
        );
    }
}

#[deprecated(
    note = "use install_cruft_host; install_bun_host is pre-Cruft host-v2 migration vocabulary"
)]
pub fn install_bun_host(rt: &mut Runtime, argv: Vec<String>) {
    install_cruft_host(rt, argv)
}

fn install_cruftscript_boundary_audit_helper(rt: &mut Runtime) {
    let native: NativeFn = Rc::new(|rt, _args| {
        let audit = rt.alloc_object(Object::new_ordinary());
        rt.object_set(
            audit,
            "skipReturnCount".to_string(),
            Value::Number(rt.boundary_skip_return_opt_out_count as f64),
        );
        rt.object_set(
            audit,
            "skipReturnLast".to_string(),
            rt.boundary_skip_return_last_opt_out
                .clone()
                .map(|value| Value::String(Rc::new(rusty_js_runtime::value::JsString::from(value))))
                .unwrap_or(Value::Undefined),
        );
        rt.object_set(
            audit,
            "debugViolationCount".to_string(),
            Value::Number(rt.boundary_debug_violation_count as f64),
        );
        rt.object_set(
            audit,
            "debugViolationLast".to_string(),
            rt.boundary_debug_last_violation
                .clone()
                .map(|value| Value::String(Rc::new(rusty_js_runtime::value::JsString::from(value))))
                .unwrap_or(Value::Undefined),
        );
        Ok(Value::Object(audit))
    });
    let mut properties = indexmap::IndexMap::new();
    rusty_js_runtime::value::install_function_meta_props(
        &mut properties,
        "__cruftscript_boundary_audit",
        0.0,
    );
    let fn_obj = Object {
        proto: None,
        extensible: true,
        properties,
        internal_kind: InternalKind::Function(Box::new(FunctionInternals {
            name: "__cruftscript_boundary_audit".to_string(),
            length: 0,
            native,
            is_constructor: false,
            creation_realm: 0,
            roots: Vec::new(),
        })),
        ..Default::default()
    };
    let fn_id = rt.alloc_object(fn_obj);
    rt.define_global_property("__cruftscript_boundary_audit", Value::Object(fn_id));
}

fn install_cruftscript_endowment_helper(rt: &mut Runtime) {
    let registry = rt.alloc_object(Object::new_ordinary());
    rt.define_global_property("__cruftscript_endowments", Value::Object(registry));
    let native: NativeFn = Rc::new(|rt, args| {
        let (Some(Value::String(compartment)), Some(Value::String(name)), Some(value)) =
            (args.first(), args.get(1), args.get(2))
        else {
            return Err(RuntimeError::TypeError(
                "__cruftscript_endow(compartment, name, value) requires string compartment, string name, and value".to_string(),
            ));
        };
        let key = format!("{}::{}", compartment.as_str(), name.as_str());
        let registry = rt.global_get("__cruftscript_endowments");
        let Value::Object(registry_id) = registry else {
            return Err(RuntimeError::TypeError(
                "CruftScript endowment registry is not installed".to_string(),
            ));
        };
        rt.object_set(registry_id, key, value.clone());
        Ok(Value::Undefined)
    });
    let mut properties = indexmap::IndexMap::new();
    rusty_js_runtime::value::install_function_meta_props(
        &mut properties,
        "__cruftscript_endow",
        3.0,
    );
    let fn_obj = Object {
        proto: None,
        extensible: true,
        properties,
        internal_kind: InternalKind::Function(Box::new(FunctionInternals {
            name: "__cruftscript_endow".to_string(),
            length: 3,
            native,
            is_constructor: false,
            creation_realm: 0,
            roots: Vec::new(),
        })),
        ..Default::default()
    };
    let fn_id = rt.alloc_object(fn_obj);
    rt.define_global_property("__cruftscript_endow", Value::Object(fn_id));
}

fn install_cruftscript_module_loader(rt: &mut Runtime) {
    rt.install_host_hook(HostHook::LoadCruftScriptModule(Box::new(
        |rt, url, source| build_cruftscript_module_namespace(rt, url, source),
    )));
}

pub(crate) fn build_cruftscript_module_namespace(
    rt: &mut Runtime,
    url: &str,
    source: &str,
) -> Result<Option<ObjectRef>, RuntimeError> {
    let checked = cruftscript_type_checker::CruftScriptCheckedUnit::parse_and_check(url, source)
        .map_err(|rejected| {
            let structured = rejected
                .report
                .diagnostics
                .iter()
                .map(|d| {
                    cruftscript_type_checker::CruftScriptDiagnosticRecord::from_check(
                        &rejected.provenance,
                        d,
                        70,
                    )
                    .tooling_line()
                })
                .collect::<Vec<_>>()
                .join("; ");
            RuntimeError::SyntaxError(format!(
                "cruftscript module rejected in {}: {}; structured: {}",
                rejected.provenance.path,
                rejected
                    .report
                    .diagnostics
                    .iter()
                    .map(|d| format!(
                        "{:?} {}..{}: {}",
                        d.code, d.span.start, d.span.end, d.message
                    ))
                    .collect::<Vec<_>>()
                    .join("; "),
                structured
            ))
        })?;
    let export_module = cruftscript_type_checker::CruftScriptExportModule::lower_exports(&checked)
        .map_err(|diagnostics| {
            let structured = diagnostics
                .iter()
                .map(|d| {
                    cruftscript_type_checker::CruftScriptDiagnosticRecord::from_lowering(
                        &checked.provenance,
                        d,
                        70,
                    )
                    .tooling_line()
                })
                .collect::<Vec<_>>()
                .join("; ");
            RuntimeError::SyntaxError(format!(
                "cruftscript module lowering rejected in {}: {}; structured: {}",
                checked.provenance.path,
                diagnostics
                    .iter()
                    .map(|d| format!(
                        "{:?} {}..{}: {}",
                        d.code, d.span.start, d.span.end, d.message
                    ))
                    .collect::<Vec<_>>()
                    .join("; "),
                structured
            ))
        })?;
    let wrapper_plan = cruftscript_type_checker::CruftScriptBoundaryWrapperPlan::emit_installs(
        &checked,
    )
    .map_err(|diagnostics| {
        let structured = diagnostics
            .iter()
            .map(|d| {
                format!(
                    "stage=runtime-boundary code={:?} span={}..{} path={} message={}",
                    d.code, d.span.start, d.span.end, checked.provenance.path, d.message
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        RuntimeError::SyntaxError(format!(
            "cruftscript boundary wrapper emission rejected in {}: {}",
            checked.provenance.path, structured
        ))
    })?;
    let compartment_policy_levels = cruftscript_compartment_policy_levels(&checked);
    let compartment_policy_objects = compartment_policy_levels
        .iter()
        .map(|(name, level)| {
            (
                name.clone(),
                cruftscript_compartment_policy_object(rt, *level),
            )
        })
        .collect::<HashMap<_, _>>();

    let ns = rt.alloc_object(Object::new_module_namespace());
    for export in export_module.exports {
        let name = export.name.clone();
        let arity = export.function.arity;
        let compartment = export.function.compartment.clone();
        let native = native_for_cruftscript_lowered_function(url.to_string(), export.function);
        let mut properties = indexmap::IndexMap::new();
        rusty_js_runtime::value::install_function_meta_props(&mut properties, &name, arity as f64);
        let fn_obj = Object {
            proto: None,
            extensible: true,
            properties,
            internal_kind: InternalKind::Function(Box::new(FunctionInternals {
                name: name.clone(),
                length: arity as u32,
                native,
                is_constructor: false,
                creation_realm: 0,
                roots: Vec::new(),
            })),
            ..Default::default()
        };
        let fn_id = rt.alloc_object(fn_obj);
        let export_value = if let Some(install) = wrapper_plan.installs.iter().find(|install| {
            install.site == cruftscript_type_checker::BoundaryWrapperInstallSite::ExportFunction
                && install.target_name == name
                && install.compartment == compartment
        }) {
            cruftscript_install_export_boundary_wrapper(
                rt,
                Value::Object(fn_id),
                install,
                compartment_policy_objects
                    .get(&install.compartment)
                    .copied(),
            )
        } else {
            Value::Object(fn_id)
        };
        rt.object_set(ns, name, export_value);
    }
    Ok(Some(ns))
}

fn cruftscript_install_export_boundary_wrapper(
    rt: &mut Runtime,
    target: Value,
    install: &cruftscript_type_checker::BoundaryWrapperInstallRecord,
    installer_compartment: Option<ObjectRef>,
) -> Value {
    let validator_value = match install.validator {
        cruftscript_type_checker::BoundaryWrapperValidatorPlan::SyncHaltOnly => {
            cruftscript_policy_aware_boundary_validator(rt, install.policy_id, &install.param_types)
        }
        cruftscript_type_checker::BoundaryWrapperValidatorPlan::TrustReturnOptOutRecorded => {
            let validator =
                cruftscript_boundary_validator(rt, "__cruftscript_boundary_fail", false);
            return rt.install_boundary_wrapper_trust_return_opt_out(
                target,
                install.policy_id,
                validator,
            );
        }
    };
    if install.policy_id == 2 {
        let defaults = install
            .param_types
            .iter()
            .map(|ty| cruftscript_sanitizer_default_for_type(rt, &install.sanitizer_defaults, ty))
            .collect::<Vec<_>>();
        let return_default = install.return_type.as_ref().and_then(|ty| {
            cruftscript_sanitizer_default_for_type(rt, &install.sanitizer_defaults, ty)
        });
        return rt.install_boundary_wrapper_sanitize_in(
            target,
            install.policy_id,
            validator_value,
            installer_compartment,
            defaults,
            return_default,
        );
    }
    rt.install_boundary_wrapper_in(
        target,
        install.policy_id,
        validator_value,
        installer_compartment,
    )
}

fn cruftscript_boundary_validator(rt: &mut Runtime, name: &'static str, verdict: bool) -> Value {
    let native: NativeFn = Rc::new(move |_rt, _args| Ok(Value::Boolean(verdict)));
    let mut properties = indexmap::IndexMap::new();
    rusty_js_runtime::value::install_function_meta_props(&mut properties, name, 0.0);
    let fn_obj = Object {
        proto: None,
        extensible: true,
        properties,
        internal_kind: InternalKind::Function(Box::new(FunctionInternals {
            name: name.to_string(),
            length: 0,
            native,
            is_constructor: false,
            creation_realm: 0,
            roots: Vec::new(),
        })),
        ..Default::default()
    };
    Value::Object(rt.alloc_object(fn_obj))
}

fn cruftscript_policy_aware_boundary_validator(
    rt: &mut Runtime,
    policy_id: u32,
    param_types: &[cruftscript_type_checker::TypeTerm],
) -> Value {
    let required_level = cruftscript_policy_level_for_id(policy_id);
    let undefined_allowed_by_param = param_types
        .iter()
        .map(cruftscript_type_term_allows_undefined)
        .collect::<Vec<_>>();
    let native: NativeFn = Rc::new(move |_rt, args| {
        let Some(Value::Number(level)) = args.last() else {
            return Ok(Value::Boolean(true));
        };
        if required_level > 0
            && args
                .iter()
                .take(args.len().saturating_sub(1))
                .enumerate()
                .any(|(index, arg)| {
                    matches!(arg, Value::Undefined)
                        && !undefined_allowed_by_param
                            .get(index)
                            .copied()
                            .unwrap_or(false)
                })
        {
            return Ok(Value::Boolean(false));
        }
        Ok(Value::Boolean((*level as u8) >= required_level))
    });
    let mut properties = indexmap::IndexMap::new();
    rusty_js_runtime::value::install_function_meta_props(
        &mut properties,
        "__cruftscript_boundary_policy_level",
        1.0,
    );
    let fn_obj = Object {
        proto: None,
        extensible: true,
        properties,
        internal_kind: InternalKind::Function(Box::new(FunctionInternals {
            name: "__cruftscript_boundary_policy_level".to_string(),
            length: 1,
            native,
            is_constructor: false,
            creation_realm: 0,
            roots: Vec::new(),
        })),
        ..Default::default()
    };
    Value::Object(rt.alloc_object(fn_obj))
}

fn cruftscript_type_term_allows_undefined(ty: &cruftscript_type_checker::TypeTerm) -> bool {
    match ty {
        cruftscript_type_checker::TypeTerm::Named { name, .. } => name == "undefined",
        cruftscript_type_checker::TypeTerm::Union { members, .. } => {
            members.iter().any(cruftscript_type_term_allows_undefined)
        }
        _ => false,
    }
}

fn cruftscript_compartment_policy_object(rt: &mut Runtime, level: u8) -> ObjectRef {
    let id = rt.alloc_object(Object::new_ordinary());
    rt.object_set(
        id,
        "__compartment_effective_policy_level".to_string(),
        Value::Number(level as f64),
    );
    id
}

fn cruftscript_compartment_policy_levels(
    checked: &cruftscript_type_checker::CruftScriptCheckedUnit,
) -> HashMap<String, u8> {
    checked
        .boundary_facts()
        .compartments
        .iter()
        .map(|fact| {
            (
                fact.compartment.clone(),
                fact.effective_policy
                    .as_ref()
                    .map(|policy| cruftscript_policy_ref_level(checked, policy))
                    .unwrap_or(1),
            )
        })
        .collect()
}

fn cruftscript_policy_ref_level(
    checked: &cruftscript_type_checker::CruftScriptCheckedUnit,
    policy: &cruftscript_type_checker::BoundaryPolicyRef,
) -> u8 {
    match policy {
        cruftscript_type_checker::BoundaryPolicyRef::Named(name) if name == "debug" => 0,
        cruftscript_type_checker::BoundaryPolicyRef::Named(name) if name == "secure" => 1,
        cruftscript_type_checker::BoundaryPolicyRef::Named(name) if name == "default" => checked
            .boundary_facts()
            .default_policy
            .as_ref()
            .map(|default| cruftscript_policy_name_level(&default.policy))
            .unwrap_or(1),
        cruftscript_type_checker::BoundaryPolicyRef::Named(name) => {
            cruftscript_policy_name_level(name)
        }
        cruftscript_type_checker::BoundaryPolicyRef::Default => checked
            .boundary_facts()
            .default_policy
            .as_ref()
            .map(|default| cruftscript_policy_name_level(&default.policy))
            .unwrap_or(1),
        cruftscript_type_checker::BoundaryPolicyRef::WeakenTo(name) => {
            cruftscript_policy_name_level(name)
        }
        cruftscript_type_checker::BoundaryPolicyRef::Override(name) => {
            cruftscript_policy_name_level(name)
        }
    }
}

fn cruftscript_policy_name_level(name: &str) -> u8 {
    match name {
        "debug" => 0,
        _ => 1,
    }
}

fn cruftscript_policy_level_for_id(policy_id: u32) -> u8 {
    if policy_id == 0 {
        0
    } else {
        1
    }
}

fn native_for_cruftscript_lowered_function(
    module_url: String,
    function: cruftscript_type_checker::LoweredFunction,
) -> NativeFn {
    let function_name = function.function_name.clone();
    let function_span = function.span;
    let function_arity = function.arity;
    match function.body {
        cruftscript_type_checker::LoweredFunctionBody::Constant(result) => {
            Rc::new(move |rt, args| {
                if !args.is_empty() {
                    let record =
                        cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
                            &module_url,
                            "RuntimeExportArityMismatch",
                            function_span,
                            format!(
                                "cruftscript runtime export `{function_name}` expects 0 argument(s), got {}",
                                args.len()
                            ),
                            70,
                        );
                    return Err(RuntimeError::TypeError(format!(
                        "{}; {}",
                        record.message,
                        record.tooling_line()
                    )));
                }
                cruftscript_lowered_value_to_runtime(rt, &module_url, &result)
            })
        }
        cruftscript_type_checker::LoweredFunctionBody::ImportedCall(call) => {
            Rc::new(move |rt, args| {
                if !args.is_empty() {
                    let record =
                        cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
                            &module_url,
                            "RuntimeExportArityMismatch",
                            function_span,
                            format!(
                                "cruftscript runtime export `{function_name}` expects 0 argument(s), got {}",
                                args.len()
                            ),
                            70,
                        );
                    return Err(RuntimeError::TypeError(format!(
                        "{}; {}",
                        record.message,
                        record.tooling_line()
                    )));
                }
                cruftscript_eval_imported_call(rt, &module_url, &call)
            })
        }
        cruftscript_type_checker::LoweredFunctionBody::RuntimeExpression(expression) => {
            Rc::new(move |rt, args| {
                if !args.is_empty() {
                    let record =
                        cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
                            &module_url,
                            "RuntimeExportArityMismatch",
                            function_span,
                            format!(
                                "cruftscript runtime export `{function_name}` expects 0 argument(s), got {}",
                                args.len()
                            ),
                            70,
                        );
                    return Err(RuntimeError::TypeError(format!(
                        "{}; {}",
                        record.message,
                        record.tooling_line()
                    )));
                }
                cruftscript_runtime_expression_to_runtime(rt, &module_url, &expression)
            })
        }
        cruftscript_type_checker::LoweredFunctionBody::RuntimeParameterizedExport(template) => {
            let lowered_function = cruftscript_type_checker::LoweredFunction {
                compartment: function.compartment,
                function_name,
                arity: function_arity,
                body: cruftscript_type_checker::LoweredFunctionBody::RuntimeParameterizedExport(
                    template,
                ),
                span: function_span,
            };
            Rc::new(move |rt, args| {
                if args.len() > lowered_function.arity {
                    let record =
                        cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
                            &module_url,
                            "RuntimeExportArityMismatch",
                            lowered_function.span,
                            format!(
                                "cruftscript runtime export `{}` expects at most {} argument(s), got {}",
                                lowered_function.function_name,
                                lowered_function.arity,
                                args.len()
                            ),
                            70,
                        );
                    return Err(RuntimeError::TypeError(format!(
                        "{}; {}",
                        record.message,
                        record.tooling_line()
                    )));
                }
                let mut runtime_args = args.to_vec();
                if let cruftscript_type_checker::LoweredFunctionBody::RuntimeParameterizedExport(
                    template,
                ) = &lowered_function.body
                {
                    let mut default_env = template
                        .params
                        .iter()
                        .take(runtime_args.len())
                        .cloned()
                        .zip(runtime_args.iter().cloned())
                        .collect::<HashMap<_, _>>();
                    for index in 0..template.params.len() {
                        let needs_default = runtime_args
                            .get(index)
                            .map(|value| matches!(value, Value::Undefined))
                            .unwrap_or(true);
                        if !needs_default {
                            continue;
                        }
                        let Some(default) =
                            template.param_defaults.get(index).and_then(|d| d.as_ref())
                        else {
                            if index >= runtime_args.len() {
                                let record = cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
                                    &module_url,
                                    "RuntimeExportArityMismatch",
                                    lowered_function.span,
                                    format!(
                                        "cruftscript runtime export `{}` missing required parameter `{}`",
                                        lowered_function.function_name, template.params[index]
                                    ),
                                    70,
                                );
                                return Err(RuntimeError::TypeError(format!(
                                    "{}; {}",
                                    record.message,
                                    record.tooling_line()
                                )));
                            }
                            continue;
                        };
                        let default_value = cruftscript_runtime_expression_to_runtime_with_env(
                            rt,
                            &module_url,
                            default,
                            &mut default_env,
                        )?;
                        if index < runtime_args.len() {
                            runtime_args[index] = default_value.clone();
                        } else {
                            runtime_args.push(default_value.clone());
                        }
                        default_env.insert(template.params[index].clone(), default_value);
                    }
                }
                let mut lowered_args = Vec::with_capacity(runtime_args.len());
                for (index, arg) in runtime_args.iter().enumerate() {
                    lowered_args.push(cruftscript_runtime_arg_to_lowered_for_export(
                        rt,
                        &module_url,
                        &lowered_function,
                        index,
                        arg,
                    )?);
                }
                if let cruftscript_type_checker::LoweredFunctionBody::RuntimeParameterizedExport(
                    template,
                ) = &lowered_function.body
                {
                    if let Some(expression) = &template.body {
                        if cruftscript_runtime_parameterized_export_returns_class(template) {
                            let result = lowered_function
                                .call_with_lowered_args(lowered_args)
                                .map_err(|diagnostic| {
                                    let record = cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
                                        &module_url,
                                        cruftscript_lowering_runtime_code(&diagnostic.code),
                                        diagnostic.span,
                                        diagnostic.message,
                                        70,
                                    );
                                    RuntimeError::TypeError(format!(
                                        "{}; {}",
                                        record.message,
                                        record.tooling_line()
                                    ))
                                })?;
                            return cruftscript_lowered_value_to_runtime(rt, &module_url, &result);
                        }
                        lowered_function.validate_lowered_args(&lowered_args).map_err(
                            |diagnostic| {
                                let record = cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
                                    &module_url,
                                    cruftscript_lowering_runtime_code(&diagnostic.code),
                                    diagnostic.span,
                                    diagnostic.message,
                                    70,
                                );
                                RuntimeError::TypeError(format!(
                                    "{}; {}",
                                    record.message,
                                    record.tooling_line()
                                ))
                            },
                        )?;
                        let mut env = template
                            .params
                            .iter()
                            .cloned()
                            .zip(runtime_args.iter().cloned())
                            .collect::<HashMap<_, _>>();
                        return cruftscript_runtime_expression_to_runtime_with_env(
                            rt,
                            &module_url,
                            expression,
                            &mut env,
                        );
                    }
                }
                let result = lowered_function
                    .call_with_lowered_args(lowered_args)
                    .map_err(|diagnostic| {
                        let record =
                            cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
                                &module_url,
                                cruftscript_lowering_runtime_code(&diagnostic.code),
                                diagnostic.span,
                                diagnostic.message,
                                70,
                            );
                        RuntimeError::TypeError(format!(
                            "{}; {}",
                            record.message,
                            record.tooling_line()
                        ))
                    })?;
                cruftscript_lowered_value_to_runtime(rt, &module_url, &result)
            })
        }
    }
}

fn cruftscript_runtime_parameterized_export_returns_class(
    template: &cruftscript_type_checker::LoweredRuntimeParameterizedExport,
) -> bool {
    let Some(cruftscript_type_checker::TypeTerm::Named { name, .. }) = &template.return_type else {
        return false;
    };
    template
        .type_environment
        .class_constructors
        .iter()
        .any(|class| class.name == *name)
}

fn cruftscript_runtime_arg_to_lowered(
    rt: &Runtime,
    module_url: &str,
    function: &cruftscript_type_checker::LoweredFunction,
    index: usize,
    value: &Value,
) -> Result<cruftscript_type_checker::LoweredValue, RuntimeError> {
    let mut budget = CruftScriptStructuralArgBudget {
        depth_remaining: 6,
        nodes_remaining: 128,
    };
    cruftscript_runtime_value_to_lowered(rt, module_url, function, index, value, &mut budget)
}

fn cruftscript_lowering_runtime_code(
    code: &cruftscript_type_checker::LoweringDiagnosticCode,
) -> String {
    match code {
        cruftscript_type_checker::LoweringDiagnosticCode::ArgumentTypeMismatch => {
            "RuntimeExportArgumentTypeMismatch".to_string()
        }
        other => format!("{other:?}"),
    }
}

fn cruftscript_runtime_arg_to_lowered_for_export(
    rt: &Runtime,
    module_url: &str,
    function: &cruftscript_type_checker::LoweredFunction,
    index: usize,
    value: &Value,
) -> Result<cruftscript_type_checker::LoweredValue, RuntimeError> {
    if cruftscript_export_param_accepts_callback(function, index)
        && matches!(value, Value::Object(id) if rt.is_callable(&Value::Object(*id)))
    {
        return Ok(cruftscript_type_checker::LoweredValue::Undefined);
    }
    cruftscript_runtime_arg_to_lowered(rt, module_url, function, index, value)
}

fn cruftscript_export_param_accepts_callback(
    function: &cruftscript_type_checker::LoweredFunction,
    index: usize,
) -> bool {
    let cruftscript_type_checker::LoweredFunctionBody::RuntimeParameterizedExport(template) =
        &function.body
    else {
        return false;
    };
    let Some(param_name) = template.params.get(index) else {
        return false;
    };
    template.body.as_ref().is_some_and(|body| {
        cruftscript_runtime_expression_calls_callback_param(body, param_name)
            && !cruftscript_runtime_expression_retains_callback_param(body, param_name)
    })
}

fn cruftscript_runtime_expression_calls_callback_param(
    expression: &cruftscript_type_checker::LoweredRuntimeExpression,
    param_name: &str,
) -> bool {
    match expression {
        cruftscript_type_checker::LoweredRuntimeExpression::RuntimeCallbackCall(call) => {
            call.callback_name == param_name
        }
        cruftscript_type_checker::LoweredRuntimeExpression::PredicateRecordCall(call) => {
            cruftscript_runtime_expression_calls_callback_param(&call.key, param_name)
                || cruftscript_runtime_expression_calls_callback_param(&call.arg, param_name)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::Let { value, body, .. } => {
            cruftscript_runtime_expression_calls_callback_param(value, param_name)
                || cruftscript_runtime_expression_calls_callback_param(body, param_name)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::Array(elements) => {
            elements.iter().any(|element| {
                cruftscript_runtime_expression_calls_callback_param(element, param_name)
            })
        }
        cruftscript_type_checker::LoweredRuntimeExpression::Object(properties)
        | cruftscript_type_checker::LoweredRuntimeExpression::ClassObject { properties, .. } => {
            properties.iter().any(|property| {
                cruftscript_runtime_expression_calls_callback_param(&property.value, param_name)
            })
        }
        cruftscript_type_checker::LoweredRuntimeExpression::BuiltinMethodCall {
            receiver,
            args,
            ..
        } => {
            cruftscript_runtime_expression_calls_callback_param(receiver, param_name)
                || args
                    .iter()
                    .any(|arg| cruftscript_runtime_expression_calls_callback_param(arg, param_name))
        }
        cruftscript_type_checker::LoweredRuntimeExpression::BuiltinConstruct { args, .. } => args
            .iter()
            .any(|arg| cruftscript_runtime_expression_calls_callback_param(arg, param_name)),
        cruftscript_type_checker::LoweredRuntimeExpression::Arrow { body, .. } => {
            cruftscript_runtime_expression_calls_callback_param(body, param_name)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::Binary { left, right, .. } => {
            cruftscript_runtime_expression_calls_callback_param(left, param_name)
                || cruftscript_runtime_expression_calls_callback_param(right, param_name)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::PropertyAccess { object, .. } => {
            cruftscript_runtime_expression_calls_callback_param(object, param_name)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::ComputedPropertyAccess {
            object,
            key,
            ..
        } => {
            cruftscript_runtime_expression_calls_callback_param(object, param_name)
                || cruftscript_runtime_expression_calls_callback_param(key, param_name)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::NonNullCheck { expr, .. } => {
            cruftscript_runtime_expression_calls_callback_param(expr, param_name)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::TypeCheck { expr, .. } => {
            cruftscript_runtime_expression_calls_callback_param(expr, param_name)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::Conditional {
            subject,
            consequent,
            alternate,
            ..
        } => {
            cruftscript_runtime_expression_calls_callback_param(subject, param_name)
                || cruftscript_runtime_expression_calls_callback_param(consequent, param_name)
                || cruftscript_runtime_expression_calls_callback_param(alternate, param_name)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::Ternary {
            condition,
            consequent,
            alternate,
        } => {
            cruftscript_runtime_expression_calls_callback_param(condition, param_name)
                || cruftscript_runtime_expression_calls_callback_param(consequent, param_name)
                || cruftscript_runtime_expression_calls_callback_param(alternate, param_name)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::Value(_)
        | cruftscript_type_checker::LoweredRuntimeExpression::LocalRef(_)
        | cruftscript_type_checker::LoweredRuntimeExpression::EndowmentRef(_)
        | cruftscript_type_checker::LoweredRuntimeExpression::IntrinsicRef(_)
        | cruftscript_type_checker::LoweredRuntimeExpression::ImportedCall(_)
        | cruftscript_type_checker::LoweredRuntimeExpression::CrossCompartmentCall(_) => false,
    }
}

fn cruftscript_runtime_expression_retains_callback_param(
    expression: &cruftscript_type_checker::LoweredRuntimeExpression,
    param_name: &str,
) -> bool {
    match expression {
        cruftscript_type_checker::LoweredRuntimeExpression::LocalRef(name) => name == param_name,
        cruftscript_type_checker::LoweredRuntimeExpression::RuntimeCallbackCall(call) => call
            .args
            .iter()
            .any(|arg| cruftscript_runtime_expression_retains_callback_param(arg, param_name)),
        cruftscript_type_checker::LoweredRuntimeExpression::PredicateRecordCall(call) => {
            cruftscript_runtime_expression_retains_callback_param(&call.key, param_name)
                || cruftscript_runtime_expression_retains_callback_param(&call.arg, param_name)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::Let { value, body, .. } => {
            cruftscript_runtime_expression_retains_callback_param(value, param_name)
                || cruftscript_runtime_expression_retains_callback_param(body, param_name)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::Array(elements) => {
            elements.iter().any(|element| {
                cruftscript_runtime_expression_retains_callback_param(element, param_name)
            })
        }
        cruftscript_type_checker::LoweredRuntimeExpression::Object(properties)
        | cruftscript_type_checker::LoweredRuntimeExpression::ClassObject { properties, .. } => {
            properties.iter().any(|property| {
                cruftscript_runtime_expression_retains_callback_param(&property.value, param_name)
            })
        }
        cruftscript_type_checker::LoweredRuntimeExpression::BuiltinMethodCall {
            receiver,
            args,
            ..
        } => {
            cruftscript_runtime_expression_retains_callback_param(receiver, param_name)
                || args.iter().any(|arg| {
                    cruftscript_runtime_expression_retains_callback_param(arg, param_name)
                })
        }
        cruftscript_type_checker::LoweredRuntimeExpression::BuiltinConstruct { args, .. } => args
            .iter()
            .any(|arg| cruftscript_runtime_expression_retains_callback_param(arg, param_name)),
        cruftscript_type_checker::LoweredRuntimeExpression::Arrow { body, .. } => {
            cruftscript_runtime_expression_retains_callback_param(body, param_name)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::Binary { left, right, .. } => {
            cruftscript_runtime_expression_retains_callback_param(left, param_name)
                || cruftscript_runtime_expression_retains_callback_param(right, param_name)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::PropertyAccess { object, .. } => {
            cruftscript_runtime_expression_retains_callback_param(object, param_name)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::ComputedPropertyAccess {
            object,
            key,
            ..
        } => {
            cruftscript_runtime_expression_retains_callback_param(object, param_name)
                || cruftscript_runtime_expression_retains_callback_param(key, param_name)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::NonNullCheck { expr, .. } => {
            cruftscript_runtime_expression_retains_callback_param(expr, param_name)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::TypeCheck { expr, .. } => {
            cruftscript_runtime_expression_retains_callback_param(expr, param_name)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::Conditional {
            subject,
            consequent,
            alternate,
            ..
        } => {
            cruftscript_runtime_expression_retains_callback_param(subject, param_name)
                || cruftscript_runtime_expression_retains_callback_param(consequent, param_name)
                || cruftscript_runtime_expression_retains_callback_param(alternate, param_name)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::Ternary {
            condition,
            consequent,
            alternate,
        } => {
            cruftscript_runtime_expression_retains_callback_param(condition, param_name)
                || cruftscript_runtime_expression_retains_callback_param(consequent, param_name)
                || cruftscript_runtime_expression_retains_callback_param(alternate, param_name)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::Value(_)
        | cruftscript_type_checker::LoweredRuntimeExpression::EndowmentRef(_)
        | cruftscript_type_checker::LoweredRuntimeExpression::IntrinsicRef(_)
        | cruftscript_type_checker::LoweredRuntimeExpression::ImportedCall(_)
        | cruftscript_type_checker::LoweredRuntimeExpression::CrossCompartmentCall(_) => false,
    }
}

#[derive(Debug, Clone, Copy)]
struct CruftScriptStructuralArgBudget {
    depth_remaining: usize,
    nodes_remaining: usize,
}

fn cruftscript_runtime_value_to_lowered(
    rt: &Runtime,
    module_url: &str,
    function: &cruftscript_type_checker::LoweredFunction,
    index: usize,
    value: &Value,
    budget: &mut CruftScriptStructuralArgBudget,
) -> Result<cruftscript_type_checker::LoweredValue, RuntimeError> {
    if budget.nodes_remaining == 0 || budget.depth_remaining == 0 {
        return Err(cruftscript_runtime_structural_arg_error(
            module_url,
            function,
            index,
            "budget",
            "runtime structural argument exceeds the derived depth/size budget",
        ));
    }
    budget.nodes_remaining -= 1;
    match value {
        Value::Undefined => Ok(cruftscript_type_checker::LoweredValue::Undefined),
        Value::Null => Ok(cruftscript_type_checker::LoweredValue::Null),
        Value::String(value) => Ok(cruftscript_type_checker::LoweredValue::String(
            value.as_str().to_string(),
        )),
        Value::Number(value) => Ok(cruftscript_type_checker::LoweredValue::Number(
            value.to_string(),
        )),
        Value::Boolean(value) => Ok(cruftscript_type_checker::LoweredValue::Boolean(*value)),
        Value::BigInt(value) => Ok(cruftscript_type_checker::LoweredValue::BigInt(
            value.to_decimal(),
        )),
        Value::Object(id) => {
            if rt.typed_array_views.contains_key(id) {
                return cruftscript_runtime_typed_array_to_lowered(
                    rt, module_url, function, index, *id, budget,
                );
            }
            let object = rt.obj(*id);
            match &object.internal_kind {
                InternalKind::Array => cruftscript_runtime_array_to_lowered(
                    rt, module_url, function, index, *id, budget,
                ),
                InternalKind::Ordinary => cruftscript_runtime_object_to_lowered(
                    rt, module_url, function, index, *id, budget,
                ),
                InternalKind::Function(_)
                | InternalKind::Closure(_)
                | InternalKind::BoundFunction(_) => Err(cruftscript_runtime_structural_arg_error(
                    module_url,
                    function,
                    index,
                    "function",
                    "runtime structural argument contains a callable object",
                )),
                InternalKind::Promise(_) => Err(cruftscript_runtime_structural_arg_error(
                    module_url,
                    function,
                    index,
                    "promise",
                    "runtime structural argument contains a Promise",
                )),
                _ => Err(cruftscript_runtime_structural_arg_error(
                    module_url,
                    function,
                    index,
                    "host-object",
                    "runtime structural argument is not a plain ordinary object or dense array",
                )),
            }
        }
        other => {
            let domain = match other {
                Value::Symbol(_) => "symbol",
                Value::Undefined
                | Value::Null
                | Value::String(_)
                | Value::Number(_)
                | Value::Boolean(_)
                | Value::BigInt(_)
                | Value::Object(_) => unreachable!(),
            };
            let record = cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
                module_url,
                "RuntimeExportUnsupportedArgument",
                function.span,
                format!(
                    "cruftscript runtime export `{}` argument {} has unsupported runtime domain `{}`; derived calls currently accept only string, number, boolean, and bigint",
                    function.function_name, index, domain
                ),
                70,
            );
            Err(RuntimeError::TypeError(format!(
                "{}; {}",
                record.message,
                record.tooling_line()
            )))
        }
    }
}

fn cruftscript_runtime_array_to_lowered(
    rt: &Runtime,
    module_url: &str,
    function: &cruftscript_type_checker::LoweredFunction,
    index: usize,
    id: ObjectRef,
    budget: &mut CruftScriptStructuralArgBudget,
) -> Result<cruftscript_type_checker::LoweredValue, RuntimeError> {
    let object = rt.obj(id);
    if !cruftscript_runtime_array_has_plain_prototype(rt, object.proto) {
        return Err(cruftscript_runtime_structural_arg_error(
            module_url,
            function,
            index,
            "prototype",
            "runtime array argument has a prototype requiring semantic inspection",
        ));
    }
    if !object.array_dense {
        return Err(cruftscript_runtime_structural_arg_error(
            module_url,
            function,
            index,
            "sparse-array",
            "runtime array argument is not represented as a dense 0..length-1 own element range",
        ));
    }
    for (key, desc) in &object.properties {
        match key {
            PropertyKey::String(name) if name == "length" => {}
            PropertyKey::String(_) => {
                return Err(cruftscript_runtime_structural_arg_error(
                    module_url,
                    function,
                    index,
                    "array-extra-property",
                    "runtime array argument has non-index own properties",
                ));
            }
            PropertyKey::Symbol(_) => {
                return Err(cruftscript_runtime_structural_arg_error(
                    module_url,
                    function,
                    index,
                    "symbol-key",
                    "runtime array argument has an own Symbol-keyed property",
                ));
            }
        }
        if desc.getter.is_some() || desc.setter.is_some() {
            return Err(cruftscript_runtime_structural_arg_error(
                module_url,
                function,
                index,
                "accessor",
                "runtime array argument has an accessor descriptor",
            ));
        }
    }
    let len = object.array_store_len();
    if len > budget.nodes_remaining {
        return Err(cruftscript_runtime_structural_arg_error(
            module_url,
            function,
            index,
            "budget",
            "runtime array argument exceeds the derived size budget",
        ));
    }
    let elements = (0..len)
        .map(|i| {
            let value = rt.obj(id).array_store_get(i);
            let mut child_budget = *budget;
            child_budget.depth_remaining -= 1;
            let lowered = cruftscript_runtime_value_to_lowered(
                rt,
                module_url,
                function,
                index,
                &value,
                &mut child_budget,
            )?;
            budget.nodes_remaining = child_budget.nodes_remaining;
            Ok(lowered)
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    Ok(cruftscript_type_checker::LoweredValue::Array(elements))
}

fn cruftscript_runtime_typed_array_to_lowered(
    rt: &Runtime,
    module_url: &str,
    function: &cruftscript_type_checker::LoweredFunction,
    index: usize,
    id: ObjectRef,
    budget: &mut CruftScriptStructuralArgBudget,
) -> Result<cruftscript_type_checker::LoweredValue, RuntimeError> {
    let view = rt.typed_array_views.get(&id).ok_or_else(|| {
        cruftscript_runtime_structural_arg_error(
            module_url,
            function,
            index,
            "typed-array",
            "runtime typed-array argument has no view record",
        )
    })?;
    let kind = &*view.element_kind;
    if kind == "DataView" {
        return Err(cruftscript_runtime_structural_arg_error(
            module_url,
            function,
            index,
            "dataview",
            "runtime DataView argument carries byte-order and mutable-buffer semantics requiring explicit adapter derivation",
        ));
    }
    if kind == "BigInt64Array" || kind == "BigUint64Array" {
        return Err(cruftscript_runtime_structural_arg_error(
            module_url,
            function,
            index,
            "bigint-typed-array",
            "runtime BigInt typed-array argument cannot project into Array<number>",
        ));
    }
    let buffer = rt.array_buffers.get(&view.buffer).ok_or_else(|| {
        cruftscript_runtime_structural_arg_error(
            module_url,
            function,
            index,
            "typed-array-buffer",
            "runtime typed-array backing buffer is missing",
        )
    })?;
    if buffer.detached {
        return Err(cruftscript_runtime_structural_arg_error(
            module_url,
            function,
            index,
            "typed-array-detached",
            "runtime typed-array backing buffer is detached",
        ));
    }
    if buffer.shared.is_some() {
        return Err(cruftscript_runtime_structural_arg_error(
            module_url,
            function,
            index,
            "shared-typed-array",
            "runtime typed-array backed by shared memory cannot be copied through this adapter",
        ));
    }
    let width = view.bytes_per_element.max(1);
    let len = view
        .fixed_length
        .unwrap_or_else(|| buffer.byte_length.saturating_sub(view.byte_offset) / width);
    let byte_len = len.checked_mul(width).ok_or_else(|| {
        cruftscript_runtime_structural_arg_error(
            module_url,
            function,
            index,
            "budget",
            "runtime typed-array argument exceeds the derived size budget",
        )
    })?;
    let end = view.byte_offset.checked_add(byte_len).ok_or_else(|| {
        cruftscript_runtime_structural_arg_error(
            module_url,
            function,
            index,
            "typed-array-bounds",
            "runtime typed-array byte range overflows",
        )
    })?;
    if end > buffer.data.len() {
        return Err(cruftscript_runtime_structural_arg_error(
            module_url,
            function,
            index,
            "typed-array-bounds",
            "runtime typed-array byte range is out of bounds",
        ));
    }
    if len > budget.nodes_remaining {
        return Err(cruftscript_runtime_structural_arg_error(
            module_url,
            function,
            index,
            "budget",
            "runtime typed-array argument exceeds the derived size budget",
        ));
    }

    let mut elements = Vec::with_capacity(len);
    for i in 0..len {
        let start = view.byte_offset + i * width;
        let raw = rusty_js_runtime::abstract_ops::raw_bytes_to_numeric(
            kind,
            &buffer.data[start..start + width],
        );
        let Value::Number(number) = raw else {
            return Err(cruftscript_runtime_structural_arg_error(
                module_url,
                function,
                index,
                "typed-array-domain",
                "runtime typed-array element did not project to number",
            ));
        };
        budget.nodes_remaining -= 1;
        elements.push(cruftscript_type_checker::LoweredValue::Number(
            number.to_string(),
        ));
    }
    Ok(cruftscript_type_checker::LoweredValue::Array(elements))
}

fn cruftscript_runtime_array_has_plain_prototype(rt: &Runtime, proto: Option<ObjectRef>) -> bool {
    let Some(proto) = proto else {
        return true;
    };
    let Value::Object(array_ctor) = rt.global_get("Array") else {
        return false;
    };
    matches!(rt.object_get(array_ctor, "prototype"), Value::Object(array_proto) if array_proto == proto)
}

fn cruftscript_runtime_object_to_lowered(
    rt: &Runtime,
    module_url: &str,
    function: &cruftscript_type_checker::LoweredFunction,
    index: usize,
    id: ObjectRef,
    budget: &mut CruftScriptStructuralArgBudget,
) -> Result<cruftscript_type_checker::LoweredValue, RuntimeError> {
    let object = rt.obj(id);
    if !cruftscript_runtime_object_has_plain_prototype(rt, object.proto) {
        return Err(cruftscript_runtime_structural_arg_error(
            module_url,
            function,
            index,
            "prototype",
            "runtime object argument has a prototype requiring semantic inspection",
        ));
    }
    if object
        .properties
        .keys()
        .any(|key| matches!(key, PropertyKey::Symbol(_)))
    {
        return Err(cruftscript_runtime_structural_arg_error(
            module_url,
            function,
            index,
            "symbol-key",
            "runtime object argument has an own Symbol-keyed property",
        ));
    }

    let keys = object.string_key_clones().collect::<Vec<_>>();
    if keys.len() > budget.nodes_remaining {
        return Err(cruftscript_runtime_structural_arg_error(
            module_url,
            function,
            index,
            "budget",
            "runtime object argument exceeds the derived size budget",
        ));
    }

    let mut properties = Vec::with_capacity(keys.len());
    for name in keys {
        let value = {
            let object = rt.obj(id);
            if let Some(value) = object.shape_get(&name) {
                value.clone()
            } else {
                let Some(desc) = object.get_own_str_borrowed(&name) else {
                    return Err(cruftscript_runtime_structural_arg_error(
                        module_url,
                        function,
                        index,
                        "missing-descriptor",
                        "runtime object argument key had no own descriptor",
                    ));
                };
                if desc.getter.is_some() || desc.setter.is_some() {
                    return Err(cruftscript_runtime_structural_arg_error(
                        module_url,
                        function,
                        index,
                        "accessor",
                        "runtime object argument has an accessor descriptor",
                    ));
                }
                if !desc.enumerable {
                    return Err(cruftscript_runtime_structural_arg_error(
                        module_url,
                        function,
                        index,
                        "non-enumerable",
                        "runtime object argument has a non-enumerable own data property",
                    ));
                }
                desc.value.clone()
            }
        };
        let mut child_budget = *budget;
        child_budget.depth_remaining -= 1;
        let lowered = cruftscript_runtime_value_to_lowered(
            rt,
            module_url,
            function,
            index,
            &value,
            &mut child_budget,
        )?;
        budget.nodes_remaining = child_budget.nodes_remaining;
        properties.push(cruftscript_type_checker::LoweredObjectProperty {
            name,
            value: lowered,
        });
    }

    Ok(cruftscript_type_checker::LoweredValue::Object(properties))
}

fn cruftscript_runtime_object_has_plain_prototype(rt: &Runtime, proto: Option<ObjectRef>) -> bool {
    let Some(proto) = proto else {
        return true;
    };
    let Value::Object(object_ctor) = rt.global_get("Object") else {
        return false;
    };
    matches!(rt.object_get(object_ctor, "prototype"), Value::Object(object_proto) if object_proto == proto)
}

fn cruftscript_runtime_structural_arg_error(
    module_url: &str,
    function: &cruftscript_type_checker::LoweredFunction,
    index: usize,
    coordinate: &str,
    message: &str,
) -> RuntimeError {
    let record = cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
        module_url,
        "RuntimeExportUnsupportedStructuralArgument",
        function.span,
        format!(
            "cruftscript runtime export `{}` argument {} has unsupported structural shape `{}`: {}",
            function.function_name, index, coordinate, message
        ),
        70,
    );
    RuntimeError::TypeError(format!("{}; {}", record.message, record.tooling_line()))
}

fn cruftscript_lowered_value_to_runtime(
    rt: &mut Runtime,
    module_url: &str,
    value: &cruftscript_type_checker::LoweredValue,
) -> Result<Value, RuntimeError> {
    match value {
        cruftscript_type_checker::LoweredValue::Undefined => Ok(Value::Undefined),
        cruftscript_type_checker::LoweredValue::Null => Ok(Value::Null),
        cruftscript_type_checker::LoweredValue::String(value) => Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(value.clone()),
        ))),
        cruftscript_type_checker::LoweredValue::Number(value) => {
            Ok(Value::Number(value.parse::<f64>().unwrap_or(f64::NAN)))
        }
        cruftscript_type_checker::LoweredValue::Boolean(value) => Ok(Value::Boolean(*value)),
        cruftscript_type_checker::LoweredValue::BigInt(value) => {
            let bigint = rusty_js_runtime::bigint::JsBigInt::from_decimal(value)
                .ok_or_else(|| RuntimeError::TypeError("invalid lowered bigint".to_string()))?;
            Ok(Value::BigInt(Rc::new(bigint)))
        }
        cruftscript_type_checker::LoweredValue::FunctionRef(name) => {
            let namespace = rt.load_module(module_url)?;
            let exported = rt.object_get(namespace, name);
            match exported {
                Value::Object(id)
                    if matches!(rt.obj(id).internal_kind, InternalKind::BoundaryWrapper(_)) =>
                {
                    Ok(Value::Object(id))
                }
                _ => Err(RuntimeError::TypeError(format!(
                    "cruftscript function value `{name}` cannot cross boundary because the module export is not a BoundaryWrapper"
                ))),
            }
        }
        cruftscript_type_checker::LoweredValue::ClassConstructorRef(name) => {
            Err(RuntimeError::TypeError(format!(
                "cruftscript class constructor value `{name}` cannot cross boundary as a runtime value"
            )))
        }
        cruftscript_type_checker::LoweredValue::Array(elements) => {
            let array = rt.alloc_object(Object::new_array());
            for (index, element) in elements.iter().enumerate() {
                let value = cruftscript_lowered_value_to_runtime(rt, module_url, element)?;
                rt.object_set(array, index.to_string(), value);
            }
            rt.object_set(
                array,
                "length".to_string(),
                Value::Number(elements.len() as f64),
            );
            Ok(Value::Object(array))
        }
        cruftscript_type_checker::LoweredValue::Object(properties) => {
            let object = rt.alloc_object(Object::new_ordinary());
            for property in properties {
                let value = cruftscript_lowered_value_to_runtime(rt, module_url, &property.value)?;
                rt.object_set(object, property.name.clone(), value);
            }
            Ok(Value::Object(object))
        }
    }
}

fn cruftscript_runtime_expression_to_runtime(
    rt: &mut Runtime,
    module_url: &str,
    expression: &cruftscript_type_checker::LoweredRuntimeExpression,
) -> Result<Value, RuntimeError> {
    let mut env = HashMap::new();
    cruftscript_runtime_expression_to_runtime_with_env(rt, module_url, expression, &mut env)
}

fn cruftscript_builtin_receiver_method(rt: &mut Runtime, receiver: &Value, method: &str) -> Value {
    let proto_id = match receiver {
        Value::Object(id) => return rt.object_get(*id, method),
        Value::String(_) => cruftscript_intrinsic_prototype(rt, "String"),
        Value::Number(_) => cruftscript_intrinsic_prototype(rt, "Number"),
        Value::Boolean(_) => cruftscript_intrinsic_prototype(rt, "Boolean"),
        _ => None,
    };
    match proto_id {
        Some(proto_id) => rt.object_get(proto_id, method),
        None => Value::Undefined,
    }
}

fn cruftscript_intrinsic_prototype(rt: &mut Runtime, intrinsic: &str) -> Option<ObjectRef> {
    let Value::Object(ctor_id) = rt.global_get(intrinsic) else {
        return None;
    };
    match rt.object_get(ctor_id, "prototype") {
        Value::Object(proto_id) => Some(proto_id),
        _ => None,
    }
}

fn cruftscript_binary_num(v: &Value) -> f64 {
    match v {
        Value::Number(n) => *n,
        Value::Boolean(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        Value::Null => 0.0,
        _ => f64::NAN,
    }
}

fn cruftscript_truthy(v: &Value) -> bool {
    match v {
        Value::Boolean(b) => *b,
        Value::Number(n) => *n != 0.0 && !n.is_nan(),
        Value::String(s) => s.code_unit_len() != 0,
        Value::Null | Value::Undefined => false,
        _ => true,
    }
}

fn cruftscript_strict_eq(l: &Value, r: &Value) -> bool {
    match (l, r) {
        (Value::Number(a), Value::Number(b)) => a == b,
        (Value::String(a), Value::String(b)) => a.as_str() == b.as_str(),
        (Value::Boolean(a), Value::Boolean(b)) => a == b,
        (Value::Null, Value::Null) | (Value::Undefined, Value::Undefined) => true,
        (Value::Object(a), Value::Object(b)) => a == b,
        _ => false,
    }
}

fn cruftscript_relational(
    l: &Value,
    r: &Value,
    op: cruftscript_type_checker::RuntimeBinaryOp,
) -> bool {
    use cruftscript_type_checker::RuntimeBinaryOp::{Ge, Gt, Le, Lt};
    if let (Value::String(a), Value::String(b)) = (l, r) {
        let (a, b) = (a.as_str(), b.as_str());
        return match op {
            Lt => a < b,
            Gt => a > b,
            Le => a <= b,
            Ge => a >= b,
            _ => false,
        };
    }
    let (a, b) = (cruftscript_binary_num(l), cruftscript_binary_num(r));
    if a.is_nan() || b.is_nan() {
        return false;
    }
    match op {
        Lt => a < b,
        Gt => a > b,
        Le => a <= b,
        Ge => a >= b,
        _ => false,
    }
}

fn cruftscript_apply_binary(
    rt: &mut Runtime,
    op: cruftscript_type_checker::RuntimeBinaryOp,
    l: Value,
    r: Value,
) -> Result<Value, RuntimeError> {
    use cruftscript_type_checker::RuntimeBinaryOp::*;
    Ok(match op {
        Add => return rt.op_add_rt(&l, &r),
        Sub => Value::Number(cruftscript_binary_num(&l) - cruftscript_binary_num(&r)),
        Mul => Value::Number(cruftscript_binary_num(&l) * cruftscript_binary_num(&r)),
        Pow => Value::Number(cruftscript_binary_num(&l).powf(cruftscript_binary_num(&r))),
        Div => Value::Number(cruftscript_binary_num(&l) / cruftscript_binary_num(&r)),
        Rem => Value::Number(cruftscript_binary_num(&l) % cruftscript_binary_num(&r)),
        Lt | Gt | Le | Ge => Value::Boolean(cruftscript_relational(&l, &r, op)),
        StrictEq => Value::Boolean(cruftscript_strict_eq(&l, &r)),
        StrictNe => Value::Boolean(!cruftscript_strict_eq(&l, &r)),
        NullishCoalesce => {
            if matches!(l, Value::Undefined | Value::Null) {
                r
            } else {
                l
            }
        }
        And => {
            if cruftscript_truthy(&l) {
                r
            } else {
                l
            }
        }
        Or => {
            if cruftscript_truthy(&l) {
                l
            } else {
                r
            }
        }
    })
}

fn cruftscript_runtime_expression_to_runtime_with_env(
    rt: &mut Runtime,
    module_url: &str,
    expression: &cruftscript_type_checker::LoweredRuntimeExpression,
    env: &mut HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match expression {
        cruftscript_type_checker::LoweredRuntimeExpression::Value(value) => {
            cruftscript_lowered_value_to_runtime(rt, module_url, value)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::LocalRef(name) => {
            Ok(env.get(name).cloned().unwrap_or(Value::Undefined))
        }
        cruftscript_type_checker::LoweredRuntimeExpression::EndowmentRef(endowment) => {
            cruftscript_runtime_endowment_get(rt, module_url, endowment)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::IntrinsicRef(intrinsic) => {
            Ok(rt.global_get(&intrinsic.name))
        }
        cruftscript_type_checker::LoweredRuntimeExpression::Let { name, value, body } => {
            let value =
                cruftscript_runtime_expression_to_runtime_with_env(rt, module_url, value, env)?;
            let previous = env.insert(name.clone(), value);
            let result =
                cruftscript_runtime_expression_to_runtime_with_env(rt, module_url, body, env);
            if let Some(previous) = previous {
                env.insert(name.clone(), previous);
            } else {
                env.remove(name);
            }
            result
        }
        cruftscript_type_checker::LoweredRuntimeExpression::ImportedCall(call) => {
            cruftscript_eval_imported_call(rt, module_url, call)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::CrossCompartmentCall(call) => {
            cruftscript_eval_cross_compartment_call(rt, module_url, call)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::RuntimeCallbackCall(call) => {
            cruftscript_eval_runtime_callback_call(rt, module_url, call, env)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::PredicateRecordCall(call) => {
            cruftscript_eval_predicate_record_call(rt, module_url, call, env)
        }

        cruftscript_type_checker::LoweredRuntimeExpression::BuiltinMethodCall {
            receiver,
            method,
            args,
            ..
        } => {
            let receiver_value =
                cruftscript_runtime_expression_to_runtime_with_env(rt, module_url, receiver, env)?;
            let method_fn = cruftscript_builtin_receiver_method(rt, &receiver_value, method);
            let mut argv = Vec::with_capacity(args.len());
            for arg in args {
                argv.push(cruftscript_runtime_expression_to_runtime_with_env(
                    rt, module_url, arg, env,
                )?);
            }
            rt.call_function(method_fn, receiver_value, argv)
        }

        cruftscript_type_checker::LoweredRuntimeExpression::BuiltinConstruct {
            constructor,
            args,
            ..
        } => {
            let ctor = rt.global_get(constructor);
            let mut argv = Vec::with_capacity(args.len());
            for arg in args {
                argv.push(cruftscript_runtime_expression_to_runtime_with_env(
                    rt, module_url, arg, env,
                )?);
            }
            rt.construct(ctor, argv)
        }

        cruftscript_type_checker::LoweredRuntimeExpression::Binary {
            op, left, right, ..
        } => {
            let l = cruftscript_runtime_expression_to_runtime_with_env(rt, module_url, left, env)?;
            let r = cruftscript_runtime_expression_to_runtime_with_env(rt, module_url, right, env)?;
            cruftscript_apply_binary(rt, *op, l, r)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::Arrow { params, body, .. } => {
            let params = params.clone();
            let body = (**body).clone();
            let module_url_owned = module_url.to_string();
            let captured_env = env.clone();
            Ok(crate::register::native_function(
                rt,
                "cruftscriptArrow",
                move |rt, args| {
                    let mut call_env = captured_env.clone();
                    for (index, name) in params.iter().enumerate() {
                        call_env.insert(
                            name.clone(),
                            args.get(index).cloned().unwrap_or(Value::Undefined),
                        );
                    }
                    cruftscript_runtime_expression_to_runtime_with_env(
                        rt,
                        &module_url_owned,
                        &body,
                        &mut call_env,
                    )
                },
            ))
        }
        cruftscript_type_checker::LoweredRuntimeExpression::Array(elements) => {
            let array = rt.alloc_object(Object::new_array());
            for (index, element) in elements.iter().enumerate() {
                let value = cruftscript_runtime_expression_to_runtime_with_env(
                    rt, module_url, element, env,
                )?;
                rt.object_set(array, index.to_string(), value);
            }
            rt.object_set(
                array,
                "length".to_string(),
                Value::Number(elements.len() as f64),
            );
            Ok(Value::Object(array))
        }
        cruftscript_type_checker::LoweredRuntimeExpression::Object(properties) => {
            let object = rt.alloc_object(Object::new_ordinary());
            for property in properties {
                let value = cruftscript_runtime_expression_to_runtime_with_env(
                    rt,
                    module_url,
                    &property.value,
                    env,
                )?;
                rt.object_set(object, property.name.clone(), value);
            }
            Ok(Value::Object(object))
        }
        cruftscript_type_checker::LoweredRuntimeExpression::ClassObject {
            class_name,
            properties,
        } => {
            let object = rt.alloc_object(Object::new_ordinary());
            rt.obj_mut(object).cruftscript_class_brand = Some(class_name.clone());
            for property in properties {
                let value = cruftscript_runtime_expression_to_runtime_with_env(
                    rt,
                    module_url,
                    &property.value,
                    env,
                )?;
                rt.object_set(object, property.name.clone(), value);
            }
            Ok(Value::Object(object))
        }
        cruftscript_type_checker::LoweredRuntimeExpression::PropertyAccess {
            object,
            property,
            optional,
            span,
        } => {
            let object =
                cruftscript_runtime_expression_to_runtime_with_env(rt, module_url, object, env)?;
            if *optional && matches!(object, Value::Undefined | Value::Null) {
                return Ok(Value::Undefined);
            }
            match object {

                Value::Object(object_id) => rt.get_via(
                    &Value::Object(object_id),
                    &Value::String(std::rc::Rc::new(rusty_js_runtime::value::JsString::from(
                        property.as_str(),
                    ))),
                ),
                Value::Undefined | Value::Null => Err(
                    cruftscript_runtime_expression_property_error(module_url, *span, property),
                ),

                Value::String(ref s) if property == "length" => {
                    Ok(Value::Number(s.code_unit_len() as f64))
                }

                other => Ok(cruftscript_builtin_receiver_method(rt, &other, property)),
            }
        }
        cruftscript_type_checker::LoweredRuntimeExpression::ComputedPropertyAccess {
            object,
            key,
            optional,
            span,
        } => {
            let object =
                cruftscript_runtime_expression_to_runtime_with_env(rt, module_url, object, env)?;
            if *optional && matches!(object, Value::Undefined | Value::Null) {
                return Ok(Value::Undefined);
            }
            let key = cruftscript_runtime_expression_to_runtime_with_env(rt, module_url, key, env)?;
            let key = match key {
                Value::String(key) => key.as_str().to_string(),
                Value::Number(number)
                    if number.is_finite() && number >= 0.0 && number.fract() == 0.0 =>
                {
                    format!("{number:.0}")
                }
                _ => {
                    return Err(cruftscript_runtime_expression_computed_property_error(
                        module_url, *span,
                    ));
                }
            };
            let Value::Object(object_id) = object else {
                return Err(cruftscript_runtime_expression_computed_property_error(
                    module_url, *span,
                ));
            };
            Ok(rt.object_get(object_id, &key))
        }
        cruftscript_type_checker::LoweredRuntimeExpression::NonNullCheck { expr, span } => {
            let value =
                cruftscript_runtime_expression_to_runtime_with_env(rt, module_url, expr, env)?;
            if matches!(value, Value::Null | Value::Undefined) {
                return Err(cruftscript_runtime_non_null_assertion_error(
                    module_url, *span,
                ));
            }
            Ok(value)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::TypeCheck {
            expr,
            target_type,
            span,
        } => {
            let value =
                cruftscript_runtime_expression_to_runtime_with_env(rt, module_url, expr, env)?;
            if !cruftscript_runtime_value_matches_type(rt, &value, target_type) {
                return Err(cruftscript_runtime_type_assertion_error(
                    module_url,
                    *span,
                    &cruftscript_type_term_name(target_type),
                    cruftscript_runtime_value_type_name(&value),
                ));
            }
            Ok(value)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::Conditional {
            subject,
            guard,
            consequent,
            alternate,
        } => {
            let subject =
                cruftscript_runtime_expression_to_runtime_with_env(rt, module_url, subject, env)?;
            let selected = if cruftscript_runtime_narrowing_guard_matches_with_env(
                rt, module_url, &subject, guard, env,
            )? {
                consequent
            } else {
                alternate
            };
            cruftscript_runtime_expression_to_runtime_with_env(rt, module_url, selected, env)
        }
        cruftscript_type_checker::LoweredRuntimeExpression::Ternary {
            condition,
            consequent,
            alternate,
        } => {
            let condition =
                cruftscript_runtime_expression_to_runtime_with_env(rt, module_url, condition, env)?;
            let selected = if cruftscript_truthy(&condition) {
                consequent
            } else {
                alternate
            };
            cruftscript_runtime_expression_to_runtime_with_env(rt, module_url, selected, env)
        }
    }
}

fn cruftscript_eval_predicate_record_call(
    rt: &mut Runtime,
    module_url: &str,
    call: &cruftscript_type_checker::LoweredRuntimePredicateRecordCall,
    env: &mut HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    let key = cruftscript_runtime_expression_to_runtime_with_env(rt, module_url, &call.key, env)?;
    let Value::String(key) = key else {
        return Err(RuntimeError::TypeError(format!(
            "cruftscript dynamic predicate record call requires a string key at {}:{}",
            module_url, call.span.start
        )));
    };
    let Some(candidate) = call
        .candidates
        .iter()
        .find(|candidate| candidate.property == key.as_str())
    else {
        if call.optional {
            return Ok(Value::Undefined);
        }
        return Err(RuntimeError::TypeError(format!(
            "cruftscript dynamic predicate record call cannot resolve own predicate key `{}` at {}:{}",
            key.as_str(),
            module_url,
            call.span.start
        )));
    };
    let arg = cruftscript_runtime_expression_to_runtime_with_env(rt, module_url, &call.arg, env)?;
    let namespace = rt.load_module(module_url)?;
    let callee = rt.object_get(namespace, &candidate.function_name);
    match rt.call_function(callee, Value::Undefined, vec![arg])? {
        Value::Boolean(value) => Ok(Value::Boolean(value)),
        other => Err(RuntimeError::TypeError(format!(
            "cruftscript dynamic predicate `{}` returned {}, expected boolean",
            candidate.function_name,
            cruftscript_runtime_value_type_name(&other)
        ))),
    }
}

fn cruftscript_eval_runtime_callback_call(
    rt: &mut Runtime,
    module_url: &str,
    call: &cruftscript_type_checker::LoweredRuntimeCallbackCall,
    env: &mut HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    let callback = env
        .get(&call.callback_name)
        .cloned()
        .unwrap_or(Value::Undefined);
    if call.optional && matches!(callback, Value::Undefined | Value::Null) {
        return Ok(Value::Undefined);
    }
    if !rt.is_callable(&callback) {
        let record = cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
            module_url,
            "RuntimeCallbackTargetNonCallable",
            call.span,
            format!(
                "cruftscript runtime callback `{}` requires a callable boundary argument; policy_id={}; policy_name={}; resolution_chain={}",
                call.callback_name,
                call.policy_id,
                call.policy_name,
                call.resolution_chain
            ),
            70,
        );
        return Err(RuntimeError::TypeError(format!(
            "{}; {}",
            record.message,
            record.tooling_line()
        )));
    }
    let args = call
        .args
        .iter()
        .map(|arg| cruftscript_runtime_expression_to_runtime_with_env(rt, module_url, arg, env))
        .collect::<Result<Vec<_>, _>>()?;
    match rt.call_function(callback, Value::Undefined, args) {
        Ok(value) => Ok(value),
        Err(err) => Err(cruftscript_runtime_callback_error(
            rt, module_url, call, err,
        )),
    }
}

fn cruftscript_runtime_callback_error(
    rt: &Runtime,
    module_url: &str,
    call: &cruftscript_type_checker::LoweredRuntimeCallbackCall,
    err: RuntimeError,
) -> RuntimeError {
    let message = match err {
        RuntimeError::TypeError(message)
        | RuntimeError::ReferenceError(message)
        | RuntimeError::SyntaxError(message) => message,
        RuntimeError::Thrown(value) => cruftscript_format_thrown_value(rt, &value),
        other => format!("{other:?}"),
    };
    let record = cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
        module_url,
        "RuntimeCallbackCallRejected",
        call.span,
        format!(
            "cruftscript runtime callback `{}` rejected during boundary invocation: {}; policy_id={}; policy_name={}; resolution_chain={}",
            call.callback_name,
            message,
            call.policy_id,
            call.policy_name,
            call.resolution_chain
        ),
        70,
    );
    RuntimeError::TypeError(format!("{}; {}", record.message, record.tooling_line()))
}

fn cruftscript_format_thrown_value(rt: &Runtime, value: &Value) -> String {
    match value {
        Value::String(message) => format!("Thrown: {}", message.as_str()),
        Value::Object(id) => {
            let mut name = match rt.object_get(*id, "name") {
                Value::String(name) => name.as_str().to_string(),
                _ => String::new(),
            };
            if name.is_empty() {
                if let Value::Object(ctor) = rt.object_get(*id, "constructor") {
                    if let Value::String(ctor_name) = rt.object_get(ctor, "name") {
                        name = ctor_name.as_str().to_string();
                    }
                }
            }
            let message = match rt.object_get(*id, "message") {
                Value::String(message) => message.as_str().to_string(),
                _ => String::new(),
            };
            if !name.is_empty() && !message.is_empty() {
                format!("Thrown: {name}: {message}")
            } else if !message.is_empty() {
                format!("Thrown: {message}")
            } else if !name.is_empty() {
                format!("Thrown: {name}")
            } else {
                format!("Thrown: {:?}", value)
            }
        }
        _ => format!("Thrown: {:?}", value),
    }
}

fn cruftscript_runtime_endowment_get(
    rt: &mut Runtime,
    module_url: &str,
    endowment: &cruftscript_type_checker::LoweredEndowmentRef,
) -> Result<Value, RuntimeError> {
    let registry = rt.global_get("__cruftscript_endowments");
    let Value::Object(registry_id) = registry else {
        return Err(cruftscript_runtime_endowment_error(module_url, endowment));
    };
    let key = format!("{}::{}", endowment.compartment, endowment.name);
    let value = rt.object_get(registry_id, &key);
    if matches!(value, Value::Undefined) {
        return Err(cruftscript_runtime_endowment_error(module_url, endowment));
    }
    Ok(value)
}

fn cruftscript_runtime_endowment_error(
    module_url: &str,
    endowment: &cruftscript_type_checker::LoweredEndowmentRef,
) -> RuntimeError {
    let record = cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
        module_url,
        "RuntimeEndowmentMissing",
        endowment.span,
        format!(
            "cruftscript compartment `{}` requires explicit endowment `{}` before runtime execution",
            endowment.compartment, endowment.name
        ),
        70,
    );
    RuntimeError::ReferenceError(format!("{}; {}", record.message, record.tooling_line()))
}

fn cruftscript_runtime_narrowing_guard_matches_with_env(
    rt: &mut Runtime,
    module_url: &str,
    value: &Value,
    guard: &cruftscript_type_checker::LoweredRuntimeNarrowingGuard,
    env: &mut HashMap<String, Value>,
) -> Result<bool, RuntimeError> {
    Ok(match guard {
        cruftscript_type_checker::LoweredRuntimeNarrowingGuard::TypeofEquals { expected } => {
            cruftscript_runtime_typeof(value) == expected
        }
        cruftscript_type_checker::LoweredRuntimeNarrowingGuard::OwnProperty { property } => {
            cruftscript_runtime_value_has_plain_own_property(rt, value, property)
        }
        cruftscript_type_checker::LoweredRuntimeNarrowingGuard::DynamicOwnProperty { key } => {
            let key = cruftscript_runtime_expression_to_runtime_with_env(rt, module_url, key, env)?;
            let Value::String(key) = key else {
                return Ok(false);
            };
            cruftscript_runtime_value_has_plain_own_property(rt, value, key.as_str())
        }
        cruftscript_type_checker::LoweredRuntimeNarrowingGuard::Instanceof { constructor } => {
            matches!(
                value,
                Value::Object(id)
                    if rt.obj(*id).cruftscript_class_brand.as_deref() == Some(constructor.as_str())
            )
        }
        cruftscript_type_checker::LoweredRuntimeNarrowingGuard::Truthy => {
            cruftscript_runtime_truthy(value)
        }
    })
}

fn cruftscript_runtime_value_has_plain_own_property(
    rt: &Runtime,
    value: &Value,
    property: &str,
) -> bool {
    let Value::Object(id) = value else {
        return false;
    };
    if !cruftscript_runtime_object_has_plain_projection_shape(rt, *id) {
        return false;
    }
    cruftscript_runtime_plain_object_property_value(rt, *id, property).is_some()
}

fn cruftscript_runtime_typeof(value: &Value) -> &'static str {
    match value {
        Value::Undefined => "undefined",
        Value::String(_) => "string",
        Value::Number(_) => "number",
        Value::Boolean(_) => "boolean",
        Value::BigInt(_) => "bigint",
        Value::Symbol(_) => "symbol",
        Value::Null | Value::Object(_) => "object",
    }
}

fn cruftscript_runtime_truthy(value: &Value) -> bool {
    match value {
        Value::Undefined | Value::Null => false,
        Value::Boolean(value) => *value,
        Value::Number(value) => *value != 0.0 && !value.is_nan(),
        Value::String(value) => !value.as_str().is_empty(),
        Value::BigInt(_) | Value::Symbol(_) => true,
        Value::Object(_) => true,
    }
}

fn cruftscript_runtime_expression_property_error(
    module_url: &str,
    span: Span,
    property: &str,
) -> RuntimeError {
    let record = cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
        module_url,
        "RuntimeExpressionPropertyAccessUnsupported",
        span,
        format!("cruftscript runtime expression cannot read property `{property}` from a non-object value"),
        70,
    );
    RuntimeError::TypeError(format!("{}; {}", record.message, record.tooling_line()))
}

fn cruftscript_runtime_expression_computed_property_error(
    module_url: &str,
    span: Span,
) -> RuntimeError {
    let record = cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
        module_url,
        "RuntimeExpressionComputedPropertyAccessUnsupported",
        span,
        "cruftscript runtime expression computed property access requires an object and string key"
            .to_string(),
        70,
    );
    RuntimeError::TypeError(format!("{}; {}", record.message, record.tooling_line()))
}

fn cruftscript_runtime_non_null_assertion_error(module_url: &str, span: Span) -> RuntimeError {
    let record = cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
        module_url,
        "RuntimeNonNullAssertionFailed",
        span,
        "cruftscript non-null assertion failed at runtime".to_string(),
        70,
    );
    RuntimeError::TypeError(format!("{}; {}", record.message, record.tooling_line()))
}

fn cruftscript_runtime_type_assertion_error(
    module_url: &str,
    span: Span,
    expected: &str,
    received: &str,
) -> RuntimeError {
    let record = cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
        module_url,
        "RuntimeTypeAssertionFailed",
        span,
        format!("cruftscript type assertion failed at runtime: expected {expected}, received {received}"),
        70,
    );
    RuntimeError::TypeError(format!("{}; {}", record.message, record.tooling_line()))
}

fn cruftscript_runtime_value_type_name(value: &Value) -> &'static str {
    match value {
        Value::Undefined => "undefined",
        Value::Null => "object",
        Value::Boolean(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::BigInt(_) => "bigint",
        Value::Symbol(_) => "symbol",
        Value::Object(_) => "object",
    }
}

fn cruftscript_eval_cross_compartment_call(
    rt: &mut Runtime,
    module_url: &str,
    call: &cruftscript_type_checker::LoweredCrossCompartmentCall,
) -> Result<Value, RuntimeError> {
    let namespace = rt.load_module(module_url)?;
    let callee = rt.object_get(namespace, &call.callee_name);
    let Value::Object(callee_id) = callee.clone() else {
        return Err(cruftscript_cross_compartment_non_callable_error(
            module_url, call,
        ));
    };
    if !matches!(
        rt.obj(callee_id).internal_kind,
        InternalKind::Function(_)
            | InternalKind::Closure(_)
            | InternalKind::BoundFunction(_)
            | InternalKind::BoundaryWrapper(_)
    ) {
        return Err(cruftscript_cross_compartment_non_callable_error(
            module_url, call,
        ));
    }
    let args = call
        .args
        .iter()
        .map(|arg| cruftscript_lowered_value_to_runtime(rt, module_url, arg))
        .collect::<Result<Vec<_>, _>>()?;
    rt.unified_cross_boundary_call(callee, args, None, None)
        .map_err(|err| cruftscript_cross_compartment_error(module_url, call, err))
}

fn cruftscript_cross_compartment_error(
    module_url: &str,
    call: &cruftscript_type_checker::LoweredCrossCompartmentCall,
    err: RuntimeError,
) -> RuntimeError {
    let message = match err {
        RuntimeError::TypeError(message)
        | RuntimeError::ReferenceError(message)
        | RuntimeError::SyntaxError(message) => message,
        other => format!("{other:?}"),
    };
    let record = cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
        module_url,
        "CrossCompartmentCallRejected",
        call.span,
        format!(
            "cruftscript cross-compartment call `{}` from `{}` to `{}` rejected (policy_id={} policy_name={} resolution_chain={}): {}",
            call.callee_name,
            call.caller_compartment,
            call.callee_compartment,
            call.policy_id,
            call.policy_name,
            call.resolution_chain,
            message
        ),
        70,
    );
    RuntimeError::TypeError(format!("{}; {}", record.message, record.tooling_line()))
}

fn cruftscript_cross_compartment_non_callable_error(
    module_url: &str,
    call: &cruftscript_type_checker::LoweredCrossCompartmentCall,
) -> RuntimeError {
    let record = cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
        module_url,
        "CrossCompartmentTargetNonCallable",
        call.span,
        format!(
            "cruftscript cross-compartment call `{}` from `{}` to `{}` requires a callable exported target (policy_id={} policy_name={} resolution_chain={})",
            call.callee_name,
            call.caller_compartment,
            call.callee_compartment,
            call.policy_id,
            call.policy_name,
            call.resolution_chain
        ),
        70,
    );
    RuntimeError::TypeError(format!("{}; {}", record.message, record.tooling_line()))
}

fn cruftscript_eval_imported_call(
    rt: &mut Runtime,
    module_url: &str,
    call: &cruftscript_type_checker::LoweredImportedCall,
) -> Result<Value, RuntimeError> {
    let resolved = Runtime::resolve_module(module_url, &call.source)?;
    let namespace = rt.load_module(&resolved)?;
    let imported = rt.object_get(namespace, &call.imported_name);
    let Value::Object(imported_id) = imported.clone() else {
        return Err(cruftscript_import_edge_non_callable_error(module_url, call));
    };
    if !matches!(
        rt.obj(imported_id).internal_kind,
        InternalKind::Function(_) | InternalKind::Closure(_) | InternalKind::BoundFunction(_)
    ) {
        return Err(cruftscript_import_edge_non_callable_error(module_url, call));
    }
    let result = rt.call_function(imported, Value::Undefined, Vec::new())?;
    cruftscript_adopt_import_edge_result(rt, module_url, call, result)
}

fn cruftscript_adopt_import_edge_result(
    rt: &mut Runtime,
    module_url: &str,
    call: &cruftscript_type_checker::LoweredImportedCall,
    result: Value,
) -> Result<Value, RuntimeError> {
    let Value::Object(result_id) = result.clone() else {
        return cruftscript_validate_import_edge_result(rt, module_url, call, result);
    };
    let promise_state = {
        let object = rt.obj(result_id);
        match &object.internal_kind {
            InternalKind::Promise(state) => Some((state.status, state.value.clone())),
            _ => None,
        }
    };
    let Some((status, value)) = promise_state else {
        if cruftscript_import_edge_result_is_thenable(rt, result_id) {
            return Err(cruftscript_import_edge_thenable_error(module_url, call));
        }
        if cruftscript_import_edge_result_is_fs_watcher(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "fs-watcher",
                "retained fs watcher handles cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_fs_read_stream(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "fs-read-stream",
                "retained fs.ReadStream host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_blob(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "blob",
                "retained Blob host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process",
                "retained process host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_argv(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-argv",
                "retained process.argv invocation metadata arrays cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_exec_argv(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-exec-argv",
                "retained process.execArgv invocation flag arrays cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_hrtime(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-hrtime",
                "retained process.hrtime high-resolution clock arrays cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_binding_constants(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-binding-constants",
                "retained process.binding(\"constants\") internal namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_binding_fs(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-binding-fs",
                "retained process.binding(\"fs\") internal namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_binding_unknown(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-binding-unknown",
                "retained process.binding fallback namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_env(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-env",
                "retained process.env host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_versions(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-versions",
                "retained process.versions host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_release(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-release",
                "retained process.release host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_config(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-config",
                "retained process.config host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_events(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-events",
                "retained process._events host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_events_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-events-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_event_emitter(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "event-emitter",
                "retained EventEmitter objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_finalization(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-finalization",
                "retained process.finalization host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_module_load_list(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-module-load-list",
                "retained process.moduleLoadList host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_preload_modules(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-preload-modules",
                "retained process._preload_modules host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_features(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-features",
                "retained process.features host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_allowed_node_environment_flags(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-allowed-node-environment-flags",
                "retained process.allowedNodeEnvironmentFlags host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_cpu_usage(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-cpu-usage",
                "retained process.cpuUsage host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_memory_usage(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-memory-usage",
                "retained process.memoryUsage host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_resource_usage(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-resource-usage",
                "retained process.resourceUsage host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_thread_cpu_usage(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-thread-cpu-usage",
                "retained process.threadCpuUsage host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_groups(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-groups",
                "retained process.getgroups host group identity arrays cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_active_resources(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-active-resources",
                "retained process.getActiveResourcesInfo host activity arrays cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_event_names(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-event-names",
                "retained process.eventNames host event registry arrays cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_listeners(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-listeners",
                "retained process.listeners host listener registry arrays cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_raw_listeners(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-raw-listeners",
                "retained process.rawListeners host listener registry arrays cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_stdio(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-stdio",
                "retained process stdio host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_report(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-report",
                "retained process report host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_process_report_snapshot(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "process-report-snapshot",
                "retained process.report.getReport snapshot objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_fs_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-fs-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_fs_promises_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-fs-promises-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_path_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-path-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_url_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-url-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_querystring_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-querystring-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_util_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-util-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_punycode_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-punycode-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_string_decoder_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-string-decoder-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_buffer_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-buffer-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_constants_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-constants-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_timers_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-timers-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_tty_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-tty-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_v8_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-v8-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_vm_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-vm-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_os_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-os-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_child_process_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-child-process-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_crypto_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-crypto-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_dns_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-dns-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_dns_promises_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-dns-promises-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_diagnostics_channel_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-diagnostics-channel-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_domain_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-domain-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_perf_hooks_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-perf-hooks-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_trace_events_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-trace-events-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_async_hooks_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-async-hooks-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_worker_threads_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-worker-threads-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_readline_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-readline-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_inspector_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-inspector-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_cluster_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-cluster-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_repl_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-repl-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_http2_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-http2-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_tls_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-tls-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_net_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-net-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_http_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-http-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_https_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-https-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_zlib_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-zlib-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_dgram_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-dgram-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_stream_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-stream-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_node_module_module(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "node-module",
                "retained builtin module namespace objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_crypto_hash(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "crypto-hash",
                "retained crypto Hash host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_crypto_hmac(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "crypto-hmac",
                "retained crypto Hmac host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_crypto_cipher(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "crypto-cipher",
                "retained crypto Cipher host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_crypto_decipher(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "crypto-decipher",
                "retained crypto Decipher host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_crypto_sign(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "crypto-sign",
                "retained crypto Sign host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_crypto_verify(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "crypto-verify",
                "retained crypto Verify host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_crypto_secret_key(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "crypto-secret-key",
                "retained crypto SecretKey host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_crypto_private_key(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "crypto-private-key",
                "retained crypto PrivateKey host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_crypto_public_key(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "crypto-public-key",
                "retained crypto PublicKey host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_crypto_ecdh(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "crypto-ecdh",
                "retained crypto ECDH host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_crypto_diffie_hellman(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "crypto-diffie-hellman",
                "retained crypto DiffieHellman host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_crypto_x509_certificate(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "crypto-x509-certificate",
                "retained crypto X509Certificate host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_crypto_certificate_constructor(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "crypto-certificate-constructor",
                "retained crypto Certificate constructor objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_crypto_keyobject_constructor(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "crypto-keyobject-constructor",
                "retained crypto KeyObject constructor objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_crypto_hash_constructor(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "crypto-hash-constructor",
                "retained crypto Hash constructor objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_crypto_hmac_constructor(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "crypto-hmac-constructor",
                "retained crypto Hmac constructor objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_http_server(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "http-server",
                "retained HTTP server host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_tls_server(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "tls-server",
                "retained TLS server host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_http2_server(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "http2-server",
                "retained HTTP/2 server host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_http2_client_session(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "http2-client-session",
                "retained HTTP/2 client session host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_zlib_stream(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "zlib-stream",
                "retained zlib stream host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_fs_write_stream(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "fs-write-stream",
                "retained fs.WriteStream host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_stream_handle(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "stream",
                "retained node:stream host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_stream_web_constructor(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "stream-web-constructor",
                "retained node:stream/web constructor objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_diagnostics_channel(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "diagnostics-channel",
                "retained diagnostics_channel host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_domain_handle(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "domain",
                "retained domain host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_perf_event_loop_delay_monitor(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "perf-event-loop-delay-monitor",
                "retained perf_hooks event loop delay monitor host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_perf_histogram(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "perf-histogram",
                "retained perf_hooks histogram host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_readline_interface(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "readline-interface",
                "retained readline Interface host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_trace_events_tracing(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "trace-events-tracing",
                "retained trace_events tracing host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_async_hooks_hook(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "async-hooks-hook",
                "retained async_hooks hook host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_worker_message_channel(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "worker-message-channel",
                "retained worker_threads MessageChannel host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_worker_handle(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "worker-handle",
                "retained worker_threads Worker host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_dgram_socket(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "dgram-socket",
                "retained dgram Socket host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_net_socket(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "net-socket",
                "retained net.Socket host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_net_server(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "net-server",
                "retained net.Server host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_child_process_handle(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "child-process",
                "retained child_process ChildProcess host objects cannot cross a CruftScript boundary in this substrate generation",
            ));
        }
        if cruftscript_import_edge_result_is_async_iterator_handle(rt, result_id) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "function-field",
                "retained async iterator objects with callable own fields cannot cross a CruftScript import boundary as plain structural data in this substrate generation",
            ));
        }
        if rt.is_callable(&result) {
            return Err(cruftscript_import_edge_host_handle_error(
                module_url,
                call,
                "function-field",
                "retained callable objects cannot cross a CruftScript import boundary as plain structural data in this substrate generation",
            ));
        }
        return cruftscript_validate_import_edge_result(rt, module_url, call, result);
    };

    match status {
        PromiseStatus::Fulfilled => {
            cruftscript_adopt_import_edge_result(rt, module_url, call, value)
        }
        PromiseStatus::Rejected => {
            rt.pending_unhandled.remove(&result_id);
            if call.policy_id == 0 {
                return Ok(rt.boundary_unknown_envelope(call.policy_id, value));
            }
            if call.policy_id == 2 {
                return cruftscript_sanitize_import_edge_failure(rt, module_url, call, value);
            }
            Err(cruftscript_import_edge_promise_rejected_error(
                rt, module_url, call, value,
            ))
        }
        PromiseStatus::Pending => match cruftscript_await_pending_import_promise(rt, result_id) {
            Ok(settled) => cruftscript_adopt_import_edge_result(rt, module_url, call, settled),
            Err(PendingImportPromiseError::Idle) => {
                if call.policy_id == 0 {
                    return Ok(rt.boundary_unknown_envelope(call.policy_id, result));
                }
                if call.policy_id == 2 {
                    return cruftscript_sanitize_import_edge_failure(rt, module_url, call, result);
                }
                Err(cruftscript_import_edge_pending_promise_error(
                    module_url, call,
                ))
            }
            Err(PendingImportPromiseError::Pump(error)) => Err(error),
            Err(PendingImportPromiseError::MaxPumps) => Err(
                cruftscript_import_edge_pending_promise_pump_bound_error(module_url, call),
            ),
        },
    }
}

fn cruftscript_validate_import_edge_result(
    rt: &mut Runtime,
    module_url: &str,
    call: &cruftscript_type_checker::LoweredImportedCall,
    result: Value,
) -> Result<Value, RuntimeError> {
    let Some(expected_type) = call.expected_type.as_ref() else {
        return Ok(result);
    };
    match cruftscript_project_import_edge_result(rt, &result, expected_type, 32) {
        Some(projected) => Ok(projected),
        None => Err(cruftscript_import_edge_result_type_error(
            module_url,
            call,
            expected_type,
            &result,
        )),
    }
}

fn cruftscript_project_import_edge_result(
    rt: &mut Runtime,
    value: &Value,
    expected_type: &cruftscript_type_checker::TypeTerm,
    depth_remaining: usize,
) -> Option<Value> {
    if depth_remaining == 0 {
        return None;
    }
    if cruftscript_type_term_is_unknown(expected_type) {
        if let Value::Object(id) = value {
            if let Some(fd) = cruftscript_project_import_edge_filehandle_fd(rt, *id) {
                return Some(Value::Number(fd));
            }
        }
    }
    match expected_type {
        cruftscript_type_checker::TypeTerm::Object { properties, .. } => {
            let Value::Object(id) = value else {
                return None;
            };
            if !cruftscript_runtime_object_has_plain_projection_shape(rt, *id) {
                return None;
            }
            let projected = rt.alloc_object(Object::new_ordinary());
            for property in properties {
                let Some(value) =
                    cruftscript_runtime_plain_object_property_value(rt, *id, &property.name)
                else {
                    if property.optional {
                        continue;
                    }
                    return None;
                };
                let projected_value = cruftscript_project_import_edge_result(
                    rt,
                    &value,
                    &property.ty,
                    depth_remaining - 1,
                )?;
                rt.object_set(projected, property.name.clone(), projected_value);
            }
            Some(Value::Object(projected))
        }
        cruftscript_type_checker::TypeTerm::Union { members, .. } => {
            for member in members {
                if let Some(projected) =
                    cruftscript_project_import_edge_result(rt, value, member, depth_remaining)
                {
                    return Some(projected);
                }
            }
            None
        }
        cruftscript_type_checker::TypeTerm::TypeRef {
            name, type_args, ..
        } if name == "Array" && type_args.len() == 1 => {
            let Value::Object(id) = value else {
                return None;
            };
            if rt.typed_array_views.contains_key(id) {
                return cruftscript_project_import_edge_typed_array(
                    rt,
                    *id,
                    &type_args[0],
                    depth_remaining - 1,
                );
            }
            if cruftscript_runtime_value_matches_type(rt, value, expected_type) {
                return Some(value.clone());
            }
            None
        }
        _ if cruftscript_runtime_value_matches_type(rt, value, expected_type) => {
            Some(value.clone())
        }
        _ => None,
    }
}

fn cruftscript_type_term_is_unknown(expected_type: &cruftscript_type_checker::TypeTerm) -> bool {
    match expected_type {
        cruftscript_type_checker::TypeTerm::Unknown { .. } => true,
        cruftscript_type_checker::TypeTerm::Named { name, .. } => name == "unknown",
        _ => false,
    }
}

fn cruftscript_project_import_edge_filehandle_fd(rt: &Runtime, id: ObjectRef) -> Option<f64> {
    let fd = match rt.object_get(id, "__cruft_fd") {
        Value::Number(fd) if fd.is_finite() => fd,
        _ => return None,
    };
    if !matches!(rt.object_get(id, "fd"), Value::Number(_)) {
        return None;
    }
    if !rt.is_callable(&rt.object_get(id, "stat")) || !rt.is_callable(&rt.object_get(id, "close")) {
        return None;
    }
    Some(fd)
}

fn cruftscript_project_import_edge_typed_array(
    rt: &mut Runtime,
    id: ObjectRef,
    element_type: &cruftscript_type_checker::TypeTerm,
    depth_remaining: usize,
) -> Option<Value> {
    if depth_remaining == 0 {
        return None;
    }
    let (len, values) = {
        let view = rt.typed_array_views.get(&id)?;
        let kind = &*view.element_kind;
        if kind == "DataView" || kind == "BigInt64Array" || kind == "BigUint64Array" {
            return None;
        }
        let buffer = rt.array_buffers.get(&view.buffer)?;
        if buffer.detached || buffer.shared.is_some() {
            return None;
        }
        let width = view.bytes_per_element.max(1);
        let len = view
            .fixed_length
            .unwrap_or_else(|| buffer.byte_length.saturating_sub(view.byte_offset) / width);
        let byte_len = len.checked_mul(width)?;
        let end = view.byte_offset.checked_add(byte_len)?;
        if end > buffer.data.len() {
            return None;
        }

        let values = (0..len)
            .map(|index| {
                let start = view.byte_offset + index * width;
                rusty_js_runtime::abstract_ops::raw_bytes_to_numeric(
                    kind,
                    &buffer.data[start..start + width],
                )
            })
            .collect::<Vec<_>>();
        (len, values)
    };

    let mut copied = Vec::with_capacity(len);
    for value in values {
        let projected =
            cruftscript_project_import_edge_result(rt, &value, element_type, depth_remaining - 1)?;
        copied.push(projected);
    }
    let array = rt.alloc_object(Object::new_array());
    for (index, value) in copied.into_iter().enumerate() {
        rt.object_set(array, index.to_string(), value);
    }
    rt.object_set(array, "length".to_string(), Value::Number(len as f64));
    Some(Value::Object(array))
}

fn cruftscript_runtime_object_has_plain_projection_shape(rt: &Runtime, id: ObjectRef) -> bool {
    let object = rt.obj(id);
    if !cruftscript_runtime_object_has_plain_prototype(rt, object.proto) {
        return false;
    }
    object.properties.iter().all(|(key, desc)| {
        matches!(key, PropertyKey::String(_))
            && desc.getter.is_none()
            && desc.setter.is_none()
            && desc.enumerable
    })
}

fn cruftscript_runtime_plain_object_property_value(
    rt: &Runtime,
    id: ObjectRef,
    name: &str,
) -> Option<Value> {
    let object = rt.obj(id);
    if let Some(value) = object.shape_get(name) {
        return Some(value.clone());
    }
    let desc = object.get_own_str_borrowed(name)?;
    if desc.getter.is_some() || desc.setter.is_some() || !desc.enumerable {
        return None;
    }
    Some(desc.value.clone())
}

fn cruftscript_runtime_value_matches_type(
    rt: &Runtime,
    value: &Value,
    expected_type: &cruftscript_type_checker::TypeTerm,
) -> bool {
    match expected_type {
        cruftscript_type_checker::TypeTerm::Unknown { .. } => true,
        cruftscript_type_checker::TypeTerm::Named { name, .. } => match name.as_str() {
            "unknown" => true,
            "undefined" | "void" => matches!(value, Value::Undefined),
            "null" => matches!(value, Value::Null),
            "string" => matches!(value, Value::String(_)),
            "number" => matches!(value, Value::Number(_)),
            "boolean" => matches!(value, Value::Boolean(_)),
            "bigint" => matches!(value, Value::BigInt(_)),
            "symbol" => matches!(value, Value::Symbol(_)),
            _ => false,
        },
        cruftscript_type_checker::TypeTerm::Union { members, .. } => members
            .iter()
            .any(|member| cruftscript_runtime_value_matches_type(rt, value, member)),
        cruftscript_type_checker::TypeTerm::Tuple { elements, .. } => {
            let Value::Object(id) = value else {
                return false;
            };
            cruftscript_runtime_tuple_matches_type(rt, *id, elements, 32)
        }
        cruftscript_type_checker::TypeTerm::Object { properties, .. } => {
            let Value::Object(id) = value else {
                return false;
            };
            cruftscript_runtime_object_matches_type(rt, *id, properties, 32)
        }
        cruftscript_type_checker::TypeTerm::TypeRef {
            name, type_args, ..
        } if name == "Array" && type_args.len() == 1 => {
            let Value::Object(id) = value else {
                return false;
            };
            cruftscript_runtime_array_matches_type(rt, *id, &type_args[0], 32)
        }
        cruftscript_type_checker::TypeTerm::TypeRef {
            name, type_args, ..
        } if name == "__CruftStringRecord" && type_args.len() == 1 => {
            let Value::Object(id) = value else {
                return false;
            };
            cruftscript_runtime_string_record_matches_type(rt, *id, &type_args[0], 32)
        }
        cruftscript_type_checker::TypeTerm::TypeRef { .. } => false,
        _ => false,
    }
}

fn cruftscript_runtime_object_matches_type(
    rt: &Runtime,
    id: ObjectRef,
    properties: &[cruftscript_type_checker::ObjectPropertyTerm],
    depth_remaining: usize,
) -> bool {
    if depth_remaining == 0 {
        return false;
    }
    let object = rt.obj(id);
    if !cruftscript_runtime_object_has_plain_prototype(rt, object.proto) {
        return false;
    }
    if object
        .properties
        .keys()
        .any(|key| matches!(key, PropertyKey::Symbol(_)))
    {
        return false;
    }
    let keys = object.string_key_clones().collect::<Vec<_>>();
    if keys
        .iter()
        .any(|key| !properties.iter().any(|property| property.name == *key))
    {
        return false;
    }
    properties.iter().all(|property| {
        let value = {
            let object = rt.obj(id);
            if let Some(value) = object.shape_get(&property.name) {
                value.clone()
            } else {
                let Some(desc) = object.get_own_str_borrowed(&property.name) else {
                    return property.optional;
                };
                if desc.getter.is_some() || desc.setter.is_some() || !desc.enumerable {
                    return false;
                }
                desc.value.clone()
            }
        };
        cruftscript_runtime_value_matches_type_with_depth(
            rt,
            &value,
            &property.ty,
            depth_remaining - 1,
        )
    })
}

fn cruftscript_runtime_array_matches_type(
    rt: &Runtime,
    id: ObjectRef,
    element_type: &cruftscript_type_checker::TypeTerm,
    depth_remaining: usize,
) -> bool {
    if depth_remaining == 0 {
        return false;
    }
    let object = rt.obj(id);
    if !cruftscript_runtime_array_has_plain_prototype(rt, object.proto) {
        return false;
    }
    for (key, desc) in &object.properties {
        match key {
            PropertyKey::String(name) if name == "length" => {}
            PropertyKey::String(name)
                if !object.array_dense && cruftscript_is_canonical_array_index(name) => {}
            PropertyKey::String(_) | PropertyKey::Symbol(_) => return false,
        }
        if desc.getter.is_some() || desc.setter.is_some() {
            return false;
        }
    }
    if object.array_dense {
        return (0..object.array_store_len()).all(|index| {
            let value = object.array_store_get(index);
            cruftscript_runtime_value_matches_type_with_depth(
                rt,
                &value,
                element_type,
                depth_remaining - 1,
            )
        });
    }
    if !matches!(object.internal_kind, InternalKind::Array) {
        return false;
    }
    let Some(len) = cruftscript_array_length_descriptor(object.get_own_str_borrowed("length"))
    else {
        return false;
    };
    (0..len).all(|index| {
        let key = index.to_string();
        let Some(desc) = object.get_own_str_borrowed(&key) else {
            return false;
        };
        desc.getter.is_none()
            && desc.setter.is_none()
            && cruftscript_runtime_value_matches_type_with_depth(
                rt,
                &desc.value,
                element_type,
                depth_remaining - 1,
            )
    })
}

fn cruftscript_runtime_tuple_matches_type(
    rt: &Runtime,
    id: ObjectRef,
    elements: &[cruftscript_type_checker::TypeTerm],
    depth_remaining: usize,
) -> bool {
    if depth_remaining == 0 {
        return false;
    }
    let object = rt.obj(id);
    if !cruftscript_runtime_array_has_plain_prototype(rt, object.proto) {
        return false;
    }
    for (key, desc) in &object.properties {
        match key {
            PropertyKey::String(name) if name == "length" => {}
            PropertyKey::String(name)
                if !object.array_dense && cruftscript_is_canonical_array_index(name) => {}
            PropertyKey::String(_) | PropertyKey::Symbol(_) => return false,
        }
        if desc.getter.is_some() || desc.setter.is_some() {
            return false;
        }
    }
    if object.array_dense {
        if object.array_store_len() != elements.len() {
            return false;
        }
        return elements.iter().enumerate().all(|(index, expected)| {
            let value = object.array_store_get(index);
            cruftscript_runtime_value_matches_type_with_depth(
                rt,
                &value,
                expected,
                depth_remaining - 1,
            )
        });
    }
    if !matches!(object.internal_kind, InternalKind::Array) {
        return false;
    }
    let Some(len) = cruftscript_array_length_descriptor(object.get_own_str_borrowed("length"))
    else {
        return false;
    };
    if len as usize != elements.len() {
        return false;
    }
    elements.iter().enumerate().all(|(index, expected)| {
        let key = index.to_string();
        let Some(desc) = object.get_own_str_borrowed(&key) else {
            return false;
        };
        desc.getter.is_none()
            && desc.setter.is_none()
            && cruftscript_runtime_value_matches_type_with_depth(
                rt,
                &desc.value,
                expected,
                depth_remaining - 1,
            )
    })
}

fn cruftscript_runtime_string_record_matches_type(
    rt: &Runtime,
    id: ObjectRef,
    value_type: &cruftscript_type_checker::TypeTerm,
    depth_remaining: usize,
) -> bool {
    if depth_remaining == 0 {
        return false;
    }
    let object = rt.obj(id);
    if !cruftscript_runtime_object_has_plain_prototype(rt, object.proto) {
        return false;
    }
    for (key, desc) in &object.properties {
        if matches!(key, PropertyKey::Symbol(_)) {
            return false;
        }
        if desc.getter.is_some() || desc.setter.is_some() || !desc.enumerable {
            return false;
        }
    }
    object.string_key_clones().all(|key| {
        let value = {
            let object = rt.obj(id);
            if let Some(value) = object.shape_get(&key) {
                value.clone()
            } else {
                let Some(desc) = object.get_own_str_borrowed(&key) else {
                    return false;
                };
                desc.value.clone()
            }
        };
        cruftscript_runtime_value_matches_type_with_depth(
            rt,
            &value,
            value_type,
            depth_remaining - 1,
        )
    })
}

fn cruftscript_runtime_value_matches_type_with_depth(
    rt: &Runtime,
    value: &Value,
    expected_type: &cruftscript_type_checker::TypeTerm,
    depth_remaining: usize,
) -> bool {
    if depth_remaining == 0 {
        return false;
    }
    match expected_type {
        cruftscript_type_checker::TypeTerm::TypeRef {
            name, type_args, ..
        } if name == "Array" && type_args.len() == 1 => {
            let Value::Object(id) = value else {
                return false;
            };
            cruftscript_runtime_array_matches_type(rt, *id, &type_args[0], depth_remaining)
        }
        cruftscript_type_checker::TypeTerm::TypeRef {
            name, type_args, ..
        } if name == "__CruftStringRecord" && type_args.len() == 1 => {
            let Value::Object(id) = value else {
                return false;
            };
            cruftscript_runtime_string_record_matches_type(rt, *id, &type_args[0], depth_remaining)
        }
        cruftscript_type_checker::TypeTerm::Union { members, .. } => members.iter().any(|member| {
            cruftscript_runtime_value_matches_type_with_depth(rt, value, member, depth_remaining)
        }),
        cruftscript_type_checker::TypeTerm::Tuple { elements, .. } => {
            let Value::Object(id) = value else {
                return false;
            };
            cruftscript_runtime_tuple_matches_type(rt, *id, elements, depth_remaining)
        }
        cruftscript_type_checker::TypeTerm::Object { properties, .. } => {
            let Value::Object(id) = value else {
                return false;
            };
            cruftscript_runtime_object_matches_type(rt, *id, properties, depth_remaining)
        }
        _ => cruftscript_runtime_value_matches_type(rt, value, expected_type),
    }
}

fn cruftscript_is_canonical_array_index(name: &str) -> bool {
    if name.is_empty() || (name.len() > 1 && name.starts_with('0')) {
        return false;
    }
    name.parse::<usize>()
        .map(|index| index.to_string() == name)
        .unwrap_or(false)
}

fn cruftscript_array_length_descriptor(
    desc: Option<&rusty_js_runtime::value::PropertyDescriptor>,
) -> Option<usize> {
    let desc = desc?;
    if desc.getter.is_some() || desc.setter.is_some() {
        return None;
    }
    let Value::Number(len) = desc.value else {
        return None;
    };
    if !len.is_finite() || len.fract() != 0.0 || len < 0.0 || len > usize::MAX as f64 {
        return None;
    }
    Some(len as usize)
}

fn cruftscript_import_edge_result_is_thenable(rt: &Runtime, object_id: ObjectRef) -> bool {
    let then_value = rt.object_get(object_id, "then");
    let Value::Object(then_id) = then_value else {
        return false;
    };
    matches!(
        rt.obj(then_id).internal_kind,
        InternalKind::Function(_) | InternalKind::Closure(_) | InternalKind::BoundFunction(_)
    )
}

fn cruftscript_import_edge_result_is_async_iterator_handle(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    rt.is_callable(&rt.object_get(object_id, "next"))
        && rt.is_callable(&rt.object_get(object_id, "return"))
}

fn cruftscript_import_edge_result_is_fs_watcher(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(rt.object_get(object_id, "__watch_id"), Value::Number(_))
}

fn cruftscript_import_edge_result_is_fs_read_stream(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(rt.object_get(object_id, "path"), Value::String(_))
        && matches!(rt.object_get(object_id, "bytesRead"), Value::Number(_))
        && rt.is_callable(&rt.object_get(object_id, "close"))
        && rt.is_callable(&rt.object_get(object_id, "destroy"))
        && rt.is_callable(&rt.object_get(object_id, "pause"))
        && rt.is_callable(&rt.object_get(object_id, "resume"))
        && rt.is_callable(&rt.object_get(object_id, "pipe"))
}

fn cruftscript_import_edge_result_is_fs_write_stream(rt: &Runtime, object_id: ObjectRef) -> bool {
    if matches!(
        rt.object_get(object_id, "__cruft_net_socket"),
        Value::Boolean(true)
    ) {
        return false;
    }
    matches!(rt.object_get(object_id, "path"), Value::String(_))
        && matches!(rt.object_get(object_id, "bytesWritten"), Value::Number(_))
        && rt.is_callable(&rt.object_get(object_id, "write"))
        && rt.is_callable(&rt.object_get(object_id, "end"))
        && rt.is_callable(&rt.object_get(object_id, "destroy"))
}

fn cruftscript_import_edge_result_is_blob(rt: &Runtime, object_id: ObjectRef) -> bool {
    (matches!(
        rt.object_get(object_id, "__blob_file_path"),
        Value::String(_)
    ) || matches!(rt.object_get(object_id, "__blob_bytes"), Value::String(_))
        || matches!(rt.object_get(object_id, "__blob_chunks"), Value::Object(_)))
        && matches!(rt.object_get(object_id, "__blob_type"), Value::String(_))
}

fn cruftscript_import_edge_result_is_process(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(rt.object_get(object_id, "pid"), Value::Number(_))
        && matches!(rt.object_get(object_id, "ppid"), Value::Number(_))
        && matches!(rt.object_get(object_id, "env"), Value::Object(_))
        && matches!(rt.object_get(object_id, "argv"), Value::Object(_))
        && matches!(rt.object_get(object_id, "versions"), Value::Object(_))
        && rt.is_callable(&rt.object_get(object_id, "nextTick"))
        && rt.is_callable(&rt.object_get(object_id, "cwd"))
        && rt.is_callable(&rt.object_get(object_id, "kill"))
        && rt.is_callable(&rt.object_get(object_id, "on"))
}

fn cruftscript_import_edge_result_is_process_argv(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(
        rt.object_get(object_id, "__process_argv__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_process_exec_argv(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(
        rt.object_get(object_id, "__process_exec_argv__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_process_hrtime(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(
        rt.object_get(object_id, "__process_hrtime__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_process_binding_constants(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(
        rt.object_get(object_id, "__process_binding_constants__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_process_binding_fs(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(
        rt.object_get(object_id, "__process_binding_fs__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_process_binding_unknown(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(
        rt.object_get(object_id, "__process_binding_unknown__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_process_env(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(
        rt.object_get(object_id, "__process_env__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_process_versions(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(
        rt.object_get(object_id, "__process_versions__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_process_release(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(
        rt.object_get(object_id, "__process_release__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_process_config(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(
        rt.object_get(object_id, "__process_config__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_process_events(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(
        rt.object_get(object_id, "__process_events__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_node_events_module(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    rt.is_callable(&rt.object_get(object_id, "EventEmitter"))
        && rt.is_callable(&rt.object_get(object_id, "EventEmitterAsyncResource"))
        && rt.is_callable(&rt.object_get(object_id, "getEventListeners"))
        && rt.is_callable(&rt.object_get(object_id, "getMaxListeners"))
        && rt.is_callable(&rt.object_get(object_id, "setMaxListeners"))
        && matches!(
            rt.object_get(object_id, "captureRejections"),
            Value::Boolean(_)
        )
}

fn cruftscript_import_edge_result_is_event_emitter(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(
        rt.object_get(object_id, "__cruft_event_emitter__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_process_finalization(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(
        rt.object_get(object_id, "__process_finalization__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_process_module_load_list(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(
        rt.object_get(object_id, "__process_module_load_list__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_process_preload_modules(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(
        rt.object_get(object_id, "__process_preload_modules__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_process_features(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(
        rt.object_get(object_id, "__process_features__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_process_allowed_node_environment_flags(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(
        rt.object_get(object_id, "__process_allowed_node_environment_flags__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_process_cpu_usage(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(
        rt.object_get(object_id, "__process_cpu_usage__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_process_memory_usage(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(
        rt.object_get(object_id, "__process_memory_usage__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_process_resource_usage(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(
        rt.object_get(object_id, "__process_resource_usage__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_process_thread_cpu_usage(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(
        rt.object_get(object_id, "__process_thread_cpu_usage__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_process_groups(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(
        rt.object_get(object_id, "__process_groups__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_process_active_resources(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(
        rt.object_get(object_id, "__process_active_resources__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_process_event_names(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(
        rt.object_get(object_id, "__process_event_names__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_process_listeners(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(
        rt.object_get(object_id, "__process_listeners__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_process_raw_listeners(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(
        rt.object_get(object_id, "__process_raw_listeners__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_process_stdio(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(rt.object_get(object_id, "fd"), Value::Number(_))
        && matches!(
            rt.object_get(object_id, "isTTY"),
            Value::Boolean(_) | Value::Undefined
        )
        && matches!(
            rt.object_get(object_id, "columns"),
            Value::Number(_) | Value::Undefined
        )
        && matches!(
            rt.object_get(object_id, "rows"),
            Value::Number(_) | Value::Undefined
        )
        && rt.is_callable(&rt.object_get(object_id, "write"))
        && rt.is_callable(&rt.object_get(object_id, "on"))
}

fn cruftscript_import_edge_result_is_process_report(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(
        rt.object_get(object_id, "reportOnFatalError"),
        Value::Boolean(_)
    ) && matches!(
        rt.object_get(object_id, "reportOnSignal"),
        Value::Boolean(_)
    ) && matches!(
        rt.object_get(object_id, "reportOnUncaughtException"),
        Value::Boolean(_)
    ) && matches!(rt.object_get(object_id, "directory"), Value::String(_))
        && rt.is_callable(&rt.object_get(object_id, "writeReport"))
        && rt.is_callable(&rt.object_get(object_id, "getReport"))
}

fn cruftscript_import_edge_result_is_process_report_snapshot(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(
        rt.object_get(object_id, "__process_report_snapshot__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_node_fs_module(rt: &Runtime, object_id: ObjectRef) -> bool {
    rt.is_callable(&rt.object_get(object_id, "readFileSync"))
        && rt.is_callable(&rt.object_get(object_id, "writeFileSync"))
        && rt.is_callable(&rt.object_get(object_id, "existsSync"))
        && rt.is_callable(&rt.object_get(object_id, "createReadStream"))
        && matches!(rt.object_get(object_id, "promises"), Value::Object(_))
        && matches!(rt.object_get(object_id, "constants"), Value::Object(_))
}

fn cruftscript_import_edge_result_is_node_fs_promises_module(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    rt.is_callable(&rt.object_get(object_id, "readFile"))
        && rt.is_callable(&rt.object_get(object_id, "writeFile"))
        && rt.is_callable(&rt.object_get(object_id, "readdir"))
        && rt.is_callable(&rt.object_get(object_id, "stat"))
        && rt.is_callable(&rt.object_get(object_id, "cp"))
        && matches!(rt.object_get(object_id, "constants"), Value::Object(_))
        && matches!(rt.object_get(object_id, "readFileSync"), Value::Undefined)
        && matches!(rt.object_get(object_id, "promises"), Value::Undefined)
}

fn cruftscript_import_edge_result_is_node_path_module(rt: &Runtime, object_id: ObjectRef) -> bool {
    rt.is_callable(&rt.object_get(object_id, "join"))
        && rt.is_callable(&rt.object_get(object_id, "resolve"))
        && rt.is_callable(&rt.object_get(object_id, "dirname"))
        && rt.is_callable(&rt.object_get(object_id, "basename"))
        && rt.is_callable(&rt.object_get(object_id, "extname"))
        && rt.is_callable(&rt.object_get(object_id, "parse"))
        && rt.is_callable(&rt.object_get(object_id, "format"))
        && matches!(rt.object_get(object_id, "sep"), Value::String(_))
        && matches!(rt.object_get(object_id, "delimiter"), Value::String(_))
}

fn cruftscript_import_edge_result_is_node_url_module(rt: &Runtime, object_id: ObjectRef) -> bool {
    rt.is_callable(&rt.object_get(object_id, "fileURLToPath"))
        && rt.is_callable(&rt.object_get(object_id, "pathToFileURL"))
        && rt.is_callable(&rt.object_get(object_id, "parse"))
        && rt.is_callable(&rt.object_get(object_id, "format"))
        && rt.is_callable(&rt.object_get(object_id, "resolve"))
        && rt.is_callable(&rt.object_get(object_id, "domainToASCII"))
        && rt.is_callable(&rt.object_get(object_id, "domainToUnicode"))
        && rt.is_callable(&rt.object_get(object_id, "urlToHttpOptions"))
        && matches!(rt.object_get(object_id, "URL"), Value::Object(_))
        && matches!(
            rt.object_get(object_id, "URLSearchParams"),
            Value::Object(_)
        )
}

fn cruftscript_import_edge_result_is_node_querystring_module(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    rt.is_callable(&rt.object_get(object_id, "parse"))
        && rt.is_callable(&rt.object_get(object_id, "stringify"))
        && rt.is_callable(&rt.object_get(object_id, "escape"))
        && rt.is_callable(&rt.object_get(object_id, "unescape"))
        && rt.is_callable(&rt.object_get(object_id, "decode"))
        && rt.is_callable(&rt.object_get(object_id, "encode"))
}

fn cruftscript_import_edge_result_is_node_util_module(rt: &Runtime, object_id: ObjectRef) -> bool {
    rt.is_callable(&rt.object_get(object_id, "inspect"))
        && rt.is_callable(&rt.object_get(object_id, "format"))
        && rt.is_callable(&rt.object_get(object_id, "styleText"))
        && rt.is_callable(&rt.object_get(object_id, "promisify"))
        && rt.is_callable(&rt.object_get(object_id, "callbackify"))
        && rt.is_callable(&rt.object_get(object_id, "stripVTControlCharacters"))
        && matches!(rt.object_get(object_id, "types"), Value::Object(_))
}

fn cruftscript_import_edge_result_is_node_punycode_module(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    rt.is_callable(&rt.object_get(object_id, "encode"))
        && rt.is_callable(&rt.object_get(object_id, "decode"))
        && rt.is_callable(&rt.object_get(object_id, "toASCII"))
        && rt.is_callable(&rt.object_get(object_id, "toUnicode"))
        && matches!(rt.object_get(object_id, "version"), Value::String(_))
        && matches!(rt.object_get(object_id, "ucs2"), Value::Object(_))
}

fn cruftscript_import_edge_result_is_node_string_decoder_module(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    rt.is_callable(&rt.object_get(object_id, "StringDecoder"))
}

fn cruftscript_import_edge_result_is_node_buffer_module(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    rt.is_callable(&rt.object_get(object_id, "Buffer"))
        && matches!(rt.object_get(object_id, "Blob"), Value::Object(_))
        && matches!(rt.object_get(object_id, "constants"), Value::Object(_))
        && matches!(
            rt.object_get(object_id, "INSPECT_MAX_BYTES"),
            Value::Number(_) | Value::Undefined
        )
}

fn cruftscript_import_edge_result_is_node_constants_module(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(rt.object_get(object_id, "COPYFILE_EXCL"), Value::Number(_))
        && matches!(rt.object_get(object_id, "O_RDONLY"), Value::Number(_))
        && matches!(rt.object_get(object_id, "ENOENT"), Value::Number(_))
        && matches!(
            rt.object_get(object_id, "RSA_PKCS1_PADDING"),
            Value::Number(_)
        )
        && matches!(rt.object_get(object_id, "SIGTERM"), Value::Number(_))
}

fn cruftscript_import_edge_result_is_node_timers_module(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    rt.is_callable(&rt.object_get(object_id, "setTimeout"))
        && rt.is_callable(&rt.object_get(object_id, "clearTimeout"))
        && rt.is_callable(&rt.object_get(object_id, "setImmediate"))
        && rt.is_callable(&rt.object_get(object_id, "clearImmediate"))
        && rt.is_callable(&rt.object_get(object_id, "setInterval"))
}

fn cruftscript_import_edge_result_is_node_tty_module(rt: &Runtime, object_id: ObjectRef) -> bool {
    rt.is_callable(&rt.object_get(object_id, "isatty"))
        && rt.is_callable(&rt.object_get(object_id, "ReadStream"))
        && rt.is_callable(&rt.object_get(object_id, "WriteStream"))
}

fn cruftscript_import_edge_result_is_node_v8_module(rt: &Runtime, object_id: ObjectRef) -> bool {
    rt.is_callable(&rt.object_get(object_id, "getHeapStatistics"))
        && rt.is_callable(&rt.object_get(object_id, "cachedDataVersionTag"))
        && rt.is_callable(&rt.object_get(object_id, "Serializer"))
}

fn cruftscript_import_edge_result_is_node_vm_module(rt: &Runtime, object_id: ObjectRef) -> bool {
    rt.is_callable(&rt.object_get(object_id, "createContext"))
        && rt.is_callable(&rt.object_get(object_id, "isContext"))
        && rt.is_callable(&rt.object_get(object_id, "runInContext"))
        && rt.is_callable(&rt.object_get(object_id, "runInNewContext"))
        && rt.is_callable(&rt.object_get(object_id, "runInThisContext"))
        && rt.is_callable(&rt.object_get(object_id, "Script"))
}

fn cruftscript_import_edge_result_is_node_os_module(rt: &Runtime, object_id: ObjectRef) -> bool {
    rt.is_callable(&rt.object_get(object_id, "platform"))
        && rt.is_callable(&rt.object_get(object_id, "arch"))
        && rt.is_callable(&rt.object_get(object_id, "type"))
        && rt.is_callable(&rt.object_get(object_id, "release"))
        && rt.is_callable(&rt.object_get(object_id, "hostname"))
        && rt.is_callable(&rt.object_get(object_id, "homedir"))
        && rt.is_callable(&rt.object_get(object_id, "tmpdir"))
        && rt.is_callable(&rt.object_get(object_id, "cpus"))
        && rt.is_callable(&rt.object_get(object_id, "networkInterfaces"))
        && rt.is_callable(&rt.object_get(object_id, "userInfo"))
        && matches!(rt.object_get(object_id, "EOL"), Value::String(_))
        && matches!(rt.object_get(object_id, "devNull"), Value::String(_))
}

fn cruftscript_import_edge_result_is_node_child_process_module(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    rt.is_callable(&rt.object_get(object_id, "spawn"))
        && rt.is_callable(&rt.object_get(object_id, "spawnSync"))
        && rt.is_callable(&rt.object_get(object_id, "exec"))
        && rt.is_callable(&rt.object_get(object_id, "execSync"))
        && rt.is_callable(&rt.object_get(object_id, "execFile"))
        && rt.is_callable(&rt.object_get(object_id, "execFileSync"))
        && rt.is_callable(&rt.object_get(object_id, "fork"))
        && matches!(rt.object_get(object_id, "ChildProcess"), Value::Object(_))
}

fn cruftscript_import_edge_result_is_node_crypto_module(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    rt.is_callable(&rt.object_get(object_id, "createHash"))
        && rt.is_callable(&rt.object_get(object_id, "createHmac"))
        && rt.is_callable(&rt.object_get(object_id, "randomBytes"))
        && rt.is_callable(&rt.object_get(object_id, "randomUUID"))
        && rt.is_callable(&rt.object_get(object_id, "getRandomValues"))
        && rt.is_callable(&rt.object_get(object_id, "createCipheriv"))
        && rt.is_callable(&rt.object_get(object_id, "createDecipheriv"))
        && rt.is_callable(&rt.object_get(object_id, "pbkdf2Sync"))
        && rt.is_callable(&rt.object_get(object_id, "scryptSync"))
        && rt.is_callable(&rt.object_get(object_id, "sign"))
        && rt.is_callable(&rt.object_get(object_id, "verify"))
        && matches!(rt.object_get(object_id, "webcrypto"), Value::Object(_))
}

fn cruftscript_import_edge_result_is_node_dns_module(rt: &Runtime, object_id: ObjectRef) -> bool {
    rt.is_callable(&rt.object_get(object_id, "lookup"))
        && rt.is_callable(&rt.object_get(object_id, "resolve4"))
        && rt.is_callable(&rt.object_get(object_id, "resolve6"))
        && rt.is_callable(&rt.object_get(object_id, "resolve"))
        && rt.is_callable(&rt.object_get(object_id, "reverse"))
        && rt.is_callable(&rt.object_get(object_id, "getServers"))
        && rt.is_callable(&rt.object_get(object_id, "setServers"))
        && rt.is_callable(&rt.object_get(object_id, "getDefaultResultOrder"))
        && matches!(rt.object_get(object_id, "promises"), Value::Object(_))
        && matches!(rt.object_get(object_id, "Resolver"), Value::Object(_))
}

fn cruftscript_import_edge_result_is_node_dns_promises_module(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    rt.is_callable(&rt.object_get(object_id, "lookup"))
        && rt.is_callable(&rt.object_get(object_id, "resolve4"))
        && rt.is_callable(&rt.object_get(object_id, "resolve6"))
        && rt.is_callable(&rt.object_get(object_id, "resolve"))
        && rt.is_callable(&rt.object_get(object_id, "reverse"))
        && matches!(rt.object_get(object_id, "Resolver"), Value::Object(_))
}

fn cruftscript_import_edge_result_is_node_diagnostics_channel_module(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    rt.is_callable(&rt.object_get(object_id, "channel"))
        && rt.is_callable(&rt.object_get(object_id, "tracingChannel"))
        && rt.is_callable(&rt.object_get(object_id, "subscribe"))
        && rt.is_callable(&rt.object_get(object_id, "unsubscribe"))
        && rt.is_callable(&rt.object_get(object_id, "hasSubscribers"))
        && rt.is_callable(&rt.object_get(object_id, "Channel"))
        && rt.is_callable(&rt.object_get(object_id, "BoundedChannel"))
}

fn cruftscript_import_edge_result_is_node_domain_module(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    rt.is_callable(&rt.object_get(object_id, "create"))
        && rt.is_callable(&rt.object_get(object_id, "createDomain"))
        && rt.is_callable(&rt.object_get(object_id, "Domain"))
        && matches!(rt.object_get(object_id, "active"), Value::Null)
        && matches!(rt.object_get(object_id, "_stack"), Value::Object(_))
}

fn cruftscript_import_edge_result_is_node_perf_hooks_module(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(rt.object_get(object_id, "performance"), Value::Object(_))
        && rt.is_callable(&rt.object_get(object_id, "PerformanceObserver"))
        && rt.is_callable(&rt.object_get(object_id, "monitorEventLoopDelay"))
        && rt.is_callable(&rt.object_get(object_id, "createHistogram"))
        && rt.is_callable(&rt.object_get(object_id, "Performance"))
        && rt.is_callable(&rt.object_get(object_id, "eventLoopUtilization"))
}

fn cruftscript_import_edge_result_is_node_trace_events_module(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    rt.is_callable(&rt.object_get(object_id, "createTracing"))
        && rt.is_callable(&rt.object_get(object_id, "getEnabledCategories"))
}

fn cruftscript_import_edge_result_is_node_async_hooks_module(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    rt.is_callable(&rt.object_get(object_id, "executionAsyncId"))
        && rt.is_callable(&rt.object_get(object_id, "triggerAsyncId"))
        && rt.is_callable(&rt.object_get(object_id, "executionAsyncResource"))
        && rt.is_callable(&rt.object_get(object_id, "createHook"))
        && rt.is_callable(&rt.object_get(object_id, "AsyncResource"))
        && rt.is_callable(&rt.object_get(object_id, "AsyncLocalStorage"))
        && matches!(
            rt.object_get(object_id, "asyncWrapProviders"),
            Value::Object(_)
        )
}

fn cruftscript_import_edge_result_is_node_worker_threads_module(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(
        rt.object_get(object_id, "isMainThread"),
        Value::Boolean(true)
    ) && matches!(
        rt.object_get(object_id, "isInternalThread"),
        Value::Boolean(false)
    ) && matches!(rt.object_get(object_id, "threadId"), Value::Number(_))
        && matches!(rt.object_get(object_id, "parentPort"), Value::Null)
        && matches!(rt.object_get(object_id, "workerData"), Value::Null)
        && matches!(rt.object_get(object_id, "Worker"), Value::Object(_))
        && matches!(rt.object_get(object_id, "MessageChannel"), Value::Object(_))
        && matches!(rt.object_get(object_id, "MessagePort"), Value::Object(_))
        && matches!(
            rt.object_get(object_id, "BroadcastChannel"),
            Value::Object(_)
        )
        && matches!(rt.object_get(object_id, "resourceLimits"), Value::Object(_))
        && matches!(rt.object_get(object_id, "locks"), Value::Object(_))
        && rt.is_callable(&rt.object_get(object_id, "receiveMessageOnPort"))
        && rt.is_callable(&rt.object_get(object_id, "postMessageToThread"))
}

fn cruftscript_import_edge_result_is_node_readline_module(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    rt.is_callable(&rt.object_get(object_id, "createInterface"))
        && rt.is_callable(&rt.object_get(object_id, "Interface"))
        && rt.is_callable(&rt.object_get(object_id, "cursorTo"))
        && rt.is_callable(&rt.object_get(object_id, "moveCursor"))
        && rt.is_callable(&rt.object_get(object_id, "clearLine"))
        && rt.is_callable(&rt.object_get(object_id, "clearScreenDown"))
        && rt.is_callable(&rt.object_get(object_id, "emitKeypressEvents"))
        && matches!(rt.object_get(object_id, "promises"), Value::Object(_))
}

fn cruftscript_import_edge_result_is_node_inspector_module(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    rt.is_callable(&rt.object_get(object_id, "open"))
        && rt.is_callable(&rt.object_get(object_id, "close"))
        && rt.is_callable(&rt.object_get(object_id, "url"))
        && rt.is_callable(&rt.object_get(object_id, "waitForDebugger"))
        && rt.is_callable(&rt.object_get(object_id, "Session"))
        && matches!(rt.object_get(object_id, "Network"), Value::Object(_))
        && matches!(
            rt.object_get(object_id, "NetworkResources"),
            Value::Object(_)
        )
        && matches!(rt.object_get(object_id, "DOMStorage"), Value::Object(_))
        && matches!(rt.object_get(object_id, "console"), Value::Object(_))
}

fn cruftscript_import_edge_result_is_node_cluster_module(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(rt.object_get(object_id, "isPrimary"), Value::Boolean(true))
        && matches!(rt.object_get(object_id, "isMaster"), Value::Boolean(true))
        && matches!(rt.object_get(object_id, "isWorker"), Value::Boolean(false))
        && matches!(rt.object_get(object_id, "SCHED_NONE"), Value::Number(_))
        && matches!(rt.object_get(object_id, "SCHED_RR"), Value::Number(_))
        && matches!(
            rt.object_get(object_id, "schedulingPolicy"),
            Value::Number(_)
        )
        && matches!(rt.object_get(object_id, "workers"), Value::Object(_))
        && rt.is_callable(&rt.object_get(object_id, "Worker"))
        && rt.is_callable(&rt.object_get(object_id, "disconnect"))
        && rt.is_callable(&rt.object_get(object_id, "fork"))
        && rt.is_callable(&rt.object_get(object_id, "setupMaster"))
        && rt.is_callable(&rt.object_get(object_id, "setupPrimary"))
}

fn cruftscript_import_edge_result_is_node_repl_module(rt: &Runtime, object_id: ObjectRef) -> bool {
    rt.is_callable(&rt.object_get(object_id, "REPLServer"))
        && rt.is_callable(&rt.object_get(object_id, "Recoverable"))
        && matches!(
            rt.object_get(object_id, "REPL_MODE_SLOPPY"),
            Value::Symbol(_)
        )
        && matches!(
            rt.object_get(object_id, "REPL_MODE_STRICT"),
            Value::Symbol(_)
        )
        && rt.is_callable(&rt.object_get(object_id, "start"))
        && rt.is_callable(&rt.object_get(object_id, "writer"))
        && rt.is_callable(&rt.object_get(object_id, "isValidSyntax"))
}

fn cruftscript_import_edge_result_is_node_http2_module(rt: &Runtime, object_id: ObjectRef) -> bool {
    rt.is_callable(&rt.object_get(object_id, "createSecureServer"))
        && rt.is_callable(&rt.object_get(object_id, "createServer"))
        && rt.is_callable(&rt.object_get(object_id, "connect"))
        && rt.is_callable(&rt.object_get(object_id, "getDefaultSettings"))
        && rt.is_callable(&rt.object_get(object_id, "getPackedSettings"))
        && rt.is_callable(&rt.object_get(object_id, "getUnpackedSettings"))
        && rt.is_callable(&rt.object_get(object_id, "performServerHandshake"))
        && rt.is_callable(&rt.object_get(object_id, "Http2ServerRequest"))
        && rt.is_callable(&rt.object_get(object_id, "Http2ServerResponse"))
        && matches!(rt.object_get(object_id, "constants"), Value::Object(_))
        && matches!(
            rt.object_get(object_id, "sensitiveHeaders"),
            Value::Symbol(_)
        )
}

fn cruftscript_import_edge_result_is_node_tls_module(rt: &Runtime, object_id: ObjectRef) -> bool {
    rt.is_callable(&rt.object_get(object_id, "connect"))
        && rt.is_callable(&rt.object_get(object_id, "createSecureContext"))
        && rt.is_callable(&rt.object_get(object_id, "createServer"))
        && rt.is_callable(&rt.object_get(object_id, "TLSSocket"))
        && rt.is_callable(&rt.object_get(object_id, "Server"))
        && rt.is_callable(&rt.object_get(object_id, "SecureContext"))
        && matches!(
            rt.object_get(object_id, "CLIENT_RENEG_LIMIT"),
            Value::Number(_)
        )
        && matches!(
            rt.object_get(object_id, "DEFAULT_MAX_VERSION"),
            Value::String(_)
        )
        && matches!(
            rt.object_get(object_id, "DEFAULT_MIN_VERSION"),
            Value::String(_)
        )
        && rt.is_callable(&rt.object_get(object_id, "checkServerIdentity"))
        && rt.is_callable(&rt.object_get(object_id, "getCiphers"))
}

fn cruftscript_import_edge_result_is_node_net_module(rt: &Runtime, object_id: ObjectRef) -> bool {
    rt.is_callable(&rt.object_get(object_id, "isIP"))
        && rt.is_callable(&rt.object_get(object_id, "isIPv4"))
        && rt.is_callable(&rt.object_get(object_id, "isIPv6"))
        && rt.is_callable(&rt.object_get(object_id, "createServer"))
        && rt.is_callable(&rt.object_get(object_id, "connect"))
        && rt.is_callable(&rt.object_get(object_id, "createConnection"))
        && rt.is_callable(&rt.object_get(object_id, "Server"))
        && rt.is_callable(&rt.object_get(object_id, "Socket"))
        && rt.is_callable(&rt.object_get(object_id, "BlockList"))
        && rt.is_callable(&rt.object_get(object_id, "SocketAddress"))
        && rt.is_callable(&rt.object_get(object_id, "Stream"))
        && rt.is_callable(&rt.object_get(object_id, "getDefaultAutoSelectFamily"))
        && rt.is_callable(&rt.object_get(object_id, "getDefaultAutoSelectFamilyAttemptTimeout"))
}

fn cruftscript_import_edge_result_is_node_http_module(rt: &Runtime, object_id: ObjectRef) -> bool {
    rt.is_callable(&rt.object_get(object_id, "request"))
        && rt.is_callable(&rt.object_get(object_id, "get"))
        && rt.is_callable(&rt.object_get(object_id, "createServer"))
        && rt.is_callable(&rt.object_get(object_id, "Agent"))
        && rt.is_callable(&rt.object_get(object_id, "Server"))
        && rt.is_callable(&rt.object_get(object_id, "ServerResponse"))
        && rt.is_callable(&rt.object_get(object_id, "IncomingMessage"))
        && rt.is_callable(&rt.object_get(object_id, "ClientRequest"))
        && matches!(rt.object_get(object_id, "STATUS_CODES"), Value::Object(_))
        && matches!(rt.object_get(object_id, "METHODS"), Value::Object(_))
        && matches!(rt.object_get(object_id, "maxHeaderSize"), Value::Number(_))
}

fn cruftscript_import_edge_result_is_node_https_module(rt: &Runtime, object_id: ObjectRef) -> bool {
    rt.is_callable(&rt.object_get(object_id, "request"))
        && rt.is_callable(&rt.object_get(object_id, "get"))
        && rt.is_callable(&rt.object_get(object_id, "createServer"))
        && rt.is_callable(&rt.object_get(object_id, "Agent"))
        && rt.is_callable(&rt.object_get(object_id, "Server"))
        && matches!(rt.object_get(object_id, "globalAgent"), Value::Object(_))
}

fn cruftscript_import_edge_result_is_node_zlib_module(rt: &Runtime, object_id: ObjectRef) -> bool {
    rt.is_callable(&rt.object_get(object_id, "gzipSync"))
        && rt.is_callable(&rt.object_get(object_id, "gunzipSync"))
        && rt.is_callable(&rt.object_get(object_id, "deflateSync"))
        && rt.is_callable(&rt.object_get(object_id, "inflateSync"))
        && rt.is_callable(&rt.object_get(object_id, "createGzip"))
        && rt.is_callable(&rt.object_get(object_id, "createGunzip"))
        && rt.is_callable(&rt.object_get(object_id, "crc32"))
        && matches!(rt.object_get(object_id, "constants"), Value::Object(_))
}

fn cruftscript_import_edge_result_is_node_dgram_module(rt: &Runtime, object_id: ObjectRef) -> bool {
    rt.is_callable(&rt.object_get(object_id, "createSocket"))
        && rt.is_callable(&rt.object_get(object_id, "Socket"))
}

fn cruftscript_import_edge_result_is_node_module_module(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    rt.is_callable(&rt.object_get(object_id, "createRequire"))
        && rt.is_callable(&rt.object_get(object_id, "_resolveFilename"))
        && rt.is_callable(&rt.object_get(object_id, "_load"))
        && matches!(rt.object_get(object_id, "builtinModules"), Value::Object(_))
        && matches!(rt.object_get(object_id, "Module"), Value::Object(_))
        && (matches!(rt.object_get(object_id, "prototype"), Value::Object(_))
            || rt.is_callable(&rt.object_get(object_id, "wrap")))
}

fn cruftscript_import_edge_result_is_crypto_hash(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(rt.object_get(object_id, "__cruft_crypto_kind"), Value::String(s) if s.as_str() == "hash")
        && rt.is_callable(&rt.object_get(object_id, "update"))
        && rt.is_callable(&rt.object_get(object_id, "digest"))
}

fn cruftscript_import_edge_result_is_crypto_hmac(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(rt.object_get(object_id, "__cruft_crypto_kind"), Value::String(s) if s.as_str() == "hmac")
        && rt.is_callable(&rt.object_get(object_id, "update"))
        && rt.is_callable(&rt.object_get(object_id, "digest"))
}

fn cruftscript_import_edge_result_is_crypto_cipher(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(rt.object_get(object_id, "__cruft_crypto_kind"), Value::String(s) if s.as_str() == "cipher")
        && rt.is_callable(&rt.object_get(object_id, "update"))
        && rt.is_callable(&rt.object_get(object_id, "final"))
}

fn cruftscript_import_edge_result_is_crypto_decipher(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(rt.object_get(object_id, "__cruft_crypto_kind"), Value::String(s) if s.as_str() == "decipher")
        && rt.is_callable(&rt.object_get(object_id, "update"))
        && rt.is_callable(&rt.object_get(object_id, "final"))
}

fn cruftscript_import_edge_result_is_crypto_sign(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(rt.object_get(object_id, "__sign_algo"), Value::String(_))
        && matches!(rt.object_get(object_id, "__sign_data"), Value::String(_))
        && rt.is_callable(&rt.object_get(object_id, "update"))
        && rt.is_callable(&rt.object_get(object_id, "sign"))
}

fn cruftscript_import_edge_result_is_crypto_verify(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(rt.object_get(object_id, "__sign_algo"), Value::String(_))
        && matches!(rt.object_get(object_id, "__sign_data"), Value::String(_))
        && rt.is_callable(&rt.object_get(object_id, "update"))
        && rt.is_callable(&rt.object_get(object_id, "verify"))
}

fn cruftscript_import_edge_result_is_crypto_secret_key(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(rt.object_get(object_id, "type"), Value::String(ref s) if s.as_str() == "secret")
        && matches!(rt.object_get(object_id, "__keybytes"), Value::String(_))
        && rt.is_callable(&rt.object_get(object_id, "export"))
}

fn cruftscript_import_edge_result_is_crypto_private_key(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(rt.object_get(object_id, "type"), Value::String(ref s) if s.as_str() == "private")
        && matches!(rt.object_get(object_id, "__pem"), Value::String(_))
        && matches!(
            rt.object_get(object_id, "asymmetricKeyType"),
            Value::String(_)
        )
        && rt.is_callable(&rt.object_get(object_id, "export"))
}

fn cruftscript_import_edge_result_is_crypto_public_key(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(rt.object_get(object_id, "type"), Value::String(ref s) if s.as_str() == "public")
        && matches!(rt.object_get(object_id, "__pem"), Value::String(_))
        && matches!(
            rt.object_get(object_id, "asymmetricKeyType"),
            Value::String(_)
        )
        && rt.is_callable(&rt.object_get(object_id, "export"))
}

fn cruftscript_import_edge_result_is_crypto_ecdh(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(rt.object_get(object_id, "__ecdh_curve"), Value::String(_))
        && rt.is_callable(&rt.object_get(object_id, "generateKeys"))
        && rt.is_callable(&rt.object_get(object_id, "getPublicKey"))
        && rt.is_callable(&rt.object_get(object_id, "getPrivateKey"))
        && rt.is_callable(&rt.object_get(object_id, "computeSecret"))
}

fn cruftscript_import_edge_result_is_crypto_diffie_hellman(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(rt.object_get(object_id, "__dh_p"), Value::String(_))
        && matches!(rt.object_get(object_id, "__dh_g"), Value::String(_))
        && rt.is_callable(&rt.object_get(object_id, "generateKeys"))
        && rt.is_callable(&rt.object_get(object_id, "getPublicKey"))
        && rt.is_callable(&rt.object_get(object_id, "getPrivateKey"))
        && rt.is_callable(&rt.object_get(object_id, "computeSecret"))
        && rt.is_callable(&rt.object_get(object_id, "getPrime"))
        && rt.is_callable(&rt.object_get(object_id, "getGenerator"))
}

fn cruftscript_import_edge_result_is_crypto_x509_certificate(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(rt.object_get(object_id, "subject"), Value::String(_))
        && matches!(rt.object_get(object_id, "issuer"), Value::String(_))
        && matches!(rt.object_get(object_id, "serialNumber"), Value::String(_))
        && matches!(rt.object_get(object_id, "fingerprint"), Value::String(_))
        && matches!(rt.object_get(object_id, "fingerprint256"), Value::String(_))
        && matches!(rt.object_get(object_id, "fingerprint512"), Value::String(_))
        && matches!(rt.object_get(object_id, "validFrom"), Value::String(_))
        && matches!(rt.object_get(object_id, "validTo"), Value::String(_))
        && matches!(rt.object_get(object_id, "raw"), Value::Object(_))
        && matches!(rt.object_get(object_id, "__pem"), Value::String(_))
        && rt.is_callable(&rt.object_get(object_id, "toString"))
        && rt.is_callable(&rt.object_get(object_id, "toLegacyObject"))
        && rt.is_callable(&rt.object_get(object_id, "checkHost"))
}

fn cruftscript_import_edge_result_is_crypto_certificate_constructor(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(rt.object_get(object_id, "name"), Value::String(ref s) if s.as_str() == "Certificate")
        && matches!(rt.object_get(object_id, "prototype"), Value::Object(proto)
            if matches!(rt.object_get(proto, "constructor"), Value::Object(ctor) if ctor == object_id))
}

fn cruftscript_import_edge_result_is_crypto_keyobject_constructor(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(rt.object_get(object_id, "name"), Value::String(ref s) if s.as_str() == "KeyObject")
        && matches!(rt.object_get(object_id, "prototype"), Value::Object(proto)
            if matches!(rt.object_get(proto, "constructor"), Value::Object(ctor) if ctor == object_id))
}

fn cruftscript_import_edge_result_is_crypto_hash_constructor(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(rt.object_get(object_id, "name"), Value::String(ref s) if s.as_str() == "Hash")
        && matches!(rt.object_get(object_id, "prototype"), Value::Object(proto)
            if matches!(rt.object_get(proto, "constructor"), Value::Object(ctor) if ctor == object_id))
}

fn cruftscript_import_edge_result_is_crypto_hmac_constructor(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(rt.object_get(object_id, "name"), Value::String(ref s) if s.as_str() == "Hmac")
        && matches!(rt.object_get(object_id, "prototype"), Value::Object(proto)
            if matches!(rt.object_get(proto, "constructor"), Value::Object(ctor) if ctor == object_id))
}

fn cruftscript_import_edge_result_is_http_server(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(
        rt.object_get(object_id, "__http_server__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_tls_server(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(
        rt.object_get(object_id, "__tls_server__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_http2_server(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(
        rt.object_get(object_id, "__http2_server__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_http2_client_session(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(
        rt.object_get(object_id, "__http2_client_session__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_zlib_stream(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(
        rt.object_get(object_id, "__zlib_stream__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_stream_handle(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(rt.object_get(object_id, "__stream_kind"), Value::String(_))
        && (rt.is_callable(&rt.object_get(object_id, "on"))
            || rt.is_callable(&rt.object_get(object_id, "write"))
            || rt.is_callable(&rt.object_get(object_id, "pipe")))
}

fn cruftscript_import_edge_result_is_node_stream_module(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    rt.is_callable(&rt.object_get(object_id, "Readable"))
        && rt.is_callable(&rt.object_get(object_id, "Writable"))
        && rt.is_callable(&rt.object_get(object_id, "Duplex"))
        && rt.is_callable(&rt.object_get(object_id, "Transform"))
        && rt.is_callable(&rt.object_get(object_id, "PassThrough"))
        && rt.is_callable(&rt.object_get(object_id, "pipeline"))
        && rt.is_callable(&rt.object_get(object_id, "finished"))
        && matches!(rt.object_get(object_id, "promises"), Value::Object(_))
}

fn cruftscript_import_edge_result_is_stream_web_constructor(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    let Value::String(name) = rt.object_get(object_id, "name") else {
        return false;
    };
    if !matches!(
        name.as_str(),
        "ReadableStream"
            | "WritableStream"
            | "TransformStream"
            | "ByteLengthQueuingStrategy"
            | "CountQueuingStrategy"
            | "ReadableStreamDefaultController"
            | "ReadableStreamDefaultReader"
            | "ReadableStreamBYOBReader"
            | "ReadableStreamBYOBRequest"
            | "ReadableByteStreamController"
            | "WritableStreamDefaultController"
            | "WritableStreamDefaultWriter"
            | "TransformStreamDefaultController"
            | "TextEncoderStream"
            | "TextDecoderStream"
            | "CompressionStream"
            | "DecompressionStream"
    ) {
        return false;
    }
    matches!(rt.object_get(object_id, "prototype"), Value::Object(proto)
        if matches!(rt.object_get(proto, "constructor"), Value::Object(ctor) if ctor == object_id))
}

fn cruftscript_import_edge_result_is_diagnostics_channel(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(
        rt.object_get(object_id, "__diagnostics_channel__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_domain_handle(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(
        rt.object_get(object_id, "__domain_handle__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_perf_event_loop_delay_monitor(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(
        rt.object_get(object_id, "__perf_event_loop_delay_monitor__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_perf_histogram(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(
        rt.object_get(object_id, "__perf_histogram__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_readline_interface(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(
        rt.object_get(object_id, "__readline_interface__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_trace_events_tracing(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(
        rt.object_get(object_id, "__trace_events_tracing__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_async_hooks_hook(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(
        rt.object_get(object_id, "__async_hooks_hook__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_worker_message_channel(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    matches!(
        rt.object_get(object_id, "__worker_message_channel__"),
        Value::Boolean(true)
    )
}

fn cruftscript_import_edge_result_is_worker_handle(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(
        rt.object_get(object_id, "__worker_threads_worker__"),
        Value::Boolean(true)
    ) && rt.is_callable(&rt.object_get(object_id, "postMessage"))
        && rt.is_callable(&rt.object_get(object_id, "terminate"))
}

fn cruftscript_import_edge_result_is_dgram_socket(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(rt.object_get(object_id, "__dgram_type"), Value::String(_))
        && rt.is_callable(&rt.object_get(object_id, "bind"))
        && rt.is_callable(&rt.object_get(object_id, "send"))
        && rt.is_callable(&rt.object_get(object_id, "close"))
}

fn cruftscript_import_edge_result_is_net_socket(rt: &Runtime, object_id: ObjectRef) -> bool {
    let has_net_socket_tag = matches!(
        rt.object_get(object_id, "__cruft_net_socket"),
        Value::Boolean(true)
    );
    let has_stream_id = matches!(
        rt.object_get(object_id, "__net_stream_id"),
        Value::Number(_)
    );
    let has_socket_state = matches!(rt.object_get(object_id, "readyState"), Value::String(_))
        && matches!(rt.object_get(object_id, "connecting"), Value::Boolean(_));
    (has_net_socket_tag || has_stream_id || has_socket_state)
        && rt.is_callable(&rt.object_get(object_id, "write"))
        && rt.is_callable(&rt.object_get(object_id, "end"))
        && rt.is_callable(&rt.object_get(object_id, "destroy"))
}

fn cruftscript_import_edge_result_is_net_server(rt: &Runtime, object_id: ObjectRef) -> bool {
    matches!(rt.object_get(object_id, "listening"), Value::Boolean(_))
        && rt.is_callable(&rt.object_get(object_id, "listen"))
        && rt.is_callable(&rt.object_get(object_id, "address"))
        && rt.is_callable(&rt.object_get(object_id, "close"))
}

fn cruftscript_import_edge_result_is_child_process_handle(
    rt: &Runtime,
    object_id: ObjectRef,
) -> bool {
    let streaming_child = matches!(rt.object_get(object_id, "__child_id"), Value::Number(_))
        && rt.is_callable(&rt.object_get(object_id, "kill"))
        && matches!(rt.object_get(object_id, "stdin"), Value::Object(_));
    let fork_child = matches!(rt.object_get(object_id, "pid"), Value::Number(_))
        && matches!(rt.object_get(object_id, "connected"), Value::Boolean(_))
        && rt.is_callable(&rt.object_get(object_id, "send"))
        && rt.is_callable(&rt.object_get(object_id, "disconnect"))
        && rt.is_callable(&rt.object_get(object_id, "kill"));
    streaming_child || fork_child
}

enum PendingImportPromiseError {
    Idle,
    MaxPumps,
    Pump(RuntimeError),
}

fn cruftscript_await_pending_import_promise(
    rt: &mut Runtime,
    promise_id: ObjectRef,
) -> Result<Value, PendingImportPromiseError> {
    const MAX_PUMPS: usize = 100_000;
    let mut pumps = 0usize;
    loop {
        let (status, value) = {
            let object = rt.obj(promise_id);
            let InternalKind::Promise(state) = &object.internal_kind else {
                return Err(PendingImportPromiseError::Pump(RuntimeError::TypeError(
                    "cruftscript import edge lost native Promise during async adoption".into(),
                )));
            };
            (state.status, state.value.clone())
        };
        match status {
            PromiseStatus::Fulfilled => {
                rt.pending_unhandled.remove(&promise_id);
                return Ok(value);
            }
            PromiseStatus::Rejected => {
                rt.pending_unhandled.remove(&promise_id);
                return Ok(Value::Object(promise_id));
            }
            PromiseStatus::Pending => {}
        }

        let did_work = job_queue::pump_one_tick(rt).map_err(PendingImportPromiseError::Pump)?;
        if !did_work {
            let progressed = if let Some(poll) = rt.host_hooks.poll_io.take() {
                let progressed = poll(rt).map_err(PendingImportPromiseError::Pump)?;
                rt.host_hooks.poll_io = Some(poll);
                progressed
            } else {
                false
            };
            if !progressed {
                return Err(PendingImportPromiseError::Idle);
            }
        }

        pumps += 1;
        if pumps > MAX_PUMPS {
            return Err(PendingImportPromiseError::MaxPumps);
        }
    }
}

fn cruftscript_import_edge_pending_promise_error(
    module_url: &str,
    call: &cruftscript_type_checker::LoweredImportedCall,
) -> RuntimeError {
    let record = cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
        module_url,
        "ImportEdgePendingPromiseUnsupported",
        call.span,
        format!(
            "cruftscript import edge `{}` from `{}` returned a pending Promise; async boundary adoption requires a settled native Promise in this substrate generation (policy_id={} policy_name={} resolution_chain={})",
            call.imported_name,
            call.source,
            call.policy_id,
            call.policy_name,
            call.resolution_chain
        ),
        70,
    );
    RuntimeError::TypeError(format!("{}; {}", record.message, record.tooling_line()))
}

fn cruftscript_import_edge_pending_promise_pump_bound_error(
    module_url: &str,
    call: &cruftscript_type_checker::LoweredImportedCall,
) -> RuntimeError {
    let record = cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
        module_url,
        "ImportEdgePendingPromisePumpBoundExceeded",
        call.span,
        format!(
            "cruftscript import edge `{}` from `{}` returned a pending Promise that did not settle before the async adoption pump bound (policy_id={} policy_name={} resolution_chain={})",
            call.imported_name,
            call.source,
            call.policy_id,
            call.policy_name,
            call.resolution_chain
        ),
        70,
    );
    RuntimeError::TypeError(format!("{}; {}", record.message, record.tooling_line()))
}

fn cruftscript_import_edge_promise_rejected_error(
    rt: &Runtime,
    module_url: &str,
    call: &cruftscript_type_checker::LoweredImportedCall,
    reason: Value,
) -> RuntimeError {
    let reason_detail = cruftscript_runtime_rejection_reason_detail(rt, &reason);
    let record = cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
        module_url,
        "ImportEdgePromiseRejected",
        call.span,
        format!(
            "cruftscript import edge `{}` from `{}` returned a rejected Promise (policy_id={} policy_name={} resolution_chain={} reason={})",
            call.imported_name,
            call.source,
            call.policy_id,
            call.policy_name,
            call.resolution_chain,
            reason_detail
        ),
        70,
    );
    RuntimeError::TypeError(format!("{}; {}", record.message, record.tooling_line()))
}

fn cruftscript_import_edge_result_type_error(
    module_url: &str,
    call: &cruftscript_type_checker::LoweredImportedCall,
    expected_type: &cruftscript_type_checker::TypeTerm,
    received: &Value,
) -> RuntimeError {
    let record = cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
        module_url,
        "ImportEdgeResultTypeMismatch",
        call.span,
        format!(
            "cruftscript import edge `{}` from `{}` returned a value outside its declared result type (policy_id={} policy_name={} resolution_chain={} expected={} received={})",
            call.imported_name,
            call.source,
            call.policy_id,
            call.policy_name,
            call.resolution_chain,
            cruftscript_type_term_name(expected_type),
            cruftscript_runtime_value_label(received)
        ),
        70,
    );
    RuntimeError::TypeError(format!("{}; {}", record.message, record.tooling_line()))
}

fn cruftscript_import_edge_thenable_error(
    module_url: &str,
    call: &cruftscript_type_checker::LoweredImportedCall,
) -> RuntimeError {
    let record = cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
        module_url,
        "ImportEdgeThenableUnsupported",
        call.span,
        format!(
            "cruftscript import edge `{}` from `{}` returned an arbitrary thenable; thenable adoption requires a typed authority model and is not trusted in this substrate generation (policy_id={} policy_name={} resolution_chain={})",
            call.imported_name,
            call.source,
            call.policy_id,
            call.policy_name,
            call.resolution_chain
        ),
        70,
    );
    RuntimeError::TypeError(format!("{}; {}", record.message, record.tooling_line()))
}

fn cruftscript_import_edge_host_handle_error(
    module_url: &str,
    call: &cruftscript_type_checker::LoweredImportedCall,
    handle_kind: &str,
    reason: &str,
) -> RuntimeError {
    let record = cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
        module_url,
        "ImportEdgeHostHandleUnsupported",
        call.span,
        format!(
            "cruftscript import edge `{}` from `{}` returned unsupported host handle `{}`: {} (policy_id={} policy_name={} resolution_chain={})",
            call.imported_name,
            call.source,
            handle_kind,
            reason,
            call.policy_id,
            call.policy_name,
            call.resolution_chain
        ),
        70,
    );
    RuntimeError::TypeError(format!("{}; {}", record.message, record.tooling_line()))
}

fn cruftscript_sanitize_import_edge_failure(
    rt: &mut Runtime,
    module_url: &str,
    call: &cruftscript_type_checker::LoweredImportedCall,
    received: Value,
) -> Result<Value, RuntimeError> {
    let Some(expected_type) = call.expected_type.as_ref() else {
        return Err(cruftscript_missing_sanitizer_default_error(
            module_url,
            call,
            "untyped-import-edge",
        ));
    };
    let Some(default) = call
        .sanitizer_defaults
        .iter()
        .find(|default| cruftscript_type_terms_equivalent(&default.target_type, expected_type))
    else {
        return Err(cruftscript_missing_sanitizer_default_error(
            module_url,
            call,
            &cruftscript_type_term_name(expected_type),
        ));
    };
    let Some(value) = cruftscript_sanitizer_default_to_value(rt, &default.expr) else {
        return Err(cruftscript_missing_sanitizer_default_error(
            module_url,
            call,
            &cruftscript_type_term_name(expected_type),
        ));
    };
    let record = cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
        module_url,
        "ImportEdgeSanitized",
        call.span,
        format!(
            "cruftscript import edge `{}` from `{}` sanitized failed boundary value to declared default (policy_id={} policy_name={} resolution_chain={} target_type={} received={})",
            call.imported_name,
            call.source,
            call.policy_id,
            call.policy_name,
            call.resolution_chain,
            cruftscript_type_term_name(expected_type),
            cruftscript_runtime_value_label(&received)
        ),
        0,
    );
    rt.boundary_debug_violation_count += 1;
    rt.boundary_debug_last_violation = Some(record.message);
    Ok(value)
}

fn cruftscript_missing_sanitizer_default_error(
    module_url: &str,
    call: &cruftscript_type_checker::LoweredImportedCall,
    target_type: &str,
) -> RuntimeError {
    let record = cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
        module_url,
        "SanitizerDefaultMissingAtRuntime",
        call.span,
        format!(
            "cruftscript import edge `{}` from `{}` has no runtime sanitizer default for `{}` (policy_id={} policy_name={} resolution_chain={})",
            call.imported_name,
            call.source,
            target_type,
            call.policy_id,
            call.policy_name,
            call.resolution_chain
        ),
        70,
    );
    RuntimeError::TypeError(format!("{}; {}", record.message, record.tooling_line()))
}

fn cruftscript_sanitizer_default_to_value(
    rt: &mut Runtime,
    expr: &cruftscript_type_checker::SanitizerDefaultExpr,
) -> Option<Value> {
    match expr {
        cruftscript_type_checker::SanitizerDefaultExpr::StringLiteral(value) => {
            Some(Value::String(Rc::new(
                rusty_js_runtime::value::JsString::from(value.as_str()),
            )))
        }
        cruftscript_type_checker::SanitizerDefaultExpr::NumberLiteral(value) => {
            value.parse::<f64>().ok().map(Value::Number)
        }
        cruftscript_type_checker::SanitizerDefaultExpr::BooleanLiteral(value) => {
            Some(Value::Boolean(*value))
        }
        cruftscript_type_checker::SanitizerDefaultExpr::Null => Some(Value::Null),
        cruftscript_type_checker::SanitizerDefaultExpr::Undefined => Some(Value::Undefined),
        cruftscript_type_checker::SanitizerDefaultExpr::ArrayLiteral(elements) => {
            let array = rt.alloc_object(rusty_js_runtime::value::Object::new_ordinary());
            for (index, element) in elements.iter().enumerate() {
                let value = cruftscript_expr_envelope_to_value(rt, element)?;
                rt.object_set(array, index.to_string(), value);
            }
            rt.object_set(array, "length".into(), Value::Number(elements.len() as f64));
            Some(Value::Object(array))
        }
        cruftscript_type_checker::SanitizerDefaultExpr::ObjectLiteral(properties) => {
            let object = rt.alloc_object(rusty_js_runtime::value::Object::new_ordinary());
            for property in properties {
                let value = cruftscript_expr_envelope_to_value(rt, &property.value)?;
                rt.object_set(object, property.name.clone(), value);
            }
            Some(Value::Object(object))
        }
        cruftscript_type_checker::SanitizerDefaultExpr::Unsupported(_) => None,
    }
}

fn cruftscript_sanitizer_default_for_type(
    rt: &mut Runtime,
    defaults: &[cruftscript_type_checker::SanitizerDefaultBinding],
    expected_type: &cruftscript_type_checker::TypeTerm,
) -> Option<Value> {
    defaults
        .iter()
        .find(|default| cruftscript_type_terms_equivalent(&default.target_type, expected_type))
        .and_then(|default| cruftscript_sanitizer_default_to_value(rt, &default.expr))
}

fn cruftscript_expr_envelope_to_value(
    rt: &mut Runtime,
    expr: &cruftscript_type_checker::SanitizerExprEnvelope,
) -> Option<Value> {
    match &expr.kind {
        cruftscript_type_checker::SanitizerExprEnvelopeKind::StringLiteral(value) => {
            Some(Value::String(Rc::new(
                rusty_js_runtime::value::JsString::from(value.as_str()),
            )))
        }
        cruftscript_type_checker::SanitizerExprEnvelopeKind::NumberLiteral(value) => {
            value.parse::<f64>().ok().map(Value::Number)
        }
        cruftscript_type_checker::SanitizerExprEnvelopeKind::BooleanLiteral(value) => {
            Some(Value::Boolean(*value))
        }
        cruftscript_type_checker::SanitizerExprEnvelopeKind::ArrayLiteral(elements) => {
            let array = rt.alloc_object(rusty_js_runtime::value::Object::new_ordinary());
            for (index, element) in elements.iter().enumerate() {
                let value = cruftscript_expr_envelope_to_value(rt, element)?;
                rt.object_set(array, index.to_string(), value);
            }
            rt.object_set(array, "length".into(), Value::Number(elements.len() as f64));
            Some(Value::Object(array))
        }
        cruftscript_type_checker::SanitizerExprEnvelopeKind::ObjectLiteral(properties) => {
            let object = rt.alloc_object(rusty_js_runtime::value::Object::new_ordinary());
            for property in properties {
                let value = cruftscript_expr_envelope_to_value(rt, &property.value)?;
                rt.object_set(object, property.name.clone(), value);
            }
            Some(Value::Object(object))
        }
        _ => None,
    }
}

fn cruftscript_runtime_value_label(value: &Value) -> &'static str {
    match value {
        Value::Undefined => "undefined",
        Value::Null => "null",
        Value::String(_) => "string",
        Value::Number(_) => "number",
        Value::Boolean(_) => "boolean",
        Value::Object(_) => "object",
        _ => "value",
    }
}

fn cruftscript_runtime_rejection_reason_detail(rt: &Runtime, value: &Value) -> String {
    let base = cruftscript_runtime_value_label(value);
    let Value::Object(object_id) = value else {
        return base.to_string();
    };
    let Value::String(name) = rt.object_get(*object_id, "name") else {
        return base.to_string();
    };
    let name = name.as_str();
    if name.is_empty() {
        base.to_string()
    } else {
        format!("{base} reason_name={name}")
    }
}

fn cruftscript_type_terms_equivalent(
    actual: &cruftscript_type_checker::TypeTerm,
    expected: &cruftscript_type_checker::TypeTerm,
) -> bool {
    match (actual, expected) {
        (
            cruftscript_type_checker::TypeTerm::Named { name: a, .. },
            cruftscript_type_checker::TypeTerm::Named { name: e, .. },
        ) => a == e,
        (
            cruftscript_type_checker::TypeTerm::TypeRef {
                name: a,
                type_args: a_args,
                ..
            },
            cruftscript_type_checker::TypeTerm::TypeRef {
                name: e,
                type_args: e_args,
                ..
            },
        ) => {
            a == e
                && a_args.len() == e_args.len()
                && a_args
                    .iter()
                    .zip(e_args)
                    .all(|(a, e)| cruftscript_type_terms_equivalent(a, e))
        }
        _ => false,
    }
}

fn cruftscript_type_term_name(ty: &cruftscript_type_checker::TypeTerm) -> String {
    match ty {
        cruftscript_type_checker::TypeTerm::Named { name, .. } => name.clone(),
        cruftscript_type_checker::TypeTerm::Object { .. } => "object".to_string(),
        cruftscript_type_checker::TypeTerm::TypeRef {
            name, type_args, ..
        } => {
            if type_args.is_empty() {
                name.clone()
            } else {
                format!(
                    "{}<{}>",
                    name,
                    type_args
                        .iter()
                        .map(cruftscript_type_term_name)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        }
        cruftscript_type_checker::TypeTerm::Union { members, .. } => members
            .iter()
            .map(cruftscript_type_term_name)
            .collect::<Vec<_>>()
            .join(" | "),
        cruftscript_type_checker::TypeTerm::Function { .. } => "function".to_string(),
        cruftscript_type_checker::TypeTerm::TypePredicate { target, .. } => {
            format!("predicate<{}>", cruftscript_type_term_name(target))
        }
        cruftscript_type_checker::TypeTerm::Constructor { instance_type, .. } => {
            format!("constructor<{}>", cruftscript_type_term_name(instance_type))
        }
        cruftscript_type_checker::TypeTerm::Tuple { elements, .. } => {
            format!(
                "[{}]",
                elements
                    .iter()
                    .map(cruftscript_type_term_name)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        cruftscript_type_checker::TypeTerm::Unknown { .. } => "unknown".to_string(),
        _ => "unsupported".to_string(),
    }
}

fn cruftscript_import_edge_non_callable_error(
    module_url: &str,
    call: &cruftscript_type_checker::LoweredImportedCall,
) -> RuntimeError {
    let record = cruftscript_type_checker::CruftScriptDiagnosticRecord::runtime_boundary(
        module_url,
        "ImportEdgeNonCallable",
        call.span,
        format!(
            "cruftscript import edge `{}` from `{}` requires a callable named export (policy_id={} policy_name={} resolution_chain={})",
            call.imported_name, call.source, call.policy_id, call.policy_name, call.resolution_chain
        ),
        70,
    );
    RuntimeError::TypeError(format!("{}; {}", record.message, record.tooling_line()))
}

pub fn install_builtin_module_resolver(rt: &mut Runtime) {
    let root_global = rt.global_object;
    rt.install_host_hook(HostHook::ResolveBuiltinModule(Box::new(move |rt, specifier| {

        if specifier == "cruft:presto" {
            if let Value::Object(id) = rt.global_get("__cruft_presto") {
                return Ok(Some(id));
            }
            match build_cruftscript_module_namespace(
                rt,
                "cruft:presto",
                include_str!("presto/presto-core.fts"),
            )? {
                Some(ns) => {
                    if let Some(g) = rt.global_object {
                        rt.object_set(g, "__cruft_presto".to_string(), Value::Object(ns));
                    }
                    return Ok(Some(ns));
                }
                None => return Ok(None),
            }
        }

        let global_name = match specifier {

            "node:fs/promises" | "fs/promises" => "fs_promises",
            "node:fs" | "fs" => "fs",
            "cruft:fs" => "__cruft_fs",
            "node:path" | "path" => "path",
            "cruft:path" => "__cruft_path",
            "node:os" | "os" => "os",
            "cruft:os" => "__cruft_os",
            "node:process" | "process" => "process",
            "cruft:process" => "__cruft_process",

            "node:http" | "http" => "http",
            "_http_common" => "_http_common",
            "cruft:http" => "__cruft_http",
            "cruft:serve" => "__cruft_serve",
            "cruft:ws" => "__cruft_ws",
            "node:crypto" | "crypto" => {
                crate::node_stubs::install_constants_default_cipher_list(rt);
                "crypto"
            }

            "cruft:pm" => "__cruft_pm",
            "cruft:press" => "__cruft_press",

            "node:assert/strict" | "assert/strict" => "__node_assert_strict",
            "node:assert" | "assert" => "__node_assert",
            "node:https" | "https" => {
                crate::node_stubs::install_constants_default_cipher_list(rt);
                "https"
            }
            "node:readable-stream" | "readable-stream" => "__readable_stream_compat",
            "node:stream/consumers" | "stream/consumers" => "stream_consumers",
            "node:stream/promises" | "stream/promises" => "stream_promises",
            "node:stream/web" | "stream/web" => "stream_web",
            "node:stream" | "stream" => "stream",
            "node:url" | "url" => "url",
            "cruft:url" => "__cruft_url",
            "node:util" | "util" => "util",

            "node:zlib" | "zlib" => "zlib",
            "node:tty" | "tty" => "tty",
            "node:events" | "events" => "events",
            "cruft:events" => "__cruft_events",

            "cruft:spawn" => "__cruft_spawn",
            "bun:sqlite" => "__bun_sqlite",
            "node:sqlite" => "__node_sqlite",
            "cruft:sqlite" => "__cruft_sqlite",
            "cruft:orm" => "__crizzle",
            "node:child_process" | "child_process" | "cruft:child_process"
            | "internal/child_process" => "child_process",
            "node:tls" | "tls" => {
                crate::node_stubs::install_constants_default_cipher_list(rt);
                "tls"
            }
            "cruft:tls" => "__cruft_tls",
            "node:readline/promises" | "readline/promises" => "readline_promises",
            "node:readline" | "readline" => "readline",
            "node:trace_events" | "trace_events" => "trace_events",
            "node:wasi" | "wasi" => "wasi",
            "node:dgram" | "dgram" => "dgram",
            "node:constants" | "constants" => "constants",
            "node:string_decoder" | "string_decoder" => "string_decoder",
            "node:buffer" | "buffer" => "buffer",
            "cruft:buffer" => "__cruft_buffer",

            "node:dns/promises" | "dns/promises" => "dns_promises",
            "node:dns" | "dns" => "dns",
            "cruft:dns" => "__cruft_dns",
            "node:module" | "module" => "__node_module",

            "node:test" => "__cruft_node_test",
            "node:test/reporters" => "__cruft_test_reporters",
            "internal/errors" => {
                return Ok(Some(crate::node_stubs::make_internal_errors_module(rt)));
            }
            "internal/test/binding" => {
                return Ok(Some(crate::node_stubs::make_internal_test_binding_module(rt)));
            }
            "internal/test_runner/snapshot" => "__cruft_internal_test_runner_snapshot",
            "internal/test_runner/utils" => "__cruft_internal_test_runner_utils",
            "internal/assert/myers_diff" => "__cruft_internal_assert_myers_diff",
            "internal/async_hooks" => "__cruft_internal_async_hooks",
            "internal/timers" => "__cruft_internal_timers",
            "internal/event_target" => "__cruft_internal_event_target",
            "internal/webstreams/util" => "__cruft_internal_webstreams_util",
            "cruft:test" => "__cruft_test",
            "node:http2" | "http2" => {
                crate::node_stubs::install_constants_default_cipher_list(rt);
                "http2"
            }
            "cruft:http2" => "__cruft_http2",

            "node:net" | "net" => "net",
            "node:diagnostics_channel" | "diagnostics_channel" => "diagnostics_channel",

            "node:v8" | "v8" => "v8",
            "node:inspector/promises" | "inspector/promises" => "inspector",
            "node:inspector" | "inspector" => "inspector",
            "node:vm" | "vm" => "vm",
            "cruft:vm" => "__cruft_vm",

            "node:punycode" | "punycode" => {
                let dependency_load = rt
                    .current_module_url
                    .last()
                    .is_some_and(|url| url.contains("/node_modules/"));
                if !dependency_load {
                    rusty_js_runtime::interp::queue_node_warning_once(
                        "node-punycode-deprecation",
                        format!(
                            "(node:{}) [DEP0040] DeprecationWarning: The `punycode` module is deprecated. Please use a userland alternative instead.\n(Use `node --trace-deprecation ...` to show where the warning was created)",
                            std::process::id()
                        ),
                    );
                }
                "punycode"
            }

            "node:console" | "console" => "node_console",
            "node:util/types" | "util/types" => "util_types",
            "node:domain" | "domain" => "domain",
            "node:async_hooks" | "async_hooks" => "async_hooks",
            "node:perf_hooks" | "perf_hooks" => "perf_hooks",
            "safe-regex2" => "__safe_regex2_compat",
            "node:worker_threads" | "worker_threads" => "worker_threads",
            "cruft:worker" => "__cruft_worker",
            "node:querystring" | "querystring" => "querystring",
            "cruft:querystring" => "__cruft_querystring",
            "node:timers" | "timers" => "timers",
            "node:timers/promises" | "timers/promises" => "timers_promises",

            "node:sys" | "sys" => "util",
            "node:cluster" | "cluster" => "cluster",
            "node:repl" | "repl" => "repl",

            "node:path/posix" | "path/posix" => {
                if let Some(path_id) = resolve_builtin_backing_global(rt, root_global, "path") {
                    if let Value::Object(sub) = rt.object_get(path_id, "posix") {
                        return Ok(Some(sub));
                    }
                }
                return Ok(None);
            }
            "node:path/win32" | "path/win32" => {
                if let Some(path_id) = resolve_builtin_backing_global(rt, root_global, "path") {
                    if let Value::Object(sub) = rt.object_get(path_id, "win32") {
                        return Ok(Some(sub));
                    }
                }
                return Ok(None);
            }
            _ => return Ok(None),
        };

        rt.materialize_lazy_host_module(global_name);
        Ok(resolve_builtin_backing_global(rt, root_global, global_name))
    })));
}

fn resolve_builtin_backing_global(
    rt: &Runtime,
    root_global: Option<rusty_js_runtime::value::ObjectRef>,
    global_name: &str,
) -> Option<rusty_js_runtime::value::ObjectRef> {

    if let Some(root) = root_global {
        if let Value::Object(id) = rt.object_get(root, global_name) {
            return Some(id);
        }
    }
    if let Value::Object(id) = rt.global_get(global_name) {
        return Some(id);
    }
    for realm in rt.realms.iter().rev() {
        if let Some(Value::Object(id)) = realm
            .primordial_full_snapshot
            .last()
            .and_then(|snapshot| snapshot.get(global_name))
        {
            return Some(*id);
        }
    }
    None
}
