
use crate::register::{make_callable_rooted, new_object, register_method};
use rusty_js_runtime::promise::{new_promise, reject_promise, resolve_promise};
use rusty_js_runtime::value::{JsString, ObjectRef};
use rusty_js_runtime::{HostEnqueuePhase, Object, Runtime, RuntimeError, Value};
use std::rc::Rc;

fn attach_exec_promisify(rt: &mut Runtime, ns: ObjectRef, name: &str, shell: bool) {
    let fn_obj = match rt.object_get(ns, name) {
        Value::Object(id) => id,
        _ => return,
    };
    let custom = make_callable_rooted(rt, "execPromisified", vec![], move |rt, args| {
        let program = match args.first() {
            Some(Value::String(s)) => s.to_string(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "exec: command must be a string".into(),
                ))
            }
        };

        let mut arglist: Vec<String> = Vec::new();
        let mut opts_idx = 1;
        if !shell {
            if let Some(Value::Object(arr)) = args.get(1) {
                if matches!(rt.object_get(*arr, "length"), Value::Number(_)) {
                    let len = rt.array_length(*arr);
                    for i in 0..len {
                        if let Value::String(s) = rt.object_get(*arr, &i.to_string()) {
                            arglist.push(s.to_string());
                        }
                    }
                    opts_idx = 2;
                }
            }
        }
        let opts = args.get(opts_idx);
        let cwd = read_cwd(rt, opts);

        let env = read_env(rt, opts);
        let signal = signal_option(rt, opts)?;
        let promise = new_promise(rt);
        if let Some(sig) = signal {
            if matches!(rt.object_get(sig, "aborted"), Value::Boolean(true)) {
                let cause = rt.object_get(sig, "reason");
                let err = abort_error_value(rt, cause);
                reject_promise(rt, promise, err);
                return Ok(Value::Object(promise));
            }
        }
        let cb = make_callable_rooted(rt, "execPromiseCb", vec![promise], move |rt, cb_args| {
            let err = cb_args.first().cloned().unwrap_or(Value::Null);
            let stdout = cb_args.get(1).cloned().unwrap_or(Value::Undefined);
            let stderr = cb_args.get(2).cloned().unwrap_or(Value::Undefined);
            if !matches!(err, Value::Null | Value::Undefined | Value::Boolean(false)) {
                if let Value::Object(eid) = &err {
                    rt.object_set(*eid, "stdout".into(), stdout);
                    rt.object_set(*eid, "stderr".into(), stderr);
                }
                reject_promise(rt, promise, err);
            } else {
                let obj = new_object(rt);
                rt.object_set(obj, "stdout".into(), stdout);
                rt.object_set(obj, "stderr".into(), stderr);
                resolve_promise(rt, promise, Value::Object(obj));
            }
            Ok(Value::Undefined)
        });
        let env_ref: Option<&[(String, String)]> = if env.is_empty() {
            None
        } else {
            Some(env.as_slice())
        };
        let child = crate::spawn::spawn_child(
            rt,
            &program,
            &arglist,
            cwd.as_deref(),
            env_ref,
            shell,
            true,
            Some(Value::Object(cb)),
            None,
            None,
            crate::spawn::StdioConfig::PIPED,
        )?;
        if let Some(sig) = signal {
            let listener = make_callable_rooted(
                rt,
                "execPromiseAbort",
                vec![promise, child, sig],
                move |rt, _| {
                    let cause = rt.object_get(sig, "reason");
                    let err = abort_error_value(rt, cause);
                    reject_promise(rt, promise, err);
                    let kill = rt.object_get(child, "kill");
                    if rt.is_callable(&kill) {
                        let _ = rt.call_function(kill, Value::Object(child), Vec::new());
                    }
                    Ok(Value::Undefined)
                },
            );
            let add = rt.object_get(sig, "addEventListener");
            let _ = rt.call_function(
                add,
                Value::Object(sig),
                vec![js_string("abort"), Value::Object(listener)],
            );
        }
        Ok(Value::Object(promise))
    });
    rt.set_engine_sentinel(
        fn_obj,
        "@@sym:nodejs.util.promisify.custom",
        Value::Object(custom),
    );
}

fn read_cwd(rt: &Runtime, opts: Option<&Value>) -> Option<String> {
    if let Some(Value::Object(o)) = opts {
        if let Value::String(c) = rt.object_get(*o, "cwd") {
            return Some(c.to_string());
        }
    }
    None
}

fn read_env(rt: &mut Runtime, opts: Option<&Value>) -> Vec<(String, String)> {
    let Some(Value::Object(opts_obj)) = opts else {
        return Vec::new();
    };
    let Value::Object(env_obj) = rt.object_get(*opts_obj, "env") else {
        return Vec::new();
    };
    let Ok(Value::Object(names)) = rt.own_property_names_via(&Value::Object(env_obj)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let len = rt.array_length(names);
    for i in 0..len {
        let Value::String(name) = rt.object_get(names, &i.to_string()) else {
            continue;
        };
        let name = name.as_str().to_string();
        if name == "length" {
            continue;
        }
        let value = rt.object_get(env_obj, &name);
        if matches!(value, Value::Undefined | Value::Null) {
            continue;
        }
        let value = match &value {
            Value::String(s) => s.as_str().to_string(),
            other => rusty_js_runtime::abstract_ops::to_string(other)
                .as_ref()
                .clone(),
        };
        out.push((name, value));
    }
    out
}

fn read_input_bytes(rt: &mut Runtime, opts: Option<&Value>) -> Option<Vec<u8>> {
    let Some(Value::Object(opts_obj)) = opts else {
        return None;
    };
    match rt.object_get(*opts_obj, "input") {
        Value::Undefined | Value::Null => None,
        Value::String(s) => Some(s.as_bytes().to_vec()),
        Value::Object(id) => {
            let len = rt.array_length(id);
            let mut out = Vec::with_capacity(len);
            for i in 0..len {
                if let Value::Number(n) = rt.object_get(id, &i.to_string()) {
                    out.push(n as u8);
                }
            }
            Some(out)
        }
        other => Some(
            rusty_js_runtime::abstract_ops::to_string(&other)
                .as_ref()
                .as_bytes()
                .to_vec(),
        ),
    }
}

const DEFAULT_MAX_BUFFER: usize = 1024 * 1024;
const NODE_ENOBUFS_ERRNO: i32 = -105;
const NODE_SHAPE_BUFFER_MAX: usize = 64 * 1024;

fn read_max_buffer(rt: &Runtime, opts: Option<&Value>) -> Option<usize> {
    let Some(Value::Object(opts_obj)) = opts else {
        return Some(DEFAULT_MAX_BUFFER);
    };
    match rt.object_get(*opts_obj, "maxBuffer") {
        Value::Undefined => Some(DEFAULT_MAX_BUFFER),
        Value::Number(n) if n.is_infinite() && n.is_sign_positive() => None,
        Value::Number(n) if n.is_finite() && n >= 0.0 => Some(n as usize),
        _ => Some(DEFAULT_MAX_BUFFER),
    }
}

fn sync_output_exceeds_max_buffer(o: &crate::spawn::Outcome, max_buffer: Option<usize>) -> bool {
    let Some(max) = max_buffer else {
        return false;
    };
    o.stdout.len() > max || o.stderr.len() > max
}

fn node_buffer_from_bytes(rt: &mut Runtime, bytes: &[u8]) -> Value {
    if bytes.len() > NODE_SHAPE_BUFFER_MAX {
        return crate::net::net_buffer_from_bytes(rt, bytes);
    }
    let buffer_ctor = rt.global_get("Buffer");
    if let Value::Object(buffer_ctor_id) = buffer_ctor.clone() {
        let from = rt.object_get(buffer_ctor_id, "from");
        if rt.is_callable(&from) {
            let arr = rt.alloc_object(Object::new_array());
            let _roots = rt.push_temporary_value_roots(&[Value::Object(arr), buffer_ctor.clone()]);
            for (i, b) in bytes.iter().enumerate() {
                rt.object_set(arr, i.to_string(), Value::Number(*b as f64));
            }
            rt.object_set(arr, "length".into(), Value::Number(bytes.len() as f64));
            if let Ok(value) = rt.call_function(from, buffer_ctor, vec![Value::Object(arr)]) {
                return value;
            }
        }
    }
    crate::net::net_buffer_from_bytes(rt, bytes)
}

fn enobufs_error_value(rt: &mut Runtime, cmd: &str) -> Value {
    let msg = format!("spawnSync {cmd} ENOBUFS");
    match rusty_js_runtime::intrinsics::make_error_instance(rt, "Error", &msg) {
        Some(id) => {
            rt.object_set(id, "code".into(), js_string("ENOBUFS"));
            rt.object_set(id, "errno".into(), Value::Number(NODE_ENOBUFS_ERRNO as f64));
            rt.object_set(id, "syscall".into(), js_string(&format!("spawnSync {cmd}")));
            rt.object_set(id, "path".into(), js_string(cmd));
            Value::Object(id)
        }
        None => js_string(&msg),
    }
}

fn child_process_enobufs_error(
    rt: &mut Runtime,
    cmd: &str,
    stdout: &[u8],
    stderr: &[u8],
) -> RuntimeError {

    match enobufs_error_value(rt, cmd) {
        Value::Object(id) => {
            let _root = rt.push_temporary_value_roots(&[Value::Object(id)]);
            let stdout = node_buffer_from_bytes(rt, stdout);
            let stderr = node_buffer_from_bytes(rt, stderr);
            rt.object_set(id, "stdout".into(), stdout);
            rt.object_set(id, "stderr".into(), stderr);
            RuntimeError::Thrown(Value::Object(id))
        }
        _ => RuntimeError::TypeError(format!("spawnSync {cmd} ENOBUFS")),
    }
}

fn exec_sync_failure_error(
    rt: &mut Runtime,
    command: &str,
    code: i32,
    signal: Option<&str>,
    stdout: &[u8],
    stderr: &[u8],
) -> RuntimeError {
    let msg = format!("Command failed: {command}");
    match rusty_js_runtime::intrinsics::make_error_instance(rt, "Error", &msg) {
        Some(id) => {
            let _root = rt.push_temporary_value_roots(&[Value::Object(id)]);
            match signal {
                Some(sig) => {
                    rt.object_set(id, "status".into(), Value::Null);
                    rt.object_set(id, "signal".into(), js_string(sig.to_string()));
                }
                None => {
                    rt.object_set(id, "status".into(), Value::Number(code as f64));
                    rt.object_set(id, "signal".into(), Value::Null);
                }
            }
            let out = node_buffer_from_bytes(rt, stdout);
            let err = node_buffer_from_bytes(rt, stderr);
            rt.object_set(id, "stdout".into(), out);
            rt.object_set(id, "stderr".into(), err);
            RuntimeError::Thrown(Value::Object(id))
        }
        None => RuntimeError::TypeError(msg),
    }
}

fn sync_stdout_value(rt: &mut Runtime, stdout: &[u8], opts: Option<&Value>) -> Value {
    let encoding = match opts {
        Some(Value::Object(opts_obj)) => rt.object_get(*opts_obj, "encoding"),
        _ => Value::Undefined,
    };
    match encoding {
        Value::String(s)
            if s.as_str().eq_ignore_ascii_case("utf8")
                || s.as_str().eq_ignore_ascii_case("utf-8") =>
        {
            js_string(String::from_utf8_lossy(stdout).into_owned())
        }
        _ => node_buffer_from_bytes(rt, stdout),
    }
}

fn emit_execfile_shell_args_warning(rt: &mut Runtime) {
    let process = rt.global_get("process");
    let Value::Object(process_id) = process else {
        return;
    };
    if matches!(
        rt.object_get(process_id, "__execfile_shell_args_dep0190"),
        Value::Boolean(true)
    ) {
        return;
    }
    rt.set_engine_sentinel(
        process_id,
        "__execfile_shell_args_dep0190",
        Value::Boolean(true),
    );
    let emit_warning = rt.object_get(process_id, "emitWarning");
    if !rt.is_callable(&emit_warning) {
        return;
    }
    let opts = new_object(rt);
    rt.object_set(opts, "type".into(), js_string("DeprecationWarning"));
    rt.object_set(opts, "code".into(), js_string("DEP0190"));
    let msg = "Passing args to a child process with shell option true can lead to security \
vulnerabilities, as the arguments are not escaped, only concatenated.";
    let _ = rt.call_function(
        emit_warning,
        Value::Object(process_id),
        vec![js_string(msg), Value::Object(opts)],
    );
}

fn js_string(s: impl Into<String>) -> Value {
    Value::String(Rc::new(JsString::from(s.into())))
}

fn invalid_arg_suffix(rt: &Runtime, value: &Value) -> String {
    match value {
        Value::Undefined => " Received undefined".to_string(),
        Value::Null => " Received null".to_string(),
        Value::String(s) => format!(" Received type string ('{}')", s.as_str()),
        Value::Number(n) if n.is_nan() => " Received type number (NaN)".to_string(),
        Value::Number(n) => format!(" Received type number ({})", n),
        Value::Boolean(b) => format!(" Received type boolean ({})", b),
        Value::Object(id) => {
            let ctor = match rt.object_get(*id, "constructor") {
                Value::Object(c) => match rt.object_get(c, "name") {
                    Value::String(name) if !name.as_str().is_empty() => name.as_str().to_string(),
                    _ => "Object".to_string(),
                },
                _ => "Object".to_string(),
            };
            format!(" Received an instance of {ctor}")
        }
        _ => " Received an invalid value".to_string(),
    }
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

fn node_error_value(rt: &mut Runtime, name: &str, code: &str, msg: &str, cmd: &str) -> Value {
    match rusty_js_runtime::intrinsics::make_error_instance(rt, name, msg) {
        Some(id) => {
            rt.object_set(id, "code".into(), js_string(code));
            rt.object_set(id, "cmd".into(), js_string(cmd));
            Value::Object(id)
        }
        None => js_string(msg),
    }
}

fn abort_error_value(rt: &mut Runtime, cause: Value) -> Value {
    match rusty_js_runtime::intrinsics::make_error_instance(
        rt,
        "Error",
        "The operation was aborted",
    ) {
        Some(id) => {
            rt.object_set(id, "name".into(), js_string("AbortError"));
            rt.object_set(id, "code".into(), js_string("ABORT_ERR"));
            rt.object_set(id, "cause".into(), cause);
            Value::Object(id)
        }
        None => js_string("The operation was aborted"),
    }
}

fn signal_option(
    rt: &mut Runtime,
    opts: Option<&Value>,
) -> Result<Option<ObjectRef>, RuntimeError> {
    let Some(Value::Object(opts_obj)) = opts else {
        return Ok(None);
    };
    let signal = rt.object_get(*opts_obj, "signal");
    if matches!(signal, Value::Undefined) {
        return Ok(None);
    }
    let Value::Object(sig) = signal else {
        return Err(node_code_type_error(
            rt,
            "ERR_INVALID_ARG_TYPE",
            "The \"options.signal\" property must be an AbortSignal",
        ));
    };
    if !matches!(rt.object_get(sig, "aborted"), Value::Boolean(_)) {
        return Err(node_code_type_error(
            rt,
            "ERR_INVALID_ARG_TYPE",
            "The \"options.signal\" property must be an AbortSignal",
        ));
    }
    if !rt.is_callable(&rt.object_get(sig, "addEventListener")) {
        return Err(node_code_type_error(
            rt,
            "ERR_INVALID_ARG_TYPE",
            "The \"options.signal\" property must be an AbortSignal",
        ));
    }
    Ok(Some(sig))
}

fn is_spawn_enoent(err: &RuntimeError) -> bool {
    match err {
        RuntimeError::TypeError(msg) => {
            msg.contains("No such file") || msg.contains("os error 2") || msg.contains("ENOENT")
        }
        _ => false,
    }
}

fn failed_execfile_child(rt: &mut Runtime, file: &str, cb: Option<Value>) -> ObjectRef {
    let child = new_object(rt);
    crate::net::install_emitter(rt, child);

    let stdin = new_object(rt);
    let stdout = new_object(rt);
    let stderr = new_object(rt);
    crate::net::install_emitter(rt, stdin);
    crate::net::install_emitter(rt, stdout);
    crate::net::install_emitter(rt, stderr);

    let stdio = rt.alloc_object(Object::new_array());
    rt.object_set(stdio, "0".into(), Value::Object(stdin));
    rt.object_set(stdio, "1".into(), Value::Object(stdout));
    rt.object_set(stdio, "2".into(), Value::Object(stderr));
    rt.object_set(stdio, "length".into(), Value::Number(3.0));

    rt.object_set(child, "stdin".into(), Value::Object(stdin));
    rt.object_set(child, "stdout".into(), Value::Object(stdout));
    rt.object_set(child, "stderr".into(), Value::Object(stderr));
    rt.object_set(child, "stdio".into(), Value::Object(stdio));

    if let Some(cb_v) = cb.filter(|v| rt.is_callable(v)) {
        let mut roots = vec![child, stdin, stdout, stderr, stdio];
        if let Value::Object(cb_obj) = cb_v {
            roots.push(cb_obj);
        }
        let file = file.to_string();
        rt.enqueue_host_phase_rooted(
            HostEnqueuePhase::HostCompletionMacrotask,
            "node:child_process.execFile.enoent",
            roots,
            move |rt| {
                let msg = format!("spawn {file} ENOENT");
                let err = node_error_value(rt, "Error", "ENOENT", &msg, &file);
                let empty = js_string("");
                let _ = rt.call_function(
                    cb_v.clone(),
                    Value::Undefined,
                    vec![err, empty.clone(), empty],
                );
                Ok(())
            },
        );
    }

    child
}

fn failed_spawn_child(rt: &mut Runtime, command: &str, code: &str, errno: i32) -> ObjectRef {
    let child = new_object(rt);
    crate::net::install_emitter(rt, child);
    let stdin = new_object(rt);
    let stdout = new_object(rt);
    let stderr = new_object(rt);
    crate::net::install_emitter(rt, stdin);
    crate::net::install_emitter(rt, stdout);
    crate::net::install_emitter(rt, stderr);
    let stdio = rt.alloc_object(Object::new_array());
    rt.object_set(stdio, "0".into(), Value::Object(stdin));
    rt.object_set(stdio, "1".into(), Value::Object(stdout));
    rt.object_set(stdio, "2".into(), Value::Object(stderr));
    rt.object_set(stdio, "length".into(), Value::Number(3.0));
    rt.object_set(child, "stdin".into(), Value::Object(stdin));
    rt.object_set(child, "stdout".into(), Value::Object(stdout));
    rt.object_set(child, "stderr".into(), Value::Object(stderr));
    rt.object_set(child, "stdio".into(), Value::Object(stdio));
    let command = command.to_string();
    let code = code.to_string();
    rt.enqueue_host_phase_rooted(
        HostEnqueuePhase::HostCompletionMacrotask,
        "node:child_process.spawn.error",
        vec![child, stdin, stdout, stderr, stdio],
        move |rt| {
            let syscall = format!("spawn {command}");
            let msg = format!("{syscall} {code}");
            let err = node_error_value(rt, "Error", &code, &msg, &command);
            if let Value::Object(eid) = &err {
                rt.object_set(*eid, "errno".into(), Value::Number(errno as f64));
                rt.object_set(*eid, "syscall".into(), js_string(&syscall));
                rt.object_set(*eid, "path".into(), js_string(&command));
            }
            crate::net::net_emit(rt, child, "error", vec![err]);
            Ok(())
        },
    );
    child
}

fn failed_execfile_abort_child(rt: &mut Runtime, cb: Option<Value>, cause: Value) -> ObjectRef {
    let child = new_object(rt);
    crate::net::install_emitter(rt, child);

    let stdin = new_object(rt);
    let stdout = new_object(rt);
    let stderr = new_object(rt);
    crate::net::install_emitter(rt, stdin);
    crate::net::install_emitter(rt, stdout);
    crate::net::install_emitter(rt, stderr);

    let stdio = rt.alloc_object(Object::new_array());
    rt.object_set(stdio, "0".into(), Value::Object(stdin));
    rt.object_set(stdio, "1".into(), Value::Object(stdout));
    rt.object_set(stdio, "2".into(), Value::Object(stderr));
    rt.object_set(stdio, "length".into(), Value::Number(3.0));

    rt.object_set(child, "stdin".into(), Value::Object(stdin));
    rt.object_set(child, "stdout".into(), Value::Object(stdout));
    rt.object_set(child, "stderr".into(), Value::Object(stderr));
    rt.object_set(child, "stdio".into(), Value::Object(stdio));

    if let Some(cb_v) = cb.filter(|v| rt.is_callable(v)) {
        let mut roots = vec![child, stdin, stdout, stderr, stdio];
        if let Value::Object(cb_obj) = cb_v {
            roots.push(cb_obj);
        }
        if let Value::Object(cause_obj) = cause {
            roots.push(cause_obj);
        }
        rt.enqueue_host_phase_rooted(
            HostEnqueuePhase::HostCompletionMacrotask,
            "node:child_process.execFile.abort",
            roots,
            move |rt| {
                let err = abort_error_value(rt, cause.clone());
                let empty = js_string("");
                let _ = rt.call_function(
                    cb_v.clone(),
                    Value::Undefined,
                    vec![err, empty.clone(), empty],
                );
                Ok(())
            },
        );
    }

    child
}

fn invalid_arg_type_error(rt: &mut Runtime, prefix: &str, value: &Value) -> RuntimeError {
    let msg = format!("{prefix}{}", invalid_arg_suffix(rt, value));
    node_code_type_error(rt, "ERR_INVALID_ARG_TYPE", &msg)
}

fn is_array_like(rt: &Runtime, value: &Value) -> bool {
    matches!(value, Value::Object(id) if matches!(rt.object_get(*id, "length"), Value::Number(_)))
}

fn child_process_arg_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.to_string()),
        Value::Number(_) | Value::Boolean(_) | Value::Null | Value::Undefined => Some(
            rusty_js_runtime::abstract_ops::to_string(value)
                .as_ref()
                .clone(),
        ),
        _ => None,
    }
}

fn read_argv_array(rt: &mut Runtime, arr: ObjectRef) -> Vec<String> {
    let len = rt.array_length(arr);
    let mut out = Vec::new();
    for i in 0..len {
        if let Some(arg) = child_process_arg_to_string(&rt.object_get(arr, &i.to_string())) {
            out.push(arg);
        }
    }
    out
}

fn copy_child_process_shape(rt: &mut Runtime, target: ObjectRef, spawned: ObjectRef) {
    for name in [
        "pid",
        "stdin",
        "stdout",
        "stderr",
        "stdio",
        "kill",
        "ref",
        "unref",
        "__child_id",
    ] {
        let value = rt.object_get(spawned, name);
        if !matches!(value, Value::Undefined) {
            if name.starts_with("__") {
                rt.set_engine_sentinel(target, name, value);
            } else {
                rt.object_set(target, name.into(), value);
            }
        }
    }
}

fn read_timeout_ms(rt: &Runtime, opts: Option<&Value>) -> Option<u64> {
    let Some(Value::Object(opts_obj)) = opts else {
        return None;
    };
    match rt.object_get(*opts_obj, "timeout") {
        Value::Number(n) if n.is_finite() && n > 0.0 => Some(n as u64),
        _ => None,
    }
}

fn read_kill_signal(rt: &Runtime, opts: Option<&Value>) -> Option<String> {
    let Some(Value::Object(opts_obj)) = opts else {
        return None;
    };
    match rt.object_get(*opts_obj, "killSignal") {
        Value::String(s) => Some(s.to_string()),
        _ => None,
    }
}

pub fn install(rt: &mut Runtime) {
    let ns = new_object(rt);

    register_method(rt, ns, "execSync", |rt, args| {
        let command = match args.first() {
            Some(Value::String(s)) => s.to_string(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "execSync: command must be a string".into(),
                ))
            }
        };
        crate::spawn::require_spawn(rt, &command)?;
        let opts = args.get(1);
        let cwd = read_cwd(rt, opts);
        let env = read_env(rt, opts);
        let input = read_input_bytes(rt, opts);
        let max_buffer = read_max_buffer(rt, opts);
        let o = crate::spawn::run_sync_with_options(
            &command,
            &[],
            cwd.as_deref(),
            true,
            None,
            None,
            if env.is_empty() { None } else { Some(&env) },
            input.as_deref(),
        )
        .map_err(RuntimeError::TypeError)?;
        if sync_output_exceeds_max_buffer(&o, max_buffer) {
            return Err(child_process_enobufs_error(
                rt, "/bin/sh", &o.stdout, &o.stderr,
            ));
        }
        if o.code != Some(0) {
            let code = o.code.unwrap_or(-1);
            return Err(exec_sync_failure_error(
                rt,
                &command,
                code,
                o.signal.as_deref(),
                &o.stdout,
                &o.stderr,
            ));
        }
        Ok(sync_stdout_value(rt, &o.stdout, opts))
    });

    register_method(rt, ns, "spawnSync", |rt, args| {
        let command = match args.first() {
            Some(Value::String(s)) => s.to_string(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "spawnSync: command must be a string".into(),
                ))
            }
        };
        crate::spawn::require_spawn(rt, &command)?;
        let mut arglist = Vec::new();
        let mut opts_idx = 1;
        if let Some(Value::Object(arr)) = args.get(1) {
            if matches!(rt.object_get(*arr, "length"), Value::Number(_)) {
                arglist = read_argv_array(rt, *arr);
                opts_idx = 2;
            }
        }
        let cwd = read_cwd(rt, args.get(opts_idx));
        let env = read_env(rt, args.get(opts_idx));
        let input = read_input_bytes(rt, args.get(opts_idx));
        let timeout_ms = read_timeout_ms(rt, args.get(opts_idx));
        let kill_signal = read_kill_signal(rt, args.get(opts_idx));

        let shell = match args.get(opts_idx) {
            Some(Value::Object(id)) => matches!(
                rt.object_get(*id, "shell"),
                Value::Boolean(true) | Value::String(_)
            ),
            _ => false,
        };
        let (run_command, run_args): (String, Vec<String>) = if shell {
            let full = if arglist.is_empty() {
                command.clone()
            } else {
                format!("{} {}", command, arglist.join(" "))
            };
            (full, Vec::new())
        } else {
            (command.clone(), arglist.clone())
        };
        let opts_val = args.get(opts_idx).cloned();
        let result = new_object(rt);
        let _result_root = rt.push_temporary_value_roots(&[Value::Object(result)]);
        match crate::spawn::run_sync_with_options(
            &run_command,
            &run_args,
            cwd.as_deref(),
            shell,
            timeout_ms,
            kill_signal.as_deref(),
            if env.is_empty() { None } else { Some(&env) },
            input.as_deref(),
        ) {
            Ok(o) => {
                rt.object_set(
                    result,
                    "status".into(),
                    match o.code {
                        Some(c) => Value::Number(c as f64),
                        None => Value::Null,
                    },
                );

                let stdout_val = sync_stdout_value(rt, &o.stdout, opts_val.as_ref());
                let stderr_val = sync_stdout_value(rt, &o.stderr, opts_val.as_ref());
                rt.object_set(result, "stdout".into(), stdout_val.clone());
                rt.object_set(result, "stderr".into(), stderr_val.clone());
                let output = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
                rt.object_set(output, "0".into(), Value::Null);
                rt.object_set(output, "1".into(), stdout_val);
                rt.object_set(output, "2".into(), stderr_val);
                rt.object_set(output, "length".into(), Value::Number(3.0));
                rt.object_set(result, "output".into(), Value::Object(output));
                rt.object_set(result, "pid".into(), Value::Number(0.0));
                rt.object_set(
                    result,
                    "signal".into(),
                    o.signal
                        .as_ref()
                        .map(|s| {
                            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                                s.clone(),
                            )))
                        })
                        .unwrap_or(Value::Null),
                );
                if o.timed_out {
                    rt.object_set(
                        result,
                        "error".into(),
                        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                            "spawnSync ETIMEDOUT".to_string(),
                        ))),
                    );
                }

                if sync_output_exceeds_max_buffer(&o, read_max_buffer(rt, opts_val.as_ref())) {
                    let cmd = if shell { "/bin/sh" } else { command.as_str() };
                    let err = enobufs_error_value(rt, cmd);
                    rt.object_set(result, "error".into(), err);
                    rt.object_set(result, "status".into(), Value::Null);
                    rt.object_set(result, "signal".into(), js_string("SIGTERM"));
                }
            }
            Err(e) => {

                rt.object_set(result, "status".into(), Value::Null);
                rt.object_set(result, "signal".into(), Value::Null);
                rt.object_set(result, "pid".into(), Value::Number(0.0));
                rt.object_set(result, "output".into(), Value::Null);
                let is_enoent =
                    e.contains("No such file") || e.contains("os error 2") || e.contains("ENOENT");
                let err_val = if is_enoent {
                    let msg = format!("spawnSync {run_command} ENOENT");
                    match rusty_js_runtime::intrinsics::make_error_instance(rt, "Error", &msg) {
                        Some(id) => {
                            rt.object_set(id, "code".into(), js_string("ENOENT"));
                            rt.object_set(id, "errno".into(), Value::Number(-2.0));
                            rt.object_set(
                                id,
                                "syscall".into(),
                                js_string(&format!("spawnSync {run_command}")),
                            );
                            rt.object_set(id, "path".into(), js_string(&run_command));
                            Value::Object(id)
                        }
                        None => js_string(&msg),
                    }
                } else {
                    match rusty_js_runtime::intrinsics::make_error_instance(rt, "Error", &e) {
                        Some(id) => Value::Object(id),
                        None => js_string(&e),
                    }
                };
                rt.object_set(result, "error".into(), err_val);
            }
        }
        Ok(Value::Object(result))
    });

    register_method(rt, ns, "spawn", |rt, args| {
        let command = match args.first() {
            Some(Value::String(s)) => s.to_string(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "spawn: command must be a string".into(),
                ))
            }
        };
        let mut arglist = Vec::new();
        let mut opts_idx = 1;
        if let Some(Value::Object(arr)) = args.get(1) {
            if matches!(rt.object_get(*arr, "length"), Value::Number(_)) {
                arglist = read_argv_array(rt, *arr);
                opts_idx = 2;
            }
        }
        let (cwd, env, shell, stdio) = read_opts(rt, args.get(opts_idx));
        let child = match crate::spawn::spawn_child(
            rt,
            &command,
            &arglist,
            cwd.as_deref(),
            if env.is_empty() { None } else { Some(&env) },
            shell,
            false,
            None,
            None,
            None,
            stdio,
        ) {
            Ok(c) => c,

            Err(e) if is_spawn_enoent(&e) => failed_spawn_child(rt, &command, "ENOENT", -2),
            Err(e) => return Err(e),
        };
        Ok(Value::Object(child))
    });

    register_method(rt, ns, "exec", |rt, args| {
        let command = match args.first() {
            Some(Value::String(s)) => s.to_string(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "exec: command must be a string".into(),
                ))
            }
        };
        let cb = args.iter().rev().cloned().find(|v| rt.is_callable(v));
        let opts = args.get(1);
        let (cwd, env, _shell, _) = read_opts(rt, opts);
        let timeout_ms = read_timeout_ms(rt, opts);
        let kill_signal = read_kill_signal(rt, opts);
        let child = crate::spawn::spawn_child(
            rt,
            &command,
            &[],
            cwd.as_deref(),
            if env.is_empty() { None } else { Some(&env) },
            true,
            true,
            cb.clone(),
            timeout_ms,
            kill_signal.as_deref(),
            crate::spawn::StdioConfig::PIPED,
        )?;
        Ok(Value::Object(child))
    });

    register_method(rt, ns, "execFile", |rt, args| {
        let file = match args.first() {
            Some(Value::String(s)) => s.to_string(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "execFile: file must be a string".into(),
                ))
            }
        };
        let mut arglist = Vec::new();
        let mut opts_idx = 1;
        if let Some(Value::Object(arr)) = args.get(1) {
            if matches!(rt.object_get(*arr, "length"), Value::Number(_)) {
                arglist = read_argv_array(rt, *arr);
                opts_idx = 2;
            }
        }
        let cb = args.iter().rev().cloned().find(|v| rt.is_callable(v));
        let opts = args.get(opts_idx);
        let signal = signal_option(rt, opts)?;
        if let Some(sig) = signal {
            if matches!(rt.object_get(sig, "aborted"), Value::Boolean(true)) {
                let cause = rt.object_get(sig, "reason");
                let child = failed_execfile_abort_child(rt, cb, cause);
                return Ok(Value::Object(child));
            }
        }
        let (cwd, env, shell, _) = read_opts(rt, opts);
        if shell && !arglist.is_empty() {
            emit_execfile_shell_args_warning(rt);
        }
        let signal_state = cb.as_ref().filter(|v| rt.is_callable(v)).map(|_| {
            let state = new_object(rt);
            rt.object_set(state, "aborted".into(), Value::Boolean(false));
            state
        });
        let cb = if let Some(cb_v) = cb.filter(|v| rt.is_callable(v)) {
            let state = signal_state.expect("signal_state exists for callable callback");
            Some(Value::Object(make_callable_rooted(
                rt,
                "execFileCbSignalGate",
                vec![state],
                move |rt, cb_args| {
                    if matches!(rt.object_get(state, "aborted"), Value::Boolean(true)) {
                        return Ok(Value::Undefined);
                    }
                    let signal = rt.object_get(state, "signal");
                    let listener = rt.object_get(state, "listener");
                    if let (Value::Object(signal), Value::Object(listener)) = (signal, listener) {
                        let remove = rt.object_get(signal, "removeEventListener");
                        if rt.is_callable(&remove) {
                            let _ = rt.call_function(
                                remove,
                                Value::Object(signal),
                                vec![js_string("abort"), Value::Object(listener)],
                            );
                        }
                    }
                    let err = cb_args.first().cloned().unwrap_or(Value::Null);
                    let stdout = cb_args.get(1).cloned().unwrap_or(Value::Undefined);
                    let stderr = cb_args.get(2).cloned().unwrap_or(Value::Undefined);
                    let _ =
                        rt.call_function(cb_v.clone(), Value::Undefined, vec![err, stdout, stderr]);
                    Ok(Value::Undefined)
                },
            )))
        } else {
            None
        };
        let child = match crate::spawn::spawn_child(
            rt,
            &file,
            &arglist,
            cwd.as_deref(),
            if env.is_empty() { None } else { Some(&env) },
            shell,
            true,
            cb.clone(),
            read_timeout_ms(rt, opts),
            read_kill_signal(rt, opts).as_deref(),
            crate::spawn::StdioConfig::PIPED,
        ) {
            Ok(child) => child,
            Err(err) if is_spawn_enoent(&err) => failed_execfile_child(rt, &file, cb.clone()),
            Err(err) => return Err(err),
        };
        if let Some(sig) = signal {
            if let (Some(Value::Object(cb_obj)), Some(state)) = (cb.clone(), signal_state) {
                let listener = make_callable_rooted(
                    rt,
                    "execFileAbort",
                    vec![child, sig, cb_obj, state],
                    move |rt, _| {
                        if matches!(rt.object_get(state, "aborted"), Value::Boolean(true)) {
                            return Ok(Value::Undefined);
                        }
                        let cause = rt.object_get(sig, "reason");
                        let err = abort_error_value(rt, cause);
                        let empty = js_string("");
                        let _ = rt.call_function(
                            Value::Object(cb_obj),
                            Value::Undefined,
                            vec![err, empty.clone(), empty],
                        );
                        rt.object_set(state, "aborted".into(), Value::Boolean(true));
                        let kill = rt.object_get(child, "kill");
                        if rt.is_callable(&kill) {
                            let _ = rt.call_function(kill, Value::Object(child), Vec::new());
                        }
                        Ok(Value::Undefined)
                    },
                );
                let add = rt.object_get(sig, "addEventListener");
                let _ = rt.call_function(
                    add,
                    Value::Object(sig),
                    vec![js_string("abort"), Value::Object(listener)],
                );
                rt.object_set(state, "signal".into(), Value::Object(sig));
                rt.object_set(state, "listener".into(), Value::Object(listener));
            }
        }
        Ok(Value::Object(child))
    });

    attach_exec_promisify(rt, ns, "exec", true);
    attach_exec_promisify(rt, ns, "execFile", false);

    register_method(rt, ns, "execFileSync", |rt, args| {
        let file = match args.first() {
            Some(Value::String(s)) => s.to_string(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "execFileSync: file must be a string".into(),
                ))
            }
        };
        crate::spawn::require_spawn(rt, &file)?;
        let mut arglist = Vec::new();
        let mut opts_idx = 1;
        if let Some(Value::Object(arr)) = args.get(1) {
            if matches!(rt.object_get(*arr, "length"), Value::Number(_)) {
                arglist = read_argv_array(rt, *arr);
                opts_idx = 2;
            }
        }
        let opts = args.get(opts_idx);
        let cwd = read_cwd(rt, opts);
        let env = read_env(rt, opts);
        let input = read_input_bytes(rt, opts);
        let max_buffer = read_max_buffer(rt, opts);
        let o = crate::spawn::run_sync_with_options(
            &file,
            &arglist,
            cwd.as_deref(),
            false,
            None,
            None,
            if env.is_empty() { None } else { Some(&env) },
            input.as_deref(),
        )
        .map_err(RuntimeError::TypeError)?;
        if sync_output_exceeds_max_buffer(&o, max_buffer) {
            return Err(child_process_enobufs_error(rt, &file, &o.stdout, &o.stderr));
        }
        Ok(sync_stdout_value(rt, &o.stdout, opts))
    });

    register_method(rt, ns, "fork", |rt, args| {
        let module = match args.first() {
            Some(Value::String(s)) => s.to_string(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "fork: modulePath must be a string".into(),
                ))
            }
        };
        let mut arglist = Vec::new();
        let mut opts_idx = 1;
        if let Some(Value::Object(arr)) = args.get(1) {
            if matches!(rt.object_get(*arr, "length"), Value::Number(_)) {
                let len = rt.array_length(*arr);
                for i in 0..len {
                    if let Value::String(s) = rt.object_get(*arr, &i.to_string()) {
                        arglist.push(s.to_string());
                    }
                }
                opts_idx = 2;
            }
        }
        let cwd = read_cwd(rt, args.get(opts_idx));
        let env = read_env(rt, args.get(opts_idx));
        crate::spawn::require_spawn(rt, &module)?;
        crate::ipc::fork_child(rt, &module, &arglist, cwd.as_deref(), &env).map(Value::Object)
    });

    let child_process_ctor =
        crate::register::make_callable(rt, "ChildProcess", |rt, _a| Ok(rt.current_this()));
    let cp_proto = new_object(rt);
    rt.object_set(
        cp_proto,
        "constructor".into(),
        Value::Object(child_process_ctor),
    );
    register_method(rt, cp_proto, "spawn", |rt, args| {
        let Some(options) = args.first() else {
            return Err(invalid_arg_type_error(
                rt,
                "The \"options\" argument must be of type object.",
                &Value::Undefined,
            ));
        };
        let Value::Object(options_obj) = options else {
            return Err(invalid_arg_type_error(
                rt,
                "The \"options\" argument must be of type object.",
                options,
            ));
        };

        let file = rt.object_get(*options_obj, "file");
        let Value::String(file_name) = file else {
            return Err(invalid_arg_type_error(
                rt,
                "The \"options.file\" property must be of type string.",
                &file,
            ));
        };

        let env_pairs = rt.object_get(*options_obj, "envPairs");
        if !matches!(env_pairs, Value::Undefined) && !is_array_like(rt, &env_pairs) {
            return Err(invalid_arg_type_error(
                rt,
                "The \"options.envPairs\" property must be an instance of Array.",
                &env_pairs,
            ));
        }

        let args_value = rt.object_get(*options_obj, "args");
        if !matches!(args_value, Value::Undefined) && !is_array_like(rt, &args_value) {
            return Err(invalid_arg_type_error(
                rt,
                "The \"options.args\" property must be an instance of Array.",
                &args_value,
            ));
        }

        let mut arglist = Vec::new();
        if let Value::Object(arr) = args_value {
            let len = rt.array_length(arr);
            for i in 0..len {
                if let Value::String(s) = rt.object_get(arr, &i.to_string()) {
                    arglist.push(s.as_str().to_string());
                }
            }
        }

        let cwd = match rt.object_get(*options_obj, "cwd") {
            Value::String(s) => Some(s.as_str().to_string()),
            _ => None,
        };
        crate::spawn::require_spawn(rt, file_name.as_str())?;
        let spawned = crate::spawn::spawn_child(
            rt,
            file_name.as_str(),
            &arglist,
            cwd.as_deref(),
            None,
            false,
            false,
            None,
            None,
            None,
            crate::spawn::StdioConfig::PIPED,
        )?;
        if let Value::Object(this) = rt.current_this() {
            copy_child_process_shape(rt, this, spawned);
        }
        Ok(Value::Undefined)
    });
    rt.object_set(
        child_process_ctor,
        "prototype".into(),
        Value::Object(cp_proto),
    );
    rt.object_set(ns, "ChildProcess".into(), Value::Object(child_process_ctor));

    register_method(rt, ns, "_forkChild", |_rt, _a| Ok(Value::Undefined));

    rt.define_global_property("child_process", Value::Object(ns));
}

fn read_opts(
    rt: &mut Runtime,
    opts: Option<&Value>,
) -> (
    Option<String>,
    Vec<(String, String)>,
    bool,
    crate::spawn::StdioConfig,
) {
    if let Some(Value::Object(o)) = opts {
        let cwd = match rt.object_get(*o, "cwd") {
            Value::String(c) => Some(c.to_string()),
            _ => None,
        };
        let env = read_env(rt, opts);
        let shell = matches!(rt.object_get(*o, "shell"), Value::Boolean(true));
        let stdio_all_ignore =
            matches!(rt.object_get(*o, "stdio"), Value::String(s) if s.as_str() == "ignore");
        let is_ignore = |name: &str| {
            stdio_all_ignore
                || matches!(rt.object_get(*o, name), Value::String(s) if s.as_str() == "ignore")
        };
        let stdio = crate::spawn::StdioConfig {
            stdin_ignore: is_ignore("stdin"),
            stdout_ignore: is_ignore("stdout"),
            stderr_ignore: is_ignore("stderr"),
        };
        (cwd, env, shell, stdio)
    } else {
        (None, Vec::new(), false, crate::spawn::StdioConfig::PIPED)
    }
}
