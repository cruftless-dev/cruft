
pub mod export_shape;
pub mod fetcher;
pub mod http;
pub mod install;
pub mod integrity;
pub mod linker;
pub mod lockfile;
pub mod module_map;
pub mod registry_policy;
pub mod resolver;
pub mod security_metadata;
pub mod semver;
pub mod smoke;
pub mod store;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_roundtrip() {
        smoke::roundtrip_synthetic_tarball().expect("smoke test failed");
    }

    #[test]
    fn integrity_sri_sha512() {

        let empty_sri = "sha512-z4PhNX7vuL3xVChQ1m2AB9Yg5AULVxXcg/SpIdNs6c5H0NE8XYXysP+DGNKHfuwvY7kxvUdBeoGlODJ6+SfaPg==";
        integrity::verify_sri(b"", empty_sri).expect("empty SRI must verify");
        integrity::verify_sri(b"x", empty_sri).expect_err("non-empty input must fail");
    }
}
