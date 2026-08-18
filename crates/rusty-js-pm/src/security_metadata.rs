
use std::path::{Path, PathBuf};

use crate::resolver::ResolvedDep;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageRiskFact {
    pub kind: String,
    pub level: String,
    pub subject: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownFact {
    pub field: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryDistFact {
    pub registry: String,
    pub name: String,
    pub version: String,
    pub tarball_url: String,
    pub integrity: Option<String>,
    pub shasum: Option<String>,
    pub publisher: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSecurityMetadata {
    pub package_json: Option<PathBuf>,
    pub registry_dist: Option<RegistryDistFact>,
    pub risks: Vec<PackageRiskFact>,
    pub unknown_facts: Vec<UnknownFact>,
}

fn parse_manifest(path: &Path) -> Result<rusty_json_manifest::Value, String> {
    let body = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    rusty_json_manifest::from_slice::<rusty_json_manifest::Value>(&body)
        .map_err(|e| format!("cannot parse {}: {e}", path.display()))
}

fn push_manifest_dependency_risk(
    risks: &mut Vec<PackageRiskFact>,
    field: &str,
    name: &str,
    spec: &str,
) {
    let spec_lower = spec.to_ascii_lowercase();
    let risk_kind = if spec_lower.starts_with("git+")
        || spec_lower.starts_with("git:")
        || spec_lower.contains("github:")
        || spec_lower.contains("gitlab:")
    {
        Some("git-dependency")
    } else if spec_lower.starts_with("http://")
        || spec_lower.starts_with("https://")
        || spec_lower.ends_with(".tgz")
    {
        Some("tarball-dependency")
    } else if spec == "*" || spec_lower == "latest" {
        Some("novelty-package-risk")
    } else {
        None
    };
    if let Some(kind) = risk_kind {
        risks.push(PackageRiskFact {
            kind: kind.to_string(),
            level: "advisory".to_string(),
            subject: format!("{name}@{spec}"),
            reason: format!("{field} uses a non-pinned or non-registry-shaped spec"),
        });
    }
}

pub fn manifest_security_metadata(package_json: &Path) -> PackageSecurityMetadata {
    let mut risks = Vec::new();
    let mut unknown_facts: Vec<UnknownFact> = Vec::new();
    let Ok(json) = parse_manifest(package_json) else {
        return PackageSecurityMetadata {
            package_json: Some(package_json.to_path_buf()),
            registry_dist: None,
            risks,
            unknown_facts: vec![UnknownFact {
                field: "package_json".to_string(),
                reason: "manifest_parse_unavailable".to_string(),
            }],
        };
    };
    if let Some(scripts) = json.get("scripts").and_then(|v| v.as_object()) {
        for name in ["preinstall", "install", "postinstall", "prepare"] {
            if let Some(script) = scripts.get(name).and_then(|v| v.as_str()) {
                risks.push(PackageRiskFact {
                    kind: "lifecycle-script".to_string(),
                    level: "advisory".to_string(),
                    subject: name.to_string(),
                    reason: script.to_string(),
                });
            }
        }
    }
    for field in ["dependencies", "devDependencies", "optionalDependencies"] {
        if let Some(deps) = json.get(field).and_then(|v| v.as_object()) {
            for (name, value) in deps {
                if let Some(spec) = value.as_str() {
                    push_manifest_dependency_risk(&mut risks, field, name, spec);
                }
            }
        }
    }
    if package_json
        .parent()
        .map(|p| p.join("binding.gyp").is_file())
        .unwrap_or(false)
        || json
            .get("gypfile")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    {
        risks.push(PackageRiskFact {
            kind: "native-addon".to_string(),
            level: "advisory".to_string(),
            subject: package_json.display().to_string(),
            reason: "native build marker observed".to_string(),
        });
    }
    risks.sort_by(|a, b| (&a.kind, &a.subject, &a.reason).cmp(&(&b.kind, &b.subject, &b.reason)));
    risks.dedup_by(|a, b| a.kind == b.kind && a.subject == b.subject && a.reason == b.reason);
    unknown_facts.sort_by(|a, b| (&a.field, &a.reason).cmp(&(&b.field, &b.reason)));
    unknown_facts.dedup();
    PackageSecurityMetadata {
        package_json: Some(package_json.to_path_buf()),
        registry_dist: None,
        risks,
        unknown_facts,
    }
}

pub fn package_dir_security_metadata(package_dir: &Path) -> PackageSecurityMetadata {
    let package_json = package_dir.join("package.json");
    if package_json.is_file() {
        manifest_security_metadata(&package_json)
    } else {
        PackageSecurityMetadata {
            package_json: None,
            registry_dist: None,
            risks: Vec::new(),
            unknown_facts: vec![UnknownFact {
                field: "package_json".to_string(),
                reason: "manifest_not_found".to_string(),
            }],
        }
    }
}

pub fn resolved_dep_security_metadata(
    registry: &str,
    dep: &ResolvedDep,
) -> PackageSecurityMetadata {
    let mut risks = Vec::new();
    let mut unknown_facts = Vec::new();
    if dep.integrity.is_none() {
        if dep.shasum.is_some() {
            risks.push(PackageRiskFact {
                kind: "legacy-shasum-only".to_string(),
                level: "advisory".to_string(),
                subject: format!("{}@{}", dep.name, dep.version),
                reason: "registry dist lacks strong SRI integrity; only shasum is present"
                    .to_string(),
            });
            unknown_facts.push(UnknownFact {
                field: "dist.integrity".to_string(),
                reason: "legacy_shasum_only".to_string(),
            });
        } else {
            unknown_facts.push(UnknownFact {
                field: "dist.integrity".to_string(),
                reason: "missing_from_registry_dist".to_string(),
            });
        }
    }
    if dep.publisher.is_none() {
        unknown_facts.push(UnknownFact {
            field: "publisher".to_string(),
            reason: "missing_from_registry_metadata".to_string(),
        });
    }
    risks.sort_by(|a, b| (&a.kind, &a.subject, &a.reason).cmp(&(&b.kind, &b.subject, &b.reason)));
    risks.dedup_by(|a, b| a.kind == b.kind && a.subject == b.subject && a.reason == b.reason);
    unknown_facts.sort_by(|a, b| (&a.field, &a.reason).cmp(&(&b.field, &b.reason)));
    unknown_facts.dedup();
    PackageSecurityMetadata {
        package_json: None,
        registry_dist: Some(RegistryDistFact {
            registry: registry.to_string(),
            name: dep.name.clone(),
            version: dep.version.clone(),
            tarball_url: dep.tarball_url.clone(),
            integrity: dep.integrity.clone(),
            shasum: dep.shasum.clone(),
            publisher: dep.publisher.clone(),
        }),
        risks,
        unknown_facts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rusty-js-pm-security-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn manifest_security_metadata_reports_static_risks_without_execution() {
        let dir = temp_dir("manifest");
        std::fs::write(
            dir.join("package.json"),
            r#"{
              "name": "risk-fixture",
              "version": "1.0.0",
              "scripts": { "postinstall": "node post.js", "prepare": "node prepare.js" },
              "dependencies": {
                "gitdep": "github:owner/repo",
                "tarballdep": "https://example.com/pkg.tgz",
                "novel": "latest"
              },
              "gypfile": true
            }"#,
        )
        .unwrap();
        std::fs::write(dir.join("post.js"), "throw new Error('must not run')").unwrap();
        let meta = package_dir_security_metadata(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        let risks = meta
            .risks
            .iter()
            .map(|risk| (risk.kind.as_str(), risk.subject.as_str()))
            .collect::<Vec<_>>();
        assert!(risks.contains(&("lifecycle-script", "postinstall")));
        assert!(risks.contains(&("lifecycle-script", "prepare")));
        assert!(risks.contains(&("git-dependency", "gitdep@github:owner/repo")));
        assert!(risks.contains(&(
            "tarball-dependency",
            "tarballdep@https://example.com/pkg.tgz"
        )));
        assert!(risks.contains(&("novelty-package-risk", "novel@latest")));
        assert!(risks.iter().any(|(kind, _)| *kind == "native-addon"));
    }

    #[test]
    fn missing_manifest_is_unknown_fact() {
        let dir = temp_dir("missing");
        let meta = package_dir_security_metadata(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(meta.risks.is_empty());
        assert_eq!(meta.unknown_facts.len(), 1);
        assert_eq!(meta.unknown_facts[0].field, "package_json");
        assert_eq!(meta.unknown_facts[0].reason, "manifest_not_found");
    }

    fn dep(integrity: Option<&str>, shasum: Option<&str>, publisher: Option<&str>) -> ResolvedDep {
        ResolvedDep {
            name: "left-pad".to_string(),
            version: "1.3.0".to_string(),
            tarball_url: "https://registry.example/left-pad/-/left-pad-1.3.0.tgz".to_string(),
            integrity: integrity.map(str::to_string),
            shasum: shasum.map(str::to_string),
            dependencies: BTreeMap::new(),
            optional_dependencies: BTreeMap::new(),
            os: Vec::new(),
            cpu: Vec::new(),
            peer_dependencies: BTreeMap::new(),
            optional_peers: BTreeSet::new(),
            caps: Default::default(),
            publisher: publisher.map(str::to_string),
        }
    }

    #[test]
    fn resolved_dep_security_metadata_projects_registry_dist_facts() {
        let meta = resolved_dep_security_metadata(
            "https://registry.example",
            &dep(Some("sha512-good="), None, Some("alice")),
        );
        let dist = meta.registry_dist.expect("registry dist facts");
        assert_eq!(dist.registry, "https://registry.example");
        assert_eq!(dist.name, "left-pad");
        assert_eq!(dist.version, "1.3.0");
        assert_eq!(
            dist.tarball_url,
            "https://registry.example/left-pad/-/left-pad-1.3.0.tgz"
        );
        assert_eq!(dist.integrity.as_deref(), Some("sha512-good="));
        assert_eq!(dist.publisher.as_deref(), Some("alice"));
        assert!(meta.risks.is_empty());
        assert!(meta.unknown_facts.is_empty());
    }

    #[test]
    fn resolved_dep_security_metadata_names_missing_registry_provenance() {
        let meta = resolved_dep_security_metadata(
            "https://registry.example",
            &dep(None, Some("abc123"), None),
        );
        assert!(meta
            .risks
            .iter()
            .any(|risk| risk.kind == "legacy-shasum-only"));
        let unknowns = meta
            .unknown_facts
            .iter()
            .map(|fact| (fact.field.as_str(), fact.reason.as_str()))
            .collect::<Vec<_>>();
        assert!(unknowns.contains(&("dist.integrity", "legacy_shasum_only")));
        assert!(unknowns.contains(&("publisher", "missing_from_registry_metadata")));
    }
}
