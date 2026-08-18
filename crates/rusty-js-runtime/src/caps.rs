
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapMode {

    Compat,

    Audit,

    SealedDeps,

    Sealed,
}

impl Default for CapMode {
    fn default() -> Self {
        Self::Compat
    }
}

impl CapMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Compat => "compat",
            Self::Audit => "audit",
            Self::SealedDeps => "sealed-deps",
            Self::Sealed => "sealed",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "compat" | "0" => Some(Self::Compat),
            "audit" | "1" => Some(Self::Audit),
            "sealed-deps" | "2" => Some(Self::SealedDeps),
            "sealed" | "3" => Some(Self::Sealed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleProvenance {

    Application,

    Dependency,

    External,

    Builtin,
}

impl ModuleProvenance {
    pub fn is_application(&self) -> bool {
        matches!(self, Self::Application | Self::Builtin)
    }
}

#[derive(Debug, Clone)]
pub struct ModuleId {
    pub url: String,
    pub provenance: ModuleProvenance,
}

impl ModuleId {
    pub fn application(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            provenance: ModuleProvenance::Application,
        }
    }

    pub fn dependency(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            provenance: ModuleProvenance::Dependency,
        }
    }

    pub fn builtin(name: impl Into<String>) -> Self {
        Self {
            url: name.into(),
            provenance: ModuleProvenance::Builtin,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CapabilityError {
    pub capability: &'static str,
    pub operation: String,
    pub calling_module: String,
    pub mode: CapMode,
    pub hint: Option<String>,
}

impl std::fmt::Display for CapabilityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}.{}: no {} capability granted to module '{}' (mode: {})",
            self.capability,
            self.operation,
            self.capability,
            self.calling_module,
            self.mode.as_str()
        )?;
        if let Some(hint) = &self.hint {
            write!(f, " — hint: {hint}")?;
        }
        Ok(())
    }
}

impl std::error::Error for CapabilityError {}

#[derive(Debug, Clone)]
pub enum PathPolicy {
    None,
    Any,
    Prefix(PathBuf),
    Prefixes(Vec<PathBuf>),
    Exact(Vec<PathBuf>),
}

impl PathPolicy {
    pub fn allows(&self, path: &std::path::Path) -> bool {
        match self {
            Self::None => false,
            Self::Any => true,
            Self::Prefix(p) => path.starts_with(p),
            Self::Prefixes(ps) => ps.iter().any(|p| path.starts_with(p)),
            Self::Exact(ps) => ps.iter().any(|p| p == path),
        }
    }
}

fn path_allowed_for_fs_op(policy: &PathPolicy, path: &std::path::Path) -> bool {
    if policy.allows(path) {
        return true;
    }
    if path.is_absolute() {
        return false;
    }
    std::env::current_dir()
        .ok()
        .map(|cwd| policy.allows(&cwd.join(path)))
        .unwrap_or(false)
}

#[derive(Debug, Clone)]
pub struct Fs {
    pub read: PathPolicy,
    pub write: PathPolicy,
    pub list: PathPolicy,
    pub stat: PathPolicy,
    pub mkdir: PathPolicy,
    pub remove: PathPolicy,
}

impl Fs {

    pub fn full() -> Self {
        Self {
            read: PathPolicy::Any,
            write: PathPolicy::Any,
            list: PathPolicy::Any,
            stat: PathPolicy::Any,
            mkdir: PathPolicy::Any,
            remove: PathPolicy::Any,
        }
    }

    pub fn none() -> Self {
        Self {
            read: PathPolicy::None,
            write: PathPolicy::None,
            list: PathPolicy::None,
            stat: PathPolicy::None,
            mkdir: PathPolicy::None,
            remove: PathPolicy::None,
        }
    }

    pub fn sub_dir(&self, prefix: impl Into<PathBuf>) -> Self {
        let prefix = prefix.into();
        let narrow = |p: &PathPolicy| -> PathPolicy {
            match p {
                PathPolicy::None => PathPolicy::None,
                _ => PathPolicy::Prefix(prefix.clone()),
            }
        };
        Self {
            read: narrow(&self.read),
            write: narrow(&self.write),
            list: narrow(&self.list),
            stat: narrow(&self.stat),
            mkdir: narrow(&self.mkdir),
            remove: narrow(&self.remove),
        }
    }

    pub fn read_only(&self) -> Self {
        Self {
            read: self.read.clone(),
            list: self.list.clone(),
            stat: self.stat.clone(),
            write: PathPolicy::None,
            mkdir: PathPolicy::None,
            remove: PathPolicy::None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum FsOp {
    Read(PathBuf),
    Write(PathBuf),
    List(PathBuf),
    Stat(PathBuf),
    Mkdir(PathBuf),
    Remove(PathBuf),
}

impl FsOp {
    fn describe(&self) -> String {
        match self {
            Self::Read(p) => format!("read({})", p.display()),
            Self::Write(p) => format!("write({})", p.display()),
            Self::List(p) => format!("list({})", p.display()),
            Self::Stat(p) => format!("stat({})", p.display()),
            Self::Mkdir(p) => format!("mkdir({})", p.display()),
            Self::Remove(p) => format!("remove({})", p.display()),
        }
    }

    fn policy<'a>(&self, cap: &'a Fs) -> (&'a PathPolicy, &'static str, &std::path::Path) {
        match self {
            Self::Read(p) => (&cap.read, "read", p),
            Self::Write(p) => (&cap.write, "write", p),
            Self::List(p) => (&cap.list, "list", p),
            Self::Stat(p) => (&cap.stat, "stat", p),
            Self::Mkdir(p) => (&cap.mkdir, "mkdir", p),
            Self::Remove(p) => (&cap.remove, "remove", p),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Stdio {
    pub stdout: bool,
    pub stderr: bool,
}

impl Stdio {
    pub fn full() -> Self {
        Self {
            stdout: true,
            stderr: true,
        }
    }
    pub fn none() -> Self {
        Self {
            stdout: false,
            stderr: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum StdioOp {
    Stdout(Vec<u8>),
    Stderr(Vec<u8>),
}

#[derive(Debug, Clone, Copy)]
pub enum ClockResolution {
    Disabled,
    Coarse(Duration),
    Fine,
}

#[derive(Debug, Clone, Copy)]
pub struct Clock {
    pub resolution: ClockResolution,
}

impl Clock {
    pub fn fine() -> Self {
        Self {
            resolution: ClockResolution::Fine,
        }
    }
    pub fn disabled() -> Self {
        Self {
            resolution: ClockResolution::Disabled,
        }
    }
    pub fn coarse(d: Duration) -> Self {
        Self {
            resolution: ClockResolution::Coarse(d),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ClockOp {
    Now,
    HighResolution,
}

#[derive(Debug, Clone, Copy)]
pub struct Scheduler {
    pub timers: bool,
    pub microtasks: bool,
    pub min_delay: Duration,
}

impl Scheduler {
    pub fn full() -> Self {
        Self {
            timers: true,
            microtasks: true,
            min_delay: Duration::ZERO,
        }
    }
    pub fn none() -> Self {
        Self {
            timers: false,
            microtasks: false,
            min_delay: Duration::ZERO,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SchedulerOp {
    Timer(Duration),
    Microtask,
}

#[derive(Debug, Clone, Copy)]
pub struct Process {
    pub may_exit: bool,
    pub may_read_cwd: bool,
    pub may_read_pid: bool,

    pub may_spawn: bool,
}

impl Process {
    pub fn full() -> Self {
        Self {
            may_exit: true,
            may_read_cwd: true,
            may_read_pid: true,
            may_spawn: true,
        }
    }
    pub fn none() -> Self {
        Self {
            may_exit: false,
            may_read_cwd: false,
            may_read_pid: false,
            may_spawn: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ProcessOp {
    Exit(i32),
    ReadCwd,
    ReadPid,

    Spawn {
        program: String,
    },
}

#[derive(Debug, Clone)]
pub enum EnvVarPolicy {
    None,
    Any,
    Whitelist(Vec<String>),
}

impl EnvVarPolicy {
    pub fn allows(&self, name: &str) -> bool {
        match self {
            Self::None => false,
            Self::Any => true,
            Self::Whitelist(ws) => ws.iter().any(|w| w == name),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Env {
    pub vars: EnvVarPolicy,
    pub system_info: bool,
}

impl Env {
    pub fn full() -> Self {
        Self {
            vars: EnvVarPolicy::Any,
            system_info: true,
        }
    }
    pub fn none() -> Self {
        Self {
            vars: EnvVarPolicy::None,
            system_info: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum EnvOp {
    ReadVar(String),
    SystemInfo(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetEndpoint {
    pub host: String,
    pub port: u16,
}

impl NetEndpoint {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
        }
    }

    fn matches(&self, host: &str, port: u16) -> bool {
        self.host == host && self.port == port
    }
}

#[derive(Debug, Clone)]
pub enum NetListenPolicy {
    None,
    Any,
    LoopbackAnyPort,
    Exact(Vec<NetEndpoint>),
}

impl NetListenPolicy {
    fn allows(&self, host: &str, port: u16) -> bool {
        match self {
            Self::None => false,
            Self::Any => true,
            Self::LoopbackAnyPort => is_loopback_host(host),
            Self::Exact(endpoints) => endpoints.iter().any(|ep| ep.matches(host, port)),
        }
    }
}

#[derive(Debug, Clone)]
pub enum NetConnectPolicy {
    None,
    Any,
    LoopbackAnyPort,
    Hosts(Vec<String>),
    Exact(Vec<NetEndpoint>),
}

impl NetConnectPolicy {
    fn allows(&self, host: &str, port: u16) -> bool {
        match self {
            Self::None => false,
            Self::Any => true,
            Self::LoopbackAnyPort => is_loopback_host(host),
            Self::Hosts(hosts) => hosts.iter().any(|h| h == host),
            Self::Exact(endpoints) => endpoints.iter().any(|ep| ep.matches(host, port)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Net {
    pub listen: NetListenPolicy,
    pub connect: NetConnectPolicy,
}

impl Net {
    pub fn full() -> Self {
        Self {
            listen: NetListenPolicy::Any,
            connect: NetConnectPolicy::Any,
        }
    }

    pub fn none() -> Self {
        Self {
            listen: NetListenPolicy::None,
            connect: NetConnectPolicy::None,
        }
    }

    pub fn loopback_server() -> Self {
        Self {
            listen: NetListenPolicy::LoopbackAnyPort,
            connect: NetConnectPolicy::LoopbackAnyPort,
        }
    }

    pub fn listen_exact(host: impl Into<String>, port: u16) -> Self {
        Self {
            listen: NetListenPolicy::Exact(vec![NetEndpoint::new(host, port)]),
            connect: NetConnectPolicy::None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum NetOp {
    Listen { host: String, port: u16 },
    Connect { host: String, port: u16 },
}

impl NetOp {
    fn describe(&self) -> String {
        match self {
            Self::Listen { host, port } => format!("listen({host}:{port})"),
            Self::Connect { host, port } => format!("connect({host}:{port})"),
        }
    }

    fn allows(&self, cap: &Net) -> bool {
        match self {
            Self::Listen { host, port } => cap.listen.allows(host, *port),
            Self::Connect { host, port } => cap.connect.allows(host, *port),
        }
    }

    fn hint(&self) -> String {
        match self {
            Self::Listen { host, port } => {
                format!("add to cruft-caps.json: {{ \"net\": [\"{host}:{port}\"] }}")
            }
            Self::Connect { host, port } => {
                format!("add to cruft-caps.json: {{ \"net\": [\"{host}:{port}\"] }}")
            }
        }
    }
}

fn is_loopback_host(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "::1" | "[::1]")
}

#[derive(Debug, Clone)]
pub struct AmbientCaps {
    pub fs: Fs,
    pub stdio: Stdio,
    pub clock: Clock,
    pub scheduler: Scheduler,
    pub process: Process,
    pub env: Env,
    pub net: Net,
}

impl AmbientCaps {
    pub fn full() -> Self {
        Self {
            fs: Fs::full(),
            stdio: Stdio::full(),
            clock: Clock::fine(),
            scheduler: Scheduler::full(),
            process: Process::full(),
            env: Env::full(),
            net: Net::full(),
        }
    }
}

impl Default for AmbientCaps {
    fn default() -> Self {
        Self::full()
    }
}

#[derive(Debug, Default)]
pub struct AuditLog {
    pub records: Vec<AuditRecord>,
}

#[derive(Debug, Clone)]
pub struct AuditRecord {
    pub caller: String,
    pub capability: &'static str,
    pub operation: String,
    pub timestamp_micros: u128,
}

impl AuditLog {
    pub fn record(&mut self, caller: &ModuleId, capability: &'static str, op: &str) {
        let timestamp_micros = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros())
            .unwrap_or(0);
        self.records.push(AuditRecord {
            caller: caller.url.clone(),
            capability,
            operation: op.to_string(),
            timestamp_micros,
        });
    }
}

#[derive(Debug, Clone)]
pub struct ModuleGrant {
    pub fs: Fs,
    pub net: Net,
    pub env: Env,
    pub process: Process,
    pub stdio: Stdio,
}

pub struct CapDispatcher {
    pub mode: CapMode,
    pub ambient: AmbientCaps,
    pub net_grant: Net,
    pub audit: Mutex<AuditLog>,

    per_module: Mutex<std::collections::HashMap<String, ModuleGrant>>,
}

impl CapDispatcher {
    pub fn new(mode: CapMode) -> Self {
        Self {
            mode,
            ambient: AmbientCaps::full(),
            net_grant: Net::none(),
            audit: Mutex::new(AuditLog::default()),
            per_module: Mutex::new(std::collections::HashMap::new()),
        }
    }

    pub fn grant_module(
        &self,
        url: &str,
        fs: Fs,
        net: Net,
        env: Env,
        process: Process,
        stdio: Stdio,
    ) {
        if let Ok(mut g) = self.per_module.lock() {
            g.insert(
                url.to_string(),
                ModuleGrant {
                    fs,
                    net,
                    env,
                    process,
                    stdio,
                },
            );
        }
    }

    fn module_grant(&self, url: &str) -> Option<ModuleGrant> {
        self.per_module
            .lock()
            .ok()
            .and_then(|g| g.get(url).cloned())
    }

    pub fn with_net_grant(mut self, net: Net) -> Self {
        self.net_grant = net;
        self
    }

    pub fn compat() -> Self {
        Self::new(CapMode::Compat)
    }

    pub fn audit_mode() -> Self {
        Self::new(CapMode::Audit)
    }

    pub fn drain_audit(&self) -> Vec<AuditRecord> {
        let mut g = self.audit.lock().expect("audit log poisoned");
        std::mem::take(&mut g.records)
    }

    fn record_audit(&self, caller: &ModuleId, capability: &'static str, op: &str) {
        if !matches!(self.mode, CapMode::Audit) {
            return;
        }
        if let Ok(mut g) = self.audit.lock() {
            g.record(caller, capability, op);
        }
    }

    pub fn require_fs(&self, cap: &Fs, op: FsOp, caller: &ModuleId) -> Result<(), CapabilityError> {
        let op_desc = op.describe();
        self.record_audit(caller, "fs", &op_desc);
        match self.mode {
            CapMode::Compat | CapMode::Audit => Ok(()),
            CapMode::SealedDeps if caller.provenance.is_application() => Ok(()),
            CapMode::SealedDeps | CapMode::Sealed => {
                let (policy, action, path) = op.policy(cap);
                let granted = self
                    .module_grant(&caller.url)
                    .is_some_and(|g| path_allowed_for_fs_op(op.policy(&g.fs).0, path));
                if path_allowed_for_fs_op(policy, path) || granted {
                    Ok(())
                } else {
                    Err(CapabilityError {
                        capability: "fs",
                        operation: op_desc,
                        calling_module: caller.url.clone(),
                        mode: self.mode,
                        hint: Some(format!(
                            "add to cruft-caps.json: {{ \"fs\": [\"{}\"] }} for fs {action}",
                            path.display()
                        )),
                    })
                }
            }
        }
    }

    pub fn require_stdio(
        &self,
        cap: &Stdio,
        op: StdioOp,
        caller: &ModuleId,
    ) -> Result<(), CapabilityError> {
        let (allowed, stream) = match &op {
            StdioOp::Stdout(_) => (cap.stdout, "stdout"),
            StdioOp::Stderr(_) => (cap.stderr, "stderr"),
        };
        let op_desc = format!("write({stream})");
        self.record_audit(caller, "stdio", &op_desc);
        match self.mode {
            CapMode::Compat | CapMode::Audit => Ok(()),
            CapMode::SealedDeps if caller.provenance.is_application() => Ok(()),
            CapMode::SealedDeps | CapMode::Sealed => {
                let granted = self.module_grant(&caller.url).is_some_and(|g| match &op {
                    StdioOp::Stdout(_) => g.stdio.stdout,
                    StdioOp::Stderr(_) => g.stdio.stderr,
                });
                if allowed || granted {
                    Ok(())
                } else {
                    Err(CapabilityError {
                        capability: "stdio",
                        operation: op_desc,
                        calling_module: caller.url.clone(),
                        mode: self.mode,
                        hint: Some(format!(
                            "add to cruft-caps.json: {{ \"stdio\": {{ \"{stream}\": true }} }}"
                        )),
                    })
                }
            }
        }
    }

    pub fn require_clock(
        &self,
        cap: &Clock,
        op: ClockOp,
        caller: &ModuleId,
    ) -> Result<(), CapabilityError> {
        let op_desc = match op {
            ClockOp::Now => "now()".to_string(),
            ClockOp::HighResolution => "highResolution()".to_string(),
        };
        self.record_audit(caller, "clock", &op_desc);
        match self.mode {
            CapMode::Compat | CapMode::Audit => Ok(()),
            CapMode::SealedDeps if caller.provenance.is_application() => Ok(()),
            CapMode::SealedDeps | CapMode::Sealed => {
                match cap.resolution {
                    ClockResolution::Disabled => Err(CapabilityError {
                        capability: "clock",
                        operation: op_desc,
                        calling_module: caller.url.clone(),
                        mode: self.mode,
                        hint: Some(
                            "no cruft-caps.json grant surface exists for clock yet; avoid clock access or run outside --sealed".into(),
                        ),
                    }),
                    _ => Ok(()),
                }
            }
        }
    }

    pub fn require_scheduler(
        &self,
        cap: &Scheduler,
        op: SchedulerOp,
        caller: &ModuleId,
    ) -> Result<(), CapabilityError> {
        let (allowed, kind, op_desc) = match op {
            SchedulerOp::Timer(d) => (cap.timers, "timers", format!("timer({}ms)", d.as_millis())),
            SchedulerOp::Microtask => (cap.microtasks, "microtasks", "microtask".to_string()),
        };
        self.record_audit(caller, "scheduler", &op_desc);
        match self.mode {
            CapMode::Compat | CapMode::Audit => Ok(()),
            CapMode::SealedDeps if caller.provenance.is_application() => Ok(()),
            CapMode::SealedDeps | CapMode::Sealed => {
                if allowed {
                    Ok(())
                } else {
                    Err(CapabilityError {
                        capability: "scheduler",
                        operation: op_desc,
                        calling_module: caller.url.clone(),
                        mode: self.mode,
                        hint: Some(format!(
                            "no cruft-caps.json grant surface exists for scheduler {kind} yet; avoid scheduler access or run outside --sealed"
                        )),
                    })
                }
            }
        }
    }

    pub fn require_process(
        &self,
        cap: &Process,
        op: ProcessOp,
        caller: &ModuleId,
    ) -> Result<(), CapabilityError> {
        let allows_by = |c: &Process| match &op {
            ProcessOp::Exit(_) => c.may_exit,
            ProcessOp::ReadCwd => c.may_read_cwd,
            ProcessOp::ReadPid => c.may_read_pid,
            ProcessOp::Spawn { .. } => c.may_spawn,
        };
        let (allowed, op_desc) = match &op {
            ProcessOp::Exit(c) => (cap.may_exit, format!("exit({c})")),
            ProcessOp::ReadCwd => (cap.may_read_cwd, "cwd()".into()),
            ProcessOp::ReadPid => (cap.may_read_pid, "pid".into()),
            ProcessOp::Spawn { program } => (cap.may_spawn, format!("spawn({program})")),
        };
        self.record_audit(caller, "process", &op_desc);
        match self.mode {
            CapMode::Compat | CapMode::Audit => Ok(()),
            CapMode::SealedDeps if caller.provenance.is_application() => Ok(()),
            CapMode::SealedDeps | CapMode::Sealed => {
                let granted = self
                    .module_grant(&caller.url)
                    .is_some_and(|g| allows_by(&g.process));
                if allowed || granted {
                    Ok(())
                } else {
                    let hint = match &op {
                        ProcessOp::Spawn { program } => {
                            format!("add to cruft-caps.json: {{ \"exec\": [\"{program}\"] }}")
                        }
                        ProcessOp::Exit(_) => {
                            "no cruft-caps.json grant surface exists for process exit yet; avoid process.exit or run outside --sealed".into()
                        }
                        ProcessOp::ReadCwd => {
                            "no cruft-caps.json grant surface exists for process cwd yet; avoid process.cwd or run outside --sealed".into()
                        }
                        ProcessOp::ReadPid => {
                            "no cruft-caps.json grant surface exists for process pid yet; avoid process.pid or run outside --sealed".into()
                        }
                    };
                    Err(CapabilityError {
                        capability: "process",
                        operation: op_desc,
                        calling_module: caller.url.clone(),
                        mode: self.mode,
                        hint: Some(hint),
                    })
                }
            }
        }
    }

    pub fn require_env(
        &self,
        cap: &Env,
        op: EnvOp,
        caller: &ModuleId,
    ) -> Result<(), CapabilityError> {
        let op_desc = match &op {
            EnvOp::ReadVar(name) => format!("readVar({name})"),
            EnvOp::SystemInfo(field) => format!("systemInfo({field})"),
        };
        self.record_audit(caller, "env", &op_desc);
        match self.mode {
            CapMode::Compat | CapMode::Audit => Ok(()),
            CapMode::SealedDeps if caller.provenance.is_application() => Ok(()),
            CapMode::SealedDeps | CapMode::Sealed => {
                let allowed_by = |c: &Env| match &op {
                    EnvOp::ReadVar(name) => c.vars.allows(name),
                    EnvOp::SystemInfo(_) => c.system_info,
                };
                let granted = self
                    .module_grant(&caller.url)
                    .is_some_and(|g| allowed_by(&g.env));
                if allowed_by(cap) || granted {
                    Ok(())
                } else {
                    let hint = match &op {
                        EnvOp::ReadVar(name) => {
                            format!("add to cruft-caps.json: {{ \"env\": [\"{name}\"] }}")
                        }
                        EnvOp::SystemInfo(_) => {
                            "no cruft-caps.json grant surface exists for env systemInfo yet; avoid systemInfo access or run outside --sealed".into()
                        }
                    };
                    Err(CapabilityError {
                        capability: "env",
                        operation: op_desc,
                        calling_module: caller.url.clone(),
                        mode: self.mode,
                        hint: Some(hint),
                    })
                }
            }
        }
    }

    pub fn require_net(
        &self,
        cap: &Net,
        op: NetOp,
        caller: &ModuleId,
    ) -> Result<(), CapabilityError> {
        let op_desc = op.describe();
        self.record_audit(caller, "net", &op_desc);
        match self.mode {
            CapMode::Compat | CapMode::Audit => Ok(()),
            CapMode::SealedDeps if caller.provenance.is_application() => Ok(()),
            CapMode::SealedDeps | CapMode::Sealed => {
                let granted = self
                    .module_grant(&caller.url)
                    .is_some_and(|g| op.allows(&g.net));
                if op.allows(cap) || op.allows(&self.net_grant) || granted {
                    Ok(())
                } else {
                    Err(CapabilityError {
                        capability: "net",
                        operation: op_desc,
                        calling_module: caller.url.clone(),
                        mode: self.mode,
                        hint: Some(op.hint()),
                    })
                }
            }
        }
    }

    pub fn has_explicit_net_grant(&self, op: &NetOp, caller: &ModuleId) -> bool {
        op.allows(&self.net_grant)
            || self
                .module_grant(&caller.url)
                .is_some_and(|g| op.allows(&g.net))
    }

    pub fn require_native_addon_load(
        &self,
        path: &str,
        caller: &ModuleId,
    ) -> Result<(), CapabilityError> {
        let op_desc = format!("dlopen({path})");
        self.record_audit(caller, "native-addon", &op_desc);
        match self.mode {
            CapMode::Compat | CapMode::Audit => Ok(()),
            CapMode::SealedDeps | CapMode::Sealed => Err(CapabilityError {
                capability: "native-addon",
                operation: op_desc,
                calling_module: caller.url.clone(),
                mode: self.mode,
                hint: Some(
                    ".node native addons execute arbitrary host code; run outside sealed modes"
                        .into(),
                ),
            }),
        }
    }
}

impl Default for CapDispatcher {
    fn default() -> Self {
        Self::compat()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_caller() -> ModuleId {
        ModuleId::application("file:///proj/app.mjs")
    }
    fn dep_caller() -> ModuleId {
        ModuleId::dependency("file:///proj/node_modules/lodash/index.js")
    }

    #[test]
    fn per_module_grant_permits_granted_dep_op() {

        let d = CapDispatcher::new(CapMode::Sealed);
        let dep = dep_caller();
        let read = || FsOp::Read(std::path::PathBuf::from("/data/x"));

        assert!(d.require_fs(&Fs::none(), read(), &dep).is_err());

        let mut fs = Fs::none();
        fs.read = PathPolicy::Prefixes(vec![std::path::PathBuf::from("/data")]);
        d.grant_module(
            &dep.url,
            fs,
            Net::none(),
            Env::none(),
            Process::none(),
            Stdio::none(),
        );

        assert!(d.require_fs(&Fs::none(), read(), &dep).is_ok());

        assert!(d
            .require_fs(
                &Fs::none(),
                FsOp::Read(std::path::PathBuf::from("/etc/passwd")),
                &dep
            )
            .is_err());

        let other = ModuleId::dependency("file:///proj/node_modules/evil/index.js");
        assert!(d.require_fs(&Fs::none(), read(), &other).is_err());
    }

    #[test]
    fn per_module_grant_covers_net_and_env() {

        let d = CapDispatcher::new(CapMode::Sealed);
        let dep = dep_caller();
        let connect = || NetOp::Connect {
            host: "api.example.com".into(),
            port: 443,
        };
        let read_secret = || EnvOp::ReadVar("SECRET".into());

        assert!(d.require_net(&Net::none(), connect(), &dep).is_err());
        assert!(d.require_env(&Env::none(), read_secret(), &dep).is_err());

        let net = Net {
            listen: NetListenPolicy::None,
            connect: NetConnectPolicy::Exact(vec![NetEndpoint::new("api.example.com", 443)]),
        };
        let env = Env {
            vars: EnvVarPolicy::Whitelist(vec!["SECRET".into()]),
            system_info: false,
        };
        d.grant_module(
            &dep.url,
            Fs::none(),
            net,
            env,
            Process::none(),
            Stdio::none(),
        );

        assert!(d.require_net(&Net::none(), connect(), &dep).is_ok());
        assert!(d.require_env(&Env::none(), read_secret(), &dep).is_ok());

        assert!(d
            .require_net(
                &Net::none(),
                NetOp::Connect {
                    host: "evil.com".into(),
                    port: 80
                },
                &dep
            )
            .is_err());
        assert!(d
            .require_env(&Env::none(), EnvOp::ReadVar("OTHER".into()), &dep)
            .is_err());

        let other = ModuleId::dependency("file:///proj/node_modules/x/i.js");
        assert!(d.require_net(&Net::none(), connect(), &other).is_err());
        assert!(d.require_env(&Env::none(), read_secret(), &other).is_err());
    }

    #[test]
    fn per_module_grant_covers_process_spawn() {

        let d = CapDispatcher::new(CapMode::Sealed);
        let dep = dep_caller();
        let spawn = || ProcessOp::Spawn {
            program: "echo".into(),
        };

        assert!(d.require_process(&Process::none(), spawn(), &dep).is_err());

        d.grant_module(
            &dep.url,
            Fs::none(),
            Net::none(),
            Env::none(),
            Process {
                may_spawn: true,
                ..Process::none()
            },
            Stdio::none(),
        );

        assert!(d.require_process(&Process::none(), spawn(), &dep).is_ok());

        let other = ModuleId::dependency("file:///proj/node_modules/evil/index.js");
        assert!(d
            .require_process(&Process::none(), spawn(), &other)
            .is_err());
    }

    #[test]
    fn per_module_grant_covers_stdio() {
        let d = CapDispatcher::new(CapMode::Sealed);
        let dep = dep_caller();
        assert!(d
            .require_stdio(&Stdio::none(), StdioOp::Stdout(b"x".to_vec()), &dep)
            .is_err());
        d.grant_module(
            &dep.url,
            Fs::none(),
            Net::none(),
            Env::none(),
            Process::none(),
            Stdio {
                stdout: true,
                stderr: false,
            },
        );
        assert!(d
            .require_stdio(&Stdio::none(), StdioOp::Stdout(b"x".to_vec()), &dep)
            .is_ok());
        assert!(d
            .require_stdio(&Stdio::none(), StdioOp::Stderr(b"x".to_vec()), &dep)
            .is_err());
    }

    #[test]
    fn mode_default_is_compat() {
        assert_eq!(CapMode::default(), CapMode::Compat);
    }

    #[test]
    fn cap_mode_parse() {
        assert_eq!(CapMode::from_str("compat"), Some(CapMode::Compat));
        assert_eq!(CapMode::from_str("audit"), Some(CapMode::Audit));
        assert_eq!(CapMode::from_str("sealed-deps"), Some(CapMode::SealedDeps));
        assert_eq!(CapMode::from_str("sealed"), Some(CapMode::Sealed));
        assert_eq!(CapMode::from_str("nope"), None);
    }

    #[test]
    fn compat_mode_allows_everything_no_cap() {
        let d = CapDispatcher::compat();

        let cap = Fs::none();
        let result = d.require_fs(&cap, FsOp::Read("/etc/passwd".into()), &dep_caller());
        assert!(result.is_ok());
    }

    #[test]
    fn audit_mode_allows_and_records() {
        let d = CapDispatcher::audit_mode();
        let cap = Fs::none();
        let r = d.require_fs(&cap, FsOp::Read("/etc/passwd".into()), &dep_caller());
        assert!(r.is_ok());
        let records = d.drain_audit();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].capability, "fs");
        assert!(records[0].operation.contains("/etc/passwd"));
    }

    #[test]
    fn native_addon_audit_records_dlopen() {
        let d = CapDispatcher::audit_mode();
        assert!(d
            .require_native_addon_load("/proj/addon.node", &dep_caller())
            .is_ok());
        let records = d.drain_audit();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].capability, "native-addon");
        assert_eq!(records[0].operation, "dlopen(/proj/addon.node)");
    }

    #[test]
    fn native_addon_sealed_modes_deny_before_dlopen() {
        let sealed_deps = CapDispatcher::new(CapMode::SealedDeps);
        let dep = sealed_deps
            .require_native_addon_load("/proj/node_modules/pkg/addon.node", &dep_caller())
            .unwrap_err();
        assert_eq!(dep.capability, "native-addon");
        assert!(dep.operation.contains("addon.node"));
        assert!(dep.hint.unwrap().contains("arbitrary host code"));

        let sealed = CapDispatcher::new(CapMode::Sealed);
        assert!(sealed
            .require_native_addon_load("/proj/addon.node", &app_caller())
            .is_err());
    }

    #[test]
    fn sealed_deps_dep_blocked() {
        let d = CapDispatcher::new(CapMode::SealedDeps);
        let cap = Fs::none();
        let r = d.require_fs(&cap, FsOp::Read("/etc/passwd".into()), &dep_caller());
        assert!(r.is_err());
        let e = r.unwrap_err();
        assert_eq!(e.capability, "fs");
        assert!(e.hint.is_some());
    }

    #[test]
    fn sealed_deps_app_passes() {
        let d = CapDispatcher::new(CapMode::SealedDeps);
        let cap = Fs::none();

        let r = d.require_fs(&cap, FsOp::Read("/etc/passwd".into()), &app_caller());
        assert!(r.is_ok());
    }

    #[test]
    fn sealed_blocks_app_too() {
        let d = CapDispatcher::new(CapMode::Sealed);
        let cap = Fs::none();

        let r = d.require_fs(&cap, FsOp::Read("/etc/passwd".into()), &app_caller());
        assert!(r.is_err());
    }

    #[test]
    fn fs_prefix_policy() {
        let cap = Fs {
            read: PathPolicy::Prefix("/proj/data".into()),
            ..Fs::none()
        };
        let d = CapDispatcher::new(CapMode::Sealed);
        let ok = d.require_fs(&cap, FsOp::Read("/proj/data/x.txt".into()), &dep_caller());
        assert!(ok.is_ok());
        let denied = d.require_fs(&cap, FsOp::Read("/etc/passwd".into()), &dep_caller());
        assert!(denied.is_err());
    }

    #[test]
    fn fs_sub_dir_narrows() {
        let cap = Fs::full().sub_dir("/proj/data");
        let d = CapDispatcher::new(CapMode::Sealed);
        assert!(d
            .require_fs(&cap, FsOp::Read("/proj/data/x".into()), &dep_caller())
            .is_ok());
        assert!(d
            .require_fs(&cap, FsOp::Read("/proj/secrets".into()), &dep_caller())
            .is_err());
    }

    #[test]
    fn fs_read_only_strips_writes() {
        let cap = Fs::full().read_only();
        let d = CapDispatcher::new(CapMode::Sealed);
        assert!(d
            .require_fs(&cap, FsOp::Read("/x".into()), &dep_caller())
            .is_ok());
        assert!(d
            .require_fs(&cap, FsOp::Write("/x".into()), &dep_caller())
            .is_err());
    }

    #[test]
    fn stdio_sealed_blocks_unless_granted() {
        let d = CapDispatcher::new(CapMode::Sealed);
        let none = Stdio::none();
        assert!(d
            .require_stdio(&none, StdioOp::Stdout(b"x".to_vec()), &dep_caller())
            .is_err());
        let stdout_only = Stdio {
            stdout: true,
            stderr: false,
        };
        assert!(d
            .require_stdio(&stdout_only, StdioOp::Stdout(b"x".to_vec()), &dep_caller())
            .is_ok());
        assert!(d
            .require_stdio(&stdout_only, StdioOp::Stderr(b"x".to_vec()), &dep_caller())
            .is_err());
    }

    #[test]
    fn clock_disabled_blocks() {
        let d = CapDispatcher::new(CapMode::Sealed);
        let cap = Clock::disabled();
        assert!(d.require_clock(&cap, ClockOp::Now, &dep_caller()).is_err());
        let cap2 = Clock::fine();
        assert!(d.require_clock(&cap2, ClockOp::Now, &dep_caller()).is_ok());
    }

    #[test]
    fn scheduler_disabled_blocks() {

        let sealed = CapDispatcher::new(CapMode::Sealed);
        let none = Scheduler::none();
        assert!(sealed
            .require_scheduler(&none, SchedulerOp::Timer(Duration::ZERO), &dep_caller())
            .is_err());
        assert!(sealed
            .require_scheduler(&none, SchedulerOp::Microtask, &dep_caller())
            .is_err());
        let full = Scheduler::full();
        assert!(sealed
            .require_scheduler(&full, SchedulerOp::Timer(Duration::ZERO), &dep_caller())
            .is_ok());
        let compat = CapDispatcher::new(CapMode::Compat);
        assert!(compat
            .require_scheduler(&none, SchedulerOp::Timer(Duration::ZERO), &dep_caller())
            .is_ok());
    }

    #[test]
    fn process_exit_gated() {
        let d = CapDispatcher::new(CapMode::Sealed);
        let none = Process::none();
        assert!(d
            .require_process(&none, ProcessOp::Exit(1), &dep_caller())
            .is_err());
        let full = Process::full();
        assert!(d
            .require_process(&full, ProcessOp::Exit(1), &dep_caller())
            .is_ok());
    }

    #[test]
    fn env_whitelist() {
        let cap = Env {
            vars: EnvVarPolicy::Whitelist(vec!["LANG".into(), "TZ".into()]),
            system_info: false,
        };
        let d = CapDispatcher::new(CapMode::Sealed);
        assert!(d
            .require_env(&cap, EnvOp::ReadVar("LANG".into()), &dep_caller())
            .is_ok());
        assert!(d
            .require_env(&cap, EnvOp::ReadVar("AWS_KEY".into()), &dep_caller())
            .is_err());
        assert!(d
            .require_env(&cap, EnvOp::SystemInfo("cpus"), &dep_caller())
            .is_err());
    }

    #[test]
    fn net_loopback_listen_policy() {
        let d = CapDispatcher::new(CapMode::Sealed);
        let cap = Net::loopback_server();
        assert!(d
            .require_net(
                &cap,
                NetOp::Listen {
                    host: "127.0.0.1".into(),
                    port: 0
                },
                &dep_caller()
            )
            .is_ok());
        assert!(d
            .require_net(
                &cap,
                NetOp::Listen {
                    host: "localhost".into(),
                    port: 3000
                },
                &dep_caller()
            )
            .is_ok());
        assert!(d
            .require_net(
                &cap,
                NetOp::Listen {
                    host: "0.0.0.0".into(),
                    port: 0
                },
                &dep_caller()
            )
            .is_err());
    }

    #[test]
    fn net_exact_listen_policy() {
        let d = CapDispatcher::new(CapMode::Sealed);
        let cap = Net::listen_exact("127.0.0.1", 8080);
        assert!(d
            .require_net(
                &cap,
                NetOp::Listen {
                    host: "127.0.0.1".into(),
                    port: 8080
                },
                &dep_caller()
            )
            .is_ok());
        assert!(d
            .require_net(
                &cap,
                NetOp::Listen {
                    host: "127.0.0.1".into(),
                    port: 8081
                },
                &dep_caller()
            )
            .is_err());
    }

    #[test]
    fn net_audit_records_listen() {
        let d = CapDispatcher::audit_mode();
        let cap = Net::none();
        let r = d.require_net(
            &cap,
            NetOp::Listen {
                host: "127.0.0.1".into(),
                port: 0,
            },
            &dep_caller(),
        );
        assert!(r.is_ok());
        let records = d.drain_audit();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].capability, "net");
        assert_eq!(records[0].operation, "listen(127.0.0.1:0)");
    }

    #[test]
    fn net_sealed_deps_app_passes() {
        let d = CapDispatcher::new(CapMode::SealedDeps);
        let cap = Net::none();
        assert!(d
            .require_net(
                &cap,
                NetOp::Listen {
                    host: "0.0.0.0".into(),
                    port: 0
                },
                &app_caller()
            )
            .is_ok());
        assert!(d
            .require_net(
                &cap,
                NetOp::Listen {
                    host: "0.0.0.0".into(),
                    port: 0
                },
                &dep_caller()
            )
            .is_err());
    }

    #[test]
    fn capability_error_display() {
        let d = CapDispatcher::new(CapMode::Sealed);
        let cap = Fs::none();
        let e = d
            .require_fs(&cap, FsOp::Read("/etc/passwd".into()), &dep_caller())
            .unwrap_err();
        let s = format!("{e}");
        assert!(s.contains("fs"));
        assert!(s.contains("/etc/passwd"));
        assert!(s.contains("sealed"));
        assert!(s.contains("hint"));
    }
}
