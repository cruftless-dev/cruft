
use rusty_http_codec::{parse_response, serialize_request};
use rusty_tls::driver::{tls_connect, TcpTlsTransport, TlsSession};
use rusty_tls::store::TrustStore;

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

type IdlePool = HashMap<(String, u16), Vec<TlsSession<TcpTlsTransport>>>;
const MAX_IDLE_PER_HOST: usize = 16;

fn session_pool() -> &'static Mutex<IdlePool> {
    static POOL: OnceLock<Mutex<IdlePool>> = OnceLock::new();
    POOL.get_or_init(|| Mutex::new(HashMap::new()))
}

fn take_pooled(host: &str, port: u16) -> Option<TlsSession<TcpTlsTransport>> {
    let mut pool = session_pool().lock().unwrap();
    pool.get_mut(&(host.to_string(), port))
        .and_then(|v| v.pop())
}

fn return_pooled(host: &str, port: u16, session: TlsSession<TcpTlsTransport>) {
    let mut pool = session_pool().lock().unwrap();
    let idle = pool.entry((host.to_string(), port)).or_default();
    if idle.len() < MAX_IDLE_PER_HOST {
        idle.push(session);
    }

}

fn response_keeps_alive(resp: &rusty_http_codec::ParsedResponse) -> bool {
    let hdr = |name: &str| {
        resp.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.to_ascii_lowercase())
    };
    let framed = hdr("content-length").is_some()
        || hdr("transfer-encoding").is_some_and(|v| v.contains("chunked"));
    let server_closes = hdr("connection").is_some_and(|v| v.contains("close"));
    framed && !server_closes
}

#[derive(Debug)]
pub enum HttpError {
    UnsupportedScheme(String),
    MalformedUrl(String),
    TrustStore(String),
    Tls(String),
    Codec(String),
    Status { code: u16, body_prefix: String },
}

struct ParsedUrl {
    host: String,
    port: u16,
    path: String,
}

fn parse_url(url: &str) -> Result<ParsedUrl, HttpError> {

    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| HttpError::UnsupportedScheme(url.to_string()))?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    if authority.is_empty() {
        return Err(HttpError::MalformedUrl(url.to_string()));
    }
    let (host, port) = match authority.find(':') {
        Some(i) => {
            let p: u16 = authority[i + 1..]
                .parse()
                .map_err(|_| HttpError::MalformedUrl(url.to_string()))?;
            (&authority[..i], p)
        }
        None => (authority, 443),
    };
    Ok(ParsedUrl {
        host: host.to_string(),
        port,
        path: if path.is_empty() {
            "/".to_string()
        } else {
            path.to_string()
        },
    })
}

pub fn pm_http_get(url: &str) -> Result<Vec<u8>, HttpError> {
    let resp = pm_http_get_raw(url, "application/json,*/*")?;
    finalize_raw(resp)
}

pub fn pm_http_get_follow(url: &str, max_hops: u8) -> Result<Vec<u8>, HttpError> {
    let mut current = url.to_string();
    for _ in 0..=max_hops {
        let resp = pm_http_get_raw(&current, "application/json,*/*")?;
        if (300..400).contains(&resp.status) {
            let loc = header_value(&resp.headers, "location").ok_or_else(|| HttpError::Status {
                code: resp.status,
                body_prefix: "3xx without Location header".into(),
            })?;
            current = resolve_location(&current, &loc)?;
            continue;
        }
        return finalize_raw(resp);
    }
    Err(HttpError::Status {
        code: 310,
        body_prefix: format!("too many redirects from {url}"),
    })
}

fn header_value(headers: &[(String, String)], name: &str) -> Option<String> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.clone())
}

fn resolve_location(base: &str, loc: &str) -> Result<String, HttpError> {
    if loc.starts_with("https://") {
        Ok(loc.to_string())
    } else if loc.starts_with("http://") {
        Err(HttpError::UnsupportedScheme(loc.to_string()))
    } else if loc.starts_with('/') {

        let b = parse_url(base)?;
        let port_suffix = if b.port == 443 {
            String::new()
        } else {
            format!(":{}", b.port)
        };
        Ok(format!("https://{}{}{}", b.host, port_suffix, loc))
    } else {
        Err(HttpError::MalformedUrl(format!(
            "relative redirect not supported: {loc}"
        )))
    }
}

fn finalize_raw(resp: rusty_http_codec::ParsedResponse) -> Result<Vec<u8>, HttpError> {
    if !(200..300).contains(&resp.status) {
        let prefix: String = String::from_utf8_lossy(&resp.body)
            .chars()
            .take(200)
            .collect();
        return Err(HttpError::Status {
            code: resp.status,
            body_prefix: prefix,
        });
    }

    if let Some(enc) = header_value(&resp.headers, "content-encoding") {
        if enc.to_ascii_lowercase().contains("gzip") {
            return rusty_compression::gunzip(&resp.body)
                .map_err(|e| HttpError::Codec(format!("gunzip: {e:?}")));
        }
    }
    Ok(resp.body)
}

fn exchange(
    session: &mut TlsSession<TcpTlsTransport>,
    u: &ParsedUrl,
    accept: &str,
) -> Result<rusty_http_codec::ParsedResponse, HttpError> {
    let request = serialize_request(
        "GET",
        &u.path,
        &[
            ("Host".into(), u.host.clone()),
            ("User-Agent".into(), "cruft-pm/0.1.0".into()),
            ("Accept".into(), accept.into()),

            ("Accept-Encoding".into(), "gzip".into()),
            ("Connection".into(), "keep-alive".into()),
        ],
        &[],
    );
    session
        .send_application_data(&request)
        .map_err(|e| HttpError::Tls(format!("send: {e:?}")))?;

    let mut raw = Vec::<u8>::new();
    let mut accumulator = Vec::<u8>::new();
    loop {
        match session.receive_application_data(&mut accumulator) {
            Ok(chunk) => {
                if chunk.is_empty() && accumulator.is_empty() {
                    break;
                }
                raw.extend_from_slice(&chunk);

                if let Ok(resp) = parse_response(&raw) {
                    if resp.status >= 100 {
                        return Ok(resp);
                    }
                }
            }
            Err(rusty_tls::record::TlsError::CloseNotify) => break,
            Err(rusty_tls::record::TlsError::UnexpectedEnd) => break,
            Err(e) => return Err(HttpError::Tls(format!("recv: {e:?}"))),
        }
    }
    parse_response(&raw).map_err(|e| HttpError::Codec(format!("{e:?}")))
}

fn pm_http_get_raw(url: &str, accept: &str) -> Result<rusty_http_codec::ParsedResponse, HttpError> {
    let dbg = std::env::var("CRUFT_TLS_DEBUG").is_ok();
    if dbg {
        eprintln!("[pm_http_get] start {}", url);
    }
    let u = parse_url(url)?;

    if let Some(mut session) = take_pooled(&u.host, u.port) {
        if dbg {
            eprintln!(
                "[pm_http_get] reusing pooled session → {}:{}",
                u.host, u.port
            );
        }
        match exchange(&mut session, &u, accept) {
            Ok(resp) => {
                if response_keeps_alive(&resp) {
                    return_pooled(&u.host, u.port, session);
                }
                return Ok(resp);
            }
            Err(_) => {

                if dbg {
                    eprintln!("[pm_http_get] pooled session stale; reconnecting");
                }
            }
        }
    }

    let trust_store =
        TrustStore::load_system_default().map_err(|e| HttpError::TrustStore(format!("{e:?}")))?;
    if dbg {
        eprintln!("[pm_http_get] connecting → {}:{}", u.host, u.port);
    }
    let mut session = tls_connect(&u.host, u.port, &trust_store)
        .map_err(|e| HttpError::Tls(format!("connect {}:{}: {e:?}", u.host, u.port)))?;
    if dbg {
        eprintln!("[pm_http_get] handshake OK");
    }
    let resp = exchange(&mut session, &u, accept)?;
    if response_keeps_alive(&resp) {
        return_pooled(&u.host, u.port, session);
    }
    Ok(resp)
}

pub const ABBREVIATED_PACKUMENT_ACCEPT: &str = "application/vnd.npm.install-v1+json";

pub fn pm_http_get_accept(url: &str, accept: &str) -> Result<Vec<u8>, HttpError> {
    let resp = pm_http_get_raw(url, accept)?;
    finalize_raw(resp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_parse_basic() {
        let u = parse_url("https://registry.npmjs.org/lodash/4.17.21").unwrap();
        assert_eq!(u.host, "registry.npmjs.org");
        assert_eq!(u.port, 443);
        assert_eq!(u.path, "/lodash/4.17.21");
    }

    #[test]
    fn url_parse_no_path() {
        let u = parse_url("https://registry.npmjs.org").unwrap();
        assert_eq!(u.path, "/");
    }

    #[test]
    fn url_parse_explicit_port() {
        let u = parse_url("https://example.com:8443/foo").unwrap();
        assert_eq!(u.port, 8443);
    }

    #[test]
    fn url_parse_rejects_http() {
        assert!(matches!(
            parse_url("http://x/y"),
            Err(HttpError::UnsupportedScheme(_))
        ));
    }

    #[test]
    #[ignore]
    fn fetch_lodash_manifest() {
        let body = pm_http_get("https://registry.npmjs.org/lodash/4.17.21")
            .expect("registry fetch failed");
        let text = std::str::from_utf8(&body).expect("body not utf-8");
        assert!(
            text.contains("\"version\":\"4.17.21\""),
            "expected version in response, got {} bytes; first 200: {}",
            text.len(),
            &text[..text.len().min(200)]
        );
        assert!(
            text.contains("\"tarball\""),
            "expected dist.tarball in response"
        );
    }
}
