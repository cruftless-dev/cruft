
use rusty_js_percent_encoding as pe;

pub fn is_special(scheme: &str) -> bool {
    matches!(scheme, "ftp" | "file" | "http" | "https" | "ws" | "wss")
}

pub fn default_port(scheme: &str) -> Option<u16> {
    match scheme {
        "ftp" => Some(21),
        "http" | "ws" => Some(80),
        "https" | "wss" => Some(443),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Path {
    Opaque(String),
    List(Vec<String>),
}

impl Path {
    pub fn is_opaque(&self) -> bool {
        matches!(self, Path::Opaque(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url {
    pub scheme: String,
    pub username: String,
    pub password: String,

    pub host: Option<String>,
    pub port: Option<u16>,
    pub path: Path,
    pub query: Option<String>,
    pub fragment: Option<String>,
}

impl Url {
    pub fn includes_credentials(&self) -> bool {
        !self.username.is_empty() || !self.password.is_empty()
    }

    pub fn has_opaque_path(&self) -> bool {
        self.path.is_opaque()
    }

    pub fn serialize(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.scheme);
        out.push(':');
        if self.host.is_some() {
            out.push_str("//");
            if self.includes_credentials() {
                out.push_str(&self.username);
                if !self.password.is_empty() {
                    out.push(':');
                    out.push_str(&self.password);
                }
                out.push('@');
            }
            out.push_str(self.host.as_deref().unwrap_or(""));
            if let Some(p) = self.port {
                out.push(':');
                out.push_str(&p.to_string());
            }
        } else if !self.has_opaque_path()
            && matches!(&self.path, Path::List(segs) if segs.len() > 1 && segs[0].is_empty())
        {

            out.push_str("/.");
        }
        match &self.path {
            Path::Opaque(s) => out.push_str(s),
            Path::List(segs) => {
                for seg in segs {
                    out.push('/');
                    out.push_str(seg);
                }
            }
        }
        if let Some(q) = &self.query {
            out.push('?');
            out.push_str(q);
        }
        if let Some(f) = &self.fragment {
            out.push('#');
            out.push_str(f);
        }
        out
    }

    pub fn protocol(&self) -> String {
        format!("{}:", self.scheme)
    }

    pub fn host_str(&self) -> String {
        match (&self.host, self.port) {
            (Some(h), Some(p)) => format!("{}:{}", h, p),
            (Some(h), None) => h.clone(),
            (None, _) => String::new(),
        }
    }

    pub fn hostname(&self) -> String {
        self.host.clone().unwrap_or_default()
    }

    pub fn port_str(&self) -> String {
        self.port.map(|p| p.to_string()).unwrap_or_default()
    }

    pub fn pathname(&self) -> String {
        match &self.path {
            Path::Opaque(s) => s.clone(),
            Path::List(segs) => {
                let mut out = String::new();
                for seg in segs {
                    out.push('/');
                    out.push_str(seg);
                }
                out
            }
        }
    }

    pub fn search(&self) -> String {
        match &self.query {
            Some(q) if !q.is_empty() => format!("?{}", q),
            _ => String::new(),
        }
    }

    pub fn hash(&self) -> String {
        match &self.fragment {
            Some(f) if !f.is_empty() => format!("#{}", f),
            _ => String::new(),
        }
    }

    pub fn origin(&self) -> String {
        match self.scheme.as_str() {
            "blob" => self.blob_origin(),
            "ftp" | "http" | "https" | "ws" | "wss" => match &self.host {
                Some(h) => match self.port {
                    Some(p) => format!("{}://{}:{}", self.scheme, h, p),
                    None => format!("{}://{}", self.scheme, h),
                },
                None => String::new(),
            },
            "file" => String::from("null"),
            _ => String::from("null"),
        }
    }

    fn blob_origin(&self) -> String {
        let inner = match &self.path {
            Path::Opaque(path) => path.as_str(),
            Path::List(segments) => segments.first().map(String::as_str).unwrap_or(""),
        };
        match crate::state_machine::parse(inner, None) {
            Ok(url) => {
                let origin = url.origin();
                if origin.is_empty() {
                    String::from("null")
                } else {
                    origin
                }
            }
            Err(()) => String::from("null"),
        }
    }

    pub fn to_json(&self) -> String {
        self.serialize()
    }

    pub fn set_component(&mut self, which: &str, val: &str) -> bool {
        match which {
            "protocol" => {
                let scheme = val.strip_suffix(':').unwrap_or(val);
                if scheme.is_empty()
                    || !scheme
                        .chars()
                        .next()
                        .map(|c| c.is_ascii_alphabetic())
                        .unwrap_or(false)
                    || !scheme
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
                {
                    return false;
                }
                self.scheme = scheme.to_ascii_lowercase();
                if default_port(&self.scheme) == self.port {
                    self.port = None;
                }
                true
            }
            "username" => {
                self.username = pe::encode(val.as_bytes(), &pe::USERINFO);
                true
            }
            "password" => {
                self.password = pe::encode(val.as_bytes(), &pe::USERINFO);
                true
            }
            "host" => {
                if val.is_empty() {
                    self.host = None;
                    self.port = None;
                    return true;
                }
                let (host_part, port_part) = split_host_port(val);
                let host = match crate::state_machine::parse_host(host_part, &self.scheme) {
                    Ok(host) => host,
                    Err(_) => return false,
                };
                let port = match port_part {
                    Some("") => None,
                    Some(p) => match p.parse::<u16>() {
                        Ok(port) if default_port(&self.scheme) != Some(port) => Some(port),
                        Ok(_) => None,
                        Err(_) => return false,
                    },
                    None => None,
                };
                self.host = Some(host);
                self.port = port;
                true
            }
            "hostname" => {
                if val.is_empty() {
                    self.host = None;
                    return true;
                }
                match crate::state_machine::parse_host(val, &self.scheme) {
                    Ok(host) => {
                        self.host = Some(host);
                        true
                    }
                    Err(_) => false,
                }
            }
            "port" => {
                if val.is_empty() {
                    self.port = None;
                    return true;
                }
                match val.parse::<u16>() {
                    Ok(port) => {
                        self.port = if default_port(&self.scheme) == Some(port) {
                            None
                        } else {
                            Some(port)
                        };
                        true
                    }
                    Err(_) => false,
                }
            }
            "pathname" => {
                let encoded = pe::encode(val.as_bytes(), &pe::PATH);
                self.path = if self.host.is_some() || encoded.starts_with('/') {
                    Path::List(
                        encoded
                            .trim_start_matches('/')
                            .split('/')
                            .map(|s| s.to_string())
                            .collect(),
                    )
                } else {
                    Path::Opaque(encoded)
                };
                true
            }
            "search" => {
                let q = val.strip_prefix('?').unwrap_or(val);
                self.query = if q.is_empty() {
                    None
                } else {
                    Some(pe::encode(q.as_bytes(), &pe::SPECIAL_QUERY))
                };
                true
            }
            "hash" => {
                let f = val.strip_prefix('#').unwrap_or(val);
                self.fragment = if f.is_empty() {
                    None
                } else {
                    Some(pe::encode(f.as_bytes(), &pe::FRAGMENT))
                };
                true
            }
            _ => false,
        }
    }
}

fn split_host_port(input: &str) -> (&str, Option<&str>) {
    if input.starts_with('[') {
        if let Some(close) = input.find(']') {
            let host = &input[..=close];
            let rest = &input[close + 1..];
            return match rest.strip_prefix(':') {
                Some(port) => (host, Some(port)),
                None => (input, None),
            };
        }
        return (input, None);
    }
    match input.rsplit_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (input, None),
    }
}
