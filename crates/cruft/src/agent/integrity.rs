use super::packages::{collect_simple_agent_package_graph, is_agent_package_specifier};
use crate::json_string_literal;
use std::process::ExitCode;

pub(crate) fn agent_source_hash(source: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in source.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{h:016x}")
}

fn agent_package_graph_hash(package_json: &str, main_rel: &str, main_source: &str) -> String {
    agent_source_hash(&format!(
        "agent-package-graph:v1\npackage_json:{package_json}\nmain:{main_rel}\nsource:{main_source}"
    ))
}

fn agent_package_graph_entries_hash(
    package_json: &str,
    entries: &[(String, String, String)],
) -> String {
    if entries.len() == 1 {
        return agent_package_graph_hash(package_json, &entries[0].1, &entries[0].2);
    }
    let mut payload = format!("agent-package-graph:v2\npackage_json:{package_json}\n");
    for (specifier, rel, source) in entries {
        payload.push_str("entry:");
        payload.push_str(specifier);
        payload.push('\n');
        payload.push_str("main:");
        payload.push_str(rel);
        payload.push('\n');
        payload.push_str("source:");
        payload.push_str(source);
        payload.push('\n');
    }
    agent_source_hash(&payload)
}

pub(crate) fn agent_package_graph_manifest_entries_hash(
    manifests: &[(String, String)],
    entries: &[(String, String, String)],
) -> String {
    if manifests.len() == 1 {
        return agent_package_graph_entries_hash(&manifests[0].1, entries);
    }
    let mut payload = String::from("agent-package-graph:v3\n");
    for (specifier, package_json) in manifests {
        payload.push_str("package:");
        payload.push_str(specifier);
        payload.push('\n');
        payload.push_str("package_json:");
        payload.push_str(package_json);
        payload.push('\n');
    }
    for (specifier, rel, source) in entries {
        payload.push_str("entry:");
        payload.push_str(specifier);
        payload.push('\n');
        payload.push_str("main:");
        payload.push_str(rel);
        payload.push('\n');
        payload.push_str("source:");
        payload.push_str(source);
        payload.push('\n');
    }
    agent_source_hash(&payload)
}

pub(crate) fn run_agent_hash_subcommand(args: &[String]) -> ExitCode {
    let mut json = false;
    let mut kind = "source";
    let mut specifier: Option<String> = None;
    let mut path: Option<String> = None;
    let mut idx = 0;
    while idx < args.len() {
        let arg = &args[idx];
        if arg == "--json" {
            json = true;
        } else if let Some(value) = arg.strip_prefix("--kind=") {
            kind = value;
        } else if arg == "--kind" {
            idx += 1;
            let Some(value) = args.get(idx) else {
                eprintln!("cruft agent hash: --kind requires an argument");
                return ExitCode::from(64);
            };
            kind = value;
        } else if let Some(value) = arg.strip_prefix("--specifier=") {
            specifier = Some(value.to_string());
        } else if arg == "--specifier" {
            idx += 1;
            let Some(value) = args.get(idx) else {
                eprintln!("cruft agent hash: --specifier requires an argument");
                return ExitCode::from(64);
            };
            specifier = Some(value.to_string());
        } else if path.is_none() {
            path = Some(arg.clone());
        } else {
            eprintln!("cruft agent hash: unexpected argument {arg}");
            return ExitCode::from(64);
        }
        idx += 1;
    }
    let Some(path) = path else {
        eprintln!("cruft agent hash: usage: cruft agent hash <path> [--kind=source|module|import-hook|package] [--specifier=<specifier>] [--json]");
        return ExitCode::from(64);
    };
    let path_obj = std::path::Path::new(&path);
    let (output_kind, integrity, admitted_count) = match kind {
        "source" | "module" | "import-hook" => {
            let source = match std::fs::read_to_string(path_obj) {
                Ok(source) => source,
                Err(e) => {
                    eprintln!("cruft agent hash: cannot read {path}: {e}");
                    return ExitCode::from(66);
                }
            };
            (kind.to_string(), agent_source_hash(&source), 1usize)
        }
        "package" => {
            let Some(package_specifier) = specifier.as_deref() else {
                eprintln!("cruft agent hash: --kind=package requires --specifier=<package>");
                return ExitCode::from(64);
            };
            if !is_agent_package_specifier(package_specifier) {
                eprintln!("cruft agent hash: invalid package specifier {package_specifier:?}");
                return ExitCode::from(64);
            }
            let meta = match std::fs::metadata(path_obj) {
                Ok(meta) => meta,
                Err(e) => {
                    eprintln!("cruft agent hash: cannot stat {path}: {e}");
                    return ExitCode::from(66);
                }
            };
            if meta.is_dir() {
                let (modules, graph_hash, package_kind) =
                    match collect_simple_agent_package_graph(package_specifier, path_obj) {
                        Ok(graph) => graph,
                        Err(reason) => {
                            eprintln!("cruft agent hash: {reason}");
                            return ExitCode::from(65);
                        }
                    };
                (package_kind, graph_hash, modules.len())
            } else {
                let source = match std::fs::read_to_string(path_obj) {
                    Ok(source) => source,
                    Err(e) => {
                        eprintln!("cruft agent hash: cannot read {path}: {e}");
                        return ExitCode::from(66);
                    }
                };
                ("package".to_string(), agent_source_hash(&source), 1usize)
            }
        }
        other => {
            eprintln!(
                "cruft agent hash: unsupported --kind={other:?}; available: source, module, import-hook, package"
            );
            return ExitCode::from(64);
        }
    };
    let specifier_json = specifier.as_deref().unwrap_or("");
    if json {
        println!(
            "{{\"type\":\"agent_hash\",\"kind\":{},\"specifier\":{},\"path\":{},\"integrity\":{},\"admitted_specifiers\":{}}}",
            json_string_literal(&output_kind),
            json_string_literal(specifier_json),
            json_string_literal(&path),
            json_string_literal(&integrity),
            admitted_count
        );
    } else {
        println!("{integrity}");
        if let Some(specifier) = specifier {
            match kind {
                "package" => println!("--package-integrity={specifier}={integrity}"),
                "import-hook" => println!("--import-hook-integrity={specifier}={integrity}"),
                _ => println!("# source_hash={integrity}"),
            }
        }
    }
    ExitCode::SUCCESS
}

pub(crate) fn agent_integrity_repair_command(kind: &str, specifier: &str, path: &str) -> String {
    format!("cruft agent hash {path} --kind={kind} --specifier={specifier}")
}

pub(crate) fn print_agent_package_integrity_repair(specifier: &str, path: &str) {
    eprintln!(
        "cruft agent run: repair: run `{}` and rerun with the printed --package-integrity flag",
        agent_integrity_repair_command("package", specifier, path)
    );
    eprintln!(
        "cruft agent run: project workflow: `cruft agent add-package <project> {specifier} {path}` computes and stores matching integrity"
    );
}

pub(crate) fn print_agent_import_hook_integrity_repair(specifier: &str, path: &str) {
    eprintln!(
        "cruft agent run: repair: run `{}` and rerun with the printed --import-hook-integrity flag",
        agent_integrity_repair_command("import-hook", specifier, path)
    );
    eprintln!(
        "cruft agent run: project workflow: `cruft agent add-import-hook <project> {specifier} {path}` computes and stores matching integrity"
    );
}

pub(crate) fn agent_integrity_for_path(
    kind: &str,
    specifier: Option<&str>,
    path: &str,
) -> Result<String, String> {
    let path_obj = std::path::Path::new(path);
    match kind {
        "source" | "module" | "import-hook" => {
            let source = std::fs::read_to_string(path_obj)
                .map_err(|e| format!("cannot read {path}: {e}"))?;
            Ok(agent_source_hash(&source))
        }
        "package" => {
            let Some(package_specifier) = specifier else {
                return Err("--kind=package requires --specifier=<package>".to_string());
            };
            if !is_agent_package_specifier(package_specifier) {
                return Err(format!("invalid package specifier {package_specifier:?}"));
            }
            let meta =
                std::fs::metadata(path_obj).map_err(|e| format!("cannot stat {path}: {e}"))?;
            if meta.is_dir() {
                let (_modules, graph_hash, _package_kind) =
                    collect_simple_agent_package_graph(package_specifier, path_obj)?;
                Ok(graph_hash)
            } else {
                let source = std::fs::read_to_string(path_obj)
                    .map_err(|e| format!("cannot read {path}: {e}"))?;
                Ok(agent_source_hash(&source))
            }
        }
        other => Err(format!(
            "unsupported integrity kind {other:?}; available: source, module, import-hook, package"
        )),
    }
}
