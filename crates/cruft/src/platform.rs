
use std::path::PathBuf;

pub fn user_home_dir() -> Option<PathBuf> {
    if let Some(h) = std::env::var_os("HOME") {
        if !h.is_empty() {
            return Some(PathBuf::from(h));
        }
    }
    if let Some(up) = std::env::var_os("USERPROFILE") {
        if !up.is_empty() {
            return Some(PathBuf::from(up));
        }
    }
    match (std::env::var_os("HOMEDRIVE"), std::env::var_os("HOMEPATH")) {
        (Some(drive), Some(path)) if !drive.is_empty() && !path.is_empty() => {
            let mut p = std::ffi::OsString::from(drive);
            p.push(path);
            Some(PathBuf::from(p))
        }
        _ => None,
    }
}
