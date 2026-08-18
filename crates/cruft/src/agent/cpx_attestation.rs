use crate::json_string_literal;
use std::process::ExitCode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CpxAuditAttestationValidation {
    pub issuer: String,
    pub key_id: String,
    pub package_name: String,
    pub version: String,
    pub registry: String,
    pub integrity: String,
    pub scope: String,
    pub score: String,
    pub disposition: String,
    pub issued_at: String,
    pub expires_at: String,
    pub signature_algorithm: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CpxAuditAttestationTrustKey {
    pub issuer: String,
    pub key_id: String,
    pub algorithm: String,
    pub public_key: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CpxAuditAttestationExpectedBinding {
    pub package_name: String,
    pub version: String,
    pub registry: String,
    pub integrity: String,
}

fn object<'a>(value: &'a serde_json::Value, field: &str) -> Result<&'a serde_json::Map, String> {
    value
        .as_object()
        .ok_or_else(|| format!("{field} must be an object"))
}

fn string_field<'a>(object: &'a serde_json::Map, field: &str) -> Result<&'a str, String> {
    let value = object
        .get(field)
        .and_then(|value| value.as_str())
        .ok_or_else(|| format!("field {field:?} must be a string"))?;
    if value.trim().is_empty() {
        return Err(format!("field {field:?} must not be empty"));
    }
    Ok(value)
}

fn array_field<'a>(
    object: &'a serde_json::Map,
    field: &str,
) -> Result<&'a [serde_json::Value], String> {
    object
        .get(field)
        .and_then(|value| value.as_array())
        .ok_or_else(|| format!("field {field:?} must be an array"))
}

fn validate_disposition(value: &str) -> Result<(), String> {
    match value {
        "allow" | "warn" | "requires_approval" | "blocked" | "unknown" => Ok(()),
        _ => Err(format!("unsupported disposition {value:?}")),
    }
}

fn validate_findings(values: &[serde_json::Value]) -> Result<(), String> {
    for value in values {
        let finding = object(value, "findings[]")?;
        string_field(finding, "id")?;
        string_field(finding, "severity")?;
        string_field(finding, "summary")?;
    }
    Ok(())
}

pub(crate) fn validate_cpx_audit_attestation(
    value: &serde_json::Value,
) -> Result<CpxAuditAttestationValidation, String> {
    let root = object(value, "attestation root")?;
    let schema = string_field(root, "schema")?;
    if schema != "cruft.package_exec_audit_attestation.v1" {
        return Err(format!("unsupported schema {schema:?}"));
    }
    let issuer = string_field(root, "issuer")?.to_string();
    let key_id = string_field(root, "issuer_key_id")?.to_string();
    let package = object(
        root.get("package")
            .ok_or_else(|| "field \"package\" is required".to_string())?,
        "package",
    )?;
    let package_name = string_field(package, "name")?.to_string();
    let version = string_field(package, "version")?.to_string();
    let registry = string_field(package, "registry")?.to_string();
    let integrity = string_field(package, "integrity")?.to_string();
    let scope = string_field(root, "scope")?.to_string();
    let score = string_field(root, "score")?.to_string();
    let disposition = string_field(root, "disposition")?.to_string();
    validate_disposition(&disposition)?;
    let findings = array_field(root, "findings")?;
    validate_findings(findings)?;
    let issued_at = string_field(root, "issued_at")?.to_string();
    let expires_at = string_field(root, "expires_at")?.to_string();
    let signature_algorithm = string_field(root, "signature_algorithm")?.to_string();
    let signature = string_field(root, "signature")?.to_string();
    Ok(CpxAuditAttestationValidation {
        issuer,
        key_id,
        package_name,
        version,
        registry,
        integrity,
        scope,
        score,
        disposition,
        issued_at,
        expires_at,
        signature_algorithm,
        signature,
    })
}

fn parse_trust_key(value: &serde_json::Value) -> Result<CpxAuditAttestationTrustKey, String> {
    let root = object(value, "trusted key root")?;
    let schema = string_field(root, "schema")?;
    if schema != "cruft.package_exec_audit_key.v1" {
        return Err(format!("unsupported trusted key schema {schema:?}"));
    }
    let algorithm = string_field(root, "algorithm")?.to_ascii_lowercase();
    if algorithm != "ed25519" {
        return Err(format!("unsupported trusted key algorithm {algorithm:?}"));
    }
    let public_key = rusty_js_basen::decode_base64(string_field(root, "public_key")?)
        .map_err(|e| format!("invalid trusted key public_key base64: {e:?}"))?;
    if public_key.len() != 32 {
        return Err("trusted key public_key must decode to 32 bytes".to_string());
    }
    Ok(CpxAuditAttestationTrustKey {
        issuer: string_field(root, "issuer")?.to_string(),
        key_id: string_field(root, "issuer_key_id")?.to_string(),
        algorithm,
        public_key,
    })
}

fn canonical_json_excluding_signature_fields(value: &serde_json::Value) -> String {
    fn write(value: &serde_json::Value, out: &mut String, skip_signature: bool) {
        match value {
            serde_json::Value::Null
            | serde_json::Value::Bool(_)
            | serde_json::Value::Number(_)
            | serde_json::Value::String(_) => out.push_str(&value.to_compact_string()),
            serde_json::Value::Array(items) => {
                out.push('[');
                for (idx, item) in items.iter().enumerate() {
                    if idx > 0 {
                        out.push(',');
                    }
                    write(item, out, false);
                }
                out.push(']');
            }
            serde_json::Value::Object(map) => {
                let mut entries: Vec<(&String, &serde_json::Value)> = map.iter().collect();
                if skip_signature {
                    entries.retain(|(key, _)| {
                        key.as_str() != "signature" && key.as_str() != "signature_algorithm"
                    });
                }
                entries.sort_by(|(a, _), (b, _)| a.cmp(b));
                out.push('{');
                for (idx, (key, item)) in entries.into_iter().enumerate() {
                    if idx > 0 {
                        out.push(',');
                    }
                    out.push_str(&json_string_literal(key));
                    out.push(':');
                    write(item, out, false);
                }
                out.push('}');
            }
        }
    }
    let mut out = String::new();
    write(value, &mut out, true);
    out
}

pub(crate) fn verify_cpx_audit_attestation_signature(
    value: &serde_json::Value,
    trust_key: &CpxAuditAttestationTrustKey,
) -> Result<(), String> {
    let validation = validate_cpx_audit_attestation(value)?;
    if validation.issuer != trust_key.issuer {
        return Err(format!(
            "issuer mismatch: attestation {:?} trusted key {:?}",
            validation.issuer, trust_key.issuer
        ));
    }
    if validation.key_id != trust_key.key_id {
        return Err(format!(
            "issuer_key_id mismatch: attestation {:?} trusted key {:?}",
            validation.key_id, trust_key.key_id
        ));
    }
    if validation.signature_algorithm.to_ascii_lowercase() != trust_key.algorithm {
        return Err(format!(
            "signature algorithm mismatch: attestation {:?} trusted key {:?}",
            validation.signature_algorithm, trust_key.algorithm
        ));
    }
    let signature = rusty_js_basen::decode_base64(&validation.signature)
        .map_err(|e| format!("invalid attestation signature base64: {e:?}"))?;
    if signature.len() != 64 {
        return Err("attestation signature must decode to 64 bytes".to_string());
    }
    let payload = canonical_json_excluding_signature_fields(value);
    if rusty_web_crypto::ed25519_verify(&trust_key.public_key, payload.as_bytes(), &signature) {
        Ok(())
    } else {
        Err("attestation signature verification failed".to_string())
    }
}

pub(crate) fn verify_cpx_audit_attestation_binding(
    validation: &CpxAuditAttestationValidation,
    expected: &CpxAuditAttestationExpectedBinding,
) -> Result<(), String> {
    if validation.package_name != expected.package_name {
        return Err(format!(
            "package name mismatch: attestation {:?} expected {:?}",
            validation.package_name, expected.package_name
        ));
    }
    if validation.version != expected.version {
        return Err(format!(
            "package version mismatch: attestation {:?} expected {:?}",
            validation.version, expected.version
        ));
    }
    if validation.registry != expected.registry {
        return Err(format!(
            "registry mismatch: attestation {:?} expected {:?}",
            validation.registry, expected.registry
        ));
    }
    if validation.integrity != expected.integrity {
        return Err(format!(
            "integrity mismatch: attestation {:?} expected {:?}",
            validation.integrity, expected.integrity
        ));
    }
    Ok(())
}

#[derive(Default)]
struct BindingArgs {
    package_name: Option<String>,
    version: Option<String>,
    registry: Option<String>,
    integrity: Option<String>,
}

impl BindingArgs {
    fn into_expected(self) -> Result<Option<CpxAuditAttestationExpectedBinding>, String> {
        let any = self.package_name.is_some()
            || self.version.is_some()
            || self.registry.is_some()
            || self.integrity.is_some();
        if !any {
            return Ok(None);
        }
        let Some(package_name) = self.package_name else {
            return Err("--package-name is required when checking attestation binding".to_string());
        };
        let Some(version) = self.version else {
            return Err(
                "--package-version is required when checking attestation binding".to_string(),
            );
        };
        let Some(registry) = self.registry else {
            return Err("--registry is required when checking attestation binding".to_string());
        };
        let Some(integrity) = self.integrity else {
            return Err("--integrity is required when checking attestation binding".to_string());
        };
        Ok(Some(CpxAuditAttestationExpectedBinding {
            package_name,
            version,
            registry,
            integrity,
        }))
    }
}

fn validate_timestamp_shape(value: &str, field: &str) -> Result<(), String> {
    if value.len() < 20
        || !value.ends_with('Z')
        || value.as_bytes().get(4) != Some(&b'-')
        || value.as_bytes().get(7) != Some(&b'-')
        || value.as_bytes().get(10) != Some(&b'T')
        || value.as_bytes().get(13) != Some(&b':')
        || value.as_bytes().get(16) != Some(&b':')
    {
        return Err(format!("{field} must be an ISO-8601 UTC timestamp"));
    }
    Ok(())
}

pub(crate) fn verify_cpx_audit_attestation_freshness_and_scope(
    validation: &CpxAuditAttestationValidation,
    now: Option<&str>,
    required_scope: Option<&str>,
) -> Result<(), String> {
    if let Some(now) = now {
        validate_timestamp_shape(&validation.issued_at, "issued_at")?;
        validate_timestamp_shape(&validation.expires_at, "expires_at")?;
        validate_timestamp_shape(now, "--now")?;
        if validation.issued_at.as_str() > now {
            return Err(format!(
                "attestation issued_at {:?} is after now {:?}",
                validation.issued_at, now
            ));
        }
        if validation.expires_at.as_str() <= now {
            return Err(format!(
                "attestation expired at {:?} before now {:?}",
                validation.expires_at, now
            ));
        }
    }
    if let Some(required_scope) = required_scope {
        if validation.scope != required_scope {
            return Err(format!(
                "scope mismatch: attestation {:?} required {:?}",
                validation.scope, required_scope
            ));
        }
    }
    Ok(())
}

pub(crate) fn run_agent_cpx_attestation_subcommand(args: &[String]) -> ExitCode {
    let command = args.first().map(|s| s.as_str());
    if command != Some("validate") {
        eprintln!(
            "cruft agent cpx-attestation: usage: cruft agent cpx-attestation validate [--json|--human] [--trusted-key <key.json>] [--package-name <name> --package-version <version> --registry <url> --integrity <sri>] [--now <timestamp>] [--required-scope <scope>] <attestation.json>"
        );
        return ExitCode::from(64);
    }
    let mut json = false;
    let mut human = false;
    let mut trusted_key_path: Option<&str> = None;
    let mut binding = BindingArgs::default();
    let mut now: Option<String> = None;
    let mut required_scope: Option<String> = None;
    let mut path: Option<&str> = None;
    let mut idx = 1;
    while idx < args.len() {
        let arg = &args[idx];
        match arg.as_str() {
            "--json" => json = true,
            "--human" => human = true,
            "--trusted-key" => {
                idx += 1;
                let Some(value) = args.get(idx) else {
                    eprintln!(
                        "cruft agent cpx-attestation validate: --trusted-key requires a path"
                    );
                    return ExitCode::from(64);
                };
                trusted_key_path = Some(value);
            }
            "--package-name" => {
                idx += 1;
                let Some(value) = args.get(idx) else {
                    eprintln!(
                        "cruft agent cpx-attestation validate: --package-name requires a value"
                    );
                    return ExitCode::from(64);
                };
                binding.package_name = Some(value.clone());
            }
            "--package-version" => {
                idx += 1;
                let Some(value) = args.get(idx) else {
                    eprintln!(
                        "cruft agent cpx-attestation validate: --package-version requires a value"
                    );
                    return ExitCode::from(64);
                };
                binding.version = Some(value.clone());
            }
            "--registry" => {
                idx += 1;
                let Some(value) = args.get(idx) else {
                    eprintln!("cruft agent cpx-attestation validate: --registry requires a value");
                    return ExitCode::from(64);
                };
                binding.registry = Some(value.clone());
            }
            "--integrity" => {
                idx += 1;
                let Some(value) = args.get(idx) else {
                    eprintln!("cruft agent cpx-attestation validate: --integrity requires a value");
                    return ExitCode::from(64);
                };
                binding.integrity = Some(value.clone());
            }
            "--now" => {
                idx += 1;
                let Some(value) = args.get(idx) else {
                    eprintln!("cruft agent cpx-attestation validate: --now requires a value");
                    return ExitCode::from(64);
                };
                now = Some(value.clone());
            }
            "--required-scope" => {
                idx += 1;
                let Some(value) = args.get(idx) else {
                    eprintln!(
                        "cruft agent cpx-attestation validate: --required-scope requires a value"
                    );
                    return ExitCode::from(64);
                };
                required_scope = Some(value.clone());
            }
            arg if path.is_none() => path = Some(arg),
            arg => {
                eprintln!("cruft agent cpx-attestation validate: unexpected argument {arg}");
                return ExitCode::from(64);
            }
        }
        idx += 1;
    }
    let Some(path) = path else {
        eprintln!(
            "cruft agent cpx-attestation validate: usage: cruft agent cpx-attestation validate [--json|--human] [--trusted-key <key.json>] [--package-name <name> --package-version <version> --registry <url> --integrity <sri>] [--now <timestamp>] [--required-scope <scope>] <attestation.json>"
        );
        return ExitCode::from(64);
    };
    let expected_binding = match binding.into_expected() {
        Ok(value) => value,
        Err(reason) => {
            if json {
                println!(
                    "{{\"schema\":\"cruft.agent_cpx_attestation_validation.v1\",\"valid\":false,\"reason\":{}}}",
                    json_string_literal(&reason)
                );
            } else {
                eprintln!("cruft agent cpx-attestation validate: {reason}");
            }
            return ExitCode::from(64);
        }
    };
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(e) => {
            let reason = format!("cannot read {path}: {e}");
            if json {
                println!(
                    "{{\"schema\":\"cruft.agent_cpx_attestation_validation.v1\",\"valid\":false,\"reason\":{}}}",
                    json_string_literal(&reason)
                );
            } else {
                eprintln!("cruft agent cpx-attestation validate: {reason}");
            }
            return ExitCode::from(66);
        }
    };
    let value = match serde_json::from_str::<serde_json::Value>(&source) {
        Ok(value) => value,
        Err(e) => {
            let reason = format!("invalid JSON: {e}");
            if json {
                println!(
                    "{{\"schema\":\"cruft.agent_cpx_attestation_validation.v1\",\"valid\":false,\"reason\":{}}}",
                    json_string_literal(&reason)
                );
            } else {
                eprintln!("cruft agent cpx-attestation validate: {reason}");
            }
            return ExitCode::from(65);
        }
    };
    match validate_cpx_audit_attestation(&value) {
        Ok(result) => {
            if let Err(reason) = verify_cpx_audit_attestation_freshness_and_scope(
                &result,
                now.as_deref(),
                required_scope.as_deref(),
            ) {
                if json {
                    println!(
                        "{{\"schema\":\"cruft.agent_cpx_attestation_validation.v1\",\"valid\":false,\"reason\":{}}}",
                        json_string_literal(&reason)
                    );
                } else {
                    eprintln!("cruft agent cpx-attestation validate: {reason}");
                }
                return ExitCode::from(65);
            }
            let freshness_status = if now.is_some() { "fresh" } else { "unchecked" };
            let scope_status = if required_scope.is_some() {
                "matched"
            } else {
                "unchecked"
            };
            let binding_status = if let Some(expected) = expected_binding {
                if let Err(reason) = verify_cpx_audit_attestation_binding(&result, &expected) {
                    if json {
                        println!(
                            "{{\"schema\":\"cruft.agent_cpx_attestation_validation.v1\",\"valid\":false,\"reason\":{}}}",
                            json_string_literal(&reason)
                        );
                    } else {
                        eprintln!("cruft agent cpx-attestation validate: {reason}");
                    }
                    return ExitCode::from(65);
                }
                "matched".to_string()
            } else {
                "unchecked".to_string()
            };
            let signature_status = if let Some(trusted_key_path) = trusted_key_path {
                match read_trusted_key(trusted_key_path).and_then(|trust_key| {
                    verify_cpx_audit_attestation_signature(&value, &trust_key)
                }) {
                    Ok(()) => "verified".to_string(),
                    Err(reason) => {
                        if json {
                            println!(
                                "{{\"schema\":\"cruft.agent_cpx_attestation_validation.v1\",\"valid\":false,\"reason\":{}}}",
                                json_string_literal(&reason)
                            );
                        } else {
                            eprintln!("cruft agent cpx-attestation validate: {reason}");
                        }
                        return ExitCode::from(65);
                    }
                }
            } else {
                "unchecked".to_string()
            };
            if json {
                println!(
                    "{{\"schema\":\"cruft.agent_cpx_attestation_validation.v1\",\"valid\":true,\"issuer\":{},\"issuer_key_id\":{},\"package\":{{\"name\":{},\"version\":{},\"registry\":{},\"integrity\":{}}},\"scope\":{},\"score\":{},\"disposition\":{},\"issued_at\":{},\"expires_at\":{},\"signature_status\":{},\"binding_status\":{},\"freshness_status\":{},\"scope_status\":{}}}",
                    json_string_literal(&result.issuer),
                    json_string_literal(&result.key_id),
                    json_string_literal(&result.package_name),
                    json_string_literal(&result.version),
                    json_string_literal(&result.registry),
                    json_string_literal(&result.integrity),
                    json_string_literal(&result.scope),
                    json_string_literal(&result.score),
                    json_string_literal(&result.disposition),
                    json_string_literal(&result.issued_at),
                    json_string_literal(&result.expires_at),
                    json_string_literal(&signature_status),
                    json_string_literal(&binding_status),
                    json_string_literal(freshness_status),
                    json_string_literal(scope_status)
                );
            } else {
                let _ = human;
                println!(
                    "valid cpx audit attestation: issuer={} package={}@{} scope={} disposition={} signature_status={} binding_status={} freshness_status={} scope_status={}",
                    result.issuer,
                    result.package_name,
                    result.version,
                    result.scope,
                    result.disposition,
                    signature_status,
                    binding_status,
                    freshness_status,
                    scope_status
                );
            }
            ExitCode::SUCCESS
        }
        Err(reason) => {
            if json {
                println!(
                    "{{\"schema\":\"cruft.agent_cpx_attestation_validation.v1\",\"valid\":false,\"reason\":{}}}",
                    json_string_literal(&reason)
                );
            } else {
                eprintln!("cruft agent cpx-attestation validate: {reason}");
            }
            ExitCode::from(65)
        }
    }
}

fn read_trusted_key(path: &str) -> Result<CpxAuditAttestationTrustKey, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read trusted key {path}: {e}"))?;
    let value = serde_json::from_str::<serde_json::Value>(&source)
        .map_err(|e| format!("invalid trusted key JSON: {e}"))?;
    parse_trust_key(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_attestation() -> serde_json::Value {
        serde_json::from_str(
            r#"{
              "schema": "cruft.package_exec_audit_attestation.v1",
              "issuer": "Example Audit Co",
              "issuer_key_id": "example-key-1",
              "package": {
                "name": "left-pad",
                "version": "1.3.0",
                "registry": "https://registry.npmjs.org",
                "integrity": "sha512-example"
              },
              "scope": "package-artifact",
              "score": "reviewed",
              "disposition": "allow",
              "findings": [
                {"id": "A-1", "severity": "info", "summary": "fixture"}
              ],
              "issued_at": "2026-08-12T00:00:00Z",
              "expires_at": "2026-09-12T00:00:00Z",
              "signature_algorithm": "ed25519",
              "signature": "base64-fixture"
            }"#,
        )
        .unwrap()
    }

    fn attestation_missing_integrity() -> serde_json::Value {
        serde_json::from_str(
            r#"{
              "schema": "cruft.package_exec_audit_attestation.v1",
              "issuer": "Example Audit Co",
              "issuer_key_id": "example-key-1",
              "package": {
                "name": "left-pad",
                "version": "1.3.0",
                "registry": "https://registry.npmjs.org"
              },
              "scope": "package-artifact",
              "score": "reviewed",
              "disposition": "allow",
              "findings": [],
              "issued_at": "2026-08-12T00:00:00Z",
              "expires_at": "2026-09-12T00:00:00Z",
              "signature_algorithm": "ed25519",
              "signature": "base64-fixture"
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn cpx_audit_attestation_schema_accepts_valid_shape() {
        let result = validate_cpx_audit_attestation(&valid_attestation()).unwrap();
        assert_eq!(result.issuer, "Example Audit Co");
        assert_eq!(result.package_name, "left-pad");
        assert_eq!(result.disposition, "allow");
    }

    #[test]
    fn cpx_audit_attestation_schema_rejects_missing_binding() {
        let value = attestation_missing_integrity();
        let err = validate_cpx_audit_attestation(&value).unwrap_err();
        assert!(err.contains("integrity"), "{err}");
    }

    fn signed_attestation_and_key() -> (serde_json::Value, CpxAuditAttestationTrustKey) {
        let seed = [7u8; 32];
        let public_key = rusty_web_crypto::ed25519_public_key(&seed);
        let mut value = valid_attestation();
        let payload = canonical_json_excluding_signature_fields(&value);
        let signature = rusty_web_crypto::ed25519_sign(&seed, payload.as_bytes());
        if let serde_json::Value::Object(ref mut root) = value {
            root.insert(
                "signature".to_string(),
                serde_json::Value::String(rusty_js_basen::encode_base64(&signature)),
            );
        }
        (
            value,
            CpxAuditAttestationTrustKey {
                issuer: "Example Audit Co".to_string(),
                key_id: "example-key-1".to_string(),
                algorithm: "ed25519".to_string(),
                public_key,
            },
        )
    }

    #[test]
    fn cpx_audit_attestation_signature_accepts_valid_ed25519() {
        let (value, key) = signed_attestation_and_key();
        verify_cpx_audit_attestation_signature(&value, &key).unwrap();
    }

    #[test]
    fn cpx_audit_attestation_signature_rejects_tamper() {
        let (mut value, key) = signed_attestation_and_key();
        if let serde_json::Value::Object(ref mut root) = value {
            root.insert(
                "score".to_string(),
                serde_json::Value::String("excellent".to_string()),
            );
        }
        let err = verify_cpx_audit_attestation_signature(&value, &key).unwrap_err();
        assert!(err.contains("verification failed"), "{err}");
    }

    #[test]
    fn cpx_audit_attestation_binding_rejects_wrong_integrity() {
        let validation = validate_cpx_audit_attestation(&valid_attestation()).unwrap();
        let err = verify_cpx_audit_attestation_binding(
            &validation,
            &CpxAuditAttestationExpectedBinding {
                package_name: "left-pad".to_string(),
                version: "1.3.0".to_string(),
                registry: "https://registry.npmjs.org".to_string(),
                integrity: "sha512-other".to_string(),
            },
        )
        .unwrap_err();
        assert!(err.contains("integrity mismatch"), "{err}");
    }

    #[test]
    fn cpx_audit_attestation_freshness_and_scope_rejects_expired() {
        let validation = validate_cpx_audit_attestation(&valid_attestation()).unwrap();
        let err = verify_cpx_audit_attestation_freshness_and_scope(
            &validation,
            Some("2026-10-12T00:00:00Z"),
            Some("package-artifact"),
        )
        .unwrap_err();
        assert!(err.contains("expired"), "{err}");
    }

    #[test]
    fn cpx_audit_attestation_freshness_and_scope_rejects_wrong_scope() {
        let validation = validate_cpx_audit_attestation(&valid_attestation()).unwrap();
        let err = verify_cpx_audit_attestation_freshness_and_scope(
            &validation,
            Some("2026-08-13T00:00:00Z"),
            Some("source-review"),
        )
        .unwrap_err();
        assert!(err.contains("scope mismatch"), "{err}");
    }
}
