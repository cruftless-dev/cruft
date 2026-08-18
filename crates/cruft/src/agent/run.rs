use super::integrity::{
    agent_source_hash, print_agent_import_hook_integrity_repair,
    print_agent_package_integrity_repair,
};
use super::model::agent_load_model_fixture;
use super::model::{agent_model_fixture_js, AgentModelFixture};
use super::named_network::{agent_load_osv_fixture, agent_osv_fixture_js, AgentOsvFixture};
use super::packages::{collect_simple_agent_package_graph, is_agent_package_specifier};
use super::policy::{
    agent_env_key_is_valid, agent_policy_expand_run_args, agent_project_policy_path,
    agent_secret_scope_parse, agent_secret_scopes_js,
};
use super::process::{
    agent_collect_process_commands, agent_collect_process_cwds, agent_collect_process_env,
    agent_process_commands_js, agent_process_cwds_js, agent_process_env_js, AgentProcessCommand,
    AgentProcessCwd, AgentProcessEnv,
};
use super::tools::{
    agent_collect_fs_read_caps, agent_collect_fs_write_roots, agent_fs_read_caps_js,
    agent_fs_write_roots_js, agent_validate_fs_read_pattern, is_agent_builtin_tool_specifier,
    AgentFsReadFile, AgentFsWriteRoot,
};
use crate::json_string_literal;
use std::io::Write;
use std::process::ExitCode;

#[derive(Clone)]
pub(crate) struct AgentExpectedEventSchema {
    pub(crate) kind: String,
    pub(crate) schema_json: String,
}

pub(crate) struct AgentWorkerEmitConfig<'a> {
    pub(crate) source_path: &'a str,
    pub(crate) source: &'a str,
    pub(crate) audit_log: &'a str,
    pub(crate) run_id: &'a str,
    pub(crate) context_json: &'a str,
    pub(crate) state_json: &'a str,
    pub(crate) session_file: Option<&'a str>,
    pub(crate) entry_module: Option<&'a str>,
    pub(crate) tools: &'a [String],
    pub(crate) modules: &'a [(String, String)],
    pub(crate) import_hooks: &'a [(String, String)],
    pub(crate) package_imports: &'a str,
    pub(crate) timeout_ms: u64,
    pub(crate) max_events: u64,
    pub(crate) max_event_bytes: u64,
    pub(crate) max_state_bytes: u64,
    pub(crate) max_tool_arg_bytes: u64,
    pub(crate) max_tool_result_bytes: u64,
    pub(crate) tool_timeout_ms: u64,
    pub(crate) max_microtasks: u64,
    pub(crate) max_steps: Option<u64>,
    pub(crate) max_rss_mb: Option<u64>,
    pub(crate) redact_fields: &'a [String],
    pub(crate) fs_read_files: &'a [AgentFsReadFile],
    pub(crate) fs_write_roots: &'a [AgentFsWriteRoot],
    pub(crate) osv_fixture: Option<&'a AgentOsvFixture>,
    pub(crate) model_fixture: Option<&'a AgentModelFixture>,
    pub(crate) model_provider: Option<&'a str>,
    pub(crate) model_api_key_env: Option<&'a str>,
    pub(crate) model_api_key_value: Option<&'a str>,
    pub(crate) secret_scopes: &'a [(String, String)],
    pub(crate) approval_required_tools: &'a [String],
    pub(crate) approved_tools: &'a [String],
    pub(crate) approval_log: Option<&'a str>,
    pub(crate) approval_max_age_ms: Option<u64>,
    pub(crate) process_commands: &'a [AgentProcessCommand],
    pub(crate) process_cwds: &'a [AgentProcessCwd],
    pub(crate) process_env: &'a [AgentProcessEnv],
    pub(crate) named_network_cache_dir: Option<&'a str>,
    pub(crate) named_network_cache_mode: &'a str,
    pub(crate) named_network_cache_max_age_ms: Option<u64>,
    pub(crate) named_network_cache_max_entries: Option<u64>,
    pub(crate) expected_event_schemas: &'a [AgentExpectedEventSchema],
}

pub(crate) fn run_agent_worker_emit_harness(config: AgentWorkerEmitConfig<'_>) -> ExitCode {
    let max_steps_js = json_optional_u64_literal(config.max_steps);
    let max_rss_mb_js = json_optional_u64_literal(config.max_rss_mb);
    let tools_js = json_string_list_literal(config.tools);
    let modules_js = agent_worker_source_records_js(config.modules);
    let import_hooks_js = agent_worker_source_records_js(config.import_hooks);
    let session_file_js = json_optional_string_literal(config.session_file);
    let entry_module_js = json_optional_string_literal(config.entry_module);
    let memory_rss_js = agent_memory_rss_control_js(config.max_rss_mb);
    let package_imports_js = json_string_literal(config.package_imports);
    let redact_fields_js = json_string_array_literal(config.redact_fields);
    let fs_read_caps_js = agent_fs_read_caps_js(config.fs_read_files);
    let fs_write_roots_js = agent_fs_write_roots_js(config.fs_write_roots);
    let osv_fixture_js = agent_osv_fixture_js(config.osv_fixture.map(|f| f.json.as_str()));
    let model_fixture_js = agent_model_fixture_js(config.model_fixture.map(|f| f.json.as_str()));
    let model_provider_js = json_optional_string_literal(config.model_provider);
    let model_api_key_env_js = json_optional_string_literal(config.model_api_key_env);
    let model_api_key_value_js = json_optional_string_literal(config.model_api_key_value);
    let secret_scopes_js = agent_secret_scopes_js(config.secret_scopes);
    let approval_required_tools_js = json_string_array_literal(config.approval_required_tools);
    let approved_tools_js = json_string_array_literal(config.approved_tools);
    let approval_log_js = json_optional_string_literal(config.approval_log);
    let approval_max_age_ms_js = json_optional_u64_literal(config.approval_max_age_ms);
    let process_commands_js = agent_process_commands_js(config.process_commands);
    let process_cwds_js = agent_process_cwds_js(config.process_cwds);
    let process_env_js = agent_process_env_js(config.process_env);
    let named_network_cache_dir_js = json_optional_string_literal(config.named_network_cache_dir);
    let named_network_cache_mode_js = json_string_literal(config.named_network_cache_mode);
    let named_network_cache_max_age_ms_js =
        json_optional_u64_literal(config.named_network_cache_max_age_ms);
    let named_network_cache_max_entries_js =
        json_optional_u64_literal(config.named_network_cache_max_entries);
    let expected_event_schemas_js = agent_expected_event_schemas_js(config.expected_event_schemas);
    let worker_source = r#"
function(e) {
  const cfg = e.data;
  const hostCall = globalThis.__cruft_hostCall;
  const context = cfg.context;
  let stateStore = cfg.state;
  const allowedTools = cfg.tools || [];
  const admittedModules = cfg.modules || [];
  const admittedHooks = cfg.importHooks || [];
  const fsReadCaps = cfg.fsReadCaps || [];
  const fsWriteRoots = cfg.fsWriteRoots || [];
  const osvFixture = cfg.osvFixture || null;
  const modelFixture = cfg.modelFixture || null;
  const secretScopes = cfg.secretScopes || [];
  const processCommands = cfg.processCommands || [];
  const processCwds = cfg.processCwds || [];
  const processEnv = cfg.processEnv || [];
  const approvalRequiredTools = cfg.approvalRequiredTools || [];
  const expectedEventSchemas = cfg.expectedEventSchemas || [];
  const redactFields = cfg.redactFields || [];
  const moduleCache = {};
  let emittedEvents = 0;
  let emittedEventBytes = 0;
  let open = true;
  const records = [];
  function hasTool(name) {
    for (let i = 0; i < allowedTools.length; i++) {
      if (allowedTools[i] === name) return true;
    }
    return false;
  }
  function findModule(specifier) {
    for (let i = 0; i < admittedModules.length; i++) {
      if (admittedModules[i].specifier === specifier) return admittedModules[i];
    }
    return null;
  }
  function findHook(specifier) {
    for (let i = 0; i < admittedHooks.length; i++) {
      if (admittedHooks[i].specifier === specifier) return admittedHooks[i];
    }
    return null;
  }
  function findFsReadFile(requested) {
    requested = String(requested);
    for (let i = 0; i < fsReadCaps.length; i++) {
      const f = fsReadCaps[i];
      if (requested === f.path || requested === f.relative || requested === "./" + f.relative) return f;
    }
    return null;
  }
  function artifactHash(content) {
    let h = 0xcbf29ce484222325n;
    const s = String(content);
    for (let i = 0; i < s.length; i++) {
      h ^= BigInt(s.charCodeAt(i) & 0xff);
      h = BigInt.asUintN(64, h * 0x100000001b3n);
    }
    return "fnv1a64:" + h.toString(16).padStart(16, "0");
  }
  function normalizeArtifactPath(requested) {
    requested = String(requested);
    if (requested.length === 0 || requested.charAt(0) === "/" || requested.indexOf("\\") >= 0) return null;
    const parts = requested.split("/");
    const out = [];
    for (let i = 0; i < parts.length; i++) {
      const part = parts[i];
      if (part.length === 0 || part === ".") continue;
      if (part === "..") return null;
      out.push(part);
    }
    if (out.length === 0) return null;
    return out.join("/");
  }
  function validateOsvQueryArgs(arg) {
    const args = cloneJsonObject(arg, "osv.query args");
    if (args.package === null || typeof args.package !== "object" || Array.isArray(args.package)) {
      throw new TypeError("osv.query args package must be an object");
    }
    if (typeof args.package.ecosystem !== "string" || args.package.ecosystem.length === 0 || typeof args.package.name !== "string" || args.package.name.length === 0) {
      throw new TypeError("osv.query args package.ecosystem and package.name must be non-empty strings");
    }
    if (args.version !== void 0 && (typeof args.version !== "string" || args.version.length === 0)) {
      throw new TypeError("osv.query args version must be a non-empty string when present");
    }
    return {package:{ecosystem:args.package.ecosystem, name:args.package.name}, version:args.version === void 0 ? null : args.version};
  }
  function osvFixtureLookup(args) {
    if (osvFixture === null || !Array.isArray(osvFixture.queries)) return null;
    for (let i = 0; i < osvFixture.queries.length; i++) {
      const q = osvFixture.queries[i];
      if (!q || !q.package) continue;
      const version = q.version === void 0 ? null : q.version;
      if (q.package.ecosystem === args.package.ecosystem && q.package.name === args.package.name && version === args.version) return cloneJsonValue(q.response || {}, "osv.query response");
    }
    return {vulns:[]};
  }
  function validateNpmMetadataArgs(arg) {
    const args = cloneJsonObject(arg, "npm.metadata args");
    if (typeof args.package !== "string" || args.package.length === 0) {
      throw new TypeError("npm.metadata args package must be a non-empty string");
    }
    const name = args.package;
    if (name.length > 214 || name.indexOf("\\") >= 0 || name.indexOf(" ") >= 0 || name.indexOf("..") >= 0) {
      throw new TypeError("npm.metadata args package must be a bounded npm package name");
    }
    if (name.charAt(0) === "@") {
      const parts = name.split("/");
      if (parts.length !== 2 || parts[0].length < 2 || parts[1].length === 0 || parts[1].indexOf("/") >= 0) {
        throw new TypeError("npm.metadata scoped package must be @scope/name");
      }
    } else if (name.indexOf("/") >= 0) {
      throw new TypeError("npm.metadata package must not contain path separators unless scoped");
    }
    return {package:name};
  }
  function validateGithubIssueReadArgs(arg) {
    const args = cloneJsonObject(arg, "github.issue.read args");
    if (typeof args.owner !== "string" || typeof args.repo !== "string") {
      throw new TypeError("github.issue.read args owner and repo must be strings");
    }
    const owner = args.owner;
    const repo = args.repo;
    const number = Number(args.number);
    const nameRe = /^[A-Za-z0-9_.-]+$/;
    if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) {
      throw new TypeError("github.issue.read args owner and repo must be bounded GitHub path components");
    }
    if (!Number.isInteger(number) || number <= 0 || number > 2147483647) {
      throw new TypeError("github.issue.read args number must be a positive integer");
    }
    return {owner, repo, number};
  }
  function validateGithubPrReadArgs(arg) {
    const args = cloneJsonObject(arg, "github.pr.read args");
    if (typeof args.owner !== "string" || typeof args.repo !== "string") {
      throw new TypeError("github.pr.read args owner and repo must be strings");
    }
    const owner = args.owner;
    const repo = args.repo;
    const number = Number(args.number);
    const nameRe = /^[A-Za-z0-9_.-]+$/;
    if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) {
      throw new TypeError("github.pr.read args owner and repo must be bounded GitHub path components");
    }
    if (!Number.isInteger(number) || number <= 0 || number > 2147483647) {
      throw new TypeError("github.pr.read args number must be a positive integer");
    }
    return {owner, repo, number};
  }
  function validateGithubPrFilesListArgs(arg) {
    const args = cloneJsonObject(arg, "github.pr.files.list args");
    if (typeof args.owner !== "string" || typeof args.repo !== "string") {
      throw new TypeError("github.pr.files.list args owner and repo must be strings");
    }
    const owner = args.owner;
    const repo = args.repo;
    const number = Number(args.number);
    const nameRe = /^[A-Za-z0-9_.-]+$/;
    if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) {
      throw new TypeError("github.pr.files.list args owner and repo must be bounded GitHub path components");
    }
    if (!Number.isInteger(number) || number <= 0 || number > 2147483647) {
      throw new TypeError("github.pr.files.list args number must be a positive integer");
    }
    return {owner, repo, number};
  }
  function validateGithubReleaseLatestReadArgs(arg) {
    const args = cloneJsonObject(arg, "github.release.latest.read args");
    if (typeof args.owner !== "string" || typeof args.repo !== "string") {
      throw new TypeError("github.release.latest.read args owner and repo must be strings");
    }
    const owner = args.owner;
    const repo = args.repo;
    const nameRe = /^[A-Za-z0-9_.-]+$/;
    if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) {
      throw new TypeError("github.release.latest.read args owner and repo must be bounded GitHub path components");
    }
    return {owner, repo};
  }
  function validateGithubFileReadArgs(arg) {
    const args = cloneJsonObject(arg, "github.file.read args");
    if (typeof args.owner !== "string" || typeof args.repo !== "string" || typeof args.path !== "string") {
      throw new TypeError("github.file.read args owner, repo, and path must be strings");
    }
    const owner = args.owner;
    const repo = args.repo;
    const path = args.path;
    const ref = args.ref === void 0 ? null : args.ref;
    const nameRe = /^[A-Za-z0-9_.-]+$/;
    if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) {
      throw new TypeError("github.file.read args owner and repo must be bounded GitHub path components");
    }
    if (path.length === 0 || path.length > 1024 || path.charAt(0) === "/" || path.indexOf("\\") >= 0 || path.split("/").some(function(part) { return part.length === 0 || part === "." || part === ".."; })) {
      throw new TypeError("github.file.read args path must be a bounded relative repository path");
    }
    if (ref !== null && (typeof ref !== "string" || ref.length === 0 || ref.length > 200 || ref.indexOf("\\") >= 0 || ref.indexOf("..") >= 0)) {
      throw new TypeError("github.file.read args ref must be a bounded ref string when present");
    }
    const result = {owner, repo, path};
    if (ref !== null) result.ref = ref;
    return result;
  }
  function validateGithubCompareReadArgs(arg) {
    const args = cloneJsonObject(arg, "github.compare.read args");
    if (typeof args.owner !== "string" || typeof args.repo !== "string" || typeof args.base !== "string" || typeof args.head !== "string") {
      throw new TypeError("github.compare.read args owner, repo, base, and head must be strings");
    }
    const owner = args.owner;
    const repo = args.repo;
    const base = args.base;
    const head = args.head;
    const nameRe = /^[A-Za-z0-9_.-]+$/;
    const refRe = /^[A-Za-z0-9_./-]+$/;
    if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) {
      throw new TypeError("github.compare.read args owner and repo must be bounded GitHub path components");
    }
    if (base.length === 0 || base.length > 200 || head.length === 0 || head.length > 200 || base.indexOf("..") >= 0 || head.indexOf("..") >= 0 || base.indexOf("\\") >= 0 || head.indexOf("\\") >= 0 || !refRe.test(base) || !refRe.test(head)) {
      throw new TypeError("github.compare.read args base and head must be bounded GitHub refs");
    }
    return {owner, repo, base, head};
  }
  function validateGithubCommitReadArgs(arg) {
    const args = cloneJsonObject(arg, "github.commit.read args");
    if (typeof args.owner !== "string" || typeof args.repo !== "string" || typeof args.ref !== "string") {
      throw new TypeError("github.commit.read args owner, repo, and ref must be strings");
    }
    const owner = args.owner;
    const repo = args.repo;
    const ref = args.ref;
    const nameRe = /^[A-Za-z0-9_.-]+$/;
    const refRe = /^[A-Za-z0-9_./-]+$/;
    if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) {
      throw new TypeError("github.commit.read args owner and repo must be bounded GitHub path components");
    }
    if (ref.length === 0 || ref.length > 200 || ref.indexOf("..") >= 0 || ref.indexOf("\\") >= 0 || !refRe.test(ref)) {
      throw new TypeError("github.commit.read args ref must be a bounded GitHub ref");
    }
    return {owner, repo, ref};
  }
  function validateGithubWorkflowRunReadArgs(arg) {
    const args = cloneJsonObject(arg, "github.workflow.run.read args");
    if (typeof args.owner !== "string" || typeof args.repo !== "string") {
      throw new TypeError("github.workflow.run.read args owner and repo must be strings");
    }
    const owner = args.owner;
    const repo = args.repo;
    const nameRe = /^[A-Za-z0-9_.-]+$/;
    if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) {
      throw new TypeError("github.workflow.run.read args owner and repo must be bounded GitHub path components");
    }
    let runId;
    if (typeof args.run_id === "number") {
      if (!Number.isInteger(args.run_id) || args.run_id <= 0 || args.run_id > 9007199254740991) {
        throw new TypeError("github.workflow.run.read args run_id must be a positive integer");
      }
      runId = String(args.run_id);
    } else if (typeof args.run_id === "string") {
      if (!/^[0-9]+$/.test(args.run_id) || args.run_id.length === 0 || args.run_id.length > 32 || args.run_id === "0") {
        throw new TypeError("github.workflow.run.read args run_id must be a bounded decimal string");
      }
      runId = args.run_id;
    } else {
      throw new TypeError("github.workflow.run.read args run_id must be a positive integer or decimal string");
    }
    return {owner, repo, run_id:runId};
  }
  function validateGithubWorkflowJobsListArgs(arg) {
    const args = cloneJsonObject(arg, "github.workflow.jobs.list args");
    if (typeof args.owner !== "string" || typeof args.repo !== "string") {
      throw new TypeError("github.workflow.jobs.list args owner and repo must be strings");
    }
    const owner = args.owner;
    const repo = args.repo;
    const nameRe = /^[A-Za-z0-9_.-]+$/;
    if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) {
      throw new TypeError("github.workflow.jobs.list args owner and repo must be bounded GitHub path components");
    }
    let runId;
    if (typeof args.run_id === "number") {
      if (!Number.isInteger(args.run_id) || args.run_id <= 0 || args.run_id > 9007199254740991) {
        throw new TypeError("github.workflow.jobs.list args run_id must be a positive integer");
      }
      runId = String(args.run_id);
    } else if (typeof args.run_id === "string") {
      if (!/^[0-9]+$/.test(args.run_id) || args.run_id.length === 0 || args.run_id.length > 32 || args.run_id === "0") {
        throw new TypeError("github.workflow.jobs.list args run_id must be a bounded decimal string");
      }
      runId = args.run_id;
    } else {
      throw new TypeError("github.workflow.jobs.list args run_id must be a positive integer or decimal string");
    }
    return {owner, repo, run_id:runId};
  }
  function validateGithubCheckRunsListArgs(arg) {
    const args = cloneJsonObject(arg, "github.check.runs.list args");
    if (typeof args.owner !== "string" || typeof args.repo !== "string" || typeof args.ref !== "string") {
      throw new TypeError("github.check.runs.list args owner, repo, and ref must be strings");
    }
    const owner = args.owner;
    const repo = args.repo;
    const ref = args.ref;
    const nameRe = /^[A-Za-z0-9_.-]+$/;
    const refRe = /^[A-Za-z0-9_./-]+$/;
    if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) {
      throw new TypeError("github.check.runs.list args owner and repo must be bounded GitHub path components");
    }
    if (ref.length === 0 || ref.length > 200 || ref.indexOf("..") >= 0 || ref.indexOf("\\") >= 0 || !refRe.test(ref)) {
      throw new TypeError("github.check.runs.list args ref must be a bounded Git ref or sha path component");
    }
    return {owner, repo, ref};
  }
  function validateGithubRepoReadArgs(arg) {
    const args = cloneJsonObject(arg, "github.repo.read args");
    if (typeof args.owner !== "string" || typeof args.repo !== "string") {
      throw new TypeError("github.repo.read args owner and repo must be strings");
    }
    const owner = args.owner;
    const repo = args.repo;
    const nameRe = /^[A-Za-z0-9_.-]+$/;
    if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) {
      throw new TypeError("github.repo.read args owner and repo must be bounded GitHub path components");
    }
    return {owner, repo};
  }
  function validateModelCallArgs(arg) {
    const args = cloneJsonObject(arg, "model.call args");
    if (typeof args.id !== "string" || args.id.length === 0) {
      throw new TypeError("model.call args id must be a non-empty string");
    }
    if (args.model !== void 0 && (typeof args.model !== "string" || args.model.length === 0)) {
      throw new TypeError("model.call args model must be a non-empty string when present");
    }
    return {id:args.id, model:args.model === void 0 ? null : args.model, input:args.input === void 0 ? null : cloneJsonValue(args.input, "model.call input")};
  }
  function modelFixtureLookup(args) {
    if (modelFixture === null || !Array.isArray(modelFixture.responses)) return null;
    for (let i = 0; i < modelFixture.responses.length; i++) {
      const r = modelFixture.responses[i];
      if (!r || r.id !== args.id) continue;
      const requestedModel = args.model === void 0 ? null : args.model;
      const responseModel = r.model === void 0 ? null : r.model;
      if (requestedModel !== null && responseModel !== null && responseModel !== requestedModel) continue;
      return cloneJsonValue(r.response || {}, "model.call response");
    }
    return null;
  }
  function findProcessCommand(name) {
    name = String(name);
    for (let i = 0; i < processCommands.length; i++) {
      if (processCommands[i].name === name) return processCommands[i];
    }
    return null;
  }
  function processCwdPath(requested) {
    if (processCwds.length === 0) throw new Error("agent tool denied: process cwd not configured");
    const candidate = requested === void 0 || requested === null ? processCwds[0].root : String(requested);
    for (let i = 0; i < processCwds.length; i++) {
      const root = String(processCwds[i].root);
      if (candidate === root || candidate.indexOf(root + "/") === 0) return candidate;
    }
    throw new Error("agent tool denied: process cwd not admitted");
  }
  function processEnvObject(overrides) {
    const out = {};
    const allowed = {};
    for (let i = 0; i < processEnv.length; i++) {
      out[processEnv[i].key] = processEnv[i].value;
      allowed[processEnv[i].key] = true;
    }
    if (overrides !== void 0) {
      if (overrides === null || typeof overrides !== "object" || Array.isArray(overrides)) {
        throw new TypeError("process args env must be an object when present");
      }
      const names = Object.getOwnPropertyNames(overrides);
      for (let i = 0; i < names.length; i++) {
        const key = names[i];
        if (!allowed[key]) throw new Error("agent tool denied: process env key not admitted");
        if (typeof overrides[key] !== "string") throw new TypeError("process args env values must be strings");
        out[key] = overrides[key];
      }
    }
    return out;
  }
  function validateProcessArgs(arg) {
    const args = cloneJsonObject(arg, "process args");
    if (typeof args.command !== "string" || args.command.length === 0) {
      throw new TypeError("process args command must be a non-empty string");
    }
    const command = findProcessCommand(args.command);
    if (command === null) throw new Error("agent tool denied: process command not admitted");
    const argv = [];
    if (args.args !== void 0) {
      if (!Array.isArray(args.args)) throw new TypeError("process args args must be an array of strings");
      for (let i = 0; i < args.args.length; i++) {
        if (typeof args.args[i] !== "string") throw new TypeError("process args args must be an array of strings");
        argv.push(args.args[i]);
      }
    }
    let output = "full";
    if (args.output !== void 0) {
      if (args.output === "stream") {
        records.push({type:"unsupported_control", status:"error", control:"worker_hosted", reason:"agent_worker_process_stream_backpressure_required_not_available"});
        throw new Error("agent worker process stream/backpressure required not available");
      }
      if (args.output !== "full" && args.output !== "summary") throw new TypeError("worker process args output must be \"full\" or \"summary\"");
      output = args.output;
    }
    return {command:args.command, path:command.path, args:argv, cwd:processCwdPath(args.cwd), env:processEnvObject(args.env), output};
  }
  function safeMessage(e) {
    return e && e.message ? String(e.message) : String(e);
  }
  function cloneJsonObject(value, label) {
    if (value === null || typeof value !== "object" || Array.isArray(value)) {
      throw new TypeError(label + " must be a JSON object");
    }
    return JSON.parse(JSON.stringify(value));
  }
  function cloneJsonValue(value, label) {
    if (value === void 0 || typeof value === "function" || typeof value === "symbol") {
      throw new TypeError(label + " must be JSON-serializable");
    }
    return JSON.parse(JSON.stringify(value));
  }
  function stateBytesOf(value) {
    return JSON.stringify(value).length;
  }
  function enforceStateBudget(candidate) {
    const bytes = stateBytesOf(candidate);
    if (bytes > cfg.maxStateBytes) {
      records.push({type:"budget_exceeded", budget:"state_bytes", limit:cfg.maxStateBytes, attempted:bytes});
      throw new Error("agent state budget exceeded");
    }
    return bytes;
  }
  function redact(value) {
    if (value === null || typeof value !== "object") return value;
    if (Array.isArray(value)) {
      const arr = [];
      for (let i = 0; i < value.length; i++) arr.push(redact(value[i]));
      return arr;
    }
    const out = {};
    const keys = Object.getOwnPropertyNames(value);
    for (let i = 0; i < keys.length; i++) {
      const key = keys[i];
      if (key === "secret" || key === "token" || key === "password" || key === "api_key" || redactFields.indexOf(key) >= 0) {
        out[key] = "[REDACTED]";
      } else {
        out[key] = redact(value[key]);
      }
    }
    return out;
  }
  function modelAuditDisposition() {
    return {
      transcript_persistence:"metadata_and_redacted_payload_fields",
      prompt_disposition:"redacted_audit_fields_only",
      output_disposition:"redacted_audit_fields_only",
      raw_prompt_persisted:false,
      raw_output_persisted:false
    };
  }
  function enforcePayloadBudget(kind, tool, value, limit) {
    const bytes = JSON.stringify(value).length;
    if (bytes > limit) {
      records.push({type:"budget_exceeded", budget:kind, tool, limit, attempted:bytes});
      throw new Error("agent " + kind + " budget exceeded: " + tool);
    }
    return bytes;
  }
  function schemaTypeOf(value) {
    if (value === null) return "null";
    if (Array.isArray(value)) return "array";
    return typeof value;
  }
  function validateAgainstExpectedEventSchema(kind, event) {
    for (let i = 0; i < expectedEventSchemas.length; i++) {
      const expectation = expectedEventSchemas[i];
      if (expectation.kind !== kind) continue;
      const schema = expectation.schema || {};
      const required = Array.isArray(schema.required) ? schema.required : [];
      for (let r = 0; r < required.length; r++) {
        const field = String(required[r]);
        if (!Object.prototype.hasOwnProperty.call(event, field)) {
          records.push({type:"schema_validation", kind, status:"fail", reason:"missing_required", field});
          throw new Error("agent event schema validation failed: " + kind + " missing " + field);
        }
      }
      const properties = schema.properties && typeof schema.properties === "object" && !Array.isArray(schema.properties) ? schema.properties : {};
      const propertyNames = Object.getOwnPropertyNames(properties);
      for (let p = 0; p < propertyNames.length; p++) {
        const field = propertyNames[p];
        if (!Object.prototype.hasOwnProperty.call(event, field)) continue;
        const expected = String(properties[field]);
        if (expected === "any") continue;
        const actual = schemaTypeOf(event[field]);
        if (actual !== expected) {
          records.push({type:"schema_validation", kind, status:"fail", reason:"type_mismatch", field, expected, actual});
          throw new Error("agent event schema validation failed: " + kind + " " + field);
        }
      }
      if (schema.additional_properties === false) {
        const eventNames = Object.getOwnPropertyNames(event);
        for (let e = 0; e < eventNames.length; e++) {
          const field = eventNames[e];
          if (field === "kind") continue;
          if (!Object.prototype.hasOwnProperty.call(properties, field)) {
            records.push({type:"schema_validation", kind, status:"fail", reason:"additional_property", field});
            throw new Error("agent event schema validation failed: " + kind + " additional " + field);
          }
        }
      }
      records.push({type:"schema_validation", kind, status:"pass"});
    }
  }
  function harden(value, seen) {
    if (value === null || (typeof value !== "object" && typeof value !== "function")) return value;
    if (seen.indexOf(value) >= 0) return value;
    seen.push(value);
    const names = Object.getOwnPropertyNames(value);
    for (let i = 0; i < names.length; i++) harden(value[names[i]], seen);
    return Object.freeze(value);
  }
  harden(context, []);
  function ensureOpen(surface) {
    if (!open) {
      records.push({type:"revocation_denial", surface, reason:"closed"});
      throw new Error("agent compartment closed: " + surface);
    }
  }
  function approvalRequiredFor(name) {
    return approvalRequiredTools.indexOf(name) >= 0 || approvalRequiredTools.indexOf("*") >= 0;
  }
  function requireWorkerToolApproval(name, args, argBytes) {
    if (!approvalRequiredFor(name)) return;
    if (typeof hostCall !== "function") {
      records.push({type:"tool_denial", tool:name, policy:"denied", reason:"worker_approval_host_call_not_available"});
      throw new Error("agent tool denied: worker approval host-call not available");
    }
    const result = hostCall("agent.approval", {tool:name, args, arg_bytes:argBytes});
    if (result && result.ok === false) {
      throw new Error(String(result.error || "agent tool approval required: " + name));
    }
    if (!result || result.ok !== true) {
      throw new Error("agent tool approval host-call returned invalid result: " + name);
    }
  }
  function emit(event) {
    ensureOpen("emit");
    const cloned = cloneJsonObject(event, "agent event");
    const bytes = JSON.stringify(cloned).length;
    if (emittedEvents + 1 > cfg.maxEvents) {
      throw new Error("agent event budget exceeded: events");
    }
    if (emittedEventBytes + bytes > cfg.maxEventBytes) {
      throw new Error("agent event budget exceeded: event_bytes");
    }
    emittedEvents++;
    emittedEventBytes += bytes;
    records.push({
      type: "event",
      event: redact(cloned),
      event_index: emittedEvents,
      event_bytes: bytes,
      event_bytes_used: emittedEventBytes
    });
    if (typeof cloned.kind === "string") validateAgainstExpectedEventSchema(cloned.kind, cloned);
  }
  function auditNote(note) {
    ensureOpen("auditNote");
    const cloned = cloneJsonObject(note, "agent audit note");
    const bytes = JSON.stringify(cloned).length;
    if (bytes > cfg.maxEventBytes) {
      records.push({type:"budget_exceeded", budget:"audit_note_bytes", limit:cfg.maxEventBytes, attempted:bytes});
      throw new Error("agent audit note budget exceeded");
    }
    records.push({type:"audit_note", note:redact(cloned), note_bytes:bytes});
  }
  function auditControls() {
    ensureOpen("auditControls");
    const controls = {
      worker_hosted: "bounded_audit_controls",
      tools: allowedTools.slice(),
      event_budget: { max_events: cfg.maxEvents, max_event_bytes: cfg.maxEventBytes },
      tool_payload_budget: { max_tool_arg_bytes: cfg.maxToolArgBytes, max_tool_result_bytes: cfg.maxToolResultBytes },
      module_policy: {
        mode: "worker_explicit_source_modules",
        admitted_count: admittedModules.length,
        import_hook_count: admittedHooks.length
      },
      state_controls: { mode: "worker_single_turn_snapshot", max_state_bytes: cfg.maxStateBytes },
      availability_controls: {
        sync_timeout: "enforced_by_child_process_wall_supervisor",
        pending_promise_disposition: "detached_at_worker_turn_end",
        async_tool_timeout: "enforced_for_worker_promise_tools",
        max_steps: cfg.maxSteps
      }
    };
    const cloned = cloneJsonValue(controls, "audit controls");
    records.push({type:"audit_controls", controls:cloneJsonValue(cloned, "audit controls record")});
    return cloned;
  }
  function callTool(name, arg) {
    ensureOpen("callTool");
    name = String(name);
    if (approvalRequiredFor(name)) {
      let approvalArgs;
      try {
        approvalArgs = arg === void 0 ? {} : cloneJsonValue(arg, name + " approval args");
      } catch (e) {
        records.push({type:"tool_invalid_args", tool:name, policy:"denied", phase:"approval", message:safeMessage(e)});
        throw e;
      }
      const approvalArgBytes = enforcePayloadBudget("tool_arg_bytes", name, approvalArgs, cfg.maxToolArgBytes);
      requireWorkerToolApproval(name, approvalArgs, approvalArgBytes);
    }
    if (name === "echo" && hasTool("echo")) {
      let args;
      try {
        args = cloneJsonObject(arg, "echo args");
      } catch (e) {
        records.push({type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)});
        throw e;
      }
      const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, cfg.maxToolArgBytes);
      records.push({type:"tool_call", tool:name, policy:"allowed", args:redact(args), arg_bytes:argBytes});
      const result = cloneJsonObject(args, "echo result");
      const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, cfg.maxToolResultBytes);
      records.push({type:"tool_result", tool:name, result:redact(result), result_bytes:resultBytes});
      return result;
    }
    if (name === "fail" && hasTool("fail")) {
      let args;
      try {
        args = cloneJsonObject(arg, "fail args");
      } catch (e) {
        records.push({type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)});
        throw e;
      }
      const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, cfg.maxToolArgBytes);
      records.push({type:"tool_call", tool:name, policy:"allowed", args:redact(args), arg_bytes:argBytes});
      const err = new Error("agent tool host failure: fail");
      records.push({type:"tool_error", tool:name, policy:"allowed", message:safeMessage(err)});
      throw err;
    }
    if (name === "slow" && hasTool("slow")) {
      let args;
      try {
        args = cloneJsonObject(arg, "slow args");
      } catch (e) {
        records.push({type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)});
        throw e;
      }
      const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, cfg.maxToolArgBytes);
      const requested = args.delay_ms === void 0 ? cfg.toolTimeoutMs * 2 : Number(args.delay_ms);
      if (!Number.isFinite(requested) || requested < 0) {
        const err = new TypeError("slow args delay_ms must be a non-negative number");
        records.push({type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(err)});
        throw err;
      }
      const delay = Math.floor(requested);
      records.push({type:"tool_call", tool:name, policy:"allowed", args:redact(args), arg_bytes:argBytes});
      return Promise.resolve().then(function() {
        if (delay > cfg.toolTimeoutMs) {
          records.push({type:"tool_timeout", tool:name, policy:"allowed", timeout_ms:cfg.toolTimeoutMs, duration_ms:cfg.toolTimeoutMs});
          throw new Error("agent tool timeout: " + name + " after " + cfg.toolTimeoutMs + "ms");
        }
        const result = {ok:true, tool:"slow", delay_ms:delay, value:args.value === void 0 ? null : args.value};
        const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, cfg.maxToolResultBytes);
        records.push({type:"tool_result", tool:name, result:redact(result), result_bytes:resultBytes, duration_ms:delay});
        return result;
      });
    }
    if (name === "readFile" && fsReadCaps.length > 0) {
      let args;
      try {
        args = cloneJsonObject(arg, "readFile args");
      } catch (e) {
        records.push({type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)});
        throw e;
      }
      if (typeof args.path !== "string" || args.path.length === 0) {
        const err = new TypeError("readFile args path must be a non-empty string");
        records.push({type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(err)});
        throw err;
      }
      const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, cfg.maxToolArgBytes);
      const file = findFsReadFile(args.path);
      if (file === null) {
        records.push({type:"tool_denial", tool:name, policy:"denied", reason:"fs_read_path_not_admitted", path:args.path});
        throw new Error("agent tool denied: readFile path not admitted");
      }
      if (file.readable !== true) {
        records.push({type:"tool_denial", tool:name, policy:"denied", reason:file.reason || "fs_read_entry_not_readable", path:args.path, kind:file.kind || "unknown"});
        throw new Error("agent tool denied: readFile entry not readable");
      }
      records.push({type:"tool_call", tool:name, policy:"allowed", args:redact(args), arg_bytes:argBytes, path:file.path, bytes:file.bytes, source_hash:file.source_hash});
      const result = {path:file.relative, content:file.content, bytes:file.bytes, source_hash:file.source_hash};
      const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, cfg.maxToolResultBytes);
      records.push({type:"tool_result", tool:name, result:redact(result), result_bytes:resultBytes});
      return result;
    }
    if (name === "listFiles" && fsReadCaps.length > 0) {
      let args;
      try {
        args = arg === void 0 ? {} : cloneJsonObject(arg, "listFiles args");
      } catch (e) {
        records.push({type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)});
        throw e;
      }
      const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, cfg.maxToolArgBytes);
      const files = [];
      for (let i = 0; i < fsReadCaps.length; i++) files.push({path:fsReadCaps[i].relative, bytes:fsReadCaps[i].bytes, kind:fsReadCaps[i].kind, readable:fsReadCaps[i].readable, reason:fsReadCaps[i].reason, source_hash:fsReadCaps[i].source_hash || null});
      records.push({type:"tool_call", tool:name, policy:"allowed", args:redact(args), arg_bytes:argBytes, files:files.length});
      const result = {files};
      const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, cfg.maxToolResultBytes);
      records.push({type:"tool_result", tool:name, result:redact(result), result_bytes:resultBytes});
      return result;
    }
    if (name === "writeArtifact" && fsWriteRoots.length > 0) {
      let args;
      try {
        args = cloneJsonObject(arg, "writeArtifact args");
      } catch (e) {
        records.push({type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)});
        throw e;
      }
      if (typeof args.path !== "string" || typeof args.content !== "string") {
        const err = new TypeError("writeArtifact args path and content must be strings");
        records.push({type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(err)});
        throw err;
      }
      const relative = normalizeArtifactPath(args.path);
      if (relative === null) {
        records.push({type:"tool_denial", tool:name, policy:"denied", reason:"artifact_path_not_admitted", path:args.path});
        throw new Error("agent tool denied: writeArtifact path not admitted");
      }
      const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, cfg.maxToolArgBytes);
      const bytes = args.content.length;
      if (bytes > cfg.maxToolResultBytes) {
        records.push({type:"budget_exceeded", budget:"artifact_bytes", tool:name, limit:cfg.maxToolResultBytes, attempted:bytes});
        throw new Error("agent artifact byte budget exceeded");
      }
      const result = {path:relative, bytes, hash:artifactHash(args.content)};
      records.push({type:"tool_call", tool:name, policy:"allowed", args:redact({path:relative, content:"[content omitted]"}), arg_bytes:argBytes, path:relative, bytes});
      records.push({type:"artifact_write_request", tool:name, root:fsWriteRoots[0].root, path:relative, content:args.content, bytes, hash:result.hash});
      return result;
    }
    if (name === "osv.query" && hasTool("osv.query")) {
      let args;
      try {
        args = validateOsvQueryArgs(arg);
      } catch (e) {
        records.push({type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)});
        throw e;
      }
      const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, cfg.maxToolArgBytes);
      if (osvFixture === null) {
        const endpoint = "https://api.osv.dev/v1/query";
        records.push({type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, worker_host_call:true});
        if (typeof hostCall !== "function") {
          records.push({type:"tool_denial", tool:name, policy:"denied", reason:"worker_host_call_not_available"});
          throw new Error("agent tool denied: worker host-call not available");
        }
        const result = hostCall("osv.query", args);
        if (result && result.ok === false && result.tool === "osv.query") {
          records.push({type:"tool_error", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", message:String(result.error || "worker osv.query host-call failed"), worker_host_call:true});
          throw new Error(String(result.error || "worker osv.query host-call failed"));
        }
        const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, cfg.maxToolResultBytes);
        records.push({type:"tool_result", tool:name, endpoint, transport:"persistent_cache", result:redact(result), result_bytes:resultBytes, response_bytes:0, status_code:200, freshness:"cached_persistent", cache_hit:true, cache_scope:"persistent", worker_host_call:true});
        return result;
      }
      records.push({type:"tool_call", tool:name, policy:"allowed", endpoint:"fixture://osv/v1/query", args:redact(args), arg_bytes:argBytes});
      const result = osvFixtureLookup(args);
      const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, cfg.maxToolResultBytes);
      records.push({type:"tool_result", tool:name, endpoint:"fixture://osv/v1/query", result:redact(result), result_bytes:resultBytes});
      return result;
    }
    if (name === "npm.metadata" && hasTool("npm.metadata")) {
      let args;
      try {
        args = validateNpmMetadataArgs(arg);
      } catch (e) {
        records.push({type:"tool_invalid_args", tool:name, policy:"denied", reason:"invalid_args", message:safeMessage(e)});
        throw e;
      }
      const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, cfg.maxToolArgBytes);
      const endpoint = "https://registry.npmjs.org/" + (args.package.charAt(0) === "@" ? encodeURIComponent(args.package.slice(0, args.package.indexOf("/"))) + "%2f" + encodeURIComponent(args.package.slice(args.package.indexOf("/") + 1)) : encodeURIComponent(args.package));
      records.push({type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, worker_host_call:true});
      if (typeof hostCall !== "function") {
        records.push({type:"tool_denial", tool:name, policy:"denied", reason:"worker_host_call_not_available"});
        throw new Error("agent tool denied: worker host-call not available");
      }
      const result = hostCall("npm.metadata", args);
      if (result && result.ok === false && result.tool === "npm.metadata") {
        records.push({type:"tool_error", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", message:String(result.error || "worker npm.metadata host-call failed"), worker_host_call:true});
        throw new Error(String(result.error || "worker npm.metadata host-call failed"));
      }
      const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, cfg.maxToolResultBytes);
      records.push({type:"tool_result", tool:name, endpoint, transport:"persistent_cache", result:redact(result), result_bytes:resultBytes, response_bytes:0, status_code:200, freshness:"cached_persistent", cache_hit:true, cache_scope:"persistent", worker_host_call:true});
      return result;
    }
    if (name === "github.issue.read" && hasTool("github.issue.read")) {
      let args;
      try {
        args = validateGithubIssueReadArgs(arg);
      } catch (e) {
        records.push({type:"tool_invalid_args", tool:name, policy:"denied", reason:"invalid_args", message:safeMessage(e)});
        throw e;
      }
      const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, cfg.maxToolArgBytes);
      const endpoint = "https://api.github.com/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/issues/" + String(args.number);
      records.push({type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, credential_mode:"none", worker_host_call:true});
      if (typeof hostCall !== "function") {
        records.push({type:"tool_denial", tool:name, policy:"denied", reason:"worker_host_call_not_available"});
        throw new Error("agent tool denied: worker host-call not available");
      }
      const result = hostCall("github.issue.read", args);
      if (result && result.ok === false && result.tool === "github.issue.read") {
        records.push({type:"tool_error", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", message:String(result.error || "worker github.issue.read host-call failed"), worker_host_call:true});
        throw new Error(String(result.error || "worker github.issue.read host-call failed"));
      }
      const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, cfg.maxToolResultBytes);
      records.push({type:"tool_result", tool:name, endpoint, transport:"persistent_cache", result:redact(result), result_bytes:resultBytes, response_bytes:0, status_code:200, freshness:"cached_persistent", cache_hit:true, cache_scope:"persistent", credential_mode:"none", worker_host_call:true});
      return result;
    }
    if (name === "github.pr.read" && hasTool("github.pr.read")) {
      let args;
      try {
        args = validateGithubPrReadArgs(arg);
      } catch (e) {
        records.push({type:"tool_invalid_args", tool:name, policy:"denied", reason:"invalid_args", message:safeMessage(e)});
        throw e;
      }
      const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, cfg.maxToolArgBytes);
      const endpoint = "https://api.github.com/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/pulls/" + String(args.number);
      records.push({type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, credential_mode:"none", worker_host_call:true});
      if (typeof hostCall !== "function") {
        records.push({type:"tool_denial", tool:name, policy:"denied", reason:"worker_host_call_not_available"});
        throw new Error("agent tool denied: worker host-call not available");
      }
      const result = hostCall("github.pr.read", args);
      if (result && result.ok === false && result.tool === "github.pr.read") {
        records.push({type:"tool_error", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", message:String(result.error || "worker github.pr.read host-call failed"), worker_host_call:true});
        throw new Error(String(result.error || "worker github.pr.read host-call failed"));
      }
      const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, cfg.maxToolResultBytes);
      records.push({type:"tool_result", tool:name, endpoint, transport:"persistent_cache", result:redact(result), result_bytes:resultBytes, response_bytes:0, status_code:200, freshness:"cached_persistent", cache_hit:true, cache_scope:"persistent", credential_mode:"none", worker_host_call:true});
      return result;
    }
    if (name === "github.pr.files.list" && hasTool("github.pr.files.list")) {
      let args;
      try {
        args = validateGithubPrFilesListArgs(arg);
      } catch (e) {
        records.push({type:"tool_invalid_args", tool:name, policy:"denied", reason:"invalid_args", message:safeMessage(e)});
        throw e;
      }
      const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, cfg.maxToolArgBytes);
      const endpoint = "https://api.github.com/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/pulls/" + String(args.number) + "/files";
      records.push({type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, credential_mode:"none", worker_host_call:true});
      if (typeof hostCall !== "function") {
        records.push({type:"tool_denial", tool:name, policy:"denied", reason:"worker_host_call_not_available"});
        throw new Error("agent tool denied: worker host-call not available");
      }
      const result = hostCall("github.pr.files.list", args);
      if (result && result.ok === false && result.tool === "github.pr.files.list") {
        records.push({type:"tool_error", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", message:String(result.error || "worker github.pr.files.list host-call failed"), worker_host_call:true});
        throw new Error(String(result.error || "worker github.pr.files.list host-call failed"));
      }
      const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, cfg.maxToolResultBytes);
      records.push({type:"tool_result", tool:name, endpoint, transport:"persistent_cache", result:redact(result), result_bytes:resultBytes, response_bytes:0, status_code:200, freshness:"cached_persistent", cache_hit:true, cache_scope:"persistent", credential_mode:"none", worker_host_call:true});
      return result;
    }
    if (name === "github.release.latest.read" && hasTool("github.release.latest.read")) {
      let args;
      try {
        args = validateGithubReleaseLatestReadArgs(arg);
      } catch (e) {
        records.push({type:"tool_invalid_args", tool:name, policy:"denied", reason:"invalid_args", message:safeMessage(e)});
        throw e;
      }
      const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, cfg.maxToolArgBytes);
      const endpoint = "https://api.github.com/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/releases/latest";
      records.push({type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, credential_mode:"none", worker_host_call:true});
      if (typeof hostCall !== "function") {
        records.push({type:"tool_denial", tool:name, policy:"denied", reason:"worker_host_call_not_available"});
        throw new Error("agent tool denied: worker host-call not available");
      }
      const result = hostCall("github.release.latest.read", args);
      if (result && result.ok === false && result.tool === "github.release.latest.read") {
        records.push({type:"tool_error", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", message:String(result.error || "worker github.release.latest.read host-call failed"), worker_host_call:true});
        throw new Error(String(result.error || "worker github.release.latest.read host-call failed"));
      }
      const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, cfg.maxToolResultBytes);
      records.push({type:"tool_result", tool:name, endpoint, transport:"persistent_cache", result:redact(result), result_bytes:resultBytes, response_bytes:0, status_code:200, freshness:"cached_persistent", cache_hit:true, cache_scope:"persistent", credential_mode:"none", worker_host_call:true});
      return result;
    }
    if (name === "github.file.read" && hasTool("github.file.read")) {
      let args;
      try {
        args = validateGithubFileReadArgs(arg);
      } catch (e) {
        records.push({type:"tool_invalid_args", tool:name, policy:"denied", reason:"invalid_args", message:safeMessage(e)});
        throw e;
      }
      const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, cfg.maxToolArgBytes);
      const encodedPath = args.path.split("/").map(encodeURIComponent).join("/");
      let endpoint = "https://api.github.com/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/contents/" + encodedPath;
      if (typeof args.ref === "string") endpoint += "?ref=" + encodeURIComponent(args.ref);
      records.push({type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, credential_mode:"none", worker_host_call:true});
      if (typeof hostCall !== "function") {
        records.push({type:"tool_denial", tool:name, policy:"denied", reason:"worker_host_call_not_available"});
        throw new Error("agent tool denied: worker host-call not available");
      }
      const result = hostCall("github.file.read", args);
      if (result && result.ok === false && result.tool === "github.file.read") {
        records.push({type:"tool_error", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", message:String(result.error || "worker github.file.read host-call failed"), worker_host_call:true});
        throw new Error(String(result.error || "worker github.file.read host-call failed"));
      }
      const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, cfg.maxToolResultBytes);
      records.push({type:"tool_result", tool:name, endpoint, transport:"persistent_cache", result:redact(result), result_bytes:resultBytes, response_bytes:0, status_code:200, freshness:"cached_persistent", cache_hit:true, cache_scope:"persistent", credential_mode:"none", worker_host_call:true});
      return result;
    }
    if (name === "github.compare.read" && hasTool("github.compare.read")) {
      let args;
      try {
        args = validateGithubCompareReadArgs(arg);
      } catch (e) {
        records.push({type:"tool_invalid_args", tool:name, policy:"denied", reason:"invalid_args", message:safeMessage(e)});
        throw e;
      }
      const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, cfg.maxToolArgBytes);
      const endpoint = "https://api.github.com/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/compare/" + encodeURIComponent(args.base) + "..." + encodeURIComponent(args.head);
      records.push({type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, credential_mode:"none", worker_host_call:true});
      if (typeof hostCall !== "function") {
        records.push({type:"tool_denial", tool:name, policy:"denied", reason:"worker_host_call_not_available"});
        throw new Error("agent tool denied: worker host-call not available");
      }
      const result = hostCall("github.compare.read", args);
      if (result && result.ok === false && result.tool === "github.compare.read") {
        records.push({type:"tool_error", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", message:String(result.error || "worker github.compare.read host-call failed"), worker_host_call:true});
        throw new Error(String(result.error || "worker github.compare.read host-call failed"));
      }
      const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, cfg.maxToolResultBytes);
      records.push({type:"tool_result", tool:name, endpoint, transport:"persistent_cache", result:redact(result), result_bytes:resultBytes, response_bytes:0, status_code:200, freshness:"cached_persistent", cache_hit:true, cache_scope:"persistent", credential_mode:"none", worker_host_call:true});
      return result;
    }
    if (name === "github.commit.read" && hasTool("github.commit.read")) {
      let args;
      try {
        args = validateGithubCommitReadArgs(arg);
      } catch (e) {
        records.push({type:"tool_invalid_args", tool:name, policy:"denied", reason:"invalid_args", message:safeMessage(e)});
        throw e;
      }
      const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, cfg.maxToolArgBytes);
      const endpoint = "https://api.github.com/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/commits/" + encodeURIComponent(args.ref);
      records.push({type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, credential_mode:"none", worker_host_call:true});
      if (typeof hostCall !== "function") {
        records.push({type:"tool_denial", tool:name, policy:"denied", reason:"worker_host_call_not_available"});
        throw new Error("agent tool denied: worker host-call not available");
      }
      const result = hostCall("github.commit.read", args);
      if (result && result.ok === false && result.tool === "github.commit.read") {
        records.push({type:"tool_error", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", message:String(result.error || "worker github.commit.read host-call failed"), worker_host_call:true});
        throw new Error(String(result.error || "worker github.commit.read host-call failed"));
      }
      const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, cfg.maxToolResultBytes);
      records.push({type:"tool_result", tool:name, endpoint, transport:"persistent_cache", result:redact(result), result_bytes:resultBytes, response_bytes:0, status_code:200, freshness:"cached_persistent", cache_hit:true, cache_scope:"persistent", credential_mode:"none", worker_host_call:true});
      return result;
    }
    if (name === "github.workflow.run.read" && hasTool("github.workflow.run.read")) {
      let args;
      try {
        args = validateGithubWorkflowRunReadArgs(arg);
      } catch (e) {
        records.push({type:"tool_invalid_args", tool:name, policy:"denied", reason:"invalid_args", message:safeMessage(e)});
        throw e;
      }
      const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, cfg.maxToolArgBytes);
      const endpoint = "https://api.github.com/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/actions/runs/" + encodeURIComponent(args.run_id);
      records.push({type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, credential_mode:"none", worker_host_call:true});
      if (typeof hostCall !== "function") {
        records.push({type:"tool_denial", tool:name, policy:"denied", reason:"worker_host_call_not_available"});
        throw new Error("agent tool denied: worker host-call not available");
      }
      const result = hostCall("github.workflow.run.read", args);
      if (result && result.ok === false && result.tool === "github.workflow.run.read") {
        records.push({type:"tool_error", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", message:String(result.error || "worker github.workflow.run.read host-call failed"), worker_host_call:true});
        throw new Error(String(result.error || "worker github.workflow.run.read host-call failed"));
      }
      const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, cfg.maxToolResultBytes);
      records.push({type:"tool_result", tool:name, endpoint, transport:"persistent_cache", result:redact(result), result_bytes:resultBytes, response_bytes:0, status_code:200, freshness:"cached_persistent", cache_hit:true, cache_scope:"persistent", credential_mode:"none", worker_host_call:true});
      return result;
    }
    if (name === "github.workflow.jobs.list" && hasTool("github.workflow.jobs.list")) {
      let args;
      try {
        args = validateGithubWorkflowJobsListArgs(arg);
      } catch (e) {
        records.push({type:"tool_invalid_args", tool:name, policy:"denied", reason:"invalid_args", message:safeMessage(e)});
        throw e;
      }
      const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, cfg.maxToolArgBytes);
      const endpoint = "https://api.github.com/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/actions/runs/" + encodeURIComponent(args.run_id) + "/jobs";
      records.push({type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, credential_mode:"none", worker_host_call:true});
      if (typeof hostCall !== "function") {
        records.push({type:"tool_denial", tool:name, policy:"denied", reason:"worker_host_call_not_available"});
        throw new Error("agent tool denied: worker host-call not available");
      }
      const result = hostCall("github.workflow.jobs.list", args);
      if (result && result.ok === false && result.tool === "github.workflow.jobs.list") {
        records.push({type:"tool_error", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", message:String(result.error || "worker github.workflow.jobs.list host-call failed"), worker_host_call:true});
        throw new Error(String(result.error || "worker github.workflow.jobs.list host-call failed"));
      }
      const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, cfg.maxToolResultBytes);
      records.push({type:"tool_result", tool:name, endpoint, transport:"persistent_cache", result:redact(result), result_bytes:resultBytes, response_bytes:0, status_code:200, freshness:"cached_persistent", cache_hit:true, cache_scope:"persistent", credential_mode:"none", worker_host_call:true});
      return result;
    }
    if (name === "github.check.runs.list" && hasTool("github.check.runs.list")) {
      let args;
      try {
        args = validateGithubCheckRunsListArgs(arg);
      } catch (e) {
        records.push({type:"tool_invalid_args", tool:name, policy:"denied", reason:"invalid_args", message:safeMessage(e)});
        throw e;
      }
      const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, cfg.maxToolArgBytes);
      const endpoint = "https://api.github.com/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/commits/" + encodeURIComponent(args.ref) + "/check-runs";
      records.push({type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, credential_mode:"none", worker_host_call:true});
      if (typeof hostCall !== "function") {
        records.push({type:"tool_denial", tool:name, policy:"denied", reason:"worker_host_call_not_available"});
        throw new Error("agent tool denied: worker host-call not available");
      }
      const result = hostCall("github.check.runs.list", args);
      if (result && result.ok === false && result.tool === "github.check.runs.list") {
        records.push({type:"tool_error", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", message:String(result.error || "worker github.check.runs.list host-call failed"), worker_host_call:true});
        throw new Error(String(result.error || "worker github.check.runs.list host-call failed"));
      }
      const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, cfg.maxToolResultBytes);
      records.push({type:"tool_result", tool:name, endpoint, transport:"persistent_cache", result:redact(result), result_bytes:resultBytes, response_bytes:0, status_code:200, freshness:"cached_persistent", cache_hit:true, cache_scope:"persistent", credential_mode:"none", worker_host_call:true});
      return result;
    }
    if (name === "github.repo.read" && hasTool("github.repo.read")) {
      let args;
      try {
        args = validateGithubRepoReadArgs(arg);
      } catch (e) {
        records.push({type:"tool_invalid_args", tool:name, policy:"denied", reason:"invalid_args", message:safeMessage(e)});
        throw e;
      }
      const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, cfg.maxToolArgBytes);
      const endpoint = "https://api.github.com/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo);
      records.push({type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, credential_mode:"none", worker_host_call:true});
      if (typeof hostCall !== "function") {
        records.push({type:"tool_denial", tool:name, policy:"denied", reason:"worker_host_call_not_available"});
        throw new Error("agent tool denied: worker host-call not available");
      }
      const result = hostCall("github.repo.read", args);
      if (result && result.ok === false && result.tool === "github.repo.read") {
        records.push({type:"tool_error", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", message:String(result.error || "worker github.repo.read host-call failed"), worker_host_call:true});
        throw new Error(String(result.error || "worker github.repo.read host-call failed"));
      }
      const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, cfg.maxToolResultBytes);
      records.push({type:"tool_result", tool:name, endpoint, transport:"persistent_cache", result:redact(result), result_bytes:resultBytes, response_bytes:0, status_code:200, freshness:"cached_persistent", cache_hit:true, cache_scope:"persistent", credential_mode:"none", worker_host_call:true});
      return result;
    }
    if (name === "model.call" && hasTool("model.call")) {
      let args;
      try {
        args = validateModelCallArgs(arg);
      } catch (e) {
        records.push({type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)});
        throw e;
      }
      if (modelFixture === null) {
        const endpoint = "https://api.openai.com/v1/responses";
        const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, cfg.maxToolArgBytes);
        records.push(Object.assign({type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"host_call_provider_test", args:redact(args), arg_bytes:argBytes, timeout_ms:cfg.toolTimeoutMs, provider:"openai.responses", credential_mode:"host_env_bearer", worker_host_call:true}, modelAuditDisposition()));
        if (typeof hostCall !== "function") {
          records.push({type:"tool_denial", tool:name, policy:"denied", reason:"worker_host_call_not_available"});
          throw new Error("agent tool denied: worker host-call not available");
        }
        const result = hostCall("model.call", args);
        if (result && result.ok === false && result.tool === "model.call") {
          records.push(Object.assign({type:"tool_error", tool:name, endpoint, transport:"host_call_provider_test", provider:"openai.responses", message:String(result.error || "worker model.call host-call failed"), credential_mode:"host_env_bearer", error_kind:result.error_kind || "handler_error", retryable:result.retryable === true, provider_error_policy:"classified_no_retry", worker_host_call:true}, modelAuditDisposition()));
          throw new Error(String(result.error || "worker model.call host-call failed"));
        }
        const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, cfg.maxToolResultBytes);
        records.push(Object.assign({type:"tool_result", tool:name, endpoint, transport:"test_provider", provider:"openai.responses", result:redact(result), result_bytes:resultBytes, response_bytes:JSON.stringify(result).length, status_code:200, freshness:"test_provider", credential_mode:"host_env_bearer", worker_host_call:true}, modelAuditDisposition()));
        return result;
      }
      const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, cfg.maxToolArgBytes);
      records.push(Object.assign({type:"tool_call", tool:name, policy:"allowed", endpoint:"fixture://model/call", transport:"fixture", args:redact(args), arg_bytes:argBytes, timeout_ms:cfg.toolTimeoutMs}, modelAuditDisposition()));
      const result = modelFixtureLookup(args);
      if (result === null) {
        const message = "agent tool denied: model.call fixture response not found";
        records.push({type:"tool_denial", tool:name, policy:"denied", endpoint:"fixture://model/call", transport:"fixture", message, reason:"model_fixture_response_not_found"});
        throw new Error(message);
      }
      const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, cfg.maxToolResultBytes);
      records.push(Object.assign({type:"tool_result", tool:name, endpoint:"fixture://model/call", transport:"fixture", result:redact(result), result_bytes:resultBytes, freshness:"fixture"}, modelAuditDisposition()));
      return result;
    }
    if (name === "process" && hasTool("process")) {
      let args;
      try {
        args = validateProcessArgs(arg);
      } catch (e) {
        const message = safeMessage(e);
        if (message.indexOf("process command not admitted") >= 0) {
          records.push({type:"tool_denial", tool:name, policy:"denied", reason:"process_command_not_admitted", message});
        } else if (message.indexOf("process cwd not admitted") >= 0 || message.indexOf("process cwd not configured") >= 0) {
          records.push({type:"tool_denial", tool:name, policy:"denied", reason:"process_cwd_not_admitted", message});
        } else if (message.indexOf("process env key not admitted") >= 0) {
          records.push({type:"tool_denial", tool:name, policy:"denied", reason:"process_env_key_not_admitted", message});
        } else {
          records.push({type:"tool_invalid_args", tool:name, policy:"denied", reason:"invalid_args", message});
        }
        throw e;
      }
      const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, cfg.maxToolArgBytes);
      records.push({type:"tool_call", tool:name, policy:"allowed", args:redact({command:args.command, args:args.args, cwd:args.cwd, env:args.env, output:args.output}), arg_bytes:argBytes, worker_host_call:true});
      if (typeof hostCall !== "function") {
        records.push({type:"tool_denial", tool:name, policy:"denied", reason:"worker_host_call_not_available"});
        throw new Error("agent tool denied: worker host-call not available");
      }
      const result = hostCall("process", args);
      if (result && result.ok === false && result.tool === "process") {
        records.push({type:"tool_error", tool:name, policy:"allowed", message:String(result.error || "worker process host-call failed"), worker_host_call:true});
        throw new Error(String(result.error || "worker process host-call failed"));
      }
      const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, cfg.maxToolResultBytes);
      records.push({type:"tool_result", tool:name, result:redact(result), result_bytes:resultBytes, worker_host_call:true});
      return result;
    }
    records.push({type:"tool_denial", tool:name, policy:"denied"});
    throw new Error("agent tool denied: " + name);
  }
  function exportConstValue(source, binding) {
    const needle = "export const " + binding + " =";
    const start = source.indexOf(needle);
    if (start < 0) throw new Error("agent module binding not found: " + binding);
    let expr = source.slice(start + needle.length);
    const semi = expr.indexOf(";");
    if (semi >= 0) expr = expr.slice(0, semi);
    return Function("\"use strict\"; return (" + expr + ");")();
  }
  function importValue(specifier, binding) {
    ensureOpen("importValue");
    specifier = String(specifier);
    binding = String(binding);
    const mod = findModule(specifier);
    let sourceRecord = mod;
    if (sourceRecord === null) {
      const hook = findHook(specifier);
      if (hook !== null) {
        records.push({type:"import_hook_load", specifier, policy:"allowed"});
        sourceRecord = hook;
      }
    }
    if (sourceRecord === null) {
      records.push({type:"module_denial", specifier, binding, policy:"denied", reason:"specifier_not_admitted"});
      throw new Error("agent module denied: " + specifier);
    }
    records.push({type:"module_import", specifier, binding, policy:"allowed"});
    return { then: function(ok, fail) {
      try {
        const key = specifier + "\u0000" + binding;
        if (!Object.prototype.hasOwnProperty.call(moduleCache, key)) {
          moduleCache[key] = cloneJsonValue(exportConstValue(sourceRecord.source, binding), "module export");
        }
        records.push({type:"module_result", specifier, binding, source_hash:sourceRecord.source_hash});
        if (ok) return ok(cloneJsonValue(moduleCache[key], "module export"));
        return cloneJsonValue(moduleCache[key], "module export");
      } catch (e) {
        records.push({type:"module_error", specifier, binding, message:safeMessage(e)});
        if (fail) return fail(e);
        throw e;
      }
    } };
  }
  function executableModuleSource(source) {
    return String(source).replace(/export\s+const\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*=/g, "const $1 =");
  }
  function evaluateTenantSource(source) {
    const agentFn = Function(
      "context",
      "emit",
      "callTool",
      "state",
      "close",
      "agentSurfaces",
      "\"use strict\";\nconst importValue = agentSurfaces.importValue;\nconst auditNote = agentSurfaces.auditNote;\nconst auditControls = agentSurfaces.auditControls;\n" + source
    );
    return agentFn(context, emit, callTool, state, close, Object.freeze({importValue, auditNote, auditControls}));
  }
  const state = Object.freeze({
    get(key) {
      ensureOpen("state.get");
      key = String(key);
      if (!Object.prototype.hasOwnProperty.call(stateStore, key)) return void 0;
      records.push({type:"state_get", key});
      return cloneJsonValue(stateStore[key], "state value");
    },
    set(key, value) {
      ensureOpen("state.set");
      key = String(key);
      const cloned = cloneJsonValue(value, "state value");
      const candidate = cloneJsonValue(stateStore, "state store");
      candidate[key] = cloned;
      const bytes = enforceStateBudget(candidate);
      stateStore = candidate;
      records.push({type:"state_set", key, state_bytes:bytes});
    },
    delete(key) {
      ensureOpen("state.delete");
      key = String(key);
      const existed = Object.prototype.hasOwnProperty.call(stateStore, key);
      delete stateStore[key];
      records.push({type:"state_delete", key, existed});
      return existed;
    },
    list() {
      ensureOpen("state.list");
      const keys = Object.getOwnPropertyNames(stateStore);
      records.push({type:"state_list", count:keys.length});
      return keys;
    },
    reset() {
      ensureOpen("state.reset");
      const previousBytes = stateBytesOf(stateStore);
      stateStore = {};
      records.push({type:"state_reset", previous_state_bytes:previousBytes, state_bytes:stateBytesOf(stateStore)});
    }
  });
  function close() {
    if (!open) return;
    open = false;
    records.push({type:"revocation", reason:"closed"});
  }
  function classifyError(message) {
    if (message.indexOf("step budget") >= 0) return "step_budget_exceeded";
    if (message.indexOf("agent tool timeout:") >= 0) return "tool_timeout";
    if (message.indexOf("timeout") >= 0) return "timeout";
    if (message.indexOf("budget exceeded") >= 0) return "budget_exceeded";
    if (message.indexOf("agent event must be a JSON object") >= 0) return "invalid_event";
    if (message.indexOf("agent tool host failure") >= 0) return "tool_error";
    if (message.indexOf("agent tool denied") >= 0) return "tool_denial";
    if (message.indexOf("agent event schema validation failed:") >= 0) return "schema_validation_failed";
    return "exception";
  }
  try {
    let ret;
    if (cfg.entryModule === null) {
      ret = evaluateTenantSource(cfg.agentSource);
    } else {
      const entry = findModule(cfg.entryModule);
      if (entry === null) {
        records.push({type:"module_entry_denial", specifier:cfg.entryModule, policy:"denied", reason:"specifier_not_admitted"});
        throw new Error("agent module denied: " + cfg.entryModule);
      }
      records.push({type:"module_entry", specifier:cfg.entryModule, policy:"allowed"});
      ret = evaluateTenantSource(executableModuleSource(entry.source));
      records.push({type:"module_entry_result", specifier:cfg.entryModule, source_hash:entry.source_hash});
    }
    return Promise.resolve(ret).then(function() {
      return Promise.resolve().then(function() {
        const finalState = cloneJsonValue(stateStore, "state store");
        return {
          status: "ok",
          records,
          emitted_events: emittedEvents,
          emitted_event_bytes: emittedEventBytes,
          state: finalState,
          state_bytes: stateBytesOf(finalState)
        };
      });
    }, function(e) {
      const message = safeMessage(e);
      const finalState = cloneJsonValue(stateStore, "state store");
      return {
        status: "error",
        reason: classifyError(message),
        message,
        records,
        emitted_events: emittedEvents,
        emitted_event_bytes: emittedEventBytes,
        state: finalState,
        state_bytes: stateBytesOf(finalState)
      };
    });
  } catch (e) {
    const message = safeMessage(e);
    const finalState = cloneJsonValue(stateStore, "state store");
    return {
      status: "error",
      reason: classifyError(message),
      message,
      records,
      emitted_events: emittedEvents,
      emitted_event_bytes: emittedEventBytes,
      state: finalState,
      state_bytes: stateBytesOf(finalState)
    };
  }
}
"#;
    let harness = format!(
        r#"
const fs = require('node:fs');
const childProcess = require('node:child_process');
const auditPath = {audit};
const runId = {run_id};
const agentPath = {agent_path};
const agentSource = {agent_source};
const context = JSON.parse({context});
const configuredInitialState = JSON.parse({state});
const sessionPath = {session_file};
const entryModule = {entry_module};
const allowedTools = [{tools}];
const admittedModules = [{modules}];
const admittedHooks = [{import_hooks}];
const fsReadCaps = [{fs_read_caps}];
const fsWriteRoots = [{fs_write_roots}];
const osvFixture = {osv_fixture};
const modelFixture = {model_fixture};
const modelProvider = {model_provider};
const modelApiKeyEnv = {model_api_key_env};
const modelApiKeyToken = {model_api_key_value};
const secretScopes = Object.freeze([{secret_scopes}]);
const approvalRequiredTools = Object.freeze({approval_required_tools});
const approvedTools = Object.freeze({approved_tools});
const approvalLogPath = {approval_log};
const approvalMaxAgeMs = {approval_max_age_ms};
const processCommands = [{process_commands}];
const processCwds = [{process_cwds}];
const processEnv = [{process_env}];
const namedNetworkCacheDir = {named_network_cache_dir};
const namedNetworkCacheMode = {named_network_cache_mode};
const namedNetworkCacheMaxAgeMs = {named_network_cache_max_age_ms};
const namedNetworkCacheMaxEntries = {named_network_cache_max_entries};
const expectedEventSchemas = [{expected_event_schemas}];
let initialState = configuredInitialState;
let sessionTurn = 0;
let sessionLoaded = false;
function cloneJsonValue(value, label) {{
  if (value === void 0 || typeof value === "function" || typeof value === "symbol") {{
    throw new TypeError(label + " must be JSON-serializable");
  }}
  return JSON.parse(JSON.stringify(value));
}}
function stateBytesOf(value) {{
  return JSON.stringify(value).length;
}}
if (sessionPath !== null && fs.existsSync(sessionPath)) {{
  const sessionEnvelope = JSON.parse(fs.readFileSync(sessionPath, "utf8"));
  if (sessionEnvelope === null || typeof sessionEnvelope !== "object" || Array.isArray(sessionEnvelope) || sessionEnvelope.state === null || typeof sessionEnvelope.state !== "object" || Array.isArray(sessionEnvelope.state)) {{
    throw new Error("agent session file must contain a JSON object envelope with object state");
  }}
  initialState = cloneJsonValue(sessionEnvelope.state, "session state");
  sessionTurn = Number(sessionEnvelope.turn_id || 0);
  sessionLoaded = true;
}}
const stateRequested = JSON.stringify(initialState) !== "{{}}";
const modulesRequested = admittedModules.length > 0;
const workerHostedMode = allowedTools.length === 0
  ? (stateRequested ? (modulesRequested ? "emit_state_and_module_forwarding" : "emit_and_state_forwarding") : (modulesRequested ? "emit_and_module_forwarding" : "emit_forwarding"))
  : (stateRequested ? (modulesRequested ? "emit_sync_tool_state_and_module_forwarding" : "emit_sync_tool_and_state_forwarding") : (modulesRequested ? "emit_sync_tool_and_module_forwarding" : "emit_and_sync_tool_forwarding"));
const maxEvents = {max_events};
const maxEventBytes = {max_event_bytes};
const maxStateBytes = {max_state_bytes};
const maxToolArgBytes = {max_tool_arg_bytes};
const maxToolResultBytes = {max_tool_result_bytes};
const maxSteps = {max_steps};
const memoryRssControl = {memory_rss};
const maxRssMb = {max_rss_mb};
const redactFields = {redact_fields};
let writtenArtifacts = [];
let runArtifactManifestEmitted = false;
function audit(record) {{
  record.ts_ms = Date.now();
  fs.appendFileSync(auditPath, JSON.stringify(record) + "\n");
}}
function safeMessage(e) {{
  return e && e.message ? String(e.message) : String(e);
}}
function redact(value) {{
  if (value === null || typeof value !== "object") return value;
  if (Array.isArray(value)) {{
    const out = [];
    for (let i = 0; i < value.length; i++) out.push(redact(value[i]));
    return out;
  }}
  const out = {{}};
  const names = Object.getOwnPropertyNames(value);
  for (let i = 0; i < names.length; i++) {{
    const key = names[i];
    out[key] = key === "secret" || key === "token" || key === "password" || key === "api_key" || redactFields.indexOf(key) >= 0 ? "[REDACTED]" : redact(value[key]);
  }}
  return out;
}}
function workerArtifactHash(content) {{
  let h = 0xcbf29ce484222325n;
  const s = String(content);
  for (let i = 0; i < s.length; i++) {{
    h ^= BigInt(s.charCodeAt(i) & 0xff);
    h = BigInt.asUintN(64, h * 0x100000001b3n);
  }}
  return "fnv1a64:" + h.toString(16).padStart(16, "0");
}}
function modelAuditDisposition() {{
  return {{
    transcript_persistence:"metadata_and_redacted_payload_fields",
    prompt_disposition:"redacted_audit_fields_only",
    output_disposition:"redacted_audit_fields_only",
    raw_prompt_persisted:false,
    raw_output_persisted:false
  }};
}}
function approvalRequiredFor(name) {{
  return approvalRequiredTools.indexOf(name) >= 0 || approvalRequiredTools.indexOf("*") >= 0;
}}
function approvalGrantedFor(name) {{
  return approvedTools.indexOf(name) >= 0 || approvedTools.indexOf("*") >= 0;
}}
function approvalRequestId(name, args) {{
  return workerArtifactHash(name + "\n" + JSON.stringify(args));
}}
function appendApprovalRecord(record) {{
  if (approvalLogPath === null) return;
  const parent = require("node:path").dirname(approvalLogPath);
  fs.mkdirSync(parent, {{recursive:true}});
  fs.appendFileSync(approvalLogPath, JSON.stringify(record) + "\n");
}}
function approvalDecisionFor(id) {{
  if (approvalLogPath === null || !fs.existsSync(approvalLogPath)) return null;
  const lines = fs.readFileSync(approvalLogPath, "utf8").split(/\n/);
  let decision = null;
  const now = Date.now();
  for (let i = 0; i < lines.length; i++) {{
    const line = lines[i].trim();
    if (line.length === 0) continue;
    try {{
      const record = JSON.parse(line);
      if (record && record.type === "agent_approval_decision" && record.id === id && (record.status === "allowed" || record.status === "denied")) {{
        decision = {{status:record.status, created_at_ms:Number(record.created_at_ms || 0)}};
      }}
    }} catch (_e) {{}}
  }}
  if (decision !== null && approvalMaxAgeMs !== null) {{
    const ageMs = now - decision.created_at_ms;
    if (!Number.isFinite(ageMs) || decision.created_at_ms <= 0 || ageMs > approvalMaxAgeMs) {{
      return {{status:"stale", created_at_ms:decision.created_at_ms, age_ms:ageMs, max_age_ms:approvalMaxAgeMs}};
    }}
    decision.age_ms = ageMs;
    decision.max_age_ms = approvalMaxAgeMs;
  }}
  return decision;
}}
function requireParentToolApproval(name, args, argBytes) {{
  if (!approvalRequiredFor(name)) return {{ok:true, status:"not_required"}};
  const approvalId = approvalRequestId(name, args);
  if (approvalGrantedFor(name)) {{
    audit({{type:"tool_approval_granted", tool:name, approval_id:approvalId, policy:"approved", approval_mode:"pregranted", args:redact(args), arg_bytes:argBytes, worker_host_call:true}});
    return {{ok:true, status:"granted", approval_id:approvalId, approval_mode:"pregranted"}};
  }}
  const decision = approvalDecisionFor(approvalId);
  if (decision && decision.status === "allowed") {{
    audit({{type:"tool_approval_granted", tool:name, approval_id:approvalId, policy:"approved", approval_mode:"approval_log", approval_log:approvalLogPath, approval_age_ms:decision.age_ms, approval_max_age_ms:decision.max_age_ms, args:redact(args), arg_bytes:argBytes, worker_host_call:true}});
    return {{ok:true, status:"granted", approval_id:approvalId, approval_mode:"approval_log"}};
  }}
  if (decision && decision.status === "denied") {{
    audit({{type:"tool_approval_denied", tool:name, approval_id:approvalId, policy:"denied", approval_mode:"approval_log", approval_log:approvalLogPath, approval_age_ms:decision.age_ms, approval_max_age_ms:decision.max_age_ms, args:redact(args), arg_bytes:argBytes, worker_host_call:true}});
    return {{ok:false, status:"denied", approval_id:approvalId, error:"agent tool approval denied: " + name}};
  }}
  if (decision && decision.status === "stale") {{
    audit({{type:"tool_approval_stale", tool:name, approval_id:approvalId, policy:"denied", approval_mode:"approval_log", approval_log:approvalLogPath, approval_age_ms:decision.age_ms, approval_max_age_ms:decision.max_age_ms, args:redact(args), arg_bytes:argBytes, worker_host_call:true}});
    return {{ok:false, status:"stale", approval_id:approvalId, error:"agent tool approval stale: " + name}};
  }}
  appendApprovalRecord({{type:"agent_approval_pending", id:approvalId, tool:name, args:redact(args), arg_bytes:argBytes, status:"pending"}});
  audit({{type:"tool_approval_pending", tool:name, approval_id:approvalId, policy:"pending", reason:"approval_required", approval_mode:"pre_effect_required", resume:approvalLogPath === null ? "pregrant_only" : "approval_log", approval_log:approvalLogPath, args:redact(args), arg_bytes:argBytes, worker_host_call:true}});
  return {{ok:false, status:"pending", approval_id:approvalId, error:"agent tool approval required: " + name}};
}}
function emitRunArtifactManifest(status, reason) {{
  if (runArtifactManifestEmitted) return;
  runArtifactManifestEmitted = true;
  audit({{
    type:"run_artifact_manifest",
    version:1,
    run_id:runId,
    status,
    reason:reason || null,
    policy:{{mouth:"cruft agent run", audit_log:auditPath, run_id:runId, worker_hosted:workerHostedMode}},
    source:{{agent:{{path:agentPath, source_hash:workerArtifactHash(agentSource)}}, entry_module:entryModule, modules:admittedModules.map(function(m) {{ return {{specifier:m.specifier, source_hash:m.source_hash || null}}; }}), fs_read_source_manifest:fsReadCaps.map(function(f) {{ return {{path:f.relative, bytes:f.bytes, kind:f.kind, readable:f.readable, reason:f.reason, source_hash:f.source_hash || null}}; }})}},
    tools:allowedTools.slice(),
    approvals:{{required_tools:[], approved_tools:[], approval_log_configured:false, worker_forwarding:"not_available"}},
    secret_scopes:secretScopes.map(function(s) {{ return {{tool:s.tool, credential_mode:s.credential_mode, credential_env:s.credential_env}}; }}),
    named_network_cache:{{configured:namedNetworkCacheDir !== null, cache_mode:namedNetworkCacheMode, max_age_ms:namedNetworkCacheMaxAgeMs, max_entries:namedNetworkCacheMaxEntries, worker_forwarding:namedNetworkCacheDir === null ? "not_available" : "persistent_cache_host_call_only_live_async_routed"}},
    model_call:{{configured:allowedTools.indexOf("model.call") >= 0, mode:allowedTools.indexOf("model.call") >= 0 ? (modelFixture === null ? "not_configured" : "fixture_backed_named_model_tool") : "not_configured", endpoint:allowedTools.indexOf("model.call") >= 0 && modelFixture !== null ? "fixture://model/call" : null, disposition:modelAuditDisposition()}},
    artifacts:writtenArtifacts.slice(),
    budgets:{{max_events:maxEvents, max_event_bytes:maxEventBytes, max_tool_arg_bytes:maxToolArgBytes, max_tool_result_bytes:maxToolResultBytes, tool_timeout_ms:{tool_timeout_ms}, max_state_bytes:maxStateBytes, max_steps:maxSteps, memory_rss:memoryRssControl, max_rss_mb:maxRssMb}},
    replay:{{audit_log:auditPath, run_id:runId, bundle_file:"run-artifact-manifest.json"}}
  }});
}}
fs.writeFileSync(auditPath, "");
if (sessionPath !== null) {{
  audit({{type:"session_load", run_id:runId, path:sessionPath, existed:sessionLoaded, turn_id:sessionTurn, state_bytes:stateBytesOf(initialState)}});
}}
audit({{
  type:"run_start",
  run_id:runId,
  mouth:"cruft agent run",
  agent:agentPath,
  worker_hosted:workerHostedMode,
  timeout_ms:{timeout_ms},
  tools:allowedTools,
  secret_scopes:secretScopes.map(function(s) {{ return {{tool:s.tool, credential_mode:s.credential_mode, credential_env:s.credential_env}}; }}),
  fs_read:{{mode:fsReadCaps.length === 0 ? "not_configured" : "path_caps_with_byte_budget", files:fsReadCaps.length}},
  fs_read_source_manifest:fsReadCaps.map(function(f) {{ return {{path:f.relative, bytes:f.bytes, kind:f.kind, readable:f.readable, reason:f.reason, source_hash:f.source_hash || null}}; }}),
  artifact_write:{{mode:fsWriteRoots.length === 0 ? "not_configured" : "path_caps_with_byte_budget", roots:fsWriteRoots.length}},
  osv_query:{{mode:osvFixture === null ? "not_configured" : "fixture_backed_named_lookup", endpoint:"fixture://osv/v1/query"}},
  npm_metadata:{{mode:allowedTools.indexOf("npm.metadata") >= 0 ? "worker_persistent_cache_host_call_only" : "not_configured", endpoint:"https://registry.npmjs.org/<package>", live_forwarding:"not_available"}},
  model_call:{{mode:modelFixture === null ? "not_configured" : "fixture_backed_named_model_tool", endpoint:"fixture://model/call", disposition:modelAuditDisposition()}},
  process_tools:{{mode:allowedTools.indexOf("process") >= 0 ? "argv_policy_supervised_sync" : "not_configured", commands:processCommands.map(function(c) {{ return c.name; }}), cwd_roots:processCwds.length, env_keys:processEnv.map(function(e) {{ return e.key; }})}},
  expected_events:expectedEventSchemas.map(function(e) {{ return e.kind; }}),
  event_budget:{{max_events:maxEvents, max_event_bytes:maxEventBytes}},
  audit_redaction:{{common_fields:["secret","token","password","api_key"], policy_fields:redactFields}},
  tool_payload_budget:{{max_tool_arg_bytes:maxToolArgBytes, max_tool_result_bytes:maxToolResultBytes}},
  resource_controls:{{clone_payload_bytes:"enforced", memory_rss:memoryRssControl, max_rss_mb:maxRssMb}},
  module_policy:{{mode:"worker_explicit_source_modules", entrypoint:entryModule === null ? "script" : "static_module", entry_module:entryModule, admitted:admittedModules.map(function(m) {{ return {{specifier:m.specifier, source_hash:m.source_hash}}; }}), package_imports:{package_imports}, import_hooks:admittedHooks.length === 0 ? "not_available_in_worker_emit_mode" : "source_hash_caps"}},
  state_controls:{{mode:"worker_single_turn_snapshot", max_state_bytes:maxStateBytes, reset:"worker_single_turn_store", resume:sessionPath === null ? "not_available_in_worker_emit_mode" : "worker_file_session", close_revocation:"sync_and_promise_turn"}},
  availability_controls:{{
    sync_timeout:"enforced_by_child_process_wall_supervisor",
    tenant_timeout_catchability:"uncatchable_gate",
    microtask_budget:"enforced",
    max_microtasks:{max_microtasks},
    pending_promise_disposition:"detached_at_worker_turn_end",
    async_tool_timeout:"enforced_for_worker_promise_tools",
    step_budget:maxSteps === null ? "available_with_--max-steps" : "enforced",
    max_steps:maxSteps
  }}
}});
const workerSource = {worker_source};
function findProcessCommand(name) {{
  name = String(name);
  for (let i = 0; i < processCommands.length; i++) {{
    if (processCommands[i].name === name) return processCommands[i];
  }}
  return null;
}}
function processOutputSummary(stream, text, forceTruncated) {{
  text = text === null || text === undefined ? "" : String(text);
  let previewLimit = Math.min(maxEventBytes, Math.floor(maxToolResultBytes / 8), 512);
  if (!Number.isFinite(previewLimit) || previewLimit <= 0) previewLimit = 1;
  const preview = text.slice(0, previewLimit);
  return {{stream, bytes:text.length, captured_bytes:preview.length, truncated:!!forceTruncated || preview.length < text.length, preview}};
}}
function auditProcessOutputStream(command, stream, text, forceTruncated) {{
  text = text === null || text === undefined ? "" : String(text);
  if (text.length === 0) return;
  let limit = maxEventBytes;
  if (!Number.isFinite(limit) || limit <= 0) limit = 1024;
  limit = Math.max(1, Math.min(limit, maxToolResultBytes, 8192));
  const captured = text.slice(0, limit);
  const truncated = !!forceTruncated || captured.length < text.length;
  audit({{type:"process_output_stream", tool:"process", policy:"allowed", command, stream, chunk_index:0, chunk_count:1, chunk_bytes:captured.length, captured_bytes:captured.length, original_bytes:text.length, truncated, text:captured, worker_host_call:true}});
}}
function namedNetworkCacheHash(tool, args) {{
  const payload = tool + "\n" + JSON.stringify(args);
  let h = 0x811c9dc5;
  for (let i = 0; i < payload.length; i++) {{
    h ^= payload.charCodeAt(i) & 0xff;
    h = Math.imul(h, 0x01000193) >>> 0;
  }}
  return h.toString(16).padStart(8, "0");
}}
function namedNetworkPersistentCachePath(tool, args) {{
  if (namedNetworkCacheDir === null) return null;
  const safeTool = String(tool).replace(/[^A-Za-z0-9_.-]/g, "_");
  return namedNetworkCacheDir + "/" + safeTool + "-" + namedNetworkCacheHash(tool, args) + ".json";
}}
function validateWorkerOsvQueryArgs(args) {{
  if (args === null || typeof args !== "object" || Array.isArray(args)) throw new TypeError("osv.query args must be an object");
  if (args.package === null || typeof args.package !== "object" || Array.isArray(args.package)) throw new TypeError("osv.query args package must be an object");
  if (typeof args.package.ecosystem !== "string" || args.package.ecosystem.length === 0 || typeof args.package.name !== "string" || args.package.name.length === 0) throw new TypeError("osv.query args package.ecosystem and package.name must be non-empty strings");
  if (args.version !== null && args.version !== undefined && (typeof args.version !== "string" || args.version.length === 0)) throw new TypeError("osv.query args version must be a non-empty string when present");
  return {{package:{{ecosystem:args.package.ecosystem, name:args.package.name}}, version:args.version === undefined ? null : args.version}};
}}
function validateWorkerNpmMetadataArgs(args) {{
  if (args === null || typeof args !== "object" || Array.isArray(args)) throw new TypeError("npm.metadata args must be an object");
  if (typeof args.package !== "string" || args.package.length === 0) throw new TypeError("npm.metadata args package must be a non-empty string");
  const name = args.package;
  if (name.length > 214 || name.indexOf("\\") >= 0 || name.indexOf(" ") >= 0 || name.indexOf("..") >= 0) throw new TypeError("npm.metadata args package must be a bounded npm package name");
  if (name.charAt(0) === "@") {{
    const parts = name.split("/");
    if (parts.length !== 2 || parts[0].length < 2 || parts[1].length === 0 || parts[1].indexOf("/") >= 0) throw new TypeError("npm.metadata scoped package must be @scope/name");
  }} else if (name.indexOf("/") >= 0) {{
    throw new TypeError("npm.metadata package must not contain path separators unless scoped");
  }}
  return {{package:name}};
}}
function validateWorkerGithubIssueReadArgs(args) {{
  if (args === null || typeof args !== "object" || Array.isArray(args)) throw new TypeError("github.issue.read args must be an object");
  if (typeof args.owner !== "string" || typeof args.repo !== "string") throw new TypeError("github.issue.read args owner and repo must be strings");
  const owner = args.owner;
  const repo = args.repo;
  const number = Number(args.number);
  const nameRe = /^[A-Za-z0-9_.-]+$/;
  if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) throw new TypeError("github.issue.read args owner and repo must be bounded GitHub path components");
  if (!Number.isInteger(number) || number <= 0 || number > 2147483647) throw new TypeError("github.issue.read args number must be a positive integer");
  return {{owner, repo, number}};
}}
function validateWorkerGithubPrReadArgs(args) {{
  if (args === null || typeof args !== "object" || Array.isArray(args)) throw new TypeError("github.pr.read args must be an object");
  if (typeof args.owner !== "string" || typeof args.repo !== "string") throw new TypeError("github.pr.read args owner and repo must be strings");
  const owner = args.owner;
  const repo = args.repo;
  const number = Number(args.number);
  const nameRe = /^[A-Za-z0-9_.-]+$/;
  if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) throw new TypeError("github.pr.read args owner and repo must be bounded GitHub path components");
  if (!Number.isInteger(number) || number <= 0 || number > 2147483647) throw new TypeError("github.pr.read args number must be a positive integer");
  return {{owner, repo, number}};
}}
function validateWorkerGithubPrFilesListArgs(args) {{
  if (args === null || typeof args !== "object" || Array.isArray(args)) throw new TypeError("github.pr.files.list args must be an object");
  if (typeof args.owner !== "string" || typeof args.repo !== "string") throw new TypeError("github.pr.files.list args owner and repo must be strings");
  const owner = args.owner;
  const repo = args.repo;
  const number = Number(args.number);
  const nameRe = /^[A-Za-z0-9_.-]+$/;
  if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) throw new TypeError("github.pr.files.list args owner and repo must be bounded GitHub path components");
  if (!Number.isInteger(number) || number <= 0 || number > 2147483647) throw new TypeError("github.pr.files.list args number must be a positive integer");
  return {{owner, repo, number}};
}}
function validateWorkerGithubReleaseLatestReadArgs(args) {{
  if (args === null || typeof args !== "object" || Array.isArray(args)) throw new TypeError("github.release.latest.read args must be an object");
  if (typeof args.owner !== "string" || typeof args.repo !== "string") throw new TypeError("github.release.latest.read args owner and repo must be strings");
  const owner = args.owner;
  const repo = args.repo;
  const nameRe = /^[A-Za-z0-9_.-]+$/;
  if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) throw new TypeError("github.release.latest.read args owner and repo must be bounded GitHub path components");
  return {{owner, repo}};
}}
function validateWorkerGithubFileReadArgs(args) {{
  if (args === null || typeof args !== "object" || Array.isArray(args)) throw new TypeError("github.file.read args must be an object");
  if (typeof args.owner !== "string" || typeof args.repo !== "string" || typeof args.path !== "string") throw new TypeError("github.file.read args owner, repo, and path must be strings");
  const owner = args.owner;
  const repo = args.repo;
  const path = args.path;
  const ref = args.ref === undefined ? null : args.ref;
  const nameRe = /^[A-Za-z0-9_.-]+$/;
  if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) throw new TypeError("github.file.read args owner and repo must be bounded GitHub path components");
  if (path.length === 0 || path.length > 1024 || path.charAt(0) === "/" || path.indexOf("\\\\") >= 0 || path.split("/").some(function(part) {{ return part.length === 0 || part === "." || part === ".."; }})) throw new TypeError("github.file.read args path must be a bounded relative repository path");
  if (ref !== null && (typeof ref !== "string" || ref.length === 0 || ref.length > 200 || ref.indexOf("\\\\") >= 0 || ref.indexOf("..") >= 0)) throw new TypeError("github.file.read args ref must be a bounded ref string when present");
  const result = {{owner, repo, path}};
  if (ref !== null) result.ref = ref;
  return result;
}}
function validateWorkerGithubCompareReadArgs(args) {{
  if (args === null || typeof args !== "object" || Array.isArray(args)) throw new TypeError("github.compare.read args must be an object");
  if (typeof args.owner !== "string" || typeof args.repo !== "string" || typeof args.base !== "string" || typeof args.head !== "string") throw new TypeError("github.compare.read args owner, repo, base, and head must be strings");
  const owner = args.owner;
  const repo = args.repo;
  const base = args.base;
  const head = args.head;
  const nameRe = /^[A-Za-z0-9_.-]+$/;
  const refRe = /^[A-Za-z0-9_./-]+$/;
  if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) throw new TypeError("github.compare.read args owner and repo must be bounded GitHub path components");
  if (base.length === 0 || base.length > 200 || head.length === 0 || head.length > 200 || base.indexOf("..") >= 0 || head.indexOf("..") >= 0 || base.indexOf("\\\\") >= 0 || head.indexOf("\\\\") >= 0 || !refRe.test(base) || !refRe.test(head)) throw new TypeError("github.compare.read args base and head must be bounded GitHub refs");
  return {{owner, repo, base, head}};
}}
function validateWorkerGithubCommitReadArgs(args) {{
  if (args === null || typeof args !== "object" || Array.isArray(args)) throw new TypeError("github.commit.read args must be an object");
  if (typeof args.owner !== "string" || typeof args.repo !== "string" || typeof args.ref !== "string") throw new TypeError("github.commit.read args owner, repo, and ref must be strings");
  const owner = args.owner;
  const repo = args.repo;
  const ref = args.ref;
  const nameRe = /^[A-Za-z0-9_.-]+$/;
  const refRe = /^[A-Za-z0-9_./-]+$/;
  if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) throw new TypeError("github.commit.read args owner and repo must be bounded GitHub path components");
  if (ref.length === 0 || ref.length > 200 || ref.indexOf("..") >= 0 || ref.indexOf("\\\\") >= 0 || !refRe.test(ref)) throw new TypeError("github.commit.read args ref must be a bounded GitHub ref");
  return {{owner, repo, ref}};
}}
function validateWorkerGithubWorkflowRunReadArgs(args) {{
  if (args === null || typeof args !== "object" || Array.isArray(args)) throw new TypeError("github.workflow.run.read args must be an object");
  if (typeof args.owner !== "string" || typeof args.repo !== "string") throw new TypeError("github.workflow.run.read args owner and repo must be strings");
  const owner = args.owner;
  const repo = args.repo;
  const nameRe = /^[A-Za-z0-9_.-]+$/;
  if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) throw new TypeError("github.workflow.run.read args owner and repo must be bounded GitHub path components");
  let runId;
  if (typeof args.run_id === "number") {{
    if (!Number.isInteger(args.run_id) || args.run_id <= 0 || args.run_id > 9007199254740991) throw new TypeError("github.workflow.run.read args run_id must be a positive integer");
    runId = String(args.run_id);
  }} else if (typeof args.run_id === "string") {{
    if (!/^[0-9]+$/.test(args.run_id) || args.run_id.length === 0 || args.run_id.length > 32 || args.run_id === "0") throw new TypeError("github.workflow.run.read args run_id must be a bounded decimal string");
    runId = args.run_id;
  }} else {{
    throw new TypeError("github.workflow.run.read args run_id must be a positive integer or decimal string");
  }}
  return {{owner, repo, run_id:runId}};
}}
function validateWorkerGithubWorkflowJobsListArgs(args) {{
  if (args === null || typeof args !== "object" || Array.isArray(args)) throw new TypeError("github.workflow.jobs.list args must be an object");
  if (typeof args.owner !== "string" || typeof args.repo !== "string") throw new TypeError("github.workflow.jobs.list args owner and repo must be strings");
  const owner = args.owner;
  const repo = args.repo;
  const nameRe = /^[A-Za-z0-9_.-]+$/;
  if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) throw new TypeError("github.workflow.jobs.list args owner and repo must be bounded GitHub path components");
  let runId;
  if (typeof args.run_id === "number") {{
    if (!Number.isInteger(args.run_id) || args.run_id <= 0 || args.run_id > 9007199254740991) throw new TypeError("github.workflow.jobs.list args run_id must be a positive integer");
    runId = String(args.run_id);
  }} else if (typeof args.run_id === "string") {{
    if (!/^[0-9]+$/.test(args.run_id) || args.run_id.length === 0 || args.run_id.length > 32 || args.run_id === "0") throw new TypeError("github.workflow.jobs.list args run_id must be a bounded decimal string");
    runId = args.run_id;
  }} else {{
    throw new TypeError("github.workflow.jobs.list args run_id must be a positive integer or decimal string");
  }}
  return {{owner, repo, run_id:runId}};
}}
function validateWorkerGithubCheckRunsListArgs(args) {{
  if (args === null || typeof args !== "object" || Array.isArray(args)) throw new TypeError("github.check.runs.list args must be an object");
  if (typeof args.owner !== "string" || typeof args.repo !== "string" || typeof args.ref !== "string") throw new TypeError("github.check.runs.list args owner, repo, and ref must be strings");
  const owner = args.owner;
  const repo = args.repo;
  const ref = args.ref;
  const nameRe = /^[A-Za-z0-9_.-]+$/;
  const refRe = /^[A-Za-z0-9_./-]+$/;
  if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) throw new TypeError("github.check.runs.list args owner and repo must be bounded GitHub path components");
  if (ref.length === 0 || ref.length > 200 || ref.indexOf("..") >= 0 || ref.indexOf("\\\\") >= 0 || !refRe.test(ref)) throw new TypeError("github.check.runs.list args ref must be a bounded Git ref or sha path component");
  return {{owner, repo, ref}};
}}
function validateWorkerGithubRepoReadArgs(args) {{
  if (args === null || typeof args !== "object" || Array.isArray(args)) throw new TypeError("github.repo.read args must be an object");
  if (typeof args.owner !== "string" || typeof args.repo !== "string") throw new TypeError("github.repo.read args owner and repo must be strings");
  const owner = args.owner;
  const repo = args.repo;
  const nameRe = /^[A-Za-z0-9_.-]+$/;
  if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) throw new TypeError("github.repo.read args owner and repo must be bounded GitHub path components");
  return {{owner, repo}};
}}
function validateWorkerModelCallArgs(args) {{
  if (args === null || typeof args !== "object" || Array.isArray(args)) throw new TypeError("model.call args must be an object");
  if (typeof args.id !== "string" || args.id.length === 0) throw new TypeError("model.call args id must be a non-empty string");
  if (typeof args.model !== "string" || args.model.length === 0) throw new TypeError("model.call provider args model must be a non-empty string");
  return {{id:args.id, model:args.model, input:args.input === undefined ? null : cloneJsonValue(args.input, "model.call input")}};
}}
function githubIssuePath(args) {{
  return "/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/issues/" + String(args.number);
}}
function githubPrPath(args) {{
  return "/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/pulls/" + String(args.number);
}}
function githubPrFilesPath(args) {{
  return "/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/pulls/" + String(args.number) + "/files";
}}
function githubReleaseLatestPath(args) {{
  return "/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/releases/latest";
}}
function githubFilePath(args) {{
  const encodedPath = args.path.split("/").map(encodeURIComponent).join("/");
  let path = "/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/contents/" + encodedPath;
  if (typeof args.ref === "string") path += "?ref=" + encodeURIComponent(args.ref);
  return path;
}}
function githubComparePath(args) {{
  return "/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/compare/" + encodeURIComponent(args.base) + "..." + encodeURIComponent(args.head);
}}
function githubCommitPath(args) {{
  return "/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/commits/" + encodeURIComponent(args.ref);
}}
function githubWorkflowRunPath(args) {{
  return "/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/actions/runs/" + encodeURIComponent(args.run_id);
}}
function githubWorkflowJobsPath(args) {{
  return "/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/actions/runs/" + encodeURIComponent(args.run_id) + "/jobs";
}}
function githubCheckRunsPath(args) {{
  return "/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/commits/" + encodeURIComponent(args.ref) + "/check-runs";
}}
function githubRepoPath(args) {{
  return "/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo);
}}
function npmMetadataPath(name) {{
  if (name.charAt(0) === "@") {{
    const slash = name.indexOf("/");
    return "/" + encodeURIComponent(name.slice(0, slash)) + "%2f" + encodeURIComponent(name.slice(slash + 1));
  }}
  return "/" + encodeURIComponent(name);
}}
function runWorkerNpmMetadataHostCall(args) {{
  const cleanArgs = validateWorkerNpmMetadataArgs(args);
  const endpoint = "https://registry.npmjs.org" + npmMetadataPath(cleanArgs.package);
  const path = namedNetworkPersistentCachePath("npm.metadata", cleanArgs);
  if (path === null) throw new Error("agent tool denied: npm.metadata worker persistent cache not configured");
  if (!fs.existsSync(path)) throw new Error("agent worker named network live async host-call required: npm.metadata cache miss");
  const envelope = JSON.parse(fs.readFileSync(path, "utf8"));
  if (envelope === null || envelope.tool !== "npm.metadata" || JSON.stringify(envelope.args) !== JSON.stringify(cleanArgs) || envelope.result === undefined) {{
    throw new Error("agent worker named network cache envelope invalid: npm.metadata");
  }}
  const storedMs = Number(envelope.stored_ms || 0);
  const ageMs = storedMs > 0 ? Math.max(0, Date.now() - storedMs) : null;
  if (namedNetworkCacheMaxAgeMs !== null && (ageMs === null || ageMs > namedNetworkCacheMaxAgeMs)) {{
    audit({{type:"named_network_cache_stale", tool:"npm.metadata", endpoint, cache_scope:"persistent", cache_path:path, stored_ms:storedMs, age_ms:ageMs, max_age_ms:namedNetworkCacheMaxAgeMs, disposition:"failed_closed", worker_host_call:true}});
    throw new Error("agent worker named network cache stale: npm.metadata");
  }}
  return cloneJsonValue(envelope.result, "npm.metadata persistent cache result");
}}
function runWorkerOsvQueryHostCall(args) {{
  const cleanArgs = validateWorkerOsvQueryArgs(args);
  const endpoint = "https://api.osv.dev/v1/query";
  const path = namedNetworkPersistentCachePath("osv.query", cleanArgs);
  if (path === null) throw new Error("agent tool denied: osv.query worker persistent cache not configured");
  if (!fs.existsSync(path)) throw new Error("agent worker named network live async host-call required: osv.query cache miss");
  const envelope = JSON.parse(fs.readFileSync(path, "utf8"));
  if (envelope === null || envelope.tool !== "osv.query" || JSON.stringify(envelope.args) !== JSON.stringify(cleanArgs) || envelope.result === undefined) {{
    throw new Error("agent worker named network cache envelope invalid: osv.query");
  }}
  const storedMs = Number(envelope.stored_ms || 0);
  const ageMs = storedMs > 0 ? Math.max(0, Date.now() - storedMs) : null;
  if (namedNetworkCacheMaxAgeMs !== null && (ageMs === null || ageMs > namedNetworkCacheMaxAgeMs)) {{
    audit({{type:"named_network_cache_stale", tool:"osv.query", endpoint, cache_scope:"persistent", cache_path:path, stored_ms:storedMs, age_ms:ageMs, max_age_ms:namedNetworkCacheMaxAgeMs, disposition:"failed_closed", worker_host_call:true}});
    throw new Error("agent worker named network cache stale: osv.query");
  }}
  return cloneJsonValue(envelope.result, "osv.query persistent cache result");
}}
function runWorkerGithubIssueReadHostCall(args) {{
  const cleanArgs = validateWorkerGithubIssueReadArgs(args);
  const endpoint = "https://api.github.com" + githubIssuePath(cleanArgs);
  const path = namedNetworkPersistentCachePath("github.issue.read", cleanArgs);
  if (path === null) throw new Error("agent tool denied: github.issue.read worker persistent cache not configured");
  if (!fs.existsSync(path)) throw new Error("agent worker named network live async host-call required: github.issue.read cache miss");
  const envelope = JSON.parse(fs.readFileSync(path, "utf8"));
  if (envelope === null || envelope.tool !== "github.issue.read" || JSON.stringify(envelope.args) !== JSON.stringify(cleanArgs) || envelope.result === undefined) {{
    throw new Error("agent worker named network cache envelope invalid: github.issue.read");
  }}
  const storedMs = Number(envelope.stored_ms || 0);
  const ageMs = storedMs > 0 ? Math.max(0, Date.now() - storedMs) : null;
  if (namedNetworkCacheMaxAgeMs !== null && (ageMs === null || ageMs > namedNetworkCacheMaxAgeMs)) {{
    audit({{type:"named_network_cache_stale", tool:"github.issue.read", endpoint, cache_scope:"persistent", cache_path:path, stored_ms:storedMs, age_ms:ageMs, max_age_ms:namedNetworkCacheMaxAgeMs, disposition:"failed_closed", worker_host_call:true}});
    throw new Error("agent worker named network cache stale: github.issue.read");
  }}
  return cloneJsonValue(envelope.result, "github.issue.read persistent cache result");
}}
function runWorkerGithubPrReadHostCall(args) {{
  const cleanArgs = validateWorkerGithubPrReadArgs(args);
  const endpoint = "https://api.github.com" + githubPrPath(cleanArgs);
  const path = namedNetworkPersistentCachePath("github.pr.read", cleanArgs);
  if (path === null) throw new Error("agent tool denied: github.pr.read worker persistent cache not configured");
  if (!fs.existsSync(path)) throw new Error("agent worker named network live async host-call required: github.pr.read cache miss");
  const envelope = JSON.parse(fs.readFileSync(path, "utf8"));
  if (envelope === null || envelope.tool !== "github.pr.read" || JSON.stringify(envelope.args) !== JSON.stringify(cleanArgs) || envelope.result === undefined) {{
    throw new Error("agent worker named network cache envelope invalid: github.pr.read");
  }}
  const storedMs = Number(envelope.stored_ms || 0);
  const ageMs = storedMs > 0 ? Math.max(0, Date.now() - storedMs) : null;
  if (namedNetworkCacheMaxAgeMs !== null && (ageMs === null || ageMs > namedNetworkCacheMaxAgeMs)) {{
    audit({{type:"named_network_cache_stale", tool:"github.pr.read", endpoint, cache_scope:"persistent", cache_path:path, stored_ms:storedMs, age_ms:ageMs, max_age_ms:namedNetworkCacheMaxAgeMs, disposition:"failed_closed", worker_host_call:true}});
    throw new Error("agent worker named network cache stale: github.pr.read");
  }}
  return cloneJsonValue(envelope.result, "github.pr.read persistent cache result");
}}
function runWorkerGithubPrFilesListHostCall(args) {{
  const cleanArgs = validateWorkerGithubPrFilesListArgs(args);
  const endpoint = "https://api.github.com" + githubPrFilesPath(cleanArgs);
  const path = namedNetworkPersistentCachePath("github.pr.files.list", cleanArgs);
  if (path === null) throw new Error("agent tool denied: github.pr.files.list worker persistent cache not configured");
  if (!fs.existsSync(path)) throw new Error("agent worker named network live async host-call required: github.pr.files.list cache miss");
  const envelope = JSON.parse(fs.readFileSync(path, "utf8"));
  if (envelope === null || envelope.tool !== "github.pr.files.list" || JSON.stringify(envelope.args) !== JSON.stringify(cleanArgs) || envelope.result === undefined) {{
    throw new Error("agent worker named network cache envelope invalid: github.pr.files.list");
  }}
  const storedMs = Number(envelope.stored_ms || 0);
  const ageMs = storedMs > 0 ? Math.max(0, Date.now() - storedMs) : null;
  if (namedNetworkCacheMaxAgeMs !== null && (ageMs === null || ageMs > namedNetworkCacheMaxAgeMs)) {{
    audit({{type:"named_network_cache_stale", tool:"github.pr.files.list", endpoint, cache_scope:"persistent", cache_path:path, stored_ms:storedMs, age_ms:ageMs, max_age_ms:namedNetworkCacheMaxAgeMs, disposition:"failed_closed", worker_host_call:true}});
    throw new Error("agent worker named network cache stale: github.pr.files.list");
  }}
  return cloneJsonValue(envelope.result, "github.pr.files.list persistent cache result");
}}
function runWorkerGithubReleaseLatestReadHostCall(args) {{
  const cleanArgs = validateWorkerGithubReleaseLatestReadArgs(args);
  const endpoint = "https://api.github.com" + githubReleaseLatestPath(cleanArgs);
  const path = namedNetworkPersistentCachePath("github.release.latest.read", cleanArgs);
  if (path === null) throw new Error("agent tool denied: github.release.latest.read worker persistent cache not configured");
  if (!fs.existsSync(path)) throw new Error("agent worker named network live async host-call required: github.release.latest.read cache miss");
  const envelope = JSON.parse(fs.readFileSync(path, "utf8"));
  if (envelope === null || envelope.tool !== "github.release.latest.read" || JSON.stringify(envelope.args) !== JSON.stringify(cleanArgs) || envelope.result === undefined) {{
    throw new Error("agent worker named network cache envelope invalid: github.release.latest.read");
  }}
  const storedMs = Number(envelope.stored_ms || 0);
  const ageMs = storedMs > 0 ? Math.max(0, Date.now() - storedMs) : null;
  if (namedNetworkCacheMaxAgeMs !== null && (ageMs === null || ageMs > namedNetworkCacheMaxAgeMs)) {{
    audit({{type:"named_network_cache_stale", tool:"github.release.latest.read", endpoint, cache_scope:"persistent", cache_path:path, stored_ms:storedMs, age_ms:ageMs, max_age_ms:namedNetworkCacheMaxAgeMs, disposition:"failed_closed", worker_host_call:true}});
    throw new Error("agent worker named network cache stale: github.release.latest.read");
  }}
  return cloneJsonValue(envelope.result, "github.release.latest.read persistent cache result");
}}
function runWorkerGithubFileReadHostCall(args) {{
  const cleanArgs = validateWorkerGithubFileReadArgs(args);
  const endpoint = "https://api.github.com" + githubFilePath(cleanArgs);
  const path = namedNetworkPersistentCachePath("github.file.read", cleanArgs);
  if (path === null) throw new Error("agent tool denied: github.file.read worker persistent cache not configured");
  if (!fs.existsSync(path)) throw new Error("agent worker named network live async host-call required: github.file.read cache miss");
  const envelope = JSON.parse(fs.readFileSync(path, "utf8"));
  if (envelope === null || envelope.tool !== "github.file.read" || JSON.stringify(envelope.args) !== JSON.stringify(cleanArgs) || envelope.result === undefined) {{
    throw new Error("agent worker named network cache envelope invalid: github.file.read");
  }}
  const storedMs = Number(envelope.stored_ms || 0);
  const ageMs = storedMs > 0 ? Math.max(0, Date.now() - storedMs) : null;
  if (namedNetworkCacheMaxAgeMs !== null && (ageMs === null || ageMs > namedNetworkCacheMaxAgeMs)) {{
    audit({{type:"named_network_cache_stale", tool:"github.file.read", endpoint, cache_scope:"persistent", cache_path:path, stored_ms:storedMs, age_ms:ageMs, max_age_ms:namedNetworkCacheMaxAgeMs, disposition:"failed_closed", worker_host_call:true}});
    throw new Error("agent worker named network cache stale: github.file.read");
  }}
  return cloneJsonValue(envelope.result, "github.file.read persistent cache result");
}}
function runWorkerGithubCompareReadHostCall(args) {{
  const cleanArgs = validateWorkerGithubCompareReadArgs(args);
  const endpoint = "https://api.github.com" + githubComparePath(cleanArgs);
  const path = namedNetworkPersistentCachePath("github.compare.read", cleanArgs);
  if (path === null) throw new Error("agent tool denied: github.compare.read worker persistent cache not configured");
  if (!fs.existsSync(path)) throw new Error("agent worker named network live async host-call required: github.compare.read cache miss");
  const envelope = JSON.parse(fs.readFileSync(path, "utf8"));
  if (envelope === null || envelope.tool !== "github.compare.read" || JSON.stringify(envelope.args) !== JSON.stringify(cleanArgs) || envelope.result === undefined) {{
    throw new Error("agent worker named network cache envelope invalid: github.compare.read");
  }}
  const storedMs = Number(envelope.stored_ms || 0);
  const ageMs = storedMs > 0 ? Math.max(0, Date.now() - storedMs) : null;
  if (namedNetworkCacheMaxAgeMs !== null && (ageMs === null || ageMs > namedNetworkCacheMaxAgeMs)) {{
    audit({{type:"named_network_cache_stale", tool:"github.compare.read", endpoint, cache_scope:"persistent", cache_path:path, stored_ms:storedMs, age_ms:ageMs, max_age_ms:namedNetworkCacheMaxAgeMs, disposition:"failed_closed", worker_host_call:true}});
    throw new Error("agent worker named network cache stale: github.compare.read");
  }}
  return cloneJsonValue(envelope.result, "github.compare.read persistent cache result");
}}
function runWorkerGithubCommitReadHostCall(args) {{
  const cleanArgs = validateWorkerGithubCommitReadArgs(args);
  const endpoint = "https://api.github.com" + githubCommitPath(cleanArgs);
  const path = namedNetworkPersistentCachePath("github.commit.read", cleanArgs);
  if (path === null) throw new Error("agent tool denied: github.commit.read worker persistent cache not configured");
  if (!fs.existsSync(path)) throw new Error("agent worker named network live async host-call required: github.commit.read cache miss");
  const envelope = JSON.parse(fs.readFileSync(path, "utf8"));
  if (envelope === null || envelope.tool !== "github.commit.read" || JSON.stringify(envelope.args) !== JSON.stringify(cleanArgs) || envelope.result === undefined) {{
    throw new Error("agent worker named network cache envelope invalid: github.commit.read");
  }}
  const storedMs = Number(envelope.stored_ms || 0);
  const ageMs = storedMs > 0 ? Math.max(0, Date.now() - storedMs) : null;
  if (namedNetworkCacheMaxAgeMs !== null && (ageMs === null || ageMs > namedNetworkCacheMaxAgeMs)) {{
    audit({{type:"named_network_cache_stale", tool:"github.commit.read", endpoint, cache_scope:"persistent", cache_path:path, stored_ms:storedMs, age_ms:ageMs, max_age_ms:namedNetworkCacheMaxAgeMs, disposition:"failed_closed", worker_host_call:true}});
    throw new Error("agent worker named network cache stale: github.commit.read");
  }}
  return cloneJsonValue(envelope.result, "github.commit.read persistent cache result");
}}
function runWorkerGithubWorkflowRunReadHostCall(args) {{
  const cleanArgs = validateWorkerGithubWorkflowRunReadArgs(args);
  const endpoint = "https://api.github.com" + githubWorkflowRunPath(cleanArgs);
  const path = namedNetworkPersistentCachePath("github.workflow.run.read", cleanArgs);
  if (path === null) throw new Error("agent tool denied: github.workflow.run.read worker persistent cache not configured");
  if (!fs.existsSync(path)) throw new Error("agent worker named network live async host-call required: github.workflow.run.read cache miss");
  const envelope = JSON.parse(fs.readFileSync(path, "utf8"));
  if (envelope === null || envelope.tool !== "github.workflow.run.read" || JSON.stringify(envelope.args) !== JSON.stringify(cleanArgs) || envelope.result === undefined) {{
    throw new Error("agent worker named network cache envelope invalid: github.workflow.run.read");
  }}
  const storedMs = Number(envelope.stored_ms || 0);
  const ageMs = storedMs > 0 ? Math.max(0, Date.now() - storedMs) : null;
  if (namedNetworkCacheMaxAgeMs !== null && (ageMs === null || ageMs > namedNetworkCacheMaxAgeMs)) {{
    audit({{type:"named_network_cache_stale", tool:"github.workflow.run.read", endpoint, cache_scope:"persistent", cache_path:path, stored_ms:storedMs, age_ms:ageMs, max_age_ms:namedNetworkCacheMaxAgeMs, disposition:"failed_closed", worker_host_call:true}});
    throw new Error("agent worker named network cache stale: github.workflow.run.read");
  }}
  return cloneJsonValue(envelope.result, "github.workflow.run.read persistent cache result");
}}
function runWorkerGithubWorkflowJobsListHostCall(args) {{
  const cleanArgs = validateWorkerGithubWorkflowJobsListArgs(args);
  const endpoint = "https://api.github.com" + githubWorkflowJobsPath(cleanArgs);
  const path = namedNetworkPersistentCachePath("github.workflow.jobs.list", cleanArgs);
  if (path === null) throw new Error("agent tool denied: github.workflow.jobs.list worker persistent cache not configured");
  if (!fs.existsSync(path)) throw new Error("agent worker named network live async host-call required: github.workflow.jobs.list cache miss");
  const envelope = JSON.parse(fs.readFileSync(path, "utf8"));
  if (envelope === null || envelope.tool !== "github.workflow.jobs.list" || JSON.stringify(envelope.args) !== JSON.stringify(cleanArgs) || envelope.result === undefined) {{
    throw new Error("agent worker named network cache envelope invalid: github.workflow.jobs.list");
  }}
  const storedMs = Number(envelope.stored_ms || 0);
  const ageMs = storedMs > 0 ? Math.max(0, Date.now() - storedMs) : null;
  if (namedNetworkCacheMaxAgeMs !== null && (ageMs === null || ageMs > namedNetworkCacheMaxAgeMs)) {{
    audit({{type:"named_network_cache_stale", tool:"github.workflow.jobs.list", endpoint, cache_scope:"persistent", cache_path:path, stored_ms:storedMs, age_ms:ageMs, max_age_ms:namedNetworkCacheMaxAgeMs, disposition:"failed_closed", worker_host_call:true}});
    throw new Error("agent worker named network cache stale: github.workflow.jobs.list");
  }}
  return cloneJsonValue(envelope.result, "github.workflow.jobs.list persistent cache result");
}}
function runWorkerGithubCheckRunsListHostCall(args) {{
  const cleanArgs = validateWorkerGithubCheckRunsListArgs(args);
  const endpoint = "https://api.github.com" + githubCheckRunsPath(cleanArgs);
  const path = namedNetworkPersistentCachePath("github.check.runs.list", cleanArgs);
  if (path === null) throw new Error("agent tool denied: github.check.runs.list worker persistent cache not configured");
  if (!fs.existsSync(path)) throw new Error("agent worker named network live async host-call required: github.check.runs.list cache miss");
  const envelope = JSON.parse(fs.readFileSync(path, "utf8"));
  if (envelope === null || envelope.tool !== "github.check.runs.list" || JSON.stringify(envelope.args) !== JSON.stringify(cleanArgs) || envelope.result === undefined) {{
    throw new Error("agent worker named network cache envelope invalid: github.check.runs.list");
  }}
  const storedMs = Number(envelope.stored_ms || 0);
  const ageMs = storedMs > 0 ? Math.max(0, Date.now() - storedMs) : null;
  if (namedNetworkCacheMaxAgeMs !== null && (ageMs === null || ageMs > namedNetworkCacheMaxAgeMs)) {{
    audit({{type:"named_network_cache_stale", tool:"github.check.runs.list", endpoint, cache_scope:"persistent", cache_path:path, stored_ms:storedMs, age_ms:ageMs, max_age_ms:namedNetworkCacheMaxAgeMs, disposition:"failed_closed", worker_host_call:true}});
    throw new Error("agent worker named network cache stale: github.check.runs.list");
  }}
  return cloneJsonValue(envelope.result, "github.check.runs.list persistent cache result");
}}
function runWorkerGithubRepoReadHostCall(args) {{
  const cleanArgs = validateWorkerGithubRepoReadArgs(args);
  const endpoint = "https://api.github.com" + githubRepoPath(cleanArgs);
  const path = namedNetworkPersistentCachePath("github.repo.read", cleanArgs);
  if (path === null) throw new Error("agent tool denied: github.repo.read worker persistent cache not configured");
  if (!fs.existsSync(path)) throw new Error("agent worker named network live async host-call required: github.repo.read cache miss");
  const envelope = JSON.parse(fs.readFileSync(path, "utf8"));
  if (envelope === null || envelope.tool !== "github.repo.read" || JSON.stringify(envelope.args) !== JSON.stringify(cleanArgs) || envelope.result === undefined) {{
    throw new Error("agent worker named network cache envelope invalid: github.repo.read");
  }}
  const storedMs = Number(envelope.stored_ms || 0);
  const ageMs = storedMs > 0 ? Math.max(0, Date.now() - storedMs) : null;
  if (namedNetworkCacheMaxAgeMs !== null && (ageMs === null || ageMs > namedNetworkCacheMaxAgeMs)) {{
    audit({{type:"named_network_cache_stale", tool:"github.repo.read", endpoint, cache_scope:"persistent", cache_path:path, stored_ms:storedMs, age_ms:ageMs, max_age_ms:namedNetworkCacheMaxAgeMs, disposition:"failed_closed", worker_host_call:true}});
    throw new Error("agent worker named network cache stale: github.repo.read");
  }}
  return cloneJsonValue(envelope.result, "github.repo.read persistent cache result");
}}
function runWorkerModelCallProviderHostCall(args) {{
  const cleanArgs = validateWorkerModelCallArgs(args);
  if (modelProvider !== "openai.responses") throw new Error("agent model provider not admitted");
  if (modelApiKeyToken === null) throw new Error("agent model provider credential not configured");
  const endpoint = "https://api.openai.com/v1/responses";
  const testResponse = process.env.CRUFT_AGENT_TEST_MODEL_PROVIDER_RESPONSE || "";
  if (testResponse.length === 0) throw new Error("agent worker model provider live async host-call required");
  const body = JSON.stringify({{model:cleanArgs.model, input:cleanArgs.input === null ? cleanArgs.id : cleanArgs.input}});
  if (Buffer.byteLength(body) > maxToolArgBytes) throw new Error("agent model provider request byte budget exceeded");
  let result;
  try {{
    result = JSON.parse(testResponse);
  }} catch (_e) {{
    throw new Error("agent model provider test response invalid json");
  }}
  const resultBytes = JSON.stringify(result).length;
  if (resultBytes > maxToolResultBytes) throw new Error("agent model provider response byte budget exceeded");
  return cloneJsonValue(result, "model.call provider result");
}}
function workerModelProviderErrorTaxonomy(e) {{
  const message = safeMessage(e);
  if (message.indexOf("credential not configured") >= 0) return {{error_kind:"credential_not_configured", retryable:false}};
  if (message.indexOf("provider args model") >= 0) return {{error_kind:"invalid_args", retryable:false}};
  if (message.indexOf("request byte budget") >= 0) return {{error_kind:"request_byte_budget", retryable:false}};
  if (message.indexOf("response byte budget") >= 0) return {{error_kind:"response_byte_budget", retryable:false}};
  if (message.indexOf("live async host-call required") >= 0) return {{error_kind:"async_live_required", retryable:false}};
  if (message.indexOf("invalid json") >= 0) return {{error_kind:"invalid_json", retryable:false}};
  return {{error_kind:"handler_error", retryable:false}};
}}
function runWorkerProcessHostCall(args) {{
  if (args === null || typeof args !== "object" || Array.isArray(args)) throw new TypeError("process args must be an object");
  if (typeof args.command !== "string" || args.command.length === 0) throw new TypeError("process args command must be a non-empty string");
  const command = findProcessCommand(args.command);
  if (command === null) throw new Error("agent tool denied: process command not admitted");
  const argv = Array.isArray(args.args) ? args.args.map(function(v) {{ return String(v); }}) : [];
  let cwd = typeof args.cwd === "string" ? args.cwd : (processCwds[0] && processCwds[0].root);
  try {{
    cwd = fs.realpathSync(cwd);
  }} catch (_e) {{
    throw new Error("agent tool denied: process cwd not admitted");
  }}
  let cwdAdmitted = false;
  for (let i = 0; i < processCwds.length; i++) {{
    const root = String(processCwds[i].root);
    if (cwd === root || cwd.indexOf(root + "/") === 0) cwdAdmitted = true;
  }}
  if (!cwdAdmitted) throw new Error("agent tool denied: process cwd not admitted");
  if (args.output === "stream") {{
    audit({{type:"unsupported_control", status:"error", control:"worker_hosted", reason:"agent_worker_process_stream_backpressure_required_not_available"}});
    throw new Error("agent worker process stream/backpressure required not available");
  }}
  let output = args.output === "summary" ? "summary" : "full";
  const started = Date.now();
  const processCaptureLimit = Math.max(maxToolResultBytes, maxEventBytes, 65536);
  const out = childProcess.spawnSync(command.path, argv, {{cwd, env:args.env || {{}}, encoding:"utf8", timeout:{tool_timeout_ms}, maxBuffer:processCaptureLimit, killSignal:"SIGKILL", shell:false}});
  const duration = Date.now() - started;
  const stdout = out.stdout === null || out.stdout === undefined ? "" : String(out.stdout);
  const stderr = out.stderr === null || out.stderr === undefined ? "" : String(out.stderr);
  if ((out.error && String(out.error.code || "") === "ETIMEDOUT") || (out.signal !== null && out.signal !== undefined && duration >= {tool_timeout_ms})) {{
    audit({{type:"tool_timeout", tool:"process", policy:"allowed", command:args.command, timeout_ms:{tool_timeout_ms}, duration_ms:duration, cancelled:true, cancellation:"killed", kill_signal:"SIGKILL", signal:out.signal === null || out.signal === undefined ? "SIGKILL" : out.signal, worker_host_call:true}});
    throw new Error("agent tool timeout: process after {tool_timeout_ms}ms");
  }}
  const outputOverflow = (out.error && String(out.error.code || "") === "ENOBUFS") || stdout.length > maxToolResultBytes || stderr.length > maxToolResultBytes;
  if (outputOverflow && output === "summary") {{
    auditProcessOutputStream(args.command, "stdout", stdout, true);
    auditProcessOutputStream(args.command, "stderr", stderr, true);
    audit({{type:"process_output_budget", tool:"process", policy:"allowed", command:args.command, limit:maxToolResultBytes, stdout_bytes:stdout.length, stderr_bytes:stderr.length, truncated:true, disposition:"summarized", worker_host_call:true}});
    return {{command:args.command, exit_code:out.status === null || out.status === undefined ? null : out.status, signal:out.signal === null || out.signal === undefined ? null : out.signal, stdout:"", stderr:"", stdout_summary:processOutputSummary("stdout", stdout, true), stderr_summary:processOutputSummary("stderr", stderr, true), output_mode:"summary", duration_ms:duration}};
  }}
  if (outputOverflow) {{
    auditProcessOutputStream(args.command, "stdout", stdout, true);
    auditProcessOutputStream(args.command, "stderr", stderr, true);
    audit({{type:"process_output_budget", tool:"process", policy:"allowed", command:args.command, limit:maxToolResultBytes, stdout_bytes:stdout.length, stderr_bytes:stderr.length, truncated:true, disposition:"failed_closed", worker_host_call:true}});
    audit({{type:"budget_exceeded", budget:"process_output_bytes", tool:"process", limit:maxToolResultBytes, stdout_bytes:stdout.length, stderr_bytes:stderr.length, worker_host_call:true}});
    throw new Error("agent process output byte budget exceeded");
  }}
  auditProcessOutputStream(args.command, "stdout", stdout, false);
  auditProcessOutputStream(args.command, "stderr", stderr, false);
  if (output === "summary") {{
    return {{command:args.command, exit_code:out.status === null || out.status === undefined ? null : out.status, signal:out.signal === null || out.signal === undefined ? null : out.signal, stdout:"", stderr:"", stdout_summary:processOutputSummary("stdout", stdout, false), stderr_summary:processOutputSummary("stderr", stderr, false), output_mode:"summary", duration_ms:duration}};
  }}
  return {{command:args.command, exit_code:out.status === null || out.status === undefined ? null : out.status, signal:out.signal === null || out.signal === undefined ? null : out.signal, stdout, stderr, duration_ms:duration}};
}}
if (allowedTools.indexOf("process") >= 0) {{
  __cruft_registerHostCall("process", function(args) {{
    const started = Date.now();
    let argBytes = 0;
    try {{
      argBytes = JSON.stringify(args).length;
    }} catch (_e) {{}}
    try {{
      const result = runWorkerProcessHostCall(args);
      let resultBytes = 0;
      try {{
        resultBytes = JSON.stringify(result).length;
      }} catch (_e) {{}}
      audit({{type:"worker_host_call", tool:"process", outcome:"ok", policy:"allowed", arg_bytes:argBytes, result_bytes:resultBytes, elapsed_ms:Date.now() - started, adapter:"agent_process"}});
      return result;
    }} catch (e) {{
      const message = safeMessage(e);
      const errorKind = message.indexOf("timeout") >= 0 ? "timeout" : (message.indexOf("denied") >= 0 ? "denied" : (message.indexOf("budget") >= 0 ? "payload_cap" : "handler_error"));
      audit({{type:"worker_host_call", tool:"process", outcome:"error", policy:"allowed", arg_bytes:argBytes, elapsed_ms:Date.now() - started, error_kind:errorKind, error:message, adapter:"agent_process"}});
      throw e;
    }}
  }});
}}
if (approvalRequiredTools.length > 0) {{
  __cruft_registerHostCall("agent.approval", function(request) {{
    const started = Date.now();
    let argBytes = 0;
    try {{
      argBytes = JSON.stringify(request).length;
    }} catch (_e) {{}}
    try {{
      if (request === null || typeof request !== "object" || Array.isArray(request)) throw new TypeError("agent.approval request must be an object");
      const tool = String(request.tool || "");
      if (tool.length === 0 || allowedTools.indexOf(tool) < 0) throw new Error("agent approval denied: tool not admitted");
      const args = request.args === undefined ? {{}} : cloneJsonValue(request.args, "approval args");
      const approvalArgBytes = Number(request.arg_bytes || JSON.stringify(args).length);
      const result = requireParentToolApproval(tool, args, approvalArgBytes);
      audit({{type:"worker_host_call", tool:"agent.approval", approved_tool:tool, outcome:result.ok === true ? "ok" : "error", policy:"allowed", arg_bytes:argBytes, result_bytes:JSON.stringify(result).length, elapsed_ms:Date.now() - started, adapter:"agent_approval"}});
      return result;
    }} catch (e) {{
      const message = safeMessage(e);
      audit({{type:"worker_host_call", tool:"agent.approval", outcome:"error", policy:"allowed", arg_bytes:argBytes, elapsed_ms:Date.now() - started, error_kind:message.indexOf("not admitted") >= 0 ? "denied" : "handler_error", error:message, adapter:"agent_approval"}});
      return {{ok:false, status:"error", error:message}};
    }}
  }});
}}
if (allowedTools.indexOf("osv.query") >= 0 && osvFixture === null) {{
  __cruft_registerHostCall("osv.query", function(args) {{
    const started = Date.now();
    let argBytes = 0;
    try {{
      argBytes = JSON.stringify(args).length;
    }} catch (_e) {{}}
    try {{
      const result = runWorkerOsvQueryHostCall(args);
      let resultBytes = 0;
      try {{
        resultBytes = JSON.stringify(result).length;
      }} catch (_e) {{}}
      audit({{type:"worker_host_call", tool:"osv.query", outcome:"ok", policy:"allowed", arg_bytes:argBytes, result_bytes:resultBytes, elapsed_ms:Date.now() - started, adapter:"agent_named_network_persistent_cache"}});
      return result;
    }} catch (e) {{
      const message = safeMessage(e);
      const errorKind = message.indexOf("cache stale") >= 0 ? "cache_stale" : (message.indexOf("cache miss") >= 0 || message.indexOf("live async host-call required") >= 0 ? "async_live_required" : (message.indexOf("denied") >= 0 ? "denied" : (message.indexOf("invalid") >= 0 ? "invalid_cache" : "handler_error")));
      audit({{type:"worker_host_call", tool:"osv.query", outcome:"error", policy:"allowed", arg_bytes:argBytes, elapsed_ms:Date.now() - started, error_kind:errorKind, error:message, adapter:"agent_named_network_persistent_cache"}});
      throw e;
    }}
  }});
}}
if (allowedTools.indexOf("npm.metadata") >= 0) {{
  __cruft_registerHostCall("npm.metadata", function(args) {{
    const started = Date.now();
    let argBytes = 0;
    try {{
      argBytes = JSON.stringify(args).length;
    }} catch (_e) {{}}
    try {{
      const result = runWorkerNpmMetadataHostCall(args);
      let resultBytes = 0;
      try {{
        resultBytes = JSON.stringify(result).length;
      }} catch (_e) {{}}
      audit({{type:"worker_host_call", tool:"npm.metadata", outcome:"ok", policy:"allowed", arg_bytes:argBytes, result_bytes:resultBytes, elapsed_ms:Date.now() - started, adapter:"agent_named_network_persistent_cache"}});
      return result;
    }} catch (e) {{
      const message = safeMessage(e);
      const errorKind = message.indexOf("cache stale") >= 0 ? "cache_stale" : (message.indexOf("cache miss") >= 0 || message.indexOf("live async host-call required") >= 0 ? "async_live_required" : (message.indexOf("denied") >= 0 ? "denied" : (message.indexOf("invalid") >= 0 ? "invalid_cache" : "handler_error")));
      audit({{type:"worker_host_call", tool:"npm.metadata", outcome:"error", policy:"allowed", arg_bytes:argBytes, elapsed_ms:Date.now() - started, error_kind:errorKind, error:message, adapter:"agent_named_network_persistent_cache"}});
      throw e;
    }}
  }});
}}
if (allowedTools.indexOf("github.issue.read") >= 0) {{
  __cruft_registerHostCall("github.issue.read", function(args) {{
    const started = Date.now();
    let argBytes = 0;
    try {{
      argBytes = JSON.stringify(args).length;
    }} catch (_e) {{}}
    try {{
      const result = runWorkerGithubIssueReadHostCall(args);
      let resultBytes = 0;
      try {{
        resultBytes = JSON.stringify(result).length;
      }} catch (_e) {{}}
      audit({{type:"worker_host_call", tool:"github.issue.read", outcome:"ok", policy:"allowed", arg_bytes:argBytes, result_bytes:resultBytes, elapsed_ms:Date.now() - started, adapter:"agent_named_network_persistent_cache"}});
      return result;
    }} catch (e) {{
      const message = safeMessage(e);
      const errorKind = message.indexOf("cache stale") >= 0 ? "cache_stale" : (message.indexOf("cache miss") >= 0 || message.indexOf("live async host-call required") >= 0 ? "async_live_required" : (message.indexOf("denied") >= 0 ? "denied" : (message.indexOf("invalid") >= 0 ? "invalid_cache" : "handler_error")));
      audit({{type:"worker_host_call", tool:"github.issue.read", outcome:"error", policy:"allowed", arg_bytes:argBytes, elapsed_ms:Date.now() - started, error_kind:errorKind, error:message, adapter:"agent_named_network_persistent_cache"}});
      throw e;
    }}
  }});
}}
if (allowedTools.indexOf("github.pr.read") >= 0) {{
  __cruft_registerHostCall("github.pr.read", function(args) {{
    const started = Date.now();
    let argBytes = 0;
    try {{
      argBytes = JSON.stringify(args).length;
    }} catch (_e) {{}}
    try {{
      const result = runWorkerGithubPrReadHostCall(args);
      let resultBytes = 0;
      try {{
        resultBytes = JSON.stringify(result).length;
      }} catch (_e) {{}}
      audit({{type:"worker_host_call", tool:"github.pr.read", outcome:"ok", policy:"allowed", arg_bytes:argBytes, result_bytes:resultBytes, elapsed_ms:Date.now() - started, adapter:"agent_named_network_persistent_cache"}});
      return result;
    }} catch (e) {{
      const message = safeMessage(e);
      const errorKind = message.indexOf("cache stale") >= 0 ? "cache_stale" : (message.indexOf("cache miss") >= 0 || message.indexOf("live async host-call required") >= 0 ? "async_live_required" : (message.indexOf("denied") >= 0 ? "denied" : (message.indexOf("invalid") >= 0 ? "invalid_cache" : "handler_error")));
      audit({{type:"worker_host_call", tool:"github.pr.read", outcome:"error", policy:"allowed", arg_bytes:argBytes, elapsed_ms:Date.now() - started, error_kind:errorKind, error:message, adapter:"agent_named_network_persistent_cache"}});
      throw e;
    }}
  }});
}}
if (allowedTools.indexOf("github.pr.files.list") >= 0) {{
  __cruft_registerHostCall("github.pr.files.list", function(args) {{
    const started = Date.now();
    let argBytes = 0;
    try {{
      argBytes = JSON.stringify(args).length;
    }} catch (_e) {{}}
    try {{
      const result = runWorkerGithubPrFilesListHostCall(args);
      let resultBytes = 0;
      try {{
        resultBytes = JSON.stringify(result).length;
      }} catch (_e) {{}}
      audit({{type:"worker_host_call", tool:"github.pr.files.list", outcome:"ok", policy:"allowed", arg_bytes:argBytes, result_bytes:resultBytes, elapsed_ms:Date.now() - started, adapter:"agent_named_network_persistent_cache"}});
      return result;
    }} catch (e) {{
      const message = safeMessage(e);
      const errorKind = message.indexOf("cache stale") >= 0 ? "cache_stale" : (message.indexOf("cache miss") >= 0 || message.indexOf("live async host-call required") >= 0 ? "async_live_required" : (message.indexOf("denied") >= 0 ? "denied" : (message.indexOf("invalid") >= 0 ? "invalid_cache" : "handler_error")));
      audit({{type:"worker_host_call", tool:"github.pr.files.list", outcome:"error", policy:"allowed", arg_bytes:argBytes, elapsed_ms:Date.now() - started, error_kind:errorKind, error:message, adapter:"agent_named_network_persistent_cache"}});
      throw e;
    }}
  }});
}}
if (allowedTools.indexOf("github.release.latest.read") >= 0) {{
  __cruft_registerHostCall("github.release.latest.read", function(args) {{
    const started = Date.now();
    let argBytes = 0;
    try {{
      argBytes = JSON.stringify(args).length;
    }} catch (_e) {{}}
    try {{
      const result = runWorkerGithubReleaseLatestReadHostCall(args);
      let resultBytes = 0;
      try {{
        resultBytes = JSON.stringify(result).length;
      }} catch (_e) {{}}
      audit({{type:"worker_host_call", tool:"github.release.latest.read", outcome:"ok", policy:"allowed", arg_bytes:argBytes, result_bytes:resultBytes, elapsed_ms:Date.now() - started, adapter:"agent_named_network_persistent_cache"}});
      return result;
    }} catch (e) {{
      const message = safeMessage(e);
      const errorKind = message.indexOf("cache stale") >= 0 ? "cache_stale" : (message.indexOf("cache miss") >= 0 || message.indexOf("live async host-call required") >= 0 ? "async_live_required" : (message.indexOf("denied") >= 0 ? "denied" : (message.indexOf("invalid") >= 0 ? "invalid_cache" : "handler_error")));
      audit({{type:"worker_host_call", tool:"github.release.latest.read", outcome:"error", policy:"allowed", arg_bytes:argBytes, elapsed_ms:Date.now() - started, error_kind:errorKind, error:message, adapter:"agent_named_network_persistent_cache"}});
      throw e;
    }}
  }});
}}
if (allowedTools.indexOf("github.file.read") >= 0) {{
  __cruft_registerHostCall("github.file.read", function(args) {{
    const started = Date.now();
    let argBytes = 0;
    try {{
      argBytes = JSON.stringify(args).length;
    }} catch (_e) {{}}
    try {{
      const result = runWorkerGithubFileReadHostCall(args);
      let resultBytes = 0;
      try {{
        resultBytes = JSON.stringify(result).length;
      }} catch (_e) {{}}
      audit({{type:"worker_host_call", tool:"github.file.read", outcome:"ok", policy:"allowed", arg_bytes:argBytes, result_bytes:resultBytes, elapsed_ms:Date.now() - started, adapter:"agent_named_network_persistent_cache"}});
      return result;
    }} catch (e) {{
      const message = safeMessage(e);
      const errorKind = message.indexOf("cache stale") >= 0 ? "cache_stale" : (message.indexOf("cache miss") >= 0 || message.indexOf("live async host-call required") >= 0 ? "async_live_required" : (message.indexOf("denied") >= 0 ? "denied" : (message.indexOf("invalid") >= 0 ? "invalid_cache" : "handler_error")));
      audit({{type:"worker_host_call", tool:"github.file.read", outcome:"error", policy:"allowed", arg_bytes:argBytes, elapsed_ms:Date.now() - started, error_kind:errorKind, error:message, adapter:"agent_named_network_persistent_cache"}});
      throw e;
    }}
  }});
}}
if (allowedTools.indexOf("github.compare.read") >= 0) {{
  __cruft_registerHostCall("github.compare.read", function(args) {{
    const started = Date.now();
    let argBytes = 0;
    try {{
      argBytes = JSON.stringify(args).length;
    }} catch (_e) {{}}
    try {{
      const result = runWorkerGithubCompareReadHostCall(args);
      let resultBytes = 0;
      try {{
        resultBytes = JSON.stringify(result).length;
      }} catch (_e) {{}}
      audit({{type:"worker_host_call", tool:"github.compare.read", outcome:"ok", policy:"allowed", arg_bytes:argBytes, result_bytes:resultBytes, elapsed_ms:Date.now() - started, adapter:"agent_named_network_persistent_cache"}});
      return result;
    }} catch (e) {{
      const message = safeMessage(e);
      const errorKind = message.indexOf("cache stale") >= 0 ? "cache_stale" : (message.indexOf("cache miss") >= 0 || message.indexOf("live async host-call required") >= 0 ? "async_live_required" : (message.indexOf("denied") >= 0 ? "denied" : (message.indexOf("invalid") >= 0 ? "invalid_cache" : "handler_error")));
      audit({{type:"worker_host_call", tool:"github.compare.read", outcome:"error", policy:"allowed", arg_bytes:argBytes, elapsed_ms:Date.now() - started, error_kind:errorKind, error:message, adapter:"agent_named_network_persistent_cache"}});
      throw e;
    }}
  }});
}}
if (allowedTools.indexOf("github.commit.read") >= 0) {{
  __cruft_registerHostCall("github.commit.read", function(args) {{
    const started = Date.now();
    let argBytes = 0;
    try {{
      argBytes = JSON.stringify(args).length;
    }} catch (_e) {{}}
    try {{
      const result = runWorkerGithubCommitReadHostCall(args);
      let resultBytes = 0;
      try {{
        resultBytes = JSON.stringify(result).length;
      }} catch (_e) {{}}
      audit({{type:"worker_host_call", tool:"github.commit.read", outcome:"ok", policy:"allowed", arg_bytes:argBytes, result_bytes:resultBytes, elapsed_ms:Date.now() - started, adapter:"agent_named_network_persistent_cache"}});
      return result;
    }} catch (e) {{
      const message = safeMessage(e);
      const errorKind = message.indexOf("cache stale") >= 0 ? "cache_stale" : (message.indexOf("cache miss") >= 0 || message.indexOf("live async host-call required") >= 0 ? "async_live_required" : (message.indexOf("denied") >= 0 ? "denied" : (message.indexOf("invalid") >= 0 ? "invalid_cache" : "handler_error")));
      audit({{type:"worker_host_call", tool:"github.commit.read", outcome:"error", policy:"allowed", arg_bytes:argBytes, elapsed_ms:Date.now() - started, error_kind:errorKind, error:message, adapter:"agent_named_network_persistent_cache"}});
      throw e;
    }}
  }});
}}
if (allowedTools.indexOf("github.workflow.run.read") >= 0) {{
  __cruft_registerHostCall("github.workflow.run.read", function(args) {{
    const started = Date.now();
    let argBytes = 0;
    try {{
      argBytes = JSON.stringify(args).length;
    }} catch (_e) {{}}
    try {{
      const result = runWorkerGithubWorkflowRunReadHostCall(args);
      let resultBytes = 0;
      try {{
        resultBytes = JSON.stringify(result).length;
      }} catch (_e) {{}}
      audit({{type:"worker_host_call", tool:"github.workflow.run.read", outcome:"ok", policy:"allowed", arg_bytes:argBytes, result_bytes:resultBytes, elapsed_ms:Date.now() - started, adapter:"agent_named_network_persistent_cache"}});
      return result;
    }} catch (e) {{
      const message = safeMessage(e);
      const errorKind = message.indexOf("cache stale") >= 0 ? "cache_stale" : (message.indexOf("cache miss") >= 0 || message.indexOf("live async host-call required") >= 0 ? "async_live_required" : (message.indexOf("denied") >= 0 ? "denied" : (message.indexOf("invalid") >= 0 ? "invalid_cache" : "handler_error")));
      audit({{type:"worker_host_call", tool:"github.workflow.run.read", outcome:"error", policy:"allowed", arg_bytes:argBytes, elapsed_ms:Date.now() - started, error_kind:errorKind, error:message, adapter:"agent_named_network_persistent_cache"}});
      throw e;
    }}
  }});
}}
if (allowedTools.indexOf("github.workflow.jobs.list") >= 0) {{
  __cruft_registerHostCall("github.workflow.jobs.list", function(args) {{
    const started = Date.now();
    let argBytes = 0;
    try {{
      argBytes = JSON.stringify(args).length;
    }} catch (_e) {{}}
    try {{
      const result = runWorkerGithubWorkflowJobsListHostCall(args);
      let resultBytes = 0;
      try {{
        resultBytes = JSON.stringify(result).length;
      }} catch (_e) {{}}
      audit({{type:"worker_host_call", tool:"github.workflow.jobs.list", outcome:"ok", policy:"allowed", arg_bytes:argBytes, result_bytes:resultBytes, elapsed_ms:Date.now() - started, adapter:"agent_named_network_persistent_cache"}});
      return result;
    }} catch (e) {{
      const message = safeMessage(e);
      const errorKind = message.indexOf("cache stale") >= 0 ? "cache_stale" : (message.indexOf("cache miss") >= 0 || message.indexOf("live async host-call required") >= 0 ? "async_live_required" : (message.indexOf("denied") >= 0 ? "denied" : (message.indexOf("invalid") >= 0 ? "invalid_cache" : "handler_error")));
      audit({{type:"worker_host_call", tool:"github.workflow.jobs.list", outcome:"error", policy:"allowed", arg_bytes:argBytes, elapsed_ms:Date.now() - started, error_kind:errorKind, error:message, adapter:"agent_named_network_persistent_cache"}});
      throw e;
    }}
  }});
}}
if (allowedTools.indexOf("github.check.runs.list") >= 0) {{
  __cruft_registerHostCall("github.check.runs.list", function(args) {{
    const started = Date.now();
    let argBytes = 0;
    try {{
      argBytes = JSON.stringify(args).length;
    }} catch (_e) {{}}
    try {{
      const result = runWorkerGithubCheckRunsListHostCall(args);
      let resultBytes = 0;
      try {{
        resultBytes = JSON.stringify(result).length;
      }} catch (_e) {{}}
      audit({{type:"worker_host_call", tool:"github.check.runs.list", outcome:"ok", policy:"allowed", arg_bytes:argBytes, result_bytes:resultBytes, elapsed_ms:Date.now() - started, adapter:"agent_named_network_persistent_cache"}});
      return result;
    }} catch (e) {{
      const message = safeMessage(e);
      const errorKind = message.indexOf("cache stale") >= 0 ? "cache_stale" : (message.indexOf("cache miss") >= 0 || message.indexOf("live async host-call required") >= 0 ? "async_live_required" : (message.indexOf("denied") >= 0 ? "denied" : (message.indexOf("invalid") >= 0 ? "invalid_cache" : "handler_error")));
      audit({{type:"worker_host_call", tool:"github.check.runs.list", outcome:"error", policy:"allowed", arg_bytes:argBytes, elapsed_ms:Date.now() - started, error_kind:errorKind, error:message, adapter:"agent_named_network_persistent_cache"}});
      throw e;
    }}
  }});
}}
if (allowedTools.indexOf("github.repo.read") >= 0) {{
  __cruft_registerHostCall("github.repo.read", function(args) {{
    const started = Date.now();
    let argBytes = 0;
    try {{
      argBytes = JSON.stringify(args).length;
    }} catch (_e) {{}}
    try {{
      const result = runWorkerGithubRepoReadHostCall(args);
      let resultBytes = 0;
      try {{
        resultBytes = JSON.stringify(result).length;
      }} catch (_e) {{}}
      audit({{type:"worker_host_call", tool:"github.repo.read", outcome:"ok", policy:"allowed", arg_bytes:argBytes, result_bytes:resultBytes, elapsed_ms:Date.now() - started, adapter:"agent_named_network_persistent_cache"}});
      return result;
    }} catch (e) {{
      const message = safeMessage(e);
      const errorKind = message.indexOf("cache stale") >= 0 ? "cache_stale" : (message.indexOf("cache miss") >= 0 || message.indexOf("live async host-call required") >= 0 ? "async_live_required" : (message.indexOf("denied") >= 0 ? "denied" : (message.indexOf("invalid") >= 0 ? "invalid_cache" : "handler_error")));
      audit({{type:"worker_host_call", tool:"github.repo.read", outcome:"error", policy:"allowed", arg_bytes:argBytes, elapsed_ms:Date.now() - started, error_kind:errorKind, error:message, adapter:"agent_named_network_persistent_cache"}});
      throw e;
    }}
  }});
}}
if (allowedTools.indexOf("model.call") >= 0 && modelFixture === null) {{
  __cruft_registerHostCall("model.call", function(args) {{
    const started = Date.now();
    let argBytes = 0;
    try {{
      argBytes = JSON.stringify(args).length;
    }} catch (_e) {{}}
    try {{
      const result = runWorkerModelCallProviderHostCall(args);
      let resultBytes = 0;
      try {{
        resultBytes = JSON.stringify(result).length;
      }} catch (_e) {{}}
      audit({{type:"worker_host_call", tool:"model.call", outcome:"ok", policy:"allowed", arg_bytes:argBytes, result_bytes:resultBytes, elapsed_ms:Date.now() - started, adapter:"agent_model_provider_test", credential_mode:"host_env_bearer", credential_env:modelApiKeyEnv}});
      return result;
    }} catch (e) {{
      const message = safeMessage(e);
      const taxonomy = workerModelProviderErrorTaxonomy(e);
      const errorKind = taxonomy.error_kind;
      audit({{type:"worker_host_call", tool:"model.call", outcome:"error", policy:"allowed", arg_bytes:argBytes, elapsed_ms:Date.now() - started, error_kind:errorKind, retryable:taxonomy.retryable, error:message, adapter:"agent_model_provider_test", credential_mode:"host_env_bearer", credential_env:modelApiKeyEnv}});
      throw e;
    }}
  }});
}}
const worker = new Compartment({{ worker: true, onMessageSource: workerSource }});
function artifactHostWrite(record) {{
  const relative = String(record.path || "");
  if (relative.length === 0 || relative.charAt(0) === "/" || relative.indexOf("\\") >= 0 || relative.split("/").indexOf("..") >= 0) {{
    audit({{type:"tool_denial", tool:"writeArtifact", policy:"denied", reason:"artifact_path_not_admitted", path:relative}});
    throw new Error("agent tool denied: writeArtifact path not admitted");
  }}
  const root = String(record.root || (fsWriteRoots[0] && fsWriteRoots[0].root) || "");
  const target = root + "/" + relative;
  if (fs.existsSync(target)) {{
    audit({{type:"tool_denial", tool:"writeArtifact", policy:"denied", reason:"artifact_overwrite_denied", path:relative}});
    throw new Error("agent tool denied: writeArtifact overwrite denied");
  }}
  const dir = target.split("/").slice(0, -1).join("/");
  if (dir.length > 0) fs.mkdirSync(dir, {{recursive:true}});
  const tmp = target + ".cruft-tmp-" + Date.now() + "-" + Math.floor(Math.random() * 1000000);
  fs.writeFileSync(tmp, String(record.content || ""));
  fs.renameSync(tmp, target);
  const result = {{path:relative, bytes:record.bytes, hash:record.hash}};
  writtenArtifacts.push(result);
  audit({{type:"tool_result", tool:"writeArtifact", result, result_bytes:JSON.stringify(result).length, path:relative, bytes:record.bytes, hash:record.hash}});
}}
try {{
  worker.request({{
    agentSource,
    entryModule,
    context,
    state:initialState,
    tools:allowedTools,
    modules:admittedModules,
    importHooks:admittedHooks,
    fsReadCaps,
    fsWriteRoots,
    osvFixture,
    modelFixture,
    processCommands,
    processCwds,
    processEnv,
    expectedEventSchemas,
    maxEvents,
    maxEventBytes,
    maxStateBytes,
    maxToolArgBytes,
    maxToolResultBytes,
    toolTimeoutMs:{tool_timeout_ms},
    timeoutMs:{timeout_ms},
    redactFields:{redact_fields},
    secretScopes,
    approvalRequiredTools,
    maxSteps
  }}).then(function(result) {{
    for (let i = 0; i < result.records.length; i++) {{
      if (result.records[i].type === "artifact_write_request") artifactHostWrite(result.records[i]);
      else audit(result.records[i]);
    }}
    audit({{type:"state_snapshot", state:result.state, state_bytes:result.state_bytes}});
    audit({{type:"worker_forwarding", status:result.status, events:result.emitted_events, event_bytes:result.emitted_event_bytes}});
    if (result.status !== "ok") {{
      audit({{type:result.reason || "exception", status:"error", message:result.message || "agent worker error"}});
      audit({{type:"run_end", run_id:runId, status:"error", reason:result.reason || "exception"}});
      throw new Error(result.message || "agent worker error");
    }}
    if (sessionPath !== null) {{
      const nextTurn = sessionTurn + 1;
      fs.writeFileSync(sessionPath, JSON.stringify({{type:"agent_session", turn_id:nextTurn, state:result.state}}));
      audit({{type:"session_save", run_id:runId, path:sessionPath, turn_id:nextTurn, state_bytes:result.state_bytes}});
    }}
    audit({{type:"availability_check", sync_timeout:"enforced_by_child_process_wall_supervisor", tenant_timeout_catchability:"uncatchable_gate", microtask_budget:"enforced", max_microtasks:{max_microtasks}, pending_promise_disposition:"detached_at_worker_turn_end", async_tool_timeout:"enforced_for_worker_promise_tools", tool_timeout_ms:{tool_timeout_ms}, step_budget:maxSteps === null ? "available_with_--max-steps" : "enforced", max_steps:maxSteps}});
    emitRunArtifactManifest("ok", null);
    audit({{type:"run_end", run_id:runId, status:"ok"}});
  }}, function(e) {{
    const message = safeMessage(e);
    const kind = message.indexOf("agent event schema validation failed:") >= 0 ? "schema_validation_failed" : (message.indexOf("step budget") >= 0 ? "step_budget_exceeded" : (message.indexOf("timeout") >= 0 ? "timeout" : (message.indexOf("budget exceeded") >= 0 ? "budget_exceeded" : (message.indexOf("agent event must be a JSON object") >= 0 ? "invalid_event" : "exception"))));
    audit({{type:kind, status:"error", message}});
    emitRunArtifactManifest("error", kind);
    audit({{type:"run_end", run_id:runId, status:"error", reason:kind}});
    throw e;
  }});
}} catch (e) {{
  const message = safeMessage(e);
  audit({{type:"exception", status:"error", message}});
  emitRunArtifactManifest("error", "exception");
  audit({{type:"run_end", run_id:runId, status:"error", reason:"exception"}});
  throw e;
}}
"#,
        audit = json_string_literal(config.audit_log),
        run_id = json_string_literal(config.run_id),
        agent_path = json_string_literal(config.source_path),
        agent_source = json_string_literal(config.source),
        context = json_string_literal(config.context_json),
        state = json_string_literal(config.state_json),
        session_file = session_file_js,
        entry_module = entry_module_js,
        tools = tools_js,
        modules = modules_js,
        import_hooks = import_hooks_js,
        fs_read_caps = fs_read_caps_js,
        fs_write_roots = fs_write_roots_js,
        osv_fixture = osv_fixture_js,
        model_fixture = model_fixture_js,
        model_provider = model_provider_js,
        model_api_key_env = model_api_key_env_js,
        model_api_key_value = model_api_key_value_js,
        secret_scopes = secret_scopes_js,
        approval_required_tools = approval_required_tools_js,
        approved_tools = approved_tools_js,
        approval_log = approval_log_js,
        approval_max_age_ms = approval_max_age_ms_js,
        process_commands = process_commands_js,
        process_cwds = process_cwds_js,
        process_env = process_env_js,
        named_network_cache_dir = named_network_cache_dir_js,
        named_network_cache_mode = named_network_cache_mode_js,
        named_network_cache_max_age_ms = named_network_cache_max_age_ms_js,
        named_network_cache_max_entries = named_network_cache_max_entries_js,
        expected_event_schemas = expected_event_schemas_js,
        package_imports = package_imports_js,
        max_events = config.max_events,
        max_event_bytes = config.max_event_bytes,
        max_state_bytes = config.max_state_bytes,
        max_tool_arg_bytes = config.max_tool_arg_bytes,
        max_tool_result_bytes = config.max_tool_result_bytes,
        tool_timeout_ms = config.tool_timeout_ms,
        max_steps = max_steps_js,
        memory_rss = memory_rss_js,
        max_rss_mb = max_rss_mb_js,
        redact_fields = redact_fields_js,
        timeout_ms = config.timeout_ms,
        max_microtasks = config.max_microtasks,
        worker_source = json_string_literal(worker_source)
    );
    let harness_path = std::env::temp_dir().join(format!(
        "cruft-agent-worker-harness-{}-{}.js",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    if let Err(e) = std::fs::write(&harness_path, harness) {
        eprintln!("cruft agent run: cannot create worker harness: {e}");
        return ExitCode::from(74);
    }
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("cruft agent run: cannot locate current executable: {e}");
            let _ = std::fs::remove_file(&harness_path);
            return ExitCode::from(70);
        }
    };
    let output = run_agent_harness_child(
        exe,
        &harness_path,
        config.max_microtasks,
        config.max_rss_mb,
        Some(config.timeout_ms),
        config.audit_log,
    );
    let _ = std::fs::remove_file(&harness_path);
    match output {
        Ok(output) if output.status.success() => {
            let _ = std::io::stdout().write_all(&output.stdout);
            let _ = std::io::stderr().write_all(&output.stderr);
            ExitCode::SUCCESS
        }
        Ok(output) => {
            let _ = std::io::stdout().write_all(&output.stdout);
            let _ = std::io::stderr().write_all(&output.stderr);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let memory_rss_already_recorded = std::fs::read_to_string(config.audit_log)
                .map(|s| s.contains("\"reason\":\"memory_rss_exceeded\""))
                .unwrap_or(false);
            let timeout_already_recorded = std::fs::read_to_string(config.audit_log)
                .map(|s| s.contains("\"reason\":\"timeout\""))
                .unwrap_or(false);
            let reason = if memory_rss_already_recorded {
                "memory_rss_exceeded"
            } else if timeout_already_recorded {
                "timeout"
            } else if stderr.contains("microtask budget exceeded") {
                "microtask_budget_exceeded"
            } else if stderr.contains("step budget") {
                "step_budget_exceeded"
            } else if stderr.contains("Interrupted") || stderr.contains("timeout") {
                "timeout"
            } else {
                "child_runtime_error"
            };
            let message = stderr.lines().last().unwrap_or(reason);
            if reason != "memory_rss_exceeded" && !timeout_already_recorded {
                append_agent_post_turn_failure(config.audit_log, reason, message);
            }
            ExitCode::from(output.status.code().unwrap_or(70) as u8)
        }
        Err(e) => {
            eprintln!("cruft agent run: cannot execute worker runtime harness: {e}");
            ExitCode::from(70)
        }
    }
}

pub(crate) fn run_agent_run_subcommand(args: &[String]) -> ExitCode {
    let mut expanded_args = vec![args[0].clone()];
    let mut expand_i = 1;
    while expand_i < args.len() {
        let arg = &args[expand_i];
        if arg == "--policy" {
            expand_i += 1;
            let Some(path) = args.get(expand_i) else {
                eprintln!("cruft agent run: --policy requires an argument");
                return ExitCode::from(64);
            };
            match agent_policy_expand_run_args(path) {
                Ok(mut policy_args) => expanded_args.append(&mut policy_args),
                Err(e) => {
                    eprintln!("cruft agent run: {e}");
                    return ExitCode::from(65);
                }
            }
        } else if let Some(path) = arg.strip_prefix("--policy=") {
            match agent_policy_expand_run_args(path) {
                Ok(mut policy_args) => expanded_args.append(&mut policy_args),
                Err(e) => {
                    eprintln!("cruft agent run: {e}");
                    return ExitCode::from(65);
                }
            }
        } else if arg == "--project" {
            expand_i += 1;
            let Some(project_dir) = args.get(expand_i) else {
                eprintln!("cruft agent run: --project requires an argument");
                return ExitCode::from(64);
            };
            let policy_path = agent_project_policy_path(project_dir);
            match agent_policy_expand_run_args(&policy_path) {
                Ok(mut policy_args) => expanded_args.append(&mut policy_args),
                Err(e) => {
                    eprintln!("cruft agent run: {e}");
                    return ExitCode::from(65);
                }
            }
        } else if let Some(project_dir) = arg.strip_prefix("--project=") {
            let policy_path = agent_project_policy_path(project_dir);
            match agent_policy_expand_run_args(&policy_path) {
                Ok(mut policy_args) => expanded_args.append(&mut policy_args),
                Err(e) => {
                    eprintln!("cruft agent run: {e}");
                    return ExitCode::from(65);
                }
            }
        } else {
            expanded_args.push(arg.clone());
        }
        expand_i += 1;
    }
    let args = expanded_args;
    let mut source_path: Option<String> = None;
    let mut timeout_ms: u64 = 250;
    let mut audit_log: Option<String> = None;
    let mut run_id: Option<String> = None;
    let mut context_json = "{}".to_string();
    let mut state_json = "{}".to_string();
    let mut session_file: Option<String> = None;
    let mut tools: Vec<String> = Vec::new();
    let mut approval_required_tools: Vec<String> = Vec::new();
    let mut approved_tools: Vec<String> = Vec::new();
    let mut approval_log: Option<String> = None;
    let mut approval_max_age_ms: Option<u64> = None;
    let mut max_events: u64 = 128;
    let mut max_event_bytes: u64 = 65_536;
    let mut max_tool_arg_bytes: u64 = 65_536;
    let mut max_tool_result_bytes: u64 = 65_536;
    let mut process_output_stream_chunk_bytes: u64 = 1024;
    let mut tool_timeout_ms: u64 = 250;
    let mut max_state_bytes: u64 = 65_536;
    let mut max_microtasks: u64 = 10_000;
    let mut max_steps: Option<u64> = None;
    let mut max_rss_mb: Option<u64> = None;
    let mut module_specs: Vec<(String, String)> = Vec::new();
    let mut package_specs: Vec<(String, String)> = Vec::new();
    let mut package_integrities: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut import_hook_specs: Vec<(String, String)> = Vec::new();
    let mut import_hook_integrities: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut fs_read_specs: Vec<(String, String)> = Vec::new();
    let mut fs_write_specs: Vec<String> = Vec::new();
    let mut osv_fixture_path: Option<String> = None;
    let mut model_fixture_path: Option<String> = None;
    let mut model_provider: Option<String> = None;
    let mut model_api_key_env: Option<String> = None;
    let mut named_network_cache_dir: Option<String> = None;
    let mut named_network_cache_mode = "read-through".to_string();
    let mut named_network_cache_max_age_ms: Option<u64> = None;
    let mut named_network_cache_max_entries: Option<u64> = None;
    let mut named_network_retry_attempts: u64 = 0;
    let mut github_token_env: Option<String> = None;
    let mut secret_scopes: Vec<(String, String)> = Vec::new();
    let mut process_command_specs: Vec<(String, String)> = Vec::new();
    let mut process_cwd_specs: Vec<String> = Vec::new();
    let mut process_env_specs: Vec<(String, String)> = Vec::new();
    let mut expected_event_specs: Vec<(String, String)> = Vec::new();
    let mut redact_fields: Vec<String> = Vec::new();
    let mut fs_read_include_patterns: Vec<String> = Vec::new();
    let mut fs_read_exclude_patterns: Vec<String> = Vec::new();
    let mut entry_module: Option<String> = None;
    let mut scheduler_await_out: Option<String> = None;
    let mut worker_requested = false;
    let mut i = 1;
    while i < args.len() {
        let a = args[i].as_str();
        if a == "--worker" {
            worker_requested = true;
        } else if a == "--scheduler-await-out" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --scheduler-await-out requires a path");
                return ExitCode::from(64);
            };
            scheduler_await_out = Some(v.clone());
        } else if let Some(v) = a.strip_prefix("--scheduler-await-out=") {
            scheduler_await_out = Some(v.to_string());
        } else if a == "--timeout-ms" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --timeout-ms requires an argument");
                return ExitCode::from(64);
            };
            timeout_ms = match v.parse::<u64>() {
                Ok(n) if n > 0 => n,
                _ => {
                    eprintln!("cruft agent run: --timeout-ms must be a positive integer");
                    return ExitCode::from(64);
                }
            };
        } else if let Some(v) = a.strip_prefix("--timeout-ms=") {
            timeout_ms = match v.parse::<u64>() {
                Ok(n) if n > 0 => n,
                _ => {
                    eprintln!("cruft agent run: --timeout-ms must be a positive integer");
                    return ExitCode::from(64);
                }
            };
        } else if a == "--audit-log" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --audit-log requires an argument");
                return ExitCode::from(64);
            };
            audit_log = Some(v.clone());
        } else if let Some(v) = a.strip_prefix("--audit-log=") {
            audit_log = Some(v.to_string());
        } else if a == "--run-id" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --run-id requires an argument");
                return ExitCode::from(64);
            };
            if !agent_validate_run_id(v) {
                eprintln!("cruft agent run: --run-id must be 1-128 chars using ASCII letters, digits, dot, underscore, colon, slash, or hyphen");
                return ExitCode::from(64);
            }
            run_id = Some(v.clone());
        } else if let Some(v) = a.strip_prefix("--run-id=") {
            if !agent_validate_run_id(v) {
                eprintln!("cruft agent run: --run-id must be 1-128 chars using ASCII letters, digits, dot, underscore, colon, slash, or hyphen");
                return ExitCode::from(64);
            }
            run_id = Some(v.to_string());
        } else if a == "--context-json" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --context-json requires an argument");
                return ExitCode::from(64);
            };
            context_json = v.clone();
        } else if let Some(v) = a.strip_prefix("--context-json=") {
            context_json = v.to_string();
        } else if a == "--state-json" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --state-json requires an argument");
                return ExitCode::from(64);
            };
            state_json = v.clone();
        } else if let Some(v) = a.strip_prefix("--state-json=") {
            state_json = v.to_string();
        } else if a == "--session-file" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --session-file requires an argument");
                return ExitCode::from(64);
            };
            session_file = Some(v.clone());
        } else if let Some(v) = a.strip_prefix("--session-file=") {
            session_file = Some(v.to_string());
        } else if a == "--max-state-bytes" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --max-state-bytes requires an argument");
                return ExitCode::from(64);
            };
            max_state_bytes = match v.parse::<u64>() {
                Ok(n) if n > 0 => n,
                _ => {
                    eprintln!("cruft agent run: --max-state-bytes must be a positive integer");
                    return ExitCode::from(64);
                }
            };
        } else if let Some(v) = a.strip_prefix("--max-state-bytes=") {
            max_state_bytes = match v.parse::<u64>() {
                Ok(n) if n > 0 => n,
                _ => {
                    eprintln!("cruft agent run: --max-state-bytes must be a positive integer");
                    return ExitCode::from(64);
                }
            };
        } else if a == "--max-events" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --max-events requires an argument");
                return ExitCode::from(64);
            };
            max_events = match v.parse::<u64>() {
                Ok(n) if n > 0 => n,
                _ => {
                    eprintln!("cruft agent run: --max-events must be a positive integer");
                    return ExitCode::from(64);
                }
            };
        } else if let Some(v) = a.strip_prefix("--max-events=") {
            max_events = match v.parse::<u64>() {
                Ok(n) if n > 0 => n,
                _ => {
                    eprintln!("cruft agent run: --max-events must be a positive integer");
                    return ExitCode::from(64);
                }
            };
        } else if a == "--max-event-bytes" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --max-event-bytes requires an argument");
                return ExitCode::from(64);
            };
            max_event_bytes = match v.parse::<u64>() {
                Ok(n) if n > 0 => n,
                _ => {
                    eprintln!("cruft agent run: --max-event-bytes must be a positive integer");
                    return ExitCode::from(64);
                }
            };
        } else if let Some(v) = a.strip_prefix("--max-event-bytes=") {
            max_event_bytes = match v.parse::<u64>() {
                Ok(n) if n > 0 => n,
                _ => {
                    eprintln!("cruft agent run: --max-event-bytes must be a positive integer");
                    return ExitCode::from(64);
                }
            };
        } else if a == "--max-tool-arg-bytes" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --max-tool-arg-bytes requires an argument");
                return ExitCode::from(64);
            };
            max_tool_arg_bytes = match v.parse::<u64>() {
                Ok(n) if n > 0 => n,
                _ => {
                    eprintln!("cruft agent run: --max-tool-arg-bytes must be a positive integer");
                    return ExitCode::from(64);
                }
            };
        } else if let Some(v) = a.strip_prefix("--max-tool-arg-bytes=") {
            max_tool_arg_bytes = match v.parse::<u64>() {
                Ok(n) if n > 0 => n,
                _ => {
                    eprintln!("cruft agent run: --max-tool-arg-bytes must be a positive integer");
                    return ExitCode::from(64);
                }
            };
        } else if a == "--max-tool-result-bytes" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --max-tool-result-bytes requires an argument");
                return ExitCode::from(64);
            };
            max_tool_result_bytes = match v.parse::<u64>() {
                Ok(n) if n > 0 => n,
                _ => {
                    eprintln!(
                        "cruft agent run: --max-tool-result-bytes must be a positive integer"
                    );
                    return ExitCode::from(64);
                }
            };
        } else if let Some(v) = a.strip_prefix("--max-tool-result-bytes=") {
            max_tool_result_bytes = match v.parse::<u64>() {
                Ok(n) if n > 0 => n,
                _ => {
                    eprintln!(
                        "cruft agent run: --max-tool-result-bytes must be a positive integer"
                    );
                    return ExitCode::from(64);
                }
            };
        } else if a == "--process-output-stream-chunk-bytes" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!(
                    "cruft agent run: --process-output-stream-chunk-bytes requires an argument"
                );
                return ExitCode::from(64);
            };
            process_output_stream_chunk_bytes = match v.parse::<u64>() {
                Ok(n) if (1..=8192).contains(&n) => n,
                _ => {
                    eprintln!(
                        "cruft agent run: --process-output-stream-chunk-bytes must be an integer from 1 to 8192"
                    );
                    return ExitCode::from(64);
                }
            };
        } else if let Some(v) = a.strip_prefix("--process-output-stream-chunk-bytes=") {
            process_output_stream_chunk_bytes = match v.parse::<u64>() {
                Ok(n) if (1..=8192).contains(&n) => n,
                _ => {
                    eprintln!(
                        "cruft agent run: --process-output-stream-chunk-bytes must be an integer from 1 to 8192"
                    );
                    return ExitCode::from(64);
                }
            };
        } else if a == "--tool-timeout-ms" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --tool-timeout-ms requires an argument");
                return ExitCode::from(64);
            };
            tool_timeout_ms = match v.parse::<u64>() {
                Ok(n) if n > 0 => n,
                _ => {
                    eprintln!("cruft agent run: --tool-timeout-ms must be a positive integer");
                    return ExitCode::from(64);
                }
            };
        } else if let Some(v) = a.strip_prefix("--tool-timeout-ms=") {
            tool_timeout_ms = match v.parse::<u64>() {
                Ok(n) if n > 0 => n,
                _ => {
                    eprintln!("cruft agent run: --tool-timeout-ms must be a positive integer");
                    return ExitCode::from(64);
                }
            };
        } else if a == "--max-microtasks" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --max-microtasks requires an argument");
                return ExitCode::from(64);
            };
            max_microtasks = match v.parse::<u64>() {
                Ok(n) if n > 0 => n,
                _ => {
                    eprintln!("cruft agent run: --max-microtasks must be a positive integer");
                    return ExitCode::from(64);
                }
            };
        } else if let Some(v) = a.strip_prefix("--max-microtasks=") {
            max_microtasks = match v.parse::<u64>() {
                Ok(n) if n > 0 => n,
                _ => {
                    eprintln!("cruft agent run: --max-microtasks must be a positive integer");
                    return ExitCode::from(64);
                }
            };
        } else if a == "--max-steps" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --max-steps requires an argument");
                return ExitCode::from(64);
            };
            max_steps = match v.parse::<u64>() {
                Ok(n) if n > 0 => Some(n),
                _ => {
                    eprintln!("cruft agent run: --max-steps must be a positive integer");
                    return ExitCode::from(64);
                }
            };
        } else if let Some(v) = a.strip_prefix("--max-steps=") {
            max_steps = match v.parse::<u64>() {
                Ok(n) if n > 0 => Some(n),
                _ => {
                    eprintln!("cruft agent run: --max-steps must be a positive integer");
                    return ExitCode::from(64);
                }
            };
        } else if a == "--max-rss-mb" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --max-rss-mb requires an argument");
                return ExitCode::from(64);
            };
            max_rss_mb = match v.parse::<u64>() {
                Ok(n) if n > 0 => Some(n),
                _ => {
                    eprintln!("cruft agent run: --max-rss-mb must be a positive integer");
                    return ExitCode::from(64);
                }
            };
        } else if let Some(v) = a.strip_prefix("--max-rss-mb=") {
            max_rss_mb = match v.parse::<u64>() {
                Ok(n) if n > 0 => Some(n),
                _ => {
                    eprintln!("cruft agent run: --max-rss-mb must be a positive integer");
                    return ExitCode::from(64);
                }
            };
        } else if a == "--redact-field" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --redact-field requires a field name");
                return ExitCode::from(64);
            };
            if v.is_empty() {
                eprintln!("cruft agent run: --redact-field requires a non-empty field name");
                return ExitCode::from(64);
            }
            redact_fields.push(v.clone());
        } else if let Some(v) = a.strip_prefix("--redact-field=") {
            if v.is_empty() {
                eprintln!("cruft agent run: --redact-field requires a non-empty field name");
                return ExitCode::from(64);
            }
            redact_fields.push(v.to_string());
        } else if a == "--fs-read" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --fs-read requires a path");
                return ExitCode::from(64);
            };
            if v.is_empty() {
                eprintln!("cruft agent run: --fs-read requires a non-empty path");
                return ExitCode::from(64);
            }
            fs_read_specs.push((v.clone(), v.clone()));
        } else if let Some(v) = a.strip_prefix("--fs-read=") {
            if v.is_empty() {
                eprintln!("cruft agent run: --fs-read requires a non-empty path");
                return ExitCode::from(64);
            }
            fs_read_specs.push((v.to_string(), v.to_string()));
        } else if a == "--fs-read-include" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --fs-read-include requires a pattern");
                return ExitCode::from(64);
            };
            if !agent_validate_fs_read_pattern(v) {
                eprintln!("cruft agent run: --fs-read-include requires a relative glob pattern");
                return ExitCode::from(64);
            }
            fs_read_include_patterns.push(v.clone());
        } else if let Some(v) = a.strip_prefix("--fs-read-include=") {
            if !agent_validate_fs_read_pattern(v) {
                eprintln!("cruft agent run: --fs-read-include requires a relative glob pattern");
                return ExitCode::from(64);
            }
            fs_read_include_patterns.push(v.to_string());
        } else if a == "--fs-read-exclude" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --fs-read-exclude requires a pattern");
                return ExitCode::from(64);
            };
            if !agent_validate_fs_read_pattern(v) {
                eprintln!("cruft agent run: --fs-read-exclude requires a relative glob pattern");
                return ExitCode::from(64);
            }
            fs_read_exclude_patterns.push(v.clone());
        } else if let Some(v) = a.strip_prefix("--fs-read-exclude=") {
            if !agent_validate_fs_read_pattern(v) {
                eprintln!("cruft agent run: --fs-read-exclude requires a relative glob pattern");
                return ExitCode::from(64);
            }
            fs_read_exclude_patterns.push(v.to_string());
        } else if a == "--fs-write" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --fs-write requires a path");
                return ExitCode::from(64);
            };
            if v.is_empty() {
                eprintln!("cruft agent run: --fs-write requires a non-empty path");
                return ExitCode::from(64);
            }
            fs_write_specs.push(v.clone());
        } else if let Some(v) = a.strip_prefix("--fs-write=") {
            if v.is_empty() {
                eprintln!("cruft agent run: --fs-write requires a non-empty path");
                return ExitCode::from(64);
            }
            fs_write_specs.push(v.to_string());
        } else if a == "--osv-fixture" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --osv-fixture requires a path");
                return ExitCode::from(64);
            };
            if v.is_empty() {
                eprintln!("cruft agent run: --osv-fixture requires a non-empty path");
                return ExitCode::from(64);
            }
            osv_fixture_path = Some(v.clone());
        } else if let Some(v) = a.strip_prefix("--osv-fixture=") {
            if v.is_empty() {
                eprintln!("cruft agent run: --osv-fixture requires a non-empty path");
                return ExitCode::from(64);
            }
            osv_fixture_path = Some(v.to_string());
        } else if a == "--model-fixture" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --model-fixture requires a path");
                return ExitCode::from(64);
            };
            if v.is_empty() {
                eprintln!("cruft agent run: --model-fixture requires a non-empty path");
                return ExitCode::from(64);
            }
            model_fixture_path = Some(v.clone());
        } else if let Some(v) = a.strip_prefix("--model-fixture=") {
            if v.is_empty() {
                eprintln!("cruft agent run: --model-fixture requires a non-empty path");
                return ExitCode::from(64);
            }
            model_fixture_path = Some(v.to_string());
        } else if a == "--model-provider" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --model-provider requires an argument");
                return ExitCode::from(64);
            };
            if v != "openai.responses" {
                eprintln!("cruft agent run: --model-provider must be openai.responses");
                return ExitCode::from(64);
            }
            model_provider = Some(v.clone());
        } else if let Some(v) = a.strip_prefix("--model-provider=") {
            if v != "openai.responses" {
                eprintln!("cruft agent run: --model-provider must be openai.responses");
                return ExitCode::from(64);
            }
            model_provider = Some(v.to_string());
        } else if a == "--model-api-key-env" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --model-api-key-env requires an argument");
                return ExitCode::from(64);
            };
            if !agent_env_key_is_valid(v) {
                eprintln!("cruft agent run: --model-api-key-env requires a non-empty env var name");
                return ExitCode::from(64);
            }
            model_api_key_env = Some(v.clone());
        } else if let Some(v) = a.strip_prefix("--model-api-key-env=") {
            if !agent_env_key_is_valid(v) {
                eprintln!("cruft agent run: --model-api-key-env requires a non-empty env var name");
                return ExitCode::from(64);
            }
            model_api_key_env = Some(v.to_string());
        } else if a == "--secret" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --secret requires <tool=ENV>");
                return ExitCode::from(64);
            };
            match agent_secret_scope_parse(v) {
                Ok(scope) => secret_scopes.push(scope),
                Err(e) => {
                    eprintln!("cruft agent run: {e}");
                    return ExitCode::from(64);
                }
            }
        } else if let Some(v) = a.strip_prefix("--secret=") {
            match agent_secret_scope_parse(v) {
                Ok(scope) => secret_scopes.push(scope),
                Err(e) => {
                    eprintln!("cruft agent run: {e}");
                    return ExitCode::from(64);
                }
            }
        } else if a == "--named-network-cache-dir" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --named-network-cache-dir requires a path");
                return ExitCode::from(64);
            };
            if v.is_empty() {
                eprintln!("cruft agent run: --named-network-cache-dir requires a non-empty path");
                return ExitCode::from(64);
            }
            named_network_cache_dir = Some(v.clone());
        } else if let Some(v) = a.strip_prefix("--named-network-cache-dir=") {
            if v.is_empty() {
                eprintln!("cruft agent run: --named-network-cache-dir requires a non-empty path");
                return ExitCode::from(64);
            }
            named_network_cache_dir = Some(v.to_string());
        } else if a == "--named-network-cache-mode" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --named-network-cache-mode requires an argument");
                return ExitCode::from(64);
            };
            if v != "read-through" && v != "offline" {
                eprintln!(
                    "cruft agent run: --named-network-cache-mode must be read-through or offline"
                );
                return ExitCode::from(64);
            }
            named_network_cache_mode = v.clone();
        } else if let Some(v) = a.strip_prefix("--named-network-cache-mode=") {
            if v != "read-through" && v != "offline" {
                eprintln!(
                    "cruft agent run: --named-network-cache-mode must be read-through or offline"
                );
                return ExitCode::from(64);
            }
            named_network_cache_mode = v.to_string();
        } else if a == "--named-network-cache-max-age-ms" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --named-network-cache-max-age-ms requires an argument");
                return ExitCode::from(64);
            };
            named_network_cache_max_age_ms = match v.parse::<u64>() {
                Ok(n) if n > 0 => Some(n),
                _ => {
                    eprintln!(
                        "cruft agent run: --named-network-cache-max-age-ms must be a positive integer"
                    );
                    return ExitCode::from(64);
                }
            };
        } else if let Some(v) = a.strip_prefix("--named-network-cache-max-age-ms=") {
            named_network_cache_max_age_ms = match v.parse::<u64>() {
                Ok(n) if n > 0 => Some(n),
                _ => {
                    eprintln!(
                        "cruft agent run: --named-network-cache-max-age-ms must be a positive integer"
                    );
                    return ExitCode::from(64);
                }
            };
        } else if a == "--named-network-cache-max-entries" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!(
                    "cruft agent run: --named-network-cache-max-entries requires an argument"
                );
                return ExitCode::from(64);
            };
            named_network_cache_max_entries = match v.parse::<u64>() {
                Ok(n) if n > 0 => Some(n),
                _ => {
                    eprintln!(
                        "cruft agent run: --named-network-cache-max-entries must be a positive integer"
                    );
                    return ExitCode::from(64);
                }
            };
        } else if let Some(v) = a.strip_prefix("--named-network-cache-max-entries=") {
            named_network_cache_max_entries = match v.parse::<u64>() {
                Ok(n) if n > 0 => Some(n),
                _ => {
                    eprintln!(
                        "cruft agent run: --named-network-cache-max-entries must be a positive integer"
                    );
                    return ExitCode::from(64);
                }
            };
        } else if a == "--named-network-retry-attempts" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --named-network-retry-attempts requires an argument");
                return ExitCode::from(64);
            };
            named_network_retry_attempts = match v.parse::<u64>() {
                Ok(n) if n <= 3 => n,
                _ => {
                    eprintln!("cruft agent run: --named-network-retry-attempts must be an integer from 0 to 3");
                    return ExitCode::from(64);
                }
            };
        } else if let Some(v) = a.strip_prefix("--named-network-retry-attempts=") {
            named_network_retry_attempts = match v.parse::<u64>() {
                Ok(n) if n <= 3 => n,
                _ => {
                    eprintln!("cruft agent run: --named-network-retry-attempts must be an integer from 0 to 3");
                    return ExitCode::from(64);
                }
            };
        } else if a == "--github-token-env" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --github-token-env requires an env key");
                return ExitCode::from(64);
            };
            if !agent_env_key_is_valid(v) {
                eprintln!("cruft agent run: --github-token-env must be a non-empty ASCII env key");
                return ExitCode::from(64);
            }
            github_token_env = Some(v.clone());
        } else if let Some(v) = a.strip_prefix("--github-token-env=") {
            if !agent_env_key_is_valid(v) {
                eprintln!("cruft agent run: --github-token-env must be a non-empty ASCII env key");
                return ExitCode::from(64);
            }
            github_token_env = Some(v.to_string());
        } else if a == "--process-command" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --process-command requires <name=path>");
                return ExitCode::from(64);
            };
            let Some((name, path)) = v.split_once('=') else {
                eprintln!("cruft agent run: --process-command requires <name=path>");
                return ExitCode::from(64);
            };
            if name.is_empty() || path.is_empty() {
                eprintln!("cruft agent run: --process-command requires non-empty name and path");
                return ExitCode::from(64);
            }
            process_command_specs.push((name.to_string(), path.to_string()));
        } else if let Some(v) = a.strip_prefix("--process-command=") {
            let Some((name, path)) = v.split_once('=') else {
                eprintln!("cruft agent run: --process-command requires <name=path>");
                return ExitCode::from(64);
            };
            if name.is_empty() || path.is_empty() {
                eprintln!("cruft agent run: --process-command requires non-empty name and path");
                return ExitCode::from(64);
            }
            process_command_specs.push((name.to_string(), path.to_string()));
        } else if a == "--process-cwd" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --process-cwd requires a path");
                return ExitCode::from(64);
            };
            if v.is_empty() {
                eprintln!("cruft agent run: --process-cwd requires a non-empty path");
                return ExitCode::from(64);
            }
            process_cwd_specs.push(v.clone());
        } else if let Some(v) = a.strip_prefix("--process-cwd=") {
            if v.is_empty() {
                eprintln!("cruft agent run: --process-cwd requires a non-empty path");
                return ExitCode::from(64);
            }
            process_cwd_specs.push(v.to_string());
        } else if a == "--process-env" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --process-env requires <KEY=value>");
                return ExitCode::from(64);
            };
            let Some((key, value)) = v.split_once('=') else {
                eprintln!("cruft agent run: --process-env requires <KEY=value>");
                return ExitCode::from(64);
            };
            process_env_specs.push((key.to_string(), value.to_string()));
        } else if let Some(v) = a.strip_prefix("--process-env=") {
            let Some((key, value)) = v.split_once('=') else {
                eprintln!("cruft agent run: --process-env requires <KEY=value>");
                return ExitCode::from(64);
            };
            process_env_specs.push((key.to_string(), value.to_string()));
        } else if a == "--expect-event" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --expect-event requires <kind=schema.json>");
                return ExitCode::from(64);
            };
            let Some((kind, path)) = v.split_once('=') else {
                eprintln!("cruft agent run: --expect-event requires <kind=schema.json>");
                return ExitCode::from(64);
            };
            if kind.is_empty() || path.is_empty() {
                eprintln!("cruft agent run: --expect-event requires non-empty kind and path");
                return ExitCode::from(64);
            }
            expected_event_specs.push((kind.to_string(), path.to_string()));
        } else if let Some(v) = a.strip_prefix("--expect-event=") {
            let Some((kind, path)) = v.split_once('=') else {
                eprintln!("cruft agent run: --expect-event requires <kind=schema.json>");
                return ExitCode::from(64);
            };
            if kind.is_empty() || path.is_empty() {
                eprintln!("cruft agent run: --expect-event requires non-empty kind and path");
                return ExitCode::from(64);
            }
            expected_event_specs.push((kind.to_string(), path.to_string()));
        } else if a == "--module" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --module requires <specifier=path>");
                return ExitCode::from(64);
            };
            let Some((specifier, path)) = v.split_once('=') else {
                eprintln!("cruft agent run: --module requires <specifier=path>");
                return ExitCode::from(64);
            };
            if specifier.is_empty() || path.is_empty() {
                eprintln!("cruft agent run: --module requires non-empty specifier and path");
                return ExitCode::from(64);
            }
            module_specs.push((specifier.to_string(), path.to_string()));
        } else if let Some(v) = a.strip_prefix("--module=") {
            let Some((specifier, path)) = v.split_once('=') else {
                eprintln!("cruft agent run: --module requires <specifier=path>");
                return ExitCode::from(64);
            };
            if specifier.is_empty() || path.is_empty() {
                eprintln!("cruft agent run: --module requires non-empty specifier and path");
                return ExitCode::from(64);
            }
            module_specs.push((specifier.to_string(), path.to_string()));
        } else if a == "--package" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --package requires <specifier=path>");
                return ExitCode::from(64);
            };
            let Some((specifier, path)) = v.split_once('=') else {
                eprintln!("cruft agent run: --package requires <specifier=path>");
                return ExitCode::from(64);
            };
            if specifier.is_empty() || path.is_empty() {
                eprintln!("cruft agent run: --package requires non-empty specifier and path");
                return ExitCode::from(64);
            }
            package_specs.push((specifier.to_string(), path.to_string()));
        } else if let Some(v) = a.strip_prefix("--package=") {
            let Some((specifier, path)) = v.split_once('=') else {
                eprintln!("cruft agent run: --package requires <specifier=path>");
                return ExitCode::from(64);
            };
            if specifier.is_empty() || path.is_empty() {
                eprintln!("cruft agent run: --package requires non-empty specifier and path");
                return ExitCode::from(64);
            }
            package_specs.push((specifier.to_string(), path.to_string()));
        } else if a == "--package-integrity" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --package-integrity requires <specifier=hash>");
                return ExitCode::from(64);
            };
            let Some((specifier, integrity)) = v.split_once('=') else {
                eprintln!("cruft agent run: --package-integrity requires <specifier=hash>");
                return ExitCode::from(64);
            };
            if specifier.is_empty() || integrity.is_empty() {
                eprintln!(
                    "cruft agent run: --package-integrity requires non-empty specifier and hash"
                );
                return ExitCode::from(64);
            }
            package_integrities.insert(specifier.to_string(), integrity.to_string());
        } else if let Some(v) = a.strip_prefix("--package-integrity=") {
            let Some((specifier, integrity)) = v.split_once('=') else {
                eprintln!("cruft agent run: --package-integrity requires <specifier=hash>");
                return ExitCode::from(64);
            };
            if specifier.is_empty() || integrity.is_empty() {
                eprintln!(
                    "cruft agent run: --package-integrity requires non-empty specifier and hash"
                );
                return ExitCode::from(64);
            }
            package_integrities.insert(specifier.to_string(), integrity.to_string());
        } else if a == "--import-hook" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --import-hook requires <specifier=path>");
                return ExitCode::from(64);
            };
            let Some((specifier, path)) = v.split_once('=') else {
                eprintln!("cruft agent run: --import-hook requires <specifier=path>");
                return ExitCode::from(64);
            };
            if specifier.is_empty() || path.is_empty() {
                eprintln!("cruft agent run: --import-hook requires non-empty specifier and path");
                return ExitCode::from(64);
            }
            import_hook_specs.push((specifier.to_string(), path.to_string()));
        } else if let Some(v) = a.strip_prefix("--import-hook=") {
            let Some((specifier, path)) = v.split_once('=') else {
                eprintln!("cruft agent run: --import-hook requires <specifier=path>");
                return ExitCode::from(64);
            };
            if specifier.is_empty() || path.is_empty() {
                eprintln!("cruft agent run: --import-hook requires non-empty specifier and path");
                return ExitCode::from(64);
            }
            import_hook_specs.push((specifier.to_string(), path.to_string()));
        } else if a == "--import-hook-integrity" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --import-hook-integrity requires <specifier=hash>");
                return ExitCode::from(64);
            };
            let Some((specifier, integrity)) = v.split_once('=') else {
                eprintln!("cruft agent run: --import-hook-integrity requires <specifier=hash>");
                return ExitCode::from(64);
            };
            if specifier.is_empty() || integrity.is_empty() {
                eprintln!(
                    "cruft agent run: --import-hook-integrity requires non-empty specifier and hash"
                );
                return ExitCode::from(64);
            }
            import_hook_integrities.insert(specifier.to_string(), integrity.to_string());
        } else if let Some(v) = a.strip_prefix("--import-hook-integrity=") {
            let Some((specifier, integrity)) = v.split_once('=') else {
                eprintln!("cruft agent run: --import-hook-integrity requires <specifier=hash>");
                return ExitCode::from(64);
            };
            if specifier.is_empty() || integrity.is_empty() {
                eprintln!(
                    "cruft agent run: --import-hook-integrity requires non-empty specifier and hash"
                );
                return ExitCode::from(64);
            }
            import_hook_integrities.insert(specifier.to_string(), integrity.to_string());
        } else if a == "--entry-module" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --entry-module requires a specifier");
                return ExitCode::from(64);
            };
            entry_module = Some(v.clone());
        } else if let Some(v) = a.strip_prefix("--entry-module=") {
            entry_module = Some(v.to_string());
        } else if a == "--tool" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --tool requires an argument");
                return ExitCode::from(64);
            };
            tools.push(v.clone());
        } else if let Some(v) = a.strip_prefix("--tool=") {
            tools.push(v.to_string());
        } else if a == "--require-approval" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --require-approval requires a tool name");
                return ExitCode::from(64);
            };
            approval_required_tools.push(v.clone());
        } else if let Some(v) = a.strip_prefix("--require-approval=") {
            approval_required_tools.push(v.to_string());
        } else if a == "--approve-tool" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --approve-tool requires a tool name");
                return ExitCode::from(64);
            };
            approved_tools.push(v.clone());
        } else if let Some(v) = a.strip_prefix("--approve-tool=") {
            approved_tools.push(v.to_string());
        } else if a == "--approval-log" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --approval-log requires a path");
                return ExitCode::from(64);
            };
            if v.is_empty() {
                eprintln!("cruft agent run: --approval-log requires a non-empty path");
                return ExitCode::from(64);
            }
            approval_log = Some(v.clone());
        } else if let Some(v) = a.strip_prefix("--approval-log=") {
            if v.is_empty() {
                eprintln!("cruft agent run: --approval-log requires a non-empty path");
                return ExitCode::from(64);
            }
            approval_log = Some(v.to_string());
        } else if a == "--approval-max-age-ms" {
            i += 1;
            let Some(v) = args.get(i) else {
                eprintln!("cruft agent run: --approval-max-age-ms requires a positive integer");
                return ExitCode::from(64);
            };
            match v.parse::<u64>() {
                Ok(n) if n > 0 => approval_max_age_ms = Some(n),
                _ => {
                    eprintln!("cruft agent run: --approval-max-age-ms requires a positive integer");
                    return ExitCode::from(64);
                }
            }
        } else if let Some(v) = a.strip_prefix("--approval-max-age-ms=") {
            match v.parse::<u64>() {
                Ok(n) if n > 0 => approval_max_age_ms = Some(n),
                _ => {
                    eprintln!("cruft agent run: --approval-max-age-ms requires a positive integer");
                    return ExitCode::from(64);
                }
            }
        } else if a.starts_with('-') {
            eprintln!("cruft agent run: unknown option {a}");
            return ExitCode::from(64);
        } else if source_path.is_none() {
            source_path = Some(a.to_string());
        } else {
            eprintln!("cruft agent run: unexpected argument {a}");
            return ExitCode::from(64);
        }
        i += 1;
    }
    let Some(source_path) = source_path else {
        eprintln!("cruft agent run: missing <agent.js>");
        return ExitCode::from(64);
    };
    let Some(audit_log) = audit_log else {
        eprintln!("cruft agent run: --audit-log is required for the MVP mouth");
        return ExitCode::from(64);
    };
    let run_id = run_id.unwrap_or_else(|| {
        std::path::Path::new(&audit_log)
            .file_stem()
            .and_then(|s| s.to_str())
            .filter(|s| agent_validate_run_id(s))
            .unwrap_or("default")
            .to_string()
    });
    let mut expected_event_schemas = Vec::new();
    for (kind, path) in &expected_event_specs {
        match agent_load_event_schema(kind, path) {
            Ok(schema) => expected_event_schemas.push(schema),
            Err(e) => {
                eprintln!("cruft agent run: {e}");
                return ExitCode::from(65);
            }
        }
    }
    let osv_fixture = match osv_fixture_path.as_deref() {
        Some(path) => match agent_load_osv_fixture(path) {
            Ok(json) => Some(AgentOsvFixture { json }),
            Err(e) => {
                eprintln!("cruft agent run: {e}");
                return ExitCode::from(65);
            }
        },
        None => None,
    };
    let model_fixture = match model_fixture_path.as_deref() {
        Some(path) => match agent_load_model_fixture(path) {
            Ok(json) => Some(AgentModelFixture { json }),
            Err(e) => {
                eprintln!("cruft agent run: {e}");
                return ExitCode::from(65);
            }
        },
        None => None,
    };
    let mut seen_secret_tools = std::collections::HashSet::new();
    for (tool, env) in &secret_scopes {
        if !seen_secret_tools.insert(tool.clone()) {
            eprintln!("cruft agent run: duplicate --secret scope for tool {tool:?}");
            return ExitCode::from(64);
        }
        if !tools.iter().any(|t| t == tool) {
            eprintln!("cruft agent run: --secret {tool}=<ENV> requires --tool={tool}");
            return ExitCode::from(64);
        }
        match std::env::var(env) {
            Ok(value) if !value.is_empty() => {}
            Ok(_) => {
                eprintln!("cruft agent run: --secret {tool} env {env} is empty");
                return ExitCode::from(65);
            }
            Err(_) => {
                eprintln!("cruft agent run: --secret {tool} env {env} is not set");
                return ExitCode::from(65);
            }
        }
        if tool.starts_with("github.") && github_token_env.is_none() {
            github_token_env = Some(env.clone());
        }
        if tool == "model.call"
            && model_provider.as_deref() == Some("openai.responses")
            && model_api_key_env.is_none()
        {
            model_api_key_env = Some(env.clone());
        }
    }
    if model_provider.is_some() != model_api_key_env.is_some() {
        eprintln!(
            "cruft agent run: --model-provider=openai.responses requires --model-api-key-env, and --model-api-key-env requires --model-provider"
        );
        return ExitCode::from(64);
    }
    let model_api_key_value = match model_api_key_env.as_deref() {
        Some(key) => match std::env::var(key) {
            Ok(value) if !value.is_empty() => Some(value),
            Ok(_) => {
                eprintln!("cruft agent run: --model-api-key-env {key} is empty");
                return ExitCode::from(65);
            }
            Err(_) => {
                eprintln!("cruft agent run: --model-api-key-env {key} is not set");
                return ExitCode::from(65);
            }
        },
        None => None,
    };
    if let Some(path) = named_network_cache_dir.as_deref() {
        match std::fs::create_dir_all(path) {
            Ok(()) => {
                if !std::path::Path::new(path).is_dir() {
                    eprintln!("cruft agent run: --named-network-cache-dir must be a directory");
                    return ExitCode::from(66);
                }
            }
            Err(e) => {
                eprintln!("cruft agent run: cannot create --named-network-cache-dir {path}: {e}");
                return ExitCode::from(66);
            }
        }
    } else if named_network_cache_mode != "read-through"
        || named_network_cache_max_age_ms.is_some()
        || named_network_cache_max_entries.is_some()
    {
        eprintln!(
            "cruft agent run: --named-network-cache-mode, --named-network-cache-max-age-ms, and --named-network-cache-max-entries require --named-network-cache-dir"
        );
        return ExitCode::from(64);
    }
    let process_commands = match agent_collect_process_commands(&process_command_specs) {
        Ok(commands) => commands,
        Err(e) => {
            eprintln!("cruft agent run: {e}");
            return ExitCode::from(65);
        }
    };
    let process_cwds = match agent_collect_process_cwds(&process_cwd_specs) {
        Ok(cwds) => cwds,
        Err(e) => {
            eprintln!("cruft agent run: {e}");
            return ExitCode::from(66);
        }
    };
    let process_env = match agent_collect_process_env(&process_env_specs) {
        Ok(env) => env,
        Err(e) => {
            eprintln!("cruft agent run: {e}");
            return ExitCode::from(64);
        }
    };
    for tool in &tools {
        if matches!(tool.as_str(), "shell" | "exec" | "spawn")
            || (tool == "process" && (process_commands.is_empty() || process_cwds.is_empty()))
        {
            let reason = "process_tool_supervisor_required_not_available";
            append_agent_unsupported_control(&audit_log, "process_tool", reason);
            eprintln!(
                "cruft agent run: --tool={tool} requires process tool supervisor/cancellation substrate"
            );
            return ExitCode::from(78);
        }
        if worker_requested && tool == "process" {
            let reason = "agent_worker_process_host_call_required_not_available";
            append_agent_unsupported_control(&audit_log, "worker_hosted", reason);
            eprintln!(
                "cruft agent run: --worker --tool=process requires worker process host-call membrane repair; use same-thread process or omit --worker until that substrate closes"
            );
            return ExitCode::from(78);
        }
        if !is_agent_builtin_tool_specifier(tool) {
            if worker_requested {
                let reason = "agent_worker_full_membrane_required_not_available";
                append_agent_unsupported_control(&audit_log, "worker_hosted", reason);
                eprintln!(
                    "cruft agent run: --worker --tool={tool} requires the full agent worker host membrane"
                );
                return ExitCode::from(78);
            }
            eprintln!(
                "cruft agent run: unsupported MVP tool {tool:?}; available: echo, fail, slow, readFile, listFiles, writeArtifact, osv.query, npm.metadata, github.issue.read, github.pr.read, github.pr.files.list, github.release.latest.read, github.file.read, github.compare.read, github.commit.read, github.repo.read, github.workflow.run.read, github.workflow.jobs.list, github.check.runs.list, model.call, process"
            );
            return ExitCode::from(64);
        }
        if worker_requested && tool == "osv.query" {
            let reason = "agent_worker_osv_tool_membrane_required_not_available";
            append_agent_unsupported_control(&audit_log, "worker_hosted", reason);
            eprintln!(
                "cruft agent run: --worker --tool=osv.query requires worker OSV tool membrane repair; use same-thread osv.query until that substrate closes"
            );
            return ExitCode::from(78);
        }
        if worker_requested && tool == "npm.metadata" {
            let reason = "agent_worker_npm_metadata_tool_membrane_required_not_available";
            append_agent_unsupported_control(&audit_log, "worker_hosted", reason);
            eprintln!(
                "cruft agent run: --worker --tool=npm.metadata requires worker npm.metadata tool membrane repair; use same-thread npm.metadata until that substrate closes"
            );
            return ExitCode::from(78);
        }
        if worker_requested
            && matches!(
                tool.as_str(),
                "github.pr.files.list"
                    | "github.commit.read"
                    | "github.workflow.jobs.list"
                    | "github.check.runs.list"
            )
        {
            let reason = "agent_worker_github_complex_payload_tool_membrane_required_not_available";
            append_agent_unsupported_control(&audit_log, "worker_hosted", reason);
            eprintln!(
                "cruft agent run: --worker --tool={tool} requires worker GitHub complex-payload tool membrane repair; use same-thread {tool} until that substrate closes"
            );
            return ExitCode::from(78);
        }
        if worker_requested
            && tool == "osv.query"
            && osv_fixture.is_none()
            && named_network_cache_dir.is_none()
        {
            let reason = "osv_live_transport_not_available";
            append_agent_unsupported_control(&audit_log, "osv_query", reason);
            eprintln!(
                "cruft agent run: --worker --tool=osv.query requires --osv-fixture or --named-network-cache-dir until worker live OSV transport forwarding closes"
            );
            return ExitCode::from(78);
        }
        if worker_requested && tool == "npm.metadata" && named_network_cache_dir.is_none() {
            let reason = "agent_worker_named_network_tool_forwarding_required_not_available";
            append_agent_unsupported_control(&audit_log, "worker_hosted", reason);
            eprintln!(
                "cruft agent run: --worker --tool=npm.metadata requires --named-network-cache-dir until worker live named network forwarding closes"
            );
            return ExitCode::from(78);
        }
        if worker_requested && tool == "github.issue.read" && named_network_cache_dir.is_none() {
            let reason = "agent_worker_named_network_tool_forwarding_required_not_available";
            append_agent_unsupported_control(&audit_log, "worker_hosted", reason);
            eprintln!(
                "cruft agent run: --worker --tool=github.issue.read requires --named-network-cache-dir until worker live named network forwarding closes"
            );
            return ExitCode::from(78);
        }
        if worker_requested && tool == "github.pr.read" && named_network_cache_dir.is_none() {
            let reason = "agent_worker_named_network_tool_forwarding_required_not_available";
            append_agent_unsupported_control(&audit_log, "worker_hosted", reason);
            eprintln!(
                "cruft agent run: --worker --tool=github.pr.read requires --named-network-cache-dir until worker live named network forwarding closes"
            );
            return ExitCode::from(78);
        }
        if worker_requested && tool == "github.pr.files.list" && named_network_cache_dir.is_none() {
            let reason = "agent_worker_named_network_tool_forwarding_required_not_available";
            append_agent_unsupported_control(&audit_log, "worker_hosted", reason);
            eprintln!(
                "cruft agent run: --worker --tool=github.pr.files.list requires --named-network-cache-dir until worker live named network forwarding closes"
            );
            return ExitCode::from(78);
        }
        if worker_requested
            && tool == "github.release.latest.read"
            && named_network_cache_dir.is_none()
        {
            let reason = "agent_worker_named_network_tool_forwarding_required_not_available";
            append_agent_unsupported_control(&audit_log, "worker_hosted", reason);
            eprintln!(
                "cruft agent run: --worker --tool=github.release.latest.read requires --named-network-cache-dir until worker live named network forwarding closes"
            );
            return ExitCode::from(78);
        }
        if worker_requested && tool == "github.file.read" && named_network_cache_dir.is_none() {
            let reason = "agent_worker_named_network_tool_forwarding_required_not_available";
            append_agent_unsupported_control(&audit_log, "worker_hosted", reason);
            eprintln!(
                "cruft agent run: --worker --tool=github.file.read requires --named-network-cache-dir until worker live named network forwarding closes"
            );
            return ExitCode::from(78);
        }
        if worker_requested && tool == "github.compare.read" && named_network_cache_dir.is_none() {
            let reason = "agent_worker_named_network_tool_forwarding_required_not_available";
            append_agent_unsupported_control(&audit_log, "worker_hosted", reason);
            eprintln!(
                "cruft agent run: --worker --tool=github.compare.read requires --named-network-cache-dir until worker live named network forwarding closes"
            );
            return ExitCode::from(78);
        }
        if worker_requested && tool == "github.commit.read" && named_network_cache_dir.is_none() {
            let reason = "agent_worker_named_network_tool_forwarding_required_not_available";
            append_agent_unsupported_control(&audit_log, "worker_hosted", reason);
            eprintln!(
                "cruft agent run: --worker --tool=github.commit.read requires --named-network-cache-dir until worker live named network forwarding closes"
            );
            return ExitCode::from(78);
        }
        if worker_requested && tool == "github.repo.read" && named_network_cache_dir.is_none() {
            let reason = "agent_worker_named_network_tool_forwarding_required_not_available";
            append_agent_unsupported_control(&audit_log, "worker_hosted", reason);
            eprintln!(
                "cruft agent run: --worker --tool=github.repo.read requires --named-network-cache-dir until worker live named network forwarding closes"
            );
            return ExitCode::from(78);
        }
        if worker_requested
            && tool == "github.workflow.run.read"
            && named_network_cache_dir.is_none()
        {
            let reason = "agent_worker_named_network_tool_forwarding_required_not_available";
            append_agent_unsupported_control(&audit_log, "worker_hosted", reason);
            eprintln!(
                "cruft agent run: --worker --tool=github.workflow.run.read requires --named-network-cache-dir until worker live named network forwarding closes"
            );
            return ExitCode::from(78);
        }
        if worker_requested
            && tool == "github.workflow.jobs.list"
            && named_network_cache_dir.is_none()
        {
            let reason = "agent_worker_named_network_tool_forwarding_required_not_available";
            append_agent_unsupported_control(&audit_log, "worker_hosted", reason);
            eprintln!(
                "cruft agent run: --worker --tool=github.workflow.jobs.list requires --named-network-cache-dir until worker live named network forwarding closes"
            );
            return ExitCode::from(78);
        }
        if worker_requested && tool == "github.check.runs.list" && named_network_cache_dir.is_none()
        {
            let reason = "agent_worker_named_network_tool_forwarding_required_not_available";
            append_agent_unsupported_control(&audit_log, "worker_hosted", reason);
            eprintln!(
                "cruft agent run: --worker --tool=github.check.runs.list requires --named-network-cache-dir until worker live named network forwarding closes"
            );
            return ExitCode::from(78);
        }
        if tool == "model.call" {
            let reason = "agent_model_tool_membrane_required_not_available";
            let control = if worker_requested {
                "worker_hosted"
            } else {
                "model_tool"
            };
            append_agent_unsupported_control(&audit_log, control, reason);
            eprintln!(
                "cruft agent run: --tool=model.call requires model tool membrane repair; omit model.call until that substrate closes"
            );
            return ExitCode::from(78);
        }
    }
    for tool in approval_required_tools.iter().chain(approved_tools.iter()) {
        if !is_agent_builtin_tool_specifier(tool) {
            eprintln!(
                "cruft agent run: approval tool {tool:?} is unknown; run `cruft agent tool list`"
            );
            return ExitCode::from(64);
        }
    }
    if let Some(path) = approval_log.as_deref() {
        let parent = std::path::Path::new(path)
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!(
                "cruft agent run: cannot create --approval-log parent {}: {e}",
                parent.display()
            );
            return ExitCode::from(66);
        }
    } else if approval_max_age_ms.is_some() {
        eprintln!("cruft agent run: --approval-max-age-ms requires --approval-log");
        return ExitCode::from(64);
    }
    let fs_read_files = match agent_collect_fs_read_caps(
        &fs_read_specs,
        &fs_read_include_patterns,
        &fs_read_exclude_patterns,
    ) {
        Ok(files) => files,
        Err(e) => {
            eprintln!("cruft agent run: {e}");
            return ExitCode::from(66);
        }
    };
    let fs_write_roots = match agent_collect_fs_write_roots(&fs_write_specs) {
        Ok(roots) => roots,
        Err(e) => {
            eprintln!("cruft agent run: {e}");
            return ExitCode::from(66);
        }
    };
    let source = match std::fs::read_to_string(&source_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cruft agent run: cannot read {source_path}: {e}");
            return ExitCode::from(66);
        }
    };
    if worker_requested {
        let worker_memory_tools_only = tools.iter().all(|tool| {
            tool == "echo"
                || tool == "fail"
                || tool == "slow"
                || tool == "osv.query"
                || tool == "model.call"
                || (tool == "npm.metadata" && named_network_cache_dir.is_some())
                || (tool == "github.issue.read" && named_network_cache_dir.is_some())
                || (tool == "github.pr.read" && named_network_cache_dir.is_some())
                || (tool == "github.pr.files.list" && named_network_cache_dir.is_some())
                || (tool == "github.release.latest.read" && named_network_cache_dir.is_some())
                || (tool == "github.file.read" && named_network_cache_dir.is_some())
                || (tool == "github.compare.read" && named_network_cache_dir.is_some())
                || (tool == "github.commit.read" && named_network_cache_dir.is_some())
                || (tool == "github.workflow.run.read" && named_network_cache_dir.is_some())
                || (tool == "github.workflow.jobs.list" && named_network_cache_dir.is_some())
                || (tool == "github.check.runs.list" && named_network_cache_dir.is_some())
                || (tool == "github.repo.read" && named_network_cache_dir.is_some())
        });
        let full_worker_membrane_requested = !worker_memory_tools_only;
        if full_worker_membrane_requested {
            let reason = "agent_worker_full_membrane_required_not_available";
            append_agent_unsupported_control(&audit_log, "worker_hosted", reason);
            eprintln!(
                "cruft agent run: --worker currently supports source/context/emit, in-memory tools, single-turn state, file-backed sessions, close, exact source modules/entry modules, package caps, and source import hooks only; external async tools and audit RPC require the full agent worker membrane"
            );
            return ExitCode::from(78);
        }
        let mut worker_module_sources = Vec::new();
        let mut worker_import_hook_sources = Vec::new();
        let mut worker_admitted_specifiers = std::collections::HashSet::new();
        for (specifier, path) in &module_specs {
            if !(specifier.starts_with("./") || specifier.starts_with("../")) {
                eprintln!(
                    "cruft agent run: --module specifier must be relative for MVP policy: {specifier:?}"
                );
                return ExitCode::from(64);
            }
            worker_admitted_specifiers.insert(specifier.clone());
            let module_source = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("cruft agent run: cannot read module {path}: {e}");
                    return ExitCode::from(66);
                }
            };
            worker_module_sources.push((specifier.clone(), module_source));
        }
        for (specifier, path) in &package_specs {
            if !is_agent_package_specifier(specifier) {
                eprintln!(
                    "cruft agent run: --package specifier must be a bare package name for hash-capped package policy: {specifier:?}"
                );
                return ExitCode::from(64);
            }
            if worker_admitted_specifiers.contains(specifier) {
                eprintln!(
                    "cruft agent run: duplicate admitted module/package/import-hook specifier: {specifier:?}"
                );
                return ExitCode::from(64);
            }
            let package_path = std::path::Path::new(path);
            let Some(expected_integrity) = package_integrities.get(specifier) else {
                eprintln!(
                    "cruft agent run: --package requires matching --package-integrity for {specifier:?}"
                );
                print_agent_package_integrity_repair(specifier, path);
                return ExitCode::from(64);
            };
            let package_meta = match std::fs::metadata(package_path) {
                Ok(meta) => meta,
                Err(e) => {
                    eprintln!("cruft agent run: cannot stat package {path}: {e}");
                    return ExitCode::from(66);
                }
            };
            let (package_modules, source_hash) = if package_meta.is_dir() {
                let (package_modules, graph_hash, _) = match collect_simple_agent_package_graph(
                    specifier,
                    package_path,
                ) {
                    Ok(graph) => graph,
                    Err(reason) => {
                        append_agent_unsupported_control(&audit_log, "package_graph", reason);
                        eprintln!(
                                "cruft agent run: --worker package directory graph for {specifier:?} exceeds conservative package graph policy"
                            );
                        return ExitCode::from(78);
                    }
                };
                for (admitted_specifier, _) in &package_modules {
                    if worker_admitted_specifiers.contains(admitted_specifier) {
                        eprintln!(
                            "cruft agent run: duplicate admitted module/package/import-hook specifier: {admitted_specifier:?}"
                        );
                        return ExitCode::from(64);
                    }
                }
                (package_modules, graph_hash)
            } else {
                let source = match std::fs::read_to_string(path) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("cruft agent run: cannot read package {path}: {e}");
                        return ExitCode::from(66);
                    }
                };
                let source_hash = agent_source_hash(&source);
                (vec![(specifier.clone(), source)], source_hash)
            };
            if &source_hash != expected_integrity {
                eprintln!(
                    "cruft agent run: package integrity mismatch for {specifier:?}: expected {expected_integrity}, got {source_hash}"
                );
                print_agent_package_integrity_repair(specifier, path);
                return ExitCode::from(65);
            }
            for (admitted_specifier, package_source) in package_modules {
                if agent_source_has_exported_function_surface(&package_source) {
                    let reason = "agent_worker_package_function_exports_required_not_available";
                    append_agent_unsupported_control(&audit_log, "worker_hosted", reason);
                    eprintln!(
                        "cruft agent run: --worker package {admitted_specifier:?} exports function code; worker package function execution requires the full agent worker package membrane"
                    );
                    return ExitCode::from(78);
                }
                worker_admitted_specifiers.insert(admitted_specifier.clone());
                worker_module_sources.push((admitted_specifier, package_source));
            }
        }
        for specifier in package_integrities.keys() {
            if !package_specs.iter().any(|(s, _)| s == specifier) {
                eprintln!(
                    "cruft agent run: --package-integrity has no matching --package for {specifier:?}"
                );
                return ExitCode::from(64);
            }
        }
        for (specifier, path) in &import_hook_specs {
            if !(specifier.starts_with("./")
                || specifier.starts_with("../")
                || is_agent_package_specifier(specifier))
            {
                eprintln!(
                    "cruft agent run: --import-hook specifier must be relative or a bare package name for worker source-hook policy: {specifier:?}"
                );
                return ExitCode::from(64);
            }
            if worker_admitted_specifiers.contains(specifier) {
                eprintln!("cruft agent run: duplicate admitted module/import-hook specifier: {specifier:?}");
                return ExitCode::from(64);
            }
            let Some(expected_integrity) = import_hook_integrities.get(specifier) else {
                eprintln!(
                    "cruft agent run: --import-hook requires matching --import-hook-integrity for {specifier:?}"
                );
                print_agent_import_hook_integrity_repair(specifier, path);
                return ExitCode::from(64);
            };
            let hook_source = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("cruft agent run: cannot read import-hook source {path}: {e}");
                    return ExitCode::from(66);
                }
            };
            let source_hash = agent_source_hash(&hook_source);
            if &source_hash != expected_integrity {
                eprintln!(
                    "cruft agent run: import-hook integrity mismatch for {specifier:?}: expected {expected_integrity}, got {source_hash}"
                );
                print_agent_import_hook_integrity_repair(specifier, path);
                return ExitCode::from(65);
            }
            worker_admitted_specifiers.insert(specifier.clone());
            worker_import_hook_sources.push((specifier.clone(), hook_source));
        }
        for specifier in import_hook_integrities.keys() {
            if !import_hook_specs.iter().any(|(s, _)| s == specifier) {
                eprintln!(
                    "cruft agent run: --import-hook-integrity has no matching --import-hook for {specifier:?}"
                );
                return ExitCode::from(64);
            }
        }
        if let Some(specifier) = &entry_module {
            if !(specifier.starts_with("./") || specifier.starts_with("../")) {
                eprintln!("cruft agent run: --entry-module specifier must be relative for MVP policy: {specifier:?}");
                return ExitCode::from(64);
            }
            if !worker_admitted_specifiers.contains(specifier) {
                eprintln!(
                    "cruft agent run: --entry-module must name an admitted --module specifier: {specifier:?}"
                );
                return ExitCode::from(64);
            }
        }
        if let Some((specifier, binding)) =
            agent_first_unadmitted_literal_import_value(&source, &worker_admitted_specifiers)
        {
            append_agent_module_denial(&audit_log, &specifier, &binding);
            eprintln!("cruft agent run: agent module denied: {specifier}");
            return ExitCode::from(70);
        }
        return run_agent_worker_emit_harness(AgentWorkerEmitConfig {
            source_path: &source_path,
            source: &source,
            audit_log: &audit_log,
            run_id: &run_id,
            context_json: &context_json,
            state_json: &state_json,
            tools: &tools,
            modules: &worker_module_sources,
            import_hooks: &worker_import_hook_sources,
            package_imports: if package_specs.is_empty() {
                "not_available_in_worker_emit_mode"
            } else if package_specs
                .iter()
                .any(|(_, path)| std::path::Path::new(path).is_dir())
            {
                "explicit_package_graph_caps"
            } else {
                "explicit_file_hash_caps"
            },
            timeout_ms,
            max_events,
            max_event_bytes,
            max_state_bytes,
            max_tool_arg_bytes,
            max_tool_result_bytes,
            tool_timeout_ms,
            session_file: session_file.as_deref(),
            entry_module: entry_module.as_deref(),
            max_microtasks,
            max_steps,
            max_rss_mb,
            redact_fields: &redact_fields,
            fs_read_files: &fs_read_files,
            fs_write_roots: &fs_write_roots,
            osv_fixture: osv_fixture.as_ref(),
            model_fixture: model_fixture.as_ref(),
            model_provider: model_provider.as_deref(),
            model_api_key_env: model_api_key_env.as_deref(),
            model_api_key_value: model_api_key_value.as_deref(),
            secret_scopes: &secret_scopes,
            approval_required_tools: &approval_required_tools,
            approved_tools: &approved_tools,
            approval_log: approval_log.as_deref(),
            approval_max_age_ms,
            process_commands: &process_commands,
            process_cwds: &process_cwds,
            process_env: &process_env,
            named_network_cache_dir: named_network_cache_dir.as_deref(),
            named_network_cache_mode: &named_network_cache_mode,
            named_network_cache_max_age_ms,
            named_network_cache_max_entries,
            expected_event_schemas: &expected_event_schemas,
        });
    }
    let mut module_entries = Vec::new();
    let mut module_policy_entries = Vec::new();
    let mut admitted_specifiers = std::collections::HashSet::new();
    let mut admitted_package_graph = false;
    for (specifier, path) in &module_specs {
        if !(specifier.starts_with("./") || specifier.starts_with("../")) {
            eprintln!("cruft agent run: --module specifier must be relative for MVP policy: {specifier:?}");
            return ExitCode::from(64);
        }
        admitted_specifiers.insert(specifier.clone());
        let module_source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("cruft agent run: cannot read module {path}: {e}");
                return ExitCode::from(66);
            }
        };
        module_entries.push(format!(
            "{}:{}",
            json_string_literal(specifier),
            json_string_literal(&module_source)
        ));
        module_policy_entries.push(format!(
            "{{specifier:{}, source_hash:{}}}",
            json_string_literal(specifier),
            json_string_literal(&agent_source_hash(&module_source))
        ));
    }
    for (specifier, path) in &package_specs {
        if !is_agent_package_specifier(specifier) {
            eprintln!(
                "cruft agent run: --package specifier must be a bare package name for hash-capped package policy: {specifier:?}"
            );
            return ExitCode::from(64);
        }
        if admitted_specifiers.contains(specifier) {
            eprintln!(
                "cruft agent run: duplicate admitted module/package specifier: {specifier:?}"
            );
            return ExitCode::from(64);
        }
        let Some(expected_integrity) = package_integrities.get(specifier) else {
            eprintln!(
                "cruft agent run: --package requires matching --package-integrity for {specifier:?}"
            );
            print_agent_package_integrity_repair(specifier, path);
            return ExitCode::from(64);
        };
        let package_path = std::path::Path::new(path);
        let package_meta = match std::fs::metadata(package_path) {
            Ok(meta) => meta,
            Err(e) => {
                eprintln!("cruft agent run: cannot stat package {path}: {e}");
                return ExitCode::from(66);
            }
        };
        let (package_modules, source_hash, package_kind) = if package_meta.is_dir() {
            let (package_modules, graph_hash, package_kind) =
                match collect_simple_agent_package_graph(specifier, package_path) {
                    Ok(graph) => graph,
                    Err(reason) => {
                        append_agent_unsupported_control(&audit_log, "package_graph", reason);
                        eprintln!(
                            "cruft agent run: --package directory graph for {specifier:?} exceeds conservative package graph policy"
                        );
                        return ExitCode::from(78);
                    }
                };
            for (admitted_specifier, _) in &package_modules {
                if admitted_specifiers.contains(admitted_specifier) {
                    eprintln!(
                        "cruft agent run: duplicate admitted module/package specifier: {admitted_specifier:?}"
                    );
                    return ExitCode::from(64);
                }
            }
            admitted_package_graph = true;
            (package_modules, graph_hash, package_kind)
        } else {
            let source = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("cruft agent run: cannot read package {path}: {e}");
                    return ExitCode::from(66);
                }
            };
            let source_hash = agent_source_hash(&source);
            (
                vec![(specifier.clone(), source)],
                source_hash,
                "package".to_string(),
            )
        };
        if &source_hash != expected_integrity {
            eprintln!(
                "cruft agent run: package integrity mismatch for {specifier:?}: expected {expected_integrity}, got {source_hash}"
            );
            print_agent_package_integrity_repair(specifier, path);
            return ExitCode::from(65);
        }
        for (admitted_specifier, package_source) in package_modules {
            admitted_specifiers.insert(admitted_specifier.clone());
            module_entries.push(format!(
                "{}:{}",
                json_string_literal(&admitted_specifier),
                json_string_literal(&package_source)
            ));
            module_policy_entries.push(format!(
                "{{specifier:{}, kind:\"{}\", source_hash:{}, integrity:{}}}",
                json_string_literal(&admitted_specifier),
                package_kind,
                json_string_literal(&source_hash),
                json_string_literal(expected_integrity)
            ));
        }
    }
    for specifier in package_integrities.keys() {
        if !package_specs.iter().any(|(s, _)| s == specifier) {
            eprintln!(
                "cruft agent run: --package-integrity has no matching --package for {specifier:?}"
            );
            return ExitCode::from(64);
        }
    }
    let mut import_hook_entries = Vec::new();
    for (specifier, path) in &import_hook_specs {
        if !(specifier.starts_with("./")
            || specifier.starts_with("../")
            || is_agent_package_specifier(specifier))
        {
            eprintln!(
                "cruft agent run: --import-hook specifier must be relative or a bare package name for source-hook policy: {specifier:?}"
            );
            return ExitCode::from(64);
        }
        if admitted_specifiers.contains(specifier) {
            eprintln!("cruft agent run: duplicate admitted module/package/import-hook specifier: {specifier:?}");
            return ExitCode::from(64);
        }
        let Some(expected_integrity) = import_hook_integrities.get(specifier) else {
            eprintln!(
                "cruft agent run: --import-hook requires matching --import-hook-integrity for {specifier:?}"
            );
            print_agent_import_hook_integrity_repair(specifier, path);
            return ExitCode::from(64);
        };
        let hook_source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("cruft agent run: cannot read import-hook source {path}: {e}");
                return ExitCode::from(66);
            }
        };
        let source_hash = agent_source_hash(&hook_source);
        if &source_hash != expected_integrity {
            eprintln!(
                "cruft agent run: import-hook integrity mismatch for {specifier:?}: expected {expected_integrity}, got {source_hash}"
            );
            print_agent_import_hook_integrity_repair(specifier, path);
            return ExitCode::from(65);
        }
        admitted_specifiers.insert(specifier.clone());
        import_hook_entries.push(format!(
            "{}:{}",
            json_string_literal(specifier),
            json_string_literal(&hook_source)
        ));
        module_policy_entries.push(format!(
            "{{specifier:{}, kind:\"import_hook_source\", source_hash:{}, integrity:{}}}",
            json_string_literal(specifier),
            json_string_literal(&source_hash),
            json_string_literal(expected_integrity)
        ));
    }
    for specifier in import_hook_integrities.keys() {
        if !import_hook_specs.iter().any(|(s, _)| s == specifier) {
            eprintln!(
                "cruft agent run: --import-hook-integrity has no matching --import-hook for {specifier:?}"
            );
            return ExitCode::from(64);
        }
    }
    if let Some(specifier) = &entry_module {
        if !(specifier.starts_with("./") || specifier.starts_with("../")) {
            eprintln!("cruft agent run: --entry-module specifier must be relative for MVP policy: {specifier:?}");
            return ExitCode::from(64);
        }
        if !admitted_specifiers.contains(specifier) {
            eprintln!(
                "cruft agent run: --entry-module must name an admitted --module specifier: {specifier:?}"
            );
            return ExitCode::from(64);
        }
    }
    let module_entries_js = module_entries.join(",");
    let import_hook_entries_js = import_hook_entries.join(",");
    let module_policy_entries_js = module_policy_entries.join(",");
    let package_imports_js = if package_specs.is_empty() {
        "\"explicit_file_hash_caps_available\""
    } else if admitted_package_graph {
        "\"explicit_package_graph_caps\""
    } else {
        "\"explicit_file_hash_caps\""
    };
    let import_hooks_js = if import_hook_specs.is_empty() {
        "\"source_hash_caps_available\""
    } else {
        "\"source_hash_caps\""
    };
    let entry_module_js = json_optional_string_literal(entry_module.as_deref());
    let session_file_js = json_optional_string_literal(session_file.as_deref());
    let max_steps_js = json_optional_u64_literal(max_steps);
    let memory_rss_js = agent_memory_rss_control_js(max_rss_mb);
    let max_rss_mb_js = json_optional_u64_literal(max_rss_mb);
    let redact_fields_js = json_string_array_literal(&redact_fields);
    let approval_required_tools_js = json_string_array_literal(&approval_required_tools);
    let approved_tools_js = json_string_array_literal(&approved_tools);
    let approval_log_js = json_optional_string_literal(approval_log.as_deref());
    let approval_max_age_ms_js = json_optional_u64_literal(approval_max_age_ms);
    let secret_scopes_js = agent_secret_scopes_js(&secret_scopes);
    let fs_read_caps_js = agent_fs_read_caps_js(&fs_read_files);
    let fs_write_roots_js = agent_fs_write_roots_js(&fs_write_roots);
    let osv_fixture_js = agent_osv_fixture_js(osv_fixture.as_ref().map(|f| f.json.as_str()));
    let model_fixture_js = agent_model_fixture_js(model_fixture.as_ref().map(|f| f.json.as_str()));
    let model_provider_js = json_optional_string_literal(model_provider.as_deref());
    let model_api_key_env_js = json_optional_string_literal(model_api_key_env.as_deref());
    let model_api_key_value_js = json_optional_string_literal(model_api_key_value.as_deref());
    let named_network_cache_dir_js =
        json_optional_string_literal(named_network_cache_dir.as_deref());
    let named_network_cache_mode_js = json_string_literal(&named_network_cache_mode);
    let named_network_cache_max_age_ms_js =
        json_optional_u64_literal(named_network_cache_max_age_ms);
    let named_network_cache_max_entries_js =
        json_optional_u64_literal(named_network_cache_max_entries);
    let named_network_retry_attempts_js = named_network_retry_attempts.to_string();
    let github_token_value = match github_token_env.as_deref() {
        Some(key) => match std::env::var(key) {
            Ok(value) if !value.is_empty() => Some(value),
            Ok(_) => {
                eprintln!("cruft agent run: --github-token-env {key} is empty");
                return ExitCode::from(65);
            }
            Err(_) => {
                eprintln!("cruft agent run: --github-token-env {key} is not set");
                return ExitCode::from(65);
            }
        },
        None => None,
    };
    let github_token_env_js = json_optional_string_literal(github_token_env.as_deref());
    let github_token_value_js = json_optional_string_literal(github_token_value.as_deref());
    let process_commands_js = agent_process_commands_js(&process_commands);
    let process_cwds_js = agent_process_cwds_js(&process_cwds);
    let process_env_js = agent_process_env_js(&process_env);
    let expected_event_schemas_js = agent_expected_event_schemas_js(&expected_event_schemas);
    let allow_echo = tools.iter().any(|t| t == "echo");
    let allow_fail = tools.iter().any(|t| t == "fail");
    let allow_slow = tools.iter().any(|t| t == "slow");
    let allow_osv = tools.iter().any(|t| t == "osv.query");
    let allow_npm_metadata = tools.iter().any(|t| t == "npm.metadata");
    let allow_github_issue_read = tools.iter().any(|t| t == "github.issue.read");
    let allow_github_pr_read = tools.iter().any(|t| t == "github.pr.read");
    let allow_github_pr_files_list = tools.iter().any(|t| t == "github.pr.files.list");
    let allow_github_release_latest_read = tools.iter().any(|t| t == "github.release.latest.read");
    let allow_github_file_read = tools.iter().any(|t| t == "github.file.read");
    let allow_github_compare_read = tools.iter().any(|t| t == "github.compare.read");
    let allow_github_commit_read = tools.iter().any(|t| t == "github.commit.read");
    let allow_github_repo_read = tools.iter().any(|t| t == "github.repo.read");
    let allow_github_workflow_run_read = tools.iter().any(|t| t == "github.workflow.run.read");
    let allow_github_workflow_jobs_list = tools.iter().any(|t| t == "github.workflow.jobs.list");
    let allow_github_check_runs_list = tools.iter().any(|t| t == "github.check.runs.list");
    let allow_model_call = tools.iter().any(|t| t == "model.call");
    let allow_process = tools.iter().any(|t| t == "process");
    let scheduler_await_out_js = json_optional_string_literal(scheduler_await_out.as_deref());
    let harness = format!(
        r#"
const fs = require('node:fs');
const timers = require('node:timers');
const childProcess = require('node:child_process');
const https = require('node:https');
const auditPath = {audit};
const runId = {run_id};
const agentPath = {agent_path};
const agentSource = {agent_source};
const schedulerAwaitOut = {scheduler_await_out};
const entryModule = {entry_module};
const pollutionKey = "__cruft_agent_proto_polluted__";
const globalPollutionKey = "__cruftAgentGlobalPolluted";
const sessionPath = {session_file};
function harden(value, seen) {{
  if (value === null || (typeof value !== "object" && typeof value !== "function")) return value;
  if (seen.indexOf(value) >= 0) return value;
  seen.push(value);
  const names = Object.getOwnPropertyNames(value);
  for (let i = 0; i < names.length; i++) harden(value[names[i]], seen);
  return Object.freeze(value);
}}
const context = harden(JSON.parse({context}), []);
let stateStore = {{}};
let sessionTurn = 0;
let sessionLoaded = false;
const allowEcho = {allow_echo};
const allowFail = {allow_fail};
const allowSlow = {allow_slow};
const allowOsv = {allow_osv};
const allowNpmMetadata = {allow_npm_metadata};
const allowGithubIssueRead = {allow_github_issue_read};
const allowGithubPrRead = {allow_github_pr_read};
const allowGithubPrFilesList = {allow_github_pr_files_list};
const allowGithubReleaseLatestRead = {allow_github_release_latest_read};
const allowGithubFileRead = {allow_github_file_read};
const allowGithubCompareRead = {allow_github_compare_read};
const allowGithubCommitRead = {allow_github_commit_read};
const allowGithubRepoRead = {allow_github_repo_read};
const allowGithubWorkflowRunRead = {allow_github_workflow_run_read};
const allowGithubWorkflowJobsList = {allow_github_workflow_jobs_list};
const allowGithubCheckRunsList = {allow_github_check_runs_list};
const allowModelCall = {allow_model_call};
const allowProcess = {allow_process};
const approvalRequiredTools = Object.freeze({approval_required_tools});
const approvedTools = Object.freeze({approved_tools});
const approvalLogPath = {approval_log};
const approvalMaxAgeMs = {approval_max_age_ms};
const secretScopes = Object.freeze([{secret_scopes}]);
const namedNetworkCacheDir = {named_network_cache_dir};
const namedNetworkCacheMode = {named_network_cache_mode};
const namedNetworkCacheMaxAgeMs = {named_network_cache_max_age_ms};
const namedNetworkCacheMaxEntries = {named_network_cache_max_entries};
const namedNetworkRetryAttempts = {named_network_retry_attempts};
const githubIssueCredentialEnv = {github_token_env};
const githubIssueCredentialToken = {github_token_value};
const osvLiveCache = Object.create(null);
const npmMetadataCache = Object.create(null);
const githubIssueCache = Object.create(null);
const githubPrCache = Object.create(null);
const githubPrFilesCache = Object.create(null);
const githubReleaseLatestCache = Object.create(null);
const githubFileCache = Object.create(null);
const githubCompareCache = Object.create(null);
const githubCommitCache = Object.create(null);
const githubRepoCache = Object.create(null);
const githubWorkflowRunCache = Object.create(null);
const githubWorkflowJobsCache = Object.create(null);
const githubCheckRunsCache = Object.create(null);
const maxEvents = {max_events};
const maxEventBytes = {max_event_bytes};
const maxToolArgBytes = {max_tool_arg_bytes};
const maxToolResultBytes = {max_tool_result_bytes};
const processOutputStreamChunkBytes = {process_output_stream_chunk_bytes};
const toolTimeoutMs = {tool_timeout_ms};
const maxStateBytes = {max_state_bytes};
const maxSteps = {max_steps};
const memoryRssControl = {memory_rss};
const maxRssMb = {max_rss_mb};
const redactFields = {redact_fields};
const modulePolicyEntries = [{module_policy_entries}];
const admittedModules = Object.freeze({{{module_entries}}});
const admittedHookSources = Object.freeze({{{import_hook_entries}}});
const fsReadCaps = Object.freeze([{fs_read_caps}]);
const fsWriteRoots = Object.freeze([{fs_write_roots}]);
const osvFixture = {osv_fixture};
const modelFixture = {model_fixture};
const modelProvider = {model_provider};
const modelApiKeyEnv = {model_api_key_env};
const modelApiKeyToken = {model_api_key_value};
const processCommands = Object.freeze([{process_commands}]);
const processCwds = Object.freeze([{process_cwds}]);
const processEnv = Object.freeze([{process_env}]);
const expectedEventSchemas = Object.freeze([{expected_event_schemas}]);
const admittedModuleSpecifiers = Object.freeze((function() {{
  const out = {{}};
  for (let i = 0; i < modulePolicyEntries.length; i++) out[modulePolicyEntries[i].specifier] = true;
  return out;
}})());
let emittedEvents = 0;
let emittedEventBytes = 0;
let agentClosed = false;
let writtenArtifacts = [];
let runArtifactManifestEmitted = false;
function audit(record) {{
  record.ts_ms = Date.now();
  fs.appendFileSync(auditPath, JSON.stringify(record) + "\n");
}}
function safeMessage(e) {{
  return e && e.message ? String(e.message) : String(e);
}}
fs.writeFileSync(auditPath, "");
if (sessionPath !== null && fs.existsSync(sessionPath)) {{
  const sessionEnvelope = JSON.parse(fs.readFileSync(sessionPath, "utf8"));
  if (sessionEnvelope === null || typeof sessionEnvelope !== "object" || Array.isArray(sessionEnvelope) || sessionEnvelope.state === null || typeof sessionEnvelope.state !== "object" || Array.isArray(sessionEnvelope.state)) {{
    throw new Error("agent session file must contain a JSON object envelope with object state");
  }}
  stateStore = cloneStateValue(sessionEnvelope.state);
  sessionTurn = Number(sessionEnvelope.turn_id || 0);
  sessionLoaded = true;
}} else {{
  stateStore = JSON.parse({state});
}}
if (stateStore === null || typeof stateStore !== "object" || Array.isArray(stateStore)) {{
  throw new Error("agent state must be a JSON object");
}}
if (sessionPath !== null) {{
  audit({{type:"session_load", run_id:runId, path:sessionPath, existed:sessionLoaded, turn_id:sessionTurn, state_bytes:stateBytes()}});
}}
audit({{
  type:"run_start",
  run_id:runId,
  mouth:"cruft agent run",
  agent:agentPath,
  timeout_ms:{timeout_ms},
  tools: [allowEcho ? "echo" : null, allowFail ? "fail" : null, allowSlow ? "slow" : null, allowOsv ? "osv.query" : null, allowNpmMetadata ? "npm.metadata" : null, allowGithubIssueRead ? "github.issue.read" : null, allowGithubPrRead ? "github.pr.read" : null, allowGithubPrFilesList ? "github.pr.files.list" : null, allowGithubReleaseLatestRead ? "github.release.latest.read" : null, allowGithubFileRead ? "github.file.read" : null, allowGithubCompareRead ? "github.compare.read" : null, allowGithubCommitRead ? "github.commit.read" : null, allowGithubRepoRead ? "github.repo.read" : null, allowGithubWorkflowRunRead ? "github.workflow.run.read" : null, allowGithubWorkflowJobsList ? "github.workflow.jobs.list" : null, allowGithubCheckRunsList ? "github.check.runs.list" : null, allowModelCall ? "model.call" : null, allowProcess ? "process" : null].filter(Boolean),
  secret_scopes:secretScopes.map(function(s) {{ return {{tool:s.tool, credential_mode:s.credential_mode, credential_env:s.credential_env}}; }}),
  approval_gates:{{mode:approvalRequiredTools.length === 0 ? "not_configured" : "pre_effect_required", required_tools:approvalRequiredTools.slice(), approved_tools:approvedTools.slice(), approval_log_configured:approvalLogPath !== null, approval_max_age_ms:approvalMaxAgeMs, resume:approvalLogPath === null ? "pregrant_only" : "approval_log_decision_replay"}},
  fs_read:{{mode:fsReadCaps.length === 0 ? "not_configured" : "path_caps_with_byte_budget", files:fsReadCaps.length}},
  fs_read_source_manifest:fsReadCaps.map(function(f) {{ return {{path:f.relative, bytes:f.bytes, kind:f.kind, readable:f.readable, reason:f.reason, source_hash:f.source_hash || null}}; }}),
  artifact_write:{{mode:fsWriteRoots.length === 0 ? "not_configured" : "path_caps_with_byte_budget", roots:fsWriteRoots.length}},
  osv_query:{{mode:allowOsv ? (osvFixture === null ? "live_pinned_https" : "fixture_backed_named_lookup") : "not_configured", endpoint:osvFixture === null ? "https://api.osv.dev/v1/query" : "fixture://osv/v1/query"}},
  named_network_cache:{{mode:namedNetworkCacheDir === null ? "run_memory_only" : "persistent_directory", configured:namedNetworkCacheDir !== null, cache_mode:namedNetworkCacheMode, max_age_ms:namedNetworkCacheMaxAgeMs, max_entries:namedNetworkCacheMaxEntries}},
  named_network_retry:{{policy:namedNetworkRetryAttempts === 0 ? "none" : "bounded", max_attempts:namedNetworkRetryAttempts}},
  npm_metadata:{{mode:allowNpmMetadata ? "live_pinned_https" : "not_configured", endpoint:"https://registry.npmjs.org/<package>"}},
  github_issue_read:{{mode:allowGithubIssueRead ? "live_pinned_https" : "not_configured", endpoint:"https://api.github.com/repos/<owner>/<repo>/issues/<number>", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}},
  github_pr_read:{{mode:allowGithubPrRead ? "live_pinned_https" : "not_configured", endpoint:"https://api.github.com/repos/<owner>/<repo>/pulls/<number>", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}},
  github_pr_files_list:{{mode:allowGithubPrFilesList ? "live_pinned_https" : "not_configured", endpoint:"https://api.github.com/repos/<owner>/<repo>/pulls/<number>/files", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}},
  github_release_latest_read:{{mode:allowGithubReleaseLatestRead ? "live_pinned_https" : "not_configured", endpoint:"https://api.github.com/repos/<owner>/<repo>/releases/latest", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}},
  github_file_read:{{mode:allowGithubFileRead ? "live_pinned_https" : "not_configured", endpoint:"https://api.github.com/repos/<owner>/<repo>/contents/<path>?ref=<ref>", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}},
  github_compare_read:{{mode:allowGithubCompareRead ? "live_pinned_https" : "not_configured", endpoint:"https://api.github.com/repos/<owner>/<repo>/compare/<base>...<head>", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}},
  github_commit_read:{{mode:allowGithubCommitRead ? "live_pinned_https" : "not_configured", endpoint:"https://api.github.com/repos/<owner>/<repo>/commits/<ref>", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}},
  github_repo_read:{{mode:allowGithubRepoRead ? "live_pinned_https" : "not_configured", endpoint:"https://api.github.com/repos/<owner>/<repo>", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}},
  github_workflow_run_read:{{mode:allowGithubWorkflowRunRead ? "live_pinned_https" : "not_configured", endpoint:"https://api.github.com/repos/<owner>/<repo>/actions/runs/<run_id>", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}},
  github_workflow_jobs_list:{{mode:allowGithubWorkflowJobsList ? "live_pinned_https" : "not_configured", endpoint:"https://api.github.com/repos/<owner>/<repo>/actions/runs/<run_id>/jobs", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}},
  github_check_runs_list:{{mode:allowGithubCheckRunsList ? "live_pinned_https" : "not_configured", endpoint:"https://api.github.com/repos/<owner>/<repo>/commits/<ref>/check-runs", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}},
  model_call:{{mode:allowModelCall ? (modelFixture === null ? "provider_openai_responses" : "fixture_backed_named_model_tool") : "not_configured", endpoint:modelFixture === null ? "https://api.openai.com/v1/responses" : "fixture://model/call", credential_mode:modelApiKeyEnv === null ? "none" : "host_env_bearer", credential_env:modelApiKeyEnv, disposition:modelAuditDisposition()}},
  process_tools:{{mode:allowProcess ? "argv_policy_supervised_sync" : "not_configured", commands:processCommands.map(function(c) {{ return c.name; }}), cwd_roots:processCwds.length, env_keys:processEnv.map(function(e) {{ return e.key; }}), output_stream_chunk_bytes:processOutputStreamChunkBytes}},
  expected_events:expectedEventSchemas.map(function(e) {{ return e.kind; }}),
  event_budget:{{max_events:maxEvents, max_event_bytes:maxEventBytes}},
  audit_redaction:{{common_fields:["secret","token","password","api_key"], policy_fields:redactFields}},
  tool_payload_budget:{{max_arg_bytes:maxToolArgBytes, max_result_bytes:maxToolResultBytes, process_output_stream_chunk_bytes:processOutputStreamChunkBytes}},
  resource_controls:{{clone_payload_bytes:"enforced", memory_rss:memoryRssControl, max_rss_mb:maxRssMb}},
  module_policy:{{mode:"explicit", entrypoint:entryModule === null ? "script" : "static_module", entry_module:entryModule, admitted:modulePolicyEntries, package_imports:{package_imports}, import_hooks:{import_hooks}}},
  state_controls:{{mode:"single_turn_snapshot", max_state_bytes:maxStateBytes, reset:"single_turn_store", resume:sessionPath === null ? "not_available" : "file_session", close_revocation:"sync_and_promise_turn"}},
  availability_controls:{{
    sync_timeout:"enforced",
    tenant_timeout_catchability:"uncatchable_gate",
    microtask_budget:"enforced",
    max_microtasks:{max_microtasks},
    pending_promise_disposition:"detached_at_turn_end",
    timer_handles:"not_endowed_use_scheduler_sleep_protocol",
    async_tool_timeout:"enforced_for_cli_tool_promises",
    tool_timeout_ms:toolTimeoutMs,
    step_budget:maxSteps === null ? "available_with_--max-steps" : "enforced",
    max_steps:maxSteps
  }}
}});
function cloneJsonish(value, label) {{
  if (value === null || typeof value !== "object" || Array.isArray(value)) {{
    throw new TypeError(label + " must be a JSON object");
  }}
  return JSON.parse(JSON.stringify(value));
}}
function cloneStateValue(value) {{
  const encoded = JSON.stringify(value);
  if (typeof encoded !== "string") throw new TypeError("state value must be JSON-serializable");
  return JSON.parse(encoded);
}}
function payloadBytes(value) {{
  return JSON.stringify(value).length;
}}
function enforcePayloadBudget(kind, tool, value, limit) {{
  const bytes = payloadBytes(value);
  if (bytes > limit) {{
    audit({{type:"budget_exceeded", budget:kind, tool, limit, attempted:bytes}});
    throw new Error("agent " + kind + " budget exceeded");
  }}
  return bytes;
}}
function stateBytes() {{
  return JSON.stringify(stateStore).length;
}}
function enforceStateBudget() {{
  const bytes = stateBytes();
  if (bytes > maxStateBytes) {{
    audit({{type:"budget_exceeded", budget:"state_bytes", limit:maxStateBytes, attempted:bytes}});
    throw new Error("agent state budget exceeded");
  }}
  return bytes;
}}
function enforceCandidateStateBudget(candidate) {{
  const bytes = JSON.stringify(candidate).length;
  if (bytes > maxStateBytes) {{
    audit({{type:"budget_exceeded", budget:"state_bytes", limit:maxStateBytes, attempted:bytes}});
    throw new Error("agent state budget exceeded");
  }}
  return bytes;
}}
function schemaTypeOf(value) {{
  if (value === null) return "null";
  if (Array.isArray(value)) return "array";
  return typeof value;
}}
function validateAgainstExpectedEventSchema(kind, event) {{
  for (let i = 0; i < expectedEventSchemas.length; i++) {{
    const expectation = expectedEventSchemas[i];
    if (expectation.kind !== kind) continue;
    const schema = expectation.schema || {{}};
    const required = Array.isArray(schema.required) ? schema.required : [];
    for (let r = 0; r < required.length; r++) {{
      const field = String(required[r]);
      if (!Object.prototype.hasOwnProperty.call(event, field)) {{
        audit({{type:"schema_validation", kind, status:"fail", reason:"missing_required", field}});
        throw new Error("agent event schema validation failed: " + kind + " missing " + field);
      }}
    }}
    const properties = schema.properties && typeof schema.properties === "object" && !Array.isArray(schema.properties) ? schema.properties : {{}};
    const propertyNames = Object.getOwnPropertyNames(properties);
    for (let p = 0; p < propertyNames.length; p++) {{
      const field = propertyNames[p];
      if (!Object.prototype.hasOwnProperty.call(event, field)) continue;
      const expected = String(properties[field]);
      if (expected === "any") continue;
      const actual = schemaTypeOf(event[field]);
      if (actual !== expected) {{
        audit({{type:"schema_validation", kind, status:"fail", reason:"type_mismatch", field, expected, actual}});
        throw new Error("agent event schema validation failed: " + kind + " " + field);
      }}
    }}
    if (schema.additional_properties === false) {{
      const eventNames = Object.getOwnPropertyNames(event);
      for (let e = 0; e < eventNames.length; e++) {{
        const field = eventNames[e];
        if (field === "kind") continue;
        if (!Object.prototype.hasOwnProperty.call(properties, field)) {{
          audit({{type:"schema_validation", kind, status:"fail", reason:"additional_property", field}});
          throw new Error("agent event schema validation failed: " + kind + " additional " + field);
        }}
      }}
    }}
    audit({{type:"schema_validation", kind, status:"pass"}});
  }}
}}
function ensureOpen(surface) {{
  if (agentClosed) {{
    audit({{type:"revocation_denial", surface, policy:"closed"}});
    throw new Error("agent compartment closed: " + surface);
  }}
}}
function redact(value) {{
  if (value === null || typeof value !== "object") return value;
  if (Array.isArray(value)) {{
    const out = [];
    for (let i = 0; i < value.length; i++) out.push(redact(value[i]));
    return out;
  }}
  const out = {{}};
  const names = Object.getOwnPropertyNames(value);
  for (let i = 0; i < names.length; i++) {{
    const k = names[i];
    out[k] = k === "secret" || k === "token" || k === "password" || k === "api_key" || redactFields.indexOf(k) >= 0 ? "[redacted]" : redact(value[k]);
  }}
  return out;
}}
function modelAuditDisposition() {{
  return {{
    transcript_persistence:"metadata_and_redacted_payload_fields",
    prompt_disposition:"redacted_audit_fields_only",
    output_disposition:"redacted_audit_fields_only",
    raw_prompt_persisted:false,
    raw_output_persisted:false
  }};
}}
function validateEvent(event) {{
  if (event === null || typeof event !== "object" || Array.isArray(event)) {{
    throw new TypeError("agent event must be a JSON object");
  }}
  return cloneJsonish(event, "agent event");
}}
function emitBudgeted(event) {{
  const cloned = validateEvent(event);
  const payload = JSON.stringify(cloned);
  const bytes = payload.length;
  if (emittedEvents + 1 > maxEvents) {{
    audit({{type:"budget_exceeded", budget:"events", limit:maxEvents, used:emittedEvents, attempted:emittedEvents + 1}});
    throw new Error("agent event budget exceeded: events");
  }}
  if (emittedEventBytes + bytes > maxEventBytes) {{
    audit({{type:"budget_exceeded", budget:"event_bytes", limit:maxEventBytes, used:emittedEventBytes, attempted:emittedEventBytes + bytes}});
    throw new Error("agent event budget exceeded: event_bytes");
  }}
  emittedEvents++;
  emittedEventBytes += bytes;
  audit({{type:"event", event:redact(cloned), event_index:emittedEvents, event_bytes:bytes, event_bytes_used:emittedEventBytes}});
  if (typeof cloned.kind === "string") validateAgainstExpectedEventSchema(cloned.kind, cloned);
}}
function approvalRequiredFor(name) {{
  return approvalRequiredTools.indexOf(name) >= 0 || approvalRequiredTools.indexOf("*") >= 0;
}}
function approvalGrantedFor(name) {{
  return approvedTools.indexOf(name) >= 0 || approvedTools.indexOf("*") >= 0;
}}
function approvalRequestId(name, args) {{
  return artifactHash(name + "\n" + JSON.stringify(args));
}}
function appendApprovalRecord(record) {{
  if (approvalLogPath === null) return;
  const parent = require("node:path").dirname(approvalLogPath);
  fs.mkdirSync(parent, {{recursive:true}});
  fs.appendFileSync(approvalLogPath, JSON.stringify(record) + "\n");
}}
function approvalDecisionFor(id) {{
  if (approvalLogPath === null || !fs.existsSync(approvalLogPath)) return null;
  const lines = fs.readFileSync(approvalLogPath, "utf8").split(/\n/);
  let decision = null;
  const now = Date.now();
  for (let i = 0; i < lines.length; i++) {{
    const line = lines[i].trim();
    if (line.length === 0) continue;
    try {{
      const record = JSON.parse(line);
      if (record && record.type === "agent_approval_decision" && record.id === id && (record.status === "allowed" || record.status === "denied")) {{
        decision = {{status:record.status, created_at_ms:Number(record.created_at_ms || 0)}};
      }}
    }} catch (_) {{}}
  }}
  if (decision !== null && approvalMaxAgeMs !== null) {{
    const ageMs = now - decision.created_at_ms;
    if (!Number.isFinite(ageMs) || decision.created_at_ms <= 0 || ageMs > approvalMaxAgeMs) {{
      return {{status:"stale", created_at_ms:decision.created_at_ms, age_ms:ageMs, max_age_ms:approvalMaxAgeMs}};
    }}
    decision.age_ms = ageMs;
    decision.max_age_ms = approvalMaxAgeMs;
  }}
  return decision;
}}
function requireToolApproval(name, args, argBytes) {{
  if (!approvalRequiredFor(name)) return;
  const approvalId = approvalRequestId(name, args);
  if (approvalGrantedFor(name)) {{
    audit({{type:"tool_approval_granted", tool:name, approval_id:approvalId, policy:"approved", approval_mode:"pregranted", args:redact(args), arg_bytes:argBytes}});
    return;
  }}
  const decision = approvalDecisionFor(approvalId);
  if (decision && decision.status === "allowed") {{
    audit({{type:"tool_approval_granted", tool:name, approval_id:approvalId, policy:"approved", approval_mode:"approval_log", approval_log:approvalLogPath, approval_age_ms:decision.age_ms, approval_max_age_ms:decision.max_age_ms, args:redact(args), arg_bytes:argBytes}});
    return;
  }}
  if (decision && decision.status === "denied") {{
    audit({{type:"tool_approval_denied", tool:name, approval_id:approvalId, policy:"denied", approval_mode:"approval_log", approval_log:approvalLogPath, approval_age_ms:decision.age_ms, approval_max_age_ms:decision.max_age_ms, args:redact(args), arg_bytes:argBytes}});
    throw new Error("agent tool approval denied: " + name);
  }}
  if (decision && decision.status === "stale") {{
    audit({{type:"tool_approval_stale", tool:name, approval_id:approvalId, policy:"denied", approval_mode:"approval_log", approval_log:approvalLogPath, approval_age_ms:decision.age_ms, approval_max_age_ms:decision.max_age_ms, args:redact(args), arg_bytes:argBytes}});
    throw new Error("agent tool approval stale: " + name);
  }}
  appendApprovalRecord({{type:"agent_approval_pending", id:approvalId, tool:name, args:redact(args), arg_bytes:argBytes, status:"pending"}});
  audit({{type:"tool_approval_pending", tool:name, approval_id:approvalId, policy:"pending", reason:"approval_required", approval_mode:"pre_effect_required", resume:approvalLogPath === null ? "pregrant_only" : "approval_log", approval_log:approvalLogPath, args:redact(args), arg_bytes:argBytes}});
  throw new Error("agent tool approval required: " + name);
}}
function findFsReadFile(requested) {{
  requested = String(requested);
  for (let i = 0; i < fsReadCaps.length; i++) {{
    const f = fsReadCaps[i];
    if (requested === f.path || requested === f.relative || requested === "./" + f.relative) return f;
  }}
  return null;
}}
function artifactHash(content) {{
  let h = 0xcbf29ce484222325n;
  const s = String(content);
  for (let i = 0; i < s.length; i++) {{
    h ^= BigInt(s.charCodeAt(i) & 0xff);
    h = BigInt.asUintN(64, h * 0x100000001b3n);
  }}
  return "fnv1a64:" + h.toString(16).padStart(16, "0");
}}
function emitRunArtifactManifest(status, reason) {{
  if (runArtifactManifestEmitted) return;
  runArtifactManifestEmitted = true;
  audit({{
    type:"run_artifact_manifest",
    version:1,
    run_id:runId,
    status,
    reason:reason || null,
    policy:{{mouth:"cruft agent run", audit_log:auditPath, run_id:runId}},
    source:{{agent:{{path:agentPath, source_hash:artifactHash(agentSource)}}, entry_module:entryModule, modules:modulePolicyEntries.map(function(m) {{ return {{specifier:m.specifier, source_hash:m.source_hash || null}}; }}), fs_read_source_manifest:fsReadCaps.map(function(f) {{ return {{path:f.relative, bytes:f.bytes, kind:f.kind, readable:f.readable, reason:f.reason, source_hash:f.source_hash || null}}; }})}},
    tools:[allowEcho ? "echo" : null, allowFail ? "fail" : null, allowSlow ? "slow" : null, allowOsv ? "osv.query" : null, allowNpmMetadata ? "npm.metadata" : null, allowGithubIssueRead ? "github.issue.read" : null, allowGithubPrRead ? "github.pr.read" : null, allowGithubPrFilesList ? "github.pr.files.list" : null, allowGithubReleaseLatestRead ? "github.release.latest.read" : null, allowGithubFileRead ? "github.file.read" : null, allowGithubCompareRead ? "github.compare.read" : null, allowGithubCommitRead ? "github.commit.read" : null, allowGithubRepoRead ? "github.repo.read" : null, allowGithubWorkflowRunRead ? "github.workflow.run.read" : null, allowGithubWorkflowJobsList ? "github.workflow.jobs.list" : null, allowGithubCheckRunsList ? "github.check.runs.list" : null, allowModelCall ? "model.call" : null, allowProcess ? "process" : null].filter(Boolean),
    approvals:{{required_tools:approvalRequiredTools.slice(), approved_tools:approvedTools.slice(), approval_log_configured:approvalLogPath !== null}},
    secret_scopes:secretScopes.map(function(s) {{ return {{tool:s.tool, credential_mode:s.credential_mode, credential_env:s.credential_env}}; }}),
    named_network_cache:{{configured:namedNetworkCacheDir !== null, cache_mode:namedNetworkCacheMode, max_age_ms:namedNetworkCacheMaxAgeMs, max_entries:namedNetworkCacheMaxEntries}},
    named_network_retry:{{max_attempts:namedNetworkRetryAttempts}},
    model_call:{{configured:allowModelCall, mode:allowModelCall ? (modelFixture === null ? "provider_openai_responses" : "fixture_backed_named_model_tool") : "not_configured", endpoint:allowModelCall ? (modelFixture === null ? "https://api.openai.com/v1/responses" : "fixture://model/call") : null, disposition:modelAuditDisposition()}},
    artifacts:writtenArtifacts.slice(),
    budgets:{{max_events:maxEvents, max_event_bytes:maxEventBytes, max_tool_arg_bytes:maxToolArgBytes, max_tool_result_bytes:maxToolResultBytes, process_output_stream_chunk_bytes:processOutputStreamChunkBytes, tool_timeout_ms:toolTimeoutMs, max_state_bytes:maxStateBytes, max_steps:maxSteps, memory_rss:memoryRssControl, max_rss_mb:maxRssMb}},
    replay:{{audit_log:auditPath, run_id:runId, bundle_file:"run-artifact-manifest.json"}}
  }});
}}
function normalizeArtifactPath(requested) {{
  requested = String(requested);
  if (requested.length === 0 || requested.charAt(0) === "/" || requested.indexOf("\\") >= 0) return null;
  const parts = requested.split("/");
  const out = [];
  for (let i = 0; i < parts.length; i++) {{
    const part = parts[i];
    if (part.length === 0 || part === ".") continue;
    if (part === "..") return null;
    out.push(part);
  }}
  if (out.length === 0) return null;
  return out.join("/");
}}
function validateOsvQueryArgs(arg) {{
  const args = cloneJsonish(arg, "osv.query args");
  if (args.package === null || typeof args.package !== "object" || Array.isArray(args.package)) {{
    throw new TypeError("osv.query args package must be an object");
  }}
  if (typeof args.package.ecosystem !== "string" || args.package.ecosystem.length === 0 || typeof args.package.name !== "string" || args.package.name.length === 0) {{
    throw new TypeError("osv.query args package.ecosystem and package.name must be non-empty strings");
  }}
  if (args.version !== undefined && (typeof args.version !== "string" || args.version.length === 0)) {{
    throw new TypeError("osv.query args version must be a non-empty string when present");
  }}
  return {{package:{{ecosystem:args.package.ecosystem, name:args.package.name}}, version:args.version === undefined ? null : args.version}};
}}
function osvFixtureLookup(args) {{
  if (osvFixture === null || !Array.isArray(osvFixture.queries)) return null;
  for (let i = 0; i < osvFixture.queries.length; i++) {{
    const q = osvFixture.queries[i];
    if (!q || !q.package) continue;
    const version = q.version === undefined ? null : q.version;
    if (q.package.ecosystem === args.package.ecosystem && q.package.name === args.package.name && version === args.version) return cloneStateValue(q.response || {{}});
  }}
  return {{vulns:[]}};
}}
function osvLiveQuery(args) {{
  const body = JSON.stringify({{package:args.package, version:args.version === null ? undefined : args.version}});
  const started = Date.now();
  return new Promise(function(resolve, reject) {{
    let settled = false;
    const req = https.request({{
      protocol: "https:",
      hostname: "api.osv.dev",
      port: 443,
      method: "POST",
      path: "/v1/query",
      timeout: toolTimeoutMs,
      headers: {{
        "content-type": "application/json",
        "content-length": Buffer.byteLength(body),
        "user-agent": "cruft-agent-osv-query"
      }}
    }}, function(res) {{
      const chunks = [];
      let bytes = 0;
      res.on("data", function(chunk) {{
        bytes += chunk.length;
        if (bytes > maxToolResultBytes) {{
          settled = true;
          reject(new Error("agent osv response byte budget exceeded"));
          req.destroy();
          return;
        }}
        chunks.push(chunk);
      }});
      res.on("end", function() {{
        if (settled) return;
        settled = true;
        const text = Buffer.concat(chunks).toString("utf8");
        if (res.statusCode < 200 || res.statusCode >= 300) {{
          reject(new Error("agent osv live http status " + res.statusCode));
          return;
        }}
        try {{
          const result = JSON.parse(text);
          if (result !== null && typeof result === "object" && !Array.isArray(result) && !Array.isArray(result.vulns)) {{
            result.vulns = [];
          }}
          resolve({{result, response_bytes:bytes, status_code:res.statusCode, duration_ms:Date.now() - started}});
        }} catch (e) {{
          reject(new Error("agent osv live invalid json"));
        }}
      }});
    }});
    req.on("timeout", function() {{
      if (settled) return;
      settled = true;
      req.destroy(new Error("agent osv live timeout"));
    }});
    req.on("error", function(e) {{
      if (settled) return;
      settled = true;
      reject(e);
    }});
    req.write(body);
    req.end();
  }});
}}
function namedNetworkErrorTaxonomy(e) {{
  const message = safeMessage(e);
  if (message.indexOf("response byte budget exceeded") >= 0) return {{error_kind:"response_byte_budget", retryable:false}};
  if (message.indexOf("timeout") >= 0) return {{error_kind:"timeout", retryable:true}};
  if (message.indexOf("http status 429") >= 0 || message.indexOf("http status 5") >= 0) return {{error_kind:"http_retryable", retryable:true}};
  if (message.indexOf("http status") >= 0) return {{error_kind:"http_nonretryable", retryable:false}};
  if (message.indexOf("invalid json") >= 0) return {{error_kind:"invalid_json", retryable:false}};
  return {{error_kind:"transport_error", retryable:true}};
}}
function namedNetworkRetryPolicy() {{
  return namedNetworkRetryAttempts === 0 ? "none" : "bounded";
}}
function namedNetworkRetryInfo(e) {{
  if (e && e.__cruftNamedNetworkRetry) return e.__cruftNamedNetworkRetry;
  return {{attempts:0, policy:namedNetworkRetryPolicy(), max_attempts:namedNetworkRetryAttempts}};
}}
function namedNetworkRetryBackoffMs(attempt) {{
  if (attempt <= 0) return 0;
  return Math.min(250, 25 * attempt);
}}
function namedNetworkLiveWithRetry(tool, endpoint, query, args, started, attempt) {{
  return query(args).catch(function(e) {{
    const taxonomy = namedNetworkErrorTaxonomy(e);
    if (taxonomy.retryable && attempt < namedNetworkRetryAttempts) {{
      const nextAttempt = attempt + 1;
      const backoffMs = namedNetworkRetryBackoffMs(nextAttempt);
      audit({{type:"named_network_retry", tool, endpoint, transport:"pinned_https", attempt:nextAttempt, max_attempts:namedNetworkRetryAttempts, error_kind:taxonomy.error_kind, retryable:taxonomy.retryable, retry_policy:namedNetworkRetryPolicy(), backoff_ms:backoffMs, elapsed_ms:Date.now() - started}});
      return namedNetworkLiveWithRetry(tool, endpoint, query, args, started, nextAttempt);
    }}
    if (e && typeof e === "object") {{
      e.__cruftNamedNetworkRetry = {{attempts:attempt, policy:namedNetworkRetryPolicy(), max_attempts:namedNetworkRetryAttempts}};
    }}
    throw e;
  }});
}}
function validateNpmMetadataArgs(arg) {{
  const args = cloneJsonish(arg, "npm.metadata args");
  if (typeof args.package !== "string" || args.package.length === 0) {{
    throw new TypeError("npm.metadata args package must be a non-empty string");
  }}
  const name = args.package;
  if (name.length > 214 || name.indexOf("\\") >= 0 || name.indexOf(" ") >= 0 || name.indexOf("..") >= 0) {{
    throw new TypeError("npm.metadata args package must be a bounded npm package name");
  }}
  if (name.charAt(0) === "@") {{
    const parts = name.split("/");
    if (parts.length !== 2 || parts[0].length < 2 || parts[1].length === 0 || parts[1].indexOf("/") >= 0) {{
      throw new TypeError("npm.metadata scoped package must be @scope/name");
    }}
  }} else if (name.indexOf("/") >= 0) {{
    throw new TypeError("npm.metadata package must not contain path separators unless scoped");
  }}
  return {{package:name}};
}}
function npmMetadataPath(name) {{
  if (name.charAt(0) === "@") {{
    const slash = name.indexOf("/");
    return "/" + encodeURIComponent(name.slice(0, slash)) + "%2f" + encodeURIComponent(name.slice(slash + 1));
  }}
  return "/" + encodeURIComponent(name);
}}
function npmMetadataLiveQuery(args) {{
  const requestPath = npmMetadataPath(args.package);
  const endpoint = "https://registry.npmjs.org" + requestPath;
  const started = Date.now();
  return new Promise(function(resolve, reject) {{
    let settled = false;
    const req = https.request({{
      protocol: "https:",
      hostname: "registry.npmjs.org",
      port: 443,
      method: "GET",
      path: requestPath,
      timeout: toolTimeoutMs,
      headers: {{
        "accept": "application/vnd.npm.install-v1+json; q=1.0, application/json; q=0.8, */*",
        "user-agent": "cruft-agent-npm-metadata"
      }}
    }}, function(res) {{
      const chunks = [];
      let bytes = 0;
      res.on("data", function(chunk) {{
        bytes += chunk.length;
        if (bytes > maxToolResultBytes) {{
          settled = true;
          reject(new Error("agent npm metadata response byte budget exceeded"));
          req.destroy();
          return;
        }}
        chunks.push(chunk);
      }});
      res.on("end", function() {{
        if (settled) return;
        settled = true;
        const text = Buffer.concat(chunks).toString("utf8");
        if (res.statusCode < 200 || res.statusCode >= 300) {{
          reject(new Error("agent npm metadata http status " + res.statusCode));
          return;
        }}
        try {{
          const result = JSON.parse(text);
          resolve({{result, endpoint, response_bytes:bytes, status_code:res.statusCode, duration_ms:Date.now() - started}});
        }} catch (e) {{
          reject(new Error("agent npm metadata invalid json"));
        }}
      }});
    }});
    req.on("timeout", function() {{
      if (settled) return;
      settled = true;
      req.destroy(new Error("agent npm metadata timeout"));
    }});
    req.on("error", function(e) {{
      if (settled) return;
      settled = true;
      reject(e);
    }});
    req.end();
  }});
}}
function validateGithubIssueReadArgs(arg) {{
  const args = cloneJsonish(arg, "github.issue.read args");
  if (typeof args.owner !== "string" || typeof args.repo !== "string") {{
    throw new TypeError("github.issue.read args owner and repo must be strings");
  }}
  const owner = args.owner;
  const repo = args.repo;
  const number = Number(args.number);
  const nameRe = /^[A-Za-z0-9_.-]+$/;
  if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) {{
    throw new TypeError("github.issue.read args owner and repo must be bounded GitHub path components");
  }}
  if (!Number.isInteger(number) || number <= 0 || number > 2147483647) {{
    throw new TypeError("github.issue.read args number must be a positive integer");
  }}
  return {{owner, repo, number}};
}}
function validateGithubPrReadArgs(arg) {{
  const args = cloneJsonish(arg, "github.pr.read args");
  if (typeof args.owner !== "string" || typeof args.repo !== "string") {{
    throw new TypeError("github.pr.read args owner and repo must be strings");
  }}
  const owner = args.owner;
  const repo = args.repo;
  const number = Number(args.number);
  const nameRe = /^[A-Za-z0-9_.-]+$/;
  if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) {{
    throw new TypeError("github.pr.read args owner and repo must be bounded GitHub path components");
  }}
  if (!Number.isInteger(number) || number <= 0 || number > 2147483647) {{
    throw new TypeError("github.pr.read args number must be a positive integer");
  }}
  return {{owner, repo, number}};
}}
function validateGithubPrFilesListArgs(arg) {{
  const args = cloneJsonish(arg, "github.pr.files.list args");
  if (typeof args.owner !== "string" || typeof args.repo !== "string") {{
    throw new TypeError("github.pr.files.list args owner and repo must be strings");
  }}
  const owner = args.owner;
  const repo = args.repo;
  const number = Number(args.number);
  const nameRe = /^[A-Za-z0-9_.-]+$/;
  if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) {{
    throw new TypeError("github.pr.files.list args owner and repo must be bounded GitHub path components");
  }}
  if (!Number.isInteger(number) || number <= 0 || number > 2147483647) {{
    throw new TypeError("github.pr.files.list args number must be a positive integer");
  }}
  return {{owner, repo, number}};
}}
function validateGithubRepoReadArgs(arg) {{
  const args = cloneJsonish(arg, "github.repo.read args");
  if (typeof args.owner !== "string" || typeof args.repo !== "string") {{
    throw new TypeError("github.repo.read args owner and repo must be strings");
  }}
  const owner = args.owner;
  const repo = args.repo;
  const nameRe = /^[A-Za-z0-9_.-]+$/;
  if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) {{
    throw new TypeError("github.repo.read args owner and repo must be bounded GitHub path components");
  }}
  return {{owner, repo}};
}}
function validateGithubReleaseLatestReadArgs(arg) {{
  const args = cloneJsonish(arg, "github.release.latest.read args");
  if (typeof args.owner !== "string" || typeof args.repo !== "string") {{
    throw new TypeError("github.release.latest.read args owner and repo must be strings");
  }}
  const owner = args.owner;
  const repo = args.repo;
  const nameRe = /^[A-Za-z0-9_.-]+$/;
  if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) {{
    throw new TypeError("github.release.latest.read args owner and repo must be bounded GitHub path components");
  }}
  return {{owner, repo}};
}}
function validateGithubFileReadArgs(arg) {{
  const args = cloneJsonish(arg, "github.file.read args");
  if (typeof args.owner !== "string" || typeof args.repo !== "string" || typeof args.path !== "string") {{
    throw new TypeError("github.file.read args owner, repo, and path must be strings");
  }}
  const owner = args.owner;
  const repo = args.repo;
  const path = args.path;
  const ref = args.ref === undefined ? null : args.ref;
  const nameRe = /^[A-Za-z0-9_.-]+$/;
  if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) {{
    throw new TypeError("github.file.read args owner and repo must be bounded GitHub path components");
  }}
  if (path.length === 0 || path.length > 1024 || path.charAt(0) === "/" || path.indexOf("\\\\") >= 0 || path.split("/").some(function(part) {{ return part.length === 0 || part === "." || part === ".."; }})) {{
    throw new TypeError("github.file.read args path must be a bounded relative repository path");
  }}
  if (ref !== null && (typeof ref !== "string" || ref.length === 0 || ref.length > 200 || ref.indexOf("\\\\") >= 0 || ref.indexOf("..") >= 0)) {{
    throw new TypeError("github.file.read args ref must be a bounded ref string when present");
  }}
  const result = {{owner, repo, path}};
  if (ref !== null) result.ref = ref;
  return result;
}}
function validateGithubCompareReadArgs(arg) {{
  const args = cloneJsonish(arg, "github.compare.read args");
  if (typeof args.owner !== "string" || typeof args.repo !== "string" || typeof args.base !== "string" || typeof args.head !== "string") {{
    throw new TypeError("github.compare.read args owner, repo, base, and head must be strings");
  }}
  const owner = args.owner;
  const repo = args.repo;
  const base = args.base;
  const head = args.head;
  const nameRe = /^[A-Za-z0-9_.-]+$/;
  const refRe = /^[A-Za-z0-9_./-]+$/;
  if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) {{
    throw new TypeError("github.compare.read args owner and repo must be bounded GitHub path components");
  }}
  if (base.length === 0 || base.length > 200 || head.length === 0 || head.length > 200 || base.indexOf("..") >= 0 || head.indexOf("..") >= 0 || base.indexOf("\\\\") >= 0 || head.indexOf("\\\\") >= 0 || !refRe.test(base) || !refRe.test(head)) {{
    throw new TypeError("github.compare.read args base and head must be bounded GitHub refs");
  }}
  return {{owner, repo, base, head}};
}}
function validateGithubCommitReadArgs(arg) {{
  const args = cloneJsonish(arg, "github.commit.read args");
  if (typeof args.owner !== "string" || typeof args.repo !== "string" || typeof args.ref !== "string") {{
    throw new TypeError("github.commit.read args owner, repo, and ref must be strings");
  }}
  const owner = args.owner;
  const repo = args.repo;
  const ref = args.ref;
  const nameRe = /^[A-Za-z0-9_.-]+$/;
  const refRe = /^[A-Za-z0-9_./-]+$/;
  if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) {{
    throw new TypeError("github.commit.read args owner and repo must be bounded GitHub path components");
  }}
  if (ref.length === 0 || ref.length > 200 || ref.indexOf("..") >= 0 || ref.indexOf("\\\\") >= 0 || !refRe.test(ref)) {{
    throw new TypeError("github.commit.read args ref must be a bounded GitHub ref");
  }}
  return {{owner, repo, ref}};
}}
function validateGithubWorkflowRunReadArgs(arg) {{
  const args = cloneJsonish(arg, "github.workflow.run.read args");
  if (typeof args.owner !== "string" || typeof args.repo !== "string") {{
    throw new TypeError("github.workflow.run.read args owner and repo must be strings");
  }}
  const owner = args.owner;
  const repo = args.repo;
  const nameRe = /^[A-Za-z0-9_.-]+$/;
  if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) {{
    throw new TypeError("github.workflow.run.read args owner and repo must be bounded GitHub path components");
  }}
  let runId;
  if (typeof args.run_id === "number") {{
    if (!Number.isInteger(args.run_id) || args.run_id <= 0 || args.run_id > 9007199254740991) {{
      throw new TypeError("github.workflow.run.read args run_id must be a positive integer");
    }}
    runId = String(args.run_id);
  }} else if (typeof args.run_id === "string") {{
    if (!/^[0-9]+$/.test(args.run_id) || args.run_id.length === 0 || args.run_id.length > 32 || args.run_id === "0") {{
      throw new TypeError("github.workflow.run.read args run_id must be a bounded decimal string");
    }}
    runId = args.run_id;
  }} else {{
    throw new TypeError("github.workflow.run.read args run_id must be a positive integer or decimal string");
  }}
  return {{owner, repo, run_id:runId}};
}}
function validateGithubWorkflowJobsListArgs(arg) {{
  const args = cloneJsonish(arg, "github.workflow.jobs.list args");
  if (typeof args.owner !== "string" || typeof args.repo !== "string") {{
    throw new TypeError("github.workflow.jobs.list args owner and repo must be strings");
  }}
  const owner = args.owner;
  const repo = args.repo;
  const nameRe = /^[A-Za-z0-9_.-]+$/;
  if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) {{
    throw new TypeError("github.workflow.jobs.list args owner and repo must be bounded GitHub path components");
  }}
  let runId;
  if (typeof args.run_id === "number") {{
    if (!Number.isInteger(args.run_id) || args.run_id <= 0 || args.run_id > 9007199254740991) {{
      throw new TypeError("github.workflow.jobs.list args run_id must be a positive integer");
    }}
    runId = String(args.run_id);
  }} else if (typeof args.run_id === "string") {{
    if (!/^[0-9]+$/.test(args.run_id) || args.run_id.length === 0 || args.run_id.length > 32 || args.run_id === "0") {{
      throw new TypeError("github.workflow.jobs.list args run_id must be a bounded decimal string");
    }}
    runId = args.run_id;
  }} else {{
    throw new TypeError("github.workflow.jobs.list args run_id must be a positive integer or decimal string");
  }}
  return {{owner, repo, run_id:runId}};
}}
function validateGithubCheckRunsListArgs(arg) {{
  const args = cloneJsonish(arg, "github.check.runs.list args");
  if (typeof args.owner !== "string" || typeof args.repo !== "string" || typeof args.ref !== "string") {{
    throw new TypeError("github.check.runs.list args owner, repo, and ref must be strings");
  }}
  const owner = args.owner;
  const repo = args.repo;
  const ref = args.ref;
  const nameRe = /^[A-Za-z0-9_.-]+$/;
  const refRe = /^[A-Za-z0-9_.\/-]+$/;
  if (owner.length === 0 || owner.length > 100 || repo.length === 0 || repo.length > 100 || !nameRe.test(owner) || !nameRe.test(repo)) {{
    throw new TypeError("github.check.runs.list args owner and repo must be bounded GitHub path components");
  }}
  if (ref.length === 0 || ref.length > 160 || ref.indexOf("..") >= 0 || ref[0] === "/" || ref[ref.length - 1] === "/" || !refRe.test(ref)) {{
    throw new TypeError("github.check.runs.list args ref must be a bounded Git ref or sha path component");
  }}
  return {{owner, repo, ref}};
}}
function githubIssuePath(args) {{
  return "/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/issues/" + String(args.number);
}}
function githubPrPath(args) {{
  return "/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/pulls/" + String(args.number);
}}
function githubPrFilesPath(args) {{
  return "/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/pulls/" + String(args.number) + "/files";
}}
function githubRepoPath(args) {{
  return "/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo);
}}
function githubReleaseLatestPath(args) {{
  return "/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/releases/latest";
}}
function githubFilePath(args) {{
  const encodedPath = args.path.split("/").map(encodeURIComponent).join("/");
  let path = "/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/contents/" + encodedPath;
  if (typeof args.ref === "string") path += "?ref=" + encodeURIComponent(args.ref);
  return path;
}}
function githubComparePath(args) {{
  return "/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/compare/" + encodeURIComponent(args.base) + "..." + encodeURIComponent(args.head);
}}
function githubCommitPath(args) {{
  return "/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/commits/" + encodeURIComponent(args.ref);
}}
function githubWorkflowRunPath(args) {{
  return "/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/actions/runs/" + encodeURIComponent(args.run_id);
}}
function githubWorkflowJobsPath(args) {{
  return "/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/actions/runs/" + encodeURIComponent(args.run_id) + "/jobs";
}}
function githubCheckRunsPath(args) {{
  return "/repos/" + encodeURIComponent(args.owner) + "/" + encodeURIComponent(args.repo) + "/commits/" + encodeURIComponent(args.ref) + "/check-runs";
}}
function githubIssueLiveQuery(args) {{
  const requestPath = githubIssuePath(args);
  const endpoint = "https://api.github.com" + requestPath;
  const started = Date.now();
  const headers = {{
    "accept": "application/vnd.github+json",
    "user-agent": "cruft-agent-github-issue-read",
    "x-github-api-version": "2022-11-28"
  }};
  if (githubIssueCredentialToken !== null) {{
    headers.authorization = "Bearer " + githubIssueCredentialToken;
  }}
  return new Promise(function(resolve, reject) {{
    let settled = false;
    const req = https.request({{
      protocol: "https:",
      hostname: "api.github.com",
      port: 443,
      method: "GET",
      path: requestPath,
      timeout: toolTimeoutMs,
      headers
    }}, function(res) {{
      const chunks = [];
      let bytes = 0;
      res.on("data", function(chunk) {{
        bytes += chunk.length;
        if (bytes > maxToolResultBytes) {{
          settled = true;
          reject(new Error("agent github issue response byte budget exceeded"));
          req.destroy();
          return;
        }}
        chunks.push(chunk);
      }});
      res.on("end", function() {{
        if (settled) return;
        settled = true;
        const text = Buffer.concat(chunks).toString("utf8");
        if (res.statusCode < 200 || res.statusCode >= 300) {{
          reject(new Error("agent github issue http status " + res.statusCode));
          return;
        }}
        try {{
          const result = JSON.parse(text);
          resolve({{result, endpoint, response_bytes:bytes, status_code:res.statusCode, duration_ms:Date.now() - started}});
        }} catch (e) {{
          reject(new Error("agent github issue invalid json"));
        }}
      }});
    }});
    req.on("timeout", function() {{
      if (settled) return;
      settled = true;
      req.destroy(new Error("agent github issue timeout"));
    }});
    req.on("error", function(e) {{
      if (settled) return;
      settled = true;
      reject(e);
    }});
    req.end();
  }});
}}
function githubPrLiveQuery(args) {{
  const requestPath = githubPrPath(args);
  const endpoint = "https://api.github.com" + requestPath;
  const started = Date.now();
  const headers = {{
    "accept": "application/vnd.github+json",
    "user-agent": "cruft-agent-github-pr-read",
    "x-github-api-version": "2022-11-28"
  }};
  if (githubIssueCredentialToken !== null) {{
    headers.authorization = "Bearer " + githubIssueCredentialToken;
  }}
  return new Promise(function(resolve, reject) {{
    let settled = false;
    const req = https.request({{
      protocol: "https:",
      hostname: "api.github.com",
      port: 443,
      method: "GET",
      path: requestPath,
      timeout: toolTimeoutMs,
      headers
    }}, function(res) {{
      const chunks = [];
      let bytes = 0;
      res.on("data", function(chunk) {{
        bytes += chunk.length;
        if (bytes > maxToolResultBytes) {{
          settled = true;
          reject(new Error("agent github pr response byte budget exceeded"));
          req.destroy();
          return;
        }}
        chunks.push(chunk);
      }});
      res.on("end", function() {{
        if (settled) return;
        settled = true;
        const text = Buffer.concat(chunks).toString("utf8");
        if (res.statusCode < 200 || res.statusCode >= 300) {{
          reject(new Error("agent github pr http status " + res.statusCode));
          return;
        }}
        try {{
          const result = JSON.parse(text);
          resolve({{result, endpoint, response_bytes:bytes, status_code:res.statusCode, duration_ms:Date.now() - started}});
        }} catch (e) {{
          reject(new Error("agent github pr invalid json"));
        }}
      }});
    }});
    req.on("timeout", function() {{
      if (settled) return;
      settled = true;
      req.destroy(new Error("agent github pr timeout"));
    }});
    req.on("error", function(e) {{
      if (settled) return;
      settled = true;
      reject(e);
    }});
    req.end();
  }});
}}
function githubPrFilesLiveQuery(args) {{
  const requestPath = githubPrFilesPath(args);
  const endpoint = "https://api.github.com" + requestPath;
  const started = Date.now();
  const headers = {{
    "accept": "application/vnd.github+json",
    "user-agent": "cruft-agent-github-pr-files-list",
    "x-github-api-version": "2022-11-28"
  }};
  if (githubIssueCredentialToken !== null) {{
    headers.authorization = "Bearer " + githubIssueCredentialToken;
  }}
  return new Promise(function(resolve, reject) {{
    let settled = false;
    const req = https.request({{
      protocol: "https:",
      hostname: "api.github.com",
      port: 443,
      method: "GET",
      path: requestPath,
      timeout: toolTimeoutMs,
      headers
    }}, function(res) {{
      const chunks = [];
      let bytes = 0;
      res.on("data", function(chunk) {{
        bytes += chunk.length;
        if (bytes > maxToolResultBytes) {{
          settled = true;
          reject(new Error("agent github pr files response byte budget exceeded"));
          req.destroy();
          return;
        }}
        chunks.push(chunk);
      }});
      res.on("end", function() {{
        if (settled) return;
        settled = true;
        const text = Buffer.concat(chunks).toString("utf8");
        if (res.statusCode < 200 || res.statusCode >= 300) {{
          reject(new Error("agent github pr files http status " + res.statusCode));
          return;
        }}
        try {{
          const result = JSON.parse(text);
          resolve({{result, endpoint, response_bytes:bytes, status_code:res.statusCode, duration_ms:Date.now() - started}});
        }} catch (e) {{
          reject(new Error("agent github pr files invalid json"));
        }}
      }});
    }});
    req.on("timeout", function() {{
      if (settled) return;
      settled = true;
      req.destroy(new Error("agent github pr files timeout"));
    }});
    req.on("error", function(e) {{
      if (settled) return;
      settled = true;
      reject(e);
    }});
    req.end();
  }});
}}
function githubReleaseLatestLiveQuery(args) {{
  const requestPath = githubReleaseLatestPath(args);
  const endpoint = "https://api.github.com" + requestPath;
  const started = Date.now();
  const headers = {{
    "accept": "application/vnd.github+json",
    "user-agent": "cruft-agent-github-release-latest-read",
    "x-github-api-version": "2022-11-28"
  }};
  if (githubIssueCredentialToken !== null) {{
    headers.authorization = "Bearer " + githubIssueCredentialToken;
  }}
  return new Promise(function(resolve, reject) {{
    let settled = false;
    const req = https.request({{
      protocol: "https:",
      hostname: "api.github.com",
      port: 443,
      method: "GET",
      path: requestPath,
      timeout: toolTimeoutMs,
      headers
    }}, function(res) {{
      const chunks = [];
      let bytes = 0;
      res.on("data", function(chunk) {{
        bytes += chunk.length;
        if (bytes > maxToolResultBytes) {{
          settled = true;
          reject(new Error("agent github latest release response byte budget exceeded"));
          req.destroy();
          return;
        }}
        chunks.push(chunk);
      }});
      res.on("end", function() {{
        if (settled) return;
        settled = true;
        const text = Buffer.concat(chunks).toString("utf8");
        if (res.statusCode < 200 || res.statusCode >= 300) {{
          reject(new Error("agent github latest release http status " + res.statusCode));
          return;
        }}
        try {{
          const result = JSON.parse(text);
          resolve({{result, endpoint, response_bytes:bytes, status_code:res.statusCode, duration_ms:Date.now() - started}});
        }} catch (e) {{
          reject(new Error("agent github latest release invalid json"));
        }}
      }});
    }});
    req.on("timeout", function() {{
      if (settled) return;
      settled = true;
      req.destroy(new Error("agent github latest release timeout"));
    }});
    req.on("error", function(e) {{
      if (settled) return;
      settled = true;
      reject(e);
    }});
    req.end();
  }});
}}
function githubFileLiveQuery(args) {{
  const requestPath = githubFilePath(args);
  const endpoint = "https://api.github.com" + requestPath;
  const started = Date.now();
  const headers = {{
    "accept": "application/vnd.github+json",
    "user-agent": "cruft-agent-github-file-read",
    "x-github-api-version": "2022-11-28"
  }};
  if (githubIssueCredentialToken !== null) {{
    headers.authorization = "Bearer " + githubIssueCredentialToken;
  }}
  return new Promise(function(resolve, reject) {{
    let settled = false;
    const req = https.request({{
      protocol: "https:",
      hostname: "api.github.com",
      port: 443,
      method: "GET",
      path: requestPath,
      timeout: toolTimeoutMs,
      headers
    }}, function(res) {{
      const chunks = [];
      let bytes = 0;
      res.on("data", function(chunk) {{
        bytes += chunk.length;
        if (bytes > maxToolResultBytes) {{
          settled = true;
          reject(new Error("agent github file response byte budget exceeded"));
          req.destroy();
          return;
        }}
        chunks.push(chunk);
      }});
      res.on("end", function() {{
        if (settled) return;
        settled = true;
        const text = Buffer.concat(chunks).toString("utf8");
        if (res.statusCode < 200 || res.statusCode >= 300) {{
          reject(new Error("agent github file http status " + res.statusCode));
          return;
        }}
        try {{
          const result = JSON.parse(text);
          resolve({{result, endpoint, response_bytes:bytes, status_code:res.statusCode, duration_ms:Date.now() - started}});
        }} catch (e) {{
          reject(new Error("agent github file invalid json"));
        }}
      }});
    }});
    req.on("timeout", function() {{
      if (settled) return;
      settled = true;
      req.destroy(new Error("agent github file timeout"));
    }});
    req.on("error", function(e) {{
      if (settled) return;
      settled = true;
      reject(e);
    }});
    req.end();
  }});
}}
function githubCompareLiveQuery(args) {{
  const requestPath = githubComparePath(args);
  const endpoint = "https://api.github.com" + requestPath;
  const started = Date.now();
  const headers = {{
    "accept": "application/vnd.github+json",
    "user-agent": "cruft-agent-github-compare-read",
    "x-github-api-version": "2022-11-28"
  }};
  if (githubIssueCredentialToken !== null) {{
    headers.authorization = "Bearer " + githubIssueCredentialToken;
  }}
  return new Promise(function(resolve, reject) {{
    let settled = false;
    const req = https.request({{
      protocol: "https:",
      hostname: "api.github.com",
      port: 443,
      method: "GET",
      path: requestPath,
      timeout: toolTimeoutMs,
      headers
    }}, function(res) {{
      const chunks = [];
      let bytes = 0;
      res.on("data", function(chunk) {{
        bytes += chunk.length;
        if (bytes > maxToolResultBytes) {{
          settled = true;
          reject(new Error("agent github compare response byte budget exceeded"));
          req.destroy();
          return;
        }}
        chunks.push(chunk);
      }});
      res.on("end", function() {{
        if (settled) return;
        settled = true;
        const text = Buffer.concat(chunks).toString("utf8");
        if (res.statusCode < 200 || res.statusCode >= 300) {{
          reject(new Error("agent github compare http status " + res.statusCode));
          return;
        }}
        try {{
          const result = JSON.parse(text);
          resolve({{result, endpoint, response_bytes:bytes, status_code:res.statusCode, duration_ms:Date.now() - started}});
        }} catch (e) {{
          reject(new Error("agent github compare invalid json"));
        }}
      }});
    }});
    req.on("timeout", function() {{
      if (settled) return;
      settled = true;
      req.destroy(new Error("agent github compare timeout"));
    }});
    req.on("error", function(e) {{
      if (settled) return;
      settled = true;
      reject(e);
    }});
    req.end();
  }});
}}
function githubCommitLiveQuery(args) {{
  const requestPath = githubCommitPath(args);
  const endpoint = "https://api.github.com" + requestPath;
  const started = Date.now();
  const headers = {{
    "accept": "application/vnd.github+json",
    "user-agent": "cruft-agent-github-commit-read",
    "x-github-api-version": "2022-11-28"
  }};
  if (githubIssueCredentialToken !== null) {{
    headers.authorization = "Bearer " + githubIssueCredentialToken;
  }}
  return new Promise(function(resolve, reject) {{
    let settled = false;
    const req = https.request({{
      protocol: "https:",
      hostname: "api.github.com",
      port: 443,
      method: "GET",
      path: requestPath,
      timeout: toolTimeoutMs,
      headers
    }}, function(res) {{
      const chunks = [];
      let bytes = 0;
      res.on("data", function(chunk) {{
        bytes += chunk.length;
        if (bytes > maxToolResultBytes) {{
          settled = true;
          reject(new Error("agent github commit response byte budget exceeded"));
          req.destroy();
          return;
        }}
        chunks.push(chunk);
      }});
      res.on("end", function() {{
        if (settled) return;
        settled = true;
        const text = Buffer.concat(chunks).toString("utf8");
        if (res.statusCode < 200 || res.statusCode >= 300) {{
          reject(new Error("agent github commit http status " + res.statusCode));
          return;
        }}
        try {{
          const result = JSON.parse(text);
          resolve({{result, endpoint, response_bytes:bytes, status_code:res.statusCode, duration_ms:Date.now() - started}});
        }} catch (e) {{
          reject(new Error("agent github commit invalid json"));
        }}
      }});
    }});
    req.on("timeout", function() {{
      if (settled) return;
      settled = true;
      req.destroy(new Error("agent github commit timeout"));
    }});
    req.on("error", function(e) {{
      if (settled) return;
      settled = true;
      reject(e);
    }});
    req.end();
  }});
}}
function githubWorkflowRunLiveQuery(args) {{
  const requestPath = githubWorkflowRunPath(args);
  const endpoint = "https://api.github.com" + requestPath;
  const started = Date.now();
  const headers = {{
    "accept": "application/vnd.github+json",
    "user-agent": "cruft-agent-github-workflow-run-read",
    "x-github-api-version": "2022-11-28"
  }};
  if (githubIssueCredentialToken !== null) {{
    headers.authorization = "Bearer " + githubIssueCredentialToken;
  }}
  return new Promise(function(resolve, reject) {{
    let settled = false;
    const req = https.request({{
      protocol: "https:",
      hostname: "api.github.com",
      port: 443,
      method: "GET",
      path: requestPath,
      timeout: toolTimeoutMs,
      headers
    }}, function(res) {{
      const chunks = [];
      let bytes = 0;
      res.on("data", function(chunk) {{
        bytes += chunk.length;
        if (bytes > maxToolResultBytes) {{
          settled = true;
          reject(new Error("agent github workflow run response byte budget exceeded"));
          req.destroy();
          return;
        }}
        chunks.push(chunk);
      }});
      res.on("end", function() {{
        if (settled) return;
        settled = true;
        const text = Buffer.concat(chunks).toString("utf8");
        if (res.statusCode < 200 || res.statusCode >= 300) {{
          reject(new Error("agent github workflow run http status " + res.statusCode));
          return;
        }}
        try {{
          const result = JSON.parse(text);
          resolve({{result, endpoint, response_bytes:bytes, status_code:res.statusCode, duration_ms:Date.now() - started}});
        }} catch (e) {{
          reject(new Error("agent github workflow run invalid json"));
        }}
      }});
    }});
    req.on("timeout", function() {{
      if (settled) return;
      settled = true;
      req.destroy(new Error("agent github workflow run timeout"));
    }});
    req.on("error", function(e) {{
      if (settled) return;
      settled = true;
      reject(e);
    }});
    req.end();
  }});
}}
function githubWorkflowJobsLiveQuery(args) {{
  const requestPath = githubWorkflowJobsPath(args);
  const endpoint = "https://api.github.com" + requestPath;
  const started = Date.now();
  const headers = {{
    "accept": "application/vnd.github+json",
    "user-agent": "cruft-agent-github-workflow-jobs-list",
    "x-github-api-version": "2022-11-28"
  }};
  if (githubIssueCredentialToken !== null) {{
    headers.authorization = "Bearer " + githubIssueCredentialToken;
  }}
  return new Promise(function(resolve, reject) {{
    let settled = false;
    const req = https.request({{
      protocol: "https:",
      hostname: "api.github.com",
      port: 443,
      method: "GET",
      path: requestPath,
      timeout: toolTimeoutMs,
      headers
    }}, function(res) {{
      const chunks = [];
      let bytes = 0;
      res.on("data", function(chunk) {{
        bytes += chunk.length;
        if (bytes > maxToolResultBytes) {{
          settled = true;
          reject(new Error("agent github workflow jobs response byte budget exceeded"));
          req.destroy();
          return;
        }}
        chunks.push(chunk);
      }});
      res.on("end", function() {{
        if (settled) return;
        settled = true;
        const text = Buffer.concat(chunks).toString("utf8");
        if (res.statusCode < 200 || res.statusCode >= 300) {{
          reject(new Error("agent github workflow jobs http status " + res.statusCode));
          return;
        }}
        try {{
          const result = JSON.parse(text);
          resolve({{result, endpoint, response_bytes:bytes, status_code:res.statusCode, duration_ms:Date.now() - started}});
        }} catch (e) {{
          reject(new Error("agent github workflow jobs invalid json"));
        }}
      }});
    }});
    req.on("timeout", function() {{
      if (settled) return;
      settled = true;
      req.destroy(new Error("agent github workflow jobs timeout"));
    }});
    req.on("error", function(e) {{
      if (settled) return;
      settled = true;
      reject(e);
    }});
    req.end();
  }});
}}
function githubCheckRunsLiveQuery(args) {{
  const requestPath = githubCheckRunsPath(args);
  const endpoint = "https://api.github.com" + requestPath;
  const started = Date.now();
  const headers = {{
    "accept": "application/vnd.github+json",
    "user-agent": "cruft-agent-github-check-runs-list",
    "x-github-api-version": "2022-11-28"
  }};
  if (githubIssueCredentialToken !== null) {{
    headers.authorization = "Bearer " + githubIssueCredentialToken;
  }}
  return new Promise(function(resolve, reject) {{
    let settled = false;
    const req = https.request({{
      protocol: "https:",
      hostname: "api.github.com",
      port: 443,
      method: "GET",
      path: requestPath,
      timeout: toolTimeoutMs,
      headers
    }}, function(res) {{
      const chunks = [];
      let bytes = 0;
      res.on("data", function(chunk) {{
        bytes += chunk.length;
        if (bytes > maxToolResultBytes) {{
          settled = true;
          reject(new Error("agent github check runs response byte budget exceeded"));
          req.destroy();
          return;
        }}
        chunks.push(chunk);
      }});
      res.on("end", function() {{
        if (settled) return;
        settled = true;
        const text = Buffer.concat(chunks).toString("utf8");
        if (res.statusCode < 200 || res.statusCode >= 300) {{
          reject(new Error("agent github check runs http status " + res.statusCode));
          return;
        }}
        try {{
          const result = JSON.parse(text);
          resolve({{result, endpoint, response_bytes:bytes, status_code:res.statusCode, duration_ms:Date.now() - started}});
        }} catch (e) {{
          reject(new Error("agent github check runs invalid json"));
        }}
      }});
    }});
    req.on("timeout", function() {{
      if (settled) return;
      settled = true;
      req.destroy(new Error("agent github check runs timeout"));
    }});
    req.on("error", function(e) {{
      if (settled) return;
      settled = true;
      reject(e);
    }});
    req.end();
  }});
}}
function githubRepoLiveQuery(args) {{
  const requestPath = githubRepoPath(args);
  const endpoint = "https://api.github.com" + requestPath;
  const started = Date.now();
  const transientSpec = process.env.CRUFT_AGENT_TEST_NAMED_NETWORK_TRANSIENT || "";
  if (transientSpec.length > 0) {{
    const parts = transientSpec.split(":");
    const tool = parts[0] || "";
    const failCount = Number(parts[1] || "0");
    if (tool === "github.repo.read" && Number.isFinite(failCount) && failCount > 0) {{
      const key = "__CRUFT_AGENT_TEST_NAMED_NETWORK_TRANSIENT_GITHUB_REPO_READ";
      const seen = Number(process.env[key] || "0");
      process.env[key] = String(seen + 1);
      if (seen < failCount) {{
        return Promise.reject(new Error("agent github repo test transient transport_error"));
      }}
      return Promise.resolve({{
        result:{{full_name:args.owner + "/" + args.repo, private:false, test_transient:true, attempts_before_success:seen}},
        endpoint,
        response_bytes:96,
        status_code:200,
        duration_ms:Date.now() - started,
        transport:"test_transient"
      }});
    }}
  }}
  const headers = {{
    "accept": "application/vnd.github+json",
    "user-agent": "cruft-agent-github-repo-read",
    "x-github-api-version": "2022-11-28"
  }};
  if (githubIssueCredentialToken !== null) {{
    headers.authorization = "Bearer " + githubIssueCredentialToken;
  }}
  return new Promise(function(resolve, reject) {{
    let settled = false;
    const req = https.request({{
      protocol: "https:",
      hostname: "api.github.com",
      port: 443,
      method: "GET",
      path: requestPath,
      timeout: toolTimeoutMs,
      headers
    }}, function(res) {{
      const chunks = [];
      let bytes = 0;
      res.on("data", function(chunk) {{
        bytes += chunk.length;
        if (bytes > maxToolResultBytes) {{
          settled = true;
          reject(new Error("agent github repo response byte budget exceeded"));
          req.destroy();
          return;
        }}
        chunks.push(chunk);
      }});
      res.on("end", function() {{
        if (settled) return;
        settled = true;
        const text = Buffer.concat(chunks).toString("utf8");
        if (res.statusCode < 200 || res.statusCode >= 300) {{
          reject(new Error("agent github repo http status " + res.statusCode));
          return;
        }}
        try {{
          const result = JSON.parse(text);
          resolve({{result, endpoint, response_bytes:bytes, status_code:res.statusCode, duration_ms:Date.now() - started}});
        }} catch (e) {{
          reject(new Error("agent github repo invalid json"));
        }}
      }});
    }});
    req.on("timeout", function() {{
      if (settled) return;
      settled = true;
      req.destroy(new Error("agent github repo timeout"));
    }});
    req.on("error", function(e) {{
      if (settled) return;
      settled = true;
      reject(e);
    }});
    req.end();
  }});
}}
function namedNetworkCacheHash(tool, args) {{
  const payload = tool + "\n" + JSON.stringify(args);
  let h = 0x811c9dc5;
  for (let i = 0; i < payload.length; i++) {{
    h ^= payload.charCodeAt(i) & 0xff;
    h = Math.imul(h, 0x01000193) >>> 0;
  }}
  return h.toString(16).padStart(8, "0");
}}
function namedNetworkPersistentCachePath(tool, args) {{
  if (namedNetworkCacheDir === null) return null;
  const safeTool = String(tool).replace(/[^A-Za-z0-9_.-]/g, "_");
  return namedNetworkCacheDir + "/" + safeTool + "-" + namedNetworkCacheHash(tool, args) + ".json";
}}
function namedNetworkPersistentCachePrune(tool) {{
  if (namedNetworkCacheDir === null || namedNetworkCacheMaxEntries === null) return;
  try {{
    const safeTool = String(tool).replace(/[^A-Za-z0-9_.-]/g, "_");
    const prefix = safeTool + "-";
    const names = fs.readdirSync(namedNetworkCacheDir);
    const entries = [];
    for (let i = 0; i < names.length; i++) {{
      const name = String(names[i]);
      if (name.indexOf(prefix) !== 0 || name.lastIndexOf(".json") !== name.length - 5) continue;
      const path = namedNetworkCacheDir + "/" + name;
      let storedMs = 0;
      try {{
        const envelope = JSON.parse(fs.readFileSync(path, "utf8"));
        storedMs = Number(envelope && envelope.stored_ms || 0);
      }} catch (_e) {{
        try {{
          storedMs = Number(fs.statSync(path).mtimeMs || 0);
        }} catch (_stat) {{
          storedMs = 0;
        }}
      }}
      entries.push({{name, path, stored_ms:storedMs}});
    }}
    if (entries.length <= namedNetworkCacheMaxEntries) return;
    entries.sort(function(a, b) {{
      if (a.stored_ms !== b.stored_ms) return a.stored_ms - b.stored_ms;
      return a.name < b.name ? -1 : (a.name > b.name ? 1 : 0);
    }});
    const removeCount = entries.length - namedNetworkCacheMaxEntries;
    for (let i = 0; i < removeCount; i++) {{
      const entry = entries[i];
      try {{
        fs.unlinkSync(entry.path);
        audit({{type:"named_network_cache_eviction", tool, cache_scope:"persistent", cache_path:entry.path, stored_ms:entry.stored_ms, max_entries:namedNetworkCacheMaxEntries, disposition:"retention_prune"}});
      }} catch (e) {{
        audit({{type:"named_network_cache_error", tool, policy:"ignored", operation:"retention_prune", cache_path:entry.path, message:safeMessage(e)}});
      }}
    }}
  }} catch (e) {{
    audit({{type:"named_network_cache_error", tool, policy:"ignored", operation:"retention_scan", message:safeMessage(e)}});
  }}
}}
function namedNetworkPersistentCacheRead(tool, args) {{
  const path = namedNetworkPersistentCachePath(tool, args);
  if (path === null) return null;
  try {{
    namedNetworkPersistentCachePrune(tool);
    if (!fs.existsSync(path)) return null;
    const envelope = JSON.parse(fs.readFileSync(path, "utf8"));
    if (envelope === null || envelope.tool !== tool || JSON.stringify(envelope.args) !== JSON.stringify(args) || envelope.result === undefined) return null;
    const storedMs = Number(envelope.stored_ms || 0);
    const ageMs = storedMs > 0 ? Math.max(0, Date.now() - storedMs) : null;
    if (namedNetworkCacheMaxAgeMs !== null && (ageMs === null || ageMs > namedNetworkCacheMaxAgeMs)) {{
      audit({{type:"named_network_cache_stale", tool, cache_scope:"persistent", cache_path:path, stored_ms:storedMs, age_ms:ageMs, max_age_ms:namedNetworkCacheMaxAgeMs, disposition:namedNetworkCacheMode === "offline" ? "failed_closed" : "evicted"}});
      if (namedNetworkCacheMode !== "offline") {{
        try {{
          fs.unlinkSync(path);
          audit({{type:"named_network_cache_eviction", tool, cache_scope:"persistent", cache_path:path, stored_ms:storedMs, age_ms:ageMs, max_age_ms:namedNetworkCacheMaxAgeMs, disposition:"deleted_stale_envelope"}});
        }} catch (e) {{
          audit({{type:"named_network_cache_error", tool, policy:"ignored", operation:"evict", cache_path:path, message:safeMessage(e)}});
        }}
      }}
      return null;
    }}
    return {{path, result:cloneStateValue(envelope.result), stored_ms:storedMs, age_ms:ageMs}};
  }} catch (e) {{
    audit({{type:"named_network_cache_error", tool, policy:"ignored", operation:"read", message:safeMessage(e)}});
    return null;
  }}
}}
function namedNetworkOfflineCacheMiss(tool, endpoint, args, argBytes) {{
  audit({{type:"tool_call", tool, policy:"allowed", endpoint, transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"persistent", cache_mode:"offline"}});
  audit({{type:"tool_error", tool, endpoint, transport:"persistent_cache", message:"agent named network cache miss in offline mode", error_kind:"cache_miss", retryable:false, retry_policy:"none", retry_attempts:0, cache_scope:"persistent", cache_mode:"offline"}});
  throw new Error("agent named network cache miss in offline mode");
}}
function namedNetworkPersistentCacheWrite(tool, args, live) {{
  const path = namedNetworkPersistentCachePath(tool, args);
  if (path === null) return;
  try {{
    fs.mkdirSync(namedNetworkCacheDir, {{recursive:true}});
    const envelope = {{
      type:"cruft_agent_named_network_cache_v1",
      tool,
      endpoint:live.endpoint,
      args:cloneStateValue(args),
      result:cloneStateValue(live.result),
      response_bytes:live.response_bytes,
      status_code:live.status_code,
      stored_ms:Date.now()
    }};
    const tmp = path + ".tmp-" + process.pid + "-" + Date.now();
    fs.writeFileSync(tmp, JSON.stringify(envelope));
    fs.renameSync(tmp, path);
    audit({{type:"named_network_cache_write", tool, endpoint:live.endpoint, cache_scope:"persistent", cache_path:path, result_bytes:payloadBytes(envelope.result), response_bytes:live.response_bytes, status_code:live.status_code}});
    namedNetworkPersistentCachePrune(tool);
  }} catch (e) {{
    audit({{type:"named_network_cache_error", tool, policy:"ignored", operation:"write", message:safeMessage(e)}});
  }}
}}
function writeArtifactHost(relative, content) {{
  if (fsWriteRoots.length === 0) {{
    audit({{type:"tool_denial", tool:"writeArtifact", policy:"denied", reason:"artifact_root_not_configured", path:relative}});
    throw new Error("agent tool denied: writeArtifact root not configured");
  }}
  const root = String(fsWriteRoots[0].root);
  const target = root + "/" + relative;
  if (fs.existsSync(target)) {{
    audit({{type:"tool_denial", tool:"writeArtifact", policy:"denied", reason:"artifact_overwrite_denied", path:relative}});
    throw new Error("agent tool denied: writeArtifact overwrite denied");
  }}
  const dir = target.split("/").slice(0, -1).join("/");
  if (dir.length > 0) fs.mkdirSync(dir, {{recursive:true}});
  const tmp = target + ".cruft-tmp-" + Date.now() + "-" + Math.floor(Math.random() * 1000000);
  fs.writeFileSync(tmp, content);
  fs.renameSync(tmp, target);
}}
function findProcessCommand(name) {{
  name = String(name);
  for (let i = 0; i < processCommands.length; i++) {{
    if (processCommands[i].name === name) return processCommands[i];
  }}
  return null;
}}
function processEnvObject(overrides) {{
  const out = {{}};
  const allowed = {{}};
  for (let i = 0; i < processEnv.length; i++) {{
    out[processEnv[i].key] = processEnv[i].value;
    allowed[processEnv[i].key] = true;
  }}
  if (overrides !== undefined) {{
    if (overrides === null || typeof overrides !== "object" || Array.isArray(overrides)) {{
      throw new TypeError("process args env must be an object when present");
    }}
    const names = Object.getOwnPropertyNames(overrides);
    for (let i = 0; i < names.length; i++) {{
      const key = names[i];
      if (!allowed[key]) throw new Error("agent tool denied: process env key not admitted");
      if (typeof overrides[key] !== "string") throw new TypeError("process args env values must be strings");
      out[key] = overrides[key];
    }}
  }}
  return out;
}}
function processCwdPath(requested) {{
  if (processCwds.length === 0) throw new Error("agent tool denied: process cwd not configured");
  const candidate = requested === undefined || requested === null ? processCwds[0].root : String(requested);
  let real;
  try {{
    real = fs.realpathSync(candidate);
  }} catch (e) {{
    throw new Error("agent tool denied: process cwd not admitted");
  }}
  for (let i = 0; i < processCwds.length; i++) {{
    const root = processCwds[i].root;
    if (real === root || real.indexOf(root + "/") === 0) return real;
  }}
  throw new Error("agent tool denied: process cwd not admitted");
}}
function validateModelCallArgs(arg) {{
  const args = cloneJsonish(arg, "model.call args");
  if (typeof args.id !== "string" || args.id.length === 0) {{
    throw new TypeError("model.call args id must be a non-empty string");
  }}
  if (args.model !== undefined && (typeof args.model !== "string" || args.model.length === 0)) {{
    throw new TypeError("model.call args model must be a non-empty string when present");
  }}
  return args;
}}
function modelFixtureLookup(args) {{
  if (modelFixture === null || !Array.isArray(modelFixture.responses)) return null;
  for (let i = 0; i < modelFixture.responses.length; i++) {{
    const r = modelFixture.responses[i];
    if (r.id !== args.id) continue;
    if (args.model !== undefined && r.model !== undefined && r.model !== args.model) continue;
    return cloneStateValue(r.response);
  }}
  return null;
}}
function modelProviderLiveCall(args) {{
  if (modelProvider !== "openai.responses") throw new Error("agent model provider not admitted");
  if (modelApiKeyToken === null) throw new Error("agent model provider credential not configured");
  if (typeof args.model !== "string" || args.model.length === 0) {{
    throw new TypeError("model.call provider args model must be a non-empty string");
  }}
  const endpoint = "https://api.openai.com/v1/responses";
  const started = Date.now();
  const testResponse = process.env.CRUFT_AGENT_TEST_MODEL_PROVIDER_RESPONSE || "";
  if (testResponse.length > 0) {{
    try {{
      const result = JSON.parse(testResponse);
      return Promise.resolve({{result, endpoint, response_bytes:Buffer.byteLength(testResponse), status_code:200, duration_ms:Date.now() - started, transport:"test_provider"}});
    }} catch (_e) {{
      return Promise.reject(new Error("agent model provider test response invalid json"));
    }}
  }}
  const bodyObject = {{model:args.model, input:args.input === undefined ? args.id : args.input}};
  const body = JSON.stringify(bodyObject);
  if (Buffer.byteLength(body) > maxToolArgBytes) throw new Error("agent model provider request byte budget exceeded");
  return new Promise(function(resolve, reject) {{
    let settled = false;
    const req = https.request({{
      protocol:"https:",
      hostname:"api.openai.com",
      port:443,
      method:"POST",
      path:"/v1/responses",
      timeout:toolTimeoutMs,
      headers:{{
        "content-type":"application/json",
        "accept":"application/json",
        "authorization":"Bearer " + modelApiKeyToken,
        "user-agent":"cruft-agent-model-call",
        "content-length":Buffer.byteLength(body)
      }}
    }}, function(res) {{
      const chunks = [];
      let bytes = 0;
      res.on("data", function(chunk) {{
        bytes += chunk.length;
        if (bytes > maxToolResultBytes) {{
          settled = true;
          reject(new Error("agent model provider response byte budget exceeded"));
          req.destroy();
          return;
        }}
        chunks.push(chunk);
      }});
      res.on("end", function() {{
        if (settled) return;
        settled = true;
        const text = Buffer.concat(chunks).toString("utf8");
        if (res.statusCode < 200 || res.statusCode >= 300) {{
          reject(new Error("agent model provider http status " + res.statusCode));
          return;
        }}
        try {{
          const result = JSON.parse(text);
          resolve({{result, endpoint, response_bytes:bytes, status_code:res.statusCode, duration_ms:Date.now() - started, transport:"pinned_https"}});
        }} catch (_e) {{
          reject(new Error("agent model provider invalid json"));
        }}
      }});
    }});
    req.on("timeout", function() {{
      if (settled) return;
      settled = true;
      req.destroy(new Error("agent model provider timeout"));
    }});
    req.on("error", function(e) {{
      if (settled) return;
      settled = true;
      reject(e);
    }});
    req.write(body);
    req.end();
  }});
}}
function modelProviderErrorTaxonomy(e) {{
  const message = safeMessage(e);
  if (message.indexOf("credential not configured") >= 0) return {{error_kind:"credential_not_configured", retryable:false}};
  if (message.indexOf("provider args model") >= 0) return {{error_kind:"invalid_args", retryable:false}};
  if (message.indexOf("request byte budget") >= 0) return {{error_kind:"request_byte_budget", retryable:false}};
  if (message.indexOf("response byte budget") >= 0) return {{error_kind:"response_byte_budget", retryable:false}};
  if (message.indexOf("timeout") >= 0) return {{error_kind:"timeout", retryable:true}};
  if (message.indexOf("http status 429") >= 0 || message.indexOf("http status 5") >= 0) return {{error_kind:"http_retryable", retryable:true}};
  if (message.indexOf("http status") >= 0) return {{error_kind:"http_nonretryable", retryable:false}};
  if (message.indexOf("invalid json") >= 0) return {{error_kind:"invalid_json", retryable:false}};
  return {{error_kind:"transport_error", retryable:true}};
}}
function validateProcessArgs(arg) {{
  const args = cloneJsonish(arg, "process args");
  if (typeof args.command !== "string" || args.command.length === 0) {{
    throw new TypeError("process args command must be a non-empty string");
  }}
  const command = findProcessCommand(args.command);
  if (command === null) throw new Error("agent tool denied: process command not admitted");
  const argv = [];
  if (args.args !== undefined) {{
    if (!Array.isArray(args.args)) throw new TypeError("process args args must be an array of strings");
    for (let i = 0; i < args.args.length; i++) {{
      if (typeof args.args[i] !== "string") throw new TypeError("process args args must be an array of strings");
      argv.push(args.args[i]);
    }}
  }}
  let output = "full";
  if (args.output !== undefined) {{
    if (args.output !== "full" && args.output !== "summary" && args.output !== "stream") throw new TypeError("process args output must be \"full\", \"summary\", or \"stream\"");
    output = args.output;
  }}
  return {{command:args.command, path:command.path, args:argv, cwd:processCwdPath(args.cwd), env:processEnvObject(args.env), output}};
}}
function processOutputSummary(stream, text, forceTruncated) {{
  text = text === null || text === undefined ? "" : String(text);
  let previewLimit = Math.min(maxEventBytes, Math.floor(maxToolResultBytes / 8), 512);
  if (!Number.isFinite(previewLimit) || previewLimit <= 0) previewLimit = 1;
  const preview = text.slice(0, previewLimit);
  return {{
    stream,
    bytes:text.length,
    captured_bytes:preview.length,
    truncated:!!forceTruncated || preview.length < text.length,
    preview
  }};
}}
function auditProcessOutputStream(command, stream, text, forceTruncated) {{
  text = text === null || text === undefined ? "" : String(text);
  if (text.length === 0) return;
  const chunkSize = processOutputStreamChunkBytes;
  let limit = maxEventBytes;
  if (!Number.isFinite(limit) || limit <= 0) limit = chunkSize;
  limit = Math.max(1, Math.min(limit, maxToolResultBytes, 8192));
  const captured = text.slice(0, limit);
  const chunks = Math.max(1, Math.ceil(captured.length / chunkSize));
  const truncated = !!forceTruncated || captured.length < text.length;
  for (let i = 0; i < chunks; i++) {{
    const start = i * chunkSize;
    const chunk = captured.slice(start, start + chunkSize);
    audit({{
      type:"process_output_stream",
      tool:"process",
      policy:"allowed",
      command,
      stream,
      chunk_index:i,
      chunk_count:chunks,
      chunk_bytes:chunk.length,
      captured_bytes:captured.length,
      original_bytes:text.length,
      truncated,
      text:chunk
    }});
  }}
}}
function runProcessTool(args) {{
  if (args.output === "stream") return runProcessToolStreaming(args);
  const started = Date.now();
  const processCaptureLimit = Math.max(maxToolResultBytes, maxEventBytes, 65536);
  const out = childProcess.spawnSync(args.path, args.args, {{
    cwd: args.cwd,
    env: args.env,
    encoding: "utf8",
    timeout: toolTimeoutMs,
    maxBuffer: processCaptureLimit,
    killSignal: "SIGKILL",
    shell: false
  }});
  const duration = Date.now() - started;
  const stdout = out.stdout === null || out.stdout === undefined ? "" : String(out.stdout);
  const stderr = out.stderr === null || out.stderr === undefined ? "" : String(out.stderr);
  if ((out.error && String(out.error.code || "") === "ETIMEDOUT") || (out.signal !== null && out.signal !== undefined && duration >= toolTimeoutMs)) {{
    audit({{type:"tool_timeout", tool:"process", policy:"allowed", command:args.command, timeout_ms:toolTimeoutMs, duration_ms:duration, cancelled:true, cancellation:"killed", kill_signal:"SIGKILL", signal:out.signal === null || out.signal === undefined ? "SIGKILL" : out.signal}});
    throw new Error("agent tool timeout: process after " + toolTimeoutMs + "ms");
  }}
  const outputOverflow = (out.error && String(out.error.code || "") === "ENOBUFS") || stdout.length > maxToolResultBytes || stderr.length > maxToolResultBytes;
  if (outputOverflow && args.output === "summary") {{
    auditProcessOutputStream(args.command, "stdout", stdout, true);
    auditProcessOutputStream(args.command, "stderr", stderr, true);
    audit({{type:"process_output_budget", tool:"process", policy:"allowed", command:args.command, limit:maxToolResultBytes, stdout_bytes:stdout.length, stderr_bytes:stderr.length, truncated:true, disposition:"summarized"}});
    return {{
      command: args.command,
      exit_code: out.status === null || out.status === undefined ? null : out.status,
      signal: out.signal === null || out.signal === undefined ? null : out.signal,
      stdout:"",
      stderr:"",
      stdout_summary:processOutputSummary("stdout", stdout, true),
      stderr_summary:processOutputSummary("stderr", stderr, true),
      output_mode:"summary",
      duration_ms: duration
    }};
  }}
  if (outputOverflow) {{
    auditProcessOutputStream(args.command, "stdout", stdout, true);
    auditProcessOutputStream(args.command, "stderr", stderr, true);
    audit({{type:"process_output_budget", tool:"process", policy:"allowed", command:args.command, limit:maxToolResultBytes, stdout_bytes:stdout.length, stderr_bytes:stderr.length, truncated:true, disposition:"failed_closed"}});
    audit({{type:"budget_exceeded", budget:"process_output_bytes", tool:"process", limit:maxToolResultBytes, stdout_bytes:stdout.length, stderr_bytes:stderr.length}});
    throw new Error("agent process output byte budget exceeded");
  }}
  auditProcessOutputStream(args.command, "stdout", stdout, false);
  auditProcessOutputStream(args.command, "stderr", stderr, false);
  if (args.output === "summary") {{
    return {{
      command: args.command,
      exit_code: out.status === null || out.status === undefined ? null : out.status,
      signal: out.signal === null || out.signal === undefined ? null : out.signal,
      stdout:"",
      stderr:"",
      stdout_summary:processOutputSummary("stdout", stdout, false),
      stderr_summary:processOutputSummary("stderr", stderr, false),
      output_mode:"summary",
      duration_ms: duration
    }};
  }}
  return {{
    command: args.command,
    exit_code: out.status === null || out.status === undefined ? null : out.status,
    signal: out.signal === null || out.signal === undefined ? null : out.signal,
    stdout,
    stderr,
    duration_ms: duration
  }};
}}
function runProcessToolStreaming(args) {{
  const started = Date.now();
  return new Promise(function(resolve, reject) {{
    let settled = false;
    let timedOut = false;
    let stdout = "";
    let stderr = "";
    const child = childProcess.spawn(args.path, args.args, {{
      cwd: args.cwd,
      env: args.env,
      shell: false,
      stdio: ["ignore", "pipe", "pipe"]
    }});
    const timeoutId = timers.setTimeout(function() {{
      if (settled) return;
      timedOut = true;
      try {{ child.kill("SIGKILL"); }} catch (_e) {{}}
    }}, toolTimeoutMs);
    function capture(stream, chunk) {{
      const text = String(chunk);
      if (stream === "stdout") stdout += text;
      else stderr += text;
      auditProcessOutputStream(args.command, stream, text, false);
      if (stdout.length > maxToolResultBytes || stderr.length > maxToolResultBytes) {{
        audit({{type:"process_output_budget", tool:"process", policy:"allowed", command:args.command, limit:maxToolResultBytes, stdout_bytes:stdout.length, stderr_bytes:stderr.length, truncated:true, disposition:"failed_closed"}});
        audit({{type:"budget_exceeded", budget:"process_output_bytes", tool:"process", limit:maxToolResultBytes, stdout_bytes:stdout.length, stderr_bytes:stderr.length}});
        try {{ child.kill("SIGKILL"); }} catch (_e) {{}}
      }}
    }}
    child.stdout.on("data", function(chunk) {{ capture("stdout", chunk); }});
    child.stderr.on("data", function(chunk) {{ capture("stderr", chunk); }});
    child.on("error", function(e) {{
      if (settled) return;
      settled = true;
      timers.clearTimeout(timeoutId);
      reject(e);
    }});
    child.on("close", function(code, signal) {{
      if (settled) return;
      settled = true;
      timers.clearTimeout(timeoutId);
      const duration = Date.now() - started;
      if (timedOut) {{
        audit({{type:"tool_timeout", tool:"process", policy:"allowed", command:args.command, timeout_ms:toolTimeoutMs, duration_ms:duration, cancelled:true, cancellation:"killed", kill_signal:"SIGKILL", signal:signal === null || signal === undefined ? "SIGKILL" : signal}});
        reject(new Error("agent tool timeout: process after " + toolTimeoutMs + "ms"));
        return;
      }}
      if (stdout.length > maxToolResultBytes || stderr.length > maxToolResultBytes) {{
        reject(new Error("agent process output byte budget exceeded"));
        return;
      }}
      resolve({{
        command: args.command,
        exit_code: code === null || code === undefined ? null : code,
        signal: signal === null || signal === undefined ? null : signal,
        stdout,
        stderr,
        output_mode:"stream",
        duration_ms: duration
      }});
    }});
  }});
}}
const tools = Object.freeze({{
  echo(arg) {{ return cloneJsonish(arg, "echo args"); }},
  fail(arg) {{
    cloneJsonish(arg, "fail args");
    throw new Error("agent tool host failure: fail");
  }},
  slow(arg) {{
    const args = cloneJsonish(arg, "slow args");
    const requested = args.delay_ms === undefined ? toolTimeoutMs * 2 : Number(args.delay_ms);
    if (!Number.isFinite(requested) || requested < 0) throw new TypeError("slow args delay_ms must be a non-negative number");
    const delay = Math.floor(requested);
    return new Promise(function(resolve) {{
      timers.setTimeout(function() {{
        resolve({{ok:true, tool:"slow", delay_ms:delay, value:args.value === undefined ? null : args.value}});
      }}, delay);
    }});
  }}
}});
function withToolTimeout(name, args, argBytes, invoke) {{
  const started = Date.now();
  audit({{type:"tool_call", tool:name, policy:"allowed", args:redact(args), arg_bytes:argBytes}});
  let settled = false;
  let timeoutId = null;
  const pending = Promise.resolve().then(invoke);
  return new Promise(function(resolve, reject) {{
    timeoutId = timers.setTimeout(function() {{
      if (settled) return;
      settled = true;
      audit({{type:"tool_timeout", tool:name, policy:"allowed", timeout_ms:toolTimeoutMs, duration_ms:Date.now() - started}});
      reject(new Error("agent tool timeout: " + name + " after " + toolTimeoutMs + "ms"));
    }}, toolTimeoutMs);
    pending.then(function(result) {{
      if (settled) return;
      settled = true;
      if (timeoutId !== null) timers.clearTimeout(timeoutId);
      try {{
        const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, maxToolResultBytes);
        audit({{type:"tool_result", tool:name, result:redact(result), result_bytes:resultBytes, duration_ms:Date.now() - started}});
        resolve(result);
      }} catch (e) {{
        reject(e);
      }}
    }}, function(e) {{
      if (settled) return;
      settled = true;
      if (timeoutId !== null) timers.clearTimeout(timeoutId);
      audit({{type:"tool_error", tool:name, policy:"allowed", message:safeMessage(e), duration_ms:Date.now() - started}});
      reject(e);
    }});
  }});
}}
function schedulerSuspend(kind, payload) {{
  if (schedulerAwaitOut === null) {{
    throw new Error("agent scheduler is not available in this run");
  }}
  const record = {{type:"cruft_agent_scheduler_await", version:1, kind, payload, run_id:runId, created_ms:Date.now()}};
  const encoded = JSON.stringify(record);
  fs.writeFileSync(schedulerAwaitOut, encoded);
  audit({{type:"scheduler_await", kind, payload}});
  throw new Error("agent scheduler suspended");
}}
const schedulerSurface = schedulerAwaitOut === null ? undefined : Object.freeze({{
  sleep(ms) {{
    ensureOpen("scheduler.sleep");
    const delay = Number(ms);
    if (!Number.isFinite(delay) || delay < 0 || delay > 86400000) {{
      throw new TypeError("scheduler.sleep ms must be a finite non-negative delay <= 86400000");
    }}
    const delayMs = Math.floor(delay);
    schedulerSuspend("timer", {{delay_ms:delayMs, deadline_ms:Date.now() + delayMs}});
  }},
  callTool(name, arg, options) {{
    ensureOpen("scheduler.callTool");
    name = String(name);
    const args = arg === undefined ? {{}} : cloneJsonish(arg, "scheduler tool args");
    const opts = options === undefined ? {{}} : cloneJsonish(options, "scheduler tool options");
    schedulerSuspend("tool", {{tool:name, args, options:opts}});
  }},
  waitForInput(kind, payload) {{
    ensureOpen("scheduler.waitForInput");
    kind = String(kind);
    const inputPayload = payload === undefined ? {{}} : cloneJsonish(payload, "scheduler input payload");
    const token = "input-" + Date.now() + "-" + Math.floor(Math.random() * 1000000000);
    schedulerSuspend("input", {{kind, payload:inputPayload, token}});
  }}
}});
const compartment = new Compartment({{
  modules: admittedModules,
  importHook(specifier) {{
    specifier = String(specifier);
    if (!Object.prototype.hasOwnProperty.call(admittedHookSources, specifier)) {{
      audit({{type:"import_hook_denial", specifier, policy:"denied", reason:"specifier_not_admitted"}});
      throw new Error("agent import hook denied: " + specifier);
    }}
    audit({{type:"import_hook_load", specifier, policy:"allowed", async:"promise_record"}});
    return Promise.resolve({{source: admittedHookSources[specifier]}});
  }},
  globals: {{
    context,
    scheduler: schedulerSurface,
    state: Object.freeze({{
      get(key) {{
        ensureOpen("state.get");
        key = String(key);
        if (!Object.prototype.hasOwnProperty.call(stateStore, key)) return undefined;
        audit({{type:"state_get", key}});
        return cloneStateValue(stateStore[key]);
      }},
      set(key, value) {{
        ensureOpen("state.set");
        key = String(key);
        const cloned = cloneStateValue(value);
        const candidate = cloneStateValue(stateStore);
        candidate[key] = cloned;
        const bytes = enforceCandidateStateBudget(candidate);
        stateStore = candidate;
        audit({{type:"state_set", key, state_bytes:bytes}});
      }},
      delete(key) {{
        ensureOpen("state.delete");
        key = String(key);
        const existed = Object.prototype.hasOwnProperty.call(stateStore, key);
        delete stateStore[key];
        audit({{type:"state_delete", key, existed}});
        return existed;
      }},
      list() {{
        ensureOpen("state.list");
        const keys = Object.getOwnPropertyNames(stateStore);
        audit({{type:"state_list", count:keys.length}});
        return keys;
      }},
      reset() {{
        ensureOpen("state.reset");
        const previousBytes = stateBytes();
        stateStore = {{}};
        audit({{type:"state_reset", previous_state_bytes:previousBytes, state_bytes:stateBytes()}});
      }}
    }}),
    emit(event) {{
      ensureOpen("emit");
      emitBudgeted(event);
    }},
    callTool(name, arg) {{
      ensureOpen("callTool");
      name = String(name);
      if (approvalRequiredFor(name)) {{
        let approvalArgs;
        try {{
          approvalArgs = arg === undefined ? {{}} : cloneJsonish(arg, name + " approval args");
        }} catch (e) {{
          audit({{type:"tool_invalid_args", tool:name, policy:"denied", phase:"approval", message:safeMessage(e)}});
          throw e;
        }}
        const approvalArgBytes = enforcePayloadBudget("tool_arg_bytes", name, approvalArgs, maxToolArgBytes);
        requireToolApproval(name, approvalArgs, approvalArgBytes);
      }}
      if (name === "echo" && allowEcho) {{
        let args;
        try {{
          args = cloneJsonish(arg, "echo args");
        }} catch (e) {{
          audit({{type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)}});
          throw e;
        }}
        const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, maxToolArgBytes);
        audit({{type:"tool_call", tool:name, policy:"allowed", args:redact(args), arg_bytes:argBytes}});
        const result = tools.echo(args);
        const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, maxToolResultBytes);
        audit({{type:"tool_result", tool:name, result:redact(result), result_bytes:resultBytes}});
        return result;
      }}
      if (name === "fail" && allowFail) {{
        let args;
        try {{
          args = cloneJsonish(arg, "fail args");
        }} catch (e) {{
          audit({{type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)}});
          throw e;
        }}
        const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, maxToolArgBytes);
        audit({{type:"tool_call", tool:name, policy:"allowed", args:redact(args), arg_bytes:argBytes}});
        try {{
          return tools.fail(args);
        }} catch (e) {{
          audit({{type:"tool_error", tool:name, policy:"allowed", message:safeMessage(e)}});
          throw e;
        }}
      }}
      if (name === "slow" && allowSlow) {{
        let args;
        try {{
          args = cloneJsonish(arg, "slow args");
        }} catch (e) {{
          audit({{type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)}});
          throw e;
        }}
        const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, maxToolArgBytes);
        return withToolTimeout(name, args, argBytes, function() {{ return tools.slow(args); }});
      }}
      if (name === "readFile" && fsReadCaps.length > 0) {{
        let args;
        try {{
          args = cloneJsonish(arg, "readFile args");
        }} catch (e) {{
          audit({{type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)}});
          throw e;
        }}
        if (typeof args.path !== "string" || args.path.length === 0) {{
          const err = new TypeError("readFile args path must be a non-empty string");
          audit({{type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(err)}});
          throw err;
        }}
        const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, maxToolArgBytes);
        const file = findFsReadFile(args.path);
        if (file === null) {{
          audit({{type:"tool_denial", tool:name, policy:"denied", reason:"fs_read_path_not_admitted", path:args.path}});
          throw new Error("agent tool denied: readFile path not admitted");
        }}
        if (file.readable !== true) {{
          audit({{type:"tool_denial", tool:name, policy:"denied", reason:file.reason || "fs_read_entry_not_readable", path:args.path, kind:file.kind || "unknown"}});
          throw new Error("agent tool denied: readFile entry not readable");
        }}
        audit({{type:"tool_call", tool:name, policy:"allowed", args:redact(args), arg_bytes:argBytes, path:file.path, bytes:file.bytes, source_hash:file.source_hash}});
        const result = {{path:file.relative, content:file.content, bytes:file.bytes, source_hash:file.source_hash}};
        const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, maxToolResultBytes);
        audit({{type:"tool_result", tool:name, result:redact(result), result_bytes:resultBytes}});
        return result;
      }}
      if (name === "listFiles" && fsReadCaps.length > 0) {{
        let args;
        try {{
          args = arg === undefined ? {{}} : cloneJsonish(arg, "listFiles args");
        }} catch (e) {{
          audit({{type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)}});
          throw e;
        }}
        const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, maxToolArgBytes);
        const files = [];
        for (let i = 0; i < fsReadCaps.length; i++) files.push({{path:fsReadCaps[i].relative, bytes:fsReadCaps[i].bytes, kind:fsReadCaps[i].kind, readable:fsReadCaps[i].readable, reason:fsReadCaps[i].reason, source_hash:fsReadCaps[i].source_hash || null}});
        audit({{type:"tool_call", tool:name, policy:"allowed", args:redact(args), arg_bytes:argBytes, files:files.length}});
        const result = {{files}};
        const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, maxToolResultBytes);
        audit({{type:"tool_result", tool:name, result:redact(result), result_bytes:resultBytes}});
        return result;
      }}
      if (name === "writeArtifact" && fsWriteRoots.length > 0) {{
        let args;
        try {{
          args = cloneJsonish(arg, "writeArtifact args");
        }} catch (e) {{
          audit({{type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)}});
          throw e;
        }}
        if (typeof args.path !== "string" || typeof args.content !== "string") {{
          const err = new TypeError("writeArtifact args path and content must be strings");
          audit({{type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(err)}});
          throw err;
        }}
        const relative = normalizeArtifactPath(args.path);
        if (relative === null) {{
          audit({{type:"tool_denial", tool:name, policy:"denied", reason:"artifact_path_not_admitted", path:args.path}});
          throw new Error("agent tool denied: writeArtifact path not admitted");
        }}
        const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, maxToolArgBytes);
        const bytes = args.content.length;
        if (bytes > maxToolResultBytes) {{
          audit({{type:"budget_exceeded", budget:"artifact_bytes", tool:name, limit:maxToolResultBytes, attempted:bytes}});
          throw new Error("agent artifact byte budget exceeded");
        }}
        const hash = artifactHash(args.content);
        audit({{type:"tool_call", tool:name, policy:"allowed", args:redact({{path:relative, content:"[content omitted]"}}), arg_bytes:argBytes, path:relative, bytes}});
        writeArtifactHost(relative, args.content);
        const result = {{path:relative, bytes, hash}};
        writtenArtifacts.push({{path:relative, bytes, hash}});
        const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, maxToolResultBytes);
        audit({{type:"tool_result", tool:name, result, result_bytes:resultBytes, path:relative, bytes, hash}});
        return result;
      }}
      if (name === "osv.query" && allowOsv) {{
        let args;
        try {{
          args = validateOsvQueryArgs(arg);
        }} catch (e) {{
          audit({{type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)}});
          throw e;
        }}
        const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, maxToolArgBytes);
        if (osvFixture !== null) {{
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint:"fixture://osv/v1/query", transport:"fixture", args:redact(args), arg_bytes:argBytes}});
          const result = osvFixtureLookup(args);
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint:"fixture://osv/v1/query", transport:"fixture", result:redact(result), result_bytes:resultBytes}});
          return result;
        }}
        const cacheKey = JSON.stringify(args);
        if (Object.prototype.hasOwnProperty.call(osvLiveCache, cacheKey)) {{
          const cached = cloneStateValue(osvLiveCache[cacheKey]);
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint:"https://api.osv.dev/v1/query", transport:"memory_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"run"}});
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, cached, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint:"https://api.osv.dev/v1/query", transport:"memory_cache", result:redact(cached), result_bytes:resultBytes, response_bytes:0, status_code:200, duration_ms:0, freshness:"cached_in_run", cache_hit:true}});
          return Promise.resolve(cached);
        }}
        const persistent = namedNetworkPersistentCacheRead(name, args);
        if (persistent !== null) {{
          const cached = cloneStateValue(persistent.result);
          osvLiveCache[cacheKey] = cloneStateValue(cached);
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint:"https://api.osv.dev/v1/query", transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"persistent"}});
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, cached, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint:"https://api.osv.dev/v1/query", transport:"persistent_cache", result:redact(cached), result_bytes:resultBytes, response_bytes:0, status_code:200, duration_ms:0, freshness:"cached_persistent", cache_hit:true, cache_scope:"persistent", cache_path:persistent.path, stored_ms:persistent.stored_ms, age_ms:persistent.age_ms, max_age_ms:namedNetworkCacheMaxAgeMs}});
          return Promise.resolve(cached);
        }}
        if (namedNetworkCacheMode === "offline") {{
          try {{
            namedNetworkOfflineCacheMiss(name, "https://api.osv.dev/v1/query", args, argBytes);
          }} catch (e) {{
            return Promise.reject(e);
          }}
        }}
        const liveStarted = Date.now();
        audit({{type:"tool_call", tool:name, policy:"allowed", endpoint:"https://api.osv.dev/v1/query", transport:"pinned_https", args:redact(args), arg_bytes:argBytes, timeout_ms:toolTimeoutMs}});
        return namedNetworkLiveWithRetry(name, "https://api.osv.dev/v1/query", osvLiveQuery, args, liveStarted, 0).then(function(live) {{
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, live.result, maxToolResultBytes);
          osvLiveCache[cacheKey] = cloneStateValue(live.result);
          namedNetworkPersistentCacheWrite(name, args, live);
          audit({{type:"tool_result", tool:name, endpoint:"https://api.osv.dev/v1/query", transport:"pinned_https", result:redact(live.result), result_bytes:resultBytes, response_bytes:live.response_bytes, status_code:live.status_code, duration_ms:live.duration_ms, freshness:"live"}});
          return live.result;
        }}, function(e) {{
          const taxonomy = namedNetworkErrorTaxonomy(e);
          const retry = namedNetworkRetryInfo(e);
          audit({{type:"tool_error", tool:name, endpoint:"https://api.osv.dev/v1/query", transport:"pinned_https", message:safeMessage(e), duration_ms:Date.now() - liveStarted, error_kind:taxonomy.error_kind, retryable:taxonomy.retryable, retry_policy:retry.policy, retry_attempts:retry.attempts, retry_max_attempts:retry.max_attempts}});
          throw e;
        }});
      }}
      if (name === "npm.metadata" && allowNpmMetadata) {{
        let args;
        try {{
          args = validateNpmMetadataArgs(arg);
        }} catch (e) {{
          audit({{type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)}});
          throw e;
        }}
        const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, maxToolArgBytes);
        const liveStarted = Date.now();
        const endpoint = "https://registry.npmjs.org" + npmMetadataPath(args.package);
        const cacheKey = JSON.stringify(args);
        if (Object.prototype.hasOwnProperty.call(npmMetadataCache, cacheKey)) {{
          const cached = cloneStateValue(npmMetadataCache[cacheKey]);
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"memory_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"run"}});
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, cached, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint, transport:"memory_cache", result:redact(cached), result_bytes:resultBytes, response_bytes:0, status_code:200, duration_ms:0, freshness:"cached_in_run", cache_hit:true}});
          return Promise.resolve(cached);
        }}
        const persistent = namedNetworkPersistentCacheRead(name, args);
        if (persistent !== null) {{
          const cached = cloneStateValue(persistent.result);
          npmMetadataCache[cacheKey] = cloneStateValue(cached);
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"persistent"}});
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, cached, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint, transport:"persistent_cache", result:redact(cached), result_bytes:resultBytes, response_bytes:0, status_code:200, duration_ms:0, freshness:"cached_persistent", cache_hit:true, cache_scope:"persistent", cache_path:persistent.path, stored_ms:persistent.stored_ms, age_ms:persistent.age_ms, max_age_ms:namedNetworkCacheMaxAgeMs}});
          return Promise.resolve(cached);
        }}
        if (namedNetworkCacheMode === "offline") {{
          try {{
            namedNetworkOfflineCacheMiss(name, endpoint, args, argBytes);
          }} catch (e) {{
            return Promise.reject(e);
          }}
        }}
        audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"pinned_https", args:redact(args), arg_bytes:argBytes, timeout_ms:toolTimeoutMs}});
        return namedNetworkLiveWithRetry(name, endpoint, npmMetadataLiveQuery, args, liveStarted, 0).then(function(live) {{
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, live.result, maxToolResultBytes);
          npmMetadataCache[cacheKey] = cloneStateValue(live.result);
          namedNetworkPersistentCacheWrite(name, args, live);
          audit({{type:"tool_result", tool:name, endpoint:live.endpoint, transport:live.transport || "pinned_https", result:redact(live.result), result_bytes:resultBytes, response_bytes:live.response_bytes, status_code:live.status_code, duration_ms:live.duration_ms, freshness:live.transport === "test_transient" ? "test_transient" : "live"}});
          return live.result;
        }}, function(e) {{
          const taxonomy = namedNetworkErrorTaxonomy(e);
          const retry = namedNetworkRetryInfo(e);
          audit({{type:"tool_error", tool:name, endpoint, transport:"pinned_https", message:safeMessage(e), duration_ms:Date.now() - liveStarted, error_kind:taxonomy.error_kind, retryable:taxonomy.retryable, retry_policy:retry.policy, retry_attempts:retry.attempts, retry_max_attempts:retry.max_attempts}});
          throw e;
        }});
      }}
      if (name === "github.issue.read" && allowGithubIssueRead) {{
        let args;
        try {{
          args = validateGithubIssueReadArgs(arg);
        }} catch (e) {{
          audit({{type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)}});
          throw e;
        }}
        const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, maxToolArgBytes);
        const liveStarted = Date.now();
        const endpoint = "https://api.github.com" + githubIssuePath(args);
        const cacheKey = JSON.stringify(args);
        if (Object.prototype.hasOwnProperty.call(githubIssueCache, cacheKey)) {{
          const cached = cloneStateValue(githubIssueCache[cacheKey]);
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"memory_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"run", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, cached, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint, transport:"memory_cache", result:redact(cached), result_bytes:resultBytes, response_bytes:0, status_code:200, duration_ms:0, freshness:"cached_in_run", cache_hit:true}});
          return Promise.resolve(cached);
        }}
        const persistent = namedNetworkPersistentCacheRead(name, args);
        if (persistent !== null) {{
          const cached = cloneStateValue(persistent.result);
          githubIssueCache[cacheKey] = cloneStateValue(cached);
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"persistent", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, cached, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint, transport:"persistent_cache", result:redact(cached), result_bytes:resultBytes, response_bytes:0, status_code:200, duration_ms:0, freshness:"cached_persistent", cache_hit:true, cache_scope:"persistent", cache_path:persistent.path, stored_ms:persistent.stored_ms, age_ms:persistent.age_ms, max_age_ms:namedNetworkCacheMaxAgeMs}});
          return Promise.resolve(cached);
        }}
        if (namedNetworkCacheMode === "offline") {{
          try {{
            namedNetworkOfflineCacheMiss(name, endpoint, args, argBytes);
          }} catch (e) {{
            return Promise.reject(e);
          }}
        }}
        audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"pinned_https", args:redact(args), arg_bytes:argBytes, timeout_ms:toolTimeoutMs, credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
        return namedNetworkLiveWithRetry(name, endpoint, githubIssueLiveQuery, args, liveStarted, 0).then(function(live) {{
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, live.result, maxToolResultBytes);
          githubIssueCache[cacheKey] = cloneStateValue(live.result);
          namedNetworkPersistentCacheWrite(name, args, live);
          audit({{type:"tool_result", tool:name, endpoint:live.endpoint, transport:live.transport || "pinned_https", result:redact(live.result), result_bytes:resultBytes, response_bytes:live.response_bytes, status_code:live.status_code, duration_ms:live.duration_ms, freshness:live.transport === "test_transient" ? "test_transient" : "live"}});
          return live.result;
        }}, function(e) {{
          const taxonomy = namedNetworkErrorTaxonomy(e);
          const retry = namedNetworkRetryInfo(e);
          audit({{type:"tool_error", tool:name, endpoint, transport:"pinned_https", message:safeMessage(e), duration_ms:Date.now() - liveStarted, error_kind:taxonomy.error_kind, retryable:taxonomy.retryable, retry_policy:retry.policy, retry_attempts:retry.attempts, retry_max_attempts:retry.max_attempts}});
          throw e;
        }});
      }}
      if (name === "github.pr.read" && allowGithubPrRead) {{
        let args;
        try {{
          args = validateGithubPrReadArgs(arg);
        }} catch (e) {{
          audit({{type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)}});
          throw e;
        }}
        const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, maxToolArgBytes);
        const liveStarted = Date.now();
        const endpoint = "https://api.github.com" + githubPrPath(args);
        const cacheKey = JSON.stringify(args);
        if (Object.prototype.hasOwnProperty.call(githubPrCache, cacheKey)) {{
          const cached = cloneStateValue(githubPrCache[cacheKey]);
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"memory_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"run", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, cached, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint, transport:"memory_cache", result:redact(cached), result_bytes:resultBytes, response_bytes:0, status_code:200, duration_ms:0, freshness:"cached_in_run", cache_hit:true}});
          return Promise.resolve(cached);
        }}
        const persistent = namedNetworkPersistentCacheRead(name, args);
        if (persistent !== null) {{
          const cached = cloneStateValue(persistent.result);
          githubPrCache[cacheKey] = cloneStateValue(cached);
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"persistent", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, cached, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint, transport:"persistent_cache", result:redact(cached), result_bytes:resultBytes, response_bytes:0, status_code:200, duration_ms:0, freshness:"cached_persistent", cache_hit:true, cache_scope:"persistent", cache_path:persistent.path, stored_ms:persistent.stored_ms, age_ms:persistent.age_ms, max_age_ms:namedNetworkCacheMaxAgeMs}});
          return Promise.resolve(cached);
        }}
        if (namedNetworkCacheMode === "offline") {{
          try {{
            namedNetworkOfflineCacheMiss(name, endpoint, args, argBytes);
          }} catch (e) {{
            return Promise.reject(e);
          }}
        }}
        audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"pinned_https", args:redact(args), arg_bytes:argBytes, timeout_ms:toolTimeoutMs, credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
        return namedNetworkLiveWithRetry(name, endpoint, githubPrLiveQuery, args, liveStarted, 0).then(function(live) {{
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, live.result, maxToolResultBytes);
          githubPrCache[cacheKey] = cloneStateValue(live.result);
          namedNetworkPersistentCacheWrite(name, args, live);
          audit({{type:"tool_result", tool:name, endpoint:live.endpoint, transport:live.transport || "pinned_https", result:redact(live.result), result_bytes:resultBytes, response_bytes:live.response_bytes, status_code:live.status_code, duration_ms:live.duration_ms, freshness:live.transport === "test_transient" ? "test_transient" : "live"}});
          return live.result;
        }}, function(e) {{
          const taxonomy = namedNetworkErrorTaxonomy(e);
          const retry = namedNetworkRetryInfo(e);
          audit({{type:"tool_error", tool:name, endpoint, transport:"pinned_https", message:safeMessage(e), duration_ms:Date.now() - liveStarted, error_kind:taxonomy.error_kind, retryable:taxonomy.retryable, retry_policy:retry.policy, retry_attempts:retry.attempts, retry_max_attempts:retry.max_attempts}});
          throw e;
        }});
      }}
      if (name === "github.pr.files.list" && allowGithubPrFilesList) {{
        let args;
        try {{
          args = validateGithubPrFilesListArgs(arg);
        }} catch (e) {{
          audit({{type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)}});
          throw e;
        }}
        const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, maxToolArgBytes);
        const liveStarted = Date.now();
        const endpoint = "https://api.github.com" + githubPrFilesPath(args);
        const cacheKey = JSON.stringify(args);
        if (Object.prototype.hasOwnProperty.call(githubPrFilesCache, cacheKey)) {{
          const cached = cloneStateValue(githubPrFilesCache[cacheKey]);
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"memory_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"run", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, cached, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint, transport:"memory_cache", result:redact(cached), result_bytes:resultBytes, response_bytes:0, status_code:200, duration_ms:0, freshness:"cached_in_run", cache_hit:true}});
          return Promise.resolve(cached);
        }}
        const persistent = namedNetworkPersistentCacheRead(name, args);
        if (persistent !== null) {{
          const cached = cloneStateValue(persistent.result);
          githubPrFilesCache[cacheKey] = cloneStateValue(cached);
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"persistent", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, cached, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint, transport:"persistent_cache", result:redact(cached), result_bytes:resultBytes, response_bytes:0, status_code:200, duration_ms:0, freshness:"cached_persistent", cache_hit:true, cache_scope:"persistent", cache_path:persistent.path, stored_ms:persistent.stored_ms, age_ms:persistent.age_ms, max_age_ms:namedNetworkCacheMaxAgeMs}});
          return Promise.resolve(cached);
        }}
        if (namedNetworkCacheMode === "offline") {{
          try {{
            namedNetworkOfflineCacheMiss(name, endpoint, args, argBytes);
          }} catch (e) {{
            return Promise.reject(e);
          }}
        }}
        audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"pinned_https", args:redact(args), arg_bytes:argBytes, timeout_ms:toolTimeoutMs, credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
        return namedNetworkLiveWithRetry(name, endpoint, githubPrFilesLiveQuery, args, liveStarted, 0).then(function(live) {{
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, live.result, maxToolResultBytes);
          githubPrFilesCache[cacheKey] = cloneStateValue(live.result);
          namedNetworkPersistentCacheWrite(name, args, live);
          audit({{type:"tool_result", tool:name, endpoint:live.endpoint, transport:live.transport || "pinned_https", result:redact(live.result), result_bytes:resultBytes, response_bytes:live.response_bytes, status_code:live.status_code, duration_ms:live.duration_ms, freshness:live.transport === "test_transient" ? "test_transient" : "live"}});
          return live.result;
        }}, function(e) {{
          const taxonomy = namedNetworkErrorTaxonomy(e);
          const retry = namedNetworkRetryInfo(e);
          audit({{type:"tool_error", tool:name, endpoint, transport:"pinned_https", message:safeMessage(e), duration_ms:Date.now() - liveStarted, error_kind:taxonomy.error_kind, retryable:taxonomy.retryable, retry_policy:retry.policy, retry_attempts:retry.attempts, retry_max_attempts:retry.max_attempts}});
          throw e;
        }});
      }}
      if (name === "github.release.latest.read" && allowGithubReleaseLatestRead) {{
        let args;
        try {{
          args = validateGithubReleaseLatestReadArgs(arg);
        }} catch (e) {{
          audit({{type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)}});
          throw e;
        }}
        const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, maxToolArgBytes);
        const liveStarted = Date.now();
        const endpoint = "https://api.github.com" + githubReleaseLatestPath(args);
        const cacheKey = JSON.stringify(args);
        if (Object.prototype.hasOwnProperty.call(githubReleaseLatestCache, cacheKey)) {{
          const cached = cloneStateValue(githubReleaseLatestCache[cacheKey]);
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"memory_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"run", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, cached, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint, transport:"memory_cache", result:redact(cached), result_bytes:resultBytes, response_bytes:0, status_code:200, duration_ms:0, freshness:"cached_in_run", cache_hit:true}});
          return Promise.resolve(cached);
        }}
        const persistent = namedNetworkPersistentCacheRead(name, args);
        if (persistent !== null) {{
          const cached = cloneStateValue(persistent.result);
          githubReleaseLatestCache[cacheKey] = cloneStateValue(cached);
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"persistent", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, cached, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint, transport:"persistent_cache", result:redact(cached), result_bytes:resultBytes, response_bytes:0, status_code:200, duration_ms:0, freshness:"cached_persistent", cache_hit:true, cache_scope:"persistent", cache_path:persistent.path, stored_ms:persistent.stored_ms, age_ms:persistent.age_ms, max_age_ms:namedNetworkCacheMaxAgeMs}});
          return Promise.resolve(cached);
        }}
        if (namedNetworkCacheMode === "offline") {{
          try {{
            namedNetworkOfflineCacheMiss(name, endpoint, args, argBytes);
          }} catch (e) {{
            return Promise.reject(e);
          }}
        }}
        audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"pinned_https", args:redact(args), arg_bytes:argBytes, timeout_ms:toolTimeoutMs, credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
        return namedNetworkLiveWithRetry(name, endpoint, githubReleaseLatestLiveQuery, args, liveStarted, 0).then(function(live) {{
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, live.result, maxToolResultBytes);
          githubReleaseLatestCache[cacheKey] = cloneStateValue(live.result);
          namedNetworkPersistentCacheWrite(name, args, live);
          audit({{type:"tool_result", tool:name, endpoint:live.endpoint, transport:live.transport || "pinned_https", result:redact(live.result), result_bytes:resultBytes, response_bytes:live.response_bytes, status_code:live.status_code, duration_ms:live.duration_ms, freshness:live.transport === "test_transient" ? "test_transient" : "live"}});
          return live.result;
        }}, function(e) {{
          const taxonomy = namedNetworkErrorTaxonomy(e);
          const retry = namedNetworkRetryInfo(e);
          audit({{type:"tool_error", tool:name, endpoint, transport:"pinned_https", message:safeMessage(e), duration_ms:Date.now() - liveStarted, error_kind:taxonomy.error_kind, retryable:taxonomy.retryable, retry_policy:retry.policy, retry_attempts:retry.attempts, retry_max_attempts:retry.max_attempts}});
          throw e;
        }});
      }}
      if (name === "github.file.read" && allowGithubFileRead) {{
        let args;
        try {{
          args = validateGithubFileReadArgs(arg);
        }} catch (e) {{
          audit({{type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)}});
          throw e;
        }}
        const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, maxToolArgBytes);
        const liveStarted = Date.now();
        const endpoint = "https://api.github.com" + githubFilePath(args);
        const cacheKey = JSON.stringify(args);
        if (Object.prototype.hasOwnProperty.call(githubFileCache, cacheKey)) {{
          const cached = cloneStateValue(githubFileCache[cacheKey]);
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"memory_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"run", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, cached, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint, transport:"memory_cache", result:redact(cached), result_bytes:resultBytes, response_bytes:0, status_code:200, duration_ms:0, freshness:"cached_in_run", cache_hit:true}});
          return Promise.resolve(cached);
        }}
        const persistent = namedNetworkPersistentCacheRead(name, args);
        if (persistent !== null) {{
          const cached = cloneStateValue(persistent.result);
          githubFileCache[cacheKey] = cloneStateValue(cached);
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"persistent", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, cached, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint, transport:"persistent_cache", result:redact(cached), result_bytes:resultBytes, response_bytes:0, status_code:200, duration_ms:0, freshness:"cached_persistent", cache_hit:true, cache_scope:"persistent", cache_path:persistent.path, stored_ms:persistent.stored_ms, age_ms:persistent.age_ms, max_age_ms:namedNetworkCacheMaxAgeMs}});
          return Promise.resolve(cached);
        }}
        if (namedNetworkCacheMode === "offline") {{
          try {{
            namedNetworkOfflineCacheMiss(name, endpoint, args, argBytes);
          }} catch (e) {{
            return Promise.reject(e);
          }}
        }}
        audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"pinned_https", args:redact(args), arg_bytes:argBytes, timeout_ms:toolTimeoutMs, credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
        return namedNetworkLiveWithRetry(name, endpoint, githubFileLiveQuery, args, liveStarted, 0).then(function(live) {{
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, live.result, maxToolResultBytes);
          githubFileCache[cacheKey] = cloneStateValue(live.result);
          namedNetworkPersistentCacheWrite(name, args, live);
          audit({{type:"tool_result", tool:name, endpoint:live.endpoint, transport:live.transport || "pinned_https", result:redact(live.result), result_bytes:resultBytes, response_bytes:live.response_bytes, status_code:live.status_code, duration_ms:live.duration_ms, freshness:live.transport === "test_transient" ? "test_transient" : "live"}});
          return live.result;
        }}, function(e) {{
          const taxonomy = namedNetworkErrorTaxonomy(e);
          const retry = namedNetworkRetryInfo(e);
          audit({{type:"tool_error", tool:name, endpoint, transport:"pinned_https", message:safeMessage(e), duration_ms:Date.now() - liveStarted, error_kind:taxonomy.error_kind, retryable:taxonomy.retryable, retry_policy:retry.policy, retry_attempts:retry.attempts, retry_max_attempts:retry.max_attempts}});
          throw e;
        }});
      }}
      if (name === "github.compare.read" && allowGithubCompareRead) {{
        let args;
        try {{
          args = validateGithubCompareReadArgs(arg);
        }} catch (e) {{
          audit({{type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)}});
          throw e;
        }}
        const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, maxToolArgBytes);
        const liveStarted = Date.now();
        const endpoint = "https://api.github.com" + githubComparePath(args);
        const cacheKey = JSON.stringify(args);
        if (Object.prototype.hasOwnProperty.call(githubCompareCache, cacheKey)) {{
          const cached = cloneStateValue(githubCompareCache[cacheKey]);
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"memory_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"run", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, cached, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint, transport:"memory_cache", result:redact(cached), result_bytes:resultBytes, response_bytes:0, status_code:200, duration_ms:0, freshness:"cached_in_run", cache_hit:true}});
          return Promise.resolve(cached);
        }}
        const persistent = namedNetworkPersistentCacheRead(name, args);
        if (persistent !== null) {{
          const cached = cloneStateValue(persistent.result);
          githubCompareCache[cacheKey] = cloneStateValue(cached);
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"persistent", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, cached, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint, transport:"persistent_cache", result:redact(cached), result_bytes:resultBytes, response_bytes:0, status_code:200, duration_ms:0, freshness:"cached_persistent", cache_hit:true, cache_scope:"persistent", cache_path:persistent.path, stored_ms:persistent.stored_ms, age_ms:persistent.age_ms, max_age_ms:namedNetworkCacheMaxAgeMs}});
          return Promise.resolve(cached);
        }}
        if (namedNetworkCacheMode === "offline") {{
          try {{
            namedNetworkOfflineCacheMiss(name, endpoint, args, argBytes);
          }} catch (e) {{
            return Promise.reject(e);
          }}
        }}
        audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"pinned_https", args:redact(args), arg_bytes:argBytes, timeout_ms:toolTimeoutMs, credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
        return namedNetworkLiveWithRetry(name, endpoint, githubCompareLiveQuery, args, liveStarted, 0).then(function(live) {{
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, live.result, maxToolResultBytes);
          githubCompareCache[cacheKey] = cloneStateValue(live.result);
          namedNetworkPersistentCacheWrite(name, args, live);
          audit({{type:"tool_result", tool:name, endpoint:live.endpoint, transport:live.transport || "pinned_https", result:redact(live.result), result_bytes:resultBytes, response_bytes:live.response_bytes, status_code:live.status_code, duration_ms:live.duration_ms, freshness:live.transport === "test_transient" ? "test_transient" : "live"}});
          return live.result;
        }}, function(e) {{
          const taxonomy = namedNetworkErrorTaxonomy(e);
          const retry = namedNetworkRetryInfo(e);
          audit({{type:"tool_error", tool:name, endpoint, transport:"pinned_https", message:safeMessage(e), duration_ms:Date.now() - liveStarted, error_kind:taxonomy.error_kind, retryable:taxonomy.retryable, retry_policy:retry.policy, retry_attempts:retry.attempts, retry_max_attempts:retry.max_attempts}});
          throw e;
        }});
      }}
      if (name === "github.commit.read" && allowGithubCommitRead) {{
        let args;
        try {{
          args = validateGithubCommitReadArgs(arg);
        }} catch (e) {{
          audit({{type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)}});
          throw e;
        }}
        const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, maxToolArgBytes);
        const liveStarted = Date.now();
        const endpoint = "https://api.github.com" + githubCommitPath(args);
        const cacheKey = JSON.stringify(args);
        if (Object.prototype.hasOwnProperty.call(githubCommitCache, cacheKey)) {{
          const cached = cloneStateValue(githubCommitCache[cacheKey]);
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"memory_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"run", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, cached, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint, transport:"memory_cache", result:redact(cached), result_bytes:resultBytes, response_bytes:0, status_code:200, duration_ms:0, freshness:"cached_in_run", cache_hit:true}});
          return Promise.resolve(cached);
        }}
        const persistent = namedNetworkPersistentCacheRead(name, args);
        if (persistent !== null) {{
          const cached = cloneStateValue(persistent.result);
          githubCommitCache[cacheKey] = cloneStateValue(cached);
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"persistent", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, cached, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint, transport:"persistent_cache", result:redact(cached), result_bytes:resultBytes, response_bytes:0, status_code:200, duration_ms:0, freshness:"cached_persistent", cache_hit:true, cache_scope:"persistent", cache_path:persistent.path, stored_ms:persistent.stored_ms, age_ms:persistent.age_ms, max_age_ms:namedNetworkCacheMaxAgeMs}});
          return Promise.resolve(cached);
        }}
        if (namedNetworkCacheMode === "offline") {{
          try {{
            namedNetworkOfflineCacheMiss(name, endpoint, args, argBytes);
          }} catch (e) {{
            return Promise.reject(e);
          }}
        }}
        audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"pinned_https", args:redact(args), arg_bytes:argBytes, timeout_ms:toolTimeoutMs, credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
        return namedNetworkLiveWithRetry(name, endpoint, githubCommitLiveQuery, args, liveStarted, 0).then(function(live) {{
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, live.result, maxToolResultBytes);
          githubCommitCache[cacheKey] = cloneStateValue(live.result);
          namedNetworkPersistentCacheWrite(name, args, live);
          audit({{type:"tool_result", tool:name, endpoint:live.endpoint, transport:live.transport || "pinned_https", result:redact(live.result), result_bytes:resultBytes, response_bytes:live.response_bytes, status_code:live.status_code, duration_ms:live.duration_ms, freshness:live.transport === "test_transient" ? "test_transient" : "live"}});
          return live.result;
        }}, function(e) {{
          const taxonomy = namedNetworkErrorTaxonomy(e);
          const retry = namedNetworkRetryInfo(e);
          audit({{type:"tool_error", tool:name, endpoint, transport:"pinned_https", message:safeMessage(e), duration_ms:Date.now() - liveStarted, error_kind:taxonomy.error_kind, retryable:taxonomy.retryable, retry_policy:retry.policy, retry_attempts:retry.attempts, retry_max_attempts:retry.max_attempts}});
          throw e;
        }});
      }}
      if (name === "github.repo.read" && allowGithubRepoRead) {{
        let args;
        try {{
          args = validateGithubRepoReadArgs(arg);
        }} catch (e) {{
          audit({{type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)}});
          throw e;
        }}
        const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, maxToolArgBytes);
        const liveStarted = Date.now();
        const endpoint = "https://api.github.com" + githubRepoPath(args);
        const cacheKey = JSON.stringify(args);
        if (Object.prototype.hasOwnProperty.call(githubRepoCache, cacheKey)) {{
          const cached = cloneStateValue(githubRepoCache[cacheKey]);
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"memory_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"run", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, cached, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint, transport:"memory_cache", result:redact(cached), result_bytes:resultBytes, response_bytes:0, status_code:200, duration_ms:0, freshness:"cached_in_run", cache_hit:true}});
          return Promise.resolve(cached);
        }}
        const persistent = namedNetworkPersistentCacheRead(name, args);
        if (persistent !== null) {{
          const cached = cloneStateValue(persistent.result);
          githubRepoCache[cacheKey] = cloneStateValue(cached);
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"persistent", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, cached, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint, transport:"persistent_cache", result:redact(cached), result_bytes:resultBytes, response_bytes:0, status_code:200, duration_ms:0, freshness:"cached_persistent", cache_hit:true, cache_scope:"persistent", cache_path:persistent.path, stored_ms:persistent.stored_ms, age_ms:persistent.age_ms, max_age_ms:namedNetworkCacheMaxAgeMs}});
          return Promise.resolve(cached);
        }}
        if (namedNetworkCacheMode === "offline") {{
          try {{
            namedNetworkOfflineCacheMiss(name, endpoint, args, argBytes);
          }} catch (e) {{
            return Promise.reject(e);
          }}
        }}
        audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"pinned_https", args:redact(args), arg_bytes:argBytes, timeout_ms:toolTimeoutMs, credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
        return namedNetworkLiveWithRetry(name, endpoint, githubRepoLiveQuery, args, liveStarted, 0).then(function(live) {{
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, live.result, maxToolResultBytes);
          githubRepoCache[cacheKey] = cloneStateValue(live.result);
          namedNetworkPersistentCacheWrite(name, args, live);
          audit({{type:"tool_result", tool:name, endpoint:live.endpoint, transport:live.transport || "pinned_https", result:redact(live.result), result_bytes:resultBytes, response_bytes:live.response_bytes, status_code:live.status_code, duration_ms:live.duration_ms, freshness:live.transport === "test_transient" ? "test_transient" : "live"}});
          return live.result;
        }}, function(e) {{
          const taxonomy = namedNetworkErrorTaxonomy(e);
          const retry = namedNetworkRetryInfo(e);
          audit({{type:"tool_error", tool:name, endpoint, transport:"pinned_https", message:safeMessage(e), duration_ms:Date.now() - liveStarted, error_kind:taxonomy.error_kind, retryable:taxonomy.retryable, retry_policy:retry.policy, retry_attempts:retry.attempts, retry_max_attempts:retry.max_attempts}});
          throw e;
        }});
      }}
      if (name === "github.workflow.run.read" && allowGithubWorkflowRunRead) {{
        let args;
        try {{
          args = validateGithubWorkflowRunReadArgs(arg);
        }} catch (e) {{
          audit({{type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)}});
          throw e;
        }}
        const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, maxToolArgBytes);
        const liveStarted = Date.now();
        const endpoint = "https://api.github.com" + githubWorkflowRunPath(args);
        const cacheKey = JSON.stringify(args);
        if (Object.prototype.hasOwnProperty.call(githubWorkflowRunCache, cacheKey)) {{
          const cached = cloneStateValue(githubWorkflowRunCache[cacheKey]);
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"memory_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"run", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, cached, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint, transport:"memory_cache", result:redact(cached), result_bytes:resultBytes, response_bytes:0, status_code:200, duration_ms:0, freshness:"cached_in_run", cache_hit:true}});
          return Promise.resolve(cached);
        }}
        const persistent = namedNetworkPersistentCacheRead(name, args);
        if (persistent !== null) {{
          const cached = cloneStateValue(persistent.result);
          githubWorkflowRunCache[cacheKey] = cloneStateValue(cached);
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"persistent", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, cached, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint, transport:"persistent_cache", result:redact(cached), result_bytes:resultBytes, response_bytes:0, status_code:200, duration_ms:0, freshness:"cached_persistent", cache_hit:true, cache_scope:"persistent", cache_path:persistent.path, stored_ms:persistent.stored_ms, age_ms:persistent.age_ms, max_age_ms:namedNetworkCacheMaxAgeMs}});
          return Promise.resolve(cached);
        }}
        if (namedNetworkCacheMode === "offline") {{
          try {{
            namedNetworkOfflineCacheMiss(name, endpoint, args, argBytes);
          }} catch (e) {{
            return Promise.reject(e);
          }}
        }}
        audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"pinned_https", args:redact(args), arg_bytes:argBytes, timeout_ms:toolTimeoutMs, credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
        return namedNetworkLiveWithRetry(name, endpoint, githubWorkflowRunLiveQuery, args, liveStarted, 0).then(function(live) {{
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, live.result, maxToolResultBytes);
          githubWorkflowRunCache[cacheKey] = cloneStateValue(live.result);
          namedNetworkPersistentCacheWrite(name, args, live);
          audit({{type:"tool_result", tool:name, endpoint:live.endpoint, transport:live.transport || "pinned_https", result:redact(live.result), result_bytes:resultBytes, response_bytes:live.response_bytes, status_code:live.status_code, duration_ms:live.duration_ms, freshness:live.transport === "test_transient" ? "test_transient" : "live"}});
          return live.result;
        }}, function(e) {{
          const taxonomy = namedNetworkErrorTaxonomy(e);
          const retry = namedNetworkRetryInfo(e);
          audit({{type:"tool_error", tool:name, endpoint, transport:"pinned_https", message:safeMessage(e), duration_ms:Date.now() - liveStarted, error_kind:taxonomy.error_kind, retryable:taxonomy.retryable, retry_policy:retry.policy, retry_attempts:retry.attempts, retry_max_attempts:retry.max_attempts}});
          throw e;
        }});
      }}
      if (name === "github.workflow.jobs.list" && allowGithubWorkflowJobsList) {{
        let args;
        try {{
          args = validateGithubWorkflowJobsListArgs(arg);
        }} catch (e) {{
          audit({{type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)}});
          throw e;
        }}
        const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, maxToolArgBytes);
        const liveStarted = Date.now();
        const endpoint = "https://api.github.com" + githubWorkflowJobsPath(args);
        const cacheKey = JSON.stringify(args);
        if (Object.prototype.hasOwnProperty.call(githubWorkflowJobsCache, cacheKey)) {{
          const cached = cloneStateValue(githubWorkflowJobsCache[cacheKey]);
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"memory_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"run", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, cached, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint, transport:"memory_cache", result:redact(cached), result_bytes:resultBytes, response_bytes:0, status_code:200, duration_ms:0, freshness:"cached_in_run", cache_hit:true}});
          return Promise.resolve(cached);
        }}
        const persistent = namedNetworkPersistentCacheRead(name, args);
        if (persistent !== null) {{
          const cached = cloneStateValue(persistent.result);
          githubWorkflowJobsCache[cacheKey] = cloneStateValue(cached);
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"persistent", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, cached, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint, transport:"persistent_cache", result:redact(cached), result_bytes:resultBytes, response_bytes:0, status_code:200, duration_ms:0, freshness:"cached_persistent", cache_hit:true, cache_scope:"persistent", cache_path:persistent.path, stored_ms:persistent.stored_ms, age_ms:persistent.age_ms, max_age_ms:namedNetworkCacheMaxAgeMs}});
          return Promise.resolve(cached);
        }}
        if (namedNetworkCacheMode === "offline") {{
          try {{
            namedNetworkOfflineCacheMiss(name, endpoint, args, argBytes);
          }} catch (e) {{
            return Promise.reject(e);
          }}
        }}
        audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"pinned_https", args:redact(args), arg_bytes:argBytes, timeout_ms:toolTimeoutMs, credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
        return namedNetworkLiveWithRetry(name, endpoint, githubWorkflowJobsLiveQuery, args, liveStarted, 0).then(function(live) {{
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, live.result, maxToolResultBytes);
          githubWorkflowJobsCache[cacheKey] = cloneStateValue(live.result);
          namedNetworkPersistentCacheWrite(name, args, live);
          audit({{type:"tool_result", tool:name, endpoint:live.endpoint, transport:live.transport || "pinned_https", result:redact(live.result), result_bytes:resultBytes, response_bytes:live.response_bytes, status_code:live.status_code, duration_ms:live.duration_ms, freshness:live.transport === "test_transient" ? "test_transient" : "live"}});
          return live.result;
        }}, function(e) {{
          const taxonomy = namedNetworkErrorTaxonomy(e);
          const retry = namedNetworkRetryInfo(e);
          audit({{type:"tool_error", tool:name, endpoint, transport:"pinned_https", message:safeMessage(e), duration_ms:Date.now() - liveStarted, error_kind:taxonomy.error_kind, retryable:taxonomy.retryable, retry_policy:retry.policy, retry_attempts:retry.attempts, retry_max_attempts:retry.max_attempts}});
          throw e;
        }});
      }}
      if (name === "github.check.runs.list" && allowGithubCheckRunsList) {{
        let args;
        try {{
          args = validateGithubCheckRunsListArgs(arg);
        }} catch (e) {{
          audit({{type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)}});
          throw e;
        }}
        const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, maxToolArgBytes);
        const liveStarted = Date.now();
        const endpoint = "https://api.github.com" + githubCheckRunsPath(args);
        const cacheKey = JSON.stringify(args);
        if (Object.prototype.hasOwnProperty.call(githubCheckRunsCache, cacheKey)) {{
          const cached = cloneStateValue(githubCheckRunsCache[cacheKey]);
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"memory_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"run", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, cached, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint, transport:"memory_cache", result:redact(cached), result_bytes:resultBytes, response_bytes:0, status_code:200, duration_ms:0, freshness:"cached_in_run", cache_hit:true}});
          return Promise.resolve(cached);
        }}
        const persistent = namedNetworkPersistentCacheRead(name, args);
        if (persistent !== null) {{
          const cached = cloneStateValue(persistent.result);
          githubCheckRunsCache[cacheKey] = cloneStateValue(cached);
          audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"persistent_cache", args:redact(args), arg_bytes:argBytes, cache_scope:"persistent", credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, cached, maxToolResultBytes);
          audit({{type:"tool_result", tool:name, endpoint, transport:"persistent_cache", result:redact(cached), result_bytes:resultBytes, response_bytes:0, status_code:200, duration_ms:0, freshness:"cached_persistent", cache_hit:true, cache_scope:"persistent", cache_path:persistent.path, stored_ms:persistent.stored_ms, age_ms:persistent.age_ms, max_age_ms:namedNetworkCacheMaxAgeMs}});
          return Promise.resolve(cached);
        }}
        if (namedNetworkCacheMode === "offline") {{
          try {{
            namedNetworkOfflineCacheMiss(name, endpoint, args, argBytes);
          }} catch (e) {{
            return Promise.reject(e);
          }}
        }}
        audit({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"pinned_https", args:redact(args), arg_bytes:argBytes, timeout_ms:toolTimeoutMs, credential_mode:githubIssueCredentialEnv === null ? "none" : "host_env_bearer", credential_env:githubIssueCredentialEnv}});
        return namedNetworkLiveWithRetry(name, endpoint, githubCheckRunsLiveQuery, args, liveStarted, 0).then(function(live) {{
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, live.result, maxToolResultBytes);
          githubCheckRunsCache[cacheKey] = cloneStateValue(live.result);
          namedNetworkPersistentCacheWrite(name, args, live);
          audit({{type:"tool_result", tool:name, endpoint:live.endpoint, transport:live.transport || "pinned_https", result:redact(live.result), result_bytes:resultBytes, response_bytes:live.response_bytes, status_code:live.status_code, duration_ms:live.duration_ms, freshness:live.transport === "test_transient" ? "test_transient" : "live"}});
          return live.result;
        }}, function(e) {{
          const taxonomy = namedNetworkErrorTaxonomy(e);
          const retry = namedNetworkRetryInfo(e);
          audit({{type:"tool_error", tool:name, endpoint, transport:"pinned_https", message:safeMessage(e), duration_ms:Date.now() - liveStarted, error_kind:taxonomy.error_kind, retryable:taxonomy.retryable, retry_policy:retry.policy, retry_attempts:retry.attempts, retry_max_attempts:retry.max_attempts}});
          throw e;
        }});
      }}
      if (name === "model.call" && allowModelCall) {{
        let args;
        try {{
          args = validateModelCallArgs(arg);
        }} catch (e) {{
          audit({{type:"tool_invalid_args", tool:name, policy:"denied", message:safeMessage(e)}});
          throw e;
        }}
        const argBytes = enforcePayloadBudget("tool_arg_bytes", name, args, maxToolArgBytes);
        const started = Date.now();
        if (modelFixture !== null) {{
          audit(Object.assign({{type:"tool_call", tool:name, policy:"allowed", endpoint:"fixture://model/call", transport:"fixture", args:redact(args), arg_bytes:argBytes, timeout_ms:toolTimeoutMs}}, modelAuditDisposition()));
          const result = modelFixtureLookup(args);
          if (result === null) {{
            const message = "agent tool denied: model.call fixture response not found";
            audit({{type:"tool_denial", tool:name, policy:"denied", endpoint:"fixture://model/call", transport:"fixture", message, reason:"model_fixture_response_not_found"}});
            throw new Error(message);
          }}
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, maxToolResultBytes);
          audit(Object.assign({{type:"tool_result", tool:name, endpoint:"fixture://model/call", transport:"fixture", result:redact(result), result_bytes:resultBytes, duration_ms:Date.now() - started, freshness:"fixture"}}, modelAuditDisposition()));
          return Promise.resolve(result);
        }}
        const endpoint = "https://api.openai.com/v1/responses";
        audit(Object.assign({{type:"tool_call", tool:name, policy:"allowed", endpoint, transport:"pinned_https", args:redact(args), arg_bytes:argBytes, timeout_ms:toolTimeoutMs, provider:modelProvider, credential_mode:"host_env_bearer", credential_env:modelApiKeyEnv}}, modelAuditDisposition()));
        let providerPromise;
        try {{
          providerPromise = modelProviderLiveCall(args);
        }} catch (e) {{
          const taxonomy = modelProviderErrorTaxonomy(e);
          audit(Object.assign({{type:"tool_error", tool:name, endpoint, transport:"pinned_https", provider:modelProvider, message:safeMessage(e), duration_ms:Date.now() - started, credential_mode:"host_env_bearer", credential_env:modelApiKeyEnv, error_kind:taxonomy.error_kind, retryable:taxonomy.retryable, provider_error_policy:"classified_no_retry"}}, modelAuditDisposition()));
          throw e;
        }}
        return providerPromise.then(function(live) {{
          const resultBytes = enforcePayloadBudget("tool_result_bytes", name, live.result, maxToolResultBytes);
          audit(Object.assign({{type:"tool_result", tool:name, endpoint:live.endpoint, transport:live.transport || "pinned_https", provider:modelProvider, result:redact(live.result), result_bytes:resultBytes, response_bytes:live.response_bytes, status_code:live.status_code, duration_ms:live.duration_ms, freshness:live.transport === "test_provider" ? "test_provider" : "live", credential_mode:"host_env_bearer", credential_env:modelApiKeyEnv}}, modelAuditDisposition()));
          return live.result;
        }}, function(e) {{
          const taxonomy = modelProviderErrorTaxonomy(e);
          audit(Object.assign({{type:"tool_error", tool:name, endpoint, transport:"pinned_https", provider:modelProvider, message:safeMessage(e), duration_ms:Date.now() - started, credential_mode:"host_env_bearer", credential_env:modelApiKeyEnv, error_kind:taxonomy.error_kind, retryable:taxonomy.retryable, provider_error_policy:"classified_no_retry"}}, modelAuditDisposition()));
          throw e;
        }});
      }}
      if (name === "process" && allowProcess) {{
        let args;
        try {{
          args = validateProcessArgs(arg);
        }} catch (e) {{
          const message = safeMessage(e);
          audit({{type:message.indexOf("agent tool denied:") >= 0 ? "tool_denial" : "tool_invalid_args", tool:name, policy:"denied", message, reason:message.indexOf("process command not admitted") >= 0 ? "process_command_not_admitted" : (message.indexOf("process cwd not admitted") >= 0 ? "process_cwd_not_admitted" : (message.indexOf("process env key not admitted") >= 0 ? "process_env_not_admitted" : "invalid_args"))}});
          throw e;
        }}
        const argBytes = enforcePayloadBudget("tool_arg_bytes", name, {{command:args.command,args:args.args,cwd:args.cwd,env_keys:Object.getOwnPropertyNames(args.env),output:args.output}}, maxToolArgBytes);
        audit({{type:"tool_call", tool:name, policy:"allowed", command:args.command, argv:args.args, cwd:args.cwd, env_keys:Object.getOwnPropertyNames(args.env), output_mode:args.output, arg_bytes:argBytes}});
        const result = runProcessTool(args);
        if (result && typeof result.then === "function") {{
          return result.then(function(resolved) {{
            const resultBytes = enforcePayloadBudget("tool_result_bytes", name, resolved, maxToolResultBytes);
            audit({{type:"tool_result", tool:name, result:redact(resolved), result_bytes:resultBytes, command:args.command, exit_code:resolved.exit_code, signal:resolved.signal, stdout_bytes:resolved.stdout.length, stderr_bytes:resolved.stderr.length, output_mode:resolved.output_mode || "full", duration_ms:resolved.duration_ms}});
            return resolved;
          }}, function(e) {{
            if (safeMessage(e).indexOf("agent tool timeout: process") < 0 && safeMessage(e).indexOf("process output byte budget") < 0) {{
              audit({{type:"tool_error", tool:name, policy:"allowed", command:args.command, message:safeMessage(e)}});
            }}
            throw e;
          }});
        }}
        const resultBytes = enforcePayloadBudget("tool_result_bytes", name, result, maxToolResultBytes);
        audit({{type:"tool_result", tool:name, result:redact(result), result_bytes:resultBytes, command:args.command, exit_code:result.exit_code, signal:result.signal, stdout_bytes:result.stdout.length, stderr_bytes:result.stderr.length, output_mode:result.output_mode || "full", duration_ms:result.duration_ms}});
        return result;
      }}
      audit({{type:"tool_denial", tool:name, policy:"denied"}});
      throw new Error("agent tool denied: " + name);
    }},
    importValue(specifier, binding) {{
      ensureOpen("importValue");
      specifier = String(specifier);
      binding = String(binding);
      if (!admittedModuleSpecifiers[specifier]) {{
        audit({{type:"module_denial", specifier, binding, policy:"denied", reason:"specifier_not_admitted"}});
        throw new Error("agent module denied: " + specifier);
      }}
      audit({{type:"module_import", specifier, binding, policy:"allowed"}});
      return compartment.importValue(specifier, binding).then(function(value) {{
        audit({{type:"module_result", specifier, binding}});
        return value;
      }}, function(e) {{
        audit({{type:"module_error", specifier, binding, message:safeMessage(e)}});
        throw e;
      }});
    }},
    auditNote(note) {{
      ensureOpen("auditNote");
      const cloned = cloneJsonish(note, "agent audit note");
      const bytes = payloadBytes(cloned);
      if (bytes > maxEventBytes) {{
        audit({{type:"budget_exceeded", budget:"audit_note_bytes", limit:maxEventBytes, attempted:bytes}});
        throw new Error("agent audit note budget exceeded");
      }}
      audit({{type:"audit_note", note:redact(cloned), note_bytes:bytes}});
    }},
    auditControls() {{
      ensureOpen("auditControls");
      const controls = {{
        worker_hosted:"same_thread",
        tools: [allowEcho ? "echo" : null, allowFail ? "fail" : null, allowSlow ? "slow" : null, allowOsv ? "osv.query" : null, allowNpmMetadata ? "npm.metadata" : null, allowGithubIssueRead ? "github.issue.read" : null, allowGithubPrRead ? "github.pr.read" : null, allowGithubPrFilesList ? "github.pr.files.list" : null, allowGithubReleaseLatestRead ? "github.release.latest.read" : null, allowGithubFileRead ? "github.file.read" : null, allowGithubCompareRead ? "github.compare.read" : null, allowGithubCommitRead ? "github.commit.read" : null, allowGithubRepoRead ? "github.repo.read" : null, allowGithubWorkflowRunRead ? "github.workflow.run.read" : null, allowGithubWorkflowJobsList ? "github.workflow.jobs.list" : null, allowGithubCheckRunsList ? "github.check.runs.list" : null, allowModelCall ? "model.call" : null, allowProcess ? "process" : null].filter(Boolean),
        event_budget: {{max_events:maxEvents, max_event_bytes:maxEventBytes}},
        tool_payload_budget: {{max_tool_arg_bytes:maxToolArgBytes, max_tool_result_bytes:maxToolResultBytes, process_output_stream_chunk_bytes:processOutputStreamChunkBytes}},
        module_policy: {{
          mode:"explicit_source_modules",
          admitted_count: modulePolicyEntries.length,
          import_hook_count: Object.getOwnPropertyNames(admittedHookSources).length
        }},
        state_controls: {{mode:"single_turn_snapshot", max_state_bytes:maxStateBytes}},
        availability_controls: {{
          sync_timeout:"enforced",
          pending_promise_disposition:"detached_at_turn_end",
          async_tool_timeout:"enforced_for_cli_tool_promises",
          max_steps:maxSteps
        }}
      }};
      const cloned = cloneStateValue(controls);
      audit({{type:"audit_controls", controls:cloneStateValue(cloned)}});
      return cloned;
    }},
    close() {{
      if (!agentClosed) {{
        agentClosed = true;
        audit({{type:"revocation", policy:"closed", surface:"agent"}});
      }}
      return true;
    }}
  }},
  timeout_ms: {timeout_ms},
  step_budget: maxSteps === null ? undefined : maxSteps
}});
compartment.evaluate(`
(function harden(value, seen) {{
  if (value === null || (typeof value !== "object" && typeof value !== "function")) return value;
  if (seen.indexOf(value) >= 0) return value;
  seen.push(value);
  const names = Object.getOwnPropertyNames(value);
  for (let i = 0; i < names.length; i++) harden(value[names[i]], seen);
  return Object.freeze(value);
}})(context, []);
`);
try {{
  enforceStateBudget();
  if (entryModule === null) {{
    compartment.evaluate(agentSource);
  }} else {{
    audit({{type:"module_entry", specifier:entryModule, policy:"allowed"}});
    compartment.import(entryModule).then(function(namespace) {{
      audit({{type:"module_entry_result", specifier:entryModule}});
      return namespace;
    }}, function(e) {{
      audit({{type:"module_entry_error", specifier:entryModule, message:safeMessage(e)}});
      throw e;
    }});
  }}
  audit({{type:"availability_check", sync_timeout:"enforced", tenant_timeout_catchability:"uncatchable_gate", microtask_budget:"enforced", max_microtasks:{max_microtasks}, pending_promise_disposition:"detached_at_turn_end", async_tool_timeout:"enforced_for_cli_tool_promises", tool_timeout_ms:toolTimeoutMs, step_budget:maxSteps === null ? "available_with_--max-steps" : "enforced", max_steps:maxSteps}});
  audit({{type:"authority_check", object_prototype_polluted:Object.prototype[pollutionKey] === true, global_polluted:typeof globalThis[globalPollutionKey] !== "undefined"}});
  const finalState = cloneStateValue(stateStore);
  const finalStateBytes = stateBytes();
  audit({{type:"state_snapshot", state:finalState, state_bytes:finalStateBytes}});
  if (sessionPath !== null) {{
    const nextTurn = sessionTurn + 1;
    fs.writeFileSync(sessionPath, JSON.stringify({{type:"agent_session", turn_id:nextTurn, state:finalState}}));
    audit({{type:"session_save", run_id:runId, path:sessionPath, turn_id:nextTurn, state_bytes:finalStateBytes}});
  }}
  emitRunArtifactManifest("ok", null);
  audit({{type:"run_end", run_id:runId, status:"ok"}});
}} catch (e) {{
  const message = safeMessage(e);
  const kind = message.indexOf("agent scheduler suspended") >= 0 ? "scheduler_suspended" : (message.indexOf("agent event schema validation failed:") >= 0 ? "schema_validation_failed" : (message.indexOf("agent tool timeout:") >= 0 ? "tool_timeout" : (message.indexOf("step budget") >= 0 ? "step_budget_exceeded" : (message.indexOf("timeout") >= 0 ? "timeout" : (message.indexOf("agent compartment closed:") >= 0 ? "closed" : (message.indexOf("agent tool denied:") >= 0 ? "denial" : (message.indexOf("agent module denied:") >= 0 ? "module_denied" : (message.indexOf("budget exceeded") >= 0 ? "budget_exceeded" : (message.indexOf("agent event must be a JSON object") >= 0 ? "invalid_event" : (message.indexOf("state value must be JSON-serializable") >= 0 ? "invalid_state" : (message.indexOf("must be a JSON object") >= 0 ? "invalid_tool_args" : (message.indexOf("agent tool host failure:") >= 0 ? "tool_error" : "exception"))))))))))));
  audit({{type:kind, status:"error", message}});
  emitRunArtifactManifest("error", kind);
  audit({{type:"run_end", run_id:runId, status:"error", reason:kind}});
  throw e;
}}
"#,
        audit = json_string_literal(&audit_log),
        run_id = json_string_literal(&run_id),
        agent_path = json_string_literal(&source_path),
        agent_source = json_string_literal(&source),
        scheduler_await_out = scheduler_await_out_js,
        entry_module = entry_module_js,
        session_file = session_file_js,
        context = json_string_literal(&context_json),
        state = json_string_literal(&state_json),
        allow_echo = if allow_echo { "true" } else { "false" },
        allow_fail = if allow_fail { "true" } else { "false" },
        allow_slow = if allow_slow { "true" } else { "false" },
        allow_osv = if allow_osv { "true" } else { "false" },
        allow_npm_metadata = if allow_npm_metadata { "true" } else { "false" },
        allow_github_issue_read = if allow_github_issue_read {
            "true"
        } else {
            "false"
        },
        allow_github_pr_read = if allow_github_pr_read {
            "true"
        } else {
            "false"
        },
        allow_github_pr_files_list = if allow_github_pr_files_list {
            "true"
        } else {
            "false"
        },
        allow_github_release_latest_read = if allow_github_release_latest_read {
            "true"
        } else {
            "false"
        },
        allow_github_file_read = if allow_github_file_read {
            "true"
        } else {
            "false"
        },
        allow_github_compare_read = if allow_github_compare_read {
            "true"
        } else {
            "false"
        },
        allow_github_commit_read = if allow_github_commit_read {
            "true"
        } else {
            "false"
        },
        allow_github_repo_read = if allow_github_repo_read {
            "true"
        } else {
            "false"
        },
        allow_github_workflow_run_read = if allow_github_workflow_run_read {
            "true"
        } else {
            "false"
        },
        allow_github_workflow_jobs_list = if allow_github_workflow_jobs_list {
            "true"
        } else {
            "false"
        },
        allow_model_call = if allow_model_call { "true" } else { "false" },
        allow_process = if allow_process { "true" } else { "false" },
        max_events = max_events,
        max_event_bytes = max_event_bytes,
        max_tool_arg_bytes = max_tool_arg_bytes,
        max_tool_result_bytes = max_tool_result_bytes,
        process_output_stream_chunk_bytes = process_output_stream_chunk_bytes,
        tool_timeout_ms = tool_timeout_ms,
        max_state_bytes = max_state_bytes,
        max_steps = max_steps_js,
        memory_rss = memory_rss_js,
        max_rss_mb = max_rss_mb_js,
        redact_fields = redact_fields_js,
        approval_required_tools = approval_required_tools_js,
        approved_tools = approved_tools_js,
        approval_log = approval_log_js,
        approval_max_age_ms = approval_max_age_ms_js,
        secret_scopes = secret_scopes_js,
        fs_read_caps = fs_read_caps_js,
        fs_write_roots = fs_write_roots_js,
        osv_fixture = osv_fixture_js,
        model_fixture = model_fixture_js,
        model_provider = model_provider_js,
        model_api_key_env = model_api_key_env_js,
        model_api_key_value = model_api_key_value_js,
        named_network_cache_dir = named_network_cache_dir_js,
        named_network_cache_mode = named_network_cache_mode_js,
        named_network_cache_max_age_ms = named_network_cache_max_age_ms_js,
        named_network_cache_max_entries = named_network_cache_max_entries_js,
        named_network_retry_attempts = named_network_retry_attempts_js,
        github_token_env = github_token_env_js,
        github_token_value = github_token_value_js,
        process_commands = process_commands_js,
        process_cwds = process_cwds_js,
        process_env = process_env_js,
        expected_event_schemas = expected_event_schemas_js,
        max_microtasks = max_microtasks,
        module_entries = module_entries_js,
        import_hook_entries = import_hook_entries_js,
        module_policy_entries = module_policy_entries_js,
        package_imports = package_imports_js,
        import_hooks = import_hooks_js,
        timeout_ms = timeout_ms
    );
    let harness_path = std::env::temp_dir().join(format!(
        "cruft-agent-harness-{}-{}.js",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    if let Err(e) = std::fs::write(&harness_path, harness) {
        eprintln!("cruft agent run: cannot create harness: {e}");
        return ExitCode::from(74);
    }
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("cruft agent run: cannot locate current executable: {e}");
            let _ = std::fs::remove_file(&harness_path);
            return ExitCode::from(70);
        }
    };
    let output = run_agent_harness_child(
        exe,
        &harness_path,
        max_microtasks,
        max_rss_mb,
        None,
        &audit_log,
    );
    let _ = std::fs::remove_file(&harness_path);
    match output {
        Ok(output) if output.status.success() => {
            let _ = std::io::stdout().write_all(&output.stdout);
            let _ = std::io::stderr().write_all(&output.stderr);
            ExitCode::SUCCESS
        }
        Ok(output) => {
            let _ = std::io::stdout().write_all(&output.stdout);
            let _ = std::io::stderr().write_all(&output.stderr);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let memory_rss_already_recorded = std::fs::read_to_string(&audit_log)
                .map(|s| s.contains("\"reason\":\"memory_rss_exceeded\""))
                .unwrap_or(false);
            let timeout_already_recorded = std::fs::read_to_string(&audit_log)
                .map(|s| s.contains("\"reason\":\"timeout\""))
                .unwrap_or(false);
            let schema_validation_already_recorded = std::fs::read_to_string(&audit_log)
                .map(|s| s.contains("\"reason\":\"schema_validation_failed\""))
                .unwrap_or(false);
            let reason = if memory_rss_already_recorded {
                "memory_rss_exceeded"
            } else if timeout_already_recorded {
                "timeout"
            } else if schema_validation_already_recorded {
                "schema_validation_failed"
            } else if stderr.contains("microtask budget exceeded") {
                "microtask_budget_exceeded"
            } else if stderr.contains("step budget") {
                "step_budget_exceeded"
            } else if stderr.contains("agent tool timeout:") {
                "tool_timeout"
            } else if stderr.contains("Interrupted") || stderr.contains("timeout") {
                "timeout"
            } else {
                "child_runtime_error"
            };
            let message = stderr.lines().last().unwrap_or(reason);
            if reason != "memory_rss_exceeded"
                && !timeout_already_recorded
                && !schema_validation_already_recorded
            {
                append_agent_post_turn_failure(&audit_log, reason, message);
            }
            ExitCode::from(output.status.code().unwrap_or(70) as u8)
        }
        Err(e) => {
            eprintln!("cruft agent run: cannot execute runtime harness: {e}");
            ExitCode::from(70)
        }
    }
}

pub(crate) fn agent_load_event_schema(
    kind: &str,
    path: &str,
) -> Result<AgentExpectedEventSchema, String> {
    if kind.is_empty() {
        return Err("expected event kind must be non-empty".to_string());
    }
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read expected event schema {kind:?} at {path}: {e}"))?;
    let value = serde_json::from_str::<serde_json::Value>(&source)
        .map_err(|e| format!("cannot parse expected event schema {kind:?} at {path}: {e}"))?;
    let Some(object) = value.as_object() else {
        return Err(format!(
            "expected event schema {kind:?} at {path} must be a JSON object"
        ));
    };
    if let Some(required) = object.get("required") {
        let Some(required) = required.as_array() else {
            return Err(format!(
                "expected event schema {kind:?} field \"required\" must be an array"
            ));
        };
        for item in required {
            if item.as_str().filter(|s| !s.is_empty()).is_none() {
                return Err(format!(
                    "expected event schema {kind:?} required entries must be non-empty strings"
                ));
            }
        }
    }
    if let Some(properties) = object.get("properties") {
        let Some(properties) = properties.as_object() else {
            return Err(format!(
                "expected event schema {kind:?} field \"properties\" must be an object"
            ));
        };
        for (name, ty) in properties {
            if name.is_empty() {
                return Err(format!(
                    "expected event schema {kind:?} property names must be non-empty"
                ));
            }
            let Some(ty) = ty.as_str() else {
                return Err(format!(
                    "expected event schema {kind:?} property {name:?} type must be a string"
                ));
            };
            if !matches!(
                ty,
                "string" | "number" | "boolean" | "object" | "array" | "null" | "any"
            ) {
                return Err(format!(
                    "expected event schema {kind:?} property {name:?} has unsupported type {ty:?}"
                ));
            }
        }
    }
    if let Some(additional) = object.get("additional_properties") {
        if additional.as_bool().is_none() {
            return Err(format!(
                "expected event schema {kind:?} field \"additional_properties\" must be boolean"
            ));
        }
    }
    Ok(AgentExpectedEventSchema {
        kind: kind.to_string(),
        schema_json: value.to_string(),
    })
}

pub(crate) fn agent_expected_event_schemas_js(schemas: &[AgentExpectedEventSchema]) -> String {
    schemas
        .iter()
        .map(|schema| {
            format!(
                "{{kind:{},schema:{}}}",
                json_string_literal(&schema.kind),
                schema.schema_json
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn json_string_array_literal(values: &[String]) -> String {
    let items = json_string_list_literal(values);
    format!("[{items}]")
}

pub(crate) fn json_string_list_literal(values: &[String]) -> String {
    values
        .iter()
        .map(|value| json_string_literal(value))
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn json_optional_string_literal(value: Option<&str>) -> String {
    value
        .map(json_string_literal)
        .unwrap_or_else(|| "null".to_string())
}

pub(crate) fn json_optional_u64_literal(value: Option<u64>) -> String {
    value
        .map(|n| n.to_string())
        .unwrap_or_else(|| "null".to_string())
}

pub(crate) fn agent_memory_rss_control_js(max_rss_mb: Option<u64>) -> &'static str {
    if max_rss_mb.is_some() {
        "\"enforced_by_child_process_supervisor\""
    } else {
        "\"available_with_--max-rss-mb\""
    }
}

pub(crate) fn agent_worker_source_records_js(records: &[(String, String)]) -> String {
    records
        .iter()
        .map(|(specifier, source)| {
            format!(
                "{{specifier:{},source:{},source_hash:{}}}",
                json_string_literal(specifier),
                json_string_literal(source),
                json_string_literal(&agent_source_hash(source))
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn agent_validate_run_id(run_id: &str) -> bool {
    !run_id.is_empty()
        && run_id.len() <= 128
        && !run_id.split('/').any(|part| part == "..")
        && run_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'/' | b'-'))
}

pub(crate) fn agent_source_has_exported_function_surface(source: &str) -> bool {
    source.lines().any(|line| {
        let line = line.trim_start();
        line.starts_with("export function ")
            || line.starts_with("export async function ")
            || (line.starts_with("export const ")
                && (line.contains("=>") || line.contains("function")))
    })
}

pub(crate) fn agent_first_unadmitted_literal_import_value(
    source: &str,
    admitted: &std::collections::HashSet<String>,
) -> Option<(String, String)> {
    let mut rest = source;
    while let Some(pos) = rest.find("importValue") {
        rest = &rest[pos + "importValue".len()..];
        let Some(after_open) = rest.trim_start().strip_prefix('(') else {
            continue;
        };
        let Some((specifier, after_specifier)) = parse_agent_string_literal(after_open) else {
            continue;
        };
        let Some(after_comma) = after_specifier.trim_start().strip_prefix(',') else {
            continue;
        };
        let binding = parse_agent_string_literal(after_comma)
            .map(|(binding, _)| binding)
            .unwrap_or_default();
        if !admitted.contains(&specifier) {
            return Some((specifier, binding));
        }
        rest = after_comma;
    }
    None
}

fn parse_agent_string_literal(source: &str) -> Option<(String, &str)> {
    let source = source.trim_start();
    let quote = source.as_bytes().first().copied()?;
    if quote != b'"' && quote != b'\'' {
        return None;
    }
    let mut out = String::new();
    let mut escaped = false;
    for (idx, b) in source.bytes().enumerate().skip(1) {
        if escaped {
            out.push(b as char);
            escaped = false;
            continue;
        }
        if b == b'\\' {
            escaped = true;
            continue;
        }
        if b == quote {
            return Some((out, &source[idx + 1..]));
        }
        out.push(b as char);
    }
    None
}

fn child_rss_kb(pid: u32) -> Option<u64> {
    let output = std::process::Command::new("ps")
        .arg("-o")
        .arg("rss=")
        .arg("-p")
        .arg(pid.to_string())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.trim().parse::<u64>().ok()
}

pub(crate) fn append_agent_post_turn_failure(audit_log: &str, reason: &str, message: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(audit_log)
    {
        let _ = writeln!(
            f,
            "{{\"type\":\"post_turn_failure\",\"status\":\"error\",\"reason\":{},\"message\":{}}}",
            json_string_literal(reason),
            json_string_literal(message)
        );
        let _ = writeln!(
            f,
            "{{\"type\":\"run_end\",\"status\":\"error\",\"reason\":{}}}",
            json_string_literal(reason)
        );
    }
}

pub(crate) fn append_agent_module_denial(audit_log: &str, specifier: &str, binding: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(audit_log)
    {
        let _ = writeln!(
            f,
            "{{\"type\":\"module_policy\",\"package_imports\":\"explicit_package_graph_caps\"}}"
        );
        let _ = writeln!(
            f,
            "{{\"type\":\"module_denial\",\"specifier\":{},\"binding\":{},\"policy\":\"denied\",\"reason\":\"specifier_not_admitted\"}}",
            json_string_literal(specifier),
            json_string_literal(binding)
        );
        let _ = writeln!(
            f,
            "{{\"type\":\"run_end\",\"status\":\"error\",\"reason\":\"module_denied\"}}"
        );
    }
}

pub(crate) fn append_agent_unsupported_control(audit_log: &str, control: &str, reason: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(audit_log)
    {
        let _ = writeln!(
            f,
            "{{\"type\":\"unsupported_control\",\"status\":\"error\",\"control\":{},\"reason\":{}}}",
            json_string_literal(control),
            json_string_literal(reason)
        );
        let _ = writeln!(
            f,
            "{{\"type\":\"run_end\",\"status\":\"error\",\"reason\":{}}}",
            json_string_literal(reason)
        );
    }
}

pub(crate) fn run_agent_harness_child(
    exe: std::path::PathBuf,
    harness_path: &std::path::Path,
    max_microtasks: u64,
    max_rss_mb: Option<u64>,
    wall_timeout_ms: Option<u64>,
    audit_log: &str,
) -> Result<std::process::Output, std::io::Error> {
    let mut child = std::process::Command::new(exe)
        .arg(harness_path)
        .env("CRUFT_MICROTASK_BUDGET", max_microtasks.to_string())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    let max_rss_kb = max_rss_mb.map(|mb| mb.saturating_mul(1024));
    let started = std::time::Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return child.wait_with_output().map(|mut output| {
                output.status = status;
                output
            });
        }
        if let Some(limit) = wall_timeout_ms {
            if started.elapsed() > std::time::Duration::from_millis(limit) {
                let _ = child.kill();
                let output = child.wait_with_output()?;
                let message = format!("agent wall timeout exceeded: timeout_ms={limit}");
                append_agent_post_turn_failure(audit_log, "timeout", &message);
                return Ok(output);
            }
        }
        if let Some(limit_kb) = max_rss_kb {
            if let Some(rss_kb) = child_rss_kb(child.id()) {
                if rss_kb > limit_kb {
                    let _ = child.kill();
                    let output = child.wait_with_output()?;
                    let message = format!(
                        "agent memory RSS budget exceeded: rss_kb={} limit_kb={}",
                        rss_kb, limit_kb
                    );
                    append_agent_post_turn_failure(audit_log, "memory_rss_exceeded", &message);
                    return Ok(output);
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
