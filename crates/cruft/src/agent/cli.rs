use super::abi::run_agent_abi_subcommand;
use super::approvals::run_agent_approval_subcommand;
use super::audit::{
    run_agent_bundle_subcommand, run_agent_bundle_verify_subcommand, run_agent_history_subcommand,
    run_agent_replay_subcommand, run_agent_reset_subcommand,
};
use super::cpx_attestation::run_agent_cpx_attestation_subcommand;
use super::cpx_risk::run_agent_cpx_risk_subcommand;
use super::doctor::run_agent_doctor_subcommand;
use super::facade::run_agent_harness_subcommand;
use super::integrity::run_agent_hash_subcommand;
use super::policy::{run_agent_policy_mutation_subcommand, run_agent_policy_subcommand};
use super::run::run_agent_run_subcommand;
use super::schedule::run_agent_schedule_subcommand;
use super::templates::{run_agent_init_subcommand, run_agent_template_subcommand};
use super::tools::run_agent_tool_subcommand;
use std::process::ExitCode;

fn print_agent_help() {
    println!(
        "Usage: cruft agent <command> [options]

Run JavaScript agents inside audited Cruft Compartments.

Common workflows:
    init <dir> [--template=<name>]       Create a bounded agent project
    doctor [--json|--human] [--project <dir>]
                                        Explain supported controls and non-claims
    run [--project=<dir>|--policy=<file>] [agent.js] [options]
                                        Execute an agent with explicit authority
    replay [--json|--human] <audit.jsonl>
                                        Replay an audit log
    bundle <project|policy> --out <dir>  Create a replayable bundle

Policy and authority:
    policy validate|explain|risk|diff    Inspect or validate policy
    add-tool|remove-tool                 Mutate tool grants
    add-module|remove-module             Mutate exact source-module grants
    add-package|remove-package           Mutate exact package-file grants
    add-import-hook|remove-import-hook   Mutate exact import-hook grants
    set-context|unset-context            Mutate context JSON
    set-budget|unset-budget              Mutate resource budgets
    set-session|unset-session            Mutate session file
    set-worker|unset-worker              Mutate worker mode

Inspection and integrations:
    tool list|describe <name>            Inspect available tools
    template list|describe <name>        Inspect project templates
    hash <path> [--kind=<kind>]          Compute source/integrity hash
    history <project|policy|audit>       Summarize audit history
    reset [--dry-run|--rollback] <target>
                                        Reset generated audit/state artifacts
    schedule start|tick|input|cancel|status|list|replay
                                        Run bounded scheduled agent jobs
    approval allow|deny <log> <id>       Resolve approval requests
    cpx-risk validate|evaluate <json>    Validate package-exec risk artifacts
    cpx-attestation validate <json>      Validate package audit attestations
    abi [--json|--human]                 Print the agent ABI contract
    harness contract [--json|--human]    Print the local harness API contract

Examples:
    cruft agent init --template=demo ./agent-demo
    cruft agent doctor --project ./agent-demo --human
    cruft agent run --project ./agent-demo
    cruft agent replay --human ./agent-demo/audit/agent.jsonl

Non-claims:
    Agent Compartments deny ambient authority by default, but they do not claim
    a general OS sandbox, shell/exec/spawn tools, ambient network policy, or
    arbitrary npm graph execution."
    );
}

pub(crate) fn run_agent_subcommand(args: &[String]) -> ExitCode {
    if matches!(
        args.first().map(|s| s.as_str()),
        None | Some("--help") | Some("-h") | Some("help")
    ) {
        print_agent_help();
        return ExitCode::SUCCESS;
    }
    if args.first().map(|s| s.as_str()) == Some("doctor") {
        return run_agent_doctor_subcommand(&args[1..]);
    }
    if args.first().map(|s| s.as_str()) == Some("abi") {
        return run_agent_abi_subcommand(&args[1..]);
    }
    if args.first().map(|s| s.as_str()) == Some("cpx-risk") {
        return run_agent_cpx_risk_subcommand(&args[1..]);
    }
    if args.first().map(|s| s.as_str()) == Some("cpx-attestation") {
        return run_agent_cpx_attestation_subcommand(&args[1..]);
    }
    if args.first().map(|s| s.as_str()) == Some("hash") {
        return run_agent_hash_subcommand(&args[1..]);
    }
    if args.first().map(|s| s.as_str()) == Some("harness") {
        return run_agent_harness_subcommand(&args[1..]);
    }
    if args.first().map(|s| s.as_str()) == Some("approval") {
        return run_agent_approval_subcommand(&args[1..]);
    }
    if args.first().map(|s| s.as_str()) == Some("policy") {
        return run_agent_policy_subcommand(&args[1..]);
    }
    if args.first().map(|s| s.as_str()) == Some("tool") {
        return run_agent_tool_subcommand(&args[1..]);
    }
    if args.first().map(|s| s.as_str()) == Some("template") {
        return run_agent_template_subcommand(&args[1..]);
    }
    if args.first().map(|s| s.as_str()) == Some("init") {
        return run_agent_init_subcommand(&args[1..]);
    }
    if args.first().map(|s| s.as_str()) == Some("bundle") {
        return run_agent_bundle_subcommand(&args[1..]);
    }
    if args.first().map(|s| s.as_str()) == Some("bundle-verify") {
        return run_agent_bundle_verify_subcommand(&args[1..]);
    }
    if let Some(
        command @ ("add-tool" | "remove-tool" | "add-module" | "remove-module" | "add-package"
        | "remove-package" | "add-import-hook" | "remove-import-hook" | "set-context"
        | "unset-context" | "set-budget" | "unset-budget" | "set-session"
        | "unset-session" | "set-worker" | "unset-worker"),
    ) = args.first().map(|s| s.as_str())
    {
        return run_agent_policy_mutation_subcommand(command, &args[1..]);
    }
    if args.first().map(|s| s.as_str()) == Some("replay") {
        return run_agent_replay_subcommand(&args[1..]);
    }
    if args.first().map(|s| s.as_str()) == Some("history") {
        return run_agent_history_subcommand(&args[1..]);
    }
    if args.first().map(|s| s.as_str()) == Some("reset") {
        return run_agent_reset_subcommand(&args[1..]);
    }
    if args.first().map(|s| s.as_str()) == Some("schedule") {
        return run_agent_schedule_subcommand(&args[1..]);
    }
    if args.first().map(|s| s.as_str()) == Some("run") {
        return run_agent_run_subcommand(args);
    }
    eprintln!(
        "cruft agent: unknown command. Run `cruft agent --help` for workflows and command groups."
    );
    ExitCode::from(64)
}
