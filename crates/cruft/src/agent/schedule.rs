use super::integrity::agent_source_hash;
use super::policy::{
    agent_policy_load_target, agent_policy_path_arg, agent_policy_string_field,
    agent_policy_validate_value_with_options, AgentPolicyValidationOptions,
};
use crate::json_string_literal;
use std::process::ExitCode;

const SCHEDULER_SCHEMA_VERSION: u64 = 1;

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn scheduler_usage() {
    eprintln!(
        "cruft agent schedule: usage: cruft agent schedule start --project <dir>|--policy <agent-policy.json> [--store <dir>] [--job-id=<id>] | cruft agent schedule tick <job-id> --store <dir> | cruft agent schedule input <job-id> --store <dir> --token <token> --json <json> | cruft agent schedule cancel <job-id> --store <dir> | cruft agent schedule replay <job-id> --store <dir> | cruft agent schedule status <job-id> [--store <dir>] | cruft agent schedule list [--store <dir>]"
    );
}

fn validate_scheduler_job_id(job_id: &str) -> bool {
    !job_id.is_empty()
        && job_id.len() <= 128
        && !job_id.split('/').any(|part| part == "." || part == "..")
        && job_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'))
}

fn generated_job_id(policy_hash: &str, agent_hash: &str) -> String {
    let payload = format!(
        "agent-scheduler-job:v1\n{}\n{}\n{}\n{}",
        std::process::id(),
        now_ms(),
        policy_hash,
        agent_hash
    );
    agent_source_hash(&payload)
        .strip_prefix("fnv1a64:")
        .map(|hash| format!("job-{hash}"))
        .unwrap_or_else(|| format!("job-{}", now_ms()))
}

fn default_store_for_policy(policy_path: &str) -> String {
    let policy = std::path::Path::new(policy_path);
    let base = policy.parent().unwrap_or_else(|| std::path::Path::new("."));
    base.join(".cruft")
        .join("agent-scheduler")
        .display()
        .to_string()
}

fn scheduler_manifest_path(store: &str, job_id: &str) -> std::path::PathBuf {
    std::path::Path::new(store)
        .join("jobs")
        .join(job_id)
        .join("manifest.json")
}

fn scheduler_events_path(store: &str, job_id: &str) -> std::path::PathBuf {
    std::path::Path::new(store)
        .join("jobs")
        .join(job_id)
        .join("events.jsonl")
}

fn append_scheduler_event(
    store: &str,
    job_id: &str,
    event: &str,
    fields: &str,
) -> Result<(), String> {
    let path = scheduler_events_path(store, job_id);
    let parent = path
        .parent()
        .ok_or_else(|| format!("scheduler events path has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    let suffix = if fields.is_empty() {
        String::new()
    } else {
        format!(",{fields}")
    };
    let line = format!(
        "{{\"type\":\"agent_scheduler_event\",\"schema_version\":{},\"job_id\":{},\"event\":{},\"at_ms\":{}{} }}\n",
        SCHEDULER_SCHEMA_VERSION,
        json_string_literal(job_id),
        json_string_literal(event),
        now_ms(),
        suffix
    );
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    file.write_all(line.as_bytes())
        .map_err(|e| format!("cannot write {}: {e}", path.display()))
}

fn scheduler_timeline_json(store: &str, job_id: &str) -> Result<Option<String>, String> {
    let path = scheduler_events_path(store, job_id);
    if !path.exists() {
        return Ok(None);
    }
    let contents = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut rows = Vec::new();
    for line in contents.lines() {
        let value = serde_json::from_str::<serde_json::Value>(line)
            .map_err(|e| format!("invalid scheduler event ledger {}: {e}", path.display()))?;
        if value.get("type").and_then(|v| v.as_str()) != Some("agent_scheduler_event") {
            return Err(format!(
                "{} contains non scheduler event row",
                path.display()
            ));
        }
        rows.push(value.to_compact_string());
    }
    Ok(Some(format!("[{}]", rows.join(","))))
}

fn write_atomic(path: &std::path::Path, contents: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("manifest path has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    let tmp = parent.join(format!(".manifest.{}.tmp", std::process::id()));
    std::fs::write(&tmp, contents).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        format!(
            "cannot replace {} with {}: {e}",
            path.display(),
            tmp.display()
        )
    })
}

fn policy_agent_path(policy_path: &str, policy: &serde_json::Value) -> Result<String, String> {
    let Some(object) = policy.as_object() else {
        return Err("policy root must be an object".to_string());
    };
    let Some(agent) = agent_policy_string_field(object, "agent")? else {
        return Err("policy missing string field `agent`".to_string());
    };
    let base = std::path::Path::new(policy_path)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    Ok(agent_policy_path_arg(base, agent))
}

fn scheduler_manifest_json(
    job_id: &str,
    status: &str,
    policy_path: &str,
    policy_hash: &str,
    agent_path: &str,
    agent_hash: &str,
    created_ms: u128,
    updated_ms: u128,
    active_turn_id: Option<&str>,
    turn_cursor: u64,
    last_turn_status: Option<&str>,
    awaits_json: &str,
) -> String {
    let last_turn = last_turn_status
        .map(|status| format!(",\n  \"last_turn_status\": {}", json_string_literal(status)))
        .unwrap_or_default();
    format!(
        "{{\n  \"type\": \"cruft_agent_scheduler_job_manifest\",\n  \"schema_version\": {},\n  \"job_id\": {},\n  \"status\": {},\n  \"created_ms\": {},\n  \"updated_ms\": {},\n  \"policy_path\": {},\n  \"policy_hash\": {},\n  \"agent_path\": {},\n  \"agent_hash\": {},\n  \"active_turn_id\": {},\n  \"turn_cursor\": {}{},\n  \"awaits\": {},\n  \"budget_ledger\": {{\n    \"turns\": {},\n    \"events\": 0,\n    \"tools\": 0,\n    \"failures\": {},\n    \"outstanding_awaits\": {}\n  }}\n}}",
        SCHEDULER_SCHEMA_VERSION,
        json_string_literal(job_id),
        json_string_literal(status),
        created_ms,
        updated_ms,
        json_string_literal(policy_path),
        json_string_literal(policy_hash),
        json_string_literal(agent_path),
        json_string_literal(agent_hash),
        active_turn_id
            .map(json_string_literal)
            .unwrap_or_else(|| "null".to_string()),
        turn_cursor,
        last_turn,
        awaits_json,
        turn_cursor,
        if status == "failed" { 1 } else { 0 },
        if awaits_json == "[]" { 0 } else { 1 }
    )
}

fn read_manifest_source(
    store: &str,
    job_id: &str,
) -> Result<(std::path::PathBuf, String, serde_json::Value), String> {
    if !validate_scheduler_job_id(job_id) {
        return Err("job id must be 1-128 safe ASCII identifier chars".to_string());
    }
    let path = scheduler_manifest_path(store, job_id);
    let source = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let value = serde_json::from_str::<serde_json::Value>(&source)
        .map_err(|e| format!("cannot parse {}: {e}", path.display()))?;
    let object = value
        .as_object()
        .ok_or_else(|| format!("{} is not a scheduler manifest object", path.display()))?;
    if object.get("type").and_then(|v| v.as_str()) != Some("cruft_agent_scheduler_job_manifest") {
        return Err(format!("{} is not a scheduler manifest", path.display()));
    }
    if object.get("schema_version").and_then(|v| v.as_u64()) != Some(SCHEDULER_SCHEMA_VERSION) {
        return Err(format!(
            "{} has unsupported scheduler schema_version",
            path.display()
        ));
    }
    if object.get("job_id").and_then(|v| v.as_str()) != Some(job_id) {
        return Err(format!("{} job_id does not match path", path.display()));
    }
    Ok((path, source, value))
}

fn read_manifest(store: &str, job_id: &str) -> Result<serde_json::Value, String> {
    read_manifest_source(store, job_id).map(|(_, _, value)| value)
}

fn manifest_string<'a>(object: &'a serde_json::Map, key: &str) -> Result<&'a str, String> {
    object
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("scheduler manifest field {key:?} must be a string"))
}

fn manifest_u128(object: &serde_json::Map, key: &str) -> Result<u128, String> {
    object
        .get(key)
        .and_then(|v| v.as_u64())
        .map(u128::from)
        .ok_or_else(|| format!("scheduler manifest field {key:?} must be a number"))
}

fn manifest_u64(object: &serde_json::Map, key: &str) -> Result<u64, String> {
    object
        .get(key)
        .and_then(|v| v.as_u64())
        .ok_or_else(|| format!("scheduler manifest field {key:?} must be a number"))
}

fn scheduler_await_record<'a>(
    value: &'a serde_json::Value,
    expected_kind: &str,
) -> Result<&'a serde_json::Value, String> {
    let Some(object) = value.as_object() else {
        return Err("scheduler manifest root must be an object".to_string());
    };
    let Some(awaits) = object.get("awaits").and_then(|v| v.as_array()) else {
        return Err("scheduler manifest field \"awaits\" must be an array".to_string());
    };
    let Some(record) = awaits.first() else {
        return Err(format!("awaiting_{expected_kind} job has no await record"));
    };
    let Some(record_object) = record.as_object() else {
        return Err("scheduler await record must be an object".to_string());
    };
    if record_object.get("kind").and_then(|v| v.as_str()) != Some(expected_kind) {
        return Err(format!(
            "awaiting_{expected_kind} job has non-{expected_kind} await record"
        ));
    }
    Ok(record)
}

fn timer_await_deadline_ms(record: &serde_json::Value) -> Result<u128, String> {
    record
        .as_object()
        .and_then(|object| object.get("payload"))
        .and_then(|payload| payload.as_object())
        .and_then(|payload| payload.get("deadline_ms"))
        .and_then(|deadline| deadline.as_u64())
        .map(u128::from)
        .ok_or_else(|| "timer await record missing numeric payload.deadline_ms".to_string())
}

fn scheduler_resume_context(record: &serde_json::Value) -> String {
    format!(
        "{{\"scheduler_resume\":{{\"kind\":\"timer\",\"await\":{}}}}}",
        record.to_compact_string()
    )
}

fn scheduler_tool_result_context(record: &serde_json::Value, result_json: &str) -> String {
    format!(
        "{{\"scheduler_resume\":{{\"kind\":\"tool\",\"await\":{},\"result\":{}}}}}",
        record.to_compact_string(),
        result_json
    )
}

fn scheduler_tool_error_context(record: &serde_json::Value, message: &str) -> String {
    format!(
        "{{\"scheduler_resume\":{{\"kind\":\"tool\",\"await\":{},\"error\":{{\"message\":{},\"disposition\":\"error\"}}}}}}",
        record.to_compact_string(),
        json_string_literal(message)
    )
}

fn scheduler_tool_timeout_context(
    record: &serde_json::Value,
    tool: &str,
    timeout_ms: u64,
    delay_ms: u64,
) -> String {
    format!(
        "{{\"scheduler_resume\":{{\"kind\":\"tool\",\"await\":{},\"error\":{{\"message\":{},\"disposition\":\"timeout\",\"tool\":{},\"timeout_ms\":{},\"duration_ms\":{},\"late_result_suppressed\":true}}}}}}",
        record.to_compact_string(),
        json_string_literal(&format!(
            "agent scheduled tool timeout: {tool} after {timeout_ms}ms"
        )),
        json_string_literal(tool),
        timeout_ms,
        delay_ms
    )
}

fn scheduler_input_result_context(record: &serde_json::Value, result_json: &str) -> String {
    format!(
        "{{\"scheduler_resume\":{{\"kind\":\"input\",\"await\":{},\"result\":{}}}}}",
        record.to_compact_string(),
        result_json
    )
}

fn scheduler_await_kind_from_source(source: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(source).ok()?;
    value
        .as_object()?
        .get("kind")?
        .as_str()
        .map(|kind| kind.to_string())
}

fn manifest_policy_allows_tool(policy_path: &str, tool: &str) -> Result<bool, String> {
    let source = std::fs::read_to_string(policy_path)
        .map_err(|e| format!("cannot read policy {policy_path}: {e}"))?;
    let value = serde_json::from_str::<serde_json::Value>(&source)
        .map_err(|e| format!("cannot parse policy {policy_path}: {e}"))?;
    let Some(object) = value.as_object() else {
        return Err(format!("policy {policy_path} must be an object"));
    };
    let Some(tools) = object.get("tools").and_then(|v| v.as_array()) else {
        return Ok(false);
    };
    Ok(tools
        .iter()
        .any(|value| value.as_str().map(|name| name == tool).unwrap_or(false)))
}

fn manifest_policy_scheduler_turn_limit(policy_path: &str) -> Result<Option<u64>, String> {
    let source = std::fs::read_to_string(policy_path)
        .map_err(|e| format!("cannot read policy {policy_path}: {e}"))?;
    let value = serde_json::from_str::<serde_json::Value>(&source)
        .map_err(|e| format!("cannot parse policy {policy_path}: {e}"))?;
    Ok(value
        .as_object()
        .and_then(|object| object.get("budgets"))
        .and_then(|budgets| budgets.as_object())
        .and_then(|budgets| budgets.get("max_scheduler_turns"))
        .and_then(|value| value.as_u64()))
}

fn tool_await_name(record: &serde_json::Value) -> Result<&str, String> {
    record
        .as_object()
        .and_then(|object| object.get("payload"))
        .and_then(|payload| payload.as_object())
        .and_then(|payload| payload.get("tool"))
        .and_then(|tool| tool.as_str())
        .ok_or_else(|| "tool await record missing string payload.tool".to_string())
}

fn tool_await_args_json(record: &serde_json::Value) -> Result<String, String> {
    record
        .as_object()
        .and_then(|object| object.get("payload"))
        .and_then(|payload| payload.as_object())
        .and_then(|payload| payload.get("args"))
        .map(|args| args.to_compact_string())
        .ok_or_else(|| "tool await record missing payload.args".to_string())
}

fn tool_await_arg_u64(record: &serde_json::Value, key: &str) -> Option<u64> {
    record
        .as_object()
        .and_then(|object| object.get("payload"))
        .and_then(|payload| payload.as_object())
        .and_then(|payload| payload.get("args"))
        .and_then(|args| args.as_object())
        .and_then(|args| args.get(key))
        .and_then(|value| value.as_u64())
}

fn tool_await_option_u64(record: &serde_json::Value, key: &str) -> Option<u64> {
    record
        .as_object()
        .and_then(|object| object.get("payload"))
        .and_then(|payload| payload.as_object())
        .and_then(|payload| payload.get("options"))
        .and_then(|options| options.as_object())
        .and_then(|options| options.get(key))
        .and_then(|value| value.as_u64())
}

fn input_await_token(record: &serde_json::Value) -> Result<&str, String> {
    record
        .as_object()
        .and_then(|object| object.get("payload"))
        .and_then(|payload| payload.as_object())
        .and_then(|payload| payload.get("token"))
        .and_then(|token| token.as_str())
        .ok_or_else(|| "input await record missing string payload.token".to_string())
}

fn scheduler_expire_for_turn_budget(
    manifest_path: &std::path::Path,
    store: &str,
    job_id: &str,
    policy_path: &str,
    policy_hash: &str,
    agent_path: &str,
    agent_hash: &str,
    created_ms: u128,
    turn_cursor: u64,
    attempted_turn: u64,
    limit: u64,
) -> Result<(), String> {
    let manifest = scheduler_manifest_json(
        job_id,
        "expired",
        policy_path,
        policy_hash,
        agent_path,
        agent_hash,
        created_ms,
        now_ms(),
        None,
        turn_cursor,
        Some("expired"),
        "[]",
    );
    write_atomic(manifest_path, &manifest)?;
    append_scheduler_event(
        store,
        job_id,
        "budget_exceeded",
        &format!(
            "\"status\":\"expired\",\"budget\":\"max_scheduler_turns\",\"limit\":{},\"attempted_turn\":{}",
            limit, attempted_turn
        ),
    )?;
    println!(
        "{{\"type\":\"agent_scheduler_budget_exceeded\",\"schema_version\":{},\"job_id\":{},\"status\":\"expired\",\"budget\":\"max_scheduler_turns\",\"limit\":{},\"attempted_turn\":{}}}",
        SCHEDULER_SCHEMA_VERSION,
        json_string_literal(job_id),
        limit,
        attempted_turn
    );
    Ok(())
}

fn run_schedule_start(args: &[String]) -> ExitCode {
    let mut project: Option<String> = None;
    let mut policy_target: Option<String> = None;
    let mut store: Option<String> = None;
    let mut job_id: Option<String> = None;
    let mut i = 0usize;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--project" {
            i += 1;
            let Some(value) = args.get(i) else {
                eprintln!("cruft agent schedule start: --project requires a directory");
                return ExitCode::from(64);
            };
            project = Some(value.clone());
        } else if let Some(value) = arg.strip_prefix("--project=") {
            project = Some(value.to_string());
        } else if arg == "--policy" {
            i += 1;
            let Some(value) = args.get(i) else {
                eprintln!("cruft agent schedule start: --policy requires a path");
                return ExitCode::from(64);
            };
            policy_target = Some(value.clone());
        } else if let Some(value) = arg.strip_prefix("--policy=") {
            policy_target = Some(value.to_string());
        } else if arg == "--store" {
            i += 1;
            let Some(value) = args.get(i) else {
                eprintln!("cruft agent schedule start: --store requires a directory");
                return ExitCode::from(64);
            };
            store = Some(value.clone());
        } else if let Some(value) = arg.strip_prefix("--store=") {
            store = Some(value.to_string());
        } else if arg == "--job-id" {
            i += 1;
            let Some(value) = args.get(i) else {
                eprintln!("cruft agent schedule start: --job-id requires an id");
                return ExitCode::from(64);
            };
            job_id = Some(value.clone());
        } else if let Some(value) = arg.strip_prefix("--job-id=") {
            job_id = Some(value.to_string());
        } else {
            eprintln!("cruft agent schedule start: unexpected argument {arg}");
            return ExitCode::from(64);
        }
        i += 1;
    }
    if project.is_some() == policy_target.is_some() {
        eprintln!("cruft agent schedule start: provide exactly one of --project or --policy");
        return ExitCode::from(64);
    }
    let target = project.or(policy_target).unwrap();
    let (policy_path, policy_source, policy) = match agent_policy_load_target(&target) {
        Ok(value) => value,
        Err(e) => {
            eprintln!("cruft agent schedule start: {e}");
            return ExitCode::from(65);
        }
    };
    if let Err(errors) = agent_policy_validate_value_with_options(
        &policy_path,
        &policy,
        AgentPolicyValidationOptions {
            strict: true,
            project_confined: false,
        },
    ) {
        eprintln!(
            "cruft agent schedule start: policy validation failed: {}",
            errors.join("; ")
        );
        return ExitCode::from(65);
    }
    let agent_path = match policy_agent_path(&policy_path, &policy) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("cruft agent schedule start: {e}");
            return ExitCode::from(65);
        }
    };
    let agent_source = match std::fs::read_to_string(&agent_path) {
        Ok(source) => source,
        Err(e) => {
            eprintln!("cruft agent schedule start: cannot read agent {agent_path}: {e}");
            return ExitCode::from(65);
        }
    };
    let policy_hash = agent_source_hash(&policy_source);
    let agent_hash = agent_source_hash(&agent_source);
    let job_id = job_id.unwrap_or_else(|| generated_job_id(&policy_hash, &agent_hash));
    if !validate_scheduler_job_id(&job_id) {
        eprintln!("cruft agent schedule start: --job-id must be 1-128 safe ASCII identifier chars");
        return ExitCode::from(64);
    }
    let store = store.unwrap_or_else(|| default_store_for_policy(&policy_path));
    let manifest_path = scheduler_manifest_path(&store, &job_id);
    if manifest_path.exists() {
        eprintln!("cruft agent schedule start: job already exists: {job_id}");
        return ExitCode::from(73);
    }
    let created_ms = now_ms();
    let manifest = scheduler_manifest_json(
        &job_id,
        "created",
        &policy_path,
        &policy_hash,
        &agent_path,
        &agent_hash,
        created_ms,
        created_ms,
        None,
        0,
        None,
        "[]",
    );
    if let Err(e) = write_atomic(&manifest_path, &manifest) {
        eprintln!("cruft agent schedule start: {e}");
        return ExitCode::from(74);
    }
    if let Err(e) = append_scheduler_event(
        &store,
        &job_id,
        "created",
        &format!(
            "\"status\":\"created\",\"manifest\":{}",
            json_string_literal(&manifest_path.display().to_string())
        ),
    ) {
        eprintln!("cruft agent schedule start: {e}");
        return ExitCode::from(74);
    }
    println!(
        "{{\"type\":\"agent_scheduler_start\",\"schema_version\":{},\"job_id\":{},\"status\":\"created\",\"store\":{},\"manifest\":{}}}",
        SCHEDULER_SCHEMA_VERSION,
        json_string_literal(&job_id),
        json_string_literal(&store),
        json_string_literal(&manifest_path.display().to_string())
    );
    ExitCode::SUCCESS
}

fn run_schedule_tick(args: &[String]) -> ExitCode {
    let mut store: Option<String> = None;
    let mut job_id: Option<String> = None;
    let mut i = 0usize;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--store" {
            i += 1;
            let Some(value) = args.get(i) else {
                eprintln!("cruft agent schedule tick: --store requires a directory");
                return ExitCode::from(64);
            };
            store = Some(value.clone());
        } else if let Some(value) = arg.strip_prefix("--store=") {
            store = Some(value.to_string());
        } else if job_id.is_none() {
            job_id = Some(arg.clone());
        } else {
            eprintln!("cruft agent schedule tick: unexpected argument {arg}");
            return ExitCode::from(64);
        }
        i += 1;
    }
    let Some(job_id) = job_id else {
        eprintln!("cruft agent schedule tick: missing job id");
        return ExitCode::from(64);
    };
    let Some(store) = store else {
        eprintln!("cruft agent schedule tick: --store is required");
        return ExitCode::from(64);
    };
    let (manifest_path, _, value) = match read_manifest_source(&store, &job_id) {
        Ok(value) => value,
        Err(e) => {
            eprintln!("cruft agent schedule tick: {e}");
            return ExitCode::from(65);
        }
    };
    let Some(object) = value.as_object() else {
        eprintln!("cruft agent schedule tick: manifest root must be an object");
        return ExitCode::from(65);
    };
    let status = match manifest_string(object, "status") {
        Ok(status) => status,
        Err(e) => {
            eprintln!("cruft agent schedule tick: {e}");
            return ExitCode::from(65);
        }
    };
    if status != "created" && status != "awaiting_timer" && status != "awaiting_tool" {
        eprintln!("cruft agent schedule tick: job {job_id} is not tickable from status {status}");
        return ExitCode::from(75);
    }
    let created_ms = match manifest_u128(object, "created_ms") {
        Ok(value) => value,
        Err(e) => {
            eprintln!("cruft agent schedule tick: {e}");
            return ExitCode::from(65);
        }
    };
    let policy_path = match manifest_string(object, "policy_path") {
        Ok(value) => value.to_string(),
        Err(e) => {
            eprintln!("cruft agent schedule tick: {e}");
            return ExitCode::from(65);
        }
    };
    let policy_hash = match manifest_string(object, "policy_hash") {
        Ok(value) => value.to_string(),
        Err(e) => {
            eprintln!("cruft agent schedule tick: {e}");
            return ExitCode::from(65);
        }
    };
    let agent_path = match manifest_string(object, "agent_path") {
        Ok(value) => value.to_string(),
        Err(e) => {
            eprintln!("cruft agent schedule tick: {e}");
            return ExitCode::from(65);
        }
    };
    let agent_hash = match manifest_string(object, "agent_hash") {
        Ok(value) => value.to_string(),
        Err(e) => {
            eprintln!("cruft agent schedule tick: {e}");
            return ExitCode::from(65);
        }
    };
    let turn_cursor = match manifest_u64(object, "turn_cursor") {
        Ok(value) => value,
        Err(e) => {
            eprintln!("cruft agent schedule tick: {e}");
            return ExitCode::from(65);
        }
    };
    let mut resume_context: Option<String> = None;
    let previous_awaits = if status == "awaiting_timer" {
        let record = match scheduler_await_record(&value, "timer") {
            Ok(record) => record,
            Err(e) => {
                eprintln!("cruft agent schedule tick: {e}");
                return ExitCode::from(65);
            }
        };
        let deadline = match timer_await_deadline_ms(record) {
            Ok(deadline) => deadline,
            Err(e) => {
                eprintln!("cruft agent schedule tick: {e}");
                return ExitCode::from(65);
            }
        };
        let now = now_ms();
        if now < deadline {
            println!(
                "{{\"type\":\"agent_scheduler_tick\",\"schema_version\":{},\"job_id\":{},\"status\":\"awaiting_timer\",\"ready\":false,\"deadline_ms\":{},\"now_ms\":{}}}",
                SCHEDULER_SCHEMA_VERSION,
                json_string_literal(&job_id),
                deadline,
                now
            );
            return ExitCode::SUCCESS;
        }
        resume_context = Some(scheduler_resume_context(record));
        format!("[{}]", record.to_compact_string())
    } else if status == "awaiting_tool" {
        let record = match scheduler_await_record(&value, "tool") {
            Ok(record) => record,
            Err(e) => {
                eprintln!("cruft agent schedule tick: {e}");
                return ExitCode::from(65);
            }
        };
        let tool = match tool_await_name(record) {
            Ok(tool) => tool,
            Err(e) => {
                eprintln!("cruft agent schedule tick: {e}");
                return ExitCode::from(65);
            }
        };
        let allowed = match manifest_policy_allows_tool(&policy_path, tool) {
            Ok(allowed) => allowed,
            Err(e) => {
                eprintln!("cruft agent schedule tick: {e}");
                return ExitCode::from(65);
            }
        };
        if !allowed {
            eprintln!(
                "cruft agent schedule tick: scheduler tool {tool:?} is not allowed by policy"
            );
            return ExitCode::from(76);
        }
        if tool != "echo" && tool != "fail" && tool != "slow" {
            eprintln!(
                "cruft agent schedule tick: scheduler durable tool {tool:?} is not available"
            );
            return ExitCode::from(76);
        }
        if tool == "fail" {
            resume_context = Some(scheduler_tool_error_context(
                record,
                "agent scheduled tool failure: fail",
            ));
        } else if tool == "slow" {
            let delay_ms = tool_await_arg_u64(record, "delay_ms").unwrap_or(0);
            let timeout_ms = tool_await_option_u64(record, "timeout_ms").unwrap_or(250);
            if delay_ms > timeout_ms {
                resume_context = Some(scheduler_tool_timeout_context(
                    record, tool, timeout_ms, delay_ms,
                ));
            } else {
                let result_json = format!(
                    "{{\"ok\":true,\"tool\":\"slow\",\"delay_ms\":{},\"value\":{}}}",
                    delay_ms,
                    record
                        .as_object()
                        .and_then(|object| object.get("payload"))
                        .and_then(|payload| payload.as_object())
                        .and_then(|payload| payload.get("args"))
                        .and_then(|args| args.as_object())
                        .and_then(|args| args.get("value"))
                        .map(|value| value.to_compact_string())
                        .unwrap_or_else(|| "null".to_string())
                );
                resume_context = Some(scheduler_tool_result_context(record, &result_json));
            }
        } else {
            let result_json = match tool_await_args_json(record) {
                Ok(args) => args,
                Err(e) => {
                    eprintln!("cruft agent schedule tick: {e}");
                    return ExitCode::from(65);
                }
            };
            resume_context = Some(scheduler_tool_result_context(record, &result_json));
        }
        format!("[{}]", record.to_compact_string())
    } else {
        "[]".to_string()
    };
    let next_turn_cursor = turn_cursor + 1;
    match manifest_policy_scheduler_turn_limit(&policy_path) {
        Ok(Some(limit)) if next_turn_cursor > limit => {
            if let Err(e) = scheduler_expire_for_turn_budget(
                &manifest_path,
                &store,
                &job_id,
                &policy_path,
                &policy_hash,
                &agent_path,
                &agent_hash,
                created_ms,
                turn_cursor,
                next_turn_cursor,
                limit,
            ) {
                eprintln!("cruft agent schedule tick: {e}");
                return ExitCode::from(74);
            }
            return ExitCode::from(70);
        }
        Ok(_) => {}
        Err(e) => {
            eprintln!("cruft agent schedule tick: {e}");
            return ExitCode::from(65);
        }
    }
    let turn_id = format!("{job_id}/turn-{next_turn_cursor}");
    let running = scheduler_manifest_json(
        &job_id,
        "running",
        &policy_path,
        &policy_hash,
        &agent_path,
        &agent_hash,
        created_ms,
        now_ms(),
        Some(&turn_id),
        turn_cursor,
        None,
        &previous_awaits,
    );
    if let Err(e) = write_atomic(&manifest_path, &running) {
        eprintln!("cruft agent schedule tick: {e}");
        return ExitCode::from(74);
    }
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            eprintln!("cruft agent schedule tick: cannot resolve current executable: {e}");
            return ExitCode::from(74);
        }
    };
    let await_out = std::env::temp_dir().join(format!(
        "cruft-agent-scheduler-await-{}-{}.json",
        std::process::id(),
        now_ms()
    ));
    let mut command = std::process::Command::new(exe);
    command
        .arg("agent")
        .arg("run")
        .arg("--policy")
        .arg(&policy_path)
        .arg("--run-id")
        .arg(&turn_id)
        .arg("--scheduler-await-out")
        .arg(&await_out);
    if let Some(context) = &resume_context {
        command.arg("--context-json").arg(context);
    }
    let out = command.output();
    let await_record = std::fs::read_to_string(&await_out).ok();
    let _ = std::fs::remove_file(&await_out);
    let (turn_status, child_code) = match out {
        Ok(output) if output.status.success() => ("completed", 0),
        Ok(output)
            if await_record
                .as_deref()
                .map(|record| record.contains("\"type\":\"cruft_agent_scheduler_await\""))
                .unwrap_or(false) =>
        {
            let status = match await_record
                .as_deref()
                .and_then(scheduler_await_kind_from_source)
                .as_deref()
            {
                Some("tool") => "awaiting_tool",
                Some("input") => "awaiting_input",
                _ => "awaiting_timer",
            };
            (status, output.status.code().unwrap_or(70))
        }
        Ok(output) => ("failed", output.status.code().unwrap_or(1)),
        Err(e) => {
            eprintln!("cruft agent schedule tick: cannot launch turn child: {e}");
            ("failed", 127)
        }
    };
    let awaits_json = await_record
        .as_deref()
        .map(|record| format!("[{}]", record.trim()))
        .unwrap_or_else(|| "[]".to_string());
    let final_manifest = scheduler_manifest_json(
        &job_id,
        turn_status,
        &policy_path,
        &policy_hash,
        &agent_path,
        &agent_hash,
        created_ms,
        now_ms(),
        None,
        next_turn_cursor,
        Some(turn_status),
        &awaits_json,
    );
    if let Err(e) = write_atomic(&manifest_path, &final_manifest) {
        eprintln!("cruft agent schedule tick: {e}");
        return ExitCode::from(74);
    }
    if let Err(e) = append_scheduler_event(
        &store,
        &job_id,
        "tick",
        &format!(
            "\"turn_id\":{},\"status\":{},\"child_exit_code\":{},\"turn_cursor\":{}",
            json_string_literal(&turn_id),
            json_string_literal(turn_status),
            child_code,
            next_turn_cursor
        ),
    ) {
        eprintln!("cruft agent schedule tick: {e}");
        return ExitCode::from(74);
    }
    println!(
        "{{\"type\":\"agent_scheduler_tick\",\"schema_version\":{},\"job_id\":{},\"turn_id\":{},\"status\":{},\"child_exit_code\":{}}}",
        SCHEDULER_SCHEMA_VERSION,
        json_string_literal(&job_id),
        json_string_literal(&turn_id),
        json_string_literal(turn_status),
        child_code
    );
    if turn_status == "completed"
        || turn_status == "awaiting_timer"
        || turn_status == "awaiting_tool"
        || turn_status == "awaiting_input"
    {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(70)
    }
}

fn run_schedule_status(args: &[String]) -> ExitCode {
    let mut store: Option<String> = None;
    let mut job_id: Option<String> = None;
    let mut i = 0usize;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--store" {
            i += 1;
            let Some(value) = args.get(i) else {
                eprintln!("cruft agent schedule status: --store requires a directory");
                return ExitCode::from(64);
            };
            store = Some(value.clone());
        } else if let Some(value) = arg.strip_prefix("--store=") {
            store = Some(value.to_string());
        } else if job_id.is_none() {
            job_id = Some(arg.clone());
        } else {
            eprintln!("cruft agent schedule status: unexpected argument {arg}");
            return ExitCode::from(64);
        }
        i += 1;
    }
    let Some(job_id) = job_id else {
        eprintln!("cruft agent schedule status: missing job id");
        return ExitCode::from(64);
    };
    let Some(store) = store else {
        eprintln!("cruft agent schedule status: --store is required");
        return ExitCode::from(64);
    };
    match read_manifest(&store, &job_id) {
        Ok(value) => {
            println!("{}", value.to_pretty_string());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("cruft agent schedule status: {e}");
            ExitCode::from(65)
        }
    }
}

fn run_schedule_replay(args: &[String]) -> ExitCode {
    let mut store: Option<String> = None;
    let mut job_id: Option<String> = None;
    let mut i = 0usize;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--store" {
            i += 1;
            let Some(value) = args.get(i) else {
                eprintln!("cruft agent schedule replay: --store requires a directory");
                return ExitCode::from(64);
            };
            store = Some(value.clone());
        } else if let Some(value) = arg.strip_prefix("--store=") {
            store = Some(value.to_string());
        } else if job_id.is_none() {
            job_id = Some(arg.clone());
        } else {
            eprintln!("cruft agent schedule replay: unexpected argument {arg}");
            return ExitCode::from(64);
        }
        i += 1;
    }
    let Some(job_id) = job_id else {
        eprintln!("cruft agent schedule replay: missing job id");
        return ExitCode::from(64);
    };
    let Some(store) = store else {
        eprintln!("cruft agent schedule replay: --store is required");
        return ExitCode::from(64);
    };
    let value = match read_manifest(&store, &job_id) {
        Ok(value) => value,
        Err(e) => {
            eprintln!("cruft agent schedule replay: {e}");
            return ExitCode::from(65);
        }
    };
    let Some(object) = value.as_object() else {
        eprintln!("cruft agent schedule replay: manifest root must be an object");
        return ExitCode::from(65);
    };
    let status = object
        .get("status")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let created_ms = object
        .get("created_ms")
        .map(|value| value.to_compact_string())
        .unwrap_or_else(|| "null".to_string());
    let updated_ms = object
        .get("updated_ms")
        .map(|value| value.to_compact_string())
        .unwrap_or_else(|| "null".to_string());
    let turn_cursor = object
        .get("turn_cursor")
        .map(|value| value.to_compact_string())
        .unwrap_or_else(|| "null".to_string());
    let awaits = object
        .get("awaits")
        .map(|value| value.to_compact_string())
        .unwrap_or_else(|| "[]".to_string());
    let budget_ledger = object
        .get("budget_ledger")
        .map(|value| value.to_compact_string())
        .unwrap_or_else(|| "{}".to_string());
    let last_turn_status = object
        .get("last_turn_status")
        .map(|value| value.to_compact_string())
        .unwrap_or_else(|| "null".to_string());
    let terminal = matches!(status, "completed" | "failed" | "cancelled" | "expired");
    let timeline = match scheduler_timeline_json(&store, &job_id) {
        Ok(Some(timeline)) => timeline,
        Ok(None) => format!(
            "[{{\"event\":\"created\",\"at_ms\":{}}},{{\"event\":\"status\",\"status\":{},\"at_ms\":{},\"turn_cursor\":{},\"last_turn_status\":{}}}]",
            created_ms,
            json_string_literal(status),
            updated_ms,
            turn_cursor,
            last_turn_status
        ),
        Err(e) => {
            eprintln!("cruft agent schedule replay: {e}");
            return ExitCode::from(65);
        }
    };
    println!(
        "{{\"type\":\"agent_scheduler_replay\",\"schema_version\":{},\"job_id\":{},\"status\":{},\"terminal\":{},\"timeline\":{},\"awaits\":{},\"budget_ledger\":{},\"claim_boundary\":{{\"source\":\"scheduler_manifest_plus_append_only_event_ledger\",\"per_turn_audit\":\"not_reconstructed_by_this_replay_slice\",\"raw_js_continuation\":\"not_claimed\"}}}}",
        SCHEDULER_SCHEMA_VERSION,
        json_string_literal(&job_id),
        json_string_literal(status),
        if terminal { "true" } else { "false" },
        timeline,
        awaits,
        budget_ledger
    );
    ExitCode::SUCCESS
}

fn run_schedule_input(args: &[String]) -> ExitCode {
    let mut store: Option<String> = None;
    let mut job_id: Option<String> = None;
    let mut token: Option<String> = None;
    let mut result_json: Option<String> = None;
    let mut i = 0usize;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--store" {
            i += 1;
            let Some(value) = args.get(i) else {
                eprintln!("cruft agent schedule input: --store requires a directory");
                return ExitCode::from(64);
            };
            store = Some(value.clone());
        } else if let Some(value) = arg.strip_prefix("--store=") {
            store = Some(value.to_string());
        } else if arg == "--token" {
            i += 1;
            let Some(value) = args.get(i) else {
                eprintln!("cruft agent schedule input: --token requires a value");
                return ExitCode::from(64);
            };
            token = Some(value.clone());
        } else if let Some(value) = arg.strip_prefix("--token=") {
            token = Some(value.to_string());
        } else if arg == "--json" {
            i += 1;
            let Some(value) = args.get(i) else {
                eprintln!("cruft agent schedule input: --json requires a JSON value");
                return ExitCode::from(64);
            };
            result_json = Some(value.clone());
        } else if let Some(value) = arg.strip_prefix("--json=") {
            result_json = Some(value.to_string());
        } else if job_id.is_none() {
            job_id = Some(arg.clone());
        } else {
            eprintln!("cruft agent schedule input: unexpected argument {arg}");
            return ExitCode::from(64);
        }
        i += 1;
    }
    let Some(job_id) = job_id else {
        eprintln!("cruft agent schedule input: missing job id");
        return ExitCode::from(64);
    };
    let Some(store) = store else {
        eprintln!("cruft agent schedule input: --store is required");
        return ExitCode::from(64);
    };
    let Some(token) = token else {
        eprintln!("cruft agent schedule input: --token is required");
        return ExitCode::from(64);
    };
    let Some(result_json) = result_json else {
        eprintln!("cruft agent schedule input: --json is required");
        return ExitCode::from(64);
    };
    let result_value = match serde_json::from_str::<serde_json::Value>(&result_json) {
        Ok(value) => value,
        Err(e) => {
            eprintln!("cruft agent schedule input: --json must parse as JSON: {e}");
            return ExitCode::from(64);
        }
    };
    let (manifest_path, _, value) = match read_manifest_source(&store, &job_id) {
        Ok(value) => value,
        Err(e) => {
            eprintln!("cruft agent schedule input: {e}");
            return ExitCode::from(65);
        }
    };
    let Some(object) = value.as_object() else {
        eprintln!("cruft agent schedule input: manifest root must be an object");
        return ExitCode::from(65);
    };
    let status = match manifest_string(object, "status") {
        Ok(status) => status,
        Err(e) => {
            eprintln!("cruft agent schedule input: {e}");
            return ExitCode::from(65);
        }
    };
    if status != "awaiting_input" {
        eprintln!(
            "cruft agent schedule input: job {job_id} is not awaiting input from status {status}"
        );
        return ExitCode::from(75);
    }
    let record = match scheduler_await_record(&value, "input") {
        Ok(record) => record,
        Err(e) => {
            eprintln!("cruft agent schedule input: {e}");
            return ExitCode::from(65);
        }
    };
    let expected_token = match input_await_token(record) {
        Ok(token) => token,
        Err(e) => {
            eprintln!("cruft agent schedule input: {e}");
            return ExitCode::from(65);
        }
    };
    if token != expected_token {
        eprintln!("cruft agent schedule input: input token does not match pending wait");
        return ExitCode::from(76);
    }
    let created_ms = match manifest_u128(object, "created_ms") {
        Ok(value) => value,
        Err(e) => {
            eprintln!("cruft agent schedule input: {e}");
            return ExitCode::from(65);
        }
    };
    let policy_path = match manifest_string(object, "policy_path") {
        Ok(value) => value.to_string(),
        Err(e) => {
            eprintln!("cruft agent schedule input: {e}");
            return ExitCode::from(65);
        }
    };
    let policy_hash = match manifest_string(object, "policy_hash") {
        Ok(value) => value.to_string(),
        Err(e) => {
            eprintln!("cruft agent schedule input: {e}");
            return ExitCode::from(65);
        }
    };
    let agent_path = match manifest_string(object, "agent_path") {
        Ok(value) => value.to_string(),
        Err(e) => {
            eprintln!("cruft agent schedule input: {e}");
            return ExitCode::from(65);
        }
    };
    let agent_hash = match manifest_string(object, "agent_hash") {
        Ok(value) => value.to_string(),
        Err(e) => {
            eprintln!("cruft agent schedule input: {e}");
            return ExitCode::from(65);
        }
    };
    let turn_cursor = match manifest_u64(object, "turn_cursor") {
        Ok(value) => value,
        Err(e) => {
            eprintln!("cruft agent schedule input: {e}");
            return ExitCode::from(65);
        }
    };
    let next_turn_cursor = turn_cursor + 1;
    match manifest_policy_scheduler_turn_limit(&policy_path) {
        Ok(Some(limit)) if next_turn_cursor > limit => {
            if let Err(e) = scheduler_expire_for_turn_budget(
                &manifest_path,
                &store,
                &job_id,
                &policy_path,
                &policy_hash,
                &agent_path,
                &agent_hash,
                created_ms,
                turn_cursor,
                next_turn_cursor,
                limit,
            ) {
                eprintln!("cruft agent schedule input: {e}");
                return ExitCode::from(74);
            }
            return ExitCode::from(70);
        }
        Ok(_) => {}
        Err(e) => {
            eprintln!("cruft agent schedule input: {e}");
            return ExitCode::from(65);
        }
    }
    let turn_id = format!("{job_id}/turn-{next_turn_cursor}");
    let previous_awaits = format!("[{}]", record.to_compact_string());
    let running = scheduler_manifest_json(
        &job_id,
        "running",
        &policy_path,
        &policy_hash,
        &agent_path,
        &agent_hash,
        created_ms,
        now_ms(),
        Some(&turn_id),
        turn_cursor,
        None,
        &previous_awaits,
    );
    if let Err(e) = write_atomic(&manifest_path, &running) {
        eprintln!("cruft agent schedule input: {e}");
        return ExitCode::from(74);
    }
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            eprintln!("cruft agent schedule input: cannot resolve current executable: {e}");
            return ExitCode::from(74);
        }
    };
    let await_out = std::env::temp_dir().join(format!(
        "cruft-agent-scheduler-await-{}-{}.json",
        std::process::id(),
        now_ms()
    ));
    let context = scheduler_input_result_context(record, &result_value.to_compact_string());
    let out = std::process::Command::new(exe)
        .arg("agent")
        .arg("run")
        .arg("--policy")
        .arg(&policy_path)
        .arg("--run-id")
        .arg(&turn_id)
        .arg("--scheduler-await-out")
        .arg(&await_out)
        .arg("--context-json")
        .arg(&context)
        .output();
    let await_record = std::fs::read_to_string(&await_out).ok();
    let _ = std::fs::remove_file(&await_out);
    let (turn_status, child_code) = match out {
        Ok(output) if output.status.success() => ("completed", 0),
        Ok(output)
            if await_record
                .as_deref()
                .map(|record| record.contains("\"type\":\"cruft_agent_scheduler_await\""))
                .unwrap_or(false) =>
        {
            let status = match await_record
                .as_deref()
                .and_then(scheduler_await_kind_from_source)
                .as_deref()
            {
                Some("tool") => "awaiting_tool",
                Some("input") => "awaiting_input",
                _ => "awaiting_timer",
            };
            (status, output.status.code().unwrap_or(70))
        }
        Ok(output) => ("failed", output.status.code().unwrap_or(1)),
        Err(e) => {
            eprintln!("cruft agent schedule input: cannot launch turn child: {e}");
            ("failed", 127)
        }
    };
    let awaits_json = await_record
        .as_deref()
        .map(|record| format!("[{}]", record.trim()))
        .unwrap_or_else(|| "[]".to_string());
    let final_manifest = scheduler_manifest_json(
        &job_id,
        turn_status,
        &policy_path,
        &policy_hash,
        &agent_path,
        &agent_hash,
        created_ms,
        now_ms(),
        None,
        next_turn_cursor,
        Some(turn_status),
        &awaits_json,
    );
    if let Err(e) = write_atomic(&manifest_path, &final_manifest) {
        eprintln!("cruft agent schedule input: {e}");
        return ExitCode::from(74);
    }
    if let Err(e) = append_scheduler_event(
        &store,
        &job_id,
        "input",
        &format!(
            "\"turn_id\":{},\"status\":{},\"child_exit_code\":{},\"turn_cursor\":{}",
            json_string_literal(&turn_id),
            json_string_literal(turn_status),
            child_code,
            next_turn_cursor
        ),
    ) {
        eprintln!("cruft agent schedule input: {e}");
        return ExitCode::from(74);
    }
    println!(
        "{{\"type\":\"agent_scheduler_input\",\"schema_version\":{},\"job_id\":{},\"turn_id\":{},\"status\":{},\"child_exit_code\":{}}}",
        SCHEDULER_SCHEMA_VERSION,
        json_string_literal(&job_id),
        json_string_literal(&turn_id),
        json_string_literal(turn_status),
        child_code
    );
    if turn_status == "completed"
        || turn_status == "awaiting_timer"
        || turn_status == "awaiting_tool"
        || turn_status == "awaiting_input"
    {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(70)
    }
}

fn run_schedule_cancel(args: &[String]) -> ExitCode {
    let mut store: Option<String> = None;
    let mut job_id: Option<String> = None;
    let mut i = 0usize;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--store" {
            i += 1;
            let Some(value) = args.get(i) else {
                eprintln!("cruft agent schedule cancel: --store requires a directory");
                return ExitCode::from(64);
            };
            store = Some(value.clone());
        } else if let Some(value) = arg.strip_prefix("--store=") {
            store = Some(value.to_string());
        } else if job_id.is_none() {
            job_id = Some(arg.clone());
        } else {
            eprintln!("cruft agent schedule cancel: unexpected argument {arg}");
            return ExitCode::from(64);
        }
        i += 1;
    }
    let Some(job_id) = job_id else {
        eprintln!("cruft agent schedule cancel: missing job id");
        return ExitCode::from(64);
    };
    let Some(store) = store else {
        eprintln!("cruft agent schedule cancel: --store is required");
        return ExitCode::from(64);
    };
    let (manifest_path, _, value) = match read_manifest_source(&store, &job_id) {
        Ok(value) => value,
        Err(e) => {
            eprintln!("cruft agent schedule cancel: {e}");
            return ExitCode::from(65);
        }
    };
    let Some(object) = value.as_object() else {
        eprintln!("cruft agent schedule cancel: manifest root must be an object");
        return ExitCode::from(65);
    };
    let status = match manifest_string(object, "status") {
        Ok(status) => status,
        Err(e) => {
            eprintln!("cruft agent schedule cancel: {e}");
            return ExitCode::from(65);
        }
    };
    if status == "cancelled" {
        println!(
            "{{\"type\":\"agent_scheduler_cancel\",\"schema_version\":{},\"job_id\":{},\"status\":\"cancelled\",\"already_cancelled\":true}}",
            SCHEDULER_SCHEMA_VERSION,
            json_string_literal(&job_id)
        );
        return ExitCode::SUCCESS;
    }
    if status != "created"
        && status != "awaiting_timer"
        && status != "awaiting_tool"
        && status != "awaiting_input"
    {
        eprintln!(
            "cruft agent schedule cancel: job {job_id} is not cancellable from status {status}"
        );
        return ExitCode::from(75);
    }
    let created_ms = match manifest_u128(object, "created_ms") {
        Ok(value) => value,
        Err(e) => {
            eprintln!("cruft agent schedule cancel: {e}");
            return ExitCode::from(65);
        }
    };
    let policy_path = match manifest_string(object, "policy_path") {
        Ok(value) => value.to_string(),
        Err(e) => {
            eprintln!("cruft agent schedule cancel: {e}");
            return ExitCode::from(65);
        }
    };
    let policy_hash = match manifest_string(object, "policy_hash") {
        Ok(value) => value.to_string(),
        Err(e) => {
            eprintln!("cruft agent schedule cancel: {e}");
            return ExitCode::from(65);
        }
    };
    let agent_path = match manifest_string(object, "agent_path") {
        Ok(value) => value.to_string(),
        Err(e) => {
            eprintln!("cruft agent schedule cancel: {e}");
            return ExitCode::from(65);
        }
    };
    let agent_hash = match manifest_string(object, "agent_hash") {
        Ok(value) => value.to_string(),
        Err(e) => {
            eprintln!("cruft agent schedule cancel: {e}");
            return ExitCode::from(65);
        }
    };
    let turn_cursor = match manifest_u64(object, "turn_cursor") {
        Ok(value) => value,
        Err(e) => {
            eprintln!("cruft agent schedule cancel: {e}");
            return ExitCode::from(65);
        }
    };
    let manifest = scheduler_manifest_json(
        &job_id,
        "cancelled",
        &policy_path,
        &policy_hash,
        &agent_path,
        &agent_hash,
        created_ms,
        now_ms(),
        None,
        turn_cursor,
        Some("cancelled"),
        "[]",
    );
    if let Err(e) = write_atomic(&manifest_path, &manifest) {
        eprintln!("cruft agent schedule cancel: {e}");
        return ExitCode::from(74);
    }
    if let Err(e) = append_scheduler_event(
        &store,
        &job_id,
        "cancelled",
        &format!(
            "\"status\":\"cancelled\",\"previous_status\":{}",
            json_string_literal(status)
        ),
    ) {
        eprintln!("cruft agent schedule cancel: {e}");
        return ExitCode::from(74);
    }
    println!(
        "{{\"type\":\"agent_scheduler_cancel\",\"schema_version\":{},\"job_id\":{},\"status\":\"cancelled\",\"already_cancelled\":false,\"previous_status\":{}}}",
        SCHEDULER_SCHEMA_VERSION,
        json_string_literal(&job_id),
        json_string_literal(status)
    );
    ExitCode::SUCCESS
}

fn run_schedule_list(args: &[String]) -> ExitCode {
    let mut store: Option<String> = None;
    let mut i = 0usize;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--store" {
            i += 1;
            let Some(value) = args.get(i) else {
                eprintln!("cruft agent schedule list: --store requires a directory");
                return ExitCode::from(64);
            };
            store = Some(value.clone());
        } else if let Some(value) = arg.strip_prefix("--store=") {
            store = Some(value.to_string());
        } else {
            eprintln!("cruft agent schedule list: unexpected argument {arg}");
            return ExitCode::from(64);
        }
        i += 1;
    }
    let Some(store) = store else {
        eprintln!("cruft agent schedule list: --store is required");
        return ExitCode::from(64);
    };
    let jobs_dir = std::path::Path::new(&store).join("jobs");
    let mut jobs = Vec::new();
    match std::fs::read_dir(&jobs_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !validate_scheduler_job_id(&name) {
                    continue;
                }
                if let Ok(value) = read_manifest(&store, &name) {
                    jobs.push(value);
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            eprintln!(
                "cruft agent schedule list: cannot read {}: {e}",
                jobs_dir.display()
            );
            return ExitCode::from(65);
        }
    }
    jobs.sort_by(|a, b| {
        a.get("job_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .cmp(b.get("job_id").and_then(|v| v.as_str()).unwrap_or(""))
    });
    let mut out = String::new();
    out.push_str("{\"type\":\"agent_scheduler_list\",\"schema_version\":");
    out.push_str(&SCHEDULER_SCHEMA_VERSION.to_string());
    out.push_str(",\"store\":");
    out.push_str(&json_string_literal(&store));
    out.push_str(",\"jobs\":[");
    for (idx, job) in jobs.iter().enumerate() {
        if idx > 0 {
            out.push(',');
        }
        out.push_str(&job.to_compact_string());
    }
    out.push_str("]}");
    println!("{out}");
    ExitCode::SUCCESS
}

pub(crate) fn run_agent_schedule_subcommand(args: &[String]) -> ExitCode {
    let Some(command) = args.first().map(|s| s.as_str()) else {
        scheduler_usage();
        return ExitCode::from(64);
    };
    match command {
        "start" => run_schedule_start(&args[1..]),
        "tick" => run_schedule_tick(&args[1..]),
        "input" => run_schedule_input(&args[1..]),
        "cancel" => run_schedule_cancel(&args[1..]),
        "replay" => run_schedule_replay(&args[1..]),
        "status" => run_schedule_status(&args[1..]),
        "list" => run_schedule_list(&args[1..]),
        _ => {
            scheduler_usage();
            ExitCode::from(64)
        }
    }
}
