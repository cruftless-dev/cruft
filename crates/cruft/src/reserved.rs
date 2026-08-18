
use std::collections::HashSet;
use std::process::ExitCode;
use std::sync::OnceLock;

pub const LIVE_WORDS: &[&str] = &[
    "run", "exec", "cpx", "agent", "compat", "promote", "policy", "trust", "wrap", "unwrap",
    "doctor", "install", "help", "test262", "node",
];

const RESERVED_STUB_WORDS: &[&str] = &[

    "test", "tests", "repl", "eval", "x", "check", "typecheck", "tsc", "inspect", "watch",
    "hot", "reload", "dev", "start", "serve", "shell", "sh", "script", "scripts",
    "i", "add", "a", "remove", "rm", "uninstall", "update", "up", "upgrade", "outdated",
    "ci", "link", "unlink", "pm", "publish", "pack", "patch", "info", "why", "audit",
    "cache", "prune", "dedupe", "dedup", "ls", "list", "tree", "deps", "vendor", "global", "g",
    "build", "compile", "bundle", "minify", "transpile", "strip", "erase", "init", "create",
    "new", "scaffold", "template", "self", "task", "tasks", "bench", "doc", "docs", "fmt",
    "format", "lint", "coverage", "clean", "config", "env", "completions", "version", "sea",
    "snapshot", "graph",

    "cloud", "deploy", "deployment", "deployments", "compute", "runner", "runners", "runs",
    "rerun", "retry", "job", "jobs", "queue", "queues", "worker", "workers", "pool", "scale",
    "region", "regions", "zone", "zones", "edge", "cluster", "clusters", "nodes", "logs", "log",
    "status", "ps", "top", "metrics", "telemetry", "observe", "health", "ping",
    "service", "services", "svc", "function", "functions", "fn", "app", "apps", "project",
    "projects", "workspace", "workspaces", "environment", "environments", "stage", "stages",
    "preview", "previews",
    "org", "orgs", "team", "teams", "account", "accounts", "user", "users", "member", "members",
    "role", "roles", "rbac", "permission", "permissions", "group", "groups", "admin", "owner",
    "auth", "login", "logout", "signin", "signup", "whoami", "me", "session", "sessions",
    "token", "tokens", "secret", "secrets", "key", "keys", "credential", "credentials",
    "identity", "sso", "oidc",
    "marketplace", "market", "registry", "hub", "store", "billing", "plan", "plans", "pricing",
    "usage", "quota", "quotas", "limit", "limits", "invoice", "invoices", "seat", "seats",
    "subscription", "subscriptions", "enterprise", "support", "sla", "terms", "license",
    "contract",
    "schedule", "cron", "trigger", "dispatch", "pipeline", "pipelines", "flow", "flows",
    "workflow", "workflows", "hook", "hooks", "webhook", "webhooks", "event", "events",
    "subscribe", "notify", "alert", "alerts", "incident", "oncall", "pause", "resume", "cancel",
    "abort", "rollback", "release", "releases",
    "policies", "audits", "replay", "provenance", "attest", "compliance",
    "dashboard", "console", "ui", "web", "api", "graphql", "rpc", "gateway", "proxy", "tunnel",
    "connect", "integration", "integrations", "provider", "providers", "source", "sources",
    "sink", "sinks", "settings", "preferences", "profile",

    "agents",

    "harness", "harnesses", "supervise", "supervisor", "orchestrate", "orchestrator", "swarm",
    "crew", "tool", "tools", "toolbox", "mcp", "skill", "skills", "memory", "context", "prompt",
    "prompts",
    "compartment", "compartments", "comp", "seal", "unseal", "sealed", "endow", "endowment",
    "endowments", "membrane", "isolate", "isolated", "realm", "realms", "boundary", "boundaries",
    "sandbox", "vm",
    "cap", "caps", "capability", "capabilities", "grant", "grants", "revoke", "attenuate",
    "mediate", "mediator", "ambient", "advisory", "enforce", "enforced", "permit", "deny",
    "allow", "guard",
    "record", "recording", "journal", "trace", "traces", "span", "attestation", "witness",
    "verify", "proof", "receipt", "receipts", "ledger",

    "plugin", "local",
];

fn stub_set() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        let live: HashSet<&str> = LIVE_WORDS.iter().copied().collect();
        RESERVED_STUB_WORDS
            .iter()
            .copied()
            .filter(|w| !live.contains(w))
            .collect()
    })
}

pub fn is_reserved_stub(word: &str) -> bool {
    stub_set().contains(word)
}

pub fn looks_like_bare_command(token: &str) -> bool {
    !token.is_empty()
        && !token.starts_with('-')
        && !token.contains('/')
        && !token.contains('\\')
        && !token.contains('.')
}

pub fn reserved_stub_exit(word: &str) -> ExitCode {
    eprintln!(
        "cruft: \"{word}\" is a reserved cruft command namespace and is not yet available."
    );
    eprintln!(
        "       To run a package.json script or a file named \"{word}\", use \
         `cruft run {word}` or `cruft ./{word}`."
    );
    ExitCode::from(64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_and_stub_sets_are_disjoint() {
        let live: HashSet<&str> = LIVE_WORDS.iter().copied().collect();
        for w in stub_set() {
            assert!(
                !live.contains(w),
                "reserved word `{w}` is both LIVE and STUB — a live command must never be stubbed",
            );
        }
    }

    #[test]
    fn every_reserved_word_is_live_or_stub() {

        let live: HashSet<&str> = LIVE_WORDS.iter().copied().collect();
        for w in RESERVED_STUB_WORDS {
            assert!(
                live.contains(w) || is_reserved_stub(w),
                "reserved word `{w}` is neither LIVE nor a reserved stub",
            );
        }
    }

    #[test]
    fn key_platform_and_primitive_words_are_reserved() {
        for w in [
            "deploy", "compartment", "caps", "capability", "audit", "seal", "grant", "agents",
            "cloud", "deployment", "workflow", "attest", "provenance", "membrane", "realm",
        ] {
            assert!(is_reserved_stub(w), "expected `{w}` to be a reserved stub word");
        }
    }

    #[test]
    fn live_words_are_not_stubbed() {
        for w in LIVE_WORDS {
            assert!(!is_reserved_stub(w), "live word `{w}` must not be a reserved stub");
        }

        assert!(!is_reserved_stub("spawn"));
    }

    #[test]
    fn bare_command_detection_excludes_paths_and_flags() {
        assert!(looks_like_bare_command("deploy"));
        assert!(!looks_like_bare_command("deploy.js"));
        assert!(!looks_like_bare_command("./deploy"));
        assert!(!looks_like_bare_command("/usr/bin/deploy"));
        assert!(!looks_like_bare_command("-e"));
        assert!(!looks_like_bare_command(""));
    }
}
