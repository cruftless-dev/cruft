
use std::collections::BTreeMap;
use std::path::Path;

use rusty_json_manifest::Value;

pub type ModuleMap = BTreeMap<String, String>;

pub const CONDITIONS: &[&str] = &["node", "import", "module-sync", "default"];

pub fn compile_module_map(manifest: &Value, package_root: &Path) -> ModuleMap {
    compile_with_conditions(manifest, package_root, CONDITIONS)
}

pub fn compile_with_conditions(
    manifest: &Value,
    package_root: &Path,
    conditions: &[&str],
) -> ModuleMap {
    let mut map = ModuleMap::new();

    if let Some(exports) = manifest.get("exports") {
        resolve_exports(exports, conditions, &mut map);
    } else {

        if let Some(Value::String(main)) = manifest.get("main") {
            let target = normalize_rel(main);

            map.insert(
                ".".to_string(),
                resolve_file_or_index(&target, package_root),
            );
        } else if let Some(idx) = probe_index(package_root) {
            map.insert(".".to_string(), idx);
        }
    }

    if let Some(Value::Object(imports)) = manifest.get("imports") {
        for (key, target) in imports {
            if !key.starts_with('#') {
                continue;
            }
            if key.contains('*') {

                if let Some(resolved) = resolve_target(target, conditions) {
                    map.insert(key.clone(), normalize_rel(&resolved));
                }
            } else if let Some(resolved) = resolve_target(target, conditions) {
                map.insert(key.clone(), normalize_rel(&resolved));
            }
        }
    }

    map
}

fn resolve_exports(exports: &Value, conditions: &[&str], map: &mut ModuleMap) {
    match exports {

        Value::String(s) => {
            map.insert(".".to_string(), normalize_rel(s));
        }
        Value::Object(obj) => {

            let is_subpath_map = obj.keys().any(|k| k.starts_with('.'));
            if is_subpath_map {
                for (subpath, target) in obj {
                    if !subpath.starts_with('.') {
                        continue;
                    }
                    if subpath.contains('*') {
                        if let Some(resolved) = resolve_target(target, conditions) {
                            map.insert(subpath.clone(), normalize_rel(&resolved));
                        }
                    } else if let Some(resolved) = resolve_target(target, conditions) {
                        map.insert(subpath.clone(), normalize_rel(&resolved));
                    }
                }
            } else if let Some(resolved) = resolve_target(exports, conditions) {

                map.insert(".".to_string(), normalize_rel(&resolved));
            }
        }
        _ => {}
    }
}

fn resolve_target(target: &Value, conditions: &[&str]) -> Option<String> {
    match target {
        Value::String(s) => Some(s.clone()),
        Value::Object(conds) => {

            for cond in conditions {
                if let Some(v) = conds.get(*cond) {
                    if let Some(r) = resolve_target(v, conditions) {
                        return Some(r);
                    }
                }
            }

            if let Some(v) = conds.get("require") {
                return resolve_target(v, conditions);
            }
            None
        }
        Value::Array(items) => {
            for item in items {
                if let Some(r) = resolve_target(item, conditions) {
                    return Some(r);
                }
            }
            None
        }
        _ => None,
    }
}

fn normalize_rel(p: &str) -> String {
    if p.starts_with("./") || p.starts_with('#') {
        p.to_string()
    } else if let Some(stripped) = p.strip_prefix('/') {
        format!("./{stripped}")
    } else {
        format!("./{p}")
    }
}

fn resolve_file_or_index(target: &str, package_root: &Path) -> String {
    let rel = target.trim_start_matches("./");
    let on_disk = package_root.join(rel);
    if on_disk.is_file() {
        return normalize_rel(target);
    }
    if on_disk.is_dir() {
        if let Some(idx) = probe_index(&on_disk) {

            let idx_rel = idx.trim_start_matches("./");
            return normalize_rel(&format!("{}/{}", rel.trim_end_matches('/'), idx_rel));
        }
    }
    for ext in ["fts", "js", "mjs", "cjs"] {
        if package_root.join(format!("{rel}.{ext}")).is_file() {
            return normalize_rel(&format!("{rel}.{ext}"));
        }
    }
    normalize_rel(target)
}

fn probe_index(dir: &Path) -> Option<String> {
    if !dir.exists() {
        return Some("./index.js".to_string());
    }
    for ext in ["fts", "js", "mjs", "cjs"] {
        if dir.join(format!("index.{ext}")).is_file() {
            return Some(format!("./index.{ext}"));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusty_json_manifest::json;

    fn nodir() -> std::path::PathBuf {
        std::path::PathBuf::from("/nonexistent-pm-ext-16-fixture-root")
    }

    #[test]
    fn main_string() {
        let m = compile_module_map(&json!({"main": "lib/entry.js"}), &nodir());
        assert_eq!(m["."], "./lib/entry.js");
    }

    #[test]
    fn index_default() {
        let m = compile_module_map(&json!({"name": "x"}), &nodir());
        assert_eq!(m["."], "./index.js");
    }

    #[test]
    fn exports_string_sugar() {
        let m = compile_module_map(&json!({"exports": "./main.js"}), &nodir());
        assert_eq!(m["."], "./main.js");
    }

    #[test]
    fn exports_default_condition() {
        let m = compile_module_map(&json!({"exports": {"default": "./d.js"}}), &nodir());
        assert_eq!(m["."], "./d.js");
    }

    #[test]
    fn exports_import_condition_priority() {
        let m = compile_module_map(
            &json!({"exports": {"require": "./r.cjs", "import": "./i.mjs", "default": "./d.js"}}),
            &nodir(),
        );
        assert_eq!(
            m["."], "./i.mjs",
            "import beats require + default in ESM set"
        );
    }

    #[test]
    fn exports_require_condition_fallback() {
        let m = compile_module_map(&json!({"exports": {"require": "./r.cjs"}}), &nodir());
        assert_eq!(m["."], "./r.cjs", "require-only export resolves its target");
    }

    #[test]
    fn exports_subpath_map() {
        let m = compile_module_map(
            &json!({"exports": {".": "./index.js", "./feature": "./src/feature.js"}}),
            &nodir(),
        );
        assert_eq!(m["."], "./index.js");
        assert_eq!(m["./feature"], "./src/feature.js");
    }

    #[test]
    fn exports_subpath_conditional() {
        let m = compile_module_map(
            &json!({"exports": {"./x": {"import": "./x.mjs", "require": "./x.cjs"}}}),
            &nodir(),
        );
        assert_eq!(m["./x"], "./x.mjs");
    }

    #[test]
    fn exports_path_pattern() {
        let m = compile_module_map(&json!({"exports": {"./*": "./src/*.js"}}), &nodir());
        assert_eq!(
            m["./*"], "./src/*.js",
            "pattern target recorded for Rung C expansion"
        );
    }

    #[test]
    fn imports_subpath() {
        let m = compile_module_map(
            &json!({"imports": {"#internal": "./src/internal.js"}}),
            &nodir(),
        );
        assert_eq!(m["#internal"], "./src/internal.js");
    }

    #[test]
    fn imports_conditional() {
        let m = compile_module_map(
            &json!({"imports": {"#dep": {"node": "./node.js", "default": "./browser.js"}}}),
            &nodir(),
        );
        assert_eq!(m["#dep"], "./node.js", "node condition wins for #dep");
    }

    #[test]
    fn imports_non_hash_ignored() {
        let m = compile_module_map(
            &json!({"main": "./i.js", "imports": {"bad": "./x.js"}}),
            &nodir(),
        );
        assert!(!m.contains_key("bad"));
        assert_eq!(m["."], "./i.js");
    }

    #[test]
    fn exports_beats_main() {
        let m = compile_module_map(
            &json!({"main": "./old.js", "exports": "./new.js"}),
            &nodir(),
        );
        assert_eq!(m["."], "./new.js");
    }

    #[test]
    fn target_array_fallback() {
        let m = compile_module_map(
            &json!({"exports": {".": [{"unknown_cond": "./a.js"}, "./b.js"]}}),
            &nodir(),
        );
        assert_eq!(m["."], "./b.js", "array falls through to the string target");
    }

    #[test]
    fn normalization() {
        let m = compile_module_map(&json!({"main": "entry.js"}), &nodir());
        assert_eq!(m["."], "./entry.js", "bare path gains ./ prefix");
    }

    #[test]
    fn main_extensionless_prefers_fts_when_present() {
        let root = std::env::temp_dir().join(format!(
            "cruft-pm-module-map-fts-main-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/entry.fts"), "").unwrap();

        let m = compile_module_map(&json!({"main": "src/entry"}), &root);
        assert_eq!(m["."], "./src/entry.fts");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn index_probe_prefers_fts_when_present() {
        let root = std::env::temp_dir().join(format!(
            "cruft-pm-module-map-fts-index-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("index.fts"), "").unwrap();

        let m = compile_module_map(&json!({"name": "fts-index"}), &root);
        assert_eq!(m["."], "./index.fts");

        let _ = std::fs::remove_dir_all(&root);
    }
}
