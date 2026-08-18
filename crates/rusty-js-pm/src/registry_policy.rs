
use std::path::{Path, PathBuf};

use crate::resolver::DEFAULT_REGISTRY;

pub const PROJECT_POLICY_FILE: &str = "cruft-registry-policy.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryPolicySnapshot {
    pub default_registry: String,
    pub source: String,
    pub source_path: Option<PathBuf>,
    pub scopes: Vec<RegistryScopeMapping>,
    pub public_fallback: String,
    pub auth_mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryScopeMapping {
    pub scope: String,
    pub registry: String,
}

fn parse_json(body: &str) -> Option<rusty_json_manifest::Value> {
    rusty_json_manifest::from_slice::<rusty_json_manifest::Value>(body.as_bytes()).ok()
}

fn json_string_field(value: &rusty_json_manifest::Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn json_scope_mappings(value: &rusty_json_manifest::Value) -> Vec<RegistryScopeMapping> {
    let mut scopes = value
        .get("scopes")
        .and_then(|value| value.as_object())
        .map(|object| {
            object
                .iter()
                .filter_map(|(scope, registry)| {
                    let registry = registry.as_str()?;
                    if !scope.starts_with('@') || scope.contains('/') || registry.is_empty() {
                        return None;
                    }
                    Some(RegistryScopeMapping {
                        scope: scope.to_string(),
                        registry: registry.to_string(),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    scopes.sort_by(|a, b| a.scope.cmp(&b.scope));
    scopes.dedup_by(|a, b| a.scope == b.scope);
    scopes
}

fn json_public_fallback(value: &rusty_json_manifest::Value) -> String {
    match json_string_field(value, "public_fallback").as_deref() {
        Some("allow") => "allow".to_string(),
        Some("warn") => "warn".to_string(),
        Some("block") => "block".to_string(),
        _ => "unconfigured".to_string(),
    }
}

fn auth_mode_from_env() -> String {
    if std::env::var_os("CRUFT_REGISTRY_TOKEN").is_some() {
        "bearer_env:CRUFT_REGISTRY_TOKEN".to_string()
    } else if std::env::var_os("CRUFT_NPM_TOKEN").is_some() {
        "bearer_env:CRUFT_NPM_TOKEN".to_string()
    } else {
        "none".to_string()
    }
}

fn project_policy_file_from(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };
    loop {
        let candidate = current.join(PROJECT_POLICY_FILE);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !current.pop() {
            return None;
        }
    }
}

pub fn resolve_registry_policy_from(start: &Path) -> RegistryPolicySnapshot {
    if let Some(path) = project_policy_file_from(start) {
        if let Ok(body) = std::fs::read_to_string(&path) {
            if let Some(value) = parse_json(&body) {
                let registry = json_string_field(&value, "default_registry")
                    .unwrap_or_else(|| DEFAULT_REGISTRY.to_string());
                return RegistryPolicySnapshot {
                    default_registry: registry,
                    source: "project".to_string(),
                    source_path: Some(path),
                    scopes: json_scope_mappings(&value),
                    public_fallback: json_public_fallback(&value),
                    auth_mode: auth_mode_from_env(),
                };
            }
        }
    }
    if let Ok(registry) = std::env::var("CRUFT_REGISTRY") {
        return RegistryPolicySnapshot {
            default_registry: registry,
            source: "env:CRUFT_REGISTRY".to_string(),
            source_path: None,
            scopes: Vec::new(),
            public_fallback: "unconfigured".to_string(),
            auth_mode: auth_mode_from_env(),
        };
    }
    if let Ok(registry) = std::env::var("CRUFTLESS_REGISTRY") {
        return RegistryPolicySnapshot {
            default_registry: registry,
            source: "env:CRUFTLESS_REGISTRY".to_string(),
            source_path: None,
            scopes: Vec::new(),
            public_fallback: "unconfigured".to_string(),
            auth_mode: auth_mode_from_env(),
        };
    }
    RegistryPolicySnapshot {
        default_registry: DEFAULT_REGISTRY.to_string(),
        source: "default".to_string(),
        source_path: None,
        scopes: Vec::new(),
        public_fallback: "unconfigured".to_string(),
        auth_mode: auth_mode_from_env(),
    }
}

pub fn resolve_registry_policy() -> RegistryPolicySnapshot {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    resolve_registry_policy_from(&cwd)
}

pub fn selected_registry_for_package<'a>(
    snapshot: &'a RegistryPolicySnapshot,
    package: &str,
) -> (&'a str, Option<&'a str>) {
    if let Some(rest) = package.strip_prefix('@') {
        if let Some((scope_name, _)) = rest.split_once('/') {
            let scope = format!("@{scope_name}");
            if let Some(mapping) = snapshot
                .scopes
                .iter()
                .find(|mapping| mapping.scope == scope)
            {
                return (&mapping.registry, Some(mapping.scope.as_str()));
            }
        }
    }
    (&snapshot.default_registry, None)
}

pub fn package_scope(package: &str) -> Option<String> {
    let rest = package.strip_prefix('@')?;
    let (scope_name, _) = rest.split_once('/')?;
    Some(format!("@{scope_name}"))
}

pub fn public_fallback_blocks_package(snapshot: &RegistryPolicySnapshot, package: &str) -> bool {
    snapshot.public_fallback == "block"
        && package_scope(package).is_some()
        && selected_registry_for_package(snapshot, package).1.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rusty-js-pm-registry-policy-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn project_registry_policy_overrides_env_snapshot() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = temp_dir("project");
        std::fs::write(
            dir.join(PROJECT_POLICY_FILE),
            r#"{"default_registry":"https://registry.project.example"}"#,
        )
        .unwrap();
        std::env::set_var("CRUFT_REGISTRY", "https://registry.env.example");
        std::env::remove_var("CRUFT_REGISTRY_TOKEN");
        std::env::remove_var("CRUFT_NPM_TOKEN");
        let snapshot = resolve_registry_policy_from(&dir);
        std::env::remove_var("CRUFT_REGISTRY");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            snapshot.default_registry,
            "https://registry.project.example"
        );
        assert_eq!(snapshot.source, "project");
        assert!(snapshot.source_path.is_some());
        assert_eq!(snapshot.public_fallback, "unconfigured");
        assert_eq!(snapshot.auth_mode, "none");
    }

    #[test]
    fn project_registry_policy_maps_package_scope() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = temp_dir("scope");
        std::fs::write(
            dir.join(PROJECT_POLICY_FILE),
            r#"{"default_registry":"https://registry.default.example","scopes":{"@company":"https://registry.company.example"}}"#,
        )
        .unwrap();
        let snapshot = resolve_registry_policy_from(&dir);
        let (registry, matched_scope) = selected_registry_for_package(&snapshot, "@company/tool");
        let (default_registry, default_scope) =
            selected_registry_for_package(&snapshot, "left-pad");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(registry, "https://registry.company.example");
        assert_eq!(matched_scope, Some("@company"));
        assert_eq!(default_registry, "https://registry.default.example");
        assert_eq!(default_scope, None);
        assert_eq!(snapshot.public_fallback, "unconfigured");
    }

    #[test]
    fn project_registry_policy_blocks_unmapped_scoped_public_fallback() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = temp_dir("fallback-block");
        std::fs::write(
            dir.join(PROJECT_POLICY_FILE),
            r#"{"default_registry":"https://registry.npmjs.org","scopes":{"@company":"https://registry.company.example"},"public_fallback":"block"}"#,
        )
        .unwrap();
        let snapshot = resolve_registry_policy_from(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(snapshot.public_fallback, "block");
        assert!(public_fallback_blocks_package(&snapshot, "@other/tool"));
        assert!(!public_fallback_blocks_package(&snapshot, "@company/tool"));
        assert!(!public_fallback_blocks_package(&snapshot, "left-pad"));
    }

    #[test]
    fn env_registry_policy_overrides_default() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = temp_dir("env");
        std::env::set_var("CRUFT_REGISTRY", "https://registry.env.example");
        let snapshot = resolve_registry_policy_from(&dir);
        std::env::remove_var("CRUFT_REGISTRY");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(snapshot.default_registry, "https://registry.env.example");
        assert_eq!(snapshot.source, "env:CRUFT_REGISTRY");
        assert!(snapshot.source_path.is_none());
        assert_eq!(snapshot.public_fallback, "unconfigured");
    }

    #[test]
    fn default_registry_policy_is_explicit() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = temp_dir("default");
        std::env::remove_var("CRUFT_REGISTRY");
        std::env::remove_var("CRUFTLESS_REGISTRY");
        std::env::remove_var("CRUFT_REGISTRY_TOKEN");
        std::env::remove_var("CRUFT_NPM_TOKEN");
        let snapshot = resolve_registry_policy_from(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(snapshot.default_registry, DEFAULT_REGISTRY);
        assert_eq!(snapshot.source, "default");
        assert_eq!(snapshot.public_fallback, "unconfigured");
        assert_eq!(snapshot.auth_mode, "none");
    }

    #[test]
    fn registry_policy_records_auth_mode_without_token_value() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = temp_dir("auth-mode");
        std::env::set_var("CRUFT_REGISTRY_TOKEN", "super-secret-token");
        std::env::remove_var("CRUFT_NPM_TOKEN");
        let snapshot = resolve_registry_policy_from(&dir);
        std::env::remove_var("CRUFT_REGISTRY_TOKEN");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(snapshot.auth_mode, "bearer_env:CRUFT_REGISTRY_TOKEN");
        assert!(!snapshot.auth_mode.contains("super-secret-token"));
    }
}
