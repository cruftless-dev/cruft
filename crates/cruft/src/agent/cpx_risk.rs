use super::integrity::agent_source_hash;
use crate::json_string_literal;
use std::io::Write;
use std::process::ExitCode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CpxRiskValidation {
    pub requested: String,
    pub source_kind: String,
    pub disposition: String,
    pub execution: String,
    pub risk_count: usize,
    pub unknown_count: usize,
    pub required_permission_count: usize,
    pub audit_attestation_accepted_count: usize,
    pub audit_attestation_rejected_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CpxRiskDecision {
    pub validation: CpxRiskValidation,
    pub decision: String,
    pub reason: String,
}

fn object<'a>(value: &'a serde_json::Value, field: &str) -> Result<&'a serde_json::Map, String> {
    value
        .as_object()
        .ok_or_else(|| format!("{field} must be an object"))
}

fn string_field<'a>(object: &'a serde_json::Map, field: &str) -> Result<&'a str, String> {
    object
        .get(field)
        .and_then(|value| value.as_str())
        .ok_or_else(|| format!("field {field:?} must be a string"))
}

fn nullable_string_field(object: &serde_json::Map, field: &str) -> Result<(), String> {
    match object.get(field) {
        Some(serde_json::Value::Null) | Some(serde_json::Value::String(_)) => Ok(()),
        Some(_) => Err(format!("field {field:?} must be null or a string")),
        None => Err(format!("field {field:?} is required")),
    }
}

fn array_field<'a>(
    object: &'a serde_json::Map,
    field: &str,
) -> Result<&'a [serde_json::Value], String> {
    object
        .get(field)
        .and_then(|value| value.as_array())
        .ok_or_else(|| format!("field {field:?} must be an array"))
}

fn validate_disposition(value: &str) -> Result<(), String> {
    match value {
        "allow" | "warn" | "requires_approval" | "blocked" | "unknown" => Ok(()),
        _ => Err(format!("unsupported policy_disposition {value:?}")),
    }
}

fn validate_source(source: &serde_json::Value) -> Result<String, String> {
    let source = object(source, "source")?;
    let kind = string_field(source, "kind")?;
    match kind {
        "local" | "registry-unfetched" | "unknown-local-miss" => {}
        _ => return Err(format!("unsupported source.kind {kind:?}")),
    }
    string_field(source, "registry")?;
    nullable_string_field(source, "local_package_dir")?;
    nullable_string_field(source, "resolved_executable")?;
    Ok(kind.to_string())
}

fn validate_packages(values: &[serde_json::Value]) -> Result<(), String> {
    for value in values {
        let package = object(value, "packages[]")?;
        string_field(package, "name")?;
        string_field(package, "range")?;
        string_field(package, "status")?;
    }
    Ok(())
}

fn validate_unknown_facts(values: &[serde_json::Value]) -> Result<(), String> {
    for value in values {
        let fact = object(value, "unknown_facts[]")?;
        string_field(fact, "field")?;
        string_field(fact, "status")?;
        string_field(fact, "reason")?;
    }
    Ok(())
}

fn validate_risks(values: &[serde_json::Value]) -> Result<(), String> {
    for value in values {
        let risk = object(value, "risks[]")?;
        string_field(risk, "kind")?;
        string_field(risk, "level")?;
        string_field(risk, "subject")?;
        string_field(risk, "reason")?;
    }
    Ok(())
}

fn validate_required_permissions(values: &[serde_json::Value]) -> Result<usize, String> {
    for value in values {
        value
            .as_str()
            .ok_or_else(|| "required_permissions[] must be a string".to_string())?;
    }
    Ok(values.len())
}

fn validate_audit_attestations(values: &[serde_json::Value]) -> Result<(usize, usize), String> {
    let mut accepted = 0usize;
    let mut rejected = 0usize;
    for value in values {
        let attestation = object(value, "audit_attestations[]")?;
        let status = string_field(attestation, "status")?;
        match status {
            "accepted" => accepted += 1,
            "rejected" => rejected += 1,
            _ => {
                return Err(format!(
                    "unsupported audit_attestations[].status {status:?}"
                ))
            }
        }
        string_field(attestation, "issuer")?;
        string_field(attestation, "issuer_key_id")?;
        string_field(attestation, "scope")?;
        let package = object(
            attestation
                .get("package")
                .ok_or_else(|| "field \"audit_attestations[].package\" is required".to_string())?,
            "audit_attestations[].package",
        )?;
        string_field(package, "name")?;
        string_field(package, "version")?;
        string_field(package, "registry")?;
        string_field(package, "integrity")?;
        if status == "rejected" {
            string_field(attestation, "reason")?;
        }
    }
    Ok((accepted, rejected))
}

fn risk_kind_blocks_agent_policy(kind: &str) -> bool {
    matches!(
        kind,
        "lifecycle-script"
            | "native-addon"
            | "known-malicious-advisory"
            | "known-vulnerability"
            | "git-dependency"
            | "tarball-dependency"
            | "novelty-package-risk"
    )
}

fn artifact_has_blocking_risk(value: &serde_json::Value) -> bool {
    value
        .as_object()
        .and_then(|root| root.get("risks"))
        .and_then(|risks| risks.as_array())
        .map(|risks| {
            risks.iter().any(|risk| {
                risk.as_object()
                    .and_then(|risk| risk.get("kind"))
                    .and_then(|kind| kind.as_str())
                    .map(risk_kind_blocks_agent_policy)
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn artifact_rejected_audit_attestations(value: &serde_json::Value) -> usize {
    value
        .as_object()
        .and_then(|root| root.get("audit_attestations"))
        .and_then(|values| values.as_array())
        .map(|values| {
            values
                .iter()
                .filter(|value| {
                    value
                        .as_object()
                        .and_then(|entry| entry.get("status"))
                        .and_then(|status| status.as_str())
                        == Some("rejected")
                })
                .count()
        })
        .unwrap_or(0)
}

pub(crate) fn validate_cpx_risk_artifact(
    value: &serde_json::Value,
) -> Result<CpxRiskValidation, String> {
    let root = object(value, "artifact root")?;
    let schema = string_field(root, "schema")?;
    if schema != "cruft.package_exec_risk.v1" {
        return Err(format!("unsupported schema {schema:?}"));
    }
    let tool = string_field(root, "tool")?;
    if tool != "cpx" {
        return Err(format!("unsupported tool {tool:?}"));
    }
    let requested = string_field(root, "requested")?.to_string();
    let source_kind = validate_source(
        root.get("source")
            .ok_or_else(|| "field \"source\" is required".to_string())?,
    )?;
    let packages = array_field(root, "packages")?;
    validate_packages(packages)?;
    let unknown_facts = array_field(root, "unknown_facts")?;
    validate_unknown_facts(unknown_facts)?;
    let risks = array_field(root, "risks")?;
    validate_risks(risks)?;
    let required_permission_count = match root.get("required_permissions") {
        Some(values) => validate_required_permissions(
            values
                .as_array()
                .ok_or_else(|| "field \"required_permissions\" must be an array".to_string())?,
        )?,
        None => 0,
    };
    let (audit_attestation_accepted_count, audit_attestation_rejected_count) =
        match root.get("audit_attestations") {
            Some(values) => validate_audit_attestations(
                values
                    .as_array()
                    .ok_or_else(|| "field \"audit_attestations\" must be an array".to_string())?,
            )?,
            None => (0, 0),
        };
    let disposition = string_field(root, "policy_disposition")?.to_string();
    validate_disposition(&disposition)?;
    let execution = string_field(root, "execution")?.to_string();
    if execution != "not_run" {
        return Err(format!("unsupported execution {execution:?}"));
    }
    Ok(CpxRiskValidation {
        requested,
        source_kind,
        disposition,
        execution,
        risk_count: risks.len(),
        unknown_count: unknown_facts.len(),
        required_permission_count,
        audit_attestation_accepted_count,
        audit_attestation_rejected_count,
    })
}

pub(crate) fn evaluate_cpx_risk_policy(
    value: &serde_json::Value,
) -> Result<CpxRiskDecision, String> {
    let validation = validate_cpx_risk_artifact(value)?;
    let has_blocking_risk = artifact_has_blocking_risk(value);
    let rejected_attestations = artifact_rejected_audit_attestations(value);
    let (decision, reason) = match validation.disposition.as_str() {
        _ if rejected_attestations > 0 => (
            "requires_approval",
            "artifact contains rejected audit attestation facts",
        ),
        "allow" if !has_blocking_risk => ("allow", "artifact disposition allows execution"),
        "allow" => (
            "requires_approval",
            "artifact contains package risk kind requiring approval",
        ),
        "warn" => ("warn", "artifact disposition warns before execution"),
        "requires_approval" => (
            "requires_approval",
            "artifact disposition requires human approval",
        ),
        "blocked" => ("blocked", "artifact disposition blocks execution"),
        "unknown" => (
            "requires_approval",
            "artifact has unknown package facts and cannot auto-allow",
        ),
        _ => return Err("validated artifact carried unsupported disposition".to_string()),
    };
    Ok(CpxRiskDecision {
        validation,
        decision: decision.to_string(),
        reason: reason.to_string(),
    })
}

fn append_cpx_risk_audit(
    audit_log: &str,
    artifact_hash: &str,
    decision: &CpxRiskDecision,
) -> Result<(), String> {
    if let Some(parent) = std::path::Path::new(audit_log).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create audit log parent {}: {e}", parent.display()))?;
        }
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(audit_log)
        .map_err(|e| format!("cannot open audit log {audit_log}: {e}"))?;
    writeln!(
        file,
        "{{\"type\":\"agent_cpx_risk_decision\",\"schema_version\":1,\"tool\":\"cpx\",\"artifact_hash\":{},\"requested\":{},\"source_kind\":{},\"policy_disposition\":{},\"execution\":{},\"risk_count\":{},\"unknown_count\":{},\"required_permission_count\":{},\"audit_attestation_accepted_count\":{},\"audit_attestation_rejected_count\":{},\"decision\":{},\"reason\":{},\"nonclaims\":[\"fnv1a64 is deterministic tamper evidence, not a cryptographic signature\"]}}",
        json_string_literal(artifact_hash),
        json_string_literal(&decision.validation.requested),
        json_string_literal(&decision.validation.source_kind),
        json_string_literal(&decision.validation.disposition),
        json_string_literal(&decision.validation.execution),
        decision.validation.risk_count,
        decision.validation.unknown_count,
        decision.validation.required_permission_count,
        decision.validation.audit_attestation_accepted_count,
        decision.validation.audit_attestation_rejected_count,
        json_string_literal(&decision.decision),
        json_string_literal(&decision.reason)
    )
    .map_err(|e| format!("cannot write audit log {audit_log}: {e}"))
}

fn append_cpx_risk_denial_audit(
    audit_log: &str,
    artifact_hash: Option<&str>,
    artifact_path: &str,
    reason: &str,
) -> Result<(), String> {
    if let Some(parent) = std::path::Path::new(audit_log).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create audit log parent {}: {e}", parent.display()))?;
        }
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(audit_log)
        .map_err(|e| format!("cannot open audit log {audit_log}: {e}"))?;
    writeln!(
        file,
        "{{\"type\":\"agent_cpx_risk_denial\",\"schema_version\":1,\"tool\":\"cpx\",\"artifact_hash\":{},\"artifact_path\":{},\"decision\":\"blocked\",\"reason\":{},\"nonclaims\":[\"missing or malformed CPX risk artifacts are denied before package execution\"]}}",
        artifact_hash
            .map(json_string_literal)
            .unwrap_or_else(|| "null".to_string()),
        json_string_literal(artifact_path),
        json_string_literal(reason)
    )
    .map_err(|e| format!("cannot write audit log {audit_log}: {e}"))
}

fn append_cpx_risk_approval_pending(
    approval_log: &str,
    artifact_hash: &str,
    decision: &CpxRiskDecision,
) -> Result<(), String> {
    if let Some(parent) = std::path::Path::new(approval_log).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "cannot create approval log parent {}: {e}",
                    parent.display()
                )
            })?;
        }
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(approval_log)
        .map_err(|e| format!("cannot open approval log {approval_log}: {e}"))?;
    writeln!(
        file,
        "{{\"type\":\"agent_approval_pending\",\"id\":{},\"tool\":\"cpx\",\"args\":{{\"requested\":{},\"artifact_hash\":{},\"policy_disposition\":{},\"reason\":{}}},\"arg_bytes\":{},\"status\":\"pending\",\"nonclaims\":[\"pending approval records grant no package execution authority\"]}}",
        json_string_literal(artifact_hash),
        json_string_literal(&decision.validation.requested),
        json_string_literal(artifact_hash),
        json_string_literal(&decision.validation.disposition),
        json_string_literal(&decision.reason),
        decision.validation.requested.len()
    )
    .map_err(|e| format!("cannot write approval log {approval_log}: {e}"))
}

fn cpx_risk_approval_status(
    approval_log: &str,
    artifact_hash: &str,
) -> Result<Option<String>, String> {
    let contents = match std::fs::read_to_string(approval_log) {
        Ok(contents) => contents,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("cannot read approval log {approval_log}: {e}")),
    };
    let mut status = None;
    for line in contents.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if value.get("type").and_then(|v| v.as_str()) != Some("agent_approval_decision") {
            continue;
        }
        if value.get("id").and_then(|v| v.as_str()) != Some(artifact_hash) {
            continue;
        }
        if let Some(next) = value.get("status").and_then(|v| v.as_str()) {
            status = Some(next.to_string());
        }
    }
    Ok(status)
}

pub(crate) fn run_agent_cpx_risk_subcommand(args: &[String]) -> ExitCode {
    let mut json = false;
    let mut human = false;
    let mut audit_log: Option<String> = None;
    let mut approval_log: Option<String> = None;
    let mut idx = 0;
    let command = args.first().map(|s| s.as_str());
    if !matches!(command, Some("validate" | "evaluate")) {
        eprintln!(
            "cruft agent cpx-risk: usage: cruft agent cpx-risk validate|evaluate [--json|--human] [--audit-log <audit.jsonl>] [--approval-log <approval.jsonl>] <risk-artifact.json>"
        );
        return ExitCode::from(64);
    }
    let command = command.unwrap();
    idx += 1;
    let mut path: Option<&str> = None;
    while idx < args.len() {
        match args[idx].as_str() {
            "--json" => json = true,
            "--human" => human = true,
            arg if arg.starts_with("--audit-log=") => {
                audit_log = Some(arg["--audit-log=".len()..].to_string());
            }
            "--audit-log" => {
                idx += 1;
                let Some(value) = args.get(idx) else {
                    eprintln!("cruft agent cpx-risk {command}: --audit-log requires an argument");
                    return ExitCode::from(64);
                };
                audit_log = Some(value.clone());
            }
            arg if arg.starts_with("--approval-log=") => {
                approval_log = Some(arg["--approval-log=".len()..].to_string());
            }
            "--approval-log" => {
                idx += 1;
                let Some(value) = args.get(idx) else {
                    eprintln!(
                        "cruft agent cpx-risk {command}: --approval-log requires an argument"
                    );
                    return ExitCode::from(64);
                };
                approval_log = Some(value.clone());
            }
            arg if path.is_none() => path = Some(arg),
            arg => {
                eprintln!("cruft agent cpx-risk validate: unexpected argument {arg}");
                return ExitCode::from(64);
            }
        }
        idx += 1;
    }
    let Some(path) = path else {
        eprintln!(
            "cruft agent cpx-risk {command}: usage: cruft agent cpx-risk {command} [--json|--human] [--audit-log <audit.jsonl>] [--approval-log <approval.jsonl>] <risk-artifact.json>"
        );
        return ExitCode::from(64);
    };
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(e) => {
            let reason = format!("cannot read {path}: {e}");
            if command == "evaluate" {
                if let Some(audit_log) = audit_log.as_deref() {
                    if let Err(audit_reason) =
                        append_cpx_risk_denial_audit(audit_log, None, path, &reason)
                    {
                        eprintln!("cruft agent cpx-risk evaluate: {audit_reason}");
                        return ExitCode::from(74);
                    }
                }
            }
            eprintln!("cruft agent cpx-risk {command}: {reason}");
            return ExitCode::from(66);
        }
    };
    let artifact_hash = agent_source_hash(&source);
    let value = match serde_json::from_str::<serde_json::Value>(&source) {
        Ok(value) => value,
        Err(e) => {
            let reason = format!("invalid JSON: {e}");
            if command == "evaluate" {
                if let Some(audit_log) = audit_log.as_deref() {
                    if let Err(audit_reason) =
                        append_cpx_risk_denial_audit(audit_log, Some(&artifact_hash), path, &reason)
                    {
                        eprintln!("cruft agent cpx-risk evaluate: {audit_reason}");
                        return ExitCode::from(74);
                    }
                }
            }
            eprintln!("cruft agent cpx-risk {command}: {reason}");
            return ExitCode::from(65);
        }
    };
    if command == "evaluate" {
        return match evaluate_cpx_risk_policy(&value) {
            Ok(mut decision) => {
                let mut approval_status: Option<String> = None;
                if decision.decision == "requires_approval" {
                    if let Some(approval_log) = approval_log.as_deref() {
                        match cpx_risk_approval_status(approval_log, &artifact_hash) {
                            Ok(Some(status)) if status == "allowed" => {
                                approval_status = Some(status);
                                decision.decision = "allow".to_string();
                                decision.reason =
                                    "artifact approval log contains allowed decision".to_string();
                            }
                            Ok(Some(status)) if status == "denied" => {
                                approval_status = Some(status);
                                decision.decision = "blocked".to_string();
                                decision.reason =
                                    "artifact approval log contains denied decision".to_string();
                            }
                            Ok(Some(status)) => {
                                approval_status = Some(status);
                            }
                            Ok(None) => {
                                approval_status = Some("pending".to_string());
                                if let Err(reason) = append_cpx_risk_approval_pending(
                                    approval_log,
                                    &artifact_hash,
                                    &decision,
                                ) {
                                    eprintln!("cruft agent cpx-risk evaluate: {reason}");
                                    return ExitCode::from(74);
                                }
                            }
                            Err(reason) => {
                                eprintln!("cruft agent cpx-risk evaluate: {reason}");
                                return ExitCode::from(66);
                            }
                        }
                    }
                }
                if let Some(audit_log) = audit_log.as_deref() {
                    if let Err(reason) = append_cpx_risk_audit(audit_log, &artifact_hash, &decision)
                    {
                        eprintln!("cruft agent cpx-risk evaluate: {reason}");
                        return ExitCode::from(74);
                    }
                }
                if json {
                    let approval_status_json = approval_status
                        .as_deref()
                        .map(json_string_literal)
                        .unwrap_or_else(|| "null".to_string());
                    println!(
                        "{{\"schema\":\"cruft.agent_cpx_risk_decision.v1\",\"valid\":true,\"artifact_hash\":{},\"requested\":{},\"source_kind\":{},\"policy_disposition\":{},\"execution\":{},\"risk_count\":{},\"unknown_count\":{},\"required_permission_count\":{},\"audit_attestation_accepted_count\":{},\"audit_attestation_rejected_count\":{},\"decision\":{},\"reason\":{},\"approval_status\":{}}}",
                        json_string_literal(&artifact_hash),
                        json_string_literal(&decision.validation.requested),
                        json_string_literal(&decision.validation.source_kind),
                        json_string_literal(&decision.validation.disposition),
                        json_string_literal(&decision.validation.execution),
                        decision.validation.risk_count,
                        decision.validation.unknown_count,
                        decision.validation.required_permission_count,
                        decision.validation.audit_attestation_accepted_count,
                        decision.validation.audit_attestation_rejected_count,
                        json_string_literal(&decision.decision),
                        json_string_literal(&decision.reason),
                        approval_status_json
                    );
                } else {
                    let _ = human;
                    println!(
                        "cpx risk decision: requested={} decision={} reason={} approval_status={}",
                        decision.validation.requested,
                        decision.decision,
                        decision.reason,
                        approval_status.as_deref().unwrap_or("not_required")
                    );
                }
                match decision.decision.as_str() {
                    "blocked" => ExitCode::from(78),
                    "requires_approval" if approval_status.as_deref() == Some("pending") => {
                        ExitCode::from(79)
                    }
                    _ => ExitCode::SUCCESS,
                }
            }
            Err(reason) => {
                if let Some(audit_log) = audit_log.as_deref() {
                    if let Err(audit_reason) =
                        append_cpx_risk_denial_audit(audit_log, Some(&artifact_hash), path, &reason)
                    {
                        eprintln!("cruft agent cpx-risk evaluate: {audit_reason}");
                        return ExitCode::from(74);
                    }
                }
                if json {
                    println!(
                        "{{\"schema\":\"cruft.agent_cpx_risk_decision.v1\",\"valid\":false,\"decision\":\"blocked\",\"reason\":{}}}",
                        json_string_literal(&reason)
                    );
                } else {
                    eprintln!("cruft agent cpx-risk evaluate: {reason}");
                }
                ExitCode::from(65)
            }
        };
    }
    match validate_cpx_risk_artifact(&value) {
        Ok(result) => {
            if json {
                println!(
                    "{{\"schema\":\"cruft.agent_cpx_risk_validation.v1\",\"valid\":true,\"requested\":{},\"source_kind\":{},\"policy_disposition\":{},\"execution\":{},\"risk_count\":{},\"unknown_count\":{},\"required_permission_count\":{},\"audit_attestation_accepted_count\":{},\"audit_attestation_rejected_count\":{}}}",
                    json_string_literal(&result.requested),
                    json_string_literal(&result.source_kind),
                    json_string_literal(&result.disposition),
                    json_string_literal(&result.execution),
                    result.risk_count,
                    result.unknown_count,
                    result.required_permission_count,
                    result.audit_attestation_accepted_count,
                    result.audit_attestation_rejected_count
                );
            } else {
                let _ = human;
                println!(
                    "valid cpx risk artifact: requested={} source={} disposition={} execution={}",
                    result.requested, result.source_kind, result.disposition, result.execution
                );
            }
            ExitCode::SUCCESS
        }
        Err(reason) => {
            if json {
                println!(
                    "{{\"schema\":\"cruft.agent_cpx_risk_validation.v1\",\"valid\":false,\"reason\":{}}}",
                    json_string_literal(&reason)
                );
            } else {
                eprintln!("cruft agent cpx-risk validate: {reason}");
            }
            ExitCode::from(65)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_artifact() -> serde_json::Value {
        serde_json::from_str(
            r#"{
              "schema": "cruft.package_exec_risk.v1",
              "tool": "cpx",
              "requested": "left-pad@1.3.0",
              "source": {
                "kind": "registry-unfetched",
                "registry": "https://registry.npmjs.org",
                "local_package_dir": null,
                "resolved_executable": null
              },
              "packages": [
                {"name": "left-pad", "range": "1.3.0", "status": "metadata_unfetched"}
              ],
              "unknown_facts": [
                {"field": "registry_metadata", "status": "unavailable", "reason": "risk_json_no_exec_does_not_fetch"}
              ],
              "risks": [
                {"kind": "native-addon", "level": "advisory", "subject": "pkg", "reason": "marker"}
              ],
              "policy_disposition": "unknown",
              "execution": "not_run"
            }"#,
        )
        .unwrap()
    }

    fn artifact_with_execution(execution: &str) -> serde_json::Value {
        let src = format!(
            r#"{{
              "schema": "cruft.package_exec_risk.v1",
              "tool": "cpx",
              "requested": "left-pad@1.3.0",
              "source": {{
                "kind": "registry-unfetched",
                "registry": "https://registry.npmjs.org",
                "local_package_dir": null,
                "resolved_executable": null
              }},
              "packages": [
                {{"name": "left-pad", "range": "1.3.0", "status": "metadata_unfetched"}}
              ],
              "unknown_facts": [
                {{"field": "registry_metadata", "status": "unavailable", "reason": "risk_json_no_exec_does_not_fetch"}}
              ],
              "risks": [
                {{"kind": "native-addon", "level": "advisory", "subject": "pkg", "reason": "marker"}}
              ],
              "policy_disposition": "unknown",
              "execution": "{execution}"
            }}"#
        );
        serde_json::from_str(&src).unwrap()
    }

    fn artifact_with_policy(disposition: &str, risks: bool, unknowns: bool) -> serde_json::Value {
        let risks_json = if risks {
            r#"[{"kind": "native-addon", "level": "advisory", "subject": "pkg", "reason": "marker"}]"#
        } else {
            "[]"
        };
        let unknowns_json = if unknowns {
            r#"[{"field": "registry_metadata", "status": "unavailable", "reason": "risk_json_no_exec_does_not_fetch"}]"#
        } else {
            "[]"
        };
        let src = format!(
            r#"{{
              "schema": "cruft.package_exec_risk.v1",
              "tool": "cpx",
              "requested": "left-pad@1.3.0",
              "source": {{
                "kind": "registry-unfetched",
                "registry": "https://registry.npmjs.org",
                "local_package_dir": null,
                "resolved_executable": null
              }},
              "packages": [
                {{"name": "left-pad", "range": "1.3.0", "status": "metadata_unfetched"}}
              ],
              "unknown_facts": {unknowns_json},
              "risks": {risks_json},
              "policy_disposition": "{disposition}",
              "execution": "not_run"
            }}"#
        );
        serde_json::from_str(&src).unwrap()
    }

    fn artifact_with_audit_attestation(status: &str, reason: Option<&str>) -> serde_json::Value {
        let reason_json = reason
            .map(|reason| format!(r#","reason":"{reason}""#))
            .unwrap_or_default();
        let src = format!(
            r#"{{
              "schema": "cruft.package_exec_risk.v1",
              "tool": "cpx",
              "requested": "left-pad@1.3.0",
              "source": {{
                "kind": "registry-unfetched",
                "registry": "https://registry.npmjs.org",
                "local_package_dir": null,
                "resolved_executable": null
              }},
              "packages": [
                {{"name": "left-pad", "range": "1.3.0", "status": "metadata_unfetched"}}
              ],
              "unknown_facts": [],
              "risks": [],
              "audit_attestations": [
                {{
                  "status": "{status}",
                  "issuer": "Example Audit Co",
                  "issuer_key_id": "example-key-1",
                  "scope": "package-artifact",
                  "package": {{
                    "name": "left-pad",
                    "version": "1.3.0",
                    "registry": "https://registry.npmjs.org",
                    "integrity": "sha512-example"
                  }}
                  {reason_json}
                }}
              ],
              "policy_disposition": "allow",
              "execution": "not_run"
            }}"#
        );
        serde_json::from_str(&src).unwrap()
    }

    #[test]
    fn validates_cpx_risk_schema_v1() {
        let result = validate_cpx_risk_artifact(&valid_artifact()).unwrap();
        assert_eq!(result.requested, "left-pad@1.3.0");
        assert_eq!(result.source_kind, "registry-unfetched");
        assert_eq!(result.disposition, "unknown");
        assert_eq!(result.risk_count, 1);
        assert_eq!(result.unknown_count, 1);
    }

    #[test]
    fn validates_accepted_audit_attestation_fact_counts() {
        let result =
            validate_cpx_risk_artifact(&artifact_with_audit_attestation("accepted", None)).unwrap();
        assert_eq!(result.audit_attestation_accepted_count, 1);
        assert_eq!(result.audit_attestation_rejected_count, 0);
    }

    #[test]
    fn evaluates_rejected_audit_attestation_as_requires_approval() {
        let decision = evaluate_cpx_risk_policy(&artifact_with_audit_attestation(
            "rejected",
            Some("signature expired"),
        ))
        .unwrap();
        assert_eq!(decision.decision, "requires_approval");
        assert!(decision.reason.contains("rejected audit attestation"));
        assert_eq!(decision.validation.audit_attestation_accepted_count, 0);
        assert_eq!(decision.validation.audit_attestation_rejected_count, 1);
    }

    #[test]
    fn rejects_malformed_cpx_risk_schema() {
        let artifact = artifact_with_execution("ran");
        let err = validate_cpx_risk_artifact(&artifact).unwrap_err();
        assert!(err.contains("unsupported execution"), "{err}");
    }

    #[test]
    fn evaluates_unknown_artifact_as_requires_approval() {
        let decision = evaluate_cpx_risk_policy(&valid_artifact()).unwrap();
        assert_eq!(decision.decision, "requires_approval");
        assert!(decision.reason.contains("unknown package facts"));
    }

    #[test]
    fn evaluates_allow_with_blocking_risk_as_requires_approval() {
        let artifact = artifact_with_policy("allow", true, true);
        let decision = evaluate_cpx_risk_policy(&artifact).unwrap();
        assert_eq!(decision.decision, "requires_approval");
        assert!(decision.reason.contains("package risk kind"));
    }

    #[test]
    fn evaluates_low_risk_allow_as_allow() {
        let artifact = artifact_with_policy("allow", false, false);
        let decision = evaluate_cpx_risk_policy(&artifact).unwrap();
        assert_eq!(decision.decision, "allow");
    }

    #[test]
    fn appends_cpx_risk_audit_row() {
        let decision = evaluate_cpx_risk_policy(&valid_artifact()).unwrap();
        let path = std::env::temp_dir().join(format!(
            "cruft-cpx-risk-audit-{}-{}.jsonl",
            std::process::id(),
            agent_source_hash(&format!("{:?}", std::time::SystemTime::now()))
        ));
        append_cpx_risk_audit(
            path.to_str().unwrap(),
            "fnv1a64:0123456789abcdef",
            &decision,
        )
        .unwrap();
        let log = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(
            log.contains("\"type\":\"agent_cpx_risk_decision\""),
            "{log}"
        );
        assert!(
            log.contains("\"artifact_hash\":\"fnv1a64:0123456789abcdef\""),
            "{log}"
        );
        assert!(log.contains("\"decision\":\"requires_approval\""), "{log}");
    }

    #[test]
    fn appends_cpx_risk_denial_audit_row() {
        let path = std::env::temp_dir().join(format!(
            "cruft-cpx-risk-denial-{}-{}.jsonl",
            std::process::id(),
            agent_source_hash(&format!("{:?}", std::time::SystemTime::now()))
        ));
        append_cpx_risk_denial_audit(
            path.to_str().unwrap(),
            Some("fnv1a64:fedcba9876543210"),
            "risk.json",
            "unsupported execution \"ran\"",
        )
        .unwrap();
        let log = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(log.contains("\"type\":\"agent_cpx_risk_denial\""), "{log}");
        assert!(log.contains("\"decision\":\"blocked\""), "{log}");
        assert!(
            log.contains("\"artifact_hash\":\"fnv1a64:fedcba9876543210\""),
            "{log}"
        );
    }
}
