
use rusty_js_runtime::{HostHook, Runtime, Value};

thread_local! {
    static PKG_CLASSIFY_MEMO: std::cell::RefCell<
        std::collections::HashMap<(u8, String), bool>,
    > = std::cell::RefCell::new(std::collections::HashMap::new());
}

fn pkg_classify_memo(tag: u8, url: &str, compute: impl FnOnce(&str) -> bool) -> bool {
    if let Some(hit) = PKG_CLASSIFY_MEMO.with(|m| m.borrow().get(&(tag, url.to_string())).copied())
    {
        return hit;
    }
    let v = compute(url);
    PKG_CLASSIFY_MEMO.with(|m| {
        m.borrow_mut().insert((tag, url.to_string()), v);
    });
    v
}

fn package_is_type_module(url: &str) -> bool {
    pkg_classify_memo(0, url, package_is_type_module_uncached)
}
fn package_is_type_module_uncached(url: &str) -> bool {
    let path_str = match url.strip_prefix("file://") {
        Some(p) => p,
        None => return false,
    };
    let path = std::path::Path::new(path_str);
    let mut cur = path.parent();
    while let Some(d) = cur {
        let candidate = d.join("package.json");
        if candidate.is_file() {
            if let Ok(text) = std::fs::read_to_string(&candidate) {
                let lower = text.replace(char::is_whitespace, "");
                return lower.contains("\"type\":\"module\"");
            }
        }
        cur = d.parent();
    }
    false
}

fn package_has_exports_field(url: &str) -> bool {
    pkg_classify_memo(1, url, package_has_exports_field_uncached)
}

fn package_exports_map_suppresses_tuple_a(url: &str) -> bool {
    package_has_exports_field(url)
}

fn package_has_exports_field_uncached(url: &str) -> bool {
    let path_str = match url.strip_prefix("file://") {
        Some(p) => p,
        None => return false,
    };
    let path = std::path::Path::new(path_str);
    let mut cur = path.parent();
    while let Some(d) = cur {
        let candidate = d.join("package.json");
        if candidate.is_file() {
            if let Ok(text) = std::fs::read_to_string(&candidate) {
                let compact = text.replace(char::is_whitespace, "");
                return compact.contains("\"exports\":");
            }
        }
        cur = d.parent();
    }
    false
}

fn is_js_under_non_type_module_package(url: &str) -> bool {
    pkg_classify_memo(2, url, is_js_under_non_type_module_package_uncached)
}
fn is_js_under_non_type_module_package_uncached(url: &str) -> bool {
    let path_str = match url.strip_prefix("file://") {
        Some(p) => p,
        None => return false,
    };
    let path = std::path::Path::new(path_str);
    if path.extension().and_then(|s| s.to_str()) != Some("js") {
        return false;
    }
    let mut cur = path.parent();
    while let Some(d) = cur {
        let candidate = d.join("package.json");
        if candidate.is_file() {
            if let Ok(text) = std::fs::read_to_string(&candidate) {

                let lower = text.replace(char::is_whitespace, "");

                if lower.contains("\"type\":\"module\"") {
                    return false;
                }

                return lower.contains("\"module\":");
            }
        }
        cur = d.parent();
    }
    false
}

pub fn install(rt: &mut Runtime) {

    rt.install_host_hook(HostHook::FinalizeModuleNamespace(Box::new(
        |rt, _ast, ns, url| {
            let (has_default, default_value, named_count): (bool, Value, usize) = {
                let o = rt.obj(ns);
                let has = o.has_own_str("default");
                let dv = o
                    .get_own("default")
                    .map(|d| d.value.clone())
                    .unwrap_or(Value::Undefined);
                let other = o
                    .properties
                    .keys()
                    .filter(|k| {
                        let key = k.as_str();
                        key != "default" && key != "@@toStringTag" && !key.starts_with("__")
                    })
                    .count();
                (has, dv, other)
            };

            let is_module_field_esm = is_js_under_non_type_module_package(url);
            let has_exports_field = package_exports_map_suppresses_tuple_a(url);

            if !has_default && named_count == 0 {
                if rt.graph_forced_esm_urls.contains(url) {
                    rt.module_ns_synth_trace.insert(
                        url.to_string(),
                        "ESM-finalize Tuple-A-empty-suppressed (graph-forced ESM)".to_string(),
                    );
                    return Ok(());
                }

                rt.module_ns_synth_trace.insert(
                    url.to_string(),
                    "ESM-finalize Tuple-A-empty-suppressed (Node oracle: no synthetic default)"
                        .to_string(),
                );
                return Ok(());
            }
            if !has_default && is_module_field_esm && !has_exports_field {

                rt.module_ns_synth_trace.insert(
                    url.to_string(),
                    "ESM-finalize Tuple-A-wide-suppressed (Node oracle: no synthetic default)"
                        .to_string(),
                );
                return Ok(());
            }
            if !has_default && is_module_field_esm && has_exports_field {
                rt.module_ns_synth_trace.insert(
                    url.to_string(),
                    "ESM-finalize Tuple-A-wide-suppressed (package exports map)".to_string(),
                );
                return Ok(());
            }

            let pkg_is_type_module = package_is_type_module(url);
            let mut synth_path = format!(
                "ESM-finalize pass-through (has_default={} named_count={} pkg_type_module={})",
                has_default, named_count, pkg_is_type_module,
            );

            let url_is_mjs = url.ends_with(".mjs");
            let suppress_lift_for_canonical_esm = url_is_mjs && has_exports_field;
            if has_default
                && named_count == 0
                && !pkg_is_type_module
                && !suppress_lift_for_canonical_esm
            {
                if let Value::Object(fn_id) = default_value {
                    use rusty_js_runtime::value::InternalKind;
                    let is_fn = matches!(
                        rt.obj(fn_id).internal_kind,
                        InternalKind::Function(_)
                            | InternalKind::Closure(_)
                            | InternalKind::BoundFunction(_)
                    );
                    if is_fn {
                        synth_path = format!(
                        "P53.E13 fn-lift applied (gates: default-only={} fn={} not-type-module={})",
                        named_count == 0, is_fn, !pkg_is_type_module,
                    );
                        for key in ["name", "length", "prototype"] {
                            let already = rt.obj(ns).has_own_str(key);
                            if !already {
                                let v = rt.object_get(fn_id, key);
                                if !matches!(v, Value::Undefined) {
                                    rt.object_set(ns, key.to_string(), v);
                                }
                            }
                        }
                    }
                }
            }
            let _ = named_count;
            rt.module_ns_synth_trace.insert(url.to_string(), synth_path);

            Ok(())
        },
    )));
}

#[cfg(test)]
mod tests {
    use super::{is_js_under_non_type_module_package, package_exports_map_suppresses_tuple_a};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_url(package_json: &str, rel: &str) -> String {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cruft-module-ns-test-{}-{}",
            std::process::id(),
            stamp
        ));
        let file = root.join(rel);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(root.join("package.json"), package_json).unwrap();
        fs::write(&file, "export const named = 1;\n").unwrap();
        format!("file://{}", file.display())
    }

    #[test]
    fn exports_map_suppresses_tuple_a_even_with_module_field() {
        let url = fixture_url(
            r#"{
              "name": "fixture",
              "main": "./dist/index.cjs.js",
              "module": "./dist/index.esm.js",
              "exports": { ".": { "import": "./dist/index.esm.js", "require": "./dist/index.cjs.js" } }
            }"#,
            "dist/index.esm.js",
        );
        assert!(is_js_under_non_type_module_package(&url));
        assert!(package_exports_map_suppresses_tuple_a(&url));
    }

    #[test]
    fn legacy_module_field_without_exports_keeps_tuple_a_eligible() {
        let url = fixture_url(
            r#"{
              "name": "fixture",
              "main": "./dist/index.cjs.js",
              "module": "./dist/index.esm.js"
            }"#,
            "dist/index.esm.js",
        );
        assert!(is_js_under_non_type_module_package(&url));
        assert!(!package_exports_map_suppresses_tuple_a(&url));
    }
}
