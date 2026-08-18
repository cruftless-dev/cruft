
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::export_shape::StaticExportShape;
use crate::fetcher::{fetch_into_store, FetchError};
use crate::linker::{materialize_from_store, LinkError, MaterializeMode};
use crate::lockfile::{Lockfile, LockfileError, LEGACY_LOCKFILE_NAME, LOCKFILE_NAME};
use crate::module_map::ModuleMap;
use crate::resolver::{
    resolve_closure_prefetch, validate_tarball_origin, Placement, ResolvedDep, ResolverError,
};

#[derive(Debug)]
pub enum InstallError {
    Io(String),
    PackageJson(String),
    Resolver(ResolverError),
    Fetch(FetchError),
    Link(LinkError),
    Lockfile(LockfileError),

    PublisherMismatch {
        name: String,
        version: String,
        pinned: String,
        seen: String,
    },
}

impl From<ResolverError> for InstallError {
    fn from(e: ResolverError) -> Self {
        Self::Resolver(e)
    }
}
impl From<FetchError> for InstallError {
    fn from(e: FetchError) -> Self {
        Self::Fetch(e)
    }
}
impl From<LinkError> for InstallError {
    fn from(e: LinkError) -> Self {
        Self::Link(e)
    }
}
impl From<LockfileError> for InstallError {
    fn from(e: LockfileError) -> Self {
        Self::Lockfile(e)
    }
}

#[derive(Debug)]
pub struct InstallReport {
    pub installed: Vec<(String, String)>,
    pub skipped: Vec<(String, String)>,
    pub dependency_groups: Vec<(String, usize)>,
    pub skipped_lifecycle_scripts: Vec<(String, String)>,
}

struct Materialized {
    dep: ResolvedDep,
    module_map: ModuleMap,
    export_shape: StaticExportShape,

    module_integrity: BTreeMap<String, String>,
}

fn source_sri(bytes: &[u8]) -> String {
    let digest = rusty_js_pm_integrity::sha512_digest(bytes);
    format!("sha512-{}", rusty_js_pm_integrity::encode_base64(&digest))
}

fn compile_module_integrity(dir: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(cur) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&cur) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_dir() {

                if entry.file_name() == "node_modules" {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if !ft.is_file() {
                continue;
            }
            let is_module = matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("js") | Some("mjs") | Some("cjs") | Some("json")
            );
            if !is_module {
                continue;
            }
            let Ok(rel) = path.strip_prefix(dir) else {
                continue;
            };

            let rel_key = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            if let Ok(bytes) = std::fs::read(&path) {
                out.insert(rel_key, source_sri(&bytes));
            }
        }
    }
    out
}

fn cruft_env_present(name: &str) -> bool {
    std::env::var(name).is_ok()
        || name
            .strip_prefix("CRUFT_")
            .is_some_and(|rest| std::env::var(format!("CRUFTLESS_{rest}")).is_ok())
}

fn materialize_one(
    resolved: &ResolvedDep,
    nm_root: &Path,
    mode: MaterializeMode,
) -> Result<Materialized, InstallError> {
    let sp = fetch_into_store(resolved)?;
    materialize_from_store(resolved, &sp.store_dir, nm_root, mode)?;

    let install_dir = nm_root.join(&resolved.name);
    let mut module_map = ModuleMap::new();
    let mut export_shape = StaticExportShape::default();

    let linked_pkg_json = install_dir.join("package.json");
    if let Ok(body) = std::fs::read(&linked_pkg_json) {
        if let Ok(manifest) = rusty_json_manifest::from_slice::<rusty_json_manifest::Value>(&body) {
            module_map = crate::module_map::compile_module_map(&manifest, &install_dir);
            let is_esm = matches!(manifest.get("type"), Some(rusty_json_manifest::Value::String(t)) if t == "module");
            if !is_esm {
                if let Some(entry_rel) = module_map.get(".") {
                    let entry_path = install_dir.join(entry_rel.trim_start_matches("./"));
                    if entry_path.is_file() {
                        export_shape = crate::export_shape::compile_export_shape_at(&entry_path);
                    }
                }
            }
        }
    }

    let module_integrity = compile_module_integrity(&install_dir);
    Ok(Materialized {
        dep: resolved.clone(),
        module_map,
        export_shape,
        module_integrity,
    })
}

fn publisher_pin_opt_in(project_dir: &Path, registry: &str) -> bool {
    cruft_env_present("CRUFT_VERIFY_PUBLISHER") || project_verify_publisher(project_dir, registry)
}

fn project_verify_publisher(project_dir: &Path, registry: &str) -> bool {
    let body = match std::fs::read(project_dir.join("package.json")) {
        Ok(b) => b,
        Err(_) => return false,
    };
    let json: rusty_json_manifest::Value = match rusty_json_manifest::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let Some(cruft) = json.get("cruft").and_then(|c| c.as_object()) else {
        return false;
    };
    let want = registry.trim_end_matches('/');

    if let Some(registries) = cruft.get("registries").and_then(|r| r.as_object()) {
        for (url, cfg) in registries {
            if url.trim_end_matches('/') == want {
                if let Some(cfg) = cfg.as_object() {
                    if cfg
                        .get("verify_publisher")
                        .and_then(rusty_json_manifest::Value::as_bool)
                        == Some(true)
                    {
                        return true;
                    }
                }
            }
        }
    }

    cruft
        .get("verify_publisher")
        .and_then(rusty_json_manifest::Value::as_bool)
        == Some(true)
}

fn publisher_is_mismatch(recorded: Option<&str>, fresh: Option<&str>) -> bool {
    matches!((recorded, fresh), (Some(a), Some(b)) if a != b)
}

fn verify_publisher_pins<'a>(
    lock: &Lockfile,
    resolved: impl IntoIterator<Item = &'a ResolvedDep>,
    opt_in: bool,
) -> Result<(), InstallError> {
    if !opt_in {
        return Ok(());
    }
    for dep in resolved {
        let recorded = lock
            .get(&dep.name, &dep.version)
            .and_then(|d| d.publisher.as_deref());
        let fresh = dep.publisher.as_deref();
        if publisher_is_mismatch(recorded, fresh) {
            return Err(InstallError::PublisherMismatch {
                name: dep.name.clone(),
                version: dep.version.clone(),
                pinned: recorded.unwrap_or_default().to_string(),
                seen: fresh.unwrap_or_default().to_string(),
            });
        }
    }
    Ok(())
}

pub fn pm_install(project_dir: &Path, registry: &str) -> Result<InstallReport, InstallError> {
    pm_install_with_mode(project_dir, registry, MaterializeMode::Link)
}

pub fn pm_install_with_mode(
    project_dir: &Path,
    registry: &str,
    mode: MaterializeMode,
) -> Result<InstallReport, InstallError> {
    let pkg_json_path = project_dir.join("package.json");
    let manifest = read_install_manifest(&pkg_json_path)?;
    let deps = manifest.dependencies;

    let lock_path = project_dir.join(LOCKFILE_NAME);
    let legacy_lock_path = project_dir.join(LEGACY_LOCKFILE_NAME);
    let mut lock = if lock_path.exists() {
        Lockfile::read_from(&lock_path)?
    } else if legacy_lock_path.exists() {

        Lockfile::read_from(&legacy_lock_path)?
    } else {
        Lockfile::new()
    };

    let nm_root = project_dir.join("node_modules");
    let mut report = InstallReport {
        installed: Vec::new(),
        skipped: Vec::new(),
        dependency_groups: manifest.dependency_groups,
        skipped_lifecycle_scripts: manifest.lifecycle_scripts,
    };

    let _prof = cruft_env_present("CRUFT_PM_PROFILE");
    let _t_resolve = std::time::Instant::now();
    let frozen = !lock.packages.is_empty() && lock.covers(&deps);

    let (closure, peer_demanded): (Vec<Placement>, std::collections::BTreeSet<String>) = if frozen {

        let closure = lock.frozen_closure();
        for placement in &closure {
            validate_tarball_origin(registry, &placement.dep.tarball_url)?;
        }
        (closure, std::collections::BTreeSet::new())
    } else {

        use std::collections::VecDeque;
        use std::sync::{Arc, Condvar, Mutex};
        let queue: Arc<(Mutex<(VecDeque<ResolvedDep>, bool)>, Condvar)> =
            Arc::new((Mutex::new((VecDeque::new(), false)), Condvar::new()));
        let n_pre = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .min(8)
            .max(1);
        let mut handles = Vec::with_capacity(n_pre);
        for _ in 0..n_pre {
            let q = Arc::clone(&queue);
            handles.push(std::thread::spawn(move || loop {
                let dep = {
                    let (lock, cv) = &*q;
                    let mut g = lock.lock().unwrap();
                    loop {
                        if let Some(d) = g.0.pop_front() {
                            break Some(d);
                        }
                        if g.1 {
                            break None;
                        }
                        g = cv.wait(g).unwrap();
                    }
                };
                match dep {
                    Some(d) => {
                        let _ = fetch_into_store(&d);
                    }
                    None => break,
                }
            }));
        }
        let q_cb = Arc::clone(&queue);
        let result = resolve_closure_prefetch(registry, &deps, &mut |d| {
            let (lock, cv) = &*q_cb;
            lock.lock().unwrap().0.push_back(d.clone());
            cv.notify_one();
        });
        {
            let (lock, cv) = &*queue;
            lock.lock().unwrap().1 = true;
            cv.notify_all();
        }
        for h in handles {
            let _ = h.join();
        }
        result?
    };
    if _prof {
        eprintln!(
            "[pm-profile] resolve+prefetch (frozen={frozen}): {:?}",
            _t_resolve.elapsed()
        );
        eprintln!("{}", crate::fetcher::fetch_profile_report());
    }

    if !frozen {
        verify_publisher_pins(
            &lock,
            closure.iter().map(|p| &p.dep),
            publisher_pin_opt_in(project_dir, registry),
        )?;
    }

    let _t_mat = std::time::Instant::now();

    let mut placement_map: std::collections::BTreeMap<(String, String), Vec<String>> =
        std::collections::BTreeMap::new();
    for p in &closure {
        placement_map
            .entry((p.dep.name.clone(), p.dep.version.clone()))
            .or_default()

            .push(p.nest_under.join("/"));
    }

    let mut auto_peers_map: std::collections::BTreeMap<(String, String), Vec<String>> =
        std::collections::BTreeMap::new();
    if !peer_demanded.is_empty() {
        for p in &closure {
            let mut peers: Vec<String> = p
                .dep
                .peer_dependencies
                .keys()
                .filter(|peer| peer_demanded.contains(*peer))
                .filter(|peer| !p.dep.optional_peers.contains(*peer))
                .cloned()
                .collect();
            if !peers.is_empty() {
                peers.sort();
                peers.dedup();
                auto_peers_map.insert((p.dep.name.clone(), p.dep.version.clone()), peers);
            }
        }
    }

    let mut to_install: Vec<(ResolvedDep, PathBuf)> = Vec::new();
    for p in closure {

        let mut target_nm = nm_root.clone();
        for ancestor in &p.nest_under {
            target_nm = target_nm.join(ancestor).join("node_modules");
        }
        let install_dir = target_nm.join(&p.dep.name);
        let already_present = install_dir.join("package.json").exists()
            && lock.get(&p.dep.name, &p.dep.version).is_some();
        if already_present {
            report.skipped.push((p.dep.name, p.dep.version));
        } else {
            to_install.push((p.dep, target_nm));
        }
    }

    let mut waves: BTreeMap<usize, Vec<(ResolvedDep, PathBuf)>> = BTreeMap::new();
    for (dep, target_nm) in to_install {
        waves
            .entry(target_nm.components().count())
            .or_default()
            .push((dep, target_nm));
    }
    let results: Mutex<Vec<Result<Materialized, InstallError>>> = Mutex::new(Vec::new());
    let mut peak_workers = 0usize;
    let mut package_count = 0usize;
    for wave in waves.values() {
        let workers = wave
            .len()
            .min(
                std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(4),
            )
            .min(8)
            .max(1);
        peak_workers = peak_workers.max(workers);
        package_count += wave.len();
        let cursor = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| loop {
                    let i = cursor.fetch_add(1, Ordering::Relaxed);
                    if i >= wave.len() {
                        break;
                    }
                    let (dep, target_nm) = &wave[i];
                    let r = materialize_one(dep, target_nm, mode);
                    results.lock().unwrap().push(r);
                });
            }
        });
    }

    if _prof {
        eprintln!(
            "[pm-profile] materialize ({} pkgs, <= {} workers/wave): {:?}",
            package_count,
            peak_workers,
            _t_mat.elapsed()
        );
    }

    let mut mats = Vec::new();
    for r in results.into_inner().unwrap() {
        mats.push(r?);
    }
    mats.sort_by(|a, b| {
        (a.dep.name.as_str(), a.dep.version.as_str())
            .cmp(&(b.dep.name.as_str(), b.dep.version.as_str()))
    });
    for m in mats {
        let name = m.dep.name.clone();
        let version = m.dep.version.clone();
        lock.insert(m.dep);
        if !m.module_map.is_empty() {
            lock.set_module_map(&name, &version, m.module_map);
        }
        if !m.export_shape.is_empty() {
            lock.set_export_shape(&name, &version, m.export_shape);
        }
        if !m.module_integrity.is_empty() {
            lock.set_module_integrity(&name, &version, m.module_integrity);
        }
        report.installed.push((name, version));
    }

    for ((name, version), places) in placement_map {
        lock.set_placements(&name, &version, places);
    }

    for ((name, version), peers) in auto_peers_map {
        lock.set_auto_peers(&name, &version, peers);
    }

    let staging_root = nm_root.join(".cruft-staging");
    if staging_root.exists() {
        let _ = std::fs::remove_dir_all(&staging_root);
    }

    lock.write_to(&lock_path)?;

    Ok(report)
}

struct InstallManifest {
    dependencies: Vec<(String, String)>,
    dependency_groups: Vec<(String, usize)>,
    lifecycle_scripts: Vec<(String, String)>,
}

fn read_install_manifest(path: &Path) -> Result<InstallManifest, InstallError> {
    let body = std::fs::read(path).map_err(|e| InstallError::Io(format!("read {path:?}: {e}")))?;
    let json: rusty_json_manifest::Value = rusty_json_manifest::from_slice(&body)
        .map_err(|e| InstallError::PackageJson(format!("{e}")))?;
    let mut merged = BTreeMap::new();
    let mut dependency_groups = Vec::new();
    for field in ["dependencies", "devDependencies", "optionalDependencies"] {
        let Some(deps) = json.get(field) else {
            continue;
        };
        let map = deps
            .as_object()
            .ok_or_else(|| InstallError::PackageJson(format!("{field} not an object")))?;
        dependency_groups.push((field.to_string(), map.len()));
        for (k, v) in map {
            let v = v
                .as_str()
                .ok_or_else(|| InstallError::PackageJson(format!("{field}.{k} not a string")))?;
            merged.insert(k.clone(), v.to_string());
        }
    }
    let mut lifecycle_scripts = Vec::new();
    if let Some(scripts) = json.get("scripts") {
        let map = scripts
            .as_object()
            .ok_or_else(|| InstallError::PackageJson("scripts not an object".into()))?;
        for name in ["preinstall", "install", "postinstall", "prepare"] {
            if let Some(script) = map.get(name) {
                let script = script.as_str().ok_or_else(|| {
                    InstallError::PackageJson(format!("scripts.{name} not a string"))
                })?;
                lifecycle_scripts.push((name.to_string(), script.to_string()));
            }
        }
    }

    Ok(InstallManifest {
        dependencies: merged.into_iter().collect(),
        dependency_groups,
        lifecycle_scripts,
    })
}

fn _tmp_workdir(tag: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "cruft-pm-install-{tag}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolver::DEFAULT_REGISTRY;

    fn workdir(tag: &str) -> PathBuf {
        _tmp_workdir(tag)
    }

    fn pub_dep(name: &str, version: &str, publisher: Option<&str>) -> ResolvedDep {
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
            publisher: publisher.map(String::from),
        }
    }

    fn lock_with(dep: ResolvedDep) -> Lockfile {
        let mut lock = Lockfile::new();
        lock.insert(dep);
        lock
    }

    #[test]
    fn publisher_predicate_semantics() {
        assert!(publisher_is_mismatch(Some("alice"), Some("mallory")));
        assert!(!publisher_is_mismatch(Some("alice"), Some("alice")));
        assert!(!publisher_is_mismatch(None, Some("mallory")));
        assert!(!publisher_is_mismatch(Some("alice"), None));
        assert!(!publisher_is_mismatch(None, None));
    }

    #[test]
    fn verify_matching_publisher_ok() {
        let lock = lock_with(pub_dep("left-pad", "1.0.0", Some("alice")));
        let fresh = pub_dep("left-pad", "1.0.0", Some("alice"));
        assert!(verify_publisher_pins(&lock, [&fresh], true).is_ok());
    }

    #[test]
    fn verify_mismatched_publisher_errors_under_opt_in() {
        let lock = lock_with(pub_dep("left-pad", "1.0.0", Some("alice")));
        let fresh = pub_dep("left-pad", "1.0.0", Some("mallory"));
        match verify_publisher_pins(&lock, [&fresh], true) {
            Err(InstallError::PublisherMismatch {
                name, pinned, seen, ..
            }) => {
                assert_eq!(name, "left-pad");
                assert_eq!(pinned, "alice");
                assert_eq!(seen, "mallory");
            }
            other => panic!("expected PublisherMismatch, got {other:?}"),
        }
    }

    #[test]
    fn verify_missing_publisher_no_enforcement() {

        let lock = lock_with(pub_dep("left-pad", "1.0.0", None));
        let fresh = pub_dep("left-pad", "1.0.0", Some("mallory"));
        assert!(verify_publisher_pins(&lock, [&fresh], true).is_ok());

        let lock2 = lock_with(pub_dep("left-pad", "1.0.0", Some("alice")));
        let fresh2 = pub_dep("left-pad", "1.0.0", None);
        assert!(verify_publisher_pins(&lock2, [&fresh2], true).is_ok());

        let empty = Lockfile::new();
        let fresh3 = pub_dep("left-pad", "1.0.0", Some("mallory"));
        assert!(verify_publisher_pins(&empty, [&fresh3], true).is_ok());
    }

    #[test]
    fn verify_opt_out_is_noop() {
        let lock = lock_with(pub_dep("left-pad", "1.0.0", Some("alice")));
        let fresh = pub_dep("left-pad", "1.0.0", Some("mallory"));
        assert!(verify_publisher_pins(&lock, [&fresh], false).is_ok());
    }

    static ENV_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn config_per_registry_opt_in_enables_enforcement() {
        let _g = ENV_GUARD.lock().unwrap();
        let dir = workdir("verify-pub-per-registry");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            r#"{
                "cruft": {
                    "registries": {
                        "https://registry.npmjs.org": { "verify_publisher": true }
                    }
                }
            }"#,
        )
        .unwrap();

        assert!(publisher_pin_opt_in(&dir, "https://registry.npmjs.org/"));

        let lock = lock_with(pub_dep("left-pad", "1.0.0", Some("alice")));
        let fresh = pub_dep("left-pad", "1.0.0", Some("mallory"));
        let opt_in = publisher_pin_opt_in(&dir, "https://registry.npmjs.org");
        assert!(matches!(
            verify_publisher_pins(&lock, [&fresh], opt_in),
            Err(InstallError::PublisherMismatch { .. })
        ));

        assert!(!publisher_pin_opt_in(&dir, "https://other.example.com"));
    }

    #[test]
    fn config_project_wide_opt_in_enables_enforcement() {
        let _g = ENV_GUARD.lock().unwrap();
        let dir = workdir("verify-pub-project-wide");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            r#"{ "cruft": { "verify_publisher": true } }"#,
        )
        .unwrap();
        assert!(publisher_pin_opt_in(&dir, "https://any.registry.example"));
    }

    #[test]
    fn env_var_opt_in_still_works() {
        let _g = ENV_GUARD.lock().unwrap();
        let dir = workdir("verify-pub-env");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("package.json"), r#"{ "dependencies": {} }"#).unwrap();

        assert!(!publisher_pin_opt_in(&dir, "https://registry.npmjs.org"));

        std::env::set_var("CRUFT_VERIFY_PUBLISHER", "1");
        assert!(publisher_pin_opt_in(&dir, "https://registry.npmjs.org"));
        std::env::remove_var("CRUFT_VERIFY_PUBLISHER");
    }

    #[test]
    fn config_and_env_both_off_is_noop() {
        let _g = ENV_GUARD.lock().unwrap();
        let dir = workdir("verify-pub-both-off");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            r#"{ "cruft": { "registries": { "https://registry.npmjs.org": { "verify_publisher": false } } } }"#,
        )
        .unwrap();
        assert!(!publisher_pin_opt_in(&dir, "https://registry.npmjs.org"));

        let missing = workdir("verify-pub-missing");
        assert!(!publisher_pin_opt_in(
            &missing,
            "https://registry.npmjs.org"
        ));
    }

    #[test]
    fn read_deps_empty() {
        let dir = workdir("read-empty");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            br#"{"name":"app","version":"0.0.1"}"#,
        )
        .unwrap();
        let manifest = read_install_manifest(&dir.join("package.json")).unwrap();
        assert!(manifest.dependencies.is_empty());
        assert!(manifest.dependency_groups.is_empty());
        assert!(manifest.lifecycle_scripts.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_deps_sorted() {
        let dir = workdir("read-sorted");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            br#"{"dependencies":{"zeta":"1.0.0","alpha":"2.0.0"}}"#,
        )
        .unwrap();
        let manifest = read_install_manifest(&dir.join("package.json")).unwrap();
        assert_eq!(
            manifest.dependencies,
            vec![
                ("alpha".to_string(), "2.0.0".to_string()),
                ("zeta".to_string(), "1.0.0".to_string()),
            ]
        );
        assert_eq!(manifest.dependency_groups, vec![("dependencies".into(), 2)]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_install_manifest_seeds_dev_and_optional_groups() {
        let dir = workdir("read-dev-optional");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            br#"{
                "dependencies":{"shared":"1.0.0","prod":"1.0.0"},
                "devDependencies":{"dev-only":"2.0.0"},
                "optionalDependencies":{"shared":"3.0.0","native":"4.0.0"}
            }"#,
        )
        .unwrap();
        let manifest = read_install_manifest(&dir.join("package.json")).unwrap();
        assert_eq!(
            manifest.dependencies,
            vec![
                ("dev-only".to_string(), "2.0.0".to_string()),
                ("native".to_string(), "4.0.0".to_string()),
                ("prod".to_string(), "1.0.0".to_string()),
                ("shared".to_string(), "3.0.0".to_string()),
            ]
        );
        assert_eq!(
            manifest.dependency_groups,
            vec![
                ("dependencies".into(), 2),
                ("devDependencies".into(), 1),
                ("optionalDependencies".into(), 2),
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_install_manifest_reports_lifecycle_scripts() {
        let dir = workdir("read-lifecycle");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            br#"{"scripts":{"preinstall":"node pre.js","test":"node test.js","postinstall":"node post.js"}}"#,
        )
        .unwrap();
        let manifest = read_install_manifest(&dir.join("package.json")).unwrap();
        assert_eq!(
            manifest.lifecycle_scripts,
            vec![
                ("preinstall".to_string(), "node pre.js".to_string()),
                ("postinstall".to_string(), "node post.js".to_string()),
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn legacy_lockfile_name_is_read_but_canonical_name_is_written() {
        let dir = workdir("legacy-lockfile-read");
        std::fs::create_dir_all(dir.join("node_modules/left-pad")).unwrap();
        std::fs::write(
            dir.join("package.json"),
            br#"{"name":"app","version":"0.0.1","dependencies":{"left-pad":"1.3.0"}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("node_modules/left-pad/package.json"),
            br#"{"name":"left-pad","version":"1.3.0"}"#,
        )
        .unwrap();

        let dep = ResolvedDep {
            name: "left-pad".to_string(),
            version: "1.3.0".to_string(),
            tarball_url: "https://example.invalid/left-pad.tgz".to_string(),
            integrity: None,
            shasum: None,
            dependencies: Default::default(),
            optional_dependencies: Default::default(),
            os: Default::default(),
            cpu: Default::default(),
            peer_dependencies: Default::default(),
            optional_peers: Default::default(),
            caps: Default::default(),
            publisher: None,
        };
        let mut legacy = Lockfile::from_resolved([dep]);
        legacy.set_placements("left-pad", "1.3.0", vec![String::new()]);
        legacy
            .write_to(&dir.join(LEGACY_LOCKFILE_NAME))
            .expect("write legacy lockfile");

        let report = pm_install(&dir, "https://example.invalid").expect("frozen install");
        assert_eq!(report.installed.len(), 0);
        assert_eq!(
            report.skipped,
            vec![("left-pad".to_string(), "1.3.0".to_string())]
        );
        assert!(
            dir.join(LOCKFILE_NAME).exists(),
            "canonical lockfile is written"
        );
        assert!(
            dir.join(LEGACY_LOCKFILE_NAME).exists(),
            "migration reads but does not remove the legacy lockfile"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn frozen_lockfile_rejects_cross_origin_tarball_before_skip() {
        let dir = workdir("frozen-lockfile-cross-origin");
        std::fs::create_dir_all(dir.join("node_modules/left-pad")).unwrap();
        std::fs::write(
            dir.join("package.json"),
            br#"{"name":"app","version":"0.0.1","dependencies":{"left-pad":"1.3.0"}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("node_modules/left-pad/package.json"),
            br#"{"name":"left-pad","version":"1.3.0"}"#,
        )
        .unwrap();

        let dep = ResolvedDep {
            name: "left-pad".to_string(),
            version: "1.3.0".to_string(),
            tarball_url: "https://attacker.invalid/left-pad.tgz".to_string(),
            integrity: Some("sha512-deadbeef".to_string()),
            shasum: None,
            dependencies: Default::default(),
            optional_dependencies: Default::default(),
            os: Default::default(),
            cpu: Default::default(),
            peer_dependencies: Default::default(),
            optional_peers: Default::default(),
            caps: Default::default(),
            publisher: None,
        };
        let mut lock = Lockfile::from_resolved([dep]);
        lock.set_placements("left-pad", "1.3.0", vec![String::new()]);
        lock.write_to(&dir.join(LOCKFILE_NAME))
            .expect("write tampered lockfile");

        let err = pm_install(&dir, DEFAULT_REGISTRY).expect_err("cross-origin lockfile rejected");
        assert!(matches!(
            err,
            InstallError::Resolver(ResolverError::TarballOrigin { .. })
        ));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[ignore]
    fn install_lodash_idempotent() {
        let dir = workdir("install-lodash");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            br#"{"name":"app","version":"0.0.1","dependencies":{"lodash":"4.17.21"}}"#,
        )
        .unwrap();

        let r1 = pm_install(&dir, DEFAULT_REGISTRY).expect("install 1");
        assert_eq!(r1.installed.len(), 1);
        assert_eq!(r1.skipped.len(), 0);
        assert_eq!(r1.installed[0].0, "lodash");

        let lockfile_path = dir.join(LOCKFILE_NAME);
        assert!(lockfile_path.exists(), "lockfile should be written");
        let lock = Lockfile::read_from(&lockfile_path).unwrap();
        assert!(lock.get("lodash", "4.17.21").is_some());

        let lodash_pkg = dir.join("node_modules/lodash/package.json");
        assert!(lodash_pkg.exists(), "lodash/package.json should exist");
        let pj = std::fs::read_to_string(&lodash_pkg).unwrap();
        assert!(pj.contains("\"version\": \"4.17.21\""));

        let r2 = pm_install(&dir, DEFAULT_REGISTRY).expect("install 2");
        assert_eq!(r2.installed.len(), 0, "second run should skip, not refetch");
        assert_eq!(r2.skipped.len(), 1);
        assert_eq!(r2.skipped[0].0, "lodash");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[ignore]
    fn install_debug_with_transitive() {
        let dir = workdir("install-debug");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            br#"{"name":"app","version":"0.0.1","dependencies":{"debug":"4.3.4"}}"#,
        )
        .unwrap();

        let r = pm_install(&dir, DEFAULT_REGISTRY).expect("install");
        assert_eq!(
            r.installed.len(),
            2,
            "expected debug + ms; got {:?}",
            r.installed
        );
        let names: Vec<&str> = r.installed.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"debug"));
        assert!(names.contains(&"ms"));

        assert!(dir.join("node_modules/debug/package.json").exists());
        assert!(dir.join("node_modules/ms/package.json").exists());

        let lock = Lockfile::read_from(&dir.join(LOCKFILE_NAME)).unwrap();
        assert!(lock.get("debug", "4.3.4").is_some());
        assert!(lock.get("ms", "2.1.2").is_some());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
