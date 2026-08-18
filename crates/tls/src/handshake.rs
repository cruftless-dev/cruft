
use crate::record::TlsError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeType {
    ClientHello = 1,
    ServerHello = 2,
    NewSessionTicket = 4,
    EndOfEarlyData = 5,
    EncryptedExtensions = 8,
    Certificate = 11,
    CertificateRequest = 13,
    CertificateVerify = 15,
    Finished = 20,
    KeyUpdate = 24,
    MessageHash = 254,
}

impl HandshakeType {
    pub fn from_u8(b: u8) -> Option<HandshakeType> {
        use HandshakeType::*;
        Some(match b {
            1 => ClientHello,
            2 => ServerHello,
            4 => NewSessionTicket,
            5 => EndOfEarlyData,
            8 => EncryptedExtensions,
            11 => Certificate,
            13 => CertificateRequest,
            15 => CertificateVerify,
            20 => Finished,
            24 => KeyUpdate,
            254 => MessageHash,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct HandshakeMessage {
    pub msg_type: HandshakeType,
    pub body: Vec<u8>,
}

pub fn encode_handshake(msg: &HandshakeMessage) -> Vec<u8> {
    let len = msg.body.len();
    let mut out = Vec::with_capacity(4 + len);
    out.push(msg.msg_type as u8);
    out.push(((len >> 16) & 0xFF) as u8);
    out.push(((len >> 8) & 0xFF) as u8);
    out.push((len & 0xFF) as u8);
    out.extend_from_slice(&msg.body);
    out
}

pub fn decode_handshake(buf: &[u8]) -> Result<(HandshakeMessage, usize), TlsError> {
    if buf.len() < 4 {
        return Err(TlsError::UnexpectedEnd);
    }
    let msg_type = HandshakeType::from_u8(buf[0]).ok_or(TlsError::UnknownContentType(buf[0]))?;
    let len = ((buf[1] as usize) << 16) | ((buf[2] as usize) << 8) | (buf[3] as usize);
    if buf.len() < 4 + len {
        return Err(TlsError::UnexpectedEnd);
    }
    let body = buf[4..4 + len].to_vec();
    Ok((HandshakeMessage { msg_type, body }, 4 + len))
}

#[derive(Debug, Clone, Copy)]
pub enum HashAlgorithm {
    Sha256,
    Sha384,
}

impl HashAlgorithm {
    pub fn output_len(&self) -> usize {
        match self {
            HashAlgorithm::Sha256 => 32,
            HashAlgorithm::Sha384 => 48,
        }
    }

    pub fn digest(&self, data: &[u8]) -> Vec<u8> {
        match self {
            HashAlgorithm::Sha256 => rusty_web_crypto::digest_sha256(data).to_vec(),
            HashAlgorithm::Sha384 => rusty_web_crypto::digest_sha384(data).to_vec(),
        }
    }

    pub fn empty_hash(&self) -> Vec<u8> {
        self.digest(&[])
    }

    pub fn hkdf_extract(&self, salt: &[u8], ikm: &[u8]) -> Vec<u8> {

        match self {
            HashAlgorithm::Sha256 => rusty_web_crypto::hmac_sha256(salt, ikm).to_vec(),
            HashAlgorithm::Sha384 => rusty_web_crypto::hmac_sha384(salt, ikm).to_vec(),
        }
    }

    pub fn hkdf_expand(&self, prk: &[u8], info: &[u8], length: usize) -> Result<Vec<u8>, String> {
        let hash_len = self.output_len();
        if length > 255 * hash_len {
            return Err("HKDF-Expand: length too large".into());
        }
        let mut out = Vec::with_capacity(length);
        let mut t_prev: Vec<u8> = Vec::new();
        let n = (length + hash_len - 1) / hash_len;
        for i in 1..=n {
            let mut input = Vec::with_capacity(t_prev.len() + info.len() + 1);
            input.extend_from_slice(&t_prev);
            input.extend_from_slice(info);
            input.push(i as u8);
            let t_i: Vec<u8> = match self {
                HashAlgorithm::Sha256 => rusty_web_crypto::hmac_sha256(prk, &input).to_vec(),
                HashAlgorithm::Sha384 => rusty_web_crypto::hmac_sha384(prk, &input).to_vec(),
            };
            out.extend_from_slice(&t_i);
            t_prev = t_i;
        }
        out.truncate(length);
        Ok(out)
    }
}

pub fn hkdf_expand_label(
    hash: HashAlgorithm,
    secret: &[u8],
    label: &[u8],
    context: &[u8],
    length: u16,
) -> Result<Vec<u8>, TlsError> {
    let full_label_len = b"tls13 ".len() + label.len();
    if full_label_len > 255 {
        return Err(TlsError::SignatureFail("label too long".into()));
    }
    if context.len() > 255 {
        return Err(TlsError::SignatureFail("context too long".into()));
    }
    let mut info = Vec::with_capacity(2 + 1 + full_label_len + 1 + context.len());
    info.push((length >> 8) as u8);
    info.push((length & 0xFF) as u8);
    info.push(full_label_len as u8);
    info.extend_from_slice(b"tls13 ");
    info.extend_from_slice(label);
    info.push(context.len() as u8);
    info.extend_from_slice(context);
    hash.hkdf_expand(secret, &info, length as usize)
        .map_err(|e| TlsError::SignatureFail(e))
}

pub fn derive_secret(
    hash: HashAlgorithm,
    secret: &[u8],
    label: &[u8],
    transcript_hash: &[u8],
) -> Result<Vec<u8>, TlsError> {
    hkdf_expand_label(
        hash,
        secret,
        label,
        transcript_hash,
        hash.output_len() as u16,
    )
}

pub struct KeySchedule {
    pub hash: HashAlgorithm,
    pub early_secret: Vec<u8>,
    pub handshake_secret: Vec<u8>,
    pub master_secret: Vec<u8>,
}

impl KeySchedule {

    pub fn new(hash: HashAlgorithm, dhe: &[u8], transcript_hello: &[u8]) -> Result<Self, TlsError> {
        let zeros = vec![0u8; hash.output_len()];

        let early_secret = hash.hkdf_extract(&zeros, &zeros);

        let empty_hash = hash.empty_hash();
        let derived_early = hkdf_expand_label(
            hash,
            &early_secret,
            b"derived",
            &empty_hash,
            hash.output_len() as u16,
        )?;

        let handshake_secret = hash.hkdf_extract(&derived_early, dhe);

        let derived_handshake = hkdf_expand_label(
            hash,
            &handshake_secret,
            b"derived",
            &empty_hash,
            hash.output_len() as u16,
        )?;

        let master_secret = hash.hkdf_extract(&derived_handshake, &zeros);
        let _ = transcript_hello;
        Ok(KeySchedule {
            hash,
            early_secret,
            handshake_secret,
            master_secret,
        })
    }

    pub fn client_handshake_traffic(
        &self,
        transcript_through_sh: &[u8],
    ) -> Result<Vec<u8>, TlsError> {
        derive_secret(
            self.hash,
            &self.handshake_secret,
            b"c hs traffic",
            transcript_through_sh,
        )
    }

    pub fn server_handshake_traffic(
        &self,
        transcript_through_sh: &[u8],
    ) -> Result<Vec<u8>, TlsError> {
        derive_secret(
            self.hash,
            &self.handshake_secret,
            b"s hs traffic",
            transcript_through_sh,
        )
    }

    pub fn client_application_traffic(
        &self,
        transcript_through_sf: &[u8],
    ) -> Result<Vec<u8>, TlsError> {
        derive_secret(
            self.hash,
            &self.master_secret,
            b"c ap traffic",
            transcript_through_sf,
        )
    }

    pub fn server_application_traffic(
        &self,
        transcript_through_sf: &[u8],
    ) -> Result<Vec<u8>, TlsError> {
        derive_secret(
            self.hash,
            &self.master_secret,
            b"s ap traffic",
            transcript_through_sf,
        )
    }
}

#[derive(Clone)]
pub struct TrafficKeys {

    pub key: Vec<u8>,

    pub iv: Vec<u8>,
}

pub fn derive_traffic_keys(
    hash: HashAlgorithm,
    traffic_secret: &[u8],
    key_len: usize,
    iv_len: usize,
) -> Result<TrafficKeys, TlsError> {
    let key = hkdf_expand_label(hash, traffic_secret, b"key", &[], key_len as u16)?;
    let iv = hkdf_expand_label(hash, traffic_secret, b"iv", &[], iv_len as u16)?;
    Ok(TrafficKeys { key, iv })
}

pub fn record_nonce(iv: &[u8], seq: u64) -> Vec<u8> {
    let mut nonce = iv.to_vec();
    let n = nonce.len();

    for i in 0..8 {
        let shift = 8 * (7 - i);
        let byte = ((seq >> shift) & 0xFF) as u8;
        nonce[n - 8 + i] ^= byte;
    }
    nonce
}

pub fn aead_encrypt_record(
    keys: &TrafficKeys,
    seq: u64,
    inner_content_type: u8,
    plaintext: &[u8],
) -> Result<Vec<u8>, TlsError> {
    let mut inner = Vec::with_capacity(plaintext.len() + 1);
    inner.extend_from_slice(plaintext);
    inner.push(inner_content_type);
    let nonce = record_nonce(&keys.iv, seq);

    let ct_len = inner.len() + 16;
    let mut aad = vec![0x17, 0x03, 0x03];
    aad.push(((ct_len >> 8) & 0xFF) as u8);
    aad.push((ct_len & 0xFF) as u8);
    rusty_web_crypto::aes_gcm_encrypt(&keys.key, &nonce, &aad, &inner)
        .map_err(|e| TlsError::SignatureFail(e))
}

pub fn aead_decrypt_record(
    keys: &TrafficKeys,
    seq: u64,
    ct_fragment: &[u8],
) -> Result<(u8, Vec<u8>), TlsError> {
    let nonce = record_nonce(&keys.iv, seq);
    let mut aad = vec![0x17, 0x03, 0x03];
    aad.push(((ct_fragment.len() >> 8) & 0xFF) as u8);
    aad.push((ct_fragment.len() & 0xFF) as u8);
    let inner = rusty_web_crypto::aes_gcm_decrypt(&keys.key, &nonce, &aad, ct_fragment)
        .map_err(|e| TlsError::SignatureFail(e))?;
    if inner.is_empty() {
        return Err(TlsError::UnexpectedEnd);
    }

    let mut end = inner.len();
    while end > 0 && inner[end - 1] == 0 {
        end -= 1;
    }
    if end == 0 {
        return Err(TlsError::UnexpectedEnd);
    }
    let content_type = inner[end - 1];
    let plaintext = inner[..end - 1].to_vec();
    Ok((content_type, plaintext))
}

pub fn finished_mac(
    hash: HashAlgorithm,
    secret: &[u8],
    transcript_hash: &[u8],
) -> Result<Vec<u8>, TlsError> {
    let finished_key = hkdf_expand_label(hash, secret, b"finished", &[], hash.output_len() as u16)?;
    Ok(match hash {
        HashAlgorithm::Sha256 => {
            rusty_web_crypto::hmac_sha256(&finished_key, transcript_hash).to_vec()
        }
        HashAlgorithm::Sha384 => {
            rusty_web_crypto::hmac_sha384(&finished_key, transcript_hash).to_vec()
        }
    })
}

pub fn finished_verify_data_equal(actual: &[u8], expected: &[u8]) -> bool {
    rusty_web_crypto::timing_safe_equal(actual, expected)
}
