
use crate::caps::{
    Env, EnvVarPolicy, Fs, Net, NetConnectPolicy, NetEndpoint, NetListenPolicy, PathPolicy,
    Process, Stdio,
};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapsGrant {

    pub fs: Vec<String>,

    pub net: Vec<String>,

    pub env: Vec<String>,

    pub exec: Vec<String>,

    pub stdio_stdout: bool,
    pub stdio_stderr: bool,
}

impl CapsGrant {

    pub fn is_empty(&self) -> bool {
        self.fs.is_empty()
            && self.net.is_empty()
            && self.env.is_empty()
            && self.exec.is_empty()
            && !self.stdio_stdout
            && !self.stdio_stderr
    }

    pub fn parse_str(src: &str) -> Result<Self, serde_json::JsonError> {
        let root: serde_json::Value = serde_json::parse_value_str(src)?;
        Ok(Self::from_json(&root))
    }

    pub fn from_json(root: &serde_json::Value) -> Self {
        let strings = |key: &str| -> Vec<String> {
            root.get(key)
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|e| e.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default()
        };
        Self {
            fs: strings("fs"),
            net: strings("net"),
            env: strings("env"),
            exec: strings("exec"),
            stdio_stdout: root
                .get("stdio")
                .and_then(|v| v.get("stdout"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            stdio_stderr: root
                .get("stdio")
                .and_then(|v| v.get("stderr"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        }
    }

    pub fn load_file(path: impl AsRef<std::path::Path>) -> Result<Self, serde_json::JsonError> {
        match std::fs::read_to_string(path.as_ref()) {
            Ok(body) => Self::parse_str(&body),
            Err(_) => Ok(Self::default()),
        }
    }

    pub fn to_fs(&self) -> Fs {
        if self.fs.is_empty() {
            return Fs::none();
        }
        let prefixes: Vec<PathBuf> = self.fs.iter().map(PathBuf::from).collect();
        let policy = PathPolicy::Prefixes(prefixes);
        Fs {
            read: policy.clone(),
            write: policy.clone(),
            list: policy.clone(),
            stat: policy.clone(),
            mkdir: policy.clone(),
            remove: policy,
        }
    }

    pub fn to_net(&self) -> Net {
        if self.net.is_empty() {
            return Net::none();
        }
        let mut exact: Vec<NetEndpoint> = Vec::new();
        let mut hosts: Vec<String> = Vec::new();
        for entry in &self.net {
            match parse_host_port(entry) {
                (host, Some(port)) => exact.push(NetEndpoint::new(host, port)),
                (host, None) => hosts.push(host),
            }
        }

        let connect = if hosts.is_empty() {
            NetConnectPolicy::Exact(exact)
        } else {
            for ep in exact {
                hosts.push(ep.host);
            }
            NetConnectPolicy::Hosts(hosts)
        };
        Net {
            listen: listen_policy_from_entries(&self.net),
            connect,
        }
    }

    pub fn to_env(&self) -> Env {
        if self.env.is_empty() {
            return Env::none();
        }
        Env {
            vars: EnvVarPolicy::Whitelist(self.env.clone()),
            system_info: false,
        }
    }

    pub fn to_process(&self) -> Process {
        if self.exec.is_empty() {
            return Process::none();
        }
        Process {
            may_spawn: true,
            ..Process::none()
        }
    }

    pub fn to_stdio(&self) -> Stdio {
        Stdio {
            stdout: self.stdio_stdout,
            stderr: self.stdio_stderr,
        }
    }
}

fn parse_host_port(entry: &str) -> (String, Option<u16>) {
    if let Some((host, port)) = entry.rsplit_once(':') {
        if let Ok(p) = port.parse::<u16>() {
            if !host.is_empty() {
                return (host.to_string(), Some(p));
            }
        }
    }
    (entry.to_string(), None)
}

fn listen_policy_from_entries(entries: &[String]) -> NetListenPolicy {
    let mut exact = Vec::new();
    for entry in entries {
        let (host, port) = parse_host_port(entry);
        if let Some(port) = port {
            exact.push(NetEndpoint::new(host, port));
        }
    }
    if exact.is_empty() {
        NetListenPolicy::None
    } else {
        NetListenPolicy::Exact(exact)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_document() {
        let src = r#"{
            "fs": ["/allowed/path", "/tmp/cache"],
            "net": ["api.example.com:443", "db.internal:5432"],
            "env": ["HOME", "PATH"],
            "exec": ["git"],
            "stdio": { "stdout": true }
        }"#;
        let g = CapsGrant::parse_str(src).expect("parse");
        assert_eq!(g.fs, vec!["/allowed/path", "/tmp/cache"]);
        assert_eq!(g.net, vec!["api.example.com:443", "db.internal:5432"]);
        assert_eq!(g.env, vec!["HOME", "PATH"]);
        assert_eq!(g.exec, vec!["git"]);
        assert!(g.stdio_stdout);
        assert!(!g.stdio_stderr);
        assert!(!g.is_empty());
    }

    #[test]
    fn empty_document_is_default() {
        let g = CapsGrant::parse_str("{}").expect("parse");
        assert!(g.is_empty());
        assert_eq!(g, CapsGrant::default());
    }

    #[test]
    fn absent_file_is_default() {
        let g = CapsGrant::load_file("/no/such/cruftless-caps.json").expect("absent ⇒ default");
        assert!(g.is_empty());
    }

    #[test]
    fn present_file_parses() {
        let mut path = std::env::temp_dir();
        path.push(format!("cruftless-caps-test-{}.json", std::process::id()));
        std::fs::write(&path, r#"{ "fs": ["/data"], "env": ["TOKEN"] }"#).unwrap();
        let g = CapsGrant::load_file(&path).expect("parse");
        let _ = std::fs::remove_file(&path);
        assert_eq!(g.fs, vec!["/data"]);
        assert_eq!(g.env, vec!["TOKEN"]);
        assert!(g.net.is_empty());
    }

    #[test]
    fn projects_onto_caps_shapes() {
        let g = CapsGrant {
            fs: vec!["/allowed".into()],
            net: vec!["host.example:443".into()],
            env: vec!["VAR".into()],
            exec: vec![],
            stdio_stdout: true,
            stdio_stderr: false,
        };

        let fs = g.to_fs();
        assert!(fs.read.allows(std::path::Path::new("/allowed/sub/file")));
        assert!(fs.write.allows(std::path::Path::new("/allowed/x")));
        assert!(!fs.read.allows(std::path::Path::new("/etc/passwd")));

        let env = g.to_env();
        assert!(env.vars.allows("VAR"));
        assert!(!env.vars.allows("OTHER"));

        let net = g.to_net();
        assert!(format!("{:?}", net).contains("host.example"));
        assert!(matches!(net.listen, NetListenPolicy::Exact(_)));
        let stdio = g.to_stdio();
        assert!(stdio.stdout);
        assert!(!stdio.stderr);
    }

    #[test]
    fn empty_grant_projects_to_none() {
        let g = CapsGrant::default();
        assert!(!g.to_fs().read.allows(std::path::Path::new("/anything")));
        assert!(!g.to_env().vars.allows("ANY"));
        assert!(!g.to_process().may_spawn);
        assert!(!g.to_stdio().stdout);
        assert!(!g.to_stdio().stderr);
    }

    #[test]
    fn exec_projects_onto_process_spawn() {

        let g = CapsGrant {
            fs: vec![],
            net: vec![],
            env: vec![],
            exec: vec!["echo".into()],
            stdio_stdout: false,
            stdio_stderr: false,
        };
        let p = g.to_process();
        assert!(p.may_spawn);

        assert!(!p.may_exit);
        assert!(!p.may_read_cwd);
        assert!(!p.may_read_pid);

        assert!(!CapsGrant::default().to_process().may_spawn);
    }

    #[test]
    fn non_array_field_contributes_nothing() {
        let g = CapsGrant::parse_str(r#"{ "fs": "not-an-array", "env": ["OK"] }"#).expect("parse");
        assert!(g.fs.is_empty());
        assert_eq!(g.env, vec!["OK"]);
    }
}
