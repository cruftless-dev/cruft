
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Default, Clone)]
pub struct ModuleMapBridge {

    by_name: BTreeMap<String, (String, BTreeMap<String, String>)>,

    export_shape_by_name: BTreeMap<String, LockExportShape>,

    version: u32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LockExportShape {
    pub named: Vec<String>,
    pub module_exports_reassigned: bool,
    pub has_es_module_flag: bool,
}

impl LockExportShape {
    fn is_empty(&self) -> bool {
        self.named.is_empty() && !self.module_exports_reassigned && !self.has_es_module_flag
    }

    pub fn lower_node_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = Vec::with_capacity(self.named.len() + 3);
        let push_unique = |keys: &mut Vec<String>, k: String| {
            if !keys.iter().any(|e| e == &k) {
                keys.push(k);
            }
        };
        for n in &self.named {
            push_unique(&mut keys, n.clone());
        }
        push_unique(&mut keys, "default".to_string());
        push_unique(&mut keys, "module.exports".to_string());
        if self.has_es_module_flag {
            push_unique(&mut keys, "__esModule".to_string());
        }
        keys
    }
}

#[derive(Debug)]
pub enum BridgeError {
    Io(String),
    Json(String),
}

impl ModuleMapBridge {

    pub fn empty() -> Self {
        ModuleMapBridge {
            by_name: BTreeMap::new(),
            export_shape_by_name: BTreeMap::new(),
            version: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty() && self.export_shape_by_name.is_empty()
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn load(lockfile_path: &Path) -> Result<Self, BridgeError> {
        let bytes = std::fs::read(lockfile_path).map_err(|e| BridgeError::Io(format!("{e}")))?;
        let json: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|e| BridgeError::Json(format!("{e}")))?;
        Self::from_json(&json)
    }

    pub fn from_json(json: &serde_json::Value) -> Result<Self, BridgeError> {
        let version = json
            .get("version")
            .or_else(|| json.get("lockfileVersion"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let mut by_name = BTreeMap::new();
        let mut export_shape_by_name = BTreeMap::new();
        if let Some(packages) = json.get("packages").and_then(|p| p.as_object()) {
            for (key, pkg) in packages {

                let module_map = match pkg.get("module_map").and_then(|m| m.as_object()) {
                    Some(obj) => obj
                        .iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect::<BTreeMap<String, String>>(),
                    None => BTreeMap::new(),
                };

                let export_shape = parse_export_shape(pkg.get("export_shape"));

                if module_map.is_empty() && export_shape.is_empty() {
                    continue;
                }

                let (name, ver) = match (
                    pkg.get("name").and_then(|v| v.as_str()),
                    pkg.get("version").and_then(|v| v.as_str()),
                ) {
                    (Some(n), Some(v)) => (n.to_string(), v.to_string()),
                    _ => match key.rsplit_once('@') {
                        Some((n, v)) if !n.is_empty() => (n.to_string(), v.to_string()),
                        _ => continue,
                    },
                };
                if !export_shape.is_empty() {
                    export_shape_by_name.insert(name.clone(), export_shape);
                }

                if !module_map.is_empty() {
                    by_name.insert(name, (ver, module_map));
                }
            }
        }
        Ok(ModuleMapBridge {
            by_name,
            export_shape_by_name,
            version,
        })
    }

    pub fn package_for_path(&self, resolved_path: &str) -> Option<(String, String)> {
        let name = package_name_from_path(resolved_path)?;
        self.by_name
            .get(&name)
            .map(|(v, _)| (name.clone(), v.clone()))
    }

    pub fn lookup(&self, package_root: &str, subpath: &str) -> Option<String> {
        let name = package_name_from_path(package_root)?;
        let (_, map) = self.by_name.get(&name)?;
        let rel = map.get(subpath)?;
        Some(join_rel(package_root, rel))
    }

    pub fn export_shape_for_name(&self, name: &str) -> Option<&LockExportShape> {
        self.export_shape_by_name.get(name)
    }

    pub fn export_shape_for_path(&self, resolved_path: &str) -> Option<&LockExportShape> {
        let name = package_name_from_path(resolved_path)?;
        self.export_shape_by_name.get(&name)
    }
}

fn parse_export_shape(v: Option<&serde_json::Value>) -> LockExportShape {
    let obj = match v.and_then(|v| v.as_object()) {
        Some(o) => o,
        None => return LockExportShape::default(),
    };
    let named = obj
        .get("named")
        .and_then(|n| n.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    LockExportShape {
        named,
        module_exports_reassigned: obj
            .get("module_exports_reassigned")
            .and_then(|b| b.as_bool())
            .unwrap_or(false),
        has_es_module_flag: obj
            .get("has_es_module_flag")
            .and_then(|b| b.as_bool())
            .unwrap_or(false),
    }
}

fn package_name_from_path(path: &str) -> Option<String> {
    let marker = "node_modules/";
    let idx = path.rfind(marker)?;
    let rest = &path[idx + marker.len()..];
    let mut comps = rest.split('/').filter(|c| !c.is_empty());
    let first = comps.next()?;
    if let Some(stripped) = first.strip_prefix('@') {

        let _ = stripped;
        let second = comps.next()?;
        Some(format!("{first}/{second}"))
    } else {
        Some(first.to_string())
    }
}

fn join_rel(package_root: &str, rel: &str) -> String {
    let root = package_root.trim_end_matches('/');
    let rel = rel.strip_prefix("./").unwrap_or(rel);
    format!("{root}/{rel}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lockfile_json(entries: &[(&str, &str, &[(&str, &str)])]) -> serde_json::Value {
        let mut packages = serde_json::Map::new();
        for (name, version, mm) in entries {
            let mut pkg = serde_json::Map::new();
            pkg.insert("name".into(), serde_json::json!(name));
            pkg.insert("version".into(), serde_json::json!(version));
            pkg.insert("tarball_url".into(), serde_json::json!("https://x/t.tgz"));
            let mut map = serde_json::Map::new();
            for (k, v) in *mm {
                map.insert((*k).into(), serde_json::json!(v));
            }
            pkg.insert("module_map".into(), serde_json::Value::Object(map));
            packages.insert(format!("{name}@{version}"), serde_json::Value::Object(pkg));
        }
        serde_json::json!({ "version": 2, "packages": serde_json::Value::Object(packages) })
    }

    #[test]
    fn export_shape_mirror_and_lower() {

        let json = serde_json::json!({
            "version": 2,
            "packages": {
                "ajv@8.0.0": {
                    "name": "ajv", "version": "8.0.0", "tarball_url": "https://x/t.tgz",
                    "export_shape": {
                        "named": ["Ajv", "CodeGen", "KeywordCxt"],
                        "module_exports_reassigned": true,
                        "has_es_module_flag": true
                    }
                }
            }
        });
        let b = ModuleMapBridge::from_json(&json).unwrap();
        let shape = b.export_shape_for_name("ajv").expect("indexed");
        let mut keys = shape.lower_node_keys();
        keys.sort();
        assert_eq!(
            keys,
            vec![
                "Ajv".to_string(),
                "CodeGen".to_string(),
                "KeywordCxt".to_string(),
                "__esModule".to_string(),
                "default".to_string(),
                "module.exports".to_string(),
            ]
        );

        assert!(b
            .export_shape_for_path("/tmp/x/node_modules/ajv/dist/ajv.js")
            .is_some());

        assert!(b.export_shape_for_name("lodash").is_none());
    }

    #[test]
    fn probe1_positive_lookup() {
        let json = lockfile_json(&[
            ("lodash", "4.17.21", &[(".", "./lodash.js")]),
            ("ms", "2.1.3", &[(".", "./index.js")]),
            (
                "debug",
                "4.4.3",
                &[(".", "./src/index.js"), ("./browser", "./src/browser.js")],
            ),
        ]);
        let b = ModuleMapBridge::from_json(&json).unwrap();
        assert_eq!(b.version(), 2);
        assert_eq!(
            b.lookup("/proj/node_modules/lodash", ".").as_deref(),
            Some("/proj/node_modules/lodash/lodash.js")
        );
        assert_eq!(
            b.lookup("/proj/node_modules/debug", "./browser").as_deref(),
            Some("/proj/node_modules/debug/src/browser.js")
        );

        assert_eq!(
            b.package_for_path("/proj/node_modules/ms/index.js"),
            Some(("ms".to_string(), "2.1.3".to_string()))
        );
    }

    #[test]
    fn probe2_negative_absent_package() {
        let json = lockfile_json(&[("lodash", "4.17.21", &[(".", "./lodash.js")])]);
        let b = ModuleMapBridge::from_json(&json).unwrap();
        assert_eq!(b.lookup("/proj/node_modules/express", "."), None);
        assert_eq!(
            b.package_for_path("/proj/node_modules/express/index.js"),
            None
        );
    }

    #[test]
    fn probe3_edge_empty_module_map_skipped() {

        let json = lockfile_json(&[("emptypkg", "1.0.0", &[])]);
        let b = ModuleMapBridge::from_json(&json).unwrap();
        assert!(b.is_empty());
        assert_eq!(b.lookup("/proj/node_modules/emptypkg", "."), None);
    }

    #[test]
    fn probe4_round_trip_byte_stable_tuples() {

        let mm: &[(&str, &str)] = &[
            ("#internal", "./src/x.js"),
            (".", "./index.js"),
            ("./feature", "./src/feature.js"),
        ];
        let json = lockfile_json(&[("pkg", "3.2.1", mm)]);
        let b = ModuleMapBridge::from_json(&json).unwrap();
        let read: Vec<(String, String)> = b
            .by_name
            .get("pkg")
            .unwrap()
            .1
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let mut expected: Vec<(String, String)> = mm
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        expected.sort();
        assert_eq!(read, expected);
    }

    #[test]
    fn scoped_package_name_extraction() {
        let json = lockfile_json(&[("@babel/core", "7.0.0", &[(".", "./lib/index.js")])]);
        let b = ModuleMapBridge::from_json(&json).unwrap();
        assert_eq!(
            b.lookup("/proj/node_modules/@babel/core", ".").as_deref(),
            Some("/proj/node_modules/@babel/core/lib/index.js")
        );
        assert_eq!(
            b.package_for_path("/proj/node_modules/@babel/core/lib/index.js"),
            Some(("@babel/core".to_string(), "7.0.0".to_string()))
        );
    }

    #[test]
    fn v1_lockfile_no_module_maps() {

        let json = serde_json::json!({
            "version": 1,
            "packages": { "lodash@4.17.21": { "name": "lodash", "version": "4.17.21" } }
        });
        let b = ModuleMapBridge::from_json(&json).unwrap();
        assert_eq!(b.version(), 1);
        assert!(b.is_empty());
    }

    #[test]
    fn probe5_pure_esm_leaf_bridge_lookup() {

        let json = lockfile_json(&[
            (
                "chalk",
                "5.3.0",
                &[
                    ("#ansi-styles", "./source/vendor/ansi-styles/index.js"),
                    ("#supports-color", "./source/vendor/supports-color/index.js"),
                    (".", "./source/index.js"),
                ],
            ),
            ("uuid", "9.0.1", &[(".", "./wrapper.mjs")]),
        ]);
        let b = ModuleMapBridge::from_json(&json).unwrap();

        assert_eq!(
            b.lookup("/proj/node_modules/chalk", ".").as_deref(),
            Some("/proj/node_modules/chalk/source/index.js")
        );

        assert_eq!(
            b.lookup("/proj/node_modules/uuid", ".").as_deref(),
            Some("/proj/node_modules/uuid/wrapper.mjs")
        );

        assert_eq!(
            b.lookup("/proj/node_modules/chalk", "#ansi-styles")
                .as_deref(),
            Some("/proj/node_modules/chalk/source/vendor/ansi-styles/index.js")
        );

        assert_eq!(
            b.package_for_path("/proj/node_modules/chalk/source/index.js"),
            Some(("chalk".to_string(), "5.3.0".to_string()))
        );
    }
}
