
use crate::handshake::{
    aead_decrypt_record, decode_handshake, finished_verify_data_equal, HandshakeType, TrafficKeys,
};
use crate::record::TlsError;
use crate::server::{server_handshake, ServerConfig};

fn frame_record(content_type: u8, fragment: &[u8]) -> Vec<u8> {
    let mut r = Vec::with_capacity(5 + fragment.len());
    r.push(content_type);
    r.extend_from_slice(&[0x03, 0x03]);
    r.push((fragment.len() >> 8) as u8);
    r.push((fragment.len() & 0xFF) as u8);
    r.extend_from_slice(fragment);
    r
}

fn take_record(buf: &[u8]) -> Option<(u8, Vec<u8>, usize)> {
    if buf.len() < 5 {
        return None;
    }
    let len = ((buf[3] as usize) << 8) | (buf[4] as usize);
    if buf.len() < 5 + len {
        return None;
    }
    Some((buf[0], buf[5..5 + len].to_vec(), 5 + len))
}

struct Pending {
    read_keys: TrafficKeys,
    expected_client_finished: Vec<u8>,
    server_app: TrafficKeys,
    client_app: TrafficKeys,
    client_seq: u64,
}

enum Phase {
    WaitClientHello,
    WaitClientFinished(Box<Pending>),
    Done,
    Failed,
}

pub struct ServerHandshakeMachine {
    config: ServerConfig,
    server_random: [u8; 32],
    rbuf: Vec<u8>,
    phase: Phase,
    pub server_app: Option<TrafficKeys>,
    pub client_app: Option<TrafficKeys>,

    pub negotiated_alpn: Option<Vec<u8>>,
}

impl ServerHandshakeMachine {
    pub fn new(config: ServerConfig) -> Result<Self, TlsError> {
        let mut server_random = [0u8; 32];
        rusty_web_crypto::get_random_values(&mut server_random)
            .map_err(|e| TlsError::SignatureFail(format!("RNG: {}", e)))?;
        Ok(Self {
            config,
            server_random,
            rbuf: Vec::new(),
            phase: Phase::WaitClientHello,
            server_app: None,
            client_app: None,
            negotiated_alpn: None,
        })
    }

    pub fn is_complete(&self) -> bool {
        matches!(self.phase, Phase::Done)
    }

    pub fn drain_buffered(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.rbuf)
    }

    pub fn feed(&mut self, inbound: &[u8]) -> Result<Vec<u8>, TlsError> {
        self.rbuf.extend_from_slice(inbound);
        let mut out = Vec::new();
        while let Some((ct, frag, consumed)) = take_record(&self.rbuf) {
            self.rbuf.drain(0..consumed);
            if ct == 20 {
                continue;
            }
            match &mut self.phase {
                Phase::WaitClientHello => {

                    let res = server_handshake(&self.config, &frag, &self.server_random)?;
                    self.negotiated_alpn = res.selected_alpn.clone();
                    out.extend_from_slice(&frame_record(22, &res.server_hello));
                    out.extend_from_slice(&frame_record(20, &[0x01]));
                    for rec in &res.encrypted_flight {
                        out.extend_from_slice(&frame_record(23, rec));
                    }
                    self.phase = Phase::WaitClientFinished(Box::new(Pending {
                        read_keys: res.hs_keys.read.clone(),
                        expected_client_finished: res.expected_client_finished,
                        server_app: res.server_app,
                        client_app: res.client_app,
                        client_seq: 0,
                    }));
                }
                Phase::WaitClientFinished(p) => {

                    let (inner_ct, plaintext) =
                        aead_decrypt_record(&p.read_keys, p.client_seq, &frag)?;
                    p.client_seq += 1;
                    if inner_ct != 22 {
                        continue;
                    }
                    let (hs, _) = decode_handshake(&plaintext)?;
                    if hs.msg_type != HandshakeType::Finished {
                        self.phase = Phase::Failed;
                        return Err(TlsError::SignatureFail("expected client Finished".into()));
                    }
                    if !finished_verify_data_equal(&hs.body, &p.expected_client_finished) {
                        self.phase = Phase::Failed;
                        return Err(TlsError::SignatureFail(
                            "client Finished MAC mismatch".into(),
                        ));
                    }
                    self.server_app = Some(p.server_app.clone());
                    self.client_app = Some(p.client_app.clone());
                    self.phase = Phase::Done;
                    break;
                }
                Phase::Done | Phase::Failed => break,
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{
        decode_server_hello, encode_client_hello, ClientHelloParams, CIPHER_AES_128_GCM_SHA256,
        GROUP_SECP256R1, SIG_ECDSA_SECP256R1_SHA256,
    };
    use crate::handshake::{
        aead_encrypt_record, derive_traffic_keys, encode_handshake, finished_mac, HandshakeMessage,
        HashAlgorithm,
    };
    use crate::server::{certificate_verify_tbs, derive_server_handshake_keys};
    use crate::EphemeralEcdh;

    #[test]
    fn record_framed_handshake_completes() {
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
        let mut machine = ServerHandshakeMachine::new(ServerConfig {
            rsa_key: None,
            cert_chain: vec![vec![0xCAu8, 0xFE]],
            signing_key: server_key.private_scalar.clone(),
            suites: vec![suite],
            groups: vec![GROUP_SECP256R1],
            alpn_protocols: vec![],
        })
        .unwrap();

        let server_out = machine.feed(&frame_record(22, &ch)).unwrap();
        assert!(!machine.is_complete());

        let mut buf = &server_out[..];
        let mut records = Vec::new();
        while let Some((ct, frag, used)) = take_record(buf) {
            records.push((ct, frag));
            buf = &buf[used..];
        }
        assert_eq!(records[0].0, 22);
        let sh = decode_server_hello(&decode_handshake(&records[0].1).unwrap().0.body).unwrap();
        let (_g, server_pub) = sh.server_key_share().unwrap();
        let dhe = client_ecdh.shared_secret(server_pub).unwrap();
        let mut transcript = ch.clone();
        transcript.extend_from_slice(&records[0].1);
        let chk = derive_server_handshake_keys(&dhe, &hash.digest(&transcript), suite).unwrap();

        let enc: Vec<&(u8, Vec<u8>)> = records.iter().filter(|(ct, _)| *ct == 23).collect();
        let mut msgs = Vec::new();
        for (seq, (_ct, frag)) in enc.iter().enumerate() {
            let (ict, pt) = aead_decrypt_record(&chk.write, seq as u64, frag).unwrap();
            assert_eq!(ict, 22);
            msgs.push(pt);
        }

        for m in &msgs {
            transcript.extend_from_slice(m);
        }
        let transcript_sf = hash.digest(&transcript);

        let cfin_data = finished_mac(hash, &chk.client_hs_secret, &transcript_sf).unwrap();
        let cfin_msg = encode_handshake(&HandshakeMessage {
            msg_type: HandshakeType::Finished,
            body: cfin_data,
        });

        let cfin_ct = aead_encrypt_record(&chk.read, 0, 22, &cfin_msg).unwrap();
        let done_out = machine.feed(&frame_record(23, &cfin_ct)).unwrap();
        assert!(done_out.is_empty());
        assert!(machine.is_complete());

        let server_app = machine.server_app.clone().unwrap();
        let client_server_app = derive_traffic_keys(
            hash,
            &chk.key_schedule
                .server_application_traffic(&transcript_sf)
                .unwrap(),
            16,
            12,
        )
        .unwrap();
        assert_eq!(server_app.key, client_server_app.key);
        let _ = certificate_verify_tbs;
        let app = b"echo over the record layer";
        let rec = aead_encrypt_record(&server_app, 0, 23, app).unwrap();
        let (ct, recovered) = aead_decrypt_record(&client_server_app, 0, &rec).unwrap();
        assert_eq!(ct, 23);
        assert_eq!(recovered, app);
    }
}
