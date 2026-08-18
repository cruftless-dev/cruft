
use std::path::{Path, PathBuf};

const COMPLETE_MARKER: &str = ".cruft-store-complete";

pub fn store_root() -> PathBuf {
    if let Ok(p) = std::env::var("CRUFT_STORE") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    if let Some(home) = user_home_dir() {
        return home.join(".cruft").join("store");
    }
    PathBuf::from(".cruft-store")
}

fn user_home_dir() -> Option<PathBuf> {
    for var in ["HOME", "USERPROFILE"] {
        if let Some(v) = std::env::var_os(var) {
            if !v.is_empty() {
                return Some(PathBuf::from(v));
            }
        }
    }
    match (std::env::var_os("HOMEDRIVE"), std::env::var_os("HOMEPATH")) {
        (Some(d), Some(p)) if !d.is_empty() && !p.is_empty() => {
            let mut s = std::ffi::OsString::from(d);
            s.push(p);
            Some(PathBuf::from(s))
        }
        _ => None,
    }
}

pub fn content_key(integrity: Option<&str>, shasum: Option<&str>) -> Option<String> {
    let raw = integrity.or(shasum)?;
    if raw.is_empty() {
        return None;
    }
    Some(format!(
        "addr-{}",
        rusty_js_pm_integrity::canonical_sha256_hex(raw.as_bytes())
    ))
}

pub fn entry_dir(key: &str) -> PathBuf {
    store_root().join(key)
}

pub fn is_present(key: &str) -> bool {
    entry_dir(key).join(COMPLETE_MARKER).is_file()
}

pub fn mark_complete(entry: &Path) -> std::io::Result<()> {
    std::fs::write(entry.join(COMPLETE_MARKER), b"1")
}

pub fn temp_entry_dir(key: &str) -> PathBuf {
    store_root().join(format!(".tmp-{}-{}", key, std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_key_is_safe_and_stable() {
        let k = content_key(Some("sha512-AbC123+/=="), None).unwrap();
        assert!(k.starts_with("addr-"));
        assert!(k["addr-".len()..].chars().all(|c| c.is_ascii_hexdigit()));

        assert_eq!(
            content_key(Some("sha512-AbC123+/=="), None),
            content_key(Some("sha512-AbC123+/=="), Some("ignored"))
        );
        assert_ne!(
            content_key(Some("sha512-AbC123+/=="), None),
            content_key(Some("sha512-AbC123/++="), None)
        );

        assert!(content_key(None, Some("abc123def"))
            .unwrap()
            .starts_with("addr-"));
        assert_eq!(content_key(None, None), None);
    }

    #[test]
    fn entry_under_root() {
        std::env::set_var("CRUFT_STORE", "/tmp/cruft-store-test");
        assert_eq!(entry_dir("k").to_str().unwrap(), "/tmp/cruft-store-test/k");
        std::env::remove_var("CRUFT_STORE");
    }
}
