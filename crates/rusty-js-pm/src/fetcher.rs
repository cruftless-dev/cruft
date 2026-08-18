
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use crate::http::{pm_http_get_follow, HttpError};
use crate::integrity::{verify_shasum, verify_sri, IntegrityError};
use crate::resolver::ResolvedDep;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

fn cruft_env_present(name: &str) -> bool {
    std::env::var(name).is_ok()
        || name
            .strip_prefix("CRUFT_")
            .is_some_and(|rest| std::env::var(format!("CRUFTLESS_{rest}")).is_ok())
}

pub static FETCH_DL_NS: AtomicU64 = AtomicU64::new(0);
pub static FETCH_VERIFY_NS: AtomicU64 = AtomicU64::new(0);
pub static FETCH_EXTRACT_NS: AtomicU64 = AtomicU64::new(0);
pub static FETCH_COUNT: AtomicU64 = AtomicU64::new(0);
pub static FETCH_BYTES: AtomicU64 = AtomicU64::new(0);

pub const MAX_TARBALL_BYTES: usize = 64 * 1024 * 1024;

pub fn fetch_profile_report() -> String {
    let ms = |ns: u64| ns / 1_000_000;
    format!(
        "[pm-profile]   tarball fetch: {} pkgs, {} KiB | download {}ms, verify {}ms, gunzip+untar {}ms (summed across workers)",
        FETCH_COUNT.load(AtomicOrdering::Relaxed),
        FETCH_BYTES.load(AtomicOrdering::Relaxed) / 1024,
        ms(FETCH_DL_NS.load(AtomicOrdering::Relaxed)),
        ms(FETCH_VERIFY_NS.load(AtomicOrdering::Relaxed)),
        ms(FETCH_EXTRACT_NS.load(AtomicOrdering::Relaxed)),
    )
}

#[derive(Debug)]
pub enum FetchError {
    Http(HttpError),
    NoIntegrity,
    WeakLegacyShasum,
    TarballTooLarge { len: usize, limit: usize },
    Integrity(IntegrityError),
    Io(String),
    UnsafePath(String),
    UnsafeEntryKind(String),
    Tar(String),
}

impl From<HttpError> for FetchError {
    fn from(e: HttpError) -> Self {
        FetchError::Http(e)
    }
}

#[derive(Debug)]
pub struct FetchedPackage {
    pub staging_dir: PathBuf,
    pub file_count: usize,
}

pub fn fetch_and_extract(
    dep: &ResolvedDep,
    staging_dir: &Path,
) -> Result<FetchedPackage, FetchError> {
    if dep.integrity.is_none() && dep.shasum.is_none() {
        return Err(FetchError::NoIntegrity);
    }

    let bytes = pm_http_get_follow(&dep.tarball_url, 5)?;
    check_tarball_size(bytes.len())?;

    verify_and_extract_bytes(dep, &bytes, staging_dir)
}

fn verify_and_extract_bytes(
    dep: &ResolvedDep,
    bytes: &[u8],
    staging_dir: &Path,
) -> Result<FetchedPackage, FetchError> {
    check_tarball_size(bytes.len())?;
    verify_bytes(dep, &bytes)?;

    std::fs::create_dir_all(staging_dir)
        .map_err(|e| FetchError::Io(format!("create staging {staging_dir:?}: {e}")))?;
    let count = extract_tar_into(bytes, staging_dir)?;

    Ok(FetchedPackage {
        staging_dir: staging_dir.to_path_buf(),
        file_count: count,
    })
}

fn check_tarball_size(len: usize) -> Result<(), FetchError> {
    if len > MAX_TARBALL_BYTES {
        return Err(FetchError::TarballTooLarge {
            len,
            limit: MAX_TARBALL_BYTES,
        });
    }
    Ok(())
}

fn verify_bytes(dep: &ResolvedDep, bytes: &[u8]) -> Result<(), FetchError> {
    if let Some(sri) = &dep.integrity {
        verify_sri(bytes, sri).map_err(FetchError::Integrity)?;
    } else if let Some(shasum) = &dep.shasum {
        if !allow_legacy_sha1_shasum() {
            return Err(FetchError::WeakLegacyShasum);
        }
        verify_shasum(bytes, shasum).map_err(FetchError::Integrity)?;
    }
    Ok(())
}

fn allow_legacy_sha1_shasum() -> bool {
    cruft_env_present("CRUFT_PM_ALLOW_SHA1_SHASUM")
}

fn extract_tar_into(bytes: &[u8], dest: &Path) -> Result<usize, FetchError> {

    let decompressed =
        rusty_compression::gunzip(bytes).map_err(|e| FetchError::Tar(format!("gunzip: {e:?}")))?;
    let entries =
        rusty_js_tar::parse_archive(&decompressed).map_err(|e| FetchError::Tar(format!("{e}")))?;
    let alternate_root = common_alternate_root(&entries);
    let mut count = 0usize;
    for entry in entries {
        if let rusty_js_tar::EntryKind::Other(kind) = entry.kind {
            if matches!(kind, b'1' | b'2') {
                return Err(FetchError::UnsafeEntryKind(format!(
                    "{}",
                    entry.path.display()
                )));
            }

            continue;
        }
        let safe = sanitize_entry_path_with_root(&entry.path, alternate_root.as_ref())?;

        if safe.as_os_str().is_empty() {
            continue;
        }
        let d = dest.join(&safe);
        if entry.kind.is_dir() {
            std::fs::create_dir_all(&d).map_err(|e| FetchError::Io(format!("mkdir {d:?}: {e}")))?;
            continue;
        }
        if let Some(parent) = d.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| FetchError::Io(format!("mkdir {parent:?}: {e}")))?;
        }
        let mut out =
            std::fs::File::create(&d).map_err(|e| FetchError::Io(format!("create {d:?}: {e}")))?;
        std::io::copy(&mut entry.data.as_slice(), &mut out)
            .map_err(|e| FetchError::Io(format!("write {d:?}: {e}")))?;
        count += 1;
    }
    Ok(count)
}

fn common_alternate_root(entries: &[rusty_js_tar::Entry]) -> Option<OsString> {
    let mut root: Option<OsString> = None;
    for entry in entries {
        if !entry.kind.is_file() && !entry.kind.is_dir() {
            continue;
        }
        let Some(Component::Normal(first)) = entry.path.components().next() else {
            continue;
        };
        if first == "package" {
            return None;
        }
        match &root {
            Some(candidate) if candidate != first => return None,
            Some(_) => {}
            None => root = Some(first.to_os_string()),
        }
    }
    root
}

#[derive(Debug)]
pub struct StorePackage {
    pub store_dir: PathBuf,

    pub from_cache: bool,
    pub file_count: usize,
    pub tarball_bytes: Option<usize>,
}

pub fn fetch_into_store(dep: &ResolvedDep) -> Result<StorePackage, FetchError> {
    if dep.integrity.is_none() && dep.shasum.is_none() {
        return Err(FetchError::NoIntegrity);
    }
    let key = crate::store::content_key(dep.integrity.as_deref(), dep.shasum.as_deref())
        .ok_or(FetchError::NoIntegrity)?;
    let entry = crate::store::entry_dir(&key);
    if crate::store::is_present(&key) {
        let file_count = count_regular_files(&entry)?;
        return Ok(StorePackage {
            store_dir: entry,
            from_cache: true,
            file_count,
            tarball_bytes: None,
        });
    }
    let prof = cruft_env_present("CRUFT_PM_PROFILE");
    let t = std::time::Instant::now();
    let bytes = pm_http_get_follow(&dep.tarball_url, 5)?;
    check_tarball_size(bytes.len())?;
    if prof {
        FETCH_DL_NS.fetch_add(t.elapsed().as_nanos() as u64, AtomicOrdering::Relaxed);
        FETCH_BYTES.fetch_add(bytes.len() as u64, AtomicOrdering::Relaxed);
        FETCH_COUNT.fetch_add(1, AtomicOrdering::Relaxed);
    }
    let t = std::time::Instant::now();
    verify_bytes(dep, &bytes)?;
    if prof {
        FETCH_VERIFY_NS.fetch_add(t.elapsed().as_nanos() as u64, AtomicOrdering::Relaxed);
    }
    let tmp = crate::store::temp_entry_dir(&key);
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp)
        .map_err(|e| FetchError::Io(format!("create store tmp {tmp:?}: {e}")))?;
    let t = std::time::Instant::now();
    let file_count = extract_tar_into(&bytes, &tmp)?;
    if prof {
        FETCH_EXTRACT_NS.fetch_add(t.elapsed().as_nanos() as u64, AtomicOrdering::Relaxed);
    }
    crate::store::mark_complete(&tmp)
        .map_err(|e| FetchError::Io(format!("mark complete {tmp:?}: {e}")))?;
    if let Some(parent) = entry.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    match std::fs::rename(&tmp, &entry) {
        Ok(()) => {}
        Err(_) if crate::store::is_present(&key) => {
            let _ = std::fs::remove_dir_all(&tmp);
        }
        Err(e) => return Err(FetchError::Io(format!("publish store {entry:?}: {e}"))),
    }
    Ok(StorePackage {
        store_dir: entry,
        from_cache: false,
        file_count,
        tarball_bytes: Some(bytes.len()),
    })
}

fn count_regular_files(dir: &Path) -> Result<usize, FetchError> {
    fn walk(path: &Path, count: &mut usize) -> Result<(), FetchError> {
        for entry in
            std::fs::read_dir(path).map_err(|e| FetchError::Io(format!("read {path:?}: {e}")))?
        {
            let entry = entry.map_err(|e| FetchError::Io(format!("read {path:?}: {e}")))?;
            if entry.file_name().to_string_lossy().starts_with(".cruft-") {
                continue;
            }
            let ty = entry
                .file_type()
                .map_err(|e| FetchError::Io(format!("stat {:?}: {e}", entry.path())))?;
            if ty.is_dir() {
                walk(&entry.path(), count)?;
            } else if ty.is_file() {
                *count += 1;
            }
        }
        Ok(())
    }
    let mut count = 0;
    walk(dir, &mut count)?;
    Ok(count)
}

fn sanitize_entry_path(p: &Path) -> Result<PathBuf, FetchError> {
    sanitize_entry_path_with_root(p, None)
}

fn sanitize_entry_path_with_root(
    p: &Path,
    alternate_root: Option<&OsString>,
) -> Result<PathBuf, FetchError> {
    if path_has_windows_root_shape(p) {
        return Err(FetchError::UnsafePath(p.display().to_string()));
    }
    let mut components = p.components();
    let first = components.next();

    let mut out = PathBuf::new();

    match first {
        Some(Component::Normal(s)) if s == "package" || alternate_root.is_some_and(|r| r == s) => {
        }
        Some(Component::Normal(s)) if is_windows_drive_component(s) => {
            return Err(FetchError::UnsafePath(p.display().to_string()));
        }
        Some(Component::Normal(s)) => out.push(s),
        Some(Component::CurDir) => {   }
        Some(Component::RootDir) | Some(Component::Prefix(_)) => {
            return Err(FetchError::UnsafePath(p.display().to_string()));
        }
        Some(Component::ParentDir) => {
            return Err(FetchError::UnsafePath(p.display().to_string()));
        }

        None => return Ok(PathBuf::new()),
    }

    for c in components {
        match c {
            Component::Normal(s) if is_windows_drive_component(s) => {
                return Err(FetchError::UnsafePath(p.display().to_string()));
            }
            Component::Normal(s) => out.push(s),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(FetchError::UnsafePath(p.display().to_string()));
            }
        }
    }

    Ok(out)
}

fn path_has_windows_root_shape(p: &Path) -> bool {
    let s = p.to_string_lossy();
    s.contains('\\') || s.starts_with("//") || looks_like_drive_prefix(&s)
}

fn is_windows_drive_component(s: &std::ffi::OsStr) -> bool {
    s.to_str().is_some_and(looks_like_drive_prefix)
}

fn looks_like_drive_prefix(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp_staging(suffix: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "cruft-pm-test-{}-{}",
            suffix,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        p
    }

    #[test]
    fn sanitize_strips_package_prefix() {
        let out = sanitize_entry_path(Path::new("package/lib/index.js")).unwrap();
        assert_eq!(out, PathBuf::from("lib/index.js"));
    }

    #[test]
    fn sanitize_rejects_absolute() {
        assert!(sanitize_entry_path(Path::new("/etc/passwd")).is_err());
    }

    #[test]
    fn sanitize_root_dir_entry_is_empty_not_unsafe() {

        assert_eq!(
            sanitize_entry_path(Path::new("package")).unwrap(),
            PathBuf::new()
        );
        assert_eq!(
            sanitize_entry_path(Path::new("package/")).unwrap(),
            PathBuf::new()
        );
        assert_eq!(
            sanitize_entry_path(Path::new("./")).unwrap(),
            PathBuf::new()
        );

        assert!(sanitize_entry_path(Path::new("package/../evil")).is_err());
        assert!(sanitize_entry_path(Path::new("../evil")).is_err());
    }

    #[test]
    fn sanitize_rejects_parent_dir() {
        assert!(sanitize_entry_path(Path::new("package/../../etc/passwd")).is_err());
    }

    #[test]
    fn sanitize_rejects_windows_root_shapes() {
        assert!(sanitize_entry_path(Path::new("C:/temp/evil.js")).is_err());
        assert!(sanitize_entry_path(Path::new("package/C:/evil.js")).is_err());
        assert!(sanitize_entry_path(Path::new(r"package\evil.js")).is_err());
    }

    #[test]
    fn sanitize_keeps_first_component_when_not_package() {
        let out = sanitize_entry_path(Path::new("other/file.js")).unwrap();
        assert_eq!(out, PathBuf::from("other/file.js"));
    }

    fn dep_with_integrity(integrity: Option<String>, shasum: Option<String>) -> ResolvedDep {
        ResolvedDep {
            name: "legacy".to_string(),
            version: "1.0.0".to_string(),
            tarball_url: "https://registry.npmjs.org/legacy/-/legacy-1.0.0.tgz".to_string(),
            integrity,
            shasum,
            ..ResolvedDep::default()
        }
    }

    static SHA1_LEGACY_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn verify_rejects_sha1_only_metadata_by_default() {
        let _guard = SHA1_LEGACY_ENV_LOCK.lock().unwrap();
        std::env::remove_var("CRUFT_PM_ALLOW_SHA1_SHASUM");
        std::env::remove_var("CRUFTLESS_PM_ALLOW_SHA1_SHASUM");
        let shasum = "e10f6e70661d167ef514ab6e6d98607438c6a8c6".to_string();
        let dep = dep_with_integrity(None, Some(shasum));
        assert!(matches!(
            verify_bytes(&dep, b"tarball"),
            Err(FetchError::WeakLegacyShasum)
        ));
    }

    #[test]
    fn verify_accepts_sha1_only_metadata_with_legacy_opt_in() {
        let _guard = SHA1_LEGACY_ENV_LOCK.lock().unwrap();
        std::env::set_var("CRUFT_PM_ALLOW_SHA1_SHASUM", "1");
        let shasum = "e10f6e70661d167ef514ab6e6d98607438c6a8c6".to_string();
        let dep = dep_with_integrity(None, Some(shasum));
        assert!(verify_bytes(&dep, b"tarball").is_ok());
        std::env::remove_var("CRUFT_PM_ALLOW_SHA1_SHASUM");
    }

    #[test]
    fn tarball_size_limit_rejects_oversized_download() {
        assert!(check_tarball_size(MAX_TARBALL_BYTES).is_ok());
        assert!(matches!(
            check_tarball_size(MAX_TARBALL_BYTES + 1),
            Err(FetchError::TarballTooLarge { .. })
        ));
    }

    #[test]
    fn integrity_mismatch_aborts_before_extracting() {
        let tar = rusty_js_tar::build_ustar_archive(&[("package/index.js", b"ok")]).unwrap();
        let gz = rusty_compression::gzip_deflate_stored(&tar);
        let other_digest = rusty_js_pm_integrity::sha512_digest(b"different tarball");
        let bad_sri = format!(
            "sha512-{}",
            rusty_js_pm_integrity::encode_base64(&other_digest)
        );
        let dep = dep_with_integrity(Some(bad_sri), None);
        let staging = tmp_staging("integrity-mismatch");

        assert!(matches!(
            verify_and_extract_bytes(&dep, &gz, &staging),
            Err(FetchError::Integrity(_))
        ));
        assert!(
            !staging.exists(),
            "staging directory should not be created after integrity mismatch"
        );
        let _ = std::fs::remove_dir_all(staging);
    }

    #[test]
    fn extract_rejects_traversal_from_real_archive() {
        let tar = rusty_js_tar::build_ustar_archive(&[("package/../evil.js", b"bad")]).unwrap();
        let gz = rusty_compression::gzip_deflate_stored(&tar);
        let staging = tmp_staging("traversal");
        let err = extract_tar_into(&gz, &staging).expect_err("traversal must reject");
        assert!(matches!(err, FetchError::UnsafePath(_)));
        let _ = std::fs::remove_dir_all(staging);
    }

    #[test]
    fn count_regular_files_ignores_store_metadata_marker() {
        let dir = tmp_staging("store-file-count");
        std::fs::create_dir_all(dir.join("lib")).unwrap();
        std::fs::write(dir.join("package.json"), "{}").unwrap();
        std::fs::write(dir.join("lib").join("index.js"), "module.exports = 1").unwrap();
        std::fs::write(dir.join(".cruft-store-complete"), "1").unwrap();

        let count = count_regular_files(&dir).unwrap();
        let _ = std::fs::remove_dir_all(dir);
        assert_eq!(count, 2);
    }

    #[test]
    #[ignore]
    fn fetch_lodash_end_to_end() {
        use crate::resolver::{resolve_specifier, DEFAULT_REGISTRY};
        let dep = resolve_specifier(DEFAULT_REGISTRY, "lodash", "4.17.21").expect("resolve");
        let staging = tmp_staging("lodash");
        let result = fetch_and_extract(&dep, &staging).expect("fetch+extract");
        assert!(
            result.file_count > 100,
            "expected >100 files in lodash, got {}",
            result.file_count
        );
        let pkg_json_path = result.staging_dir.join("package.json");
        let body = std::fs::read_to_string(&pkg_json_path)
            .unwrap_or_else(|e| panic!("read {pkg_json_path:?}: {e}"));
        assert!(
            body.contains("\"version\": \"4.17.21\""),
            "package.json missing expected version: {}",
            &body[..body.len().min(200)]
        );

        let _ = std::fs::remove_dir_all(&staging);
    }
}
