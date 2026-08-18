use std::process::ExitCode;

use crate::json_string_literal;

#[derive(Clone, Copy)]
struct AgentAbiEntry {
    name: &'static str,
    kind: &'static str,
    endowment: &'static str,
    availability: &'static str,
    clone_semantics: &'static str,
    budget_semantics: &'static str,
    error_semantics: &'static str,
    worker_semantics: &'static str,
    revocation_semantics: &'static str,
    nonclaims: &'static [&'static str],
}

const AGENT_ABI_SCHEMA_VERSION: &str = "cruft-agent-abi.v1";

const AGENT_ABI_ENTRIES: &[AgentAbiEntry] = &[
    AgentAbiEntry {
        name: "emit",
        kind: "event",
        endowment: "function",
        availability: "agent_run_and_worker",
        clone_semantics: "JSON-object event cloned into audit/outbox",
        budget_semantics: "--max-events and --max-event-bytes",
        error_semantics: "invalid event or budget overflow fails the run deterministically",
        worker_semantics: "worker-hosted emit is forwarded through the host-call membrane",
        revocation_semantics: "closed compartment rejects emit and records revocation denial",
        nonclaims: &["no stdout streaming ABI", "no arbitrary object graph clone"],
    },
    AgentAbiEntry {
        name: "callTool",
        kind: "tool-call",
        endowment: "function",
        availability: "agent_run_and_worker",
        clone_semantics: "JSON-ish args/results cloned through the host tool membrane",
        budget_semantics: "--max-tool-arg-bytes, --max-tool-result-bytes, --tool-timeout-ms",
        error_semantics: "denial, invalid args, host error, and timeout are structured and audited",
        worker_semantics:
            "worker-hosted sync/promise tools use P-RUNTIME-WORKER-HOST-CALL-RPC where proven",
        revocation_semantics: "closed compartment rejects callTool and records revocation denial",
        nonclaims: &[
            "no ambient tools",
            "no shell/exec/spawn ABI",
            "no general network ABI",
        ],
    },
    AgentAbiEntry {
        name: "state",
        kind: "state",
        endowment: "object",
        availability: "agent_run_and_worker",
        clone_semantics: "JSON-serializable values snapshotted under the session state budget",
        budget_semantics: "--max-state-bytes",
        error_semantics: "non-serializable or oversized state fails deterministically",
        worker_semantics: "worker state is host-mediated and session-file backed where configured",
        revocation_semantics: "closed compartment rejects state mutation surfaces",
        nonclaims: &[
            "no arbitrary host object persistence",
            "no database-backed state ABI",
        ],
    },
    AgentAbiEntry {
        name: "importValue",
        kind: "module-import",
        endowment: "function",
        availability: "agent_run_and_worker",
        clone_semantics: "imports only explicitly admitted module/package/import-hook values",
        budget_semantics: "source/integrity caps and module policy admission",
        error_semantics: "unadmitted specifier rejects through module_denial/audit path",
        worker_semantics:
            "worker source-module/package/import-hook forwarding is only the proven policy surface",
        revocation_semantics: "closed compartment rejects importValue",
        nonclaims: &[
            "no ambient require",
            "no arbitrary npm graph execution",
            "no network import ABI",
        ],
    },
    AgentAbiEntry {
        name: "close",
        kind: "lifecycle",
        endowment: "function",
        availability: "agent_run_and_worker",
        clone_semantics: "no payload",
        budget_semantics: "turn-local lifecycle transition",
        error_semantics: "post-close ABI calls fail closed",
        worker_semantics: "worker close revokes forwarded host-call surfaces",
        revocation_semantics: "sets the compartment closed flag and audits later denial surfaces",
        nonclaims: &[
            "no external process cancellation",
            "no durable scheduler cancellation",
        ],
    },
    AgentAbiEntry {
        name: "auditNote",
        kind: "audit",
        endowment: "function",
        availability: "agent_run_and_worker",
        clone_semantics: "JSON-object note cloned and redacted into audit",
        budget_semantics: "--max-event-bytes payload budget",
        error_semantics: "invalid or over-budget note fails deterministically",
        worker_semantics: "worker auditNote is host-mediated",
        revocation_semantics: "closed compartment rejects auditNote and records revocation denial",
        nonclaims: &[
            "no secret-preserving private channel",
            "no unaudited tenant note",
        ],
    },
    AgentAbiEntry {
        name: "auditControls",
        kind: "audit",
        endowment: "function",
        availability: "agent_run_and_worker",
        clone_semantics: "returns a JSON control snapshot",
        budget_semantics: "read-only control observation",
        error_semantics: "closed compartment rejects auditControls",
        worker_semantics: "worker auditControls is host-mediated",
        revocation_semantics:
            "closed compartment records revocation denial without emitting controls",
        nonclaims: &["no authority grant", "no policy mutation ABI"],
    },
    AgentAbiEntry {
        name: "scheduler.sleep",
        kind: "scheduler",
        endowment: "scheduler object method",
        availability: "scheduler_only",
        clone_semantics: "sleep request is serialized into scheduler await state",
        budget_semantics: "scheduler turn and wake predicates",
        error_semantics: "ordinary agent run does not endow scheduler",
        worker_semantics: "resident worker continuation persistence is routed",
        revocation_semantics: "cancelled/closed job refuses further scheduler effects",
        nonclaims: &[
            "not endowed by ordinary cruft agent run",
            "no arbitrary Promise continuation persistence",
        ],
    },
    AgentAbiEntry {
        name: "scheduler.callTool",
        kind: "scheduler",
        endowment: "scheduler object method",
        availability: "scheduler_only",
        clone_semantics: "tool request/result is serialized through scheduler await state",
        budget_semantics: "scheduler turn, tool payload, and timeout budgets",
        error_semantics: "ordinary agent run does not endow scheduler",
        worker_semantics: "effectful scheduler tool adapters remain routed unless proven",
        revocation_semantics: "cancelled/closed job refuses further scheduler effects",
        nonclaims: &[
            "not endowed by ordinary cruft agent run",
            "no external effect cancellation guarantee",
        ],
    },
    AgentAbiEntry {
        name: "scheduler.waitForInput",
        kind: "scheduler",
        endowment: "scheduler object method",
        availability: "scheduler_only",
        clone_semantics: "input wait token and resumed JSON payload are serialized by scheduler",
        budget_semantics: "scheduler input token and turn predicates",
        error_semantics: "ordinary agent run does not endow scheduler",
        worker_semantics: "resident worker continuation persistence is routed",
        revocation_semantics: "cancelled/closed job refuses further scheduler effects",
        nonclaims: &[
            "not endowed by ordinary cruft agent run",
            "no unauthenticated input resume",
        ],
    },
];

pub(crate) fn run_agent_abi_subcommand(args: &[String]) -> ExitCode {
    let json = args.first().map(|s| s.as_str()) == Some("--json");
    let human = args.first().map(|s| s.as_str()) == Some("--human");
    if args.len() > if json || human { 1 } else { 0 } {
        eprintln!("cruft agent abi: usage: cruft agent abi [--json|--human]");
        return ExitCode::from(64);
    }
    if json {
        println!("{}", agent_abi_json());
    } else {
        print_agent_abi_human();
    }
    ExitCode::SUCCESS
}

fn print_agent_abi_human() {
    println!("Cruft Agent ABI v1");
    println!("schema_version: {AGENT_ABI_SCHEMA_VERSION}");
    for entry in AGENT_ABI_ENTRIES {
        println!(
            "  {}  kind={} availability={} budget={}",
            entry.name, entry.kind, entry.availability, entry.budget_semantics
        );
        println!("    clone: {}", entry.clone_semantics);
        println!("    errors: {}", entry.error_semantics);
        println!("    worker: {}", entry.worker_semantics);
        println!("    revocation: {}", entry.revocation_semantics);
        if !entry.nonclaims.is_empty() {
            println!("    nonclaims: {}", entry.nonclaims.join("; "));
        }
    }
}

fn agent_abi_json() -> String {
    format!(
        "{{\"schema_version\":{},\"entries\":[{}],\"nonclaims\":{}}}",
        json_string_literal(AGENT_ABI_SCHEMA_VERSION),
        AGENT_ABI_ENTRIES
            .iter()
            .map(agent_abi_entry_json)
            .collect::<Vec<_>>()
            .join(","),
        json_str_array(&[
            "no ambient fs/network/process/package/model authority",
            "no OS process sandbox claim",
            "no direct allocator pre-kill claim",
            "no arbitrary Promise continuation persistence",
            "scheduler.* is scheduler-only and not endowed by ordinary cruft agent run",
        ])
    )
}

fn agent_abi_entry_json(entry: &AgentAbiEntry) -> String {
    format!(
        "{{\"name\":{},\"kind\":{},\"endowment\":{},\"availability\":{},\"clone_semantics\":{},\"budget_semantics\":{},\"error_semantics\":{},\"worker_semantics\":{},\"revocation_semantics\":{},\"nonclaims\":{}}}",
        json_string_literal(entry.name),
        json_string_literal(entry.kind),
        json_string_literal(entry.endowment),
        json_string_literal(entry.availability),
        json_string_literal(entry.clone_semantics),
        json_string_literal(entry.budget_semantics),
        json_string_literal(entry.error_semantics),
        json_string_literal(entry.worker_semantics),
        json_string_literal(entry.revocation_semantics),
        json_str_array(entry.nonclaims)
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
