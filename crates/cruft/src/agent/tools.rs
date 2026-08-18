use std::process::ExitCode;

use super::integrity::agent_source_hash;
use super::process::is_agent_process_tool_specifier;
use crate::json_string_literal;

pub(crate) fn is_agent_builtin_tool_specifier(tool: &str) -> bool {
    matches!(
        tool,
        "echo"
            | "fail"
            | "slow"
            | "readFile"
            | "listFiles"
            | "writeArtifact"
            | "osv.query"
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
            | "github.check.runs.list"
            | "model.call"
            | "process"
    )
}

#[derive(Clone)]
pub(crate) struct AgentFsReadFile {
    pub(crate) root: String,
    pub(crate) path: String,
    pub(crate) relative: String,
    pub(crate) source: String,
    pub(crate) source_hash: String,
    pub(crate) bytes: usize,
    pub(crate) kind: String,
    pub(crate) readable: bool,
    pub(crate) reason: String,
}

#[derive(Clone)]
struct AgentFsReadIgnorePattern {
    pattern: String,
    negated: bool,
    anchored: bool,
    directory_only: bool,
    path_pattern: bool,
}

#[derive(Clone)]
pub(crate) struct AgentFsWriteRoot {
    pub(crate) root: String,
}

pub(crate) fn agent_collect_fs_read_caps(
    paths: &[(String, String)],
    include_patterns: &[String],
    exclude_patterns: &[String],
) -> Result<Vec<AgentFsReadFile>, String> {
    let mut out = Vec::new();
    for (_label, raw_path) in paths {
        let root = std::fs::canonicalize(raw_path)
            .map_err(|e| format!("cannot canonicalize --fs-read {raw_path}: {e}"))?;
        let ignore_patterns = agent_fs_read_gitignore_patterns(&root);
        let mut collected = 0usize;
        let mut bytes = 0usize;
        agent_collect_fs_read_path(
            &root,
            &root,
            &ignore_patterns,
            include_patterns,
            exclude_patterns,
            &mut collected,
            &mut bytes,
            &mut out,
        )?;
    }
    Ok(out)
}

fn agent_collect_fs_read_path(
    root: &std::path::Path,
    path: &std::path::Path,
    ignore_patterns: &[AgentFsReadIgnorePattern],
    include_patterns: &[String],
    exclude_patterns: &[String],
    collected: &mut usize,
    bytes: &mut usize,
    out: &mut Vec<AgentFsReadFile>,
) -> Result<(), String> {
    const MAX_FILES_PER_CAP: usize = 256;
    const MAX_TOTAL_BYTES_PER_CAP: usize = 1024 * 1024;
    const MAX_TEXT_FILE_BYTES: usize = 256 * 1024;
    let symlink_meta = std::fs::symlink_metadata(path)
        .map_err(|e| format!("cannot inspect fs-read path {}: {e}", path.display()))?;
    if symlink_meta.file_type().is_symlink() {
        agent_push_fs_read_disposition(root, path, out, "symlink", "symlink_refused", 0)?;
        return Ok(());
    }
    if agent_fs_read_ignored(root, path, ignore_patterns) {
        agent_push_fs_read_disposition(root, path, out, "ignored", "gitignore_default", 0)?;
        return Ok(());
    }
    if agent_fs_read_excluded(root, path, exclude_patterns) {
        agent_push_fs_read_disposition(root, path, out, "excluded", "exclude_pattern", 0)?;
        return Ok(());
    }
    let meta = std::fs::metadata(path)
        .map_err(|e| format!("cannot inspect fs-read path {}: {e}", path.display()))?;
    if meta.is_dir() {
        let mut entries = std::fs::read_dir(path)
            .map_err(|e| format!("cannot list fs-read directory {}: {e}", path.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("cannot list fs-read directory {}: {e}", path.display()))?;
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            agent_collect_fs_read_path(
                root,
                &entry.path(),
                ignore_patterns,
                include_patterns,
                exclude_patterns,
                collected,
                bytes,
                out,
            )?;
        }
        return Ok(());
    }
    if !meta.is_file() {
        agent_push_fs_read_disposition(root, path, out, "special", "non_file_refused", 0)?;
        return Ok(());
    }
    if !agent_fs_read_included(root, path, include_patterns) {
        agent_push_fs_read_disposition(root, path, out, "not_included", "include_pattern", 0)?;
        return Ok(());
    }
    if *collected >= MAX_FILES_PER_CAP {
        return Err(format!(
            "fs-read cap {} exceeds file count limit {MAX_FILES_PER_CAP}",
            root.display()
        ));
    }
    let data = std::fs::read(path)
        .map_err(|e| format!("cannot read fs-read file {}: {e}", path.display()))?;
    *bytes = bytes.saturating_add(data.len());
    if *bytes > MAX_TOTAL_BYTES_PER_CAP {
        return Err(format!(
            "fs-read cap {} exceeds byte limit {MAX_TOTAL_BYTES_PER_CAP}",
            root.display()
        ));
    }
    if data.len() > MAX_TEXT_FILE_BYTES {
        agent_push_fs_read_disposition(
            root,
            path,
            out,
            "large",
            "large_file_summary_only",
            data.len(),
        )?;
        *collected += 1;
        return Ok(());
    }
    let source = match String::from_utf8(data) {
        Ok(source) => source,
        Err(_) => {
            agent_push_fs_read_disposition(
                root,
                path,
                out,
                "binary",
                "binary_refused",
                meta.len() as usize,
            )?;
            *collected += 1;
            return Ok(());
        }
    };
    let canonical = std::fs::canonicalize(path)
        .map_err(|e| format!("cannot canonicalize fs-read file {}: {e}", path.display()))?;
    let relative = canonical
        .strip_prefix(root)
        .ok()
        .and_then(|p| p.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| canonical.file_name().and_then(|s| s.to_str()).unwrap_or(""))
        .to_string();
    out.push(AgentFsReadFile {
        root: root.to_string_lossy().into_owned(),
        path: canonical.to_string_lossy().into_owned(),
        relative,
        source_hash: agent_source_hash(&source),
        bytes: source.len(),
        source,
        kind: "text".to_string(),
        readable: true,
        reason: "admitted".to_string(),
    });
    *collected += 1;
    Ok(())
}

fn agent_fs_read_gitignore_patterns(root: &std::path::Path) -> Vec<AgentFsReadIgnorePattern> {
    std::fs::read_to_string(root.join(".gitignore"))
        .ok()
        .map(|source| {
            source
                .lines()
                .filter_map(agent_parse_fs_read_gitignore_line)
                .collect()
        })
        .unwrap_or_default()
}

fn agent_parse_fs_read_gitignore_line(line: &str) -> Option<AgentFsReadIgnorePattern> {
    let mut raw = line.trim();
    if raw.is_empty() {
        return None;
    }
    let mut escaped_leading_marker = false;
    let mut chars = raw.chars();
    match chars.next() {
        Some('#') => return None,
        Some('\\') => {
            if matches!(chars.next(), Some('#' | '!')) {
                raw = &raw[1..];
                escaped_leading_marker = true;
            }
        }
        _ => {}
    }
    let mut negated = false;
    if !escaped_leading_marker {
        if let Some(rest) = raw.strip_prefix('!') {
            negated = true;
            raw = rest;
        }
    }
    if raw.is_empty() {
        return None;
    }
    let anchored = raw.starts_with('/');
    raw = raw.trim_start_matches('/');
    let directory_only = raw.ends_with('/');
    raw = raw.trim_end_matches('/');
    if raw.is_empty() || raw.contains('\\') || raw.split('/').any(|part| part == "..") {
        return None;
    }
    Some(AgentFsReadIgnorePattern {
        pattern: raw.to_string(),
        negated,
        anchored,
        directory_only,
        path_pattern: raw.contains('/'),
    })
}

pub(crate) fn agent_validate_fs_read_pattern(pattern: &str) -> bool {
    !pattern.is_empty()
        && !pattern.starts_with('/')
        && !pattern.contains('\\')
        && !pattern.split('/').any(|part| part == "..")
}

fn agent_fs_read_ignored(
    root: &std::path::Path,
    path: &std::path::Path,
    patterns: &[AgentFsReadIgnorePattern],
) -> bool {
    if path == root {
        return false;
    }
    let is_dir = std::fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_dir())
        .unwrap_or(false);
    let mut ignored = false;
    for pattern in patterns {
        if agent_fs_read_ignore_pattern_matches(root, path, is_dir, pattern) {
            ignored = !pattern.negated;
        }
    }
    ignored
}

fn agent_fs_read_ignore_pattern_matches(
    root: &std::path::Path,
    path: &std::path::Path,
    is_dir: bool,
    pattern: &AgentFsReadIgnorePattern,
) -> bool {
    let relative = path
        .strip_prefix(root)
        .ok()
        .and_then(|p| p.to_str())
        .unwrap_or("");
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if pattern.directory_only && !is_dir && !relative.starts_with(&format!("{}/", pattern.pattern))
    {
        return false;
    }
    if pattern.anchored || pattern.path_pattern {
        relative == pattern.pattern
            || relative.starts_with(&format!("{}/", pattern.pattern))
            || agent_simple_glob_matches(&pattern.pattern, relative)
    } else {
        name == pattern.pattern
            || relative.starts_with(&format!("{}/", pattern.pattern))
            || relative.contains(&format!("/{}/", pattern.pattern))
            || relative.ends_with(&format!("/{}", pattern.pattern))
            || agent_simple_glob_matches(&pattern.pattern, name)
    }
}

fn agent_fs_read_included(
    root: &std::path::Path,
    path: &std::path::Path,
    patterns: &[String],
) -> bool {
    patterns.is_empty() || agent_fs_read_matches_any(root, path, patterns)
}

fn agent_fs_read_excluded(
    root: &std::path::Path,
    path: &std::path::Path,
    patterns: &[String],
) -> bool {
    if path == root {
        return false;
    }
    agent_fs_read_matches_any(root, path, patterns)
}

fn agent_fs_read_matches_any(
    root: &std::path::Path,
    path: &std::path::Path,
    patterns: &[String],
) -> bool {
    let relative = path
        .strip_prefix(root)
        .ok()
        .and_then(|p| p.to_str())
        .unwrap_or("");
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    patterns.iter().any(|pattern| {
        agent_simple_glob_matches(pattern, relative)
            || agent_simple_glob_matches(pattern, name)
            || relative.starts_with(&format!("{pattern}/"))
    })
}

fn agent_simple_glob_matches(pattern: &str, text: &str) -> bool {
    if pattern == "**" || pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return pattern == text;
    }
    let mut rest = text;
    let anchored_start = !pattern.starts_with('*');
    let anchored_end = !pattern.ends_with('*');
    let parts: Vec<&str> = pattern.split('*').filter(|part| !part.is_empty()).collect();
    if parts.is_empty() {
        return true;
    }
    for (idx, part) in parts.iter().enumerate() {
        let Some(pos) = rest.find(part) else {
            return false;
        };
        if idx == 0 && anchored_start && pos != 0 {
            return false;
        }
        rest = &rest[pos + part.len()..];
    }
    !anchored_end || rest.is_empty()
}

fn agent_push_fs_read_disposition(
    root: &std::path::Path,
    path: &std::path::Path,
    out: &mut Vec<AgentFsReadFile>,
    kind: &str,
    reason: &str,
    bytes: usize,
) -> Result<(), String> {
    let relative = path
        .strip_prefix(root)
        .ok()
        .and_then(|p| p.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| path.file_name().and_then(|s| s.to_str()).unwrap_or(""))
        .to_string();
    out.push(AgentFsReadFile {
        root: root.to_string_lossy().into_owned(),
        path: path.to_string_lossy().into_owned(),
        relative,
        source: String::new(),
        source_hash: String::new(),
        bytes,
        kind: kind.to_string(),
        readable: false,
        reason: reason.to_string(),
    });
    Ok(())
}

pub(crate) fn agent_fs_read_caps_js(files: &[AgentFsReadFile]) -> String {
    files
        .iter()
        .map(|file| {
            format!(
                "{{root:{},path:{},relative:{},content:{},source_hash:{},bytes:{},kind:{},readable:{},reason:{}}}",
                json_string_literal(&file.root),
                json_string_literal(&file.path),
                json_string_literal(&file.relative),
                json_string_literal(&file.source),
                json_string_literal(&file.source_hash),
                file.bytes,
                json_string_literal(&file.kind),
                if file.readable { "true" } else { "false" },
                json_string_literal(&file.reason)
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn agent_collect_fs_write_roots(
    paths: &[String],
) -> Result<Vec<AgentFsWriteRoot>, String> {
    let mut out = Vec::new();
    for raw_path in paths {
        if raw_path.is_empty() {
            return Err("--fs-write requires a non-empty path".to_string());
        }
        std::fs::create_dir_all(raw_path)
            .map_err(|e| format!("cannot create --fs-write directory {raw_path}: {e}"))?;
        let root = std::fs::canonicalize(raw_path)
            .map_err(|e| format!("cannot canonicalize --fs-write {raw_path}: {e}"))?;
        let meta = std::fs::metadata(&root)
            .map_err(|e| format!("cannot inspect --fs-write {}: {e}", root.display()))?;
        if !meta.is_dir() {
            return Err(format!("--fs-write {} is not a directory", root.display()));
        }
        out.push(AgentFsWriteRoot {
            root: root.to_string_lossy().into_owned(),
        });
    }
    Ok(out)
}

pub(crate) fn agent_fs_write_roots_js(roots: &[AgentFsWriteRoot]) -> String {
    roots
        .iter()
        .map(|root| format!("{{root:{}}}", json_string_literal(&root.root)))
        .collect::<Vec<_>>()
        .join(",")
}

#[derive(Clone, Copy)]
struct AgentToolManifest {
    name: &'static str,
    available: &'static str,
    worker: &'static str,
    timeout: &'static str,
    authority_class: &'static str,
    grant_requirements: &'static [&'static str],
    input_schema: &'static str,
    output_schema: &'static str,
    audit_rows: &'static [&'static str],
    replay_counters: &'static [&'static str],
    secret_scopes: &'static [&'static str],
    cancellation: &'static str,
    nonclaims: &'static [&'static str],
    residual_owner: Option<&'static str>,
    human_summary: &'static str,
}

const AGENT_TOOL_MANIFEST_SCHEMA_VERSION: &str = "agent-tool-manifest.v1";

const AGENT_TOOL_MANIFESTS: &[AgentToolManifest] = &[
    AgentToolManifest {
        name: "echo",
        available: "true",
        worker: "true",
        timeout: "sync",
        authority_class: "none",
        grant_requirements: &["--tool=echo"],
        input_schema: "json-object",
        output_schema: "echoed-json",
        audit_rows: &["tool_call", "tool_result", "tool_denial"],
        replay_counters: &["tool_call", "tool_result", "tool_denial"],
        secret_scopes: &[],
        cancellation: "turn-timeout-only",
        nonclaims: &["no process authority", "no fs authority", "no network authority"],
        residual_owner: None,
        human_summary: "echo  available=true worker=true timeout=sync audit=tool_call,tool_result schema=args:json-object result:echoed-json",
    },
    AgentToolManifest {
        name: "fail",
        available: "true",
        worker: "true",
        timeout: "sync",
        authority_class: "test-host-failure",
        grant_requirements: &["--tool=fail"],
        input_schema: "json-object",
        output_schema: "error",
        audit_rows: &["tool_call", "tool_error", "tool_denial"],
        replay_counters: &["tool_call", "tool_error", "tool_denial"],
        secret_scopes: &[],
        cancellation: "turn-timeout-only",
        nonclaims: &["no process authority", "no fs authority", "no network authority"],
        residual_owner: None,
        human_summary: "fail  available=true worker=true timeout=sync audit=tool_call,tool_error schema=args:json-object result:error",
    },
    AgentToolManifest {
        name: "slow",
        available: "true",
        worker: "true",
        timeout: "--tool-timeout-ms",
        authority_class: "test-async",
        grant_requirements: &["--tool=slow"],
        input_schema: "{delay_ms?:number,value?:json}",
        output_schema: "{ok,tool,delay_ms,value}",
        audit_rows: &["tool_call", "tool_result", "tool_timeout", "tool_denial"],
        replay_counters: &["tool_call", "tool_result", "tool_timeout", "tool_denial"],
        secret_scopes: &[],
        cancellation: "tool-timeout-settles-promise",
        nonclaims: &["no process authority", "no fs authority", "no network authority"],
        residual_owner: None,
        human_summary: "slow  available=true worker=true timeout=--tool-timeout-ms audit=tool_call,tool_result,tool_timeout schema=args:{delay_ms?:number,value?:json} result:{ok,tool,delay_ms,value}",
    },
    AgentToolManifest {
        name: "readFile",
        available: "with_--fs-read",
        worker: "true",
        timeout: "sync",
        authority_class: "fs-read",
        grant_requirements: &["--tool=readFile", "--fs-read=<path>"],
        input_schema: "{path:string}",
        output_schema: "{path,content,bytes}",
        audit_rows: &["tool_call", "tool_result", "tool_denial"],
        replay_counters: &["tool_call", "tool_result", "tool_denial"],
        secret_scopes: &[],
        cancellation: "turn-timeout-only",
        nonclaims: &["no ambient fs", "read-only precollected path caps only"],
        residual_owner: None,
        human_summary: "readFile  available=with_--fs-read worker=true timeout=sync audit=tool_call,tool_result,tool_denial schema=args:{path:string} result:{path,content,bytes}",
    },
    AgentToolManifest {
        name: "listFiles",
        available: "with_--fs-read",
        worker: "true",
        timeout: "sync",
        authority_class: "fs-read",
        grant_requirements: &["--tool=listFiles", "--fs-read=<path>"],
        input_schema: "{path?:string}",
        output_schema: "{files:[...]}",
        audit_rows: &["tool_call", "tool_result", "tool_denial"],
        replay_counters: &["tool_call", "tool_result", "tool_denial"],
        secret_scopes: &[],
        cancellation: "turn-timeout-only",
        nonclaims: &["no ambient fs", "read-only precollected path caps only"],
        residual_owner: None,
        human_summary: "listFiles  available=with_--fs-read worker=true timeout=sync audit=tool_call,tool_result,tool_denial schema=args:{path?:string} result:{files:[...]}",
    },
    AgentToolManifest {
        name: "writeArtifact",
        available: "with_--fs-write",
        worker: "true",
        timeout: "sync",
        authority_class: "fs-write",
        grant_requirements: &["--tool=writeArtifact", "--fs-write=<dir>"],
        input_schema: "{path:string,content:string}",
        output_schema: "{path,bytes,hash}",
        audit_rows: &["tool_call", "tool_result", "tool_denial", "budget_exceeded"],
        replay_counters: &["tool_call", "tool_result", "tool_denial", "artifact_write"],
        secret_scopes: &[],
        cancellation: "turn-timeout-only",
        nonclaims: &["no ambient fs", "explicit output root only", "overwrite denied"],
        residual_owner: None,
        human_summary: "writeArtifact  available=with_--fs-write worker=true timeout=sync audit=tool_call,tool_result,tool_denial schema=args:{path:string,content:string} result:{path,bytes,hash}",
    },
    AgentToolManifest {
        name: "osv.query",
        available: "true",
        worker: "required_not_available reason=agent_worker_osv_tool_membrane_required_not_available",
        timeout: "--tool-timeout-ms",
        authority_class: "named-network",
        grant_requirements: &["--tool=osv.query", "--osv-fixture=<json>|--named-network-cache-dir=<dir>|same-thread live transport"],
        input_schema: "{package:{ecosystem,name},version?:string}",
        output_schema: "OSV /v1/query response",
        audit_rows: &["tool_call", "tool_result", "tool_denial", "tool_error", "named_network_cache_hit"],
        replay_counters: &["tool_call", "tool_result", "tool_denial", "tool_error", "osv_query"],
        secret_scopes: &[],
        cancellation: "tool-timeout-ms",
        nonclaims: &["no general network", "worker OSV tool membrane routed"],
        residual_owner: Some("agent worker OSV tool membrane"),
        human_summary: "osv.query  available=true worker=required_not_available reason=agent_worker_osv_tool_membrane_required_not_available timeout=--tool-timeout-ms audit=tool_call,tool_result,tool_denial,tool_error,unsupported_control schema=args:{package:{ecosystem,name},version?:string} result:OSV /v1/query response; same-thread live transport is pinned to https://api.osv.dev/v1/query; worker OSV tool membrane routed; no general network",
    },
    AgentToolManifest {
        name: "npm.metadata",
        available: "true",
        worker: "required_not_available reason=agent_worker_npm_metadata_tool_membrane_required_not_available",
        timeout: "--tool-timeout-ms",
        authority_class: "named-network",
        grant_requirements: &["--tool=npm.metadata", "--named-network-cache-dir=<dir>|same-thread live transport"],
        input_schema: "{package:string}",
        output_schema: "abbreviated npm package metadata",
        audit_rows: &["tool_call", "tool_result", "tool_denial", "tool_error", "named_network_cache_hit"],
        replay_counters: &["tool_call", "tool_result", "tool_denial", "tool_error", "npm_metadata"],
        secret_scopes: &[],
        cancellation: "tool-timeout-ms",
        nonclaims: &["no general network", "worker npm.metadata tool membrane routed"],
        residual_owner: Some("agent worker npm.metadata tool membrane"),
        human_summary: "npm.metadata  available=true worker=required_not_available reason=agent_worker_npm_metadata_tool_membrane_required_not_available timeout=--tool-timeout-ms audit=tool_call,tool_result,tool_denial,tool_error,unsupported_control schema=args:{package:string} result:abbreviated npm package metadata; same-thread live transport is pinned to https://registry.npmjs.org/<package>; worker npm.metadata tool membrane routed; no general network",
    },
    AgentToolManifest {
        name: "github.issue.read",
        available: "true",
        worker: "with_--named-network-cache-dir_persistent_cache",
        timeout: "--tool-timeout-ms",
        authority_class: "named-network",
        grant_requirements: &["--tool=github.issue.read", "--named-network-cache-dir=<dir>|same-thread live transport"],
        input_schema: "{owner:string,repo:string,number:number}",
        output_schema: "public GitHub issue JSON",
        audit_rows: &["tool_call", "tool_result", "tool_denial", "tool_error", "named_network_cache_hit"],
        replay_counters: &["tool_call", "tool_result", "tool_denial", "tool_error", "github_issue_read"],
        secret_scopes: &["optional --github-token-env host-only bearer token; token value never audited"],
        cancellation: "tool-timeout-ms",
        nonclaims: &["no general network", "no tenant credential exposure"],
        residual_owner: Some("agent named-network live worker transport"),
        human_summary: "github.issue.read  available=true worker=with_--named-network-cache-dir_persistent_cache timeout=--tool-timeout-ms credential=optional --github-token-env audit=tool_call,tool_result,tool_denial,tool_error schema=args:{owner:string,repo:string,number:number} result:public GitHub issue JSON; same-thread live transport is pinned to https://api.github.com/repos/<owner>/<repo>/issues/<number>; worker live transport and token forwarding routed; no general network",
    },
    AgentToolManifest {
        name: "model.call",
        available: "required_not_available reason=agent_model_tool_membrane_required_not_available",
        worker: "required_not_available reason=agent_model_tool_membrane_required_not_available",
        timeout: "--tool-timeout-ms",
        authority_class: "model",
        grant_requirements: &["--tool=model.call", "--model-fixture=<json>|same-thread --model-provider=openai.responses --model-api-key-env=<ENV>"],
        input_schema: "{id:string,model?:string,input?:json}",
        output_schema: "fixture response object or provider response JSON",
        audit_rows: &["tool_call", "tool_result", "tool_denial", "tool_error"],
        replay_counters: &["tool_call", "tool_result", "tool_denial", "tool_error", "model_call"],
        secret_scopes: &["host-only --model-api-key-env; token value never audited"],
        cancellation: "tool-timeout-ms",
        nonclaims: &["no ambient network", "no fetch", "no tenant credential exposure", "worker model tool membrane routed"],
        residual_owner: Some("agent model tool membrane"),
        human_summary: "model.call  available=required_not_available reason=agent_model_tool_membrane_required_not_available worker=required_not_available reason=agent_model_tool_membrane_required_not_available timeout=--tool-timeout-ms audit=unsupported_control schema=args:{id:string,model?:string,input?:json} result:routed; model tool membrane routed; no ambient network or tenant credential exposure",
    },
    AgentToolManifest {
        name: "process",
        available: "with_--process-command_and_--process-cwd",
        worker: "required_not_available",
        timeout: "--tool-timeout-ms",
        authority_class: "process",
        grant_requirements: &["--tool=process", "--process-command=<name=path>", "--process-cwd=<dir>"],
        input_schema: "{command,args?:string[],cwd?:string,env?:object,output?:full|summary|stream}",
        output_schema: "full|summary|stream process result",
        audit_rows: &["tool_call", "tool_result", "tool_denial", "tool_timeout", "process_output_stream", "unsupported_control"],
        replay_counters: &["tool_call", "tool_result", "tool_denial", "tool_timeout", "process_result", "process_output_stream"],
        secret_scopes: &["explicit --process-env keys only"],
        cancellation: "tool-timeout-ms; external child cancellation remains host supervisor bounded",
        nonclaims: &["no shell", "no exec API", "no spawn API", "no OS sandbox claim", "worker process routed"],
        residual_owner: Some("P-AGENT-WORKER-PROCESS-HOST-CALL-MEMBRANE"),
        human_summary: "process  available=with_--process-command_and_--process-cwd worker=required_not_available reason=agent_worker_process_host_call_required_not_available timeout=--tool-timeout-ms audit=tool_call,tool_result,tool_denial,tool_timeout,process_output_stream,unsupported_control schema=args:{command,args?:string[],cwd?:string,env?:object,output?:full|summary|stream} result:full {exit_code,signal,stdout,stderr,duration_ms} or summary {stdout_summary,stderr_summary,output_mode} or stream {exit_code,signal,stdout,stderr,output_mode:\"stream\"}; same-thread stream chunks bounded by --process-output-stream-chunk-bytes; no shell/OS sandbox claim",
    },
];

const AGENT_GITHUB_EXTRA_TOOL_MANIFESTS: &[(&str, &str, &str, &str)] = &[
    (
        "github.pr.read",
        "{owner:string,repo:string,number:number}",
        "public GitHub pull request JSON",
        "https://api.github.com/repos/<owner>/<repo>/pulls/<number>",
    ),
    (
        "github.pr.files.list",
        "{owner:string,repo:string,number:number}",
        "public GitHub pull request changed files JSON",
        "https://api.github.com/repos/<owner>/<repo>/pulls/<number>/files",
    ),
    (
        "github.release.latest.read",
        "{owner:string,repo:string}",
        "public GitHub latest release JSON",
        "https://api.github.com/repos/<owner>/<repo>/releases/latest",
    ),
    (
        "github.file.read",
        "{owner:string,repo:string,path:string,ref?:string}",
        "public GitHub contents file JSON",
        "https://api.github.com/repos/<owner>/<repo>/contents/<path>?ref=<ref>",
    ),
    (
        "github.compare.read",
        "{owner:string,repo:string,base:string,head:string}",
        "public GitHub compare JSON",
        "https://api.github.com/repos/<owner>/<repo>/compare/<base>...<head>",
    ),
    (
        "github.commit.read",
        "{owner:string,repo:string,ref:string}",
        "public GitHub commit JSON",
        "https://api.github.com/repos/<owner>/<repo>/commits/<ref>",
    ),
    (
        "github.repo.read",
        "{owner:string,repo:string}",
        "public GitHub repo JSON",
        "https://api.github.com/repos/<owner>/<repo>",
    ),
    (
        "github.workflow.run.read",
        "{owner:string,repo:string,run_id:number|string}",
        "public GitHub workflow run JSON",
        "https://api.github.com/repos/<owner>/<repo>/actions/runs/<run_id>",
    ),
    (
        "github.workflow.jobs.list",
        "{owner:string,repo:string,run_id:number|string}",
        "public GitHub workflow jobs JSON",
        "https://api.github.com/repos/<owner>/<repo>/actions/runs/<run_id>/jobs",
    ),
    (
        "github.check.runs.list",
        "{owner:string,repo:string,ref:string}",
        "public GitHub check runs JSON",
        "https://api.github.com/repos/<owner>/<repo>/commits/<ref>/check-runs",
    ),
];

fn agent_tool_manifest(name: &str) -> Option<AgentToolManifest> {
    if let Some(tool) = AGENT_TOOL_MANIFESTS.iter().find(|tool| tool.name == name) {
        return Some(AgentToolManifest { ..*tool });
    }
    AGENT_GITHUB_EXTRA_TOOL_MANIFESTS
        .iter()
        .find(|(tool_name, _, _, _)| *tool_name == name)
        .map(|(tool_name, input_schema, output_schema, endpoint)| {
            let worker = if matches!(
                *tool_name,
                "github.pr.files.list"
                    | "github.commit.read"
                    | "github.workflow.jobs.list"
                    | "github.check.runs.list"
            ) {
                "required_not_available reason=agent_worker_github_complex_payload_tool_membrane_required_not_available"
            } else {
                "with_--named-network-cache-dir_persistent_cache"
            };
            let residual_owner = if worker.starts_with("required_not_available") {
                Some("agent worker GitHub complex-payload tool membrane")
            } else {
                Some("agent named-network live worker transport")
            };
            let worker_summary = if worker.starts_with("required_not_available") {
                "worker complex-payload membrane routed"
            } else {
                "worker live transport and token forwarding routed"
            };
            AgentToolManifest {
                name: tool_name,
                available: "true",
                worker,
                timeout: "--tool-timeout-ms",
                authority_class: "named-network",
                grant_requirements: &["--tool=<github.*>", "--named-network-cache-dir=<dir>|same-thread live transport"],
                input_schema,
                output_schema,
                audit_rows: &["tool_call", "tool_result", "tool_denial", "tool_error", "named_network_cache_hit"],
                replay_counters: &["tool_call", "tool_result", "tool_denial", "tool_error"],
                secret_scopes: &["optional --github-token-env host-only bearer token; token value never audited"],
                cancellation: "tool-timeout-ms",
                nonclaims: &["no general network", "no tenant credential exposure", "no repo checkout unless explicitly stated by tool"],
                residual_owner,
                human_summary: Box::leak(format!("{tool_name}  available=true worker={worker} timeout=--tool-timeout-ms credential=optional --github-token-env audit=tool_call,tool_result,tool_denial,tool_error schema=args:{input_schema} result:{output_schema}; same-thread live transport is pinned to {endpoint}; {worker_summary}; no general network").into_boxed_str()),
            }
        })
}

fn agent_tool_manifest_json(tool: &AgentToolManifest) -> String {
    format!(
        "{{\"schema_version\":{},\"name\":{},\"available\":{},\"worker\":{},\"timeout\":{},\"authority_class\":{},\"grant_requirements\":{},\"input_schema\":{},\"output_schema\":{},\"audit_rows\":{},\"replay_counters\":{},\"secret_scopes\":{},\"cancellation\":{},\"nonclaims\":{},\"residual_owner\":{}}}",
        json_string_literal(AGENT_TOOL_MANIFEST_SCHEMA_VERSION),
        json_string_literal(tool.name),
        json_string_literal(tool.available),
        json_string_literal(tool.worker),
        json_string_literal(tool.timeout),
        json_string_literal(tool.authority_class),
        json_str_array(tool.grant_requirements),
        json_string_literal(tool.input_schema),
        json_string_literal(tool.output_schema),
        json_str_array(tool.audit_rows),
        json_str_array(tool.replay_counters),
        json_str_array(tool.secret_scopes),
        json_string_literal(tool.cancellation),
        json_str_array(tool.nonclaims),
        tool.residual_owner
            .map(json_string_literal)
            .unwrap_or_else(|| "null".to_string())
    )
}

fn json_str_array(values: &[&str]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| json_string_literal(value))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn all_agent_tool_manifests() -> Vec<AgentToolManifest> {
    let mut tools: Vec<AgentToolManifest> = AGENT_TOOL_MANIFESTS
        .iter()
        .map(|tool| AgentToolManifest { ..*tool })
        .collect();
    for (name, _, _, _) in AGENT_GITHUB_EXTRA_TOOL_MANIFESTS {
        if let Some(tool) = agent_tool_manifest(name) {
            tools.push(tool);
        }
    }
    tools
}

pub(crate) fn run_agent_tool_subcommand(args: &[String]) -> ExitCode {
    let Some(action) = args.first().map(|s| s.as_str()) else {
        eprintln!(
            "cruft agent tool: usage: cruft agent tool list | cruft agent tool describe <name>"
        );
        return ExitCode::from(64);
    };
    match action {
        "list" => {
            let json = args.get(1).map(|s| s.as_str()) == Some("--json");
            if args.len() > if json { 2 } else { 1 } {
                eprintln!("cruft agent tool list: unexpected argument");
                return ExitCode::from(64);
            }
            if json {
                let tools = all_agent_tool_manifests();
                println!(
                    "{{\"schema_version\":{},\"tools\":[{}],\"unavailable_process_like_tools\":[{{\"name\":\"shell\",\"reason\":\"process_tool_supervisor_required_not_available\"}},{{\"name\":\"exec\",\"reason\":\"process_tool_supervisor_required_not_available\"}},{{\"name\":\"spawn\",\"reason\":\"process_tool_supervisor_required_not_available\"}}]}}",
                    json_string_literal(AGENT_TOOL_MANIFEST_SCHEMA_VERSION),
                    tools
                        .iter()
                        .map(agent_tool_manifest_json)
                        .collect::<Vec<_>>()
                        .join(",")
                );
                return ExitCode::SUCCESS;
            }
            println!("agent tools:");
            for tool in all_agent_tool_manifests() {
                println!("  {}", tool.human_summary);
            }
            println!("unavailable process-like tools:");
            println!("  shell exec spawn  reason=process_tool_supervisor_required_not_available");
            ExitCode::SUCCESS
        }
        "describe" => {
            let Some(name) = args.get(1).map(|s| s.as_str()) else {
                eprintln!("cruft agent tool describe: missing <name>");
                return ExitCode::from(64);
            };
            let json = args.get(2).map(|s| s.as_str()) == Some("--json");
            if args.len() > if json { 3 } else { 2 } {
                eprintln!("cruft agent tool describe {name}: unexpected argument");
                return ExitCode::from(64);
            }
            if json {
                if let Some(tool) = agent_tool_manifest(name) {
                    println!("{}", agent_tool_manifest_json(&tool));
                    return ExitCode::SUCCESS;
                }
            }
            match name {
                "echo" => {
                    println!("tool: echo");
                    println!("status: available");
                    println!("worker: supported");
                    println!("arguments: JSON object");
                    println!(
                        "result: JSON object containing ok=true, tool=\"echo\", echo=<arguments>"
                    );
                    println!("audit: tool_call then tool_result; tool_denial when not granted");
                    println!("timeout: synchronous turn timeout; no process authority");
                    ExitCode::SUCCESS
                }
                "fail" => {
                    println!("tool: fail");
                    println!("status: available");
                    println!("worker: supported");
                    println!("arguments: JSON object");
                    println!("result: throws agent tool host failure");
                    println!("audit: tool_call then tool_error; tool_denial when not granted");
                    println!("timeout: synchronous turn timeout; no process authority");
                    ExitCode::SUCCESS
                }
                "slow" => {
                    println!("tool: slow");
                    println!("status: available");
                    println!("worker: supported");
                    println!("arguments: JSON object; delay_ms must be a non-negative number when present");
                    println!(
                        "result: Promise resolving to {{ok:true, tool:\"slow\", delay_ms, value}}"
                    );
                    println!("audit: tool_call then tool_result or tool_timeout; tool_denial when not granted");
                    println!("timeout: bounded by --tool-timeout-ms");
                    ExitCode::SUCCESS
                }
                "readFile" => {
                    println!("tool: readFile");
                    println!("status: available with --fs-read <path>");
                    println!("worker: supported");
                    println!("arguments: JSON object with path string");
                    println!("result: JSON object containing path, content, and bytes");
                    println!("audit: tool_call then tool_result; tool_denial for ungranted paths");
                    println!("authority: read-only precollected path caps; no ambient fs");
                    ExitCode::SUCCESS
                }
                "listFiles" => {
                    println!("tool: listFiles");
                    println!("status: available with --fs-read <path>");
                    println!("worker: supported");
                    println!("arguments: JSON object with optional path string");
                    println!("result: JSON object containing files");
                    println!("audit: tool_call then tool_result; tool_denial for ungranted roots");
                    println!("authority: read-only precollected path caps; no ambient fs");
                    ExitCode::SUCCESS
                }
                "writeArtifact" => {
                    println!("tool: writeArtifact");
                    println!("status: available with --fs-write <dir>");
                    println!("worker: supported");
                    println!("arguments: JSON object with path and content strings");
                    println!("result: JSON object containing path, bytes, and fnv1a64 hash");
                    println!(
                        "audit: tool_call then tool_result; tool_denial for escapes/overwrite"
                    );
                    println!(
                        "authority: durable writes only under explicit output root; no ambient fs"
                    );
                    ExitCode::SUCCESS
                }
                "osv.query" => {
                    println!("tool: osv.query");
                    println!("status: available; --osv-fixture <json> selects deterministic fixture mode");
                    println!("worker: required_not_available reason=agent_worker_osv_tool_membrane_required_not_available");
                    println!("arguments: JSON object with package.ecosystem, package.name, optional version");
                    println!("result: OSV /v1/query-shaped JSON object with vulns array");
                    println!("audit: tool_call then tool_result; tool_denial when not granted; tool_error on live transport failure");
                    println!("authority: named OSV lookup only; live same-thread transport is pinned to https://api.osv.dev/v1/query; no general network");
                    ExitCode::SUCCESS
                }
                "npm.metadata" => {
                    println!("tool: npm.metadata");
                    println!("status: available");
                    println!("worker: required_not_available reason=agent_worker_npm_metadata_tool_membrane_required_not_available");
                    println!(
                        "arguments: JSON object with package string; scoped names use @scope/name"
                    );
                    println!("result: abbreviated npm registry package metadata JSON");
                    println!("audit: tool_call then tool_result; tool_denial when not granted; tool_error on live transport failure");
                    println!("authority: named npm metadata lookup only; live same-thread transport is pinned to https://registry.npmjs.org/<encoded-package>; no general network");
                    ExitCode::SUCCESS
                }
                "github.issue.read" => {
                    println!("tool: github.issue.read");
                    println!("status: available");
                    println!("worker: supported with --named-network-cache-dir persistent cache; worker live transport and token forwarding routed");
                    println!("arguments: JSON object with owner string, repo string, number positive integer");
                    println!("result: public GitHub issue JSON");
                    println!("audit: tool_call then tool_result; tool_denial when not granted; tool_error on live transport failure");
                    println!("credential: optional host-only bearer token from --github-token-env <ENV>; audit records env name and credential mode, never token value");
                    println!("authority: named GitHub issue lookup only; live same-thread transport is pinned to https://api.github.com/repos/<owner>/<repo>/issues/<number>; no general network or tenant credential exposure");
                    ExitCode::SUCCESS
                }
                "github.pr.read" => {
                    println!("tool: github.pr.read");
                    println!("status: available");
                    println!("worker: supported with --named-network-cache-dir persistent cache; worker live transport and token forwarding routed");
                    println!("arguments: JSON object with owner string, repo string, number positive integer");
                    println!("result: public GitHub pull request JSON");
                    println!("audit: tool_call then tool_result; tool_denial when not granted; tool_error on live transport failure");
                    println!("credential: optional host-only bearer token from --github-token-env <ENV>; audit records env name and credential mode, never token value");
                    println!("authority: named GitHub pull request lookup only; live same-thread transport is pinned to https://api.github.com/repos/<owner>/<repo>/pulls/<number>; worker cache mode serves only matching tokenless persistent-cache envelopes; no general network or tenant credential exposure");
                    ExitCode::SUCCESS
                }
                "github.pr.files.list" => {
                    println!("tool: github.pr.files.list");
                    println!("status: available");
                    println!("worker: supported with --named-network-cache-dir persistent cache; worker live transport and token forwarding routed");
                    println!("arguments: JSON object with owner string, repo string, number positive integer");
                    println!("result: public GitHub pull request changed files JSON");
                    println!("audit: tool_call then tool_result; tool_denial when not granted; tool_error on live transport failure");
                    println!("credential: optional host-only bearer token from --github-token-env <ENV>; audit records env name and credential mode, never token value");
                    println!("authority: named GitHub PR changed-file metadata lookup only; live same-thread transport is pinned to https://api.github.com/repos/<owner>/<repo>/pulls/<number>/files; worker cache mode serves only matching tokenless persistent-cache envelopes; no repo checkout, git, GitHub search, Actions logs/artifacts, general network, or tenant credential exposure");
                    ExitCode::SUCCESS
                }
                "github.release.latest.read" => {
                    println!("tool: github.release.latest.read");
                    println!("status: available");
                    println!("worker: supported with --named-network-cache-dir persistent cache; worker live transport and token forwarding routed");
                    println!("arguments: JSON object with owner string, repo string");
                    println!("result: public GitHub latest release JSON");
                    println!("audit: tool_call then tool_result; tool_denial when not granted; tool_error on live transport failure");
                    println!("credential: optional host-only bearer token from --github-token-env <ENV>; audit records env name and credential mode, never token value");
                    println!("authority: named GitHub latest release lookup only; live same-thread transport is pinned to https://api.github.com/repos/<owner>/<repo>/releases/latest; worker cache mode serves only matching tokenless persistent-cache envelopes; no general network or tenant credential exposure");
                    ExitCode::SUCCESS
                }
                "github.file.read" => {
                    println!("tool: github.file.read");
                    println!("status: available");
                    println!("worker: supported with --named-network-cache-dir persistent cache; worker live transport and token forwarding routed");
                    println!("arguments: JSON object with owner string, repo string, path string, optional ref string");
                    println!("result: public GitHub contents file JSON");
                    println!("audit: tool_call then tool_result; tool_denial when not granted; tool_error on live transport failure");
                    println!("credential: optional host-only bearer token from --github-token-env <ENV>; audit records env name and credential mode, never token value");
                    println!("authority: named GitHub contents file lookup only; live same-thread transport is pinned to https://api.github.com/repos/<owner>/<repo>/contents/<path>?ref=<ref>; worker cache mode serves only matching tokenless persistent-cache envelopes; no general network, repo checkout, ambient fs, or tenant credential exposure");
                    ExitCode::SUCCESS
                }
                "github.compare.read" => {
                    println!("tool: github.compare.read");
                    println!("status: available");
                    println!("worker: supported with --named-network-cache-dir persistent cache; worker live transport and token forwarding routed");
                    println!("arguments: JSON object with owner string, repo string, base string, head string");
                    println!("result: public GitHub compare JSON");
                    println!("audit: tool_call then tool_result; tool_denial when not granted; tool_error on live transport failure");
                    println!("credential: optional host-only bearer token from --github-token-env <ENV>; audit records env name and credential mode, never token value");
                    println!("authority: named GitHub compare lookup only; live same-thread transport is pinned to https://api.github.com/repos/<owner>/<repo>/compare/<base>...<head>; worker cache mode serves only matching tokenless persistent-cache envelopes; no general network, repo checkout, ambient git, or tenant credential exposure");
                    ExitCode::SUCCESS
                }
                "github.commit.read" => {
                    println!("tool: github.commit.read");
                    println!("status: available");
                    println!("worker: supported with --named-network-cache-dir persistent cache; worker live transport and token forwarding routed");
                    println!("arguments: JSON object with owner string, repo string, ref string");
                    println!("result: public GitHub commit JSON");
                    println!("audit: tool_call then tool_result; tool_denial when not granted; tool_error on live transport failure");
                    println!("credential: optional host-only bearer token from --github-token-env <ENV>; audit records env name and credential mode, never token value");
                    println!("authority: named GitHub commit lookup only; live same-thread transport is pinned to https://api.github.com/repos/<owner>/<repo>/commits/<ref>; worker cache mode serves only matching tokenless persistent-cache envelopes; no general network, repo checkout, ambient git, or tenant credential exposure");
                    ExitCode::SUCCESS
                }
                "github.repo.read" => {
                    println!("tool: github.repo.read");
                    println!("status: available");
                    println!("worker: supported with --named-network-cache-dir persistent cache; worker live transport and token forwarding routed");
                    println!("arguments: JSON object with owner string, repo string");
                    println!("result: public GitHub repository JSON");
                    println!("audit: tool_call then tool_result; tool_denial when not granted; tool_error on live transport failure");
                    println!("credential: optional host-only bearer token from --github-token-env <ENV>; audit records env name and credential mode, never token value");
                    println!("authority: named GitHub repo lookup only; live same-thread transport is pinned to https://api.github.com/repos/<owner>/<repo>; no general network or tenant credential exposure");
                    ExitCode::SUCCESS
                }
                "github.workflow.run.read" => {
                    println!("tool: github.workflow.run.read");
                    println!("status: available");
                    println!("worker: supported with --named-network-cache-dir persistent cache; worker live transport and token forwarding routed");
                    println!("arguments: JSON object with owner string, repo string, run_id positive integer or decimal string");
                    println!("result: public GitHub workflow run JSON");
                    println!("audit: tool_call then tool_result; tool_denial when not granted; tool_error on live transport failure");
                    println!("credential: optional host-only bearer token from --github-token-env <ENV>; audit records env name and credential mode, never token value");
                    println!("authority: named GitHub workflow-run lookup only; live same-thread transport is pinned to https://api.github.com/repos/<owner>/<repo>/actions/runs/<run_id>; worker cache mode serves only matching tokenless persistent-cache envelopes; no general network, repo checkout, ambient git, or tenant credential exposure");
                    ExitCode::SUCCESS
                }
                "github.workflow.jobs.list" => {
                    println!("tool: github.workflow.jobs.list");
                    println!("status: available");
                    println!("worker: supported with --named-network-cache-dir persistent cache; worker live transport and token forwarding routed");
                    println!("arguments: JSON object with owner string, repo string, run_id positive integer or decimal string");
                    println!("result: public GitHub workflow jobs JSON");
                    println!("audit: tool_call then tool_result; tool_denial when not granted; tool_error on live transport failure");
                    println!("credential: optional host-only bearer token from --github-token-env <ENV>; audit records env name and credential mode, never token value");
                    println!("authority: named GitHub workflow jobs lookup only; live same-thread transport is pinned to https://api.github.com/repos/<owner>/<repo>/actions/runs/<run_id>/jobs; worker cache mode serves only matching tokenless persistent-cache envelopes; no general network, repo checkout, ambient git, Actions log download, or tenant credential exposure");
                    ExitCode::SUCCESS
                }
                "github.check.runs.list" => {
                    println!("tool: github.check.runs.list");
                    println!("status: available");
                    println!("worker: supported with --named-network-cache-dir persistent cache; worker live transport and token forwarding routed");
                    println!("arguments: JSON object with owner string, repo string, ref string");
                    println!("result: public GitHub check runs JSON");
                    println!("audit: tool_call then tool_result; tool_denial when not granted; tool_error on live transport failure");
                    println!("credential: optional host-only bearer token from --github-token-env <ENV>; audit records env name and credential mode, never token value");
                    println!("authority: named GitHub check-runs lookup only; live same-thread transport is pinned to https://api.github.com/repos/<owner>/<repo>/commits/<ref>/check-runs; worker cache mode serves only matching tokenless persistent-cache envelopes; no general network, repo checkout, ambient git, Actions log download, artifact download, GitHub search, or tenant credential exposure");
                    ExitCode::SUCCESS
                }
                "model.call" => {
                    println!("tool: model.call");
                    println!("status: required_not_available reason=agent_model_tool_membrane_required_not_available");
                    println!("worker: required_not_available reason=agent_model_tool_membrane_required_not_available");
                    println!("arguments: JSON object with id string, optional model string, optional input JSON");
                    println!("result: routed");
                    println!("audit: unsupported_control before tenant execution");
                    println!("credential: no model credential is consumed while routed");
                    println!("authority: no model authority is granted while routed");
                    ExitCode::SUCCESS
                }
                "process" => {
                    println!("tool: process");
                    println!("status: available with --process-command <name=path> and --process-cwd <dir>");
                    println!("worker: unavailable until P-AGENT-WORKER-PROCESS-HOST-CALL-MEMBRANE closes; reason agent_worker_process_host_call_required_not_available");
                    println!(
                        "arguments: JSON object with command, optional args:string[], cwd, env, output:full|summary|stream"
                    );
                    println!("result: full {{exit_code, signal, stdout, stderr, duration_ms}} or summary {{stdout_summary, stderr_summary, output_mode}}; stream is same-thread only");
                    println!("audit: tool_call then tool_result; tool_denial for unadmitted command/cwd/env; tool_timeout on timeout; unsupported_control for worker process");
                    println!("authority: argv-only supervised child process; no shell, exec, spawn API, or OS sandbox claim");
                    ExitCode::SUCCESS
                }
                name if is_agent_process_tool_specifier(name) => {
                    println!("tool: {name}");
                    println!("status: unavailable");
                    println!("worker: unavailable");
                    println!("reason: process_tool_supervisor_required_not_available");
                    println!("audit: unsupported_control when requested through run");
                    println!("timeout: requires process supervisor/cancellation substrate");
                    ExitCode::SUCCESS
                }
                other => {
                    eprintln!(
                        "cruft agent tool describe: unknown tool {other:?}; run `cruft agent tool list`"
                    );
                    ExitCode::from(65)
                }
            }
        }
        other => {
            eprintln!("cruft agent tool: unsupported action {other:?}; available: list, describe");
            ExitCode::from(64)
        }
    }
}
