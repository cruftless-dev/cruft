
pub use rusty_http_codec::ParsedResponse;

use rusty_tls::driver::{TcpTlsTransport, TlsClientConfig, TlsSession};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

pub fn parse_url(url: &str) -> Option<(String, String, u16, String)> {
    let (scheme, rest) = url.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse::<u16>().ok()?),
        None => (
            authority.to_string(),
            if scheme == "https" { 443 } else { 80 },
        ),
    };
    if host.is_empty() {
        return None;
    }
    Some((scheme, host, port, path.to_string()))
}

pub fn build_request(
    method: &str,
    target: &str,
    host: &str,
    port: u16,
    headers: &[(String, String)],
    body: &[u8],
) -> Vec<u8> {
    try_build_request(method, target, host, port, headers, body).expect("invalid HTTP request")
}

pub fn try_build_request(
    method: &str,
    target: &str,
    host: &str,
    port: u16,
    headers: &[(String, String)],
    body: &[u8],
) -> Result<Vec<u8>, String> {
    let mut h = headers.to_vec();
    if !h.iter().any(|(n, _)| n.eq_ignore_ascii_case("host")) {
        let hv = if port == 80 || port == 443 {
            host.to_string()
        } else {
            format!("{host}:{port}")
        };
        h.push(("Host".into(), hv));
    }
    if !h.iter().any(|(n, _)| n.eq_ignore_ascii_case("connection")) {
        h.push(("Connection".into(), "keep-alive".into()));
    }
    rusty_http_codec::try_serialize_request(method, target, &h, body).map_err(|e| e.to_string())
}

pub fn round_trip(
    scheme: &str,
    host: &str,
    port: u16,
    req: &[u8],
    insecure: bool,
    ca: Option<&str>,
) -> Result<ParsedResponse, String> {
    if scheme == "https" {
        tls_round_trip(host, port, req, insecure, ca)
    } else {
        plain_round_trip(&format!("{host}:{port}"), req)
    }
}

fn plain_round_trip(addr: &str, req: &[u8]) -> Result<ParsedResponse, String> {
    let id = rusty_sockets::stream_connect(addr).map_err(|e| format!("connect: {e:?}"))?;
    rusty_sockets::stream_write_all(id, req).map_err(|e| format!("write: {e:?}"))?;
    let mut buf = Vec::new();
    loop {
        let chunk = rusty_sockets::stream_read(id, 65536).map_err(|e| format!("read: {e:?}"))?;
        if chunk.is_empty() {
            break;
        }
        buf.extend_from_slice(&chunk);
        if let Ok(resp) = rusty_http_codec::parse_response(&buf) {
            if resp.status >= 100 {
                return Ok(resp);
            }
        }
    }
    rusty_http_codec::parse_response(&buf).map_err(|e| format!("parse: {e:?}"))
}

const MAX_IDLE_PER_HOST: usize = 16;

type PoolKey = (String, u16, u8, u64);

type IdlePool = HashMap<PoolKey, Vec<TlsSession<TcpTlsTransport>>>;

fn pool_key(host: &str, port: u16, insecure: bool, ca: Option<&str>) -> PoolKey {
    use std::hash::{Hash, Hasher};
    let (mode, ca_hash) = match (insecure, ca) {
        (true, _) => (1u8, 0u64),
        (false, Some(pem)) => {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            pem.hash(&mut h);
            (2u8, h.finish())
        }
        (false, None) => (0u8, 0u64),
    };
    (host.to_string(), port, mode, ca_hash)
}

fn session_pool() -> &'static Mutex<IdlePool> {
    static POOL: OnceLock<Mutex<IdlePool>> = OnceLock::new();
    POOL.get_or_init(|| Mutex::new(HashMap::new()))
}

fn take_pooled(key: &PoolKey) -> Option<TlsSession<TcpTlsTransport>> {
    let mut pool = session_pool().lock().ok()?;
    pool.get_mut(key).and_then(Vec::pop)
}

fn return_pooled(key: &PoolKey, session: TlsSession<TcpTlsTransport>) {
    if let Ok(mut pool) = session_pool().lock() {
        let idle = pool.entry(key.clone()).or_default();
        if idle.len() < MAX_IDLE_PER_HOST {
            idle.push(session);
        }

    }
}

fn default_trust() -> Result<&'static rusty_tls::store::TrustStore, String> {
    static TRUST: OnceLock<Option<rusty_tls::store::TrustStore>> = OnceLock::new();
    TRUST
        .get_or_init(|| rusty_tls::store::TrustStore::load_system_default().ok())
        .as_ref()
        .ok_or_else(
            || match rusty_tls::store::TrustStore::load_system_default() {
                Ok(_) => "trust store: load raced".to_string(),
                Err(e) => format!("trust store: {e:?}"),
            },
        )
}

fn tls_exchange(
    session: &mut TlsSession<TcpTlsTransport>,
    req: &[u8],
) -> Result<(ParsedResponse, bool), String> {
    session
        .send_application_data(req)
        .map_err(|e| format!("tls write: {e:?}"))?;
    let mut wire = Vec::new();
    let mut body = Vec::new();
    let mut guard = 0;
    loop {
        guard += 1;
        if guard > 100_000 {
            break;
        }
        match session.receive_application_data(&mut wire) {
            Ok(chunk) => {
                body.extend_from_slice(&chunk);
                if let Ok(resp) = rusty_http_codec::parse_response(&body) {
                    if resp.status >= 100 {
                        return Ok((resp, true));
                    }
                }
            }
            Err(_) => break,
        }
    }
    rusty_http_codec::parse_response(&body)
        .map(|r| (r, false))
        .map_err(|e| format!("parse: {e:?}"))
}

fn server_says_close(resp: &ParsedResponse) -> bool {
    resp.headers.iter().any(|(n, v)| {
        n.eq_ignore_ascii_case("connection") && v.to_ascii_lowercase().contains("close")
    })
}

fn tls_config(insecure: bool) -> TlsClientConfig {
    TlsClientConfig {
        insecure_skip_certificate_validation: insecure,
    }
}

fn tls_round_trip(
    host: &str,
    port: u16,
    req: &[u8],
    insecure: bool,
    ca: Option<&str>,
) -> Result<ParsedResponse, String> {

    let key = pool_key(host, port, insecure, ca);
    if let Some(mut session) = take_pooled(&key) {
        if let Ok((resp, framed)) = tls_exchange(&mut session, req) {
            if framed && !server_says_close(&resp) {
                return_pooled(&key, session);
            }
            return Ok(resp);
        }
    }

    let (resp, framed, session) = match ca {
        Some(ca_pem) => {

            let mut t = rusty_tls::store::TrustStore::new();
            t.add_pem_bundle(ca_pem)
                .map_err(|e| format!("ca parse: {e:?}"))?;
            let mut session =
                rusty_tls::driver::tls_connect_with_config(host, port, &t, tls_config(insecure))
                    .map_err(|e| format!("{e:?}"))?;
            let (resp, framed) = tls_exchange(&mut session, req)?;
            (resp, framed, session)
        }
        None => {
            let trust = if insecure {

                default_trust().unwrap_or_else(|_| {
                    static EMPTY: OnceLock<rusty_tls::store::TrustStore> = OnceLock::new();
                    EMPTY.get_or_init(rusty_tls::store::TrustStore::new)
                })
            } else {
                default_trust()?
            };
            let mut session =
                rusty_tls::driver::tls_connect_with_config(host, port, trust, tls_config(insecure))
                    .map_err(|e| format!("{e:?}"))?;
            let (resp, framed) = tls_exchange(&mut session, req)?;
            (resp, framed, session)
        }
    };
    if framed && !server_says_close(&resp) {
        return_pooled(&key, session);
    }
    Ok(resp)
}

pub use rusty_http_codec::{BodyFraming, ResponseHead};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct StreamingResponse {
    pub head: ResponseHead,
    dec: rusty_http_codec::ResponseDecoder,
    src: BodySource,
}

enum BodySource {
    Plain {

        id: u64,
    },
    Tls {

        session: Option<TlsSession<TcpTlsTransport>>,
        wire: Vec<u8>,
        key: PoolKey,

        framed: bool,
    },

    Done,
}

impl StreamingResponse {

    pub fn take_plain_upgrade_stream(&mut self) -> Option<u64> {
        if self.head.status != 101 {
            return None;
        }
        let mut src = BodySource::Done;
        std::mem::swap(&mut self.src, &mut src);
        match src {
            BodySource::Plain { id } => Some(id),
            other => {
                self.src = other;
                None
            }
        }
    }

    pub fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, String> {
        self.next_chunk_cancelled(None)
    }

    pub fn trailers(&self) -> Vec<(String, String)> {
        self.dec.trailers().to_vec()
    }

    pub fn next_chunk_cancelled(
        &mut self,
        cancel: Option<&AtomicBool>,
    ) -> Result<Option<Vec<u8>>, String> {
        loop {
            if cancel.map(|c| c.load(Ordering::SeqCst)).unwrap_or(false) {
                self.release();
                return Err("aborted".into());
            }
            let chunk = self.dec.read_body().map_err(|e| format!("parse: {e:?}"))?;
            if !chunk.is_empty() {
                return Ok(Some(chunk));
            }
            if self.dec.is_complete() {
                self.release();
                return Ok(None);
            }
            match &mut self.src {
                BodySource::Done => return Ok(None),
                BodySource::Plain { id } => {
                    let raw = match cancel {
                        Some(cancel) => loop {
                            if cancel.load(Ordering::SeqCst) {
                                let _ = rusty_sockets::handle_close(*id);
                                return Err("aborted".into());
                            }
                            match rusty_sockets::stream_try_read(*id, 65536)
                                .map_err(|e| format!("read: {e:?}"))?
                            {
                                Some(bytes) => break bytes,
                                None => std::thread::sleep(std::time::Duration::from_millis(1)),
                            }
                        },
                        None => rusty_sockets::stream_read(*id, 65536)
                            .map_err(|e| format!("read: {e:?}"))?,
                    };
                    if raw.is_empty() {
                        self.dec.close();
                    } else {
                        self.dec.push(&raw);
                    }
                }
                BodySource::Tls { session, wire, .. } => {
                    let s = match session.as_mut() {
                        Some(s) => s,
                        None => {
                            self.dec.close();
                            continue;
                        }
                    };
                    match s.receive_application_data(wire) {
                        Ok(c) => self.dec.push(&c),
                        Err(_) => self.dec.close(),
                    }
                }
            }
        }
    }

    pub fn read_to_end(&mut self) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        while let Some(c) = self.next_chunk()? {
            out.extend_from_slice(&c);
        }
        Ok(out)
    }

    fn release(&mut self) {
        match &mut self.src {
            BodySource::Plain { id } => {
                let _ = rusty_sockets::handle_close(*id);
            }
            BodySource::Tls {
                session,
                key,
                framed,
                ..
            } => {
                if let Some(s) = session.take() {
                    if *framed && !head_says_close(&self.head) {
                        return_pooled(key, s);
                    }
                }
            }
            BodySource::Done => {}
        }
        self.src = BodySource::Done;
    }
}

fn head_says_close(head: &ResponseHead) -> bool {
    head.headers.iter().any(|(n, v)| {
        n.eq_ignore_ascii_case("connection") && v.to_ascii_lowercase().contains("close")
    })
}

pub fn round_trip_streaming(
    scheme: &str,
    host: &str,
    port: u16,
    req: &[u8],
    insecure: bool,
    ca: Option<&str>,
) -> Result<StreamingResponse, String> {
    round_trip_streaming_cancelled(scheme, host, port, req, insecure, ca, None)
}

pub fn round_trip_streaming_cancelled(
    scheme: &str,
    host: &str,
    port: u16,
    req: &[u8],
    insecure: bool,
    ca: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> Result<StreamingResponse, String> {
    round_trip_streaming_cancelled_with_connect_timeout(
        scheme, host, port, req, insecure, ca, cancel, None,
    )
}

pub fn round_trip_streaming_cancelled_with_connect_timeout(
    scheme: &str,
    host: &str,
    port: u16,
    req: &[u8],
    insecure: bool,
    ca: Option<&str>,
    cancel: Option<&AtomicBool>,
    connect_timeout_ms: Option<u64>,
) -> Result<StreamingResponse, String> {
    if scheme == "https" {
        tls_round_trip_streaming(host, port, req, insecure, ca)
    } else {
        plain_round_trip_streaming_cancelled(
            &format!("{host}:{port}"),
            req,
            cancel,
            connect_timeout_ms,
        )
    }
}

fn plain_round_trip_streaming_cancelled(
    addr: &str,
    req: &[u8],
    cancel: Option<&AtomicBool>,
    connect_timeout_ms: Option<u64>,
) -> Result<StreamingResponse, String> {
    let id = match connect_timeout_ms {
        Some(timeout_ms) => rusty_sockets::stream_connect_timeout(addr, timeout_ms),
        None => rusty_sockets::stream_connect(addr),
    }
    .map_err(|e| format!("connect: {e:?}"))?;
    rusty_sockets::stream_write_all(id, req).map_err(|e| format!("write: {e:?}"))?;
    if cancel.is_some() {
        let _ = rusty_sockets::stream_set_nonblocking(id, true);
    }
    let mut dec = rusty_http_codec::ResponseDecoder::new();
    loop {
        if cancel.map(|c| c.load(Ordering::SeqCst)).unwrap_or(false) {
            let _ = rusty_sockets::handle_close(id);
            return Err("aborted".into());
        }
        if let Some(head) = dec.head().map_err(|e| format!("parse: {e:?}"))? {
            let head = head.clone();
            return Ok(StreamingResponse {
                head,
                dec,
                src: BodySource::Plain { id },
            });
        }
        let raw = match cancel {
            Some(_) => match rusty_sockets::stream_try_read(id, 65536)
                .map_err(|e| format!("read: {e:?}"))?
            {
                Some(bytes) => bytes,
                None => {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                    continue;
                }
            },
            None => rusty_sockets::stream_read(id, 65536).map_err(|e| format!("read: {e:?}"))?,
        };
        if raw.is_empty() {
            dec.close();

            match dec.head().map_err(|e| format!("parse: {e:?}"))? {
                Some(head) => {
                    let head = head.clone();
                    return Ok(StreamingResponse {
                        head,
                        dec,
                        src: BodySource::Plain { id },
                    });
                }
                None => return Err("read: connection closed before response head".into()),
            }
        }
        dec.push(&raw);
    }
}

fn tls_head_exchange(
    session: &mut TlsSession<TcpTlsTransport>,
    req: &[u8],
    wire: &mut Vec<u8>,
) -> Result<(rusty_http_codec::ResponseDecoder, ResponseHead, bool), String> {
    session
        .send_application_data(req)
        .map_err(|e| format!("tls write: {e:?}"))?;
    let mut dec = rusty_http_codec::ResponseDecoder::new();
    let mut guard = 0;
    loop {
        if let Some(head) = dec.head().map_err(|e| format!("parse: {e:?}"))? {
            let head = head.clone();
            let framed = !matches!(
                dec.framing().map_err(|e| format!("parse: {e:?}"))?,
                Some(BodyFraming::Eof)
            );
            return Ok((dec, head, framed));
        }
        guard += 1;
        if guard > 100_000 {
            return Err("tls read: head never completed".into());
        }
        match session.receive_application_data(wire) {
            Ok(chunk) => dec.push(&chunk),
            Err(e) => return Err(format!("tls read: {e:?}")),
        }
    }
}

fn tls_round_trip_streaming(
    host: &str,
    port: u16,
    req: &[u8],
    insecure: bool,
    ca: Option<&str>,
) -> Result<StreamingResponse, String> {
    let key = pool_key(host, port, insecure, ca);
    if let Some(mut session) = take_pooled(&key) {
        let mut wire = Vec::new();
        if let Ok((dec, head, framed)) = tls_head_exchange(&mut session, req, &mut wire) {
            return Ok(StreamingResponse {
                head,
                dec,
                src: BodySource::Tls {
                    session: Some(session),
                    wire,
                    key,
                    framed,
                },
            });
        }

    }

    let mut session = match ca {
        Some(ca_pem) => {
            let mut t = rusty_tls::store::TrustStore::new();
            t.add_pem_bundle(ca_pem)
                .map_err(|e| format!("ca parse: {e:?}"))?;
            rusty_tls::driver::tls_connect_with_config(host, port, &t, tls_config(insecure))
                .map_err(|e| format!("{e:?}"))?
        }
        None => {
            let trust = if insecure {
                default_trust().unwrap_or_else(|_| {
                    static EMPTY: OnceLock<rusty_tls::store::TrustStore> = OnceLock::new();
                    EMPTY.get_or_init(rusty_tls::store::TrustStore::new)
                })
            } else {
                default_trust()?
            };
            rusty_tls::driver::tls_connect_with_config(host, port, trust, tls_config(insecure))
                .map_err(|e| format!("{e:?}"))?
        }
    };
    let mut wire = Vec::new();
    let (dec, head, framed) = tls_head_exchange(&mut session, req, &mut wire)?;
    Ok(StreamingResponse {
        head,
        dec,
        src: BodySource::Tls {
            session: Some(session),
            wire,
            key,
            framed,
        },
    })
}
