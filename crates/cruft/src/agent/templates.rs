use std::process::ExitCode;

use super::integrity::agent_source_hash;
use crate::json_string_literal;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentInitTemplate {
    Blank,
    Demo,
    PackageReview,
    CiTriage,
    RepoReview,
    PluginQuarantine,
}

struct AgentWorkflowTemplateMetadata {
    name: &'static str,
    status: &'static str,
    summary: &'static str,
    required_tools: &'static [&'static str],
    required_secrets: &'static [&'static str],
    required_inputs: &'static [&'static str],
    budgets: &'static [&'static str],
    expected_outputs: &'static [&'static str],
    nonclaims: &'static [&'static str],
    gates: &'static [&'static str],
}

const AGENT_WORKFLOW_TEMPLATE_REGISTRY: &[AgentWorkflowTemplateMetadata] = &[
    AgentWorkflowTemplateMetadata {
        name: "blank",
        status: "scaffold",
        summary: "least-authority empty project for hand-built agents",
        required_tools: &[],
        required_secrets: &[],
        required_inputs: &["agent.js"],
        budgets: &[
            "timeout_ms",
            "max_events",
            "max_event_bytes",
            "max_tool_arg_bytes",
            "max_tool_result_bytes",
            "max_microtasks",
            "max_steps",
        ],
        expected_outputs: &["user-defined events", "audit log", "replay summary"],
        nonclaims: &[
            "no tools are granted",
            "no ambient fs/network/process/model authority",
            "no package graph execution",
        ],
        gates: &[
            "policy validate --strict --project-confined",
            "cruft agent doctor --project",
            "cruft agent replay",
        ],
    },
    AgentWorkflowTemplateMetadata {
        name: "demo",
        status: "example",
        summary: "public-surface demonstration with explicit echo/module/package/import-hook caps",
        required_tools: &["echo"],
        required_secrets: &[],
        required_inputs: &[
            "modules/policy-module.js",
            "packages/left-pad.js",
            "hooks/hooked.js",
        ],
        budgets: &[
            "timeout_ms",
            "tool_timeout_ms",
            "max_state_bytes",
            "max_events",
            "max_event_bytes",
            "max_tool_arg_bytes",
            "max_tool_result_bytes",
            "max_microtasks",
            "max_steps",
        ],
        expected_outputs: &[
            "demo events",
            "state snapshot",
            "audit log",
            "replay summary",
        ],
        nonclaims: &[
            "not a production workflow",
            "no process/shell/network/model authority",
            "no arbitrary npm graph execution",
            "no OS sandbox claim",
        ],
        gates: &[
            "agent_init_demo_project_runs",
            "policy validate --strict --project-confined",
            "worker demo run",
        ],
    },
    AgentWorkflowTemplateMetadata {
        name: "package-review",
        status: "production-bounded",
        summary: "bounded package source/metadata review through explicit source module input",
        required_tools: &[],
        required_secrets: &[],
        required_inputs: &["modules/package-input.js"],
        budgets: &[
            "timeout_ms",
            "tool_timeout_ms",
            "max_state_bytes",
            "max_events",
            "max_event_bytes",
            "max_tool_arg_bytes",
            "max_tool_result_bytes",
            "max_microtasks",
            "max_steps",
        ],
        expected_outputs: &[
            "package-review-start event",
            "package-review-finding event",
            "session snapshot",
            "run artifact manifest",
        ],
        nonclaims: &[
            "no live npm/OSV/network lookup unless separately added",
            "no process/shell authority",
            "no arbitrary npm graph execution",
            "no OS sandbox claim",
        ],
        gates: &[
            "agent_init_package_review_template_runs_bounded_structured_workflow",
            "agent bundle/replay gates",
            "policy validate --strict --project-confined",
        ],
    },
    AgentWorkflowTemplateMetadata {
        name: "ci-triage",
        status: "production-bounded",
        summary: "bounded CI log triage through explicit source module input",
        required_tools: &[],
        required_secrets: &[],
        required_inputs: &["modules/ci-log.js"],
        budgets: &[
            "timeout_ms",
            "tool_timeout_ms",
            "max_state_bytes",
            "max_events",
            "max_event_bytes",
            "max_tool_arg_bytes",
            "max_tool_result_bytes",
            "max_microtasks",
            "max_steps",
        ],
        expected_outputs: &[
            "ci-triage-start event",
            "ci-triage-finding event",
            "audit log",
            "run artifact manifest",
        ],
        nonclaims: &[
            "does not run CI commands",
            "no process/shell authority",
            "no network/model authority by default",
            "no OS sandbox claim",
        ],
        gates: &[
            "agent_init_production_profiles_run_and_project_doctor_explains_authority",
            "policy validate --strict --project-confined",
            "doctor --project",
        ],
    },
    AgentWorkflowTemplateMetadata {
        name: "repo-review",
        status: "production-bounded",
        summary: "bounded repository review through explicit fs-read repo slice",
        required_tools: &["listFiles", "readFile"],
        required_secrets: &[],
        required_inputs: &["repo/", "fs_read_include", "fs_read_exclude"],
        budgets: &[
            "timeout_ms",
            "tool_timeout_ms",
            "max_state_bytes",
            "max_events",
            "max_event_bytes",
            "max_tool_arg_bytes",
            "max_tool_result_bytes",
            "max_microtasks",
            "max_steps",
        ],
        expected_outputs: &[
            "repo-review-start event",
            "repo-review-finding event",
            "fs-read source manifest",
            "run artifact manifest",
        ],
        nonclaims: &[
            "no ambient fs",
            "bounded root .gitignore subset only",
            "no process/shell/network/model authority by default",
            "no git checkout/search claim",
        ],
        gates: &[
            "agent_init_production_profiles_run_and_project_doctor_explains_authority",
            "agent_run_fs_read_repo_ingestion_manifest_dispositions_same_thread",
            "agent_bundle_exports_fs_read_source_manifest_dispositions_without_source_copies",
        ],
    },
    AgentWorkflowTemplateMetadata {
        name: "plugin-quarantine",
        status: "production-bounded",
        summary: "bounded third-party plugin source review without executing the plugin",
        required_tools: &[],
        required_secrets: &[],
        required_inputs: &["modules/plugin-source.js"],
        budgets: &[
            "timeout_ms",
            "tool_timeout_ms",
            "max_state_bytes",
            "max_events",
            "max_event_bytes",
            "max_tool_arg_bytes",
            "max_tool_result_bytes",
            "max_microtasks",
            "max_steps",
        ],
        expected_outputs: &[
            "plugin-quarantine-start event",
            "plugin-quarantine-finding event",
            "audit log",
            "run artifact manifest",
        ],
        nonclaims: &[
            "does not execute plugin package graphs",
            "no process/shell/network/model authority by default",
            "no ambient fs",
            "no OS sandbox claim",
        ],
        gates: &[
            "agent_init_production_profiles_run_and_project_doctor_explains_authority",
            "policy validate --strict --project-confined",
            "doctor --project",
        ],
    },
];

pub(crate) fn agent_init_template_from_str(value: &str) -> Option<AgentInitTemplate> {
    match value {
        "blank" => Some(AgentInitTemplate::Blank),
        "demo" => Some(AgentInitTemplate::Demo),
        "package-review" => Some(AgentInitTemplate::PackageReview),
        "ci-triage" => Some(AgentInitTemplate::CiTriage),
        "repo-review" => Some(AgentInitTemplate::RepoReview),
        "plugin-quarantine" => Some(AgentInitTemplate::PluginQuarantine),
        _ => None,
    }
}

pub(crate) fn agent_init_template_name(template: AgentInitTemplate) -> &'static str {
    match template {
        AgentInitTemplate::Blank => "blank",
        AgentInitTemplate::Demo => "demo",
        AgentInitTemplate::PackageReview => "package-review",
        AgentInitTemplate::CiTriage => "ci-triage",
        AgentInitTemplate::RepoReview => "repo-review",
        AgentInitTemplate::PluginQuarantine => "plugin-quarantine",
    }
}

pub(crate) fn agent_init_available_templates() -> &'static str {
    "blank, demo, package-review, ci-triage, repo-review, plugin-quarantine"
}

fn agent_workflow_template_metadata(
    template: AgentInitTemplate,
) -> &'static AgentWorkflowTemplateMetadata {
    let name = agent_init_template_name(template);
    AGENT_WORKFLOW_TEMPLATE_REGISTRY
        .iter()
        .find(|metadata| metadata.name == name)
        .expect("every init template has workflow metadata")
}

fn agent_json_string_array(values: &[&str]) -> String {
    values
        .iter()
        .map(|value| json_string_literal(value))
        .collect::<Vec<_>>()
        .join(",")
}

fn agent_workflow_template_metadata_json(
    metadata: &AgentWorkflowTemplateMetadata,
    include_detail: bool,
) -> String {
    let mut fields = vec![
        format!("\"name\":{}", json_string_literal(metadata.name)),
        format!("\"status\":{}", json_string_literal(metadata.status)),
        format!("\"summary\":{}", json_string_literal(metadata.summary)),
        format!(
            "\"required_tools\":[{}]",
            agent_json_string_array(metadata.required_tools)
        ),
        format!(
            "\"required_secrets\":[{}]",
            agent_json_string_array(metadata.required_secrets)
        ),
    ];
    if include_detail {
        fields.push(format!(
            "\"required_inputs\":[{}]",
            agent_json_string_array(metadata.required_inputs)
        ));
        fields.push(format!(
            "\"budgets\":[{}]",
            agent_json_string_array(metadata.budgets)
        ));
        fields.push(format!(
            "\"expected_outputs\":[{}]",
            agent_json_string_array(metadata.expected_outputs)
        ));
        fields.push(format!(
            "\"nonclaims\":[{}]",
            agent_json_string_array(metadata.nonclaims)
        ));
        fields.push(format!(
            "\"gates\":[{}]",
            agent_json_string_array(metadata.gates)
        ));
    }
    format!("{{{}}}", fields.join(","))
}

pub(crate) fn run_agent_template_subcommand(args: &[String]) -> ExitCode {
    let Some(action) = args.first().map(|s| s.as_str()) else {
        eprintln!(
            "cruft agent template: usage: cruft agent template list [--json] | cruft agent template describe <name> [--json]"
        );
        return ExitCode::from(64);
    };
    match action {
        "list" => {
            let mut json = false;
            for arg in &args[1..] {
                if arg == "--json" {
                    json = true;
                } else {
                    eprintln!("cruft agent template list: unexpected argument {arg}");
                    return ExitCode::from(64);
                }
            }
            if json {
                let templates = AGENT_WORKFLOW_TEMPLATE_REGISTRY
                    .iter()
                    .map(|metadata| agent_workflow_template_metadata_json(metadata, false))
                    .collect::<Vec<_>>()
                    .join(",");
                println!(
                    "{{\"type\":\"agent_workflow_template_registry\",\"version\":1,\"templates\":[{}]}}",
                    templates
                );
            } else {
                println!("agent workflow templates:");
                for metadata in AGENT_WORKFLOW_TEMPLATE_REGISTRY {
                    println!(
                        "  {}  status={} tools={} secrets={} summary={}",
                        metadata.name,
                        metadata.status,
                        if metadata.required_tools.is_empty() {
                            "none".to_string()
                        } else {
                            metadata.required_tools.join(",")
                        },
                        if metadata.required_secrets.is_empty() {
                            "none".to_string()
                        } else {
                            metadata.required_secrets.join(",")
                        },
                        metadata.summary
                    );
                }
                println!(
                    "describe: cruft agent template describe <name>; initialize: cruft agent init --template=<name> <dir>"
                );
            }
            ExitCode::SUCCESS
        }
        "describe" => {
            let Some(name) = args.get(1).map(|s| s.as_str()) else {
                eprintln!("cruft agent template describe: missing <name>");
                return ExitCode::from(64);
            };
            let mut json = false;
            for arg in &args[2..] {
                if arg == "--json" {
                    json = true;
                } else {
                    eprintln!("cruft agent template describe {name}: unexpected argument {arg}");
                    return ExitCode::from(64);
                }
            }
            let Some(template) = agent_init_template_from_str(name) else {
                eprintln!(
                    "cruft agent template describe: unknown template {name:?}; available: {}",
                    agent_init_available_templates()
                );
                return ExitCode::from(64);
            };
            let metadata = agent_workflow_template_metadata(template);
            if json {
                println!(
                    "{{\"type\":\"agent_workflow_template\",\"version\":1,\"template\":{}}}",
                    agent_workflow_template_metadata_json(metadata, true)
                );
            } else {
                println!("template: {}", metadata.name);
                println!("status: {}", metadata.status);
                println!("summary: {}", metadata.summary);
                println!(
                    "required tools: {}",
                    if metadata.required_tools.is_empty() {
                        "none".to_string()
                    } else {
                        metadata.required_tools.join(", ")
                    }
                );
                println!(
                    "required secrets: {}",
                    if metadata.required_secrets.is_empty() {
                        "none".to_string()
                    } else {
                        metadata.required_secrets.join(", ")
                    }
                );
                println!("required inputs: {}", metadata.required_inputs.join(", "));
                println!("budgets: {}", metadata.budgets.join(", "));
                println!("expected outputs: {}", metadata.expected_outputs.join(", "));
                println!("non-claims:");
                for nonclaim in metadata.nonclaims {
                    println!("  - {nonclaim}");
                }
                println!("gates:");
                for gate in metadata.gates {
                    println!("  - {gate}");
                }
            }
            ExitCode::SUCCESS
        }
        other => {
            eprintln!(
                "cruft agent template: unsupported action {other:?}; available: list, describe"
            );
            ExitCode::from(64)
        }
    }
}

pub(crate) fn run_agent_init_subcommand(args: &[String]) -> ExitCode {
    let mut force = false;
    let mut template = AgentInitTemplate::Blank;
    let mut dir: Option<String> = None;
    let mut i = 0usize;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--force" {
            force = true;
        } else if arg == "--blank" {
            template = AgentInitTemplate::Blank;
        } else if arg == "--template" {
            i += 1;
            let Some(value) = args.get(i) else {
                eprintln!("cruft agent init: --template requires an argument");
                return ExitCode::from(64);
            };
            let Some(parsed) = agent_init_template_from_str(value) else {
                eprintln!(
                    "cruft agent init: unsupported template {value:?}; available: {}",
                    agent_init_available_templates()
                );
                return ExitCode::from(64);
            };
            template = parsed;
        } else if let Some(value) = arg.strip_prefix("--template=") {
            let Some(parsed) = agent_init_template_from_str(value) else {
                eprintln!(
                    "cruft agent init: unsupported template {value:?}; available: {}",
                    agent_init_available_templates()
                );
                return ExitCode::from(64);
            };
            template = parsed;
        } else if dir.is_none() {
            dir = Some(arg.clone());
        } else {
            eprintln!("cruft agent init: unexpected argument {arg}");
            return ExitCode::from(64);
        }
        i += 1;
    }
    let Some(dir) = dir else {
        eprintln!(
            "cruft agent init: usage: cruft agent init <dir> [--blank|--template={}] [--force]",
            agent_init_available_templates()
        );
        return ExitCode::from(64);
    };
    let root = std::path::Path::new(&dir);
    if root.exists() && !force {
        let mut entries = match std::fs::read_dir(root) {
            Ok(entries) => entries,
            Err(e) => {
                eprintln!("cruft agent init: cannot inspect {dir}: {e}");
                return ExitCode::from(66);
            }
        };
        if entries.next().is_some() {
            eprintln!(
                "cruft agent init: {dir} is not empty; pass --force to overwrite generated files"
            );
            return ExitCode::from(73);
        }
    }
    for subdir in [
        "modules", "hooks", "packages", "audit", "state", "examples", "repo/src",
    ] {
        if let Err(e) = std::fs::create_dir_all(root.join(subdir)) {
            eprintln!("cruft agent init: cannot create {subdir}: {e}");
            return ExitCode::from(73);
        }
    }
    let blank_agent_source = r#"emit({kind:"start", goal:context.goal || "blank"});
state.set("lastGoal", context.goal || "blank");
"#;
    let agent_source = r#"emit({kind:"start", goal:context.goal});
const echoed = callTool("echo", {goal:context.goal, redacted:"secret"});
emit({kind:"tool", echoed});
importValue("./policy-module.js", "moduleValue").then(function (moduleValue) {
  emit({kind:"module", value:moduleValue});
});
importValue("left-pad", "packageValue").then(function (packageValue) {
  emit({kind:"package", value:packageValue});
});
importValue("./hooked.js", "hookValue").then(function (hookValue) {
  emit({kind:"hook", value:hookValue});
});
state.set("lastGoal", context.goal);
"#;
    let package_review_agent_source = r#"emit({kind:"package-review-start", package:context.package_name || "package"});
importValue("./package-input.js", "packageInput").then(function (packageInput) {
  const text = String(packageInput.source || "");
  const lines = text.length === 0 ? 0 : text.split(/\r\n|\r|\n/).length;
  const hasPostinstall = text.indexOf("postinstall") >= 0;
  const usesNetwork = /(https?:\/\/|fetch\s*\(|XMLHttpRequest)/.test(text);
  const usesProcess = /(child_process|process\.env|process\.exit)/.test(text);
  const severity = hasPostinstall || usesNetwork || usesProcess ? "review" : "ok";
  const finding = {
    kind:"package-review-finding",
    package:context.package_name || "package",
    severity,
    lines,
    signals:{postinstall:hasPostinstall, network:usesNetwork, process:usesProcess},
    recommendation: severity === "ok" ? "no high-risk install/runtime signals found" : "manual review required for elevated signals"
  };
  emit(finding);
  state.set("lastFinding", finding);
});
"#;
    let ci_triage_agent_source = r#"emit({kind:"ci-triage-start", job:context.job || "ci"});
importValue("./ci-log.js", "ciLog").then(function (ciLog) {
  const text = String(ciLog.text || "");
  const failed = /(FAIL|ERROR|panic|failed)/i.test(text);
  const timedOut = /(timeout|timed out|deadline)/i.test(text);
  const finding = {
    kind:"ci-triage-finding",
    job:context.job || "ci",
    severity: failed || timedOut ? "review" : "ok",
    signals:{failed, timed_out:timedOut},
    recommendation: failed || timedOut ? "inspect failing stage and bounded process logs" : "no high-risk CI signal found"
  };
  emit(finding);
  state.set("lastFinding", finding);
});
"#;
    let repo_review_agent_source = r#"emit({kind:"repo-review-start", repo:context.repo || "repo"});
const listed = callTool("listFiles", {});
let text = "";
let readable = 0;
let unreadable = 0;
for (const file of listed.files || []) {
  if (file && file.readable) {
    readable += 1;
    if (file.path === "src/app.js" || file.path === "package.json" || file.path === "README.md") {
      const source = callTool("readFile", {path:file.path});
      text += "\n--- " + file.path + " ---\n" + String(source.content || "");
    }
  } else {
    unreadable += 1;
  }
}
{
  const touchesSecurity = /(auth|token|secret|permission|sandbox)/i.test(text);
  const touchesInstall = /(package-lock|postinstall|install)/i.test(text);
  const finding = {
    kind:"repo-review-finding",
    repo:context.repo || "repo",
    severity: touchesSecurity || touchesInstall ? "review" : "ok",
    files:{readable, unreadable},
    signals:{security:touchesSecurity, install:touchesInstall},
    recommendation: touchesSecurity || touchesInstall ? "manual review required for sensitive diff" : "no sensitive diff signal found"
  };
  emit(finding);
  state.set("lastFinding", finding);
}
"#;
    let plugin_quarantine_agent_source = r#"emit({kind:"plugin-quarantine-start", plugin:context.plugin_name || "plugin"});
importValue("./plugin-source.js", "pluginSource").then(function (pluginSource) {
  const text = String(pluginSource.source || "");
  const ambient = /(require\s*\(|child_process|process\.env|fs\.)/.test(text);
  const network = /(fetch\s*\(|https?:\/\/|XMLHttpRequest)/.test(text);
  const finding = {
    kind:"plugin-quarantine-finding",
    plugin:context.plugin_name || "plugin",
    verdict: ambient || network ? "quarantine" : "allowed-capability-only",
    signals:{ambient, network},
    recommendation: ambient || network ? "deny until named capability policy exists" : "no ambient authority signal found"
  };
  emit(finding);
  state.set("lastFinding", finding);
});
"#;
    let package_review_input_source = r#"export const packageInput = {
  name: "sample-package",
  source: "export function add(a,b){ return a + b; }\n"
};
"#;
    let ci_triage_input_source = r#"export const ciLog = {
  text: "test suite passed\n"
};
"#;
    let repo_review_app_source = "export const ok = true;\n";
    let repo_review_package_source =
        "{\"name\":\"sample-repo\",\"scripts\":{\"test\":\"cruft agent doctor --human\"}}\n";
    let repo_review_repo_readme =
        "# Sample repo\n\nGenerated input for a bounded repo-review agent.\n";
    let repo_review_test_source = "test secret should not be reviewed by default\n";
    let plugin_quarantine_input_source = r#"export const pluginSource = {
  source: "export function summarize(input){ return String(input).slice(0, 80); }\n"
};
"#;
    let module_source = r#"export const moduleValue = 41;"#;
    let hook_source = r#"export const hookValue = 43;"#;
    let package_source = r#"export const packageValue = 42;"#;
    let denial_source =
        r#"callTool("slow", {should:"be denied because policy grants only echo"});"#;
    let blank_readme = "# Cruft Agent Compartment blank project\n\nThis project starts with least authority: no tools, modules, packages, or import hooks are granted.\n\n## Inspect before running\n\n```sh\ncruft agent doctor --human\ncruft agent tool list\ncruft agent policy validate --strict --project-confined .\ncruft agent policy explain .\ncruft agent policy explain --json .\ncruft agent policy diff --json .\n```\n\n## Run\n\n```sh\ncruft agent run --project .\ncruft agent replay --human audit/agent.jsonl\n```\n\nAdd only the authority your task needs with `cruft agent add-tool`, `add-module`, `add-package`, and `add-import-hook`; revoke it with `remove-tool`, `remove-module`, `remove-package`, and `remove-import-hook`. Use `cruft agent tool describe <name>` before granting a tool. Use `cruft agent init --template=demo <dir>` for the tutorial project with pre-granted demo authority.\n";
    let blank_policy = r#"{
  "schema_version": 1,
  "agent": "agent.js",
  "worker": false,
  "audit_log": "audit/agent.jsonl",
  "context": {"goal":"blank"},
  "state": {},
  "tools": [],
  "budgets": {
    "timeout_ms": 500,
    "tool_timeout_ms": 250,
    "max_state_bytes": 65536,
    "max_events": 32,
    "max_event_bytes": 8192,
    "max_tool_arg_bytes": 8192,
    "max_tool_result_bytes": 8192,
    "max_microtasks": 1000,
    "max_steps": 100000
  },
  "modules": {},
  "packages": {},
  "package_integrity": {},
  "import_hooks": {},
  "import_hook_integrity": {}
}
"#;
    let blank_writes = [
        ("agent.js", blank_agent_source),
        ("README.md", blank_readme),
        (
            "run.sh",
            "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\nCRUFT_BIN=${CRUFT_BIN:-cruft}\n\"$CRUFT_BIN\" agent policy validate --strict --project-confined .\n\"$CRUFT_BIN\" agent run --project .\n\"$CRUFT_BIN\" agent replay --human audit/agent.jsonl\n",
        ),
    ];
    let demo_writes = [
        ("agent.js", agent_source),
        ("modules/policy-module.js", module_source),
        ("hooks/hooked.js", hook_source),
        ("packages/left-pad.js", package_source),
        ("examples/denied-tool.js", denial_source),
        (
            "README.md",
            "# Cruft Agent Compartment example\n\nThis project is generated to exercise the public Agent Compartment surface that `cruft agent doctor` reports today.\n\n## Inspect before running\n\n```sh\ncruft agent doctor --human\ncruft agent tool list\ncruft agent tool describe echo\ncruft agent policy validate --strict --project-confined .\ncruft agent policy explain .\ncruft agent policy explain --json .\ncruft agent policy diff --json .\n```\n\n## Run the allowed example\n\n```sh\ncruft agent run --project .\ncruft agent replay --human audit/agent.jsonl\ncruft agent run --project . --worker\n```\n\n`agent.js` uses the granted `echo` tool, explicit context, bounded event/tool/state budgets, `state/session.json`, `modules/policy-module.js`, `packages/left-pad.js`, and the source-hash-capped import hook at `hooks/hooked.js`.\n\n## Exercise a denial\n\n```sh\ncruft agent run examples/denied-tool.js --timeout-ms=500 --audit-log=audit/denied-tool.jsonl\ncruft agent replay --human audit/denied-tool.jsonl\n```\n\nThe denial example is expected to fail because this policy grants only `echo`; it leaves a replayable `tool_denial` audit record.\n\n## Evolve the policy\n\n```sh\ncruft agent tool describe echo\ncruft agent add-tool . echo\ncruft agent add-module . modules/policy-module.js --specifier=./policy-module.js\ncruft agent add-package . left-pad packages/left-pad.js\ncruft agent add-import-hook . ./hooked.js hooks/hooked.js\ncruft agent set-context . '{\"goal\":\"review\"}'\ncruft agent set-budget . timeout_ms=500 max_steps=100000\ncruft agent set-session . state/session.json\ncruft agent set-worker . false\ncruft agent remove-tool . echo\ncruft agent remove-module . ./policy-module.js\ncruft agent remove-package . left-pad\ncruft agent remove-import-hook . ./hooked.js\ncruft agent unset-context .\ncruft agent unset-budget . timeout_ms\ncruft agent unset-session .\ncruft agent unset-worker .\n```\n\nThe mutation commands update `agent-policy.json`. Package and import-hook add commands generate matching integrity entries; remove commands delete matching integrity entries atomically. Users do not hand-compute or hand-remove them.\n\n## Non-claims\n\nThis project grants only the listed tool, module, package file, import hook, context, budgets, session path, and audit log. `policy validate --strict --project-confined` rejects absolute and parent-directory authority paths for shareable bundles. It does not grant OS/process sandboxing, process/shell tools, network policy, arbitrary npm graphs, direct allocator pre-kill, or external async/process cancellation. `--project` loads `agent-policy.json`; use `--policy agent-policy.json` when you want to name the policy file directly.\n",
        ),
        (
            "run.sh",
            "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\nCRUFT_BIN=${CRUFT_BIN:-cruft}\n\"$CRUFT_BIN\" agent policy validate --strict --project-confined .\n\"$CRUFT_BIN\" agent run --project .\n\"$CRUFT_BIN\" agent replay --human audit/agent.jsonl\n",
        ),
    ];
    let package_review_readme = "# Cruft Agent package-review project\n\nThis project is a production task template for bounded package review. It starts with least authority for the task: no tools, no packages, and no import hooks. The only grant is the explicit `./package-input.js` source module that contains the review input.\n\n## Inspect before running\n\n```sh\ncruft agent doctor --human\ncruft agent policy validate --strict --project-confined .\ncruft agent policy explain .\ncruft agent policy explain --json .\ncruft agent policy diff --json .\n```\n\n## Review workflow\n\nEdit `modules/package-input.js` with the package source or metadata you want reviewed, then run:\n\n```sh\ncruft agent run --project .\ncruft agent replay --human audit/agent.jsonl\n```\n\nThe agent emits structured `package-review-finding` events and stores the last finding in `state/session.json`. This template does not grant demo tools, demo packages, network policy, process/shell tools, arbitrary npm graph execution, or OS/process sandboxing.\n";
    let ci_triage_readme = "# Cruft Agent ci-triage project\n\nThis profile triages explicit CI log text without granting process, shell, network, or ambient filesystem authority. Edit `modules/ci-log.js`, then run `cruft agent run --project .`.\n\nInspect with `cruft agent doctor --project . --human`, `cruft agent policy validate --strict --project-confined .`, and `cruft agent replay --human audit/agent.jsonl`.\n";
    let repo_review_readme = "# Cruft Agent repo-review project\n\nThis profile reviews an explicit bounded `repo/` directory without granting ambient filesystem authority. Edit files under `repo/`, then run `cruft agent run --project .`.\n\nThe generated policy admits only `repo/`, includes `src/*.js`, `package.json`, and `README.md`, and excludes `*.test.js`. `listFiles` exposes source-manifest dispositions for readable files and denied entries; `readFile` returns only admitted UTF-8 text plus source hashes.\n\nInspect with `cruft agent doctor --project . --human`, `cruft agent policy explain .`, and `cruft agent replay --human audit/agent.jsonl`. This profile does not grant ambient `fs`, process/shell tools, network, model calls, arbitrary npm graph execution, or OS/process sandboxing.\n";
    let plugin_quarantine_readme = "# Cruft Agent plugin-quarantine project\n\nThis profile reviews explicit plugin source text for ambient-authority signals without executing package graphs or granting shell/network authority. Edit `modules/plugin-source.js`, then run `cruft agent run --project .`.\n\nInspect with `cruft agent doctor --project . --human`, `cruft agent policy validate --strict --project-confined .`, and `cruft agent replay --human audit/agent.jsonl`.\n";
    let package_review_writes = [
        ("agent.js", package_review_agent_source),
        ("modules/package-input.js", package_review_input_source),
        ("README.md", package_review_readme),
        (
            "run.sh",
            "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\nCRUFT_BIN=${CRUFT_BIN:-cruft}\n\"$CRUFT_BIN\" agent policy validate --strict --project-confined .\n\"$CRUFT_BIN\" agent run --project .\n\"$CRUFT_BIN\" agent replay --human audit/agent.jsonl\n",
        ),
    ];
    let ci_triage_writes = [
        ("agent.js", ci_triage_agent_source),
        ("modules/ci-log.js", ci_triage_input_source),
        ("README.md", ci_triage_readme),
        (
            "run.sh",
            "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\nCRUFT_BIN=${CRUFT_BIN:-cruft}\n\"$CRUFT_BIN\" agent doctor --project . --human\n\"$CRUFT_BIN\" agent policy validate --strict --project-confined .\n\"$CRUFT_BIN\" agent run --project .\n\"$CRUFT_BIN\" agent replay --human audit/agent.jsonl\n",
        ),
    ];
    let repo_review_writes = [
        ("agent.js", repo_review_agent_source),
        ("repo/src/app.js", repo_review_app_source),
        ("repo/src/app.test.js", repo_review_test_source),
        ("repo/package.json", repo_review_package_source),
        ("repo/README.md", repo_review_repo_readme),
        ("README.md", repo_review_readme),
        (
            "run.sh",
            "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\nCRUFT_BIN=${CRUFT_BIN:-cruft}\n\"$CRUFT_BIN\" agent doctor --project . --human\n\"$CRUFT_BIN\" agent policy validate --strict --project-confined .\n\"$CRUFT_BIN\" agent run --project .\n\"$CRUFT_BIN\" agent replay --human audit/agent.jsonl\n",
        ),
    ];
    let plugin_quarantine_writes = [
        ("agent.js", plugin_quarantine_agent_source),
        ("modules/plugin-source.js", plugin_quarantine_input_source),
        ("README.md", plugin_quarantine_readme),
        (
            "run.sh",
            "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\nCRUFT_BIN=${CRUFT_BIN:-cruft}\n\"$CRUFT_BIN\" agent doctor --project . --human\n\"$CRUFT_BIN\" agent policy validate --strict --project-confined .\n\"$CRUFT_BIN\" agent run --project .\n\"$CRUFT_BIN\" agent replay --human audit/agent.jsonl\n",
        ),
    ];
    let writes: &[(&str, &str)] = match template {
        AgentInitTemplate::Blank => &blank_writes,
        AgentInitTemplate::Demo => &demo_writes,
        AgentInitTemplate::PackageReview => &package_review_writes,
        AgentInitTemplate::CiTriage => &ci_triage_writes,
        AgentInitTemplate::RepoReview => &repo_review_writes,
        AgentInitTemplate::PluginQuarantine => &plugin_quarantine_writes,
    };
    for (rel, contents) in writes {
        let path = root.join(rel);
        if path.exists() && !force {
            eprintln!("cruft agent init: {rel} exists; pass --force to overwrite generated files");
            return ExitCode::from(73);
        }
        if let Err(e) = std::fs::write(&path, contents) {
            eprintln!("cruft agent init: cannot write {}: {e}", path.display());
            return ExitCode::from(73);
        }
    }
    let policy = match template {
        AgentInitTemplate::Blank => blank_policy.to_string(),
        AgentInitTemplate::Demo => format!(
            r#"{{
  "schema_version": 1,
  "agent": "agent.js",
  "worker": false,
  "audit_log": "audit/agent.jsonl",
  "context": {{"goal":"first-run"}},
  "state": {{"boot":true}},
  "session_file": "state/session.json",
  "tools": ["echo"],
  "redact_fields": ["redacted"],
  "budgets": {{
    "timeout_ms": 500,
    "tool_timeout_ms": 250,
    "max_state_bytes": 65536,
    "max_events": 32,
    "max_event_bytes": 8192,
    "max_tool_arg_bytes": 8192,
    "max_tool_result_bytes": 8192,
    "max_microtasks": 1000,
    "max_steps": 100000
  }},
  "modules": {{"./policy-module.js":"modules/policy-module.js"}},
  "packages": {{"left-pad":"packages/left-pad.js"}},
  "package_integrity": {{"left-pad":"{}"}},
  "import_hooks": {{"./hooked.js":"hooks/hooked.js"}},
  "import_hook_integrity": {{"./hooked.js":"{}"}}
}}
"#,
            agent_source_hash(package_source),
            agent_source_hash(hook_source)
        ),
        AgentInitTemplate::PackageReview => format!(
            r#"{{
  "schema_version": 1,
  "profile": "package-review",
  "agent": "agent.js",
  "worker": false,
  "audit_log": "audit/agent.jsonl",
  "context": {{"package_name":"sample-package","review_goal":"bounded package review"}},
  "state": {{}},
  "session_file": "state/session.json",
  "tools": [],
  "budgets": {{
    "timeout_ms": 500,
    "tool_timeout_ms": 250,
    "max_state_bytes": 65536,
    "max_events": 16,
    "max_event_bytes": 8192,
    "max_tool_arg_bytes": 8192,
    "max_tool_result_bytes": 8192,
    "max_microtasks": 1000,
    "max_steps": 100000
  }},
  "modules": {{"./package-input.js":"modules/package-input.js"}},
  "packages": {{}},
  "package_integrity": {{}},
  "import_hooks": {{}},
  "import_hook_integrity": {{}}
}}
"#
        ),
        AgentInitTemplate::CiTriage => r#"{
  "schema_version": 1,
  "profile": "ci-triage",
  "agent": "agent.js",
  "worker": false,
  "audit_log": "audit/agent.jsonl",
  "context": {"job":"local-ci"},
  "state": {},
  "session_file": "state/session.json",
  "tools": [],
  "budgets": {
    "timeout_ms": 500,
    "tool_timeout_ms": 250,
    "max_state_bytes": 65536,
    "max_events": 16,
    "max_event_bytes": 8192,
    "max_tool_arg_bytes": 8192,
    "max_tool_result_bytes": 8192,
    "max_microtasks": 1000,
    "max_steps": 100000
  },
  "modules": {"./ci-log.js":"modules/ci-log.js"},
  "packages": {},
  "package_integrity": {},
  "import_hooks": {},
  "import_hook_integrity": {}
}
"#
        .to_string(),
        AgentInitTemplate::RepoReview => r#"{
  "schema_version": 1,
  "profile": "repo-review",
  "agent": "agent.js",
  "worker": false,
  "audit_log": "audit/agent.jsonl",
  "context": {"repo":"local-repo"},
  "state": {},
  "session_file": "state/session.json",
  "tools": [],
  "budgets": {
    "timeout_ms": 500,
    "tool_timeout_ms": 250,
    "max_state_bytes": 65536,
    "max_events": 16,
    "max_event_bytes": 8192,
    "max_tool_arg_bytes": 8192,
    "max_tool_result_bytes": 8192,
    "max_microtasks": 1000,
    "max_steps": 100000
  },
  "fs_read": ["repo"],
  "fs_read_include": ["src/*.js", "package.json", "README.md"],
  "fs_read_exclude": ["*.test.js"],
  "modules": {},
  "packages": {},
  "package_integrity": {},
  "import_hooks": {},
  "import_hook_integrity": {}
}
"#
        .to_string(),
        AgentInitTemplate::PluginQuarantine => r#"{
  "schema_version": 1,
  "profile": "plugin-quarantine",
  "agent": "agent.js",
  "worker": false,
  "audit_log": "audit/agent.jsonl",
  "context": {"plugin_name":"third-party-plugin"},
  "state": {},
  "session_file": "state/session.json",
  "tools": [],
  "redact_fields": ["secret", "token"],
  "budgets": {
    "timeout_ms": 500,
    "tool_timeout_ms": 250,
    "max_state_bytes": 65536,
    "max_events": 16,
    "max_event_bytes": 8192,
    "max_tool_arg_bytes": 8192,
    "max_tool_result_bytes": 8192,
    "max_microtasks": 1000,
    "max_steps": 100000
  },
  "modules": {"./plugin-source.js":"modules/plugin-source.js"},
  "packages": {},
  "package_integrity": {},
  "import_hooks": {},
  "import_hook_integrity": {}
}
"#
        .to_string(),
    };
    if let Err(e) = std::fs::write(root.join("agent-policy.json"), policy) {
        eprintln!("cruft agent init: cannot write agent-policy.json: {e}");
        return ExitCode::from(73);
    }
    println!("created {}", root.display());
    println!("template: {}", agent_init_template_name(template));
    println!(
        "describe: cruft agent template describe {}",
        agent_init_template_name(template)
    );
    println!("run: cruft agent run --project {}", root.display());
    ExitCode::SUCCESS
}
