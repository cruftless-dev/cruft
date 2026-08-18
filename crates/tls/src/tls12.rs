
use crate::client::ServerHello;
use crate::driver::{EphemeralEcdh, TlsSession, TlsTransport};
use crate::handshake::{HashAlgorithm, TrafficKeys};
use crate::record::{
    decode_record, encode_record, ContentType, ProtocolVersion, TlsError, TlsRecord,
};
use crate::store::{validate_server_certificate, TrustStore};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

#[derive(Clone)]
pub struct StoredTicket {
    pub ticket: Vec<u8>,
    pub master_secret: Vec<u8>,
    pub cipher_suite: u16,
}

fn ticket_store() -> &'static Mutex<HashMap<String, StoredTicket>> {
    static S: OnceLock<Mutex<HashMap<String, StoredTicket>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn store_ticket(host: &str, t: StoredTicket) {
    ticket_store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(host.to_string(), t);
}

pub fn lookup_ticket(host: &str) -> Option<StoredTicket> {
    ticket_store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(host)
        .cloned()
}

fn parse_new_session_ticket(msg: &[u8]) -> Option<Vec<u8>> {
    if msg.len() < 10 || msg[0] != 4 {
        return None;
    }
    let ticket_len = ((msg[8] as usize) << 8) | (msg[9] as usize);
    let start = 10;
    if msg.len() < start + ticket_len {
        return None;
    }
    Some(msg[start..start + ticket_len].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticket_store_recovers_from_poisoning() {
        let _ = std::thread::spawn(|| {
            let _guard = ticket_store().lock().unwrap();
            panic!("poison ticket store");
        })
        .join();

        store_ticket(
            "fixture.cruft.test",
            StoredTicket {
                ticket: b"ticket".to_vec(),
                master_secret: b"secret".to_vec(),
                cipher_suite: crate::client::CIPHER_ECDHE_ECDSA_AES128_GCM_SHA256,
            },
        );
        assert!(lookup_ticket("fixture.cruft.test").is_some());
    }
}

fn resume_abbreviated_tls12<T: TlsTransport>(
    mut transport: T,
    sh: &ServerHello,
    client_random: &[u8; 32],
    mut transcript: Vec<u8>,
    mut acc: Vec<u8>,
    st: StoredTicket,
    host: &str,
) -> Result<TlsSession<T>, TlsError> {
    let server_random = sh.random;
    let master = st.master_secret;

    let mut sr_cr = Vec::with_capacity(64);
    sr_cr.extend_from_slice(&server_random);
    sr_cr.extend_from_slice(client_random);
    let kb = prf(&master, b"key expansion", &sr_cr, 40);
    let client_write_key = kb[0..16].to_vec();
    let server_write_key = kb[16..32].to_vec();
    let client_iv = kb[32..36].to_vec();
    let server_iv = kb[36..40].to_vec();

    let mut server_ccs_seen = false;
    loop {
        match decode_record(&acc) {
            Ok((rec, n)) => {
                acc.drain(..n);
                match rec.content_type {
                    ContentType::ChangeCipherSpec => server_ccs_seen = true,
                    ContentType::Handshake if !server_ccs_seen => {

                        if rec.fragment.first() == Some(&4) {
                            if let Some(ticket) = parse_new_session_ticket(&rec.fragment) {
                                store_ticket(
                                    host,
                                    StoredTicket {
                                        ticket,
                                        master_secret: master.clone(),
                                        cipher_suite: sh.cipher_suite,
                                    },
                                );
                            }
                            transcript.extend_from_slice(&rec.fragment);
                        } else {
                            return Err(TlsError::SignatureFail(
                                "resume: unexpected cleartext handshake before CCS".into(),
                            ));
                        }
                    }
                    ContentType::Handshake => {
                        let pt = decrypt_record_tls12(
                            &server_write_key,
                            &server_iv,
                            0,
                            ContentType::Handshake as u8,
                            &rec.fragment,
                        )?;
                        if pt.len() < 4 || pt[0] != 20 {
                            return Err(TlsError::SignatureFail(
                                "resume: decrypted server msg is not Finished".into(),
                            ));
                        }
                        let expected_sf = prf(
                            &master,
                            b"server finished",
                            &rusty_web_crypto::digest_sha256(&transcript),
                            12,
                        );
                        if !crate::handshake::finished_verify_data_equal(&pt[4..], &expected_sf) {
                            return Err(TlsError::SignatureFail(
                                "resume: server Finished verify_data mismatch".into(),
                            ));
                        }

                        transcript.extend_from_slice(&pt);
                        break;
                    }
                    ContentType::Alert => return Err(crate::record::classify_alert(&rec.fragment)),
                    _ => {}
                }
            }
            Err(_) => {
                transport.read_some(&mut acc)?;
            }
        }
    }

    transport.write_all(&encode_record(&TlsRecord {
        content_type: ContentType::ChangeCipherSpec,
        version: ProtocolVersion::LEGACY,
        fragment: vec![0x01],
    })?)?;

    let vd = prf(
        &master,
        b"client finished",
        &rusty_web_crypto::digest_sha256(&transcript),
        12,
    );
    let mut fin_msg = Vec::with_capacity(16);
    fin_msg.push(20);
    fin_msg.extend_from_slice(&[0x00, 0x00, 0x0c]);
    fin_msg.extend_from_slice(&vd);
    let fin_frag = encrypt_record_tls12(
        &client_write_key,
        &client_iv,
        0,
        ContentType::Handshake as u8,
        &fin_msg,
    )?;
    transport.write_all(&encode_record(&TlsRecord {
        content_type: ContentType::Handshake,
        version: ProtocolVersion::LEGACY,
        fragment: fin_frag,
    })?)?;

    if std::env::var("CRUFT_TLS_DEBUG").is_ok() {
        eprintln!("[resumption] abbreviated handshake OK: host={host} (no ECDHE/cert/verify)");
    }

    Ok(TlsSession {
        transport,
        client_app_keys: TrafficKeys {
            key: client_write_key,
            iv: client_iv,
        },
        server_app_keys: TrafficKeys {
            key: server_write_key,
            iv: server_iv,
        },
        client_app_seq: 1,
        server_app_seq: 1,
        hash: HashAlgorithm::Sha256,
        tls12: true,
    })
}

fn prf(secret: &[u8], label: &[u8], seed: &[u8], out_len: usize) -> Vec<u8> {
    let mut label_seed = Vec::with_capacity(label.len() + seed.len());
    label_seed.extend_from_slice(label);
    label_seed.extend_from_slice(seed);
    let mut out = Vec::with_capacity(out_len + 32);
    let mut a = rusty_web_crypto::hmac_sha256(secret, &label_seed).to_vec();
    while out.len() < out_len {
        let mut input = a.clone();
        input.extend_from_slice(&label_seed);
        out.extend_from_slice(&rusty_web_crypto::hmac_sha256(secret, &input));
        a = rusty_web_crypto::hmac_sha256(secret, &a).to_vec();
    }
    out.truncate(out_len);
    out
}

fn der_ecdsa_to_raw(der: &[u8]) -> Result<Vec<u8>, TlsError> {
    let err = || TlsError::SignatureFail("malformed DER ECDSA signature".into());
    if der.len() < 2 || der[0] != 0x30 {
        return Err(err());
    }

    let (mut p, _seq_len) = read_der_len(der, 1).ok_or_else(err)?;
    let read_int = |buf: &[u8], at: usize| -> Option<(Vec<u8>, usize)> {
        if buf.get(at)? != &0x02 {
            return None;
        }
        let (vp, len) = read_der_len(buf, at + 1)?;
        let raw = buf.get(vp..vp + len)?;

        let mut v = raw;
        while v.len() > 1 && v[0] == 0 {
            v = &v[1..];
        }
        if v.len() > 32 {
            return None;
        }
        let mut padded = vec![0u8; 32 - v.len()];
        padded.extend_from_slice(v);
        Some((padded, vp + len))
    };
    let (r, np) = read_int(der, p).ok_or_else(err)?;
    p = np;
    let (s, _) = read_int(der, p).ok_or_else(err)?;
    let mut out = r;
    out.extend_from_slice(&s);
    Ok(out)
}

fn read_der_len(buf: &[u8], at: usize) -> Option<(usize, usize)> {
    let b = *buf.get(at)?;
    if b & 0x80 == 0 {
        Some((at + 1, b as usize))
    } else {
        let n = (b & 0x7f) as usize;
        if n == 0 || n > 4 {
            return None;
        }
        let mut len = 0usize;
        for i in 0..n {
            len = (len << 8) | (*buf.get(at + 1 + i)? as usize);
        }
        Some((at + 1 + n, len))
    }
}

fn parse_cert_list_tls12(body: &[u8]) -> Result<Vec<rusty_x509::Certificate>, TlsError> {
    if body.len() < 3 {
        return Err(TlsError::UnexpectedEnd);
    }
    let list_len = ((body[0] as usize) << 16) | ((body[1] as usize) << 8) | (body[2] as usize);
    let mut pos = 3;
    let end = pos + list_len;
    if body.len() < end {
        return Err(TlsError::UnexpectedEnd);
    }
    let mut certs = Vec::new();
    while pos < end {
        if body.len() < pos + 3 {
            return Err(TlsError::UnexpectedEnd);
        }
        let clen = ((body[pos] as usize) << 16)
            | ((body[pos + 1] as usize) << 8)
            | (body[pos + 2] as usize);
        pos += 3;
        if body.len() < pos + clen {
            return Err(TlsError::UnexpectedEnd);
        }
        certs.push(rusty_x509::parse_certificate(&body[pos..pos + clen]).map_err(TlsError::X509)?);
        pos += clen;
    }
    Ok(certs)
}

pub fn encrypt_record_tls12(
    key: &[u8],
    fixed_iv: &[u8],
    seq: u64,
    content_type: u8,
    plaintext: &[u8],
) -> Result<Vec<u8>, TlsError> {
    let explicit = seq.to_be_bytes();
    let mut nonce = Vec::with_capacity(12);
    nonce.extend_from_slice(fixed_iv);
    nonce.extend_from_slice(&explicit);
    let mut aad = Vec::with_capacity(13);
    aad.extend_from_slice(&explicit);
    aad.push(content_type);
    aad.extend_from_slice(&[0x03, 0x03]);
    aad.push((plaintext.len() >> 8) as u8);
    aad.push((plaintext.len() & 0xFF) as u8);
    let ct = rusty_web_crypto::aes_gcm_encrypt(key, &nonce, &aad, plaintext)
        .map_err(TlsError::SignatureFail)?;
    let mut frag = Vec::with_capacity(8 + ct.len());
    frag.extend_from_slice(&explicit);
    frag.extend_from_slice(&ct);
    Ok(frag)
}

pub fn decrypt_record_tls12(
    key: &[u8],
    fixed_iv: &[u8],
    seq: u64,
    content_type: u8,
    fragment: &[u8],
) -> Result<Vec<u8>, TlsError> {
    if fragment.len() < 8 + 16 {
        return Err(TlsError::UnexpectedEnd);
    }
    let explicit = &fragment[..8];
    let ct = &fragment[8..];
    let plaintext_len = ct.len() - 16;
    let mut nonce = Vec::with_capacity(12);
    nonce.extend_from_slice(fixed_iv);
    nonce.extend_from_slice(explicit);
    let mut aad = Vec::with_capacity(13);
    aad.extend_from_slice(&seq.to_be_bytes());
    aad.push(content_type);
    aad.extend_from_slice(&[0x03, 0x03]);
    aad.push((plaintext_len >> 8) as u8);
    aad.push((plaintext_len & 0xFF) as u8);
    rusty_web_crypto::aes_gcm_decrypt(key, &nonce, &aad, ct).map_err(TlsError::SignatureFail)
}

fn read_cleartext_handshake<T: TlsTransport>(
    transport: &mut T,
    acc: &mut Vec<u8>,
    hs_buf: &mut Vec<u8>,
    transcript: &mut Vec<u8>,
) -> Result<(u8, Vec<u8>), TlsError> {
    loop {

        if hs_buf.len() >= 4 {
            let len =
                ((hs_buf[1] as usize) << 16) | ((hs_buf[2] as usize) << 8) | (hs_buf[3] as usize);
            if hs_buf.len() >= 4 + len {
                let msg_type = hs_buf[0];
                let raw = hs_buf[..4 + len].to_vec();
                transcript.extend_from_slice(&raw);
                let body = hs_buf[4..4 + len].to_vec();
                hs_buf.drain(..4 + len);
                return Ok((msg_type, body));
            }
        }

        match decode_record(acc) {
            Ok((rec, n)) => {
                acc.drain(..n);
                match rec.content_type {
                    ContentType::Handshake => hs_buf.extend_from_slice(&rec.fragment),
                    ContentType::ChangeCipherSpec => {}
                    ContentType::Alert => return Err(crate::record::classify_alert(&rec.fragment)),
                    _ => {
                        return Err(TlsError::SignatureFail(
                            "unexpected record in 1.2 handshake".into(),
                        ))
                    }
                }
            }
            Err(_) => {
                transport.read_some(acc)?;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn complete_handshake_tls12<T: TlsTransport>(
    mut transport: T,
    sh: &ServerHello,
    ephemeral: EphemeralEcdh,
    client_random: &[u8; 32],
    mut transcript: Vec<u8>,
    mut acc: Vec<u8>,
    trust_store: &TrustStore,
    host: &str,
    client_session_id: &[u8],
    offered_ticket: Option<StoredTicket>,
    config: crate::driver::TlsClientConfig,
) -> Result<TlsSession<T>, TlsError> {
    let server_random = sh.random;

    if let Some(st) = offered_ticket {
        if !client_session_id.is_empty()
            && sh.legacy_session_id_echo == client_session_id
            && sh.cipher_suite == st.cipher_suite
        {
            return resume_abbreviated_tls12(
                transport,
                sh,
                client_random,
                transcript,
                acc,
                st,
                host,
            );
        }
    }

    let mut hs_buf: Vec<u8> = Vec::new();

    let mut server_certs: Vec<rusty_x509::Certificate> = Vec::new();
    let mut ske_body: Vec<u8> = Vec::new();
    loop {
        let (mt, body) =
            read_cleartext_handshake(&mut transport, &mut acc, &mut hs_buf, &mut transcript)?;
        match mt {
            11 => server_certs = parse_cert_list_tls12(&body)?,
            12 => ske_body = body,
            13 => {   }
            14 => break,
            other => {
                return Err(TlsError::SignatureFail(format!(
                    "unexpected 1.2 handshake msg type {other}"
                )))
            }
        }
    }

    let leaf = server_certs
        .first()
        .ok_or_else(|| TlsError::SignatureFail("no leaf cert".into()))?
        .clone();
    let intermediates: Vec<_> = server_certs.iter().skip(1).cloned().collect();
    let _prof = std::env::var("CRUFT_TLS_PROFILE").is_ok();
    let _t = std::time::Instant::now();
    if !config.insecure_skip_certificate_validation {
        validate_server_certificate(
            &leaf,
            &intermediates,
            trust_store,
            8,
            host,
            std::time::SystemTime::now(),
        )?;
    }
    if _prof {
        eprintln!(
            "[hs-prof] chain_walk(verify) {}us",
            _t.elapsed().as_micros()
        );
    }

    if ske_body.len() < 4 {
        return Err(TlsError::SignatureFail("short ServerKeyExchange".into()));
    }
    if ske_body[0] != 3 || ske_body[1] != 0x00 || ske_body[2] != 0x17 {
        return Err(TlsError::SignatureFail(
            "SKE not named_curve secp256r1".into(),
        ));
    }
    let pub_len = ske_body[3] as usize;
    let params_end = 4 + pub_len;
    if ske_body.len() < params_end + 4 {
        return Err(TlsError::UnexpectedEnd);
    }
    let server_ecdhe_pub = ske_body[4..params_end].to_vec();
    let signed_params = &ske_body[..params_end];
    let _sig_hash = ske_body[params_end];
    let _sig_alg = ske_body[params_end + 1];
    let sig_len = ((ske_body[params_end + 2] as usize) << 8) | (ske_body[params_end + 3] as usize);
    let sig_start = params_end + 4;
    if ske_body.len() < sig_start + sig_len {
        return Err(TlsError::UnexpectedEnd);
    }
    let der_sig = &ske_body[sig_start..sig_start + sig_len];

    let mut to_verify = Vec::with_capacity(64 + signed_params.len());
    to_verify.extend_from_slice(client_random);
    to_verify.extend_from_slice(&server_random);
    to_verify.extend_from_slice(signed_params);
    let digest = rusty_web_crypto::digest_sha256(&to_verify);
    let raw_sig = der_ecdsa_to_raw(der_sig)?;
    let (curve_oid, point) = match &leaf.subject_public_key_info.key {
        rusty_x509::PublicKey::Ec { curve_oid, point } => (curve_oid, point),
        _ => {
            return Err(TlsError::SignatureFail(
                "ECDHE-ECDSA but leaf cert is not EC".into(),
            ))
        }
    };
    if curve_oid.as_str() != rusty_x509::OID_P256_CURVE || point.len() != 65 || point[0] != 0x04 {
        return Err(TlsError::SignatureFail("leaf EC pubkey not P-256".into()));
    }
    let _t = std::time::Instant::now();
    rusty_web_crypto::ecdsa_verify(
        &rusty_web_crypto::curve_p256(),
        &point[1..33],
        &point[33..65],
        &digest,
        &raw_sig,
    )
    .map_err(|e| TlsError::SignatureFail(format!("SKE signature: {e}")))?;
    if _prof {
        eprintln!("[hs-prof] ske_ecdsa_verify {}us", _t.elapsed().as_micros());
    }

    let _t = std::time::Instant::now();
    let premaster = ephemeral.shared_secret(&server_ecdhe_pub)?;
    if _prof {
        eprintln!(
            "[hs-prof] ecdhe_shared_secret {}us",
            _t.elapsed().as_micros()
        );
    }
    let mut cr_sr = Vec::with_capacity(64);
    cr_sr.extend_from_slice(client_random);
    cr_sr.extend_from_slice(&server_random);
    let master = prf(&premaster, b"master secret", &cr_sr, 48);
    let mut sr_cr = Vec::with_capacity(64);
    sr_cr.extend_from_slice(&server_random);
    sr_cr.extend_from_slice(client_random);

    let kb = prf(&master, b"key expansion", &sr_cr, 40);
    let client_write_key = kb[0..16].to_vec();
    let server_write_key = kb[16..32].to_vec();
    let client_iv = kb[32..36].to_vec();
    let server_iv = kb[36..40].to_vec();

    let mut cke = Vec::new();
    cke.push(16);
    let cke_body_len = 1 + ephemeral.public_point.len();
    cke.push((cke_body_len >> 16) as u8);
    cke.push((cke_body_len >> 8) as u8);
    cke.push((cke_body_len & 0xFF) as u8);
    cke.push(ephemeral.public_point.len() as u8);
    cke.extend_from_slice(&ephemeral.public_point);
    transcript.extend_from_slice(&cke);
    transport.write_all(&encode_record(&TlsRecord {
        content_type: ContentType::Handshake,
        version: ProtocolVersion::LEGACY,
        fragment: cke,
    })?)?;

    transport.write_all(&encode_record(&TlsRecord {
        content_type: ContentType::ChangeCipherSpec,
        version: ProtocolVersion::LEGACY,
        fragment: vec![0x01],
    })?)?;

    let vd = prf(
        &master,
        b"client finished",
        &rusty_web_crypto::digest_sha256(&transcript),
        12,
    );
    let mut fin_msg = Vec::with_capacity(4 + 12);
    fin_msg.push(20);
    fin_msg.extend_from_slice(&[0x00, 0x00, 0x0c]);
    fin_msg.extend_from_slice(&vd);
    transcript.extend_from_slice(&fin_msg);
    let fin_frag = encrypt_record_tls12(
        &client_write_key,
        &client_iv,
        0,
        ContentType::Handshake as u8,
        &fin_msg,
    )?;
    transport.write_all(&encode_record(&TlsRecord {
        content_type: ContentType::Handshake,
        version: ProtocolVersion::LEGACY,
        fragment: fin_frag,
    })?)?;

    let mut server_ccs_seen = false;
    loop {
        match decode_record(&acc) {
            Ok((rec, n)) => {
                acc.drain(..n);
                match rec.content_type {
                    ContentType::ChangeCipherSpec => server_ccs_seen = true,
                    ContentType::Handshake if !server_ccs_seen => {

                        if rec.fragment.first() == Some(&4) {
                            if let Some(ticket) = parse_new_session_ticket(&rec.fragment) {
                                if std::env::var("CRUFT_TLS_DEBUG").is_ok() {
                                    eprintln!(
                                        "[resumption] captured ticket: host={host} {} bytes, cipher=0x{:04x}",
                                        ticket.len(),
                                        sh.cipher_suite
                                    );
                                }
                                store_ticket(
                                    host,
                                    StoredTicket {
                                        ticket,
                                        master_secret: master.clone(),
                                        cipher_suite: sh.cipher_suite,
                                    },
                                );
                            }

                            transcript.extend_from_slice(&rec.fragment);
                        } else {
                            return Err(TlsError::SignatureFail(
                                "unexpected cleartext handshake before ChangeCipherSpec".into(),
                            ));
                        }
                    }
                    ContentType::Handshake => {
                        let pt = decrypt_record_tls12(
                            &server_write_key,
                            &server_iv,
                            0,
                            ContentType::Handshake as u8,
                            &rec.fragment,
                        )?;
                        if pt.len() < 4 || pt[0] != 20 {
                            return Err(TlsError::SignatureFail(
                                "decrypted server msg is not Finished".into(),
                            ));
                        }
                        let expected_sf = prf(
                            &master,
                            b"server finished",
                            &rusty_web_crypto::digest_sha256(&transcript),
                            12,
                        );
                        if !crate::handshake::finished_verify_data_equal(&pt[4..], &expected_sf) {
                            return Err(TlsError::SignatureFail(
                                "server Finished verify_data mismatch".into(),
                            ));
                        }
                        break;
                    }
                    ContentType::Alert => return Err(crate::record::classify_alert(&rec.fragment)),
                    _ => {}
                }
            }
            Err(_) => {
                transport.read_some(&mut acc)?;
            }
        }
    }

    Ok(TlsSession {
        transport,
        client_app_keys: TrafficKeys {
            key: client_write_key,
            iv: client_iv,
        },
        server_app_keys: TrafficKeys {
            key: server_write_key,
            iv: server_iv,
        },
        client_app_seq: 1,
        server_app_seq: 1,
        hash: HashAlgorithm::Sha256,
        tls12: true,
    })
}
