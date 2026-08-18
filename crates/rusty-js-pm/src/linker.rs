
use std::path::{Path, PathBuf};

use crate::fetcher::FetchedPackage;
use crate::resolver::ResolvedDep;

#[derive(Debug)]
pub enum LinkError {
    InvalidPackageName(String),
    Io(String),
}

#[derive(Debug)]
pub struct LinkedPackage {
    pub install_dir: PathBuf,
}

pub fn link_package(
    dep: &ResolvedDep,
    pkg: FetchedPackage,
    node_modules_root: &Path,
) -> Result<LinkedPackage, LinkError> {
    let install_dir = resolve_install_path(node_modules_root, &dep.name)?;

    if let Some(parent) = install_dir.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| LinkError::Io(format!("mkdir {parent:?}: {e}")))?;
    }
    if install_dir.exists() {
        std::fs::remove_dir_all(&install_dir)
            .map_err(|e| LinkError::Io(format!("rm {install_dir:?}: {e}")))?;
    }

    match std::fs::rename(&pkg.staging_dir, &install_dir) {
        Ok(()) => {}
        Err(e) if is_cross_device(&e) => {
            copy_dir_recursive(&pkg.staging_dir, &install_dir)?;
            std::fs::remove_dir_all(&pkg.staging_dir)
                .map_err(|e| LinkError::Io(format!("rm staging {:?}: {e}", pkg.staging_dir)))?;
        }
        Err(e) => {
            return Err(LinkError::Io(format!(
                "rename {:?} → {:?}: {e}",
                pkg.staging_dir, install_dir
            )))
        }
    }

    Ok(LinkedPackage { install_dir })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterializeMode {

    Link,

    Copy,
}

pub fn link_from_store(
    dep: &ResolvedDep,
    store_dir: &Path,
    node_modules_root: &Path,
) -> Result<LinkedPackage, LinkError> {
    materialize_from_store(dep, store_dir, node_modules_root, MaterializeMode::Link)
}

pub fn materialize_from_store(
    dep: &ResolvedDep,
    store_dir: &Path,
    node_modules_root: &Path,
    mode: MaterializeMode,
) -> Result<LinkedPackage, LinkError> {
    let install_dir = resolve_install_path(node_modules_root, &dep.name)?;
    if let Some(parent) = install_dir.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| LinkError::Io(format!("mkdir {parent:?}: {e}")))?;
    }
    if install_dir.exists() {
        std::fs::remove_dir_all(&install_dir)
            .map_err(|e| LinkError::Io(format!("rm {install_dir:?}: {e}")))?;
    }
    materialize_dir_recursive(store_dir, &install_dir, mode)?;
    Ok(LinkedPackage { install_dir })
}

fn materialize_dir_recursive(
    src: &Path,
    dst: &Path,
    mode: MaterializeMode,
) -> Result<(), LinkError> {
    std::fs::create_dir_all(dst).map_err(|e| LinkError::Io(format!("mkdir {dst:?}: {e}")))?;
    for entry in
        std::fs::read_dir(src).map_err(|e| LinkError::Io(format!("readdir {src:?}: {e}")))?
    {
        let entry = entry.map_err(|e| LinkError::Io(format!("dirent {src:?}: {e}")))?;
        let name = entry.file_name();
        if name == ".cruft-store-complete" {
            continue;
        }
        let from = entry.path();
        let to = dst.join(&name);
        let ft = entry
            .file_type()
            .map_err(|e| LinkError::Io(format!("filetype {from:?}: {e}")))?;
        if ft.is_dir() {
            materialize_dir_recursive(&from, &to, mode)?;
        } else {
            match mode {
                MaterializeMode::Copy => {
                    std::fs::copy(&from, &to)
                        .map_err(|e| LinkError::Io(format!("copy {from:?} → {to:?}: {e}")))?;
                }
                MaterializeMode::Link => match std::fs::hard_link(&from, &to) {
                    Ok(()) => {}
                    Err(_) => {

                        std::fs::copy(&from, &to)
                            .map_err(|e| LinkError::Io(format!("copy {from:?} → {to:?}: {e}")))?;
                    }
                },
            }
        }
    }
    Ok(())
}

fn resolve_install_path(root: &Path, name: &str) -> Result<PathBuf, LinkError> {
    if name.is_empty() || name == "." || name == ".." || name.contains('\0') {
        return Err(LinkError::InvalidPackageName(name.to_string()));
    }
    if let Some(rest) = name.strip_prefix('@') {
        let slash = rest
            .find('/')
            .ok_or_else(|| LinkError::InvalidPackageName(name.to_string()))?;
        let scope = &rest[..slash];
        let bare = &rest[slash + 1..];
        if scope.is_empty() || bare.is_empty() || bare.contains('/') {
            return Err(LinkError::InvalidPackageName(name.to_string()));
        }
        Ok(root.join(format!("@{scope}")).join(bare))
    } else {
        if name.contains('/') {
            return Err(LinkError::InvalidPackageName(name.to_string()));
        }
        Ok(root.join(name))
    }
}

fn is_cross_device(e: &std::io::Error) -> bool {

    matches!(e.raw_os_error(), Some(18))
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), LinkError> {
    std::fs::create_dir_all(dst).map_err(|e| LinkError::Io(format!("mkdir {dst:?}: {e}")))?;
    for entry in
        std::fs::read_dir(src).map_err(|e| LinkError::Io(format!("readdir {src:?}: {e}")))?
    {
        let entry = entry.map_err(|e| LinkError::Io(format!("{e}")))?;
        let ty = entry
            .file_type()
            .map_err(|e| LinkError::Io(format!("{e}")))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else if ty.is_file() {
            std::fs::copy(&from, &to)
                .map_err(|e| LinkError::Io(format!("copy {from:?} → {to:?}: {e}")))?;
        } else {

            return Err(LinkError::Io(format!("unexpected entry type at {from:?}")));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp(suffix: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "cruft-pm-link-{}-{}",
            suffix,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        p
    }

    #[test]
    fn install_path_bare() {
        let p = resolve_install_path(Path::new("/r"), "lodash").unwrap();
        assert_eq!(p, PathBuf::from("/r/lodash"));
    }

    #[test]
    fn install_path_scoped() {
        let p = resolve_install_path(Path::new("/r"), "@babel/core").unwrap();
        assert_eq!(p, PathBuf::from("/r/@babel/core"));
    }

    #[test]
    fn install_path_rejects_bare_with_slash() {
        assert!(resolve_install_path(Path::new("/r"), "foo/bar").is_err());
    }

    #[test]
    fn install_path_rejects_scoped_without_slash() {
        assert!(resolve_install_path(Path::new("/r"), "@foo").is_err());
    }

    #[test]
    fn install_path_rejects_empty_scope() {
        assert!(resolve_install_path(Path::new("/r"), "@/bar").is_err());
    }

    #[test]
    fn install_path_rejects_double_slash_in_scoped() {
        assert!(resolve_install_path(Path::new("/r"), "@a/b/c").is_err());
    }

    #[test]
    fn link_smoke_same_fs() {
        let staging = tmp("staging");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(
            staging.join("package.json"),
            br#"{"name":"x","version":"0.0.1"}"#,
        )
        .unwrap();
        std::fs::create_dir_all(staging.join("lib")).unwrap();
        std::fs::write(staging.join("lib/index.js"), b"module.exports=1;").unwrap();

        let root = tmp("nm");
        let dep = ResolvedDep {
            name: "x".into(),
            version: "0.0.1".into(),
            tarball_url: "ignored".into(),
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
        let pkg = FetchedPackage {
            staging_dir: staging.clone(),
            file_count: 2,
        };
        let linked = link_package(&dep, pkg, &root).expect("link");

        assert!(linked.install_dir.join("package.json").exists());
        assert!(linked.install_dir.join("lib/index.js").exists());
        assert!(!staging.exists(), "staging should be moved away");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn link_overwrites_existing() {
        let root = tmp("nm-ow");
        std::fs::create_dir_all(root.join("x")).unwrap();
        std::fs::write(root.join("x/stale.txt"), b"old").unwrap();

        let staging = tmp("staging-ow");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("fresh.txt"), b"new").unwrap();

        let dep = ResolvedDep {
            name: "x".into(),
            version: "0.0.2".into(),
            tarball_url: "ignored".into(),
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
        let pkg = FetchedPackage {
            staging_dir: staging,
            file_count: 1,
        };
        let linked = link_package(&dep, pkg, &root).unwrap();

        assert!(linked.install_dir.join("fresh.txt").exists());
        assert!(!linked.install_dir.join("stale.txt").exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    #[ignore]
    fn install_lodash_end_to_end() {
        use crate::fetcher::fetch_and_extract;
        use crate::resolver::{resolve_specifier, DEFAULT_REGISTRY};

        let dep = resolve_specifier(DEFAULT_REGISTRY, "lodash", "4.17.21").expect("resolve");
        let staging = tmp("lodash-stage");
        let pkg = fetch_and_extract(&dep, &staging).expect("fetch");

        let root = tmp("lodash-nm");
        let linked = link_package(&dep, pkg, &root).expect("link");

        let pj = linked.install_dir.join("package.json");
        let body = std::fs::read_to_string(&pj).unwrap();
        assert!(body.contains("\"version\": \"4.17.21\""));
        assert!(linked.install_dir.ends_with("lodash"));

        let _ = std::fs::remove_dir_all(&root);
    }
}
