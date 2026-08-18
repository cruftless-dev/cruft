
use std::collections::BTreeMap;
use std::path::Path;

use crate::export_shape::StaticExportShape;
use crate::module_map::ModuleMap;
use crate::resolver::{Caps, Placement, ResolvedDep};
use rusty_json_manifest::{Map, Value};

pub const LOCKFILE_NAME: &str = "cruft-lock.json";

pub const LEGACY_LOCKFILE_NAME: &str = "cruftless-lock.json";

pub const LOCKFILE_VERSION: u32 = 2;

pub const LOCKFILE_VERSION_V1: u32 = 1;

pub const CRUFT_PM_VERSION: &str = "0.2.0";

pub type CapsGrant = BTreeMap<String, Value>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LockfileMetadata {
    pub cruft_pm_version: String,
    pub registry: String,

    pub resolved_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LockPackage {
    pub dep: ResolvedDep,

    pub compartment_id: String,

    pub caps_grant: CapsGrant,

    pub module_map: ModuleMap,

    pub export_shape: StaticExportShape,

    pub placements: Vec<String>,

    pub auto_peers: Vec<String>,

    pub module_integrity: BTreeMap<String, String>,
}

#[derive(Debug, PartialEq)]
pub struct Lockfile {
    pub version: u32,
    pub metadata: LockfileMetadata,
    pub packages: BTreeMap<String, LockPackage>,
}

#[derive(Debug)]
pub enum LockfileError {
    Io(String),
    Json(String),
    UnsupportedVersion(u32),
}

fn string_value(s: impl Into<String>) -> Value {
    Value::String(s.into())
}

fn string_array(values: &[String]) -> Value {
    Value::Array(values.iter().cloned().map(Value::String).collect())
}

fn string_map_value(map: &BTreeMap<String, String>) -> Value {
    let mut object = Map::new();
    for (key, value) in map {
        object.insert(key.clone(), Value::String(value.clone()));
    }
    Value::Object(object)
}

fn string_set_value(set: &std::collections::BTreeSet<String>) -> Value {
    Value::Array(set.iter().cloned().map(Value::String).collect())
}

fn module_map_value(map: &ModuleMap) -> Value {
    string_map_value(map)
}

fn caps_grant_value(grant: &CapsGrant) -> Value {
    let mut object = Map::new();
    for (key, value) in grant {
        object.insert(key.clone(), value.clone());
    }
    Value::Object(object)
}

fn resolved_dep_value(dep: &ResolvedDep, object: &mut Map) {
    object.insert("name".to_string(), string_value(&dep.name));
    object.insert("version".to_string(), string_value(&dep.version));
    object.insert("tarball_url".to_string(), string_value(&dep.tarball_url));
    object.insert(
        "integrity".to_string(),
        dep.integrity
            .as_ref()
            .map(|s| string_value(s))
            .unwrap_or(Value::Null),
    );
    object.insert(
        "shasum".to_string(),
        dep.shasum
            .as_ref()
            .map(|s| string_value(s))
            .unwrap_or(Value::Null),
    );
    if !dep.dependencies.is_empty() {
        object.insert(
            "dependencies".to_string(),
            string_map_value(&dep.dependencies),
        );
    }
    if !dep.optional_dependencies.is_empty() {
        object.insert(
            "optional_dependencies".to_string(),
            string_map_value(&dep.optional_dependencies),
        );
    }
    if !dep.os.is_empty() {
        object.insert("os".to_string(), string_array(&dep.os));
    }
    if !dep.cpu.is_empty() {
        object.insert("cpu".to_string(), string_array(&dep.cpu));
    }
    if !dep.peer_dependencies.is_empty() {
        object.insert(
            "peer_dependencies".to_string(),
            string_map_value(&dep.peer_dependencies),
        );
    }
    if !dep.optional_peers.is_empty() {
        object.insert(
            "optional_peers".to_string(),
            string_set_value(&dep.optional_peers),
        );
    }

    if !dep.caps.is_empty() {
        object.insert("caps".to_string(), caps_value(&dep.caps));
    }

    if let Some(publisher) = &dep.publisher {
        object.insert("publisher".to_string(), string_value(publisher));
    }
}

fn caps_value(caps: &Caps) -> Value {
    let mut object = Map::new();
    if !caps.net.is_empty() {
        object.insert("net".to_string(), string_array(&caps.net));
    }
    if !caps.fs.is_empty() {
        object.insert("fs".to_string(), string_array(&caps.fs));
    }
    if !caps.env.is_empty() {
        object.insert("env".to_string(), string_array(&caps.env));
    }
    if !caps.exec.is_empty() {
        object.insert("exec".to_string(), string_array(&caps.exec));
    }
    Value::Object(object)
}

fn caps_field(object: &Map, key: &str) -> Caps {
    let mut caps = Caps::default();
    let Some(Value::Object(map)) = object.get(key) else {
        return caps;
    };
    let read_class = |k: &str| -> Vec<String> {
        map.get(k)
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default()
    };
    caps.net = read_class("net");
    caps.fs = read_class("fs");
    caps.env = read_class("env");
    caps.exec = read_class("exec");
    caps
}

fn lock_metadata_value(metadata: &LockfileMetadata) -> Value {
    let mut object = Map::new();
    if !metadata.cruft_pm_version.is_empty() {
        object.insert(
            "cruft_pm_version".to_string(),
            string_value(&metadata.cruft_pm_version),
        );
    }
    if !metadata.registry.is_empty() {
        object.insert("registry".to_string(), string_value(&metadata.registry));
    }
    if !metadata.resolved_at.is_empty() {
        object.insert(
            "resolved_at".to_string(),
            string_value(&metadata.resolved_at),
        );
    }
    Value::Object(object)
}

fn lock_package_value(package: &LockPackage) -> Value {
    let mut object = Map::new();
    resolved_dep_value(&package.dep, &mut object);
    if !package.compartment_id.is_empty() {
        object.insert(
            "compartment_id".to_string(),
            string_value(&package.compartment_id),
        );
    }
    if !package.caps_grant.is_empty() {
        object.insert(
            "caps_grant".to_string(),
            caps_grant_value(&package.caps_grant),
        );
    }
    if !package.module_map.is_empty() {
        object.insert(
            "module_map".to_string(),
            module_map_value(&package.module_map),
        );
    }
    if !package.export_shape.is_empty() {
        object.insert(
            "export_shape".to_string(),
            package.export_shape.to_json_value(),
        );
    }
    if !package.placements.is_empty() {
        object.insert("placements".to_string(), string_array(&package.placements));
    }
    if !package.auto_peers.is_empty() {
        object.insert("auto_peers".to_string(), string_array(&package.auto_peers));
    }

    if !package.module_integrity.is_empty() {
        object.insert(
            "module_integrity".to_string(),
            string_map_value(&package.module_integrity),
        );
    }
    Value::Object(object)
}

fn lockfile_value(lock: &Lockfile) -> Value {
    let mut object = Map::new();
    object.insert(
        "version".to_string(),
        rusty_json_manifest::to_value(lock.version),
    );
    object.insert("metadata".to_string(), lock_metadata_value(&lock.metadata));
    let mut packages = Map::new();
    for (key, package) in &lock.packages {
        packages.insert(key.clone(), lock_package_value(package));
    }
    object.insert("packages".to_string(), Value::Object(packages));
    Value::Object(object)
}

fn as_object<'a>(value: &'a Value, ctx: &str) -> Result<&'a Map, LockfileError> {
    value
        .as_object()
        .ok_or_else(|| LockfileError::Json(format!("{ctx} must be an object")))
}

fn string_field(object: &Map, key: &str) -> Option<String> {
    object.get(key).and_then(Value::as_str).map(String::from)
}

fn required_string(object: &Map, key: &'static str) -> Result<String, LockfileError> {
    string_field(object, key)
        .ok_or_else(|| LockfileError::Json(format!("lockfile package missing string `{key}`")))
}

fn optional_string(object: &Map, key: &str) -> Option<String> {
    match object.get(key) {
        Some(Value::String(s)) => Some(s.clone()),
        Some(Value::Null) | None => None,
        _ => None,
    }
}

fn string_map_field(object: &Map, key: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if let Some(Value::Object(map)) = object.get(key) {
        for (k, v) in map {
            if let Some(s) = v.as_str() {
                out.insert(k.clone(), s.to_string());
            }
        }
    }
    out
}

fn string_vec_field(object: &Map, key: &str) -> Vec<String> {
    object
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

fn string_set_field(object: &Map, key: &str) -> std::collections::BTreeSet<String> {
    string_vec_field(object, key).into_iter().collect()
}

fn caps_grant_field(object: &Map, key: &str) -> CapsGrant {
    let mut out = CapsGrant::new();
    if let Some(Value::Object(map)) = object.get(key) {
        for (k, v) in map {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

fn resolved_dep_from_object(object: &Map) -> Result<ResolvedDep, LockfileError> {
    Ok(ResolvedDep {
        name: required_string(object, "name")?,
        version: required_string(object, "version")?,
        tarball_url: required_string(object, "tarball_url")?,
        integrity: optional_string(object, "integrity"),
        shasum: optional_string(object, "shasum"),
        dependencies: string_map_field(object, "dependencies"),
        optional_dependencies: string_map_field(object, "optional_dependencies"),
        os: string_vec_field(object, "os"),
        cpu: string_vec_field(object, "cpu"),
        peer_dependencies: string_map_field(object, "peer_dependencies"),
        optional_peers: string_set_field(object, "optional_peers"),

        caps: caps_field(object, "caps"),
        publisher: optional_string(object, "publisher"),
    })
}

fn metadata_from_value(value: Option<&Value>) -> LockfileMetadata {
    let Some(Value::Object(object)) = value else {
        return LockfileMetadata::default();
    };
    LockfileMetadata {
        cruft_pm_version: string_field(object, "cruft_pm_version").unwrap_or_default(),
        registry: string_field(object, "registry").unwrap_or_default(),
        resolved_at: string_field(object, "resolved_at").unwrap_or_default(),
    }
}

fn lock_package_from_value(value: &Value, key: &str) -> Result<LockPackage, LockfileError> {
    let object = as_object(value, "lockfile package")?;
    Ok(LockPackage {
        dep: resolved_dep_from_object(object)?,
        compartment_id: string_field(object, "compartment_id").unwrap_or_else(|| key.to_string()),
        caps_grant: caps_grant_field(object, "caps_grant"),
        module_map: string_map_field(object, "module_map"),
        export_shape: object
            .get("export_shape")
            .map(StaticExportShape::from_json_value)
            .unwrap_or_default(),
        placements: string_vec_field(object, "placements"),
        auto_peers: string_vec_field(object, "auto_peers"),

        module_integrity: string_map_field(object, "module_integrity"),
    })
}

fn lockfile_from_value(value: Value) -> Result<Lockfile, LockfileError> {
    let object = as_object(&value, "lockfile")?;
    let version = object
        .get("version")
        .or_else(|| object.get("lockfileVersion"))
        .and_then(Value::as_u64)
        .ok_or_else(|| LockfileError::Json("lockfile missing numeric version".to_string()))?
        as u32;
    if version != LOCKFILE_VERSION && version != LOCKFILE_VERSION_V1 {
        return Err(LockfileError::UnsupportedVersion(version));
    }

    let packages_obj = object
        .get("packages")
        .ok_or_else(|| LockfileError::Json("lockfile missing packages".to_string()))
        .and_then(|v| as_object(v, "lockfile packages"))?;
    let mut packages = BTreeMap::new();
    for (key, package_value) in packages_obj {
        packages.insert(key.clone(), lock_package_from_value(package_value, key)?);
    }

    Ok(Lockfile {
        version: LOCKFILE_VERSION,
        metadata: metadata_from_value(object.get("metadata")),
        packages,
    })
}

impl Lockfile {
    pub fn new() -> Self {
        Self {
            version: LOCKFILE_VERSION,
            metadata: LockfileMetadata {
                cruft_pm_version: CRUFT_PM_VERSION.to_string(),
                registry: String::new(),
                resolved_at: String::new(),
            },
            packages: BTreeMap::new(),
        }
    }

    pub fn from_resolved(deps: impl IntoIterator<Item = ResolvedDep>) -> Self {
        let mut lock = Self::new();
        for dep in deps {
            lock.insert(dep);
        }
        lock
    }

    pub fn insert(&mut self, dep: ResolvedDep) {
        let key = format!("{}@{}", dep.name, dep.version);

        let placements = self
            .packages
            .get(&key)
            .map(|e| e.placements.clone())
            .unwrap_or_default();

        let auto_peers = self
            .packages
            .get(&key)
            .map(|e| e.auto_peers.clone())
            .unwrap_or_default();

        let module_integrity = self
            .packages
            .get(&key)
            .map(|e| e.module_integrity.clone())
            .unwrap_or_default();
        let entry = LockPackage {
            compartment_id: key.clone(),
            caps_grant: CapsGrant::new(),
            module_map: ModuleMap::new(),
            export_shape: StaticExportShape::default(),
            placements,
            auto_peers,
            module_integrity,
            dep,
        };
        self.packages.insert(key, entry);
    }

    pub fn set_placements(&mut self, name: &str, version: &str, mut places: Vec<String>) {
        places.sort();
        places.dedup();
        if let Some(entry) = self.packages.get_mut(&format!("{name}@{version}")) {
            entry.placements = places;
        }
    }

    pub fn set_auto_peers(&mut self, name: &str, version: &str, mut peers: Vec<String>) {
        peers.sort();
        peers.dedup();
        if let Some(entry) = self.packages.get_mut(&format!("{name}@{version}")) {
            entry.auto_peers = peers;
        }
    }

    pub fn set_module_map(&mut self, name: &str, version: &str, map: ModuleMap) {
        if let Some(entry) = self.packages.get_mut(&format!("{name}@{version}")) {
            entry.module_map = map;
        }
    }

    pub fn set_export_shape(&mut self, name: &str, version: &str, shape: StaticExportShape) {
        if let Some(entry) = self.packages.get_mut(&format!("{name}@{version}")) {
            entry.export_shape = shape;
        }
    }

    pub fn set_module_integrity(
        &mut self,
        name: &str,
        version: &str,
        map: BTreeMap<String, String>,
    ) {
        if let Some(entry) = self.packages.get_mut(&format!("{name}@{version}")) {
            entry.module_integrity = map;
        }
    }

    pub fn get(&self, name: &str, version: &str) -> Option<&ResolvedDep> {
        self.packages
            .get(&format!("{name}@{version}"))
            .map(|entry| &entry.dep)
    }

    pub fn get_package(&self, name: &str, version: &str) -> Option<&LockPackage> {
        self.packages.get(&format!("{name}@{version}"))
    }

    pub fn frozen_closure(&self) -> Vec<Placement> {
        let mut out = Vec::new();
        for p in self.packages.values() {
            if p.placements.is_empty() {

                out.push(Placement {
                    dep: p.dep.clone(),
                    nest_under: Vec::new(),
                });
            } else {
                for loc in &p.placements {

                    out.push(Placement {
                        dep: p.dep.clone(),
                        nest_under: if loc.is_empty() {
                            Vec::new()
                        } else {
                            loc.split('/').map(String::from).collect()
                        },
                    });
                }
            }
        }
        out
    }

    pub fn covers(&self, deps: &[(String, String)]) -> bool {
        deps.iter().all(|(name, range)| {
            self.packages.values().any(|p| {
                p.dep.name == *name
                    && (p.dep.version == *range
                        || crate::semver::satisfies(range, &p.dep.version).unwrap_or(false))
            })
        })
    }

    pub fn write_to(&self, path: &Path) -> Result<(), LockfileError> {
        let json = lockfile_value(self).to_pretty_string();

        let bytes = format!("{json}\n");
        std::fs::write(path, bytes).map_err(|e| LockfileError::Io(format!("write {path:?}: {e}")))
    }

    pub fn read_from(path: &Path) -> Result<Self, LockfileError> {
        let bytes =
            std::fs::read(path).map_err(|e| LockfileError::Io(format!("read {path:?}: {e}")))?;
        let value = rusty_json_manifest::from_slice::<Value>(&bytes)
            .map_err(|e| LockfileError::Json(format!("{e}")))?;
        lockfile_from_value(value)
    }
}

impl Default for Lockfile {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_dep(name: &str, version: &str) -> ResolvedDep {
        ResolvedDep {
            name: name.into(),
            version: version.into(),
            tarball_url: format!("https://cdn.example/{name}-{version}.tgz"),
            integrity: Some(format!("sha512-{name}{version}=")),
            shasum: None,
            dependencies: Default::default(),
            optional_dependencies: Default::default(),
            os: Default::default(),
            cpu: Default::default(),
            peer_dependencies: Default::default(),
            optional_peers: Default::default(),
            caps: Default::default(),
            publisher: None,
        }
    }

    #[test]
    fn roundtrip_empty() {
        let lock = Lockfile::new();
        let path = std::env::temp_dir().join(format!(
            "lock-empty-{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        lock.write_to(&path).unwrap();
        let back = Lockfile::read_from(&path).unwrap();
        assert_eq!(lock, back);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn roundtrip_two_deps_stable_order() {
        let mut lock = Lockfile::new();

        lock.insert(sample_dep("lodash", "4.17.21"));
        lock.insert(sample_dep("@babel/core", "7.24.0"));

        let path = std::env::temp_dir().join(format!(
            "lock-stable-{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        lock.write_to(&path).unwrap();
        let serialized = std::fs::read_to_string(&path).unwrap();
        let babel_pos = serialized.find("@babel/core").unwrap();
        let lodash_pos = serialized.find("lodash").unwrap();
        assert!(
            babel_pos < lodash_pos,
            "BTreeMap should sort @babel before lodash"
        );

        let back = Lockfile::read_from(&path).unwrap();
        assert_eq!(lock, back);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn platform_and_optional_dependency_fields_roundtrip() {
        let mut lock = Lockfile::new();
        let mut dep = sample_dep("native-wrapper", "1.0.0");
        dep.optional_dependencies
            .insert("native-linux".into(), "1.0.0".into());
        dep.os = vec!["linux".into(), "!darwin".into()];
        dep.cpu = vec!["x64".into()];
        lock.insert(dep);
        let path = tmp("platform-fields");
        lock.write_to(&path).unwrap();
        let back = Lockfile::read_from(&path).unwrap();
        let dep = &back.get_package("native-wrapper", "1.0.0").unwrap().dep;
        assert_eq!(dep.optional_dependencies["native-linux"], "1.0.0");
        assert_eq!(dep.os, vec!["linux", "!darwin"]);
        assert_eq!(dep.cpu, vec!["x64"]);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn auto_peers_frozen_replay_roundtrip() {

        let mut lock = Lockfile::new();
        lock.insert(sample_dep("react-dom", "18.3.1"));
        lock.insert(sample_dep("react", "18.3.1"));
        lock.set_placements("react-dom", "18.3.1", vec![String::new()]);
        lock.set_placements("react", "18.3.1", vec![String::new()]);
        lock.set_auto_peers("react-dom", "18.3.1", vec!["react".into()]);

        let path = std::env::temp_dir().join(format!(
            "lock-autopeers-{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        lock.write_to(&path).unwrap();
        let back = Lockfile::read_from(&path).unwrap();
        assert_eq!(lock, back, "auto_peers must round-trip identically");
        assert_eq!(
            back.packages["react-dom@18.3.1"].auto_peers,
            vec!["react".to_string()],
            "react-dom records react as an auto-installed peer"
        );
        assert!(
            back.packages["react@18.3.1"].auto_peers.is_empty(),
            "the peer itself records no auto_peers (skip-if-empty omits the field)"
        );

        let closure = back.frozen_closure();
        assert!(
            closure
                .iter()
                .any(|p| p.dep.name == "react" && p.nest_under.is_empty()),
            "frozen closure includes the hoisted auto-installed react"
        );

        let mut replayed = back;
        replayed.insert(sample_dep("react-dom", "18.3.1"));
        assert_eq!(
            replayed.packages["react-dom@18.3.1"].auto_peers,
            vec!["react".to_string()],
            "re-insert must preserve auto_peers (like placements)"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn get_by_name_version() {
        let mut lock = Lockfile::new();
        lock.insert(sample_dep("lodash", "4.17.21"));
        assert!(lock.get("lodash", "4.17.21").is_some());
        assert!(lock.get("lodash", "4.17.22").is_none());
        assert!(lock.get("underscore", "4.17.21").is_none());
    }

    #[test]
    fn rejects_unsupported_version() {
        let path = std::env::temp_dir().join(format!(
            "lock-badver-{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, br#"{"lockfileVersion":999,"packages":{}}"#).unwrap();
        let r = Lockfile::read_from(&path);
        assert!(matches!(r, Err(LockfileError::UnsupportedVersion(999))));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn byte_stable_across_runs() {

        let mut a = Lockfile::new();
        a.insert(sample_dep("z-pkg", "1.0.0"));
        a.insert(sample_dep("a-pkg", "1.0.0"));
        let mut b = Lockfile::new();
        b.insert(sample_dep("a-pkg", "1.0.0"));
        b.insert(sample_dep("z-pkg", "1.0.0"));

        let pa = std::env::temp_dir().join("lock-stable-a.json");
        let pb = std::env::temp_dir().join("lock-stable-b.json");
        a.write_to(&pa).unwrap();
        b.write_to(&pb).unwrap();
        let sa = std::fs::read_to_string(&pa).unwrap();
        let sb = std::fs::read_to_string(&pb).unwrap();
        assert_eq!(
            sa, sb,
            "lockfile must be byte-stable across insertion orders"
        );
        let _ = std::fs::remove_file(&pa);
        let _ = std::fs::remove_file(&pb);
    }

    fn tmp(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "lockv2-{tag}-{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn v1_reads_as_v2_default_fill() {
        let path = tmp("v1read");
        std::fs::write(
            &path,
            br#"{"lockfileVersion":1,"packages":{"lodash@4.17.21":{"name":"lodash","version":"4.17.21","tarball_url":"https://cdn/lodash-4.17.21.tgz","integrity":"sha512-x=","shasum":null}}}"#,
        )
        .unwrap();
        let lock = Lockfile::read_from(&path).unwrap();
        assert_eq!(
            lock.version, LOCKFILE_VERSION,
            "v1 normalizes to v2 in memory"
        );
        let entry = lock.get_package("lodash", "4.17.21").unwrap();
        assert_eq!(
            entry.compartment_id, "lodash@4.17.21",
            "default compartment_id"
        );
        assert!(entry.caps_grant.is_empty(), "default-fill empty caps_grant");
        assert_eq!(entry.dep.tarball_url, "https://cdn/lodash-4.17.21.tgz");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn v1_writeback_migrates_to_v2() {
        let path = tmp("v1mig");
        std::fs::write(
            &path,
            br#"{"lockfileVersion":1,"packages":{"ms@2.1.2":{"name":"ms","version":"2.1.2","tarball_url":"https://cdn/ms.tgz","integrity":"sha512-m=","shasum":null}}}"#,
        )
        .unwrap();
        let lock = Lockfile::read_from(&path).unwrap();
        let out = tmp("v1migout");
        lock.write_to(&out).unwrap();
        let s = std::fs::read_to_string(&out).unwrap();
        assert!(s.contains("\"version\": 2"), "emits v2 version key");
        assert!(
            !s.contains("lockfileVersion"),
            "no legacy version key on write"
        );
        assert!(
            s.contains("\"compartment_id\": \"ms@2.1.2\""),
            "compartment column present"
        );
        let re = Lockfile::read_from(&out).unwrap();
        assert_eq!(re.version, 2);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&out);
    }

    #[test]
    fn v2_roundtrip_equal() {
        let mut lock = Lockfile::new();
        lock.insert(sample_dep("chalk", "5.3.0"));
        lock.insert(sample_dep("uuid", "9.0.1"));
        let path = tmp("v2rt");
        lock.write_to(&path).unwrap();
        let back = Lockfile::read_from(&path).unwrap();
        assert_eq!(lock, back);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn v2_metadata_version_emitted() {
        let lock = Lockfile::new();
        let path = tmp("v2meta");
        lock.write_to(&path).unwrap();
        let s = std::fs::read_to_string(&path).unwrap();
        assert!(s.contains("\"metadata\""), "metadata block present");
        assert!(s.contains(&format!("\"cruft_pm_version\": \"{CRUFT_PM_VERSION}\"")));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn v2_empty_columns_skipped() {
        let mut lock = Lockfile::new();
        lock.insert(sample_dep("ms", "2.1.2"));
        let path = tmp("v2skip");
        lock.write_to(&path).unwrap();
        let s = std::fs::read_to_string(&path).unwrap();
        assert!(!s.contains("caps_grant"), "empty caps_grant skipped");
        assert!(
            !s.contains("resolved_at"),
            "empty resolved_at skipped (byte-stability)"
        );
        assert!(
            s.contains("compartment_id"),
            "non-empty compartment_id emitted"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn v2_compartment_id_defaults_to_key() {
        let mut lock = Lockfile::new();
        lock.insert(sample_dep("@babel/core", "7.24.0"));
        let e = lock.get_package("@babel/core", "7.24.0").unwrap();
        assert_eq!(e.compartment_id, "@babel/core@7.24.0");
    }

    #[test]
    fn v2_explicit_caps_grant_roundtrip() {
        let mut lock = Lockfile::new();
        lock.insert(sample_dep("debug", "4.3.4"));
        {
            let e = lock.packages.get_mut("debug@4.3.4").unwrap();
            e.caps_grant
                .insert("env".to_string(), rusty_json_manifest::json!(["DEBUG"]));
        }
        let path = tmp("v2caps");
        lock.write_to(&path).unwrap();
        let s = std::fs::read_to_string(&path).unwrap();
        assert!(s.contains("caps_grant"), "non-empty caps_grant emitted");
        let back = Lockfile::read_from(&path).unwrap();
        assert_eq!(lock, back, "explicit caps_grant round-trips");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn caps_and_publisher_roundtrip() {
        let mut lock = Lockfile::new();
        let mut secure = sample_dep("secure-pkg", "2.0.0");
        secure.caps = Caps {
            net: vec!["registry.npmjs.org".into()],
            fs: vec!["./cache".into()],
            env: vec![],
            exec: vec!["git".into()],
        };
        secure.publisher = Some("alice".into());
        lock.insert(secure);

        let mut pubonly = sample_dep("pub-pkg", "1.0.0");
        pubonly.publisher = Some("bob".into());
        lock.insert(pubonly);

        let path = tmp("capspub");
        lock.write_to(&path).unwrap();
        let s = std::fs::read_to_string(&path).unwrap();
        assert!(s.contains("\"caps\""), "non-empty caps serialized");
        assert!(
            s.contains("\"publisher\": \"alice\""),
            "publisher serialized"
        );
        assert!(s.contains("\"publisher\": \"bob\""));

        let back = Lockfile::read_from(&path).unwrap();
        assert_eq!(lock, back, "caps + publisher round-trip identically");
        let secure_back = &back.packages["secure-pkg@2.0.0"].dep;
        assert_eq!(secure_back.caps.net, vec!["registry.npmjs.org".to_string()]);
        assert_eq!(secure_back.caps.exec, vec!["git".to_string()]);
        assert!(secure_back.caps.env.is_empty());
        assert_eq!(secure_back.publisher, Some("alice".to_string()));
        assert!(
            back.packages["pub-pkg@1.0.0"].dep.caps.is_empty(),
            "empty caps stays empty across round-trip"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn legacy_without_caps_or_publisher_loads() {
        let path = tmp("legacynocaps");
        std::fs::write(
            &path,
            br#"{"lockfileVersion":1,"packages":{"lodash@4.17.21":{"name":"lodash","version":"4.17.21","tarball_url":"https://cdn/lodash-4.17.21.tgz","integrity":"sha512-x=","shasum":null}}}"#,
        )
        .unwrap();
        let lock = Lockfile::read_from(&path).unwrap();
        let dep = &lock.get_package("lodash", "4.17.21").unwrap().dep;
        assert!(dep.caps.is_empty(), "missing caps ⇒ empty");
        assert_eq!(dep.publisher, None, "missing publisher ⇒ None");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn module_integrity_roundtrip() {
        let mut lock = Lockfile::new();
        lock.insert(sample_dep("with-src", "1.0.0"));
        lock.insert(sample_dep("no-src", "1.0.0"));
        let mut mi = BTreeMap::new();
        mi.insert("index.js".to_string(), "sha512-AAAA".to_string());
        mi.insert("lib/util.js".to_string(), "sha512-BBBB".to_string());
        lock.set_module_integrity("with-src", "1.0.0", mi.clone());

        let path = tmp("modintegrity");
        lock.write_to(&path).unwrap();
        let s = std::fs::read_to_string(&path).unwrap();
        assert!(s.contains("\"module_integrity\""), "map serialized");
        assert!(s.contains("\"lib/util.js\""), "nested path serialized");

        let back = Lockfile::read_from(&path).unwrap();
        assert_eq!(lock, back, "module_integrity round-trips identically");
        assert_eq!(
            back.packages["with-src@1.0.0"].module_integrity, mi,
            "map preserved verbatim"
        );
        assert!(
            back.packages["no-src@1.0.0"].module_integrity.is_empty(),
            "empty map stays empty (omitted) across round-trip"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn legacy_without_module_integrity_loads() {
        let path = tmp("legacynomi");
        std::fs::write(
            &path,
            br#"{"lockfileVersion":1,"packages":{"lodash@4.17.21":{"name":"lodash","version":"4.17.21","tarball_url":"https://cdn/lodash-4.17.21.tgz","integrity":"sha512-x=","shasum":null}}}"#,
        )
        .unwrap();
        let lock = Lockfile::read_from(&path).unwrap();
        assert!(
            lock.get_package("lodash", "4.17.21")
                .unwrap()
                .module_integrity
                .is_empty(),
            "missing module_integrity ⇒ empty"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn v2_byte_stable_across_runs() {
        let mut a = Lockfile::new();
        a.insert(sample_dep("z-pkg", "1.0.0"));
        a.insert(sample_dep("a-pkg", "1.0.0"));
        let mut b = Lockfile::new();
        b.insert(sample_dep("a-pkg", "1.0.0"));
        b.insert(sample_dep("z-pkg", "1.0.0"));
        let pa = tmp("v2sa");
        let pb = tmp("v2sb");
        a.write_to(&pa).unwrap();
        b.write_to(&pb).unwrap();
        assert_eq!(
            std::fs::read_to_string(&pa).unwrap(),
            std::fs::read_to_string(&pb).unwrap(),
            "v2 byte-stable across insertion orders"
        );
        let _ = std::fs::remove_file(&pa);
        let _ = std::fs::remove_file(&pb);
    }

    #[test]
    fn v2_rejects_unsupported_version() {
        let path = tmp("v2bad");
        std::fs::write(&path, br#"{"version":7,"packages":{}}"#).unwrap();
        assert!(matches!(
            Lockfile::read_from(&path),
            Err(LockfileError::UnsupportedVersion(7))
        ));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn v2_missing_compartment_id_default_filled() {
        let path = tmp("v2fill");
        std::fs::write(
            &path,
            br#"{"version":2,"metadata":{"cruft_pm_version":"0.2.0"},"packages":{"uuid@9.0.1":{"name":"uuid","version":"9.0.1","tarball_url":"https://cdn/uuid.tgz","integrity":"sha512-u=","shasum":null}}}"#,
        )
        .unwrap();
        let lock = Lockfile::read_from(&path).unwrap();
        assert_eq!(
            lock.get_package("uuid", "9.0.1").unwrap().compartment_id,
            "uuid@9.0.1"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn corpus_v2_roundtrip_stable() {
        let corpus = [
            ("lodash", "4.17.21"),
            ("ms", "2.1.2"),
            ("chalk", "5.3.0"),
            ("uuid", "9.0.1"),
            ("debug", "4.3.4"),
            ("semver", "7.6.0"),
            ("@babel/core", "7.24.0"),
            ("react", "18.2.0"),
        ];
        let mut lock = Lockfile::new();
        for (n, v) in corpus.iter() {
            lock.insert(sample_dep(n, v));
        }
        let path = tmp("corpus");
        lock.write_to(&path).unwrap();
        let s = std::fs::read_to_string(&path).unwrap();

        for (n, v) in corpus.iter() {
            assert!(
                s.contains(&format!("\"compartment_id\": \"{n}@{v}\"")),
                "missing compartment_id for {n}@{v}"
            );
        }

        let back = Lockfile::read_from(&path).unwrap();
        assert_eq!(lock, back);

        let path2 = tmp("corpus2");
        back.write_to(&path2).unwrap();
        assert_eq!(s, std::fs::read_to_string(&path2).unwrap());
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&path2);
    }
}
