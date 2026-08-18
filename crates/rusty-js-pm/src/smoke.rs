
use std::path::PathBuf;

use crate::integrity;

#[derive(Debug)]
pub enum SmokeError {
    Io(std::io::Error),
    Integrity(integrity::IntegrityError),
    Mismatch(String),
}

impl From<std::io::Error> for SmokeError {
    fn from(e: std::io::Error) -> Self {
        SmokeError::Io(e)
    }
}
impl From<integrity::IntegrityError> for SmokeError {
    fn from(e: integrity::IntegrityError) -> Self {
        SmokeError::Integrity(e)
    }
}

pub fn roundtrip_synthetic_tarball() -> Result<(), SmokeError> {

    let files: &[(&str, &[u8])] = &[
        (
            "package/package.json",
            br#"{"name":"smoke","version":"0.0.0"}"#,
        ),
        ("package/index.js", b"module.exports = 42;\n"),
        ("package/README.md", b"# smoke\n"),
    ];
    let tar_bytes: Vec<u8> = {
        rusty_js_tar::build_ustar_archive(files)
            .map_err(|e| SmokeError::Mismatch(format!("tar build: {e}")))?
    };

    let gz_bytes: Vec<u8> = rusty_compression::gzip_deflate_stored(&tar_bytes);

    let digest = rusty_js_pm_integrity::sha512_digest(&gz_bytes);
    let sri = format!("sha512-{}", rusty_js_pm_integrity::encode_base64(&digest));
    integrity::verify_sri(&gz_bytes, &sri)?;

    let dest: PathBuf = {
        let mut p = std::env::temp_dir();
        p.push(format!("rusty-js-pm-smoke-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p)?;
        p
    };
    {
        let tar = rusty_compression::gunzip(&gz_bytes)
            .map_err(|e| SmokeError::Mismatch(format!("gunzip: {e:?}")))?;
        rusty_js_tar::extract_archive(&tar, &dest)
            .map_err(|e| SmokeError::Mismatch(format!("tar extract: {e}")))?;
    }

    for (path, expected) in files {
        let mut full = dest.clone();
        full.push(path);
        let got = std::fs::read(&full)
            .map_err(|e| SmokeError::Mismatch(format!("read {}: {e}", full.display())))?;
        if got != *expected {
            return Err(SmokeError::Mismatch(format!(
                "content mismatch at {}: expected {} bytes, got {} bytes",
                path,
                expected.len(),
                got.len()
            )));
        }
    }

    let _ = std::fs::remove_dir_all(&dest);
    Ok(())
}
