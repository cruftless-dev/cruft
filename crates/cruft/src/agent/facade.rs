use crate::json_string_literal;
use std::process::ExitCode;

pub(crate) fn agent_harness_contract_json() -> String {
    let ops = [
        ("create_run", "cruft agent run --project <dir>"),
        ("stream_audit", "tail/read configured audit_log JSONL"),
        ("fetch_replay", "cruft agent replay --json <audit.jsonl>"),
        (
            "append_approval",
            "cruft agent approval allow|deny <approval.jsonl> <approval-id>",
        ),
        (
            "approval_inbox",
            "cruft agent approval inbox <approval.jsonl> --json",
        ),
        (
            "cancel_job",
            "cruft agent schedule cancel <job-id> --store <dir>",
        ),
        (
            "resume_job",
            "cruft agent schedule tick|input <job-id> --store <dir>",
        ),
        (
            "fetch_scheduler_replay",
            "cruft agent schedule replay <job-id> --store <dir>",
        ),
        (
            "inspect_policy_risk",
            "cruft agent policy risk --json <project|policy>",
        ),
        (
            "fetch_tool_manifest",
            "cruft agent tool list|describe <name> --json",
        ),
        ("fetch_agent_abi", "cruft agent abi --json"),
        (
            "export_evidence_bundle",
            "cruft agent bundle <project|policy> --out <dir>",
        ),
        (
            "verify_evidence_bundle",
            "cruft agent bundle-verify <bundle-dir>",
        ),
    ];
    let operations = ops
        .iter()
        .map(|(name, cli)| {
            format!(
                "{{\"name\":{},\"local_cli\":{},\"remote_status\":\"contract_only\"}}",
                json_string_literal(name),
                json_string_literal(cli)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"type\":\"agent_harness_api_facade\",\"schema_version\":\"cruft-agent-harness-api.v1\",\"transport\":\"local_contract\",\"operations\":[{}],\"nonclaims\":[\"no hosted service or multi-tenant control plane is implemented by this contract\",\"remote API authentication, tenancy, billing, and network transport are future hosting-owned surfaces\",\"each operation maps to an existing local CLI/product gate and inherits its policy membrane\"]}}",
        operations
    )
}

pub(crate) fn run_agent_harness_subcommand(args: &[String]) -> ExitCode {
    if matches!(
        args.first().map(|s| s.as_str()),
        None | Some("--help") | Some("-h") | Some("help")
    ) {
        println!("Usage: cruft agent harness contract [--json|--human]");
        println!();
        println!("Print the local Agent Harness API contract.");
        println!("This is a contract-only facade over existing local CLI gates.");
        return ExitCode::SUCCESS;
    }
    if args.first().map(|s| s.as_str()) != Some("contract") {
        eprintln!("cruft agent harness: usage: cruft agent harness contract [--json|--human]");
        return ExitCode::from(64);
    }
    let mut json = true;
    for arg in &args[1..] {
        if arg == "--json" {
            json = true;
        } else if arg == "--human" {
            json = false;
        } else {
            eprintln!("cruft agent harness contract: unexpected argument {arg}");
            return ExitCode::from(64);
        }
    }
    if json {
        println!("{}", agent_harness_contract_json());
    } else {
        println!("Cruft Agent Harness API Facade v1");
        println!("transport: local contract");
        println!("remote service: not implemented");
        for (name, cli) in [
            ("create_run", "cruft agent run --project <dir>"),
            (
                "approval_inbox",
                "cruft agent approval inbox <approval.jsonl> --json",
            ),
            (
                "fetch_scheduler_replay",
                "cruft agent schedule replay <job-id> --store <dir>",
            ),
            (
                "verify_evidence_bundle",
                "cruft agent bundle-verify <bundle-dir>",
            ),
        ] {
            println!("- {name}: {cli}");
        }
    }
    ExitCode::SUCCESS
}
