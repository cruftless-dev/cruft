use std::process::ExitCode;

use super::policy::{
    agent_policy_load_target, agent_policy_string_field, agent_policy_validate_value_with_options,
    AgentPolicyValidationOptions,
};
use crate::json_escape;

pub(crate) fn run_agent_doctor_subcommand(args: &[String]) -> ExitCode {
    let mut human = false;
    let mut project: Option<String> = None;
    let mut i = 0usize;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--json" {
            human = false;
        } else if arg == "--human" {
            human = true;
        } else if arg == "--project" {
            i += 1;
            let Some(value) = args.get(i) else {
                eprintln!("cruft agent doctor: --project requires a directory");
                return ExitCode::from(64);
            };
            project = Some(value.clone());
        } else if let Some(value) = arg.strip_prefix("--project=") {
            project = Some(value.to_string());
        } else {
            eprintln!(
                "cruft agent doctor: usage: cruft agent doctor [--json|--human] [--project <dir>]"
            );
            return ExitCode::from(64);
        }
        i += 1;
    }
    if let Some(project) = project {
        return run_agent_project_doctor(&project, human);
    }
    if human {
        println!("Cruft agent doctor");
        println!("status: available");
        println!("ambient authority: denied by default");
        println!("tools: explicit registry; process is argv-policy supervised with --process-command/--process-cwd; shell/exec/spawn unavailable");
        println!("approval gates: --require-approval=<tool> blocks same-thread and worker tools before host effect unless --approve-tool=<tool> pregrants; --approval-log plus --approval-max-age-ms enforces durable decision freshness; pending/granted/denied/stale decisions are replay-visible; worker approval uses P-RUNTIME-WORKER-HOST-CALL-RPC without tenant approval-log authority");
        println!("modules: explicit source modules and static entry modules");
        println!("packages: exact file caps or conservative package-root graph caps; arbitrary npm/lockfile graphs unavailable");
        println!("fs read: explicit --fs-read path caps expose readFile/listFiles with source-manifest dispositions; --fs-read-include/--fs-read-exclude narrow repo slices; no ambient fs");
        println!(
            "expected events: --expect-event kind=schema.json validates selected event payloads"
        );
        println!(
            "artifact write: explicit --fs-write output caps expose writeArtifact; no ambient fs"
        );
        println!("osv query: named osv.query lookup; same-thread live transport pinned to https://api.osv.dev/v1/query or fixture-backed with --osv-fixture; worker osv.query remains routed and fails closed before tenant execution with agent_worker_osv_tool_membrane_required_not_available; no general network");
        println!("npm metadata: named npm.metadata lookup; same-thread live transport pinned to https://registry.npmjs.org/<package>; worker npm.metadata remains routed and fails closed before tenant execution with agent_worker_npm_metadata_tool_membrane_required_not_available; no general network");
        println!("github issue: named github.issue.read lookup; same-thread live transport pinned to https://api.github.com/repos/<owner>/<repo>/issues/<number> or worker persistent-cache-backed with --named-network-cache-dir; optional host-only bearer token via --github-token-env is same-thread only; audit exposes env name/mode, never token; worker live transport and token forwarding routed; no general network or tenant credential exposure");
        println!("github pr: named github.pr.read lookup; same-thread live transport pinned to https://api.github.com/repos/<owner>/<repo>/pulls/<number> or worker persistent-cache-backed with --named-network-cache-dir; optional host-only bearer token via --github-token-env is same-thread only; audit exposes env name/mode, never token; worker live transport and token forwarding routed; no general network or tenant credential exposure");
        println!("github pr files: named github.pr.files.list lookup; same-thread live transport pinned to https://api.github.com/repos/<owner>/<repo>/pulls/<number>/files; worker github.pr.files.list remains routed and fails closed before tenant execution with agent_worker_github_complex_payload_tool_membrane_required_not_available; optional host-only bearer token via --github-token-env is same-thread only; audit exposes env name/mode, never token; no repo checkout, git, search, general network, or tenant credential exposure");
        println!("github latest release: named github.release.latest.read lookup; same-thread live transport pinned to https://api.github.com/repos/<owner>/<repo>/releases/latest or worker persistent-cache-backed with --named-network-cache-dir; optional host-only bearer token via --github-token-env is same-thread only; audit exposes env name/mode, never token; worker live transport and token forwarding routed; no general network or tenant credential exposure");
        println!("github file: named github.file.read lookup; same-thread live transport pinned to https://api.github.com/repos/<owner>/<repo>/contents/<path>?ref=<ref> or worker persistent-cache-backed with --named-network-cache-dir; optional host-only bearer token via --github-token-env is same-thread only; audit exposes env name/mode, never token; worker live transport and token forwarding routed; no general network, repo checkout, ambient fs, or tenant credential exposure");
        println!("github compare: named github.compare.read lookup; same-thread live transport pinned to https://api.github.com/repos/<owner>/<repo>/compare/<base>...<head> or worker persistent-cache-backed with --named-network-cache-dir; optional host-only bearer token via --github-token-env is same-thread only; audit exposes env name/mode, never token; worker live transport and token forwarding routed; no general network, repo checkout, ambient git, or tenant credential exposure");
        println!("github commit: named github.commit.read lookup; same-thread live transport pinned to https://api.github.com/repos/<owner>/<repo>/commits/<ref>; worker github.commit.read remains routed and fails closed before tenant execution with agent_worker_github_complex_payload_tool_membrane_required_not_available; optional host-only bearer token via --github-token-env is same-thread only; audit exposes env name/mode, never token; no general network, repo checkout, ambient git, or tenant credential exposure");
        println!("github repo: named github.repo.read lookup; same-thread live transport pinned to https://api.github.com/repos/<owner>/<repo> or worker persistent-cache-backed with --named-network-cache-dir; optional host-only bearer token via --github-token-env is same-thread only; audit exposes env name/mode, never token; worker live transport and token forwarding routed; no general network or tenant credential exposure");
        println!("named network cache: optional same-thread persistent cache with --named-network-cache-dir, --named-network-cache-mode, --named-network-cache-max-age-ms, and --named-network-cache-max-entries; cache hits/stale/offline misses/retention prunes are replay-visible; no hidden freshness overclaim");
        println!("named network retry: optional bounded same-thread live retry with --named-network-retry-attempts=0..3; retry audit rows and replay counts are visible; nonretryable errors do not retry");
        println!("model.call: named model tool currently routed and fails closed before tenant execution with agent_model_tool_membrane_required_not_available; fixture/provider and worker variants remain non-claimed until the model tool membrane is repaired");
        println!("secret scopes: --secret <tool=ENV> grants a host env credential only to admitted named host tools; audit/run_start expose env name and mode, never value; model.call remains routed; broader lifecycle/rotation remains routed");
        println!("process output: same-thread process supports per-call output:\"summary\" for bounded tenant previews/counts; default full output remains byte-capped/fail-closed; replay reports stream and budget dispositions");
        println!("import hooks: source-hash caps only");
        println!("audit redaction: tenant events and tool payloads redact common secret fields plus --redact-field / policy redact_fields");
        println!("budgets: sync timeout, microtasks, steps, event bytes, tool payloads, clone payloads, and child RSS with --max-rss-mb");
        println!("state/session: single-turn state plus file session with --session-file");
        println!("worker: bounded worker mouth available; full external/effectful tool membrane unavailable");
        println!(
            "replay: use cruft agent replay --human <audit.jsonl> for support-readable summaries"
        );
        println!("claim predicate: sandbox claims must be scoped to these controls");
        return ExitCode::SUCCESS;
    }
    println!("{}", agent_doctor_json());
    ExitCode::SUCCESS
}

fn run_agent_project_doctor(project: &str, human: bool) -> ExitCode {
    let (policy_path, _source, value) = match agent_policy_load_target(project) {
        Ok(loaded) => loaded,
        Err(e) => {
            eprintln!("cruft agent doctor --project: {e}");
            return ExitCode::from(65);
        }
    };
    let validation = agent_policy_validate_value_with_options(
        &policy_path,
        &value,
        AgentPolicyValidationOptions {
            strict: true,
            project_confined: true,
        },
    );
    let object = value.as_object().expect("policy object validated at load");
    let profile = agent_policy_string_field(object, "profile")
        .ok()
        .flatten()
        .unwrap_or("custom");
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
    if human {
        println!("Cruft agent project doctor");
        println!("project: {project}");
        println!("policy: {policy_path}");
        println!("profile: {profile}");
        println!(
            "validation: {}",
            if validation.is_ok() {
                "valid strict project-confined"
            } else {
                "invalid"
            }
        );
        println!(
            "authority: tools={} approval_required_tools={} approved_tools={} secrets={} modules={} packages={} import_hooks={} fs_read={} fs_write={} process_commands={}",
            count_array("tools"),
            count_array("approval_required_tools"),
            count_array("approved_tools"),
            count_array("secrets"),
            count_object("modules"),
            count_object("packages"),
            count_object("import_hooks"),
            count_array("fs_read"),
            count_array("fs_write"),
            count_array("process_commands")
        );
        println!(
            "controls: see generic doctor below; public claims remain scoped to listed controls"
        );
        println!("non-claims: ambient fs/network/process/model authority, shell/exec/spawn, OS sandboxing, arbitrary npm/lockfile graphs");
        if let Err(errors) = validation {
            for error in errors {
                println!("  - {error}");
            }
            return ExitCode::from(65);
        }
        return ExitCode::SUCCESS;
    }
    println!(
        "{{\"type\":\"agent_project_doctor\",\"project\":\"{}\",\"policy\":\"{}\",\"profile\":\"{}\",\"validation\":{{\"valid\":{},\"profile\":\"strict_project_confined\"}},\"authority_counts\":{{\"tools\":{},\"approval_required_tools\":{},\"approved_tools\":{},\"secrets\":{},\"modules\":{},\"packages\":{},\"import_hooks\":{},\"fs_read\":{},\"fs_write\":{},\"process_commands\":{}}},\"non_claims\":[\"ambient fs\",\"ambient network\",\"ambient process/model authority\",\"shell/exec/spawn\",\"OS sandboxing\",\"arbitrary npm/lockfile graphs\"],\"control_snapshot\":{}}}",
        json_escape(project),
        json_escape(&policy_path),
        json_escape(profile),
        if validation.is_ok() { "true" } else { "false" },
        count_array("tools"),
        count_array("approval_required_tools"),
        count_array("approved_tools"),
        count_array("secrets"),
        count_object("modules"),
        count_object("packages"),
        count_object("import_hooks"),
        count_array("fs_read"),
        count_array("fs_write"),
        count_array("process_commands"),
        agent_doctor_json()
    );
    if validation.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(65)
    }
}

pub(crate) fn agent_doctor_json() -> &'static str {
    r#"{"type":"agent_doctor","mouth":"cruft agent","status":"available","controls":{"ambient_authority":"denied_by_default","tools":"explicit_registry","approval_gates":"same_thread_and_worker_pre_effect_with_--require-approval_and_--approve-tool_durable_approval_log_allow_deny_stale_with_--approval-max-age-ms_replay_visible_worker_approval_uses_P-RUNTIME-WORKER-HOST-CALL-RPC_no_tenant_approval_log_authority","secret_scopes":"same_thread_named_tool_host_env_with_--secret_tool_ENV_audit_env_name_mode_no_value_model_call_routed","event_budget":"enforced","expected_events":"schema_validation_with_--expect-event","audit_redaction":"tenant_events_and_tool_payloads_common_secret_fields_plus_--redact-field_or_policy_redact_fields","tool_payload_budget":"enforced","clone_payload_bytes":"enforced","memory_rss":"enforced_by_child_process_supervisor_with_--max-rss-mb","sync_timeout":"enforced","tenant_timeout_catchability":"uncatchable_gate","microtask_budget":"enforced_with_--max-microtasks","pending_promise_disposition":"detached_at_turn_end","async_tool_timeout":"enforced_for_cli_tool_promises_with_--tool-timeout-ms","process_tools":"argv_policy_supervised_sync_with_--process-command_and_--process-cwd_output_summary_mode_worker_required_not_available_reason_agent_worker_process_host_call_required_not_available_shell_exec_spawn_unavailable","step_budget":"enforced_with_--max-steps","module_policy":"explicit","entrypoint":"script_or_explicit_static_module","package_imports":"explicit_file_or_conservative_graph_caps_with_--package","fs_read":"path_caps_with_byte_budget_source_manifest_include_exclude_dispositions_with_--fs-read","artifact_write":"path_caps_with_byte_budget_with_--fs-write","osv_query":"named_lookup_fixture_or_same_thread_pinned_https_api_osv_dev_v1_query_worker_required_not_available_reason_agent_worker_osv_tool_membrane_required_not_available_no_general_network","npm_metadata":"named_lookup_same_thread_pinned_https_registry_npmjs_org_package_worker_required_not_available_reason_agent_worker_npm_metadata_tool_membrane_required_not_available_no_general_network","github_issue_read":"named_lookup_same_thread_pinned_https_api_github_com_repos_owner_repo_issues_number_optional_host_env_bearer_token_with_env_name_mode_audit_no_token_value_no_general_network_no_tenant_credential_exposure_worker_live_routed","github_pr_read":"named_lookup_same_thread_pinned_https_api_github_com_repos_owner_repo_pulls_number_or_worker_persistent_cache_optional_host_env_bearer_token_same_thread_only_with_env_name_mode_audit_no_token_value_no_general_network_no_tenant_credential_exposure_worker_live_routed","github_release_latest_read":"named_lookup_same_thread_pinned_https_api_github_com_repos_owner_repo_releases_latest_or_worker_persistent_cache_optional_host_env_bearer_token_same_thread_only_with_env_name_mode_audit_no_token_value_no_general_network_no_tenant_credential_exposure_worker_live_routed","github_repo_read":"named_lookup_same_thread_pinned_https_api_github_com_repos_owner_repo_or_worker_persistent_cache_optional_host_env_bearer_token_same_thread_only_with_env_name_mode_audit_no_token_value_no_general_network_no_tenant_credential_exposure_worker_live_routed","github_workflow_run_read":"named_lookup_same_thread_pinned_https_api_github_com_repos_owner_repo_actions_runs_run_id_optional_host_env_bearer_token_with_env_name_mode_audit_no_token_value_no_general_network_no_tenant_credential_exposure_worker_live_routed","github_workflow_jobs_list":"named_lookup_same_thread_pinned_https_api_github_com_repos_owner_repo_actions_runs_run_id_jobs_worker_required_not_available_reason_agent_worker_github_complex_payload_tool_membrane_required_not_available_optional_host_env_bearer_token_same_thread_only_with_env_name_mode_audit_no_token_value_no_general_network_no_tenant_credential_exposure_no_actions_log_download","github_check_runs_list":"named_lookup_same_thread_pinned_https_api_github_com_repos_owner_repo_commits_ref_check_runs_worker_required_not_available_reason_agent_worker_github_complex_payload_tool_membrane_required_not_available_optional_host_env_bearer_token_same_thread_only_with_env_name_mode_audit_no_token_value_no_general_network_no_tenant_credential_exposure_no_actions_log_or_artifact_download","named_network_cache":"optional_same_thread_and_worker_selected_tool_persistent_cache_with_named_network_cache_dir_mode_max_age_and_max_entries_replay_visible_no_hidden_freshness_overclaim","named_network_retry":"optional_bounded_same_thread_live_retry_with_named_network_retry_attempts_0_to_3_audit_replay_backoff_and_success_visible_nonretryable_errors_do_not_retry","model_call":"required_not_available_reason_agent_model_tool_membrane_required_not_available","import_hooks":"source_hash_caps_with_--import-hook","worker_hosted":"emit_sync_and_promise_tool_state_close_module_entry_session_source_hook_package_cap_audit_note_audit_controls_forwarding_with_--worker_full_membrane_required_not_available","state":"single_turn_snapshot","reset":"single_turn_store_and_product_session_file_reset_with_one_backup_rollback","resume":"file_session_with_--session-file","close_revocation":"sync_and_promise_turn","replay":"history_summary_run_ids_diff_event_diff_product_reset_rollback_and_bundle_scope_available"},"public_claim_predicate":"sandbox claims must be scoped to listed controls"}"#
}
