use std::process::ExitCode;

fn json_escape(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn json_string_literal(s: &str) -> String {
    format!("\"{}\"", json_escape(s))
}

fn agent_approval_id_is_valid(id: &str) -> bool {
    id.strip_prefix("fnv1a64:")
        .map(|hex| hex.len() == 16 && hex.chars().all(|c| c.is_ascii_hexdigit()))
        .unwrap_or(false)
}

pub(crate) fn append_agent_approval_decision(
    approval_log: &str,
    id: &str,
    status: &str,
    reason: Option<&str>,
) -> Result<(), String> {
    if !agent_approval_id_is_valid(id) {
        return Err("approval id must be fnv1a64:<16 hex chars>".to_string());
    }
    let parent = std::path::Path::new(approval_log)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    std::fs::create_dir_all(parent).map_err(|e| {
        format!(
            "cannot create approval log parent {}: {e}",
            parent.display()
        )
    })?;
    let reason_json = reason
        .map(|r| format!(",\"reason\":{}", json_string_literal(r)))
        .unwrap_or_default();
    let created_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let line = format!(
        "{{\"type\":\"agent_approval_decision\",\"id\":{},\"status\":{},\"decision\":{},\"created_at_ms\":{}{}}}\n",
        json_string_literal(id),
        json_string_literal(status),
        json_string_literal(status),
        created_at_ms,
        reason_json
    );
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(approval_log)
        .map_err(|e| format!("cannot open approval log {approval_log}: {e}"))?;
    file.write_all(line.as_bytes())
        .map_err(|e| format!("cannot write approval log {approval_log}: {e}"))
}

pub(crate) fn run_agent_approval_subcommand(args: &[String]) -> ExitCode {
    let Some(command) = args.first().map(|s| s.as_str()) else {
        eprintln!(
            "cruft agent approval: usage: cruft agent approval inbox <approval.jsonl> [--json|--human] | cruft agent approval allow|deny <approval.jsonl> <approval-id> [--reason=<text>]"
        );
        return ExitCode::from(64);
    };
    if command == "inbox" {
        return run_agent_approval_inbox_subcommand(&args[1..]);
    }
    if command != "allow" && command != "deny" {
        eprintln!("cruft agent approval: command must be inbox, allow, or deny");
        return ExitCode::from(64);
    }
    let Some(approval_log) = args.get(1) else {
        eprintln!("cruft agent approval: missing <approval.jsonl>");
        return ExitCode::from(64);
    };
    let Some(id) = args.get(2) else {
        eprintln!("cruft agent approval: missing <approval-id>");
        return ExitCode::from(64);
    };
    let mut reason: Option<String> = None;
    for arg in &args[3..] {
        if let Some(value) = arg.strip_prefix("--reason=") {
            reason = Some(value.to_string());
        } else {
            eprintln!("cruft agent approval: unexpected argument {arg}");
            return ExitCode::from(64);
        }
    }
    let status = if command == "allow" {
        "allowed"
    } else {
        "denied"
    };
    match append_agent_approval_decision(approval_log, id, status, reason.as_deref()) {
        Ok(()) => {
            println!(
                "{{\"type\":\"agent_approval_decision\",\"approval_log\":{},\"id\":{},\"status\":{}}}",
                json_string_literal(approval_log),
                json_string_literal(id),
                json_string_literal(status)
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("cruft agent approval: {e}");
            ExitCode::from(65)
        }
    }
}

fn run_agent_approval_inbox_subcommand(args: &[String]) -> ExitCode {
    let mut path: Option<&String> = None;
    let mut json = true;
    for arg in args {
        if arg == "--json" {
            json = true;
        } else if arg == "--human" {
            json = false;
        } else if path.is_none() {
            path = Some(arg);
        } else {
            eprintln!("cruft agent approval inbox: unexpected argument {arg}");
            return ExitCode::from(64);
        }
    }
    let Some(path) = path else {
        eprintln!("cruft agent approval inbox: usage: cruft agent approval inbox <approval.jsonl> [--json|--human]");
        return ExitCode::from(64);
    };
    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(e) => {
            eprintln!("cruft agent approval inbox: cannot read {path}: {e}");
            return ExitCode::from(66);
        }
    };
    let mut rows: Vec<serde_json::Value> = Vec::new();
    let mut decisions: std::collections::HashMap<String, serde_json::Value> =
        std::collections::HashMap::new();
    for line in contents.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let ty = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if ty == "agent_approval_decision" {
            if let Some(id) = value.get("id").and_then(|v| v.as_str()) {
                decisions.insert(id.to_string(), value);
            }
        } else if ty == "agent_approval_pending" {
            rows.push(value);
        }
    }
    let mut indexed = Vec::new();
    let mut pending = 0usize;
    let mut allowed = 0usize;
    let mut denied = 0usize;
    for row in rows {
        let id = row.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let tool = row.get("tool").and_then(|v| v.as_str()).unwrap_or("");
        let args = row.get("args").cloned().unwrap_or(serde_json::Value::Null);
        let arg_bytes = row.get("arg_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
        let decision = decisions.get(id);
        let status = decision
            .and_then(|v| v.get("status"))
            .and_then(|v| v.as_str())
            .unwrap_or("pending");
        match status {
            "allowed" => allowed += 1,
            "denied" => denied += 1,
            _ => pending += 1,
        }
        let decision_json = decision
            .map(|v| v.to_compact_string())
            .unwrap_or_else(|| "null".to_string());
        indexed.push(format!(
            "{{\"id\":{},\"tool\":{},\"status\":{},\"arg_bytes\":{},\"args\":{},\"decision\":{}}}",
            json_string_literal(id),
            json_string_literal(tool),
            json_string_literal(status),
            arg_bytes,
            args.to_compact_string(),
            decision_json
        ));
    }
    if json {
        println!(
            "{{\"type\":\"agent_approval_inbox\",\"schema_version\":1,\"approval_log\":{},\"counts\":{{\"pending\":{},\"allowed\":{},\"denied\":{},\"total\":{}}},\"items\":[{}],\"nonclaims\":[\"inbox is a read model over the approval log and grants no tool authority\"]}}",
            json_string_literal(path),
            pending,
            allowed,
            denied,
            pending + allowed + denied,
            indexed.join(",")
        );
    } else {
        println!("Agent approval inbox: {path}");
        println!("pending={pending} allowed={allowed} denied={denied}");
        for item in indexed {
            let value = serde_json::from_str::<serde_json::Value>(&item).unwrap();
            println!(
                "- {} {} {}",
                value.get("status").and_then(|v| v.as_str()).unwrap_or(""),
                value.get("tool").and_then(|v| v.as_str()).unwrap_or(""),
                value.get("id").and_then(|v| v.as_str()).unwrap_or("")
            );
        }
    }
    ExitCode::SUCCESS
}
