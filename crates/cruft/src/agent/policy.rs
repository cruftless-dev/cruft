use super::doctor::agent_doctor_json;
use super::integrity::agent_integrity_for_path;
use super::model::agent_load_model_fixture;
use super::named_network::agent_load_osv_fixture;
use super::run::{agent_load_event_schema, agent_validate_run_id};
use super::tools::{agent_validate_fs_read_pattern, is_agent_builtin_tool_specifier};
use crate::{json_escape, json_string_literal};
use std::process::ExitCode;

pub(crate) fn agent_env_key_is_valid(key: &str) -> bool {
    !key.is_empty()
        && !key.contains('=')
        && !key.contains('\0')
        && key.chars().all(|c| c == '_' || c.is_ascii_alphanumeric())
}

pub(crate) fn agent_secret_scopes_js(scopes: &[(String, String)]) -> String {
    scopes
        .iter()
        .map(|(tool, env)| {
            format!(
                "{{tool:{},credential_mode:\"host_env_bearer\",credential_env:{}}}",
                json_string_literal(tool),
                json_string_literal(env)
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn agent_secret_scope_parse(raw: &str) -> Result<(String, String), String> {
    let Some((tool, env)) = raw.split_once('=') else {
        return Err("--secret requires <tool=ENV>".to_string());
    };
    if !is_agent_builtin_tool_specifier(tool) {
        return Err(format!(
            "--secret tool {tool:?} is unknown; run `cruft agent tool list`"
        ));
    }
    if !agent_env_key_is_valid(env) {
        return Err(format!(
            "--secret env must be a non-empty ASCII env key: {env:?}"
        ));
    }
    Ok((tool.to_string(), env.to_string()))
}

pub(crate) fn agent_policy_path_arg(base: &std::path::Path, path: &str) -> String {
    let path_obj = std::path::Path::new(path);
    if path_obj.is_absolute() {
        path.to_string()
    } else {
        base.join(path_obj).display().to_string()
    }
}

pub(crate) fn agent_policy_string_field<'a>(
    object: &'a serde_json::Map,
    key: &str,
) -> Result<Option<&'a str>, String> {
    match object.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(value)) => Ok(Some(value)),
        Some(_) => Err(format!("policy field {key:?} must be a string")),
    }
}

fn agent_policy_u64_field(object: &serde_json::Map, key: &str) -> Result<Option<u64>, String> {
    match object.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(value @ serde_json::Value::Number(_)) => value
            .as_u64()
            .filter(|n| *n > 0)
            .map(Some)
            .ok_or_else(|| format!("policy field {key:?} must be a positive integer")),
        Some(_) => Err(format!("policy field {key:?} must be a positive integer")),
    }
}

fn agent_policy_bool_field(object: &serde_json::Map, key: &str) -> Result<bool, String> {
    match object.get(key) {
        None | Some(serde_json::Value::Null) => Ok(false),
        Some(serde_json::Value::Bool(value)) => Ok(*value),
        Some(_) => Err(format!("policy field {key:?} must be a boolean")),
    }
}

pub(crate) fn agent_policy_string_array_field(
    object: &serde_json::Map,
    key: &str,
) -> Result<Vec<String>, String> {
    let Some(value) = object.get(key) else {
        return Ok(Vec::new());
    };
    let Some(values) = value.as_array() else {
        return Err(format!("policy field {key:?} must be an array of strings"));
    };
    let mut out = Vec::new();
    for value in values {
        let Some(value) = value.as_str() else {
            return Err(format!("policy field {key:?} must be an array of strings"));
        };
        out.push(value.to_string());
    }
    Ok(out)
}

fn agent_policy_path_map_field(
    object: &serde_json::Map,
    base: &std::path::Path,
    key: &str,
) -> Result<Vec<(String, String)>, String> {
    let Some(value) = object.get(key) else {
        return Ok(Vec::new());
    };
    let Some(map) = value.as_object() else {
        return Err(format!("policy field {key:?} must be an object"));
    };
    let mut out = Vec::new();
    for (specifier, path) in map {
        let Some(path) = path.as_str() else {
            return Err(format!("policy field {key:?} values must be strings"));
        };
        out.push((specifier.clone(), agent_policy_path_arg(base, path)));
    }
    Ok(out)
}

pub(crate) fn agent_policy_string_map_field(
    object: &serde_json::Map,
    key: &str,
) -> Result<Vec<(String, String)>, String> {
    let Some(value) = object.get(key) else {
        return Ok(Vec::new());
    };
    let Some(map) = value.as_object() else {
        return Err(format!("policy field {key:?} must be an object"));
    };
    let mut out = Vec::new();
    for (specifier, value) in map {
        let Some(value) = value.as_str() else {
            return Err(format!("policy field {key:?} values must be strings"));
        };
        out.push((specifier.clone(), value.to_string()));
    }
    Ok(out)
}

pub(crate) fn agent_policy_expand_run_args(policy_path: &str) -> Result<Vec<String>, String> {
    let source = std::fs::read_to_string(policy_path)
        .map_err(|e| format!("cannot read policy {policy_path}: {e}"))?;
    let value = serde_json::from_str::<serde_json::Value>(&source)
        .map_err(|e| format!("cannot parse policy {policy_path}: {e}"))?;
    let Some(object) = value.as_object() else {
        return Err("policy root must be an object".to_string());
    };
    let policy_base = std::path::Path::new(policy_path)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let mut out = Vec::new();
    if let Some(agent) = agent_policy_string_field(object, "agent")? {
        out.push(agent_policy_path_arg(policy_base, agent));
    }
    if agent_policy_bool_field(object, "worker")? {
        out.push("--worker".to_string());
    }
    if let Some(audit_log) = agent_policy_string_field(object, "audit_log")? {
        out.push(format!(
            "--audit-log={}",
            agent_policy_path_arg(policy_base, audit_log)
        ));
    }
    if let Some(run_id) = agent_policy_string_field(object, "run_id")? {
        out.push(format!("--run-id={run_id}"));
    }
    if let Some(context) = object.get("context") {
        out.push(format!("--context-json={context}"));
    }
    if let Some(state) = object.get("state") {
        out.push(format!("--state-json={state}"));
    }
    if let Some(session_file) = agent_policy_string_field(object, "session_file")? {
        out.push(format!(
            "--session-file={}",
            agent_policy_path_arg(policy_base, session_file)
        ));
    }
    if let Some(osv_fixture) = agent_policy_string_field(object, "osv_fixture")? {
        out.push(format!(
            "--osv-fixture={}",
            agent_policy_path_arg(policy_base, osv_fixture)
        ));
    }
    if let Some(model_fixture) = agent_policy_string_field(object, "model_fixture")? {
        out.push(format!(
            "--model-fixture={}",
            agent_policy_path_arg(policy_base, model_fixture)
        ));
    }
    if let Some(named_network_cache_dir) =
        agent_policy_string_field(object, "named_network_cache_dir")?
    {
        out.push(format!(
            "--named-network-cache-dir={}",
            agent_policy_path_arg(policy_base, named_network_cache_dir)
        ));
    }
    if let Some(named_network_cache_mode) =
        agent_policy_string_field(object, "named_network_cache_mode")?
    {
        out.push(format!(
            "--named-network-cache-mode={named_network_cache_mode}"
        ));
    }
    if let Some(named_network_cache_max_age_ms) =
        agent_policy_u64_field(object, "named_network_cache_max_age_ms")?
    {
        out.push(format!(
            "--named-network-cache-max-age-ms={named_network_cache_max_age_ms}"
        ));
    }
    if let Some(named_network_cache_max_entries) =
        agent_policy_u64_field(object, "named_network_cache_max_entries")?
    {
        out.push(format!(
            "--named-network-cache-max-entries={named_network_cache_max_entries}"
        ));
    }
    if let Some(named_network_retry_attempts) =
        agent_policy_u64_field(object, "named_network_retry_attempts")?
    {
        out.push(format!(
            "--named-network-retry-attempts={named_network_retry_attempts}"
        ));
    }
    if let Some(github_token_env) = agent_policy_string_field(object, "github_token_env")? {
        out.push(format!("--github-token-env={github_token_env}"));
    }
    for secret in agent_policy_string_array_field(object, "secrets")? {
        out.push(format!("--secret={secret}"));
    }
    if let Some(approval_log) = agent_policy_string_field(object, "approval_log")? {
        out.push(format!(
            "--approval-log={}",
            agent_policy_path_arg(policy_base, approval_log)
        ));
    }
    if let Some(approval_max_age_ms) = agent_policy_u64_field(object, "approval_max_age_ms")? {
        out.push(format!("--approval-max-age-ms={approval_max_age_ms}"));
    }
    for tool in agent_policy_string_array_field(object, "tools")? {
        out.push(format!("--tool={tool}"));
    }
    for tool in agent_policy_string_array_field(object, "approval_required_tools")? {
        out.push(format!("--require-approval={tool}"));
    }
    for tool in agent_policy_string_array_field(object, "approved_tools")? {
        out.push(format!("--approve-tool={tool}"));
    }
    for path in agent_policy_string_array_field(object, "fs_read")? {
        out.push(format!(
            "--fs-read={}",
            agent_policy_path_arg(policy_base, &path)
        ));
    }
    for pattern in agent_policy_string_array_field(object, "fs_read_include")? {
        out.push(format!("--fs-read-include={pattern}"));
    }
    for pattern in agent_policy_string_array_field(object, "fs_read_exclude")? {
        out.push(format!("--fs-read-exclude={pattern}"));
    }
    for path in agent_policy_string_array_field(object, "fs_write")? {
        out.push(format!(
            "--fs-write={}",
            agent_policy_path_arg(policy_base, &path)
        ));
    }
    for command in agent_policy_string_array_field(object, "process_commands")? {
        let Some((name, path)) = command.split_once('=') else {
            return Err("policy field \"process_commands\" entries must be name=path".to_string());
        };
        out.push(format!(
            "--process-command={}={}",
            name,
            agent_policy_path_arg(policy_base, path)
        ));
    }
    for path in agent_policy_string_array_field(object, "process_cwd")? {
        out.push(format!(
            "--process-cwd={}",
            agent_policy_path_arg(policy_base, &path)
        ));
    }
    for env in agent_policy_string_array_field(object, "process_env")? {
        out.push(format!("--process-env={env}"));
    }
    for (kind, path) in agent_policy_path_map_field(object, policy_base, "expected_events")? {
        out.push(format!("--expect-event={kind}={path}"));
    }
    if let Some(budgets) = object.get("budgets") {
        let Some(budgets) = budgets.as_object() else {
            return Err("policy field \"budgets\" must be an object".to_string());
        };
        for (key, flag) in [
            ("timeout_ms", "--timeout-ms"),
            ("tool_timeout_ms", "--tool-timeout-ms"),
            ("max_state_bytes", "--max-state-bytes"),
            ("max_events", "--max-events"),
            ("max_event_bytes", "--max-event-bytes"),
            ("max_tool_arg_bytes", "--max-tool-arg-bytes"),
            ("max_tool_result_bytes", "--max-tool-result-bytes"),
            (
                "process_output_stream_chunk_bytes",
                "--process-output-stream-chunk-bytes",
            ),
            ("max_microtasks", "--max-microtasks"),
            ("max_steps", "--max-steps"),
            ("max_rss_mb", "--max-rss-mb"),
        ] {
            if let Some(value) = agent_policy_u64_field(budgets, key)? {
                out.push(format!("{flag}={value}"));
            }
        }
    }
    for field in agent_policy_string_array_field(object, "redact_fields")? {
        out.push(format!("--redact-field={field}"));
    }
    for (specifier, path) in agent_policy_path_map_field(object, policy_base, "modules")? {
        out.push(format!("--module={specifier}={path}"));
    }
    for (specifier, path) in agent_policy_path_map_field(object, policy_base, "packages")? {
        out.push(format!("--package={specifier}={path}"));
    }
    for (specifier, integrity) in agent_policy_string_map_field(object, "package_integrity")? {
        out.push(format!("--package-integrity={specifier}={integrity}"));
    }
    for (specifier, path) in agent_policy_path_map_field(object, policy_base, "import_hooks")? {
        out.push(format!("--import-hook={specifier}={path}"));
    }
    for (specifier, integrity) in agent_policy_string_map_field(object, "import_hook_integrity")? {
        out.push(format!("--import-hook-integrity={specifier}={integrity}"));
    }
    if let Some(entry_module) = agent_policy_string_field(object, "entry_module")? {
        out.push(format!("--entry-module={entry_module}"));
    }
    Ok(out)
}

pub(crate) fn agent_project_policy_path(project_dir: &str) -> String {
    std::path::Path::new(project_dir)
        .join("agent-policy.json")
        .to_string_lossy()
        .into_owned()
}

fn agent_policy_target_path(target: &str) -> String {
    let path = std::path::Path::new(target);
    if path.is_dir() {
        agent_project_policy_path(target)
    } else {
        target.to_string()
    }
}

fn agent_policy_load_project(project_dir: &str) -> Result<(String, serde_json::Value), String> {
    let policy_path = agent_policy_target_path(project_dir);
    let source = std::fs::read_to_string(&policy_path)
        .map_err(|e| format!("cannot read policy {policy_path}: {e}"))?;
    let value = serde_json::from_str::<serde_json::Value>(&source)
        .map_err(|e| format!("cannot parse policy {policy_path}: {e}"))?;
    if value.as_object().is_none() {
        return Err("policy root must be an object".to_string());
    }
    Ok((policy_path, value))
}

pub(crate) fn agent_policy_load_target(
    target: &str,
) -> Result<(String, String, serde_json::Value), String> {
    let policy_path = agent_policy_target_path(target);
    let source = std::fs::read_to_string(&policy_path)
        .map_err(|e| format!("cannot read policy {policy_path}: {e}"))?;
    let value = serde_json::from_str::<serde_json::Value>(&source)
        .map_err(|e| format!("cannot parse policy {policy_path}: {e}"))?;
    if value.as_object().is_none() {
        return Err("policy root must be an object".to_string());
    }
    Ok((policy_path, source, value))
}

fn agent_policy_write(policy_path: &str, value: &serde_json::Value) -> Result<(), String> {
    let rendered = value.to_pretty_string();
    std::fs::write(policy_path, format!("{rendered}\n"))
        .map_err(|e| format!("cannot write policy {policy_path}: {e}"))
}

fn agent_policy_check_path(
    base: &std::path::Path,
    label: &str,
    path: &str,
) -> Result<String, String> {
    let resolved = agent_policy_path_arg(base, path);
    if !std::path::Path::new(&resolved).exists() {
        return Err(format!("{label} path does not exist: {resolved}"));
    }
    Ok(resolved)
}

fn agent_policy_check_project_confined_path(label: &str, path: &str) -> Result<(), String> {
    let path_obj = std::path::Path::new(path);
    if path_obj.is_absolute() {
        return Err(format!(
            "{label} path must be project-relative under --project-confined: {path}"
        ));
    }
    if path_obj
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(format!(
            "{label} path must not escape the project root under --project-confined: {path}"
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Default)]
pub(crate) struct AgentPolicyValidationOptions {
    pub(crate) strict: bool,
    pub(crate) project_confined: bool,
}

fn agent_policy_allowed_root_field(key: &str) -> bool {
    matches!(
        key,
        "schema_version"
            | "profile"
            | "agent"
            | "worker"
            | "audit_log"
            | "run_id"
            | "context"
            | "state"
            | "session_file"
            | "osv_fixture"
            | "model_fixture"
            | "named_network_cache_dir"
            | "named_network_cache_mode"
            | "named_network_cache_max_age_ms"
            | "named_network_cache_max_entries"
            | "named_network_retry_attempts"
            | "github_token_env"
            | "secrets"
            | "approval_log"
            | "approval_max_age_ms"
            | "tools"
            | "approval_required_tools"
            | "approved_tools"
            | "fs_read"
            | "fs_read_include"
            | "fs_read_exclude"
            | "fs_write"
            | "process_commands"
            | "process_cwd"
            | "process_env"
            | "expected_events"
            | "redact_fields"
            | "budgets"
            | "modules"
            | "packages"
            | "package_integrity"
            | "import_hooks"
            | "import_hook_integrity"
            | "entry_module"
    )
}

fn agent_policy_allowed_budget_key(key: &str) -> bool {
    matches!(
        key,
        "timeout_ms"
            | "tool_timeout_ms"
            | "max_state_bytes"
            | "max_events"
            | "max_event_bytes"
            | "max_tool_arg_bytes"
            | "max_tool_result_bytes"
            | "process_output_stream_chunk_bytes"
            | "max_microtasks"
            | "max_steps"
            | "max_scheduler_turns"
            | "max_rss_mb"
    )
}

fn agent_policy_push_duplicate_key_errors(
    object: &serde_json::Map,
    label: &str,
    errors: &mut Vec<String>,
) {
    let mut seen = std::collections::HashSet::new();
    for key in object.keys() {
        if !seen.insert(key.as_str()) {
            errors.push(format!("{label} duplicate key {key:?}"));
        }
    }
}

pub(crate) fn agent_policy_validate_value_with_options(
    policy_path: &str,
    value: &serde_json::Value,
    options: AgentPolicyValidationOptions,
) -> Result<Vec<String>, Vec<String>> {
    let Some(object) = value.as_object() else {
        return Err(vec!["policy root must be an object".to_string()]);
    };
    let base = std::path::Path::new(policy_path)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let mut errors = Vec::new();
    let mut notes = Vec::new();
    if options.strict {
        agent_policy_push_duplicate_key_errors(object, "policy root", &mut errors);
        for key in object.keys() {
            if !agent_policy_allowed_root_field(key) {
                errors.push(format!("unknown policy field {key:?}"));
            }
        }
        if let Some(schema_version) = object.get("schema_version") {
            match schema_version.as_u64() {
                Some(1) => {}
                Some(other) => errors.push(format!(
                    "unsupported policy schema_version {other}; supported: 1"
                )),
                None => {
                    errors.push("policy field \"schema_version\" must be the integer 1".to_string())
                }
            }
        }
    }
    if let Err(e) = agent_policy_string_field(object, "profile") {
        errors.push(e);
    }
    match agent_policy_string_field(object, "agent") {
        Ok(Some(agent)) => {
            if options.project_confined {
                if let Err(e) = agent_policy_check_project_confined_path("agent", agent) {
                    errors.push(e);
                }
            }
            match agent_policy_check_path(base, "agent", agent) {
                Ok(path) => notes.push(format!("agent={path}")),
                Err(e) => errors.push(e),
            }
        }
        Ok(None) => errors.push("policy field \"agent\" is required".to_string()),
        Err(e) => errors.push(e),
    }
    if let Err(e) = agent_policy_bool_field(object, "worker") {
        errors.push(e);
    }
    match agent_policy_string_field(object, "audit_log") {
        Ok(Some(audit_log)) => {
            if options.project_confined {
                if let Err(e) = agent_policy_check_project_confined_path("audit_log", audit_log) {
                    errors.push(e);
                }
            }
            notes.push(format!(
                "audit_log={}",
                agent_policy_path_arg(base, audit_log)
            ));
        }
        Ok(None) => errors.push("policy field \"audit_log\" is required".to_string()),
        Err(e) => errors.push(e),
    }
    match agent_policy_string_field(object, "run_id") {
        Ok(Some(run_id)) => {
            if !agent_validate_run_id(run_id) {
                errors.push(format!(
                    "run_id must be 1-128 chars using ASCII letters, digits, dot, underscore, colon, slash, or hyphen: {run_id:?}"
                ));
            }
        }
        Ok(None) => {}
        Err(e) => errors.push(e),
    }
    if let Some(context) = object.get("context") {
        if context.as_object().is_none() {
            errors.push("policy field \"context\" must be an object".to_string());
        }
    }
    if let Some(state) = object.get("state") {
        if state.as_object().is_none() {
            errors.push("policy field \"state\" must be an object".to_string());
        }
    }
    match agent_policy_string_field(object, "session_file") {
        Ok(Some(session_file)) => {
            if options.project_confined {
                if let Err(e) =
                    agent_policy_check_project_confined_path("session_file", session_file)
                {
                    errors.push(e);
                }
            }
        }
        Ok(None) => {}
        Err(e) => errors.push(e),
    }
    match agent_policy_string_field(object, "osv_fixture") {
        Ok(Some(osv_fixture)) => {
            if options.project_confined {
                if let Err(e) = agent_policy_check_project_confined_path("osv_fixture", osv_fixture)
                {
                    errors.push(e);
                }
            }
            let resolved = agent_policy_path_arg(base, osv_fixture);
            if !std::path::Path::new(&resolved).exists() {
                errors.push(format!("osv_fixture path does not exist: {resolved}"));
            } else if let Err(e) = agent_load_osv_fixture(&resolved) {
                errors.push(e);
            }
        }
        Ok(None) => {}
        Err(e) => errors.push(e),
    }
    match agent_policy_string_field(object, "model_fixture") {
        Ok(Some(model_fixture)) => {
            if options.project_confined {
                if let Err(e) =
                    agent_policy_check_project_confined_path("model_fixture", model_fixture)
                {
                    errors.push(e);
                }
            }
            let resolved = agent_policy_path_arg(base, model_fixture);
            if !std::path::Path::new(&resolved).exists() {
                errors.push(format!("model_fixture path does not exist: {resolved}"));
            } else if let Err(e) = agent_load_model_fixture(&resolved) {
                errors.push(e);
            }
        }
        Ok(None) => {}
        Err(e) => errors.push(e),
    }
    match agent_policy_string_field(object, "named_network_cache_dir") {
        Ok(Some(named_network_cache_dir)) => {
            if options.project_confined {
                if let Err(e) = agent_policy_check_project_confined_path(
                    "named_network_cache_dir",
                    named_network_cache_dir,
                ) {
                    errors.push(e);
                }
            }
        }
        Ok(None) => {}
        Err(e) => errors.push(e),
    }
    match agent_policy_string_field(object, "named_network_cache_mode") {
        Ok(Some(mode)) => {
            if mode != "read-through" && mode != "offline" {
                errors.push(
                    "named_network_cache_mode must be \"read-through\" or \"offline\"".to_string(),
                );
            }
        }
        Ok(None) => {}
        Err(e) => errors.push(e),
    }
    if let Err(e) = agent_policy_u64_field(object, "named_network_cache_max_age_ms") {
        errors.push(e);
    }
    match agent_policy_u64_field(object, "named_network_cache_max_entries") {
        Ok(Some(0)) => {
            errors.push("named_network_cache_max_entries must be a positive integer".to_string())
        }
        Ok(_) => {}
        Err(e) => errors.push(e),
    }
    let has_named_network_cache_dir = matches!(
        agent_policy_string_field(object, "named_network_cache_dir"),
        Ok(Some(_))
    );
    if !has_named_network_cache_dir
        && (matches!(
            agent_policy_string_field(object, "named_network_cache_mode"),
            Ok(Some(_))
        ) || matches!(
            agent_policy_u64_field(object, "named_network_cache_max_age_ms"),
            Ok(Some(_))
        ) || matches!(
            agent_policy_u64_field(object, "named_network_cache_max_entries"),
            Ok(Some(_))
        ))
    {
        errors.push(
            "named_network_cache_mode, named_network_cache_max_age_ms, and named_network_cache_max_entries require named_network_cache_dir"
                .to_string(),
        );
    }
    match agent_policy_u64_field(object, "named_network_retry_attempts") {
        Ok(Some(n)) if n > 3 => {
            errors.push("named_network_retry_attempts must be between 0 and 3".to_string());
        }
        Ok(_) => {}
        Err(e) => errors.push(e),
    }
    match agent_policy_string_field(object, "github_token_env") {
        Ok(Some(key)) => {
            if !agent_env_key_is_valid(key) {
                errors.push(format!(
                    "github_token_env must be a non-empty ASCII env key: {key:?}"
                ));
            }
        }
        Ok(None) => {}
        Err(e) => errors.push(e),
    }
    match agent_policy_string_array_field(object, "secrets") {
        Ok(secrets) => {
            if options.strict {
                let mut seen = std::collections::HashSet::new();
                for secret in &secrets {
                    if !seen.insert(secret.as_str()) {
                        errors.push(format!("duplicate secrets entry {secret:?}"));
                    }
                }
            }
            let tool_set = agent_policy_string_array_field(object, "tools").unwrap_or_default();
            for secret in secrets {
                match agent_secret_scope_parse(&secret) {
                    Ok((tool, _env)) => {
                        if !tool_set.iter().any(|t| t == &tool) {
                            errors.push(format!(
                                "secret scope {tool:?} requires the same tool in policy field \"tools\""
                            ));
                        }
                    }
                    Err(e) => errors.push(e),
                }
            }
        }
        Err(e) => errors.push(e),
    }
    match agent_policy_string_field(object, "approval_log") {
        Ok(Some(path)) => {
            if options.project_confined {
                if let Err(e) = agent_policy_check_project_confined_path("approval_log", path) {
                    errors.push(e);
                }
            }
        }
        Ok(None) => {}
        Err(e) => errors.push(e),
    }
    if let Err(e) = agent_policy_u64_field(object, "approval_max_age_ms") {
        errors.push(e);
    }
    match agent_policy_string_array_field(object, "tools") {
        Ok(tools) => {
            if options.strict {
                let mut seen = std::collections::HashSet::new();
                for tool in &tools {
                    if !seen.insert(tool.as_str()) {
                        errors.push(format!("duplicate tool {tool:?}"));
                    }
                }
            }
            for tool in tools {
                if matches!(tool.as_str(), "shell" | "exec" | "spawn") {
                    errors.push(format!(
                        "tool {tool:?} requires process_tool_supervisor_required_not_available"
                    ));
                } else if !is_agent_builtin_tool_specifier(&tool) {
                    errors.push(format!(
                        "unknown tool {tool:?}; run `cruft agent tool list`"
                    ));
                }
            }
        }
        Err(e) => errors.push(e),
    }
    for field in ["approval_required_tools", "approved_tools"] {
        match agent_policy_string_array_field(object, field) {
            Ok(tools) => {
                if options.strict {
                    let mut seen = std::collections::HashSet::new();
                    for tool in &tools {
                        if !seen.insert(tool.as_str()) {
                            errors.push(format!("duplicate {field} entry {tool:?}"));
                        }
                    }
                }
                for tool in tools {
                    if !is_agent_builtin_tool_specifier(&tool) {
                        errors.push(format!(
                            "unknown {field} entry {tool:?}; run `cruft agent tool list`"
                        ));
                    }
                }
            }
            Err(e) => errors.push(e),
        }
    }
    let process_commands_for_validation =
        agent_policy_string_array_field(object, "process_commands").unwrap_or_default();
    let process_cwds_for_validation =
        agent_policy_string_array_field(object, "process_cwd").unwrap_or_default();
    if agent_policy_string_array_field(object, "tools")
        .map(|tools| tools.iter().any(|tool| tool == "process"))
        .unwrap_or(false)
        && (process_commands_for_validation.is_empty() || process_cwds_for_validation.is_empty())
    {
        errors.push(
            "tool \"process\" requires non-empty process_commands and process_cwd policy fields"
                .to_string(),
        );
    }
    match agent_policy_string_array_field(object, "fs_read") {
        Ok(paths) => {
            for path in paths {
                if options.project_confined {
                    if let Err(e) = agent_policy_check_project_confined_path("fs_read", &path) {
                        errors.push(e);
                    }
                }
                let resolved = agent_policy_path_arg(base, &path);
                if !std::path::Path::new(&resolved).exists() {
                    errors.push(format!("fs_read path does not exist: {resolved}"));
                }
            }
        }
        Err(e) => errors.push(e),
    }
    for field in ["fs_read_include", "fs_read_exclude"] {
        match agent_policy_string_array_field(object, field) {
            Ok(patterns) => {
                for pattern in patterns {
                    if !agent_validate_fs_read_pattern(&pattern) {
                        errors.push(format!(
                            "{field} pattern must be non-empty relative glob without parent segments: {pattern:?}"
                        ));
                    }
                }
            }
            Err(e) => errors.push(e),
        }
    }
    match agent_policy_string_array_field(object, "fs_write") {
        Ok(paths) => {
            for path in paths {
                if options.project_confined {
                    if let Err(e) = agent_policy_check_project_confined_path("fs_write", &path) {
                        errors.push(e);
                    }
                }
                let resolved = agent_policy_path_arg(base, &path);
                let p = std::path::Path::new(&resolved);
                if p.exists() && !p.is_dir() {
                    errors.push(format!("fs_write path is not a directory: {resolved}"));
                } else if !p.exists() && p.parent().map(|parent| !parent.exists()).unwrap_or(false)
                {
                    errors.push(format!("fs_write parent path does not exist: {resolved}"));
                }
            }
        }
        Err(e) => errors.push(e),
    }
    match agent_policy_string_array_field(object, "process_commands") {
        Ok(commands) => {
            for command in commands {
                let Some((name, raw_path)) = command.split_once('=') else {
                    errors.push("process_commands entries must be name=path".to_string());
                    continue;
                };
                if name.is_empty() || raw_path.is_empty() {
                    errors.push(
                        "process_commands entries require non-empty name and path".to_string(),
                    );
                    continue;
                }
                if options.project_confined {
                    if let Err(e) =
                        agent_policy_check_project_confined_path("process_commands", raw_path)
                    {
                        errors.push(e);
                    }
                }
                let path = agent_policy_path_arg(base, raw_path);
                if !std::path::Path::new(&path).exists() {
                    errors.push(format!(
                        "process_commands {name:?} path does not exist: {path}"
                    ));
                }
            }
        }
        Err(e) => errors.push(e),
    }
    match agent_policy_string_array_field(object, "process_cwd") {
        Ok(paths) => {
            for raw_path in paths {
                if options.project_confined {
                    if let Err(e) =
                        agent_policy_check_project_confined_path("process_cwd", &raw_path)
                    {
                        errors.push(e);
                    }
                }
                let path = agent_policy_path_arg(base, &raw_path);
                if !std::path::Path::new(&path).is_dir() {
                    errors.push(format!("process_cwd path is not a directory: {path}"));
                }
            }
        }
        Err(e) => errors.push(e),
    }
    match agent_policy_string_array_field(object, "process_env") {
        Ok(envs) => {
            for env in envs {
                let Some((key, _)) = env.split_once('=') else {
                    errors.push("process_env entries must be KEY=value".to_string());
                    continue;
                };
                if key.is_empty()
                    || key.contains('\0')
                    || !key.chars().all(|c| c == '_' || c.is_ascii_alphanumeric())
                {
                    errors.push(format!("process_env key must be an ASCII env key: {key:?}"));
                }
            }
        }
        Err(e) => errors.push(e),
    }
    match agent_policy_string_map_field(object, "expected_events") {
        Ok(expected_events) => {
            for (kind, raw_path) in expected_events {
                if kind.is_empty() {
                    errors.push("expected_events kind must be non-empty".to_string());
                }
                if options.project_confined {
                    if let Err(e) =
                        agent_policy_check_project_confined_path("expected_events", &raw_path)
                    {
                        errors.push(e);
                    }
                }
                let path = agent_policy_path_arg(base, &raw_path);
                if !std::path::Path::new(&path).exists() {
                    errors.push(format!(
                        "expected_events {kind:?} path does not exist: {path}"
                    ));
                } else if let Err(e) = agent_load_event_schema(&kind, &path) {
                    errors.push(e);
                }
            }
        }
        Err(e) => errors.push(e),
    }
    if let Some(budgets) = object.get("budgets") {
        if let Some(budgets) = budgets.as_object() {
            if options.strict {
                agent_policy_push_duplicate_key_errors(budgets, "budgets", &mut errors);
            }
            for key in budgets.keys() {
                if options.strict && !agent_policy_allowed_budget_key(key) {
                    errors.push(format!("unknown budget key {key:?}"));
                    continue;
                }
                if let Err(e) = agent_policy_u64_field(budgets, key) {
                    errors.push(e);
                }
            }
        } else {
            errors.push("policy field \"budgets\" must be an object".to_string());
        }
    }
    match agent_policy_string_map_field(object, "modules") {
        Ok(modules) => {
            for (specifier, raw_path) in modules {
                if options.project_confined {
                    if let Err(e) = agent_policy_check_project_confined_path("module", &raw_path) {
                        errors.push(format!("module {specifier:?}: {e}"));
                    }
                }
                let path = agent_policy_path_arg(base, &raw_path);
                if !std::path::Path::new(&path).exists() {
                    errors.push(format!("module {specifier:?} path does not exist: {path}"));
                }
            }
        }
        Err(e) => errors.push(e),
    }
    let package_integrities = match agent_policy_string_map_field(object, "package_integrity") {
        Ok(values) => values
            .into_iter()
            .collect::<std::collections::HashMap<_, _>>(),
        Err(e) => {
            errors.push(e);
            std::collections::HashMap::new()
        }
    };
    match agent_policy_string_map_field(object, "packages") {
        Ok(packages) => {
            for (specifier, raw_path) in packages {
                if options.project_confined {
                    if let Err(e) = agent_policy_check_project_confined_path("package", &raw_path) {
                        errors.push(format!("package {specifier:?}: {e}"));
                    }
                }
                let path = agent_policy_path_arg(base, &raw_path);
                let Some(expected) = package_integrities.get(&specifier) else {
                    errors.push(format!("package {specifier:?} missing package_integrity"));
                    continue;
                };
                match agent_integrity_for_path("package", Some(&specifier), &path) {
                    Ok(actual) if &actual == expected => {}
                    Ok(actual) => errors.push(format!(
                        "package {specifier:?} integrity mismatch: expected {expected}, actual {actual}"
                    )),
                    Err(e) => errors.push(format!("package {specifier:?}: {e}")),
                }
            }
        }
        Err(e) => errors.push(e),
    }
    let hook_integrities = match agent_policy_string_map_field(object, "import_hook_integrity") {
        Ok(values) => values
            .into_iter()
            .collect::<std::collections::HashMap<_, _>>(),
        Err(e) => {
            errors.push(e);
            std::collections::HashMap::new()
        }
    };
    match agent_policy_string_map_field(object, "import_hooks") {
        Ok(hooks) => {
            for (specifier, raw_path) in hooks {
                if options.project_confined {
                    if let Err(e) =
                        agent_policy_check_project_confined_path("import hook", &raw_path)
                    {
                        errors.push(format!("import hook {specifier:?}: {e}"));
                    }
                }
                let path = agent_policy_path_arg(base, &raw_path);
                let Some(expected) = hook_integrities.get(&specifier) else {
                    errors.push(format!(
                        "import hook {specifier:?} missing import_hook_integrity"
                    ));
                    continue;
                };
                match agent_integrity_for_path("import-hook", Some(&specifier), &path) {
                    Ok(actual) if &actual == expected => {}
                    Ok(actual) => errors.push(format!(
                        "import hook {specifier:?} integrity mismatch: expected {expected}, actual {actual}"
                    )),
                    Err(e) => errors.push(format!("import hook {specifier:?}: {e}")),
                }
            }
        }
        Err(e) => errors.push(e),
    }
    if errors.is_empty() {
        Ok(notes)
    } else {
        Err(errors)
    }
}

fn agent_policy_validate_value(
    policy_path: &str,
    value: &serde_json::Value,
) -> Result<Vec<String>, Vec<String>> {
    agent_policy_validate_value_with_options(
        policy_path,
        value,
        AgentPolicyValidationOptions::default(),
    )
}

fn agent_policy_json_path_entry(
    out: &mut String,
    first: &mut bool,
    key: &str,
    base: &std::path::Path,
    raw: Option<&str>,
) {
    if let Some(raw) = raw {
        if !*first {
            out.push_str(",\n");
        }
        *first = false;
        let resolved = agent_policy_path_arg(base, raw);
        out.push_str(&format!(
            "    \"{}\": {{\"raw\": \"{}\", \"resolved\": \"{}\"}}",
            json_escape(key),
            json_escape(raw),
            json_escape(&resolved)
        ));
    }
}

fn agent_policy_json_path_map(
    out: &mut String,
    first: &mut bool,
    key: &str,
    base: &std::path::Path,
    object: &serde_json::Map,
) {
    let Some(value) = object.get(key).and_then(|v| v.as_object()) else {
        return;
    };
    if !*first {
        out.push_str(",\n");
    }
    *first = false;
    out.push_str(&format!("    \"{}\": [", json_escape(key)));
    let mut entry_first = true;
    for (specifier, raw_path) in value {
        let Some(raw_path) = raw_path.as_str() else {
            continue;
        };
        if !entry_first {
            out.push_str(", ");
        }
        entry_first = false;
        let resolved = agent_policy_path_arg(base, raw_path);
        out.push_str(&format!(
            "{{\"specifier\": \"{}\", \"raw\": \"{}\", \"resolved\": \"{}\"}}",
            json_escape(specifier),
            json_escape(raw_path),
            json_escape(&resolved)
        ));
    }
    out.push(']');
}

fn agent_policy_explain_json(
    policy_path: &str,
    value: &serde_json::Value,
    validation: &Result<Vec<String>, Vec<String>>,
) -> String {
    let object = value.as_object().expect("policy object validated at load");
    let base = std::path::Path::new(policy_path)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let count_array = |key: &str| {
        object
            .get(key)
            .and_then(|v| v.as_array())
            .map(|v| v.len())
            .unwrap_or(0)
    };
    let count_object = |key: &str| {
        object
            .get(key)
            .and_then(|v| v.as_object())
            .map(|v| v.len())
            .unwrap_or(0)
    };

    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"type\": \"agent_policy_explain\",\n");
    out.push_str(&format!(
        "  \"policy\": \"{}\",\n",
        json_escape(policy_path)
    ));
    out.push_str("  \"authority_counts\": {\n");
    out.push_str(&format!(
        "    \"tools\": {},\n    \"approval_required_tools\": {},\n    \"approved_tools\": {},\n    \"modules\": {},\n    \"packages\": {},\n    \"import_hooks\": {},\n    \"budgets\": {},\n    \"package_integrity\": {},\n    \"import_hook_integrity\": {}\n",
        count_array("tools"),
        count_array("approval_required_tools"),
        count_array("approved_tools"),
        count_object("modules"),
        count_object("packages"),
        count_object("import_hooks"),
        count_object("budgets"),
        count_object("package_integrity"),
        count_object("import_hook_integrity")
    ));
    out.push_str("  },\n");
    out.push_str("  \"resolved_paths\": {\n");
    let mut first = true;
    agent_policy_json_path_entry(
        &mut out,
        &mut first,
        "agent",
        base,
        agent_policy_string_field(object, "agent").ok().flatten(),
    );
    agent_policy_json_path_entry(
        &mut out,
        &mut first,
        "audit_log",
        base,
        agent_policy_string_field(object, "audit_log")
            .ok()
            .flatten(),
    );
    agent_policy_json_path_entry(
        &mut out,
        &mut first,
        "session_file",
        base,
        agent_policy_string_field(object, "session_file")
            .ok()
            .flatten(),
    );
    agent_policy_json_path_map(&mut out, &mut first, "modules", base, object);
    agent_policy_json_path_map(&mut out, &mut first, "packages", base, object);
    agent_policy_json_path_map(&mut out, &mut first, "import_hooks", base, object);
    if !first {
        out.push('\n');
    }
    out.push_str("  },\n");
    match validation {
        Ok(notes) => {
            out.push_str("  \"validation\": {\"valid\": true, \"errors\": [], \"notes\": [");
            for (i, note) in notes.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&json_string_literal(note));
            }
            out.push_str("]},\n");
        }
        Err(errors) => {
            out.push_str("  \"validation\": {\"valid\": false, \"errors\": [");
            for (i, error) in errors.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&json_string_literal(error));
            }
            out.push_str("], \"notes\": []},\n");
        }
    }
    out.push_str("  \"non_claims\": [\"OS/process sandboxing\", \"process/shell/exec/spawn tools\", \"network policy\", \"arbitrary npm/lockfile graphs\", \"direct allocator pre-kill\", \"external async/process cancellation\"],\n");
    out.push_str("  \"doctor_control_snapshot\": ");
    out.push_str(agent_doctor_json());
    out.push_str("\n}\n");
    out
}

fn agent_policy_risk_level(score: u64, valid: bool) -> &'static str {
    if !valid {
        "rejected"
    } else if score >= 9 {
        "privileged"
    } else if score >= 5 {
        "effectful"
    } else if score >= 2 {
        "bounded"
    } else {
        "minimal"
    }
}

fn agent_policy_tool_authority(tool: &str) -> &'static str {
    match tool {
        "echo" | "fail" | "slow" => "none",
        "readFile" | "listFiles" => "fs-read",
        "writeArtifact" => "fs-write",
        "osv.query"
        | "npm.metadata"
        | "github.issue.read"
        | "github.pr.read"
        | "github.pr.files.list"
        | "github.release.latest.read"
        | "github.file.read"
        | "github.compare.read"
        | "github.commit.read"
        | "github.repo.read"
        | "github.workflow.run.read"
        | "github.workflow.jobs.list"
        | "github.check.runs.list" => "named-network",
        "model.call" => "model",
        "process" => "process",
        _ => "unknown",
    }
}

fn agent_policy_risk_json(
    policy_path: &str,
    value: &serde_json::Value,
    validation: &Result<Vec<String>, Vec<String>>,
) -> String {
    let object = value.as_object().expect("policy object validated at load");
    let tools = object
        .get("tools")
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let tool_count = tools.len() as u64;
    let named_network_tools = tools
        .iter()
        .filter(|tool| agent_policy_tool_authority(tool) == "named-network")
        .count() as u64;
    let model_tools = tools
        .iter()
        .filter(|tool| agent_policy_tool_authority(tool) == "model")
        .count() as u64;
    let process_tools = tools
        .iter()
        .filter(|tool| agent_policy_tool_authority(tool) == "process")
        .count() as u64;
    let fs_read_tools = tools
        .iter()
        .filter(|tool| agent_policy_tool_authority(tool) == "fs-read")
        .count() as u64;
    let fs_write_tools = tools
        .iter()
        .filter(|tool| agent_policy_tool_authority(tool) == "fs-write")
        .count() as u64;
    let modules = object
        .get("modules")
        .and_then(|value| value.as_object())
        .map(|map| map.len() as u64)
        .unwrap_or(0);
    let packages = object
        .get("packages")
        .and_then(|value| value.as_object())
        .map(|map| map.len() as u64)
        .unwrap_or(0);
    let import_hooks = object
        .get("import_hooks")
        .and_then(|value| value.as_object())
        .map(|map| map.len() as u64)
        .unwrap_or(0);
    let budgets = object
        .get("budgets")
        .and_then(|value| value.as_object())
        .map(|map| map.len() as u64)
        .unwrap_or(0);
    let approvals = object
        .get("approval_required_tools")
        .and_then(|value| value.as_array())
        .map(|values| values.len() as u64)
        .unwrap_or(0);
    let secrets = object
        .get("secrets")
        .and_then(|value| value.as_object())
        .map(|map| map.len() as u64)
        .unwrap_or(0);
    let session = agent_policy_string_field(object, "session_file")
        .ok()
        .flatten()
        .is_some();
    let worker = agent_policy_bool_field(object, "worker").unwrap_or(false);
    let score = fs_read_tools
        + (fs_write_tools * 2)
        + (named_network_tools * 2)
        + (model_tools * 3)
        + (process_tools * 4)
        + modules
        + (packages * 2)
        + (import_hooks * 2)
        + secrets
        + if session { 1 } else { 0 }
        + if worker { 1 } else { 0 }
        + if approvals > 0 {
            0
        } else if tool_count > 0 {
            1
        } else {
            0
        };
    let valid = validation.is_ok();
    let risk = agent_policy_risk_level(score, valid);
    let validation_json = match validation {
        Ok(notes) => format!(
            "{{\"valid\":true,\"errors\":[],\"notes\":{}}}",
            json_string_array(notes)
        ),
        Err(errors) => format!(
            "{{\"valid\":false,\"errors\":{},\"notes\":[]}}",
            json_string_array(errors)
        ),
    };
    format!(
        "{{\n  \"type\": \"agent_policy_risk\",\n  \"policy\": {},\n  \"valid\": {},\n  \"risk\": {},\n  \"score\": {},\n  \"authority\": {{\"tools\": {}, \"fs_read_tools\": {}, \"fs_write_tools\": {}, \"named_network_tools\": {}, \"model_tools\": {}, \"process_tools\": {}, \"modules\": {}, \"packages\": {}, \"import_hooks\": {}, \"secrets\": {}, \"worker\": {}, \"session\": {}, \"approval_required_tools\": {}, \"budget_fields\": {}}},\n  \"explanation\": {},\n  \"validation\": {},\n  \"nonclaims\": {}\n}}\n",
        json_string_literal(policy_path),
        if valid { "true" } else { "false" },
        json_string_literal(risk),
        score,
        tool_count,
        fs_read_tools,
        fs_write_tools,
        named_network_tools,
        model_tools,
        process_tools,
        modules,
        packages,
        import_hooks,
        secrets,
        if worker { "true" } else { "false" },
        if session { "true" } else { "false" },
        approvals,
        budgets,
        json_string_literal(&format!(
            "{risk} policy: process/model/named-network/fs/module/package/import-hook/session authorities increase score; approval requirements lower review posture but do not grant authority"
        )),
        validation_json,
        json_str_array(&[
            "risk is advisory and never bypasses policy validation",
            "no OS/process sandbox claim",
            "no general network claim outside named tools",
            "no direct allocator pre-kill claim",
        ])
    )
}

fn print_agent_policy_risk_human(
    policy_path: &str,
    value: &serde_json::Value,
    validation: &Result<Vec<String>, Vec<String>>,
) {
    let risk_json = agent_policy_risk_json(policy_path, value, validation);
    let parsed: serde_json::Value =
        serde_json::from_str(&risk_json).expect("agent policy risk json is internal");
    let object = parsed.as_object().expect("agent policy risk json object");
    println!(
        "policy risk: {}",
        object.get("risk").unwrap().as_str().unwrap()
    );
    println!("policy: {policy_path}");
    println!("score: {}", object.get("score").unwrap().as_u64().unwrap());
    println!("valid: {}", object.get("valid").unwrap().as_bool().unwrap());
    let authority = object.get("authority").unwrap().as_object().unwrap();
    for key in [
        "tools",
        "fs_read_tools",
        "fs_write_tools",
        "named_network_tools",
        "model_tools",
        "process_tools",
        "modules",
        "packages",
        "import_hooks",
        "secrets",
        "approval_required_tools",
        "budget_fields",
    ] {
        println!(
            "{key}: {}",
            authority.get(key).unwrap().as_u64().unwrap_or(0)
        );
    }
    println!(
        "worker: {}",
        authority.get("worker").unwrap().as_bool().unwrap()
    );
    println!(
        "session: {}",
        authority.get("session").unwrap().as_bool().unwrap()
    );
    println!(
        "explanation: {}",
        object.get("explanation").unwrap().as_str().unwrap()
    );
}

fn json_string_array(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| json_string_literal(value))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn json_str_array(values: &[&str]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| json_string_literal(value))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

pub(crate) fn run_agent_policy_subcommand(args: &[String]) -> ExitCode {
    let Some(action) = args.first().map(|s| s.as_str()) else {
        eprintln!("cruft agent policy: usage: cruft agent policy validate [--strict] [--project-confined] <project|agent-policy.json> | cruft agent policy explain [--json] <project|agent-policy.json> | cruft agent policy risk [--json|--human] <project|agent-policy.json> | cruft agent policy diff [--json] <project|agent-policy.json>");
        return ExitCode::from(64);
    };
    let mut strict = false;
    let mut project_confined = false;
    let mut json = false;
    let mut target: Option<&String> = None;
    for arg in &args[1..] {
        if *arg == "--strict" && action == "validate" {
            strict = true;
        } else if *arg == "--project-confined" && action == "validate" {
            project_confined = true;
        } else if *arg == "--json" && matches!(action, "explain" | "diff" | "risk") {
            json = true;
        } else if *arg == "--human" && action == "risk" {
            json = false;
        } else if target.is_none() {
            target = Some(arg);
        } else {
            eprintln!("cruft agent policy {action}: unexpected argument {arg}");
            return ExitCode::from(64);
        }
    }
    let Some(target) = target else {
        eprintln!("cruft agent policy {action}: missing <project|agent-policy.json>");
        return ExitCode::from(64);
    };
    let (policy_path, source, value) = match agent_policy_load_target(target) {
        Ok(policy) => policy,
        Err(e) => {
            eprintln!("cruft agent policy {action}: {e}");
            return ExitCode::from(65);
        }
    };
    match action {
        "validate" => match agent_policy_validate_value_with_options(
            &policy_path,
            &value,
            AgentPolicyValidationOptions {
                strict,
                project_confined,
            },
        ) {
            Ok(notes) => {
                println!("policy valid: {policy_path}");
                if strict {
                    println!("profile: strict");
                }
                if project_confined {
                    println!("profile: project-confined");
                }
                for note in notes {
                    println!("{note}");
                }
                ExitCode::SUCCESS
            }
            Err(errors) => {
                eprintln!("policy invalid: {policy_path}");
                for error in errors {
                    eprintln!("{error}");
                }
                ExitCode::from(65)
            }
        },
        "explain" => {
            let validation = agent_policy_validate_value(&policy_path, &value);
            if json {
                print!(
                    "{}",
                    agent_policy_explain_json(&policy_path, &value, &validation)
                );
                return if validation.is_ok() {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(65)
                };
            }
            let object = value.as_object().expect("policy object validated at load");
            println!("policy: {policy_path}");
            if let Some(agent) = agent_policy_string_field(object, "agent").ok().flatten() {
                println!("agent: {agent}");
            }
            println!(
                "worker: {}",
                agent_policy_bool_field(object, "worker").unwrap_or(false)
            );
            for (key, label) in [
                ("tools", "tools"),
                ("modules", "modules"),
                ("packages", "packages"),
                ("import_hooks", "import hooks"),
            ] {
                if let Some(value) = object.get(key) {
                    if let Some(array) = value.as_array() {
                        println!("{label}: {} entries", array.len());
                    } else if let Some(map) = value.as_object() {
                        println!("{label}: {} entries", map.len());
                        for specifier in map.keys() {
                            println!("  - {specifier}");
                        }
                    }
                }
            }
            if let Some(budgets) = object.get("budgets").and_then(|v| v.as_object()) {
                println!("budgets: {} entries", budgets.len());
                for key in budgets.keys() {
                    println!("  - {key}");
                }
            }
            match validation {
                Ok(_) => println!("validation: valid"),
                Err(errors) => {
                    println!("validation: invalid");
                    for error in errors {
                        println!("  - {error}");
                    }
                    return ExitCode::from(65);
                }
            }
            ExitCode::SUCCESS
        }
        "risk" => {
            let validation = agent_policy_validate_value(&policy_path, &value);
            if json {
                print!(
                    "{}",
                    agent_policy_risk_json(&policy_path, &value, &validation)
                );
            } else {
                print_agent_policy_risk_human(&policy_path, &value, &validation);
            }
            if validation.is_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(65)
            }
        }
        "diff" => {
            let canonical = format!("{}\n", value.to_pretty_string());
            let clean = source == canonical;
            if json {
                println!(
                    "{{\n  \"type\": \"agent_policy_diff\",\n  \"policy\": \"{}\",\n  \"canonical\": {},\n  \"status\": \"{}\",\n  \"canonical_policy\": {}\n}}",
                    json_escape(&policy_path),
                    if clean { "true" } else { "false" },
                    if clean { "clean" } else { "differs" },
                    value.to_pretty_string()
                );
            } else if clean {
                println!("policy diff: no canonical normalization changes");
            } else {
                println!("policy diff: canonical normalization differs");
                println!("{canonical}");
            }
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("cruft agent policy: unsupported action {action:?}; available: validate, explain, risk, diff");
            ExitCode::from(64)
        }
    }
}

fn agent_policy_take_object(value: serde_json::Value) -> Result<serde_json::Map, String> {
    match value {
        serde_json::Value::Object(object) => Ok(object),
        _ => Err("policy root must be an object".to_string()),
    }
}

fn agent_policy_take_object_field(
    object: &serde_json::Map,
    key: &str,
) -> Result<serde_json::Map, String> {
    match object.get(key) {
        None | Some(serde_json::Value::Null) => Ok(serde_json::Map::new()),
        Some(serde_json::Value::Object(value)) => Ok(value.clone()),
        Some(_) => Err(format!("policy field {key:?} must be an object")),
    }
}

fn agent_policy_remove_object_key(object: serde_json::Map, key: &str) -> serde_json::Map {
    let mut next = serde_json::Map::new();
    for (entry_key, value) in object {
        if entry_key != key {
            next.insert(entry_key, value);
        }
    }
    next
}

fn agent_policy_take_array_field(
    object: &serde_json::Map,
    key: &str,
) -> Result<Vec<serde_json::Value>, String> {
    match object.get(key) {
        None | Some(serde_json::Value::Null) => Ok(Vec::new()),
        Some(serde_json::Value::Array(value)) => Ok(value.clone()),
        Some(_) => Err(format!("policy field {key:?} must be an array")),
    }
}

fn agent_policy_mutation_usage(command: &str) {
    match command {
        "add-tool" => {
            eprintln!("cruft agent add-tool: usage: cruft agent add-tool <project> <tool>")
        }
        "remove-tool" => {
            eprintln!("cruft agent remove-tool: usage: cruft agent remove-tool <project> <tool>")
        }
        "add-module" => eprintln!(
            "cruft agent add-module: usage: cruft agent add-module <project> <path> --specifier=<specifier>"
        ),
        "remove-module" => eprintln!(
            "cruft agent remove-module: usage: cruft agent remove-module <project> <specifier>"
        ),
        "add-package" => eprintln!(
            "cruft agent add-package: usage: cruft agent add-package <project> <specifier> <path>"
        ),
        "remove-package" => eprintln!(
            "cruft agent remove-package: usage: cruft agent remove-package <project> <specifier>"
        ),
        "add-import-hook" => eprintln!(
            "cruft agent add-import-hook: usage: cruft agent add-import-hook <project> <specifier> <path>"
        ),
        "remove-import-hook" => eprintln!(
            "cruft agent remove-import-hook: usage: cruft agent remove-import-hook <project> <specifier>"
        ),
        "set-context" => eprintln!(
            "cruft agent set-context: usage: cruft agent set-context <project> <json-object>"
        ),
        "unset-context" => {
            eprintln!("cruft agent unset-context: usage: cruft agent unset-context <project>")
        }
        "set-budget" => eprintln!(
            "cruft agent set-budget: usage: cruft agent set-budget <project> key=value [key=value ...]"
        ),
        "unset-budget" => eprintln!(
            "cruft agent unset-budget: usage: cruft agent unset-budget <project> <key> [key ...]"
        ),
        "set-session" => eprintln!(
            "cruft agent set-session: usage: cruft agent set-session <project> <session-file>"
        ),
        "unset-session" => {
            eprintln!("cruft agent unset-session: usage: cruft agent unset-session <project>")
        }
        "set-worker" => {
            eprintln!("cruft agent set-worker: usage: cruft agent set-worker <project> true|false")
        }
        "unset-worker" => {
            eprintln!("cruft agent unset-worker: usage: cruft agent unset-worker <project>")
        }
        _ => {}
    }
}

pub(crate) fn run_agent_policy_mutation_subcommand(command: &str, args: &[String]) -> ExitCode {
    if args.is_empty() {
        agent_policy_mutation_usage(command);
        return ExitCode::from(64);
    }
    let project_dir = &args[0];
    let (policy_path, policy) = match agent_policy_load_project(project_dir) {
        Ok(policy) => policy,
        Err(e) => {
            eprintln!("cruft agent {command}: {e}");
            return ExitCode::from(65);
        }
    };
    let result: Result<(String, serde_json::Map), String> = (|| {
        let mut object = agent_policy_take_object(policy)?;
        match command {
            "add-tool" => {
                let Some(tool) = args.get(1) else {
                    return Err("missing tool".to_string());
                };
                if args.len() != 2 {
                    return Err(format!("unexpected argument {}", args[2]));
                }
                let mut tools = agent_policy_take_array_field(&object, "tools")?;
                if !tools
                    .iter()
                    .any(|value| value.as_str() == Some(tool.as_str()))
                {
                    tools.push(serde_json::Value::String(tool.clone()));
                }
                object.insert("tools".to_string(), serde_json::Value::Array(tools));
                Ok((format!("tools += {tool}"), object))
            }
            "remove-tool" => {
                let Some(tool) = args.get(1) else {
                    return Err("missing tool".to_string());
                };
                if args.len() != 2 {
                    return Err(format!("unexpected argument {}", args[2]));
                }
                let mut tools = agent_policy_take_array_field(&object, "tools")?;
                tools.retain(|value| value.as_str() != Some(tool.as_str()));
                object.insert("tools".to_string(), serde_json::Value::Array(tools));
                Ok((format!("tools -= {tool}"), object))
            }
            "add-module" => {
                let Some(path) = args.get(1) else {
                    return Err("missing module path".to_string());
                };
                let mut specifier: Option<String> = None;
                let mut idx = 2;
                while idx < args.len() {
                    let arg = &args[idx];
                    if let Some(value) = arg.strip_prefix("--specifier=") {
                        specifier = Some(value.to_string());
                    } else if arg == "--specifier" {
                        idx += 1;
                        let Some(value) = args.get(idx) else {
                            return Err("--specifier requires an argument".to_string());
                        };
                        specifier = Some(value.clone());
                    } else {
                        return Err(format!("unexpected argument {arg}"));
                    }
                    idx += 1;
                }
                let Some(specifier) = specifier else {
                    return Err("add-module requires --specifier=<specifier>".to_string());
                };
                let mut modules = agent_policy_take_object_field(&object, "modules")?;
                modules.insert(specifier.clone(), serde_json::Value::String(path.clone()));
                object.insert("modules".to_string(), serde_json::Value::Object(modules));
                Ok((format!("modules.{specifier} = {path}"), object))
            }
            "remove-module" => {
                let Some(specifier) = args.get(1) else {
                    return Err("missing module specifier".to_string());
                };
                if args.len() != 2 {
                    return Err(format!("unexpected argument {}", args[2]));
                }
                let modules = agent_policy_take_object_field(&object, "modules")?;
                let modules = agent_policy_remove_object_key(modules, specifier);
                object.insert("modules".to_string(), serde_json::Value::Object(modules));
                Ok((format!("modules.{specifier} removed"), object))
            }
            "add-package" => {
                let Some(specifier) = args.get(1) else {
                    return Err("missing package specifier".to_string());
                };
                let Some(path) = args.get(2) else {
                    return Err("missing package path".to_string());
                };
                if args.len() != 3 {
                    return Err(format!("unexpected argument {}", args[3]));
                }
                let integrity = agent_integrity_for_path("package", Some(specifier), path)?;
                let mut packages = agent_policy_take_object_field(&object, "packages")?;
                packages.insert(specifier.clone(), serde_json::Value::String(path.clone()));
                object.insert("packages".to_string(), serde_json::Value::Object(packages));
                let mut package_integrity =
                    agent_policy_take_object_field(&object, "package_integrity")?;
                package_integrity.insert(
                    specifier.clone(),
                    serde_json::Value::String(integrity.clone()),
                );
                object.insert(
                    "package_integrity".to_string(),
                    serde_json::Value::Object(package_integrity),
                );
                Ok((
                    format!(
                        "packages.{specifier} = {path}; package_integrity.{specifier} = {integrity}"
                    ),
                    object,
                ))
            }
            "remove-package" => {
                let Some(specifier) = args.get(1) else {
                    return Err("missing package specifier".to_string());
                };
                if args.len() != 2 {
                    return Err(format!("unexpected argument {}", args[2]));
                }
                let packages = agent_policy_take_object_field(&object, "packages")?;
                let packages = agent_policy_remove_object_key(packages, specifier);
                object.insert("packages".to_string(), serde_json::Value::Object(packages));
                let package_integrity =
                    agent_policy_take_object_field(&object, "package_integrity")?;
                let package_integrity =
                    agent_policy_remove_object_key(package_integrity, specifier);
                object.insert(
                    "package_integrity".to_string(),
                    serde_json::Value::Object(package_integrity),
                );
                Ok((
                    format!("packages.{specifier} removed; package_integrity.{specifier} removed"),
                    object,
                ))
            }
            "add-import-hook" => {
                let Some(specifier) = args.get(1) else {
                    return Err("missing import-hook specifier".to_string());
                };
                let Some(path) = args.get(2) else {
                    return Err("missing import-hook path".to_string());
                };
                if args.len() != 3 {
                    return Err(format!("unexpected argument {}", args[3]));
                }
                let integrity = agent_integrity_for_path("import-hook", Some(specifier), path)?;
                let mut import_hooks = agent_policy_take_object_field(&object, "import_hooks")?;
                import_hooks.insert(specifier.clone(), serde_json::Value::String(path.clone()));
                object.insert(
                    "import_hooks".to_string(),
                    serde_json::Value::Object(import_hooks),
                );
                let mut import_hook_integrity =
                    agent_policy_take_object_field(&object, "import_hook_integrity")?;
                import_hook_integrity.insert(
                    specifier.clone(),
                    serde_json::Value::String(integrity.clone()),
                );
                object.insert(
                    "import_hook_integrity".to_string(),
                    serde_json::Value::Object(import_hook_integrity),
                );
                Ok((
                    format!(
                        "import_hooks.{specifier} = {path}; import_hook_integrity.{specifier} = {integrity}"
                    ),
                    object,
                ))
            }
            "remove-import-hook" => {
                let Some(specifier) = args.get(1) else {
                    return Err("missing import-hook specifier".to_string());
                };
                if args.len() != 2 {
                    return Err(format!("unexpected argument {}", args[2]));
                }
                let import_hooks = agent_policy_take_object_field(&object, "import_hooks")?;
                let import_hooks = agent_policy_remove_object_key(import_hooks, specifier);
                object.insert(
                    "import_hooks".to_string(),
                    serde_json::Value::Object(import_hooks),
                );
                let import_hook_integrity =
                    agent_policy_take_object_field(&object, "import_hook_integrity")?;
                let import_hook_integrity =
                    agent_policy_remove_object_key(import_hook_integrity, specifier);
                object.insert(
                    "import_hook_integrity".to_string(),
                    serde_json::Value::Object(import_hook_integrity),
                );
                Ok((
                    format!(
                        "import_hooks.{specifier} removed; import_hook_integrity.{specifier} removed"
                    ),
                    object,
                ))
            }
            "set-context" => {
                let Some(context) = args.get(1) else {
                    return Err("missing context JSON".to_string());
                };
                if args.len() != 2 {
                    return Err(format!("unexpected argument {}", args[2]));
                }
                let value = serde_json::from_str::<serde_json::Value>(context)
                    .map_err(|e| format!("cannot parse context JSON: {e}"))?;
                if value.as_object().is_none() {
                    return Err("context must be a JSON object".to_string());
                }
                object.insert("context".to_string(), value);
                Ok(("context = <json>".to_string(), object))
            }
            "unset-context" => {
                if args.len() != 1 {
                    return Err(format!("unexpected argument {}", args[1]));
                }
                object = agent_policy_remove_object_key(object, "context");
                Ok(("context removed".to_string(), object))
            }
            "set-budget" => {
                if args.len() < 2 {
                    return Err("missing budget key=value".to_string());
                }
                let mut budgets = agent_policy_take_object_field(&object, "budgets")?;
                let mut changed = Vec::new();
                for item in &args[1..] {
                    let Some((key, value)) = item.split_once('=') else {
                        return Err(format!("budget entry {item:?} must be key=value"));
                    };
                    let n = value
                        .parse::<u64>()
                        .ok()
                        .filter(|n| *n > 0)
                        .ok_or_else(|| format!("budget {key:?} must be a positive integer"))?;
                    budgets.insert(key.to_string(), serde_json::to_value(n));
                    changed.push(format!("{key}={n}"));
                }
                object.insert("budgets".to_string(), serde_json::Value::Object(budgets));
                Ok((format!("budgets {}", changed.join(" ")), object))
            }
            "unset-budget" => {
                if args.len() < 2 {
                    return Err("missing budget key".to_string());
                }
                let mut budgets = agent_policy_take_object_field(&object, "budgets")?;
                for key in &args[1..] {
                    budgets = agent_policy_remove_object_key(budgets, key);
                }
                object.insert("budgets".to_string(), serde_json::Value::Object(budgets));
                Ok((format!("budgets removed {}", args[1..].join(" ")), object))
            }
            "set-session" => {
                let Some(path) = args.get(1) else {
                    return Err("missing session file".to_string());
                };
                if args.len() != 2 {
                    return Err(format!("unexpected argument {}", args[2]));
                }
                object.insert(
                    "session_file".to_string(),
                    serde_json::Value::String(path.clone()),
                );
                Ok((format!("session_file = {path}"), object))
            }
            "unset-session" => {
                if args.len() != 1 {
                    return Err(format!("unexpected argument {}", args[1]));
                }
                object = agent_policy_remove_object_key(object, "session_file");
                Ok(("session_file removed".to_string(), object))
            }
            "set-worker" => {
                let Some(value) = args.get(1) else {
                    return Err("missing worker value".to_string());
                };
                if args.len() != 2 {
                    return Err(format!("unexpected argument {}", args[2]));
                }
                let value = match value.as_str() {
                    "true" => true,
                    "false" => false,
                    _ => return Err("worker value must be true or false".to_string()),
                };
                object.insert("worker".to_string(), serde_json::Value::Bool(value));
                Ok((format!("worker = {value}"), object))
            }
            "unset-worker" => {
                if args.len() != 1 {
                    return Err(format!("unexpected argument {}", args[1]));
                }
                object = agent_policy_remove_object_key(object, "worker");
                Ok(("worker removed".to_string(), object))
            }
            _ => Err(format!("unknown mutation command {command}")),
        }
    })();
    let (summary, object) = match result {
        Ok(result) => result,
        Err(e) => {
            agent_policy_mutation_usage(command);
            eprintln!("cruft agent {command}: {e}");
            return ExitCode::from(64);
        }
    };
    let policy = serde_json::Value::Object(object);
    if let Err(e) = agent_policy_write(&policy_path, &policy) {
        eprintln!("cruft agent {command}: {e}");
        return ExitCode::from(73);
    }
    println!("updated {policy_path}");
    println!("{summary}");
    ExitCode::SUCCESS
}
