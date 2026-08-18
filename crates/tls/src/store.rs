
use rusty_x509::*;
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::record::TlsError;

#[derive(Debug)]
pub struct TrustStore {
    certs: Vec<Certificate>,

    by_subject: HashMap<Vec<u8>, Vec<usize>>,
}

impl TrustStore {
    pub fn new() -> Self {
        TrustStore {
            certs: Vec::new(),
            by_subject: HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.certs.len()
    }
    pub fn is_empty(&self) -> bool {
        self.certs.is_empty()
    }

    pub fn add(&mut self, cert: Certificate) {
        let key = cert.subject.raw_der.clone();
        let idx = self.certs.len();
        self.certs.push(cert);
        self.by_subject.entry(key).or_default().push(idx);
    }

    pub fn add_pem_bundle(&mut self, pem: &str) -> Result<usize, TlsError> {
        let ders = pem_all_to_der(pem);
        let mut n = 0;
        for der in ders {

            if let Ok(cert) = parse_certificate(&der) {
                self.add(cert);
                n += 1;
            }
        }
        Ok(n)
    }

    pub fn load_system_default() -> Result<Self, TlsError> {
        #[cfg(windows)]
        {
            Self::load_windows_root_store()
        }
        #[cfg(not(windows))]
        {
            let mut store = TrustStore::new();
            let candidates = [
                "/etc/ssl/certs/ca-certificates.crt",
                "/etc/pki/tls/certs/ca-bundle.crt",
                "/etc/ssl/cert.pem",
                "/etc/ssl/ca-bundle.pem",
            ];
            for path in &candidates {
                if let Ok(contents) = std::fs::read_to_string(path) {
                    store.add_pem_bundle(&contents)?;
                    if !store.is_empty() {
                        return Ok(store);
                    }
                }
            }
            Err(TlsError::StoreLoad("no platform CA bundle found".into()))
        }
    }

    #[cfg(windows)]
    fn load_windows_root_store() -> Result<Self, TlsError> {
        use std::os::raw::c_void;

        #[repr(C)]
        struct CertContext {
            dw_cert_encoding_type: u32,
            pb_cert_encoded: *const u8,
            cb_cert_encoded: u32,
            p_cert_info: *mut c_void,
            h_cert_store: *mut c_void,
        }

        #[link(name = "crypt32")]
        unsafe extern "system" {
            fn CertOpenSystemStoreW(h_prov: usize, sz: *const u16) -> *mut c_void;
            fn CertEnumCertificatesInStore(
                store: *mut c_void,
                prev: *const CertContext,
            ) -> *const CertContext;
            fn CertCloseStore(store: *mut c_void, flags: u32) -> i32;
        }

        let mut store = TrustStore::new();

        let name: Vec<u16> = "ROOT".encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let h = CertOpenSystemStoreW(0, name.as_ptr());
            if h.is_null() {
                return Err(TlsError::StoreLoad(
                    "CertOpenSystemStore(ROOT) failed".into(),
                ));
            }

            let mut ctx = CertEnumCertificatesInStore(h, std::ptr::null());
            while !ctx.is_null() {
                let c = &*ctx;
                if !c.pb_cert_encoded.is_null() && c.cb_cert_encoded > 0 {
                    let der =
                        std::slice::from_raw_parts(c.pb_cert_encoded, c.cb_cert_encoded as usize);
                    if let Ok(cert) = parse_certificate(der) {
                        store.add(cert);
                    }
                }
                ctx = CertEnumCertificatesInStore(h, ctx);
            }
            CertCloseStore(h, 0);
        }
        if store.is_empty() {
            return Err(TlsError::StoreLoad("Windows ROOT store empty".into()));
        }
        Ok(store)
    }

    pub fn find_issuers(&self, child: &Certificate) -> Vec<&Certificate> {
        if let Some(idxs) = self.by_subject.get(&child.issuer.raw_der) {
            idxs.iter().map(|i| &self.certs[*i]).collect()
        } else {
            Vec::new()
        }
    }

    pub fn is_trust_anchor(&self, cert: &Certificate) -> bool {
        if cert.issuer.raw_der != cert.subject.raw_der {
            return false;
        }

        if let Some(idxs) = self.by_subject.get(&cert.subject.raw_der) {
            for i in idxs {
                if self.certs[*i].tbs_certificate == cert.tbs_certificate {
                    return true;
                }
            }
        }
        false
    }
}

fn verified_chain_cache() -> &'static Mutex<HashSet<[u8; 32]>> {
    static CACHE: OnceLock<Mutex<HashSet<[u8; 32]>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashSet::new()))
}

fn chain_fingerprint(leaf: &Certificate, intermediates: &[Certificate]) -> [u8; 32] {
    let mut buf = Vec::new();
    buf.extend_from_slice(&leaf.tbs_certificate);
    buf.extend_from_slice(&leaf.signature_value);
    for c in intermediates {
        buf.extend_from_slice(&c.tbs_certificate);
        buf.extend_from_slice(&c.signature_value);
    }
    rusty_web_crypto::digest_sha256(&buf)
}

pub fn chain_walk(
    leaf: &Certificate,
    intermediates: &[Certificate],
    store: &TrustStore,
    max_depth: usize,
) -> Result<(), TlsError> {
    let fp = chain_fingerprint(leaf, intermediates);
    let cache = verified_chain_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if cache.contains(&fp) {
        return Ok(());
    }
    drop(cache);
    let result = chain_walk_uncached(leaf, intermediates, store, max_depth);
    if result.is_ok() {
        let mut cache = verified_chain_cache()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        if cache.len() >= 512 {
            cache.clear();
        }
        cache.insert(fp);
    }
    result
}

pub fn validate_server_certificate(
    leaf: &Certificate,
    intermediates: &[Certificate],
    store: &TrustStore,
    max_depth: usize,
    hostname: &str,
    now: SystemTime,
) -> Result<(), TlsError> {
    chain_walk(leaf, intermediates, store, max_depth)?;
    validate_certificate_time(leaf, now)?;
    validate_server_auth_eku(leaf)?;
    validate_certificate_name(leaf, hostname)
}

fn validate_server_auth_eku(cert: &Certificate) -> Result<(), TlsError> {
    let Some(eku) = cert.extended_key_usage()? else {
        return Ok(());
    };
    if eku.usages.iter().any(|oid| {
        oid == "1.3.6.1.5.5.7.3.1"
            || oid == "2.5.29.37.0"
    }) {
        Ok(())
    } else {
        Err(TlsError::CertificatePurposeMismatch)
    }
}

pub fn validate_certificate_time(cert: &Certificate, now: SystemTime) -> Result<(), TlsError> {
    let not_before =
        x509_time_to_system_time(cert.validity.not_before_tag, &cert.validity.not_before)?;
    let not_after =
        x509_time_to_system_time(cert.validity.not_after_tag, &cert.validity.not_after)?;
    if now < not_before || now > not_after {
        return Err(TlsError::ValidityExpired);
    }
    Ok(())
}

pub fn validate_certificate_name(cert: &Certificate, hostname: &str) -> Result<(), TlsError> {
    let host = hostname.trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() {
        return Err(TlsError::CertificateNameMismatch(hostname.into()));
    }
    let sans = cert.subject_alt_names()?;
    let ip = host.parse::<IpAddr>().ok();
    if !sans.is_empty() {
        let matched = sans.iter().any(|name| match (name, ip) {
            (GeneralName::DnsName(pattern), None) => dns_name_matches(pattern, &host),
            (GeneralName::IpAddress(bytes), Some(addr)) => ip_san_matches(bytes, addr),
            _ => false,
        });
        if matched {
            return Ok(());
        }
        return Err(TlsError::CertificateNameMismatch(hostname.into()));
    }
    if ip.is_none() {
        if let Some(cn) = cert.common_name() {
            if dns_name_matches(&cn.to_ascii_lowercase(), &host) {
                return Ok(());
            }
        }
    }
    Err(TlsError::CertificateNameMismatch(hostname.into()))
}

fn dns_name_matches(pattern: &str, host: &str) -> bool {
    let pattern = pattern.trim_end_matches('.').to_ascii_lowercase();
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if pattern == host {
        return true;
    }
    let Some(suffix) = pattern.strip_prefix("*.") else {
        return false;
    };
    if suffix.is_empty() || !host.ends_with(suffix) {
        return false;
    }
    let prefix_len = host.len().saturating_sub(suffix.len());
    prefix_len > 1
        && host.as_bytes().get(prefix_len - 1) == Some(&b'.')
        && !host[..prefix_len - 1].contains('.')
}

fn ip_san_matches(bytes: &[u8], addr: IpAddr) -> bool {
    match (bytes, addr) {
        ([a, b, c, d], IpAddr::V4(ip)) => Ipv4Addr::new(*a, *b, *c, *d) == ip,
        (b, IpAddr::V6(ip)) if b.len() == 16 => {
            let mut octets = [0u8; 16];
            octets.copy_from_slice(b);
            Ipv6Addr::from(octets) == ip
        }
        _ => false,
    }
}

fn x509_time_to_system_time(tag: u8, value: &[u8]) -> Result<SystemTime, TlsError> {
    let s = std::str::from_utf8(value).map_err(|_| TlsError::ValidityExpired)?;
    let (year, rest) = if tag == rusty_asn1_der::TAG_UTC_TIME {
        if s.len() != 13 || !s.ends_with('Z') {
            return Err(TlsError::ValidityExpired);
        }
        let yy = parse_digits(&s[0..2])?;
        let year = if yy >= 50 { 1900 + yy } else { 2000 + yy };
        (year, &s[2..12])
    } else if tag == rusty_asn1_der::TAG_GENERALIZED_TIME {
        if s.len() != 15 || !s.ends_with('Z') {
            return Err(TlsError::ValidityExpired);
        }
        (parse_digits(&s[0..4])?, &s[4..14])
    } else {
        return Err(TlsError::ValidityExpired);
    };
    let month = parse_digits(&rest[0..2])?;
    let day = parse_digits(&rest[2..4])?;
    let hour = parse_digits(&rest[4..6])?;
    let minute = parse_digits(&rest[6..8])?;
    let second = parse_digits(&rest[8..10])?;
    let days = days_from_civil(year, month, day).ok_or(TlsError::ValidityExpired)?;
    let seconds = days
        .checked_mul(86_400)
        .and_then(|v| v.checked_add((hour as i64) * 3600 + (minute as i64) * 60 + second as i64))
        .ok_or(TlsError::ValidityExpired)?;
    if seconds < 0 {
        return Err(TlsError::ValidityExpired);
    }
    Ok(UNIX_EPOCH + Duration::from_secs(seconds as u64))
}

fn parse_digits(s: &str) -> Result<i64, TlsError> {
    if !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(TlsError::ValidityExpired);
    }
    s.parse::<i64>().map_err(|_| TlsError::ValidityExpired)
}

fn days_from_civil(year: i64, month: i64, day: i64) -> Option<i64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let y = year - if month <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

fn chain_walk_uncached(
    leaf: &Certificate,
    intermediates: &[Certificate],
    store: &TrustStore,
    max_depth: usize,
) -> Result<(), TlsError> {
    let mut current = leaf;
    let mut ca_depth_below = 0usize;
    for _depth in 0..max_depth {

        if current.issuer.raw_der == current.subject.raw_der {
            if store.is_trust_anchor(current) {
                if !certificate_can_issue(current, ca_depth_below)? {
                    return Err(TlsError::NoIssuerFound);
                }

                match verify_signature(current, &current.subject_public_key_info) {
                    Ok(()) => {}
                    Err(X509Error::UnsupportedSigAlg(_)) => {}
                    Err(e) => return Err(e.into()),
                }
                return Ok(());
            }
            return Err(TlsError::SelfSignedNotInTrust);
        }

        let mut issuer_opt = None;
        for candidate in store.find_issuers(current) {
            if issuer_candidate_valid(candidate, current, ca_depth_below)?
                && verify_signature(current, &candidate.subject_public_key_info).is_ok()
            {
                issuer_opt = Some(candidate);
                break;
            }
        }
        if issuer_opt.is_none() {
            for candidate in intermediates {
                if candidate.subject.raw_der == current.issuer.raw_der
                    && issuer_candidate_valid(candidate, current, ca_depth_below)?
                    && verify_signature(current, &candidate.subject_public_key_info).is_ok()
                {
                    issuer_opt = Some(candidate);
                    break;
                }
            }
        }
        let issuer = issuer_opt.ok_or(TlsError::NoIssuerFound)?;
        if current.is_ca()? {
            ca_depth_below = ca_depth_below.saturating_add(1);
        }
        current = issuer;
    }
    Err(TlsError::NoIssuerFound)
}

fn issuer_candidate_valid(
    issuer: &Certificate,
    child: &Certificate,
    ca_depth_below_child: usize,
) -> Result<bool, TlsError> {
    if issuer.subject.raw_der != child.issuer.raw_der {
        return Ok(false);
    }
    let ca_depth_below_issuer = if child.is_ca()? {
        ca_depth_below_child.saturating_add(1)
    } else {
        ca_depth_below_child
    };
    Ok(certificate_can_issue(issuer, ca_depth_below_issuer)?
        && issuer_name_constraints_allow_child(issuer, child)?)
}

fn certificate_can_issue(cert: &Certificate, ca_depth_below: usize) -> Result<bool, TlsError> {
    let Some(basic_constraints) = cert.basic_constraints()? else {
        return Ok(false);
    };
    if !basic_constraints.ca {
        return Ok(false);
    }
    if let Some(path_len) = basic_constraints.path_len_constraint {
        if ca_depth_below > path_len as usize {
            return Ok(false);
        }
    }
    if let Some(key_usage) = cert.key_usage()? {
        if !key_usage.key_cert_sign {
            return Ok(false);
        }
    }
    Ok(true)
}

fn issuer_name_constraints_allow_child(
    issuer: &Certificate,
    child: &Certificate,
) -> Result<bool, TlsError> {
    let Some(constraints) = issuer.name_constraints()? else {
        return Ok(true);
    };
    let child_names = certificate_constraint_names(child)?;
    if child_names.is_empty() {
        return Ok(false);
    }
    for name in &child_names {
        if constraints
            .excluded_subtrees
            .iter()
            .any(|subtree| general_name_within_subtree(name, &subtree.base))
        {
            return Ok(false);
        }
    }
    let has_supported_permitted = constraints.permitted_subtrees.iter().any(|subtree| {
        matches!(
            subtree.base,
            GeneralName::DnsName(_) | GeneralName::IpAddress(_)
        )
    });
    if has_supported_permitted {
        for name in &child_names {
            if !constraints
                .permitted_subtrees
                .iter()
                .any(|subtree| general_name_within_subtree(name, &subtree.base))
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn certificate_constraint_names(cert: &Certificate) -> Result<Vec<GeneralName>, TlsError> {
    let sans = cert.subject_alt_names()?;
    if !sans.is_empty() {
        return Ok(sans);
    }
    Ok(cert
        .common_name()
        .map(|cn| vec![GeneralName::DnsName(cn.to_ascii_lowercase())])
        .unwrap_or_default())
}

fn general_name_within_subtree(name: &GeneralName, subtree: &GeneralName) -> bool {
    match (name, subtree) {
        (GeneralName::DnsName(name), GeneralName::DnsName(base)) => {
            dns_name_within_constraint(name, base)
        }
        (GeneralName::IpAddress(name), GeneralName::IpAddress(base)) => name == base,
        _ => false,
    }
}

fn dns_name_within_constraint(name: &str, base: &str) -> bool {
    let name = name.trim_end_matches('.').to_ascii_lowercase();
    let base = base.trim_end_matches('.').to_ascii_lowercase();
    if base.is_empty() {
        return false;
    }
    if let Some(suffix) = base.strip_prefix('.') {
        name == suffix || name.ends_with(&format!(".{suffix}"))
    } else {
        name == base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verified_chain_cache_recovers_from_poisoning() {
        let _ = std::thread::spawn(|| {
            let _guard = verified_chain_cache().lock().unwrap();
            panic!("poison verified chain cache");
        })
        .join();

        let mut cache = verified_chain_cache()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        cache.clear();
        assert!(cache.is_empty());
    }
}
