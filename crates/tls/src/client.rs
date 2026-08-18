
use crate::handshake::{encode_handshake, HandshakeMessage, HandshakeType};
use crate::record::TlsError;

pub const CIPHER_AES_128_GCM_SHA256: u16 = 0x1301;

pub const CIPHER_ECDHE_ECDSA_AES128_GCM_SHA256: u16 = 0xC02B;

pub const EXT_EC_POINT_FORMATS: u16 = 0x000B;
pub const CIPHER_AES_256_GCM_SHA384: u16 = 0x1302;
pub const CIPHER_CHACHA20_POLY1305_SHA256: u16 = 0x1303;

pub const GROUP_SECP256R1: u16 = 0x0017;
pub const GROUP_SECP384R1: u16 = 0x0018;
pub const GROUP_X25519: u16 = 0x001D;

pub const SIG_RSA_PKCS1_SHA256: u16 = 0x0401;
pub const SIG_RSA_PKCS1_SHA384: u16 = 0x0501;
pub const SIG_RSA_PKCS1_SHA512: u16 = 0x0601;
pub const SIG_ECDSA_SECP256R1_SHA256: u16 = 0x0403;
pub const SIG_ECDSA_SECP384R1_SHA384: u16 = 0x0503;
pub const SIG_RSA_PSS_RSAE_SHA256: u16 = 0x0804;
pub const SIG_RSA_PSS_RSAE_SHA384: u16 = 0x0805;

pub const EXT_SERVER_NAME: u16 = 0x0000;
pub const EXT_SUPPORTED_GROUPS: u16 = 0x000A;
pub const EXT_SIGNATURE_ALGORITHMS: u16 = 0x000D;
pub const EXT_ALPN: u16 = 0x0010;
pub const EXT_SUPPORTED_VERSIONS: u16 = 0x002B;
pub const EXT_KEY_SHARE: u16 = 0x0033;

pub const EXT_SESSION_TICKET: u16 = 0x0023;

pub struct ClientHelloParams<'a> {
    pub random: &'a [u8; 32],
    pub legacy_session_id: &'a [u8],
    pub cipher_suites: &'a [u16],
    pub server_name: Option<&'a str>,
    pub supported_groups: &'a [u16],
    pub signature_algorithms: &'a [u16],
    pub key_shares: &'a [(u16, Vec<u8>)],
    pub alpn: Option<&'a [&'a [u8]]>,

    pub session_ticket: Option<&'a [u8]>,
}

pub fn encode_client_hello(p: &ClientHelloParams) -> Result<Vec<u8>, TlsError> {
    let mut body = Vec::new();

    body.extend_from_slice(&[0x03, 0x03]);

    body.extend_from_slice(p.random);

    if p.legacy_session_id.len() > 32 {
        return Err(TlsError::SignatureFail("session id too long".into()));
    }
    body.push(p.legacy_session_id.len() as u8);
    body.extend_from_slice(p.legacy_session_id);

    let cs_len = 2 * p.cipher_suites.len();
    body.push((cs_len >> 8) as u8);
    body.push((cs_len & 0xFF) as u8);
    for &cs in p.cipher_suites {
        body.push((cs >> 8) as u8);
        body.push((cs & 0xFF) as u8);
    }

    body.push(0x01);
    body.push(0x00);

    let exts = encode_client_extensions(p)?;
    body.push(((exts.len() >> 8) & 0xFF) as u8);
    body.push((exts.len() & 0xFF) as u8);
    body.extend_from_slice(&exts);

    let msg = HandshakeMessage {
        msg_type: HandshakeType::ClientHello,
        body,
    };
    Ok(encode_handshake(&msg))
}

fn encode_client_extensions(p: &ClientHelloParams) -> Result<Vec<u8>, TlsError> {
    let mut exts = Vec::new();

    if let Some(host) = p.server_name {
        let host_bytes = host.as_bytes();
        let mut sn_ext = Vec::new();

        let entry_len = 1 + 2 + host_bytes.len();
        sn_ext.push((entry_len >> 8) as u8);
        sn_ext.push((entry_len & 0xFF) as u8);
        sn_ext.push(0x00);
        sn_ext.push(((host_bytes.len() >> 8) & 0xFF) as u8);
        sn_ext.push((host_bytes.len() & 0xFF) as u8);
        sn_ext.extend_from_slice(host_bytes);
        push_extension(&mut exts, EXT_SERVER_NAME, &sn_ext);
    }

    let mut sv = Vec::new();
    sv.push(4);
    sv.push(0x03);
    sv.push(0x04);
    sv.push(0x03);
    sv.push(0x03);
    push_extension(&mut exts, EXT_SUPPORTED_VERSIONS, &sv);

    push_extension(&mut exts, EXT_EC_POINT_FORMATS, &[1, 0]);

    if let Some(ticket) = p.session_ticket {
        push_extension(&mut exts, EXT_SESSION_TICKET, ticket);
    }

    let sg_len = 2 * p.supported_groups.len();
    let mut sg = Vec::new();
    sg.push((sg_len >> 8) as u8);
    sg.push((sg_len & 0xFF) as u8);
    for &g in p.supported_groups {
        sg.push((g >> 8) as u8);
        sg.push((g & 0xFF) as u8);
    }
    push_extension(&mut exts, EXT_SUPPORTED_GROUPS, &sg);

    let sa_len = 2 * p.signature_algorithms.len();
    let mut sa = Vec::new();
    sa.push((sa_len >> 8) as u8);
    sa.push((sa_len & 0xFF) as u8);
    for &a in p.signature_algorithms {
        sa.push((a >> 8) as u8);
        sa.push((a & 0xFF) as u8);
    }
    push_extension(&mut exts, EXT_SIGNATURE_ALGORITHMS, &sa);

    let mut ks = Vec::new();
    let mut entries = Vec::new();
    for (group, pubkey) in p.key_shares {
        entries.push((*group >> 8) as u8);
        entries.push((*group & 0xFF) as u8);
        entries.push(((pubkey.len() >> 8) & 0xFF) as u8);
        entries.push((pubkey.len() & 0xFF) as u8);
        entries.extend_from_slice(pubkey);
    }
    ks.push((entries.len() >> 8) as u8);
    ks.push((entries.len() & 0xFF) as u8);
    ks.extend_from_slice(&entries);
    push_extension(&mut exts, EXT_KEY_SHARE, &ks);

    if let Some(protos) = p.alpn {
        let mut alpn = Vec::new();
        let mut entries = Vec::new();
        for proto in protos {
            entries.push(proto.len() as u8);
            entries.extend_from_slice(proto);
        }
        alpn.push((entries.len() >> 8) as u8);
        alpn.push((entries.len() & 0xFF) as u8);
        alpn.extend_from_slice(&entries);
        push_extension(&mut exts, EXT_ALPN, &alpn);
    }

    Ok(exts)
}

pub(crate) fn push_extension(out: &mut Vec<u8>, ext_type: u16, body: &[u8]) {
    out.push((ext_type >> 8) as u8);
    out.push((ext_type & 0xFF) as u8);
    out.push(((body.len() >> 8) & 0xFF) as u8);
    out.push((body.len() & 0xFF) as u8);
    out.extend_from_slice(body);
}

#[derive(Debug, Clone)]
pub struct ServerHello {

    pub legacy_version: u16,
    pub random: [u8; 32],
    pub legacy_session_id_echo: Vec<u8>,
    pub cipher_suite: u16,
    pub legacy_compression_method: u8,
    pub extensions: Vec<(u16, Vec<u8>)>,
}

impl ServerHello {
    pub fn has_tls13_downgrade_sentinel(&self) -> bool {
        let marker = &self.random[24..32];
        marker == b"DOWNGRD\x01" || marker == b"DOWNGRD\x00"
    }

    pub fn find_extension(&self, ext_type: u16) -> Option<&[u8]> {
        self.extensions
            .iter()
            .find(|(t, _)| *t == ext_type)
            .map(|(_, v)| v.as_slice())
    }

    pub fn selected_version(&self) -> Option<u16> {

        if let Some(v) = self.find_extension(EXT_SUPPORTED_VERSIONS) {
            if v.len() == 2 {
                return Some(((v[0] as u16) << 8) | (v[1] as u16));
            }
        }
        Some(self.legacy_version)
    }

    pub fn server_key_share(&self) -> Option<(u16, &[u8])> {
        let v = self.find_extension(EXT_KEY_SHARE)?;
        if v.len() < 4 {
            return None;
        }
        let group = ((v[0] as u16) << 8) | (v[1] as u16);
        let len = ((v[2] as usize) << 8) | (v[3] as usize);
        if v.len() < 4 + len {
            return None;
        }
        Some((group, &v[4..4 + len]))
    }
}

pub fn decode_server_hello(body: &[u8]) -> Result<ServerHello, TlsError> {
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
    let legacy_session_id_echo = body[pos..pos + sid_len].to_vec();
    pos += sid_len;

    if body.len() < pos + 2 {
        return Err(TlsError::UnexpectedEnd);
    }
    let cipher_suite = ((body[pos] as u16) << 8) | (body[pos + 1] as u16);
    pos += 2;

    if body.len() < pos + 1 {
        return Err(TlsError::UnexpectedEnd);
    }
    let legacy_compression_method = body[pos];
    pos += 1;

    if body.len() < pos + 2 {
        return Err(TlsError::UnexpectedEnd);
    }
    let exts_len = ((body[pos] as usize) << 8) | (body[pos + 1] as usize);
    pos += 2;
    if body.len() < pos + exts_len {
        return Err(TlsError::UnexpectedEnd);
    }
    let extensions = decode_extensions(&body[pos..pos + exts_len])?;
    Ok(ServerHello {
        legacy_version,
        random,
        legacy_session_id_echo,
        cipher_suite,
        legacy_compression_method,
        extensions,
    })
}

pub(crate) fn decode_extensions(buf: &[u8]) -> Result<Vec<(u16, Vec<u8>)>, TlsError> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < buf.len() {
        if buf.len() < pos + 4 {
            return Err(TlsError::UnexpectedEnd);
        }
        let ext_type = ((buf[pos] as u16) << 8) | (buf[pos + 1] as u16);
        let len = ((buf[pos + 2] as usize) << 8) | (buf[pos + 3] as usize);
        pos += 4;
        if buf.len() < pos + len {
            return Err(TlsError::UnexpectedEnd);
        }
        let body = buf[pos..pos + len].to_vec();
        out.push((ext_type, body));
        pos += len;
    }
    Ok(out)
}
