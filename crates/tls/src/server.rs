
use crate::client::{
    decode_extensions, push_extension, CIPHER_AES_128_GCM_SHA256, EXT_ALPN, EXT_KEY_SHARE,
    EXT_SUPPORTED_VERSIONS, GROUP_SECP256R1, GROUP_X25519, SIG_ECDSA_SECP256R1_SHA256,
    SIG_RSA_PSS_RSAE_SHA256,
};

pub fn select_alpn(client: &[Vec<u8>], server: &[Vec<u8>]) -> Option<Vec<u8>> {
    server
        .iter()
        .find(|s| client.iter().any(|c| c == *s))
        .cloned()
}

fn alpn_extension(proto: &[u8]) -> (u16, Vec<u8>) {
    let mut data = Vec::with_capacity(3 + proto.len());
    let list_len = 1 + proto.len();
    data.push((list_len >> 8) as u8);
    data.push((list_len & 0xFF) as u8);
    data.push(proto.len() as u8);
    data.extend_from_slice(proto);
    (EXT_ALPN, data)
}

pub enum ServerEphemeral {
    P256(crate::driver::EphemeralEcdh),
    X25519 { scalar: [u8; 32], public: Vec<u8> },
}

impl ServerEphemeral {
    pub fn generate(group: u16) -> Result<Self, TlsError> {
        match group {
            GROUP_SECP256R1 => Ok(ServerEphemeral::P256(
                crate::driver::EphemeralEcdh::generate()?,
            )),
            GROUP_X25519 => {
                let mut scalar = [0u8; 32];
                rusty_web_crypto::get_random_values(&mut scalar)
                    .map_err(|e| TlsError::SignatureFail(format!("RNG: {}", e)))?;

                let public = rusty_web_crypto::x25519_base(&scalar);
                Ok(ServerEphemeral::X25519 { scalar, public })
            }
            _ => Err(TlsError::SignatureFail(format!(
                "server: unsupported key-share group 0x{:04x}",
                group
            ))),
        }
    }

    pub fn public_point(&self) -> &[u8] {
        match self {
            ServerEphemeral::P256(e) => &e.public_point,
            ServerEphemeral::X25519 { public, .. } => public,
        }
    }

    pub fn shared_secret(&self, client_pub: &[u8]) -> Result<Vec<u8>, TlsError> {
        match self {
            ServerEphemeral::P256(e) => e.shared_secret(client_pub),
            ServerEphemeral::X25519 { scalar, .. } => {
                Ok(rusty_web_crypto::x25519(scalar, client_pub))
            }
        }
    }
}
use crate::handshake::{
    derive_traffic_keys, encode_handshake, finished_mac, HandshakeMessage, HandshakeType,
    HashAlgorithm, KeySchedule, TrafficKeys,
};
use crate::record::TlsError;

#[derive(Debug, Clone)]
pub struct ClientHello {
    pub legacy_version: u16,
    pub random: [u8; 32],
    pub legacy_session_id: Vec<u8>,
    pub cipher_suites: Vec<u16>,
    pub extensions: Vec<(u16, Vec<u8>)>,
}

impl ClientHello {
    pub fn find_extension(&self, ext_type: u16) -> Option<&[u8]> {
        self.extensions
            .iter()
            .find(|(t, _)| *t == ext_type)
            .map(|(_, v)| v.as_slice())
    }

    pub fn key_shares(&self) -> Vec<(u16, Vec<u8>)> {
        let mut out = Vec::new();
        let v = match self.find_extension(EXT_KEY_SHARE) {
            Some(v) if v.len() >= 2 => v,
            _ => return out,
        };
        let list_len = ((v[0] as usize) << 8) | (v[1] as usize);
        let end = (2 + list_len).min(v.len());
        let mut pos = 2;
        while pos + 4 <= end {
            let group = ((v[pos] as u16) << 8) | (v[pos + 1] as u16);
            let klen = ((v[pos + 2] as usize) << 8) | (v[pos + 3] as usize);
            pos += 4;
            if pos + klen > end {
                break;
            }
            out.push((group, v[pos..pos + klen].to_vec()));
            pos += klen;
        }
        out
    }

    pub fn alpn_protocols(&self) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        let v = match self.find_extension(EXT_ALPN) {
            Some(v) if v.len() >= 2 => v,
            _ => return out,
        };
        let list_len = ((v[0] as usize) << 8) | (v[1] as usize);
        let end = (2 + list_len).min(v.len());
        let mut pos = 2;
        while pos < end {
            let plen = v[pos] as usize;
            pos += 1;
            if pos + plen > end {
                break;
            }
            out.push(v[pos..pos + plen].to_vec());
            pos += plen;
        }
        out
    }
}

pub fn decode_client_hello(body: &[u8]) -> Result<ClientHello, TlsError> {

    if body.len() < 2 + 32 + 1 {
        return Err(TlsError::UnexpectedEnd);
    }
    let legacy_version = ((body[0] as u16) << 8) | (body[1] as u16);
    let mut pos = 2;
    let mut random = [0u8; 32];
    random.copy_from_slice(&body[pos..pos + 32]);
    pos += 32;

    let sid_len = body[pos] as usize;
    pos += 1;
    if body.len() < pos + sid_len {
        return Err(TlsError::UnexpectedEnd);
    }
    let legacy_session_id = body[pos..pos + sid_len].to_vec();
    pos += sid_len;

    if body.len() < pos + 2 {
        return Err(TlsError::UnexpectedEnd);
    }
    let cs_bytes = ((body[pos] as usize) << 8) | (body[pos + 1] as usize);
    pos += 2;
    if body.len() < pos + cs_bytes || cs_bytes % 2 != 0 {
        return Err(TlsError::UnexpectedEnd);
    }
    let mut cipher_suites = Vec::with_capacity(cs_bytes / 2);
    let cs_end = pos + cs_bytes;
    while pos + 2 <= cs_end {
        cipher_suites.push(((body[pos] as u16) << 8) | (body[pos + 1] as u16));
        pos += 2;
    }

    if body.len() < pos + 1 {
        return Err(TlsError::UnexpectedEnd);
    }
    let comp_len = body[pos] as usize;
    pos += 1 + comp_len;

    let extensions = if body.len() >= pos + 2 {
        let exts_len = ((body[pos] as usize) << 8) | (body[pos + 1] as usize);
        pos += 2;
        if body.len() < pos + exts_len {
            return Err(TlsError::UnexpectedEnd);
        }
        decode_extensions(&body[pos..pos + exts_len])?
    } else {
        Vec::new()
    };
    Ok(ClientHello {
        legacy_version,
        random,
        legacy_session_id,
        cipher_suites,
        extensions,
    })
}

pub struct ServerHelloParams<'a> {
    pub random: &'a [u8; 32],

    pub legacy_session_id_echo: &'a [u8],
    pub cipher_suite: u16,
    pub key_share_group: u16,
    pub key_share_pubkey: &'a [u8],
}

pub fn encode_server_hello(p: &ServerHelloParams) -> Result<Vec<u8>, TlsError> {
    if p.legacy_session_id_echo.len() > 32 {
        return Err(TlsError::SignatureFail("session id too long".into()));
    }
    let mut body = Vec::new();
    body.extend_from_slice(&[0x03, 0x03]);
    body.extend_from_slice(p.random);
    body.push(p.legacy_session_id_echo.len() as u8);
    body.extend_from_slice(p.legacy_session_id_echo);
    body.push((p.cipher_suite >> 8) as u8);
    body.push((p.cipher_suite & 0xFF) as u8);
    body.push(0x00);

    let mut exts = Vec::new();
    push_extension(&mut exts, EXT_SUPPORTED_VERSIONS, &[0x03, 0x04]);
    let mut ks = Vec::new();
    ks.push((p.key_share_group >> 8) as u8);
    ks.push((p.key_share_group & 0xFF) as u8);
    ks.push(((p.key_share_pubkey.len() >> 8) & 0xFF) as u8);
    ks.push((p.key_share_pubkey.len() & 0xFF) as u8);
    ks.extend_from_slice(p.key_share_pubkey);
    push_extension(&mut exts, EXT_KEY_SHARE, &ks);

    body.push(((exts.len() >> 8) & 0xFF) as u8);
    body.push((exts.len() & 0xFF) as u8);
    body.extend_from_slice(&exts);

    let msg = HandshakeMessage {
        msg_type: HandshakeType::ServerHello,
        body,
    };
    Ok(encode_handshake(&msg))
}

pub fn select_cipher_suite(client_suites: &[u16], supported: &[u16]) -> Option<u16> {
    supported
        .iter()
        .copied()
        .find(|s| client_suites.contains(s))
}

pub fn select_key_share(
    client_shares: &[(u16, Vec<u8>)],
    supported_groups: &[u16],
) -> Option<(u16, Vec<u8>)> {
    for &g in supported_groups {
        if let Some((_, pk)) = client_shares.iter().find(|(cg, _)| *cg == g) {
            return Some((g, pk.clone()));
        }
    }
    None
}

pub fn suite_params(suite: u16) -> Result<(HashAlgorithm, usize), TlsError> {
    match suite {
        CIPHER_AES_128_GCM_SHA256 => Ok((HashAlgorithm::Sha256, 16)),
        _ => Err(TlsError::SignatureFail(format!(
            "server: unsupported cipher suite 0x{:04x}",
            suite
        ))),
    }
}

pub struct ServerHandshakeKeys {
    pub hash: HashAlgorithm,
    pub key_schedule: KeySchedule,
    pub server_hs_secret: Vec<u8>,
    pub client_hs_secret: Vec<u8>,
    pub write: TrafficKeys,
    pub read: TrafficKeys,
}

pub fn derive_server_handshake_keys(
    dhe: &[u8],
    transcript_through_sh: &[u8],
    suite: u16,
) -> Result<ServerHandshakeKeys, TlsError> {
    let (hash, key_len) = suite_params(suite)?;
    let ks = KeySchedule::new(hash, dhe, transcript_through_sh)?;
    let server_hs_secret = ks.server_handshake_traffic(transcript_through_sh)?;
    let client_hs_secret = ks.client_handshake_traffic(transcript_through_sh)?;
    let write = derive_traffic_keys(hash, &server_hs_secret, key_len, 12)?;
    let read = derive_traffic_keys(hash, &client_hs_secret, key_len, 12)?;
    Ok(ServerHandshakeKeys {
        hash,
        key_schedule: ks,
        server_hs_secret,
        client_hs_secret,
        write,
        read,
    })
}

pub fn encode_encrypted_extensions(exts: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut inner = Vec::new();
    for (t, v) in exts {
        push_extension(&mut inner, *t, v);
    }
    let mut body = Vec::with_capacity(2 + inner.len());
    body.push(((inner.len() >> 8) & 0xFF) as u8);
    body.push((inner.len() & 0xFF) as u8);
    body.extend_from_slice(&inner);
    encode_handshake(&HandshakeMessage {
        msg_type: HandshakeType::EncryptedExtensions,
        body,
    })
}

pub fn encode_certificate(cert_chain: &[&[u8]]) -> Vec<u8> {
    let mut entries = Vec::new();
    for cert in cert_chain {
        let n = cert.len();
        entries.push(((n >> 16) & 0xFF) as u8);
        entries.push(((n >> 8) & 0xFF) as u8);
        entries.push((n & 0xFF) as u8);
        entries.extend_from_slice(cert);
        entries.extend_from_slice(&[0x00, 0x00]);
    }
    let mut body = Vec::new();
    body.push(0x00);
    body.push(((entries.len() >> 16) & 0xFF) as u8);
    body.push(((entries.len() >> 8) & 0xFF) as u8);
    body.push((entries.len() & 0xFF) as u8);
    body.extend_from_slice(&entries);
    encode_handshake(&HandshakeMessage {
        msg_type: HandshakeType::Certificate,
        body,
    })
}

pub fn certificate_verify_tbs(transcript_hash_through_cert: &[u8]) -> Vec<u8> {
    let mut tbs = Vec::with_capacity(64 + 33 + 1 + transcript_hash_through_cert.len());
    tbs.extend_from_slice(&[0x20u8; 64]);
    tbs.extend_from_slice(b"TLS 1.3, server CertificateVerify");
    tbs.push(0x00);
    tbs.extend_from_slice(transcript_hash_through_cert);
    tbs
}

pub fn encode_certificate_verify(scheme: u16, signature: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(4 + signature.len());
    body.push((scheme >> 8) as u8);
    body.push((scheme & 0xFF) as u8);
    body.push(((signature.len() >> 8) & 0xFF) as u8);
    body.push((signature.len() & 0xFF) as u8);
    body.extend_from_slice(signature);
    encode_handshake(&HandshakeMessage {
        msg_type: HandshakeType::CertificateVerify,
        body,
    })
}

pub fn sign_certificate_verify_ecdsa_p256(d_bytes: &[u8], tbs: &[u8]) -> Result<Vec<u8>, TlsError> {
    let raw = rusty_web_crypto::ecdsa_p256_sha256_sign_deterministic(d_bytes, tbs)
        .map_err(TlsError::SignatureFail)?;
    if raw.len() != 64 {
        return Err(TlsError::SignatureFail("ECDSA signature size".into()));
    }
    Ok(rusty_asn1_der::enc_sequence(&[
        rusty_asn1_der::enc_integer_unsigned(&raw[..32]),
        rusty_asn1_der::enc_integer_unsigned(&raw[32..]),
    ]))
}

pub fn encode_finished(
    hash: HashAlgorithm,
    server_hs_secret: &[u8],
    transcript_hash_through_cert_verify: &[u8],
) -> Result<Vec<u8>, TlsError> {
    let verify_data = finished_mac(hash, server_hs_secret, transcript_hash_through_cert_verify)?;
    Ok(encode_handshake(&HandshakeMessage {
        msg_type: HandshakeType::Finished,
        body: verify_data,
    }))
}

pub fn parse_cert_chain_pem(pem: &str) -> Vec<Vec<u8>> {
    rusty_x509::pem_all_to_der(pem)
}

pub fn parse_ec_p256_private_key_pem(pem: &str) -> Result<Vec<u8>, TlsError> {

    let der = pem_block_to_der(pem)?;
    parse_ec_p256_private_key_der(&der)
}

fn pem_block_to_der(pem: &str) -> Result<Vec<u8>, TlsError> {

    let begin = pem
        .match_indices("-----BEGIN ")
        .find(|(i, _)| {
            pem[*i..]
                .lines()
                .next()
                .map(|l| l.contains("PRIVATE KEY"))
                .unwrap_or(false)
        })
        .map(|(i, _)| i)
        .or_else(|| pem.find("-----BEGIN "))
        .ok_or_else(|| TlsError::SignatureFail("PEM: missing BEGIN".into()))?;
    let body_start = pem[begin..]
        .find('\n')
        .map(|n| begin + n + 1)
        .ok_or_else(|| TlsError::SignatureFail("PEM: malformed header".into()))?;
    let end = pem[body_start..]
        .find("-----END ")
        .map(|e| body_start + e)
        .ok_or_else(|| TlsError::SignatureFail("PEM: missing END".into()))?;
    let b64: String = pem[body_start..end]
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    pem_base64_decode(&b64)
}

fn pem_base64_decode(s: &str) -> Result<Vec<u8>, TlsError> {
    const BAD: u8 = 255;
    let mut table = [BAD; 256];
    for (i, c) in b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
        .iter()
        .enumerate()
    {
        table[*c as usize] = i as u8;
    }
    let bytes: Vec<u8> = s.bytes().filter(|b| *b != b'=').collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    let mut acc = 0u32;
    let mut nbits = 0u32;
    for &b in &bytes {
        let v = table[b as usize];
        if v == BAD {
            return Err(TlsError::SignatureFail("PEM: invalid base64".into()));
        }
        acc = (acc << 6) | v as u32;
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            out.push((acc >> nbits) as u8);
        }
    }
    Ok(out)
}

pub fn parse_ec_p256_private_key_der(der: &[u8]) -> Result<Vec<u8>, TlsError> {
    let seq = rusty_asn1_der::parse_single(der)
        .map_err(|e| TlsError::SignatureFail(format!("EC key DER: {}", e)))?;
    if seq.tag != rusty_asn1_der::TAG_SEQUENCE {
        return Err(TlsError::SignatureFail("EC key: outer not SEQUENCE".into()));
    }
    let mut rd = rusty_asn1_der::DerReader::new(seq.content);

    rd.read_tag(rusty_asn1_der::TAG_INTEGER)
        .map_err(|e| TlsError::SignatureFail(format!("EC key version: {}", e)))?;
    let second = rd
        .read_tlv()
        .map_err(|e| TlsError::SignatureFail(format!("EC key body: {}", e)))?;
    match second.tag {

        rusty_asn1_der::TAG_OCTET_STRING => normalize_p256_scalar(second.content),

        rusty_asn1_der::TAG_SEQUENCE => {
            let pk = rd
                .read_tag(rusty_asn1_der::TAG_OCTET_STRING)
                .map_err(|e| TlsError::SignatureFail(format!("PKCS#8 privateKey: {}", e)))?;
            parse_ec_p256_private_key_der(pk.content)
        }
        _ => Err(TlsError::SignatureFail(
            "EC key: unexpected structure after version".into(),
        )),
    }
}

fn normalize_p256_scalar(bytes: &[u8]) -> Result<Vec<u8>, TlsError> {
    if bytes.is_empty() || bytes.len() > 32 {
        return Err(TlsError::SignatureFail(format!(
            "EC key: P-256 scalar must be 1..=32 bytes (got {})",
            bytes.len()
        )));
    }
    let mut out = vec![0u8; 32];
    out[32 - bytes.len()..].copy_from_slice(bytes);
    Ok(out)
}

fn strip_int_sign(bytes: &[u8]) -> &[u8] {
    if bytes.len() > 1 && bytes[0] == 0x00 {
        &bytes[1..]
    } else {
        bytes
    }
}

pub fn parse_rsa_private_key_pem(pem: &str) -> Result<(Vec<u8>, Vec<u8>), TlsError> {
    let der = pem_block_to_der(pem)?;
    parse_rsa_private_key_der(&der)
}

pub fn parse_rsa_private_key_der(der: &[u8]) -> Result<(Vec<u8>, Vec<u8>), TlsError> {
    let seq = rusty_asn1_der::parse_single(der)
        .map_err(|e| TlsError::SignatureFail(format!("RSA key DER: {}", e)))?;
    if seq.tag != rusty_asn1_der::TAG_SEQUENCE {
        return Err(TlsError::SignatureFail(
            "RSA key: outer not SEQUENCE".into(),
        ));
    }
    let mut rd = rusty_asn1_der::DerReader::new(seq.content);
    rd.read_tag(rusty_asn1_der::TAG_INTEGER)
        .map_err(|e| TlsError::SignatureFail(format!("RSA key version: {}", e)))?;
    let second = rd
        .read_tlv()
        .map_err(|e| TlsError::SignatureFail(format!("RSA key body: {}", e)))?;
    match second.tag {

        rusty_asn1_der::TAG_SEQUENCE => {
            let pk = rd
                .read_tag(rusty_asn1_der::TAG_OCTET_STRING)
                .map_err(|e| TlsError::SignatureFail(format!("PKCS#8 RSA privateKey: {}", e)))?;
            parse_rsa_private_key_der(pk.content)
        }

        rusty_asn1_der::TAG_INTEGER => {
            let n = strip_int_sign(second.content).to_vec();
            rd.read_tag(rusty_asn1_der::TAG_INTEGER)
                .map_err(|e| TlsError::SignatureFail(format!("RSA publicExponent: {}", e)))?;
            let d = rd
                .read_tag(rusty_asn1_der::TAG_INTEGER)
                .map_err(|e| TlsError::SignatureFail(format!("RSA privateExponent: {}", e)))?;
            Ok((n, strip_int_sign(d.content).to_vec()))
        }
        _ => Err(TlsError::SignatureFail(
            "RSA key: unexpected structure after version".into(),
        )),
    }
}

pub fn sign_certificate_verify_rsa_pss(
    n: &[u8],
    d: &[u8],
    tbs: &[u8],
) -> Result<Vec<u8>, TlsError> {
    let mut salt = [0u8; 32];
    let _ = rusty_web_crypto::get_random_values(&mut salt);
    rusty_web_crypto::rsa_pss_sign(
        n,
        d,
        tbs,
        &salt,
        |m| rusty_web_crypto::digest_sha256(m).to_vec(),
        32,
    )
    .map_err(|e| TlsError::SignatureFail(format!("RSA-PSS CertificateVerify sign: {e}")))
}

#[derive(Clone)]
pub struct ServerConfig {

    pub cert_chain: Vec<Vec<u8>>,

    pub signing_key: Vec<u8>,

    pub rsa_key: Option<(Vec<u8>, Vec<u8>)>,

    pub suites: Vec<u16>,
    pub groups: Vec<u16>,

    pub alpn_protocols: Vec<Vec<u8>>,
}

pub struct ServerHandshakeResult {
    pub server_hello: Vec<u8>,
    pub encrypted_flight: Vec<Vec<u8>>,
    pub hs_keys: ServerHandshakeKeys,
    pub server_app: TrafficKeys,
    pub client_app: TrafficKeys,

    pub expected_client_finished: Vec<u8>,

    pub selected_alpn: Option<Vec<u8>>,
}

pub fn server_handshake(
    config: &ServerConfig,
    client_hello_msg: &[u8],
    server_random: &[u8; 32],
) -> Result<ServerHandshakeResult, TlsError> {

    let (hs, _) = crate::handshake::decode_handshake(client_hello_msg)?;
    if hs.msg_type != HandshakeType::ClientHello {
        return Err(TlsError::SignatureFail("expected ClientHello".into()));
    }
    let ch = decode_client_hello(&hs.body)?;
    let suite = select_cipher_suite(&ch.cipher_suites, &config.suites)
        .ok_or_else(|| TlsError::SignatureFail("no mutually-supported cipher suite".into()))?;
    let (group, client_pub) = select_key_share(&ch.key_shares(), &config.groups)
        .ok_or_else(|| TlsError::SignatureFail("no mutually-supported key share".into()))?;
    let (hash, _key_len) = suite_params(suite)?;

    let server_eph = ServerEphemeral::generate(group)?;
    let dhe = server_eph.shared_secret(&client_pub)?;
    let server_hello = encode_server_hello(&ServerHelloParams {
        random: server_random,
        legacy_session_id_echo: &ch.legacy_session_id,
        cipher_suite: suite,
        key_share_group: group,
        key_share_pubkey: server_eph.public_point(),
    })?;

    let mut transcript = client_hello_msg.to_vec();
    transcript.extend_from_slice(&server_hello);
    let hs_keys = derive_server_handshake_keys(&dhe, &hash.digest(&transcript), suite)?;

    let selected_alpn = select_alpn(&ch.alpn_protocols(), &config.alpn_protocols);
    let ee_exts: Vec<(u16, Vec<u8>)> = match &selected_alpn {
        Some(p) => vec![alpn_extension(p)],
        None => Vec::new(),
    };
    let ee = encode_encrypted_extensions(&ee_exts);
    transcript.extend_from_slice(&ee);
    let cert_refs: Vec<&[u8]> = config.cert_chain.iter().map(|c| c.as_slice()).collect();
    let cert = encode_certificate(&cert_refs);
    transcript.extend_from_slice(&cert);

    let tbs = certificate_verify_tbs(&hash.digest(&transcript));
    let (cv_scheme, sig) = if let Some((n, d)) = config.rsa_key.as_ref() {
        (
            SIG_RSA_PSS_RSAE_SHA256,
            sign_certificate_verify_rsa_pss(n, d, &tbs)?,
        )
    } else {
        (
            SIG_ECDSA_SECP256R1_SHA256,
            sign_certificate_verify_ecdsa_p256(&config.signing_key, &tbs)?,
        )
    };
    let cv = encode_certificate_verify(cv_scheme, &sig);
    transcript.extend_from_slice(&cv);

    let fin = encode_finished(hash, &hs_keys.server_hs_secret, &hash.digest(&transcript))?;
    transcript.extend_from_slice(&fin);

    let transcript_sf = hash.digest(&transcript);
    let server_app_secret = hs_keys
        .key_schedule
        .server_application_traffic(&transcript_sf)?;
    let client_app_secret = hs_keys
        .key_schedule
        .client_application_traffic(&transcript_sf)?;
    let (_h, key_len) = suite_params(suite)?;
    let server_app = derive_traffic_keys(hash, &server_app_secret, key_len, 12)?;
    let client_app = derive_traffic_keys(hash, &client_app_secret, key_len, 12)?;

    let expected_client_finished = finished_mac(hash, &hs_keys.client_hs_secret, &transcript_sf)?;

    let mut encrypted_flight = Vec::with_capacity(4);
    for (seq, msg) in [&ee, &cert, &cv, &fin].iter().enumerate() {
        encrypted_flight.push(crate::handshake::aead_encrypt_record(
            &hs_keys.write,
            seq as u64,
            22,
            msg,
        )?);
    }

    Ok(ServerHandshakeResult {
        server_hello,
        encrypted_flight,
        hs_keys,
        server_app,
        client_app,
        expected_client_finished,
        selected_alpn,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{
        encode_client_hello, ClientHelloParams, CIPHER_AES_128_GCM_SHA256, GROUP_SECP256R1,
        SIG_ECDSA_SECP256R1_SHA256,
    };
    use crate::driver::EphemeralEcdh;
    use crate::handshake::decode_handshake;

    #[test]
    fn server_parses_client_hello_and_ecdh_secret_matches() {

        let client_ecdh = EphemeralEcdh::generate().unwrap();
        let client_random = [7u8; 32];
        let session_id = [9u8; 32];
        let ch_bytes = encode_client_hello(&ClientHelloParams {
            random: &client_random,
            legacy_session_id: &session_id,
            cipher_suites: &[CIPHER_AES_128_GCM_SHA256],
            server_name: Some("localhost"),
            supported_groups: &[GROUP_SECP256R1],
            signature_algorithms: &[SIG_ECDSA_SECP256R1_SHA256],
            key_shares: &[(GROUP_SECP256R1, client_ecdh.public_point.clone())],
            alpn: None,
            session_ticket: None,
        })
        .unwrap();

        let (hs, _used) = decode_handshake(&ch_bytes).unwrap();
        assert_eq!(hs.msg_type, HandshakeType::ClientHello);
        let ch = decode_client_hello(&hs.body).unwrap();
        assert_eq!(ch.random, client_random);
        assert_eq!(ch.legacy_session_id, session_id.to_vec());
        assert_eq!(ch.cipher_suites, vec![CIPHER_AES_128_GCM_SHA256]);

        let suite = select_cipher_suite(&ch.cipher_suites, &[CIPHER_AES_128_GCM_SHA256]).unwrap();
        assert_eq!(suite, CIPHER_AES_128_GCM_SHA256);
        let (group, client_pub) = select_key_share(&ch.key_shares(), &[GROUP_SECP256R1]).unwrap();
        assert_eq!(group, GROUP_SECP256R1);
        assert_eq!(client_pub, client_ecdh.public_point);

        let server_ecdh = EphemeralEcdh::generate().unwrap();
        let server_random = [3u8; 32];
        let sh_bytes = encode_server_hello(&ServerHelloParams {
            random: &server_random,
            legacy_session_id_echo: &ch.legacy_session_id,
            cipher_suite: suite,
            key_share_group: group,
            key_share_pubkey: &server_ecdh.public_point,
        })
        .unwrap();

        let (shs, _u) = decode_handshake(&sh_bytes).unwrap();
        assert_eq!(shs.msg_type, HandshakeType::ServerHello);
        let sh = crate::client::decode_server_hello(&shs.body).unwrap();
        assert_eq!(sh.cipher_suite, suite);
        assert_eq!(sh.selected_version(), Some(0x0304));
        let (sh_group, sh_pub) = sh.server_key_share().unwrap();
        assert_eq!(sh_group, GROUP_SECP256R1);
        assert_eq!(sh_pub, &server_ecdh.public_point[..]);
        assert_eq!(sh.legacy_session_id_echo, ch.legacy_session_id);

        let server_secret = server_ecdh.shared_secret(&client_pub).unwrap();
        let client_secret = client_ecdh.shared_secret(sh_pub).unwrap();
        assert_eq!(server_secret, client_secret);
        assert!(!server_secret.is_empty());
    }

    #[test]
    fn server_handshake_keys_match_and_record_round_trips() {

        let client_ecdh = EphemeralEcdh::generate().unwrap();
        let server_ecdh = EphemeralEcdh::generate().unwrap();
        let suite = CIPHER_AES_128_GCM_SHA256;

        let ch_bytes = encode_client_hello(&ClientHelloParams {
            random: &[1u8; 32],
            legacy_session_id: &[2u8; 32],
            cipher_suites: &[suite],
            server_name: None,
            supported_groups: &[GROUP_SECP256R1],
            signature_algorithms: &[SIG_ECDSA_SECP256R1_SHA256],
            key_shares: &[(GROUP_SECP256R1, client_ecdh.public_point.clone())],
            alpn: None,
            session_ticket: None,
        })
        .unwrap();
        let ch = decode_client_hello(&decode_handshake(&ch_bytes).unwrap().0.body).unwrap();
        let (group, client_pub) = select_key_share(&ch.key_shares(), &[GROUP_SECP256R1]).unwrap();
        let sh_bytes = encode_server_hello(&ServerHelloParams {
            random: &[4u8; 32],
            legacy_session_id_echo: &ch.legacy_session_id,
            cipher_suite: suite,
            key_share_group: group,
            key_share_pubkey: &server_ecdh.public_point,
        })
        .unwrap();

        let mut transcript_msgs = ch_bytes.clone();
        transcript_msgs.extend_from_slice(&sh_bytes);
        let transcript = HashAlgorithm::Sha256.digest(&transcript_msgs);

        let dhe_server = server_ecdh.shared_secret(&client_pub).unwrap();

        let sk = derive_server_handshake_keys(&dhe_server, &transcript, suite).unwrap();

        let client_view = derive_server_handshake_keys(&dhe_server, &transcript, suite).unwrap();
        assert_eq!(sk.write.key, client_view.write.key);
        assert_eq!(sk.write.iv, client_view.write.iv);
        assert_eq!(sk.server_hs_secret, client_view.server_hs_secret);
        assert_ne!(sk.server_hs_secret, sk.client_hs_secret);

        let plaintext = b"\x08\x00\x00\x02\x00\x00";
        let ct = crate::handshake::aead_encrypt_record(&sk.write, 0, 22, plaintext).unwrap();
        let (content_type, recovered) =
            crate::handshake::aead_decrypt_record(&client_view.write, 0, &ct).unwrap();
        assert_eq!(content_type, 22);
        assert_eq!(recovered, plaintext);
    }

    #[test]
    fn server_flight_messages_frame_correctly() {
        use crate::handshake::{decode_handshake, finished_mac, HashAlgorithm};

        let ee = encode_encrypted_extensions(&[]);
        let (ee_hs, _) = decode_handshake(&ee).unwrap();
        assert_eq!(ee_hs.msg_type, HandshakeType::EncryptedExtensions);
        assert_eq!(ee_hs.body, vec![0x00, 0x00]);

        let cert = vec![0xDEu8, 0xAD, 0xBE, 0xEF];
        let c = encode_certificate(&[&cert]);
        let (c_hs, _) = decode_handshake(&c).unwrap();
        assert_eq!(c_hs.msg_type, HandshakeType::Certificate);

        assert_eq!(c_hs.body[0], 0x00);
        let list_len = ((c_hs.body[1] as usize) << 16)
            | ((c_hs.body[2] as usize) << 8)
            | (c_hs.body[3] as usize);
        assert_eq!(list_len, 3 + cert.len() + 2);
        let cl = ((c_hs.body[4] as usize) << 16)
            | ((c_hs.body[5] as usize) << 8)
            | (c_hs.body[6] as usize);
        assert_eq!(cl, cert.len());
        assert_eq!(&c_hs.body[7..7 + cert.len()], &cert[..]);

        let transcript = HashAlgorithm::Sha256.digest(b"some transcript");
        let tbs = certificate_verify_tbs(&transcript);
        assert_eq!(&tbs[..64], &[0x20u8; 64][..]);
        assert_eq!(&tbs[64..64 + 33], b"TLS 1.3, server CertificateVerify");
        assert_eq!(tbs[97], 0x00);
        assert_eq!(&tbs[98..], &transcript[..]);

        let sig = vec![1u8, 2, 3, 4, 5];
        let cv = encode_certificate_verify(SIG_ECDSA_SECP256R1_SHA256, &sig);
        let (cv_hs, _) = decode_handshake(&cv).unwrap();
        assert_eq!(cv_hs.msg_type, HandshakeType::CertificateVerify);
        let scheme = ((cv_hs.body[0] as u16) << 8) | (cv_hs.body[1] as u16);
        assert_eq!(scheme, SIG_ECDSA_SECP256R1_SHA256);
        let siglen = ((cv_hs.body[2] as usize) << 8) | (cv_hs.body[3] as usize);
        assert_eq!(siglen, sig.len());
        assert_eq!(&cv_hs.body[4..], &sig[..]);

        let server_hs_secret = vec![0x42u8; 32];
        let fin = encode_finished(HashAlgorithm::Sha256, &server_hs_secret, &transcript).unwrap();
        let (fin_hs, _) = decode_handshake(&fin).unwrap();
        assert_eq!(fin_hs.msg_type, HandshakeType::Finished);
        let expected = finished_mac(HashAlgorithm::Sha256, &server_hs_secret, &transcript).unwrap();
        assert_eq!(fin_hs.body, expected);
    }

    #[test]
    fn server_certificate_verify_signature_round_trips() {
        use crate::handshake::HashAlgorithm;

        let kp = EphemeralEcdh::generate().unwrap();
        let qx = &kp.public_point[1..33];
        let qy = &kp.public_point[33..65];

        let transcript = HashAlgorithm::Sha256.digest(b"server transcript through certificate");
        let tbs = certificate_verify_tbs(&transcript);

        let der_sig = sign_certificate_verify_ecdsa_p256(&kp.private_scalar, &tbs).unwrap();
        let cv = encode_certificate_verify(SIG_ECDSA_SECP256R1_SHA256, &der_sig);
        let (cv_hs, _) = decode_handshake(&cv).unwrap();
        let on_wire_sig = &cv_hs.body[4..];
        assert_eq!(on_wire_sig, &der_sig[..]);

        let seq = rusty_asn1_der::parse_single(&der_sig).unwrap();
        assert_eq!(seq.tag, rusty_asn1_der::TAG_SEQUENCE);
        let mut reader = rusty_asn1_der::DerReader::new(seq.content);
        let r = reader
            .read_tag(rusty_asn1_der::TAG_INTEGER)
            .unwrap()
            .as_unsigned_integer()
            .unwrap();
        let s = reader
            .read_tag(rusty_asn1_der::TAG_INTEGER)
            .unwrap()
            .as_unsigned_integer()
            .unwrap();
        let mut raw = vec![0u8; 64];
        raw[32 - r.len()..32].copy_from_slice(r);
        raw[64 - s.len()..64].copy_from_slice(s);
        rusty_web_crypto::ecdsa_p256_sha256_verify(qx, qy, &tbs, &raw)
            .expect("server CertificateVerify signature must verify against its public key");

        let other = certificate_verify_tbs(&HashAlgorithm::Sha256.digest(b"different"));
        assert!(rusty_web_crypto::ecdsa_p256_sha256_verify(qx, qy, &other, &raw).is_err());
    }

    #[test]
    fn full_in_memory_server_handshake_completes_and_app_data_flows() {
        use crate::handshake::{
            aead_decrypt_record, decode_handshake, derive_traffic_keys, finished_mac, HashAlgorithm,
        };

        let hash = HashAlgorithm::Sha256;
        let suite = CIPHER_AES_128_GCM_SHA256;

        let client_ecdh = EphemeralEcdh::generate().unwrap();
        let ch = encode_client_hello(&ClientHelloParams {
            random: &[5u8; 32],
            legacy_session_id: &[6u8; 32],
            cipher_suites: &[suite],
            server_name: Some("localhost"),
            supported_groups: &[GROUP_SECP256R1],
            signature_algorithms: &[SIG_ECDSA_SECP256R1_SHA256],
            key_shares: &[(GROUP_SECP256R1, client_ecdh.public_point.clone())],
            alpn: None,
            session_ticket: None,
        })
        .unwrap();

        let server_key = EphemeralEcdh::generate().unwrap();
        let config = ServerConfig {
            rsa_key: None,
            cert_chain: vec![vec![0xCAu8, 0xFE, 0xBA, 0xBE]],
            signing_key: server_key.private_scalar.clone(),
            suites: vec![suite],
            groups: vec![GROUP_SECP256R1],
            alpn_protocols: vec![],
        };
        let res = server_handshake(&config, &ch, &[8u8; 32]).unwrap();

        let (sh_hs, _) = decode_handshake(&res.server_hello).unwrap();
        let sh = crate::client::decode_server_hello(&sh_hs.body).unwrap();
        let (_g, server_pub) = sh.server_key_share().unwrap();
        let dhe_client = client_ecdh.shared_secret(server_pub).unwrap();
        let mut transcript = ch.clone();
        transcript.extend_from_slice(&res.server_hello);
        let client_hs =
            derive_server_handshake_keys(&dhe_client, &hash.digest(&transcript), suite).unwrap();

        assert_eq!(client_hs.write.key, res.hs_keys.write.key);

        let mut flight_msgs = Vec::new();
        for (seq, rec) in res.encrypted_flight.iter().enumerate() {
            let (ct, pt) = aead_decrypt_record(&client_hs.write, seq as u64, rec).unwrap();
            assert_eq!(ct, 22);
            flight_msgs.push(pt);
        }
        let (ee, cert, cv, fin) = (
            &flight_msgs[0],
            &flight_msgs[1],
            &flight_msgs[2],
            &flight_msgs[3],
        );
        assert_eq!(
            decode_handshake(ee).unwrap().0.msg_type,
            HandshakeType::EncryptedExtensions
        );

        transcript.extend_from_slice(ee);
        transcript.extend_from_slice(cert);
        let tbs = certificate_verify_tbs(&hash.digest(&transcript));
        let (cv_hs, _) = decode_handshake(cv).unwrap();
        let der_sig = &cv_hs.body[4..];
        let seq = rusty_asn1_der::parse_single(der_sig).unwrap();
        let mut rd = rusty_asn1_der::DerReader::new(seq.content);
        let r = rd
            .read_tag(rusty_asn1_der::TAG_INTEGER)
            .unwrap()
            .as_unsigned_integer()
            .unwrap();
        let s = rd
            .read_tag(rusty_asn1_der::TAG_INTEGER)
            .unwrap()
            .as_unsigned_integer()
            .unwrap();
        let mut raw = vec![0u8; 64];
        raw[32 - r.len()..32].copy_from_slice(r);
        raw[64 - s.len()..64].copy_from_slice(s);
        rusty_web_crypto::ecdsa_p256_sha256_verify(
            &server_key.public_point[1..33],
            &server_key.public_point[33..65],
            &tbs,
            &raw,
        )
        .expect("client must accept the server CertificateVerify");

        transcript.extend_from_slice(cv);
        let expected_fin =
            finished_mac(hash, &client_hs.server_hs_secret, &hash.digest(&transcript)).unwrap();
        let (fin_hs, _) = decode_handshake(fin).unwrap();
        assert_eq!(fin_hs.body, expected_fin);

        transcript.extend_from_slice(fin);
        let transcript_sf = hash.digest(&transcript);
        let client_server_app = derive_traffic_keys(
            hash,
            &client_hs
                .key_schedule
                .server_application_traffic(&transcript_sf)
                .unwrap(),
            16,
            12,
        )
        .unwrap();
        assert_eq!(client_server_app.key, res.server_app.key);
        assert_eq!(client_server_app.iv, res.server_app.iv);

        let app = b"hello over TLS 1.3";
        let rec = crate::handshake::aead_encrypt_record(&res.server_app, 0, 23, app).unwrap();
        let (ct, recovered) = aead_decrypt_record(&client_server_app, 0, &rec).unwrap();
        assert_eq!(ct, 23);
        assert_eq!(recovered, app);
    }

    #[test]
    fn server_negotiates_alpn() {
        assert_eq!(
            select_alpn(
                &[b"http/1.1".to_vec(), b"h2".to_vec()],
                &[b"h2".to_vec(), b"http/1.1".to_vec()]
            ),
            Some(b"h2".to_vec())
        );
        assert_eq!(select_alpn(&[b"spdy".to_vec()], &[b"h2".to_vec()]), None);

        let client_ecdh = EphemeralEcdh::generate().unwrap();
        let suite = CIPHER_AES_128_GCM_SHA256;
        let ch = encode_client_hello(&ClientHelloParams {
            random: &[1u8; 32],
            legacy_session_id: &[2u8; 32],
            cipher_suites: &[suite],
            server_name: None,
            supported_groups: &[GROUP_SECP256R1],
            signature_algorithms: &[SIG_ECDSA_SECP256R1_SHA256],
            key_shares: &[(GROUP_SECP256R1, client_ecdh.public_point.clone())],
            alpn: Some(&[b"h2", b"http/1.1"]),
            session_ticket: None,
        })
        .unwrap();
        let server_key = EphemeralEcdh::generate().unwrap();
        let config = ServerConfig {
            rsa_key: None,
            cert_chain: vec![vec![0xCAu8, 0xFE]],
            signing_key: server_key.private_scalar.clone(),
            suites: vec![suite],
            groups: vec![GROUP_SECP256R1],
            alpn_protocols: vec![b"http/1.1".to_vec()],
        };
        let res = server_handshake(&config, &ch, &[3u8; 32]).unwrap();
        assert_eq!(res.selected_alpn, Some(b"http/1.1".to_vec()));

        let parsed = decode_client_hello(&decode_handshake(&ch).unwrap().0.body).unwrap();
        assert_eq!(
            parsed.alpn_protocols(),
            vec![b"h2".to_vec(), b"http/1.1".to_vec()]
        );
    }

    #[test]
    fn parse_ec_private_key_sec1_and_pkcs8() {

        let kp = EphemeralEcdh::generate().unwrap();
        let scalar = kp.private_scalar.clone();

        let sec1 = rusty_asn1_der::enc_sequence(&[
            rusty_asn1_der::enc_integer_small(1),
            rusty_asn1_der::enc_tlv(rusty_asn1_der::TAG_OCTET_STRING, &scalar),
        ]);
        assert_eq!(parse_ec_p256_private_key_der(&sec1).unwrap(), scalar);

        let pkcs8 = rusty_asn1_der::enc_sequence(&[
            rusty_asn1_der::enc_integer_small(0),
            rusty_asn1_der::enc_sequence(&[]),
            rusty_asn1_der::enc_tlv(rusty_asn1_der::TAG_OCTET_STRING, &sec1),
        ]);
        assert_eq!(parse_ec_p256_private_key_der(&pkcs8).unwrap(), scalar);

        let parsed = parse_ec_p256_private_key_der(&sec1).unwrap();
        let (qx, qy) =
            rusty_web_crypto::ec_public_from_private(&rusty_web_crypto::curve_p256(), &parsed)
                .expect("derive public from parsed scalar");
        let tbs = certificate_verify_tbs(&HashAlgorithm::Sha256.digest(b"k"));
        let sig = sign_certificate_verify_ecdsa_p256(&parsed, &tbs).unwrap();
        let seq = rusty_asn1_der::parse_single(&sig).unwrap();
        let mut rd = rusty_asn1_der::DerReader::new(seq.content);
        let r = rd
            .read_tag(rusty_asn1_der::TAG_INTEGER)
            .unwrap()
            .as_unsigned_integer()
            .unwrap();
        let s = rd
            .read_tag(rusty_asn1_der::TAG_INTEGER)
            .unwrap()
            .as_unsigned_integer()
            .unwrap();
        let mut raw = vec![0u8; 64];
        raw[32 - r.len()..32].copy_from_slice(r);
        raw[64 - s.len()..64].copy_from_slice(s);
        rusty_web_crypto::ecdsa_p256_sha256_verify(&qx, &qy, &tbs, &raw)
            .expect("CertificateVerify from the parsed key must verify");
    }
}
