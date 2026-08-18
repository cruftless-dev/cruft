
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::http::{pm_http_get, pm_http_get_accept, HttpError, ABBREVIATED_PACKUMENT_ACCEPT};

pub const DEFAULT_REGISTRY: &str = "https://registry.npmjs.org";
pub(crate) const MAX_RESOLUTION_EDGES: usize = 10_000;

fn cruft_env_var(name: &str) -> Result<String, std::env::VarError> {
    std::env::var(name).or_else(|err| {
        if let Some(rest) = name.strip_prefix("CRUFT_") {
            std::env::var(format!("CRUFTLESS_{rest}"))
        } else {
            Err(err)
        }
    })
}

fn cruft_env_present(name: &str) -> bool {
    cruft_env_var(name).is_ok()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Caps {
    pub net: Vec<String>,
    pub fs: Vec<String>,
    pub env: Vec<String>,
    pub exec: Vec<String>,
}

impl Caps {

    pub fn is_empty(&self) -> bool {
        self.net.is_empty() && self.fs.is_empty() && self.env.is_empty() && self.exec.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedDep {
    pub name: String,
    pub version: String,
    pub tarball_url: String,
    pub integrity: Option<String>,
    pub shasum: Option<String>,

    pub dependencies: BTreeMap<String, String>,

    pub optional_dependencies: BTreeMap<String, String>,
    pub os: Vec<String>,
    pub cpu: Vec<String>,

    pub peer_dependencies: BTreeMap<String, String>,

    pub optional_peers: BTreeSet<String>,

    pub caps: Caps,

    pub publisher: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Placement {
    pub dep: ResolvedDep,
    pub nest_under: Vec<String>,
}

#[derive(Debug)]
pub enum ResolverError {
    Http(HttpError),
    Json(String),
    MissingField(&'static str),

    NonExactVersionSpec(String),

    NoSatisfyingVersion {
        name: String,
        range: String,
    },

    BadRange(String),

    UnsupportedPlatform {
        name: String,
        os: Vec<String>,
        cpu: Vec<String>,
    },

    TarballOrigin {
        registry: String,
        tarball: String,
    },

    ConflictUnsatisfiable {
        name: String,
        ranges: Vec<String>,

        chains: Vec<Vec<(String, String)>>,
    },

    PeerConflict {
        name: String,
        ranges: Vec<String>,
    },

    DependencyGraphLimit {
        limit: usize,
    },
}

impl From<HttpError> for ResolverError {
    fn from(e: HttpError) -> Self {
        ResolverError::Http(e)
    }
}

impl std::fmt::Display for ResolverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolverError::Http(e) => write!(f, "registry http error: {e:?}"),
            ResolverError::Json(m) => write!(f, "registry json error: {m}"),
            ResolverError::MissingField(field) => {
                write!(f, "registry response missing field: {field}")
            }
            ResolverError::NonExactVersionSpec(s) => write!(f, "non-exact version spec: {s}"),
            ResolverError::NoSatisfyingVersion { name, range } => {
                write!(f, "no published version of `{name}` satisfies `{range}`")
            }
            ResolverError::BadRange(s) => write!(f, "unparseable semver range: {s}"),
            ResolverError::UnsupportedPlatform { name, os, cpu } => write!(
                f,
                "package `{name}` does not support this host platform (os: {os:?}, cpu: {cpu:?})"
            ),
            ResolverError::TarballOrigin { registry, tarball } => write!(
                f,
                "registry tarball origin mismatch: registry {registry} emitted {tarball}"
            ),
            ResolverError::ConflictUnsatisfiable {
                name,
                ranges,
                chains,
            } => {
                write!(
                    f,
                    "unsatisfiable version conflict for `{name}`: no single version satisfies all of [{}]",
                    ranges.join(", ")
                )?;
                for chain in chains {
                    if chain.is_empty() {
                        continue;
                    }
                    let rendered: Vec<String> = chain
                        .iter()
                        .map(|(req, spec)| format!("{req} requires {spec}"))
                        .collect();
                    write!(f, "\n    chain: {}", rendered.join(" → "))?;
                }
                Ok(())
            }
            ResolverError::PeerConflict { name, ranges } => write!(
                f,
                "unsatisfiable peer dependency conflict for `{name}`: no single version satisfies all peer ranges [{}] (npm ERESOLVE; --legacy-peer-deps to override)",
                ranges.join(", ")
            ),
            ResolverError::DependencyGraphLimit { limit } => {
                write!(f, "dependency graph exceeds resolver limit of {limit} edges")
            }
        }
    }
}

impl std::error::Error for ResolverError {}

fn compute_nested_placements(
    conflicted: &BTreeSet<String>,
    chosen_ver: &dyn Fn(&str) -> Option<String>,
    edges: &[(Option<String>, String, String)],
    versions_of: &dyn Fn(&str) -> Vec<String>,
    deps_of: &dyn Fn(&str, &str) -> Vec<(String, String)>,
) -> Vec<(Vec<String>, String, String)> {
    const MAX_NEST_DEPTH: usize = 32;
    let mut out: Vec<(Vec<String>, String, String)> = Vec::new();
    let mut work: VecDeque<(Vec<String>, String, String)> = VecDeque::new();
    let mut seen: BTreeSet<(Vec<String>, String, String)> = BTreeSet::new();

    for name in conflicted {
        let Some(hoisted) = chosen_ver(name) else {
            continue;
        };
        let keys = versions_of(name);
        if keys.is_empty() {
            continue;
        }
        for (parent, ename, range) in edges {
            if ename != name {
                continue;
            }
            if crate::semver::satisfies(range, &hoisted).unwrap_or(false) {
                continue;
            }
            let Some(parent) = parent else { continue };
            let Some(nver) = crate::semver::max_satisfying(range, &keys) else {
                continue;
            };
            if nver == hoisted {
                continue;
            }
            let item = (vec![parent.clone()], name.clone(), nver.to_string());
            if seen.insert(item.clone()) {
                work.push_back(item);
            }
        }
    }

    while let Some((ancestor, name, version)) = work.pop_front() {
        out.push((ancestor.clone(), name.clone(), version.clone()));
        let mut child_ancestor = ancestor.clone();
        child_ancestor.push(name.clone());
        if child_ancestor.len() > MAX_NEST_DEPTH {
            continue;
        }
        for (dn, dr) in deps_of(&name, &version) {
            if let Some(tv) = chosen_ver(&dn) {
                if crate::semver::satisfies(&dr, &tv).unwrap_or(false) {
                    continue;
                }
            }
            if child_ancestor.iter().any(|a| a == &dn) {
                continue;
            }
            let dkeys = versions_of(&dn);
            let Some(dver) = crate::semver::max_satisfying(&dr, &dkeys) else {
                continue;
            };
            let item = (child_ancestor.clone(), dn.clone(), dver.to_string());
            if seen.insert(item.clone()) {
                work.push_back(item);
            }
        }
    }
    out
}

fn reconstruct_chain(
    edges: &[(Option<String>, String, String)],
    name: &str,
    range: &str,
) -> Vec<(String, String)> {
    let mut chain: Vec<(String, String)> = Vec::new();
    let mut visited: Vec<String> = Vec::new();
    let mut cur_child = name.to_string();
    let mut cur_range = range.to_string();
    let mut guard = 0usize;
    loop {
        guard += 1;
        if guard > 256 {
            break;
        }
        if visited.iter().any(|c| c == &cur_child) {
            break;
        }
        visited.push(cur_child.clone());

        let edge = edges
            .iter()
            .find(|(_, c, r)| c == &cur_child && r == &cur_range)
            .or_else(|| edges.iter().find(|(_, c, _)| c == &cur_child));
        let Some((parent, c, r)) = edge else {
            break;
        };
        let requirer = parent.clone().unwrap_or_else(|| "<root>".to_string());
        chain.push((requirer, format!("{c}@{r}")));
        match parent {
            None => break,
            Some(p) => {

                let next_range = edges
                    .iter()
                    .find(|(_, c2, _)| c2 == p)
                    .map(|(_, _, r2)| r2.clone())
                    .unwrap_or_default();
                cur_child = p.clone();
                cur_range = next_range;
            }
        }
    }
    chain.reverse();
    chain
}

pub fn resolve_specifier(
    registry: &str,
    name: &str,
    version: &str,
) -> Result<ResolvedDep, ResolverError> {

    if crate::semver::is_exact_pin(version) {
        return resolve_exact(registry, name, version);
    }

    resolve_via_packument(registry, name, version)
}

fn fetch_packument(
    registry: &str,
    name: &str,
) -> Result<rusty_json_manifest::Value, ResolverError> {
    let url = format!("{}/{}", registry.trim_end_matches('/'), name);
    let body = pm_http_get_accept(&url, ABBREVIATED_PACKUMENT_ACCEPT)?;
    rusty_json_manifest::from_slice(&body).map_err(|e| ResolverError::Json(format!("{e:?}")))
}

fn fetch_packuments_parallel(
    registry: &str,
    names: &[String],
) -> Vec<(String, Result<rusty_json_manifest::Value, ResolverError>)> {
    if names.is_empty() {
        return Vec::new();
    }

    let cap = cruft_env_var("CRUFT_PM_CONCURRENCY")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(16);
    let workers = names
        .len()
        .min(
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
        )
        .min(cap)
        .max(1);
    if cruft_env_present("CRUFT_PM_PROFILE") {
        eprintln!(
            "[pm-profile]   packument wave: {} names, {} workers",
            names.len(),
            workers
        );
    }
    let cursor = AtomicUsize::new(0);
    let out: Mutex<Vec<(String, Result<rusty_json_manifest::Value, ResolverError>)>> =
        Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let i = cursor.fetch_add(1, Ordering::Relaxed);
                if i >= names.len() {
                    break;
                }
                let name = &names[i];
                let r = fetch_packument(registry, name);
                out.lock().unwrap().push((name.clone(), r));
            });
        }
    });
    out.into_inner().unwrap()
}

fn resolve_via_packument(
    registry: &str,
    name: &str,
    range: &str,
) -> Result<ResolvedDep, ResolverError> {
    crate::semver::Range::parse(range).map_err(|e| ResolverError::BadRange(format!("{e}")))?;
    let url = format!("{}/{}", registry.trim_end_matches('/'), name);
    let body = pm_http_get_accept(&url, ABBREVIATED_PACKUMENT_ACCEPT)?;
    let json: rusty_json_manifest::Value = rusty_json_manifest::from_slice(&body)
        .map_err(|e| ResolverError::Json(format!("{e:?}")))?;
    let versions = json
        .get("versions")
        .and_then(|v| v.as_object())
        .ok_or(ResolverError::MissingField("versions"))?;
    let keys: Vec<String> = versions.keys().cloned().collect();

    let latest_pref = json
        .get("dist-tags")
        .and_then(|d| d.get("latest"))
        .and_then(|v| v.as_str())
        .filter(|lv| {
            keys.iter().any(|k| k == *lv) && crate::semver::satisfies(range, lv).unwrap_or(false)
        })
        .map(|lv| lv.to_string());
    let chosen = match latest_pref {
        Some(ref lv) => lv.as_str(),
        None => crate::semver::max_satisfying(range, &keys).ok_or_else(|| {
            ResolverError::NoSatisfyingVersion {
                name: name.to_string(),
                range: range.to_string(),
            }
        })?,
    };
    let manifest = versions
        .get(chosen)
        .ok_or(ResolverError::MissingField("versions.<chosen>"))?;
    build_resolved_dep(registry, name, chosen, manifest)
}

fn parse_caps(manifest: &rusty_json_manifest::Value) -> Caps {
    let mut caps = Caps::default();
    let Some(obj) = manifest.get("caps").and_then(|v| v.as_object()) else {
        return caps;
    };
    let read_class = |key: &str| -> Vec<String> {
        obj.get(key)
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
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

fn parse_publisher(manifest: &rusty_json_manifest::Value) -> Option<String> {
    if let Some(name) = manifest
        .get("_npmUser")
        .and_then(|u| u.get("name"))
        .and_then(|v| v.as_str())
    {
        return Some(name.to_string());
    }
    manifest
        .get("maintainers")
        .and_then(|m| m.as_array())
        .and_then(|arr| arr.first())
        .and_then(|m| m.get("name"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn host_os() -> &'static str {
    if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "win32"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        std::env::consts::OS
    }
}

fn host_cpu() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        std::env::consts::ARCH
    }
}

fn platform_axis_matches(constraints: &[String], host: &str) -> bool {
    if constraints.iter().any(|entry| entry == &format!("!{host}")) {
        return false;
    }
    let has_positive = constraints.iter().any(|entry| !entry.starts_with('!'));
    !has_positive || constraints.iter().any(|entry| entry == host)
}

fn dep_matches_host(dep: &ResolvedDep) -> bool {
    platform_axis_matches(&dep.os, host_os()) && platform_axis_matches(&dep.cpu, host_cpu())
}

fn build_resolved_dep(
    registry: &str,
    name: &str,
    version: &str,
    manifest: &rusty_json_manifest::Value,
) -> Result<ResolvedDep, ResolverError> {
    let platform_list = |key: &str| {
        manifest
            .get(key)
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };
    let dist = manifest
        .get("dist")
        .ok_or(ResolverError::MissingField("dist"))?;
    let tarball = dist
        .get("tarball")
        .and_then(|v| v.as_str())
        .ok_or(ResolverError::MissingField("dist.tarball"))?;
    validate_tarball_origin(registry, tarball)?;
    let integrity = dist
        .get("integrity")
        .and_then(|v| v.as_str())
        .map(String::from);
    let shasum = dist
        .get("shasum")
        .and_then(|v| v.as_str())
        .map(String::from);
    let mut dependencies = BTreeMap::new();
    if let Some(obj) = manifest.get("dependencies").and_then(|v| v.as_object()) {
        for (k, v) in obj {
            if let Some(s) = v.as_str() {
                dependencies.insert(k.clone(), s.to_string());
            }
        }
    }
    let mut optional_dependencies = BTreeMap::new();
    if let Some(obj) = manifest
        .get("optionalDependencies")
        .and_then(|v| v.as_object())
    {
        for (k, v) in obj {
            if let Some(s) = v.as_str() {

                dependencies.remove(k);
                optional_dependencies.insert(k.clone(), s.to_string());
            }
        }
    }

    let mut peer_dependencies = BTreeMap::new();
    if let Some(obj) = manifest.get("peerDependencies").and_then(|v| v.as_object()) {
        for (k, v) in obj {
            if let Some(s) = v.as_str() {
                peer_dependencies.insert(k.clone(), s.to_string());
            }
        }
    }
    let mut optional_peers = BTreeSet::new();
    if let Some(obj) = manifest
        .get("peerDependenciesMeta")
        .and_then(|v| v.as_object())
    {
        for (k, v) in obj {
            if v.get("optional").and_then(|o| o.as_bool()).unwrap_or(false) {
                optional_peers.insert(k.clone());
            }
        }
    }
    Ok(ResolvedDep {
        name: name.to_string(),
        version: version.to_string(),
        tarball_url: tarball.to_string(),
        integrity,
        shasum,
        dependencies,
        optional_dependencies,
        os: platform_list("os"),
        cpu: platform_list("cpu"),
        peer_dependencies,
        optional_peers,
        caps: parse_caps(manifest),
        publisher: parse_publisher(manifest),
    })
}

fn origin(url: &str) -> Result<(String, String, u16), ResolverError> {
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| ResolverError::Json(format!("unsupported URL scheme: {url}")))?;
    let authority = rest.split('/').next().unwrap_or(rest);
    if authority.is_empty() || authority.contains('@') {
        return Err(ResolverError::Json(format!(
            "malformed URL authority: {url}"
        )));
    }
    let (host, port) = if let Some((host, port)) = authority.rsplit_once(':') {
        if host.is_empty() {
            return Err(ResolverError::Json(format!(
                "malformed URL authority: {url}"
            )));
        }
        let port = port
            .parse::<u16>()
            .map_err(|_| ResolverError::Json(format!("malformed URL port: {url}")))?;
        (host.to_ascii_lowercase(), port)
    } else {
        (authority.to_ascii_lowercase(), 443)
    };
    Ok(("https".to_string(), host, port))
}

pub(crate) fn validate_tarball_origin(registry: &str, tarball: &str) -> Result<(), ResolverError> {
    let registry_origin = origin(registry)?;
    let tarball_origin = origin(tarball)?;
    if registry_origin != tarball_origin {
        return Err(ResolverError::TarballOrigin {
            registry: registry.to_string(),
            tarball: tarball.to_string(),
        });
    }
    Ok(())
}

pub fn resolve_range(registry: &str, name: &str, range: &str) -> Result<String, ResolverError> {

    crate::semver::Range::parse(range).map_err(|e| ResolverError::BadRange(format!("{e}")))?;
    let available = fetch_all_versions(registry, name)?;
    match crate::semver::max_satisfying(range, &available) {
        Some(v) => Ok(v.to_string()),
        None => Err(ResolverError::NoSatisfyingVersion {
            name: name.to_string(),
            range: range.to_string(),
        }),
    }
}

pub fn fetch_all_versions(registry: &str, name: &str) -> Result<Vec<String>, ResolverError> {
    let url = format!("{}/{}", registry.trim_end_matches('/'), name);
    let body = pm_http_get(&url)?;
    let json: rusty_json_manifest::Value = rusty_json_manifest::from_slice(&body)
        .map_err(|e| ResolverError::Json(format!("{e:?}")))?;
    let versions = json
        .get("versions")
        .and_then(|v| v.as_object())
        .ok_or(ResolverError::MissingField("versions"))?;
    Ok(versions.keys().cloned().collect())
}

fn resolve_exact(registry: &str, name: &str, version: &str) -> Result<ResolvedDep, ResolverError> {
    let url = format!("{}/{}/{}", registry.trim_end_matches('/'), name, version);
    let body = pm_http_get(&url)?;
    let json: rusty_json_manifest::Value = rusty_json_manifest::from_slice(&body)
        .map_err(|e| ResolverError::Json(format!("{e:?}")))?;

    let name_returned = json
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or(ResolverError::MissingField("name"))?;
    let version_returned = json
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or(ResolverError::MissingField("version"))?;
    let dist = json
        .get("dist")
        .ok_or(ResolverError::MissingField("dist"))?;
    let tarball = dist
        .get("tarball")
        .and_then(|v| v.as_str())
        .ok_or(ResolverError::MissingField("dist.tarball"))?;
    validate_tarball_origin(registry, tarball)?;
    let integrity = dist
        .get("integrity")
        .and_then(|v| v.as_str())
        .map(String::from);
    let shasum = dist
        .get("shasum")
        .and_then(|v| v.as_str())
        .map(String::from);

    let mut dependencies = BTreeMap::new();
    if let Some(obj) = json.get("dependencies").and_then(|v| v.as_object()) {
        for (k, v) in obj {
            if let Some(s) = v.as_str() {
                dependencies.insert(k.clone(), s.to_string());
            }
        }
    }
    let mut optional_dependencies = BTreeMap::new();
    if let Some(obj) = json.get("optionalDependencies").and_then(|v| v.as_object()) {
        for (k, v) in obj {
            if let Some(s) = v.as_str() {
                dependencies.remove(k);
                optional_dependencies.insert(k.clone(), s.to_string());
            }
        }
    }

    let mut peer_dependencies = BTreeMap::new();
    if let Some(obj) = json.get("peerDependencies").and_then(|v| v.as_object()) {
        for (k, v) in obj {
            if let Some(s) = v.as_str() {
                peer_dependencies.insert(k.clone(), s.to_string());
            }
        }
    }
    let mut optional_peers = BTreeSet::new();
    if let Some(obj) = json.get("peerDependenciesMeta").and_then(|v| v.as_object()) {
        for (k, v) in obj {
            if v.get("optional").and_then(|o| o.as_bool()).unwrap_or(false) {
                optional_peers.insert(k.clone());
            }
        }
    }

    if name_returned != name {
        return Err(ResolverError::Json(format!(
            "registry returned name={} for requested {}",
            name_returned, name
        )));
    }
    if version_returned != version {
        return Err(ResolverError::Json(format!(
            "registry returned version={} for requested {}",
            version_returned, version
        )));
    }

    let platform_list = |key: &str| {
        json.get(key)
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };
    Ok(ResolvedDep {
        name: name.to_string(),
        version: version.to_string(),
        tarball_url: tarball.to_string(),
        integrity,
        shasum,
        dependencies,
        optional_dependencies,
        os: platform_list("os"),
        cpu: platform_list("cpu"),
        peer_dependencies,
        optional_peers,
        caps: parse_caps(&json),
        publisher: parse_publisher(&json),
    })
}

pub fn fetch_latest_version(registry: &str, name: &str) -> Result<String, ResolverError> {
    let url = format!("{}/{}", registry.trim_end_matches('/'), name);
    let body = pm_http_get(&url)?;
    let json: rusty_json_manifest::Value = rusty_json_manifest::from_slice(&body)
        .map_err(|e| ResolverError::Json(format!("{e:?}")))?;
    let v = json
        .get("dist-tags")
        .and_then(|d| d.get("latest"))
        .and_then(|v| v.as_str())
        .ok_or(ResolverError::MissingField("dist-tags.latest"))?;
    Ok(v.to_string())
}

pub fn resolve_closure(
    registry: &str,
    roots: &[(String, String)],
) -> Result<Vec<Placement>, ResolverError> {

    resolve_closure_prefetch(registry, roots, &mut |_| {}).map(|(placements, _)| placements)
}

fn enqueue_resolution_edge(
    work: &mut VecDeque<(Option<String>, String, String, bool)>,
    edge_count: &mut usize,
    parent: Option<String>,
    name: String,
    range: String,
    optional: bool,
) -> Result<(), ResolverError> {
    if *edge_count >= MAX_RESOLUTION_EDGES {
        return Err(ResolverError::DependencyGraphLimit {
            limit: MAX_RESOLUTION_EDGES,
        });
    }
    *edge_count += 1;
    work.push_back((parent, name, range, optional));
    Ok(())
}

pub fn resolve_closure_prefetch(
    registry: &str,
    roots: &[(String, String)],
    on_resolved: &mut dyn FnMut(&ResolvedDep),
) -> Result<(Vec<Placement>, BTreeSet<String>), ResolverError> {

    let mut constraints: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut chosen: BTreeMap<String, ResolvedDep> = BTreeMap::new();
    let mut packument_cache: BTreeMap<String, rusty_json_manifest::Value> = BTreeMap::new();
    let mut conflicted: BTreeSet<String> = BTreeSet::new();

    let mut peer_demanded: BTreeSet<String> = BTreeSet::new();

    let legacy_peer_deps = cruft_env_present("CRUFT_PM_LEGACY_PEER_DEPS");

    let mut edges: Vec<(Option<String>, String, String)> = Vec::new();

    let mut work: VecDeque<(Option<String>, String, String, bool)> = VecDeque::new();
    let mut edge_count = 0usize;

    for (n, s) in roots {
        enqueue_resolution_edge(
            &mut work,
            &mut edge_count,
            None,
            n.clone(),
            s.clone(),
            false,
        )?;
    }

    loop {
        while !work.is_empty() {
            let mut wave: Vec<(Option<String>, String, String, bool)> = Vec::new();
            while let Some(e) = work.pop_front() {
                wave.push(e);
            }

            let mut names: Vec<String> = Vec::new();
            let mut required: BTreeMap<String, bool> = BTreeMap::new();
            for (parent, name, range, optional) in &wave {
                edges.push((parent.clone(), name.clone(), range.clone()));
                if !optional {
                    required.insert(name.clone(), true);
                } else {
                    required.entry(name.clone()).or_insert(false);
                }
                let v = constraints.entry(name.clone()).or_default();
                if !v.iter().any(|s| s == range) {
                    v.push(range.clone());
                }
                if !names.contains(name) {
                    names.push(name.clone());
                }
            }
            let need: Vec<String> = names
                .iter()
                .filter(|n| !packument_cache.contains_key(*n))
                .cloned()
                .collect();

            let mut fetched = fetch_packuments_parallel(registry, &need);
            fetched.sort_by(|a, b| a.0.cmp(&b.0));
            for (name, res) in fetched {
                match res {
                    Ok(packument) => {
                        packument_cache.insert(name, packument);
                    }
                    Err(error) if required.get(&name).copied().unwrap_or(true) => {
                        return Err(error);
                    }
                    Err(_) => {

                    }
                }
            }
            for name in &names {
                let Some(pack) = packument_cache.get(name) else {
                    continue;
                };
                let ranges = constraints.get(name).cloned().unwrap_or_default();
                let versions = pack
                    .get("versions")
                    .and_then(|v| v.as_object())
                    .ok_or(ResolverError::MissingField("versions"))?;
                let keys: Vec<String> = versions.keys().cloned().collect();

                let latest_pref = pack
                    .get("dist-tags")
                    .and_then(|d| d.get("latest"))
                    .and_then(|v| v.as_str())
                    .filter(|lv| {
                        keys.iter().any(|k| k == *lv)
                            && ranges
                                .iter()
                                .all(|r| crate::semver::satisfies(r, lv).unwrap_or(false))
                    });
                let best = match latest_pref {
                    Some(lv) => lv.to_string(),
                    None => match crate::semver::max_satisfying_all(&ranges, &keys) {
                        Some(v) => v.to_string(),
                        None => {

                            if peer_demanded.contains(name) && !legacy_peer_deps {
                                return Err(ResolverError::PeerConflict {
                                    name: name.clone(),
                                    ranges: ranges.clone(),
                                });
                            }
                            conflicted.insert(name.clone());
                            let pick = crate::semver::version_satisfying_most(&ranges, &keys)
                                .ok_or_else(|| {

                                    let chains: Vec<Vec<(String, String)>> = ranges
                                        .iter()
                                        .map(|r| reconstruct_chain(&edges, name, r))
                                        .collect();
                                    ResolverError::ConflictUnsatisfiable {
                                        name: name.clone(),
                                        ranges: ranges.clone(),
                                        chains,
                                    }
                                })?
                                .to_string();

                            if peer_demanded.contains(name) && legacy_peer_deps {
                                eprintln!(
                                    "cruft warn: peer dependency {name} resolved to {pick} \
                                 (--legacy-peer-deps); not all peer ranges satisfied: {:?}",
                                    ranges
                                );
                            }
                            pick
                        }
                    },
                };
                if chosen.get(name).map(|d| d.version.as_str()) == Some(best.as_str()) {
                    continue;
                }
                let manifest = versions
                    .get(&best)
                    .ok_or(ResolverError::MissingField("versions.<chosen>"))?;
                let resolved = build_resolved_dep(registry, name, &best, manifest)?;
                if !dep_matches_host(&resolved) {
                    if required.get(name).copied().unwrap_or(true) {
                        return Err(ResolverError::UnsupportedPlatform {
                            name: name.clone(),
                            os: resolved.os.clone(),
                            cpu: resolved.cpu.clone(),
                        });
                    }
                    continue;
                }
                let deps = resolved.dependencies.clone();
                let optional_deps = resolved.optional_dependencies.clone();
                on_resolved(&resolved);
                chosen.insert(name.clone(), resolved);
                for (dn, dv) in &deps {
                    enqueue_resolution_edge(
                        &mut work,
                        &mut edge_count,
                        Some(name.clone()),
                        dn.clone(),
                        dv.clone(),
                        false,
                    )?;
                }
                for (dn, dv) in &optional_deps {
                    enqueue_resolution_edge(
                        &mut work,
                        &mut edge_count,
                        Some(name.clone()),
                        dn.clone(),
                        dv.clone(),
                        true,
                    )?;
                }
            }
        }

        if cruft_env_present("CRUFT_PM_NO_PEERS") {
            break;
        }
        let mut to_inject: BTreeSet<(String, String)> = BTreeSet::new();
        for dep in chosen.values() {
            for (peer, range) in &dep.peer_dependencies {
                if dep.optional_peers.contains(peer) {
                    continue;
                }
                if chosen.contains_key(peer) {
                    continue;
                }
                to_inject.insert((peer.clone(), range.clone()));
            }
        }
        if to_inject.is_empty() {
            break;
        }
        for (peer, range) in to_inject {
            peer_demanded.insert(peer.clone());
            enqueue_resolution_edge(&mut work, &mut edge_count, None, peer, range, false)?;
        }
    }

    let mut placements: Vec<Placement> = chosen
        .values()
        .map(|dep| Placement {
            dep: dep.clone(),
            nest_under: Vec::new(),
        })
        .collect();

    let cache_cell = std::cell::RefCell::new(packument_cache);
    let nested_items = {
        let ensure_cached = |n: &str| {
            if !cache_cell.borrow().contains_key(n) {
                if let Ok(p) = fetch_packument(registry, n) {
                    cache_cell.borrow_mut().insert(n.to_string(), p);
                }
            }
        };
        let chosen_ver = |n: &str| chosen.get(n).map(|d| d.version.clone());
        let versions_of = |n: &str| -> Vec<String> {
            ensure_cached(n);
            cache_cell
                .borrow()
                .get(n)
                .and_then(|p| {
                    p.get("versions")
                        .and_then(|v| v.as_object())
                        .map(|o| o.keys().cloned().collect::<Vec<String>>())
                })
                .unwrap_or_default()
        };
        let deps_of = |n: &str, v: &str| -> Vec<(String, String)> {
            ensure_cached(n);
            cache_cell
                .borrow()
                .get(n)
                .and_then(|p| p.get("versions").and_then(|vs| vs.get(v)))
                .and_then(|m| m.get("dependencies").and_then(|d| d.as_object()).cloned())
                .map(|o| {
                    o.iter()
                        .filter_map(|(k, val)| val.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect()
                })
                .unwrap_or_default()
        };
        compute_nested_placements(&conflicted, &chosen_ver, &edges, &versions_of, &deps_of)
    };
    let packument_cache = cache_cell.into_inner();
    for (ancestor, name, version) in nested_items {
        let versions = match packument_cache
            .get(&name)
            .and_then(|p| p.get("versions").and_then(|v| v.as_object()))
        {
            Some(v) => v,
            None => continue,
        };
        let manifest = match versions.get(&version) {
            Some(m) => m,
            None => continue,
        };
        let nresolved = build_resolved_dep(registry, &name, &version, manifest)?;
        if !dep_matches_host(&nresolved) {
            continue;
        }
        placements.push(Placement {
            dep: nresolved,
            nest_under: ancestor,
        });
    }

    placements.sort_by(|a, b| {
        a.dep
            .name
            .cmp(&b.dep.name)
            .then_with(|| a.dep.version.cmp(&b.dep.version))
            .then_with(|| a.nest_under.cmp(&b.nest_under))
    });

    Ok((placements, peer_demanded))
}

pub fn canonical_resolve_key(roots: &[(String, String)]) -> String {
    use std::collections::{BTreeMap, BTreeSet};

    let mut by_name: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (name, range) in roots {
        let canon = match crate::semver::Range::parse(range) {
            Ok(r) => r.canonical_string(),
            Err(_) => range.clone(),
        };
        by_name.entry(name.clone()).or_default().insert(canon);
    }

    let mut buf = String::new();
    for (name, ranges) in &by_name {
        buf.push_str(name);
        buf.push('@');
        let joined: Vec<&str> = ranges.iter().map(String::as_str).collect();
        buf.push_str(&joined.join(","));
        buf.push(';');
    }

    rusty_js_pm_integrity::canonical_sha256_hex(buf.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_axis_honors_allow_and_deny_entries() {
        assert!(platform_axis_matches(&[], "linux"));
        assert!(platform_axis_matches(
            &["linux".into(), "darwin".into()],
            "linux"
        ));
        assert!(!platform_axis_matches(&["darwin".into()], "linux"));
        assert!(!platform_axis_matches(
            &["!linux".into(), "linux".into()],
            "linux"
        ));
        assert!(platform_axis_matches(&["!darwin".into()], "linux"));
    }

    #[test]
    fn resolution_edge_limit_rejects_hostile_graph_growth() {
        let mut work = VecDeque::new();
        let mut edge_count = MAX_RESOLUTION_EDGES;
        assert!(matches!(
            enqueue_resolution_edge(
                &mut work,
                &mut edge_count,
                None,
                "pkg".to_string(),
                "1.0.0".to_string(),
                false,
            ),
            Err(ResolverError::DependencyGraphLimit { .. })
        ));
    }

    #[test]
    fn optional_dependencies_override_ordinary_dependencies() {
        let manifest = rusty_json_manifest::json!({
            "dist": { "tarball": "https://registry.example/pkg/-/pkg-1.0.0.tgz" },
            "dependencies": { "native": "1.0.0", "ordinary": "1.0.0" },
            "optionalDependencies": { "native": "2.0.0" }
        });
        let dep =
            build_resolved_dep("https://registry.example", "pkg", "1.0.0", &manifest).unwrap();
        assert_eq!(
            dep.optional_dependencies.get("native"),
            Some(&"2.0.0".to_string())
        );
        assert!(!dep.dependencies.contains_key("native"));
        assert_eq!(dep.dependencies.get("ordinary"), Some(&"1.0.0".to_string()));
    }

    #[test]
    fn registry_tarball_origin_must_match_registry_authority() {
        let same = rusty_json_manifest::json!({
            "dist": { "tarball": "https://registry.example/pkg/-/pkg-1.0.0.tgz" }
        });
        let dep = build_resolved_dep("https://registry.example/", "pkg", "1.0.0", &same).unwrap();
        assert_eq!(
            dep.tarball_url,
            "https://registry.example/pkg/-/pkg-1.0.0.tgz"
        );

        let port_match = rusty_json_manifest::json!({
            "dist": { "tarball": "https://registry.example:8443/pkg.tgz" }
        });
        assert!(
            build_resolved_dep("https://registry.example:8443", "pkg", "1.0.0", &port_match)
                .is_ok()
        );

        let cross_origin = rusty_json_manifest::json!({
            "dist": { "tarball": "https://evil.example/pkg.tgz" }
        });
        assert!(matches!(
            build_resolved_dep("https://registry.example", "pkg", "1.0.0", &cross_origin),
            Err(ResolverError::TarballOrigin { .. })
        ));

        let http_tarball = rusty_json_manifest::json!({
            "dist": { "tarball": "http://registry.example/pkg.tgz" }
        });
        assert!(matches!(
            build_resolved_dep("https://registry.example", "pkg", "1.0.0", &http_tarball),
            Err(ResolverError::Json(_))
        ));
    }

    #[test]
    fn exact_pin_detection() {
        assert!(crate::semver::is_exact_pin("4.17.21"));
        assert!(!crate::semver::is_exact_pin("^4.17.21"));
        assert!(!crate::semver::is_exact_pin("~4.17.21"));
        assert!(!crate::semver::is_exact_pin("1.x"));
        assert!(!crate::semver::is_exact_pin("*"));
    }

    #[test]
    fn parse_caps_reads_declared_classes() {
        let manifest = rusty_json_manifest::json!({
            "caps": { "net": ["registry.npmjs.org"], "fs": ["./cache"], "env": [], "exec": ["git"] }
        });
        let caps = parse_caps(&manifest);
        assert_eq!(caps.net, vec!["registry.npmjs.org".to_string()]);
        assert_eq!(caps.fs, vec!["./cache".to_string()]);
        assert!(caps.env.is_empty());
        assert_eq!(caps.exec, vec!["git".to_string()]);
        assert!(!caps.is_empty());
    }

    #[test]
    fn parse_caps_absent_is_empty() {
        let manifest = rusty_json_manifest::json!({ "name": "leaf", "version": "1.0.0" });
        assert!(parse_caps(&manifest).is_empty());
    }

    #[test]
    fn parse_publisher_from_npm_user_then_maintainers() {
        let a = rusty_json_manifest::json!({ "_npmUser": { "name": "alice" } });
        assert_eq!(parse_publisher(&a), Some("alice".to_string()));
        let b = rusty_json_manifest::json!({ "maintainers": [{ "name": "bob" }] });
        assert_eq!(parse_publisher(&b), Some("bob".to_string()));
        let c = rusty_json_manifest::json!({ "name": "leaf" });
        assert_eq!(parse_publisher(&c), None);
    }

    #[test]
    fn bad_range_rejected_before_network() {

        let r = resolve_range(DEFAULT_REGISTRY, "lodash", "^^bogus");
        assert!(matches!(r, Err(ResolverError::BadRange(_))));
    }

    #[test]
    fn canonical_key_stable_under_root_order() {

        let a = vec![
            ("lodash".to_string(), "^4.17.0".to_string()),
            ("axios".to_string(), "^1.0.0".to_string()),
            ("debug".to_string(), "~4.3.0".to_string()),
        ];
        let mut b = a.clone();
        b.reverse();
        assert_eq!(
            canonical_resolve_key(&a),
            canonical_resolve_key(&b),
            "root order must not change the canonical key"
        );
    }

    #[test]
    fn canonical_key_range_equivalence() {

        let sugar = vec![("lodash".to_string(), "^1.2.3".to_string())];
        let desugared = vec![("lodash".to_string(), ">=1.2.3 <2.0.0".to_string())];
        assert_eq!(
            canonical_resolve_key(&sugar),
            canonical_resolve_key(&desugared),
            "^1.2.3 and >=1.2.3 <2.0.0 must canonicalize identically"
        );

        let reordered = vec![("lodash".to_string(), "<2.0.0 >=1.2.3".to_string())];
        assert_eq!(
            canonical_resolve_key(&sugar),
            canonical_resolve_key(&reordered),
            "comparator order within a set must not change the key"
        );
    }

    #[test]
    fn canonical_key_merges_dup_names_deterministically() {

        let a = vec![
            ("react".to_string(), "^18.0.0".to_string()),
            ("react".to_string(), "^18.2.0".to_string()),
        ];
        let mut b = a.clone();
        b.reverse();
        assert_eq!(canonical_resolve_key(&a), canonical_resolve_key(&b));

        assert_eq!(canonical_resolve_key(&a).len(), 64);
    }

    #[test]
    fn placement_sort_order_independent() {

        fn mk(name: &str, version: &str, nest: Vec<&str>) -> Placement {
            Placement {
                dep: ResolvedDep {
                    name: name.to_string(),
                    version: version.to_string(),
                    tarball_url: String::new(),
                    integrity: None,
                    shasum: None,
                    dependencies: BTreeMap::new(),
                    optional_dependencies: BTreeMap::new(),
                    os: Vec::new(),
                    cpu: Vec::new(),
                    peer_dependencies: BTreeMap::new(),
                    optional_peers: BTreeSet::new(),
                    caps: Caps::default(),
                    publisher: None,
                },
                nest_under: nest.into_iter().map(String::from).collect(),
            }
        }
        let sort = |mut v: Vec<Placement>| -> Vec<(String, String, Vec<String>)> {
            v.sort_by(|a, b| {
                a.dep
                    .name
                    .cmp(&b.dep.name)
                    .then_with(|| a.dep.version.cmp(&b.dep.version))
                    .then_with(|| a.nest_under.cmp(&b.nest_under))
            });
            v.into_iter()
                .map(|p| (p.dep.name, p.dep.version, p.nest_under))
                .collect()
        };
        let order1 = vec![
            mk("b", "2.0.0", vec![]),
            mk("a", "1.0.0", vec![]),
            mk("a", "1.0.0", vec!["b"]),
        ];
        let mut order2 = order1.clone();
        order2.reverse();
        assert_eq!(sort(order1), sort(order2));
    }

    #[test]
    fn deep_nested_transitive_resolution() {

        let conflicted: BTreeSet<String> = ["app".to_string()].into_iter().collect();
        let chosen_ver = |n: &str| match n {
            "app" => Some("1.0.0".to_string()),
            "util" => Some("3.0.0".to_string()),
            _ => None,
        };

        let edges: Vec<(Option<String>, String, String)> = vec![
            (None, "app".to_string(), "^1".to_string()),
            (None, "legacy".to_string(), "1.0.0".to_string()),
            (
                Some("legacy".to_string()),
                "app".to_string(),
                "^0.9".to_string(),
            ),
        ];
        let versions_of = |n: &str| -> Vec<String> {
            match n {
                "app" => vec!["0.9.0".to_string(), "1.0.0".to_string()],
                "util" => vec!["2.5.0".to_string(), "3.0.0".to_string()],
                _ => vec![],
            }
        };
        let deps_of = |n: &str, v: &str| -> Vec<(String, String)> {
            match (n, v) {

                ("app", "0.9.0") => vec![("util".to_string(), "^2".to_string())],
                _ => vec![],
            }
        };
        let out =
            compute_nested_placements(&conflicted, &chosen_ver, &edges, &versions_of, &deps_of);

        assert!(
            out.contains(&(
                vec!["legacy".to_string()],
                "app".to_string(),
                "0.9.0".to_string()
            )),
            "app@0.9.0 must nest under legacy; got {out:?}"
        );
        assert!(
            out.contains(&(
                vec!["legacy".to_string(), "app".to_string()],
                "util".to_string(),
                "2.5.0".to_string()
            )),
            "util@2.5.0 must nest one level deeper under legacy/app; got {out:?}"
        );
    }

    #[test]
    fn deep_nested_shares_compatible_hoist() {

        let conflicted: BTreeSet<String> = ["app".to_string()].into_iter().collect();
        let chosen_ver = |n: &str| match n {
            "app" => Some("1.0.0".to_string()),
            "util" => Some("3.0.0".to_string()),
            _ => None,
        };
        let edges: Vec<(Option<String>, String, String)> = vec![
            (None, "app".to_string(), "^1".to_string()),
            (
                Some("legacy".to_string()),
                "app".to_string(),
                "^0.9".to_string(),
            ),
        ];
        let versions_of = |n: &str| -> Vec<String> {
            match n {
                "app" => vec!["0.9.0".to_string(), "1.0.0".to_string()],
                "util" => vec!["2.5.0".to_string(), "3.0.0".to_string()],
                _ => vec![],
            }
        };
        let deps_of = |n: &str, v: &str| -> Vec<(String, String)> {
            match (n, v) {

                ("app", "0.9.0") => vec![("util".to_string(), "^3".to_string())],
                _ => vec![],
            }
        };
        let out =
            compute_nested_placements(&conflicted, &chosen_ver, &edges, &versions_of, &deps_of);
        assert!(out
            .iter()
            .any(|(a, n, v)| a == &vec!["legacy".to_string()] && n == "app" && v == "0.9.0"));
        assert!(
            !out.iter().any(|(_, n, _)| n == "util"),
            "util is satisfied by the hoist and must NOT get a nested copy; got {out:?}"
        );
    }

    #[test]
    fn conflict_chain_reconstruction() {

        let edges: Vec<(Option<String>, String, String)> = vec![
            (None, "x".to_string(), "^1".to_string()),
            (None, "a".to_string(), "^1".to_string()),
            (Some("x".to_string()), "z".to_string(), "1.x".to_string()),
            (Some("a".to_string()), "z".to_string(), "2.x".to_string()),
        ];
        let c1 = reconstruct_chain(&edges, "z", "1.x");

        assert_eq!(
            c1,
            vec![
                ("<root>".to_string(), "x@^1".to_string()),
                ("x".to_string(), "z@1.x".to_string()),
            ],
            "chain for z@1.x"
        );
        let c2 = reconstruct_chain(&edges, "z", "2.x");
        assert_eq!(
            c2,
            vec![
                ("<root>".to_string(), "a@^1".to_string()),
                ("a".to_string(), "z@2.x".to_string()),
            ],
            "chain for z@2.x"
        );
    }

    #[test]
    fn conflict_unsatisfiable_display_renders_chains() {
        let err = ResolverError::ConflictUnsatisfiable {
            name: "z".to_string(),
            ranges: vec!["1.x".to_string(), "2.x".to_string()],
            chains: vec![
                vec![
                    ("<root>".to_string(), "x@^1".to_string()),
                    ("x".to_string(), "z@1.x".to_string()),
                ],
                vec![
                    ("<root>".to_string(), "a@^1".to_string()),
                    ("a".to_string(), "z@2.x".to_string()),
                ],
            ],
        };
        let s = format!("{err}");
        assert!(s.contains("unsatisfiable version conflict for `z`"), "{s}");
        assert!(s.contains("[1.x, 2.x]"), "{s}");
        assert!(s.contains("x requires z@1.x"), "{s}");
        assert!(s.contains("<root> requires a@^1 → a requires z@2.x"), "{s}");
    }

    #[test]
    fn nest_under_location_string_roundtrip() {

        let chain = vec!["legacy".to_string(), "app".to_string()];
        let encoded = chain.join("/");
        assert_eq!(encoded, "legacy/app");
        let decoded: Vec<String> = encoded.split('/').map(String::from).collect();
        assert_eq!(decoded, chain);

        assert_eq!(Vec::<String>::new().join("/"), "");
    }

    #[test]
    #[ignore]
    fn resolve_caret_range_picks_greatest_satisfying() {

        let r = resolve_specifier(DEFAULT_REGISTRY, "lodash", "^4.17.0")
            .expect("caret range should resolve");
        assert_eq!(r.name, "lodash");
        assert!(
            r.version.starts_with("4."),
            "expected a 4.x version, got {}",
            r.version
        );
    }

    #[test]
    #[ignore]
    fn recon_8_rangeat_packages() {
        let pkgs = [
            ("debug", "^4.3.0"),
            ("axios", "^1.0.0"),
            ("express", "^4.18.0"),
            ("yargs", "^17.0.0"),
            ("glob", "^10.0.0"),
            ("rimraf", "^5.0.0"),
            ("fs-extra", "^11.0.0"),
            ("prop-types", "^15.8.0"),
        ];
        let mut flipped = 0;
        for (name, range) in pkgs {
            match resolve_specifier(DEFAULT_REGISTRY, name, range) {
                Ok(r) => {
                    eprintln!("FLIP {name}{range} -> {}@{}", r.name, r.version);
                    flipped += 1;
                }
                Err(e) => eprintln!("MISS {name}{range}: {e:?}"),
            }
        }
        eprintln!("recon flip count: {flipped}/8");
        assert!(
            flipped >= 1,
            "expected at least one RangeAt package to resolve"
        );
    }

    #[test]
    #[ignore]
    fn closure_lodash_is_leaf() {
        let roots = vec![("lodash".to_string(), "4.17.21".to_string())];
        let closure = resolve_closure(DEFAULT_REGISTRY, &roots).expect("closure");
        assert_eq!(closure.len(), 1, "lodash 4.17.21 is zero-transitive");
        assert_eq!(closure[0].dep.name, "lodash");
        assert!(closure[0].dep.dependencies.is_empty());
    }

    #[test]
    #[ignore]
    fn closure_probe_small_transitive() {

        let roots = vec![("debug".to_string(), "4.3.4".to_string())];
        let result = resolve_closure(DEFAULT_REGISTRY, &roots);
        match result {
            Ok(closure) => {
                eprintln!("debug@4.3.4 closure: {} packages", closure.len());
                for r in &closure {
                    eprintln!("  {}@{}", r.dep.name, r.dep.version);
                }
                assert!(closure.iter().any(|r| r.dep.name == "debug"));
            }
            Err(ResolverError::NonExactVersionSpec(v)) => {
                eprintln!("debug@4.3.4 transitive surfaced range: {v}");
            }
            Err(e) => panic!("unexpected error: {e:?}"),
        }
    }

    #[test]
    #[ignore]
    fn resolve_lodash_4_17_21() {
        let r = resolve_specifier(DEFAULT_REGISTRY, "lodash", "4.17.21")
            .expect("lodash 4.17.21 should resolve via npmmirror.com");
        assert_eq!(r.name, "lodash");
        assert_eq!(r.version, "4.17.21");
        assert!(
            r.tarball_url.ends_with("lodash-4.17.21.tgz"),
            "unexpected tarball URL: {}",
            r.tarball_url
        );

        assert!(
            r.integrity.is_some() || r.shasum.is_some(),
            "neither integrity nor shasum present"
        );
    }
}
