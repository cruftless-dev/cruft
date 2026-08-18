use super::integrity::agent_package_graph_manifest_entries_hash;

pub(crate) fn is_agent_package_specifier(specifier: &str) -> bool {
    if specifier.is_empty()
        || specifier.starts_with("./")
        || specifier.starts_with("../")
        || specifier.starts_with('/')
        || specifier.contains(':')
    {
        return false;
    }
    if let Some(rest) = specifier.strip_prefix('@') {
        let Some((scope, name)) = rest.split_once('/') else {
            return false;
        };
        return !scope.is_empty() && !name.is_empty() && !name.contains('/');
    }
    !specifier.contains('/')
}

fn simple_agent_package_target(target: &str) -> Result<String, &'static str> {
    let Some(stripped) = target.strip_prefix("./") else {
        return Err("package_graph_resolution_required_not_available");
    };
    if stripped.is_empty()
        || stripped.starts_with('/')
        || stripped.starts_with('\\')
        || stripped.contains('\\')
        || stripped
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err("package_graph_resolution_required_not_available");
    }
    Ok(stripped.to_string())
}

fn simple_agent_package_main_target(main: &str) -> Result<String, &'static str> {
    if main.starts_with("./") {
        simple_agent_package_target(main)
    } else {
        simple_agent_package_target(&format!("./{main}"))
    }
}

fn simple_agent_package_main_entry_target(
    package_path: &std::path::Path,
    main: &str,
) -> Result<String, &'static str> {
    let rel = simple_agent_package_main_target(main)?;
    if package_path.join(&rel).is_file() {
        return Ok(rel);
    }
    if std::path::Path::new(&rel).extension().is_none() {
        let js_rel = format!("{rel}.js");
        if package_path.join(&js_rel).is_file() {
            return Ok(js_rel);
        }
        let index_rel = format!("{rel}/index.js");
        if package_path.join(&index_rel).is_file() {
            return Ok(index_rel);
        }
    }
    Ok(rel)
}

fn simple_agent_exports_target(target: &serde_json::Value) -> Result<String, &'static str> {
    match target {
        serde_json::Value::String(target) => simple_agent_package_target(target),
        serde_json::Value::Object(conditions) => {
            let runtime_keys: Vec<&str> = conditions
                .keys()
                .map(|key| key.as_str())
                .filter(|key| matches!(*key, "import" | "node" | "default"))
                .collect();
            if runtime_keys.is_empty() {
                return Err("package_graph_resolution_required_not_available");
            };
            let target = conditions
                .get("import")
                .or_else(|| conditions.get("node"))
                .or_else(|| conditions.get("default"))
                .ok_or("package_graph_resolution_required_not_available")?;
            simple_agent_exports_target(target)
        }
        serde_json::Value::Array(fallbacks) => {
            for fallback in fallbacks {
                if let Ok(target) = simple_agent_exports_target(fallback) {
                    return Ok(target);
                }
            }
            Err("package_graph_resolution_required_not_available")
        }
        _ => Err("package_graph_resolution_required_not_available"),
    }
}

fn simple_agent_single_star_pattern(s: &str) -> Result<(&str, &str), &'static str> {
    let Some(star) = s.find('*') else {
        return Err("package_graph_resolution_required_not_available");
    };
    if s[star + 1..].contains('*') {
        return Err("package_graph_resolution_required_not_available");
    }
    Ok((&s[..star], &s[star + 1..]))
}

fn simple_agent_expand_exports_pattern(
    package_name: &str,
    key: &str,
    rel_pattern: &str,
    package_path: &std::path::Path,
) -> Result<Vec<(String, String)>, &'static str> {
    let Some(key_subpath) = key.strip_prefix("./") else {
        return Err("package_graph_resolution_required_not_available");
    };
    let (key_prefix, key_suffix) = simple_agent_single_star_pattern(key_subpath)?;
    let (rel_prefix, rel_suffix) = simple_agent_single_star_pattern(rel_pattern)?;
    let valid_pattern_fragment = |fragment: &str| {
        fragment.is_empty()
            || !fragment.contains('\\')
                && fragment
                    .trim_matches('/')
                    .split('/')
                    .all(|part| !part.is_empty() && part != "." && part != "..")
    };
    if key_prefix.is_empty()
        || rel_prefix.is_empty()
        || key_prefix.contains('\\')
        || key_suffix.contains('\\')
        || rel_prefix.contains('\\')
        || rel_suffix.contains('\\')
        || !valid_pattern_fragment(key_prefix)
        || !valid_pattern_fragment(key_suffix)
        || !valid_pattern_fragment(rel_prefix)
        || !valid_pattern_fragment(rel_suffix)
    {
        return Err("package_graph_resolution_required_not_available");
    }
    let rel_dir = rel_prefix
        .rsplit_once('/')
        .map(|(dir, _)| dir)
        .unwrap_or("");
    let scan_dir = if rel_dir.is_empty() {
        package_path.to_path_buf()
    } else {
        package_path.join(rel_dir)
    };
    let entries = std::fs::read_dir(scan_dir)
        .map_err(|_| "package_graph_resolution_required_not_available")?;
    let mut expanded = std::collections::BTreeMap::new();
    for entry in entries {
        let entry = entry.map_err(|_| "package_graph_resolution_required_not_available")?;
        let file_type = entry
            .file_type()
            .map_err(|_| "package_graph_resolution_required_not_available")?;
        if !file_type.is_file() {
            continue;
        }
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or("package_graph_resolution_required_not_available")?;
        let rel = if rel_dir.is_empty() {
            name.to_string()
        } else {
            format!("{rel_dir}/{name}")
        };
        if !rel.starts_with(rel_prefix) || !rel.ends_with(rel_suffix) {
            continue;
        }
        let capture = &rel[rel_prefix.len()..rel.len() - rel_suffix.len()];
        if capture.is_empty() || capture.contains('/') || capture.contains('\\') {
            return Err("package_graph_resolution_required_not_available");
        }
        let specifier = format!("{package_name}/{key_prefix}{capture}{key_suffix}");
        let rel = simple_agent_package_target(&format!("./{rel}"))?;
        expanded.insert(specifier, rel);
    }
    if expanded.is_empty() {
        return Err("package_graph_resolution_required_not_available");
    }
    Ok(expanded.into_iter().collect())
}

fn simple_agent_exports_entries(
    package_name: &str,
    exports: &serde_json::Value,
    package_path: &std::path::Path,
) -> Result<Vec<(String, String)>, &'static str> {
    match exports {
        serde_json::Value::String(_) | serde_json::Value::Array(_) => Ok(vec![(
            package_name.to_string(),
            simple_agent_exports_target(exports)?,
        )]),
        serde_json::Value::Object(exports) => {
            let mut entries = std::collections::BTreeMap::new();
            for (key, value) in exports {
                let rel = simple_agent_exports_target(value)?;
                if key.contains('*') || rel.contains('*') {
                    for (specifier, rel) in
                        simple_agent_expand_exports_pattern(package_name, key, &rel, package_path)?
                    {
                        entries.insert(specifier, rel);
                    }
                    continue;
                }
                let specifier = if key == "." {
                    package_name.to_string()
                } else if let Some(subpath) = key.strip_prefix("./") {
                    if subpath.is_empty()
                        || subpath.starts_with('/')
                        || subpath.contains('\\')
                        || subpath
                            .split('/')
                            .any(|part| part.is_empty() || part == "." || part == "..")
                    {
                        return Err("package_graph_resolution_required_not_available");
                    }
                    format!("{package_name}/{subpath}")
                } else {
                    return Err("package_graph_resolution_required_not_available");
                };
                entries.insert(specifier, rel);
            }
            if entries.is_empty() {
                return Err("package_graph_resolution_required_not_available");
            }
            Ok(entries.into_iter().collect())
        }
        _ => Err("package_graph_resolution_required_not_available"),
    }
}

fn simple_agent_package_entries(
    package_name: &str,
    package_json: &str,
    package_path: &std::path::Path,
) -> Result<Vec<(String, String)>, &'static str> {
    let manifest = serde_json::from_str::<serde_json::Value>(package_json)
        .map_err(|_| "package_graph_resolution_required_not_available")?;
    let Some(obj) = manifest.as_object() else {
        return Err("package_graph_resolution_required_not_available");
    };
    for unsupported in [
        "imports",
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
        "workspaces",
    ] {
        if obj.get(unsupported).is_some() {
            return Err("package_graph_resolution_required_not_available");
        }
    }
    let entries = match obj.get("exports") {
        Some(exports) => simple_agent_exports_entries(package_name, exports, package_path)?,
        None => {
            let main = match obj.get("main") {
                Some(serde_json::Value::String(main)) => {
                    Some(simple_agent_package_main_entry_target(package_path, main)?)
                }
                None => match obj.get("module") {
                    Some(serde_json::Value::String(module)) => {
                        Some(simple_agent_package_main_target(module)?)
                    }
                    None => {
                        let index = package_path.join("index.js");
                        index.is_file().then_some("index.js".to_string())
                    }
                    _ => return Err("package_graph_resolution_required_not_available"),
                },
                _ => return Err("package_graph_resolution_required_not_available"),
            }
            .ok_or("package_graph_resolution_required_not_available")?;
            vec![(package_name.to_string(), main)]
        }
    };
    if let Some(main_field) = obj.get("main") {
        let Some(main) = main_field.as_str() else {
            return Err("package_graph_resolution_required_not_available");
        };
        let main_rel = simple_agent_package_main_entry_target(package_path, main)?;
        if let Some(root) = entries
            .iter()
            .find_map(|(specifier, rel)| (specifier == package_name).then_some(rel))
        {
            if main_rel != *root {
                return Err("package_graph_resolution_required_not_available");
            }
        } else if obj.get("exports").is_none() {
            return Err("package_graph_resolution_required_not_available");
        }
    }
    Ok(entries)
}

fn simple_agent_package_dependency_names(package_json: &str) -> Result<Vec<String>, &'static str> {
    let manifest = serde_json::from_str::<serde_json::Value>(package_json)
        .map_err(|_| "package_graph_resolution_required_not_available")?;
    let Some(obj) = manifest.as_object() else {
        return Err("package_graph_resolution_required_not_available");
    };
    let Some(dependencies) = obj.get("dependencies") else {
        return Ok(Vec::new());
    };
    let Some(dependencies) = dependencies.as_object() else {
        return Err("package_graph_resolution_required_not_available");
    };
    let mut names = Vec::new();
    for (name, version) in dependencies {
        if !is_agent_package_specifier(name) || !version.is_string() {
            return Err("package_graph_resolution_required_not_available");
        }
        names.push(name.clone());
    }
    names.sort();
    Ok(names)
}

fn collect_simple_agent_package_node_modules_entry(
    node_modules: &std::path::Path,
    package_name: &str,
    manifests: &mut Vec<(String, String)>,
    graph_entries: &mut Vec<(String, String, String)>,
    package_modules: &mut Vec<(String, String)>,
    seen_specifiers: &mut std::collections::BTreeSet<String>,
) -> Result<String, &'static str> {
    let package_path = node_modules.join(package_name);
    let package_json_path = package_path.join("package.json");
    let package_json = std::fs::read_to_string(&package_json_path)
        .map_err(|_| "package_graph_resolution_required_not_available")?;
    let package_entries = simple_agent_package_entries(package_name, &package_json, &package_path)?;
    let package_root = std::fs::canonicalize(&package_path)
        .map_err(|_| "package_graph_resolution_required_not_available")?;
    manifests.push((package_name.to_string(), package_json.clone()));
    for (admitted_specifier, rel) in package_entries {
        if admitted_specifier != package_name
            && !admitted_specifier
                .strip_prefix(package_name)
                .is_some_and(|rest| rest.starts_with('/'))
        {
            return Err("package_graph_resolution_required_not_available");
        }
        if !seen_specifiers.insert(admitted_specifier.clone()) {
            return Err("package_graph_resolution_required_not_available");
        }
        let entry_path = package_path.join(&rel);
        let entry_canon = std::fs::canonicalize(&entry_path)
            .map_err(|_| "package_graph_resolution_required_not_available")?;
        if !entry_canon.starts_with(&package_root) {
            return Err("package_graph_resolution_required_not_available");
        }
        let source = std::fs::read_to_string(&entry_canon)
            .map_err(|_| "package_graph_resolution_required_not_available")?;
        graph_entries.push((admitted_specifier.clone(), rel, source.clone()));
        package_modules.push((admitted_specifier, source));
    }
    Ok(package_json)
}

pub(crate) fn collect_simple_agent_package_graph(
    package_name: &str,
    package_path: &std::path::Path,
) -> Result<(Vec<(String, String)>, String, String), &'static str> {
    let package_json_path = package_path.join("package.json");
    let package_json = std::fs::read_to_string(&package_json_path)
        .map_err(|_| "package_graph_resolution_required_not_available")?;
    let package_entries = simple_agent_package_entries(package_name, &package_json, package_path)?;
    let package_root = std::fs::canonicalize(package_path)
        .map_err(|_| "package_graph_resolution_required_not_available")?;
    let mut manifests = vec![(package_name.to_string(), package_json.clone())];
    let mut graph_entries = Vec::new();
    let mut package_modules = Vec::new();
    let mut seen_specifiers = std::collections::BTreeSet::new();
    for (admitted_specifier, rel) in package_entries {
        if !seen_specifiers.insert(admitted_specifier.clone()) {
            return Err("package_graph_resolution_required_not_available");
        }
        let entry_path = package_path.join(&rel);
        let entry_canon = std::fs::canonicalize(&entry_path)
            .map_err(|_| "package_graph_resolution_required_not_available")?;
        if !entry_canon.starts_with(&package_root) {
            return Err("package_graph_resolution_required_not_available");
        }
        let source = std::fs::read_to_string(&entry_canon)
            .map_err(|_| "package_graph_resolution_required_not_available")?;
        graph_entries.push((admitted_specifier.clone(), rel, source.clone()));
        package_modules.push((admitted_specifier, source));
    }
    let dependencies = simple_agent_package_dependency_names(&package_json)?;
    if !dependencies.is_empty() {
        let Some(node_modules) = package_path.parent() else {
            return Err("package_graph_resolution_required_not_available");
        };
        for dependency in dependencies {
            let dependency_json = collect_simple_agent_package_node_modules_entry(
                node_modules,
                &dependency,
                &mut manifests,
                &mut graph_entries,
                &mut package_modules,
                &mut seen_specifiers,
            )?;
            for nested_dependency in simple_agent_package_dependency_names(&dependency_json)? {
                let nested_json = collect_simple_agent_package_node_modules_entry(
                    node_modules,
                    &nested_dependency,
                    &mut manifests,
                    &mut graph_entries,
                    &mut package_modules,
                    &mut seen_specifiers,
                )?;
                for third_dependency in simple_agent_package_dependency_names(&nested_json)? {
                    let third_json = collect_simple_agent_package_node_modules_entry(
                        node_modules,
                        &third_dependency,
                        &mut manifests,
                        &mut graph_entries,
                        &mut package_modules,
                        &mut seen_specifiers,
                    )?;
                    if !simple_agent_package_dependency_names(&third_json)?.is_empty() {
                        return Err("package_graph_resolution_required_not_available");
                    }
                }
            }
        }
    }
    let graph_hash = agent_package_graph_manifest_entries_hash(&manifests, &graph_entries);
    let package_kind = if manifests.len() > 1 {
        if manifests.len() > 2 {
            if manifests.len() > 3 {
                "package_graph_conservative_three_level_dependencies"
            } else {
                "package_graph_conservative_two_level_dependencies"
            }
        } else {
            "package_graph_conservative_one_level_dependencies"
        }
    } else if graph_entries.len() == 1 {
        "package_graph_conservative_root"
    } else {
        "package_graph_conservative_subpath_exports"
    };
    Ok((package_modules, graph_hash, package_kind.to_string()))
}
