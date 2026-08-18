
pub use rusty_js_pm_integrity::IntegrityError;

pub fn verify_sri(bytes: &[u8], sri: &str) -> Result<(), IntegrityError> {
    rusty_js_pm_integrity::verify_sri(bytes, sri)
}

pub fn verify_shasum(bytes: &[u8], shasum: &str) -> Result<(), IntegrityError> {
    rusty_js_pm_integrity::verify_shasum(bytes, shasum)
}
