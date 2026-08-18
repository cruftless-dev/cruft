use std::process::ExitCode;

use super::doctor::agent_doctor_json;
use super::integrity::{agent_integrity_for_path, agent_source_hash};
use super::policy::{
    agent_policy_load_target, agent_policy_path_arg, agent_policy_string_array_field,
    agent_policy_string_field, agent_policy_string_map_field,
    agent_policy_validate_value_with_options, AgentPolicyValidationOptions,
};
use super::run::agent_validate_run_id;
use super::tools::{agent_collect_fs_read_caps, AgentFsReadFile};

fn json_escape(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < '\u{20}' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn agent_bundle_write(path: &std::path::Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    std::fs::write(path, contents).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

fn agent_bundle_file_hash(path: &std::path::Path) -> Result<(u64, String), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let contents = String::from_utf8_lossy(&bytes);
    Ok((bytes.len() as u64, agent_source_hash(&contents)))
}

fn agent_bundle_digest_manifest(out_dir: &std::path::Path) -> Result<String, String> {
    let mut entries = Vec::new();
    for rel in [
        "agent-policy.json",
        "source-manifest.json",
        "doctor.json",
        "run-artifact-manifest.json",
        "replay.json",
        "replay.txt",
        "audit-redacted.jsonl",
        "README.txt",
    ] {
        let path = out_dir.join(rel);
        if !path.is_file() {
            continue;
        }
        let (bytes, hash) = agent_bundle_file_hash(&path)?;
        entries.push(format!(
            "{{\"path\":{},\"bytes\":{},\"hash\":{}}}",
            json_string_literal(rel),
            bytes,
            json_string_literal(&hash)
        ));
    }
    let payload = format!(
        "{{\"schema_version\":\"cruft-agent-evidence-bundle.v1\",\"type\":\"agent_evidence_bundle\",\"signature_status\":\"unsigned\",\"signature_owner\":\"P-AGENT-CRYPTO-SIGNATURE-KEY-MATERIAL\",\"nonclaims\":[\"fnv1a64 is a deterministic tamper-evidence checksum, not a cryptographic signature\",\"verification proves bundle files still match this manifest; it does not authenticate the publisher\"],\"entries\":[{}]}}\n",
        entries.join(",")
    );
    Ok(payload)
}

pub(crate) fn run_agent_bundle_verify_subcommand(args: &[String]) -> ExitCode {
    let Some(path) = args.first() else {
        eprintln!("cruft agent bundle-verify: usage: cruft agent bundle-verify <bundle-dir>");
        return ExitCode::from(64);
    };
    if args.len() != 1 {
        eprintln!("cruft agent bundle-verify: unexpected argument {}", args[1]);
        return ExitCode::from(64);
    }
    let dir = std::path::Path::new(path);
    let manifest_path = dir.join("evidence-bundle.json");
    let manifest = match std::fs::read_to_string(&manifest_path) {
        Ok(manifest) => manifest,
        Err(e) => {
            eprintln!(
                "cruft agent bundle-verify: cannot read {}: {e}",
                manifest_path.display()
            );
            return ExitCode::from(66);
        }
    };
    let value = match serde_json::from_str::<serde_json::Value>(&manifest) {
        Ok(value) => value,
        Err(e) => {
            eprintln!("cruft agent bundle-verify: invalid evidence-bundle.json: {e}");
            return ExitCode::from(65);
        }
    };
    if value.get("type").and_then(|v| v.as_str()) != Some("agent_evidence_bundle") {
        eprintln!("cruft agent bundle-verify: evidence-bundle.json has wrong type");
        return ExitCode::from(65);
    }
    let Some(entries) = value.get("entries").and_then(|v| v.as_array()) else {
        eprintln!("cruft agent bundle-verify: evidence-bundle.json has no entries array");
        return ExitCode::from(65);
    };
    for entry in entries {
        let Some(rel) = entry.get("path").and_then(|v| v.as_str()) else {
            eprintln!("cruft agent bundle-verify: entry missing path");
            return ExitCode::from(65);
        };
        if rel.starts_with('/') || rel.contains("..") {
            eprintln!("cruft agent bundle-verify: rejected unsafe entry path {rel:?}");
            return ExitCode::from(65);
        }
        let expected_bytes = entry
            .get("bytes")
            .and_then(|v| v.as_u64())
            .unwrap_or(u64::MAX);
        let expected_hash = entry.get("hash").and_then(|v| v.as_str()).unwrap_or("");
        match agent_bundle_file_hash(&dir.join(rel)) {
            Ok((actual_bytes, actual_hash)) => {
                if actual_bytes != expected_bytes || actual_hash != expected_hash {
                    eprintln!(
                        "cruft agent bundle-verify: mismatch for {rel}: expected {expected_bytes} {expected_hash}, got {actual_bytes} {actual_hash}"
                    );
                    return ExitCode::from(65);
                }
            }
            Err(e) => {
                eprintln!("cruft agent bundle-verify: {e}");
                return ExitCode::from(66);
            }
        }
    }
    println!(
        "verified agent evidence bundle: {} entries, signature_status={}",
        entries.len(),
        value
            .get("signature_status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
    );
    ExitCode::SUCCESS
}

fn agent_bundle_manifest_entry(
    base: &std::path::Path,
    kind: &str,
    specifier: &str,
    raw_path: &str,
) -> Result<String, String> {
    let resolved = agent_policy_path_arg(base, raw_path);
    let integrity = agent_integrity_for_path(kind, Some(specifier), &resolved)?;
    Ok(format!(
        "{{\"kind\":{},\"specifier\":{},\"path\":{},\"integrity\":{}}}",
        json_string_literal(kind),
        json_string_literal(specifier),
        json_string_literal(raw_path),
        json_string_literal(&integrity)
    ))
}

fn agent_bundle_fs_read_manifest_entry(file: &AgentFsReadFile) -> String {
    format!(
        "{{\"kind\":\"fs-read\",\"root\":{},\"path\":{},\"relative\":{},\"bytes\":{},\"entry_kind\":{},\"readable\":{},\"reason\":{},\"source_hash\":{}}}",
        json_string_literal(&file.root),
        json_string_literal(&file.path),
        json_string_literal(&file.relative),
        file.bytes,
        json_string_literal(&file.kind),
        if file.readable { "true" } else { "false" },
        json_string_literal(&file.reason),
        if file.source_hash.is_empty() {
            "null".to_string()
        } else {
            json_string_literal(&file.source_hash)
        }
    )
}

fn agent_filter_audit_for_run_id(contents: &str, selected_run_id: &str) -> Result<String, String> {
    let mut out = String::new();
    let mut active = false;
    let mut matched = false;
    for line in contents.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let ty = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let record_run_id = value.get("run_id").and_then(|v| v.as_str());
        let starts_selected = ty == "run_start" && record_run_id == Some(selected_run_id);
        if starts_selected {
            active = true;
            matched = true;
        }
        let include =
            active || record_run_id == Some(selected_run_id) || (ty == "run_end" && active);
        if include {
            out.push_str(line);
            out.push('\n');
        }
        if ty == "run_end" && active {
            active = false;
        }
    }
    if matched {
        Ok(out)
    } else {
        Err(format!(
            "audit log contains no run_id {:?} to bundle",
            selected_run_id
        ))
    }
}

fn agent_latest_failed_run_id(contents: &str) -> Option<String> {
    let mut active_run_id: Option<String> = None;
    let mut latest_failed: Option<String> = None;
    for line in contents.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let ty = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if ty == "run_start" {
            active_run_id = value
                .get("run_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| Some("default".to_string()));
            continue;
        }
        if ty == "run_end" {
            let run_id = value
                .get("run_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| active_run_id.clone());
            let Some(run_id) = run_id else {
                continue;
            };
            let status = value.get("status").and_then(|v| v.as_str()).unwrap_or("");
            if status != "ok" {
                latest_failed = Some(run_id);
            }
            active_run_id = None;
        }
    }
    latest_failed
}

fn agent_bundle_run_artifact_manifest(contents: &str) -> String {
    let mut manifests = Vec::new();
    for line in contents.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if value.get("type").and_then(|v| v.as_str()) == Some("run_artifact_manifest") {
            manifests.push(line.trim().to_string());
        }
    }
    format!(
        "{{\"type\":\"agent_support_run_artifact_manifest\",\"version\":1,\"manifests\":[{}]}}\n",
        manifests.join(",")
    )
}

pub(crate) fn run_agent_bundle_subcommand(args: &[String]) -> ExitCode {
    let mut target: Option<&String> = None;
    let mut out: Option<String> = None;
    let mut run_id: Option<String> = None;
    let mut failed = false;
    let mut i = 0usize;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--out" {
            i += 1;
            let Some(value) = args.get(i) else {
                eprintln!("cruft agent bundle: --out requires a directory");
                return ExitCode::from(64);
            };
            out = Some(value.clone());
        } else if let Some(value) = arg.strip_prefix("--out=") {
            out = Some(value.to_string());
        } else if arg == "--run-id" {
            i += 1;
            let Some(value) = args.get(i) else {
                eprintln!("cruft agent bundle: --run-id requires an argument");
                return ExitCode::from(64);
            };
            if !agent_validate_run_id(value) {
                eprintln!("cruft agent bundle: --run-id must be 1-128 chars using ASCII letters, digits, dot, underscore, colon, slash, or hyphen, with no '..' path segment");
                return ExitCode::from(64);
            }
            run_id = Some(value.clone());
        } else if let Some(value) = arg.strip_prefix("--run-id=") {
            if !agent_validate_run_id(value) {
                eprintln!("cruft agent bundle: --run-id must be 1-128 chars using ASCII letters, digits, dot, underscore, colon, slash, or hyphen, with no '..' path segment");
                return ExitCode::from(64);
            }
            run_id = Some(value.to_string());
        } else if arg == "--failed" {
            failed = true;
        } else if target.is_none() {
            target = Some(arg);
        } else {
            eprintln!("cruft agent bundle: unexpected argument {arg}");
            return ExitCode::from(64);
        }
        i += 1;
    }
    let Some(target) = target else {
        eprintln!(
            "cruft agent bundle: usage: cruft agent bundle <project|agent-policy.json> --out <dir> [--run-id=<id>|--failed]"
        );
        return ExitCode::from(64);
    };
    if failed && run_id.is_some() {
        eprintln!("cruft agent bundle: --failed cannot be combined with --run-id");
        return ExitCode::from(64);
    }
    let Some(out) = out else {
        eprintln!("cruft agent bundle: --out <dir> is required");
        return ExitCode::from(64);
    };
    let (policy_path, policy_source, value) = match agent_policy_load_target(target) {
        Ok(policy) => policy,
        Err(e) => {
            eprintln!("cruft agent bundle: {e}");
            return ExitCode::from(65);
        }
    };
    if let Err(errors) = agent_policy_validate_value_with_options(
        &policy_path,
        &value,
        AgentPolicyValidationOptions {
            strict: true,
            project_confined: true,
        },
    ) {
        eprintln!("cruft agent bundle: policy invalid for support export");
        for error in errors {
            eprintln!("{error}");
        }
        return ExitCode::from(65);
    }
    let object = value.as_object().expect("policy object validated at load");
    let base = std::path::Path::new(&policy_path)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let out_dir = std::path::Path::new(&out);
    if out_dir.exists() {
        match std::fs::read_dir(out_dir) {
            Ok(mut entries) => {
                if entries.next().is_some() {
                    eprintln!("cruft agent bundle: {out} is not empty");
                    return ExitCode::from(73);
                }
            }
            Err(e) => {
                eprintln!("cruft agent bundle: cannot inspect {out}: {e}");
                return ExitCode::from(66);
            }
        }
    }
    if let Err(e) = std::fs::create_dir_all(out_dir) {
        eprintln!("cruft agent bundle: cannot create {out}: {e}");
        return ExitCode::from(73);
    }
    if let Err(e) = agent_bundle_write(&out_dir.join("agent-policy.json"), &policy_source) {
        eprintln!("cruft agent bundle: {e}");
        return ExitCode::from(73);
    }
    if let Err(e) = agent_bundle_write(&out_dir.join("doctor.json"), agent_doctor_json()) {
        eprintln!("cruft agent bundle: {e}");
        return ExitCode::from(73);
    }
    let mut manifest_entries = Vec::new();
    if let Ok(Some(agent)) = agent_policy_string_field(object, "agent") {
        let resolved = agent_policy_path_arg(base, agent);
        match agent_integrity_for_path("source", None, &resolved) {
            Ok(integrity) => manifest_entries.push(format!(
                "{{\"kind\":\"agent\",\"specifier\":\"agent\",\"path\":{},\"integrity\":{}}}",
                json_string_literal(agent),
                json_string_literal(&integrity)
            )),
            Err(e) => {
                eprintln!("cruft agent bundle: agent: {e}");
                return ExitCode::from(65);
            }
        }
    }
    for (specifier, raw_path) in
        agent_policy_string_map_field(object, "modules").unwrap_or_default()
    {
        match agent_bundle_manifest_entry(base, "module", &specifier, &raw_path) {
            Ok(entry) => manifest_entries.push(entry),
            Err(e) => {
                eprintln!("cruft agent bundle: module {specifier:?}: {e}");
                return ExitCode::from(65);
            }
        }
    }
    for (specifier, raw_path) in
        agent_policy_string_map_field(object, "packages").unwrap_or_default()
    {
        match agent_bundle_manifest_entry(base, "package", &specifier, &raw_path) {
            Ok(entry) => manifest_entries.push(entry),
            Err(e) => {
                eprintln!("cruft agent bundle: package {specifier:?}: {e}");
                return ExitCode::from(65);
            }
        }
    }
    for (specifier, raw_path) in
        agent_policy_string_map_field(object, "import_hooks").unwrap_or_default()
    {
        match agent_bundle_manifest_entry(base, "import-hook", &specifier, &raw_path) {
            Ok(entry) => manifest_entries.push(entry),
            Err(e) => {
                eprintln!("cruft agent bundle: import hook {specifier:?}: {e}");
                return ExitCode::from(65);
            }
        }
    }
    let fs_read_specs = agent_policy_string_array_field(object, "fs_read")
        .unwrap_or_default()
        .into_iter()
        .map(|path| {
            let resolved = agent_policy_path_arg(base, &path);
            (path, resolved)
        })
        .collect::<Vec<_>>();
    let fs_read_include_patterns =
        agent_policy_string_array_field(object, "fs_read_include").unwrap_or_default();
    let fs_read_exclude_patterns =
        agent_policy_string_array_field(object, "fs_read_exclude").unwrap_or_default();
    match agent_collect_fs_read_caps(
        &fs_read_specs,
        &fs_read_include_patterns,
        &fs_read_exclude_patterns,
    ) {
        Ok(files) => {
            for file in files {
                manifest_entries.push(agent_bundle_fs_read_manifest_entry(&file));
            }
        }
        Err(e) => {
            eprintln!("cruft agent bundle: {e}");
            return ExitCode::from(66);
        }
    }
    let manifest = format!(
        "{{\"type\":\"agent_support_source_manifest\",\"policy\":{},\"entries\":[{}]}}\n",
        json_string_literal("agent-policy.json"),
        manifest_entries.join(",")
    );
    if let Err(e) = agent_bundle_write(&out_dir.join("source-manifest.json"), &manifest) {
        eprintln!("cruft agent bundle: {e}");
        return ExitCode::from(73);
    }
    let mut effective_run_id = run_id.clone();
    if let Ok(Some(audit_log)) = agent_policy_string_field(object, "audit_log") {
        let audit_path = agent_policy_path_arg(base, audit_log);
        match std::fs::read_to_string(&audit_path) {
            Ok(full_audit) => {
                if failed {
                    match agent_latest_failed_run_id(&full_audit) {
                        Some(failed_run_id) => effective_run_id = Some(failed_run_id),
                        None => {
                            eprintln!(
                                "cruft agent bundle: audit log contains no failed run to bundle"
                            );
                            return ExitCode::from(65);
                        }
                    }
                }
                let audit = if let Some(run_id) = effective_run_id.as_deref() {
                    match agent_filter_audit_for_run_id(&full_audit, run_id) {
                        Ok(filtered) => filtered,
                        Err(e) => {
                            eprintln!("cruft agent bundle: {e}");
                            return ExitCode::from(65);
                        }
                    }
                } else {
                    full_audit
                };
                if let Err(e) = agent_bundle_write(&out_dir.join("audit-redacted.jsonl"), &audit) {
                    eprintln!("cruft agent bundle: {e}");
                    return ExitCode::from(73);
                }
                let run_manifest = agent_bundle_run_artifact_manifest(&audit);
                if let Err(e) =
                    agent_bundle_write(&out_dir.join("run-artifact-manifest.json"), &run_manifest)
                {
                    eprintln!("cruft agent bundle: {e}");
                    return ExitCode::from(73);
                }
                let replay_json = agent_replay_json_summary(audit_log, &audit);
                let replay_human = agent_replay_human_summary(audit_log, &audit);
                if let Err(e) = agent_bundle_write(&out_dir.join("replay.json"), &replay_json) {
                    eprintln!("cruft agent bundle: {e}");
                    return ExitCode::from(73);
                }
                if let Err(e) = agent_bundle_write(&out_dir.join("replay.txt"), &replay_human) {
                    eprintln!("cruft agent bundle: {e}");
                    return ExitCode::from(73);
                }
            }
            Err(e) => {
                eprintln!("cruft agent bundle: cannot read audit log {audit_path}: {e}");
                return ExitCode::from(66);
            }
        }
    }
    let readme = if let Some(run_id) = effective_run_id.as_deref() {
        format!(
            "Cruft Agent support bundle\n\nScope: run_id={run_id}\n\nFiles:\n- agent-policy.json: exported policy\n- source-manifest.json: admitted source paths and hashes\n- run-artifact-manifest.json: schema-versioned per-run handoff manifest derived from audit truth, without copied source or secret values\n- doctor.json: current `cruft agent doctor --json` claim boundary\n- replay.json / replay.txt: audit summaries generated without rerunning tenant code\n- audit-redacted.jsonl: policy-produced audit slice for the selected run; fields redacted by runtime policy remain redacted here\n\nNo source files outside the policy manifest are copied.\n"
        )
    } else {
        "Cruft Agent support bundle\n\nScope: full audit log\n\nFiles:\n- agent-policy.json: exported policy\n- source-manifest.json: admitted source paths and hashes\n- run-artifact-manifest.json: schema-versioned per-run handoff manifest derived from audit truth, without copied source or secret values\n- doctor.json: current `cruft agent doctor --json` claim boundary\n- replay.json / replay.txt: audit summaries generated without rerunning tenant code\n- audit-redacted.jsonl: policy-produced audit log; fields redacted by runtime policy remain redacted here\n\nNo source files outside the policy manifest are copied.\n".to_string()
    };
    if let Err(e) = agent_bundle_write(&out_dir.join("README.txt"), &readme) {
        eprintln!("cruft agent bundle: {e}");
        return ExitCode::from(73);
    }
    match agent_bundle_digest_manifest(out_dir) {
        Ok(evidence) => {
            if let Err(e) = agent_bundle_write(&out_dir.join("evidence-bundle.json"), &evidence) {
                eprintln!("cruft agent bundle: {e}");
                return ExitCode::from(73);
            }
        }
        Err(e) => {
            eprintln!("cruft agent bundle: {e}");
            return ExitCode::from(73);
        }
    }
    println!("created support bundle: {}", out_dir.display());
    ExitCode::SUCCESS
}

pub(crate) fn jsonl_type_count(contents: &str, ty: &str) -> usize {
    let needle = format!("\"type\":\"{}\"", json_escape(ty));
    contents
        .lines()
        .filter(|line| line.contains(&needle))
        .count()
}

pub(crate) fn jsonl_tool_family_count(contents: &str, ty: &str, tools: &[&str]) -> usize {
    let type_needle = format!("\"type\":\"{}\"", json_escape(ty));
    contents
        .lines()
        .filter(|line| {
            line.contains(&type_needle)
                && tools.iter().any(|tool| {
                    let tool_needle = format!("\"tool\":\"{}\"", json_escape(tool));
                    line.contains(&tool_needle)
                })
        })
        .count()
}

pub(crate) fn jsonl_artifact_write_count(contents: &str) -> usize {
    jsonl_tool_family_count(contents, "tool_result", &["writeArtifact"])
}

pub(crate) fn jsonl_osv_query_count(contents: &str) -> usize {
    jsonl_tool_family_count(contents, "tool_result", &["osv.query"])
}

pub(crate) fn jsonl_npm_metadata_count(contents: &str) -> usize {
    jsonl_tool_family_count(contents, "tool_result", &["npm.metadata"])
}

pub(crate) fn jsonl_github_issue_read_count(contents: &str) -> usize {
    jsonl_tool_family_count(contents, "tool_result", &["github.issue.read"])
}

pub(crate) fn jsonl_github_pr_read_count(contents: &str) -> usize {
    jsonl_tool_family_count(contents, "tool_result", &["github.pr.read"])
}

pub(crate) fn jsonl_github_pr_files_list_count(contents: &str) -> usize {
    jsonl_tool_family_count(contents, "tool_result", &["github.pr.files.list"])
}

pub(crate) fn jsonl_github_release_latest_read_count(contents: &str) -> usize {
    jsonl_tool_family_count(contents, "tool_result", &["github.release.latest.read"])
}

pub(crate) fn jsonl_github_file_read_count(contents: &str) -> usize {
    jsonl_tool_family_count(contents, "tool_result", &["github.file.read"])
}

pub(crate) fn jsonl_github_compare_read_count(contents: &str) -> usize {
    jsonl_tool_family_count(contents, "tool_result", &["github.compare.read"])
}

pub(crate) fn jsonl_github_commit_read_count(contents: &str) -> usize {
    jsonl_tool_family_count(contents, "tool_result", &["github.commit.read"])
}

pub(crate) fn jsonl_github_repo_read_count(contents: &str) -> usize {
    jsonl_tool_family_count(contents, "tool_result", &["github.repo.read"])
}

pub(crate) fn jsonl_github_workflow_run_read_count(contents: &str) -> usize {
    jsonl_tool_family_count(contents, "tool_result", &["github.workflow.run.read"])
}

pub(crate) fn jsonl_github_workflow_jobs_list_count(contents: &str) -> usize {
    jsonl_tool_family_count(contents, "tool_result", &["github.workflow.jobs.list"])
}

pub(crate) fn jsonl_github_check_runs_list_count(contents: &str) -> usize {
    jsonl_tool_family_count(contents, "tool_result", &["github.check.runs.list"])
}

pub(crate) fn jsonl_model_call_count(contents: &str) -> usize {
    jsonl_tool_family_count(contents, "tool_result", &["model.call"])
}

pub(crate) fn jsonl_process_result_count(contents: &str) -> usize {
    jsonl_tool_family_count(contents, "tool_result", &["process"])
}

pub(crate) fn jsonl_process_timeout_count(contents: &str) -> usize {
    jsonl_tool_family_count(contents, "tool_timeout", &["process"])
}

pub(crate) fn jsonl_process_output_budget_count(contents: &str) -> usize {
    contents
        .lines()
        .filter(|line| {
            line.contains("\"type\":\"process_output_budget\"")
                && line.contains("\"tool\":\"process\"")
        })
        .count()
}

pub(crate) fn jsonl_process_output_stream_count(contents: &str) -> usize {
    contents
        .lines()
        .filter(|line| {
            line.contains("\"type\":\"process_output_stream\"")
                && line.contains("\"tool\":\"process\"")
        })
        .count()
}

#[derive(Default)]
pub(crate) struct AgentProcessOutputReplayStats {
    pub(crate) stream_records: usize,
    pub(crate) stream_captured_bytes: u64,
    pub(crate) stream_truncated_records: usize,
    pub(crate) budget_stdout_bytes: u64,
    pub(crate) budget_stderr_bytes: u64,
    pub(crate) budget_summarized: usize,
    pub(crate) budget_failed_closed: usize,
}

pub(crate) fn jsonl_process_output_replay_stats(contents: &str) -> AgentProcessOutputReplayStats {
    let mut stats = AgentProcessOutputReplayStats::default();
    for line in contents.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(ty) = value.get("type").and_then(|v| v.as_str()) else {
            continue;
        };
        if value.get("tool").and_then(|v| v.as_str()) != Some("process") {
            continue;
        }
        match ty {
            "process_output_stream" => {
                stats.stream_records += 1;
                stats.stream_captured_bytes += value
                    .get("chunk_bytes")
                    .and_then(|v| v.as_u64())
                    .or_else(|| value.get("captured_bytes").and_then(|v| v.as_u64()))
                    .unwrap_or(0);
                if value
                    .get("truncated")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    stats.stream_truncated_records += 1;
                }
            }
            "process_output_budget" => {
                stats.budget_stdout_bytes += value
                    .get("stdout_bytes")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                stats.budget_stderr_bytes += value
                    .get("stderr_bytes")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                match value.get("disposition").and_then(|v| v.as_str()) {
                    Some("summarized") => stats.budget_summarized += 1,
                    Some("failed_closed") => stats.budget_failed_closed += 1,
                    _ => {}
                }
            }
            _ => {}
        }
    }
    stats
}

pub(crate) fn jsonl_named_network_cache_hit_count(contents: &str) -> usize {
    contents
        .lines()
        .filter(|line| {
            line.contains("\"type\":\"tool_result\"")
                && line.contains("\"cache_hit\":true")
                && (line.contains("\"tool\":\"osv.query\"")
                    || line.contains("\"tool\":\"npm.metadata\"")
                    || line.contains("\"tool\":\"github.issue.read\"")
                    || line.contains("\"tool\":\"github.pr.read\"")
                    || line.contains("\"tool\":\"github.pr.files.list\"")
                    || line.contains("\"tool\":\"github.release.latest.read\"")
                    || line.contains("\"tool\":\"github.file.read\"")
                    || line.contains("\"tool\":\"github.compare.read\"")
                    || line.contains("\"tool\":\"github.commit.read\"")
                    || line.contains("\"tool\":\"github.repo.read\"")
                    || line.contains("\"tool\":\"github.workflow.run.read\"")
                    || line.contains("\"tool\":\"github.workflow.jobs.list\"")
                    || line.contains("\"tool\":\"github.check.runs.list\""))
        })
        .count()
}

pub(crate) fn jsonl_named_network_cache_stale_count(contents: &str) -> usize {
    contents
        .lines()
        .filter(|line| line.contains("\"type\":\"named_network_cache_stale\""))
        .count()
}

pub(crate) fn jsonl_named_network_cache_eviction_count(contents: &str) -> usize {
    contents
        .lines()
        .filter(|line| line.contains("\"type\":\"named_network_cache_eviction\""))
        .count()
}

pub(crate) fn jsonl_named_network_retry_count(contents: &str) -> usize {
    contents
        .lines()
        .filter(|line| line.contains("\"type\":\"named_network_retry\""))
        .count()
}

pub(crate) fn jsonl_tool_approval_pending_count(contents: &str) -> usize {
    contents
        .lines()
        .filter(|line| line.contains("\"type\":\"tool_approval_pending\""))
        .count()
}

pub(crate) fn jsonl_tool_approval_granted_count(contents: &str) -> usize {
    contents
        .lines()
        .filter(|line| line.contains("\"type\":\"tool_approval_granted\""))
        .count()
}

pub(crate) fn jsonl_tool_approval_denied_count(contents: &str) -> usize {
    contents
        .lines()
        .filter(|line| line.contains("\"type\":\"tool_approval_denied\""))
        .count()
}

pub(crate) fn jsonl_tool_approval_stale_count(contents: &str) -> usize {
    contents
        .lines()
        .filter(|line| line.contains("\"type\":\"tool_approval_stale\""))
        .count()
}

pub(crate) fn jsonl_schema_validation_count(contents: &str, status: &str) -> usize {
    let status_needle = format!("\"status\":\"{}\"", json_escape(status));
    contents
        .lines()
        .filter(|line| {
            line.contains("\"type\":\"schema_validation\"") && line.contains(&status_needle)
        })
        .count()
}

pub(crate) fn jsonl_unsupported_control_count(contents: &str, control: &str) -> usize {
    let control_needle = format!("\"control\":\"{}\"", json_escape(control));
    contents
        .lines()
        .filter(|line| {
            line.contains("\"type\":\"unsupported_control\"") && line.contains(&control_needle)
        })
        .count()
}

fn json_string_literal(s: &str) -> String {
    format!("\"{}\"", json_escape(s))
}

fn json_string_array_literal(values: &[String]) -> String {
    let parts: Vec<String> = values
        .iter()
        .map(|value| json_string_literal(value))
        .collect();
    format!("[{}]", parts.join(","))
}

fn json_bool(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

pub(crate) fn run_agent_replay_subcommand(args: &[String]) -> ExitCode {
    let mut human = false;
    let mut diff = false;
    let mut event_diff = false;
    let mut path: Option<&String> = None;
    let mut other_path: Option<&String> = None;
    for arg in args {
        if arg == "--human" {
            human = true;
        } else if arg == "--json" {
            human = false;
        } else if arg == "--diff" {
            diff = true;
        } else if arg == "--events" {
            event_diff = true;
        } else if path.is_none() {
            path = Some(arg);
        } else if diff && other_path.is_none() {
            other_path = Some(arg);
        } else {
            eprintln!("cruft agent replay: unexpected argument {arg}");
            return ExitCode::from(64);
        }
    }
    if diff {
        let (Some(left), Some(right)) = (path, other_path) else {
            eprintln!(
                "cruft agent replay: usage: cruft agent replay --diff [--events] [--json|--human] <before.jsonl> <after.jsonl>"
            );
            return ExitCode::from(64);
        };
        return run_agent_replay_diff_subcommand(left, right, human, event_diff);
    }
    if event_diff {
        eprintln!("cruft agent replay: --events requires --diff");
        return ExitCode::from(64);
    }
    let Some(path) = path else {
        eprintln!(
            "cruft agent replay: usage: cruft agent replay [--json|--human] <audit.jsonl> | cruft agent replay --diff [--events] [--json|--human] <before.jsonl> <after.jsonl>"
        );
        return ExitCode::from(64);
    };
    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(e) => {
            eprintln!("cruft agent replay: cannot read {path}: {e}");
            return ExitCode::from(66);
        }
    };
    let events = jsonl_type_count(&contents, "event");
    let tool_calls = jsonl_type_count(&contents, "tool_call");
    let tool_results = jsonl_type_count(&contents, "tool_result");
    let tool_denials = jsonl_type_count(&contents, "tool_denial");
    let tool_errors = jsonl_type_count(&contents, "tool_error");
    let tool_timeouts = jsonl_type_count(&contents, "tool_timeout");
    let fs_read_calls = jsonl_tool_family_count(&contents, "tool_call", &["readFile", "listFiles"]);
    let fs_read_results =
        jsonl_tool_family_count(&contents, "tool_result", &["readFile", "listFiles"]);
    let fs_read_denials =
        jsonl_tool_family_count(&contents, "tool_denial", &["readFile", "listFiles"]);
    let schema_validation_pass = jsonl_schema_validation_count(&contents, "pass");
    let schema_validation_fail = jsonl_schema_validation_count(&contents, "fail");
    let artifact_writes = jsonl_artifact_write_count(&contents);
    let osv_queries = jsonl_osv_query_count(&contents);
    let npm_metadata_queries = jsonl_npm_metadata_count(&contents);
    let github_issue_reads = jsonl_github_issue_read_count(&contents);
    let github_pr_reads = jsonl_github_pr_read_count(&contents);
    let github_pr_files_reads = jsonl_github_pr_files_list_count(&contents);
    let github_release_latest_reads = jsonl_github_release_latest_read_count(&contents);
    let github_file_reads = jsonl_github_file_read_count(&contents);
    let github_compare_reads = jsonl_github_compare_read_count(&contents);
    let github_commit_reads = jsonl_github_commit_read_count(&contents);
    let github_repo_reads = jsonl_github_repo_read_count(&contents);
    let github_workflow_run_reads = jsonl_github_workflow_run_read_count(&contents);
    let github_workflow_jobs_reads = jsonl_github_workflow_jobs_list_count(&contents);
    let github_check_runs_reads = jsonl_github_check_runs_list_count(&contents);
    let model_calls = jsonl_model_call_count(&contents);
    let named_network_cache_hits = jsonl_named_network_cache_hit_count(&contents);
    let named_network_cache_evictions = jsonl_named_network_cache_eviction_count(&contents);
    let named_network_retries = jsonl_named_network_retry_count(&contents);
    let tool_approval_pending = jsonl_tool_approval_pending_count(&contents);
    let tool_approval_granted = jsonl_tool_approval_granted_count(&contents);
    let tool_approval_denied = jsonl_tool_approval_denied_count(&contents);
    let tool_approval_stale = jsonl_tool_approval_stale_count(&contents);
    let process_results = jsonl_process_result_count(&contents);
    let process_timeouts = jsonl_process_timeout_count(&contents);
    let process_output_budgets = jsonl_process_output_budget_count(&contents);
    let process_output_streams = jsonl_process_output_stream_count(&contents);
    let process_output_stats = jsonl_process_output_replay_stats(&contents);
    let worker_host_calls = jsonl_type_count(&contents, "worker_host_call");
    let module_imports = jsonl_type_count(&contents, "module_import");
    let module_results = jsonl_type_count(&contents, "module_result");
    let module_denials = jsonl_type_count(&contents, "module_denial");
    let audit_controls = jsonl_type_count(&contents, "audit_controls");
    let audit_notes = jsonl_type_count(&contents, "audit_note");
    let state_snapshots = jsonl_type_count(&contents, "state_snapshot");
    let state_resets = jsonl_type_count(&contents, "state_reset");
    let session_loads = jsonl_type_count(&contents, "session_load");
    let session_saves = jsonl_type_count(&contents, "session_save");
    let product_resets = jsonl_type_count(&contents, "product_reset");
    let revocations = jsonl_type_count(&contents, "revocation");
    let revocation_denials = jsonl_type_count(&contents, "revocation_denial");
    let run_ids = jsonl_run_ids(&contents);
    let run_ids_json = json_string_array_literal(&run_ids);
    let budget_exceeded = jsonl_type_count(&contents, "budget_exceeded");
    let timeouts = jsonl_type_count(&contents, "timeout");
    let unsupported_controls = jsonl_type_count(&contents, "unsupported_control");
    let unsupported_worker_hosted = jsonl_unsupported_control_count(&contents, "worker_hosted");
    let unsupported_package_graph = jsonl_unsupported_control_count(&contents, "package_graph");
    let unsupported_process_tool = jsonl_unsupported_control_count(&contents, "process_tool");
    let unsupported_osv_query = jsonl_unsupported_control_count(&contents, "osv_query");
    let memory_rss_exceeded = contents
        .lines()
        .filter(|line| {
            line.contains("\"type\":\"post_turn_failure\"")
                && line.contains("\"reason\":\"memory_rss_exceeded\"")
        })
        .count();
    let run_end_ok = contents
        .lines()
        .filter(|line| line.contains("\"type\":\"run_end\"") && line.contains("\"status\":\"ok\""))
        .count();
    let run_end_error = contents
        .lines()
        .filter(|line| line.contains("\"type\":\"run_end\"") && !line.contains("\"status\":\"ok\""))
        .count();
    if human {
        println!("Cruft agent replay");
        println!("audit log: {path}");
        println!("runs: {}", jsonl_type_count(&contents, "run_start"));
        println!(
            "run ids: {}",
            if run_ids.is_empty() {
                "none".to_string()
            } else {
                run_ids.join(",")
            }
        );
        println!(
            "final disposition: ok={} error={}",
            run_end_ok, run_end_error
        );
        println!("events: {events}");
        println!(
            "tools: calls={tool_calls} results={tool_results} denials={tool_denials} errors={tool_errors} timeouts={tool_timeouts}"
        );
        println!("tool approvals: pending={tool_approval_pending} granted={tool_approval_granted} denied={tool_approval_denied} stale={tool_approval_stale}");
        println!(
            "fs read: calls={fs_read_calls} results={fs_read_results} denials={fs_read_denials}"
        );
        println!("schema validation: pass={schema_validation_pass} fail={schema_validation_fail}");
        println!("artifacts: writes={artifact_writes}");
        println!("osv: queries={osv_queries}");
        println!("npm metadata: queries={npm_metadata_queries}");
        println!("github issue: reads={github_issue_reads}");
        println!("github pr: reads={github_pr_reads}");
        println!("github pr files: reads={github_pr_files_reads}");
        println!("github latest release: reads={github_release_latest_reads}");
        println!("github file: reads={github_file_reads}");
        println!("github compare: reads={github_compare_reads}");
        println!("github commit: reads={github_commit_reads}");
        println!("github repo: reads={github_repo_reads}");
        println!("github workflow run: reads={github_workflow_run_reads}");
        println!("github workflow jobs: reads={github_workflow_jobs_reads}");
        println!("github check runs: reads={github_check_runs_reads}");
        println!("model: calls={model_calls}");
        let named_network_cache_stale = jsonl_named_network_cache_stale_count(&contents);
        println!(
            "named network cache: hits={named_network_cache_hits} stale={named_network_cache_stale} evicted={named_network_cache_evictions}"
        );
        println!("named network retry: attempts={named_network_retries}");
        println!(
            "process: results={process_results} timeouts={process_timeouts} output_budgets={process_output_budgets} output_streams={process_output_streams}"
        );
        println!(
            "process output: captured_bytes={} truncated_streams={} budget_stdout_bytes={} budget_stderr_bytes={} summarized={} failed_closed={}",
            process_output_stats.stream_captured_bytes,
            process_output_stats.stream_truncated_records,
            process_output_stats.budget_stdout_bytes,
            process_output_stats.budget_stderr_bytes,
            process_output_stats.budget_summarized,
            process_output_stats.budget_failed_closed
        );
        println!("worker host calls: {worker_host_calls}");
        println!(
            "modules/packages: imports={module_imports} results={module_results} denials={module_denials}"
        );
        println!("audit records: notes={audit_notes} controls={audit_controls}");
        println!("state/session: snapshots={state_snapshots} resets={state_resets} loads={session_loads} saves={session_saves} product_resets={product_resets}");
        println!("revocation: close={revocations} denials={revocation_denials}");
        println!(
            "budgets/failures: budget_exceeded={budget_exceeded} timeouts={timeouts} memory_rss_exceeded={memory_rss_exceeded}"
        );
        println!(
            "unsupported controls: total={unsupported_controls} worker_hosted={unsupported_worker_hosted} package_graph={unsupported_package_graph} process_tool={unsupported_process_tool} osv_query={unsupported_osv_query}"
        );
        println!(
            "effective control records: availability={} resource={} module_policy={} state={}",
            contents.contains("\"availability_controls\""),
            contents.contains("\"resource_controls\""),
            contents.contains("\"module_policy\""),
            contents.contains("\"state_controls\"")
        );
        return ExitCode::SUCCESS;
    }
    println!(
        "{{\"type\":\"agent_replay\",\"audit_log\":{},\"runs\":{},\"run_ids\":{},\"events\":{},\"tool_calls\":{},\"tool_results\":{},\"tool_denials\":{},\"tool_errors\":{},\"tool_timeouts\":{},\"tool_approval_pending\":{},\"tool_approval_granted\":{},\"tool_approval_denied\":{},\"tool_approval_stale\":{},\"fs_read_calls\":{},\"fs_read_results\":{},\"fs_read_denials\":{},\"schema_validation_pass\":{},\"schema_validation_fail\":{},\"artifact_writes\":{},\"osv_queries\":{},\"npm_metadata_queries\":{},\"github_issue_reads\":{},\"github_pr_reads\":{},\"github_pr_files_reads\":{},\"github_release_latest_reads\":{},\"github_file_reads\":{},\"github_compare_reads\":{},\"github_commit_reads\":{},\"github_repo_reads\":{},\"github_workflow_run_reads\":{},\"github_workflow_jobs_reads\":{},\"github_check_runs_reads\":{},\"model_calls\":{},\"named_network_cache_hits\":{},\"named_network_cache_stale\":{},\"named_network_cache_evictions\":{},\"named_network_retries\":{},\"process_results\":{},\"process_timeouts\":{},\"process_output_budgets\":{},\"process_output_streams\":{},\"process_output_stream_captured_bytes\":{},\"process_output_stream_truncated_records\":{},\"process_output_budget_stdout_bytes\":{},\"process_output_budget_stderr_bytes\":{},\"process_output_budget_summarized\":{},\"process_output_budget_failed_closed\":{},\"worker_host_calls\":{},\"module_imports\":{},\"module_results\":{},\"module_denials\":{},\"audit_controls\":{},\"audit_notes\":{},\"state_snapshots\":{},\"state_resets\":{},\"session_loads\":{},\"session_saves\":{},\"product_resets\":{},\"revocations\":{},\"revocation_denials\":{},\"budget_exceeded\":{},\"timeouts\":{},\"unsupported_controls\":{},\"unsupported_control_breakdown\":{{\"worker_hosted\":{},\"package_graph\":{},\"process_tool\":{},\"osv_query\":{}}},\"memory_rss_exceeded\":{},\"run_end_ok\":{},\"run_end_error\":{},\"has_availability_controls\":{},\"has_resource_controls\":{},\"has_module_policy\":{},\"has_state_controls\":{}}}",
        json_string_literal(path),
        jsonl_type_count(&contents, "run_start"),
        run_ids_json,
        events,
        tool_calls,
        tool_results,
        tool_denials,
        tool_errors,
        tool_timeouts,
        tool_approval_pending,
        tool_approval_granted,
        tool_approval_denied,
        tool_approval_stale,
        fs_read_calls,
        fs_read_results,
        fs_read_denials,
        schema_validation_pass,
        schema_validation_fail,
        artifact_writes,
        osv_queries,
        npm_metadata_queries,
        github_issue_reads,
        github_pr_reads,
        github_pr_files_reads,
        github_release_latest_reads,
        github_file_reads,
        github_compare_reads,
        github_commit_reads,
        github_repo_reads,
        github_workflow_run_reads,
        github_workflow_jobs_reads,
        github_check_runs_reads,
        model_calls,
        named_network_cache_hits,
        jsonl_named_network_cache_stale_count(&contents),
        named_network_cache_evictions,
        named_network_retries,
        process_results,
        process_timeouts,
        process_output_budgets,
        process_output_streams,
        process_output_stats.stream_captured_bytes,
        process_output_stats.stream_truncated_records,
        process_output_stats.budget_stdout_bytes,
        process_output_stats.budget_stderr_bytes,
        process_output_stats.budget_summarized,
        process_output_stats.budget_failed_closed,
        worker_host_calls,
        module_imports,
        module_results,
        module_denials,
        audit_controls,
        audit_notes,
        state_snapshots,
        state_resets,
        session_loads,
        session_saves,
        product_resets,
        revocations,
        revocation_denials,
        budget_exceeded,
        timeouts,
        unsupported_controls,
        unsupported_worker_hosted,
        unsupported_package_graph,
        unsupported_process_tool,
        unsupported_osv_query,
        memory_rss_exceeded,
        run_end_ok,
        run_end_error,
        json_bool(contents.contains("\"availability_controls\"")),
        json_bool(contents.contains("\"resource_controls\"")),
        json_bool(contents.contains("\"module_policy\"")),
        json_bool(contents.contains("\"state_controls\""))
    );
    ExitCode::SUCCESS
}

fn run_agent_replay_diff_subcommand(
    left: &str,
    right: &str,
    human: bool,
    event_diff: bool,
) -> ExitCode {
    let left_contents = match std::fs::read_to_string(left) {
        Ok(contents) => contents,
        Err(e) => {
            eprintln!("cruft agent replay: cannot read {left}: {e}");
            return ExitCode::from(66);
        }
    };
    let right_contents = match std::fs::read_to_string(right) {
        Ok(contents) => contents,
        Err(e) => {
            eprintln!("cruft agent replay: cannot read {right}: {e}");
            return ExitCode::from(66);
        }
    };
    let left_json = agent_replay_json_summary(left, &left_contents);
    let right_json = agent_replay_json_summary(right, &right_contents);
    let left_value =
        serde_json::from_str::<serde_json::Value>(&left_json).unwrap_or(serde_json::Value::Null);
    let right_value =
        serde_json::from_str::<serde_json::Value>(&right_json).unwrap_or(serde_json::Value::Null);
    if human {
        if event_diff {
            println!(
                "{}",
                agent_replay_event_diff_human_summary(
                    left,
                    right,
                    &left_value,
                    &right_value,
                    &left_contents,
                    &right_contents
                )
            );
        } else {
            println!(
                "{}",
                agent_replay_diff_human_summary(left, right, &left_value, &right_value)
            );
        }
    } else {
        if event_diff {
            println!(
                "{}",
                agent_replay_event_diff_json_summary(
                    left,
                    right,
                    &left_value,
                    &right_value,
                    &left_contents,
                    &right_contents
                )
            );
        } else {
            println!(
                "{}",
                agent_replay_diff_json_summary(left, right, &left_value, &right_value)
            );
        }
    }
    ExitCode::SUCCESS
}

fn agent_replay_diff_keys() -> &'static [&'static str] {
    &[
        "runs",
        "events",
        "tool_calls",
        "tool_results",
        "tool_denials",
        "tool_errors",
        "fs_read_calls",
        "fs_read_results",
        "fs_read_denials",
        "schema_validation_pass",
        "schema_validation_fail",
        "artifact_writes",
        "osv_queries",
        "process_results",
        "process_timeouts",
        "worker_host_calls",
        "module_imports",
        "module_results",
        "module_denials",
        "audit_controls",
        "audit_notes",
        "state_snapshots",
        "state_resets",
        "session_loads",
        "session_saves",
        "budget_exceeded",
        "timeouts",
        "unsupported_controls",
        "memory_rss_exceeded",
        "run_end_ok",
        "run_end_error",
    ]
}

fn replay_summary_u64(value: &serde_json::Value, key: &str) -> u64 {
    value.get(key).and_then(|v| v.as_u64()).unwrap_or(0)
}

fn replay_summary_run_ids(value: &serde_json::Value) -> Vec<String> {
    value
        .get("run_ids")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn agent_replay_diff_json_summary(
    left_path: &str,
    right_path: &str,
    left: &serde_json::Value,
    right: &serde_json::Value,
) -> String {
    let mut rows = Vec::new();
    let mut changed = 0usize;
    for key in agent_replay_diff_keys() {
        let before = replay_summary_u64(left, key);
        let after = replay_summary_u64(right, key);
        let delta = after as i128 - before as i128;
        if delta != 0 {
            changed += 1;
        }
        rows.push(format!(
            "{{\"field\":{},\"before\":{},\"after\":{},\"delta\":{}}}",
            json_string_literal(key),
            before,
            after,
            delta
        ));
    }
    let left_run_ids = replay_summary_run_ids(left);
    let right_run_ids = replay_summary_run_ids(right);
    format!(
        "{{\"type\":\"agent_replay_diff\",\"left\":{},\"right\":{},\"left_run_ids\":{},\"right_run_ids\":{},\"changed_fields\":{},\"fields\":[{}]}}",
        json_string_literal(left_path),
        json_string_literal(right_path),
        json_string_array_literal(&left_run_ids),
        json_string_array_literal(&right_run_ids),
        changed,
        rows.join(",")
    )
}

fn agent_replay_diff_human_summary(
    left_path: &str,
    right_path: &str,
    left: &serde_json::Value,
    right: &serde_json::Value,
) -> String {
    let left_run_ids = replay_summary_run_ids(left);
    let right_run_ids = replay_summary_run_ids(right);
    let mut out = String::new();
    out.push_str("Cruft agent replay diff\n");
    out.push_str(&format!("before: {left_path}\n"));
    out.push_str(&format!("after: {right_path}\n"));
    out.push_str(&format!(
        "before run ids: {}\n",
        if left_run_ids.is_empty() {
            "none".to_string()
        } else {
            left_run_ids.join(",")
        }
    ));
    out.push_str(&format!(
        "after run ids: {}\n",
        if right_run_ids.is_empty() {
            "none".to_string()
        } else {
            right_run_ids.join(",")
        }
    ));
    out.push_str("field deltas:\n");
    for key in agent_replay_diff_keys() {
        let before = replay_summary_u64(left, key);
        let after = replay_summary_u64(right, key);
        let delta = after as i128 - before as i128;
        out.push_str(&format!("  {key}: {before} -> {after} ({delta:+})\n"));
    }
    out
}

#[derive(Clone)]
struct AgentReplayEvent {
    index: usize,
    run_id: Option<String>,
    kind: String,
    payload: serde_json::Value,
}

fn agent_replay_events(contents: &str) -> Vec<AgentReplayEvent> {
    let mut events = Vec::new();
    for line in contents.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if value.get("type").and_then(|v| v.as_str()) != Some("event") {
            continue;
        }
        let payload = value
            .get("event")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let kind = payload
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        events.push(AgentReplayEvent {
            index: events.len(),
            run_id: value
                .get("run_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            kind,
            payload,
        });
    }
    events
}

fn agent_replay_event_json(event: Option<&AgentReplayEvent>) -> String {
    let Some(event) = event else {
        return "null".to_string();
    };
    format!(
        "{{\"index\":{},\"run_id\":{},\"kind\":{},\"payload\":{}}}",
        event.index,
        event
            .run_id
            .as_deref()
            .map(json_string_literal)
            .unwrap_or_else(|| "null".to_string()),
        json_string_literal(&event.kind),
        event.payload
    )
}

fn agent_replay_event_diff_json_summary(
    left_path: &str,
    right_path: &str,
    left: &serde_json::Value,
    right: &serde_json::Value,
    left_contents: &str,
    right_contents: &str,
) -> String {
    let left_events = agent_replay_events(left_contents);
    let right_events = agent_replay_events(right_contents);
    let max_len = left_events.len().max(right_events.len());
    let mut changes = Vec::new();
    for index in 0..max_len {
        let before = left_events.get(index);
        let after = right_events.get(index);
        if before.map(|e| &e.payload) == after.map(|e| &e.payload) {
            continue;
        }
        let disposition = match (before, after) {
            (Some(_), Some(_)) => "changed",
            (None, Some(_)) => "added",
            (Some(_), None) => "removed",
            (None, None) => "unchanged",
        };
        changes.push(format!(
            "{{\"index\":{},\"disposition\":{},\"before\":{},\"after\":{}}}",
            index,
            json_string_literal(disposition),
            agent_replay_event_json(before),
            agent_replay_event_json(after)
        ));
    }
    let left_run_ids = replay_summary_run_ids(left);
    let right_run_ids = replay_summary_run_ids(right);
    format!(
        "{{\"type\":\"agent_replay_event_diff\",\"left\":{},\"right\":{},\"left_run_ids\":{},\"right_run_ids\":{},\"left_events\":{},\"right_events\":{},\"changed_events\":{},\"events\":[{}]}}",
        json_string_literal(left_path),
        json_string_literal(right_path),
        json_string_array_literal(&left_run_ids),
        json_string_array_literal(&right_run_ids),
        left_events.len(),
        right_events.len(),
        changes.len(),
        changes.join(",")
    )
}

fn agent_replay_event_diff_human_summary(
    left_path: &str,
    right_path: &str,
    left: &serde_json::Value,
    right: &serde_json::Value,
    left_contents: &str,
    right_contents: &str,
) -> String {
    let json = agent_replay_event_diff_json_summary(
        left_path,
        right_path,
        left,
        right,
        left_contents,
        right_contents,
    );
    let value = serde_json::from_str::<serde_json::Value>(&json).unwrap_or(serde_json::Value::Null);
    let left_run_ids = replay_summary_run_ids(left);
    let right_run_ids = replay_summary_run_ids(right);
    let mut out = String::new();
    out.push_str("Cruft agent replay event diff\n");
    out.push_str(&format!("before: {left_path}\n"));
    out.push_str(&format!("after: {right_path}\n"));
    out.push_str(&format!(
        "before run ids: {}\n",
        if left_run_ids.is_empty() {
            "none".to_string()
        } else {
            left_run_ids.join(",")
        }
    ));
    out.push_str(&format!(
        "after run ids: {}\n",
        if right_run_ids.is_empty() {
            "none".to_string()
        } else {
            right_run_ids.join(",")
        }
    ));
    out.push_str(&format!(
        "events: {} -> {} changed={}\n",
        value
            .get("left_events")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        value
            .get("right_events")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        value
            .get("changed_events")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
    ));
    out.push_str("event deltas:\n");
    if let Some(events) = value.get("events").and_then(|v| v.as_array()) {
        for event in events {
            let index = event.get("index").and_then(|v| v.as_u64()).unwrap_or(0);
            let disposition = event
                .get("disposition")
                .and_then(|v| v.as_str())
                .unwrap_or("changed");
            let before_kind = event
                .get("before")
                .and_then(|v| v.get("kind"))
                .and_then(|v| v.as_str())
                .unwrap_or("none");
            let after_kind = event
                .get("after")
                .and_then(|v| v.get("kind"))
                .and_then(|v| v.as_str())
                .unwrap_or("none");
            out.push_str(&format!(
                "  {index}: {disposition} {before_kind} -> {after_kind}\n"
            ));
        }
    }
    out
}

fn jsonl_run_ids(contents: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for line in contents.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(run_id) = value.get("run_id").and_then(|v| v.as_str()) else {
            continue;
        };
        if seen.insert(run_id.to_string()) {
            out.push(run_id.to_string());
        }
    }
    out
}

pub(crate) fn agent_replay_json_summary(path: &str, contents: &str) -> String {
    let events = jsonl_type_count(contents, "event");
    let tool_calls = jsonl_type_count(contents, "tool_call");
    let tool_results = jsonl_type_count(contents, "tool_result");
    let tool_denials = jsonl_type_count(contents, "tool_denial");
    let tool_errors = jsonl_type_count(contents, "tool_error");
    let tool_timeouts = jsonl_type_count(contents, "tool_timeout");
    let fs_read_calls = jsonl_tool_family_count(contents, "tool_call", &["readFile", "listFiles"]);
    let fs_read_results =
        jsonl_tool_family_count(contents, "tool_result", &["readFile", "listFiles"]);
    let fs_read_denials =
        jsonl_tool_family_count(contents, "tool_denial", &["readFile", "listFiles"]);
    let schema_validation_pass = jsonl_schema_validation_count(contents, "pass");
    let schema_validation_fail = jsonl_schema_validation_count(contents, "fail");
    let artifact_writes = jsonl_artifact_write_count(contents);
    let osv_queries = jsonl_osv_query_count(contents);
    let npm_metadata_queries = jsonl_npm_metadata_count(contents);
    let github_issue_reads = jsonl_github_issue_read_count(contents);
    let github_pr_reads = jsonl_github_pr_read_count(contents);
    let github_pr_files_reads = jsonl_github_pr_files_list_count(contents);
    let github_release_latest_reads = jsonl_github_release_latest_read_count(contents);
    let github_file_reads = jsonl_github_file_read_count(contents);
    let github_compare_reads = jsonl_github_compare_read_count(contents);
    let github_commit_reads = jsonl_github_commit_read_count(contents);
    let github_repo_reads = jsonl_github_repo_read_count(contents);
    let github_workflow_run_reads = jsonl_github_workflow_run_read_count(contents);
    let github_workflow_jobs_reads = jsonl_github_workflow_jobs_list_count(contents);
    let github_check_runs_reads = jsonl_github_check_runs_list_count(contents);
    let model_calls = jsonl_model_call_count(contents);
    let named_network_cache_hits = jsonl_named_network_cache_hit_count(contents);
    let named_network_cache_stale = jsonl_named_network_cache_stale_count(contents);
    let named_network_cache_evictions = jsonl_named_network_cache_eviction_count(contents);
    let named_network_retries = jsonl_named_network_retry_count(contents);
    let tool_approval_pending = jsonl_tool_approval_pending_count(contents);
    let tool_approval_granted = jsonl_tool_approval_granted_count(contents);
    let tool_approval_denied = jsonl_tool_approval_denied_count(contents);
    let tool_approval_stale = jsonl_tool_approval_stale_count(contents);
    let process_results = jsonl_process_result_count(contents);
    let process_timeouts = jsonl_process_timeout_count(contents);
    let process_output_budgets = jsonl_process_output_budget_count(contents);
    let process_output_streams = jsonl_process_output_stream_count(contents);
    let process_output_stats = jsonl_process_output_replay_stats(contents);
    let worker_host_calls = jsonl_type_count(contents, "worker_host_call");
    let module_imports = jsonl_type_count(contents, "module_import");
    let module_results = jsonl_type_count(contents, "module_result");
    let module_denials = jsonl_type_count(contents, "module_denial");
    let audit_controls = jsonl_type_count(contents, "audit_controls");
    let audit_notes = jsonl_type_count(contents, "audit_note");
    let state_snapshots = jsonl_type_count(contents, "state_snapshot");
    let state_resets = jsonl_type_count(contents, "state_reset");
    let session_loads = jsonl_type_count(contents, "session_load");
    let session_saves = jsonl_type_count(contents, "session_save");
    let product_resets = jsonl_type_count(contents, "product_reset");
    let revocations = jsonl_type_count(contents, "revocation");
    let revocation_denials = jsonl_type_count(contents, "revocation_denial");
    let run_ids = jsonl_run_ids(contents);
    let run_ids_json = json_string_array_literal(&run_ids);
    let budget_exceeded = jsonl_type_count(contents, "budget_exceeded");
    let timeouts = jsonl_type_count(contents, "timeout");
    let unsupported_controls = jsonl_type_count(contents, "unsupported_control");
    let unsupported_worker_hosted = jsonl_unsupported_control_count(contents, "worker_hosted");
    let unsupported_package_graph = jsonl_unsupported_control_count(contents, "package_graph");
    let unsupported_process_tool = jsonl_unsupported_control_count(contents, "process_tool");
    let unsupported_osv_query = jsonl_unsupported_control_count(contents, "osv_query");
    let memory_rss_exceeded = contents
        .lines()
        .filter(|line| {
            line.contains("\"type\":\"post_turn_failure\"")
                && line.contains("\"reason\":\"memory_rss_exceeded\"")
        })
        .count();
    let run_end_ok = contents
        .lines()
        .filter(|line| line.contains("\"type\":\"run_end\"") && line.contains("\"status\":\"ok\""))
        .count();
    let run_end_error = contents
        .lines()
        .filter(|line| line.contains("\"type\":\"run_end\"") && !line.contains("\"status\":\"ok\""))
        .count();
    format!(
        "{{\"type\":\"agent_replay\",\"audit_log\":{},\"runs\":{},\"run_ids\":{},\"events\":{},\"tool_calls\":{},\"tool_results\":{},\"tool_denials\":{},\"tool_errors\":{},\"tool_timeouts\":{},\"fs_read_calls\":{},\"fs_read_results\":{},\"fs_read_denials\":{},\"schema_validation_pass\":{},\"schema_validation_fail\":{},\"artifact_writes\":{},\"osv_queries\":{},\"npm_metadata_queries\":{},\"github_issue_reads\":{},\"github_pr_reads\":{},\"github_pr_files_reads\":{},\"github_release_latest_reads\":{},\"github_file_reads\":{},\"github_compare_reads\":{},\"github_commit_reads\":{},\"github_repo_reads\":{},\"github_workflow_run_reads\":{},\"github_workflow_jobs_reads\":{},\"github_check_runs_reads\":{},\"model_calls\":{},\"named_network_cache_hits\":{},\"named_network_cache_stale\":{},\"named_network_cache_evictions\":{},\"named_network_retries\":{},\"tool_approval_pending\":{},\"tool_approval_granted\":{},\"tool_approval_denied\":{},\"tool_approval_stale\":{},\"process_results\":{},\"process_timeouts\":{},\"process_output_budgets\":{},\"process_output_streams\":{},\"process_output_stream_captured_bytes\":{},\"process_output_stream_truncated_records\":{},\"process_output_budget_stdout_bytes\":{},\"process_output_budget_stderr_bytes\":{},\"process_output_budget_summarized\":{},\"process_output_budget_failed_closed\":{},\"worker_host_calls\":{},\"module_imports\":{},\"module_results\":{},\"module_denials\":{},\"audit_controls\":{},\"audit_notes\":{},\"state_snapshots\":{},\"state_resets\":{},\"session_loads\":{},\"session_saves\":{},\"product_resets\":{},\"revocations\":{},\"revocation_denials\":{},\"budget_exceeded\":{},\"timeouts\":{},\"unsupported_controls\":{},\"unsupported_control_breakdown\":{{\"worker_hosted\":{},\"package_graph\":{},\"process_tool\":{},\"osv_query\":{}}},\"memory_rss_exceeded\":{},\"run_end_ok\":{},\"run_end_error\":{},\"has_availability_controls\":{},\"has_resource_controls\":{},\"has_module_policy\":{},\"has_state_controls\":{}}}",
        json_string_literal(path),
        jsonl_type_count(contents, "run_start"),
        run_ids_json,
        events,
        tool_calls,
        tool_results,
        tool_denials,
        tool_errors,
        tool_timeouts,
        fs_read_calls,
        fs_read_results,
        fs_read_denials,
        schema_validation_pass,
        schema_validation_fail,
        artifact_writes,
        osv_queries,
        npm_metadata_queries,
        github_issue_reads,
        github_pr_reads,
        github_pr_files_reads,
        github_release_latest_reads,
        github_file_reads,
        github_compare_reads,
        github_commit_reads,
        github_repo_reads,
        github_workflow_run_reads,
        github_workflow_jobs_reads,
        github_check_runs_reads,
        model_calls,
        named_network_cache_hits,
        named_network_cache_stale,
        named_network_cache_evictions,
        named_network_retries,
        tool_approval_pending,
        tool_approval_granted,
        tool_approval_denied,
        tool_approval_stale,
        process_results,
        process_timeouts,
        process_output_budgets,
        process_output_streams,
        process_output_stats.stream_captured_bytes,
        process_output_stats.stream_truncated_records,
        process_output_stats.budget_stdout_bytes,
        process_output_stats.budget_stderr_bytes,
        process_output_stats.budget_summarized,
        process_output_stats.budget_failed_closed,
        worker_host_calls,
        module_imports,
        module_results,
        module_denials,
        audit_controls,
        audit_notes,
        state_snapshots,
        state_resets,
        session_loads,
        session_saves,
        product_resets,
        revocations,
        revocation_denials,
        budget_exceeded,
        timeouts,
        unsupported_controls,
        unsupported_worker_hosted,
        unsupported_package_graph,
        unsupported_process_tool,
        unsupported_osv_query,
        memory_rss_exceeded,
        run_end_ok,
        run_end_error,
        json_bool(contents.contains("\"availability_controls\"")),
        json_bool(contents.contains("\"resource_controls\"")),
        json_bool(contents.contains("\"module_policy\"")),
        json_bool(contents.contains("\"state_controls\""))
    )
}

pub(crate) fn agent_replay_human_summary(path: &str, contents: &str) -> String {
    let json = agent_replay_json_summary(path, contents);
    let value = serde_json::from_str::<serde_json::Value>(&json).unwrap_or(serde_json::Value::Null);
    let get = |key: &str| value.get(key).and_then(|v| v.as_u64()).unwrap_or(0);
    let run_ids = value
        .get("run_ids")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()
                .join(",")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "none".to_string());
    format!(
"Cruft agent replay\naudit log: {path}\nruns: {}\nrun ids: {run_ids}\nfinal disposition: ok={} error={}\nevents: {}\ntools: calls={} results={} denials={} errors={} timeouts={}\ntool approvals: pending={} granted={} denied={}\nfs read: calls={} results={} denials={}\nschema validation: pass={} fail={}\nartifacts: writes={}\nosv: queries={}\nnpm metadata: queries={}\ngithub issue: reads={}\ngithub pr: reads={}\ngithub pr files: reads={}\ngithub latest release: reads={}\ngithub file: reads={}\ngithub compare: reads={}\ngithub commit: reads={}\ngithub repo: reads={}\ngithub workflow run: reads={}\ngithub workflow jobs: reads={}\ngithub check runs: reads={}\nmodel: calls={}\nnamed network cache: hits={} stale={} evicted={}\nnamed network retry: attempts={}\nprocess: results={} timeouts={} output_budgets={} output_streams={}\nprocess output: captured_bytes={} truncated_streams={} budget_stdout_bytes={} budget_stderr_bytes={} summarized={} failed_closed={}\nworker host calls: {}\nmodules/packages: imports={} results={} denials={}\naudit records: notes={} controls={}\nstate/session: snapshots={} resets={} loads={} saves={} product_resets={}\nrevocation: close={} denials={}\nbudgets/failures: budget_exceeded={} timeouts={} memory_rss_exceeded={}\nunsupported controls: total={} worker_hosted={} package_graph={} process_tool={} osv_query={}\neffective control records: availability={} resource={} module_policy={} state={}\n",
        get("runs"),
        get("run_end_ok"),
        get("run_end_error"),
        get("events"),
        get("tool_calls"),
        get("tool_results"),
        get("tool_denials"),
        get("tool_errors"),
        get("tool_timeouts"),
        get("tool_approval_pending"),
        get("tool_approval_granted"),
        get("tool_approval_denied"),
        get("fs_read_calls"),
        get("fs_read_results"),
        get("fs_read_denials"),
        get("schema_validation_pass"),
        get("schema_validation_fail"),
        get("artifact_writes"),
        get("osv_queries"),
        get("npm_metadata_queries"),
        get("github_issue_reads"),
        get("github_pr_reads"),
        get("github_pr_files_reads"),
        get("github_release_latest_reads"),
        get("github_file_reads"),
        get("github_compare_reads"),
        get("github_commit_reads"),
        get("github_repo_reads"),
        get("github_workflow_run_reads"),
        get("github_workflow_jobs_reads"),
        get("github_check_runs_reads"),
        get("model_calls"),
        get("named_network_cache_hits"),
        get("named_network_cache_stale"),
        get("named_network_cache_evictions"),
        get("named_network_retries"),
        get("process_results"),
        get("process_timeouts"),
        get("process_output_budgets"),
        get("process_output_streams"),
        get("process_output_stream_captured_bytes"),
        get("process_output_stream_truncated_records"),
        get("process_output_budget_stdout_bytes"),
        get("process_output_budget_stderr_bytes"),
        get("process_output_budget_summarized"),
        get("process_output_budget_failed_closed"),
        get("worker_host_calls"),
        get("module_imports"),
        get("module_results"),
        get("module_denials"),
        get("audit_notes"),
        get("audit_controls"),
        get("state_snapshots"),
        get("state_resets"),
        get("session_loads"),
        get("session_saves"),
        get("product_resets"),
        get("revocations"),
        get("revocation_denials"),
        get("budget_exceeded"),
        get("timeouts"),
        get("memory_rss_exceeded"),
        get("unsupported_controls"),
        value
            .get("unsupported_control_breakdown")
            .and_then(|v| v.get("worker_hosted"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        value
            .get("unsupported_control_breakdown")
            .and_then(|v| v.get("package_graph"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        value
            .get("unsupported_control_breakdown")
            .and_then(|v| v.get("process_tool"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        value
            .get("unsupported_control_breakdown")
            .and_then(|v| v.get("osv_query"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        value
            .get("has_availability_controls")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        value
            .get("has_resource_controls")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        value
            .get("has_module_policy")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        value
            .get("has_state_controls")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    )
}

#[derive(Default)]
struct AgentHistoryEntry {
    index: usize,
    run_id: String,
    status: String,
    reason: Option<String>,
    events: usize,
    tool_calls: usize,
    tool_results: usize,
    tool_denials: usize,
    tool_errors: usize,
    started_ts_ms: Option<u64>,
    ended_ts_ms: Option<u64>,
}

fn agent_history_audit_log_target(target: &str) -> Result<String, String> {
    let path = std::path::Path::new(target);
    if path.is_dir() || path.extension().and_then(|s| s.to_str()) == Some("json") {
        let (policy_path, _source, value) = agent_policy_load_target(target)?;
        let Some(object) = value.as_object() else {
            return Err("policy root must be an object".to_string());
        };
        let audit_log = agent_policy_string_field(object, "audit_log")?
            .ok_or_else(|| "policy field \"audit_log\" is required".to_string())?;
        let base = std::path::Path::new(&policy_path)
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        return Ok(agent_policy_path_arg(base, audit_log));
    }
    Ok(target.to_string())
}

fn agent_history_entries(contents: &str) -> Vec<AgentHistoryEntry> {
    let mut entries: Vec<AgentHistoryEntry> = Vec::new();
    let mut active: Option<usize> = None;
    for line in contents.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let ty = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if ty == "run_start" {
            let run_id = value
                .get("run_id")
                .and_then(|v| v.as_str())
                .unwrap_or("default")
                .to_string();
            let started_ts_ms = value.get("ts_ms").and_then(|v| v.as_u64());
            entries.push(AgentHistoryEntry {
                index: entries.len() + 1,
                run_id,
                status: "running_or_incomplete".to_string(),
                started_ts_ms,
                ..AgentHistoryEntry::default()
            });
            active = Some(entries.len() - 1);
            continue;
        }
        let Some(index) = active else {
            continue;
        };
        let entry = &mut entries[index];
        match ty {
            "event" => entry.events += 1,
            "tool_call" => entry.tool_calls += 1,
            "tool_result" => entry.tool_results += 1,
            "tool_denial" => entry.tool_denials += 1,
            "tool_error" => entry.tool_errors += 1,
            "run_end" => {
                entry.status = value
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                entry.reason = value
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                entry.ended_ts_ms = value.get("ts_ms").and_then(|v| v.as_u64());
                active = None;
            }
            _ => {}
        }
    }
    entries
}

fn agent_history_json_summary(audit_log: &str, contents: &str) -> String {
    let entries = agent_history_entries(contents);
    let run_ids = entries
        .iter()
        .map(|entry| entry.run_id.clone())
        .collect::<Vec<_>>();
    let rows = entries
        .iter()
        .map(|entry| {
            format!(
                "{{\"index\":{},\"run_id\":{},\"status\":{},\"reason\":{},\"events\":{},\"tool_calls\":{},\"tool_results\":{},\"tool_denials\":{},\"tool_errors\":{},\"started_ts_ms\":{},\"ended_ts_ms\":{}}}",
                entry.index,
                json_string_literal(&entry.run_id),
                json_string_literal(&entry.status),
                entry
                    .reason
                    .as_deref()
                    .map(json_string_literal)
                    .unwrap_or_else(|| "null".to_string()),
                entry.events,
                entry.tool_calls,
                entry.tool_results,
                entry.tool_denials,
                entry.tool_errors,
                entry
                    .started_ts_ms
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "null".to_string()),
                entry
                    .ended_ts_ms
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "null".to_string())
            )
        })
        .collect::<Vec<_>>();
    format!(
        "{{\"type\":\"agent_history\",\"audit_log\":{},\"runs\":{},\"run_ids\":{},\"entries\":[{}]}}",
        json_string_literal(audit_log),
        entries.len(),
        json_string_array_literal(&run_ids),
        rows.join(",")
    )
}

fn agent_history_human_summary(audit_log: &str, contents: &str) -> String {
    let entries = agent_history_entries(contents);
    let mut out = String::new();
    out.push_str("Cruft agent history\n");
    out.push_str(&format!("audit log: {audit_log}\n"));
    out.push_str(&format!("runs: {}\n", entries.len()));
    for entry in entries {
        let reason = entry
            .reason
            .as_deref()
            .map(|reason| format!(" reason={reason}"))
            .unwrap_or_default();
        out.push_str(&format!(
            "- {} {} status={} events={} tools=calls:{} results:{} denials:{} errors:{}{}\n",
            entry.index,
            entry.run_id,
            entry.status,
            entry.events,
            entry.tool_calls,
            entry.tool_results,
            entry.tool_denials,
            entry.tool_errors,
            reason
        ));
    }
    out
}

pub(crate) fn run_agent_history_subcommand(args: &[String]) -> ExitCode {
    let mut human = false;
    let mut target: Option<&String> = None;
    for arg in args {
        if arg == "--human" {
            human = true;
        } else if arg == "--json" {
            human = false;
        } else if target.is_none() {
            target = Some(arg);
        } else {
            eprintln!("cruft agent history: unexpected argument {arg}");
            return ExitCode::from(64);
        }
    }
    let Some(target) = target else {
        eprintln!(
            "cruft agent history: usage: cruft agent history [--json|--human] <project|agent-policy.json|audit.jsonl>"
        );
        return ExitCode::from(64);
    };
    let audit_log = match agent_history_audit_log_target(target) {
        Ok(audit_log) => audit_log,
        Err(e) => {
            eprintln!("cruft agent history: {e}");
            return ExitCode::from(65);
        }
    };
    let contents = match std::fs::read_to_string(&audit_log) {
        Ok(contents) => contents,
        Err(e) => {
            eprintln!("cruft agent history: cannot read {audit_log}: {e}");
            return ExitCode::from(66);
        }
    };
    if human {
        print!("{}", agent_history_human_summary(&audit_log, &contents));
    } else {
        println!("{}", agent_history_json_summary(&audit_log, &contents));
    }
    ExitCode::SUCCESS
}

fn agent_reset_audit_record(
    session_file: &str,
    backup_file: &str,
    mode: &str,
    existed: bool,
    removed: bool,
    restored: bool,
    dry_run: bool,
) -> String {
    let ts_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!(
        "{{\"type\":\"product_reset\",\"ts_ms\":{},\"session_file\":{},\"backup_file\":{},\"mode\":{},\"existed\":{},\"removed\":{},\"restored\":{},\"dry_run\":{},\"scope\":\"session_file_only\",\"rollback\":\"one_backup\"}}\n",
        ts_ms,
        json_string_literal(session_file),
        json_string_literal(backup_file),
        json_string_literal(mode),
        existed,
        removed,
        restored,
        dry_run
    )
}

pub(crate) fn run_agent_reset_subcommand(args: &[String]) -> ExitCode {
    let mut human = false;
    let mut dry_run = false;
    let mut rollback = false;
    let mut target: Option<&String> = None;
    for arg in args {
        if arg == "--human" {
            human = true;
        } else if arg == "--json" {
            human = false;
        } else if arg == "--dry-run" {
            dry_run = true;
        } else if arg == "--rollback" {
            rollback = true;
        } else if target.is_none() {
            target = Some(arg);
        } else {
            eprintln!("cruft agent reset: unexpected argument {arg}");
            return ExitCode::from(64);
        }
    }
    let Some(target) = target else {
        eprintln!(
            "cruft agent reset: usage: cruft agent reset [--dry-run] [--rollback] [--json|--human] <project|agent-policy.json>"
        );
        return ExitCode::from(64);
    };
    let (policy_path, _source, value) = match agent_policy_load_target(target) {
        Ok(policy) => policy,
        Err(e) => {
            eprintln!("cruft agent reset: {e}");
            return ExitCode::from(65);
        }
    };
    if let Err(errors) = agent_policy_validate_value_with_options(
        &policy_path,
        &value,
        AgentPolicyValidationOptions {
            strict: true,
            project_confined: true,
        },
    ) {
        eprintln!("cruft agent reset: policy invalid");
        for error in errors {
            eprintln!("{error}");
        }
        return ExitCode::from(65);
    }
    let object = value.as_object().expect("policy object validated at load");
    let base = std::path::Path::new(&policy_path)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let Some(session_file) = agent_policy_string_field(object, "session_file").unwrap_or(None)
    else {
        eprintln!("cruft agent reset: policy has no session_file to reset");
        return ExitCode::from(65);
    };
    let session_path = agent_policy_path_arg(base, session_file);
    let backup_path = format!("{session_path}.cruft-reset-backup");
    let existed = std::path::Path::new(&session_path).exists();
    let backup_existed = std::path::Path::new(&backup_path).exists();
    let mut removed = false;
    let mut restored = false;
    if rollback {
        if !backup_existed {
            eprintln!("cruft agent reset: rollback backup not found {backup_path}");
            return ExitCode::from(66);
        }
        if !dry_run {
            if let Some(parent) = std::path::Path::new(&session_path).parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    eprintln!(
                        "cruft agent reset: cannot create session directory {}: {e}",
                        parent.display()
                    );
                    return ExitCode::from(73);
                }
            }
            if let Err(e) = std::fs::copy(&backup_path, &session_path) {
                eprintln!(
                    "cruft agent reset: cannot restore session file {session_path} from {backup_path}: {e}"
                );
                return ExitCode::from(73);
            }
            restored = true;
        }
    } else if existed && !dry_run {
        if let Err(e) = std::fs::copy(&session_path, &backup_path) {
            eprintln!("cruft agent reset: cannot create rollback backup {backup_path}: {e}");
            return ExitCode::from(73);
        }
        match std::fs::remove_file(&session_path) {
            Ok(()) => removed = true,
            Err(e) => {
                eprintln!("cruft agent reset: cannot remove session file {session_path}: {e}");
                return ExitCode::from(73);
            }
        }
    }
    if let Ok(Some(audit_log)) = agent_policy_string_field(object, "audit_log") {
        let audit_path = agent_policy_path_arg(base, audit_log);
        if let Some(parent) = std::path::Path::new(&audit_path).parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                eprintln!(
                    "cruft agent reset: cannot create audit directory {}: {e}",
                    parent.display()
                );
                return ExitCode::from(73);
            }
        }
        let record = agent_reset_audit_record(
            &session_path,
            &backup_path,
            if rollback { "rollback" } else { "reset" },
            if rollback { backup_existed } else { existed },
            removed,
            restored,
            dry_run,
        );
        if let Err(e) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&audit_path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, record.as_bytes()))
        {
            eprintln!("cruft agent reset: cannot append audit log {audit_path}: {e}");
            return ExitCode::from(73);
        }
    }
    if human {
        println!("Cruft agent reset");
        println!("policy: {policy_path}");
        println!("session file: {session_path}");
        println!("backup file: {backup_path}");
        println!("mode: {}", if rollback { "rollback" } else { "reset" });
        println!(
            "existed: {}",
            if rollback { backup_existed } else { existed }
        );
        println!("removed: {removed}");
        println!("restored: {restored}");
        println!("dry run: {dry_run}");
        println!("scope: session_file_only");
        println!("rollback: one_backup");
    } else {
        println!(
            "{{\"type\":\"agent_reset\",\"policy\":{},\"session_file\":{},\"backup_file\":{},\"mode\":{},\"existed\":{},\"removed\":{},\"restored\":{},\"dry_run\":{},\"scope\":\"session_file_only\",\"rollback\":\"one_backup\"}}",
            json_string_literal(&policy_path),
            json_string_literal(&session_path),
            json_string_literal(&backup_path),
            json_string_literal(if rollback { "rollback" } else { "reset" }),
            if rollback { backup_existed } else { existed },
            removed,
            restored,
            dry_run
        );
    }
    ExitCode::SUCCESS
}
