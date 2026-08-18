
use crate::client::*;
use crate::handshake::*;
use crate::record::{
    decode_record, encode_record, ContentType, ProtocolVersion, TlsError, TlsRecord,
    MAX_CIPHERTEXT_LEN,
};
use crate::store::*;

use rusty_x509::{Certificate as X509Cert, PublicKey, SubjectPublicKeyInfo};

pub struct TcpTlsTransport {
    pub stream: std::net::TcpStream,
}

impl TlsTransport for TcpTlsTransport {
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TlsError> {
        use std::io::Write;
        self.stream
            .write_all(bytes)
            .map_err(|e| TlsError::SignatureFail(format!("tcp write: {}", e)))
    }
    fn read_some(&mut self, buf: &mut Vec<u8>) -> Result<usize, TlsError> {
        use std::io::Read;
        let mut tmp = [0u8; 8192];
        let n = match self.stream.read(&mut tmp) {
            Ok(n) => n,

            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                return Err(TlsError::WouldBlock)
            }
            Err(e) => return Err(TlsError::SignatureFail(format!("tcp read: {}", e))),
        };
        if n == 0 {
            return Err(TlsError::UnexpectedEnd);
        }
        buf.extend_from_slice(&tmp[..n]);
        Ok(n)
    }
}

impl TcpTlsTransport {

    pub fn set_nonblocking(&self, on: bool) -> Result<(), TlsError> {
        self.stream
            .set_nonblocking(on)
            .map_err(|e| TlsError::SignatureFail(format!("tls set_nonblocking: {}", e)))
    }

    #[cfg(unix)]
    pub fn raw_fd(&self) -> std::os::unix::io::RawFd {
        use std::os::unix::io::AsRawFd;
        self.stream.as_raw_fd()
    }
}

thread_local! {

    static TLS_INSECURE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[deprecated(note = "use per-handshake TlsClientConfig instead of ambient TLS policy")]
pub fn set_tls_insecure(v: bool) {
    TLS_INSECURE.with(|c| c.set(v));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TlsClientConfig {
    pub insecure_skip_certificate_validation: bool,
}

impl TlsClientConfig {
    pub const fn secure() -> Self {
        Self {
            insecure_skip_certificate_validation: false,
        }
    }

    pub const fn insecure_no_verify() -> Self {
        Self {
            insecure_skip_certificate_validation: true,
        }
    }

    pub const fn from_reject_unauthorized(reject_unauthorized: bool) -> Self {
        Self {
            insecure_skip_certificate_validation: !reject_unauthorized,
        }
    }
}

pub fn tls_connect(
    host: &str,
    port: u16,
    trust_store: &TrustStore,
) -> Result<TlsSession<TcpTlsTransport>, TlsError> {
    tls_connect_with_alpn_config(host, port, trust_store, None, TlsClientConfig::secure())
}

pub fn tls_connect_with_alpn(
    host: &str,
    port: u16,
    trust_store: &TrustStore,
    alpn: Option<&[&[u8]]>,
) -> Result<TlsSession<TcpTlsTransport>, TlsError> {
    tls_connect_with_alpn_config(host, port, trust_store, alpn, TlsClientConfig::secure())
}

pub fn tls_connect_with_config(
    host: &str,
    port: u16,
    trust_store: &TrustStore,
    config: TlsClientConfig,
) -> Result<TlsSession<TcpTlsTransport>, TlsError> {
    tls_connect_with_alpn_config(host, port, trust_store, None, config)
}

pub fn tls_connect_with_alpn_config(
    host: &str,
    port: u16,
    trust_store: &TrustStore,
    alpn: Option<&[&[u8]]>,
    config: TlsClientConfig,
) -> Result<TlsSession<TcpTlsTransport>, TlsError> {
    let stream = happy_eyeballs_connect(host, port)
        .map_err(|e| TlsError::SignatureFail(format!("connect {}:{}: {}", host, port, e)))?;
    let mut transport = TcpTlsTransport { stream };
    let ephemeral = EphemeralEcdh::generate()?;
    let mut client_random = [0u8; 32];
    rusty_web_crypto::get_random_values(&mut client_random)
        .map_err(|e| TlsError::SignatureFail(format!("RNG: {}", e)))?;

    let mut legacy_session_id = [0u8; 32];
    rusty_web_crypto::get_random_values(&mut legacy_session_id)
        .map_err(|e| TlsError::SignatureFail(format!("RNG: {}", e)))?;

    let offered = crate::tls12::lookup_ticket(host);
    let offered_ticket_bytes: &[u8] = offered.as_ref().map(|t| t.ticket.as_slice()).unwrap_or(&[]);
    let ch_params = ClientHelloParams {
        random: &client_random,
        legacy_session_id: &legacy_session_id,
        cipher_suites: &[
            CIPHER_AES_128_GCM_SHA256,
            CIPHER_ECDHE_ECDSA_AES128_GCM_SHA256,
        ],
        server_name: Some(host),
        supported_groups: &[GROUP_SECP256R1],
        signature_algorithms: &[
            SIG_ECDSA_SECP256R1_SHA256,
            SIG_RSA_PKCS1_SHA256,
            SIG_RSA_PSS_RSAE_SHA256,
        ],
        key_shares: &[(GROUP_SECP256R1, ephemeral.public_point.clone())],
        alpn,
        session_ticket: Some(offered_ticket_bytes),
    };
    let ch_bytes = encode_client_hello(&ch_params)?;
    let record = TlsRecord {
        content_type: ContentType::Handshake,
        version: ProtocolVersion::LEGACY,
        fragment: ch_bytes.clone(),
    };
    transport.write_all(&encode_record(&record)?)?;
    complete_handshake(
        transport,
        ephemeral,
        &ch_bytes,
        trust_store,
        host,
        &legacy_session_id,
        offered,
        config,
    )
}

fn happy_eyeballs_connect(host: &str, port: u16) -> std::io::Result<std::net::TcpStream> {
    use std::net::{TcpStream, ToSocketAddrs};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{mpsc, Arc};
    use std::time::Duration;

    let resolved: Vec<std::net::SocketAddr> = (host, port).to_socket_addrs()?.collect();
    if resolved.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no addresses resolved",
        ));
    }
    if resolved.len() == 1 {
        return TcpStream::connect_timeout(&resolved[0], Duration::from_secs(10));
    }

    let (v4, v6): (Vec<_>, Vec<_>) = resolved.into_iter().partition(|a| a.is_ipv4());
    let (mut i4, mut i6) = (v4.into_iter(), v6.into_iter());
    let mut ordered = Vec::new();
    loop {
        let (a, b) = (i4.next(), i6.next());
        if a.is_none() && b.is_none() {
            break;
        }
        ordered.extend(a);
        ordered.extend(b);
    }
    ordered.truncate(8);

    let stagger = Duration::from_millis(50);

    let attempt_timeout = Duration::from_secs(10);
    let done = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel::<TcpStream>();
    for (i, addr) in ordered.into_iter().enumerate() {
        let done = Arc::clone(&done);
        let tx = tx.clone();
        std::thread::spawn(move || {
            if i > 0 {
                std::thread::sleep(stagger * i as u32);
            }
            if done.load(Ordering::Acquire) {
                return;
            }
            if let Ok(s) = TcpStream::connect_timeout(&addr, attempt_timeout) {
                if !done.swap(true, Ordering::AcqRel) {
                    let _ = tx.send(s);
                }
            }
        });
    }
    drop(tx);
    match rx.recv() {
        Ok(s) => Ok(s),
        Err(_) => Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            "happy-eyeballs: all connect attempts failed",
        )),
    }
}

fn verify_certificate_verify_signature(
    scheme: u16,
    spki: &SubjectPublicKeyInfo,
    tbs: &[u8],
    signature: &[u8],
) -> Result<(), TlsError> {
    match scheme {

        SIG_RSA_PKCS1_SHA256 | SIG_RSA_PKCS1_SHA384 | SIG_RSA_PKCS1_SHA512 => {
            let (n, e) = match &spki.key {
                PublicKey::Rsa { n, e } => (n, e),
                _ => {
                    return Err(TlsError::SignatureFail(
                        "RSA scheme but leaf is not RSA".into(),
                    ))
                }
            };
            let (hash, name) = match scheme {
                SIG_RSA_PKCS1_SHA256 => (rusty_web_crypto::digest_sha256(tbs).to_vec(), "SHA-256"),
                SIG_RSA_PKCS1_SHA384 => (rusty_web_crypto::digest_sha384(tbs).to_vec(), "SHA-384"),
                SIG_RSA_PKCS1_SHA512 => (rusty_web_crypto::digest_sha512(tbs).to_vec(), "SHA-512"),
                _ => unreachable!(),
            };
            rusty_web_crypto::rsa_pkcs1_v15_verify(n, e, &hash, signature, name)
                .map_err(TlsError::SignatureFail)
        }

        SIG_RSA_PSS_RSAE_SHA256 | SIG_RSA_PSS_RSAE_SHA384 => {
            let (n, e) = match &spki.key {
                PublicKey::Rsa { n, e } => (n, e),
                _ => {
                    return Err(TlsError::SignatureFail(
                        "RSA-PSS scheme but leaf is not RSA".into(),
                    ))
                }
            };
            let (hlen, hash_fn): (usize, fn(&[u8]) -> Vec<u8>) = match scheme {
                SIG_RSA_PSS_RSAE_SHA256 => (32, |d| rusty_web_crypto::digest_sha256(d).to_vec()),
                SIG_RSA_PSS_RSAE_SHA384 => (48, |d| rusty_web_crypto::digest_sha384(d).to_vec()),
                _ => unreachable!(),
            };

            rusty_web_crypto::rsa_pss_verify(n, e, tbs, signature, hlen, hash_fn, hlen)
                .map_err(TlsError::SignatureFail)
        }

        SIG_ECDSA_SECP256R1_SHA256 | SIG_ECDSA_SECP384R1_SHA384 => {
            let (curve_oid, point) = match &spki.key {
                PublicKey::Ec { curve_oid, point } => (curve_oid, point),
                _ => {
                    return Err(TlsError::SignatureFail(
                        "ECDSA scheme but leaf is not EC".into(),
                    ))
                }
            };
            let curve = match curve_oid.as_str() {
                rusty_x509::OID_P256_CURVE => rusty_web_crypto::curve_p256(),
                rusty_x509::OID_P384_CURVE => rusty_web_crypto::curve_p384(),
                _ => {
                    return Err(TlsError::SignatureFail(format!(
                        "unsupported EC curve {}",
                        curve_oid
                    )))
                }
            };
            if point.is_empty() || point[0] != 0x04 || point.len() != 1 + 2 * curve.coord_bytes {
                return Err(TlsError::SignatureFail("malformed EC pubkey".into()));
            }
            let coord = curve.coord_bytes;
            let qx = &point[1..1 + coord];
            let qy = &point[1 + coord..];
            let hash = match scheme {
                SIG_ECDSA_SECP256R1_SHA256 => rusty_web_crypto::digest_sha256(tbs).to_vec(),
                SIG_ECDSA_SECP384R1_SHA384 => rusty_web_crypto::digest_sha384(tbs).to_vec(),
                _ => unreachable!(),
            };
            let dbg_der = std::env::var("CRUFT_TLS_DEBUG").is_ok();
            if dbg_der {
                let hex = |b: &[u8]| b.iter().map(|x| format!("{:02x}", x)).collect::<String>();
                eprintln!(
                    "[ecdsa-der] signature ({} bytes)={}",
                    signature.len(),
                    hex(signature)
                );
                eprintln!("[ecdsa-der] → parse_single...");
            }

            let sig_seq = rusty_asn1_der::parse_single(signature)
                .map_err(|e| TlsError::SignatureFail(format!("ECDSA sig DER: {}", e)))?;
            if dbg_der {
                eprintln!("[ecdsa-der]   parse_single OK; tag={}", sig_seq.tag);
            }
            if sig_seq.tag != rusty_asn1_der::TAG_SEQUENCE {
                return Err(TlsError::SignatureFail("ECDSA sig not SEQUENCE".into()));
            }
            let mut reader = rusty_asn1_der::DerReader::new(sig_seq.content);
            if dbg_der {
                eprintln!("[ecdsa-der] → read r...");
            }
            let r_val = reader
                .read_tag(rusty_asn1_der::TAG_INTEGER)
                .map_err(|e| TlsError::SignatureFail(format!("ECDSA r: {}", e)))?;
            if dbg_der {
                eprintln!("[ecdsa-der] → read s...");
            }
            let s_val = reader
                .read_tag(rusty_asn1_der::TAG_INTEGER)
                .map_err(|e| TlsError::SignatureFail(format!("ECDSA s: {}", e)))?;
            if dbg_der {
                eprintln!("[ecdsa-der] → r/s as_unsigned_integer...");
            }
            let r = r_val
                .as_unsigned_integer()
                .map_err(|e| TlsError::SignatureFail(format!("ECDSA r unsigned: {}", e)))?;
            let s = s_val
                .as_unsigned_integer()
                .map_err(|e| TlsError::SignatureFail(format!("ECDSA s unsigned: {}", e)))?;
            if dbg_der {
                eprintln!("[ecdsa-der]   r.len()={} s.len()={}", r.len(), s.len());
            }
            let mut sig_raw = vec![0u8; 2 * coord];
            sig_raw[coord - r.len()..coord].copy_from_slice(r);
            sig_raw[2 * coord - s.len()..].copy_from_slice(s);
            if std::env::var("CRUFT_TLS_DEBUG").is_ok() {
                let hex = |b: &[u8]| b.iter().map(|x| format!("{:02x}", x)).collect::<String>();
                eprintln!("[hs-cv-fixture] qx={}", hex(qx));
                eprintln!("[hs-cv-fixture] qy={}", hex(qy));
                eprintln!("[hs-cv-fixture] hash={}", hex(&hash));
                eprintln!("[hs-cv-fixture] sig_raw={}", hex(&sig_raw));
                eprintln!("[hs-cv-fixture] → calling rusty_web_crypto::ecdsa_verify ...");
            }
            rusty_web_crypto::ecdsa_verify(&curve, qx, qy, &hash, &sig_raw)
                .map_err(TlsError::SignatureFail)
        }
        _ => Err(TlsError::SignatureFail(format!(
            "unsupported SignatureScheme 0x{:04x}",
            scheme
        ))),
    }
}

pub trait TlsTransport {
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TlsError>;
    fn read_some(&mut self, buf: &mut Vec<u8>) -> Result<usize, TlsError>;
}

pub struct EphemeralEcdh {
    pub private_scalar: Vec<u8>,
    pub public_point: Vec<u8>,
}

impl EphemeralEcdh {

    pub fn generate() -> Result<Self, TlsError> {
        let mut sk = [0u8; 32];
        rusty_web_crypto::get_random_values(&mut sk)
            .map_err(|e| TlsError::SignatureFail(format!("RNG: {}", e)))?;

        sk[0] &= 0x7F;
        if sk == [0u8; 32] {
            sk[31] = 1;
        }

        use rusty_web_crypto::{p256_scalar_mul_base_solinas, BigUInt};
        let scalar = BigUInt::from_be_bytes(&sk);
        let pubpt = p256_scalar_mul_base_solinas(&scalar);
        let (px, py) = match pubpt {
            rusty_web_crypto::P256Point::Affine { x, y } => (x.to_be_bytes(32), y.to_be_bytes(32)),
            rusty_web_crypto::P256Point::Identity => {
                return Err(TlsError::SignatureFail(
                    "ECDH ephemeral produced identity point".into(),
                ))
            }
        };
        let mut public_point = Vec::with_capacity(65);
        public_point.push(0x04);
        public_point.extend_from_slice(&px);
        public_point.extend_from_slice(&py);
        Ok(EphemeralEcdh {
            private_scalar: sk.to_vec(),
            public_point,
        })
    }

    pub fn shared_secret(&self, server_pubkey: &[u8]) -> Result<Vec<u8>, TlsError> {
        if server_pubkey.len() != 65 || server_pubkey[0] != 0x04 {
            return Err(TlsError::SignatureFail(
                "server key_share not uncompressed P-256".into(),
            ));
        }
        use rusty_web_crypto::{curve_p256, p256_scalar_mul, BigUInt, P256Point};
        let x = BigUInt::from_be_bytes(&server_pubkey[1..33]);
        let y = BigUInt::from_be_bytes(&server_pubkey[33..65]);
        let q = P256Point::Affine { x, y };
        let scalar = BigUInt::from_be_bytes(&self.private_scalar);
        let _curve = curve_p256();
        let shared_point = p256_scalar_mul(&scalar, &q);
        match shared_point {
            P256Point::Affine { x, .. } => Ok(x.to_be_bytes(32)),
            P256Point::Identity => Err(TlsError::SignatureFail(
                "ECDH produced identity point".into(),
            )),
        }
    }
}

pub struct TlsSession<T: TlsTransport> {
    pub transport: T,
    pub client_app_keys: TrafficKeys,
    pub server_app_keys: TrafficKeys,
    pub client_app_seq: u64,
    pub server_app_seq: u64,

    pub hash: HashAlgorithm,

    pub tls12: bool,
}

impl<T: TlsTransport> TlsSession<T> {

    pub fn send_application_data(&mut self, data: &[u8]) -> Result<(), TlsError> {
        let ct = if self.tls12 {
            crate::tls12::encrypt_record_tls12(
                &self.client_app_keys.key,
                &self.client_app_keys.iv,
                self.client_app_seq,
                ContentType::ApplicationData as u8,
                data,
            )?
        } else {
            aead_encrypt_record(
                &self.client_app_keys,
                self.client_app_seq,
                ContentType::ApplicationData as u8,
                data,
            )?
        };
        self.client_app_seq += 1;
        let record = TlsRecord {
            content_type: ContentType::ApplicationData,
            version: ProtocolVersion::LEGACY,
            fragment: ct,
        };
        self.transport.write_all(&encode_record(&record)?)
    }

    pub fn try_receive_application_data(
        &mut self,
        accumulator: &mut Vec<u8>,
    ) -> Result<Option<Vec<u8>>, TlsError> {
        loop {
            if let Ok((rec, n)) = decode_record(accumulator) {
                accumulator.drain(..n);
                if rec.content_type != ContentType::ApplicationData {
                    if rec.content_type == ContentType::ChangeCipherSpec {
                        continue;
                    }
                    return Err(TlsError::SignatureFail(
                        "unexpected plaintext record post-handshake".into(),
                    ));
                }
                let (inner_ct, plaintext) = if self.tls12 {
                    let pt = crate::tls12::decrypt_record_tls12(
                        &self.server_app_keys.key,
                        &self.server_app_keys.iv,
                        self.server_app_seq,
                        ContentType::ApplicationData as u8,
                        &rec.fragment,
                    )?;
                    (ContentType::ApplicationData as u8, pt)
                } else {
                    aead_decrypt_record(&self.server_app_keys, self.server_app_seq, &rec.fragment)?
                };
                self.server_app_seq += 1;
                match inner_ct {
                    23 => return Ok(Some(plaintext)),
                    21 => return Err(crate::record::classify_alert(&plaintext)),
                    22 => continue,
                    _ => {
                        return Err(TlsError::SignatureFail(format!(
                            "unknown inner content type {}",
                            inner_ct
                        )))
                    }
                }
            } else {
                match self.transport.read_some(accumulator) {
                    Ok(_) => {
                        if accumulator.len() > MAX_CIPHERTEXT_LEN + 5 + 16
                            && decode_record(accumulator).is_err()
                        {
                            return Err(TlsError::SignatureFail(
                                "record buffer overflow without progress".into(),
                            ));
                        }
                    }
                    Err(TlsError::WouldBlock) => return Ok(None),
                    Err(e) => return Err(e),
                }
            }
        }
    }

    pub fn receive_application_data(
        &mut self,
        accumulator: &mut Vec<u8>,
    ) -> Result<Vec<u8>, TlsError> {
        loop {

            if let Ok((rec, n)) = decode_record(accumulator) {
                let dbg = std::env::var("CRUFT_TLS_DEBUG").is_ok();
                if dbg {
                    eprintln!(
                        "[tls-c-new-1] record: ct={:?} frag_len={} seq={}",
                        rec.content_type,
                        rec.fragment.len(),
                        self.server_app_seq
                    );
                }
                accumulator.drain(..n);
                if rec.content_type != ContentType::ApplicationData {

                    if rec.content_type == ContentType::ChangeCipherSpec {
                        if dbg {
                            eprintln!("[tls-c-new-1]   ignored: ChangeCipherSpec");
                        }
                        continue;
                    }
                    return Err(TlsError::SignatureFail(
                        "unexpected plaintext record post-handshake".into(),
                    ));
                }
                let dec = if self.tls12 {
                    crate::tls12::decrypt_record_tls12(
                        &self.server_app_keys.key,
                        &self.server_app_keys.iv,
                        self.server_app_seq,
                        ContentType::ApplicationData as u8,
                        &rec.fragment,
                    )
                    .map(|pt| (ContentType::ApplicationData as u8, pt))
                } else {
                    aead_decrypt_record(&self.server_app_keys, self.server_app_seq, &rec.fragment)
                };
                let (inner_ct, plaintext) = match dec {
                    Ok(x) => x,
                    Err(e) => {
                        if dbg {
                            eprintln!("[tls-c-new-1]   DECRYPT FAILED: {:?}", e);
                        }
                        return Err(e);
                    }
                };
                if dbg {
                    eprintln!(
                        "[tls-c-new-1]   decrypt OK: inner_ct={} pt_len={}",
                        inner_ct,
                        plaintext.len()
                    );
                }
                self.server_app_seq += 1;
                match inner_ct {
                    23   => return Ok(plaintext),
                    21   => return Err(crate::record::classify_alert(&plaintext)),
                    22   => {

                        if dbg {
                            let ht = plaintext.first().copied().unwrap_or(0);
                            eprintln!("[tls-c-new-1]   post-handshake handshake_type=0x{:02x} (4=NewSessionTicket, 24=KeyUpdate)", ht);
                        }
                        continue;
                    }
                    _ => return Err(TlsError::SignatureFail(
                        format!("unknown inner content type {}", inner_ct))),
                }
            } else {
                let dbg = std::env::var("CRUFT_TLS_DEBUG").is_ok();
                if dbg {
                    eprintln!(
                        "[tls-c-new-1] need more bytes; calling transport.read_some (acc={})",
                        accumulator.len()
                    );
                }
                let n = match self.transport.read_some(accumulator) {
                    Ok(n) => n,
                    Err(e) => {
                        if dbg {
                            eprintln!("[tls-c-new-1]   read_some FAILED: {:?}", e);
                        }
                        return Err(e);
                    }
                };
                if dbg {
                    eprintln!(
                        "[tls-c-new-1] transport.read_some → {} bytes (accumulator now {})",
                        n,
                        accumulator.len()
                    );
                }
                if accumulator.len() > MAX_CIPHERTEXT_LEN + 5 + 16
                    && decode_record(accumulator).is_err()
                {
                    return Err(TlsError::SignatureFail(
                        "record buffer overflow without progress".into(),
                    ));
                }
            }
        }
    }
}

pub fn initiate_handshake<T: TlsTransport>(
    transport: &mut T,
    hostname: &str,
    _trust_store: &TrustStore,
) -> Result<EphemeralEcdh, TlsError> {
    let ephemeral = EphemeralEcdh::generate()?;
    let mut client_random = [0u8; 32];
    rusty_web_crypto::get_random_values(&mut client_random)
        .map_err(|e| TlsError::SignatureFail(format!("RNG: {}", e)))?;
    let ch = ClientHelloParams {
        random: &client_random,
        legacy_session_id: &[],
        cipher_suites: &[CIPHER_AES_128_GCM_SHA256],
        server_name: Some(hostname),
        supported_groups: &[GROUP_SECP256R1],
        signature_algorithms: &[
            SIG_ECDSA_SECP256R1_SHA256,
            SIG_RSA_PKCS1_SHA256,
            SIG_RSA_PSS_RSAE_SHA256,
        ],
        key_shares: &[(GROUP_SECP256R1, ephemeral.public_point.clone())],
        alpn: None,
        session_ticket: None,
    };
    let ch_bytes = encode_client_hello(&ch)?;
    let record = TlsRecord {
        content_type: ContentType::Handshake,
        version: ProtocolVersion::LEGACY,
        fragment: ch_bytes,
    };
    transport.write_all(&encode_record(&record)?)?;
    Ok(ephemeral)
}

pub fn complete_handshake<T: TlsTransport>(
    mut transport: T,
    ephemeral: EphemeralEcdh,
    client_hello_handshake_msg: &[u8],
    trust_store: &TrustStore,
    hostname: &str,
    client_session_id: &[u8],
    offered_ticket: Option<crate::tls12::StoredTicket>,
    config: TlsClientConfig,
) -> Result<TlsSession<T>, TlsError> {
    let hash = HashAlgorithm::Sha256;
    let mut transcript = Vec::new();
    transcript.extend_from_slice(client_hello_handshake_msg);

    let dbg_hs = std::env::var("CRUFT_TLS_DEBUG").is_ok();
    let mut accumulator: Vec<u8> = Vec::new();
    let mut server_hello: Option<ServerHello> = None;
    let mut server_hello_handshake_msg: Vec<u8> = Vec::new();
    let mut phase1_iter = 0u32;
    while server_hello.is_none() {
        phase1_iter += 1;
        if dbg_hs {
            eprintln!(
                "[hs-phase1] iter={} acc_len={}",
                phase1_iter,
                accumulator.len()
            );
        }
        let (rec, n) = match decode_record(&accumulator) {
            Ok(r) => r,
            Err(_) => {
                if dbg_hs {
                    eprintln!("[hs-phase1]   need more; read_some...");
                }
                let nb = transport.read_some(&mut accumulator)?;
                if dbg_hs {
                    eprintln!(
                        "[hs-phase1]   read_some → {} bytes (acc={})",
                        nb,
                        accumulator.len()
                    );
                }
                continue;
            }
        };
        if dbg_hs {
            eprintln!(
                "[hs-phase1]   got record ct={:?} frag_len={}",
                rec.content_type,
                rec.fragment.len()
            );
        }
        accumulator.drain(..n);
        match rec.content_type {
            ContentType::ChangeCipherSpec => continue,
            ContentType::Handshake => {
                let mut pos = 0;
                while pos < rec.fragment.len() {
                    let (msg, used) = decode_handshake(&rec.fragment[pos..])?;
                    let msg_bytes = &rec.fragment[pos..pos + used];
                    pos += used;
                    if msg.msg_type == HandshakeType::ServerHello {
                        server_hello = Some(decode_server_hello(&msg.body)?);
                        server_hello_handshake_msg = msg_bytes.to_vec();
                        transcript.extend_from_slice(msg_bytes);
                        break;
                    } else {
                        return Err(TlsError::SignatureFail(format!(
                            "unexpected handshake type {:?} before ServerHello",
                            msg.msg_type
                        )));
                    }
                }
            }
            ContentType::Alert => {
                return Err(crate::record::classify_alert(&rec.fragment));
            }
            _ => {
                return Err(TlsError::SignatureFail(
                    "unexpected content type before ServerHello".into(),
                ))
            }
        }
    }
    let sh = server_hello.ok_or(TlsError::SignatureFail(
        "peer closed before ServerHello".into(),
    ))?;
    let _ = server_hello_handshake_msg;

    match sh.selected_version() {
        Some(0x0304) => {   }
        Some(0x0303) => {
            if sh.has_tls13_downgrade_sentinel() {
                return Err(TlsError::SignatureFail(
                    "TLS 1.3 downgrade sentinel present in TLS 1.2 ServerHello".into(),
                ));
            }

            if client_hello_handshake_msg.len() < 38 {
                return Err(TlsError::SignatureFail(
                    "ClientHello too short for client_random".into(),
                ));
            }
            let mut cr = [0u8; 32];
            cr.copy_from_slice(&client_hello_handshake_msg[6..38]);
            return crate::tls12::complete_handshake_tls12(
                transport,
                &sh,
                ephemeral,
                &cr,
                transcript,
                accumulator,
                trust_store,
                hostname,
                client_session_id,
                offered_ticket,
                config,
            );
        }
        other => {
            return Err(TlsError::SignatureFail(format!(
                "server selected unsupported TLS version {other:?}"
            )));
        }
    }

    if sh.cipher_suite != CIPHER_AES_128_GCM_SHA256 {
        return Err(TlsError::SignatureFail(format!(
            "server selected unsupported cipher 0x{:04x}",
            sh.cipher_suite
        )));
    }
    let (group, server_pub) = sh.server_key_share().ok_or(TlsError::SignatureFail(
        "ServerHello missing key_share".into(),
    ))?;
    if group != GROUP_SECP256R1 {
        return Err(TlsError::SignatureFail(
            "server selected non-P256 group".into(),
        ));
    }
    let profile_sh = std::env::var("CRUFT_TLS_PROFILE").is_ok();
    let t_sh = std::time::Instant::now();
    let dhe = ephemeral.shared_secret(server_pub)?;
    if profile_sh {
        eprintln!("[wc-ext-14] ECDH shared_secret: {:?}", t_sh.elapsed());
    }
    let schedule = KeySchedule::new(hash, &dhe, &hash.digest(&transcript))?;
    let transcript_hash_sh = hash.digest(&transcript);
    let server_hs_secret = schedule.server_handshake_traffic(&transcript_hash_sh)?;
    let client_hs_secret = schedule.client_handshake_traffic(&transcript_hash_sh)?;
    let server_hs_keys = derive_traffic_keys(hash, &server_hs_secret, 16, 12)?;
    let client_hs_keys = derive_traffic_keys(hash, &client_hs_secret, 16, 12)?;

    let mut server_seq: u64 = 0;
    let mut handshake_buffer: Vec<u8> = Vec::new();
    let mut server_certs: Vec<rusty_x509::Certificate> = Vec::new();
    let mut got_finished = false;
    let mut transcript_through_cv: Option<Vec<u8>> = None;
    let mut transcript_through_finished: Option<Vec<u8>> = None;

    let mut phase3_iter = 0u32;
    'outer: while !got_finished {
        phase3_iter += 1;
        if dbg_hs {
            eprintln!(
                "[hs-phase3] iter={} acc_len={} hb_len={} seq={}",
                phase3_iter,
                accumulator.len(),
                handshake_buffer.len(),
                server_seq
            );
        }

        while !decode_record(&accumulator).is_ok() {
            if dbg_hs {
                eprintln!("[hs-phase3]   inner: need more for record; read_some...");
            }
            let nb = transport.read_some(&mut accumulator)?;
            if dbg_hs {
                eprintln!(
                    "[hs-phase3]   inner: read_some → {} bytes (acc={})",
                    nb,
                    accumulator.len()
                );
            }
        }
        let (rec, n) = decode_record(&accumulator)?;
        if dbg_hs {
            eprintln!(
                "[hs-phase3]   record ct={:?} frag_len={}",
                rec.content_type,
                rec.fragment.len()
            );
        }
        accumulator.drain(..n);
        if rec.content_type == ContentType::ChangeCipherSpec {
            continue;
        }
        if rec.content_type != ContentType::ApplicationData {
            return Err(TlsError::SignatureFail(
                "unexpected plaintext in handshake phase".into(),
            ));
        }
        let (inner_ct, plaintext) =
            aead_decrypt_record(&server_hs_keys, server_seq, &rec.fragment)?;
        if dbg_hs {
            eprintln!(
                "[hs-phase3]   decrypted: inner_ct={} pt_len={}",
                inner_ct,
                plaintext.len()
            );
        }
        server_seq += 1;
        if inner_ct != 22

        {
            return Err(TlsError::SignatureFail(format!(
                "expected Handshake inner type, got {}",
                inner_ct
            )));
        }
        handshake_buffer.extend_from_slice(&plaintext);

        let mut drain_iter = 0u32;
        loop {
            drain_iter += 1;
            if dbg_hs {
                eprintln!(
                    "[hs-phase3-drain] iter={} hb_len={}",
                    drain_iter,
                    handshake_buffer.len()
                );
            }
            let (msg, used) = match decode_handshake(&handshake_buffer) {
                Ok(p) => p,
                Err(e) => {
                    if dbg_hs {
                        eprintln!("[hs-phase3-drain]   decode_handshake Err: {:?} (continue 'outer for more bytes)", e);
                    }
                    continue 'outer;
                }
            };
            if dbg_hs {
                eprintln!(
                    "[hs-phase3-drain]   msg_type={:?} used={}",
                    msg.msg_type, used
                );
            }
            let msg_bytes = handshake_buffer[..used].to_vec();
            handshake_buffer.drain(..used);
            transcript.extend_from_slice(&msg_bytes);
            match msg.msg_type {
                HandshakeType::EncryptedExtensions => {

                }
                HandshakeType::Certificate => {
                    server_certs = parse_certificate_message(&msg.body)?;
                    if server_certs.is_empty() {
                        return Err(TlsError::SignatureFail("server sent zero certs".into()));
                    }
                }
                HandshakeType::CertificateVerify => {

                    if msg.body.len() < 4 {
                        return Err(TlsError::SignatureFail(
                            "CertificateVerify body too short".into(),
                        ));
                    }
                    let scheme = ((msg.body[0] as u16) << 8) | (msg.body[1] as u16);
                    let sig_len = ((msg.body[2] as usize) << 8) | (msg.body[3] as usize);
                    if msg.body.len() < 4 + sig_len {
                        return Err(TlsError::SignatureFail(
                            "CertificateVerify truncated".into(),
                        ));
                    }
                    let signature = &msg.body[4..4 + sig_len];

                    let mut tbs = Vec::new();
                    tbs.extend_from_slice(&[0x20u8; 64]);
                    tbs.extend_from_slice(b"TLS 1.3, server CertificateVerify");
                    tbs.push(0x00);

                    let cv_len = msg_bytes.len();
                    let mut transcript_through_cert = transcript.clone();
                    transcript_through_cert.truncate(transcript.len() - cv_len);
                    tbs.extend_from_slice(&hash.digest(&transcript_through_cert));

                    let leaf = server_certs.first().ok_or(TlsError::SignatureFail(
                        "CertificateVerify before Certificate".into(),
                    ))?;
                    if dbg_hs {
                        eprintln!("[hs-cv] scheme=0x{:04x} sig_len={}", scheme, sig_len);
                    }
                    let profile_cv = std::env::var("CRUFT_TLS_PROFILE").is_ok();
                    let t_cv = std::time::Instant::now();
                    verify_certificate_verify_signature(
                        scheme,
                        &leaf.subject_public_key_info,
                        &tbs,
                        signature,
                    )?;
                    if profile_cv {
                        eprintln!(
                            "[wc-ext-14] CertificateVerify scheme=0x{:04x}: {:?}",
                            scheme,
                            t_cv.elapsed()
                        );
                    }
                    transcript_through_cv = Some(transcript.clone());
                }
                HandshakeType::Finished => {

                    let th_through_cv =
                        transcript_through_cv
                            .clone()
                            .ok_or(TlsError::SignatureFail(
                                "Finished before CertificateVerify".into(),
                            ))?;

                    let th = hash.digest(&th_through_cv);
                    let expected = finished_mac(hash, &server_hs_secret, &th)?;
                    if !finished_verify_data_equal(&msg.body, &expected) {
                        return Err(TlsError::SignatureFail(
                            "server Finished MAC mismatch".into(),
                        ));
                    }
                    transcript_through_finished = Some(transcript.clone());
                    got_finished = true;
                    break;
                }
                _ => {
                    return Err(TlsError::SignatureFail(format!(
                        "unexpected handshake type {:?} in encrypted phase",
                        msg.msg_type
                    )));
                }
            }
        }
    }

    let leaf = server_certs
        .first()
        .ok_or(TlsError::SignatureFail("no leaf cert".into()))?;
    let intermediates: Vec<_> = server_certs.iter().skip(1).cloned().collect();
    let profile = std::env::var("CRUFT_TLS_PROFILE").is_ok();
    let t0 = std::time::Instant::now();

    if config.insecure_skip_certificate_validation {

    } else {
        validate_server_certificate(
            leaf,
            &intermediates,
            trust_store,
            8,
            hostname,
            std::time::SystemTime::now(),
        )?;
    }
    if profile {
        eprintln!(
            "[wc-ext-14] chain_walk total: {:?} ({} intermediates + 1 leaf)",
            t0.elapsed(),
            intermediates.len()
        );
    }

    let transcript_sf = transcript_through_finished.ok_or(TlsError::SignatureFail(
        "encrypted handshake ended before Finished".into(),
    ))?;
    let th_sf = hash.digest(&transcript_sf);
    let client_app_secret = schedule.client_application_traffic(&th_sf)?;
    let server_app_secret = schedule.server_application_traffic(&th_sf)?;
    let client_app_keys = derive_traffic_keys(hash, &client_app_secret, 16, 12)?;
    let server_app_keys = derive_traffic_keys(hash, &server_app_secret, 16, 12)?;

    let client_finished_mac = finished_mac(hash, &client_hs_secret, &th_sf)?;
    let cf_msg = HandshakeMessage {
        msg_type: HandshakeType::Finished,
        body: client_finished_mac,
    };
    let cf_bytes = encode_handshake(&cf_msg);
    let cf_ct = aead_encrypt_record(&client_hs_keys, 0, ContentType::Handshake as u8, &cf_bytes)?;
    let cf_record = TlsRecord {
        content_type: ContentType::ApplicationData,
        version: ProtocolVersion::LEGACY,
        fragment: cf_ct,
    };
    transport.write_all(&encode_record(&cf_record)?)?;

    Ok(TlsSession {
        transport,
        client_app_keys,
        server_app_keys,
        client_app_seq: 0,
        server_app_seq: 0,
        hash,
        tls12: false,
    })
}

pub fn parse_certificate_message(body: &[u8]) -> Result<Vec<X509Cert>, TlsError> {
    if body.is_empty() {
        return Err(TlsError::UnexpectedEnd);
    }
    let ctx_len = body[0] as usize;
    if body.len() < 1 + ctx_len + 3 {
        return Err(TlsError::UnexpectedEnd);
    }
    let list_start = 1 + ctx_len;
    let list_len = ((body[list_start] as usize) << 16)
        | ((body[list_start + 1] as usize) << 8)
        | (body[list_start + 2] as usize);
    let mut pos = list_start + 3;
    let list_end = pos + list_len;
    if body.len() < list_end {
        return Err(TlsError::UnexpectedEnd);
    }
    let mut certs = Vec::new();
    while pos < list_end {
        if body.len() < pos + 3 {
            return Err(TlsError::UnexpectedEnd);
        }
        let cert_len = ((body[pos] as usize) << 16)
            | ((body[pos + 1] as usize) << 8)
            | (body[pos + 2] as usize);
        pos += 3;
        if body.len() < pos + cert_len {
            return Err(TlsError::UnexpectedEnd);
        }
        let cert =
            rusty_x509::parse_certificate(&body[pos..pos + cert_len]).map_err(TlsError::X509)?;
        certs.push(cert);
        pos += cert_len;

        if body.len() < pos + 2 {
            return Err(TlsError::UnexpectedEnd);
        }
        let ext_len = ((body[pos] as usize) << 8) | (body[pos + 1] as usize);
        pos += 2 + ext_len;
    }
    Ok(certs)
}
