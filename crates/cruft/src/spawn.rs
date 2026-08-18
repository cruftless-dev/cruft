
use crate::net::{install_emitter, net_buffer_from_bytes, net_emit};
use crate::register::{new_object, register_method};
use rusty_js_runtime::caps::{self, ModuleId, ModuleProvenance};
use rusty_js_runtime::promise::{new_promise, reject_promise, resolve_promise};
use rusty_js_runtime::value::ObjectRef;
use rusty_js_runtime::{AgentId, HostEnqueuePhase, Runtime, RuntimeError, Value};
use std::cell::RefCell;
use std::io::{Read, Write};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub struct Outcome {
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub signal: Option<String>,
    pub timed_out: bool,
}

pub fn require_spawn(rt: &Runtime, program: &str) -> Result<(), RuntimeError> {
    let url = rt.current_module_url.last().cloned().unwrap_or_default();
    let provenance = if url.contains("/node_modules/") {
        ModuleProvenance::Dependency
    } else if url.starts_with("node:") || url.starts_with("cruft:") {
        ModuleProvenance::Builtin
    } else {
        ModuleProvenance::Application
    };
    let caller = ModuleId { url, provenance };
    rt.caps
        .require_process(
            &caps::Process::none(),
            caps::ProcessOp::Spawn {
                program: program.to_string(),
            },
            &caller,
        )
        .map_err(|e| RuntimeError::TypeError(e.to_string()))
}

pub fn run_sync(
    program: &str,
    args: &[String],
    cwd: Option<&str>,
    shell: bool,
) -> Result<Outcome, String> {
    let mut cmd = build_command(program, args, cwd, shell, None);
    let out = cmd.output().map_err(|e| format!("spawn: {e}"))?;
    Ok(Outcome {
        code: out.status.code(),
        stdout: out.stdout,
        stderr: out.stderr,
        signal: exit_signal_name(&out.status),
        timed_out: false,
    })
}

fn build_command(
    program: &str,
    args: &[String],
    cwd: Option<&str>,
    shell: bool,
    env: Option<&[(String, String)]>,
) -> Command {
    let mut cmd = if shell {
        let (sh, flag) = if cfg!(windows) {
            ("cmd", "/C")
        } else {
            ("/bin/sh", "-c")
        };
        let mut c = Command::new(sh);
        c.arg(flag).arg(program);
        c
    } else {
        let mut c = Command::new(program);
        c.args(args);
        c
    };
    if let Some(d) = cwd {
        cmd.current_dir(d);
    }
    if let Some(env) = env {
        cmd.env_clear();
        cmd.envs(env.iter().map(|(k, v)| (k, v)));
    }
    cmd
}

fn pipe_reader<T: Read + Send + 'static>(mut pipe: T) -> JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut out = Vec::new();
        let _ = pipe.read_to_end(&mut out);
        out
    })
}

fn normalize_kill_signal(signal: Option<&str>) -> &'static str {
    match signal.unwrap_or("SIGTERM") {
        "SIGKILL" | "KILL" => "SIGKILL",
        "SIGINT" | "INT" => "SIGINT",
        "SIGTERM" | "TERM" => "SIGTERM",
        _ => "SIGTERM",
    }
}

#[cfg(unix)]
fn signal_number(signal: &str) -> Option<libc::c_int> {
    Some(match signal {
        "SIGHUP" => libc::SIGHUP,
        "SIGINT" => libc::SIGINT,
        "SIGQUIT" => libc::SIGQUIT,
        "SIGILL" => libc::SIGILL,
        "SIGTRAP" => libc::SIGTRAP,
        "SIGABRT" => libc::SIGABRT,
        "SIGFPE" => libc::SIGFPE,
        "SIGKILL" => libc::SIGKILL,
        "SIGBUS" => libc::SIGBUS,
        "SIGSEGV" => libc::SIGSEGV,
        "SIGPIPE" => libc::SIGPIPE,
        "SIGALRM" => libc::SIGALRM,
        "SIGTERM" => libc::SIGTERM,
        "SIGUSR1" => libc::SIGUSR1,
        "SIGUSR2" => libc::SIGUSR2,
        _ => return None,
    })
}

#[cfg(unix)]
fn exit_signal_name(status: &ExitStatus) -> Option<String> {
    use std::os::unix::process::ExitStatusExt;
    let num = status.signal()?;
    Some(
        match num {
            libc::SIGHUP => "SIGHUP",
            libc::SIGINT => "SIGINT",
            libc::SIGQUIT => "SIGQUIT",
            libc::SIGILL => "SIGILL",
            libc::SIGTRAP => "SIGTRAP",
            libc::SIGABRT => "SIGABRT",
            libc::SIGBUS => "SIGBUS",
            libc::SIGFPE => "SIGFPE",
            libc::SIGKILL => "SIGKILL",
            libc::SIGUSR1 => "SIGUSR1",
            libc::SIGSEGV => "SIGSEGV",
            libc::SIGUSR2 => "SIGUSR2",
            libc::SIGPIPE => "SIGPIPE",
            libc::SIGALRM => "SIGALRM",
            libc::SIGTERM => "SIGTERM",
            other => return Some(format!("SIG{other}")),
        }
        .to_string(),
    )
}

#[cfg(not(unix))]
fn exit_signal_name(_status: &ExitStatus) -> Option<String> {
    None
}

fn request_child_termination(child: &mut Child, signal: &str) {
    #[cfg(unix)]
    {

        let sig = signal_number(signal).unwrap_or(libc::SIGTERM);
        let rc = unsafe { libc::kill(child.id() as libc::pid_t, sig) };
        if rc != 0 {
            let _ = child.kill();
        }
    }
    #[cfg(not(unix))]
    {
        let _ = signal;
        let _ = child.kill();
    }
}

fn wait_after_timeout(child: &mut Child, signal: &str) -> Result<ExitStatus, String> {
    request_child_termination(child, signal);
    let grace = Instant::now() + Duration::from_millis(250);
    loop {
        if let Some(status) = child.try_wait().map_err(|e| format!("spawn: {e}"))? {
            return Ok(status);
        }
        if Instant::now() >= grace {
            let _ = child.kill();
            return child.wait().map_err(|e| format!("spawn: {e}"));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

pub fn run_sync_with_timeout(
    program: &str,
    args: &[String],
    cwd: Option<&str>,
    shell: bool,
    timeout_ms: Option<u64>,
    kill_signal: Option<&str>,
) -> Result<Outcome, String> {
    run_sync_with_options(
        program,
        args,
        cwd,
        shell,
        timeout_ms,
        kill_signal,
        None,
        None,
    )
}

pub fn run_sync_with_options(
    program: &str,
    args: &[String],
    cwd: Option<&str>,
    shell: bool,
    timeout_ms: Option<u64>,
    kill_signal: Option<&str>,
    env: Option<&[(String, String)]>,
    input: Option<&[u8]>,
) -> Result<Outcome, String> {
    let Some(timeout_ms) = timeout_ms else {
        let mut cmd = build_command(program, args, cwd, shell, env);
        if input.is_some() {
            cmd.stdin(Stdio::piped());
        }
        let mut child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("spawn: {e}"))?;
        if let Some(bytes) = input {
            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(bytes)
                    .map_err(|e| format!("spawn: stdin: {e}"))?;
            }
        }
        let out = child
            .wait_with_output()
            .map_err(|e| format!("spawn: {e}"))?;
        return Ok(Outcome {
            code: out.status.code(),
            stdout: out.stdout,
            stderr: out.stderr,
            signal: exit_signal_name(&out.status),
            timed_out: false,
        });
    };
    let mut cmd = build_command(program, args, cwd, shell, env);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    if input.is_some() {
        cmd.stdin(Stdio::piped());
    }
    let mut child = cmd.spawn().map_err(|e| format!("spawn: {e}"))?;
    if let Some(bytes) = input {
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(bytes)
                .map_err(|e| format!("spawn: stdin: {e}"))?;
        }
    }
    let stdout = child.stdout.take().map(pipe_reader);
    let stderr = child.stderr.take().map(pipe_reader);
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let signal = normalize_kill_signal(kill_signal);
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| format!("spawn: {e}"))? {
            break status;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            break wait_after_timeout(&mut child, signal)?;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let stdout = stdout.and_then(|h| h.join().ok()).unwrap_or_default();
    let stderr = stderr.and_then(|h| h.join().ok()).unwrap_or_default();
    Ok(Outcome {
        code: status.code(),
        stdout,
        stderr,
        signal: if timed_out {
            Some(signal.to_string())
        } else {
            exit_signal_name(&status)
        },
        timed_out,
    })
}

pub fn outcome_to_js(rt: &mut Runtime, o: Outcome, as_bytes: bool) -> Value {
    let obj = new_object(rt);
    let _obj_root = rt.push_temporary_value_roots(&[Value::Object(obj)]);
    rt.object_set(
        obj,
        "code".into(),
        match o.code {
            Some(c) => Value::Number(c as f64),
            None => Value::Null,
        },
    );
    let to_val = |rt: &mut Runtime, bytes: Vec<u8>| -> Value {
        if as_bytes {
            Value::Object(rt.alloc_uint8_array_from_bytes(&bytes))
        } else {
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                String::from_utf8_lossy(&bytes).into_owned(),
            )))
        }
    };
    let so = to_val(rt, o.stdout);
    rt.object_set(obj, "stdout".into(), so);
    let se = to_val(rt, o.stderr);
    rt.object_set(obj, "stderr".into(), se);
    Value::Object(obj)
}

fn spawn_error(rt: &mut Runtime, msg: &str) -> Value {
    let ctor = rt.global_get("Error");
    match rt.construct(
        ctor,
        vec![Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(msg.to_string()),
        ))],
    ) {
        Ok(v) => v,
        _ => Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            msg.to_string(),
        ))),
    }
}

pub fn parse_args(
    rt: &mut Runtime,
    args: &[Value],
) -> Result<(String, Vec<String>, Option<String>, bool, bool), RuntimeError> {
    let program = match args.first() {
        Some(Value::String(s)) => s.to_string(),
        _ => {
            return Err(RuntimeError::TypeError(
                "spawn: program must be a string".into(),
            ))
        }
    };
    let mut arglist = Vec::new();
    let mut opts: Option<rusty_js_runtime::value::ObjectRef> = None;
    match args.get(1) {
        Some(Value::Object(id)) if rt.typed_array_views.get(id).is_none() => {

            let len = rt.array_length(*id);
            if len > 0 || matches!(rt.object_get(*id, "length"), Value::Number(_)) {
                for i in 0..len {
                    if let Value::String(s) = rt.object_get(*id, &i.to_string()) {
                        arglist.push(s.to_string());
                    }
                }
                if let Some(Value::Object(o)) = args.get(2) {
                    opts = Some(*o);
                }
            } else {
                opts = Some(*id);
            }
        }
        _ => {}
    }
    let (mut cwd, mut shell, mut as_bytes) = (None, false, false);
    if let Some(o) = opts {
        if let Value::String(c) = rt.object_get(o, "cwd") {
            cwd = Some(c.to_string());
        }
        shell = matches!(rt.object_get(o, "shell"), Value::Boolean(true));
        as_bytes = matches!(rt.object_get(o, "bytes"), Value::Boolean(true));
    }
    Ok((program, arglist, cwd, shell, as_bytes))
}

#[derive(Default)]
struct ChildShared {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

struct ChildRecord {
    agent_id: AgentId,
    child: std::process::Child,
    shared: Arc<Mutex<ChildShared>>,
    readers: Vec<JoinHandle<()>>,
    stdin: Option<std::process::ChildStdin>,
    child_obj: ObjectRef,
    stdin_obj: ObjectRef,
    stdout_obj: ObjectRef,
    stderr_obj: ObjectRef,
    realm: usize,
    buffer_mode: bool,
    exec_cb: Option<Value>,
    exec_cmd: String,
    deadline: Option<Instant>,
    kill_signal: String,
    timed_out: bool,
    exit_done: bool,
    unrefed: bool,
}

#[derive(Clone, Copy)]
pub struct StdioConfig {
    pub stdin_ignore: bool,
    pub stdout_ignore: bool,
    pub stderr_ignore: bool,
}

impl StdioConfig {
    pub const PIPED: Self = Self {
        stdin_ignore: false,
        stdout_ignore: false,
        stderr_ignore: false,
    };
}

thread_local! {
    static CHILDREN: RefCell<Vec<Option<ChildRecord>>> = const { RefCell::new(Vec::new()) };
}

pub fn collect_roots(roots: &mut Vec<ObjectRef>) {
    collect_roots_for_agent(AgentId::DEFAULT, roots);
}

pub fn collect_roots_for_runtime(rt: &Runtime, roots: &mut Vec<ObjectRef>) {
    collect_roots_for_agent(rt.agent_id(), roots);
}

fn collect_roots_for_agent(agent_id: AgentId, roots: &mut Vec<ObjectRef>) {
    CHILDREN.with(|v| {
        for rec in v.borrow().iter().flatten() {
            if rec.agent_id != agent_id {
                continue;
            }
            roots.push(rec.child_obj);
            roots.push(rec.stdin_obj);
            roots.push(rec.stdout_obj);
            roots.push(rec.stderr_obj);
        }
    });
}

fn child_root_key(id: usize) -> String {
    format!("spawn:child:{id}")
}

fn retain_child_roots(rt: &mut Runtime, id: usize, rec: &ChildRecord) {
    let mut roots = vec![
        Value::Object(rec.child_obj),
        Value::Object(rec.stdin_obj),
        Value::Object(rec.stdout_obj),
        Value::Object(rec.stderr_obj),
    ];
    if let Some(cb) = &rec.exec_cb {
        roots.push(cb.clone());
    }
    rt.retain_host_roots(child_root_key(id), roots);
}

fn release_child_roots(rt: &mut Runtime, id: usize) {
    rt.release_host_roots(&child_root_key(id));
}

fn write_bytes(rt: &mut Runtime, v: Option<&Value>) -> Vec<u8> {
    match v {
        Some(Value::String(s)) => s.as_bytes().to_vec(),
        Some(Value::Object(id)) => {
            let len = rt.array_length(*id);
            let mut out = Vec::with_capacity(len);
            for i in 0..len {
                if let Value::Number(n) = rt.object_get(*id, &i.to_string()) {
                    out.push(n as u8);
                }
            }
            out
        }
        _ => Vec::new(),
    }
}

fn child_id_of(rt: &Runtime, obj: ObjectRef) -> Option<usize> {
    match rt.object_get(obj, "__child_id") {
        Value::Number(n) => Some(n as usize),
        _ => None,
    }
}

fn js_string(s: &str) -> Value {
    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(s)))
}

fn node_code_type_error(rt: &mut Runtime, code: &str, msg: &str) -> RuntimeError {
    match rusty_js_runtime::intrinsics::make_error_instance(rt, "TypeError", msg) {
        Some(id) => {
            rt.object_set(id, "code".into(), js_string(code));
            RuntimeError::Thrown(Value::Object(id))
        }
        None => RuntimeError::TypeError(msg.to_string()),
    }
}

pub fn spawn_child(
    rt: &mut Runtime,
    program: &str,
    args: &[String],
    cwd: Option<&str>,
    env: Option<&[(String, String)]>,
    shell: bool,
    buffer_mode: bool,
    exec_cb: Option<Value>,
    timeout_ms: Option<u64>,
    kill_signal: Option<&str>,
    stdio: StdioConfig,
) -> Result<ObjectRef, RuntimeError> {
    use std::process::Stdio;
    let shell_program = if shell && !args.is_empty() {
        let mut parts = Vec::with_capacity(args.len() + 1);
        parts.push(program.to_string());
        parts.extend(args.iter().cloned());
        parts.join(" ")
    } else {
        program.to_string()
    };
    let exec_cmd = if shell || args.is_empty() {
        shell_program.clone()
    } else {
        let mut parts = Vec::with_capacity(args.len() + 1);
        parts.push(program.to_string());
        parts.extend(args.iter().cloned());
        parts.join(" ")
    };
    let mut cmd = if shell {
        let (sh, flag) = if cfg!(windows) {
            ("cmd", "/C")
        } else {
            ("/bin/sh", "-c")
        };
        let mut c = Command::new(sh);
        c.arg(flag).arg(shell_program);
        c
    } else {
        let mut c = Command::new(program);
        c.args(args);
        c
    };
    if let Some(d) = cwd {
        cmd.current_dir(d);
    }
    if let Some(env) = env {
        cmd.env_clear();
        cmd.envs(env.iter().map(|(k, v)| (k, v)));
    }
    cmd.stdin(if stdio.stdin_ignore {
        Stdio::null()
    } else {
        Stdio::piped()
    })
    .stdout(if stdio.stdout_ignore {
        Stdio::null()
    } else {
        Stdio::piped()
    })
    .stderr(if stdio.stderr_ignore {
        Stdio::null()
    } else {
        Stdio::piped()
    });
    let mut child = cmd
        .spawn()
        .map_err(|e| RuntimeError::TypeError(format!("spawn: {e}")))?;
    let pid = child.id();
    let shared = Arc::new(Mutex::new(ChildShared::default()));
    let mut readers = Vec::new();
    if let Some(mut o) = child.stdout.take() {
        let sh = shared.clone();
        readers.push(std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match o.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => sh.lock().unwrap().stdout.extend_from_slice(&buf[..n]),
                }
            }
        }));
    }
    if let Some(mut e) = child.stderr.take() {
        let sh = shared.clone();
        readers.push(std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match e.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => sh.lock().unwrap().stderr.extend_from_slice(&buf[..n]),
                }
            }
        }));
    }
    let stdin = child.stdin.take();

    let realm = rt.current_realm;
    let child_obj = new_object(rt);
    install_emitter(rt, child_obj);
    rt.object_set(child_obj, "pid".into(), Value::Number(pid as f64));

    rt.object_set(child_obj, "killed".into(), Value::Boolean(false));
    rt.object_set(child_obj, "exitCode".into(), Value::Null);
    rt.object_set(child_obj, "signalCode".into(), Value::Null);

    rt.object_set(child_obj, "connected".into(), Value::Boolean(false));
    let stdout_obj = new_object(rt);
    install_emitter(rt, stdout_obj);
    crate::stream::install_async_iterator(rt, stdout_obj);
    let stderr_obj = new_object(rt);
    install_emitter(rt, stderr_obj);
    crate::stream::install_async_iterator(rt, stderr_obj);
    let stdin_obj = new_object(rt);
    install_emitter(rt, stdin_obj);
    rt.object_set(
        child_obj,
        "stdout".into(),
        if stdio.stdout_ignore {
            Value::Null
        } else {
            Value::Object(stdout_obj)
        },
    );
    rt.object_set(
        child_obj,
        "stderr".into(),
        if stdio.stderr_ignore {
            Value::Null
        } else {
            Value::Object(stderr_obj)
        },
    );
    rt.object_set(
        child_obj,
        "stdin".into(),
        if stdio.stdin_ignore {
            Value::Null
        } else {
            Value::Object(stdin_obj)
        },
    );

    let stdio_arr = rt.alloc_object(rusty_js_runtime::Object::new_array());
    rt.object_set(
        stdio_arr,
        "0".into(),
        if stdio.stdin_ignore {
            Value::Null
        } else {
            Value::Object(stdin_obj)
        },
    );
    rt.object_set(
        stdio_arr,
        "1".into(),
        if stdio.stdout_ignore {
            Value::Null
        } else {
            Value::Object(stdout_obj)
        },
    );
    rt.object_set(
        stdio_arr,
        "2".into(),
        if stdio.stderr_ignore {
            Value::Null
        } else {
            Value::Object(stderr_obj)
        },
    );
    rt.object_set(stdio_arr, "length".into(), Value::Number(3.0));
    rt.object_set(child_obj, "stdio".into(), Value::Object(stdio_arr));

    let id = CHILDREN.with(|v| {
        let mut v = v.borrow_mut();
        v.push(Some(ChildRecord {
            agent_id: rt.agent_id(),
            child,
            shared,
            readers,
            stdin,
            child_obj,
            stdin_obj,
            stdout_obj,
            stderr_obj,
            realm,
            buffer_mode,
            exec_cb,
            exec_cmd,
            deadline: timeout_ms.map(|ms| Instant::now() + Duration::from_millis(ms)),
            kill_signal: normalize_kill_signal(kill_signal).to_string(),
            timed_out: false,
            exit_done: false,
            unrefed: false,
        }));
        v.len() - 1
    });
    CHILDREN.with(|v| {
        if let Some(Some(rec)) = v.borrow().get(id) {
            retain_child_roots(rt, id, rec);
        }
    });
    rt.set_engine_sentinel(child_obj, "__child_id", Value::Number(id as f64));
    rt.set_engine_sentinel(stdin_obj, "__child_id", Value::Number(id as f64));

    register_method(rt, child_obj, "kill", |rt, args| {

        let signal = match args.first() {
            Some(Value::String(s)) => {
                #[cfg(unix)]
                if signal_number(s.as_str()).is_none() {
                    return Err(node_code_type_error(
                        rt,
                        "ERR_UNKNOWN_SIGNAL",
                        "Unknown signal",
                    ));
                }
                s.as_str().to_string()
            }
            _ => "SIGTERM".to_string(),
        };
        if let Value::Object(this) = rt.current_this() {

            rt.object_set(this, "killed".into(), Value::Boolean(true));
            if let Some(id) = child_id_of(rt, this) {
                CHILDREN.with(|v| {
                    if let Some(Some(rec)) = v.borrow_mut().get_mut(id) {
                        if rec.agent_id != rt.agent_id() {
                            return;
                        }

                        #[cfg(unix)]
                        {
                            match signal_number(&signal) {
                                Some(sig) => {
                                    let rc =
                                        unsafe { libc::kill(rec.child.id() as libc::pid_t, sig) };
                                    if rc != 0 {
                                        let _ = rec.child.kill();
                                    }
                                }
                                None => {
                                    let _ = rec.child.kill();
                                }
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = &signal;
                            let _ = rec.child.kill();
                        }
                    }
                });
            }
        }
        Ok(Value::Boolean(true))
    });
    register_method(rt, child_obj, "ref", |rt, _a| {
        if let Value::Object(this) = rt.current_this() {
            if let Some(id) = child_id_of(rt, this) {
                CHILDREN.with(|v| {
                    if let Some(Some(rec)) = v.borrow_mut().get_mut(id) {
                        if rec.agent_id != rt.agent_id() {
                            return;
                        }
                        rec.unrefed = false;
                    }
                });
            }
        }
        Ok(rt.current_this())
    });
    register_method(rt, child_obj, "unref", |rt, _a| {
        if let Value::Object(this) = rt.current_this() {
            if let Some(id) = child_id_of(rt, this) {
                CHILDREN.with(|v| {
                    if let Some(Some(rec)) = v.borrow_mut().get_mut(id) {
                        if rec.agent_id != rt.agent_id() {
                            return;
                        }
                        rec.unrefed = true;
                    }
                });
            }
        }
        Ok(rt.current_this())
    });
    for noop in ["setEncoding", "resume", "pause"] {
        register_method(rt, stdout_obj, noop, |rt, _a| Ok(rt.current_this()));
        register_method(rt, stderr_obj, noop, |rt, _a| Ok(rt.current_this()));
    }
    register_method(rt, stdin_obj, "write", |rt, args| {
        if let Value::Object(this) = rt.current_this() {
            if let Some(id) = child_id_of(rt, this) {
                let bytes = write_bytes(rt, args.first());
                CHILDREN.with(|v| {
                    if let Some(Some(rec)) = v.borrow_mut().get_mut(id) {
                        if rec.agent_id != rt.agent_id() {
                            return;
                        }
                        if let Some(si) = rec.stdin.as_mut() {
                            let _ = si.write_all(&bytes);
                        }
                    }
                });
            }
        }
        Ok(Value::Boolean(true))
    });
    register_method(rt, stdin_obj, "end", |rt, args| {
        if let Value::Object(this) = rt.current_this() {
            if let Some(id) = child_id_of(rt, this) {
                let bytes = write_bytes(rt, args.first());
                CHILDREN.with(|v| {
                    if let Some(Some(rec)) = v.borrow_mut().get_mut(id) {
                        if rec.agent_id != rt.agent_id() {
                            return;
                        }
                        if let Some(mut si) = rec.stdin.take() {
                            let _ = si.write_all(&bytes);
                        }
                    }
                });
            }
        }
        Ok(rt.current_this())
    });

    rt.enqueue_host_phase_rooted(
        HostEnqueuePhase::HostCompletionMacrotask,
        "cruft:child.spawn",
        vec![child_obj],
        move |rt| {
            let prior = rt.enter_realm(realm);
            net_emit(rt, child_obj, "spawn", Vec::new());
            rt.exit_realm(prior);
            Ok(())
        },
    );
    Ok(child_obj)
}

pub fn harvest(rt: &mut Runtime) -> Result<bool, RuntimeError> {
    enum Act {
        Data(ObjectRef, Vec<u8>, usize),
        Exit(
            usize,
            ObjectRef,
            ObjectRef,
            ObjectRef,
            ObjectRef,
            Vec<u8>,
            Vec<u8>,
            Option<i32>,
            Option<String>,
            usize,
        ),
        Exec(
            usize,
            Option<Value>,
            ObjectRef,
            ObjectRef,
            Vec<u8>,
            Vec<u8>,
            Option<i32>,
            String,
            bool,
            String,
            usize,
        ),
    }
    let act = CHILDREN.with(|v| -> Option<Act> {
        let mut children = v.borrow_mut();
        let agent_id = rt.agent_id();
        for i in 0..children.len() {
            let rec = match children[i].as_mut() {
                Some(r) if !r.exit_done && r.agent_id == agent_id => r,
                _ => continue,
            };
            if !rec.buffer_mode {

                let out = {
                    let mut s = rec.shared.lock().unwrap();
                    std::mem::take(&mut s.stdout)
                };
                if !out.is_empty() {
                    return Some(Act::Data(rec.stdout_obj, out, rec.realm));
                }
                let err = {
                    let mut s = rec.shared.lock().unwrap();
                    std::mem::take(&mut s.stderr)
                };
                if !err.is_empty() {
                    return Some(Act::Data(rec.stderr_obj, err, rec.realm));
                }
            }
            if !rec.timed_out
                && rec
                    .deadline
                    .map(|deadline| Instant::now() >= deadline)
                    .unwrap_or(false)
            {
                rec.timed_out = true;
                #[cfg(unix)]
                {
                    if let Some(sig) = signal_number(&rec.kill_signal) {
                        let rc = unsafe { libc::kill(rec.child.id() as libc::pid_t, sig) };
                        if rc != 0 {
                            let _ = rec.child.kill();
                        }
                    } else {
                        let _ = rec.child.kill();
                    }
                }
                #[cfg(not(unix))]
                {
                    let _ = rec.child.kill();
                }
            }
            if let Ok(Some(status)) = rec.child.try_wait() {
                rec.exit_done = true;
                for h in rec.readers.drain(..) {
                    let _ = h.join();
                }
                let code = status.code();
                if rec.buffer_mode {
                    let (out, err) = {
                        let s = rec.shared.lock().unwrap();
                        (s.stdout.clone(), s.stderr.clone())
                    };
                    return Some(Act::Exec(
                        i,
                        rec.exec_cb.take(),
                        rec.stdout_obj,
                        rec.stderr_obj,
                        out,
                        err,
                        code,
                        rec.exec_cmd.clone(),
                        rec.timed_out,
                        rec.kill_signal.clone(),
                        rec.realm,
                    ));
                }
                let (out, err) = {
                    let mut s = rec.shared.lock().unwrap();
                    (std::mem::take(&mut s.stdout), std::mem::take(&mut s.stderr))
                };
                let signal = exit_signal_name(&status);
                return Some(Act::Exit(
                    i,
                    rec.child_obj,
                    rec.stdin_obj,
                    rec.stdout_obj,
                    rec.stderr_obj,
                    out,
                    err,
                    code,
                    signal,
                    rec.realm,
                ));
            }
        }
        None
    });
    let remove = |rt: &mut Runtime, idx: usize| {
        release_child_roots(rt, idx);
        CHILDREN.with(|v| {
            if let Some(s) = v.borrow_mut().get_mut(idx) {
                *s = None;
            }
        });
    };
    match act {
        Some(Act::Data(obj, bytes, realm)) => {
            let prior = rt.enter_realm(realm);
            let buf = net_buffer_from_bytes(rt, &bytes);
            net_emit(rt, obj, "data", vec![buf]);
            rt.exit_realm(prior);
            Ok(true)
        }
        Some(Act::Exit(
            idx,
            child_obj,
            stdin_obj,
            stdout_obj,
            stderr_obj,
            out,
            err,
            code,
            signal,
            realm,
        )) => {
            let prior = rt.enter_realm(realm);
            if !out.is_empty() {
                let b = net_buffer_from_bytes(rt, &out);
                net_emit(rt, stdout_obj, "data", vec![b]);
            }
            if !err.is_empty() {
                let b = net_buffer_from_bytes(rt, &err);
                net_emit(rt, stderr_obj, "data", vec![b]);
            }
            net_emit(rt, stdout_obj, "end", Vec::new());
            net_emit(rt, stderr_obj, "end", Vec::new());
            net_emit(rt, stdin_obj, "close", Vec::new());
            net_emit(rt, stdin_obj, "finish", Vec::new());

            let code_v = match code {
                Some(c) => Value::Number(c as f64),
                None => Value::Null,
            };
            let signal_v = match &signal {
                Some(s) => {
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(s.clone())))
                }
                None => Value::Null,
            };

            rt.object_set(child_obj, "exitCode".into(), code_v.clone());
            rt.object_set(child_obj, "signalCode".into(), signal_v.clone());
            net_emit(
                rt,
                child_obj,
                "exit",
                vec![code_v.clone(), signal_v.clone()],
            );
            net_emit(rt, child_obj, "close", vec![code_v, signal_v]);
            rt.exit_realm(prior);
            remove(rt, idx);
            Ok(true)
        }
        Some(Act::Exec(
            idx,
            cb,
            stdout_obj,
            stderr_obj,
            out,
            err,
            code,
            cmd,
            timed_out,
            kill_signal,
            realm,
        )) => {
            let prior = rt.enter_realm(realm);
            if !out.is_empty() {
                net_emit(
                    rt,
                    stdout_obj,
                    "data",
                    vec![Value::String(Rc::new(
                        rusty_js_runtime::value::JsString::from(
                            String::from_utf8_lossy(&out).into_owned(),
                        ),
                    ))],
                );
            }
            if !err.is_empty() {
                net_emit(
                    rt,
                    stderr_obj,
                    "data",
                    vec![Value::String(Rc::new(
                        rusty_js_runtime::value::JsString::from(
                            String::from_utf8_lossy(&err).into_owned(),
                        ),
                    ))],
                );
            }
            if let Some(cb) = cb {
                let stdout = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    String::from_utf8_lossy(&out).into_owned(),
                )));
                let stderr = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    String::from_utf8_lossy(&err).into_owned(),
                )));
                let err_arg = if code == Some(0) && !timed_out {
                    Value::Null
                } else {
                    let msg = format!("Command failed: {cmd}");
                    let e = spawn_error(rt, &msg);

                    if let Value::Object(eid) = &e {
                        rt.object_set(
                            *eid,
                            "code".into(),
                            if timed_out {
                                Value::Null
                            } else {
                                match code {
                                    Some(c) => Value::Number(c as f64),
                                    None => Value::Null,
                                }
                            },
                        );
                        rt.object_set(*eid, "killed".into(), Value::Boolean(timed_out));
                        rt.object_set(
                            *eid,
                            "signal".into(),
                            if timed_out {
                                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                                    kill_signal.clone(),
                                )))
                            } else {
                                Value::Null
                            },
                        );
                        rt.object_set(
                            *eid,
                            "cmd".into(),
                            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                                cmd.clone(),
                            ))),
                        );
                    }
                    e
                };
                let _ = rt.call_function(cb, Value::Undefined, vec![err_arg, stdout, stderr]);
            }
            rt.exit_realm(prior);
            remove(rt, idx);
            Ok(true)
        }
        None => Ok(false),
    }
}

pub fn has_open() -> bool {
    has_open_for_agent(AgentId::DEFAULT)
}

pub fn has_open_for_runtime(rt: &Runtime) -> bool {
    has_open_for_agent(rt.agent_id())
}

fn has_open_for_agent(agent_id: AgentId) -> bool {
    CHILDREN.with(|v| {
        v.borrow()
            .iter()
            .any(|s| matches!(s, Some(rec) if rec.agent_id == agent_id && !rec.unrefed))
    })
}

pub fn poll_io(rt: &mut Runtime) -> Result<bool, RuntimeError> {
    if harvest(rt)? {
        return Ok(true);
    }
    if has_open_for_runtime(rt) {
        std::thread::sleep(std::time::Duration::from_millis(2));
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn install(rt: &mut Runtime) {
    let ns = new_object(rt);

    register_method(rt, ns, "run", |rt, args| {
        let (program, arglist, cwd, shell, as_bytes) = parse_args(rt, args)?;
        require_spawn(rt, &program)?;
        let outcome =
            run_sync(&program, &arglist, cwd.as_deref(), shell).map_err(RuntimeError::TypeError)?;
        Ok(outcome_to_js(rt, outcome, as_bytes))
    });

    register_method(rt, ns, "exec", |rt, args| {
        let (program, arglist, cwd, shell, as_bytes) = parse_args(rt, args)?;
        require_spawn(rt, &program)?;
        let p = new_promise(rt);
        rt.enqueue_host_phase_rooted(
            HostEnqueuePhase::HostCompletionMacrotask,
            "cruft:spawn.exec",
            vec![p],
            move |rt| {
                match run_sync(&program, &arglist, cwd.as_deref(), shell) {
                    Ok(o) => {
                        let v = outcome_to_js(rt, o, as_bytes);
                        resolve_promise(rt, p, v);
                    }
                    Err(e) => {
                        let err = spawn_error(rt, &e);
                        reject_promise(rt, p, err);
                    }
                }
                Ok(())
            },
        );
        Ok(Value::Object(p))
    });

    rt.define_global_property("__cruft_spawn", Value::Object(ns));
}

#[cfg(test)]
mod tests {
    use super::{has_open_for_runtime, poll_io, spawn_child, StdioConfig};
    use rusty_js_runtime::{AgentId, Runtime};

    #[test]
    #[cfg(unix)]
    fn async_child_registry_is_scoped_by_runtime_agent_id() {
        let mut rt_a = Runtime::new_with_agent_id(AgentId::from_raw(1401));
        let mut rt_b = Runtime::new_with_agent_id(AgentId::from_raw(1402));
        let args = vec!["-c".to_string(), "sleep 0.05".to_string()];

        let _child = spawn_child(
            &mut rt_b,
            "/bin/sh",
            &args,
            None,
            None,
            false,
            false,
            None,
            None,
            None,
            StdioConfig {
                stdin_ignore: true,
                stdout_ignore: true,
                stderr_ignore: true,
            },
        )
        .expect("spawn child for agent B");

        assert!(
            !has_open_for_runtime(&rt_a),
            "agent A must not observe agent B's child-process liveness"
        );
        assert!(
            has_open_for_runtime(&rt_b),
            "agent B must observe its own live child-process record"
        );
        assert!(
            !poll_io(&mut rt_a).expect("poll agent A"),
            "agent A must not keep its loop alive from agent B's child"
        );

        for _ in 0..100 {
            if !has_open_for_runtime(&rt_b) {
                break;
            }
            let _ = poll_io(&mut rt_b).expect("poll agent B");
        }
        assert!(
            !has_open_for_runtime(&rt_b),
            "agent B child should be harvested by its owner runtime"
        );
    }
}
